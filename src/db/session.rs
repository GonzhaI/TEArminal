//! Sesión de una conexión abierta: API **bloqueante** (llámala desde un hilo, como `TearminalApp::spawn_ssh`).
//!
//! Aquí se juntan todas las reglas: la sesión conoce la conexión, la política y la concesión de escritura de la
//! pestaña, y las aplica ella misma en cada sentencia (el llamante no puede olvidarse de comprobar la solo lectura ni
//! la caducidad). Si la conexión queda inservible tras cancelar o vencer el tiempo, se reconecta sola.

use super::credentials::{self, SecretKind};
use super::driver::{runtime, CancelToken, Conn, ConnectParams, DbError, ResultSet};
use super::safety::{
    self, access_mode, check_allowed, confirmation_matches, AccessMode, Analysis, QueryLimits,
    SecurityPolicy, WriteGrant,
};
use super::schema::{self, Schema};
use super::store::{Connection, Env};
use super::tls::{self, Warning};
use super::tunnel::{self, Tunnel};
use crate::logger;
use std::time::{Duration, Instant, SystemTime};

/// Lo que el usuario confirmó para esta ejecución
#[derive(Debug, Clone, Default)]
pub(crate) struct WriteConfirmation {
    /// Lo que escribió en el campo «escribe el nombre de la conexión»
    pub(crate) typed_name: Option<String>,
    /// Aceptó el aviso de `DELETE` / `UPDATE` sin `WHERE`, `TRUNCATE` o `DROP`
    pub(crate) dangers_acknowledged: bool,
}

pub(crate) struct Session {
    pub(super) connection: Connection,
    pub(super) policy: SecurityPolicy,
    grant: WriteGrant,
    /// «No volver a pedir confirmación en esta pestaña»
    skip_typed_confirmation: bool,
    params: ConnectParams,
    conn: Option<Conn>,
    /// Modo de solo lectura que tiene aplicado el servidor ahora mismo
    server_read_only: Option<bool>,
    warnings: Vec<Warning>,
    _tunnel: Option<Tunnel>,
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const SCHEMA_LIMITS: QueryLimits = QueryLimits {
    timeout: Duration::from_secs(60),
    max_rows: 200_000,
};

/// Avisos previos a conectar (permisos de las claves, vencimiento de certificados), sin abrir nada de red
pub(crate) fn preflight(connection: &Connection, policy: &SecurityPolicy) -> Vec<Warning> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let reports = tls::inspect(&connection.tls, now, policy.cert_warn_days);
    tls::warnings(&reports, policy.warn_unsafe_pem)
}

impl Session {
    /// Abre el túnel (si hay), carga los secretos del Administrador de credenciales y conecta
    pub(crate) fn connect(
        connection: &Connection,
        policy: &SecurityPolicy,
    ) -> Result<Self, DbError> {
        connection
            .validate()
            .map_err(|e| DbError::Refused(e.to_string()))?;
        let started = Instant::now();
        let warnings = preflight(connection, policy);
        let password = credentials::load(SecretKind::DbPassword, &connection.id)?;
        let key_passphrase = credentials::load(SecretKind::ClientKeyPassphrase, &connection.id)?;

        let tunnel = if connection.tunnel.enabled {
            Some(tunnel::open(
                &connection.tunnel,
                &connection.id,
                &connection.host,
                connection.port,
            )?)
        } else {
            None
        };
        let (dial_host, dial_port) = match &tunnel {
            Some(t) => ("127.0.0.1".to_string(), t.local_port()),
            None => (connection.host.clone(), connection.port),
        };
        let params = ConnectParams {
            engine: connection.engine,
            dial_host,
            dial_port,
            tls_name: connection
                .tls
                .server_name
                .clone()
                .unwrap_or_else(|| connection.host.clone()),
            database: connection.database.clone(),
            user: connection.user.clone(),
            password,
            tls: connection.tls.clone(),
            key_passphrase,
            connect_timeout: CONNECT_TIMEOUT,
        };
        let conn = runtime()?.block_on(Conn::connect(&params))?;
        // Solo metadatos: nunca host, usuario, SQL ni contenido de archivos
        logger::log(&format!(
            "db: conectado engine={:?} tls={:?} tunel={} en {} ms",
            connection.engine,
            connection.tls.mode,
            tunnel.is_some(),
            started.elapsed().as_millis()
        ));
        let mut session = Self {
            connection: connection.clone(),
            policy: policy.clone(),
            grant: WriteGrant::default(),
            skip_typed_confirmation: false,
            params,
            conn: Some(conn),
            server_read_only: None,
            warnings,
            _tunnel: tunnel,
        };
        runtime()?.block_on(session.sync_access(Instant::now()))?;
        Ok(session)
    }

    pub(crate) fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    // ---- modo de acceso ----------------------------------------------------------------------

    pub(crate) fn access_mode(&self, now: Instant) -> AccessMode {
        access_mode(&self.connection, &self.policy, &self.grant, now)
    }

    /// Habilita la escritura temporal de esta pestaña (caduca sola tras `write_window_secs`)
    pub(crate) fn enable_write(&mut self, now: Instant) {
        self.grant
            .enable(now, Duration::from_secs(self.policy.write_window_secs));
    }

    pub(crate) fn disable_write(&mut self) {
        self.grant.disable();
    }

    pub(crate) fn write_remaining(&self, now: Instant) -> Option<Duration> {
        self.grant.remaining(now)
    }

    pub(crate) fn set_skip_typed_confirmation(&mut self, skip: bool) {
        self.skip_typed_confirmation = skip;
    }

