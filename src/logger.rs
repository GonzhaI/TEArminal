use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

static LOG_FILE: Mutex<Option<BufWriter<File>>> = Mutex::new(None);

/// Indica si el registro en `axon.log` está activo (cacheado una sola vez por proceso).
#[inline]
pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        cfg!(debug_assertions)
            || std::env::var_os("AXON_DEBUG").is_some()
            || std::env::var_os("AXON_LOG").is_some()
    })
}

/// Traza de teclas (`AXON_LOG_KEYS=1`). Es opt-in y independiente de `AXON_DEBUG`/`AXON_LOG`: registrar
/// pulsaciones puede capturar contraseñas tecleadas en un prompt, así que nunca está activa por defecto
/// ni en compilaciones de depuración.
#[inline]
pub(crate) fn keys_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("AXON_LOG_KEYS").is_some())
}

#[inline]
pub(crate) fn log(msg: &str) {
    if !enabled() {
        return;
    }
    log_impl(msg);
}

#[cold]
fn log_impl(msg: &str) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{}] {}\n", now, msg);

    #[cfg(debug_assertions)]
    eprint!("{}", line);

    if let Ok(mut guard) = LOG_FILE.lock() {
        if guard.is_none() {
            let dir = crate::config::dirs_or_fallback();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("axon.log");
            *guard = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .or_else(|_| {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(std::env::temp_dir().join("axon.log"))
                })
                .ok()
                .map(BufWriter::new);
        }
        if let Some(writer) = guard.as_mut() {
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }
    }
}
