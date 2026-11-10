//! Pruebas de integración contra una base real por motor. Van con `#[ignore]` para que `cargo test` normal no dependa
//! de ningún servidor. Para correrlas:
//!
//! ```text
//! TEARMINAL_DB_TEST_POSTGRES=127.0.0.1,55432,postgres,clave,postgres   (host,puerto,usuario,clave,base)
//! TEARMINAL_DB_TEST_MYSQL=…   TEARMINAL_DB_TEST_MARIADB=…   TEARMINAL_DB_TEST_SQLSERVER=…
//! cargo test db::integration_tests -- --ignored --test-threads=1
//! ```
//!
//! Sin la variable del motor, la prueba termina sin hacer nada. Crean y borran las tablas `tearminal_t_users` y
//! `tearminal_t_orders` en la base indicada: usa una base desechable. La clave se guarda en el Administrador de
//! credenciales con un id temporal (así se prueba también esa ruta) y se borra al terminar.

use super::credentials::{self, Secret, SecretKind};
use super::driver::{runtime, CancelToken, DbError};
use super::safety::{Danger, QueryLimits, SafetyError, SecurityPolicy};
use super::schema::TableRef;
use super::session::{Session, WriteConfirmation};
use super::store::{Connection, Engine, Env, SslMode};
use std::time::{Duration, Instant};

struct Target {
    host: String,
    port: u16,
    user: String,
    password: String,
    database: String,
}

fn target(variable: &str) -> Option<Target> {
    let value = std::env::var(variable).ok()?;
    let parts: Vec<&str> = value.splitn(5, ',').collect();
    let [host, port, user, password, database] = parts.as_slice() else {
        panic!("{variable} debe tener el formato host,puerto,usuario,clave,base");
    };
    Some(Target {
        host: host.to_string(),
        port: port.parse().expect("puerto numérico"),
        user: user.to_string(),
        password: password.to_string(),
        database: database.to_string(),
    })
}

/// Borra la clave del Administrador de credenciales al salir, incluso si la prueba falla
struct StoredPassword(String);

impl Drop for StoredPassword {
    fn drop(&mut self) {
        let _ = credentials::delete_all(&self.0);
    }
}

fn connection(engine: Engine, name: &str, env: Env, t: &Target) -> (Connection, StoredPassword) {
    let mut c = Connection::new(name, engine);
    c.host = t.host.clone();
    c.port = t.port;
    c.user = t.user.clone();
    c.database = t.database.clone();
    c.env = env;
    c.tls.mode = SslMode::Disable;
    credentials::store(
        SecretKind::DbPassword,
        &c.id,
        &Secret::new(t.password.clone()),
    )
    .expect("Administrador de credenciales");
    let guard = StoredPassword(c.id.clone());
    (c, guard)
}

