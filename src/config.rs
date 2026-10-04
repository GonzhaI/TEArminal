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
    pub font_size: f32,
    pub window_opacity: f32,
    pub theme: String,
    pub line_spacing: f32,
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
}

fn default_true() -> bool {
    true
}

impl Default for AxonConfig {
    fn default() -> Self {
        let shell = detect_preferred_shell();
        Self {
            appearance: AppearanceConfig {
                font_family: "JetBrains Mono Nerd Font".to_string(),
                font_path: None,
                font_size: 13.0,
                window_opacity: 0.98,
                theme: "ghostty-dark".to_string(),
                line_spacing: 1.0,
            },
            terminal: TerminalConfig {
                default_shell: shell,
                ghostty_prompt: true,
                scrollback_lines: 50_000,
                bracketed_paste: true,
                cursor_blink: true,
                copy_on_select: false,
            },
        }
    }
}

fn detect_preferred_shell() -> String {
    // Si pwsh (PowerShell 7+) está instalado, preferirlo para soporte nativo de colores ANSI y sintaxis
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let pwsh_exe = dir.join("pwsh.exe");
            if pwsh_exe.exists() {
                return "pwsh.exe".to_string();
            }
        }
    }
    "powershell.exe".to_string()
}

impl AxonConfig {
    pub fn config_path() -> PathBuf {
        // 1. Prioridad: axon.toml en el directorio de trabajo actual
        let local = PathBuf::from("axon.toml");
        if local.exists() {
            return local;
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
        // 1. Probar carga desde la ruta detectada
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<AxonConfig>(&content) {
                return cfg;
            }
        }

        // 2. Fallback a configuración por defecto y persistencia
        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
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

fn dirs_or_fallback() -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("com", "axon", "axon") {
        dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("axon_config")
    }
}

