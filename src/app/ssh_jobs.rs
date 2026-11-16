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
    pub(super) statuses: rustc_hash::FxHashMap<PathBuf, crate::explorer::GitStatusInfo>,
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
        is_new_file: bool,
    },
    GitAction {
        path: PathBuf,
        is_discard: bool,
    },
    GitCommitResult(Result<(), String>),
    GitPushResult(Result<(), String>),
    GitBulkActionDone,
    GitAheadBehind(Option<(usize, usize)>),
    GitCheckoutResult(Result<String, String>),
    FolderActionDone(Result<String, String>),
    RemoteMetrics(Box<crate::monitor::RemoteSystemSnapshot>),
    DbDone(Box<super::db_jobs::DbJobResult>),
}

fn remote_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl TearminalApp {
    pub(super) fn spawn_ssh(&self, work: impl FnOnce() -> SshDone + Send + 'static) -> bool {
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
            let res = crate::explorer::git_head_result(&path);
            let (lines, is_new_file) = match res {
                crate::explorer::GitHeadResult::Lines(lines) => (Some(lines), false),
                crate::explorer::GitHeadResult::NewFile => (None, true),
                crate::explorer::GitHeadResult::None => (None, false),
            };
            SshDone::GitHead {
                path,
                lines,
                is_new_file,
            }
        });
    }

    /// Ejecuta en un hilo en segundo plano una acción de Git (Stage, Unstage, Discard) sin bloquear la UI
    pub(super) fn spawn_git_action(
        &self,
        repo_root: PathBuf,
        path: PathBuf,
        action: crate::renderer::GitActionKind,
        is_untracked: bool,
    ) {
        self.spawn_ssh(move || {
            match action {
                crate::renderer::GitActionKind::Stage => {
                    let _ = crate::explorer::git_stage_file(&repo_root, &path);
                }
                crate::renderer::GitActionKind::Unstage => {
                    let _ = crate::explorer::git_unstage_file(&repo_root, &path);
                }
                crate::renderer::GitActionKind::Discard => {
                    let _ = crate::explorer::git_discard_file(&repo_root, &path, is_untracked);
                }
            }
            SshDone::GitAction {
                path,
                is_discard: action == crate::renderer::GitActionKind::Discard,
            }
        });
    }

    /// Ejecuta un commit en un hilo en segundo plano sin congelar la UI
    pub(super) fn spawn_git_commit(&mut self, repo: PathBuf, message: String, amend: bool) {
        self.file_explorer.git_commit.is_committing = true;
        self.spawn_ssh(move || {
            let res = crate::explorer::git_commit(&repo, &message, amend);
            SshDone::GitCommitResult(res)
        });
    }

    /// Ejecuta un push en un hilo en segundo plano sin congelar la UI
    pub(super) fn spawn_git_push(&mut self, repo: PathBuf) {
        self.file_explorer.git_commit.is_pushing = true;
        self.spawn_ssh(move || {
            let res = crate::explorer::git_push(&repo);
            SshDone::GitPushResult(res)
        });
    }

    /// Ejecuta una acción grupal de Git (Preparar todo, Quitar todo, Descartar todo) en segundo plano
    pub(super) fn spawn_git_bulk_action(
        &self,
        repo: PathBuf,
        action: crate::renderer::GitBulkActionKind,
    ) {
        self.spawn_ssh(move || {
            match action {
                crate::renderer::GitBulkActionKind::StageAll => {
                    let _ = crate::explorer::git_stage_all(&repo);
                }
                crate::renderer::GitBulkActionKind::UnstageAll => {
                    let _ = crate::explorer::git_unstage_all(&repo);
                }
                crate::renderer::GitBulkActionKind::DiscardAll => {
                    let _ = crate::explorer::git_discard_all(&repo);
                }
            }
            SshDone::GitBulkActionDone
        });
    }

    /// Lanza la recolección periódica de métricas del sistema en un host remoto vía SSH
    pub(super) fn spawn_remote_system_metrics(&self, dest: String) {
        let host = dest.clone();
        self.spawn_ssh(move || {
            let cmd = "cat /proc/loadavg 2>/dev/null; head -n 8 /proc/meminfo 2>/dev/null; df -k / 2>/dev/null";
            let out_str = match ssh::SafeSshDestination::parse(&host) {
                Ok(safe_dest) => {
                    let conn = ssh::SshConnection::new(safe_dest).with_timeout(5);
                    match conn.authenticate(None) {
                        Ok(auth_conn) => match auth_conn.run_command(cmd, None, 32 * 1024) {
                            Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
                            Err(_) => String::new(),
                        },
                        Err(_) => String::new(),
                    }
                }
                Err(_) => String::new(),
            };
            let parsed = crate::monitor::parse_remote_metrics(&host, &out_str).unwrap_or_else(|| {
                crate::monitor::RemoteSystemSnapshot {
                    destination: host.clone(),
                    is_connected: false,
                    cpu_pct: 0.0,
                    ram_pct: 0.0,
                    disk_pct: 0.0,
                    load_1m: 0.0,
                    ram_total_bytes: 0,
                    ram_used_bytes: 0,
                    disk_total_bytes: 0,
                    disk_used_bytes: 0,
                }
            });
            SshDone::RemoteMetrics(Box::new(parsed))
        });
    }

    /// Sincroniza las máquinas remotas conocidas (espacios y ~/.ssh/config) en el monitor.
    pub(crate) fn sync_known_remote_sources(&mut self) {
        let mut known = Vec::new();
        for space in &self.spaces {
            if space.remote && !space.name.is_empty() && !known.contains(&space.name) {
                known.push(space.name.clone());
            }
        }
        for host in crate::app::machine::read_user_ssh_hosts() {
            if !host.alias.is_empty() && !known.contains(&host.alias) {
                known.push(host.alias);
            }
        }
        for dest in known {
            self.monitor
                .remote_snapshots
                .entry(dest.clone())
                .or_insert_with(|| crate::monitor::RemoteSystemSnapshot {
                    destination: dest,
                    is_connected: false,
                    cpu_pct: 0.0,
                    ram_pct: 0.0,
                    disk_pct: 0.0,
                    load_1m: 0.0,
                    ram_total_bytes: 0,
                    ram_used_bytes: 0,
                    disk_total_bytes: 0,
                    disk_used_bytes: 0,
                });
        }
    }

    /// Consulta commits por delante y por detrás del upstream en segundo plano
    pub(super) fn spawn_git_ahead_behind(&self, repo: PathBuf) {
        self.spawn_ssh(move || {
            let counts = crate::explorer::git_ahead_behind(&repo);
            SshDone::GitAheadBehind(counts)
        });
    }

    /// Cambia de rama Git en segundo plano
    pub(super) fn spawn_git_checkout(&self, repo: PathBuf, branch: String) {
        self.spawn_ssh(move || {
            let res = crate::explorer::git_checkout_branch(&repo, &branch).map(|()| branch);
            SshDone::GitCheckoutResult(res)
        });
    }

    /// Ejecuta la acción de limpieza de una carpeta de proyecto en segundo plano
    pub(super) fn spawn_folder_action(&self, root: PathBuf, folder_name: String, action: String) {
        self.spawn_ssh(move || {
            let res = crate::monitor::run_project_folder_action(&root, &folder_name, &action);
            SshDone::FolderActionDone(res)
        });
    }

    /// Lee un archivo remoto mediante SshConnection; la pestaña se abre al llegar
    pub(super) fn ssh_open_file(&self, dest: &str, path: &Path, pin: bool) {
        let (dest, path) = (dest.to_string(), path.to_path_buf());
        let (ident, port) = self
            .file_explorer
            .remote_session
            .as_ref()
            .filter(|s| s.destination == dest)
            .map(|s| (s.identity_file.clone(), s.port))
            .unwrap_or((None, None));
        self.spawn_ssh(move || {
            let data = match ssh::SafeSshDestination::parse(&dest) {
                Ok(safe_dest) => {
                    let conn = ssh::SshConnection::new(safe_dest)
                        .with_identity_file(ident)
                        .with_port(port)
                        .with_timeout(5);
                    match conn.authenticate(None) {
                        Ok(auth_conn) => auth_conn.read_file(&remote_path_string(&path)),
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(format!("destino SSH no válido: {e}")),
            };
            SshDone::Open(Box::new(SshOpenDone {
                dest,
                path,
                pin,
                data,
            }))
        });
    }

    /// Guarda el visor remoto (`dest`, `path`) mediante SshConnection; se marca guardado al confirmarse
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
        let (ident, port) = self
            .file_explorer
            .remote_session
            .as_ref()
            .filter(|s| s.destination == dest)
            .map(|s| (s.identity_file.clone(), s.port))
            .unwrap_or((None, None));
        self.spawn_ssh(move || {
            let result = match ssh::SafeSshDestination::parse(&dest) {
                Ok(safe_dest) => {
                    let conn = ssh::SshConnection::new(safe_dest)
                        .with_identity_file(ident)
                        .with_port(port)
                        .with_timeout(5);
                    match conn.authenticate(None) {
                        Ok(auth_conn) => {
                            auth_conn.write_file(&remote_path_string(&path), content.as_bytes())
                        }
                        Err(e) => Err(format!("Error al autenticar conexión SSH: {e}")),
                    }
                }
                Err(e) => Err(format!("destino SSH no válido: {e}")),
            };
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
        let dest_str = dest.to_string();
        let (ident, port) = self
            .file_explorer
            .remote_session
            .as_ref()
            .filter(|s| s.destination == dest)
            .map(|s| (s.identity_file.clone(), s.port))
            .unwrap_or((None, None));
        self.spawn_ssh(move || {
            let remote_path = ssh::upload_image(&local_path, &dest_str, ident.as_deref(), port);
            SshDone::Upload {
                pane_id,
                local_path,
                remote_path,
            }
        });
    }

    /// Lanza en un hilo de fondo un listado remoto fuera de banda mediante `SshConnection`
    pub(super) fn spawn_ssh_listing(&self, req: RemoteLsReq) {
        self.spawn_ssh(move || {
            let result = match ssh::SafeSshDestination::parse(&req.dest) {
                Ok(safe_dest) => {
                    let conn = ssh::SshConnection::new(safe_dest)
                        .with_identity_file(req.identity_file.clone())
                        .with_port(req.port)
                        .with_timeout(4);
                    match conn.authenticate(None) {
                        Ok(auth_conn) => match req.expand {
                            None => auth_conn.list_root(&req.dir),
                            Some(_) => auth_conn.list_dir(&req.dir),
                        },
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(format!("destino SSH no válido: {e}")),
            };
            SshDone::Listing(Box::new(SshListingDone { req, result }))
        });
    }

    /// Lanza los listados y consultas git que el explorador pidió (llamar tras cada ronda de eventos)
    pub(super) fn pump_explorer_ssh(&mut self) {
        for req in self.file_explorer.take_ssh_requests() {
            if req.expand.is_some() && self.remote_list(&req) {
                continue;
            }
            self.spawn_ssh_listing(req);
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
                    if self.file_explorer.remote_expand_all {
                        match self.file_explorer.next_remote_dir_to_expand() {
                            Some(idx) => {
                                self.file_explorer.toggle_expand(idx);
                                self.pump_explorer_ssh();
                            }
                            None => self.file_explorer.remote_expand_all = false,
                        }
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
                    self.file_explorer.git_branch =
                        crate::app::shell_context::get_git_branch(Some(&repo_root));
                    self.spawn_git_ahead_behind(repo_root);
                    self.sync_fs_watcher();
                    // Si llegaron cambios durante la consulta, queda una petición pendiente
                    self.pump_explorer_ssh();
                }
                SshDone::GitHead {
                    path,
                    lines,
                    is_new_file,
                } => {
                    for tab in &mut self.tabs {
                        if let Some(viewer) = tab.file_viewer.as_mut() {
                            if viewer.path == path && viewer.remote_destination.is_none() {
                                viewer.git_base = lines.clone();
                                viewer.git_new_file = is_new_file;
                                viewer.edit_gen = viewer.edit_gen.wrapping_add(1);
                            }
                        }
                    }
                }
                SshDone::GitAction { path, is_discard } => {
                    self.file_explorer.refresh_git_status();
                    self.sync_viewer_on_git_action(&path, is_discard);
                    self.pump_explorer_ssh();
                }
                SshDone::GitCommitResult(res) => {
                    self.file_explorer.git_commit.is_committing = false;
                    match res {
                        Ok(()) => {
                            self.file_explorer.git_commit.input.text.clear();
                            self.file_explorer.git_commit.input.cursor = 0;
                            self.file_explorer.git_commit.amend = false;
                            self.file_explorer.git_commit.status_message =
                                Some(("Commit realizado con éxito".into(), false));
                            self.file_explorer.refresh_git_status();
                            if let Some(repo) = self.file_explorer.git_root.clone() {
                                self.spawn_git_ahead_behind(repo);
                            }
                        }
                        Err(e) => {
                            self.file_explorer.git_commit.status_message = Some((e, true));
                        }
                    }
                }
                SshDone::GitPushResult(res) => {
                    self.file_explorer.git_commit.is_pushing = false;
                    match res {
                        Ok(()) => {
                            self.file_explorer.git_commit.status_message =
                                Some(("Cambios subidos al remoto".into(), false));
                            if let Some(repo) = self.file_explorer.git_root.clone() {
                                self.spawn_git_ahead_behind(repo);
                            }
                        }
                        Err(e) => {
                            self.file_explorer.git_commit.status_message = Some((e, true));
                        }
                    }
                }
                SshDone::GitBulkActionDone => {
                    self.file_explorer.refresh_git_status();
                    self.pump_explorer_ssh();
                }
                SshDone::GitAheadBehind(counts) => {
                    self.file_explorer.git_commit.ahead_behind = counts;
                }
                SshDone::GitCheckoutResult(res) => match res {
                    Ok(branch) => {
                        self.file_explorer.refresh_git_status();
                        if let Some(repo) = self.file_explorer.git_root.clone() {
                            self.spawn_git_ahead_behind(repo);
                        }
                        self.show_toast(
                            crate::app::dialog::ToastSeverity::Success,
                            format!("Rama cambiada a «{branch}»"),
                        );
                    }
                    Err(e) => {
                        self.show_toast(crate::app::dialog::ToastSeverity::Error, e);
                    }
                },
                SshDone::FolderActionDone(res) => match res {
                    Ok(msg) => {
                        self.show_toast(crate::app::dialog::ToastSeverity::Success, msg);
                        self.request_immediate_monitor_sample();
                    }
                    Err(e) => {
                        self.show_toast(crate::app::dialog::ToastSeverity::Error, e);
                    }
                },
                SshDone::RemoteMetrics(snap) => {
                    self.monitor
                        .remote_snapshots
                        .insert(snap.destination.clone(), *snap);
                }
                SshDone::DbDone(res) => {
                    self.apply_db_job_result(*res);
                }
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Abre (o enfoca) la pestaña de visor de un archivo remoto con el contenido ya leído
    pub(super) fn push_remote_viewer(
        &mut self,
        dest: &str,
        path: &Path,
        data: Result<Vec<u8>, String>,
        pin: bool,
    ) {
        let branch = self
            .tabs
            .get_mut(self.active_tab_index)
            .and_then(|t| t.active_pane_mut())
            .and_then(|p| p.get_git_branch());
        let tab_id = self.alloc_tab_id();
        let tab = TerminalTab::new_remote_file_viewer_from(
            tab_id,
            dest.to_string(),
            path.to_path_buf(),
            branch,
            data,
        );
        self.install_viewer_tab(tab, pin);
    }
}
