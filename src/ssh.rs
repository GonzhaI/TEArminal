//! `ssh` no interactivo (BatchMode) con salida acotada para las vías lentas del explorador y el visor
//! remotos (sin integración en banda, o cuando esta falla). BLOQUEAN (conexión + transferencia): llámalas solo
//! desde un hilo (`AxonApp::spawn_ssh`), nunca desde el bucle de eventos.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Tope de un archivo remoto que se abre en el visor (se piden `MAX_FILE_BYTES + 1` para saber que lo supera)
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// Entradas como máximo de un listado remoto
pub const LISTING_ENTRIES: usize = 5000;
/// Tope de lo que se lee de una orden de listado (5000 nombres de hasta 255 bytes, con margen)
pub const MAX_LISTING_BYTES: usize = 2 * 1024 * 1024;
/// Tope de lo que se conserva de stderr
const MAX_STDERR_BYTES: u64 = 16 * 1024;

pub struct Output {
    pub ok: bool,
    pub truncated: bool,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// Entre comillas simples para el shell remoto
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Primeros `MAX_FILE_BYTES + 1` bytes del archivo: el remoto nunca envía más de lo que Axon va a leer
pub fn read_file_cmd(path: &str) -> String {
    format!("head -c {} -- {}", MAX_FILE_BYTES + 1, quote(path))
}

/// Guarda lo que llega por stdin (`len` bytes) en `path` sin dejarlo a medias: se recibe entero en un temporal
/// y solo se copia si tiene exactamente `len` bytes (una conexión cortada a mitad no trunca el original).
/// Va en `sh -c` para no depender del shell de login del servidor (fish, csh…).
pub fn write_file_cmd(path: &str, len: usize) -> String {
    let script = format!(
        r#"t=$(mktemp) && cat >"$t" && [ "$(wc -c <"$t")" -eq {len} ] && cat "$t" >{}; s=$?; rm -f "$t"; exit $s"#,
        quote(path)
    );
    format!("sh -c {}", quote(&script))
}

/// Carpeta actual canónica, rama git y listado de la raíz del explorador remoto
pub fn root_listing_cmd(dir: &str) -> String {
    format!(
        "cd {} 2>/dev/null && pwd && (git rev-parse --abbrev-ref HEAD 2>/dev/null || true) && echo '---' && ls -1ap | head -{}",
        quote(dir),
        LISTING_ENTRIES
    )
}

pub fn dir_listing_cmd(dir: &str) -> String {
    format!(
        "cd {} 2>/dev/null && ls -1ap | head -{}",
        quote(dir),
        LISTING_ENTRIES
    )
}

/// Ejecuta `remote_cmd` en `dest`. Lee como mucho `max_out + 1` bytes de stdout (si hay más se corta la
/// conexión y `stdout.len() > max_out` avisa al llamador); `stdin` se envía al remoto.
pub fn run(
    dest: &str,
    remote_cmd: &str,
    connect_timeout: u32,
    stdin: Option<&[u8]>,
    max_out: usize,
) -> Result<Output, String> {
    if !crate::pty::is_safe_ssh_destination(dest) {
        return Err(format!("destino SSH no válido: {dest:?}"));
    }
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
    .args(["--", dest, remote_cmd]);
    run_command(cmd, stdin, max_out)
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
        (Some(data), Some(mut pipe)) => {
            let data = data.to_vec();
            Some(std::thread::spawn(move || {
                let _ = pipe.write_all(&data);
            }))
        }
        _ => None,
    };
    let stderr_thread = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let _ = (&mut pipe).take(MAX_STDERR_BYTES).read_to_end(&mut kept);
            let _ = std::io::copy(&mut pipe, &mut std::io::sink());
            kept
        })
    });

    let mut stdout = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        let _ = pipe.take(max_out as u64 + 1).read_to_end(&mut stdout);
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

/// Orden remota que guarda stdin como `name` en una carpeta propia (`<TMPDIR|/tmp>/axon-<uid>`, modo 700) y
/// escribe su ruta: una captura puede contener secretos y `/tmp` lo leen todos los usuarios del servidor
fn upload_cmd(name: &str) -> String {
    let script = format!(
        r#"umask 077; d="${{TMPDIR:-/tmp}}/axon-$(id -u)"; mkdir -p "$d" && [ -O "$d" ] && cat >"$d/{name}" && printf %s "$d/{name}""#
    );
    format!("sh -c {}", quote(&script))
}

/// Sube una imagen local al destino por `ssh` (stdin) a una carpeta privada; devuelve la ruta remota
pub fn upload_image(local_path: &str, destination: &str) -> Option<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    if !crate::pty::is_safe_ssh_destination(destination) {
        crate::logger::log("SSH Upload: destino no válido, se omite la transferencia");
        return None;
    }
    let bytes = std::fs::read(local_path).ok()?;
    let remote_filename = format!("axon_img_{}.png", timestamp);
    crate::logger::log(&format!(
        "SSH Upload: transfiriendo captura a {destination} ({remote_filename})"
    ));
    match run(
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
        if !Command::new("sh")
            .arg("-c")
            .arg("true")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return; // sin `sh` en esta máquina
        }
        let path = std::env::temp_dir()
            .join(format!("axon_write_cmd_{}.txt", std::process::id()))
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(&path, "original").unwrap();
        let run_with = |data: &[u8], len: usize| {
            let mut cmd = Command::new("sh");
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
        let Ok(sh) = Command::new("sh").arg("-c").arg("true").output() else {
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
            let out = Command::new("sh")
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
}
