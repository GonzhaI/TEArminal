//! Estado y lógica de la pestaña del módulo de bases de datos (DISENO §5, pantallas 05 a 08 y 10).
//!
//! Controla las 4 vistas (Conexiones, Exploración/Datos, Editor SQL y Esquema),
//! la cola de cambios pendientes con generación SQL, paginación, filtros e inspector.

#![allow(dead_code)]

use crate::db::store::{Connection, ConnectionStore, Engine, Env, SslMode};
use crate::renderer::DbViewMode;
use crate::ui::TextInput;
use std::path::PathBuf;
use std::time::Instant;

/// Pestaña del formulario de conexión (05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DbConnFormTab {
    #[default]
    General,
    Tls,
    Tunnel,
    Advanced,
}

/// Paso verificado durante «Probar conexión».
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbTestStep {
    pub(crate) mark: String,
    pub(crate) text: String,
    pub(crate) elapsed_ms: u64,
    pub(crate) ok: bool,
}

/// Campo del formulario de conexión con foco de edición activo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DbConnField {
    Name,
    Host,
    Port,
    Database,
    User,
    Password,
    TlsServerName,
    TlsCaPath,
    TlsCertPath,
    TlsKeyPath,
    TunnelHost,
    TunnelPort,
    TunnelUser,
    TunnelKeyPath,
    TunnelFingerprint,
}

/// Estado de la vista de Conexiones (pantalla 05).
#[derive(Debug, Clone)]
pub(crate) struct DbConnectionsState {
    pub(crate) store: ConnectionStore,
    pub(crate) selected_id: Option<String>,
    pub(crate) search_filter: TextInput,
    pub(crate) form_tab: DbConnFormTab,
    pub(crate) active_field: Option<DbConnField>,
    pub(crate) name_input: TextInput,
    pub(crate) engine: Engine,
    pub(crate) host_input: TextInput,
    pub(crate) port_input: TextInput,
    pub(crate) database_input: TextInput,
    pub(crate) user_input: TextInput,
    pub(crate) password_input: TextInput,
    pub(crate) env: Env,
    pub(crate) read_only: bool,
    // SSL / TLS
    pub(crate) ssl_mode: SslMode,
    pub(crate) tls_server_name: TextInput,
    pub(crate) tls_ca_path: TextInput,
    pub(crate) tls_cert_path: TextInput,
    pub(crate) tls_key_path: TextInput,
    // Túnel SSH
    pub(crate) tunnel_enabled: bool,
    pub(crate) tunnel_host: TextInput,
    pub(crate) tunnel_port: TextInput,
    pub(crate) tunnel_user: TextInput,
    pub(crate) tunnel_key_path: TextInput,
    pub(crate) tunnel_fingerprint: TextInput,
    // Prueba de conexión
    pub(crate) test_steps: Vec<DbTestStep>,
    pub(crate) test_warning: Option<(String, PathBuf)>,
    pub(crate) list_scroll: usize,
}

impl Default for DbConnectionsState {
    fn default() -> Self {
        let store = ConnectionStore::load_from(&ConnectionStore::default_path())
            .unwrap_or_else(|_| ConnectionStore::default());

        let selected_id = store.connections.first().map(|c| c.id.clone());
        let mut s = Self {
            store,
            selected_id: None,
            search_filter: TextInput::new(""),
            form_tab: DbConnFormTab::General,
            active_field: None,
            name_input: TextInput::new(""),
            engine: Engine::Postgres,
            host_input: TextInput::new("localhost"),
            port_input: TextInput::new("5432"),
            database_input: TextInput::new(""),
            user_input: TextInput::new(""),
            password_input: TextInput::new(""),
            env: Env::Local,
            read_only: false,
            ssl_mode: SslMode::VerifyFull,
            tls_server_name: TextInput::new(""),
            tls_ca_path: TextInput::new(""),
            tls_cert_path: TextInput::new(""),
            tls_key_path: TextInput::new(""),
            tunnel_enabled: false,
            tunnel_host: TextInput::new(""),
            tunnel_port: TextInput::new("22"),
            tunnel_user: TextInput::new(""),
            tunnel_key_path: TextInput::new(""),
            tunnel_fingerprint: TextInput::new(""),
            test_steps: Vec::new(),
            test_warning: None,
            list_scroll: 0,
        };

        if let Some(id) = selected_id {
            s.select_connection(&id);
        }
        s
    }
}

