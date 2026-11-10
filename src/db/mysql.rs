//! Driver de MySQL y MariaDB (`mysql_async` + rustls). Protocolo de texto: los valores llegan como bytes y se
//! muestran tal cual. La cancelación es `KILL QUERY` desde una segunda conexión.

use super::driver::{
    race, CancelToken, Cell, Collector, ConnectParams, DbError, ResultSet, CANCEL_GRACE,
};
use super::safety::QueryLimits;
use super::store::SslMode;
use super::tls::{self, KeyFormat};
use mysql_async::consts::ColumnType;
use mysql_async::prelude::Queryable;
use mysql_async::{ClientIdentity, Conn, Opts, OptsBuilder, SslOpts, Value};
use std::time::Instant;

pub(crate) struct MyConn {
    conn: Conn,
    opts: Opts,
    /// Tras una interrupción o un límite de filas la conexión puede tener resultados sin leer: se reconecta
    dirty: bool,
}

fn ssl_opts(params: &ConnectParams) -> Result<Option<SslOpts>, DbError> {
    let tls_settings = &params.tls;
    if tls_settings.mode == SslMode::Disable {
        return Ok(None);
    }
    let mut ssl = SslOpts::default();
    if let Some(ca) = &tls_settings.ca_path {
        ssl = ssl
            .with_root_certs(vec![ca.clone().into()])
            .with_disable_built_in_roots(true);
    }
    ssl = match tls_settings.mode {
        SslMode::Require => ssl
            .with_danger_accept_invalid_certs(true)
            .with_danger_skip_domain_validation(true),
        SslMode::VerifyCa => ssl.with_danger_skip_domain_validation(true),
        SslMode::VerifyFull | SslMode::Disable => ssl,
    };
    if params.tls_name != params.dial_host {
        ssl = ssl.with_danger_tls_hostname_override(Some(params.tls_name.clone()));
    }
    match (
        &tls_settings.client_cert_path,
        &tls_settings.client_key_path,
    ) {
        (None, None) => {}
        (Some(cert), Some(key)) => {
            // Una clave sin cifrar la lee el driver del propio archivo; una cifrada se descifra solo en memoria
            let key_source = match tls::key_format(key)? {
                KeyFormat::Pkcs8 | KeyFormat::Pkcs1 | KeyFormat::Sec1 => key.clone().into(),
                KeyFormat::EncryptedPkcs8 | KeyFormat::LegacyEncrypted => {
                    tls::client_key_pem(key, params.key_passphrase.as_ref())?.into()
                }
            };
            ssl = ssl
                .with_client_identity(Some(ClientIdentity::new(cert.clone().into(), key_source)));
        }
        _ => return Err(tls::TlsError::IncompleteClientIdentity.into()),
    }
    Ok(Some(ssl))
}

impl MyConn {
    pub(crate) async fn connect(params: &ConnectParams) -> Result<Self, DbError> {
        let opts: Opts = OptsBuilder::default()
            .ip_or_hostname(params.dial_host.clone())
            .tcp_port(params.dial_port)
            .user(Some(params.user.clone()))
            .pass(params.password.as_ref().map(|p| p.expose().to_string()))
            .db_name((!params.database.is_empty()).then(|| params.database.clone()))
            .ssl_opts(ssl_opts(params)?)
            .prefer_socket(false)
            .into();
        let conn = tokio::time::timeout(params.connect_timeout, Conn::new(opts.clone()))
            .await
            .map_err(|_| DbError::Connect("tiempo de espera agotado".to_string()))?
            .map_err(DbError::connect)?;
        Ok(Self {
            conn,
            opts,
            dirty: false,
        })
    }

    pub(crate) fn is_healthy(&self) -> bool {
        !self.dirty
    }

