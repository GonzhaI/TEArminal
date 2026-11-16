use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TearminalConfig {
    pub(crate) appearance: AppearanceConfig,
    pub(crate) terminal: TerminalConfig,
    #[serde(default)]
    pub(crate) database: DatabaseConfig,
    #[serde(default)]
    pub(crate) performance: PerformanceConfig,
    #[serde(default)]
    pub(crate) shortcuts: ShortcutsConfig,
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
    #[serde(default = "default_line_spacing")]
    pub(crate) line_spacing: f32,
    #[serde(default = "default_theme_mode")]
    pub(crate) theme_mode: String,
    #[serde(default = "default_true")]
    pub(crate) show_status_bar: bool,
    #[serde(default = "default_false")]
    pub(crate) show_file_explorer: bool,
    /// Muestra las dimensiones de la rejilla (columnas × filas) en la esquina inferior derecha.
    #[serde(default = "default_false")]
    pub(crate) show_dimensions: bool,
    /// Ancho del panel lateral / explorador de archivos (en píxeles)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "file_explorer_width",
        alias = "explorer_width"
    )]
    pub(crate) panel_width: Option<usize>,
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
    /// Al abrir `ssh` desde TEArminal, el shell remoto (bash) informa de la rama git y el directorio
    #[serde(default = "default_true")]
    pub(crate) ssh_integration: bool,
    /// Deja que los programas de la terminal escriban en el portapapeles de Windows (OSC 52, máx. 100 KB).
    /// Activo por defecto (la lectura '?' siempre está bloqueada por seguridad).
    #[serde(default = "default_true")]
    pub(crate) osc52: bool,
    /// Ligaduras tipográficas de programación (->, !=, ==, =>...)
    #[serde(default = "default_true")]
    pub(crate) ligatures: bool,
    /// Forma del cursor: "block", "beam" / "bar", "underline"
    #[serde(default = "default_cursor_shape")]
    pub(crate) cursor_shape: String,
    /// Comprobación automática de nuevas versiones al iniciar TEArminal
    #[serde(default = "default_true")]
    pub(crate) auto_check_updates: bool,
    /// Consulta las cuotas reales de Claude (api.anthropic.com/api/oauth/usage) con la sesión de Claude Code
    /// (`~/.claude/.credentials.json`, solo lectura). Con `false` TEArminal no lee esa credencial ni hace la llamada.
    #[serde(default = "default_true")]
    pub(crate) ai_limits: bool,
    /// Modo para mantener la pantalla encendida: "off" / "apagado", "agent" / "agente", "active" / "activo".
    #[serde(default)]
    pub(crate) keep_awake: KeepAwakeMode,
    /// Sincronización vertical (VSync). Activo por defecto; desactivarlo permite la mínima latencia
    /// absoluta en pantallas de alta tasa de refresco o compatibles con VRR / G-Sync / FreeSync.
    #[serde(default = "default_true")]
    pub(crate) vsync: bool,
    /// Animaciones fluidas de interfaz: cursor elástico, transiciones de pestañas,
    /// desplazamiento amortiguado y apertura suave de modales/paleta.
    #[serde(default = "default_true")]
    pub(crate) animations: bool,
}

/// Opciones globales y valores por defecto para conexiones y consultas de bases de datos (DISENO §6).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct DatabaseConfig {
    /// Abrir conexiones de producción en modo solo lectura por defecto.
    #[serde(default = "default_true")]
    pub(crate) prod_read_only: bool,
    /// Duración de la ventana de escritura habilitada en producción (minutos).
    #[serde(default = "default_write_window_minutes")]
    pub(crate) write_window_minutes: u32,
    /// Exigir confirmación escrita tecleando el nombre de la conexión antes de aplicar en producción.
    #[serde(default = "default_true")]
    pub(crate) written_confirmation: bool,
    /// Advertir antes de ejecutar sentencias DELETE o UPDATE sin cláusula WHERE.
    #[serde(default = "default_true")]
    pub(crate) warn_no_where: bool,
    /// Aplicar cambios pendientes dentro de una transacción explícita (rollback si algo falla).
    #[serde(default = "default_true")]
    pub(crate) enforce_transaction: bool,
    /// Guardar filas originales en archivo local antes de aplicar para permitir deshacer.
    #[serde(default = "default_true")]
    pub(crate) backup_original_rows: bool,
    /// Comprobar permisos en Windows y advertir si una clave privada `.pem` es legible por otros usuarios.
    #[serde(default = "default_true")]
    pub(crate) warn_unsafe_pem_permissions: bool,
    /// Días de antelación para advertir sobre certificados TLS por vencer.
    #[serde(default = "default_cert_warn_days")]
    pub(crate) cert_warn_days: u32,
    /// Carpeta para certificados `.pem` (solo se guarda la ruta, nunca su contenido).
    #[serde(default = "default_certs_folder")]
    pub(crate) certs_folder: String,
    /// Filas por página en el visor de datos de tablas.
    #[serde(default = "default_page_size")]
    pub(crate) page_size: usize,
    /// Tiempo máximo de espera para una consulta (segundos) antes de cancelarla.
    #[serde(default = "default_query_timeout_seconds")]
    pub(crate) query_timeout_seconds: u32,
    /// Límite máximo de filas devueltas para prevenir sobrecarga de memoria.
    #[serde(default = "default_query_row_limit")]
    pub(crate) query_row_limit: usize,
    /// Umbral en milisegundos para marcar una consulta como lenta.
    #[serde(default = "default_slow_query_threshold_ms")]
    pub(crate) slow_query_threshold_ms: u32,
    /// Autocompletado de tablas y columnas basado en el esquema de la base de datos.
    #[serde(default = "default_true")]
    pub(crate) autocomplete: bool,
    /// Guardar credenciales de forma segura en el Administrador de credenciales de Windows.
    #[serde(default = "default_true")]
    pub(crate) save_passwords_in_credential_manager: bool,
    /// Guardar historial de consultas ejecutadas (solo texto SQL y duración, sin parámetros).
    #[serde(default = "default_true")]
    pub(crate) save_query_history: bool,
}

