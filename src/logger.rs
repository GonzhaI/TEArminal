use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

static LOG_MUTEX: Mutex<()> = Mutex::new(());

pub fn log(msg: &str) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{}] {}\n", now, msg);

    // Imprimir en stderr
    eprint!("{}", line);

    // Guardar en axon.log en el directorio local
    if let Ok(_lock) = LOG_MUTEX.lock() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("axon.log") {
            let _ = file.write_all(line.as_bytes());
        }
    }
}
