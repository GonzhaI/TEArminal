//! Reglas de seguridad del módulo de datos (DISENO §6): solo lectura en PROD, escritura temporal con caducidad,
//! confirmación escrita y detección de sentencias peligrosas (`DELETE` / `UPDATE` sin `WHERE`, `TRUNCATE`, `DROP`).
//!
//! El clasificador recorre el SQL ignorando comentarios, cadenas, identificadores entre comillas y bloques `$$`, así
//! que un `WHERE` o un `DELETE` dentro de un texto no engaña al análisis. Es **conservador**: ante la duda, trata la
//! sentencia como escritura.

use super::store::{Connection, Engine, Env};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Política configurable (F8 la expone en Ajustes → Bases de datos). Los valores por defecto son los del diseño.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct SecurityPolicy {
    pub(crate) prod_read_only: bool,
    pub(crate) write_window_secs: u64,
    pub(crate) require_typed_confirmation: bool,
    pub(crate) warn_dangerous: bool,
    pub(crate) use_transaction: bool,
    pub(crate) backup_original_rows: bool,
    pub(crate) warn_unsafe_pem: bool,
    pub(crate) cert_warn_days: u32,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            prod_read_only: true,
            write_window_secs: 10 * 60,
            require_typed_confirmation: true,
            warn_dangerous: true,
            use_transaction: true,
            backup_original_rows: true,
            warn_unsafe_pem: true,
            cert_warn_days: 30,
        }
    }
}

/// Límites de una consulta (DISENO §SettingsDb: 30 s y 10 000 filas)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryLimits {
    pub(crate) timeout: Duration,
    pub(crate) max_rows: usize,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_rows: 10_000,
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Modo de acceso y escritura temporal
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessMode {
    ReadOnly,
    ReadWrite,
}

/// Concesión de escritura de una pestaña: caduca sola. El tiempo se pasa como parámetro para poder probarlo.
#[derive(Debug, Clone, Default)]
pub(crate) struct WriteGrant {
    expires_at: Option<Instant>,
}

impl WriteGrant {
    pub(crate) fn enable(&mut self, now: Instant, window: Duration) {
        self.expires_at = Some(now + window);
    }

    pub(crate) fn disable(&mut self) {
        self.expires_at = None;
    }

    pub(crate) fn remaining(&self, now: Instant) -> Option<Duration> {
        self.expires_at?.checked_duration_since(now)
    }

    pub(crate) fn is_active(&self, now: Instant) -> bool {
        self.remaining(now).is_some_and(|left| !left.is_zero())
    }
}

/// Modo efectivo de una pestaña. `read_only` de la conexión es permanente (la concesión no lo levanta); el valor
/// por defecto de PROD sí se levanta con una concesión vigente.
pub(crate) fn access_mode(
    connection: &Connection,
    policy: &SecurityPolicy,
    grant: &WriteGrant,
    now: Instant,
) -> AccessMode {
    if connection.read_only {
        return AccessMode::ReadOnly;
    }
    if connection.env == Env::Prod && policy.prod_read_only && !grant.is_active(now) {
        return AccessMode::ReadOnly;
    }
    AccessMode::ReadWrite
}

/// La confirmación escrita exige el nombre exacto de la conexión (sin recortar ni ignorar mayúsculas)
pub(crate) fn confirmation_matches(typed: &str, connection_name: &str) -> bool {
    !connection_name.is_empty() && typed == connection_name
}

// -------------------------------------------------------------------------------------------------
// Clasificador de SQL
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatementKind {
    /// SELECT / SHOW / EXPLAIN / VALUES / DESCRIBE sin nada que escriba
    Read,
    Insert,
    Update,
    Delete,
    Truncate,
    Drop,
    /// CREATE / ALTER / RENAME / GRANT…
    Ddl,
    /// BEGIN / COMMIT / ROLLBACK / START TRANSACTION
    Transaction,
    /// Cualquier otra cosa (CALL, EXEC, SET…): se trata como escritura
    Other,
}

