//! Tipos de estado de la interfaz que `app` construye y el renderer dibuja.

/// Estado de una sesión según OSC 133: lo único que TEArminal sabe de forma fiable de un comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SessionStatus {
    /// Prompt libre y sin comandos terminados.
    #[default]
    Idle,
    /// Hay un comando en ejecución (un agente de CLI cuenta como tal mientras viva su proceso).
    Running,
    /// El último comando terminó con código 0.
    Done,
    /// El último comando terminó con un código distinto de 0.
    Failed,
}

impl SessionStatus {
    #[allow(dead_code)]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Idle => "En espera",
            Self::Running => "En curso",
            Self::Done => "Terminó",
            Self::Failed => "Falló",
        }
    }
}

/// Límite real de la cuenta mostrado como medidor en la barra de estado.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct StatusMeter {
    pub(crate) label: String,
    /// Consumido, `0.0..=100.0`.
    pub(crate) percent: f32,
}

/// Cabecera de la columna de sesiones: título, rama y carpeta de la sesión a la vista.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SidebarHeader {
    pub(crate) title: String,
    pub(crate) branch: Option<String>,
    pub(crate) path: Option<String>,
    /// El panel está a la derecha: el borde que lo separa del contenido es el izquierdo.
    pub(crate) on_right: bool,
    /// Fin (y) de la lista de sesiones; bajo ella queda el explorador.
    pub(crate) list_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SidebarTabItem {
    /// Posición de la pestaña en `TearminalApp::tabs`.
    pub index: usize,
    /// `None` en Ajustes y visores de archivo.
    pub status: Option<SessionStatus>,
    pub title: String,
    /// Segunda línea de la pestaña lateral: contexto real (carpeta, SSH, paneles), no decoración.
    pub detail: Option<String>,
    pub is_active: bool,
    pub has_unread: bool,
    pub pane_count: usize,
    pub is_settings: bool,
    pub is_file_viewer: bool,
    /// Vista previa (no fijada): su título va subrayado
    pub is_preview: bool,
}

#[inline]
#[allow(dead_code)]
pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

