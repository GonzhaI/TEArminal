//! Aplicar cambios con las reglas de producción: impacto estimado, copia local de las filas originales **antes** de
//! tocar nada, transacción y reversión si algo falla.
//!
//! La copia (`db-backups\<conexión>-<ms>.jsonl`) está en la carpeta local del usuario; contiene datos reales, así que
//! nunca se registra ni se muestra su contenido: solo su ruta.

use super::driver::{runtime, CancelToken, Cell, DbError};
use super::safety::{QueryLimits, SafetyError, StatementInfo, StatementKind};
use super::session::{Session, WriteConfirmation};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Impact {
    /// Posición de la sentencia en el SQL analizado
    pub(crate) statement: usize,
    /// Filas que tocaría; `None` si no se puede estimar (INSERT, sentencias complejas)
    pub(crate) rows: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ApplyReport {
    /// Filas afectadas por cada sentencia, en orden
    pub(crate) affected: Vec<u64>,
    /// Archivo con las filas originales (si la política lo pide y había algo que guardar)
    pub(crate) backup_file: Option<PathBuf>,
    /// `UPDATE` / `DELETE` / `TRUNCATE` cuyas filas originales no se pudieron derivar y quedaron sin copia
    pub(crate) backup_skipped: usize,
}

struct BackupEntry {
    statement: usize,
    sql: String,
    table: String,
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_value(cell: &Cell) -> String {
    cell.raw().map_or_else(|| "null".to_string(), json_string)
}

fn backup_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("tearminal-db-backups-{}", std::process::id()));
    }
    directories::ProjectDirs::from("com", "tearminal", "tearminal")
        .map(|dirs| dirs.data_local_dir().join("db-backups"))
        .unwrap_or_else(|| crate::config::cache_dir_or_fallback().join("db-backups"))
}

fn file_safe(name: &str) -> String {
    name.chars()
        .take(40)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Una línea JSON por sentencia. Nunca sobrescribe un archivo existente.
fn write_backup(
    dir: &Path,
    connection_name: &str,
    entries: &[BackupEntry],
    unix_ms: u128,
) -> std::io::Result<PathBuf> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}-{unix_ms}.jsonl", file_safe(connection_name)));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    for entry in entries {
        let columns: Vec<String> = entry.columns.iter().map(|c| json_string(c)).collect();
        let rows: Vec<String> = entry
            .rows
            .iter()
            .map(|row| {
                format!(
                    "[{}]",
                    row.iter().map(json_value).collect::<Vec<_>>().join(",")
                )
            })
            .collect();
        writeln!(
            file,
            "{{\"connection\":{},\"unix_ms\":{unix_ms},\"statement\":{},\"table\":{},\"sql\":{},\"columns\":[{}],\"rows\":[{}]}}",
            json_string(connection_name),
            entry.statement,
            json_string(&entry.table),
            json_string(&entry.sql),
            columns.join(","),
            rows.join(",")
        )?;
    }
    file.sync_all()?;
    Ok(path)
}

