//! Almacén de conexiones (`connections.toml`, junto a `tearminal.toml`).
//!
//! Aquí no hay contraseñas ni frases de paso: `Connection` ni siquiera tiene un campo para ellas (van al
//! Administrador de credenciales, ver `credentials.rs`). De los archivos `.pem` solo se guarda la **ruta**.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Engine {
    Postgres,
    Mysql,
    #[serde(rename = "sqlserver")]
    SqlServer,
    Mariadb,
}

impl Engine {
    pub(crate) fn default_port(self) -> u16 {
        match self {
            Self::Postgres => 5432,
            Self::Mysql | Self::Mariadb => 3306,
            Self::SqlServer => 1433,
        }
    }

    /// Sentencia con la que cada motor abre una transacción
    pub(crate) fn begin_sql(self) -> &'static str {
        match self {
            Self::Postgres => "BEGIN",
            Self::Mysql | Self::Mariadb => "START TRANSACTION",
            Self::SqlServer => "BEGIN TRANSACTION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Env {
    Local,
    Stg,
    Prod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SslMode {
    Disable,
    Require,
    VerifyCa,
    #[default]
    VerifyFull,
}

/// Pestaña SSL / TLS. Solo rutas: el contenido de los `.pem` nunca se copia ni se guarda.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TlsSettings {
    pub(crate) mode: SslMode,
    /// Nombre a verificar si no coincide con `host` (por ejemplo con túnel, donde el host real es `127.0.0.1`)
    pub(crate) server_name: Option<String>,
    pub(crate) ca_path: Option<PathBuf>,
    pub(crate) client_cert_path: Option<PathBuf>,
    pub(crate) client_key_path: Option<PathBuf>,
}

/// Pestaña Túnel SSH. La llave privada (`.pem`, `.key`, `.ppk`) se guarda como ruta; su frase de paso, en el
/// Administrador de credenciales.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TunnelSettings {
    pub(crate) enabled: bool,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) user: String,
    pub(crate) key_path: Option<PathBuf>,
    /// Huella `SHA256:…` del servidor SSH. Con ella, el túnel solo se abre contra esa llave exacta.
    pub(crate) known_fingerprint: Option<String>,
}

impl Default for TunnelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 22,
            user: String::new(),
            key_path: None,
            known_fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Connection {
    /// Identificador estable (clave de las credenciales): renombrar la conexión no las pierde
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) engine: Engine,
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(default)]
    pub(crate) database: String,
    #[serde(default)]
    pub(crate) user: String,
    pub(crate) env: Env,
    #[serde(default)]
    pub(crate) read_only: bool,
    #[serde(default)]
    pub(crate) tls: TlsSettings,
    #[serde(default)]
    pub(crate) tunnel: TunnelSettings,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("error de E/S en connections.toml: {0}")]
    Io(#[from] std::io::Error),
    #[error("connections.toml no se puede interpretar: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("no se pudo serializar connections.toml: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("connections.toml tiene errores: corrígelo antes de guardar conexiones")]
    InvalidExistingFile,
    #[error("conexión no válida: {0}")]
    Invalid(&'static str),
}

fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!(
        "{nanos:x}{:x}{:02x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed) & 0xff
    )
}

fn is_plain_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 255
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

impl Connection {
    pub(crate) fn new(name: &str, engine: Engine) -> Self {
        Self {
            id: new_id(),
            name: name.to_string(),
            engine,
            host: "localhost".to_string(),
            port: engine.default_port(),
            database: String::new(),
            user: String::new(),
            env: Env::Local,
            read_only: false,
            tls: TlsSettings::default(),
            tunnel: TunnelSettings::default(),
        }
    }