pub(crate) fn tab_icon(_tab: &SidebarTabItem) -> Option<crate::ui::LucideIcon> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SettingsSection {
    #[default]
    Appearance,
    Terminal,
    Shortcuts,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AiAgentStatus {
    #[default]
    None,
    Claude,
    Agy,
    Gemini,
    Aider,
    Copilot,
    Ollama,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum EditingSettingField {
    #[default]
    FontSize,
    UiFontSize,
    LineSpacing,
    WindowOpacity,
    ScrollbackLines,
    ThemeSearch,
    GeneralSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageState {
    pub section: SettingsSection,
    pub hovered_section: Option<SettingsSection>,
    pub hovered_search_box: bool,
    pub search_input: crate::ui::TextInput,
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
    pub hovered_cursor_shape: Option<usize>,
    pub hovered_default_shell: Option<usize>,
    pub hovered_scrollback_minus: bool,
    pub hovered_scrollback_plus: bool,
    pub hovered_scrollback_box: bool,
    pub hovered_shell_integration: bool,
    pub hovered_copy_on_select: bool,
    pub hovered_status_bar: bool,
    pub hovered_show_dimensions: bool,
    pub hovered_file_explorer: bool,
    pub hovered_restore_session: bool,
    pub hovered_auto_save: bool,
    pub hovered_word_wrap: bool,
    pub hovered_ssh_integration: bool,
    pub hovered_osc52: bool,
    pub hovered_ligatures: bool,
    pub hovered_ai_limits: bool,
    pub hovered_auto_check_updates: bool,
    pub hovered_keep_awake: Option<usize>,
    pub hovered_open_file: bool,
    pub hovered_reset_defaults: bool,
    pub hovered_scrollbar: bool,
    /// Desplazamiento vertical (px) del contenido bajo la barra de navegación
    pub scroll_y: usize,
    pub editing_setting: Option<EditingSettingField>,
    pub setting_input_buffer: String,
    /// Texto del buscador de temas y primera fila visible de la lista filtrada
    pub theme_search_input: crate::ui::TextInput,
    pub theme_filter: String,
    pub filtered_themes: Vec<&'static str>,
    pub theme_scroll: usize,
    /// Ranura que edita la lista de temas: `false` = oscuro, `true` = claro
    pub theme_slot_light: bool,
    pub hovered_theme_slot: Option<usize>,
}

impl SettingsPageState {
    /// Limpia todos los estados de hover conservando la sección activa, el scroll y la edición en curso.
    /// Devuelve `true` si algún indicador de hover cambió.
    pub(crate) fn clear_hover(&mut self) -> bool {
        let changed = self.hovered_section.is_some()
            || self.hovered_theme.is_some()
            || self.hovered_theme_mode.is_some()
            || self.hovered_tab_layout.is_some()
            || self.hovered_font_minus
            || self.hovered_font_plus
            || self.hovered_font_box
            || self.hovered_ui_font_minus
            || self.hovered_ui_font_plus
            || self.hovered_ui_font_box
            || self.hovered_spacing_minus
            || self.hovered_spacing_plus
            || self.hovered_spacing_box
            || self.hovered_opacity_minus
            || self.hovered_opacity_plus
            || self.hovered_opacity_box
            || self.hovered_cursor_blink
            || self.hovered_cursor_shape.is_some()
            || self.hovered_default_shell.is_some()
            || self.hovered_scrollback_minus
            || self.hovered_scrollback_plus
            || self.hovered_scrollback_box
            || self.hovered_shell_integration
            || self.hovered_copy_on_select
            || self.hovered_status_bar
            || self.hovered_show_dimensions
            || self.hovered_file_explorer
            || self.hovered_restore_session
            || self.hovered_auto_save
            || self.hovered_word_wrap
            || self.hovered_ssh_integration
            || self.hovered_osc52
            || self.hovered_ligatures
            || self.hovered_ai_limits
            || self.hovered_auto_check_updates
            || self.hovered_keep_awake.is_some()
            || self.hovered_open_file
            || self.hovered_reset_defaults
            || self.hovered_search_box
            || self.hovered_scrollbar
            || self.hovered_theme_slot.is_some();
        if changed {
            self.hovered_section = None;
            self.hovered_theme = None;
            self.hovered_theme_mode = None;
            self.hovered_tab_layout = None;
            self.hovered_font_minus = false;
            self.hovered_font_plus = false;
            self.hovered_font_box = false;
            self.hovered_ui_font_minus = false;
            self.hovered_ui_font_plus = false;
            self.hovered_ui_font_box = false;
            self.hovered_spacing_minus = false;
            self.hovered_spacing_plus = false;
            self.hovered_spacing_box = false;
            self.hovered_opacity_minus = false;
            self.hovered_opacity_plus = false;
            self.hovered_opacity_box = false;
            self.hovered_cursor_blink = false;
            self.hovered_cursor_shape = None;
            self.hovered_default_shell = None;
            self.hovered_scrollback_minus = false;
            self.hovered_scrollback_plus = false;
            self.hovered_scrollback_box = false;
            self.hovered_shell_integration = false;
            self.hovered_copy_on_select = false;
            self.hovered_status_bar = false;
            self.hovered_show_dimensions = false;
            self.hovered_file_explorer = false;
            self.hovered_restore_session = false;
            self.hovered_auto_save = false;
            self.hovered_word_wrap = false;
            self.hovered_ssh_integration = false;
            self.hovered_osc52 = false;
            self.hovered_ligatures = false;
            self.hovered_ai_limits = false;
            self.hovered_auto_check_updates = false;
            self.hovered_keep_awake = None;
            self.hovered_open_file = false;
            self.hovered_reset_defaults = false;
            self.hovered_search_box = false;
            self.hovered_scrollbar = false;
            self.hovered_theme_slot = None;
        }
        changed
    }
}

impl Default for SettingsPageState {
    fn default() -> Self {
        Self {
            section: SettingsSection::Appearance,
            hovered_section: None,
            hovered_search_box: false,
            search_input: crate::ui::TextInput::default(),
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
            hovered_cursor_shape: None,
            hovered_default_shell: None,
            hovered_scrollback_minus: false,
            hovered_scrollback_plus: false,
            hovered_scrollback_box: false,
            hovered_shell_integration: false,
            hovered_copy_on_select: false,
            hovered_status_bar: false,
            hovered_show_dimensions: false,
            hovered_file_explorer: false,
            hovered_restore_session: false,
            hovered_auto_save: false,
            hovered_word_wrap: false,
            hovered_ssh_integration: false,
            hovered_osc52: false,
            hovered_ligatures: false,
            hovered_ai_limits: false,
            hovered_auto_check_updates: false,
            hovered_keep_awake: None,
            hovered_open_file: false,
            hovered_reset_defaults: false,
            hovered_scrollbar: false,
            scroll_y: 0,
            editing_setting: None,
            setting_input_buffer: String::new(),
            theme_search_input: crate::ui::TextInput::default(),
            theme_filter: String::new(),
            filtered_themes: crate::theme::Theme::names().collect(),
            theme_scroll: 0,
            theme_slot_light: false,
            hovered_theme_slot: None,
        }
    }
}