impl Session {
    /// Filas que tocaría cada sentencia (`SELECT COUNT(*)` con el mismo filtro), para la pantalla de confirmación
    pub(crate) fn estimate_impact(
        &mut self,
        sql: &str,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<Vec<Impact>, DbError> {
        let analysis = self.analyze(sql);
        let runtime = runtime()?;
        let mut impacts = Vec::new();
        for (index, statement) in analysis.statements.iter().enumerate() {
            let rows = match statement.impact_query() {
                Some(count_sql) => {
                    let set = runtime.block_on(self.run_raw(&count_sql, limits, cancel))?;
                    set.rows
                        .first()
                        .and_then(|row| row.first())
                        .and_then(|cell| cell.raw())
                        .and_then(|text| text.parse().ok())
                }
                None => None,
            };
            impacts.push(Impact {
                statement: index,
                rows,
            });
        }
        Ok(impacts)
    }

    /// Aplica `sql` respetando la política: autorización (solo lectura, confirmación escrita, peligros), copia de
    /// las filas originales, transacción y reversión ante el primer error. Las sentencias `BEGIN` / `COMMIT` del
    /// texto se ignoran: la transacción la pone esta función.
    pub(crate) fn apply(
        &mut self,
        sql: &str,
        confirmation: &WriteConfirmation,
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<ApplyReport, DbError> {
        let analysis = self.analyze(sql);
        let statements: Vec<&StatementInfo> = analysis
            .statements
            .iter()
            .filter(|s| s.kind != StatementKind::Transaction)
            .collect();
        if statements.is_empty() {
            return Err(SafetyError::Empty.into());
        }
        let now = Instant::now();
        self.authorize(&analysis, confirmation, now)?;
        runtime()?.block_on(self.apply_authorized(&statements, limits, cancel, now))
    }

    async fn apply_authorized(
        &mut self,
        statements: &[&StatementInfo],
        limits: &QueryLimits,
        cancel: &CancelToken,
        now: Instant,
    ) -> Result<ApplyReport, DbError> {
        self.sync_access(now).await?;
        let mut report = ApplyReport::default();
        if self.policy.backup_original_rows {
            let (file, skipped) = self.backup_originals(statements, limits, cancel).await?;
            report.backup_file = file;
            report.backup_skipped = skipped;
        }

        let use_transaction = self.policy.use_transaction;
        if use_transaction {
            let begin = self.connection.engine.begin_sql();
            self.exec_raw(begin, limits, cancel).await?;
        }
        for (index, statement) in statements.iter().enumerate() {
            match self.exec_execute(&statement.sql, limits, cancel).await {
                Ok(count) => report.affected.push(count),
                Err(error) => return Err(self.fail_apply(index, &error, use_transaction).await),
            }
        }
        if use_transaction {
            if let Err(error) = self.exec_raw("COMMIT", limits, cancel).await {
                return Err(self.fail_apply(statements.len(), &error, true).await);
            }
        }
        // Solo longitudes y recuentos: jamás el SQL ni los valores
        crate::logger::log(&format!(
            "db: cambios aplicados sentencias={} transaccion={} copia={}",
            statements.len(),
            use_transaction,
            report.backup_file.is_some()
        ));
        Ok(report)
    }

    /// Revierte (si había transacción) y arma el error. Sin transacción, las sentencias anteriores siguen aplicadas.
    async fn fail_apply(&mut self, index: usize, error: &DbError, in_transaction: bool) -> DbError {
        let rolled_back = in_transaction && self.rollback().await;
        DbError::ApplyFailed {
            index,
            message: error.to_string(),
            rolled_back,
        }
    }

    /// `true` si se ejecutó el `ROLLBACK`. Si no se puede, se cierra la conexión (el servidor revierte al perderla)
    /// y se devuelve `false`: no hay confirmación de que se haya revertido.
    async fn rollback(&mut self) -> bool {
        let limits = QueryLimits {
            timeout: std::time::Duration::from_secs(10),
            max_rows: 1,
        };
        if self.connection_is_open()
            && self
                .exec_raw("ROLLBACK", &limits, &CancelToken::new())
                .await
                .is_ok()
        {
            return true;
        }
        self.drop_connection();
        false
    }

    /// Guarda las filas que van a cambiar. Falla (sin tocar nada) si no puede leerlas o guardarlas completas.
    async fn backup_originals(
        &mut self,
        statements: &[&StatementInfo],
        limits: &QueryLimits,
        cancel: &CancelToken,
    ) -> Result<(Option<PathBuf>, usize), DbError> {
        let mut entries = Vec::new();
        let mut skipped = 0;
        for (index, statement) in statements.iter().enumerate() {
            if !matches!(
                statement.kind,
                StatementKind::Update | StatementKind::Delete | StatementKind::Truncate
            ) {
                continue;
            }
            let Some(select) = statement.original_rows_query() else {
                skipped += 1;
                continue;
            };
            let set = self.exec_raw(&select, limits, cancel).await.map_err(|e| {
                DbError::Refused(format!(
                    "no se pudieron leer las filas originales ({e}); desactiva la copia de seguridad para aplicar sin ella"
                ))
            })?;
            if set.truncated {
                return Err(DbError::Refused(format!(
                    "la sentencia {index} toca más de {} filas: no se puede guardar la copia completa",
                    limits.max_rows
                )));
            }
            entries.push(BackupEntry {
                statement: index,
                sql: statement.sql.clone(),
                table: statement.table.clone().unwrap_or_default(),
                columns: set.columns,
                rows: set.rows,
            });
        }
        if entries.is_empty() {
            return Ok((None, skipped));
        }
        let unix_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path =
            write_backup(&backup_dir(), &self.connection.name, &entries, unix_ms).map_err(|e| {
                DbError::Refused(format!(
                    "no se pudo guardar la copia de las filas originales ({}); no se aplicó nada",
                    e.kind()
                ))
            })?;
        Ok((Some(path), skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_strings_escape_quotes_controls_and_unicode_survives() {
        assert_eq!(
            json_string("a\"b\\c\nd\u{1b}é"),
            "\"a\\\"b\\\\c\\nd\\u001bé\""
        );
        assert_eq!(json_value(&Cell::new(None)), "null");
        assert_eq!(json_value(&Cell::new(Some("x".into()))), "\"x\"");
    }

    #[test]
    fn backup_file_holds_one_json_line_per_statement_and_is_never_overwritten() {
        let dir =
            std::env::temp_dir().join(format!("tearminal-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let entry = BackupEntry {
            statement: 1,
            sql: "UPDATE users SET name = 'x' WHERE id = 5".into(),
            table: "users".into(),
            columns: vec!["id".into(), "name".into()],
            rows: vec![
                vec![
                    Cell::new(Some("5".into())),
                    Cell::new(Some("Elena\n\"V\"".into())),
                ],
                vec![Cell::new(Some("6".into())), Cell::new(None)],
            ],
        };
        let path = write_backup(&dir, "pg-main/../x", &[entry], 1234).unwrap();
        // El nombre de la conexión no puede escapar de la carpeta
        assert_eq!(path.parent(), Some(dir.as_path()));
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("pg-main____x-1234.jsonl")
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("\"rows\":[[\"5\",\"Elena\\n\\\"V\\\"\"],[\"6\",null]]"));
        assert!(text.contains("\"columns\":[\"id\",\"name\"]"));
        assert!(write_backup(&dir, "pg-main/../x", &[], 1234).is_err());
    }

    #[test]
    fn file_safe_names_are_bounded() {
        assert_eq!(file_safe("a b/c"), "a_b_c");
        assert_eq!(file_safe(&"x".repeat(100)).len(), 40);
    }
}
