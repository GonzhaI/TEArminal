//! Observador del sistema de archivos nativo de Windows (ReadDirectoryChangesW)
//! Notifica cambios en tiempo real en el árbol de archivos o metadatos de Git con 0% CPU en reposo.

use super::TearminalEvent;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

pub(super) struct FsWatcher {
    watched_path: PathBuf,
    stop_flag: Arc<super::CachePadded<AtomicBool>>,
    _thread: Option<JoinHandle<()>>,
}

impl FsWatcher {
    pub(super) fn new(path: &Path, proxy: EventLoopProxy<TearminalEvent>) -> Option<Self> {
        if !path.exists() {
            return None;
        }

        let watched_path = path.to_path_buf();
        let stop_flag = Arc::new(super::CachePadded::new(AtomicBool::new(false)));
        let stop_for_thread = Arc::clone(&stop_flag);
        let path_for_thread = watched_path.clone();

        #[cfg(target_os = "windows")]
        let thread = std::thread::Builder::new()
            .name("tearminal-fs-watcher".into())
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

    pub(super) fn watched_path(&self) -> &Path {
        &self.watched_path
    }
}

impl Drop for FsWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_os = "windows")]
fn run_watcher_windows(
    dir: PathBuf,
    stop: Arc<super::CachePadded<AtomicBool>>,
    proxy: EventLoopProxy<TearminalEvent>,
) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{
        CloseHandle, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED,
        FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
        FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
    use windows_sys::Win32::System::IO::{CancelIoEx, OVERLAPPED};

    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();

    // SAFETY: abrir directorio con permisos de listado y semántica de backup (requerido para carpetas)
    let h_dir = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            0,
        )
    };

    if h_dir == 0 || h_dir == INVALID_HANDLE_VALUE {
        return;
    }

    let change_event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    if change_event == 0 {
        unsafe { CloseHandle(h_dir) };
        return;
    }

    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_SIZE;

    let mut buffer = [0u8; 4096];

    while !stop.load(Ordering::Relaxed) {
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = change_event;

        unsafe { ResetEvent(change_event) };

        let ok = unsafe {
            ReadDirectoryChangesW(
                h_dir,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
                1, // Observar todo el subárbol del proyecto o repo
                filter,
                std::ptr::null_mut(),
                &mut overlapped,
                None,
            )
        };

        if ok == 0 {
            break;
        }

        loop {
            if stop.load(Ordering::Relaxed) {
                unsafe { CancelIoEx(h_dir, &overlapped) };
                break;
            }

            let res = unsafe { WaitForSingleObject(change_event, 500) };
            if res == WAIT_OBJECT_0 {
                // Debounce de 500 ms para unificar ráfagas de escritura rápida (git commit, compilación, etc.)
                std::thread::sleep(Duration::from_millis(500));

                if !stop.load(Ordering::Relaxed) {
                    let _ = proxy.send_event(TearminalEvent::FsChanged);
                }

                // Límite de frecuencia: como mucho un aviso cada ~1.5s durante escrituras masivas
                for _ in 0..6 {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
                break;
            } else if res != WAIT_TIMEOUT {
                unsafe { CancelIoEx(h_dir, &overlapped) };
                break;
            }
        }
    }

    unsafe {
        CloseHandle(change_event);
        CloseHandle(h_dir);
    }
}

impl super::TearminalApp {
    /// Sincroniza el observador de cambios en tiempo real según el repositorio o carpeta activa del explorador
    pub(crate) fn sync_fs_watcher(&mut self) {
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
