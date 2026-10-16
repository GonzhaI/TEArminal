use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AxonConfig {
    pub appearance: AppearanceConfig,
    pub terminal: TerminalConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppearanceConfig {
    pub font_family: String,
    #[serde(default)]
    pub font_path: Option<String>,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_ui_font_size")]
    pub ui_font_size: f32,
    pub window_opacity: f32,
    pub theme: String,
    pub line_spacing: f32,
    #[serde(default = "default_tab_layout")]
    pub tab_layout: String,
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    #[serde(default = "default_false")]
    pub show_file_explorer: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalConfig {
    pub default_shell: String,
    #[serde(default = "default_true")]
    pub ghostty_prompt: bool,
    pub scrollback_lines: usize,
    pub bracketed_paste: bool,
    pub cursor_blink: bool,
    pub copy_on_select: bool,
    #[serde(default = "default_false")]
    pub restore_session: bool,
    #[serde(default = "default_false")]
    pub auto_save: bool,
    /// Ajuste de línea del visor de archivos (los textos largos se parten dentro del recuadro)
    #[serde(default = "default_false")]
    pub word_wrap: bool,
    /// Al abrir `ssh` desde Axon, el shell remoto (bash) informa de la rama git y el directorio
    #[serde(default = "default_true")]
    pub ssh_integration: bool,
    /// Deja que los programas de la terminal escriban en el portapapeles de Windows (OSC 52, máx. 100 KB).
    /// Apagado por defecto: cualquier salida (un `cat` ajeno, un servidor SSH) podría dejar un comando listo para pegar.
    #[serde(default = "default_false")]
    pub osc52: bool,
}

fn default_font_size() -> f32 {
    15.0
}

fn default_ui_font_size() -> f32 {
    15.0
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_tab_layout() -> String {
    "horizontal".to_string()
}

fn default_theme_mode() -> String {
    "dark".to_string()
}

impl Default for AxonConfig {
    fn default() -> Self {
        let shell = detect_preferred_shell();
        Self {
            appearance: AppearanceConfig {
                font_family: "JetBrains Mono Nerd Font".to_string(),
                font_path: None,
                font_size: 15.0,
                ui_font_size: 15.0,
                window_opacity: 1.0,
                theme: "frappe".to_string(),
                line_spacing: 1.0,
                tab_layout: "horizontal".to_string(),
                theme_mode: "dark".to_string(),
                show_status_bar: true,
                show_file_explorer: false,
            },
            terminal: TerminalConfig {
                default_shell: shell,
                ghostty_prompt: true,
                scrollback_lines: 10_000,
                bracketed_paste: true,
                cursor_blink: true,
                copy_on_select: false,
                restore_session: false,
                auto_save: false,
                word_wrap: false,
                ssh_integration: true,
                osc52: false,
            },
        }
    }
}

fn detect_preferred_shell() -> String {
    // Solo usar pwsh.exe si existe como binario Win32 nativo completo en Program Files.
    // Los alias de Windows Store (AppData\Local\Microsoft\WindowsApps\pwsh.exe) bloquean ConPTY en Windows 11.
    let prog_pwsh = std::path::Path::new(r"C:\Program Files\PowerShell\7\pwsh.exe");
    if prog_pwsh.exists() {
        return prog_pwsh.to_string_lossy().to_string();
    }
    "powershell.exe".to_string()
}

impl AxonConfig {
    pub fn config_path() -> PathBuf {
        // Los tests no leen ni escriben la configuración real del usuario ni el axon.toml del repositorio
        if cfg!(test) {
            return dirs_or_fallback().join("axon.toml");
        }

        // 1. Solo en desarrollo: axon.toml del directorio actual. En release es un riesgo:
        //    un repo clonado con `default_shell` malicioso ejecutaría su programa al abrir Axon ahí.
        if cfg!(debug_assertions) {
            let local = PathBuf::from("axon.toml");
            if local.exists() {
                return local;
            }
        }

        // 2. Prioridad: axon.toml junto al ejecutable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let exe_cfg = parent.join("axon.toml");
                if exe_cfg.exists() {
                    return exe_cfg;
                }
            }
        }

        // 3. Fallback: Directorio de configuración de usuario en AppData
        let base_dir = dirs_or_fallback();
        base_dir.join("axon.toml")
    }

    pub fn load_or_default() -> Self {
        Self::load_from(&Self::config_path())
    }

    /// Nunca sobrescribe un archivo existente: si no se puede interpretar, lo copia a
    /// `axon.toml.bak` y usa los valores por defecto solo en memoria.
    fn load_from(path: &std::path::Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<AxonConfig>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    crate::logger::log(&format!(
                        "-> axon.toml inválido ({e}); copia en axon.toml.bak, usando valores por defecto"
                    ));
                    let _ = fs::write(path.with_extension("toml.bak"), content);
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let default_cfg = Self::default();
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(content) = toml::to_string_pretty(&default_cfg) {
                    let _ = fs::write(path, content);
                }
                default_cfg
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// Carpeta de configuración y sesión del usuario (`%APPDATA%\axon\axon\config`)
fn dirs_or_fallback() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("axon-test-{}", std::process::id()));
    }
    if let Some(dirs) = directories::ProjectDirs::from("com", "axon", "axon") {
        dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("axon_config")
    }
}