struct Dialect {
    create: [&'static str; 3],
    sleep: &'static str,
    control_chars_value: &'static str,
}

fn dialect(engine: Engine) -> Dialect {
    match engine {
        Engine::Postgres => Dialect {
            create: [
                "CREATE TABLE tearminal_t_users (id integer PRIMARY KEY, name text NOT NULL, note text)",
                "CREATE TABLE tearminal_t_orders (id integer PRIMARY KEY, user_id integer NOT NULL REFERENCES tearminal_t_users (id), total numeric(10,2))",
                "CREATE INDEX tearminal_t_orders_by_user ON tearminal_t_orders (user_id)",
            ],
            sleep: "SELECT pg_sleep(30)",
            control_chars_value: "'a' || chr(27) || '[31mred' || chr(10) || 'x'",
        },
        Engine::Mysql | Engine::Mariadb => Dialect {
            create: [
                "CREATE TABLE tearminal_t_users (id int PRIMARY KEY, name varchar(100) NOT NULL, note text)",
                "CREATE TABLE tearminal_t_orders (id int PRIMARY KEY, user_id int NOT NULL, total decimal(10,2), CONSTRAINT tearminal_t_orders_fk FOREIGN KEY (user_id) REFERENCES tearminal_t_users (id))",
                "CREATE INDEX tearminal_t_orders_by_user ON tearminal_t_orders (user_id)",
            ],
            sleep: "SELECT SLEEP(30)",
            control_chars_value: "CONCAT('a', CHAR(27), '[31mred', CHAR(10), 'x')",
        },
        Engine::SqlServer => Dialect {
            create: [
                "CREATE TABLE tearminal_t_users (id int PRIMARY KEY, name nvarchar(100) NOT NULL, note nvarchar(max))",
                "CREATE TABLE tearminal_t_orders (id int PRIMARY KEY, user_id int NOT NULL, total decimal(10,2), CONSTRAINT tearminal_t_orders_fk FOREIGN KEY (user_id) REFERENCES tearminal_t_users (id))",
                "CREATE INDEX tearminal_t_orders_by_user ON tearminal_t_orders (user_id)",
            ],
            sleep: "WAITFOR DELAY '00:00:30'",
            control_chars_value: "'a' + CHAR(27) + '[31mred' + CHAR(10) + 'x'",
        },
    }
}

fn limits() -> QueryLimits {
    QueryLimits {
        timeout: Duration::from_secs(20),
        max_rows: 1000,
    }
}

fn run(session: &mut Session, sql: &str) -> Result<Vec<super::driver::ResultSet>, DbError> {
    session.query(
        sql,
        &limits(),
        &CancelToken::new(),
        &WriteConfirmation::default(),
    )
}

fn first_cell(session: &mut Session, sql: &str) -> Option<String> {
    let sets = run(session, sql).expect("consulta");
    sets[0].rows.first()?.first()?.raw().map(str::to_string)
}

fn drop_tables(session: &mut Session) {
    let _ = run(session, "DROP TABLE tearminal_t_orders");
    let _ = run(session, "DROP TABLE tearminal_t_users");
}

fn exercise(engine: Engine, variable: &str) {
    let Some(t) = target(variable) else {
        eprintln!("{variable} no definida: se omite");
        return;
    };
    let d = dialect(engine);
    let policy = SecurityPolicy::default();

    // ---- conexión de trabajo (LOCAL: escribe sin restricciones) ----
    let (local, _local_secret) = connection(engine, "it-local", Env::Local, &t);
    let mut session = Session::connect(&local, &policy).expect("conectar");
    assert!(session.ping(&CancelToken::new()).is_ok());
    drop_tables(&mut session);
    for statement in d.create {
        run(&mut session, statement).expect("crear tablas");
    }
    let values: Vec<String> = (1..=25).map(|i| format!("({i}, 'user{i}')")).collect();
    run(
        &mut session,
        &format!(
            "INSERT INTO tearminal_t_users (id, name) VALUES {}",
            values.join(", ")
        ),
    )
    .expect("insertar");
    run(
        &mut session,
        "INSERT INTO tearminal_t_orders (id, user_id, total) VALUES (1, 3, 10.50)",
    )
    .expect("pedido");

    // ---- límite de filas ----
    let small = QueryLimits {
        timeout: Duration::from_secs(20),
        max_rows: 10,
    };
    let sets = session
        .query(
            "SELECT * FROM tearminal_t_users ORDER BY id",
            &small,
            &CancelToken::new(),
            &WriteConfirmation::default(),
        )
        .expect("límite");
    assert_eq!(sets[0].rows.len(), 10);
    assert!(sets[0].truncated);
    assert_eq!(sets[0].columns.len(), 3);
    // La conexión sigue sirviendo tras cortar por el límite
    assert_eq!(
        first_cell(&mut session, "SELECT COUNT(*) FROM tearminal_t_users").as_deref(),
        Some("25")
    );

    // ---- caracteres de control del servidor: crudos para exportar, escapados para mostrar ----
    run(
        &mut session,
        &format!(
            "INSERT INTO tearminal_t_users (id, name) VALUES (100, {})",
            d.control_chars_value
        ),
    )
    .expect("insertar caracteres de control");
    let sets = run(
        &mut session,
        "SELECT name FROM tearminal_t_users WHERE id = 100",
    )
    .expect("leer");
    let cell = &sets[0].rows[0][0];
    assert!(cell
        .raw()
        .is_some_and(|v| v.contains('\u{1b}') && v.contains('\n')));
    assert_eq!(cell.display().as_deref(), Some("a\\x1b[31mred\\nx"));
    run(&mut session, "DELETE FROM tearminal_t_users WHERE id = 100").expect("limpiar");

    // ---- esquema ----
    let schema = session.schema(&CancelToken::new()).expect("esquema");
    let users = schema
        .tables
        .iter()
        .find(|t| t.reference.name == "tearminal_t_users")
        .expect("tabla users");
    assert_eq!(
        users
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "name", "note"]
    );
    assert!(!users.columns[1].nullable && users.columns[2].nullable);
    assert_eq!(users.primary_key(), ["id"]);
    let orders = schema
        .tables
        .iter()
        .find(|t| t.reference.name == "tearminal_t_orders")
        .expect("tabla orders");
    assert!(orders
        .indexes
        .iter()
        .any(|i| i.name == "tearminal_t_orders_by_user" && i.columns == ["user_id"]));
    let relation = schema
        .relations
        .iter()
        .find(|r| r.from.name == "tearminal_t_orders")
        .expect("relación");
    assert_eq!(relation.to.name, "tearminal_t_users");
    assert_eq!(
        (
            relation.from_columns.as_slice(),
            relation.to_columns.as_slice()
        ),
        (&["user_id".to_string()][..], &["id".to_string()][..])
    );
    assert!(schema
        .table(&TableRef {
            schema: users.reference.schema.clone(),
            name: "tearminal_t_users".into()
        })
        .is_some());

