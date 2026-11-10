//! Túnel SSH hacia la base de datos: `ssh.exe -N -L 127.0.0.1:<puerto libre>:host:puerto` como proceso hijo sin
//! ventana (mismo patrón que `ssh.rs`). Llaves `.pem`, `.key` o `.ppk` (esta última vía `plink.exe` si está instalado).
//!
//! * **Huella conocida:** con `known_fingerprint`, se escanean las llaves del servidor y solo se conecta contra la
//!   que coincide: esa llave se escribe en un `known_hosts` privado y `ssh` corre con `StrictHostKeyChecking=yes`,
//!   así no hay hueco entre «comprobar» y «conectar».
//! * **Frase de paso:** `SSH_ASKPASS` apunta a este mismo ejecutable (ver `askpass.rs`); nunca va por argumentos.
//! * **Clave insegura:** si otros usuarios de Windows pueden leer la llave, no se abre el túnel.

use super::credentials::{self, SecretKind};
use super::perm::{self, KeyAccess};
use super::sanitize;
use super::store::TunnelSettings;
use crate::ssh::{self, SafeSshDestination};
use std::ffi::OsString;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TunnelError {
    #[error("destino del túnel no válido")]
    InvalidDestination,
    #[error("falta la llave privada del túnel")]
    KeyMissing,
    #[error("la llave privada no existe o no se puede leer: {0}")]
    KeyUnreadable(String),
    #[error(
        "otros usuarios de Windows pueden leer la llave privada ({0}); usa «Corregir permisos»"
    )]
    InsecureKey(String),
    #[error("las llaves .ppk con frase de paso no son compatibles con el túnel: conviértela a OpenSSH con puttygen")]
    PpkNeedsConversion,
    #[error("para usar una llave .ppk hace falta plink.exe (PuTTY); instálalo o convierte la llave a .pem")]
    PlinkMissing,
    #[error("no se pudieron leer las llaves del servidor SSH: {0}")]
    HostKeyScan(String),
    #[error("la huella del servidor SSH no coincide con la guardada (vista: {0})")]
    HostKeyMismatch(String),
    #[error("no se pudo lanzar el proceso SSH: {0}")]
    Spawn(String),
    #[error("el túnel SSH falló: {0}")]
    Failed(String),
    #[error("el túnel SSH no estuvo listo a tiempo")]
    Timeout,
}

/// Túnel abierto: al soltarlo se termina el proceso y se borra el `known_hosts` temporal
pub(crate) struct Tunnel {
    local_port: u16,
    child: Child,
    known_hosts: Option<PathBuf>,
}

impl Tunnel {
    pub(crate) fn local_port(&self) -> u16 {
        self.local_port
    }

