//! Driver de SQL Server (`tiberius`). TLS con Schannel (el almacén de Windows), que es lo que enlaza `tiberius`
//! sin arrastrar `aws-lc`. Limitaciones frente a PostgreSQL y MySQL:
//! * no hay certificado de cliente (TDS no lo admite en este driver);
//! * `verify-ca` verifica también el nombre del servidor (Schannel no permite separar ambas cosas);
//! * no hay modo de solo lectura por sesión: protege solo el clasificador de SQL;
//! * no hay cancelación en el servidor: al cancelar se cierra la conexión (el servidor aborta la sentencia).

use super::driver::{race, CancelToken, Cell, Collector, ConnectParams, DbError, ResultSet};
use super::safety::QueryLimits;
use super::store::SslMode;
use futures_util::TryStreamExt;
use std::time::Instant;
use tiberius::{AuthMethod, Client, ColumnData, Config, EncryptionLevel, QueryItem};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub(crate) struct MsConn {
    client: Client<Compat<TcpStream>>,
    /// Con resultados sin leer o interrumpida: hay que cerrarla y reconectar
    dirty: bool,
}

fn config(params: &ConnectParams) -> Result<Config, DbError> {
    if params.tls.client_cert_path.is_some() || params.tls.client_key_path.is_some() {
        return Err(DbError::Unsupported(
            "SQL Server no admite certificado de cliente: quita el certificado y la clave de la pestaña SSL / TLS",
        ));
    }
    let mut config = Config::new();
    config.host(&params.dial_host);
    config.port(params.dial_port);
    if params.tls_name != params.dial_host {
        config.hostname_in_certificate(&params.tls_name);
    }
    if !params.database.is_empty() {
        config.database(&params.database);
    }
    config.application_name("TEArminal");
    config.authentication(AuthMethod::sql_server(
        &params.user,
        params
            .password
            .as_ref()
            .map(|p| p.expose())
            .unwrap_or_default(),
    ));
    match params.tls.mode {
        SslMode::Disable => config.encryption(EncryptionLevel::NotSupported),
        SslMode::Require => {
            config.encryption(EncryptionLevel::Required);
            config.trust_cert();
        }
        SslMode::VerifyCa | SslMode::VerifyFull => {
            config.encryption(EncryptionLevel::Required);
            if let Some(ca) = &params.tls.ca_path {
                config.trust_cert_ca(ca.to_string_lossy());
            }
        }
    }
    Ok(config)
}

