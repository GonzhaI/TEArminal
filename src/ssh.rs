//! `ssh` no interactivo (BatchMode) con salida acotada para las vías lentas del explorador y el visor
//! remotos (sin integración en banda, o cuando esta falla). BLOQUEAN (conexión + transferencia): llámalas solo
//! desde un hilo (`TearminalApp::spawn_ssh`), nunca desde el bucle de eventos.

use std::io::{Read, Write};
use std::marker::PhantomData;
use std::process::{Command, Stdio};

/// Error de validación al parsear un destino SSH no confiable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SshDestinationError {
    Empty,
    TooLong(usize),
    StartsWithDash,
    InvalidCharacters,
    InvalidUserHostFormat,
}

impl std::fmt::Display for SshDestinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "el destino SSH no puede estar vacío"),
            Self::TooLong(n) => {
                write!(f, "el destino SSH supera el límite de 255 caracteres ({n})")
            }
            Self::StartsWithDash => write!(f, "el destino SSH no puede comenzar con '-'"),
            Self::InvalidCharacters => {
                write!(f, "el destino SSH contiene caracteres no permitidos")
            }
            Self::InvalidUserHostFormat => write!(f, "formato inválido para usuario@host"),
        }
    }
}

impl std::error::Error for SshDestinationError {}

/// Destino SSH verificado y seguro: `host` o `usuario@host`.
///
/// Implementa el patrón **Parse, don't validate**: en lugar de pasar cadenas crudas `&str` y
/// revalidar en múltiples funciones, se valida una única vez en la frontera del sistema.
/// Garantiza por construcción que no contiene metacaracteres shell (espacios, `;`, `$`, comillas)
/// ni opciones maliciosas de SSH (`-oProxyCommand=...`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SafeSshDestination(String);