    /// Reconecta si hace falta y deja el servidor con el modo de solo lectura que toca ahora
    pub(super) async fn sync_access(&mut self, now: Instant) -> Result<(), DbError> {
        if self.conn.as_ref().is_none_or(|c| !c.is_healthy()) {
            self.conn = None;
            self.server_read_only = None;
            self.conn = Some(Conn::connect(&self.params).await?);
        }
        let want_read_only = self.access_mode(now) == AccessMode::ReadOnly;
        if self.server_read_only != Some(want_read_only) {
            if let Some(conn) = self.conn.as_mut() {
                conn.set_read_only(want_read_only).await?;
            }
            self.server_read_only = Some(want_read_only);
        }
        Ok(())
    }

    /// Una sentencia en la conexión (reconectando y sincronizando el modo antes). No aplica reglas de seguridad:
    /// quien llama ya las comprobó.
    pub(super) async fn run_raw(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ResultSet, DbError> {
        self.sync_access(Instant::now()).await?;
        self.exec_raw(sql, limits, cancel).await
    }

    /// Igual que `run_raw` pero sin tocar el modo de solo lectura: dentro de una transacción abierta MySQL no
    /// permite cambiarlo.
    pub(super) async fn exec_raw(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ResultSet, DbError> {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::Runtime("sin conexión".to_string()))?;
        conn.query(sql, limits, cancel).await
    }

    pub(super) async fn exec_execute(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<u64, DbError> {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::Runtime("sin conexión".to_string()))?;
        conn.execute(sql, limits, cancel).await
    }

    /// Descarta la conexión (la siguiente operación reconecta). Tras un `ROLLBACK` fallido: lo que el servidor
    /// no pudo revertir se pierde al cerrar la sesión.
    pub(super) fn drop_connection(&mut self) {
        self.conn = None;
        self.server_read_only = None;
    }

    pub(super) fn connection_is_open(&self) -> bool {
        self.conn.as_ref().is_some_and(Conn::is_healthy)
    }

    // ---- reglas de seguridad ---------------------------------------------------------------

    pub(crate) fn analyze(&self, sql: &str) -> Analysis {
        safety::analyze(sql, self.connection.engine)
    }

    /// Puerta única de las escrituras: modo de acceso, confirmación escrita (PROD) y sentencias peligrosas
    pub(super) fn authorize(
        &self,
        analysis: &Analysis,
        confirmation: &WriteConfirmation,
        now: Instant,
    ) -> Result<(), DbError> {
        check_allowed(analysis, self.access_mode(now))?;
        if analysis.is_read_only() {
            return Ok(());
        }
        let needs_typed = self.connection.env == Env::Prod
            && self.policy.require_typed_confirmation
            && !self.skip_typed_confirmation;
        if needs_typed
            && !confirmation
                .typed_name
                .as_deref()
                .is_some_and(|typed| confirmation_matches(typed, &self.connection.name))
        {
            return Err(DbError::ConfirmationRequired);
        }
        let dangers: Vec<_> = analysis.dangers().collect();
        if self.policy.warn_dangerous && !dangers.is_empty() && !confirmation.dangers_acknowledged {
            return Err(DbError::DangerousStatements(dangers));
        }
        Ok(())
    }

    // ---- consultas ---------------------------------------------------------------------------

    /// Ejecuta `sql` (una o varias sentencias, una a una) con tiempo máximo, cancelación y límite de filas.
    /// Si una sentencia falla se detiene y devuelve el error.
    pub(crate) fn query(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
        confirmation: &WriteConfirmation,
    ) -> Result<Vec<ResultSet>, DbError> {
        let analysis = self.analyze(sql);
        self.authorize(&analysis, confirmation, Instant::now())?;
        let runtime = runtime()?;
        let mut results = Vec::with_capacity(analysis.statements.len());
        for statement in &analysis.statements {
            let result = runtime.block_on(self.run_raw(&statement.sql, limits, cancel));
            // Solo longitud y duración: jamás el SQL (puede llevar valores)
            logger::log(&format!(
                "db: consulta len={} ok={}",
                statement.sql.len(),
                result.is_ok()
            ));
            results.push(result?);
        }
        Ok(results)
    }

    /// Lee tablas, columnas, claves, índices y relaciones
    pub(crate) fn schema(&mut self, cancel: &CancelToken) -> Result<Schema, DbError> {
        let queries = schema::queries(self.connection.engine);
        let runtime = runtime()?;
        let columns = runtime.block_on(self.run_raw(queries.columns, &SCHEMA_LIMITS, cancel))?;
        let indexes = runtime.block_on(self.run_raw(queries.indexes, &SCHEMA_LIMITS, cancel))?;
        let foreign_keys =
            runtime.block_on(self.run_raw(queries.foreign_keys, &SCHEMA_LIMITS, cancel))?;
        Ok(schema::build(&columns, &indexes, &foreign_keys))
    }

    /// `SELECT 1` para la prueba de conexión: devuelve la latencia
    pub(crate) fn ping(&mut self, cancel: &CancelToken) -> Result<Duration, DbError> {
        let limits = QueryLimits {
            timeout: Duration::from_secs(10),
            max_rows: 1,
        };
        let started = Instant::now();
        runtime()?.block_on(self.run_raw("SELECT 1", &limits, cancel))?;
        Ok(started.elapsed())
    }

    /// El túnel SSH (si lo hay) sigue vivo
    pub(crate) fn tunnel_alive(&mut self) -> Option<bool> {
        self._tunnel.as_mut().map(Tunnel::is_alive)
    }
}