impl DbConnectionsState {
    pub(crate) fn select_connection(&mut self, id: &str) {
        if let Some(c) = self.store.find(id) {
            self.selected_id = Some(c.id.clone());
            self.name_input = TextInput::new(&c.name);
            self.engine = c.engine;
            self.host_input = TextInput::new(&c.host);
            self.port_input = TextInput::new(c.port.to_string());
            self.database_input = TextInput::new(&c.database);
            self.user_input = TextInput::new(&c.user);
            let saved_pwd =
                crate::db::credentials::load(crate::db::credentials::SecretKind::DbPassword, &c.id)
                    .ok()
                    .flatten()
                    .map(|s| s.expose().to_string())
                    .unwrap_or_default();
            self.password_input = TextInput::new(saved_pwd);
            self.env = c.env;
            self.read_only = c.read_only;

            self.ssl_mode = c.tls.mode;
            self.tls_server_name = TextInput::new(c.tls.server_name.as_deref().unwrap_or_default());
            self.tls_ca_path = TextInput::new(
                c.tls
                    .ca_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            self.tls_cert_path = TextInput::new(
                c.tls
                    .client_cert_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            self.tls_key_path = TextInput::new(
                c.tls
                    .client_key_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );

            self.tunnel_enabled = c.tunnel.enabled;
            self.tunnel_host = TextInput::new(&c.tunnel.host);
            self.tunnel_port = TextInput::new(c.tunnel.port.to_string());
            self.tunnel_user = TextInput::new(&c.tunnel.user);
            self.tunnel_key_path = TextInput::new(
                c.tunnel
                    .key_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            self.tunnel_fingerprint =
                TextInput::new(c.tunnel.known_fingerprint.as_deref().unwrap_or_default());
        }
    }

    pub(crate) fn reset_to_new_connection(&mut self) {
        self.selected_id = None;
        self.name_input = TextInput::new("Nueva conexión");
        self.engine = Engine::Postgres;
        self.host_input = TextInput::new("localhost");
        self.port_input = TextInput::new("5432");
        self.database_input = TextInput::new("");
        self.user_input = TextInput::new("postgres");
        self.password_input = TextInput::new("");
        self.env = Env::Local;
        self.read_only = false;
        self.ssl_mode = SslMode::Disable;
        self.tls_server_name = TextInput::new("");
        self.tls_ca_path = TextInput::new("");
        self.tls_cert_path = TextInput::new("");
        self.tls_key_path = TextInput::new("");
        self.tunnel_enabled = false;
        self.tunnel_host = TextInput::new("");
        self.tunnel_port = TextInput::new("22");
        self.tunnel_user = TextInput::new("");
        self.tunnel_key_path = TextInput::new("");
        self.tunnel_fingerprint = TextInput::new("");
        self.test_steps.clear();
        self.test_warning = None;
    }

    pub(crate) fn build_connection_from_form(&self) -> Connection {
        let mut conn = Connection::new(
            if self.name_input.text.is_empty() {
                "Nueva conexión"
            } else {
                &self.name_input.text
            },
            self.engine,
        );
        conn.host = self.host_input.text.clone();
        conn.port = self.port_input.text.parse().unwrap_or(match self.engine {
            Engine::Postgres => 5432,
            Engine::Mysql | Engine::Mariadb => 3306,
            Engine::SqlServer => 1433,
        });
        conn.database = self.database_input.text.clone();
        conn.user = self.user_input.text.clone();
        if let Some(ref id) = self.selected_id {
            conn.id = id.clone();
        }
        conn.env = self.env;
        conn.read_only = self.read_only;
        conn.tls.mode = self.ssl_mode;
        if !self.tls_server_name.text.is_empty() {
            conn.tls.server_name = Some(self.tls_server_name.text.clone());
        }
        if !self.tls_ca_path.text.is_empty() {
            conn.tls.ca_path = Some(PathBuf::from(&self.tls_ca_path.text));
        }
        if !self.tls_cert_path.text.is_empty() {
            conn.tls.client_cert_path = Some(PathBuf::from(&self.tls_cert_path.text));
        }
        if !self.tls_key_path.text.is_empty() {
            conn.tls.client_key_path = Some(PathBuf::from(&self.tls_key_path.text));
        }
        conn.tunnel.enabled = self.tunnel_enabled;
        conn.tunnel.host = self.tunnel_host.text.clone();
        conn.tunnel.port = self.tunnel_port.text.parse().unwrap_or(22);
        conn.tunnel.user = self.tunnel_user.text.clone();
        if !self.tunnel_key_path.text.is_empty() {
            conn.tunnel.key_path = Some(PathBuf::from(&self.tunnel_key_path.text));
        }
        if !self.tunnel_fingerprint.text.is_empty() {
            conn.tunnel.known_fingerprint = Some(self.tunnel_fingerprint.text.clone());
        }
        conn
    }
}

/// Columna de una tabla en la vista Browse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbBrowseColumn {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) is_pk: bool,
    pub(crate) is_fk: bool,
}

/// Fila de datos en la vista Browse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbBrowseRow {
    pub(crate) id: String,
    pub(crate) cells: Vec<String>,
    pub(crate) modified_cols: Vec<usize>,
}

/// Campo de filtro activo en la barra superior de Browse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowseFilterField {
    Where,
    OrderBy,
}

/// Modificación pendiente en una celda de la cuadrícula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingCellChange {
    pub(crate) table: String,
    pub(crate) row_id: String,
    pub(crate) col_name: String,
    pub(crate) old_value: String,
    pub(crate) new_value: String,
}

/// Relación navegable en el inspector de fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbRowRelation {
    pub(crate) table: String,
    pub(crate) count_label: String,
}

/// Estado de la vista de exploración de tablas y datos (pantalla 06).
#[derive(Debug, Clone)]
pub(crate) struct DbBrowseState {
    pub(crate) table_name: String,
    pub(crate) where_input: TextInput,
    pub(crate) order_by_input: TextInput,
    pub(crate) columns: Vec<DbBrowseColumn>,
    pub(crate) rows: Vec<DbBrowseRow>,
    pub(crate) page_size: usize,
    pub(crate) current_page: usize,
    pub(crate) total_rows: usize,
    pub(crate) selected_row: Option<usize>,
    pub(crate) selected_col: Option<usize>,
    pub(crate) editing_cell: Option<(usize, usize, TextInput)>,
    pub(crate) active_filter: Option<BrowseFilterField>,
    pub(crate) pending_changes: Vec<PendingCellChange>,
    pub(crate) relations: Vec<DbRowRelation>,
    pub(crate) scroll_y: usize,
}

impl Default for DbBrowseState {
    fn default() -> Self {
        Self {
            table_name: String::new(),
            where_input: TextInput::new(""),
            order_by_input: TextInput::new(""),
            columns: Vec::new(),
            rows: Vec::new(),
            page_size: 50,
            current_page: 0,
            total_rows: 0,
            selected_row: None,
            selected_col: None,
            editing_cell: None,
            active_filter: None,
            pending_changes: Vec::new(),
            relations: Vec::new(),
            scroll_y: 0,
        }
    }
}

