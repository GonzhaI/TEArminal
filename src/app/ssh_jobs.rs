//! Vías lentas por SSH (sin integración en banda, o cuando esta falla): abrir y guardar archivos remotos, listar
//! carpetas del explorador y subir capturas. Cada una corre en su propio hilo (`ssh.exe` tarda hasta varios
//! segundos en conectar) y avisa por `AxonEvent::SshDone`; el bucle de eventos nunca espera a la red.

use super::*;
use crate::explorer::RemoteLsReq;
use crate::ssh;
use std::path::{Path, PathBuf};

/// Resultado de un trabajo SSH, ya fuera del hilo que lo ejecutó
pub(super) enum SshDone {
    Open {
        dest: String,
        path: PathBuf,
        pin: bool,
        data: Result<Vec<u8>, String>,
    },
    Listing {
        req: RemoteLsReq,
        result: Result<ssh::Output, String>,
    },
    Save {
        dest: String,
        path: PathBuf,
        /// `edit_gen` del visor al enviar: si cambió, lo guardado ya no es lo que se ve
        edit_gen: u64,
        result: Result<(), String>,
    },
    Upload {
        pane_id: usize,
        local_path: String,
        remote_path: Option<String>,
    },
}

fn remote_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl AxonApp {
    fn spawn_ssh(&self, work: impl FnOnce() -> SshDone + Send + 'static) {
        let (tx, proxy) = (self.ssh_tx.clone(), self.proxy.clone());
        let spawned = std::thread::Builder::new()
            .name("axon-ssh".into())
            .spawn(move || {
                let _ = tx.send(work());
                if let Some(p) = proxy {
                    let _ = p.send_event(AxonEvent::SshDone);
                }
            });
        if let Err(e) = spawned {
            crate::logger::log(&format!("No se pudo lanzar el hilo SSH: {e}"));
        }
    }

    /// Lee un archivo remoto con una segunda conexión SSH (solo con clave); la pestaña se abre al llegar
    pub(super) fn ssh_open_file(&self, dest: &str, path: &Path, pin: bool) {
        let (dest, path) = (dest.to_string(), path.to_path_buf());
        self.spawn_ssh(move || {
            let cmd = ssh::read_file_cmd(&remote_path_string(&path));
            let data = ssh::run(&dest, &cmd, 5, None, ssh::MAX_FILE_BYTES + 1).and_then(|o| {
                if o.ok {
                    Ok(o.stdout)
                } else {
                    Err(o.stderr)
                }
            });
            SshDone::Open {
                dest,
                path,
                pin,
                data,
            }
        });
    }

    /// Guarda el visor remoto (`dest`, `path`) con una segunda conexión SSH; se marca guardado al confirmarse
    pub(super) fn ssh_save_viewer(&mut self, dest: &str, path: &Path) {
        let Some(viewer) = self
            .tabs
            .iter_mut()
            .filter_map(|t| t.file_viewer.as_mut())
            .find(|v| v.path == path && v.remote_destination.as_deref() == Some(dest))
        else {
            return;
        };
        let content = viewer.remote_content();
        let edit_gen = viewer.edit_gen;
        let (dest, path) = (dest.to_string(), path.to_path_buf());
        self.spawn_ssh(move || {
            let cmd = ssh::write_file_cmd(&remote_path_string(&path));
            let result = ssh::run(&dest, &cmd, 5, Some(content.as_bytes()), 4096).and_then(|o| {
                if o.ok {
                    Ok(())
                } else {
                    Err(format!(
                        "Error al guardar archivo remoto vía SSH: {}",
                        o.stderr
                    ))
                }
            });
            SshDone::Save {
                dest,
                path,
                edit_gen,
                result,
            }
        });
    }

    /// Sube una captura al remoto con `scp`; al terminar se pega su ruta (o la local si falló) en el panel
    pub(super) fn ssh_upload_image(&self, pane_id: usize, local_path: String, dest: &str) {
        let dest = dest.to_string();
        self.spawn_ssh(move || {
            let remote_path = ssh::upload_image(&local_path, &dest);
            SshDone::Upload {
                pane_id,
                local_path,
                remote_path,
            }
        });
    }

    /// Lanza los listados que el explorador pidió desde cualquier clic (llamar tras cada ronda de eventos)
    pub(super) fn pump_explorer_ssh(&mut self) {
        for req in self.file_explorer.take_ssh_requests() {
            self.spawn_ssh(move || {
                let cmd = match req.expand {
                    None => ssh::root_listing_cmd(&req.dir),
                    Some(_) => ssh::dir_listing_cmd(&req.dir),
                };
                let result = ssh::run(&req.dest, &cmd, 4, None, ssh::MAX_LISTING_BYTES);
                SshDone::Listing { req, result }
            });
        }
    }

    /// Aplica los resultados de los hilos SSH que ya terminaron
    pub(super) fn process_ssh_results(&mut self) {
        while let Ok(done) = self.ssh_rx.try_recv() {
            match done {
                SshDone::Open {
                    dest,
                    path,
                    pin,
                    data,
                } => {
                    // Un doble clic lanza dos lecturas: la segunda respuesta solo fija la pestaña ya abierta
                    let open = self.tabs.iter().position(|t| {
                        t.file_viewer.as_ref().is_some_and(|v| {
                            v.path == path && v.remote_destination.as_deref() == Some(dest.as_str())
                        })
                    });
                    match open {
                        Some(pos) => self.tabs[pos].is_preview &= !pin,
                        None => self.push_remote_viewer(&dest, &path, data, pin),
                    }
                }
                SshDone::Listing { req, result } => {
                    let branch = self.file_explorer.apply_remote_listing(&req, result);
                    // La rama del listado raíz la muestra la barra de estado del panel activo de ese servidor
                    let session = self
                        .tabs
                        .get_mut(self.active_tab_index)
                        .and_then(|t| t.active_pane_mut())
                        .and_then(|p| p.terminal.ssh_session.as_mut())
                        .filter(|s| s.destination == req.dest);
                    if let (Some(session), Some(branch)) = (session, branch) {
                        session.remote_git_branch = Some(branch);
                    }
                }
                SshDone::Save {
                    dest,
                    path,
                    edit_gen,
                    result,
                } => match result {
                    Ok(()) => {
                        let viewer = self
                            .tabs
                            .iter_mut()
                            .filter_map(|t| t.file_viewer.as_mut())
                            .find(|v| {
                                v.path == path && v.remote_destination.as_deref() == Some(&dest)
                            });
                        match viewer {
                            Some(v) if v.edit_gen == edit_gen => v.mark_remote_saved(),
                            _ => crate::logger::log(
                                "-> Guardado remoto: el archivo cambió mientras se guardaba, sigue sin guardar",
                            ),
                        }
                    }
                    Err(e) => crate::logger::log(&format!("-> Error al guardar: {e}")),
                },
                SshDone::Upload {
                    pane_id,
                    local_path,
                    remote_path,
                } => {
                    let text = format!("\"{}\" ", remote_path.unwrap_or(local_path));
                    if let Some(pane) = self.tabs.iter_mut().find_map(|t| t.find_pane_mut(pane_id))
                    {
                        Self::paste_into_pane(pane, &text);
                    }
                }
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
