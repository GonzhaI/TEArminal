//! Operaciones de archivo y directorio sobre la propia sesión SSH del panel (`__tearminal_req` en
//! `init.ps1`, disparado por `\e[1337~` y respondido con `OSC 1337;TearminalFile=...`).
//!
//! No abre una segunda conexión `ssh.exe`, así que funciona también con servidores por contraseña,
//! 2FA o claves con passphrase. Si el shell remoto no está en el prompt o no responde a tiempo,
//! la aplicación cae de forma transparente a la vía fuera de banda (`SshConnection` en `ssh_jobs.rs`).

use super::*;
use crate::explorer::RemoteLsReq;
use std::path::{Path, PathBuf};

/// Tiempo máximo esperando respuesta en banda antes de caer al respaldo fuera de banda (`SshConnection`)
const INBAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Petición enviada al shell remoto que aún espera su `OSC 1337;TearminalFile=<id>`
pub(super) struct PendingRemote {
    id: String,
    pane_id: PaneId,
    sent: Instant,
    job: RemoteJob,
}

enum RemoteJob {
    Open {
        dest: String,
        path: PathBuf,
        pin: bool,
    },
    List(RemoteLsReq),
    Save {
        dest: String,
        path: PathBuf,
        /// `edit_gen` del visor en el momento de enviar: si el usuario tecleó mientras tanto, lo guardado
        /// en el servidor ya no es lo que hay en pantalla y `is_dirty` no se apaga
        edit_gen: u64,
    },
}

/// Codifica una ruta en `%XX` (solo ASCII seguro y sin espacios): un único token para `read -r` en bash
fn percent_encode_path(p: &Path) -> String {
    let mut s = String::new();
    for &b in p.to_string_lossy().replace('\\', "/").as_bytes() {
        let keep = b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-' | b'~');
        if keep {
            s.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(s, "%{b:02X}");
        }
    }
    s
}

/// Mensaje legible para el estado de una lectura remota (`get`)
fn remote_file_error(status: &str) -> String {
    match status {
        "enoent" => "El archivo no existe en el servidor remoto".into(),
        "eisdir" => "Es un directorio".into(),
        "eacces" => "Permiso denegado al leer el archivo remoto".into(),
        "overflow" => "La respuesta del servidor remoto superó el tamaño máximo".into(),
        _ => "No se pudo leer el archivo remoto".into(),
    }
}

impl TearminalApp {
    /// Panel con integración SSH en el prompt para `dest` (el de la pestaña activa, la última de terminal o cualquiera)
    fn remote_pane_for(&mut self, dest: &str) -> Option<&mut TerminalPane> {
        let ready = |p: &TerminalPane| {
            p.terminal.remote_integration
                && p.terminal.remote_at_prompt
                && p.terminal
                    .ssh_session
                    .as_ref()
                    .is_some_and(|s| s.is_active && s.destination == dest)
        };
        let active = self.active_tab_index;
        let last = self.last_terminal_tab_index;
        let idx = [active, last]
            .into_iter()
            .find(|&i| {
                self.tabs
                    .get(i)
                    .and_then(|t| t.active_pane())
                    .is_some_and(ready)
            })
            .or_else(|| self.tabs.iter().position(|t| t.panes.iter().any(ready)))?;
        let tab = &mut self.tabs[idx];
        if tab.active_pane().is_some_and(ready) {
            tab.active_pane_mut()
        } else {
            tab.panes.iter_mut().find(|p| ready(p))
        }
    }

    /// Si ya hay una lectura en vuelo de `(dest, path)` (primer clic de un doble clic), actualiza `pin`
    pub(super) fn pin_pending_remote_open(&mut self, dest: &str, path: &Path, pin: bool) -> bool {
        let pending = self
            .pending_remote
            .iter_mut()
            .find_map(|p| match &mut p.job {
                RemoteJob::Open {
                    dest: d,
                    path: p,
                    pin: existing_pin,
                } if d == dest && p == path => Some(existing_pin),
                _ => None,
            });
        if let Some(existing_pin) = pending {
            *existing_pin |= pin;
            true
        } else {
            false
        }
    }

