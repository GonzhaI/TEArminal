//! Observador del sistema de archivos nativo de Windows (FindFirstChangeNotificationW)
//! Notifica cambios en tiempo real en el árbol de archivos o metadatos de Git con 0% CPU en reposo.

use super::AxonEvent;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

pub struct FsWatcher {
    watched_path: PathBuf,
    stop_flag: Arc<AtomicBool>,
    _thread: Option<JoinHandle<()>>,
}

impl FsWatcher {
    pub fn new(path: &Path, proxy: EventLoopProxy<AxonEvent>) -> Option<Self> {
        if !path.exists() {
            return None;
        }

        let watched_path = path.to_path_buf();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop_flag);
        let path_for_thread = watched_path.clone();

        #[cfg(target_os = "windows")]
        let thread = std::thread::Builder::new()
            .name("axon-fs-watcher".into())
            .spawn(move || {
                run_watcher_windows(path_for_thread, stop_for_thread, proxy);
            })
            .ok();

        #[cfg(not(target_os = "windows"))]
        let thread = None;

        Some(Self {
            watched_path,
            stop_flag,
            _thread: thread,
        })
    }

    pub fn watched_path(&self) -> &Path {
        &self.watched_path
    }
}

impl Drop for FsWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_os = "windows")]
fn run_watcher_windows(dir: PathBuf, stop: Arc<AtomicBool>, proxy: EventLoopProxy<AxonEvent>) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::Storage::FileSystem::{
        FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
        FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
        FILE_NOTIFY_CHANGE_SIZE,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    let mut wide: Vec<u16> = dir.as_os_str().encode_wide().collect();
    wide.push(0);

    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_SIZE;

    // SAFETY: cadena wide válida y terminada en nulo
    let handle = unsafe { FindFirstChangeNotificationW(wide.as_ptr(), 1, filter) };
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return;
    }

    while !stop.load(Ordering::Relaxed) {
        // Espera de hasta 500 ms por evento del kernel o chequeo de parada
        let res = unsafe { WaitForSingleObject(handle, 500) };
        if res == WAIT_OBJECT_0 {
            // Debounce de 500 ms para unificar ráfagas de escritura rápida (git commit, guardado, compilación)
            std::thread::sleep(Duration::from_millis(500));

            unsafe {
                FindNextChangeNotification(handle);
                // Drenar señales adicionales acumuladas durante la pausa
                while WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 {
                    FindNextChangeNotification(handle);
                }
            }

            if !stop.load(Ordering::Relaxed) {
                let _ = proxy.send_event(AxonEvent::FsChanged);
            }

            // Como mucho un aviso cada ~2 s: una compilación (`target/`) escribe sin parar y cada aviso relee el
            // explorador y lanza `git status`. Lo que cambie durante la pausa queda pendiente en el manejador.
            for _ in 0..6 {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        } else if res != WAIT_TIMEOUT {
            // WAIT_FAILED (carpeta borrada, unidad retirada): sin esto la espera vuelve al instante y gira al 100 % de CPU
            break;
        }
    }

    unsafe {
        FindCloseChangeNotification(handle);
    }
}

impl super::AxonApp {
    /// Sincroniza el observador de cambios en tiempo real según el repositorio o carpeta activa del explorador
    pub fn sync_fs_watcher(&mut self) {
        if !self.file_explorer.visible || self.file_explorer.remote_session.is_some() {
            self.fs_watcher = None;
            return;
        }

        let target = self
            .file_explorer
            .git_root
            .as_deref()
            .unwrap_or(&self.file_explorer.root_dir);

        if self.fs_watcher.as_ref().map(|w| w.watched_path()) == Some(target) {
            return;
        }

        let Some(proxy) = self.proxy.clone() else {
            return;
        };
        self.fs_watcher = FsWatcher::new(target, proxy);
    }
}
