//! Aplicación: estado global (`TearminalApp`), pestañas/paneles y bucle de eventos de winit.
//!
//! `TearminalApp` reparte sus métodos por responsabilidad en submódulos (`tabs`, `settings`, `clipboard`, ...);
//! los eventos de winit entran por `handler` y se despachan a `redraw`, `keyboard`, `cursor`, `click` y `wheel`.

mod ai_usage;
mod click;
mod clipboard;
mod context_menu;
mod cursor;
mod fs_watcher;
mod handler;
mod host;
mod keyboard;
mod keys;
mod layout;
mod palette;
mod panel;
pub(crate) mod platform;
mod redraw;
mod remote_files;
mod settings;
mod shell_context;
mod ssh_jobs;
mod tabs;
mod viewer;
mod wheel;

pub(crate) use crate::renderer::PaletteRects;
pub(crate) use ai_usage::*;
pub(crate) use context_menu::*;
pub(crate) use host::TearminalHost;
pub(crate) use keys::{encode_kitty_key, key_code_to_char, key_is_char, KeyEventType};
pub(crate) use layout::*;
pub(crate) use palette::*;
pub(crate) use shell_context::detect_ai_agent;
pub(crate) use viewer::*;

use platform::{
    apply_window_opacity_native, create_native_icon, get_physical_modifiers, update_windows_theme,
};
use shell_context::get_git_branch;

use crate::config::{PaneSession, TabSession, TearminalConfig, WindowSession};
use crate::pty::PtyManager;
use crate::renderer::{
    AiAgentStatus, EditingSettingField, PaneHeaderButton, PaneHeaderRects, SessionStatus,
    SettingsPageRects, SettingsPageState, SettingsSection, SidebarHeader, SidebarTabItem,
    StatusBarRects, TerminalRenderer, TopTabBarRects, TopTitleBarRects,
};
use crate::terminal::{SelectionRange, TerminalState};
use crate::theme::Theme;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey, SmolStr};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TearminalEvent {
    PtyData(PaneId),
    PtyClosed(PaneId),
    /// Terminó un trabajo SSH en segundo plano (`TearminalApp::process_ssh_results`)
    SshDone,
    /// Cambio detectado en el sistema de archivos del espacio de trabajo
    FsChanged,
    /// Comprobación de actualizaciones en segundo plano completada
    UpdateCheckDone,
}

pub(crate) const SIDEBAR_WIDTH: usize = crate::renderer::SidebarRects::WIDTH;

/// Acción de soltar una pestaña con el mouse (para transferir a otra ventana o nueva ventana)
pub(crate) struct TabDropAction {
    pub(crate) tab: TerminalTab,
    pub(crate) screen_pos: (i32, i32),
}

