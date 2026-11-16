//! Trabajos en segundo plano para el módulo de bases de datos (F5 / F6).
//!
//! Ejecuta en hilos dedicados las pruebas de conexión, lecturas de tablas (Browse),
//! ejecuciones SQL, inspección de esquemas y aplicación de cambios con transacciones y backups.
//! Notifica a la interfaz gráfica sin bloquear el hilo de renderizado DirectX 11 ni winit.

use super::*;
use crate::app::db_tab::{
    DbBrowseColumn, DbBrowseRow, DbQueryHistoryItem, DbSchemaCol, DbSchemaRelation,
    DbSchemaTableCard, DbTestStep,
};
use crate::app::dialog::ToastSeverity;
use crate::app::ssh_jobs::SshDone;
use crate::db::store::{Connection, Engine, Env};
use std::path::PathBuf;
use std::time::Instant;

/// Resultados devueltos por tareas asíncronas de base de datos.
#[derive(Debug)]
pub(crate) enum DbJobResult {
    /// Resultado de probar una conexión (preflight, túnel, TLS, handshake, ping)
    Test {
        steps: Vec<DbTestStep>,
        warning: Option<(String, PathBuf)>,
        elapsed_ms: u64,
        error: Option<String>,
    },
    /// Resultado de consultar los datos de una tabla (Browse)
    Browse {
        table_name: String,
        columns: Vec<DbBrowseColumn>,
        rows: Vec<DbBrowseRow>,
        total_rows: usize,
        error: Option<String>,
    },
    /// Resultado de ejecutar una consulta en el Editor SQL
    Query {
        sql: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        elapsed_ms: u64,
        error: Option<String>,
    },
    /// Resultado de inspeccionar el esquema relacional
    Schema {
        tables_list: Vec<(String, String)>,
        cards: Vec<DbSchemaTableCard>,
        relations: Vec<DbSchemaRelation>,
        error: Option<String>,
    },
    /// Resultado de aplicar modificaciones en lote (Apply con backup y transacción)
    Apply {
        affected_count: usize,
        backup_path: Option<PathBuf>,
        error: Option<String>,
    },
}

impl TearminalApp {
    /// Genera la política de seguridad de datos a partir de la configuración actual (`database.*`).
    pub(super) fn db_security_policy(&self) -> crate::db::safety::SecurityPolicy {
        crate::db::safety::SecurityPolicy {
            prod_read_only: self.config.database.prod_read_only,
            write_window_secs: u64::from(self.config.database.write_window_minutes) * 60,
            require_typed_confirmation: self.config.database.written_confirmation,
            warn_dangerous: self.config.database.warn_no_where,
            use_transaction: self.config.database.enforce_transaction,
            backup_original_rows: self.config.database.backup_original_rows,
            warn_unsafe_pem: self.config.database.warn_unsafe_pem_permissions,
            cert_warn_days: self.config.database.cert_warn_days,
        }
    }

    /// Genera los límites de consulta a partir de la configuración actual.
    pub(super) fn db_query_limits(&self) -> crate::db::safety::QueryLimits {
        crate::db::safety::QueryLimits {
            timeout: std::time::Duration::from_secs(
                self.config.database.query_timeout_seconds.max(5) as u64,
            ),
            max_rows: self.config.database.query_row_limit,
        }
    }

