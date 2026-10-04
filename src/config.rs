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
    pub scrollback_lines: usize,
    pub bracketed_paste: bool,
    pub cursor_blink: bool,
    pub copy_on_select: bool,
}

impl Default for AxonConfig {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig {
                font_family: "Cascadia Code".to_string(),
                font_path: None,
                font_size: 14.0,
                window_opacity: 0.98,
                theme: "ghostty-dark".to_string(),
                line_spacing: 1.25,
            },
            terminal: TerminalConfig {
                default_shell: "powershell.exe".to_string(),
                scrollback_lines: 50_000,
                bracketed_paste: true,
                cursor_blink: true,
                copy_on_select: false,
            },
        }
    }
}

impl AxonConfig {
    pub fn config_path() -> PathBuf {
        let base_dir = dirs_or_fallback();
        base_dir.join("axon.toml")
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<AxonConfig>(&content) {
                return cfg;
            }
        }
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