    /// `false` si el proceso SSH terminó (conexión cortada, servidor caído)
    pub(crate) fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(path) = self.known_hosts.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Llave pública de host escaneada: línea de `known_hosts`, su tipo y su huella `SHA256:…`
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostKey {
    pub(crate) known_hosts_line: String,
    pub(crate) key_type: String,
    pub(crate) fingerprint: String,
}

const SCAN_OUTPUT_LIMIT: usize = 64 * 1024;
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const STDERR_LIMIT: usize = 16 * 1024;

/// Escanea las llaves del servidor SSH (`ssh-keyscan` + `ssh-keygen -lf`), para mostrar la huella y fijarla
pub(crate) fn scan_host_keys(host: &str, port: u16) -> Result<Vec<HostKey>, TunnelError> {
    let safe = SafeSshDestination::parse(host).map_err(|_| TunnelError::InvalidDestination)?;
    let mut scan = Command::new("ssh-keyscan.exe");
    scan.args(["-T", "5", "-p", &port.to_string(), "--", safe.as_str()]);
    let output =
        ssh::run_command(scan, None, SCAN_OUTPUT_LIMIT).map_err(TunnelError::HostKeyScan)?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut keys = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let mut parts = line.split_whitespace();
        let (Some(_host), Some(key_type), Some(_blob)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let mut keygen = Command::new("ssh-keygen.exe");
        keygen.args(["-lf", "-", "-E", "sha256"]);
        let mut input = line.to_string();
        input.push('\n');
        let printed = ssh::run_command(keygen, Some(input.as_bytes()), 4096)
            .map_err(TunnelError::HostKeyScan)?;
        let Some(fingerprint) = parse_fingerprint(&String::from_utf8_lossy(&printed.stdout)) else {
            continue;
        };
        keys.push(HostKey {
            known_hosts_line: line.to_string(),
            key_type: key_type.to_string(),
            fingerprint,
        });
    }
    if keys.is_empty() {
        return Err(TunnelError::HostKeyScan(
            "el servidor no devolvió llaves (¿host o puerto incorrectos?)".to_string(),
        ));
    }
    Ok(keys)
}

/// `256 SHA256:abc… host (ED25519)` → `SHA256:abc…`
fn parse_fingerprint(keygen_output: &str) -> Option<String> {
    let token = keygen_output.split_whitespace().nth(1)?;
    (token.starts_with("SHA256:") && token.chars().all(|c| c.is_ascii_graphic()))
        .then(|| token.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyFormat {
    OpenSsh,
    Ppk,
}

fn key_format(path: &Path) -> KeyFormat {
    let is_ppk = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ppk"));
    if is_ppk {
        KeyFormat::Ppk
    } else {
        KeyFormat::OpenSsh
    }
}

/// La cabecera de un `.ppk` dice `Encryption: none` o el cifrado; solo se lee esa línea
fn ppk_is_encrypted(path: &Path) -> Result<bool, TunnelError> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(512).read_to_end(&mut head))
        .map_err(|e| TunnelError::KeyUnreadable(e.kind().to_string()))?;
    let text = String::from_utf8_lossy(&head);
    Ok(text
        .lines()
        .find_map(|l| l.strip_prefix("Encryption:"))
        .is_some_and(|value| value.trim() != "none"))
}

fn check_key(path: &Path, format: KeyFormat) -> Result<(), TunnelError> {
    if !path.is_file() {
        return Err(TunnelError::KeyUnreadable("no es un archivo".to_string()));
    }
    if format == KeyFormat::Ppk && ppk_is_encrypted(path)? {
        return Err(TunnelError::PpkNeedsConversion);
    }
    match perm::key_access(path) {
        KeyAccess::SharedWith(names) => Err(TunnelError::InsecureKey(names.join(", "))),
        KeyAccess::Private | KeyAccess::Unknown(_) => Ok(()),
    }
}

fn free_local_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

fn forward_spec(local_port: u16, remote_host: &str, remote_port: u16) -> String {
    format!("127.0.0.1:{local_port}:{remote_host}:{remote_port}")
}

/// Argumentos de `ssh.exe`. `use_askpass`: hay frase de paso guardada (entonces no se usa `BatchMode`, que
/// desactiva también el askpass) y solo se admite autenticación por llave, para que el askpass no pueda
/// contestar a una petición de contraseña del servidor.
#[allow(clippy::too_many_arguments)]
fn ssh_args(
    destination: &SafeSshDestination,
    port: u16,
    key: &Path,
    forward: &str,
    known_hosts: Option<&Path>,
    host_key_type: Option<&str>,
    use_askpass: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut opt = |value: String| {
        args.push("-o".into());
        args.push(value.into());
    };
    opt("ExitOnForwardFailure=yes".into());
    opt("ConnectTimeout=10".into());
    opt("ConnectionAttempts=1".into());
    opt("ServerAliveInterval=15".into());
    opt("ServerAliveCountMax=3".into());
    opt("StrictHostKeyChecking=yes".into());
    opt("LogLevel=ERROR".into());
    opt("IdentitiesOnly=yes".into());
    opt("PreferredAuthentications=publickey".into());
    opt("PasswordAuthentication=no".into());
    opt("KbdInteractiveAuthentication=no".into());
    opt("ControlMaster=no".into());
    opt("ControlPath=none".into());
    if !use_askpass {
        opt("BatchMode=yes".into());
    }
    if let Some(path) = known_hosts {
        opt(format!("UserKnownHostsFile={}", path.display()));
        opt("GlobalKnownHostsFile=NUL".into());
        opt("UpdateHostKeys=no".into());
    }
    if let Some(key_type) = host_key_type {
        opt(format!("HostKeyAlgorithms={key_type}"));
    }
    args.extend(["-N".into(), "-T".into()]);
    args.extend(["-p".into(), port.to_string().into()]);
    args.extend(["-i".into(), key.as_os_str().to_os_string()]);
    args.extend(["-L".into(), forward.into()]);
    args.extend(["--".into(), destination.as_str().into()]);
    args
}

fn plink_args(
    destination: &SafeSshDestination,
    port: u16,
    key: &Path,
    forward: &str,
    fingerprint: Option<&str>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-batch".into(), "-N".into(), "-ssh".into()];
    args.extend(["-P".into(), port.to_string().into()]);
    args.extend(["-i".into(), key.as_os_str().to_os_string()]);
    if let Some(fp) = fingerprint {
        args.extend(["-hostkey".into(), fp.into()]);
    }
    args.extend(["-L".into(), forward.into()]);
    args.push(destination.as_str().into());
    args
}

fn find_plink() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join("plink.exe"))
                .collect()
        })
        .unwrap_or_default();
    candidates.push(PathBuf::from(r"C:\Program Files\PuTTY\plink.exe"));
    candidates.into_iter().find(|p| p.is_file())
}

