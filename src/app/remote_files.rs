//! Peticiones en banda al bash remoto de una sesión SSH con integración: leer o guardar un archivo y
//! listar carpetas por la propia sesión (sin segunda conexión), así que funcionan también con contraseña.
//! El protocolo (`TerminalState::remote_request_bytes` y `AxonFile`) está documentado en `AGENTS.md`.

use super::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tamaño máximo de un archivo que se guarda en banda (el servidor lo recibe como una sola línea)
const MAX_SAVE_BYTES: usize = 96 * 1024;
/// Espera máxima de una respuesta antes de recurrir a una segunda conexión SSH (solo con clave)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Clone)]
pub(super) enum RemoteJob {
    Open {
        dest: String,
        path: PathBuf,
        pin: bool,
    },
    Save {
        dest: String,
        path: PathBuf,
        /// Lo que debe haber quedado escrito: `cksum` (CRC y tamaño) del contenido enviado
        expected: (u32, usize),
        /// `edit_gen` del visor al enviar: si cambió, lo guardado ya no es lo que se ve
        edit_gen: u64,
    },
    List {
        dest: String,
        dir: PathBuf,
    },
}

impl RemoteJob {
    fn dest(&self) -> &str {
        match self {
            Self::Open { dest, .. } | Self::Save { dest, .. } | Self::List { dest, .. } => dest,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingRemote {
    id: u32,
    /// Panel al que se envió la petición: solo sus respuestas la resuelven (los demás paneles SSH no son de fiar)
    pane_id: usize,
    job: RemoteJob,
    deadline: Instant,
}

/// `cksum` de POSIX (CRC-32 con polinomio 0x04C11DB7 sin reflejar, más los bytes de la longitud): el
/// gancho remoto lo calcula sobre lo que escribió y Axon lo compara con lo que envió.
fn posix_cksum(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    let mut feed = |byte: u8| {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    };
    data.iter().for_each(|&b| feed(b));
    let mut len = data.len();
    while len != 0 {
        feed((len & 0xFF) as u8);
        len >>= 8;
    }
    !crc
}

/// ¿La respuesta del gancho (`"<crc> <tamaño>"`) coincide con lo enviado? Si el usuario tecleó o llegó salida a
/// mitad de la petición, el servidor guardó otra cosa y respondió `ok` igualmente.
fn save_matches(reply: &[u8], expected: (u32, usize)) -> bool {
    let reply = String::from_utf8_lossy(reply);
    let mut it = reply.split_whitespace();
    let (Some(crc), Some(len), None) = (it.next(), it.next(), it.next()) else {
        return false;
    };
    crc.parse::<u32>().ok() == Some(expected.0) && len.parse::<usize>().ok() == Some(expected.1)
}

/// Codifica bytes para la línea de petición: solo `[A-Za-z0-9-_.~/]` van tal cual, el resto como `%XX`
pub(super) fn pct_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn remote_path_str(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl AxonApp {
    /// Envía una petición al panel con sesión `job.dest()`; `false` si no hay integración o el prompt no está libre
    fn remote_send(&mut self, job: RemoteJob, op: &str, payload: &str) -> bool {
        let id = self.next_remote_id;
        let dest = job.dest().to_string();
        let Some(pane) = self
            .tabs
            .iter_mut()
            .flat_map(|t| t.panes.iter_mut())
            .find(|p| {
                p.terminal.remote_integration
                    && p.terminal
                        .ssh_session
                        .as_ref()
                        .is_some_and(|s| s.is_active && s.destination == dest)
            })
        else {
            return false;
        };
        let Some(bytes) = pane.terminal.remote_request_bytes(op, id, payload) else {
            return false;
        };
        pane.pty.write_input(&bytes);
        let pane_id = pane.id;
        self.next_remote_id = id.wrapping_add(1);
        self.pending_remote.push(PendingRemote {
            id,
            pane_id,
            job,
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        true
    }

    /// Si ese archivo ya se pidió y sigue en camino, solo actualiza si debe quedar fijo (doble clic)
    pub(super) fn pin_pending_remote_open(&mut self, dest: &str, path: &Path, pin: bool) -> bool {
        for p in &mut self.pending_remote {
            if let RemoteJob::Open {
                dest: d,
                path: pth,
                pin: pn,
            } = &mut p.job
            {
                if d == dest && pth == path {
                    *pn |= pin;
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn remote_open(&mut self, dest: &str, path: &Path, pin: bool) -> bool {
        let payload = pct_encode(remote_path_str(path).as_bytes());
        let job = RemoteJob::Open {
            dest: dest.to_string(),
            path: path.to_path_buf(),
            pin,
        };
        self.remote_send(job, "get", &payload)
    }

    pub(super) fn remote_list(&mut self, dest: &str, dir: &Path) -> bool {
        let payload = pct_encode(remote_path_str(dir).as_bytes());
        let job = RemoteJob::List {
            dest: dest.to_string(),
            dir: dir.to_path_buf(),
        };
        self.remote_send(job, "ls", &payload)
    }

    /// Guarda el visor remoto activo por la sesión; `false` = hay que usar otra vía (archivo grande, sin integración)
    pub(super) fn remote_save_active_viewer(&mut self) -> bool {
        let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_mut() else {
            return false;
        };
        let Some(dest) = viewer.remote_destination.clone() else {
            return false;
        };
        let path = viewer.path.clone();
        let content = viewer.remote_content();
        if content.len() > MAX_SAVE_BYTES {
            return false;
        }
        let expected = (posix_cksum(content.as_bytes()), content.len());
        let edit_gen = viewer.edit_gen;
        let payload = format!(
            "{} {}",
            pct_encode(remote_path_str(&path).as_bytes()),
            pct_encode(content.as_bytes())
        );
        let job = RemoteJob::Save {
            dest,
            path,
            expected,
            edit_gen,
        };
        self.remote_send(job, "put", &payload)
    }

    /// Clic en una carpeta del explorador remoto: contrae, o pide su listado y la expande al llegar
    pub(super) fn remote_toggle_dir(&mut self, idx: usize) {
        let Some(item) = self.file_explorer.items.get(idx).cloned() else {
            return;
        };
        if item.is_expanded {
            self.file_explorer.toggle_expand(idx);
            return;
        }
        let Some(dest) = self
            .file_explorer
            .remote_session
            .as_ref()
            .map(|s| s.destination.clone())
        else {
            return;
        };
        if self.remote_list(&dest, &item.path) {
            self.file_explorer.set_expanded(&item.path, true);
        }
    }

    /// Siguiente carpeta pendiente de «Expandir todo» remoto (una petición cada vez)
    pub(super) fn remote_expand_next(&mut self) {
        let next = self.file_explorer.next_remote_dir_to_expand();
        let dest = self
            .file_explorer
            .remote_session
            .as_ref()
            .map(|s| s.destination.clone());
        let (Some(idx), Some(dest)) = (next, dest) else {
            self.file_explorer.remote_expand_all = false;
            return;
        };
        let path = self.file_explorer.items[idx].path.clone();
        if self.remote_list(&dest, &path) {
            self.file_explorer.set_expanded(&path, true);
        } else {
            self.file_explorer.remote_expand_all = false;
        }
    }

    /// Recoge las respuestas llegadas por los terminales y las aplica
    pub(super) fn process_remote_results(&mut self) {
        let mut results = Vec::new();
        for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
            let pane_id = pane.id;
            results.extend(
                pane.terminal
                    .remote_file_results
                    .drain(..)
                    .map(|r| (pane_id, r)),
            );
        }
        for (pane_id, r) in results {
            // Una respuesta solo vale para la petición que se hizo a ESE panel (un servidor hostil en otro
            // panel podría imprimir ids ajenos y suplantar archivos o dar por guardado un `Save` falso)
            let Some(pos) = self
                .pending_remote
                .iter()
                .position(|p| p.id == r.id && p.pane_id == pane_id)
            else {
                continue;
            };
            let pending = self.pending_remote.remove(pos);
            match pending.job {
                RemoteJob::Open { dest, path, pin } => {
                    let data = if r.ok {
                        Ok(r.data)
                    } else {
                        Err("no se pudo leer (¿supera 512 KB o falta permiso?)".to_string())
                    };
                    self.push_remote_viewer(&dest, &path, data, pin);
                }
                RemoteJob::Save {
                    dest,
                    path,
                    expected,
                    edit_gen,
                } => {
                    let viewer = self
                        .tabs
                        .iter_mut()
                        .filter_map(|t| t.file_viewer.as_mut())
                        .find(|v| v.path == path && v.remote_destination.as_deref() == Some(&dest));
                    let Some(v) = viewer else { continue };
                    // `ok` solo dice que `printf` no falló: se da por guardado únicamente si lo escrito en el
                    // servidor (CRC y tamaño) es exactamente lo enviado y el visor no cambió entretanto
                    let written_ok = r.ok && save_matches(&r.data, expected);
                    if !written_ok {
                        v.save_error = Some(
                            "El servidor no guardó lo enviado (¿se tecleó durante el guardado?); vuelve a guardar"
                                .to_string(),
                        );
                        crate::logger::log(
                            "-> Error al guardar el archivo remoto: contenido distinto",
                        );
                    } else if v.edit_gen != edit_gen {
                        crate::logger::log(
                            "-> Guardado remoto: el archivo cambió mientras se guardaba, sigue sin guardar",
                        );
                    } else {
                        v.save_error = None;
                        v.mark_remote_saved();
                    }
                }
                RemoteJob::List { dir, .. } => {
                    if r.ok {
                        let names =
                            crate::terminal::clean_listing(&String::from_utf8_lossy(&r.data));
                        self.file_explorer.expand_remote_dir(&dir, &names);
                    } else {
                        self.file_explorer.set_expanded(&dir, false);
                    }
                    if self.file_explorer.remote_expand_all {
                        self.remote_expand_next();
                    }
                }
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Descarta peticiones en banda cuyos paneles ya se cerraron
    pub(super) fn prune_pending_remote(&mut self) {
        let tabs = &self.tabs;
        self.pending_remote.retain(|p| {
            tabs.iter()
                .any(|t| t.panes.iter().any(|pane| pane.id == p.pane_id))
        });
    }

    /// Peticiones sin respuesta a tiempo (servidor sin bash, línea ocupada...): se intenta la vía lenta
    /// con una segunda conexión SSH. Devuelve el siguiente vencimiento para el bucle de eventos.
    pub(super) fn expire_remote_requests(&mut self) -> Option<Instant> {
        self.prune_pending_remote();
        let now = Instant::now();
        let (expired, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending_remote)
            .into_iter()
            .partition(|p| p.deadline <= now);
        self.pending_remote = pending;
        for p in expired {
            match p.job {
                RemoteJob::Open { dest, path, pin } => self.ssh_open_file(&dest, &path, pin),
                RemoteJob::Save { dest, path, .. } => self.ssh_save_viewer(&dest, &path),
                RemoteJob::List { dir, .. } => {
                    self.file_explorer.set_expanded(&dir, false);
                    self.file_explorer.remote_expand_all = false;
                }
            }
        }
        self.pending_remote.iter().map(|p| p.deadline).min()
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
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pct_encode_keeps_the_request_line_free_of_separators() {
        assert_eq!(pct_encode(b"/home/a b/x.txt"), "/home/a%20b/x.txt");
        assert_eq!(pct_encode(b"a\nb\\c%d\"e"), "a%0Ab%5Cc%25d%22e");
        assert_eq!(pct_encode("ñ".as_bytes()), "%C3%B1");
        assert!(pct_encode(&(0..=255u8).collect::<Vec<_>>())
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "%-_.~/".contains(c)));
    }

    #[test]
    fn test_posix_cksum_matches_the_standard_vector() {
        // `printf 123456789 | cksum` -> 930766865 9
        assert_eq!(posix_cksum(b"123456789"), 930_766_865);
        assert_eq!(posix_cksum(b""), 4_294_967_295);
    }

    #[test]
    fn test_save_matches_only_when_server_wrote_exactly_what_was_sent() {
        let sent = b"linea 1\nlinea 2\n";
        let expected = (posix_cksum(sent), sent.len());
        let reply = |b: &[u8]| format!("{} {}", posix_cksum(b), b.len());
        assert!(save_matches(reply(sent).as_bytes(), expected));
        // Un carácter tecleado en medio, o uno cambiado con el mismo tamaño, o sin respuesta: no es lo enviado
        assert!(!save_matches(
            reply(b"linea 1\nlxnea 2\n").as_bytes(),
            expected
        ));
        assert!(!save_matches(
            reply(b"linea 1\nlinea 2\nx").as_bytes(),
            expected
        ));
        assert!(!save_matches(b"", expected));
        assert!(!save_matches(b"basura", expected));
    }

    /// El gancho REAL de `init.ps1` (bash) responde `ok` con el `cksum` de lo que escribió, y ese valor coincide
    /// con el que calcula Axon; si el contenido llega alterado, el valor difiere
    #[test]
    fn test_hook_put_reports_checksum_of_what_it_wrote() {
        let Ok(out) = std::process::Command::new("bash")
            .args(["-c", "command -v cksum od tr"])
            .output()
        else {
            return; // sin bash en esta máquina
        };
        if !out.status.success() {
            return;
        }
        let init = include_str!("../../assets/shell/init.ps1").replace("\r\n", "\n");
        let start = init.find("$hook = @'\n").expect("gancho") + "$hook = @'\n".len();
        let hook = &init[start..start + init[start..].find("\n'@").expect("fin del gancho")];

        let run = |request_content: &str| -> (String, String) {
            let file = format!("/tmp/axon_hook_test_{}", std::process::id());
            let script = std::env::temp_dir().join(format!("axon_hook_{}.sh", std::process::id()));
            std::fs::write(
                &script,
                format!(
                    "{hook}\nREADLINE_LINE='#AXR#7 put {} {request_content}'\nREADLINE_POINT=0\n__axon_req 2>/dev/null\nprintf '\\n@@@'\ncat {file}\nrm -f {file}\n",
                    pct_encode(file.as_bytes())
                ),
            )
            .unwrap();
            let out = std::process::Command::new("bash")
                .arg(&script)
                .output()
                .unwrap();
            let _ = std::fs::remove_file(&script);
            let text = String::from_utf8_lossy(&out.stdout).replace('\r', "");
            let (osc, written) = text.split_once("\n@@@").expect("salida del gancho");
            let body = osc
                .split("AxonFile=")
                .nth(1)
                .and_then(|r| r.split('\x07').next())
                .expect("respuesta AxonFile");
            let f: Vec<&str> = body.split(';').collect();
            assert_eq!((f[0], f[1]), ("7", "ok"), "{body:?}");
            let hex = f[4];
            let reply: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            (String::from_utf8(reply).unwrap(), written.to_string())
        };

        let content = "hola mundo\nsegunda linea con 100% y 'comillas'\n";
        let (reply, written) = run(&pct_encode(content.as_bytes()));
        assert_eq!(written, content);
        assert!(save_matches(
            reply.as_bytes(),
            (posix_cksum(content.as_bytes()), content.len())
        ));

        // Bytes ajenos mezclados en la línea (lo que pasa si el usuario teclea durante el guardado)
        let tampered = pct_encode("hola mundo\nsegunda linea con 100% y 'comillas'\n".as_bytes())
            .replacen("hola", "holXa", 1);
        let (reply, _) = run(&tampered);
        assert!(!save_matches(
            reply.as_bytes(),
            (posix_cksum(content.as_bytes()), content.len())
        ));
    }

    /// Las lecturas lentas por SSH llegan por el canal de resultados: un doble clic (dos lecturas) abre una sola
    /// pestaña y la segunda respuesta solo la fija
    #[test]
    fn test_ssh_open_results_open_one_tab_and_pin() {
        use super::super::ssh_jobs::SshDone;
        let mut app = AxonApp::new(crate::config::AxonConfig::default(), None);
        let tabs_before = app.tabs.len();
        let done = |pin| SshDone::Open {
            dest: "user@host".into(),
            path: PathBuf::from("/etc/hosts"),
            pin,
            data: Ok(b"127.0.0.1 localhost\n".to_vec()),
        };
        app.ssh_tx.send(done(false)).unwrap();
        app.ssh_tx.send(done(true)).unwrap();
        app.process_ssh_results();
        assert_eq!(app.tabs.len(), tabs_before + 1);
        let tab = app.tabs.last().unwrap();
        assert!(tab.file_viewer.is_some());
        assert!(!tab.is_preview, "la segunda respuesta (doble clic) la fija");
    }

    /// Un panel no puede responder por una petición hecha a otro (mismo id): solo vale la respuesta del suyo
    #[test]
    fn test_remote_result_only_resolves_request_of_its_own_pane() {
        let mut app = AxonApp::new(crate::config::AxonConfig::default(), None);
        let pane_id = app.tabs[0].panes[0].id;
        let result = || crate::terminal::RemoteFileResult {
            id: 7,
            ok: false,
            data: Vec::new(),
        };
        let pending = |pane_id| PendingRemote {
            id: 7,
            pane_id,
            job: RemoteJob::List {
                dest: "user@host".into(),
                dir: PathBuf::from("/tmp"),
            },
            deadline: Instant::now() + REQUEST_TIMEOUT,
        };

        // La petición se hizo a otro panel: la respuesta de este se descarta y la petición sigue en curso
        app.pending_remote.push(pending(pane_id + 1000));
        app.tabs[0].panes[0]
            .terminal
            .remote_file_results
            .push(result());
        app.process_remote_results();
        assert_eq!(app.pending_remote.len(), 1);
        assert!(app.tabs[0].panes[0].terminal.remote_file_results.is_empty());

        // Petición hecha a este panel: se resuelve
        app.pending_remote.clear();
        app.pending_remote.push(pending(pane_id));
        app.tabs[0].panes[0]
            .terminal
            .remote_file_results
            .push(result());
        app.process_remote_results();
        assert!(app.pending_remote.is_empty());
    }

    #[test]
    fn test_remote_save_verifies_cksum_and_closing_tab_cancels_pending_requests() {
        let mut app = AxonApp::new(crate::config::AxonConfig::default(), None);
        app.create_new_tab();
        let pane_id = app.tabs[1].panes[0].id;
        let path = PathBuf::from("/srv/app.toml");
        app.push_remote_viewer("user@host", &path, Ok(b"a = 1\n".to_vec()), true);
        let viewer_tab_idx = app.tabs.len() - 1;
        {
            let v = app.tabs[viewer_tab_idx].file_viewer.as_mut().unwrap();
            v.is_read_only = false;
            v.insert_char('x');
            assert!(v.is_dirty);
        }
        let content = app.tabs[viewer_tab_idx]
            .file_viewer
            .as_mut()
            .unwrap()
            .remote_content();
        let expected = (posix_cksum(content.as_bytes()), content.len());
        let edit_gen = app.tabs[viewer_tab_idx]
            .file_viewer
            .as_ref()
            .unwrap()
            .edit_gen;

        // 1. Respuesta con CRC incorrecto: mantiene is_dirty = true y registra save_error
        app.pending_remote.push(PendingRemote {
            id: 42,
            pane_id,
            job: RemoteJob::Save {
                dest: "user@host".into(),
                path: path.clone(),
                expected,
                edit_gen,
            },
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        app.tabs[1].panes[0]
            .terminal
            .remote_file_results
            .push(crate::terminal::RemoteFileResult {
                id: 42,
                ok: true,
                data: b"0 999".to_vec(),
            });
        app.process_remote_results();
        let v = app.tabs[viewer_tab_idx].file_viewer.as_ref().unwrap();
        assert!(v.is_dirty);
        assert!(v.save_error.is_some());

        // 2. Respuesta con CRC exacto: marca como guardado y limpia save_error
        app.pending_remote.push(PendingRemote {
            id: 43,
            pane_id,
            job: RemoteJob::Save {
                dest: "user@host".into(),
                path: path.clone(),
                expected,
                edit_gen,
            },
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        app.tabs[1].panes[0]
            .terminal
            .remote_file_results
            .push(crate::terminal::RemoteFileResult {
                id: 43,
                ok: true,
                data: format!("{} {}", expected.0, expected.1).into_bytes(),
            });
        app.process_remote_results();
        let v = app.tabs[viewer_tab_idx].file_viewer.as_ref().unwrap();
        assert!(!v.is_dirty);
        assert!(v.save_error.is_none());

        // 3. Cerrar la pestaña del panel SSH cancela sus peticiones pendientes
        app.pending_remote.push(PendingRemote {
            id: 44,
            pane_id,
            job: RemoteJob::Open {
                dest: "user@host".into(),
                path,
                pin: true,
            },
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        app.close_tab(1);
        assert!(app.pending_remote.is_empty());
    }
}
