use crate::app::AxonEvent;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use winit::event_loop::EventLoopProxy;

/// Destino SSH aceptado: `host` o `usuario@host` (letras, dígitos, `.`, `_`, `-`), sin empezar por `-`.
///
/// El destino puede venir de secuencias de escape que emite cualquier programa (título OSC 0/2, OSC 1337,
/// OSC 7), es decir, es entrada NO confiable. Se interpola en una orden de PowerShell y en `ssh.exe`, así que
/// solo se admite este alfabeto: nada de espacios, `;`, `$`, comillas ni opciones (`-oProxyCommand=...`).
pub fn is_safe_ssh_destination(dest: &str) -> bool {
    let ok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if dest.is_empty() || dest.len() > 255 || dest.starts_with('-') {
        return false;
    }
    let mut parts = dest.splitn(2, '@');
    let first = parts.next().unwrap_or("");
    match parts.next() {
        None => first.chars().all(ok_char),
        Some(host) => {
            !first.is_empty()
                && !host.is_empty()
                && first.chars().all(ok_char)
                && host.chars().all(ok_char)
        }
    }
}

/// Directorio remoto reportado por la sesión (OSC 7) si es seguro de interpolar entre comillas simples dentro
/// de una orden de PowerShell: sin `'`, `"`, `$`, `` ` ``, `;`, `&`, `|`, `<`, `>`, `\`, etc.
pub fn safe_remote_dir(dir: &str) -> Option<&str> {
    let ok = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '/' | '.' | '_' | '-' | '~' | '+' | '@' | ':' | ',' | '=' | ' '
            )
    };
    (!dir.is_empty() && dir.len() <= 1024 && dir.chars().all(ok)).then_some(dir)
}

/// Orden de PowerShell que reconecta por SSH. La orden remota va entre comillas simples: PowerShell no
/// expande `$SHELL` ni trata `\` como escape (con comillas dobles el shell remoto recibía `exec \ -l`).
/// `--` corta el análisis de opciones de ssh: el destino nunca puede ser una opción.
fn ssh_command_line(destination: &str, remote_dir: Option<&str>) -> String {
    match remote_dir.and_then(safe_remote_dir) {
        Some(dir) => format!("ssh -t -- {destination} 'cd ''{dir}'' 2>/dev/null; exec $SHELL -l'"),
        None => format!("ssh -- {destination}"),
    }
}

/// Inicialización de PowerShell (prompt OSC 7/133, ls, agentes de IA, ssh): ver `assets/shell/init.ps1`
const INIT_SCRIPT: &str = include_str!("../assets/shell/init.ps1");