#[cfg(test)]
impl DbBrowseState {
    pub(crate) fn sample_for_tests() -> Self {
        let columns = vec![
            DbBrowseColumn {
                name: "id".into(),
                data_type: "bigint".into(),
                is_pk: true,
                is_fk: false,
            },
            DbBrowseColumn {
                name: "email".into(),
                data_type: "varchar(255)".into(),
                is_pk: false,
                is_fk: false,
            },
            DbBrowseColumn {
                name: "name".into(),
                data_type: "varchar(120)".into(),
                is_pk: false,
                is_fk: false,
            },
            DbBrowseColumn {
                name: "plan".into(),
                data_type: "varchar(32)".into(),
                is_pk: false,
                is_fk: false,
            },
            DbBrowseColumn {
                name: "active".into(),
                data_type: "boolean".into(),
                is_pk: false,
                is_fk: false,
            },
            DbBrowseColumn {
                name: "created_at".into(),
                data_type: "timestamptz".into(),
                is_pk: false,
                is_fk: false,
            },
        ];

        let rows = vec![
            DbBrowseRow {
                id: "1".into(),
                cells: vec![
                    "1".into(),
                    "ana.soto@acme.cl".into(),
                    "Ana Soto".into(),
                    "pro".into(),
                    "true".into(),
                    "2026-01-02 10:30:00".into(),
                ],
                modified_cols: Vec::new(),
            },
            DbBrowseRow {
                id: "4".into(),
                cells: vec![
                    "4".into(),
                    "elena.vidal@acme.cl".into(),
                    "Elena Vidal".into(),
                    "enterprise".into(),
                    "false".into(),
                    "2026-01-15 16:45:00".into(),
                ],
                modified_cols: vec![4],
            },
            DbBrowseRow {
                id: "5".into(),
                cells: vec![
                    "5".into(),
                    "carlos.ruiz@acme.cl".into(),
                    "Carlos R.".into(),
                    "pro".into(),
                    "true".into(),
                    "2026-02-14 11:20:00".into(),
                ],
                modified_cols: vec![2],
            },
        ];

        let pending_changes = vec![
            PendingCellChange {
                table: "users".into(),
                row_id: "4".into(),
                col_name: "active".into(),
                old_value: "false".into(),
                new_value: "true".into(),
            },
            PendingCellChange {
                table: "users".into(),
                row_id: "5".into(),
                col_name: "name".into(),
                old_value: "Carlos R.".into(),
                new_value: "Elena Vidal".into(),
            },
        ];

        Self {
            table_name: "users".into(),
            where_input: TextInput::new(""),
            order_by_input: TextInput::new("id ASC"),
            columns,
            rows,
            page_size: 50,
            current_page: 0,
            total_rows: 3,
            selected_row: Some(0),
            selected_col: Some(0),
            editing_cell: None,
            active_filter: None,
            pending_changes,
            relations: Vec::new(),
            scroll_y: 0,
        }
    }
}

impl DbBrowseState {
    /// Genera la sentencia SQL combinada a partir de los cambios pendientes acumulados.
    pub(crate) fn generated_sql(&self) -> String {
        if self.pending_changes.is_empty() {
            return String::new();
        }
        let statements: Vec<String> = self
            .pending_changes
            .iter()
            .map(|c| {
                format!(
                    "UPDATE {} SET {} = '{}' WHERE id = {};",
                    c.table, c.col_name, c.new_value, c.row_id
                )
            })
            .collect();
        statements.join(" ")
    }

    /// Descarta todos los cambios pendientes y restablece las celdas a sus valores originales.
    pub(crate) fn discard_pending(&mut self) {
        for change in &self.pending_changes {
            if let Some(row) = self.rows.iter_mut().find(|r| r.id == change.row_id) {
                if let Some(col_idx) = self.columns.iter().position(|c| c.name == change.col_name) {
                    if col_idx < row.cells.len() {
                        row.cells[col_idx] = change.old_value.clone();
                    }
                }
            }
        }
        for row in self.rows.iter_mut() {
            row.modified_cols.clear();
        }
        self.pending_changes.clear();
        self.editing_cell = None;
    }

    /// Aplica la edición de una celda y registra la modificación pendiente.
    pub(crate) fn commit_cell_edit(&mut self, row_idx: usize, col_idx: usize, new_val: String) {
        let Some(row) = self.rows.get_mut(row_idx) else {
            self.editing_cell = None;
            return;
        };
        let Some(old_val) = row.cells.get(col_idx).cloned() else {
            self.editing_cell = None;
            return;
        };
        if new_val == old_val {
            self.editing_cell = None;
            return;
        }
        let row_id = row.id.clone();
        let col_name = self
            .columns
            .get(col_idx)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        row.cells[col_idx] = new_val.clone();
        if !row.modified_cols.contains(&col_idx) {
            row.modified_cols.push(col_idx);
        }

        if let Some(existing) = self
            .pending_changes
            .iter_mut()
            .find(|c| c.row_id == row_id && c.col_name == col_name)
        {
            existing.new_value = new_val;
        } else {
            self.pending_changes.push(PendingCellChange {
                table: self.table_name.clone(),
                row_id,
                col_name,
                old_value: old_val,
                new_value: new_val,
            });
        }
        self.editing_cell = None;
    }
}

/// Sub-pestaña de la sección de resultados en el Editor SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DbQueryResultsTab {
    #[default]
    Grid,
    Plan,
    Messages,
}

/// Entrada en el historial de consultas ejecutadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbQueryHistoryItem {
    pub(crate) title: String,
    pub(crate) sql: String,
    pub(crate) meta: String,
    pub(crate) is_slow: bool,
}

/// Consulta guardada en la biblioteca lateral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbSavedQuery {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) sql: String,
    pub(crate) meta: String,
}