pub(crate) struct TearminalApp {
    pub config: TearminalConfig,
    pub theme: Theme,
    proxy: Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: TabId,
    /// (pestaña, panel) cuya carpeta mostró por última vez el explorador: si el foco pasa a otra terminal
    /// de la misma pestaña, `on_redraw` reorienta el explorador (`sync_file_explorer_root`).
    explorer_focus: Option<(TabId, PaneId)>,
    pub sidebar_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_settings_btn: bool,
    hovered_toggle_btn: bool,
    /// Botón de la cabecera de un panel dividido bajo el ratón
    hovered_pane_btn: Option<(PaneId, PaneHeaderButton)>,
    pub settings_page_state: SettingsPageState,
    pub context_menu: Option<ContextMenu>,
    pub palette: Option<PaletteState>,
    pub file_explorer: crate::explorer::FileExplorer,
    hovered_files_btn: bool,
    pub last_terminal_tab_index: usize,
    renderer: Option<TerminalRenderer>,
    window: Option<Arc<Window>>,
    /// Superficie Direct3D 11 (hardware o WARP) a la que se presenta cada escena.
    gpu_surface: Option<crate::renderer::GpuSurface>,
    /// Última vez que se registró un fallo de `present` (como mucho un registro cada 5 s).
    last_present_error_log: Option<Instant>,
    /// Escena del fotograma: se graba cada vez (memoria reutilizada) y `GpuSurface::present` la dibuja.
    canvas: crate::renderer::Canvas,
    clipboard: Option<arboard::Clipboard>,
    modifiers: ModifiersState,
    drag_start: Option<(usize, usize)>,
    /// Palabra (doble clic) o línea (triple clic) inicial: el arrastre la extiende por unidades.
    drag_anchor: Option<SelectionRange>,
    mouse_pos: PhysicalPosition<f64>,
    mouse_down: bool,
    cursor_timer: Instant,
    /// Último avance del spinner de la pestaña que ejecuta (barra superior).
    spinner_timer: Instant,
    /// Fase actual del parpadeo del cursor (true = visible)
    cursor_on: bool,
    /// Peticiones en banda al bash remoto aún sin respuesta
    pending_remote: Vec<remote_files::PendingRemote>,
    /// Resultados de los hilos SSH (`ssh_jobs`); cada hilo envía el suyo y avisa con `TearminalEvent::SshDone`
    ssh_tx: std::sync::mpsc::Sender<ssh_jobs::SshDone>,
    ssh_rx: std::sync::mpsc::Receiver<ssh_jobs::SshDone>,
    next_remote_id: u32,
    /// Instante a partir del cual se aplica el tamaño final de rejilla al PTY (antirrebote)
    resize_deadline: Option<Instant>,
    is_focused: bool,
    // Barra de desplazamiento interactiva y auto-ocultable
    scrollbar_dragging: bool,
    scrollbar_drag_pane_id: Option<PaneId>,
    scrollbar_drag_offset_y: f32,
    sidebar_scrollbar_dragging: bool,
    sidebar_scrollbar_drag_offset_y: f32,
    pub sidebar_scrollbar_hovered: bool,
    explorer_scrollbar_dragging: bool,
    /// Arrastrando el borde izquierdo del explorador para cambiar su ancho
    explorer_resizing: bool,
    explorer_scrollbar_drag_offset_y: f32,
    viewer_scrollbar_dragging: bool,
    viewer_scrollbar_drag_offset_y: f32,
    viewer_hscrollbar_dragging: bool,
    settings_scrollbar_dragging: bool,
    settings_scrollbar_drag_offset_y: f32,
    pub(crate) palette_scrollbar_dragging: bool,
    pub(crate) palette_scrollbar_drag_offset_y: f32,
    viewer_hscrollbar_drag_offset_x: f32,
    viewer_drag_selecting: bool,
    viewer_drag_start: Option<(usize, usize)>,
    last_viewer_click_time: Instant,
    last_explorer_click_time: Instant,
    last_explorer_click_idx: Option<usize>,
    last_tab_click_time: Instant,
    last_tab_click_idx: Option<usize>,
    last_viewer_click_pos: Option<(usize, usize)>,
    viewer_click_count: usize,
    last_terminal_click_time: Instant,
    last_terminal_click_pos: Option<(usize, usize)>,
    last_terminal_click_pane_id: Option<PaneId>,
    terminal_click_count: usize,
    last_window_move: Instant,
    last_input_click_time: Instant,
    last_input_click_count: usize,
    last_scroll_time: Instant,
    /// Fracción de línea pendiente de la rueda (los touchpads envían deltas pequeños)
    wheel_remainder: f32,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    last_titlebar_click: Instant,
    /// Último tema de Windows visto (true = claro), para detectar el cambio en modo "system"
    system_light: bool,
    /// Ventana minimizada/tapada: no se renderiza (el PTY se sigue procesando)
    occluded: bool,
    scroll_hide_armed: bool,
    last_window_title: std::cell::RefCell<String>,
    pub last_focus_gained: Instant,
    pub sidebar_scroll_offset: usize,
    /// Identificador de esta ventana en `session.toml`
    session_id: u32,
    /// La ventana debe cerrarse: el anfitrión (`TearminalHost`) la retira tras el evento
    closing: bool,
    /// Se pidió abrir otra ventana (la crea el anfitrión, que posee el bucle de eventos)
    pub new_window_request: bool,
    /// Pestaña desprendida que se transferirá a una nueva ventana
    pub new_window_with_tab: Option<TerminalTab>,
    /// Pestaña soltada con el mouse fuera de su barra de pestañas (para acoplar o nueva ventana)
    pub tab_drop_action: Option<TabDropAction>,
    /// Petición desde el menú para mover una pestaña a otra ventana existente
    pub move_tab_to_other_window: Option<usize>,
    /// Indica si hay múltiples ventanas activas en el anfitrión
    pub has_other_windows: bool,
    /// Divisor entre paneles que se está arrastrando
    divider_drag: Option<Divider>,
    /// Pestaña (índice) que se arrastra para reordenarla
    tab_drag: Option<usize>,
    /// Hover sobre el indicador de agente/modelo IA en la barra de estado inferior
    hovered_ai_badge: bool,
    /// Hover sobre el indicador de actualización disponible en la barra de estado inferior
    pub hovered_update_badge: bool,
    /// Estado de consumo y cuotas de modelos de IA para el menú flotante
    pub ai_usage: AiUsageState,
    /// Observador de cambios del sistema de archivos local para refresco reactivo de Git y archivos
    fs_watcher: Option<fs_watcher::FsWatcher>,
}