impl Drop for PtyManager {
    /// Cerrar un panel/pestaña (o la ventana) debe terminar siempre el shell. Soltar el ConPTY por sí solo
    /// cierra su consola, pero no alcanza a un shell que aún está arrancando: sobrevivía como proceso huérfano
    /// (cada `pwsh` ocupa ~100 MB). `TerminateProcess` sobre un proceso ya terminado es inofensivo.
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Mensaje de error nativo. Axon usa el subsistema "windows" (sin consola) y `panic = "abort"`:
/// sin esto, un fallo de ConPTY cerraría la app sin que el usuario viera nada.
pub fn report_spawn_error(shell: &str, err: &dyn std::fmt::Display) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    crate::logger::log(&format!("Error al iniciar la terminal ({shell}): {err}"));
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    let text = wide(&format!(
        "Axon no pudo iniciar la terminal.\n\nShell: {shell}\nError: {err}\n\n\
         Axon necesita Windows 10 versión 1809 (build 17763) o superior (ConPTY) y que la shell \
         configurada exista. Revisa `default_shell` en axon.toml."
    ));
    let title = wide("Axon");
    // SAFETY: cadenas UTF-16 terminadas en nulo que viven durante la llamada
    unsafe { MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
}

/// Error irrecuperable al crear la primera terminal: informar y salir
pub fn fatal_spawn_error(shell: &str, err: &dyn std::fmt::Display) -> ! {
    report_spawn_error(shell, err);
    std::process::exit(1)
}

/// 128 bits aleatorios del sistema; `None` si el RNG falla.
fn random_bytes() -> Option<[u8; 16]> {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut buf = [0u8; 16];
    // SAFETY: búfer propio de 16 bytes; con `BCRYPT_USE_SYSTEM_PREFERRED_RNG` el manejador debe ser nulo
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    (status == 0).then_some(buf)
}

/// Token aleatorio en hex. Si el RNG falla devuelve vacío: sin token no se acepta ninguna sesión SSH.
fn random_token() -> String {
    random_bytes()
        .map(|b| b.iter().map(|b| format!("{b:02x}")).collect())
        .unwrap_or_default()
}

/// Identificador inicial aleatorio de las peticiones en banda (si el RNG falla, 1)
pub fn random_id() -> u32 {
    random_bytes().map_or(1, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Pila reducida: estos hilos solo copian bytes (el defecto de 2 MB se reserva por hilo)
const THREAD_STACK: usize = 256 * 1024;

pub struct PtyManager {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    to_pty_tx: Sender<Vec<u8>>,
    from_pty_rx: Receiver<Vec<u8>>,
    /// Hay un `PtyData` en vuelo: el lector no encola otro hasta que el consumidor vacíe el canal
    output_pending: Arc<AtomicBool>,
    /// Secreto de este shell (`AXON_PANE_TOKEN`): solo el wrapper `ssh` de `init.ps1` lo conoce y lo adjunta a
    /// `AxonSSH=start`. Vacío en los paneles que no ejecutan `init.ps1` (ningún marcador es válido).
    pub token: String,
}

/// Hilos lector/escritor del PTY (compartidos por shell local y SSH). Devuelve (tx entrada, rx salida, flag de coalescencia).
/// (tx de entrada, rx de salida, flag de coalescencia de eventos)
type IoChannels = (Sender<Vec<u8>>, Receiver<Vec<u8>>, Arc<AtomicBool>);

fn spawn_io_threads(
    master: &(dyn MasterPty + Send),
    id: usize,
    proxy: Option<EventLoopProxy<AxonEvent>>,
) -> Result<IoChannels, Box<dyn std::error::Error>> {
    let mut reader = master.try_clone_reader()?;
    let mut writer = master.take_writer()?;
    // Acotado (64 x 16 KB): si la interfaz va por detrás de un `cat` enorme el lector espera y ConPTY frena al
    // proceso, en vez de acumular la salida entera en memoria
    let (from_pty_tx, from_pty_rx) = sync_channel::<Vec<u8>>(64);
    let (to_pty_tx, to_pty_rx) = channel::<Vec<u8>>();
    let pending = Arc::new(AtomicBool::new(false));
    let pending_reader = pending.clone();

    thread::Builder::new()
        .name("axon-pty-reader".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            let mut buffer = [0u8; 16384];
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 || from_pty_tx.send(buffer[..n].to_vec()).is_err() {
                    break;
                }
                // Coalescer: un solo evento por ráfaga, no uno por lectura
                if !pending_reader.swap(true, Ordering::SeqCst) {
                    if let Some(p) = &proxy {
                        let _ = p.send_event(AxonEvent::PtyData(id));
                    }
                }
            }
            if let Some(p) = &proxy {
                // ConPTY cierra el pipe unos milisegundos antes de que `try_wait()` vea el fin del proceso:
                // esperar aquí (en el hilo lector) evita bloquear el hilo de la interfaz.
                thread::sleep(std::time::Duration::from_millis(30));
                let _ = p.send_event(AxonEvent::PtyClosed(id));
            }
        })?;

    thread::Builder::new()
        .name("axon-pty-writer".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            while let Ok(data) = to_pty_rx.recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        })?;

    Ok((to_pty_tx, from_pty_rx, pending))
}

fn build_shell_command(
    shell: &str,
    shell_integration: bool,
    cwd: Option<&std::path::Path>,
    token: &str,
    path_env: &str,
) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if let Some(dir) = cwd.filter(|d| d.exists()) {
        cmd.cwd(dir);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("AXON_PANE_TOKEN", token);
    cmd.env("PATH", path_env);
    if shell_integration {
        let lower = shell.to_lowercase();
        if lower.contains("powershell") || lower.contains("pwsh") {
            cmd.arg("-NoLogo");
            cmd.arg("-NoExit");
            cmd.arg("-Command");
            cmd.arg(INIT_SCRIPT);
        }
    }
    cmd
}

