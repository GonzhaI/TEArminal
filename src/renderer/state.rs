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

#[derive(Debug, Clone)]
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
    pub hovered_open_file: bool,
    pub hovered_reset_defaults: bool,
    pub editing_setting: Option<EditingSettingField>,
    pub setting_input_buffer: String,
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
            hovered_open_file: false,
            hovered_reset_defaults: false,
            editing_setting: None,
            setting_input_buffer: String::new(),
        }
    }
}