/// Configuración de medidores en la barra de estado y monitor del sistema (F3 / F8).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct PerformanceConfig {
    /// Mostrar medidor de CPU en la barra de estado.
    #[serde(default = "default_true")]
    pub(crate) show_cpu: bool,
    /// Mostrar medidor de RAM en la barra de estado.
    #[serde(default = "default_true")]
    pub(crate) show_ram: bool,
    /// Mostrar medidor de GPU en la barra de estado.
    #[serde(default = "default_true")]
    pub(crate) show_gpu: bool,
    /// Mostrar medidor de Disco en la barra de estado.
    #[serde(default = "default_true")]
    pub(crate) show_disk: bool,
    /// Mostrar medidor de Red en la barra de estado.
    #[serde(default = "default_false")]
    pub(crate) show_net: bool,
    /// Intervalo de muestreo del monitor (en milisegundos).
    #[serde(default = "default_sample_interval_ms")]
    pub(crate) sample_interval_ms: u64,
    /// Activar alertas visuales cuando un recurso supera el umbral crítico.
    #[serde(default = "default_false")]
    pub(crate) enable_alerts: bool,
    /// Umbral porcentual de alerta para CPU (0-100).
    #[serde(default = "default_alert_threshold")]
    pub(crate) cpu_alert_threshold: u8,
    /// Umbral porcentual de alerta para RAM (0-100).
    #[serde(default = "default_alert_threshold")]
    pub(crate) ram_alert_threshold: u8,
    /// Umbral porcentual de alerta para Disco (0-100).
    #[serde(default = "default_alert_threshold")]
    pub(crate) disk_alert_threshold: u8,
    /// Ejecutables de agentes CLI IA personalizados para monitorización e `init.ps1`.
    #[serde(default = "default_agent_executables")]
    pub(crate) agent_executables: Vec<String>,
}

pub(crate) fn default_agent_executables() -> Vec<String> {
    [
        "claude",
        "agy",
        "gemini",
        "opencode",
        "aider",
        "codex",
        "goose",
        "copilot",
        "cursor-agent",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(crate) fn sanitize_agent_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let base = lower
        .strip_suffix(".exe")
        .or_else(|| lower.strip_suffix(".cmd"))
        .or_else(|| lower.strip_suffix(".bat"))
        .unwrap_or(&lower);
    let valid = !base.is_empty()
        && base.len() <= 64
        && base.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    valid.then(|| base.to_string())
}

impl PerformanceConfig {
    /// Nombres de ejecutables de agentes normalizados y sin duplicados (hasta 32).
    pub(crate) fn normalized_agent_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for raw in &self.agent_executables {
            if let Some(clean) = sanitize_agent_name(raw) {
                if !out.contains(&clean) && out.len() < 32 {
                    out.push(clean);
                }
            }
        }
        out
    }

    pub(crate) fn agent_executables_csv(&self) -> String {
        self.normalized_agent_names().join(", ")
    }

    pub(crate) fn set_agent_executables_csv(&mut self, csv: &str) {
        let mut next = Vec::new();
        for token in csv.split([',', ';', ' ', '\t']) {
            if let Some(clean) = sanitize_agent_name(token) {
                if !next.contains(&clean) && next.len() < 32 {
                    next.push(clean);
                }
            }
        }
        self.agent_executables = next;
    }
}

/// Acciones globales editables desde Ajustes -> Atajos de teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutAction {
    NewTab,
    CloseTab,
    NextTab,
    Find,
    CommandPalette,
    Settings,
    Fullscreen,
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    ToggleSidebar,
    RefreshExplorer,
}

impl ShortcutAction {
    pub(crate) const ALL: [Self; 12] = [
        Self::NewTab,
        Self::CloseTab,
        Self::NextTab,
        Self::Find,
        Self::CommandPalette,
        Self::Settings,
        Self::Fullscreen,
        Self::SplitHorizontal,
        Self::SplitVertical,
        Self::ClosePane,
        Self::ToggleSidebar,
        Self::RefreshExplorer,
    ];

    /// Distribución de las 16 filas en «Ajustes -> Atajos» (None = informativo/rango fijo).
    pub(crate) const SETTINGS_SLOTS: [Option<Self>; 16] = [
        Some(Self::NewTab),
        Some(Self::CloseTab),
        Some(Self::NextTab),
        None,
        Some(Self::Find),
        Some(Self::CommandPalette),
        Some(Self::Settings),
        Some(Self::Fullscreen),
        Some(Self::SplitHorizontal),
        Some(Self::SplitVertical),
        None,
        Some(Self::ClosePane),
        Some(Self::ToggleSidebar),
        Some(Self::RefreshExplorer),
        None,
        None,
    ];