/// Sesión persistida: una entrada por ventana abierta (`session.toml`)
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct SessionState {
    pub windows: Vec<WindowSession>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WindowSession {
    /// Posición entre las pestañas de terminal (ajustes y visores no se guardan)
    pub active_tab_index: usize,
    #[serde(default = "default_true")]
    pub sidebar_visible: bool,
    #[serde(default = "default_false")]
    pub file_explorer_visible: bool,
    pub tabs: Vec<TabSession>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TabSession {
    pub title: String,
    pub layout: PaneSession,
    /// Panel activo: posición entre las hojas de `layout`, de izquierda a derecha
    #[serde(default)]
    pub active_pane: usize,
}

/// Árbol de paneles de una pestaña (espejo serializable de `app::PaneNode`)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PaneSession {
    Leaf {
        cwd: Option<String>,
    },
    Split {
        /// `horizontal` (uno encima del otro) o `vertical` (lado a lado)
        direction: String,
        ratio: f32,
        first: Box<PaneSession>,
        second: Box<PaneSession>,
    },
}

/// Ventanas vivas de este proceso y su última instantánea: `session.toml` guarda todas a la vez
static OPEN_WINDOWS: std::sync::Mutex<std::collections::BTreeMap<u32, WindowSession>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

impl SessionState {
    pub fn session_path() -> PathBuf {
        let base_dir = dirs_or_fallback();
        base_dir.join("session.toml")
    }

    pub fn load() -> Option<Self> {
        let path = Self::session_path();
        if let Ok(content) = fs::read_to_string(&path) {
            toml::from_str::<Self>(&content).ok()
        } else {
            None
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::session_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Guarda la instantánea de la ventana `id` junto a las de las demás ventanas abiertas
    pub fn save_window(id: u32, window: WindowSession) {
        Self::update_open_windows(|open| {
            open.insert(id, window);
        });
    }

    /// La ventana `id` se cerró con otras aún abiertas: deja de formar parte de la sesión
    pub fn forget_window(id: u32) {
        Self::update_open_windows(|open| {
            open.remove(&id);
        });
    }

    fn update_open_windows(f: impl FnOnce(&mut std::collections::BTreeMap<u32, WindowSession>)) {
        let mut open = OPEN_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut open);
        let state = Self {
            windows: open.values().cloned().collect(),
        };
        let _ = state.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_toml_is_backed_up_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("axon-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("axon.toml");

        // Inexistente: se crea con los valores por defecto
        AxonConfig::load_from(&path);
        assert!(path.exists());

        // Inválido: el archivo queda intacto y se guarda copia
        let broken = "# mi nota\n[appearance\nfont_size = \"";
        fs::write(&path, broken).unwrap();
        AxonConfig::load_from(&path);
        assert_eq!(fs::read_to_string(&path).unwrap(), broken);
        assert_eq!(
            fs::read_to_string(dir.join("axon.toml.bak")).unwrap(),
            broken
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_with_nested_splits_round_trips_through_toml() {
        let leaf = |cwd: &str| PaneSession::Leaf {
            cwd: Some(cwd.to_string()),
        };
        let state = SessionState {
            windows: vec![WindowSession {
                active_tab_index: 1,
                sidebar_visible: false,
                file_explorer_visible: true,
                tabs: vec![
                    TabSession {
                        title: "Terminal 1".into(),
                        layout: leaf("C:/Users"),
                        active_pane: 0,
                    },
                    TabSession {
                        title: "Terminal 2".into(),
                        layout: PaneSession::Split {
                            direction: "vertical".into(),
                            ratio: 0.25,
                            first: Box::new(leaf("C:/a")),
                            second: Box::new(PaneSession::Split {
                                direction: "horizontal".into(),
                                ratio: 0.75,
                                first: Box::new(PaneSession::Leaf { cwd: None }),
                                second: Box::new(leaf("C:/b")),
                            }),
                        },
                        active_pane: 2,
                    },
                ],
            }],
        };
        let text = toml::to_string_pretty(&state).unwrap();
        assert_eq!(toml::from_str::<SessionState>(&text).unwrap(), state);
    }

    #[test]
    fn test_osc52_is_off_by_default_also_for_old_configs() {
        assert!(!AxonConfig::default().terminal.osc52);
        let mut old = toml::to_string(&AxonConfig::default()).unwrap();
        old = old
            .lines()
            .filter(|l| !l.starts_with("osc52"))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        assert!(!old.contains("osc52"));
        assert!(!toml::from_str::<AxonConfig>(&old).unwrap().terminal.osc52);
    }
}