    // ---- tiempo máximo y cancelación (y la conexión se recupera) ----
    let started = Instant::now();
    let tight = QueryLimits {
        timeout: Duration::from_secs(1),
        max_rows: 10,
    };
    let result = session.query(
        d.sleep,
        &tight,
        &CancelToken::new(),
        &WriteConfirmation::default(),
    );
    assert!(matches!(result, Err(DbError::Timeout)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_eq!(
        first_cell(&mut session, "SELECT COUNT(*) FROM tearminal_t_users").as_deref(),
        Some("25")
    );

    let cancel = CancelToken::new();
    let canceller = cancel.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        canceller.cancel();
    });
    let started = Instant::now();
    let result = session.query(d.sleep, &limits(), &cancel, &WriteConfirmation::default());
    handle.join().expect("hilo");
    assert!(matches!(result, Err(DbError::Cancelled)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_eq!(
        first_cell(&mut session, "SELECT COUNT(*) FROM tearminal_t_users").as_deref(),
        Some("25")
    );

    // ---- reglas de producción sobre la misma base ----
    let (prod, _prod_secret) = connection(engine, "it-prod", Env::Prod, &t);
    let mut prod_session = Session::connect(&prod, &policy).expect("conectar PROD");
    // PROD abre en solo lectura: las lecturas pasan y las escrituras no
    assert_eq!(
        first_cell(&mut prod_session, "SELECT COUNT(*) FROM tearminal_t_users").as_deref(),
        Some("25")
    );
    let blocked = run(
        &mut prod_session,
        "UPDATE tearminal_t_users SET note = 'x' WHERE id = 2",
    );
    assert!(
        matches!(blocked, Err(DbError::Safety(SafetyError::WriteInReadOnly))),
        "{blocked:?}"
    );
    // …y, donde el motor lo permite, también el servidor la rechaza aunque se salte el clasificador
    let raw_write = runtime().expect("runtime").block_on(prod_session.run_raw(
        "INSERT INTO tearminal_t_users (id, name) VALUES (500, 'x')",
        &limits(),
        &CancelToken::new(),
    ));
    match engine {
        Engine::SqlServer => {
            assert!(
                raw_write.is_ok(),
                "SQL Server no tiene solo lectura por sesión"
            );
            run(
                &mut local_cleanup(&local, &policy),
                "DELETE FROM tearminal_t_users WHERE id = 500",
            )
            .expect("limpiar");
        }
        _ => assert!(
            raw_write.is_err(),
            "el servidor debe rechazar la escritura en solo lectura"
        ),
    }

    // Escritura temporal habilitada: pide el nombre, avisa de lo peligroso y aplica en transacción
    prod_session.enable_write(Instant::now());
    let update = "UPDATE tearminal_t_users SET note = 'cambiado' WHERE id = 2";
    let no_name = prod_session.apply(
        update,
        &WriteConfirmation::default(),
        &limits(),
        &CancelToken::new(),
    );
    assert!(
        matches!(no_name, Err(DbError::ConfirmationRequired)),
        "{no_name:?}"
    );
    let wrong = WriteConfirmation {
        typed_name: Some("otra".into()),
        dangers_acknowledged: false,
    };
    assert!(matches!(
        prod_session.apply(update, &wrong, &limits(), &CancelToken::new()),
        Err(DbError::ConfirmationRequired)
    ));

    let named = WriteConfirmation {
        typed_name: Some("it-prod".into()),
        dangers_acknowledged: false,
    };
    let dangerous = prod_session.apply(
        "DELETE FROM tearminal_t_orders",
        &named,
        &limits(),
        &CancelToken::new(),
    );
    assert!(
        matches!(&dangerous, Err(DbError::DangerousStatements(d)) if d == &[(0, Danger::DeleteWithoutWhere)]),
        "{dangerous:?}"
    );
    assert_eq!(
        first_cell(&mut prod_session, "SELECT COUNT(*) FROM tearminal_t_orders").as_deref(),
        Some("1")
    );

    let impact = prod_session
        .estimate_impact(update, &limits(), &CancelToken::new())
        .expect("impacto");
    assert_eq!(impact[0].rows, Some(1));

    let report = prod_session
        .apply(update, &named, &limits(), &CancelToken::new())
        .expect("aplicar");
    assert_eq!(report.affected, [1]);
    let backup = report.backup_file.expect("copia de las filas originales");
    let saved = std::fs::read_to_string(&backup).expect("leer copia");
    assert!(
        saved.contains("\"table\":\"tearminal_t_users\"") && saved.contains("\"2\""),
        "{saved}"
    );
    let _ = std::fs::remove_file(backup);
    assert_eq!(
        first_cell(
            &mut prod_session,
            "SELECT note FROM tearminal_t_users WHERE id = 2"
        )
        .as_deref(),
        Some("cambiado")
    );

    // Un error a mitad revierte todo
    let script = "UPDATE tearminal_t_users SET note = 'no debe quedar' WHERE id = 3; INSERT INTO tearminal_t_users (id, name) VALUES (1, 'duplicado')";
    let failed = prod_session.apply(script, &named, &limits(), &CancelToken::new());
    assert!(
        matches!(
            failed,
            Err(DbError::ApplyFailed {
                index: 1,
                rolled_back: true,
                ..
            })
        ),
        "{failed:?}"
    );
    assert_eq!(
        first_cell(
            &mut prod_session,
            "SELECT COUNT(*) FROM tearminal_t_users WHERE note = 'no debe quedar'"
        )
        .as_deref(),
        Some("0")
    );

    // La escritura caduca: sin concesión vigente vuelve a solo lectura
    prod_session.disable_write();
    let after = run(
        &mut prod_session,
        "UPDATE tearminal_t_users SET note = 'otra vez' WHERE id = 4",
    );
    assert!(matches!(
        after,
        Err(DbError::Safety(SafetyError::WriteInReadOnly))
    ));
    assert_eq!(
        first_cell(
            &mut prod_session,
            "SELECT COUNT(*) FROM tearminal_t_users WHERE note = 'otra vez'"
        )
        .as_deref(),
        Some("0")
    );

    drop_tables(&mut session);
}

/// Segunda sesión LOCAL para limpiar cuando la de trabajo está ocupada
fn local_cleanup(connection: &Connection, policy: &SecurityPolicy) -> Session {
    Session::connect(connection, policy).expect("sesión de limpieza")
}

#[test]
#[ignore = "necesita TEARMINAL_DB_TEST_POSTGRES"]
fn postgres_end_to_end() {
    exercise(Engine::Postgres, "TEARMINAL_DB_TEST_POSTGRES");
}

#[test]
#[ignore = "necesita TEARMINAL_DB_TEST_MYSQL"]
fn mysql_end_to_end() {
    exercise(Engine::Mysql, "TEARMINAL_DB_TEST_MYSQL");
}

#[test]
#[ignore = "necesita TEARMINAL_DB_TEST_MARIADB"]
fn mariadb_end_to_end() {
    exercise(Engine::Mariadb, "TEARMINAL_DB_TEST_MARIADB");
}

#[test]
#[ignore = "necesita TEARMINAL_DB_TEST_SQLSERVER"]
fn sqlserver_end_to_end() {
    exercise(Engine::SqlServer, "TEARMINAL_DB_TEST_SQLSERVER");
}