    pub(crate) fn default_chord(self) -> &'static str {
        match self {
            Self::NewTab => "Ctrl+T",
            Self::CloseTab => "Ctrl+W",
            Self::NextTab => "Ctrl+Tab",
            Self::Find => "Ctrl+F",
            Self::CommandPalette => "Ctrl+Shift+P",
            Self::Settings => "Ctrl+,",
            Self::Fullscreen => "F11",
            Self::SplitHorizontal => "Alt+Shift+D",
            Self::SplitVertical => "Alt+Shift+V",
            Self::ClosePane => "Ctrl+Shift+W",
            Self::ToggleSidebar => "Ctrl+B",
            Self::RefreshExplorer => "F5",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::NewTab => "Nueva terminal o pestaña",
            Self::CloseTab => "Cerrar pestaña o panel activo",
            Self::NextTab => "Siguiente pestaña (o Ctrl+AvPág)",
            Self::Find => "Buscar en terminal o explorador",
            Self::CommandPalette => "Paleta de comandos (o F1)",
            Self::Settings => "Abrir o cerrar Ajustes",
            Self::Fullscreen => "Pantalla completa (o Alt+Enter)",
            Self::SplitHorizontal => "Dividir debajo (o Alt+Shift+-)",
            Self::SplitVertical => "Dividir al lado (o Alt+Shift++)",
            Self::ClosePane => "Cerrar panel dividido",
            Self::ToggleSidebar => "Alternar barra lateral",
            Self::RefreshExplorer => "Refrescar explorador y Git",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedKeyChord {
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
    pub(crate) alt: bool,
    pub(crate) key: String,
}

impl ParsedKeyChord {
    pub(crate) fn normalize_key_token(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let upper = trimmed.to_ascii_uppercase();
        let norm = match upper.as_str() {
            "CTRL" | "CONTROL" | "SHIFT" | "ALT" | "SUPER" | "WIN" => return None,
            "COMMA" => ",".to_string(),
            "PERIOD" => ".".to_string(),
            "PLUS" | "EQUAL" => "+".to_string(),
            "MINUS" => "-".to_string(),
            "TAB" => "Tab".to_string(),
            "ENTER" | "RETURN" => "Enter".to_string(),
            "SPACE" => "Space".to_string(),
            "ESC" | "ESCAPE" => "Escape".to_string(),
            "BACKSPACE" => "Backspace".to_string(),
            "UP" | "ARROWUP" => "Up".to_string(),
            "DOWN" | "ARROWDOWN" => "Down".to_string(),
            "LEFT" | "ARROWLEFT" => "Left".to_string(),
            "RIGHT" | "ARROWRIGHT" => "Right".to_string(),
            "PAGEUP" => "PageUp".to_string(),
            "PAGEDOWN" => "PageDown".to_string(),
            "HOME" => "Home".to_string(),
            "END" => "End".to_string(),
            "INSERT" => "Insert".to_string(),
            "DELETE" => "Delete".to_string(),
            _ if upper.starts_with('F')
                && upper[1..]
                    .parse::<u8>()
                    .is_ok_and(|n| (1..=12).contains(&n)) =>
            {
                upper
            }
            _ => {
                let chars: Vec<char> = trimmed.chars().collect();
                if chars.len() == 1 && !chars[0].is_control() && !chars[0].is_whitespace() {
                    chars[0].to_ascii_uppercase().to_string()
                } else {
                    return None;
                }
            }
        };
        Some(norm)
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut key: Option<String> = None;
        for part in raw.split('+') {
            let p = part.trim();
            if p.is_empty() {
                // Caso especial como "Alt+Shift++" donde el último '+' dejó token vacío
                if raw.trim_end().ends_with("++") && key.is_none() {
                    key = Some("+".to_string());
                }
                continue;
            }
            match p.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => ctrl = true,
                "SHIFT" => shift = true,
                "ALT" => alt = true,
                _ => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(Self::normalize_key_token(p)?);
                }
            }
        }
        Some(Self {
            ctrl,
            shift,
            alt,
            key: key?,
        })
    }

    pub(crate) fn format_compact(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        parts.push(self.key.clone());
        parts.join("+")
    }

    pub(crate) fn format_keycaps(&self) -> String {
        self.format_compact().replace('+', " + ")
    }
}

fn default_new_tab() -> String {
    ShortcutAction::NewTab.default_chord().to_string()
}
fn default_close_tab() -> String {
    ShortcutAction::CloseTab.default_chord().to_string()
}
fn default_next_tab() -> String {
    ShortcutAction::NextTab.default_chord().to_string()
}
fn default_find() -> String {
    ShortcutAction::Find.default_chord().to_string()
}
fn default_command_palette() -> String {
    ShortcutAction::CommandPalette.default_chord().to_string()
}
fn default_settings_shortcut() -> String {
    ShortcutAction::Settings.default_chord().to_string()
}
fn default_fullscreen() -> String {
    ShortcutAction::Fullscreen.default_chord().to_string()
}
fn default_split_horizontal() -> String {
    ShortcutAction::SplitHorizontal.default_chord().to_string()
}
fn default_split_vertical() -> String {
    ShortcutAction::SplitVertical.default_chord().to_string()
}
fn default_close_pane() -> String {
    ShortcutAction::ClosePane.default_chord().to_string()
}
fn default_toggle_sidebar() -> String {
    ShortcutAction::ToggleSidebar.default_chord().to_string()
}
fn default_refresh_explorer() -> String {
    ShortcutAction::RefreshExplorer.default_chord().to_string()
}

