use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct PtyManager {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
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
        let _child = pair.slave.spawn_command(cmd)?;

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
