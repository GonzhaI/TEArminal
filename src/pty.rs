use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct PtyManager {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    _child: Box<dyn Child + Send + Sync>,
    to_pty_tx: Sender<Vec<u8>>,
    from_pty_rx: Receiver<Vec<u8>>,
}

impl PtyManager {
    pub fn new(shell: &str, cols: u16, rows: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let cmd = CommandBuilder::new(shell);
        let child = pair.slave.spawn_command(cmd)?;

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        let (from_pty_tx, from_pty_rx) = channel::<Vec<u8>>();
        let (to_pty_tx, to_pty_rx) = channel::<Vec<u8>>();

        // Hilo lector del PTY (lee la salida del shell y la envía al terminal)
        thread::Builder::new()
            .name("axon-pty-reader".into())
            .spawn(move || {
                let mut buffer = [0u8; 8192];
                while let Ok(n) = reader.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    if from_pty_tx.send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
            })?;

        // Hilo escritor del PTY (toma inputs del teclado y los manda al shell)
        thread::Builder::new()
            .name("axon-pty-writer".into())
            .spawn(move || {
                while let Ok(data) = to_pty_rx.recv() {
                    if writer.write_all(&data).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
            })?;

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            _child: child,
            to_pty_tx,
            from_pty_rx,
        })
    }

    pub fn write_input(&self, data: &[u8]) {
        let _ = self.to_pty_tx.send(data.to_vec());
    }

    pub fn try_recv_output(&self) -> Option<Vec<u8>> {
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

    #[test]
    fn test_pty_powershell_spawn_and_read() {
        let pty = PtyManager::new("powershell.exe", 80, 24).unwrap();
        let mut total_output = String::new();
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                total_output.push_str(&s);
            }
        }
        println!("TOTAL PTY OUTPUT (len {}):\n{:?}", total_output.len(), total_output);

        // Enviar un comando "echo hello\r\n" a ver si responde
        pty.write_input(b"echo hello\r\n");
        let mut echo_output = String::new();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                echo_output.push_str(&s);
            }
            if echo_output.contains("hello") {
                break;
            }
        }
        println!("ECHO OUTPUT (len {}):\n{:?}", echo_output.len(), echo_output);
        assert!(echo_output.contains("hello"), "El shell debe responder al input del usuario");
    }
}