impl MsConn {
    pub(crate) async fn connect(params: &ConnectParams) -> Result<Self, DbError> {
        let config = config(params)?;
        let dial = (params.dial_host.as_str(), params.dial_port);
        let connecting = async {
            let tcp = TcpStream::connect(dial).await.map_err(DbError::connect)?;
            tcp.set_nodelay(true).map_err(DbError::connect)?;
            Client::connect(config, tcp.compat_write())
                .await
                .map_err(DbError::connect)
        };
        let client = tokio::time::timeout(params.connect_timeout, connecting)
            .await
            .map_err(|_| DbError::Connect("tiempo de espera agotado".to_string()))??;
        Ok(Self {
            client,
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
        let outcome = race(
            read_first_set(&mut self.client, sql, limits.max_rows),
            limits,
            cancel,
        )
        .await;
        match outcome {
            Ok(Ok((collector, stopped_early))) => {
                self.dirty = stopped_early;
                Ok(collector.finish(started.elapsed()))
            }
            Ok(Err(error)) => Err(error),
            Err(interrupt) => {
                self.dirty = true;
                Err(interrupt.into())
            }
        }
    }
}

async fn read_first_set(
    client: &mut Client<Compat<TcpStream>>,
    sql: &str,
    max_rows: usize,
) -> Result<(Collector, bool), DbError> {
    let mut stream = client.simple_query(sql).await.map_err(DbError::server)?;
    let mut collector = Collector::new(max_rows);
    let mut seen_set = false;
    while let Some(item) = stream.try_next().await.map_err(DbError::server)? {
        match item {
            QueryItem::Metadata(meta) => {
                if seen_set {
                    // Un segundo conjunto de resultados: se queda sin leer (la conexión se reconectará)
                    return Ok((collector, true));
                }
                seen_set = true;
                collector.set_columns(
                    meta.columns()
                        .iter()
                        .map(|c| c.name().to_string())
                        .collect(),
                );
            }
            QueryItem::Row(row) => {
                let cells = row
                    .cells()
                    .map(|(_, data)| Cell::new(column_text(data)))
                    .collect();
                if !collector.push(cells) {
                    return Ok((collector, true));
                }
            }
        }
    }
    Ok((collector, false))
}

// -------------------------------------------------------------------------------------------------
// Valores a texto
// -------------------------------------------------------------------------------------------------

/// Año, mes y día de un número de días desde 1970-01-01 (calendario gregoriano proléptico)
fn civil_from_unix_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Días entre 1970-01-01 y 0001-01-01 (`DATE` de SQL Server cuenta desde 0001-01-01)
const DAYS_0001_TO_1970: i64 = 719_162;
/// Días entre 1970-01-01 y 1900-01-01 (`DATETIME` y `SMALLDATETIME` cuentan desde 1900-01-01)
const DAYS_1900_TO_1970: i64 = 25_567;

fn format_date(unix_days: i64) -> String {
    let (year, month, day) = civil_from_unix_days(unix_days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn format_clock(total_seconds: u64, fraction: Option<(u64, u8)>) -> String {
    let base = format!(
        "{:02}:{:02}:{:02}",
        total_seconds / 3600,
        (total_seconds / 60) % 60,
        total_seconds % 60
    );
    match fraction {
        Some((value, digits)) if digits > 0 => {
            format!("{base}.{value:0width$}", width = digits as usize)
        }
        _ => base,
    }
}

fn format_time(time: tiberius::time::Time) -> String {
    let scale = time.scale().min(7);
    let divisor = 10u64.pow(u32::from(scale));
    let increments = time.increments();
    format_clock(increments / divisor, Some((increments % divisor, scale)))
}

fn format_datetime2(value: tiberius::time::DateTime2) -> String {
    format!(
        "{} {}",
        format_date(i64::from(value.date().days()) - DAYS_0001_TO_1970),
        format_time(value.time())
    )
}

fn column_text(data: &ColumnData<'_>) -> Option<String> {
    match data {
        ColumnData::U8(v) => v.map(|n| n.to_string()),
        ColumnData::I16(v) => v.map(|n| n.to_string()),
        ColumnData::I32(v) => v.map(|n| n.to_string()),
        ColumnData::I64(v) => v.map(|n| n.to_string()),
        ColumnData::F32(v) => v.map(|n| n.to_string()),
        ColumnData::F64(v) => v.map(|n| n.to_string()),
        ColumnData::Bit(v) => v.map(|b| if b { "1" } else { "0" }.to_string()),
        ColumnData::String(v) => v.as_ref().map(ToString::to_string),
        ColumnData::Guid(v) => v.map(|g| g.to_string()),
        ColumnData::Binary(v) => v.as_ref().map(|bytes| {
            let mut out = String::from("0x");
            for byte in bytes.iter() {
                out.push_str(&format!("{byte:02X}"));
            }
            out
        }),
        ColumnData::Numeric(v) => v.map(|n| n.to_string()),
        ColumnData::Xml(v) => v.as_ref().map(|xml| xml.as_ref().to_string()),
        ColumnData::DateTime(v) => v.map(|dt| {
            // Fracciones de 1/300 de segundo
            let seconds = u64::from(dt.seconds_fragments()) / 300;
            let millis = (u64::from(dt.seconds_fragments()) % 300) * 10 / 3;
            format!(
                "{} {}",
                format_date(i64::from(dt.days()) - DAYS_1900_TO_1970),
                format_clock(seconds, Some((millis, 3)))
            )
        }),
        ColumnData::SmallDateTime(v) => v.map(|dt| {
            format!(
                "{} {}",
                format_date(i64::from(dt.days()) - DAYS_1900_TO_1970),
                format_clock(u64::from(dt.seconds_fragments()) * 60, None)
            )
        }),
        ColumnData::Time(v) => v.map(format_time),
        ColumnData::Date(v) => v.map(|d| format_date(i64::from(d.days()) - DAYS_0001_TO_1970)),
        ColumnData::DateTime2(v) => v.map(format_datetime2),
        ColumnData::DateTimeOffset(v) => v.map(|value| {
            let offset = value.offset();
            let sign = if offset < 0 { '-' } else { '+' };
            let minutes = offset.unsigned_abs();
            format!(
                "{} {sign}{:02}:{:02}",
                format_datetime2(value.datetime2()),
                minutes / 60,
                minutes % 60
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiberius::time::{Date, DateTime, DateTime2, SmallDateTime, Time};

    #[test]
    fn civil_dates_match_known_days() {
        assert_eq!(civil_from_unix_days(0), (1970, 1, 1));
        assert_eq!(civil_from_unix_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_unix_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_unix_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_unix_days(-719_162), (1, 1, 1));
    }

    #[test]
    fn dates_and_times_are_formatted_like_sql_server_shows_them() {
        let unix_2024_01_01 = 19_723;
        let date = Date::new((unix_2024_01_01 + DAYS_0001_TO_1970) as u32);
        assert_eq!(
            column_text(&ColumnData::Date(Some(date))).as_deref(),
            Some("2024-01-01")
        );
        // 13:05:07.25 con escala 2
        let time = Time::new((13 * 3600 + 5 * 60 + 7) * 100 + 25, 2);
        assert_eq!(
            column_text(&ColumnData::Time(Some(time))).as_deref(),
            Some("13:05:07.25")
        );
        let dt2 = DateTime2::new(date, Time::new(0, 0));
        assert_eq!(
            column_text(&ColumnData::DateTime2(Some(dt2))).as_deref(),
            Some("2024-01-01 00:00:00")
        );
        let dt = DateTime::new((unix_2024_01_01 + DAYS_1900_TO_1970) as i32, 300 * 3661);
        assert_eq!(
            column_text(&ColumnData::DateTime(Some(dt))).as_deref(),
            Some("2024-01-01 01:01:01.000")
        );
        let small = SmallDateTime::new((unix_2024_01_01 + DAYS_1900_TO_1970) as u16, 61);
        assert_eq!(
            column_text(&ColumnData::SmallDateTime(Some(small))).as_deref(),
            Some("2024-01-01 01:01:00")
        );
    }

    #[test]
    fn nulls_numbers_bits_and_binary() {
        assert_eq!(column_text(&ColumnData::I32(None)), None);
        assert_eq!(
            column_text(&ColumnData::I64(Some(-7))).as_deref(),
            Some("-7")
        );
        assert_eq!(
            column_text(&ColumnData::Bit(Some(true))).as_deref(),
            Some("1")
        );
        assert_eq!(
            column_text(&ColumnData::Binary(Some(vec![0xde, 0xad].into()))).as_deref(),
            Some("0xDEAD")
        );
        assert_eq!(
            column_text(&ColumnData::String(Some("hola".into()))).as_deref(),
            Some("hola")
        );
    }
}