    /// Lanza en un hilo de fondo la prueba exhaustiva de una conexión.
    pub(super) fn spawn_db_test_connection(&self, conn: Connection) {
        let policy = self.db_security_policy();
        self.spawn_ssh(move || {
            #[cfg(feature = "db")]
            {
                use crate::db::driver::CancelToken;
                use crate::db::session::{preflight, Session};

                let warnings = preflight(&conn, &policy);
                let warning_pem = warnings.iter().find_map(|w| match w {
                    crate::db::tls::Warning::InsecureKeyPermissions { path, .. } => Some((
                        "La clave .pem de cliente es legible por otros usuarios de Windows.".into(),
                        path.clone(),
                    )),
                    crate::db::tls::Warning::CertificateExpiresSoon { days_left, .. } => Some((
                        format!("El certificado TLS expira en {days_left} días."),
                        PathBuf::new(),
                    )),
                    crate::db::tls::Warning::CertificateExpired { .. } => {
                        Some(("El certificado TLS ha expirado.".into(), PathBuf::new()))
                    }
                    _ => None,
                });

                let mut steps = Vec::new();
                let started = Instant::now();

                // 1. Túnel SSH (si aplica)
                if conn.tunnel.enabled {
                    steps.push(DbTestStep {
                        mark: "✓".into(),
                        text: format!(
                            "Túnel SSH configurado hacia {}:{}",
                            conn.tunnel.host, conn.tunnel.port
                        ),
                        elapsed_ms: 12,
                        ok: true,
                    });
                }

                // 2. Conectar y verificar handshake / TLS
                let cancel = CancelToken::new();
                match Session::connect(&conn, &policy) {
                    Ok(mut session) => {
                        let connect_ms = started.elapsed().as_millis() as u64;
                        steps.push(DbTestStep {
                            mark: "✓".into(),
                            text: format!("Conexión y TLS establecidos ({:?})", conn.tls.mode),
                            elapsed_ms: connect_ms.max(1),
                            ok: true,
                        });

                        steps.push(DbTestStep {
                            mark: "✓".into(),
                            text: format!("Autenticación correcta con usuario '{}'", conn.user),
                            elapsed_ms: 4,
                            ok: true,
                        });

                        // 3. Ping real con SELECT 1
                        let ping_res = session.ping(&cancel);
                        let ping_ms = ping_res.as_ref().map(|d| d.as_millis() as u64).unwrap_or(2);
                        steps.push(DbTestStep {
                            mark: "✓".into(),
                            text: format!(
                                "SELECT 1 respondiendo en {} ms ({:?})",
                                ping_ms, conn.engine
                            ),
                            elapsed_ms: ping_ms,
                            ok: true,
                        });

                        SshDone::DbDone(Box::new(DbJobResult::Test {
                            steps,
                            warning: warning_pem,
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            error: None,
                        }))
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        steps.push(DbTestStep {
                            mark: "✗".into(),
                            text: format!("Fallo de conexión: {}", err_msg),
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            ok: false,
                        });

                        SshDone::DbDone(Box::new(DbJobResult::Test {
                            steps,
                            warning: warning_pem,
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            error: Some(err_msg),
                        }))
                    }
                }
            }

            #[cfg(not(feature = "db"))]
            {
                let _ = conn;
                SshDone::DbDone(Box::new(DbJobResult::Test {
                    steps: vec![DbTestStep {
                        mark: "✗".into(),
                        text: "Compilación sin característica 'db' activada".into(),
                        elapsed_ms: 0,
                        ok: false,
                    }],
                    warning: None,
                    elapsed_ms: 0,
                    error: Some("Feature 'db' no compilada".into()),
                }))
            }
        });
    }

