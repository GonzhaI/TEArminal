//! Driver de PostgreSQL (`tokio-postgres` + rustls). Usa el protocolo de consulta simple: el servidor devuelve cada
//! valor ya como texto, sin decodificar tipos, y las filas llegan en flujo para respetar el límite.

use super::driver::{
    race, CancelToken, Cell, Collector, ConnectParams, DbError, ResultSet, CANCEL_GRACE,
};
use super::safety::QueryLimits;
use super::tls;
use futures_util::StreamExt;
use std::net::IpAddr;
use std::time::Instant;
use tokio_postgres::config::SslMode;
use tokio_postgres::{Client, Config, NoTls, SimpleQueryMessage};
use tokio_postgres_rustls::MakeRustlsConnect;

pub(crate) struct PgConn {
    client: Client,
    cancel_token: tokio_postgres::CancelToken,
    tls: Option<MakeRustlsConnect>,
}

/// Con túnel se marca el nombre del certificado como `host` y se conecta a la IP con `hostaddr`
fn pg_config(params: &ConnectParams) -> Config {
    let mut config = Config::new();
    match params.dial_host.parse::<IpAddr>() {
        Ok(ip) if params.tls_name != params.dial_host => {
            config.host(&params.tls_name).hostaddr(ip);
        }
        _ => {
            config.host(&params.dial_host);
        }
    }
    config
        .port(params.dial_port)
        .user(&params.user)
        .connect_timeout(params.connect_timeout)
        .application_name("TEArminal");
    if !params.database.is_empty() {
        config.dbname(&params.database);
    }
    if let Some(password) = &params.password {
        config.password(password.expose());
    }
    config
}

impl PgConn {
    pub(crate) async fn connect(params: &ConnectParams) -> Result<Self, DbError> {
        let rustls_config = tls::client_config(&params.tls, params.key_passphrase.as_ref())?;
        let mut config = pg_config(params);
        match rustls_config {
            None => {
                config.ssl_mode(SslMode::Disable);
                let (client, connection) = config.connect(NoTls).await.map_err(DbError::connect)?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                Ok(Self {
                    cancel_token: client.cancel_token(),
                    client,
                    tls: None,
                })
            }
            Some(rustls_config) => {
                config.ssl_mode(SslMode::Require);
                let tls = MakeRustlsConnect::new(rustls_config);
                let (client, connection) = config
                    .connect(tls.clone())
                    .await
                    .map_err(DbError::connect)?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                Ok(Self {
                    cancel_token: client.cancel_token(),
                    client,
                    tls: Some(tls),
                })
            }
        }
    }

    pub(crate) fn is_healthy(&self) -> bool {
        !self.client.is_closed()
    }

    pub(crate) async fn query(
        &self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ResultSet, DbError> {
        let started = Instant::now();
        match race(self.read_first_set(sql, limits.max_rows), limits, cancel).await {
            Ok(Ok((collector, stop_early))) => {
                if stop_early {
                    self.cancel_server_side().await;
                }
                Ok(collector.finish(started.elapsed()))
            }
            Ok(Err(error)) => Err(error),
            Err(interrupt) => {
                self.cancel_server_side().await;
                Err(interrupt.into())
            }
        }
    }

    /// Lee el primer conjunto de resultados. `true` si se dejó de leer por el límite de filas.
    async fn read_first_set(
        &self,
        sql: &str,
        max_rows: usize,
    ) -> Result<(Collector, bool), DbError> {
        let stream = self
            .client
            .simple_query_raw(sql)
            .await
            .map_err(DbError::server)?;
        let mut stream = std::pin::pin!(stream);
        let mut collector = Collector::new(max_rows);
        let mut has_columns = false;
        while let Some(message) = stream.next().await {
            match message.map_err(DbError::server)? {
                SimpleQueryMessage::RowDescription(columns) => {
                    has_columns = true;
                    collector.set_columns(columns.iter().map(|c| c.name().to_string()).collect());
                }
                SimpleQueryMessage::Row(row) => {
                    let cells = (0..row.len())
                        .map(|i| Cell::new(row.get(i).map(str::to_owned)))
                        .collect();
                    if !collector.push(cells) {
                        return Ok((collector, true));
                    }
                }
                SimpleQueryMessage::CommandComplete(count) => {
                    if !has_columns {
                        collector.set_affected(count);
                    }
                    break;
                }
                _ => {}
            }
        }
        Ok((collector, false))
    }

    /// Pide al servidor que aborte lo que esté ejecutando (conexión aparte, mejor esfuerzo y acotada).
    // ponytail: la cancelación de PostgreSQL es inherentemente carrera: si la sentencia ya terminó, no pasa nada;
    // esperar a que el servidor la acepte antes de devolver el control reduce la ventana pero no la elimina.
    async fn cancel_server_side(&self) {
        let cancelling = async {
            match &self.tls {
                Some(tls) => self.cancel_token.cancel_query(tls.clone()).await,
                None => self.cancel_token.cancel_query(NoTls).await,
            }
        };
        let _ = tokio::time::timeout(CANCEL_GRACE, cancelling).await;
    }
}