    /// Pide un archivo remoto por el propio panel SSH; la pestaña del visor se abre al llegar la respuesta
    pub(super) fn remote_open(&mut self, dest: &str, path: &Path, pin: bool) -> bool {
        let Some(id) = crate::pty::random_id() else {
            return false;
        };
        let arg = percent_encode_path(path);
        let Some(pane) = self.remote_pane_for(dest) else {
            return false;
        };
        let pane_id = pane.id;
        pane.pty
            .write_input(&TerminalState::remote_request_bytes("get", &id, &arg));
        self.pending_remote.push(PendingRemote {
            id,
            pane_id,
            sent: Instant::now(),
            job: RemoteJob::Open {
                dest: dest.to_string(),
                path: path.to_path_buf(),
                pin,
            },
        });
        true
    }

    /// Pide el listado de una subcarpeta remota por el propio panel SSH
    pub(super) fn remote_list(&mut self, req: &RemoteLsReq) -> bool {
        let Some(ref dir) = req.expand else {
            return false;
        };
        let Some(id) = crate::pty::random_id() else {
            return false;
        };
        let arg = percent_encode_path(dir);
        let Some(pane) = self.remote_pane_for(&req.dest) else {
            return false;
        };
        let pane_id = pane.id;
        pane.pty
            .write_input(&TerminalState::remote_request_bytes("ls", &id, &arg));
        self.pending_remote.push(PendingRemote {
            id,
            pane_id,
            sent: Instant::now(),
            job: RemoteJob::List(req.clone()),
        });
        true
    }

    /// Guarda el archivo del visor activo por el propio panel SSH; `is_dirty` solo baja al confirmarse
    pub(super) fn remote_save_active_viewer(&mut self, dest: &str, path: &Path) -> bool {
        use base64::prelude::*;
        let Some(id) = crate::pty::random_id() else {
            return false;
        };
        let Some(viewer) = self
            .tabs
            .get_mut(self.active_tab_index)
            .and_then(|t| t.file_viewer.as_mut())
            .filter(|v| v.path == path && v.remote_destination.as_deref() == Some(dest))
        else {
            return false;
        };
        let content = viewer.remote_content();
        let edit_gen = viewer.edit_gen;
        let Some(pane) = self.remote_pane_for(dest) else {
            return false;
        };
        let pane_id = pane.id;
        let arg = format!("{} {}", percent_encode_path(path), content.len());
        let mut buf = TerminalState::remote_request_bytes("put", &id, &arg);
        // Líneas cortas para el `read -r` del remoto: un `write_input` grande de una pieza llenaría la línea de tty (4 KB)
        for chunk in BASE64_STANDARD
            .encode(content.as_bytes())
            .as_bytes()
            .chunks(2048)
        {
            buf.extend_from_slice(chunk);
            buf.push(b'\n');
        }
        buf.extend_from_slice(b".\n");
        pane.pty.write_input(&buf);
        self.pending_remote.push(PendingRemote {
            id,
            pane_id,
            sent: Instant::now(),
            job: RemoteJob::Save {
                dest: dest.to_string(),
                path: path.to_path_buf(),
                edit_gen,
            },
        });
        true
    }