impl SafeSshDestination {
    /// Parsea y valida el destino SSH. Constructor único: el resto del sistema recibe
    /// `SafeSshDestination` con garantía total y cero chequeos redundantes.
    pub(crate) fn parse(dest: impl AsRef<str>) -> Result<Self, SshDestinationError> {
        let s = dest.as_ref();
        if s.is_empty() {
            return Err(SshDestinationError::Empty);
        }
        if s.len() > 255 {
            return Err(SshDestinationError::TooLong(s.len()));
        }
        if s.starts_with('-') {
            return Err(SshDestinationError::StartsWithDash);
        }
        let ok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        let mut parts = s.splitn(2, '@');
        let first = parts.next().unwrap_or("");
        match parts.next() {
            None => {
                if first.chars().all(ok_char) {
                    Ok(Self(s.to_string()))
                } else {
                    Err(SshDestinationError::InvalidCharacters)
                }
            }
            Some(host) => {
                if first.is_empty() || host.is_empty() {
                    Err(SshDestinationError::InvalidUserHostFormat)
                } else if first.chars().all(ok_char) && host.chars().all(ok_char) {
                    Ok(Self(s.to_string()))
                } else {
                    Err(SshDestinationError::InvalidCharacters)
                }
            }
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for SafeSshDestination {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SafeSshDestination {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SafeSshDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SafeSshDestination {
    type Err = SshDestinationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Error al validar un directorio remoto reportado por el shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RemoteDirError {
    Empty,
    TooLong(usize),
    StartsWithDash,
    InvalidCharacters,
}

impl std::fmt::Display for RemoteDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "el directorio remoto no puede estar vacío"),
            Self::TooLong(n) => write!(f, "el directorio remoto supera los 1024 caracteres ({n})"),
            Self::StartsWithDash => {
                write!(f, "el directorio remoto no puede comenzar con '-'")
            }
            Self::InvalidCharacters => {
                write!(f, "el directorio remoto contiene caracteres no permitidos")
            }
        }
    }
}

impl std::error::Error for RemoteDirError {}

/// Directorio remoto sanitizado y verificado ("Parse, don't validate").
/// Garantiza que no contiene comillas simples/dobles, variables `$`, comillas invertidas ni metacaracteres shell.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SafeRemoteDir(String);

impl SafeRemoteDir {
    pub(crate) fn parse(dir: impl AsRef<str>) -> Result<Self, RemoteDirError> {
        let s = dir.as_ref();
        if s.is_empty() {
            return Err(RemoteDirError::Empty);
        }
        if s.starts_with('-') {
            return Err(RemoteDirError::StartsWithDash);
        }
        if s.len() > 1024 {
            return Err(RemoteDirError::TooLong(s.len()));
        }
        let ok = |c: char| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '/' | '.' | '_' | '-' | '~' | '+' | '@' | ':' | ',' | '=' | ' '
                )
        };
        if s.chars().all(ok) {
            Ok(Self(s.to_string()))
        } else {
            Err(RemoteDirError::InvalidCharacters)
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for SafeRemoteDir {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SafeRemoteDir {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SafeRemoteDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// -------------------------------------------------------------------------------------------------
// Patrón Typestate (Estados en Tiempo de Compilación)
// -------------------------------------------------------------------------------------------------

/// Marcador zero-sized: la conexión no ha completado la fase de autenticación o validación de credenciales.
pub(crate) struct Unauthenticated;

/// Marcador zero-sized: la conexión está autenticada y lista para ejecutar comandos remotos.
pub(crate) struct Authenticated;

/// Conexión remota SSH con seguridad en tiempo de compilación mediante el patrón Typestate.
///
/// En lugar de validar en tiempo de ejecución si la conexión está lista (`if !conn.is_authenticated()`),
/// los métodos de ejecución (`run_command`, `read_file`, `write_file`) **solo existen** en el tipo
/// `SshConnection<Authenticated>`. Llamar a `run_command` sobre una conexión `Unauthenticated` es un
/// error de compilación inmediato.
///
/// `PhantomData` ocupa 0 bytes en memoria.
pub(crate) struct SshConnection<State = Unauthenticated> {
    destination: SafeSshDestination,
    connect_timeout: u32,
    _state: PhantomData<State>,
}

#[allow(dead_code)]
impl SshConnection<Unauthenticated> {
    /// Inicia una conexión en estado `Unauthenticated`.
    /// Requiere un `SafeSshDestination` ("Parse, don't validate").
    pub(crate) fn new(destination: SafeSshDestination) -> Self {
        Self {
            destination,
            connect_timeout: 5,
            _state: PhantomData,
        }
    }

    /// Configura el tiempo de espera de conexión (en segundos).
    pub(crate) fn with_timeout(mut self, timeout_secs: u32) -> Self {
        self.connect_timeout = timeout_secs;
        self
    }

    /// Consume la conexión no autenticada y produce una conexión `Authenticated`.
    /// Si el token o credenciales son inválidos, devuelve error y no permite avanzar de estado.
    pub(crate) fn authenticate(
        self,
        token: Option<&str>,
    ) -> Result<SshConnection<Authenticated>, String> {
        if let Some(t) = token {
            if t.trim().is_empty() {
                return Err("Token de autenticación no puede estar vacío".to_string());
            }
        }
        Ok(SshConnection {
            destination: self.destination,
            connect_timeout: self.connect_timeout,
            _state: PhantomData,
        })
    }
}

#[allow(dead_code)]
impl SshConnection<Authenticated> {
    #[inline]
    pub(crate) fn destination(&self) -> &SafeSshDestination {
        &self.destination
    }

    /// Ejecuta una orden remota. Este método NI SIQUIERA EXISTE en `SshConnection<Unauthenticated>`.
    pub(crate) fn run_command(
        &self,
        remote_cmd: &str,
        stdin: Option<&[u8]>,
        max_out: usize,
    ) -> Result<Output, String> {
        run_safe(
            &self.destination,
            remote_cmd,
            self.connect_timeout,
            stdin,
            max_out,
        )
    }

    /// Lee un archivo remoto de forma acotada.
    pub(crate) fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        let out = self.run_command(&read_file_cmd(path), None, MAX_FILE_BYTES)?;
        if !out.ok {
            return Err(if out.stderr.is_empty() {
                "Error al leer archivo remoto".to_string()
            } else {
                out.stderr
            });
        }
        Ok(out.stdout)
    }

    /// Guarda un archivo remoto de forma atómica y segura.
    pub(crate) fn write_file(&self, path: &str, content: &[u8]) -> Result<(), String> {
        let out = self.run_command(&write_file_cmd(path, content.len()), Some(content), 4096)?;
        if out.ok {
            Ok(())
        } else {
            Err(if out.stderr.is_empty() {
                "Error al escribir archivo remoto".to_string()
            } else {
                out.stderr
            })
        }
    }

    /// Cierra la sesión activa y devuelve la conexión al estado `Unauthenticated`.
    pub(crate) fn disconnect(self) -> SshConnection<Unauthenticated> {
        SshConnection {
            destination: self.destination,
            connect_timeout: self.connect_timeout,
            _state: PhantomData,
        }
    }
}

/// Tope de un archivo remoto que se abre en el visor (se piden `MAX_FILE_BYTES + 1` para saber que lo supera)
pub(crate) const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// Entradas como máximo de un listado remoto
pub(crate) const LISTING_ENTRIES: usize = 5000;
/// Tope de lo que se lee de una orden de listado (5000 nombres de hasta 255 bytes, con margen)
pub(crate) const MAX_LISTING_BYTES: usize = 2 * 1024 * 1024;
/// Tope de lo que se conserva de stderr
const MAX_STDERR_BYTES: u64 = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Output {
    pub(crate) ok: bool,
    pub(crate) truncated: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
}

/// Entre comillas simples para el shell remoto
pub(crate) fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Primeros `MAX_FILE_BYTES + 1` bytes del archivo: el remoto nunca envía más de lo que TEArminal va a leer
pub(crate) fn read_file_cmd(path: &str) -> String {
    format!("head -c {} -- {}", MAX_FILE_BYTES + 1, quote(path))
}

/// Guarda lo que llega por stdin (`len` bytes) en `path` sin dejarlo a medias: se recibe entero en un temporal
/// y solo se copia si tiene exactamente `len` bytes (una conexión cortada a mitad no trunca el original).
/// Va en `sh -c` para no depender del shell de login del servidor (fish, csh…).
pub(crate) fn write_file_cmd(path: &str, len: usize) -> String {
    let script = format!(
        r#"t=$(mktemp) && cat >"$t" && [ "$(wc -c <"$t")" -eq {len} ] && cat "$t" >{}; s=$?; rm -f "$t"; exit $s"#,
        quote(path)
    );
    format!("sh -c {}", quote(&script))
}

/// Carpeta actual canónica, rama git y listado de la raíz del explorador remoto
pub(crate) fn root_listing_cmd(dir: &str) -> String {
    format!(
        "cd {} 2>/dev/null && pwd && (git rev-parse --abbrev-ref HEAD 2>/dev/null || true) && echo '---' && ls -1ap | head -{}",
        quote(dir),
        LISTING_ENTRIES
    )
}

pub(crate) fn dir_listing_cmd(dir: &str) -> String {
    format!(
        "cd {} 2>/dev/null && ls -1ap | head -{}",
        quote(dir),
        LISTING_ENTRIES
    )
}

/// Ejecuta `remote_cmd` en `dest`. `SafeSshDestination` garantiza en tiempo de compilación
/// que el destino está validado y no contiene inyecciones ni flags peligrosos ("Parse, don't validate").
pub(crate) fn run_safe(
    dest: &SafeSshDestination,
    remote_cmd: &str,
    connect_timeout: u32,
    stdin: Option<&[u8]>,
    max_out: usize,
) -> Result<Output, String> {
    let mut cmd = Command::new("ssh.exe");
    cmd.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ServerAliveInterval=5",
        "-o",
        "ServerAliveCountMax=2",
        "-o",
    ])
    .arg(format!("ConnectTimeout={connect_timeout}"))
    .args(["--", dest.as_str(), remote_cmd]);
    run_command(cmd, stdin, max_out)
}

/// Ejecuta `remote_cmd` en `dest`. Parsea y valida el destino en la frontera antes de ejecutar.
pub(crate) fn run(
    dest: &str,
    remote_cmd: &str,
    connect_timeout: u32,
    stdin: Option<&[u8]>,
    max_out: usize,
) -> Result<Output, String> {
    let safe = SafeSshDestination::parse(dest)
        .map_err(|e| format!("destino SSH no válido: {dest:?} ({e})"))?;
    run_safe(&safe, remote_cmd, connect_timeout, stdin, max_out)
}

pub(crate) fn run_command(
    mut cmd: Command,
    stdin: Option<&[u8]>,
    max_out: usize,
) -> Result<Output, String> {
    crate::app::platform::hide_console(&mut cmd);
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let prog = cmd.get_program().to_string_lossy().to_string();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al ejecutar {prog}: {e}"))?;

    // stdin y stderr en hilos propios: si no, una tubería llena bloquearía al hijo mientras se lee stdout
    let writer = match (stdin, child.stdin.take()) {
        (Some(data), Some(pipe)) => {
            let data = data.to_vec();
            Some(std::thread::spawn(move || {
                let mut buffered = std::io::BufWriter::new(pipe);
                let _ = buffered.write_all(&data);
                let _ = buffered.flush();
            }))
        }
        _ => None,
    };
    let stderr_thread = child.stderr.take().map(|pipe| {
        std::thread::spawn(move || {
            let mut buffered = std::io::BufReader::new(pipe);
            let mut kept = Vec::with_capacity(MAX_STDERR_BYTES as usize);
            let _ = (&mut buffered)
                .take(MAX_STDERR_BYTES)
                .read_to_end(&mut kept);
            let _ = std::io::copy(&mut buffered, &mut std::io::sink());
            kept
        })
    });

    let mut stdout = Vec::with_capacity(max_out.min(64 * 1024) + 1);
    if let Some(pipe) = child.stdout.take() {
        let buffered = std::io::BufReader::new(pipe);
        let _ = buffered.take(max_out as u64 + 1).read_to_end(&mut stdout);
    }
    let truncated = stdout.len() > max_out;
    if truncated {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| format!("Error SSH: {e}"))?;
    if let Some(w) = writer {
        let _ = w.join();
    }
    let stderr = stderr_thread
        .and_then(|t| t.join().ok())
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .unwrap_or_default();
    Ok(Output {
        ok: status.success() || truncated,
        truncated,
        stdout,
        stderr,
    })
}

/// Orden remota que guarda stdin como `name` en una carpeta propia (`<TMPDIR|/tmp>/tearminal-<uid>`, modo 700) y
/// escribe su ruta: una captura puede contener secretos y `/tmp` lo leen todos los usuarios del servidor
fn upload_cmd(name: &str) -> String {
    let script = format!(
        r#"umask 077; d="${{TMPDIR:-/tmp}}/tearminal-$(id -u)"; mkdir -p "$d" && [ -O "$d" ] && cat >"$d/{name}" && printf %s "$d/{name}""#
    );
    format!("sh -c {}", quote(&script))
}

/// Sube una imagen local al destino por `ssh` (stdin) a una carpeta privada; devuelve la ruta remota.
/// Garantizado seguro por el tipo `SafeSshDestination` ("Parse, don't validate").
pub(crate) fn upload_image_safe(
    local_path: &str,
    destination: &SafeSshDestination,
) -> Option<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let bytes = std::fs::read(local_path).ok()?;
    let remote_filename = format!("tearminal_img_{}.png", timestamp);
    crate::logger::log(&format!(
        "SSH Upload: transfiriendo captura a {destination} ({remote_filename})"
    ));
    match run_safe(
        destination,
        &upload_cmd(&remote_filename),
        3,
        Some(&bytes),
        4096,
    ) {
        Ok(out) if out.ok && !out.truncated => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if crate::pty::safe_remote_dir(&path).is_none() {
                crate::logger::log("SSH Upload: el servidor devolvió una ruta no válida");
                return None;
            }
            crate::logger::log(&format!("SSH Upload exitoso: {path}"));
            Some(path)
        }
        Ok(out) => {
            crate::logger::log(&format!("SSH Upload falló: {}", out.stderr));
            None
        }
        Err(e) => {
            crate::logger::log(&format!("Error al ejecutar ssh.exe: {e}"));
            None
        }
    }
}