/// Combinaciones de atajos configurables guardadas en `tearminal.toml`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct ShortcutsConfig {
    #[serde(default = "default_new_tab")]
    pub(crate) new_tab: String,
    #[serde(default = "default_close_tab")]
    pub(crate) close_tab: String,
    #[serde(default = "default_next_tab")]
    pub(crate) next_tab: String,
    #[serde(default = "default_find")]
    pub(crate) find: String,
    #[serde(default = "default_command_palette")]
    pub(crate) command_palette: String,
    #[serde(default = "default_settings_shortcut")]
    pub(crate) settings: String,
    #[serde(default = "default_fullscreen")]
    pub(crate) fullscreen: String,
    #[serde(default = "default_split_horizontal")]
    pub(crate) split_horizontal: String,
    #[serde(default = "default_split_vertical")]
    pub(crate) split_vertical: String,
    #[serde(default = "default_close_pane")]
    pub(crate) close_pane: String,
    #[serde(default = "default_toggle_sidebar")]
    pub(crate) toggle_sidebar: String,
    #[serde(default = "default_refresh_explorer")]
    pub(crate) refresh_explorer: String,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            new_tab: ShortcutAction::NewTab.default_chord().to_string(),
            close_tab: ShortcutAction::CloseTab.default_chord().to_string(),
            next_tab: ShortcutAction::NextTab.default_chord().to_string(),
            find: ShortcutAction::Find.default_chord().to_string(),
            command_palette: ShortcutAction::CommandPalette.default_chord().to_string(),
            settings: ShortcutAction::Settings.default_chord().to_string(),
            fullscreen: ShortcutAction::Fullscreen.default_chord().to_string(),
            split_horizontal: ShortcutAction::SplitHorizontal.default_chord().to_string(),
            split_vertical: ShortcutAction::SplitVertical.default_chord().to_string(),
            close_pane: ShortcutAction::ClosePane.default_chord().to_string(),
            toggle_sidebar: ShortcutAction::ToggleSidebar.default_chord().to_string(),
            refresh_explorer: ShortcutAction::RefreshExplorer.default_chord().to_string(),
        }
    }
}

impl ShortcutsConfig {
    pub(crate) fn get(&self, action: ShortcutAction) -> &str {
        match action {
            ShortcutAction::NewTab => &self.new_tab,
            ShortcutAction::CloseTab => &self.close_tab,
            ShortcutAction::NextTab => &self.next_tab,
            ShortcutAction::Find => &self.find,
            ShortcutAction::CommandPalette => &self.command_palette,
            ShortcutAction::Settings => &self.settings,
            ShortcutAction::Fullscreen => &self.fullscreen,
            ShortcutAction::SplitHorizontal => &self.split_horizontal,
            ShortcutAction::SplitVertical => &self.split_vertical,
            ShortcutAction::ClosePane => &self.close_pane,
            ShortcutAction::ToggleSidebar => &self.toggle_sidebar,
            ShortcutAction::RefreshExplorer => &self.refresh_explorer,
        }
    }

    pub(crate) fn set(&mut self, action: ShortcutAction, chord: String) {
        match action {
            ShortcutAction::NewTab => self.new_tab = chord,
            ShortcutAction::CloseTab => self.close_tab = chord,
            ShortcutAction::NextTab => self.next_tab = chord,
            ShortcutAction::Find => self.find = chord,
            ShortcutAction::CommandPalette => self.command_palette = chord,
            ShortcutAction::Settings => self.settings = chord,
            ShortcutAction::Fullscreen => self.fullscreen = chord,
            ShortcutAction::SplitHorizontal => self.split_horizontal = chord,
            ShortcutAction::SplitVertical => self.split_vertical = chord,
            ShortcutAction::ClosePane => self.close_pane = chord,
            ShortcutAction::ToggleSidebar => self.toggle_sidebar = chord,
            ShortcutAction::RefreshExplorer => self.refresh_explorer = chord,
        }
    }

    pub(crate) fn parsed(&self, action: ShortcutAction) -> ParsedKeyChord {
        ParsedKeyChord::parse(self.get(action))
            .or_else(|| ParsedKeyChord::parse(action.default_chord()))
            .unwrap_or(ParsedKeyChord {
                ctrl: false,
                shift: false,
                alt: false,
                key: "F11".to_string(),
            })
    }

    pub(crate) fn display_keycaps(&self, action: ShortcutAction) -> String {
        self.parsed(action).format_keycaps()
    }

    pub(crate) fn find_conflict(
        &self,
        target: ShortcutAction,
        candidate: &ParsedKeyChord,
    ) -> Option<ShortcutAction> {
        ShortcutAction::ALL
            .into_iter()
            .find(|&other| other != target && &self.parsed(other) == candidate)
    }

    pub(crate) fn slot_info(&self, idx: usize) -> (String, &'static str, Option<ShortcutAction>) {
        match idx {
            3 => (
                "Ctrl + 1..9".to_string(),
                "Ir a pestaña 1–8 o última (9)",
                None,
            ),
            10 => (
                "Alt + Flechas".to_string(),
                "Mover el foco entre paneles",
                None,
            ),
            14 => (
                "Ctrl + C / V".to_string(),
                "Copiar o pegar en terminal",
                None,
            ),
            15 => ("Ctrl + Backspace".to_string(), "Borrar palabra atrás", None),
            _ => {
                if let Some(Some(action)) = ShortcutAction::SETTINGS_SLOTS.get(idx) {
                    (
                        self.display_keycaps(*action),
                        action.description(),
                        Some(*action),
                    )
                } else {
                    (String::new(), "", None)
                }
            }
        }
    }
}

/// Modos de suspensión y pantalla encendida (keep-awake) para Windows.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KeepAwakeMode {
    /// Ajustes por defecto del dispositivo (suspensión / bloqueo habitual).
    #[default]
    #[serde(alias = "off", alias = "apagado", alias = "default")]
    Off,
    /// Mantiene la pantalla encendida solo mientras un agente de IA esté trabajando.
    #[serde(alias = "agent", alias = "agente")]
    Agent,
    /// Mantiene la pantalla y el sistema encendidos siempre.
    #[serde(alias = "active", alias = "activo", alias = "always")]
    Active,
}

impl KeepAwakeMode {
    /// Pasa cíclicamente al siguiente modo: Apagado -> Agente -> Activo -> Apagado.
    #[cfg(test)]
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Off => Self::Agent,
            Self::Agent => Self::Active,
            Self::Active => Self::Off,
        }
    }

    /// Etiqueta corta para la barra de estado inferior.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Off => "Apagado",
            Self::Agent => "Agente",
            Self::Active => "Activo",
        }
    }

    /// Descripción del modo para el selector emergente de la barra de estado.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Off => "Suspensión habitual del sistema",
            Self::Agent => "Evita suspensión con agentes activos",
            Self::Active => "Mantiene pantalla encendida siempre",
        }
    }
}