    /// El host acaba en argumentos de `ssh -L` y en cadenas de conexión: solo letras, dígitos, `.`, `-` y `_`
    /// (sin literales IPv6; usa el nombre del equipo).
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        let bad_text = |s: &str| s.chars().any(char::is_control);
        if self.name.trim().is_empty() || self.name.len() > 64 || bad_text(&self.name) {
            return Err(StoreError::Invalid(
                "el nombre debe tener entre 1 y 64 caracteres",
            ));
        }
        if self.id.is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(StoreError::Invalid("identificador interno no válido"));
        }
        if !is_plain_host(&self.host) {
            return Err(StoreError::Invalid("host no válido"));
        }
        if self.port == 0 {
            return Err(StoreError::Invalid("puerto no válido"));
        }
        if bad_text(&self.database) || bad_text(&self.user) {
            return Err(StoreError::Invalid(
                "base o usuario con caracteres de control",
            ));
        }
        if self
            .tls
            .server_name
            .as_deref()
            .is_some_and(|n| !is_plain_host(n))
        {
            return Err(StoreError::Invalid("nombre del servidor TLS no válido"));
        }
        if self.tunnel.enabled {
            if !is_plain_host(&self.tunnel.host) || self.tunnel.port == 0 {
                return Err(StoreError::Invalid("host o puerto del túnel no válido"));
            }
            if crate::ssh::SafeSshDestination::parse(format!(
                "{}@{}",
                self.tunnel.user, self.tunnel.host
            ))
            .is_err()
            {
                return Err(StoreError::Invalid("usuario del túnel no válido"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct ConnectionStore {
    #[serde(default, rename = "connection")]
    pub(crate) connections: Vec<Connection>,
}

impl ConnectionStore {
    /// `connections.toml` vive junto a `tearminal.toml` (carpeta de usuario o junto al ejecutable)
    pub(crate) fn default_path() -> PathBuf {
        crate::config::TearminalConfig::config_path().with_file_name("connections.toml")
    }

    /// Un archivo ausente es un almacén vacío; uno que no se puede interpretar es un error (nunca se pisa)
    pub(crate) fn load_from(path: &Path) -> Result<Self, StoreError> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Escritura atómica (temporal + renombrado). Si el archivo actual está corrupto, no lo sobrescribe.
    pub(crate) fn save_to(&self, path: &Path) -> Result<(), StoreError> {
        if let Ok(existing) = fs::read_to_string(path) {
            if toml::from_str::<Self>(&existing).is_err() {
                return Err(StoreError::InvalidExistingFile);
            }
        }
        for connection in &self.connections {
            connection.validate()?;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
            file.flush()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub(crate) fn find(&self, id: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Añade o reemplaza (por `id`). Los nombres son únicos sin distinguir mayúsculas: la confirmación escrita de
    /// PROD exige el nombre y dos conexiones con el mismo nombre la volverían ambigua.
    pub(crate) fn upsert(&mut self, connection: Connection) -> Result<(), StoreError> {
        connection.validate()?;
        let clash = self
            .connections
            .iter()
            .any(|c| c.id != connection.id && c.name.eq_ignore_ascii_case(&connection.name));
        if clash {
            return Err(StoreError::Invalid("ya existe una conexión con ese nombre"));
        }
        match self.connections.iter_mut().find(|c| c.id == connection.id) {
            Some(slot) => *slot = connection,
            None => self.connections.push(connection),
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Connection> {
        let index = self.connections.iter().position(|c| c.id == id)?;
        Some(self.connections.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tearminal-db-store-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("connections.toml")
    }

    fn sample() -> Connection {
        let mut c = Connection::new("pg-main", Engine::Postgres);
        c.host = "db.acme.cl".into();
        c.database = "acme".into();
        c.user = "app".into();
        c.env = Env::Prod;
        c.read_only = true;
        c.tls.ca_path = Some(PathBuf::from(r"C:\certs\acme-ca.pem"));
        c.tls.client_key_path = Some(PathBuf::from(r"C:\certs\client.key.pem"));
        c.tunnel = TunnelSettings {
            enabled: true,
            host: "10.0.4.20".into(),
            user: "deploy".into(),
            key_path: Some(PathBuf::from(r"C:\Users\x\.ssh\k.pem")),
            known_fingerprint: Some("SHA256:abc".into()),
            ..TunnelSettings::default()
        };
        c
    }

    #[test]
    fn roundtrip_keeps_every_field() {
        let path = temp_path("roundtrip");
        let mut store = ConnectionStore::default();
        store.upsert(sample()).unwrap();
        store.save_to(&path).unwrap();
        assert_eq!(ConnectionStore::load_from(&path).unwrap(), store);
    }

    #[test]
    fn toml_holds_paths_but_no_secrets() {
        let text = toml::to_string_pretty(&ConnectionStore {
            connections: vec![sample()],
        })
        .unwrap();
        for forbidden in ["password", "passphrase", "secret", "BEGIN"] {
            assert!(
                !text.to_lowercase().contains(&forbidden.to_lowercase()),
                "{forbidden}"
            );
        }
        assert!(text.contains("acme-ca.pem") && text.contains("env = \"PROD\""));
        assert!(text.contains("engine = \"postgres\""));
    }

    #[test]
    fn defaults_are_verify_full_and_engine_ports() {
        assert_eq!(SslMode::default(), SslMode::VerifyFull);
        assert_eq!(TlsSettings::default().mode, SslMode::VerifyFull);
        let parsed: ConnectionStore = toml::from_str(
            "[[connection]]\nid='a1'\nname='x'\nengine='sqlserver'\nhost='h'\nport=1433\nenv='STG'\n",
        )
        .unwrap();
        assert_eq!(parsed.connections[0].tls.mode, SslMode::VerifyFull);
        assert_eq!(Engine::Mariadb.default_port(), 3306);
        assert_eq!(Engine::SqlServer.begin_sql(), "BEGIN TRANSACTION");
    }

    #[test]
    fn missing_file_is_empty_and_corrupt_file_is_never_overwritten() {
        let path = temp_path("corrupt");
        assert!(ConnectionStore::load_from(&path)
            .unwrap()
            .connections
            .is_empty());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "esto no es [toml").unwrap();
        assert!(matches!(
            ConnectionStore::load_from(&path),
            Err(StoreError::Parse(_))
        ));
        let mut store = ConnectionStore::default();
        store.upsert(sample()).unwrap();
        assert!(matches!(
            store.save_to(&path),
            Err(StoreError::InvalidExistingFile)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "esto no es [toml");
    }

    #[test]
    fn validation_rejects_hostile_values() {
        for host in ["", "-oProxyCommand=x", "a b", "h;rm", "h:5432", "h\n"] {
            let mut c = sample();
            c.host = host.into();
            assert!(c.validate().is_err(), "{host:?}");
        }
        let mut c = sample();
        c.name = "a\x1b[31m".into();
        assert!(c.validate().is_err());
        let mut c = sample();
        c.tunnel.user = "-oProxyCommand=calc".into();
        assert!(c.validate().is_err());
        let mut c = sample();
        c.tunnel.host = "h$(x)".into();
        assert!(c.validate().is_err());
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn names_are_unique_ignoring_case_and_upsert_replaces_by_id() {
        let mut store = ConnectionStore::default();
        let first = sample();
        store.upsert(first.clone()).unwrap();
        let mut clash = Connection::new("PG-MAIN", Engine::Mysql);
        clash.host = "h".into();
        assert!(store.upsert(clash).is_err());
        let mut renamed = first.clone();
        renamed.name = "pg-prod".into();
        store.upsert(renamed).unwrap();
        assert_eq!(store.connections.len(), 1);
        assert_eq!(
            store.find(&first.id).map(|c| c.name.as_str()),
            Some("pg-prod")
        );
        assert!(store.remove(&first.id).is_some());
        assert!(store.connections.is_empty());
    }

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<_> = (0..200).map(|_| new_id()).collect();
        assert_eq!(ids.len(), 200);
    }
}