impl StatementKind {
    pub(crate) fn is_write(self) -> bool {
        !matches!(self, Self::Read | Self::Transaction)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Danger {
    UpdateWithoutWhere,
    DeleteWithoutWhere,
    Truncate,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatementInfo {
    /// Texto de la sentencia (sin el `;` final), para ejecutarlas una a una
    pub(crate) sql: String,
    pub(crate) kind: StatementKind,
    pub(crate) has_where: bool,
    /// Tabla de `DELETE FROM t` / `UPDATE t` (texto tal cual, para derivar el conteo y la copia de filas)
    pub(crate) table: Option<String>,
    /// Texto desde `WHERE` hasta el final de la sentencia
    pub(crate) where_clause: Option<String>,
    /// Peligros de esta sentencia, incluidos los de sentencias anidadas en paréntesis (CTE con `DELETE`)
    pub(crate) dangers: Vec<Danger>,
}

impl StatementInfo {
    /// `SELECT COUNT(*)` equivalente al `UPDATE`/`DELETE` de una sola tabla (impacto estimado de DbConfirm)
    pub(crate) fn impact_query(&self) -> Option<String> {
        let table = self.target()?;
        Some(format!("SELECT COUNT(*) FROM {table}{}", self.filter()))
    }

    /// `SELECT *` de las filas que va a tocar el `UPDATE`/`DELETE` (copia de las filas originales)
    pub(crate) fn original_rows_query(&self) -> Option<String> {
        let table = self.target()?;
        Some(format!("SELECT * FROM {table}{}", self.filter()))
    }

    fn target(&self) -> Option<&str> {
        match self.kind {
            StatementKind::Update | StatementKind::Delete => self.table.as_deref(),
            _ => None,
        }
    }

    fn filter(&self) -> String {
        self.where_clause
            .as_deref()
            .map(|w| format!(" {w}"))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Analysis {
    pub(crate) statements: Vec<StatementInfo>,
}

impl Analysis {
    pub(crate) fn is_read_only(&self) -> bool {
        self.statements.iter().all(|s| !s.kind.is_write())
    }

    pub(crate) fn dangers(&self) -> impl Iterator<Item = (usize, Danger)> + '_ {
        self.statements
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.dangers.iter().map(move |d| (i, *d)))
    }

    pub(crate) fn is_multi_statement(&self) -> bool {
        self.statements.len() > 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SafetyError {
    #[error("la conexión está en solo lectura: la sentencia escribiría datos")]
    WriteInReadOnly,
    #[error("sentencia vacía")]
    Empty,
}

/// Rechaza el SQL que escribiría cuando el modo es solo lectura (defensa en la aplicación; PostgreSQL y MySQL además
/// lo aplican en el servidor, SQL Server no tiene modo de solo lectura por sesión)
pub(crate) fn check_allowed(analysis: &Analysis, mode: AccessMode) -> Result<(), SafetyError> {
    if analysis.statements.is_empty() {
        return Err(SafetyError::Empty);
    }
    if mode == AccessMode::ReadOnly && !analysis.is_read_only() {
        return Err(SafetyError::WriteInReadOnly);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    /// Cualquier literal o identificador entrecomillado (su contenido no cuenta como palabras clave)
    Quoted,
    Open,
    Close,
    Semicolon,
    Other(char),
}

#[derive(Debug, Clone)]
struct Spanned {
    token: Token,
    start: usize,
    end: usize,
}

fn tokenize(sql: &str, engine: Engine) -> Vec<Spanned> {
    let bytes = sql.as_bytes();
    let mysql_like = matches!(engine, Engine::Mysql | Engine::Mariadb);
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        let token = match b {
            b if b.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i = skip_line(bytes, i);
                continue;
            }
            b'#' if mysql_like => {
                i = skip_line(bytes, i);
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(
                    bytes,
                    i,
                    engine != Engine::Mysql && engine != Engine::Mariadb,
                );
                continue;
            }
            b'\'' => {
                i = skip_quoted(bytes, i, b'\'', mysql_like);
                Token::Quoted
            }
            b'"' => {
                i = skip_quoted(bytes, i, b'"', mysql_like);
                Token::Quoted
            }
            b'`' => {
                i = skip_quoted(bytes, i, b'`', false);
                Token::Quoted
            }
            b'[' if engine == Engine::SqlServer => {
                i = skip_quoted(bytes, i, b']', false);
                Token::Quoted
            }
            b'$' if engine == Engine::Postgres => match dollar_quote_end(bytes, i) {
                Some(end) => {
                    i = end;
                    Token::Quoted
                }
                None => {
                    i += 1;
                    Token::Other('$')
                }
            },
            b'(' => {
                i += 1;
                Token::Open
            }
            b')' => {
                i += 1;
                Token::Close
            }
            b';' => {
                i += 1;
                Token::Semicolon
            }
            b if b.is_ascii_alphabetic() || b == b'_' || b >= 0x80 => {
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] >= 0x80)
                {
                    i += 1;
                }
                Token::Word(sql[start..i].to_ascii_lowercase())
            }
            b if b.is_ascii_digit() => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                    i += 1;
                }
                Token::Quoted
            }
            _ => {
                i += 1;
                // Un byte no ASCII ya se trató arriba; aquí solo hay símbolos de un byte
                Token::Other(b as char)
            }
        };
        tokens.push(Spanned {
            token,
            start,
            end: i,
        });
    }
    tokens
}