impl PtyManager {
    pub fn new(
        id: usize,
        shell: &str,
        shell_integration: bool,
        cols: u16,
        rows: u16,
        cwd: Option<&std::path::Path>,
        proxy: Option<EventLoopProxy<AxonEvent>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let token = random_token();
        // `bin\` de Axon (herramientas opcionales) al final del PATH: las del usuario tienen prioridad
        let path_env = crate::tools::path_with_bundled_tools();
        let cmd = build_shell_command(shell, shell_integration, cwd, &token, &path_env);

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                if shell != "powershell.exe" {
                    let fallback = build_shell_command(
                        "powershell.exe",
                        shell_integration,
                        cwd,
                        &token,
                        &path_env,
                    );
                    pair.slave.spawn_command(fallback)?
                } else {
                    return Err(e.into());
                }
            }
        };

        let (to_pty_tx, from_pty_rx, output_pending) = spawn_io_threads(&*pair.master, id, proxy)?;

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            child,
            to_pty_tx,
            from_pty_rx,
            output_pending,
            token,
        })
    }

    /// Crea un nuevo PTY reconectando automáticamente a una sesión SSH remota
    pub fn new_ssh(
        id: usize,
        destination: &str,
        remote_dir: Option<&str>,
        cols: u16,
        rows: u16,
        proxy: Option<EventLoopProxy<AxonEvent>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !is_safe_ssh_destination(destination) {
            return Err(format!("destino SSH no válido: {destination:?}").into());
        }
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // `bin\` de Axon (herramientas opcionales) al final del PATH: las del usuario tienen prioridad
        let path_env = crate::tools::path_with_bundled_tools();
        cmd.env("PATH", &path_env);

        cmd.arg("-NoLogo");
        cmd.arg("-NoExit");
        cmd.arg("-Command");

        cmd.arg(ssh_command_line(destination, remote_dir));
        let child = pair.slave.spawn_command(cmd)?;

        let (to_pty_tx, from_pty_rx, output_pending) = spawn_io_threads(&*pair.master, id, proxy)?;

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            child,
            to_pty_tx,
            from_pty_rx,
            output_pending,
            token: String::new(),
        })
    }

    pub fn is_child_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    pub fn write_input(&self, data: &[u8]) {
        let _ = self.to_pty_tx.send(data.to_vec());
    }

    pub fn try_recv_output(&self) -> Option<Vec<u8>> {
        if let Ok(bytes) = self.from_pty_rx.try_recv() {
            return Some(bytes);
        }
        // Canal vacío: rearmar el evento y reintentar una vez para no perder datos en carrera
        self.output_pending.store(false, Ordering::SeqCst);
        self.from_pty_rx.try_recv().ok()
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El shell recibe el token del panel (lo adjunta el wrapper `ssh` a `AxonSSH=start`) y es único por panel
    #[test]
    fn test_pane_token_reaches_shell_and_is_unique() {
        let pty = PtyManager::new(0, "powershell.exe", false, 80, 24, None, None).unwrap();
        let other = PtyManager::new(1, "powershell.exe", false, 80, 24, None, None).unwrap();
        assert_eq!(pty.token.len(), 32);
        assert_ne!(pty.token, other.token);
        std::thread::sleep(std::time::Duration::from_millis(1500)); // arranque de PowerShell
        pty.write_input(b"Write-Output ('T=' + $env:AXON_PANE_TOKEN)\r\n");
        let mut out = String::new();
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                out.push_str(&String::from_utf8_lossy(&bytes));
            }
            if out.contains(&format!("T={}", pty.token)) {
                return;
            }
        }
        panic!("el shell no recibió AXON_PANE_TOKEN: {out:?}");
    }

    #[test]
    fn test_pty_powershell_spawn_and_read() {
        let pty = PtyManager::new(0, "powershell.exe", false, 80, 24, None, None).unwrap();
        let mut total_output = String::new();
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                total_output.push_str(&s);
            }
        }
        println!(
            "TOTAL PTY OUTPUT (len {}):\n{:?}",
            total_output.len(),
            total_output
        );

        // Enviar un comando "echo hello\r\n" a ver si responde
        pty.write_input(b"echo hello\r\n");
        let mut echo_output = String::new();
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                echo_output.push_str(&s);
            }
            if echo_output.contains("hello") {
                break;
            }
        }
        println!(
            "ECHO OUTPUT (len {}):\n{:?}",
            echo_output.len(),
            echo_output
        );
        assert!(
            echo_output.contains("hello"),
            "El shell debe responder al input del usuario"
        );

        // Probar Backspace con 0x7f (debe borrar solo 1 carácter)
        pty.write_input(b"wordX\x7f\r\n");
        let mut bs_output = String::new();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                bs_output.push_str(&s);
            }
            if bs_output.contains("word") {
                break;
            }
        }
        // Probar exactamente el texto de la captura del usuario
        pty.write_input(
            b"asldkflsdkflsdkflsadkf lasdkflsdkflasfksadlfk alsdkflsakdflasdkf laskdflksdlf",
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
        while pty.try_recv_output().is_some() {}

        // Enviar 0x08 para borrar la última palabra
        pty.write_input(&[0x08]);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del1 = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del1.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("DEL 1 PTY OUTPUT: {:?}", del1);

        // Enviar 0x08 para borrar otra palabra
        pty.write_input(&[0x08]);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del2 = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del2.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("DEL 2 PTY OUTPUT: {:?}", del2);
    }

    #[test]
    fn test_powershell_and_harness_kill_word() {
        let pty = PtyManager::new(0, "powershell.exe", true, 120, 30, None, None).unwrap();
        // Esperar al prompt (no a un tiempo fijo): si PowerShell aún arranca, descarta lo que se teclea.
        let (ready, boot) = read_until(&pty, "]133;B", 40_000);
        assert!(ready, "el prompt de Axon no apareció: {boot:?}");

        // 1. Probar 0x17 (Ctrl+W)
        pty.write_input(b"echo first second");
        std::thread::sleep(std::time::Duration::from_millis(200));
        while pty.try_recv_output().is_some() {}

        pty.write_input(&[0x17]); // Ctrl+W / BackwardKillWord
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del_ctrl_w = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del_ctrl_w.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("CTRL+W (0x17) PTY OUTPUT: {:?}", del_ctrl_w);

        // 2. Probar \x1b\x7f (Alt+Backspace)
        pty.write_input(b" third");
        std::thread::sleep(std::time::Duration::from_millis(200));
        while pty.try_recv_output().is_some() {}

        pty.write_input(b"\x1b\x7f"); // Alt+Backspace
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del_alt_bs = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del_alt_bs.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("ALT+BACKSPACE (\\x1b\\x7f) PTY OUTPUT: {:?}", del_alt_bs);

        // 3. Probar \x1b\r (Shift+Enter / Alt+Enter): salto de línea sin enviar hasta pulsar \r
        pty.write_input(&[0x03]); // limpiar línea previa; PSReadLine repinta el prompt al terminar
        let (ready, cleared) = read_until(&pty, "]133;B", 10_000);
        assert!(ready, "tras Ctrl+C no volvió el prompt: {cleared:?}");

        pty.write_input(b"Write-Output 'LINE_A'\x1b\rWrite-Output 'LINE_B'");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut before_enter = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            before_enter.push_str(&String::from_utf8_lossy(&bytes));
        }
        assert!(
            !before_enter.contains("\nLINE_A"),
            "\\x1b\\r no debe ejecutar la primera línea antes de Enter: {before_enter:?}"
        );
        pty.write_input(b"\r");
        let (ok, after_enter) = read_until(&pty, "LINE_B", 5_000);
        assert!(
            ok && (before_enter.contains("LINE_A") || after_enter.contains("LINE_A")),
            "tras Enter deben ejecutarse ambas líneas: {before_enter:?} / {after_enter:?}"
        );
    }

    /// Lee del PTY hasta ver `needle` o agotar el tiempo. Devuelve (encontrado, salida acumulada).
    fn read_until(pty: &PtyManager, needle: &str, timeout_ms: u64) -> (bool, String) {
        let mut out = String::new();
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            while let Some(bytes) = pty.try_recv_output() {
                out.push_str(&String::from_utf8_lossy(&bytes));
            }
            if out.contains(needle) {
                return (true, out);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        (false, out)
    }

    /// Intérpretes de PowerShell instalados: siempre `powershell.exe` (5.1, viene con Windows) y
    /// `pwsh.exe` 7 si existe como binario real (no el alias de la Tienda, que bloquea ConPTY).
    fn installed_shells() -> Vec<String> {
        let mut shells = vec!["powershell.exe".to_string()];
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        let candidates = std::env::split_paths(&path_var)
            .filter(|p| !p.to_string_lossy().contains("WindowsApps"))
            .map(|p| p.join("pwsh.exe"))
            .chain(std::iter::once(std::path::PathBuf::from(
                r"C:\Program Files\PowerShell\7\pwsh.exe",
            )));
        if let Some(pwsh) = candidates.into_iter().find(|p| p.is_file()) {
            shells.push(pwsh.to_string_lossy().to_string());
        }
        shells
    }

    /// El script de inicio (assets/shell/init.ps1) debe funcionar igual en cada versión de PowerShell:
    /// prompt con OSC 7 y OSC 133, rama git leída de .git/HEAD, búsqueda de programas y sin errores.
    #[test]
    fn test_init_script_in_every_installed_powershell() {
        let repo = std::env::temp_dir().join(format!("axon_test_repo_{}", std::process::id()));
        let cwd = repo.join("src");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(
            repo.join(".git").join("HEAD"),
            "ref: refs/heads/axon-test-branch\n",
        )
        .unwrap();

        let fake_agy = cwd.join("fake-agy.cmd");
        std::fs::write(&fake_agy, "@echo off\r\necho AGY_STATUS_%1_OK\r\n").unwrap();

        // En CI deben estar las dos versiones: si no, el test pasaría sin probar PowerShell 7
        if std::env::var_os("CI").is_some() {
            assert!(
                installed_shells().len() >= 2,
                "CI: se esperaba powershell.exe 5.1 y pwsh 7, hay {:?}",
                installed_shells()
            );
        }

        const ERRORS: [&str; 4] = [
            "CategoryInfo",
            "FullyQualifiedErrorId",
            "At line:",
            "En línea:",
        ];
        for shell in installed_shells() {
            let pty = PtyManager::new(0, &shell, true, 120, 30, Some(&cwd), None)
                .unwrap_or_else(|e| panic!("{shell}: no se pudo iniciar: {e}"));

            // 1. Arranque: el prompt completo termina con OSC 133;B
            let (ok, boot) = read_until(&pty, "]133;B", 40_000);
            assert!(
                ok,
                "{shell}: el prompt de Axon no apareció. Salida: {boot:?}"
            );
            assert!(
                boot.contains("]7;file://localhost/"),
                "{shell}: falta OSC 7 (cwd): {boot:?}"
            );
            assert!(
                boot.contains("]133;A"),
                "{shell}: falta OSC 133;A: {boot:?}"
            );
            assert!(
                boot.contains("axon-test-branch"),
                "{shell}: rama git no detectada: {boot:?}"
            );
            for e in ERRORS {
                assert!(
                    !boot.contains(e),
                    "{shell}: error en el script de inicio: {boot:?}"
                );
            }

            // 1b. Ningún error registrado por el script de inicio (el texto de los errores cambia entre
            //     PowerShell 5.1 y 7; `$Error.Count` no)
            pty.write_input(b"Write-Output ('E' + 'RRS=' + $Error.Count + ':' + (($Error | ForEach-Object { $_.FullyQualifiedErrorId }) -join ','))\r\n");
            let (ok, out) = read_until(&pty, "ERRS=", 20_000);
            assert!(ok, "{shell}: sin respuesta a $Error.Count: {out:?}");
            let (_, rest) = read_until(&pty, "\n", 2_000);
            let report = format!("{}{}", out.rsplit("ERRS=").next().unwrap_or(""), rest);
            println!("{shell}: ERRS={}", report.lines().next().unwrap_or(""));
            assert!(
                report.starts_with("0:"),
                "{shell}: el script de inicio registró errores: ERRS={report:?}"
            );

            // 2. Funciones del script: Find-AxonApp (existente / inexistente) y `ls` propio o eza
            pty.write_input(
                b"Write-Output ('FIND_' + ((Find-AxonApp 'cmd') -ne $null) + '_' + ((Find-AxonApp 'zzz-no-existe') -eq $null) + '_' + (6*7))\r\n",
            );
            let (ok, out) = read_until(&pty, "FIND_True_True_42", 20_000);
            assert!(ok, "{shell}: Find-AxonApp no funciona. Salida: {out:?}");
            pty.write_input(b"ls .; Write-Output ('LS_DONE_' + (6*7))\r\n");
            let (ok, out) = read_until(&pty, "LS_DONE_42", 20_000);
            assert!(ok, "{shell}: `ls` falló. Salida: {out:?}");
            for e in ERRORS {
                assert!(
                    !out.contains(e),
                    "{shell}: error al ejecutar funciones: {out:?}"
                );
            }

            // 3. Subcomandos de agentes (`agy update`, `--version`…) no deben limpiar la pantalla ni borrar su estado
            pty.write_input(
                b"$script:AxonApps['agy'] = (Join-Path $PWD 'fake-agy.cmd'); agy update; Write-Output ('AGY_DONE_' + (6*7))\r\n",
            );
            let (ok, agy_out) = read_until(&pty, "AGY_DONE_42", 20_000);
            assert!(ok, "{shell}: `agy update` no terminó. Salida: {agy_out:?}");
            assert!(
                agy_out.contains("AGY_STATUS_update_OK"),
                "{shell}: falta la salida de `agy update`: {agy_out:?}"
            );
            assert!(
                !agy_out.contains("\x1b[2J") && !agy_out.contains("\x1b[3J"),
                "{shell}: `agy update` limpió la pantalla: {agy_out:?}"
            );
            for e in ERRORS {
                assert!(
                    !agy_out.contains(e),
                    "{shell}: error al ejecutar `agy update`: {agy_out:?}"
                );
            }
            println!("[OK] {shell}: script de inicio verificado");
        }
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn test_pty_powershell_exit_detection() {
        let mut pty = PtyManager::new(0, "powershell.exe", false, 80, 24, None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !pty.is_child_exited(),
            "El proceso debería estar corriendo inicialmente"
        );
        pty.write_input(b"exit\r\n");
        let mut exited = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if pty.is_child_exited() {
                exited = true;
                break;
            }
        }
        assert!(
            exited,
            "El proceso hijo debe detectarse como finalizado al recibir exit"
        );
    }

    /// El destino SSH llega de secuencias de escape de cualquier programa: solo `host` / `usuario@host`.
    #[test]
    fn test_ssh_destination_rejects_injection() {
        for ok in [
            "host",
            "user@host",
            "user.name@host-1.example.com",
            "10.0.0.5",
            "a_b@c",
        ] {
            assert!(is_safe_ssh_destination(ok), "debe aceptar {ok:?}");
        }
        for bad in [
            "",
            "-oProxyCommand=calc",
            "-v",
            "host; calc",
            "host && calc",
            "$(calc)",
            "a b",
            "`calc`",
            "user@",
            "@host",
            "a@b@c",
            "host\n",
            "host'",
            "host\"",
            "host|calc",
        ] {
            assert!(!is_safe_ssh_destination(bad), "debe rechazar {bad:?}");
        }
        // el sumidero también valida (defensa en profundidad): ni siquiera abre el PTY
        assert!(PtyManager::new_ssh(0, "x; calc", None, 80, 24, None).is_err());
        assert!(PtyManager::new_ssh(0, "-oProxyCommand=calc", None, 80, 24, None).is_err());
    }

    #[test]
    fn test_ssh_command_reaches_ssh_with_remote_command_intact() {
        assert!(ssh_command_line("user@host", Some("/x")).starts_with("ssh -t -- user@host '"));
        // El `--` lo consume PowerShell al enlazar los argumentos de la función falsa; ssh.exe sí lo recibe
        for shell in installed_shells() {
            // Función `ssh` falsa: imprime cada argumento tal como lo recibiría el ejecutable
            let script = format!(
                "function ssh {{ $args | ForEach-Object {{ \"[$_]\" }} }}; {}",
                ssh_command_line("user@host", Some("/home/a b"))
            );
            let out = std::process::Command::new(&shell)
                .args(["-NoProfile", "-Command", &script])
                .output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .collect::<Vec<_>>(),
                [
                    "[-t]",
                    "[user@host]",
                    "[cd '/home/a b' 2>/dev/null; exec $SHELL -l]"
                ],
                "{shell}"
            );
        }
    }

    #[test]
    fn test_remote_dir_rejects_shell_metacharacters() {
        for ok in ["/home/user/proyecto", "~/mi carpeta", "/srv/app-1.2_x"] {
            assert_eq!(safe_remote_dir(ok), Some(ok));
        }
        for bad in [
            "",
            "'; calc; '",
            "/tmp/$(id)",
            "/tmp/`id`",
            "/a\"b",
            "C:\\x",
            "/a;b",
            "/a|b",
            "/a&b",
        ] {
            assert_eq!(safe_remote_dir(bad), None, "debe rechazar {bad:?}");
        }
    }

    /// ¿Sigue vivo el proceso `pid`?
    fn process_alive(pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        const STILL_ACTIVE: u32 = 259;
        // SAFETY: llamadas Win32 simples; el handle se cierra siempre
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h == 0 {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            ok && code == STILL_ACTIVE
        }
    }

    /// PIDs hijos directos de `parent` (consulta CIM; solo para tests)
    fn child_pids(parent: u32) -> Vec<u32> {
        let out = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "(Get-CimInstance Win32_Process -Filter 'ParentProcessId={parent}').ProcessId"
                ),
            ])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    }

    /// Cerrar un panel (soltar el `PtyManager`) o morir Axon no debe dejar PowerShell ni sus hijos vivos.
    #[test]
    fn test_dropping_pty_kills_shell_and_its_children() {
        let pty = PtyManager::new(0, "powershell.exe", true, 100, 30, None, None).unwrap();
        let shell = pty.child.process_id().expect("pid del shell");
        let (ok, boot) = read_until(&pty, "]133;B", 40_000);
        assert!(ok, "prompt no apareció: {boot:?}");

        // un hijo de larga duración (como `claude`, `ssh` o `ping`)
        pty.write_input(b"ping.exe -n 300 127.0.0.1 > $null\r\n");
        let mut children = Vec::new();
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            children = child_pids(shell);
            if !children.is_empty() {
                break;
            }
        }
        assert!(
            !children.is_empty(),
            "el shell debería tener un hijo (ping)"
        );
        assert!(process_alive(shell));

        drop(pty);

        let all: Vec<u32> = std::iter::once(shell).chain(children).collect();
        let mut alive = all.clone();
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            alive.retain(|&p| process_alive(p));
            if alive.is_empty() {
                break;
            }
        }
        for p in &alive {
            // no dejar basura aunque el test falle
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &p.to_string(), "/T", "/F"])
                .output();
        }
        assert!(
            alive.is_empty(),
            "procesos que sobreviven al cerrar el PTY: {alive:?} (de {all:?})"
        );
    }

    /// Cerrar una pestaña/panel (o la ventana) justo al abrirla, con PowerShell aún arrancando, tampoco debe
    /// dejar procesos huérfanos.
    #[test]
    fn test_dropping_pty_during_startup_leaves_no_orphans() {
        for shell in installed_shells() {
            let pids: Vec<u32> = (0..4)
                .map(|i| {
                    let pty = PtyManager::new(i, &shell, true, 100, 30, None, None).unwrap();
                    let pid = pty.child.process_id().expect("pid del shell");
                    drop(pty); // sin esperar al prompt
                    pid
                })
                .collect();

            let mut alive = pids.clone();
            for _ in 0..60 {
                alive.retain(|&p| process_alive(p));
                if alive.is_empty() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            for p in &alive {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &p.to_string(), "/T", "/F"])
                    .output();
            }
            assert!(
                alive.is_empty(),
                "{shell}: shells huérfanos tras soltar el PTY al arrancar: {alive:?}"
            );
        }
    }
}