    pub(crate) async fn query(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ResultSet, DbError> {
        let started = Instant::now();
        let id = self.conn.id();
        let outcome = race(
            read_first_set(&mut self.conn, sql, limits.max_rows),
            limits,
            cancel,
        )
        .await;
        match outcome {
            Ok(Ok((collector, stopped_early))) => {
                if stopped_early {
                    self.dirty = true;
                    self.kill_query(id).await;
                }
                Ok(collector.finish(started.elapsed()))
            }
            Ok(Err(error)) => Err(error),
            Err(interrupt) => {
                self.dirty = true;
                self.kill_query(id).await;
                Err(interrupt.into())
            }
        }
    }

    /// `KILL QUERY` desde otra conexión (mejor esfuerzo y acotado)
    async fn kill_query(&self, id: u32) {
        let killing = async {
            let mut killer = Conn::new(self.opts.clone()).await?;
            killer.query_drop(format!("KILL QUERY {id}")).await?;
            killer.disconnect().await
        };
        let _ = tokio::time::timeout(CANCEL_GRACE, killing).await;
    }
}

async fn read_first_set(
    conn: &mut Conn,
    sql: &str,
    max_rows: usize,
) -> Result<(Collector, bool), DbError> {
    let mut result = conn.query_iter(sql).await.map_err(DbError::server)?;
    let mut collector = Collector::new(max_rows);
    let columns = result.columns();
    match columns.as_deref() {
        Some(columns) if !columns.is_empty() => {
            collector.set_columns(columns.iter().map(|c| c.name_str().to_string()).collect());
            let types: Vec<ColumnType> = columns.iter().map(|c| c.column_type()).collect();
            while let Some(row) = result.next().await.map_err(DbError::server)? {
                let cells = (0..row.len())
                    .map(|i| {
                        let ty = types
                            .get(i)
                            .copied()
                            .unwrap_or(ColumnType::MYSQL_TYPE_STRING);
                        Cell::new(row.as_ref(i).and_then(|value| value_text(value, ty)))
                    })
                    .collect();
                if !collector.push(cells) {
                    return Ok((collector, true));
                }
            }
        }
        _ => collector.set_affected(result.affected_rows()),
    }
    result.drop_result().await.map_err(DbError::server)?;
    Ok((collector, false))
}

fn micros_suffix(micros: u32) -> String {
    if micros == 0 {
        String::new()
    } else {
        format!(".{micros:06}")
    }
}

fn value_text(value: &Value, column_type: ColumnType) -> Option<String> {
    Some(match value {
        Value::NULL => return None,
        Value::Bytes(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        Value::Int(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::Date(year, month, day, hour, minute, second, micros) => {
            let date_only = matches!(
                column_type,
                ColumnType::MYSQL_TYPE_DATE | ColumnType::MYSQL_TYPE_NEWDATE
            );
            if date_only {
                format!("{year:04}-{month:02}-{day:02}")
            } else {
                format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}{}",
                    micros_suffix(*micros)
                )
            }
        }
        Value::Time(negative, days, hours, minutes, seconds, micros) => {
            let total_hours = u32::from(*hours) + days * 24;
            format!(
                "{}{total_hours:02}:{minutes:02}:{seconds:02}{}",
                if *negative { "-" } else { "" },
                micros_suffix(*micros)
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_rendered_as_text() {
        let t = ColumnType::MYSQL_TYPE_VAR_STRING;
        assert_eq!(value_text(&Value::NULL, t), None);
        assert_eq!(value_text(&Value::Int(-4), t).as_deref(), Some("-4"));
        assert_eq!(
            value_text(&Value::Bytes(b"hola".to_vec()), t).as_deref(),
            Some("hola")
        );
        assert_eq!(
            value_text(
                &Value::Date(2026, 11, 9, 13, 5, 7, 250),
                ColumnType::MYSQL_TYPE_DATETIME
            )
            .as_deref(),
            Some("2026-11-09 13:05:07.000250")
        );
        assert_eq!(
            value_text(
                &Value::Date(2026, 11, 9, 0, 0, 0, 0),
                ColumnType::MYSQL_TYPE_DATE
            )
            .as_deref(),
            Some("2026-11-09")
        );
        assert_eq!(
            value_text(
                &Value::Time(true, 1, 2, 3, 4, 0),
                ColumnType::MYSQL_TYPE_TIME
            )
            .as_deref(),
            Some("-26:03:04")
        );
    }
}