fn skip_line(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

/// `nested`: PostgreSQL y SQL Server anidan `/* /* */ */`; MySQL no
fn skip_block_comment(bytes: &[u8], mut i: usize, nested: bool) -> usize {
    let mut depth = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
            depth = depth.saturating_sub(1);
            i += 2;
            if depth == 0 || !nested {
                return i;
            }
        } else {
            i += 1;
        }
    }
    bytes.len()
}

/// Salta una cadena o identificador entrecomillado. La comilla se duplica para escaparse; MySQL además admite `\`.
fn skip_quoted(bytes: &[u8], open: usize, close: u8, backslash_escapes: bool) -> usize {
    let mut i = open + 1;
    while i < bytes.len() {
        if backslash_escapes && bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == close {
            if bytes.get(i + 1) == Some(&close) {
                i += 2;
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    bytes.len()
}

/// `$$ … $$` o `$etiqueta$ … $etiqueta$`. `None` si no es una apertura (por ejemplo `$1`).
fn dollar_quote_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start + 1;
    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
    }
    if bytes.get(j) != Some(&b'$') || bytes.get(start + 1).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let tag = &bytes[start..=j];
    let body = j + 1;
    let found = bytes[body..]
        .windows(tag.len())
        .position(|window| window == tag)?;
    Some(body + found + tag.len())
}

#[derive(Default)]
struct Frame {
    first: Option<String>,
    /// Primera palabra DML en este nivel (`WITH x AS (…) DELETE …`)
    dml: Option<String>,
    has_where: bool,
    tokens: usize,
    start: Option<usize>,
    table_start: Option<usize>,
    table_end: Option<usize>,
    where_start: Option<usize>,
    where_end: Option<usize>,
}

impl Frame {
    /// Palabra que define qué hace la sentencia (tras un `WITH`, la primera DML)
    fn head(&self) -> Option<&str> {
        match self.first.as_deref() {
            Some("with") => self.dml.as_deref(),
            other => other,
        }
    }
}

const DML_WORDS: [&str; 5] = ["select", "insert", "update", "delete", "merge"];
/// Palabras que, en cualquier posición, hacen que una sentencia de lectura escriba o bloquee datos
/// (`REPLACE` y `ANALYZE` no están: son funciones / opciones habituales y el `REPLACE` de MySQL se detecta al inicio)
const WRITE_WORDS: [&str; 14] = [
    "insert", "update", "delete", "merge", "truncate", "drop", "alter", "create", "grant",
    "revoke", "into", "call", "exec", "execute",
];

fn classify(frame: &Frame, any_write_word: bool) -> StatementKind {
    let head = match frame.first.as_deref() {
        Some("with") => frame.dml.as_deref().unwrap_or("select"),
        Some(word) => word,
        None => return StatementKind::Other,
    };
    match head {
        "select" | "show" | "explain" | "values" | "describe" | "desc" | "table" => {
            if any_write_word {
                StatementKind::Other
            } else {
                StatementKind::Read
            }
        }
        "insert" | "replace" => StatementKind::Insert,
        "update" => StatementKind::Update,
        "delete" => StatementKind::Delete,
        "truncate" => StatementKind::Truncate,
        "drop" => StatementKind::Drop,
        "create" | "alter" | "rename" | "grant" | "revoke" | "comment" => StatementKind::Ddl,
        "begin" | "commit" | "rollback" | "start" | "savepoint" | "release" | "end" => {
            StatementKind::Transaction
        }
        _ => StatementKind::Other,
    }
}

fn dangers_of(kind: StatementKind, has_where: bool) -> Vec<Danger> {
    match kind {
        StatementKind::Update if !has_where => vec![Danger::UpdateWithoutWhere],
        StatementKind::Delete if !has_where => vec![Danger::DeleteWithoutWhere],
        StatementKind::Truncate => vec![Danger::Truncate],
        StatementKind::Drop => vec![Danger::Drop],
        _ => Vec::new(),
    }
}

/// Estado del recorrido: una pila de marcos (uno por nivel de paréntesis) y lo acumulado de la sentencia actual
struct Walker<'a> {
    sql: &'a str,
    stack: Vec<Frame>,
    statements: Vec<StatementInfo>,
    /// Peligros de sentencias DML anidadas en paréntesis (CTE con `DELETE … RETURNING`): suben a la sentencia externa
    nested_dangers: Vec<Danger>,
    any_write_word: bool,
}