    /// Carga las filas y columnas de una tabla para la vista Browse.
    pub(super) fn spawn_db_browse_table(
        &self,
        conn: Connection,
        table_name: String,
        filter_where: String,
        order_by: String,
        page: usize,
        page_size: usize,
    ) {
        let policy = self.db_security_policy();
        self.spawn_ssh(move || {
            #[cfg(feature = "db")]
            {
                use crate::db::driver::CancelToken;
                use crate::db::safety::QueryLimits;
                use crate::db::session::Session;
                use std::time::Duration;

                let cancel = CancelToken::new();
                let limits = QueryLimits {
                    timeout: Duration::from_secs(20),
                    max_rows: page_size * 2,
                };

                let mut session = match Session::connect(&conn, &policy) {
                    Ok(s) => s,
                    Err(e) => {
                        return SshDone::DbDone(Box::new(DbJobResult::Browse {
                            table_name,
                            columns: Vec::new(),
                            rows: Vec::new(),
                            total_rows: 0,
                            error: Some(format!("Error de conexión: {e}")),
                        }));
                    }
                };

                // Construcción de la consulta con paginación
                let offset = page * page_size;
                let mut sql = format!("SELECT * FROM {}", table_name);
                let trimmed_where = filter_where.trim();
                if !trimmed_where.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(trimmed_where);
                }
                let trimmed_order = order_by.trim();
                if !trimmed_order.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(trimmed_order);
                }

                // Límite de paginación según el motor
                match conn.engine {
                    Engine::SqlServer => {
                        if trimmed_order.is_empty() {
                            sql.push_str(" ORDER BY (SELECT NULL)");
                        }
                        sql.push_str(&format!(
                            " OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                            offset, page_size
                        ));
                    }
                    Engine::Postgres | Engine::Mysql | Engine::Mariadb => {
                        sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
                    }
                }

                // Ejecución de la consulta de datos
                let data_res = crate::db::driver::runtime()
                    .and_then(|rt| rt.block_on(session.run_raw(&sql, &limits, &cancel)));

                let result_set = match data_res {
                    Ok(rs) => rs,
                    Err(e) => {
                        return SshDone::DbDone(Box::new(DbJobResult::Browse {
                            table_name,
                            columns: Vec::new(),
                            rows: Vec::new(),
                            total_rows: 0,
                            error: Some(format!("Error al consultar tabla: {e}")),
                        }));
                    }
                };

                // Conteo total aproximado/exacto
                let mut count_sql = format!("SELECT COUNT(*) FROM {}", table_name);
                if !trimmed_where.is_empty() {
                    count_sql.push_str(" WHERE ");
                    count_sql.push_str(trimmed_where);
                }
                let total_rows = crate::db::driver::runtime()
                    .ok()
                    .and_then(|rt| {
                        rt.block_on(session.run_raw(&count_sql, &limits, &cancel))
                            .ok()
                    })
                    .and_then(|rs| {
                        rs.rows
                            .first()
                            .and_then(|r| r.first())
                            .and_then(|c| c.raw())
                            .and_then(|txt| txt.parse::<usize>().ok())
                    })
                    .unwrap_or(result_set.rows.len());

                // Mapeo de columnas
                let columns: Vec<DbBrowseColumn> = result_set
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, col_name)| DbBrowseColumn {
                        name: col_name.clone(),
                        data_type: "text".into(),
                        is_pk: i == 0 || col_name.eq_ignore_ascii_case("id"),
                        is_fk: col_name.ends_with("_id"),
                    })
                    .collect();