fn temp_known_hosts(line: &str) -> std::io::Result<PathBuf> {
    let dir = crate::config::cache_dir_or_fallback().join("db-tunnel");
    std::fs::create_dir_all(&dir)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let path = dir.join(format!("kh-{}-{nanos:x}", std::process::id()));
    std::fs::write(&path, format!("{line}\n"))?;
    Ok(path)
}

/// Abre el túnel y espera a que el puerto local acepte conexiones. `connection_id` identifica la frase de paso en
/// el Administrador de credenciales.
pub(crate) fn open(
    settings: &TunnelSettings,
    connection_id: &str,
    remote_host: &str,
    remote_port: u16,
) -> Result<Tunnel, TunnelError> {
    let destination = SafeSshDestination::parse(format!("{}@{}", settings.user, settings.host))
        .map_err(|_| TunnelError::InvalidDestination)?;
    let key = settings
        .key_path
        .as_deref()
        .ok_or(TunnelError::KeyMissing)?;
    let format = key_format(key);
    check_key(key, format)?;

    let pinned = match settings.known_fingerprint.as_deref() {
        Some(wanted) => Some(pinned_host_key(&settings.host, settings.port, wanted)?),
        None => None,
    };
    let local_port = free_local_port().map_err(|e| TunnelError::Spawn(e.to_string()))?;
    let forward = forward_spec(local_port, remote_host, remote_port);

    let mut known_hosts = None;
    let mut command;
    match format {
        KeyFormat::Ppk => {
            let plink = find_plink().ok_or(TunnelError::PlinkMissing)?;
            command = Command::new(plink);
            command.args(plink_args(
                &destination,
                settings.port,
                key,
                &forward,
                settings.known_fingerprint.as_deref(),
            ));
        }
        KeyFormat::OpenSsh => {
            if let Some(host_key) = &pinned {
                known_hosts = Some(
                    temp_known_hosts(&host_key.known_hosts_line)
                        .map_err(|e| TunnelError::Spawn(e.to_string()))?,
                );
            }
            let use_askpass = matches!(
                credentials::load(SecretKind::SshPassphrase, connection_id),
                Ok(Some(_))
            );
            command = Command::new("ssh.exe");
            command.args(ssh_args(
                &destination,
                settings.port,
                key,
                &forward,
                known_hosts.as_deref(),
                pinned.as_ref().map(|k| k.key_type.as_str()),
                use_askpass,
            ));
            if use_askpass {
                let exe = std::env::current_exe().map_err(|e| TunnelError::Spawn(e.to_string()))?;
                command
                    .env("SSH_ASKPASS", exe)
                    .env("SSH_ASKPASS_REQUIRE", "force")
                    .env(super::askpass::ENV_VAR, connection_id);
            }
        }
    }
    crate::app::platform::hide_console(&mut command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| {
        if let Some(path) = &known_hosts {
            let _ = std::fs::remove_file(path);
        }
        TunnelError::Spawn(e.kind().to_string())
    })?;
    let stderr = drain_stderr(&mut child);
    let mut tunnel = Tunnel {
        local_port,
        child,
        known_hosts,
    };
    wait_until_ready(&mut tunnel, &stderr)?;
    Ok(tunnel)
}

