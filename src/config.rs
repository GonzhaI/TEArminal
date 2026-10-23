use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct AxonConfig {
    pub(crate) appearance: AppearanceConfig,
    pub(crate) terminal: TerminalConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct AppearanceConfig {
    pub(crate) font_family: String,
    #[serde(default)]
    pub(crate) font_path: Option<String>,
    #[serde(default = "default_font_size")]
    pub(crate) font_size: f32,
    #[serde(default = "default_ui_font_size")]
    pub(crate) ui_font_size: f32,
    pub(crate) window_opacity: f32,
    /// Tema oscuro; `theme_light` es el claro. `theme_mode` decide cuál se usa
    /// ("dark", "light" o "system": el que corresponda al tema de Windows en cada momento).
    pub(crate) theme: String,
    #[serde(default)]
    pub(crate) theme_light: String,
    pub(crate) line_spacing: f32,
    #[serde(default = "default_tab_layout")]
    pub(crate) tab_layout: String,
    #[serde(default = "default_theme_mode")]
    pub(crate) theme_mode: String,
    #[serde(default = "default_true")]
    pub(crate) show_status_bar: bool,
    #[serde(default = "default_false")]
    pub(crate) show_file_explorer: bool,
    /// Lado de la ventana donde va el panel de navegación: "left" | "right".
    #[serde(default = "default_panel_side")]
    pub(crate) panel_side: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TerminalConfig {
    pub(crate) default_shell: String,
    #[serde(default = "default_true", alias = "ghostty_prompt")]
    pub(crate) shell_integration: bool,
    pub(crate) scrollback_lines: usize,
    pub(crate) cursor_blink: bool,
    pub(crate) copy_on_select: bool,
    #[serde(default = "default_true")]
    pub(crate) restore_session: bool,
    #[serde(default = "default_false")]
    pub(crate) auto_save: bool,
    /// Ajuste de línea del visor de archivos (los textos largos se parten dentro del recuadro)
    #[serde(default = "default_false")]
    pub(crate) word_wrap: bool,
    /// Al abrir `ssh` desde Axon, el shell remoto (bash) informa de la rama git y el directorio
    #[serde(default = "default_true")]
    pub(crate) ssh_integration: bool,
    /// Deja que los programas de la terminal escriban en el portapapeles de Windows (OSC 52, máx. 100 KB).
    /// Activo por defecto (la lectura '?' siempre está bloqueada por seguridad).
    #[serde(default = "default_true")]
    pub(crate) osc52: bool,
    /// Ligaduras tipográficas de programación (->, !=, ==, =>...)
    #[serde(default = "default_true")]
    pub(crate) ligatures: bool,
    /// Sin efecto: la GPU (Direct3D 11, con WARP de respaldo) es el único renderer. Se conserva solo
    /// para que un `axon.toml` antiguo con `gpu = ...` siga cargando. Nada debe leer este campo.
    #[serde(default = "default_true")]
    pub(crate) gpu: bool,
    /// Comprobación automática de nuevas versiones al iniciar Axon
    #[serde(default = "default_true")]
    pub(crate) auto_check_updates: bool,
    /// Consulta las cuotas reales de Claude (api.anthropic.com/api/oauth/usage) con la sesión de Claude Code
    /// (`~/.claude/.credentials.json`, solo lectura). Con `false` Axon no lee esa credencial ni hace la llamada.
    #[serde(default = "default_true")]
    pub(crate) ai_limits: bool,
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

fn default_explorer_width() -> usize {
    crate::explorer::FILE_EXPLORER_WIDTH
}

fn default_panel_side() -> String {
    "left".to_string()
}

fn default_tab_layout() -> String {
    "horizontal".to_string()
}

fn default_theme_mode() -> String {
    "dark".to_string()
}

impl AppearanceConfig {
    /// ¿El panel de navegación va a la derecha de la ventana?
    pub(crate) fn panel_on_right(&self) -> bool {
        self.panel_side.eq_ignore_ascii_case("right")
    }

    /// ¿El modo configurado usa el tema claro? `system_light` es el modo claro de Windows.
    pub(crate) fn uses_light_theme(&self, system_light: bool) -> bool {
        match self.theme_mode.to_ascii_lowercase().as_str() {
            "light" => true,
            "system" => system_light,
            _ => false,
        }
    }

    /// Tema activo según el modo y el tema de Windows.
    pub(crate) fn resolve_theme(&self, system_light: bool) -> crate::theme::Theme {
        crate::theme::Theme::from_name(if self.uses_light_theme(system_light) {
            &self.theme_light
        } else {
            &self.theme
        })
    }

    /// Configuraciones anteriores guardaban un único `theme` (en modo claro, uno claro).
    fn migrate(&mut self) {
        if !self.theme_light.is_empty() {
            return;
        }
        if crate::theme::Theme::from_name(&self.theme).is_light() {
            self.theme_light = std::mem::replace(
                &mut self.theme,
                crate::theme::Theme::DEFAULT_DARK.to_string(),
            );
        } else {
            self.theme_light = crate::theme::Theme::DEFAULT_LIGHT.to_string();
        }
    }
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
                theme: crate::theme::Theme::DEFAULT_DARK.to_string(),
                theme_light: crate::theme::Theme::DEFAULT_LIGHT.to_string(),
                line_spacing: 1.0,
                tab_layout: "horizontal".to_string(),
                theme_mode: "dark".to_string(),
                show_status_bar: true,
                show_file_explorer: false,
                panel_side: default_panel_side(),
            },
            terminal: TerminalConfig {
                default_shell: shell,
                shell_integration: true,
                scrollback_lines: 10_000,
                cursor_blink: true,
                copy_on_select: false,
                restore_session: true,
                auto_save: false,
                word_wrap: false,
                ssh_integration: true,
                osc52: true,
                ligatures: true,
                gpu: true,
                auto_check_updates: true,
                ai_limits: true,
            },
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        AxonConfig::default().appearance
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        AxonConfig::default().terminal
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("error de E/S al gestionar la configuración: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de serialización TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("error de deserialización TOML: {0}")]
    Deserialize(#[from] toml::de::Error),
    #[error("axon.toml tiene errores: corrígelo antes de guardar ajustes")]
    InvalidExistingConfig,
}

impl AxonConfig {
    pub(crate) fn config_path() -> PathBuf {
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

    pub(crate) fn load_or_default() -> Self {
        Self::load_from(&Self::config_path())
    }

    /// Deserializa la configuración desde cualquier lector genérico en memoria o archivo
    pub(crate) fn from_reader(mut reader: impl std::io::Read) -> Result<Self, ConfigError> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        let mut cfg = toml::from_str::<Self>(&content)?;
        cfg.appearance.migrate();
        Ok(cfg)
    }

    /// Serializa la configuración hacia cualquier flujo de salida genérico en memoria o archivo
    pub(crate) fn write_to(&self, mut writer: impl std::io::Write) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        writer.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Nunca sobrescribe un archivo existente: si no se puede interpretar, lo copia a
    /// `axon.toml.bak` y usa los valores por defecto solo en memoria.
    fn load_from(path: &std::path::Path) -> Self {
        match fs::File::open(path) {
            Ok(file) => match Self::from_reader(BufReader::new(file)) {
                Ok(cfg) => cfg,
                Err(e) => {
                    crate::logger::log(&format!(
                        "-> axon.toml inválido ({e}); copia en axon.toml.bak, usando valores por defecto"
                    ));
                    if let Ok(content) = fs::read_to_string(path) {
                        let _ = fs::write(path.with_extension("toml.bak"), content);
                    }
                    notify_invalid_config(path, &e.to_string());
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let default_cfg = Self::default();
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(file) = fs::File::create(path) {
                    let mut writer = BufWriter::new(file);
                    let _ = default_cfg.write_to(&mut writer);
                    let _ = writer.flush();
                }
                default_cfg
            }
            Err(_) => Self::default(),
        }
    }

    pub(crate) fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&Self::config_path())
    }

    fn save_to(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        // Un `axon.toml` que no se puede interpretar se carga con los valores por defecto; guardar encima
        // destruiría lo que el usuario escribió (queda también la copia `.bak`)
        if let Ok(existing) = fs::read_to_string(path) {
            if toml::from_str::<AxonConfig>(&existing).is_err() {
                crate::logger::log(
                    "-> axon.toml inválido: no se guardan los ajustes hasta corregirlo",
                );
                return Err(ConfigError::InvalidExistingConfig);
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        {
            let file = fs::File::create(&tmp)?;
            let mut writer = BufWriter::new(file);
            self.write_to(&mut writer)?;
            writer.flush()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Avisa en pantalla de que `axon.toml` no se pudo interpretar (Axon usa el subsistema "windows": sin esto el
/// usuario solo vería sus ajustes ignorados). En tests no hay ventana.
#[cfg(not(test))]
fn notify_invalid_config(path: &std::path::Path, err: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    let text = wide(&format!(
        "Axon no pudo leer {}\n\n{err}\n\nSe usan los valores por defecto y no se guardarán ajustes hasta que \
         lo corrijas. Se dejó una copia en axon.toml.bak.",
        path.display()
    ));
    let title = wide("Axon");
    // SAFETY: cadenas UTF-16 terminadas en nulo que viven durante la llamada
    unsafe { MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING) };
}

#[cfg(test)]
fn notify_invalid_config(_path: &std::path::Path, _err: &str) {}

/// Carpeta de configuración y sesión del usuario (`%APPDATA%\axon\axon\config`)
pub(crate) fn dirs_or_fallback() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("axon-test-{}", std::process::id()));
    }
    if let Some(dirs) = directories::ProjectDirs::from("com", "axon", "axon") {
        dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("axon_config")
    }
}

/// Carpeta de caché local del usuario (`%LOCALAPPDATA%\axon\axon\cache`)
pub(crate) fn cache_dir_or_fallback() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("axon-test-cache-{}", std::process::id()));
    }
    if let Some(dirs) = directories::ProjectDirs::from("com", "axon", "axon") {
        dirs.cache_dir().to_path_buf()
    } else {
        PathBuf::from("axon_cache")
    }
}

/// Sesión persistida: una entrada por ventana abierta (`session.toml`)
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct SessionState {
    pub(crate) windows: Vec<WindowSession>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct WindowSession {
    /// Posición entre las pestañas de terminal (ajustes y visores no se guardan)
    pub(crate) active_tab_index: usize,
    #[serde(default = "default_true")]
    pub(crate) sidebar_visible: bool,
    #[serde(default = "default_false")]
    pub(crate) file_explorer_visible: bool,
    #[serde(default = "default_explorer_width")]
    pub(crate) file_explorer_width: usize,
    pub(crate) tabs: Vec<TabSession>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TabSession {
    pub(crate) title: String,
    pub(crate) layout: PaneSession,
    /// Panel activo: posición entre las hojas de `layout`, de izquierda a derecha
    #[serde(default)]
    pub(crate) active_pane: usize,
}

/// Árbol de paneles de una pestaña (espejo serializable de `app::PaneNode`)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum PaneSession {
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

impl Default for PaneSession {
    fn default() -> Self {
        Self::Leaf { cwd: None }
    }
}

impl Default for WindowSession {
    fn default() -> Self {
        Self {
            active_tab_index: 0,
            sidebar_visible: true,
            file_explorer_visible: false,
            file_explorer_width: default_explorer_width(),
            tabs: Vec::new(),
        }
    }
}

impl Default for TabSession {
    fn default() -> Self {
        Self {
            title: String::new(),
            layout: PaneSession::default(),
            active_pane: 0,
        }
    }
}

/// Ventanas vivas de este proceso y su última instantánea: `session.toml` guarda todas a la vez
static OPEN_WINDOWS: std::sync::Mutex<std::collections::BTreeMap<u32, WindowSession>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

impl SessionState {
    pub(crate) fn session_path() -> PathBuf {
        let base_dir = dirs_or_fallback();
        base_dir.join("session.toml")
    }

    /// Deserializa el estado de sesión desde cualquier lector genérico
    pub(crate) fn from_reader(mut reader: impl std::io::Read) -> Result<Self, ConfigError> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        let state = toml::from_str::<Self>(&content)?;
        Ok(state)
    }

    /// Serializa el estado de sesión hacia cualquier flujo de salida genérico
    pub(crate) fn write_to(&self, mut writer: impl std::io::Write) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        writer.write_all(content.as_bytes())?;
        Ok(())
    }

    pub(crate) fn load() -> Option<Self> {
        let path = Self::session_path();
        let file = fs::File::open(&path).ok()?;
        Self::from_reader(BufReader::new(file)).ok()
    }

    pub(crate) fn save(&self) -> Result<(), ConfigError> {
        let path = Self::session_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("tmp");
        {
            let file = fs::File::create(&tmp)?;
            let mut writer = BufWriter::new(file);
            self.write_to(&mut writer)?;
            writer.flush()?;
        }
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Guarda la instantánea de la ventana `id` junto a las de las demás ventanas abiertas
    pub(crate) fn save_window(id: u32, window: WindowSession) {
        Self::update_open_windows(|open| {
            open.insert(id, window);
        });
    }

    /// La ventana `id` se cerró con otras aún abiertas: deja de formar parte de la sesión
    pub(crate) fn forget_window(id: u32) {
        Self::update_open_windows(|open| {
            open.remove(&id);
        });
    }

    fn update_open_windows(f: impl FnOnce(&mut std::collections::BTreeMap<u32, WindowSession>)) {
        let state = {
            let mut open = OPEN_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut open);
            Self {
                windows: open.values().cloned().collect(),
            }
        };
        let _ = state.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_slots_by_mode_and_system() {
        let mut a = AxonConfig::default().appearance;
        a.theme = "Dracula".into();
        a.theme_light = "One Half Light".into();
        a.theme_mode = "system".into();
        let (dark, light) = (a.resolve_theme(false), a.resolve_theme(true));
        assert!(!dark.is_light() && light.is_light());
        a.theme_mode = "dark".into();
        assert_eq!(a.resolve_theme(true).background, dark.background);
        a.theme_mode = "light".into();
        assert_eq!(a.resolve_theme(false).background, light.background);
        // "system" sigue al tema de Windows en cada momento
        a.theme_mode = "system".into();
        assert_eq!(a.resolve_theme(false).background, dark.background);
        assert_eq!(a.resolve_theme(true).background, light.background);
    }

    /// Claves que ya no existen (p. ej. `bracketed_paste`) en un `axon.toml` antiguo se ignoran, no lo invalidan
    #[test]
    fn test_removed_keys_in_old_configs_are_ignored() {
        let text = toml::to_string_pretty(&AxonConfig::default())
            .unwrap()
            .replace("[terminal]\n", "[terminal]\nbracketed_paste = true\n");
        assert!(text.contains("bracketed_paste"));
        assert!(toml::from_str::<AxonConfig>(&text).is_ok());
    }

    /// Un `axon.toml` ilegible nunca se sobrescribe al guardar ajustes
    #[test]
    fn test_save_never_overwrites_an_invalid_config() {
        let dir = std::env::temp_dir().join(format!("axon-badcfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("axon.toml");
        fs::write(&path, "theme = [esto no es toml").unwrap();
        assert!(AxonConfig::default().save_to(&path).is_err());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "theme = [esto no es toml"
        );
        fs::write(
            &path,
            toml::to_string_pretty(&AxonConfig::default()).unwrap(),
        )
        .unwrap();
        assert!(AxonConfig::default().save_to(&path).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_legacy_single_theme_is_migrated() {
        let dir = std::env::temp_dir().join(format!("axon-mig-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("axon.toml");
        let load = |theme: &str| {
            // Configuración completa pero sin `theme_light`, como la escribían las versiones anteriores
            let old: String = toml::to_string_pretty(&AxonConfig::default())
                .unwrap()
                .lines()
                .filter(|l| !l.starts_with("theme_light"))
                .map(|l| {
                    if l.starts_with("theme = ") {
                        format!("theme = \"{theme}\"")
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(
                    "
",
                );
            fs::write(&path, old).unwrap();
            AxonConfig::load_from(&path).appearance
        };
        // Un `theme` oscuro conserva su ranura y el claro toma el valor por defecto
        let a = load("Dracula");
        assert_eq!(
            (a.theme.as_str(), a.theme_light.as_str()),
            ("Dracula", "Catppuccin Latte")
        );
        // Un `theme` claro (modo claro antiguo) pasa a la ranura clara
        let a = load("latte");
        assert_eq!(
            (a.theme.as_str(), a.theme_light.as_str()),
            ("Catppuccin Frappe", "latte")
        );
        let _ = fs::remove_dir_all(&dir);
    }

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
                file_explorer_width: 320,
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
    fn test_osc52_is_on_by_default_also_for_old_configs() {
        assert!(AxonConfig::default().terminal.osc52);
        let mut old = toml::to_string(&AxonConfig::default()).unwrap();
        old = old
            .lines()
            .filter(|l| !l.starts_with("osc52"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!old.contains("osc52"));
        assert!(toml::from_str::<AxonConfig>(&old).unwrap().terminal.osc52);
    }

    #[test]
    fn test_config_gpu_key_is_accepted_and_ignored() {
        // `terminal.gpu` solo existe para que un axon.toml antiguo siga cargando: ningún código lo lee.
        let text = toml::to_string_pretty(&AxonConfig::default())
            .unwrap()
            .replace("gpu = true", "gpu = false");
        assert!(text.contains("gpu = false"));
        let cfg = toml::from_str::<AxonConfig>(&text).unwrap();
        assert!(!cfg.terminal.gpu);
    }

    #[test]
    fn test_config_and_session_read_write_traits_in_memory() {
        // Probar serialización/deserialización en memoria sobre std::io::Read / Write sin tocar disco
        let cfg = AxonConfig::default();
        let mut bytes = Vec::new();
        cfg.write_to(&mut bytes).expect("write_to debe tener éxito");

        let loaded =
            AxonConfig::from_reader(bytes.as_slice()).expect("from_reader debe tener éxito");
        assert_eq!(loaded, cfg);

        let session = SessionState::default();
        let mut session_bytes = Vec::new();
        session
            .write_to(&mut session_bytes)
            .expect("session write_to debe tener éxito");

        let loaded_session = SessionState::from_reader(session_bytes.as_slice())
            .expect("session from_reader debe tener éxito");
        assert_eq!(loaded_session, session);
    }
}