impl<'a> Walker<'a> {
    fn new(sql: &'a str) -> Self {
        Self {
            sql,
            stack: vec![Frame::default()],
            statements: Vec::new(),
            nested_dangers: Vec::new(),
            any_write_word: false,
        }
    }

    fn depth(&self) -> usize {
        self.stack.len() - 1
    }

    fn frame(&mut self) -> &mut Frame {
        let depth = self.depth();
        &mut self.stack[depth]
    }

    fn word(&mut self, word: &str, span: &Spanned) {
        if WRITE_WORDS.contains(&word) {
            self.any_write_word = true;
        }
        let frame = self.frame();
        frame.tokens += 1;
        if frame.first.is_none() {
            frame.first = Some(word.to_string());
        } else if frame.dml.is_none()
            && frame.first.as_deref() == Some("with")
            && DML_WORDS.contains(&word)
        {
            frame.dml = Some(word.to_string());
        }
        let head = frame.head().map(str::to_string);
        match word {
            "where" if frame.where_start.is_none() => {
                frame.has_where = true;
                frame.where_start = Some(span.start);
            }
            "from" if head.as_deref() == Some("delete") && frame.table_start.is_none() => {
                frame.table_start = Some(span.end);
            }
            "update" if head.as_deref() == Some("update") && frame.table_start.is_none() => {
                frame.table_start = Some(span.end);
            }
            "set" | "using" | "output" | "returning" | "where"
                if frame.table_start.is_some() && frame.table_end.is_none() =>
            {
                frame.table_end = Some(span.start);
            }
            _ => {}
        }
        // `RETURNING` / `OUTPUT` cierran el filtro: no caben en un `SELECT COUNT(*)`
        if matches!(word, "returning" | "output")
            && frame.where_start.is_some()
            && frame.where_end.is_none()
        {
            frame.where_end = Some(span.start);
        }
    }

    fn close_paren(&mut self) {
        if self.depth() == 0 {
            self.frame().tokens += 1;
            return;
        }
        let Some(inner) = self.stack.pop() else {
            return;
        };
        let kind = classify(&inner, false);
        self.nested_dangers
            .extend(dangers_of(kind, inner.has_where));
        // `(1)` o `f(a, b)` no son sentencias: solo cuenta un DML / DDL anidado de verdad
        if !matches!(
            kind,
            StatementKind::Read | StatementKind::Transaction | StatementKind::Other
        ) {
            self.any_write_word = true;
        }
    }