/// Estado de la pestaña Editor SQL (pantalla 07).
#[derive(Debug, Clone)]
pub(crate) struct DbQueryState {
    pub(crate) editor_input: TextInput,
    pub(crate) results_tab: DbQueryResultsTab,
    pub(crate) auto_commit: bool,
    pub(crate) is_executing: bool,
    pub(crate) elapsed_ms: u64,
    pub(crate) rows_count: usize,
    pub(crate) result_columns: Vec<String>,
    pub(crate) result_rows: Vec<Vec<String>>,
    pub(crate) execution_plan: String,
    pub(crate) messages: Vec<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) history: Vec<DbQueryHistoryItem>,
    pub(crate) saved_queries: Vec<DbSavedQuery>,
    pub(crate) suggest_items: Vec<(String, String)>,
    pub(crate) results_scroll_y: usize,
}

impl DbQueryState {
    /// Guarda la consulta actual del editor en la biblioteca lateral (si no está vacía ni duplicada).
    pub(crate) fn save_current_query(&mut self) -> bool {
        let sql = self.editor_input.text.trim().to_string();
        if sql.is_empty() {
            return false;
        }
        if self.saved_queries.iter().any(|q| q.sql.trim() == sql) {
            return false;
        }
        let first_line = sql
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Consulta SQL")
            .trim();
        let mut title: String = first_line.chars().take(28).collect();
        if first_line.chars().count() > 28 {
            title.push('…');
        }
        let id = format!("sq-{}", self.saved_queries.len() + 1);
        let meta = format!("{} · guardada", self.statement_type());
        self.saved_queries.insert(
            0,
            DbSavedQuery {
                id,
                title,
                sql,
                meta,
            },
        );
        true
    }
    /// Devuelve la posición de línea y columna del cursor (1-indexed).
    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        self.editor_input.cursor_line_col()
    }

    /// Detecta el tipo de sentencia SQL principal (SELECT, INSERT, UPDATE, etc.).
    pub(crate) fn statement_type(&self) -> &'static str {
        let trimmed = self.editor_input.text.trim_start().to_uppercase();
        if trimmed.starts_with("SELECT") {
            "SELECT"
        } else if trimmed.starts_with("INSERT") {
            "INSERT"
        } else if trimmed.starts_with("UPDATE") {
            "UPDATE"
        } else if trimmed.starts_with("DELETE") {
            "DELETE"
        } else if trimmed.starts_with("EXPLAIN") {
            "EXPLAIN"
        } else if trimmed.starts_with("CREATE") {
            "CREATE"
        } else if trimmed.starts_with("ALTER") {
            "ALTER"
        } else if trimmed.starts_with("DROP") {
            "DROP"
        } else if trimmed.starts_with("WITH") {
            "WITH / SELECT"
        } else if trimmed.starts_with("SHOW") {
            "SHOW"
        } else {
            "SQL"
        }
    }

    /// Exporta los resultados actuales a formato CSV.
    pub(crate) fn to_csv(&self) -> String {
        let mut out = String::new();
        // Encabezados
        let header = self
            .result_columns
            .iter()
            .map(|col| {
                if col.contains(',') || col.contains('"') || col.contains('\n') {
                    format!("\"{}\"", col.replace('"', "\"\""))
                } else {
                    col.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&header);
        out.push('\n');

        // Filas
        for row in &self.result_rows {
            let line = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    /// Exporta los resultados actuales a un array de objetos JSON.
    pub(crate) fn to_json(&self) -> String {
        let mut out = String::from("[\n");
        for (r_idx, row) in self.result_rows.iter().enumerate() {
            out.push_str("  {\n");
            for (c_idx, col) in self.result_columns.iter().enumerate() {
                let val = row.get(c_idx).map(|s| s.as_str()).unwrap_or("");
                let escaped_val = val.replace('\\', "\\\\").replace('"', "\\\"");
                let comma = if c_idx + 1 < self.result_columns.len() {
                    ","
                } else {
                    ""
                };
                out.push_str(&format!("    \"{}\": \"{}\"{}\n", col, escaped_val, comma));
            }
            let row_comma = if r_idx + 1 < self.result_rows.len() {
                ","
            } else {
                ""
            };
            out.push_str(&format!("  }}{}\n", row_comma));
        }
        out.push(']');
        out
    }

    /// Exporta los resultados en formato tabular delimitado por tabulaciones (TSV).
    pub(crate) fn to_tsv(&self) -> String {
        let mut out = self.result_columns.join("\t");
        out.push('\n');
        for row in &self.result_rows {
            out.push_str(&row.join("\t"));
            out.push('\n');
        }
        out
    }

    /// Formatea de manera básica e indentada la consulta SQL actual.
    pub(crate) fn format_current_sql(&mut self) {
        let current = self.editor_input.text.trim();
        if current.is_empty() {
            return;
        }

        let keywords = [
            "SELECT", "FROM", "WHERE", "GROUP", "ORDER", "BY", "HAVING", "LIMIT", "OFFSET", "JOIN",
            "LEFT", "RIGHT", "INNER", "CROSS", "ON", "AND", "OR", "NOT", "IN", "IS", "NULL",
            "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "UNION", "ALL", "AS",
            "DISTINCT", "COUNT", "SUM", "AVG", "MIN", "MAX", "ROUND", "EXPLAIN", "ANALYZE",
        ];

        let mut formatted = String::with_capacity(current.len() + 32);
        for line in current.lines() {
            let mut line_formatted = String::new();
            for word in line.split_whitespace() {
                let up = word.to_ascii_uppercase();
                let matched = keywords.iter().find(|&&kw| kw == up.as_str());
                if !line_formatted.is_empty() {
                    line_formatted.push(' ');
                }
                if let Some(kw) = matched {
                    line_formatted.push_str(kw);
                } else {
                    line_formatted.push_str(word);
                }
            }
            if !formatted.is_empty() {
                formatted.push('\n');
            }
            formatted.push_str(&line_formatted);
        }
        self.editor_input = TextInput::new(formatted);
    }
}

impl Default for DbQueryState {
    fn default() -> Self {
        Self {
            editor_input: TextInput::new(""),
            auto_commit: true,
            results_tab: DbQueryResultsTab::Grid,
            is_executing: false,
            elapsed_ms: 0,
            rows_count: 0,
            result_columns: Vec::new(),
            result_rows: Vec::new(),
            execution_plan: String::new(),
            messages: Vec::new(),
            error_message: None,
            history: Vec::new(),
            saved_queries: Vec::new(),
            suggest_items: Vec::new(),
            results_scroll_y: 0,
        }
    }
}

#[cfg(test)]
impl DbQueryState {
    pub(crate) fn sample_for_tests() -> Self {
        let default_sql = "SELECT\n    plan,\n    COUNT(id) AS usuarios,\n    SUM(total_clp) AS ingresos,\n    ROUND(AVG(total_clp)) AS ticket_medio\nFROM analytics.subscription_events\nWHERE created_at >= '2026-01-01'\nGROUP BY plan\nORDER BY ingresos DESC;";
        Self {
            editor_input: TextInput::new(default_sql),
            auto_commit: true,
            results_tab: DbQueryResultsTab::Grid,
            is_executing: false,
            elapsed_ms: 18,
            rows_count: 1,
            result_columns: vec![
                "plan".into(),
                "usuarios".into(),
                "ingresos".into(),
                "ticket_medio".into(),
            ],
            result_rows: vec![vec![
                "enterprise".into(),
                "142".into(),
                "42.600.000".into(),
                "300.000".into(),
            ]],
            execution_plan: String::new(),
            messages: Vec::new(),
            error_message: None,
            history: Vec::new(),
            saved_queries: Vec::new(),
            suggest_items: Vec::new(),
            results_scroll_y: 0,
        }
    }
}

/// Columna en una tarjeta de tabla del diagrama de esquema (pantalla 08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbSchemaCol {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) is_pk: bool,
    pub(crate) is_fk: bool,
}

/// Relación entre dos tablas a través de claves foráneas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbSchemaRelation {
    pub(crate) from_table: String,
    pub(crate) from_col: String,
    pub(crate) to_table: String,
    pub(crate) to_col: String,
}

/// Tarjeta de tabla en el lienzo del diagrama de esquema (pantalla 08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbSchemaTableCard {
    pub(crate) name: String,
    pub(crate) row_count_label: String,
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) columns: Vec<DbSchemaCol>,
}