fn default_font_size() -> f32 {
    16.0
}

fn default_ui_font_size() -> f32 {
    14.0
}

fn default_line_spacing() -> f32 {
    1.0
}

fn default_cursor_shape() -> String {
    "block".to_string()
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

fn default_theme_mode() -> String {
    "dark".to_string()
}

fn default_write_window_minutes() -> u32 {
    10
}

fn default_cert_warn_days() -> u32 {
    30
}

fn default_certs_folder() -> String {
    "~/.tearminal/certs".to_string()
}

fn default_page_size() -> usize {
    50
}

fn default_query_timeout_seconds() -> u32 {
    30
}

fn default_query_row_limit() -> usize {
    10_000
}

fn default_slow_query_threshold_ms() -> u32 {
    300
}

fn default_sample_interval_ms() -> u64 {
    2000
}

fn default_alert_threshold() -> u8 {
    90
}

impl AppearanceConfig {
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

    /// Configuraciones anteriores guardaban un único `theme` (en modo claro, uno claro)
    /// o valores anteriores de tipografía unificada (16/16 o 13/13).
    fn migrate(&mut self) {
        if ((self.font_size - 16.0).abs() < 0.01 && (self.ui_font_size - 16.0).abs() < 0.01)
            || ((self.font_size - 13.0).abs() < 0.01 && (self.ui_font_size - 13.0).abs() < 0.01)
        {
            self.font_size = 16.0;
            self.ui_font_size = 14.0;
            self.line_spacing = 1.0;
        }
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

impl Default for TearminalConfig {
    fn default() -> Self {
        let shell = detect_preferred_shell();
        Self {
            appearance: AppearanceConfig {
                font_family: "JetBrains Mono Nerd Font".to_string(),
                font_path: None,
                font_size: 16.0,
                ui_font_size: 14.0,
                window_opacity: 1.0,
                theme: crate::theme::Theme::DEFAULT_DARK.to_string(),
                theme_light: crate::theme::Theme::DEFAULT_LIGHT.to_string(),
                line_spacing: 1.0,
                theme_mode: "dark".to_string(),
                show_status_bar: true,
                show_file_explorer: false,
                show_dimensions: false,
                panel_width: None,
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
                cursor_shape: "block".to_string(),
                auto_check_updates: true,
                ai_limits: true,
                keep_awake: KeepAwakeMode::Off,
                vsync: true,
                animations: true,
            },
            database: DatabaseConfig::default(),
            performance: PerformanceConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        TearminalConfig::default().appearance
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        TearminalConfig::default().terminal
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            prod_read_only: true,
            write_window_minutes: 10,
            written_confirmation: true,
            warn_no_where: true,
            enforce_transaction: true,
            backup_original_rows: true,
            warn_unsafe_pem_permissions: true,
            cert_warn_days: 30,
            certs_folder: "~/.tearminal/certs".to_string(),
            page_size: 50,
            query_timeout_seconds: 30,
            query_row_limit: 10_000,
            slow_query_threshold_ms: 300,
            autocomplete: true,
            save_passwords_in_credential_manager: true,
            save_query_history: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            show_cpu: true,
            show_ram: true,
            show_gpu: true,
            show_disk: true,
            show_net: false,
            sample_interval_ms: 2000,
            enable_alerts: false,
            cpu_alert_threshold: 90,
            ram_alert_threshold: 90,
            disk_alert_threshold: 90,
            agent_executables: default_agent_executables(),
        }
    }
}

pub(crate) fn find_pwsh_path() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // 1. Comprobar rutas estándar de instalación de PowerShell 7 en Program Files
        let mut candidates = Vec::new();

        if let Ok(pf) = std::env::var("ProgramFiles") {
            let base = PathBuf::from(pf);
            candidates.push(base.join("PowerShell").join("7").join("pwsh.exe"));
            candidates.push(base.join("PowerShell").join("7-preview").join("pwsh.exe"));
        }
        if let Ok(pf64) = std::env::var("ProgramW6432") {
            let base = PathBuf::from(pf64);
            candidates.push(base.join("PowerShell").join("7").join("pwsh.exe"));
        }
        candidates.push(PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe"));

        // 2. Comprobar instalación por usuario (WinGet/MSI sin privilegios admin o Scoop)
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("PowerShell")
                    .join("7")
                    .join("pwsh.exe"),
            );
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            candidates.push(
                PathBuf::from(user_profile)
                    .join("scoop")
                    .join("apps")
                    .join("pwsh")
                    .join("current")
                    .join("pwsh.exe"),
            );
        }

        for path in candidates {
            if path.is_file() {
                return Some(path.to_string_lossy().to_string());
            }
        }

        // 3. Buscar pwsh.exe en el PATH del sistema mediante SearchPathW,
        // excluyendo expresamente los alias de Microsoft Store (WindowsApps) que bloquean ConPTY
        if let Some(path) = search_path_native("pwsh.exe") {
            let path_str = path.to_string_lossy();
            if !path_str.to_ascii_lowercase().contains("windowsapps") && path.is_file() {
                return Some(path_str.to_string());
            }
        }
    }
    None
}

pub(crate) fn detect_preferred_shell() -> String {
    find_pwsh_path().unwrap_or_else(|| "powershell.exe".to_string())
}