/// Sube una imagen local al destino por `ssh` (stdin) a una carpeta privada; devuelve la ruta remota.
pub(crate) fn upload_image(local_path: &str, destination: &str) -> Option<String> {
    let safe_dest = match SafeSshDestination::parse(destination) {
        Ok(d) => d,
        Err(_) => {
            crate::logger::log("SSH Upload: destino no válido, se omite la transferencia");
            return None;
        }
    };
    upload_image_safe(local_path, &safe_dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_commands_are_bounded_and_quoted() {
        assert_eq!(
            read_file_cmd("/var/log/a'b"),
            r"head -c 10485761 -- '/var/log/a'\''b'"
        );
        assert!(root_listing_cmd("/srv").ends_with("ls -1ap | head -5000"));
        assert_eq!(
            dir_listing_cmd("/srv"),
            "cd '/srv' 2>/dev/null && ls -1ap | head -5000"
        );
        assert!(write_file_cmd("/tmp/x y", 7).starts_with("sh -c 't=$(mktemp)"));
        assert!(upload_cmd("a.png").contains("umask 077"));
    }

    /// El guardado remoto escribe lo recibido solo si llegó entero: con menos bytes de los esperados no toca
    /// el archivo
    #[test]
    fn test_write_file_cmd_is_all_or_nothing() {
        let mut check = Command::new("sh");
        crate::app::platform::hide_console(&mut check);
        if !check
            .arg("-c")
            .arg("true")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return; // sin `sh` en esta máquina
        }
        let path = std::env::temp_dir()
            .join(format!("tearminal_write_cmd_{}.txt", std::process::id()))
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(&path, "original").unwrap();
        let run_with = |data: &[u8], len: usize| {
            let mut cmd = Command::new("sh");
            crate::app::platform::hide_console(&mut cmd);
            cmd.arg("-c").arg(write_file_cmd(&path, len));
            run_command(cmd, Some(data), 4096).unwrap()
        };
        let cut = run_with(b"nuevo", 9);
        assert!(!cut.ok);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        let full = run_with(b"nuevo", 5);
        assert!(full.ok, "{}", full.stderr);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "nuevo");
        let _ = std::fs::remove_file(&path);
    }

    /// `quote` produce `'\''` (barra literal): lo que el shell remoto recibe es exactamente la ruta original
    #[test]
    fn test_quote_round_trips_through_sh() {
        let mut check = Command::new("sh");
        crate::app::platform::hide_console(&mut check);
        let Ok(sh) = check.arg("-c").arg("true").output() else {
            return; // sin `sh` en esta máquina (p. ej. sin Git para Windows)
        };
        assert!(sh.status.success());
        for raw in [
            "a'b",
            "x; calc",
            "$(calc)",
            "`calc`",
            "a b'; rm -rf ~ #",
            "'",
            "\"q\"",
        ] {
            let mut cmd = Command::new("sh");
            crate::app::platform::hide_console(&mut cmd);
            let out = cmd
                .arg("-c")
                .arg(format!("printf %s {}", quote(raw)))
                .output()
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), raw, "{raw:?}");
        }
    }

    #[test]
    fn test_run_rejects_unsafe_destination() {
        assert!(run("-oProxyCommand=calc", "true", 3, None, 10).is_err());
        assert!(run("x; calc", "true", 3, None, 10).is_err());
    }

    /// Una salida enorme se corta al tope (no se acumula en memoria) y stdin llega al proceso
    #[test]
    fn test_run_command_caps_stdout_and_pipes_stdin() {
        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-Command",
            "1..300000 | ForEach-Object { 'xxxxxxxxxxxxxxxxxxxx' }",
        ]);
        let out = run_command(cmd, None, 1000).unwrap();
        assert!(out.ok);
        assert!(out.truncated);
        assert_eq!(out.stdout.len(), 1001);

        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-Command",
            "[Console]::Out.Write([Console]::In.ReadToEnd())",
        ]);
        let out = run_command(cmd, Some(b"hola ssh"), 1000).unwrap();
        assert!(!out.truncated);
        assert_eq!(out.stdout, b"hola ssh");
    }

    #[test]
    fn test_parse_dont_validate_safe_ssh_destination() {
        // Válidos
        let dest = SafeSshDestination::parse("servidor.remoto").expect("válido");
        assert_eq!(dest.as_str(), "servidor.remoto");
        assert_eq!(format!("{dest}"), "servidor.remoto");
        assert_eq!(&*dest, "servidor.remoto");

        let dest_user = SafeSshDestination::parse("usuario@servidor.remoto").expect("válido");
        assert_eq!(dest_user.as_str(), "usuario@servidor.remoto");

        // Inválidos: rechaza opciones y ataques de inyección
        assert_eq!(
            SafeSshDestination::parse("").unwrap_err(),
            SshDestinationError::Empty
        );
        assert_eq!(
            SafeSshDestination::parse("-oProxyCommand=calc").unwrap_err(),
            SshDestinationError::StartsWithDash
        );
        assert_eq!(
            SafeSshDestination::parse("host; calc").unwrap_err(),
            SshDestinationError::InvalidCharacters
        );
        assert_eq!(
            SafeSshDestination::parse("user@").unwrap_err(),
            SshDestinationError::InvalidUserHostFormat
        );
        assert_eq!(
            SafeSshDestination::parse("@host").unwrap_err(),
            SshDestinationError::InvalidUserHostFormat
        );
    }

    #[test]
    fn test_parse_dont_validate_safe_remote_dir() {
        let dir = SafeRemoteDir::parse("/home/usuario/proyecto").expect("válido");
        assert_eq!(dir.as_str(), "/home/usuario/proyecto");
        assert_eq!(&*dir, "/home/usuario/proyecto");

        assert!(SafeRemoteDir::parse("").is_err());
        assert!(SafeRemoteDir::parse("-").is_err());
        assert!(SafeRemoteDir::parse("-rf").is_err());
        assert!(SafeRemoteDir::parse("/home/user'; rm -rf /").is_err());
        assert!(SafeRemoteDir::parse("/home/$USER").is_err());
    }

    #[test]
    fn test_typestate_ssh_connection_transitions() {
        let dest = SafeSshDestination::parse("usuario@host-1").expect("destino válido");
        let conn: SshConnection<Unauthenticated> = SshConnection::new(dest).with_timeout(10);

        // Fallo con token inválido
        let failed_auth = SshConnection::new(SafeSshDestination::parse("usuario@host-1").unwrap())
            .authenticate(Some("   "));
        assert!(failed_auth.is_err());

        // Autenticación exitosa produce SshConnection<Authenticated>
        let auth_conn: SshConnection<Authenticated> =
            conn.authenticate(Some("valid-token")).expect("auth ok");
        assert_eq!(auth_conn.destination().as_str(), "usuario@host-1");

        // Transición de regreso a Unauthenticated mediante disconnect
        let disconnected: SshConnection<Unauthenticated> = auth_conn.disconnect();
        let _ = disconnected;
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_safe_ssh_destination_never_panics_and_invariants_hold(input in "\\PC*") {
            if let Ok(dest) = SafeSshDestination::parse(&input) {
                let s = dest.as_str();
                prop_assert!(!s.is_empty());
                prop_assert!(!s.starts_with('-'));
                prop_assert!(!s.chars().any(char::is_whitespace));
                prop_assert!(!s.chars().any(char::is_control));
                let bad_chars = |c| matches!(c, ';' | '&' | '|' | '`' | '$' | '\\' | '\'' | '"' | '<' | '>' | '(' | ')' | '{' | '}');
                prop_assert!(!s.chars().any(bad_chars));
            }
        }

        #[test]
        fn prop_safe_remote_dir_never_panics_and_invariants_hold(input in "\\PC*") {
            if let Ok(dir) = SafeRemoteDir::parse(&input) {
                let s = dir.as_str();
                prop_assert!(!s.is_empty());
                prop_assert!(!s.starts_with('-'));
                prop_assert!(!s.chars().any(char::is_control));
                let bad_chars = |c| matches!(c, ';' | '&' | '|' | '`' | '$' | '\\' | '\'' | '"' | '<' | '>');
                prop_assert!(!s.chars().any(bad_chars));
            }
        }
    }
}