// -------------------------------------------------------------------------------------------------
// Mechanical Sympathy: alineación de variables a la línea de caché (64 bytes) para evitar False Sharing
// -------------------------------------------------------------------------------------------------

/// Envoltorio alineado a la línea de caché de la CPU (64 bytes) para mitigar el **False Sharing**.
///
/// Previene que dos variables atómicas independientes residan en la misma línea de caché L1/L2
/// (generalmente de 64 bytes). Sin esta alineación, modificaciones atómicas simultáneas desde
/// diferentes núcleos provocan invalidaciones constantes de caché (cache line ping-pong),
/// degradando el rendimiento del sistema multihilo.
#[repr(align(64))]
pub(crate) struct CachePadded<T>(pub T);

impl<T> CachePadded<T> {
    pub(crate) const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for CachePadded<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Los ids de panel son únicos entre ventanas: `TearminalEvent::PtyData` los usa para encontrar su dueña.
/// Se alinean a líneas de caché separadas para evitar False Sharing entre hilos de E/S y UI.
static NEXT_PANE_ID: CachePadded<std::sync::atomic::AtomicUsize> =
    CachePadded::new(std::sync::atomic::AtomicUsize::new(0));
static NEXT_WINDOW_ID: CachePadded<std::sync::atomic::AtomicU32> =
    CachePadded::new(std::sync::atomic::AtomicU32::new(0));

pub(crate) fn alloc_pane_id() -> PaneId {
    PaneId(NEXT_PANE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

impl TearminalApp {
    /// Consulta el estado físico de las teclas modificadoras de manera consistente en todo el sistema.
    #[inline]
    pub(crate) fn physical_modifiers(&self) -> (bool, bool, bool, bool, bool) {
        #[cfg(target_os = "windows")]
        {
            get_physical_modifiers()
        }
        #[cfg(not(target_os = "windows"))]
        {
            (
                self.modifiers.control_key(),
                self.modifiers.shift_key(),
                self.modifiers.alt_key(),
                false,
                self.modifiers.super_key(),
            )
        }
    }

    /// Manejo unificado de teclado para cualquier campo de entrada de texto (navegación, borrado,
    /// atajos estándar de portapapeles, escritura y refresco visual).
    /// Devuelve `(consumido, texto_cambiado)`.
    pub(crate) fn handle_text_input_key(
        &mut self,
        input: &mut crate::ui::TextInput,
        logical_key: &winit::keyboard::Key,
        physical_key: winit::keyboard::PhysicalKey,
        text: Option<&str>,
    ) -> (bool, bool) {
        let (ctrl, shift, alt, _, _) = self.physical_modifiers();
        let paste = if ctrl && !alt && key_is_char(logical_key, physical_key, 'v') {
            self.get_clipboard_string()
        } else {
            None
        };

        let (res, to_copy) = input.handle_key(
            logical_key,
            physical_key,
            text,
            ctrl,
            shift,
            alt,
            paste.as_deref(),
        );

        if let Some(copy_str) = to_copy {
            self.set_clipboard_text(&copy_str);
        }

        match res {
            crate::ui::text_input::TextInputKeyResult::Ignored => (false, false),
            crate::ui::text_input::TextInputKeyResult::Navigated => {
                self.cursor_on = true;
                self.cursor_timer = std::time::Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                (true, false)
            }
            crate::ui::text_input::TextInputKeyResult::Changed => {
                self.cursor_on = true;
                self.cursor_timer = std::time::Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                (true, true)
            }
        }
    }

    /// Manejo unificado de clic del ratón sobre un campo de texto (posicionamiento de caret,
    /// doble clic para palabra completa, triple clic para seleccionar todo y arrastre).
    pub(crate) fn handle_text_input_click(
        &mut self,
        input: &mut crate::ui::TextInput,
        mx: usize,
        text_start_x: usize,
    ) {
        let char_idx = self
            .renderer
            .as_ref()
            .map(|r| {
                input.hit_test(mx, text_start_x, |s| {
                    r.measure_ui_text(s, crate::renderer::tokens::BODY, true)
                })
            })
            .unwrap_or(input.len());

        let now = std::time::Instant::now();
        let is_quick_click =
            now.duration_since(self.last_input_click_time) <= platform::double_click_time();
        let (_, shift, _, _, _) = self.physical_modifiers();

        if is_quick_click && self.last_input_click_count == 1 {
            input.select_word_around(char_idx);
            self.last_input_click_count = 2;
        } else if is_quick_click && self.last_input_click_count == 2 {
            input.select_all();
            self.last_input_click_count = 3;
        } else {
            input.mouse_down(char_idx, shift);
            self.last_input_click_count = 1;
        }
        self.last_input_click_time = now;
        self.cursor_on = true;
        self.cursor_timer = std::time::Instant::now();
        self.request_redraw();
    }

    /// Manejo unificado de arrastre del ratón para selección continua de texto.
    pub(crate) fn handle_text_input_drag(
        &mut self,
        input: &mut crate::ui::TextInput,
        mx: usize,
        text_start_x: usize,
    ) -> bool {
        if !input.is_dragging {
            return false;
        }
        let char_idx = self
            .renderer
            .as_ref()
            .map(|r| {
                input.hit_test(mx, text_start_x, |s| {
                    r.measure_ui_text(s, crate::renderer::tokens::BODY, true)
                })
            })
            .unwrap_or(input.len());
        input.mouse_drag(char_idx);
        self.cursor_on = true;
        self.cursor_timer = std::time::Instant::now();
        self.request_redraw();
        true
    }

    /// Solicita un redibujado de la ventana si está abierta.
    #[inline]
    pub(crate) fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Obtiene el tamaño interno actual de la ventana (ancho, alto), con fallback (800, 600) si no está abierta.
    #[inline]
    pub(crate) fn window_size(&self) -> (usize, usize) {
        if let Some(w) = &self.window {
            let s = w.inner_size();
            (s.width as usize, s.height as usize)
        } else {
            (800, 600)
        }
    }

    /// Obtiene el ancho interno de la ventana en píxeles.
    #[inline]
    pub(crate) fn window_width(&self) -> usize {
        self.window_size().0
    }

    /// Obtiene el alto interno de la ventana en píxeles.
    #[inline]
    pub(crate) fn window_height(&self) -> usize {
        self.window_size().1
    }

    /// Fija el cursor del ratón en la ventana si está abierta.
    #[inline]
    pub(crate) fn set_cursor_icon(&self, icon: winit::window::CursorIcon) {
        if let Some(w) = &self.window {
            w.set_cursor(icon);
        }
    }

    /// Rectángulo del área de contenido principal (terminales, visor o ajustes) en coordenadas de ventana:
    /// `(x, y, ancho, alto)`.
    #[inline]
    pub(crate) fn content_area_rect(
        &self,
        win_w: usize,
        win_h: usize,
    ) -> (usize, usize, usize, usize) {
        let x = self.sidebar_offset();
        let y = self.top_offset();
        let right_offset = self.file_explorer_offset();
        let status_bar_h = self.status_bar_height();
        let w = win_w.saturating_sub(x + right_offset);
        let h = win_h.saturating_sub(y + status_bar_h);
        (x, y, w, h)
    }

    /// Rectángulos de los paneles de la pestaña activa en coordenadas de ventana.
    pub(crate) fn active_tab_pane_rects(&self, win_w: usize, win_h: usize) -> Vec<PaneRect> {
        let (x, y, w, h) = self.content_area_rect(win_w, win_h);
        self.tabs
            .get(self.active_tab_index)
            .map(|t| t.compute_pane_rects(x, y, w, h))
            .unwrap_or_default()
    }

    pub(crate) fn alloc_tab_id(&mut self) -> TabId {
        let id = self.next_tab_id;
        self.next_tab_id = TabId(id.0 + 1);
        id
    }

    /// Ventana con una sola pestaña nueva
    pub(crate) fn new(
        config: TearminalConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
    ) -> Self {
        Self::from_session(config, proxy, None)
    }

    /// Ventana restaurada desde `session` (sin sesión, o sin pestañas, una pestaña nueva)
    pub(crate) fn from_session(
        config: TearminalConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
        session: Option<WindowSession>,
    ) -> Self {
        let system_light = crate::theme::is_windows_system_light_mode();
        let theme = config.appearance.resolve_theme(system_light);

        let session = session.filter(|s| !s.tabs.is_empty());
        let explorer_visible = session
            .as_ref()
            .map(|s| s.file_explorer_visible)
            .unwrap_or(config.appearance.show_file_explorer);
        let sidebar_visible = session.as_ref().is_none_or(|s| s.sidebar_visible);
        let active_tab_index = session.as_ref().map_or(0, |s| s.active_tab_index);
        let spawn = SpawnCtx {
            config: &config,
            proxy: &proxy,
            theme: &theme,
        };
        let tabs: Vec<TerminalTab> = match &session {
            Some(s) => s
                .tabs
                .iter()
                .enumerate()
                .map(|(id, t)| spawn.restore_tab(TabId(id), t))
                .collect(),
            None => {
                let pane = spawn.pane(None);
                vec![TerminalTab::new(TabId(0), "Terminal 1".to_string(), pane)]
            }
        };
        let active_tab_index = active_tab_index.min(tabs.len() - 1);

        let mut app = Self::build(
            config,
            proxy,
            theme,
            system_light,
            tabs,
            active_tab_index,
            sidebar_visible,
            explorer_visible,
        );
        if let Some(s) = &session {
            // Sin tamaño de ventana aún: solo el mínimo; `set_width_from_edge` acota el máximo al arrastrar
            app.file_explorer.width = s
                .file_explorer_width
                .max(crate::explorer::FILE_EXPLORER_MIN_WIDTH);
        }
        app
    }

    /// Ventana nueva inicializada con una pestaña existente transferida desde otra ventana
    pub(crate) fn from_tab(
        config: TearminalConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
        mut tab: TerminalTab,
    ) -> Self {
        tab.id = TabId(0);
        let system_light = crate::theme::is_windows_system_light_mode();
        let theme = config.appearance.resolve_theme(system_light);
        let explorer_visible = config.appearance.show_file_explorer;
        Self::build(
            config,
            proxy,
            theme,
            system_light,
            vec![tab],
            0,
            true,
            explorer_visible,
        )
    }

    /// Estado inicial común de toda ventana: solo cambian las pestañas y la visibilidad de paneles
    #[allow(clippy::too_many_arguments)]
    fn build(
        config: TearminalConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
        theme: Theme,
        system_light: bool,
        tabs: Vec<TerminalTab>,
        active_tab_index: usize,
        sidebar_visible: bool,
        explorer_visible: bool,
    ) -> Self {
        // Con pestañas superiores el explorador vive en el panel y se muestra con él
        let explorer_visible = if config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal")
        {
            sidebar_visible
        } else {
            explorer_visible
        };
        let next_tab_id = TabId(tabs.len());
        let file_explorer =
            crate::explorer::FileExplorer::new(std::env::current_dir().ok(), explorer_visible);
        let clipboard = arboard::Clipboard::new().ok();
        let (ssh_tx, ssh_rx) = std::sync::mpsc::channel();
        Self {
            config,
            theme,
            proxy,
            tabs,
            active_tab_index,
            next_tab_id,
            sidebar_visible,
            hovered_tab: None,
            hovered_close: None,
            hovered_new_tab: false,
            hovered_settings_btn: false,
            hovered_toggle_btn: false,
            hovered_pane_btn: None,
            settings_page_state: SettingsPageState::default(),
            context_menu: None,
            palette: None,
            file_explorer,
            hovered_files_btn: false,
            last_terminal_tab_index: 0,
            explorer_focus: None,
            renderer: None,
            window: None,
            gpu_surface: None,
            last_present_error_log: None,
            canvas: crate::renderer::Canvas::new(),
            clipboard,
            modifiers: ModifiersState::default(),
            drag_start: None,
            drag_anchor: None,
            mouse_pos: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            cursor_timer: Instant::now(),
            spinner_timer: Instant::now(),
            cursor_on: true,
            pending_remote: Vec::new(),
            ssh_tx,
            ssh_rx,
            // Aleatorio: los ids no son adivinables por un servidor que quiera responder por otro panel
            next_remote_id: crate::pty::random_id(),
            resize_deadline: None,
            is_focused: true,
            scrollbar_dragging: false,
            scrollbar_drag_pane_id: None,
            scrollbar_drag_offset_y: 0.0,
            sidebar_scrollbar_dragging: false,
            sidebar_scrollbar_drag_offset_y: 0.0,
            sidebar_scrollbar_hovered: false,
            explorer_scrollbar_dragging: false,
            explorer_resizing: false,
            explorer_scrollbar_drag_offset_y: 0.0,
            viewer_scrollbar_dragging: false,
            viewer_scrollbar_drag_offset_y: 0.0,
            viewer_hscrollbar_dragging: false,
            settings_scrollbar_dragging: false,
            settings_scrollbar_drag_offset_y: 0.0,
            palette_scrollbar_dragging: false,
            palette_scrollbar_drag_offset_y: 0.0,
            viewer_hscrollbar_drag_offset_x: 0.0,
            viewer_drag_selecting: false,
            viewer_drag_start: None,
            last_viewer_click_time: Instant::now(),
            last_explorer_click_time: Instant::now(),
            last_explorer_click_idx: None,
            last_tab_click_time: Instant::now(),
            last_tab_click_idx: None,
            last_viewer_click_pos: None,
            viewer_click_count: 0,
            last_terminal_click_time: Instant::now(),
            last_terminal_click_pos: None,
            last_terminal_click_pane_id: None,
            terminal_click_count: 0,
            last_window_move: Instant::now(),
            last_input_click_time: Instant::now(),
            last_input_click_count: 0,
            last_scroll_time: Instant::now(),
            wheel_remainder: 0.0,
            hovered_win_min: false,
            hovered_win_max: false,
            hovered_win_close: false,
            last_titlebar_click: Instant::now(),
            system_light,
            occluded: false,
            scroll_hide_armed: false,
            last_window_title: Default::default(),
            last_focus_gained: Instant::now(),
            sidebar_scroll_offset: 0,
            session_id: NEXT_WINDOW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            closing: false,
            new_window_request: false,
            new_window_with_tab: None,
            tab_drop_action: None,
            move_tab_to_other_window: None,
            has_other_windows: false,
            divider_drag: None,
            tab_drag: None,
            hovered_ai_badge: false,
            hovered_update_badge: false,
            ai_usage: AiUsageState::default(),
            fs_watcher: None,
        }
    }

    /// Estado de esta ventana tal como se restaurará: solo pestañas de terminal, con sus paneles
    fn session_snapshot(&self) -> WindowSession {
        let is_terminal = |t: &TerminalTab| !t.is_settings && t.file_viewer.is_none();
        let cwd_of = |p: &TerminalPane| {
            // Un panel SSH guarda la carpeta remota en `current_dir`: no es una ruta local
            let dir = p.terminal.current_dir.as_ref();
            dir.filter(|_| p.terminal.ssh_session.is_none())
                .map(|d| d.to_string_lossy().to_string())
        };
        let tabs: Vec<TabSession> = self
            .tabs
            .iter()
            .filter(|t| is_terminal(t))
            .map(|t| TabSession {
                title: t.title.clone(),
                layout: t.session_layout(&cwd_of),
                active_pane: t
                    .leaf_ids()
                    .iter()
                    .position(|&id| id == t.active_pane_id)
                    .unwrap_or(0),
            })
            .collect();
        // Con Ajustes o un visor al frente, la pestaña activa al restaurar es la última de terminal
        let active = if self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(is_terminal)
        {
            self.active_tab_index
        } else {
            self.last_terminal_tab_index
        };
        let active_pos = self
            .tabs
            .iter()
            .take(active)
            .filter(|t| is_terminal(t))
            .count();
        WindowSession {
            active_tab_index: active_pos.min(tabs.len().saturating_sub(1)),
            sidebar_visible: self.sidebar_visible,
            file_explorer_visible: self.file_explorer.visible,
            file_explorer_width: self.file_explorer.width,
            tabs,
        }
    }

    pub(crate) fn save_session(&self) {
        crate::config::SessionState::save_window(self.session_id, self.session_snapshot());
    }

    /// Sesión de la ventana que abre `Ctrl+Shift+N`: una pestaña en la carpeta del panel activo
    pub(super) fn new_window_session(&self) -> WindowSession {
        WindowSession {
            active_tab_index: 0,
            sidebar_visible: self.sidebar_visible,
            file_explorer_visible: self.file_explorer.visible,
            file_explorer_width: self.file_explorer.width,
            tabs: vec![TabSession {
                title: "Terminal 1".to_string(),
                layout: PaneSession::Leaf {
                    cwd: self.active_cwd().map(|d| d.to_string_lossy().to_string()),
                },
                active_pane: 0,
            }],
        }
    }

    /// Cierra esta ventana (el anfitrión la retira) conservando su sesión
    pub(super) fn request_close(&mut self) {
        for tab in &mut self.tabs {
            if let Some(viewer) = tab.file_viewer.as_mut() {
                if viewer.needs_autosave {
                    viewer.autosave_local();
                }
            }
        }
        self.save_session();
        self.closing = true;
    }

    pub(crate) fn owns_pane(&self, pane_id: PaneId) -> bool {
        self.tabs.iter().any(|t| t.find_pane(pane_id).is_some())
    }

    /// Carpeta local del panel activo (la ventana nueva arranca ahí)
    pub(crate) fn active_cwd(&self) -> Option<std::path::PathBuf> {
        let pane = self.tabs.get(self.active_tab_index)?.active_pane()?;
        let dir = pane.terminal.current_dir.clone();
        dir.filter(|d| pane.terminal.ssh_session.is_none() && d.is_dir())
    }

    pub(crate) fn is_horizontal_tabs(&self) -> bool {
        self.config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal")
    }

    /// Offsets `(left_offset, right_offset, top_offset, status_bar_h)` para los paneles de terminal.
    /// No dependen de si la pestaña activa en este momento es Ajustes: las terminales siempre
    /// usan su propia disposición según `sidebar_visible`, `panel_on_right` y `show_status_bar`.
    pub(crate) fn terminal_area_offsets(&self) -> (usize, usize, usize, usize) {
        let top = self.top_offset();
        let status_h = if self.config.appearance.show_status_bar {
            26
        } else {
            0
        };
        let (left, right) = if self.sidebar_visible {
            if self.panel_on_right() {
                (0, SIDEBAR_WIDTH)
            } else {
                (SIDEBAR_WIDTH, 0)
            }
        } else if self.file_explorer.visible {
            (0, self.file_explorer.width)
        } else {
            (0, 0)
        };
        (left, right, top, status_h)
    }

    /// Espacio que el panel lateral quita al contenido por la izquierda.
    pub(crate) fn sidebar_offset(&self) -> usize {
        if self.panel_shown() && !self.panel_on_right() {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    pub(crate) fn top_offset(&self) -> usize {
        if self.is_horizontal_tabs() {
            TopTabBarRects::HEIGHT
        } else {
            TopTitleBarRects::HEIGHT
        }
    }

    pub(crate) fn status_bar_height(&self) -> usize {
        if self.config.appearance.show_status_bar
            && self
                .tabs
                .get(self.active_tab_index)
                .is_none_or(|t| !t.is_settings)
        {
            26
        } else {
            0
        }
    }

    /// Espacio que quitan al contenido por la derecha el explorador acoplado y el panel si va a ese lado.
    /// Con el panel a la vista el explorador va dentro de él y no resta ancho.
    pub(crate) fn file_explorer_offset(&self) -> usize {
        if self.settings_fullscreen() {
            return 0;
        }
        if self.panel_shown() {
            return if self.panel_on_right() {
                SIDEBAR_WIDTH
            } else {
                0
            };
        }
        if self.file_explorer.visible {
            self.file_explorer.width
        } else {
            0
        }
    }

    fn update_window_title(&self) {
        if let (Some(w), Some(tab)) = (&self.window, self.tabs.get(self.active_tab_index)) {
            let full_title = if tab.is_settings {
                "TEArminal - ⚙ Ajustes".to_string()
            } else if let Some(ref v) = tab.file_viewer {
                format!("TEArminal - ◈ {}", v.file_name)
            } else if tab.panes.len() > 1 {
                format!(
                    "TEArminal - {} ({} paneles)",
                    tab.display_title(),
                    tab.panes.len()
                )
            } else {
                format!("TEArminal - {}", tab.display_title())
            };

            // SetWindowText fuerza trabajo de DWM: solo si el título cambió
            if *self.last_window_title.borrow() != full_title {
                w.set_title(&full_title);
                *self.last_window_title.borrow_mut() = full_title;
            }
        }
    }

    /// Pospone el resize de los PTY hasta que dejen de llegar eventos de tamaño
    fn schedule_pty_resize(&mut self) {
        self.resize_deadline = Some(Instant::now() + std::time::Duration::from_millis(120));
    }

    fn recompute_grid_for_all_tabs(&mut self) {
        if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            let (offset, right_offset, top_offset, status_bar_h) = self.terminal_area_offsets();
            let total_w = (size.width as usize).saturating_sub(offset + right_offset);
            let total_h = (size.height as usize).saturating_sub(top_offset + status_bar_h);
            if total_w == 0 || total_h == 0 {
                return;
            }

            for tab in &mut self.tabs {
                if tab.is_settings || tab.file_viewer.is_some() {
                    continue;
                }
                let pane_rects = tab.compute_pane_rects(offset, top_offset, total_w, total_h);
                for rect in pane_rects {
                    if let Some(pane) = tab.find_pane_mut(rect.pane_id) {
                        let (cols, rows) = renderer.compute_pane_grid_size(rect.width, rect.height);
                        if (cols, rows) != pane.last_grid_size {
                            pane.last_grid_size = (cols, rows);
                            pane.terminal.resize(cols, rows);
                            pane.pty.resize(cols as u16, rows as u16);
                        }
                    }
                }
            }
        }
    }
}

/// Lo necesario para lanzar shells al abrir o restaurar una ventana
struct SpawnCtx<'a> {
    config: &'a TearminalConfig,
    proxy: &'a Option<winit::event_loop::EventLoopProxy<TearminalEvent>>,
    theme: &'a Theme,
}

impl SpawnCtx<'_> {
    /// Panel con un shell nuevo en `cwd`; el tamaño real lo fija `TearminalApp::init_window`
    fn pane(&self, cwd: Option<&std::path::Path>) -> TerminalPane {
        let (cols, rows) = (100, 30);
        let id = alloc_pane_id();
        let shell = &self.config.terminal.default_shell;
        let pty = PtyManager::new(
            id,
            shell,
            self.config.terminal.shell_integration,
            cols as u16,
            rows as u16,
            cwd,
            self.proxy.clone(),
        )
        .unwrap_or_else(|e| crate::pty::fatal_spawn_error(shell, &e));
        let mut terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );
        terminal.cursor_shape =
            crate::terminal::CursorShape::from_str_name(&self.config.terminal.cursor_shape);
        terminal.current_dir = cwd.map(std::path::Path::to_path_buf);
        TerminalPane::new(id, pty, terminal, (cols, rows))
    }

    fn restore_tab(&self, id: TabId, tab: &TabSession) -> TerminalTab {
        let mut panes = Vec::with_capacity(4);
        let layout = self.restore_layout(&tab.layout, &mut panes);
        let active = panes
            .get(tab.active_pane)
            .or(panes.first())
            .map_or(PaneId(0), |p| p.id);
        TerminalTab::from_layout(id, tab.title.clone(), layout, panes, active)
    }

    fn restore_layout(&self, node: &PaneSession, panes: &mut Vec<TerminalPane>) -> PaneNode {
        match node {
            PaneSession::Leaf { cwd } => {
                // La carpeta pudo desaparecer desde la última sesión
                let dir = cwd
                    .as_deref()
                    .map(std::path::Path::new)
                    .filter(|d| d.is_dir());
                let pane = self.pane(dir);
                let id = pane.id;
                panes.push(pane);
                PaneNode::Leaf(id)
            }
            PaneSession::Split {
                direction,
                ratio,
                first,
                second,
            } => PaneNode::Split {
                direction: if direction == "vertical" {
                    SplitDirection::Vertical
                } else {
                    SplitDirection::Horizontal
                },
                ratio: if ratio.is_finite() {
                    ratio.clamp(0.1, 0.9)
                } else {
                    0.5
                },
                first: Box::new(self.restore_layout(first, panes)),
                second: Box::new(self.restore_layout(second, panes)),
            },
        }
    }
}

#[cfg(test)]
mod tests;