    /// Aplica las respuestas `TearminalFile` que hayan llegado en cualquier panel
    pub(super) fn process_remote_results(&mut self) {
        let mut results = Vec::new();
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                for r in pane.terminal.remote_file_results.drain(..) {
                    results.push((pane.id, r));
                }
            }
        }
        for (pane_id, res) in results {
            let Some(pos) = self
                .pending_remote
                .iter()
                .position(|p| p.pane_id == pane_id && p.id == res.id)
            else {
                continue;
            };
            let pending = self.pending_remote.remove(pos);
            match pending.job {
                RemoteJob::Open { dest, path, pin } => {
                    let data = if res.status == "ok" {
                        Ok(res.data)
                    } else {
                        Err(remote_file_error(&res.status))
                    };
                    self.push_remote_viewer(&dest, &path, data, pin);
                }
                RemoteJob::List(req) => {
                    let Some(ref dir) = req.expand else {
                        continue;
                    };
                    if res.status == "ok" {
                        let text = String::from_utf8_lossy(&res.data);
                        self.file_explorer
                            .expand_remote_dir(dir, &crate::terminal::clean_listing(&text));
                        if self.file_explorer.remote_expand_all {
                            match self.file_explorer.next_remote_dir_to_expand() {
                                Some(idx) => {
                                    self.file_explorer.toggle_expand(idx);
                                    self.pump_explorer_ssh();
                                }
                                None => self.file_explorer.remote_expand_all = false,
                            }
                        }
                    } else {
                        self.file_explorer.set_expanded(dir, false);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                RemoteJob::Save {
                    dest,
                    path,
                    edit_gen,
                } => {
                    if res.status == "ok" {
                        let viewer = self
                            .tabs
                            .iter_mut()
                            .filter_map(|t| t.file_viewer.as_mut())
                            .find(|v| {
                                v.path == path && v.remote_destination.as_deref() == Some(&dest)
                            });
                        match viewer {
                            Some(v) if v.edit_gen == edit_gen => {
                                v.mark_remote_saved();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                            _ => crate::logger::log(
                                "-> Guardado remoto: el archivo cambió mientras se guardaba, sigue sin guardar",
                            ),
                        }
                    } else {
                        crate::logger::log(&format!(
                            "-> Error al guardar archivo remoto: {}",
                            path.display()
                        ));
                        if let Some(v) = self
                            .tabs
                            .iter_mut()
                            .filter_map(|t| t.file_viewer.as_mut())
                            .find(|v| {
                                v.path == path && v.remote_destination.as_deref() == Some(&dest)
                            })
                        {
                            v.save_error = Some("No se pudo guardar en el servidor remoto".into());
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Descarta las peticiones cuyo panel ya no existe (se cerró el panel o la pestaña)
    pub(super) fn prune_pending_remote(&mut self) {
        let tabs = &self.tabs;
        let explorer = &mut self.file_explorer;
        self.pending_remote.retain(|p| {
            let alive = tabs.iter().any(|t| t.find_pane(p.pane_id).is_some());
            if !alive {
                if let RemoteJob::List(ref req) = p.job {
                    if let Some(ref dir) = req.expand {
                        explorer.set_expanded(dir, false);
                    }
                }
            }
            alive
        });
    }

    /// Expira las peticiones en banda sin respuesta tras 3 s cayendo al respaldo fuera de banda (`SshConnection`);
    /// devuelve cuándo vence la más próxima que sigue en vuelo
    pub(super) fn expire_remote_requests(&mut self) -> Option<Instant> {
        let now = Instant::now();
        let mut expired = Vec::new();
        let mut i = 0;
        while i < self.pending_remote.len() {
            if now >= self.pending_remote[i].sent + INBAND_TIMEOUT {
                expired.push(self.pending_remote.remove(i));
            } else {
                i += 1;
            }
        }
        for p in expired {
            match p.job {
                RemoteJob::Open { dest, path, pin } => {
                    self.ssh_open_file(&dest, &path, pin);
                }
                RemoteJob::List(req) => {
                    self.spawn_ssh_listing(req);
                }
                RemoteJob::Save { dest, path, .. } => {
                    self.ssh_save_viewer(&dest, &path);
                }
            }
        }
        self.pending_remote
            .iter()
            .map(|p| p.sent + INBAND_TIMEOUT)
            .min()
    }
}
