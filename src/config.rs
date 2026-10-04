use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AxonConfig {
    pub appearance: AppearanceConfig,
    pub terminal: TerminalConfig,
    pub ai: AiConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppearanceConfig {
    pub font_family: String,
    pub font_size: f32,
    pub window_opacity: f32,
    pub theme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalConfig {
    pub default_shell: String,
    pub scrollback_limit: usize,
    pub bracketed_paste: bool,
    pub cursor_blink: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: String,      // "ollama", "gemini", "openai"
    pub model: String,         // ej: "llama3", "gemini-1.5-pro", etc.
    pub endpoint: String,      // ej: "http://localhost:11434"
    pub api_key: Option<String>,
    pub auto_context_lines: usize,
}

impl Default for AxonConfig {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig {
                font_family: "Cascadia Code".to_string(),
                font_size: 14.5,
                window_opacity: 0.95,
                theme: "axon-dark".to_string(),
            },
            terminal: TerminalConfig {
                default_shell: "powershell.exe".to_string(),
                scrollback_limit: 10_000,
                bracketed_paste: true,
                cursor_blink: true,
            },
            ai: AiConfig {
                enabled: true,
                provider: "ollama".to_string(),
                model: "llama3".to_string(),
                endpoint: "http://localhost:11434".to_string(),
                api_key: None,
                auto_context_lines: 30,
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