    fn finish_statement(&mut self, end: usize) {
        let frame = std::mem::take(&mut self.stack[0]);
        self.stack.truncate(1);
        let nested = std::mem::take(&mut self.nested_dangers);
        let any_write = std::mem::take(&mut self.any_write_word);
        if frame.tokens == 0 {
            return;
        }
        let kind = classify(&frame, any_write);
        let mut dangers = dangers_of(kind, frame.has_where);
        dangers.extend(nested);
        let slice = |from: usize, to: usize| self.sql[from..to.max(from)].trim().to_string();
        let table = frame
            .table_start
            .map(|start| slice(start, frame.table_end.unwrap_or(end)));
        let where_clause = frame
            .where_start
            .map(|start| slice(start, frame.where_end.unwrap_or(end)));
        let sql = frame
            .start
            .map(|start| slice(start, end))
            .unwrap_or_default();
        self.statements.push(StatementInfo {
            sql,
            kind,
            has_where: frame.has_where,
            table,
            where_clause,
            dangers,
        });
    }
}

/// Analiza `sql` (una o varias sentencias separadas por `;`).
// ponytail: `WHERE 1=1` cuenta como filtro y un `UPDATE` con subconsulta correlacionada también; detectar
// tautologías exigiría un parser completo de cada dialecto.
pub(crate) fn analyze(sql: &str, engine: Engine) -> Analysis {
    let mut walker = Walker::new(sql);
    for span in tokenize(sql, engine) {
        walker.stack[0].start.get_or_insert(span.start);
        match &span.token {
            Token::Semicolon if walker.depth() == 0 => walker.finish_statement(span.start),
            Token::Open => {
                walker.frame().tokens += 1;
                walker.stack.push(Frame::default());
            }
            Token::Close => walker.close_paren(),
            Token::Word(word) => walker.word(word, &span),
            _ => walker.frame().tokens += 1,
        }
    }
    walker.finish_statement(sql.len());
    Analysis {
        statements: walker.statements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::store::Connection;

    fn pg(sql: &str) -> Analysis {
        analyze(sql, Engine::Postgres)
    }

    fn kinds(analysis: &Analysis) -> Vec<StatementKind> {
        analysis.statements.iter().map(|s| s.kind).collect()
    }

    #[test]
    fn reads_are_read_only() {
        for sql in [
            "SELECT * FROM users",
            "  select 1; select 2",
            "WITH t AS (SELECT 1) SELECT * FROM t",
            "EXPLAIN SELECT 1",
            "SHOW TABLES",
            "VALUES (1)",
        ] {
            assert!(pg(sql).is_read_only(), "{sql}");
        }
    }

    #[test]
    fn writes_are_never_read_only() {
        for sql in [
            "INSERT INTO t VALUES (1)",
            "UPDATE t SET a = 1 WHERE id = 1",
            "DELETE FROM t WHERE id = 1",
            "TRUNCATE t",
            "DROP TABLE t",
            "CREATE TABLE t (a int)",
            "SELECT * INTO copia FROM t",
            "SELECT * FROM t FOR UPDATE",
            "EXPLAIN ANALYZE DELETE FROM t",
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            "SELECT 1; DELETE FROM t WHERE id = 1",
            "CALL borrar_todo()",
            "SELECT pg_sleep(1); TRUNCATE t",
        ] {
            assert!(!pg(sql).is_read_only(), "{sql}");
        }
    }

    #[test]
    fn delete_and_update_without_where_are_dangerous() {
        let a = pg("DELETE FROM users");
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(0, Danger::DeleteWithoutWhere)]
        );
        let a = pg("update users set active = false");
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(0, Danger::UpdateWithoutWhere)]
        );
        assert_eq!(pg("TRUNCATE TABLE users").dangers().count(), 1);
        assert_eq!(
            pg("DROP TABLE users").dangers().next(),
            Some((0, Danger::Drop))
        );
    }

    #[test]
    fn filtered_statements_are_not_dangerous() {
        assert_eq!(pg("DELETE FROM users WHERE id = 4").dangers().count(), 0);
        assert_eq!(
            pg("UPDATE users SET a = 1 WHERE id = 4").dangers().count(),
            0
        );
    }

    #[test]
    fn where_inside_a_subquery_does_not_count() {
        let a = pg("DELETE FROM users USING (SELECT id FROM x WHERE a = 1) s");
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(0, Danger::DeleteWithoutWhere)]
        );
        let a = pg("UPDATE t SET a = (SELECT max(b) FROM u WHERE u.id = 1)");
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(0, Danger::UpdateWithoutWhere)]
        );
    }

    #[test]
    fn where_in_strings_and_comments_does_not_count() {
        for sql in [
            "DELETE FROM users -- WHERE id = 1",
            "DELETE FROM users /* WHERE id = 1 */",
            "DELETE FROM notes; UPDATE x SET a = 'WHERE id = 1'",
            "DELETE FROM \"where\"",
            "DELETE FROM users WHERE", // `WHERE` vacío: el servidor lo rechazará, no es un borrado total
        ] {
            let a = pg(sql);
            let has_danger = a.dangers().count() > 0;
            assert_eq!(has_danger, !sql.ends_with("WHERE"), "{sql}");
        }
    }

    #[test]
    fn keywords_inside_strings_do_not_make_a_read_a_write() {
        assert!(pg("SELECT 'DELETE FROM users; DROP TABLE x'").is_read_only());
        assert!(pg("SELECT $$ DELETE FROM x $$").is_read_only());
        assert!(pg("SELECT 1 /* UPDATE x SET a=1 */").is_read_only());
        assert!(pg("SELECT \"update\" FROM t").is_read_only());
    }

    #[test]
    fn mysql_backslash_escapes_keep_the_string_open() {
        // En MySQL `\'` no cierra la cadena: el DELETE siguiente está dentro del texto
        let sql = r"SELECT 'a\'; DELETE FROM users; --'";
        assert!(analyze(sql, Engine::Mysql).is_read_only());
        // En PostgreSQL sí la cierra: hay un DELETE real
        assert!(!analyze(sql, Engine::Postgres).is_read_only());
    }

    #[test]
    fn sql_server_brackets_and_nested_comments() {
        assert!(analyze("SELECT [delete] FROM [drop table]", Engine::SqlServer).is_read_only());
        assert!(pg("SELECT 1 /* a /* b */ DELETE FROM x */").is_read_only());
    }

    #[test]
    fn data_modifying_cte_is_flagged_through_the_outer_statement() {
        let a = pg("WITH d AS (DELETE FROM users RETURNING *) SELECT count(*) FROM d");
        assert!(!a.is_read_only());
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(0, Danger::DeleteWithoutWhere)]
        );
        let a = pg("WITH x AS (SELECT 1) DELETE FROM users");
        assert_eq!(kinds(&a), [StatementKind::Delete]);
    }

    #[test]
    fn multi_statement_is_reported_with_index_of_the_dangerous_one() {
        let a = pg("SELECT 1; UPDATE t SET a = 1; DELETE FROM t WHERE id = 1;");
        assert!(a.is_multi_statement());
        assert_eq!(a.statements.len(), 3);
        assert_eq!(
            a.dangers().collect::<Vec<_>>(),
            [(1, Danger::UpdateWithoutWhere)]
        );
    }

    #[test]
    fn insert_on_conflict_update_is_an_insert() {
        let a = pg("INSERT INTO t (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET a = 2");
        assert_eq!(kinds(&a), [StatementKind::Insert]);
        assert_eq!(a.dangers().count(), 0);
    }

    #[test]
    fn impact_and_backup_queries_for_simple_statements() {
        let a = pg("UPDATE users SET active = true WHERE id = 4");
        assert_eq!(
            a.statements[0].impact_query().as_deref(),
            Some("SELECT COUNT(*) FROM users WHERE id = 4")
        );
        assert_eq!(
            a.statements[0].original_rows_query().as_deref(),
            Some("SELECT * FROM users WHERE id = 4")
        );
        let a = pg("DELETE FROM public.users");
        assert_eq!(
            a.statements[0].impact_query().as_deref(),
            Some("SELECT COUNT(*) FROM public.users")
        );
        assert!(pg("INSERT INTO t VALUES (1)").statements[0]
            .impact_query()
            .is_none());
    }

    #[test]
    fn read_only_blocks_writes_and_allows_reads() {
        assert_eq!(
            check_allowed(&pg("DELETE FROM t WHERE id = 1"), AccessMode::ReadOnly),
            Err(SafetyError::WriteInReadOnly)
        );
        assert_eq!(check_allowed(&pg("SELECT 1"), AccessMode::ReadOnly), Ok(()));
        assert_eq!(
            check_allowed(&pg("DELETE FROM t WHERE id = 1"), AccessMode::ReadWrite),
            Ok(())
        );
        assert_eq!(
            check_allowed(&pg("  -- nada"), AccessMode::ReadWrite),
            Err(SafetyError::Empty)
        );
    }

    fn connection(env: Env, read_only: bool) -> Connection {
        let mut c = Connection::new("pg-main", Engine::Postgres);
        c.env = env;
        c.read_only = read_only;
        c
    }

    #[test]
    fn prod_opens_read_only_and_write_expires() {
        let policy = SecurityPolicy::default();
        let prod = connection(Env::Prod, false);
        let t0 = Instant::now();
        let mut grant = WriteGrant::default();
        assert_eq!(
            access_mode(&prod, &policy, &grant, t0),
            AccessMode::ReadOnly
        );

        grant.enable(t0, Duration::from_secs(policy.write_window_secs));
        assert_eq!(policy.write_window_secs, 600);
        assert_eq!(
            access_mode(&prod, &policy, &grant, t0),
            AccessMode::ReadWrite
        );
        let almost = t0 + Duration::from_secs(599);
        assert_eq!(
            access_mode(&prod, &policy, &grant, almost),
            AccessMode::ReadWrite
        );
        assert_eq!(grant.remaining(almost), Some(Duration::from_secs(1)));
        let expired = t0 + Duration::from_secs(600);
        assert_eq!(
            access_mode(&prod, &policy, &grant, expired),
            AccessMode::ReadOnly
        );

        grant.enable(t0, Duration::from_secs(600));
        grant.disable();
        assert_eq!(
            access_mode(&prod, &policy, &grant, t0),
            AccessMode::ReadOnly
        );
    }

    #[test]
    fn explicit_read_only_connection_ignores_the_grant() {
        let policy = SecurityPolicy::default();
        let mut grant = WriteGrant::default();
        let t0 = Instant::now();
        grant.enable(t0, Duration::from_secs(600));
        for env in [Env::Local, Env::Stg, Env::Prod] {
            assert_eq!(
                access_mode(&connection(env, true), &policy, &grant, t0),
                AccessMode::ReadOnly
            );
        }
    }

    #[test]
    fn non_prod_is_writable_and_policy_can_open_prod_writable() {
        let t0 = Instant::now();
        let grant = WriteGrant::default();
        let policy = SecurityPolicy::default();
        assert_eq!(
            access_mode(&connection(Env::Stg, false), &policy, &grant, t0),
            AccessMode::ReadWrite
        );
        let lax = SecurityPolicy {
            prod_read_only: false,
            ..policy
        };
        assert_eq!(
            access_mode(&connection(Env::Prod, false), &lax, &grant, t0),
            AccessMode::ReadWrite
        );
    }

    #[test]
    fn typed_confirmation_requires_exact_connection_name() {
        assert!(confirmation_matches("pg-main", "pg-main"));
        assert!(!confirmation_matches("PG-MAIN", "pg-main"));
        assert!(!confirmation_matches("pg-main ", "pg-main"));
        assert!(!confirmation_matches("", ""));
    }

    #[test]
    fn defaults_match_the_design() {
        let p = SecurityPolicy::default();
        assert!(p.prod_read_only && p.use_transaction && p.backup_original_rows);
        assert_eq!((p.write_window_secs, p.cert_warn_days), (600, 30));
        let l = QueryLimits::default();
        assert_eq!((l.timeout.as_secs(), l.max_rows), (30, 10_000));
    }
}