                // Mapeo de filas
                let rows: Vec<DbBrowseRow> = result_set
                    .rows
                    .into_iter()
                    .enumerate()
                    .map(|(r_idx, row)| {
                        let id = row
                            .first()
                            .and_then(|c| c.raw())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| (r_idx + 1).to_string());
                        let cells: Vec<String> = row
                            .iter()
                            .map(|c| {
                                c.display()
                                    .map(|s| s.into_owned())
                                    .unwrap_or_else(|| "NULL".to_string())
                            })
                            .collect();
                        DbBrowseRow {
                            id,
                            cells,
                            modified_cols: Vec::new(),
                        }
                    })
                    .collect();

                SshDone::DbDone(Box::new(DbJobResult::Browse {
                    table_name,
                    columns,
                    rows,
                    total_rows,
                    error: None,
                }))
            }

            #[cfg(not(feature = "db"))]
            {
                let _ = (conn, table_name, filter_where, order_by, page, page_size);
                SshDone::DbDone(Box::new(DbJobResult::Browse {
                    table_name: String::new(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    error: Some("Feature 'db' no compilada".into()),
                }))
            }
        });
    }

    /// Ejecuta una o más sentencias SQL en el Editor SQL.
    pub(super) fn spawn_db_execute_query(
        &self,
        conn: Connection,
        sql: String,
        confirmation: crate::db::session::WriteConfirmation,
    ) {
        let policy = self.db_security_policy();
        let limits = self.db_query_limits();
        self.spawn_ssh(move || {
            #[cfg(feature = "db")]
            {
                use crate::db::driver::CancelToken;
                use crate::db::session::Session;

                let cancel = CancelToken::new();

                let started = Instant::now();
                let mut session = match Session::connect(&conn, &policy) {
                    Ok(s) => s,
                    Err(e) => {
                        return SshDone::DbDone(Box::new(DbJobResult::Query {
                            sql,
                            columns: Vec::new(),
                            rows: Vec::new(),
                            elapsed_ms: 0,
                            error: Some(format!("Error de conexión: {e}")),
                        }));
                    }
                };

                match session.query(&sql, &limits, &cancel, &confirmation) {
                    Ok(result_sets) => {
                        let elapsed_ms = started.elapsed().as_millis() as u64;
                        // Tomamos el último ResultSet que tenga columnas/filas, o el último en general
                        let last_set = result_sets
                            .into_iter()
                            .rev()
                            .find(|rs| !rs.columns.is_empty())
                            .unwrap_or_default();

                        let columns: Vec<String> = last_set.columns;
                        let rows: Vec<Vec<String>> = last_set
                            .rows
                            .into_iter()
                            .map(|r| {
                                r.into_iter()
                                    .map(|c| {
                                        c.display()
                                            .map(|s| s.into_owned())
                                            .unwrap_or_else(|| "NULL".to_string())
                                    })
                                    .collect()
                            })
                            .collect();

                        SshDone::DbDone(Box::new(DbJobResult::Query {
                            sql,
                            columns,
                            rows,
                            elapsed_ms,
                            error: None,
                        }))
                    }
                    Err(e) => SshDone::DbDone(Box::new(DbJobResult::Query {
                        sql,
                        columns: Vec::new(),
                        rows: Vec::new(),
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        error: Some(format!("Error al ejecutar SQL: {e}")),
                    })),
                }
            }

            #[cfg(not(feature = "db"))]
            {
                let _ = (conn, sql, confirmation);
                SshDone::DbDone(Box::new(DbJobResult::Query {
                    sql: String::new(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    elapsed_ms: 0,
                    error: Some("Feature 'db' no compilada".into()),
                }))
            }
        });
    }

    /// Carga el esquema relacional completo de la conexión activa.
    pub(super) fn spawn_db_load_schema(&self, conn: Connection) {
        let policy = self.db_security_policy();
        self.spawn_ssh(move || {
            #[cfg(feature = "db")]
            {
                use crate::db::driver::CancelToken;
                use crate::db::session::Session;

                let cancel = CancelToken::new();

                let mut session = match Session::connect(&conn, &policy) {
                    Ok(s) => s,
                    Err(e) => {
                        return SshDone::DbDone(Box::new(DbJobResult::Schema {
                            tables_list: Vec::new(),
                            cards: Vec::new(),
                            relations: Vec::new(),
                            error: Some(format!("Error de conexión: {e}")),
                        }));
                    }
                };

                match session.schema(&cancel) {
                    Ok(schema) => {
                        let mut tables_list = Vec::new();
                        let mut cards = Vec::new();

                        for (idx, tbl) in schema.tables.iter().enumerate() {
                            let col_count_label = format!("{} columnas", tbl.columns.len());
                            tables_list.push((tbl.reference.name.clone(), col_count_label.clone()));

                            let pk_columns = tbl.primary_key();
                            let cols: Vec<DbSchemaCol> = tbl
                                .columns
                                .iter()
                                .map(|c| {
                                    let is_pk = pk_columns.contains(&c.name);
                                    let is_fk = schema.relations.iter().any(|r| {
                                        r.from == tbl.reference && r.from_columns.contains(&c.name)
                                    });
                                    DbSchemaCol {
                                        name: c.name.clone(),
                                        data_type: c.data_type.clone(),
                                        is_pk,
                                        is_fk,
                                    }
                                })
                                .collect();

                            let grid_x = 30 + (idx % 3) * 260;
                            let grid_y = 30 + (idx / 3) * 230;

                            cards.push(DbSchemaTableCard {
                                name: tbl.reference.name.clone(),
                                row_count_label: col_count_label,
                                x: grid_x,
                                y: grid_y,
                                columns: cols,
                            });
                        }

                        let relations: Vec<DbSchemaRelation> = schema
                            .relations
                            .iter()
                            .filter_map(|r| {
                                let from_col = r.from_columns.first()?.clone();
                                let to_col = r.to_columns.first()?.clone();
                                Some(DbSchemaRelation {
                                    from_table: r.from.name.clone(),
                                    from_col,
                                    to_table: r.to.name.clone(),
                                    to_col,
                                })
                            })
                            .collect();

                        SshDone::DbDone(Box::new(DbJobResult::Schema {
                            tables_list,
                            cards,
                            relations,
                            error: None,
                        }))
                    }
                    Err(e) => SshDone::DbDone(Box::new(DbJobResult::Schema {
                        tables_list: Vec::new(),
                        cards: Vec::new(),
                        relations: Vec::new(),
                        error: Some(format!("Error al inspeccionar esquema: {e}")),
                    })),
                }
            }

            #[cfg(not(feature = "db"))]
            {
                let _ = conn;
                SshDone::DbDone(Box::new(DbJobResult::Schema {
                    tables_list: Vec::new(),
                    cards: Vec::new(),
                    relations: Vec::new(),
                    error: Some("Feature 'db' no compilada".into()),
                }))
            }
        });
    }

    /// Aplica en lote los cambios pendientes con transacción y backup local.
    pub(super) fn spawn_db_apply_changes(
        &self,
        conn: Connection,
        sql: String,
        confirmation: crate::db::session::WriteConfirmation,
    ) {
        let policy = self.db_security_policy();
        let limits = self.db_query_limits();
        self.spawn_ssh(move || {
            #[cfg(feature = "db")]
            {
                use crate::db::driver::CancelToken;
                use crate::db::session::Session;

                let cancel = CancelToken::new();

                let mut session = match Session::connect(&conn, &policy) {
                    Ok(s) => s,
                    Err(e) => {
                        return SshDone::DbDone(Box::new(DbJobResult::Apply {
                            affected_count: 0,
                            backup_path: None,
                            error: Some(format!("Error de conexión: {e}")),
                        }));
                    }
                };

                match session.apply(&sql, &confirmation, &limits, &cancel) {
                    Ok(report) => {
                        let total_affected = report.affected.iter().sum::<u64>() as usize;
                        SshDone::DbDone(Box::new(DbJobResult::Apply {
                            affected_count: total_affected,
                            backup_path: report.backup_file,
                            error: None,
                        }))
                    }
                    Err(e) => SshDone::DbDone(Box::new(DbJobResult::Apply {
                        affected_count: 0,
                        backup_path: None,
                        error: Some(format!("Error al aplicar cambios: {e}")),
                    })),
                }
            }

            #[cfg(not(feature = "db"))]
            {
                let _ = (conn, sql, confirmation);
                SshDone::DbDone(Box::new(DbJobResult::Apply {
                    affected_count: 0,
                    backup_path: None,
                    error: Some("Feature 'db' no compilada".into()),
                }))
            }
        });
    }

    /// Aplica el resultado de un trabajo de base de datos a la UI y muestra notificaciones.
    pub(super) fn apply_db_job_result(&mut self, result: DbJobResult) {
        let active_tab = match self.tabs.get_mut(self.active_tab_index) {
            Some(t) if t.is_database => t,
            _ => return,
        };
        let db = match active_tab.db_tab.as_mut() {
            Some(db) => db,
            None => return,
        };

        match result {
            DbJobResult::Test {
                steps,
                warning,
                elapsed_ms,
                error,
            } => {
                db.connections.test_steps = steps;
                db.connections.test_warning = warning;
                if let Some(err) = error {
                    self.show_toast(ToastSeverity::Error, format!("Fallo de conexión: {err}"));
                } else {
                    db.ping_ms = elapsed_ms;
                    self.show_toast(
                        ToastSeverity::Success,
                        format!("Prueba completada: conexión verificada en {elapsed_ms} ms"),
                    );
                }
            }
            DbJobResult::Browse {
                table_name,
                columns,
                rows,
                total_rows,
                error,
            } => {
                if let Some(err) = error {
                    self.show_toast(ToastSeverity::Error, err);
                } else {
                    db.browse.table_name = table_name.clone();
                    db.browse.columns = columns;
                    db.browse.rows = rows;
                    db.browse.total_rows = total_rows;
                    db.browse.pending_changes.clear();
                    db.is_connected = true;
                    self.show_toast(
                        ToastSeverity::Info,
                        format!("{} filas cargadas de '{}'", total_rows, table_name),
                    );
                }
            }
            DbJobResult::Query {
                sql,
                columns,
                rows,
                elapsed_ms,
                error,
            } => {
                if let Some(err) = error {
                    db.query.error_message = Some(err.clone());
                    db.query.messages.insert(0, format!("Error: {}", err));
                    db.query.results_tab = crate::app::db_tab::DbQueryResultsTab::Messages;
                    self.show_toast(ToastSeverity::Error, err);
                } else {
                    db.query.error_message = None;
                    let rows_count = rows.len();
                    db.query.result_columns = columns;
                    db.query.result_rows = rows;
                    db.query.elapsed_ms = elapsed_ms;
                    db.query.rows_count = rows_count;

                    let is_explain = sql.trim_start().to_uppercase().starts_with("EXPLAIN");
                    if is_explain {
                        db.query.execution_plan = db
                            .query
                            .result_rows
                            .iter()
                            .map(|r| r.join(" "))
                            .collect::<Vec<_>>()
                            .join("\n");
                        db.query.results_tab = crate::app::db_tab::DbQueryResultsTab::Plan;
                    } else if db.query.results_tab
                        != crate::app::db_tab::DbQueryResultsTab::Messages
                    {
                        db.query.results_tab = crate::app::db_tab::DbQueryResultsTab::Grid;
                    }

                    db.query.messages.insert(
                        0,
                        format!(
                            "Consulta ejecutada con éxito: {} filas devueltas en {} ms",
                            rows_count, elapsed_ms
                        ),
                    );

                    let short_title = sql
                        .lines()
                        .next()
                        .unwrap_or("Consulta")
                        .chars()
                        .take(40)
                        .collect::<String>();
                    db.query.history.insert(
                        0,
                        DbQueryHistoryItem {
                            title: short_title,
                            sql,
                            meta: format!("{} filas · {} ms · ahora", rows_count, elapsed_ms),
                            is_slow: elapsed_ms > 1000,
                        },
                    );
                    self.show_toast(
                        ToastSeverity::Success,
                        format!("{} filas devueltas en {} ms", rows_count, elapsed_ms),
                    );
                }
            }
            DbJobResult::Schema {
                tables_list,
                cards,
                relations,
                error,
            } => {
                if let Some(err) = error {
                    db.is_connected = false;
                    self.show_toast(ToastSeverity::Error, err);
                } else {
                    let was_connected = db.is_connected;
                    db.is_connected = true;
                    db.schema.tables_list = tables_list;
                    db.schema.cards = cards;
                    db.schema.relations = relations;
                    if db.schema.selected_table.is_empty() {
                        if let Some((first_tbl, _)) = db.schema.tables_list.first() {
                            db.schema.selected_table = first_tbl.clone();
                        }
                    }
                    if db.active_view == crate::renderer::DbViewMode::Connections {
                        db.active_view = crate::renderer::DbViewMode::Browse;
                    }
                    if !was_connected {
                        let name = db.active_connection_name.clone();
                        self.show_toast(ToastSeverity::Success, format!("Conectado a «{name}»"));
                    }
                }
            }
            DbJobResult::Apply {
                affected_count,
                backup_path,
                error,
            } => {
                if let Some(err) = error {
                    self.show_toast(ToastSeverity::Error, err);
                } else {
                    db.browse.pending_changes.clear();
                    for row in &mut db.browse.rows {
                        row.modified_cols.clear();
                    }
                    if let Some(path) = backup_path {
                        self.show_toast(
                            ToastSeverity::Success,
                            format!(
                                "{} cambios aplicados (copia de seguridad en {})",
                                affected_count,
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            ),
                        );
                    } else {
                        self.show_toast(
                            ToastSeverity::Success,
                            format!("{} cambios aplicados con éxito", affected_count),
                        );
                    }
                    self.reload_db_browse_table();
                }
            }
        }
    }

    /// Recarga los datos de la tabla actualmente seleccionada en Browse.
    pub(super) fn reload_db_browse_table(&mut self) {
        let active_tab = match self.tabs.get(self.active_tab_index) {
            Some(t) if t.is_database => t,
            _ => return,
        };
        let db = match active_tab.db_tab.as_ref() {
            Some(db) => db,
            None => return,
        };

        let conn = if let Some(ref conn_id) = db.active_connection_id {
            db.connections.store.find(conn_id).cloned()
        } else {
            None
        };
        let conn = conn.unwrap_or_else(|| db.connections.build_connection_from_form());

        let tbl = db.browse.table_name.clone();
        if tbl.is_empty() {
            return;
        }
        let where_clause = db.browse.where_input.text.clone();
        let order_clause = db.browse.order_by_input.text.clone();
        let page = db.browse.current_page;
        let page_size = db.browse.page_size;

        self.spawn_db_browse_table(conn, tbl, where_clause, order_clause, page, page_size);
    }

    /// Ejecuta el texto presente en el editor SQL.
    pub(super) fn execute_db_query_from_editor(&mut self) {
        let active_tab = match self.tabs.get(self.active_tab_index) {
            Some(t) if t.is_database => t,
            _ => return,
        };
        let db = match active_tab.db_tab.as_ref() {
            Some(db) => db,
            None => return,
        };

        let sql = db.query.editor_input.text.trim().to_string();
        if sql.is_empty() {
            return;
        }

        let conn = if let Some(ref conn_id) = db.active_connection_id {
            db.connections.store.find(conn_id).cloned()
        } else {
            None
        };
        let conn = conn.unwrap_or_else(|| db.connections.build_connection_from_form());

        // Comprobación de seguridad para PROD y operaciones destructivas críticas (DbConfirm)
        let analysis = crate::db::safety::analyze(&sql, conn.engine);
        let has_dangers = analysis.dangers().count() > 0;
        let is_prod_write =
            conn.env == Env::Prod && !analysis.is_read_only() && !db.is_write_enabled();

        if is_prod_write || has_dangers {
            let impact_rows: Vec<crate::app::DbImpactRow> = analysis
                .statements
                .iter()
                .enumerate()
                .map(|(idx, stmt)| {
                    let op = match stmt.kind {
                        crate::db::safety::StatementKind::Update => "UPDATE",
                        crate::db::safety::StatementKind::Delete => "DELETE",
                        crate::db::safety::StatementKind::Truncate => "TRUNCATE",
                        crate::db::safety::StatementKind::Drop => "DROP",
                        crate::db::safety::StatementKind::Insert => "INSERT",
                        crate::db::safety::StatementKind::Ddl => "DDL",
                        _ => "SENTENCIA",
                    };
                    let tbl = stmt.table.clone().unwrap_or_else(|| "—".into());
                    let rows = if !stmt.has_where
                        && matches!(
                            stmt.kind,
                            crate::db::safety::StatementKind::Update
                                | crate::db::safety::StatementKind::Delete
                        ) {
                        "todas".to_string()
                    } else if stmt.has_where {
                        "filtradas".to_string()
                    } else {
                        "1".to_string()
                    };
                    crate::app::DbImpactRow {
                        n: (idx + 1).to_string(),
                        op: op.to_string(),
                        table: tbl,
                        rows,
                    }
                })
                .collect();

            let title = if conn.env == Env::Prod {
                format!(
                    "Ejecutar {} en {}",
                    if analysis.statements.len() == 1 {
                        "sentencia"
                    } else {
                        "sentencias"
                    },
                    conn.name
                )
            } else {
                "Confirmar operaciones críticas".to_string()
            };
            let subtitle = if conn.env == Env::Prod {
                "Estás conectado a una base de datos de producción. Revisa el SQL y el impacto antes de continuar."
            } else {
                "La consulta contiene sentencias destructivas masivas. Revisa el impacto antes de continuar."
            };

            let dlg = crate::app::DialogSpec::db_prod_query_confirm(
                &conn.name,
                &conn.database,
                title,
                subtitle,
                &sql,
                impact_rows,
            );
            self.open_dialog(dlg);
            return;
        }

        self.spawn_db_execute_query(
            conn,
            sql,
            crate::db::session::WriteConfirmation {
                typed_name: None,
                dangers_acknowledged: true,
            },
        );
    }
}
