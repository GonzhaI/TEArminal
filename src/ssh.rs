//! `ssh`/`scp` no interactivos (BatchMode) con salida acotada para las vías lentas del explorador y el visor
//! remotos (sin integración en banda, o cuando esta falla). BLOQUEAN (conexión + transferencia): llámalas solo
//! desde un hilo (`AxonApp::spawn_ssh`), nunca desde el bucle de eventos.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Tope de un archivo remoto que se abre en el visor (se piden `MAX_FILE_BYTES + 1` para saber que lo supera)
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
/// Entradas como máximo de un listado remoto
pub const LISTING_ENTRIES: usize = 300;
/// Tope de lo que se lee de una orden de listado (300 nombres de hasta 255 bytes, con margen)
pub const MAX_LISTING_BYTES: usize = 128 * 1024;
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

pub fn write_file_cmd(path: &str) -> String {
    format!("cat > {}", quote(path))
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
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
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

/// Sube una imagen local a `/tmp` del destino con `scp`; devuelve la ruta remota
pub fn upload_image(local_path: &str, destination: &str) -> Option<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    if !crate::pty::is_safe_ssh_destination(destination) {
        crate::logger::log("SSH Upload: destino no válido, se omite la transferencia");
        return None;
    }
    let remote_filename = format!("axon_img_{}.png", timestamp);
    let remote_target = format!("{}:/tmp/{}", destination, remote_filename);

    crate::logger::log(&format!(
        "SSH Upload: transfiriendo captura a {} -> /tmp/{}",
        destination, remote_filename
    ));

    let mut cmd = Command::new("scp.exe");
    cmd.args([
        "-q",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=3",
        "-o",
        "ServerAliveInterval=5",
        "-o",
        "ServerAliveCountMax=2",
        "--",
    ])
    .arg(local_path)
    .arg(&remote_target);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    match cmd.status() {
        Ok(s) if s.success() => {
            crate::logger::log(&format!("SSH Upload exitoso: /tmp/{}", remote_filename));
            Some(format!("/tmp/{}", remote_filename))
        }
        Ok(s) => {
            crate::logger::log(&format!("SSH Upload falló con código: {:?}", s.code()));
            None
        }
        Err(e) => {
            crate::logger::log(&format!("Error al ejecutar scp.exe: {:?}", e));
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
            r"head -c 5242881 -- '/var/log/a'\''b'"
        );
        assert!(root_listing_cmd("/srv").ends_with("ls -1ap | head -300"));
        assert_eq!(
            dir_listing_cmd("/srv"),
            "cd '/srv' 2>/dev/null && ls -1ap | head -300"
        );
        assert_eq!(write_file_cmd("/tmp/x y"), "cat > '/tmp/x y'");
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
