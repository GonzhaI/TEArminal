//! Modal «Añadir máquina»: hosts de `~/.ssh/config` más un campo `user@host`. Añadir crea un Space cuya
//! primera terminal ejecuta `ssh <destino>` (el `ssh` de `init.ps1` se ocupa de la integración).
//!
//! El archivo de configuración y lo que se escribe en el campo son entrada no confiable: el destino solo
//! se acepta si pasa `pty::is_safe_ssh_destination` (sin espacios, `;`, `$`, comillas ni opciones).

use super::*;
use crate::renderer::{MachineHover, MachineModalRects};

/// Máximo de hosts que se leen del archivo (el modal enseña los primeros).
const MAX_HOSTS: usize = 50;

/// Entrada de `~/.ssh/config`: el alias (lo que se teclea tras `ssh`) y a dónde llega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SshHost {
    pub alias: String,
    /// `usuario@host`, solo `host` o, sin `HostName`, el propio alias
    pub target: String,
}

/// Hosts con nombre propio de un `ssh_config`: ignora los patrones (`*`, `?`, `!`) y los repetidos.
pub(crate) fn parse_ssh_config(text: &str) -> Vec<SshHost> {
    struct Block {
        aliases: Vec<String>,
        host_name: Option<String>,
        user: Option<String>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        // `Clave valor` o `Clave=valor`
        let Some((key, value)) = line.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        let value = value.trim().trim_start_matches('=').trim();
        match key.to_ascii_lowercase().as_str() {
            "host" => blocks.push(Block {
                aliases: value
                    .split_whitespace()
                    .filter(|a| !a.contains(['*', '?', '!']))
                    .map(str::to_string)
                    .collect(),
                host_name: None,
                user: None,
            }),
            "hostname" => {
                if let Some(b) = blocks.last_mut() {
                    b.host_name.get_or_insert_with(|| value.to_string());
                }
            }
            "user" => {
                if let Some(b) = blocks.last_mut() {
                    b.user.get_or_insert_with(|| value.to_string());
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<SshHost> = Vec::new();
    for b in blocks {
        for alias in b.aliases {
            if out.iter().any(|h| h.alias == alias) {
                continue;
            }
            let host = b.host_name.clone().unwrap_or_else(|| alias.clone());
            let target = match &b.user {
                Some(u) => format!("{u}@{host}"),
                None => host,
            };
            out.push(SshHost { alias, target });
            if out.len() >= MAX_HOSTS {
                return out;
            }
        }
    }
    out
}

/// Hosts del `~/.ssh/config` del usuario (vacío si no existe o no se puede leer).
fn read_user_ssh_hosts() -> Vec<SshHost> {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return Vec::new();
    };
    let path = std::path::Path::new(&home).join(".ssh").join("config");
    std::fs::read_to_string(path)
        .map(|t| parse_ssh_config(&t))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MachineModal {
    pub hosts: Vec<SshHost>,
    pub input: crate::ui::TextInput,
    pub hovered: Option<MachineHover>,
    /// Host marcado con el teclado (↑/↓); Enter lo añade si el campo está vacío
    pub selected: Option<usize>,
    /// Último destino rechazado, para decirlo en el modal
    pub error: Option<String>,
}

impl TearminalApp {
    pub(crate) fn open_machine_modal(&mut self) {
        self.machine_modal = Some(MachineModal {
            hosts: read_user_ssh_hosts(),
            ..MachineModal::default()
        });
        self.request_redraw();
    }

    pub(crate) fn close_machine_modal(&mut self) {
        self.machine_modal = None;
        self.request_redraw();
    }

    pub(super) fn machine_rects(&self) -> Option<MachineModalRects> {
        let m = self.machine_modal.as_ref()?;
        let (w, h) = self.window_size();
        Some(MachineModalRects::compute(w, h, m.hosts.len()))
    }

    /// Añade una máquina: un Space nuevo y `ssh <destino>` en su terminal. Un destino inseguro se rechaza
    /// y el modal sigue abierto con el motivo.
    pub(crate) fn add_machine(&mut self, dest: &str) {
        let dest = dest.trim();
        if dest.is_empty() || !crate::pty::is_safe_ssh_destination(dest) {
            if let Some(m) = &mut self.machine_modal {
                m.error = Some("Destino no válido: usa usuario@host o un alias de ssh".into());
            }
            self.request_redraw();
            return;
        }
        let before = self.tabs.len();
        self.create_space(None);
        if self.tabs.len() == before {
            return;
        }
        // Nombre del Space: el host sin el usuario
        let name = dest.rsplit('@').next().unwrap_or(dest).to_string();
        let active = self.active_space();
        if let Some(space) = self.spaces.iter_mut().find(|s| s.id == active) {
            space.name = name;
            space.remote = true;
        }
        if let Some(pane) = self
            .tabs
            .get(self.active_tab_index)
            .and_then(|t| t.active_pane())
        {
            pane.pty.write_input(format!("ssh {dest}\r").as_bytes());
        }
        self.machine_modal = None;
        self.save_session();
        self.request_redraw();
    }

    /// Teclado con el modal abierto: Esc cierra, Enter añade lo escrito, el resto edita el campo.
    pub(super) fn handle_machine_key(
        &mut self,
        logical_key: &winit::keyboard::Key,
        physical_key: winit::keyboard::PhysicalKey,
        text: &Option<winit::keyboard::SmolStr>,
    ) {
        use winit::keyboard::{Key, NamedKey};
        match logical_key {
            Key::Named(NamedKey::Escape) => self.close_machine_modal(),
            Key::Named(arrow @ (NamedKey::ArrowDown | NamedKey::ArrowUp)) => {
                if let Some(m) = &mut self.machine_modal {
                    let n = m.hosts.len().min(MachineModalRects::MAX_ROWS);
                    if n > 0 {
                        m.selected =
                            Some(match (m.selected, matches!(arrow, NamedKey::ArrowDown)) {
                                (None, true) => 0,
                                (None, false) => n - 1,
                                (Some(i), true) => (i + 1) % n,
                                (Some(i), false) => (i + n - 1) % n,
                            });
                    }
                }
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                let (typed, picked) = self
                    .machine_modal
                    .as_ref()
                    .map(|m| {
                        let picked = m
                            .selected
                            .and_then(|i| m.hosts.get(i))
                            .map(|h| h.alias.clone());
                        (m.input.text().to_string(), picked)
                    })
                    .unwrap_or_default();
                match picked {
                    Some(alias) if typed.trim().is_empty() => self.add_machine(&alias),
                    _ => self.add_machine(&typed),
                }
            }
            _ => {
                let Some(m) = &mut self.machine_modal else {
                    return;
                };
                let mut input = std::mem::take(&mut m.input);
                let (_, changed) = self.handle_text_input_key(
                    &mut input,
                    logical_key,
                    physical_key,
                    text.as_deref(),
                );
                if let Some(m) = &mut self.machine_modal {
                    m.input = input;
                    if changed {
                        m.error = None;
                        m.selected = None;
                    }
                }
            }
        }
    }

    /// Clic con el modal abierto. Siempre lo consume.
    pub(super) fn click_machine_modal(&mut self, mx: usize, my: usize) {
        let Some(rects) = self.machine_rects() else {
            return;
        };
        let hit = rects.hit(mx, my);
        match hit {
            None if !MachineModalRects::inside(mx, my, rects.card) => self.close_machine_modal(),
            Some(MachineHover::Row(i)) => {
                let dest = self
                    .machine_modal
                    .as_ref()
                    .and_then(|m| m.hosts.get(i))
                    .map(|h| h.alias.clone());
                if let Some(dest) = dest {
                    self.add_machine(&dest);
                }
            }
            Some(MachineHover::TypedAdd) => {
                let typed = self
                    .machine_modal
                    .as_ref()
                    .map(|m| m.input.text().to_string())
                    .unwrap_or_default();
                self.add_machine(&typed);
            }
            Some(MachineHover::Input) => {
                if let Some(m) = &mut self.machine_modal {
                    let mut input = std::mem::take(&mut m.input);
                    self.handle_text_input_click(&mut input, mx, rects.input.0 + 14);
                    if let Some(m) = &mut self.machine_modal {
                        m.input = input;
                    }
                }
            }
            None => {}
        }
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_hosts_and_skips_patterns() {
        let cfg = "\
# comentario
Host mytiluss
    HostName 3.95.102.141
    User ec2-user

Host *
    ServerAliveInterval 30

Host github.com github-personal
    User git
Host web-?
    HostName x
Host intranet
    HostName=10.0.0.5 # con igual
";
        let hosts = parse_ssh_config(cfg);
        let aliases: Vec<_> = hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(
            aliases,
            ["mytiluss", "github.com", "github-personal", "intranet"]
        );
        assert_eq!(hosts[0].target, "ec2-user@3.95.102.141");
        assert_eq!(hosts[1].target, "git@github.com");
        assert_eq!(hosts[3].target, "10.0.0.5");
    }

    #[test]
    fn unsafe_destinations_are_rejected_by_add_machine() {
        let mut app = TearminalApp::new(crate::config::TearminalConfig::default(), None);
        app.machine_modal = Some(MachineModal::default());
        let spaces = app.spaces.len();
        for bad in ["", "-oProxyCommand=calc", "a b", "host;rm", "$(x)"] {
            app.add_machine(bad);
            assert_eq!(app.spaces.len(), spaces, "{bad:?}");
            assert!(app
                .machine_modal
                .as_ref()
                .is_some_and(|m| m.error.is_some()));
        }
    }
}