#[cfg(target_os = "windows")]
pub(crate) fn search_path_native(file_name: &str) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::SearchPathW;

    let wide_file: Vec<u16> = OsStr::new(file_name).encode_wide().chain(Some(0)).collect();
    let mut buf = [0u16; 512];
    let len = unsafe {
        SearchPathW(
            std::ptr::null(),
            wide_file.as_ptr(),
            std::ptr::null(),
            buf.len() as u32,
            buf.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    if len > 0 && (len as usize) < buf.len() {
        use std::os::windows::ffi::OsStringExt;
        let os_str = std::ffi::OsString::from_wide(&buf[..len as usize]);
        Some(PathBuf::from(os_str))
    } else {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("error de E/S al gestionar la configuración: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de serialización TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("error de deserialización TOML: {0}")]
    Deserialize(#[from] toml::de::Error),
    #[error("tearminal.toml tiene errores: corrígelo antes de guardar ajustes")]
    InvalidExistingConfig,
}

impl TearminalConfig {
    pub(crate) fn config_path() -> PathBuf {
        // Los tests no leen ni escriben la configuración real del usuario ni el tearminal.toml del repositorio
        if cfg!(test) {
            return dirs_or_fallback().join("tearminal.toml");
        }

        // 1. Solo en desarrollo: tearminal.toml del directorio actual. En release es un riesgo:
        //    un repo clonado con `default_shell` malicioso ejecutaría su programa al abrir TEArminal ahí.
        if cfg!(debug_assertions) {
            let local = PathBuf::from("tearminal.toml");
            if local.exists() {
                return local;
            }
        }

        // 2. Prioridad: tearminal.toml junto al ejecutable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let exe_cfg = parent.join("tearminal.toml");
                if exe_cfg.exists() {
                    return exe_cfg;
                }
            }
        }

        // 3. Fallback: Directorio de configuración de usuario en AppData
        let base_dir = dirs_or_fallback();
        base_dir.join("tearminal.toml")
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
    /// `tearminal.toml.bak` y usa los valores por defecto solo en memoria.
    fn load_from(path: &std::path::Path) -> Self {
        match fs::File::open(path) {
            Ok(file) => match Self::from_reader(BufReader::new(file)) {
                Ok(cfg) => cfg,
                Err(e) => {
                    crate::logger::log(&format!(
                        "-> tearminal.toml inválido ({e}); copia en tearminal.toml.bak, usando valores por defecto"
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
        // Un `tearminal.toml` que no se puede interpretar se carga con los valores por defecto; guardar encima
        // destruiría lo que el usuario escribió (queda también la copia `.bak`)
        if let Ok(existing) = fs::read_to_string(path) {
            if toml::from_str::<TearminalConfig>(&existing).is_err() {
                crate::logger::log(
                    "-> tearminal.toml inválido: no se guardan los ajustes hasta corregirlo",
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

/// Avisa en pantalla de que `tearminal.toml` no se pudo interpretar (TEArminal usa el subsistema "windows": sin esto el
/// usuario solo vería sus ajustes ignorados). En tests no hay ventana.
#[cfg(not(test))]
fn notify_invalid_config(path: &std::path::Path, err: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    let text = wide(&format!(
        "TEArminal no pudo leer {}\n\n{err}\n\nSe usan los valores por defecto y no se guardarán ajustes hasta que \
         lo corrijas. Se dejó una copia en tearminal.toml.bak.",
        path.display()
    ));
    let title = wide("TEArminal");
    // SAFETY: cadenas UTF-16 terminadas en nulo que viven durante la llamada
    unsafe { MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING) };
}

#[cfg(test)]
fn notify_invalid_config(_path: &std::path::Path, _err: &str) {}

/// Carpeta de configuración y sesión del usuario (`%APPDATA%\tearminal\tearminal\config`)
pub(crate) fn dirs_or_fallback() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("tearminal-test-{}", std::process::id()));
    }
    if let Some(dirs) = directories::ProjectDirs::from("com", "tearminal", "tearminal") {
        dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("tearminal_config")
    }
}

/// Carpeta de caché local del usuario (`%LOCALAPPDATA%\tearminal\tearminal\cache`)
pub(crate) fn cache_dir_or_fallback() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("tearminal-test-cache-{}", std::process::id()));
    }
    if let Some(dirs) = directories::ProjectDirs::from("com", "tearminal", "tearminal") {
        dirs.cache_dir().to_path_buf()
    } else {
        PathBuf::from("tearminal_cache")
    }
}

/// Módulos principales accesibles desde el riel izquierdo de la aplicación (DISENO §4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AppModule {
    #[default]
    Spaces,
    Files,
    Git,
    Data,
    System,
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
    /// Nombres de los Spaces, en orden; `TabSession::space` indexa esta lista. Vacía en sesiones
    /// antiguas: todas las pestañas caen en un único Space.
    #[serde(default)]
    pub(crate) spaces: Vec<String>,
    /// Panel de Spaces colapsado a riel (retrocompatibilidad)
    #[serde(default)]
    pub(crate) spaces_collapsed: bool,
    /// Módulo activo en el riel de navegación (DISENO §4)
    #[serde(default)]
    pub(crate) active_module: AppModule,
    /// Panel lateral contextual colapsado a riel
    #[serde(default)]
    pub(crate) sidebar_collapsed: bool,
    pub(crate) tabs: Vec<TabSession>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub(crate) struct TabSession {
    pub(crate) title: String,
    pub(crate) layout: PaneSession,
    /// Panel activo: posición entre las hojas de `layout`, de izquierda a derecha
    #[serde(default)]
    pub(crate) active_pane: usize,
    /// Posición del Space de la pestaña en `WindowSession::spaces`
    #[serde(default)]
    pub(crate) space: usize,
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
            spaces: Vec::new(),
            spaces_collapsed: false,
            active_module: AppModule::Spaces,
            sidebar_collapsed: false,
            tabs: Vec::new(),
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
        let mut a = TearminalConfig::default().appearance;
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

    /// Claves que ya no existen (p. ej. `bracketed_paste`) en un `tearminal.toml` antiguo se ignoran, no lo invalidan
    #[test]
    fn test_removed_keys_in_old_configs_are_ignored() {
        let text = toml::to_string_pretty(&TearminalConfig::default())
            .unwrap()
            .replace("[terminal]\n", "[terminal]\nbracketed_paste = true\n");
        assert!(text.contains("bracketed_paste"));
        assert!(toml::from_str::<TearminalConfig>(&text).is_ok());
    }

    /// Un `tearminal.toml` ilegible nunca se sobrescribe al guardar ajustes
    #[test]
    fn test_save_never_overwrites_an_invalid_config() {
        let dir = std::env::temp_dir().join(format!("tearminal-badcfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tearminal.toml");
        fs::write(&path, "theme = [esto no es toml").unwrap();
        assert!(TearminalConfig::default().save_to(&path).is_err());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "theme = [esto no es toml"
        );
        fs::write(
            &path,
            toml::to_string_pretty(&TearminalConfig::default()).unwrap(),
        )
        .unwrap();
        assert!(TearminalConfig::default().save_to(&path).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_legacy_single_theme_is_migrated() {
        let dir = std::env::temp_dir().join(format!("tearminal-mig-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tearminal.toml");
        let load = |theme: &str| {
            // Configuración completa pero sin `theme_light`, como la escribían las versiones anteriores
            let old: String = toml::to_string_pretty(&TearminalConfig::default())
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
            TearminalConfig::load_from(&path).appearance
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
        let dir = std::env::temp_dir().join(format!("tearminal-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tearminal.toml");

        // Inexistente: se crea con los valores por defecto
        TearminalConfig::load_from(&path);
        assert!(path.exists());

        // Inválido: el archivo queda intacto y se guarda copia
        let broken = "# mi nota\n[appearance\nfont_size = \"";
        fs::write(&path, broken).unwrap();
        TearminalConfig::load_from(&path);
        assert_eq!(fs::read_to_string(&path).unwrap(), broken);
        assert_eq!(
            fs::read_to_string(dir.join("tearminal.toml.bak")).unwrap(),
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
                spaces: vec!["Users".into(), "Trabajo".into()],
                spaces_collapsed: true,
                active_module: AppModule::Spaces,
                sidebar_collapsed: true,
                tabs: vec![
                    TabSession {
                        title: "Terminal 1".into(),
                        layout: leaf("C:/Users"),
                        active_pane: 0,
                        space: 0,
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
                        space: 1,
                    },
                ],
            }],
        };
        let text = toml::to_string_pretty(&state).unwrap();
        assert_eq!(toml::from_str::<SessionState>(&text).unwrap(), state);
    }

    #[test]
    fn test_osc52_is_on_by_default_also_for_old_configs() {
        assert!(TearminalConfig::default().terminal.osc52);
        let mut old = toml::to_string(&TearminalConfig::default()).unwrap();
        old = old
            .lines()
            .filter(|l| !l.starts_with("osc52"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!old.contains("osc52"));
        assert!(
            toml::from_str::<TearminalConfig>(&old)
                .unwrap()
                .terminal
                .osc52
        );
    }

    #[test]
    fn test_config_gpu_key_is_accepted_and_ignored() {
        // Un tearminal.toml antiguo con `gpu = ...` sigue cargando sin error aunque la clave se haya eliminado
        let text = toml::to_string_pretty(&TearminalConfig::default())
            .unwrap()
            .replace("[terminal]\n", "[terminal]\ngpu = false\n");
        let cfg = toml::from_str::<TearminalConfig>(&text);
        assert!(cfg.is_ok());
    }

    #[test]
    fn test_config_and_session_read_write_traits_in_memory() {
        // Probar serialización/deserialización en memoria sobre std::io::Read / Write sin tocar disco
        let cfg = TearminalConfig::default();
        let mut bytes = Vec::new();
        cfg.write_to(&mut bytes).expect("write_to debe tener éxito");

        let loaded =
            TearminalConfig::from_reader(bytes.as_slice()).expect("from_reader debe tener éxito");
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

    #[test]
    fn test_defaults_and_legacy_tuning_ignored() {
        let default_cfg = TearminalConfig::default();
        assert_eq!(default_cfg.appearance.font_size, 16.0);
        assert_eq!(default_cfg.appearance.ui_font_size, 14.0);
        assert_eq!(default_cfg.appearance.line_spacing, 1.0);
        assert_eq!(default_cfg.terminal.cursor_shape, "block");
        assert!(!default_cfg.appearance.show_dimensions);

        let toml_str = r#"
[appearance]
font_family = "JetBrains Mono Nerd Font"
font_size = 18.0
ui_font_size = 16.0
window_opacity = 1.0
theme = "Catppuccin Mocha"
theme_light = "Catppuccin Latte"
line_spacing = 1.0
tab_layout = "horizontal"
theme_mode = "system"
show_status_bar = true
show_file_explorer = false
panel_side = "left"
font_contrast = 1.4
image_filter = "linear"
pixel_snap = true
show_brand = true

[terminal]
default_shell = "powershell.exe"
scrollback_lines = 10000
cursor_blink = true
cursor_shape = "beam"
copy_on_select = false
restore_session = true
gpu = true
"#;
        let cfg: TearminalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.appearance.font_size, 18.0);
        assert_eq!(cfg.terminal.cursor_shape, "beam");

        let mut bytes = Vec::new();
        cfg.write_to(&mut bytes).unwrap();
        let reloaded = TearminalConfig::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(reloaded.appearance.font_size, 18.0);
        assert_eq!(reloaded.terminal.cursor_shape, "beam");
    }

    #[test]
    fn test_keep_awake_mode_cycle_and_parsing() {
        assert_eq!(KeepAwakeMode::Off.next(), KeepAwakeMode::Agent);
        assert_eq!(KeepAwakeMode::Agent.next(), KeepAwakeMode::Active);
        assert_eq!(KeepAwakeMode::Active.next(), KeepAwakeMode::Off);

        #[derive(Deserialize)]
        struct TestMode {
            mode: KeepAwakeMode,
        }

        let parsed: TestMode = toml::from_str(r#"mode = "apagado""#).unwrap();
        assert_eq!(parsed.mode, KeepAwakeMode::Off);
        let parsed: TestMode = toml::from_str(r#"mode = "agente""#).unwrap();
        assert_eq!(parsed.mode, KeepAwakeMode::Agent);
        let parsed: TestMode = toml::from_str(r#"mode = "activo""#).unwrap();
        assert_eq!(parsed.mode, KeepAwakeMode::Active);
        let parsed: TestMode = toml::from_str(r#"mode = "agent""#).unwrap();
        assert_eq!(parsed.mode, KeepAwakeMode::Agent);
        let parsed: TestMode = toml::from_str(r#"mode = "active""#).unwrap();
        assert_eq!(parsed.mode, KeepAwakeMode::Active);
    }

    #[test]
    fn test_terminal_config_animations_default_and_parsing() {
        let mut def = TearminalConfig::default();
        assert!(def.terminal.animations);

        def.terminal.animations = false;
        let mut bytes = Vec::new();
        def.write_to(&mut bytes).unwrap();
        let reloaded = TearminalConfig::from_reader(bytes.as_slice()).unwrap();
        assert!(!reloaded.terminal.animations);
    }

    #[test]
    fn test_database_and_performance_config_defaults_and_roundtrip() {
        let def = TearminalConfig::default();
        assert!(def.database.prod_read_only);
        assert_eq!(def.database.write_window_minutes, 10);
        assert!(def.database.written_confirmation);
        assert!(def.database.warn_no_where);
        assert!(def.database.enforce_transaction);
        assert!(def.database.backup_original_rows);
        assert!(def.database.warn_unsafe_pem_permissions);
        assert_eq!(def.database.cert_warn_days, 30);
        assert_eq!(def.database.page_size, 50);
        assert_eq!(def.database.query_timeout_seconds, 30);
        assert_eq!(def.database.query_row_limit, 10_000);
        assert_eq!(def.database.slow_query_threshold_ms, 300);
        assert!(def.database.autocomplete);
        assert!(def.database.save_passwords_in_credential_manager);
        assert!(def.database.save_query_history);

        assert!(def.performance.show_cpu);
        assert!(def.performance.show_ram);
        assert!(def.performance.show_gpu);
        assert!(def.performance.show_disk);
        assert!(!def.performance.show_net);
        assert_eq!(def.performance.sample_interval_ms, 2000);
        assert!(!def.performance.enable_alerts);
        assert_eq!(def.performance.cpu_alert_threshold, 90);
        assert_eq!(def.performance.ram_alert_threshold, 90);
        assert_eq!(def.performance.disk_alert_threshold, 90);

        // Deserializar configuración antigua sin [database] ni [performance]
        let old_toml = r#"
[appearance]
font_family = "JetBrains Mono"
font_size = 14.0
ui_font_size = 14.0
window_opacity = 1.0
theme = "Catppuccin Frappe"
theme_light = "Catppuccin Latte"
line_spacing = 1.0

[terminal]
default_shell = "powershell.exe"
scrollback_lines = 10000
cursor_blink = true
copy_on_select = false
"#;
        let parsed: TearminalConfig = toml::from_str(old_toml).unwrap();
        assert_eq!(parsed.database, DatabaseConfig::default());
        assert_eq!(parsed.performance, PerformanceConfig::default());

        // Serializar y deserializar con valores modificados
        let mut custom = def;
        custom.database.prod_read_only = false;
        custom.database.write_window_minutes = 25;
        custom.database.query_timeout_seconds = 60;
        custom.performance.show_net = true;
        custom.performance.sample_interval_ms = 5000;
        custom.performance.cpu_alert_threshold = 85;

        let mut bytes = Vec::new();
        custom.write_to(&mut bytes).unwrap();
        let reloaded = TearminalConfig::from_reader(bytes.as_slice()).unwrap();
        assert!(!reloaded.database.prod_read_only);
        assert_eq!(reloaded.database.write_window_minutes, 25);
        assert_eq!(reloaded.database.query_timeout_seconds, 60);
        assert!(reloaded.performance.show_net);
        assert_eq!(reloaded.performance.sample_interval_ms, 5000);
        assert_eq!(reloaded.performance.cpu_alert_threshold, 85);

        // Agentes CLI configurables + sanitización
        assert!(sanitize_agent_name("codex.exe") == Some("codex".to_string()));
        assert!(sanitize_agent_name("bad; rm -rf").is_none());
        let mut perf = PerformanceConfig::default();
        perf.set_agent_executables_csv("claude, my-agent.exe, bad$cmd, codex, claude");
        assert_eq!(perf.agent_executables, vec!["claude", "my-agent", "codex"]);

        // Atajos de teclado: parseo, formato y conflictos
        let chord = ParsedKeyChord::parse("ctrl + shift + p").expect("chord válido");
        assert!(chord.ctrl && chord.shift && !chord.alt && chord.key == "P");
        assert_eq!(chord.format_compact(), "Ctrl+Shift+P");
        assert_eq!(chord.format_keycaps(), "Ctrl + Shift + P");

        let mut sc = ShortcutsConfig::default();
        assert_eq!(
            sc.find_conflict(ShortcutAction::NewTab, &chord),
            Some(ShortcutAction::CommandPalette)
        );
        sc.set(ShortcutAction::NewTab, "Ctrl+Alt+N".to_string());
        assert_eq!(sc.display_keycaps(ShortcutAction::NewTab), "Ctrl + Alt + N");
    }
}