impl DbSchemaTableCard {
    /// Devuelve las columnas visibles de la tarjeta según el filtro de claves.
    pub(crate) fn visible_columns<'a>(
        &'a self,
        only_keys: bool,
    ) -> Box<dyn Iterator<Item = &'a DbSchemaCol> + 'a> {
        if only_keys {
            Box::new(self.columns.iter().filter(|c| c.is_pk || c.is_fk))
        } else {
            Box::new(self.columns.iter())
        }
    }

    /// Calcula la altura total de la tarjeta en píxeles.
    pub(crate) fn height(&self, only_keys: bool) -> usize {
        let count = self.visible_columns(only_keys).count();
        34 + count * 26 + 1
    }
}

/// Estado de la vista de Esquema y relaciones (pantalla 08).
#[derive(Debug, Clone)]
pub(crate) struct DbSchemaState {
    pub(crate) schema_name: String,
    pub(crate) selected_table: String,
    pub(crate) tables_list: Vec<(String, String)>,
    pub(crate) cards: Vec<DbSchemaTableCard>,
    pub(crate) relations: Vec<DbSchemaRelation>,
    pub(crate) only_keys: bool,
    pub(crate) zoom_pct: usize,
    pub(crate) pan_x: isize,
    pub(crate) pan_y: isize,
}

impl DbSchemaState {
    pub(crate) fn scale_px(&self, val: usize) -> usize {
        (val.saturating_mul(self.zoom_pct) / 100).max(1)
    }

    pub(crate) fn card_screen_rect(
        &self,
        card: &DbSchemaTableCard,
        canvas_x: usize,
        canvas_y: usize,
    ) -> (isize, isize, usize, usize) {
        let sx = canvas_x as isize + self.pan_x + self.scale_px(card.x) as isize;
        let sy = canvas_y as isize + self.pan_y + self.scale_px(card.y) as isize;
        let sw = self.scale_px(220).max(110);
        let sh = self.scale_px(card.height(self.only_keys)).max(40);
        (sx, sy, sw, sh)
    }

    pub(crate) fn hit_test_card(
        &self,
        canvas_x: usize,
        canvas_y: usize,
        mx: usize,
        my: usize,
    ) -> Option<String> {
        let mxi = mx as isize;
        let myi = my as isize;
        for card in self.cards.iter().rev() {
            let (rx, ry, rw, rh) = self.card_screen_rect(card, canvas_x, canvas_y);
            if mxi >= rx && mxi < rx + rw as isize && myi >= ry && myi < ry + rh as isize {
                return Some(card.name.clone());
            }
        }
        None
    }

    /// Recoloca las tarjetas de tabla en una disposición de cuadrícula ordenada.
    pub(crate) fn arrange_cards(&mut self, canvas_w: usize) {
        let card_w = 220;
        let gap_x = 50;
        let gap_y = 40;
        let start_x = 30;
        let start_y = 30;

        let cols = (canvas_w.saturating_sub(start_x * 2)) / (card_w + gap_x);
        let cols = cols.clamp(1, 3);

        let mut col_heights = vec![start_y; cols];
        for card in &mut self.cards {
            // Asignar a la columna con menor altura actual
            let min_col = col_heights
                .iter()
                .enumerate()
                .min_by_key(|&(_, &h)| h)
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            card.x = start_x + min_col * (card_w + gap_x);
            card.y = col_heights[min_col];

            col_heights[min_col] += card.height(self.only_keys) + gap_y;
        }
    }

