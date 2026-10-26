//! Vías lentas por SSH (sin integración en banda, o cuando esta falla): abrir y guardar archivos remotos, listar
//! carpetas del explorador y subir capturas. Cada una corre en su propio hilo (`ssh.exe` tarda hasta varios
//! segundos en conectar) y avisa por `TearminalEvent::SshDone`; el bucle de eventos nunca espera a la red.

use super::*;
use crate::explorer::RemoteLsReq;
use crate::ssh;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct SshOpenDone {
    pub(super) dest: String,
    pub(super) path: PathBuf,
    pub(super) pin: bool,
    pub(super) data: Result<Vec<u8>, String>,
}

#[derive(Debug)]
pub(super) struct SshListingDone {
    pub(super) req: RemoteLsReq,
    pub(super) result: Result<ssh::Output, String>,
}

#[derive(Debug)]
pub(super) struct SshSaveDone {
    pub(super) dest: String,
    pub(super) path: PathBuf,
    /// dit_gen del visor al enviar: si cambió, lo guardado ya no es lo que se ve
    pub(super) edit_gen: u64,
    pub(super) result: Result<(), String>,
}

#[derive(Debug)]
pub(super) struct SshGitStatusDone {
    pub(super) repo_root: PathBuf,
    pub(super) statuses: rustc_hash::FxHashMap<PathBuf, crate::explorer::GitFileStatus>,
}

/// Resultado de un trabajo SSH.
///
/// Optimizado con Box en las variantes más pesadas para eliminar el **Enum Bloat**
/// y permitir que SshDone quepa en una única línea de caché L1/L2 (64 bytes).
pub(super) enum SshDone {
    Open(Box<SshOpenDone>),
    Listing(Box<SshListingDone>),
    Save(Box<SshSaveDone>),
    Upload {
        pane_id: PaneId,
        local_path: String,
        remote_path: Option<String>,
    },
    AiLocal(Box<super::ai_usage::LocalUsageScan>),
    GitStatus(Box<SshGitStatusDone>),
    GitHead {
        path: PathBuf,
        lines: Option<Vec<String>>,
    },
}

fn remote_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl TearminalApp {
    fn spawn_ssh(&self, work: impl FnOnce() -> SshDone + Send + 'static) -> bool {
        let (tx, proxy) = (self.ssh_tx.clone(), self.proxy.clone());
        let spawned = std::thread::Builder::new()
            .name("tearminal-ssh".into())
            .spawn(move || {
                let _ = tx.send(work());
                if let Some(p) = proxy {
                    let _ = p.send_event(TearminalEvent::SshDone);
                }
            });
        match spawned {
            Ok(_) => true,
            Err(e) => {
                crate::logger::log(&format!("No se pudo lanzar el hilo SSH: {e}"));
                false
            }
        }
    }

    /// Escanea en un hilo en segundo plano las transcripciones locales de IA sin bloquear el bucle de eventos
    pub(super) fn spawn_ai_local_scan(&mut self, agents: Vec<AiAgentStatus>) {
        let fetch_limits = self.config.terminal.ai_limits && self.ai_usage.limits_due(&agents);
        if !self.spawn_ssh(move || {
            SshDone::AiLocal(Box::new(super::ai_usage::scan_local_usage_blocking(
                &agents,
                fetch_limits,
            )))
        }) {
            self.ai_usage.local_scan_in_flight = false;
        }
    }

    /// Obtiene `git show HEAD:<path>` en un hilo en segundo plano para el visor de archivos local
    pub(super) fn spawn_git_head_lines(&self, path: PathBuf) {
        self.spawn_ssh(move || {
            let lines = crate::explorer::git_head_lines(&path);
            SshDone::GitHead { path, lines }
        });
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
            SshDone::Open(Box::new(SshOpenDone {
                dest,
                path,
                pin,
                data,
            }))
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
            let cmd = ssh::write_file_cmd(&remote_path_string(&path), content.len());
            let result = ssh::run(&dest, &cmd, 5, Some(content.as_bytes()), 4096).and_then(|o| {
                if o.ok && !o.truncated {
                    Ok(())
                } else {
                    Err(format!(
                        "Error al guardar archivo remoto vía SSH: {}",
                        if o.truncated {
                            "la salida del comando remoto excedió el límite"
                        } else {
                            &o.stderr
                        }
                    ))
                }
            });
            SshDone::Save(Box::new(SshSaveDone {
                dest,
                path,
                edit_gen,
                result,
            }))
        });
    }

    /// Sube una captura al remoto por `ssh`; al terminar se pega su ruta (o la local si falló) en el panel
    pub(super) fn ssh_upload_image(&self, pane_id: PaneId, local_path: String, dest: &str) {
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

    /// Lanza los listados y consultas git que el explorador pidió (llamar tras cada ronda de eventos)
    pub(super) fn pump_explorer_ssh(&mut self) {
        for req in self.file_explorer.take_ssh_requests() {
            self.spawn_ssh(move || {
                let cmd = match req.expand {
                    None => ssh::root_listing_cmd(&req.dir),
                    Some(_) => ssh::dir_listing_cmd(&req.dir),
                };
                let result = ssh::run(&req.dest, &cmd, 4, None, ssh::MAX_LISTING_BYTES);
                SshDone::Listing(Box::new(SshListingDone { req, result }))
            });
        }
        if let Some((repo_root, changes_only)) = self.file_explorer.take_git_status_request() {
            let spawned = self.spawn_ssh(move || {
                let statuses = crate::explorer::collect_git_statuses(&repo_root, changes_only);
                SshDone::GitStatus(Box::new(SshGitStatusDone {
                    repo_root,
                    statuses,
                }))
            });
            if !spawned {
                self.file_explorer.cancel_git_status();
            }
        }
    }

    /// Aplica los resultados de los hilos SSH que ya terminaron
    pub(super) fn process_ssh_results(&mut self) {
        while let Ok(done) = self.ssh_rx.try_recv() {
            match done {
                SshDone::Open(open) => {
                    let SshOpenDone {
                        dest,
                        path,
                        pin,
                        data,
                    } = *open;
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
                SshDone::Listing(listing) => {
                    let SshListingDone { req, result } = *listing;
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
                SshDone::Save(save) => {
                    let SshSaveDone {
                        dest,
                        path,
                        edit_gen,
                        result,
                    } = *save;
                    match result {
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
                        Err(e) => {
                            crate::logger::log(&format!("-> Error al guardar: {e}"));
                            if let Some(v) = self
                                .tabs
                                .iter_mut()
                                .filter_map(|t| t.file_viewer.as_mut())
                                .find(|v| {
                                    v.path == path && v.remote_destination.as_deref() == Some(&dest)
                                })
                            {
                                v.save_error = Some(e);
                            }
                        }
                    }
                }
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
                SshDone::AiLocal(scan) => {
                    self.ai_usage.apply_local_scan(*scan);
                }
                SshDone::GitStatus(git) => {
                    let SshGitStatusDone {
                        repo_root,
                        statuses,
                    } = *git;
                    self.file_explorer.apply_git_statuses(&repo_root, statuses);
                    self.sync_fs_watcher();
                    // Si llegaron cambios durante la consulta, queda una petición pendiente
                    self.pump_explorer_ssh();
                }
                SshDone::GitHead { path, lines } => {
                    for tab in &mut self.tabs {
                        if let Some(viewer) = tab.file_viewer.as_mut() {
                            if viewer.path == path && viewer.remote_destination.is_none() {
                                viewer.git_base = lines.clone();
                                viewer.edit_gen = viewer.edit_gen.wrapping_add(1);
                            }
                        }
                    }
                }
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