fn pinned_host_key(host: &str, port: u16, wanted: &str) -> Result<HostKey, TunnelError> {
    let keys = scan_host_keys(host, port)?;
    if let Some(found) = keys.iter().find(|k| k.fingerprint == wanted) {
        return Ok(found.clone());
    }
    let seen: Vec<&str> = keys.iter().map(|k| k.fingerprint.as_str()).collect();
    Err(TunnelError::HostKeyMismatch(seen.join(", ")))
}

fn drain_stderr(child: &mut Child) -> Arc<Mutex<Vec<u8>>> {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut pipe) = child.stderr.take() {
        let sink = Arc::clone(&buffer);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 1024];
            while let Ok(n) = pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if let Ok(mut guard) = sink.lock() {
                    let room = STDERR_LIMIT.saturating_sub(guard.len());
                    guard.extend_from_slice(&chunk[..n.min(room)]);
                }
            }
        });
    }
    buffer
}

fn stderr_text(stderr: &Mutex<Vec<u8>>) -> String {
    let bytes = stderr.lock().map(|g| g.clone()).unwrap_or_default();
    // Lo que escribe ssh viene en parte del servidor: se escapa antes de mostrarlo
    sanitize::display(String::from_utf8_lossy(&bytes).trim()).into_owned()
}