    /// Genera la sentencia DDL `CREATE TABLE` para la tabla indicada.
    pub(crate) fn generate_table_ddl(&self, table_name: &str) -> String {
        let Some(card) = self.cards.iter().find(|c| c.name == table_name) else {
            return format!("-- Tabla {} no encontrada en el esquema", table_name);
        };

        let mut ddl = format!("CREATE TABLE {}.{} (\n", self.schema_name, card.name);
        let mut defs = Vec::new();
        for col in &card.columns {
            let mut line = format!("    {} {}", col.name, col.data_type);
            if col.is_pk {
                line.push_str(" PRIMARY KEY");
            }
            defs.push(line);
        }
        for rel in &self.relations {
            if rel.from_table == card.name {
                defs.push(format!(
                    "    CONSTRAINT fk_{}_{} FOREIGN KEY ({}) REFERENCES {}.{}({}) ON DELETE RESTRICT",
                    rel.from_table, rel.from_col, rel.from_col, self.schema_name, rel.to_table, rel.to_col
                ));
            }
        }
        ddl.push_str(&defs.join(",\n"));
        ddl.push_str("\n);");
        ddl
    }

    /// Genera el DDL completo de todas las tablas y relaciones del esquema.
    pub(crate) fn generate_all_ddl(&self) -> String {
        let mut out = format!("-- Esquema: {}\n\n", self.schema_name);
        for card in &self.cards {
            out.push_str(&self.generate_table_ddl(&card.name));
            out.push_str("\n\n");
        }
        out
    }

    /// Claves foráneas que salen de la tabla indicada.
    pub(crate) fn foreign_keys_for_table<'a>(
        &'a self,
        table_name: &str,
    ) -> Vec<(&'a str, &'a str, &'a str)> {
        self.relations
            .iter()
            .filter(|r| r.from_table == table_name)
            .map(|r| (r.from_col.as_str(), r.to_table.as_str(), r.to_col.as_str()))
            .collect()
    }

    /// Tablas y columnas que referencian a la tabla indicada.
    pub(crate) fn referenced_by_for_table<'a>(
        &'a self,
        table_name: &str,
    ) -> Vec<(&'a str, &'a str)> {
        self.relations
            .iter()
            .filter(|r| r.to_table == table_name)
            .map(|r| (r.from_table.as_str(), r.from_col.as_str()))
            .collect()
    }
}

impl Default for DbSchemaState {
    fn default() -> Self {
        Self {
            schema_name: String::new(),
            selected_table: String::new(),
            tables_list: Vec::new(),
            cards: Vec::new(),
            relations: Vec::new(),
            only_keys: false,
            zoom_pct: 100,
            pan_x: 0,
            pan_y: 0,
        }
    }
}

#[cfg(test)]
impl DbSchemaState {
    pub(crate) fn sample_for_tests() -> Self {
        let tables_list = vec![
            ("users".into(), "12 480".into()),
            ("orders".into(), "48 210".into()),
            ("order_items".into(), "131 904".into()),
            ("invoices".into(), "47 880".into()),
            ("sessions".into(), "3 204".into()),
            ("products".into(), "612".into()),
        ];

        let cards = vec![
            DbSchemaTableCard {
                name: "users".into(),
                row_count_label: "12 480".into(),
                x: 20,
                y: 50,
                columns: vec![DbSchemaCol {
                    name: "id".into(),
                    data_type: "bigint".into(),
                    is_pk: true,
                    is_fk: false,
                }],
            },
            DbSchemaTableCard {
                name: "orders".into(),
                row_count_label: "48 210".into(),
                x: 300,
                y: 50,
                columns: vec![
                    DbSchemaCol {
                        name: "id".into(),
                        data_type: "bigint".into(),
                        is_pk: true,
                        is_fk: false,
                    },
                    DbSchemaCol {
                        name: "user_id".into(),
                        data_type: "bigint".into(),
                        is_pk: false,
                        is_fk: true,
                    },
                ],
            },
            DbSchemaTableCard {
                name: "order_items".into(),
                row_count_label: "131 904".into(),
                x: 580,
                y: 50,
                columns: vec![],
            },
            DbSchemaTableCard {
                name: "sessions".into(),
                row_count_label: "3 204".into(),
                x: 20,
                y: 280,
                columns: vec![],
            },
            DbSchemaTableCard {
                name: "invoices".into(),
                row_count_label: "47 880".into(),
                x: 300,
                y: 280,
                columns: vec![],
            },
            DbSchemaTableCard {
                name: "products".into(),
                row_count_label: "612".into(),
                x: 580,
                y: 280,
                columns: vec![],
            },
        ];

        let relations = vec![
            DbSchemaRelation {
                from_table: "orders".into(),
                from_col: "user_id".into(),
                to_table: "users".into(),
                to_col: "id".into(),
            },
            DbSchemaRelation {
                from_table: "order_items".into(),
                from_col: "order_id".into(),
                to_table: "orders".into(),
                to_col: "id".into(),
            },
            DbSchemaRelation {
                from_table: "order_items".into(),
                from_col: "product_id".into(),
                to_table: "products".into(),
                to_col: "id".into(),
            },
            DbSchemaRelation {
                from_table: "sessions".into(),
                from_col: "user_id".into(),
                to_table: "users".into(),
                to_col: "id".into(),
            },
            DbSchemaRelation {
                from_table: "invoices".into(),
                from_col: "order_id".into(),
                to_table: "orders".into(),
                to_col: "id".into(),
            },
        ];

        Self {
            schema_name: "public".into(),
            selected_table: "orders".into(),
            tables_list,
            cards,
            relations,
            only_keys: false,
            zoom_pct: 100,
            pan_x: 0,
            pan_y: 0,
        }
    }
}

