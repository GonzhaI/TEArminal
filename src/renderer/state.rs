//! Tipos de estado de la interfaz que `app` construye y el renderer dibuja.

/// Estado de una sesión según OSC 133: lo único que Axon sabe de forma fiable de un comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Prompt libre y sin comandos terminados.
    Idle,
    /// Hay un comando en ejecución (un agente de CLI cuenta como tal mientras viva su proceso).
    Running,
    /// El último comando terminó con código 0.
    Done,
    /// El último comando terminó con un código distinto de 0.
    Failed,
}

impl SessionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "En espera",
            Self::Running => "En curso",
            Self::Done => "Terminó",
            Self::Failed => "Falló",
        }
    }
}

/// Proyecto del riel de la barra lateral.
#[derive(Debug, Clone)]
pub struct SidebarProject {
    pub name: String,
    /// Rama git de la carpeta del proyecto, si es un repositorio.
    pub branch: Option<String>,
    /// Estado agregado de sus sesiones (el más urgente); `None` si no tiene sesiones abiertas.
    pub status: Option<SessionStatus>,
    pub initial: char,
    /// Índice ANSI del color del proyecto.
    pub color_slot: usize,
    pub is_active: bool,
    /// Hay novedades (salida nueva) en una sesión de un proyecto que no está a la vista.
    pub has_news: bool,
    /// Sesiones sueltas: las que no pertenecen a ningún proyecto agregado.
    pub is_loose: bool,
}

/// Límite real de la cuenta mostrado como medidor en la barra de estado.
#[derive(Debug, Clone)]
pub struct StatusMeter {
    pub label: String,
    /// Consumido, `0.0..=100.0`.
    pub percent: f32,
}

/// Proyecto a la vista en la barra de pestañas superiores.
#[derive(Debug, Clone, Default)]
pub struct TopBarProject {
    pub title: String,
    pub initial: char,
    pub color_slot: usize,
    pub is_loose: bool,
    /// Otro proyecto tiene novedades.
    pub has_news: bool,
}

/// Archivo con cambios de git en la sección «Cambios» del panel.
#[derive(Debug, Clone)]
pub struct SidebarChange {
    pub name: String,
    pub path: std::path::PathBuf,
    pub status: crate::explorer::GitFileStatus,
}

/// Cabecera de la columna de sesiones: el proyecto seleccionado.
#[derive(Debug, Clone, Default)]
pub struct SidebarHeader {
    pub title: String,
    pub branch: Option<String>,
    pub path: Option<String>,
    /// El panel está a la derecha: el borde que lo separa del contenido es el izquierdo.
    pub on_right: bool,
    /// Fin (y) de la lista de sesiones; bajo ella queda la sección «Cambios» y el explorador.
    pub list_end: usize,
    pub explorer_visible: bool,
    /// Archivos con cambios (sección «Cambios» del panel); vacío fuera de un repositorio.
    pub changes: Vec<SidebarChange>,
}

#[derive(Debug, Clone)]
pub struct SidebarTabItem {
    /// Posición real de la pestaña en `AxonApp::tabs` (la lista del panel está filtrada por proyecto).
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
    ThemeSearch,
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
    pub hovered_shell_integration: bool,
    pub hovered_copy_on_select: bool,
    pub hovered_status_bar: bool,
    pub hovered_file_explorer: bool,
    pub hovered_restore_session: bool,
    pub hovered_auto_save: bool,
    pub hovered_word_wrap: bool,
    pub hovered_ssh_integration: bool,
    pub hovered_ligatures: bool,
    pub hovered_open_file: bool,
    pub hovered_reset_defaults: bool,
    pub hovered_scrollbar: bool,
    /// Desplazamiento vertical (px) del contenido bajo la barra de navegación
    pub scroll_y: usize,
    pub editing_setting: Option<EditingSettingField>,
    pub setting_input_buffer: String,
    /// Texto del buscador de temas y primera fila visible de la lista filtrada
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
    pub fn clear_hover(&mut self) -> bool {
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
            || self.hovered_shell_integration
            || self.hovered_copy_on_select
            || self.hovered_status_bar
            || self.hovered_file_explorer
            || self.hovered_restore_session
            || self.hovered_auto_save
            || self.hovered_word_wrap
            || self.hovered_ssh_integration
            || self.hovered_ligatures
            || self.hovered_open_file
            || self.hovered_reset_defaults
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
            self.hovered_shell_integration = false;
            self.hovered_copy_on_select = false;
            self.hovered_status_bar = false;
            self.hovered_file_explorer = false;
            self.hovered_restore_session = false;
            self.hovered_auto_save = false;
            self.hovered_word_wrap = false;
            self.hovered_ssh_integration = false;
            self.hovered_ligatures = false;
            self.hovered_open_file = false;
            self.hovered_reset_defaults = false;
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
            hovered_shell_integration: false,
            hovered_copy_on_select: false,
            hovered_status_bar: false,
            hovered_file_explorer: false,
            hovered_restore_session: false,
            hovered_auto_save: false,
            hovered_word_wrap: false,
            hovered_ssh_integration: false,
            hovered_ligatures: false,
            hovered_open_file: false,
            hovered_reset_defaults: false,
            hovered_scrollbar: false,
            scroll_y: 0,
            editing_setting: None,
            setting_input_buffer: String::new(),
            theme_filter: String::new(),
            filtered_themes: crate::theme::Theme::names().collect(),
            theme_scroll: 0,
            theme_slot_light: false,
            hovered_theme_slot: None,
        }
    }
}
