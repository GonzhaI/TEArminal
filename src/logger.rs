use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

static LOG_MUTEX: Mutex<()> = Mutex::new(());

pub fn log(msg: &str) {
    // En producción (release), NO registrar en axon.log a menos que se active explícitamente con AXON_DEBUG=1 o AXON_LOG=1
    #[cfg(not(debug_assertions))]
    {
        if std::env::var("AXON_DEBUG").is_err() && std::env::var("AXON_LOG").is_err() {
            return;
        }
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{}] {}\n", now, msg);

    #[cfg(debug_assertions)]
    eprint!("{}", line);

    // Guardar en axon.log en el directorio local
    if let Ok(_lock) = LOG_MUTEX.lock() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("axon.log") {
            let _ = file.write_all(line.as_bytes());
        }
    }
}