/// Estado global de una pestaña de base de datos (módulo Datos).
#[derive(Debug, Clone)]
pub(crate) struct DbTabState {
    pub(crate) active_view: DbViewMode,
    pub(crate) connections: DbConnectionsState,
    pub(crate) browse: DbBrowseState,
    pub(crate) query: DbQueryState,
    pub(crate) schema: DbSchemaState,
    pub(crate) active_connection_id: Option<String>,
    pub(crate) active_connection_name: String,
    pub(crate) active_connection_engine: Engine,
    pub(crate) active_connection_env: Env,
    pub(crate) active_connection_db: String,
    pub(crate) is_connected: bool,
    pub(crate) ping_ms: u64,
    pub(crate) prod_write_expires_at: Option<Instant>,
    /// Duración de cada ventana de escritura en PROD (`database.write_window_minutes`).
    pub(crate) write_window: std::time::Duration,
    pub(crate) sidebar_subtab_queries: bool,
    pub(crate) sidebar_search: TextInput,
    pub(crate) sidebar_search_focused: bool,
    pub(crate) sidebar_scroll: usize,
}

impl Default for DbTabState {
    fn default() -> Self {
        let conn_state = DbConnectionsState::default();
        let first_conn = conn_state.store.connections.first().cloned();
        let active_id = first_conn.as_ref().map(|c| c.id.clone());

        Self {
            active_view: DbViewMode::Connections,
            connections: conn_state,
            browse: DbBrowseState::default(),
            query: DbQueryState::default(),
            schema: DbSchemaState::default(),
            active_connection_id: active_id,
            active_connection_name: first_conn
                .as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            active_connection_engine: first_conn
                .as_ref()
                .map(|c| c.engine)
                .unwrap_or(Engine::Postgres),
            active_connection_env: first_conn.as_ref().map(|c| c.env).unwrap_or(Env::Local),
            active_connection_db: first_conn
                .as_ref()
                .map(|c| c.database.clone())
                .unwrap_or_default(),
            is_connected: false,
            ping_ms: 0,
            prod_write_expires_at: None,
            write_window: std::time::Duration::from_secs(600),
            sidebar_subtab_queries: false,
            sidebar_search: TextInput::new(""),
            sidebar_search_focused: false,
            sidebar_scroll: 0,
        }
    }
}

