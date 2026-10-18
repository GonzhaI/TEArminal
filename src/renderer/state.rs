//! Tipos de estado de la interfaz que `app` construye y el renderer dibuja.

#[derive(Debug, Clone)]
pub struct SidebarTabItem {
    #[allow(dead_code)]
    pub id: usize,
    pub title: String,
    pub is_active: bool,
    pub has_unread: bool,
    #[allow(dead_code)]
    pub pane_count: usize,
    pub is_settings: bool,
    pub is_file_viewer: bool,
    /// Vista previa (no fijada): su título va subrayado
    pub is_preview: bool,
}

#[inline]
pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

pub fn tab_icon(tab: &SidebarTabItem) -> crate::ui::LucideIcon {
    if tab.is_settings {
        crate::ui::LucideIcon::Settings
    } else if tab.is_file_viewer {
        let ext = std::path::Path::new(&tab.title)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if [
            "rs", "py", "js", "ts", "go", "c", "cpp", "h", "cs", "html", "css",
        ]
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
        {
            crate::ui::LucideIcon::FileCode
        } else if ["toml", "json", "yaml", "yml"]
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e))
        {
            crate::ui::LucideIcon::FileJson
        } else if ["md", "txt", "log"]
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e))
        {
            crate::ui::LucideIcon::FileText
        } else {
            crate::ui::LucideIcon::File
        }
    } else if contains_ignore_ascii_case(&tab.title, "claude")
        || contains_ignore_ascii_case(&tab.title, "agent")
        || contains_ignore_ascii_case(&tab.title, "ai")
    {
        crate::ui::LucideIcon::Sparkles
    } else if contains_ignore_ascii_case(&tab.title, "ssh") {
        crate::ui::LucideIcon::Server
    } else {
        crate::ui::LucideIcon::Terminal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Terminal,
    Shortcuts,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAgentStatus {
    None,
    Claude,
    Agy,
    Gemini,
    Aider,
    Copilot,
    Ollama,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditingSettingField {
    FontSize,
    UiFontSize,
    LineSpacing,
    WindowOpacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPageState {
    pub section: SettingsSection,
    pub hovered_section: Option<SettingsSection>,
    pub hovered_theme: Option<usize>,
    pub hovered_theme_mode: Option<usize>,
    pub hovered_tab_layout: Option<usize>,
    pub hovered_font_minus: bool,
    pub hovered_font_plus: bool,
    pub hovered_font_box: bool,
    pub hovered_ui_font_minus: bool,
    pub hovered_ui_font_plus: bool,
    pub hovered_ui_font_box: bool,
    pub hovered_spacing_minus: bool,
    pub hovered_spacing_plus: bool,
    pub hovered_spacing_box: bool,
    pub hovered_opacity_minus: bool,
    pub hovered_opacity_plus: bool,
    pub hovered_opacity_box: bool,
    pub hovered_cursor_blink: bool,
    pub hovered_ghostty_prompt: bool,
    pub hovered_copy_on_select: bool,
    pub hovered_status_bar: bool,
    pub hovered_file_explorer: bool,
    pub hovered_restore_session: bool,
    pub hovered_auto_save: bool,
    pub hovered_word_wrap: bool,
    pub hovered_ssh_integration: bool,
    pub hovered_open_file: bool,
    pub hovered_reset_defaults: bool,
    pub hovered_scrollbar: bool,
    /// Desplazamiento vertical (px) del contenido bajo la barra de navegación
    pub scroll_y: usize,
    pub editing_setting: Option<EditingSettingField>,
    pub setting_input_buffer: String,
}

impl SettingsPageState {
    /// Limpia todos los estados de hover conservando la sección activa, el scroll y la edición en curso.
    /// Devuelve `true` si algún indicador de hover cambió.
    pub fn clear_hover(&mut self) -> bool {
        let cleared = Self {
            section: self.section,
            scroll_y: self.scroll_y,
            editing_setting: self.editing_setting,
            setting_input_buffer: self.setting_input_buffer.clone(),
            ..Self::default()
        };
        if *self != cleared {
            *self = cleared;
            true
        } else {
            false
        }
    }
}

impl Default for SettingsPageState {
    fn default() -> Self {
        Self {
            section: SettingsSection::Appearance,
            hovered_section: None,
            hovered_theme: None,
            hovered_theme_mode: None,
            hovered_tab_layout: None,
            hovered_font_minus: false,
            hovered_font_plus: false,
            hovered_font_box: false,
            hovered_ui_font_minus: false,
            hovered_ui_font_plus: false,
            hovered_ui_font_box: false,
            hovered_spacing_minus: false,
            hovered_spacing_plus: false,
            hovered_spacing_box: false,
            hovered_opacity_minus: false,
            hovered_opacity_plus: false,
            hovered_opacity_box: false,
            hovered_cursor_blink: false,
            hovered_ghostty_prompt: false,
            hovered_copy_on_select: false,
            hovered_status_bar: false,
            hovered_file_explorer: false,
            hovered_restore_session: false,
            hovered_auto_save: false,
            hovered_word_wrap: false,
            hovered_ssh_integration: false,
            hovered_open_file: false,
            hovered_reset_defaults: false,
            hovered_scrollbar: false,
            scroll_y: 0,
            editing_setting: None,
            setting_input_buffer: String::new(),
        }
    }
}
