//! Interfaz común de los drivers (PostgreSQL, MySQL / MariaDB, SQL Server): conectar, ejecutar una sentencia con
//! tiempo máximo, cancelación y límite de filas, y apuntar el modo de solo lectura en el servidor.
//!
//! Los drivers son asíncronos y corren sobre un único runtime de tokio creado la primera vez que se conecta algo
//! (en reposo es un hilo bloqueado en IOCP: 0 % de CPU). `Session` envuelve todo en una API bloqueante.

use super::credentials::{CredentialError, Secret};
use super::safety::{Danger, QueryLimits, SafetyError};
use super::sanitize;
use super::store::{Engine, TlsSettings};
use super::tls::TlsError;
use super::tunnel::TunnelError;
use std::borrow::Cow;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) use tokio_util::sync::CancellationToken as CancelToken;

/// Dónde y cómo conectar. `dial_*` es la dirección real (`127.0.0.1:<túnel>` si hay túnel SSH) y `tls_name` el
/// nombre contra el que se verifica el certificado del servidor.
pub(crate) struct ConnectParams {
    pub(crate) engine: Engine,
    pub(crate) dial_host: String,
    pub(crate) dial_port: u16,
    pub(crate) tls_name: String,
    pub(crate) database: String,
    pub(crate) user: String,
    pub(crate) password: Option<Secret>,
    pub(crate) tls: TlsSettings,
    pub(crate) key_passphrase: Option<Secret>,
    pub(crate) connect_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DbError {
    #[error("no se pudo conectar: {0}")]
    Connect(String),
    #[error("TLS: {0}")]
    Tls(#[from] TlsError),
    #[error("túnel SSH: {0}")]
    Tunnel(#[from] TunnelError),
    #[error("credenciales: {0}")]
    Credentials(#[from] CredentialError),
    #[error("la consulta superó el tiempo máximo")]
    Timeout,
    #[error("consulta cancelada")]
    Cancelled,
    /// Mensaje del servidor, ya escapado y acotado
    #[error("{0}")]
    Server(String),
    #[error("{0}")]
    Safety(#[from] SafetyError),
    #[error("{0}")]
    Unsupported(&'static str),
    #[error("{0}")]
    Refused(String),
    #[error("hay que escribir el nombre de la conexión para confirmar la escritura")]
    ConfirmationRequired,
    #[error(
        "hay sentencias peligrosas (DELETE / UPDATE sin WHERE, TRUNCATE o DROP) sin confirmar"
    )]
    DangerousStatements(Vec<(usize, Danger)>),
    #[error("la sentencia {index} falló y {}: {message}", if *rolled_back { "se revirtió todo" } else { "no se pudo confirmar que se revirtiera" })]
    ApplyFailed {
        index: usize,
        message: String,
        rolled_back: bool,
    },
    #[error("el runtime de datos no se pudo iniciar: {0}")]
    Runtime(String),
}

impl DbError {
    /// Los mensajes del servidor son entrada no confiable: se escapan y se acotan
    pub(crate) fn server(error: impl std::fmt::Display) -> Self {
        let text = error.to_string();
        let escaped = sanitize::display(&text);
        Self::Server(escaped.chars().take(500).collect())
    }

    pub(crate) fn connect(error: impl std::fmt::Display) -> Self {
        match Self::server(error) {
            Self::Server(text) => Self::Connect(text),
            other => other,
        }
    }
}

/// Valor de una celda tal como lo devolvió el servidor. **No se puede mostrar sin escapar**: no implementa
/// `Display`; usa `display()` en la interfaz y `raw()` solo para exportar o generar SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cell(Option<String>);

impl Cell {
    pub(crate) fn new(value: Option<String>) -> Self {
        Self(value)
    }

    pub(crate) fn raw(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// `None` para NULL; el texto con los caracteres de control escapados
    pub(crate) fn display(&self) -> Option<Cow<'_, str>> {
        self.0.as_deref().map(sanitize::display)
    }
}

/// Resultado de una sentencia. Los nombres de columnas son del servidor: usa `sanitize::display` al mostrarlos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ResultSet {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<Cell>>,
    /// Se alcanzó el límite de filas y se dejó de leer
    pub(crate) truncated: bool,
    /// Filas afectadas por un `INSERT` / `UPDATE` / `DELETE` (si el motor lo informa)
    pub(crate) affected: Option<u64>,
    pub(crate) elapsed: Duration,
}

/// Acumula filas respetando el límite
pub(crate) struct Collector {
    max_rows: usize,
    set: ResultSet,
}

impl Collector {
    pub(crate) fn new(max_rows: usize) -> Self {
        Self {
            max_rows,
            set: ResultSet::default(),
        }
    }

    pub(crate) fn set_columns(&mut self, columns: Vec<String>) {
        self.set.columns = columns;
    }

    pub(crate) fn set_affected(&mut self, affected: u64) {
        self.set.affected = Some(affected);
    }

    /// `false` cuando se llegó al límite: hay que dejar de leer
    pub(crate) fn push(&mut self, row: Vec<Cell>) -> bool {
        if self.set.rows.len() >= self.max_rows {
            self.set.truncated = true;
            return false;
        }
        self.set.rows.push(row);
        true
    }

    pub(crate) fn finish(mut self, elapsed: Duration) -> ResultSet {
        self.set.elapsed = elapsed;
        self.set
    }
}

// -------------------------------------------------------------------------------------------------
// Runtime
// -------------------------------------------------------------------------------------------------

/// Único runtime de datos: un hilo de trabajo, creado la primera vez que hace falta
pub(crate) fn runtime() -> Result<&'static tokio::runtime::Runtime, DbError> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("tearminal-db")
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| DbError::Runtime(e.clone()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Interrupt {
    Timeout,
    Cancelled,
}

impl From<Interrupt> for DbError {
    fn from(value: Interrupt) -> Self {
        match value {
            Interrupt::Timeout => Self::Timeout,
            Interrupt::Cancelled => Self::Cancelled,
        }
    }
}

/// Corre `work` hasta que termine, venza el tiempo máximo o se cancele. Quien llama debe pedir al servidor que
/// aborte la sentencia cuando el resultado es `Err(Interrupt)`.
pub(super) async fn race<T>(
    work: impl Future<Output = Result<T, DbError>>,
    limits: &QueryLimits,
    cancel: &CancelToken,
) -> Result<Result<T, DbError>, Interrupt> {
    tokio::select! {
        result = work => Ok(result),
        () = tokio::time::sleep(limits.timeout) => Err(Interrupt::Timeout),
        () = cancel.cancelled() => Err(Interrupt::Cancelled),
    }
}

/// Espera acotada para la cancelación en el servidor (mejor esfuerzo)
pub(super) const CANCEL_GRACE: Duration = Duration::from_secs(5);

// -------------------------------------------------------------------------------------------------
// Conexión de cualquier motor
// -------------------------------------------------------------------------------------------------

pub(crate) enum Conn {
    Postgres(Box<super::pg::PgConn>),
    Mysql(super::mysql::MyConn),
    SqlServer(Box<super::mssql::MsConn>),
}

impl Conn {
    pub(crate) async fn connect(params: &ConnectParams) -> Result<Self, DbError> {
        match params.engine {
            Engine::Postgres => super::pg::PgConn::connect(params)
                .await
                .map(|c| Self::Postgres(Box::new(c))),
            Engine::Mysql | Engine::Mariadb => {
                super::mysql::MyConn::connect(params).await.map(Self::Mysql)
            }
            Engine::SqlServer => super::mssql::MsConn::connect(params)
                .await
                .map(|c| Self::SqlServer(Box::new(c))),
        }
    }

    /// Ejecuta UNA sentencia
    pub(crate) async fn query(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ResultSet, DbError> {
        match self {
            Self::Postgres(conn) => conn.query(sql, limits, cancel).await,
            Self::Mysql(conn) => conn.query(sql, limits, cancel).await,
            Self::SqlServer(conn) => conn.query(sql, limits, cancel).await,
        }
    }

    /// PostgreSQL y MySQL aplican la solo lectura en el servidor; SQL Server no tiene modo de solo lectura por
    /// sesión (ahí protege solo el clasificador de SQL).
    pub(crate) async fn set_read_only(&mut self, read_only: bool) -> Result<(), DbError> {
        let sql = match (&*self, read_only) {
            (Self::Postgres(_), true) => "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY",
            (Self::Postgres(_), false) => "SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE",
            (Self::Mysql(_), true) => "SET SESSION TRANSACTION READ ONLY",
            (Self::Mysql(_), false) => "SET SESSION TRANSACTION READ WRITE",
            (Self::SqlServer(_), _) => return Ok(()),
        };
        let limits = QueryLimits {
            timeout: Duration::from_secs(10),
            max_rows: 1,
        };
        self.query(sql, &limits, &CancelToken::new())
            .await
            .map(|_| ())
    }

    /// `false` si la conexión se cerró o quedó con resultados sin leer (tras cancelar, vencer el tiempo o cortar por
    /// el límite de filas): hay que reconectar antes de usarla.
    pub(crate) fn is_healthy(&self) -> bool {
        match self {
            Self::Postgres(conn) => conn.is_healthy(),
            Self::Mysql(conn) => conn.is_healthy(),
            Self::SqlServer(conn) => conn.is_healthy(),
        }
    }

    /// Ejecuta una sentencia y devuelve las filas afectadas. SQL Server no las informa por el flujo de resultados:
    /// se añade `SELECT @@ROWCOUNT` justo detrás de la sentencia.
    pub(crate) async fn execute(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<u64, DbError> {
        if matches!(self, Self::SqlServer(_)) {
            let counted = format!(
                "{}
SELECT @@ROWCOUNT",
                sql.trim_end().trim_end_matches(';')
            );
            let set = self.query(&counted, limits, cancel).await?;
            return Ok(set
                .rows
                .first()
                .and_then(|row| row.first())
                .and_then(|cell| cell.raw())
                .and_then(|text| text.parse().ok())
                .unwrap_or(0));
        }
        let set = self.query(sql, limits, cancel).await?;
        Ok(set.affected.unwrap_or(0))
    }
}