impl DbTabState {
    /// Tablas del esquema filtradas por `sidebar_search`.
    pub(crate) fn filtered_tables(&self) -> Vec<(String, String)> {
        let q = self.sidebar_search.text.trim().to_ascii_lowercase();
        self.schema
            .tables_list
            .iter()
            .filter(|(name, _)| q.is_empty() || name.to_ascii_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// Consultas guardadas filtradas por `sidebar_search`.
    pub(crate) fn filtered_saved_queries(&self) -> Vec<DbSavedQuery> {
        let q = self.sidebar_search.text.trim().to_ascii_lowercase();
        self.query
            .saved_queries
            .iter()
            .filter(|item| {
                q.is_empty()
                    || item.title.to_ascii_lowercase().contains(&q)
                    || item.sql.to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    /// Historial de consultas filtrado por `sidebar_search`.
    pub(crate) fn filtered_history(&self) -> Vec<DbQueryHistoryItem> {
        let q = self.sidebar_search.text.trim().to_ascii_lowercase();
        self.query
            .history
            .iter()
            .filter(|item| {
                q.is_empty()
                    || item.title.to_ascii_lowercase().contains(&q)
                    || item.sql.to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    /// Devuelve `true` si la conexión activa prohíbe edición directa de celdas (solo lectura o PROD bloqueado).
    pub(crate) fn is_effective_read_only(&self, policy_prod_read_only: bool) -> bool {
        let conn_ro = self
            .active_connection_id
            .as_ref()
            .and_then(|id| self.connections.store.find(id))
            .map_or(self.connections.read_only, |c| c.read_only);
        let prod_locked = self.active_connection_env == Env::Prod
            && policy_prod_read_only
            && !self.is_write_enabled();
        conn_ro || prod_locked
    }
    /// Estado inicial según los ajustes de bases de datos: PROD arranca en solo lectura si
    /// `prod_read_only` está activo, la ventana de escritura dura `write_window_minutes` y las
    /// tablas se paginan con `page_size`.
    pub(crate) fn from_config(config: &crate::config::DatabaseConfig) -> Self {
        let mut state = Self {
            write_window: std::time::Duration::from_secs(
                u64::from(config.write_window_minutes) * 60,
            ),
            ..Self::default()
        };
        state.prod_write_expires_at =
            (!config.prod_read_only).then(|| Instant::now() + state.write_window);
        state.browse.page_size = config.page_size;
        state
    }

    /// Aplica a una pestaña ya abierta los ajustes que cambian en caliente desde Ajustes.
    pub(crate) fn apply_config(&mut self, config: &crate::config::DatabaseConfig) {
        self.write_window =
            std::time::Duration::from_secs(u64::from(config.write_window_minutes) * 60);
        self.browse.page_size = config.page_size;
    }

    /// Tiempo restante formateado en MM:SS para el banner de escritura en PROD.
    pub(crate) fn prod_countdown_str(&self) -> Option<String> {
        let expires = self.prod_write_expires_at?;
        let now = Instant::now();
        if now >= expires {
            return Some("00:00".into());
        }
        let remaining_secs = (expires - now).as_secs();
        let mins = remaining_secs / 60;
        let secs = remaining_secs % 60;
        Some(format!("{mins:02}:{secs:02}"))
    }

    /// Comprueba si la escritura está habilitada en la conexión activa.
    pub(crate) fn is_write_enabled(&self) -> bool {
        if self.active_connection_env != Env::Prod {
            return true;
        }
        self.prod_write_expires_at
            .is_some_and(|exp| Instant::now() < exp)
    }

    /// Alterna el modo de solo lectura o renueva la ventana de escritura.
    pub(crate) fn toggle_prod_write(&mut self) {
        if self.is_write_enabled() {
            self.prod_write_expires_at = None;
        } else {
            self.prod_write_expires_at = Some(Instant::now() + self.write_window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_tab_state_initialization() {
        let tab = DbTabState::default();
        assert_eq!(tab.active_view, DbViewMode::Connections);
        assert!(!tab.is_connected);
        assert_eq!(tab.ping_ms, 0);
        assert!(tab.prod_countdown_str().is_none());
    }

    #[test]
    fn test_db_browse_pending_sql() {
        let mut browse = DbBrowseState::sample_for_tests();
        let sql = browse.generated_sql();
        assert!(sql.contains("UPDATE users SET active = 'true' WHERE id = 4;"));
        assert!(sql.contains("UPDATE users SET name = 'Elena Vidal' WHERE id = 5;"));

        // Modificar una celda adicional
        browse.commit_cell_edit(0, 3, "enterprise".into());
        assert_eq!(browse.pending_changes.len(), 3);
        let updated_sql = browse.generated_sql();
        assert!(updated_sql.contains("UPDATE users SET plan = 'enterprise' WHERE id = 1;"));

        // Descartar cambios
        browse.discard_pending();
        assert_eq!(browse.pending_changes.len(), 0);
        assert!(browse.generated_sql().is_empty());
    }

    #[test]
    fn test_db_connections_state() {
        let mut conns = DbConnectionsState::default();
        conns.name_input.text = "local-dev".into();
        let conn = conns.build_connection_from_form();
        let first_id = conn.id.clone();
        conns.store.connections.push(conn);

        // Seleccionar conexión
        conns.select_connection(&first_id);
        assert_eq!(conns.selected_id, Some(first_id));
        assert_eq!(conns.name_input.text, "local-dev");
    }

    #[test]
    fn test_db_tab_from_config_respects_read_only_window_and_page_size() {
        let mut cfg = crate::config::DatabaseConfig {
            prod_read_only: true,
            write_window_minutes: 25,
            page_size: 100,
            ..Default::default()
        };
        let mut tab = DbTabState::from_config(&cfg);
        tab.active_connection_env = Env::Prod;
        assert!(
            !tab.is_write_enabled(),
            "PROD debe arrancar en solo lectura"
        );
        assert_eq!(tab.browse.page_size, 100);
        tab.toggle_prod_write();
        assert!(tab.is_write_enabled());
        let remaining = tab.prod_write_expires_at.map(|e| e - Instant::now());
        assert!(remaining.is_some_and(|r| r > std::time::Duration::from_secs(24 * 60)));
        cfg.prod_read_only = false;
        assert!(DbTabState::from_config(&cfg).is_write_enabled());
    }

    #[test]
    fn test_build_and_reset_connection_form() {
        let mut conns = DbConnectionsState::default();
        conns.reset_to_new_connection();
        assert_eq!(conns.selected_id, None);
        assert_eq!(conns.name_input.text, "Nueva conexión");
        assert_eq!(conns.host_input.text, "localhost");
        assert_eq!(conns.engine, Engine::Postgres);
        assert_eq!(conns.env, Env::Local);

        conns.name_input.text = "mi-db-prod".into();
        conns.host_input.text = "10.0.0.5".into();
        conns.port_input.text = "5433".into();
        conns.database_input.text = "sistema".into();
        conns.user_input.text = "operador".into();
        conns.env = Env::Prod;
        conns.read_only = true;

        let conn = conns.build_connection_from_form();
        assert_eq!(conn.name, "mi-db-prod");
        assert_eq!(conn.host, "10.0.0.5");
        assert_eq!(conn.port, 5433);
        assert_eq!(conn.database, "sistema");
        assert_eq!(conn.user, "operador");
        assert_eq!(conn.env, Env::Prod);
        assert!(conn.read_only);
    }

    #[test]
    fn test_db_query_exports_and_statement_type() {
        let q = DbQueryState::sample_for_tests();
        assert_eq!(q.statement_type(), "SELECT");
        let (line, col) = q.cursor_line_col();
        assert!(line >= 8);
        assert!(col >= 1);

        let csv = q.to_csv();
        assert!(csv.starts_with("plan,usuarios,ingresos,ticket_medio\n"));
        assert!(csv.contains("enterprise,142,42.600.000,300.000\n"));

        let json = q.to_json();
        assert!(json.starts_with("[\n"));
        assert!(json.contains("\"plan\": \"enterprise\""));
        assert!(json.contains("\"usuarios\": \"142\""));
    }

    #[test]
    fn test_db_schema_relations_and_ddl() {
        let mut schema = DbSchemaState::sample_for_tests();
        assert_eq!(schema.cards.len(), 6);
        assert_eq!(schema.relations.len(), 5);

        let ddl_orders = schema.generate_table_ddl("orders");
        assert!(ddl_orders.contains("CREATE TABLE public.orders"));
        assert!(ddl_orders.contains("id bigint PRIMARY KEY"));
        assert!(ddl_orders.contains("FOREIGN KEY (user_id) REFERENCES public.users(id)"));

        let fks = schema.foreign_keys_for_table("orders");
        assert_eq!(fks.len(), 1);
        assert_eq!(fks[0], ("user_id", "users", "id"));

        let refs = schema.referenced_by_for_table("orders");
        assert_eq!(refs.len(), 2); // order_items e invoices

        // Probar recolocación de tarjetas
        schema.arrange_cards(1000);
        assert!(schema.cards[0].x >= 30);
        assert!(schema.cards[1].x >= 30);
    }
}