fn wait_until_ready(tunnel: &mut Tunnel, stderr: &Mutex<Vec<u8>>) -> Result<(), TunnelError> {
    let deadline = Instant::now() + READY_TIMEOUT;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, tunnel.local_port));
    while Instant::now() < deadline {
        if let Ok(Some(status)) = tunnel.child.try_wait() {
            // Deja que el hilo termine de leer stderr
            std::thread::sleep(Duration::from_millis(100));
            let detail = stderr_text(stderr);
            return Err(TunnelError::Failed(if detail.is_empty() {
                format!("el proceso terminó ({status})")
            } else {
                detail
            }));
        }
        if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(TunnelError::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dest() -> SafeSshDestination {
        SafeSshDestination::parse("deploy@10.0.4.20").unwrap()
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn forward_binds_loopback_only() {
        assert_eq!(
            forward_spec(5555, "db.internal", 5432),
            "127.0.0.1:5555:db.internal:5432"
        );
    }

    #[test]
    fn ssh_args_are_strict_and_end_with_the_destination() {
        let args = strings(&ssh_args(
            &dest(),
            22,
            Path::new(r"C:\k\id.pem"),
            "127.0.0.1:1:h:5432",
            None,
            None,
            false,
        ));
        for required in [
            "StrictHostKeyChecking=yes",
            "ExitOnForwardFailure=yes",
            "BatchMode=yes",
            "PasswordAuthentication=no",
            "PreferredAuthentications=publickey",
            "-N",
        ] {
            assert!(args.iter().any(|a| a == required), "{required}");
        }
        assert_eq!(&args[args.len() - 2..], ["--", "deploy@10.0.4.20"]);
        assert!(!args
            .iter()
            .any(|a| a.contains("passphrase") || a.contains("-pw")));
    }

    #[test]
    fn askpass_replaces_batch_mode_but_keeps_key_only_auth() {
        let args = strings(&ssh_args(
            &dest(),
            22,
            Path::new("k"),
            "f",
            None,
            None,
            true,
        ));
        assert!(!args.iter().any(|a| a == "BatchMode=yes"));
        assert!(args.iter().any(|a| a == "PasswordAuthentication=no"));
        assert!(args.iter().any(|a| a == "KbdInteractiveAuthentication=no"));
    }

    #[test]
    fn pinned_key_goes_to_a_private_known_hosts_file() {
        let args = strings(&ssh_args(
            &dest(),
            2222,
            Path::new("k"),
            "f",
            Some(Path::new(r"C:\tmp\kh")),
            Some("ssh-ed25519"),
            false,
        ));
        assert!(args.contains(&r"UserKnownHostsFile=C:\tmp\kh".to_string()));
        assert!(args.contains(&"GlobalKnownHostsFile=NUL".to_string()));
        assert!(args.contains(&"HostKeyAlgorithms=ssh-ed25519".to_string()));
        assert!(args.windows(2).any(|w| w == ["-p", "2222"]));
    }

    #[test]
    fn plink_gets_the_fingerprint_with_hostkey() {
        let args = strings(&plink_args(
            &dest(),
            22,
            Path::new("k.ppk"),
            "f",
            Some("SHA256:abc"),
        ));
        assert!(args.windows(2).any(|w| w == ["-hostkey", "SHA256:abc"]));
        assert!(args.contains(&"-batch".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("deploy@10.0.4.20"));
    }

    #[test]
    fn fingerprint_is_parsed_from_ssh_keygen_output() {
        assert_eq!(
            parse_fingerprint("256 SHA256:3wQ0xJ9pAbC+/= 10.0.4.20 (ED25519)\n").as_deref(),
            Some("SHA256:3wQ0xJ9pAbC+/=")
        );
        assert_eq!(parse_fingerprint("no es una huella"), None);
        assert_eq!(parse_fingerprint(""), None);
    }

    #[test]
    fn key_format_follows_the_extension() {
        assert_eq!(key_format(Path::new("a.PPK")), KeyFormat::Ppk);
        for name in ["a.pem", "a.key", "id_ed25519"] {
            assert_eq!(key_format(Path::new(name)), KeyFormat::OpenSsh);
        }
    }

    fn temp_file(tag: &str, name: &str, content: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tearminal-tunnel-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn encrypted_ppk_is_detected_and_unencrypted_is_not() {
        let encrypted = temp_file(
            "ppk1",
            "a.ppk",
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: aes256-cbc\n",
        );
        let plain = temp_file(
            "ppk2",
            "b.ppk",
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\n",
        );
        assert!(ppk_is_encrypted(&encrypted).unwrap());
        assert!(!ppk_is_encrypted(&plain).unwrap());
        assert!(matches!(
            check_key(&encrypted, KeyFormat::Ppk),
            Err(TunnelError::PpkNeedsConversion)
        ));
    }

    #[test]
    fn missing_key_and_missing_settings_are_rejected_before_spawning() {
        let mut settings = TunnelSettings {
            enabled: true,
            host: "10.0.4.20".into(),
            user: "deploy".into(),
            ..TunnelSettings::default()
        };
        assert!(matches!(
            open(&settings, "id", "db", 5432),
            Err(TunnelError::KeyMissing)
        ));
        settings.key_path = Some(PathBuf::from(r"C:\no\existe\k.pem"));
        assert!(matches!(
            open(&settings, "id", "db", 5432),
            Err(TunnelError::KeyUnreadable(_))
        ));
        settings.user = "-oProxyCommand=calc".into();
        assert!(matches!(
            open(&settings, "id", "db", 5432),
            Err(TunnelError::InvalidDestination)
        ));
    }

    #[test]
    fn key_readable_by_everyone_blocks_the_tunnel() {
        let key = temp_file("insecure", "k.pem", "relleno");
        let granted = Command::new("icacls.exe")
            .arg(&key)
            .args(["/grant", "*S-1-1-0:(R)"])
            .output();
        let Ok(granted) = granted else { return };
        if !granted.status.success() {
            return;
        }
        assert!(matches!(
            check_key(&key, KeyFormat::OpenSsh),
            Err(TunnelError::InsecureKey(names)) if names.contains("Todos")
        ));
        perm::restrict_to_current_user(&key).unwrap();
        assert!(check_key(&key, KeyFormat::OpenSsh).is_ok());
    }

    #[test]
    fn free_port_is_usable_on_loopback() {
        let port = free_local_port().unwrap();
        assert!(port > 0);
        assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }

    #[test]
    fn scanning_an_invalid_host_is_rejected_without_running_anything() {
        assert!(matches!(
            scan_host_keys("-oProxyCommand=calc", 22),
            Err(TunnelError::InvalidDestination)
        ));
    }
}
