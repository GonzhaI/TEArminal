//! Aplicación: estado global (`AxonApp`), pestañas/paneles y bucle de eventos de winit.
//!
//! `AxonApp` reparte sus métodos por responsabilidad en submódulos (`tabs`, `settings`, `clipboard`, ...);
//! los eventos de winit entran por `handler` y se despachan a `redraw`, `keyboard`, `cursor`, `click` y `wheel`.

mod click;
mod clipboard;
mod context_menu;
mod cursor;
mod handler;
mod host;
mod keyboard;
mod keys;
mod layout;
mod platform;
mod redraw;
mod remote_files;
mod settings;
mod shell_context;
mod ssh_jobs;
mod tabs;
mod viewer;
mod wheel;

pub use context_menu::*;
pub use host::AxonHost;
pub use keys::{key_code_to_char, key_is_char};
pub use layout::*;
pub(crate) use shell_context::detect_ai_agent;
pub use viewer::*;

use platform::{
    apply_window_opacity_native, create_native_icon, get_physical_modifiers, update_windows_theme,
};
use shell_context::get_git_branch;

use crate::config::{AxonConfig, PaneSession, TabSession, WindowSession};
use crate::pty::PtyManager;
use crate::renderer::{
    AiAgentStatus, EditingSettingField, SettingsPageRects, SettingsPageState, SettingsSection,
    SidebarTabItem, TerminalRenderer, TopTabBarRects, TopTitleBarRects,
};
use crate::terminal::{SelectionRange, TerminalState};
use crate::theme::Theme;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey, SmolStr};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy)]
pub enum AxonEvent {
    PtyData(usize),
    PtyClosed(usize),
    /// Terminó un trabajo SSH en segundo plano (`AxonApp::process_ssh_results`)
    SshDone,
}

pub const SIDEBAR_WIDTH: usize = 190;

pub struct AxonApp {
    pub config: AxonConfig,
    pub theme: Theme,
    proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: usize,
    pub sidebar_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_settings_btn: bool,
    hovered_toggle_btn: bool,
    pub settings_page_state: SettingsPageState,
    pub context_menu: Option<ContextMenu>,
    pub file_explorer: crate::explorer::FileExplorer,
    hovered_files_btn: bool,
    pub last_terminal_tab_index: usize,
    renderer: Option<TerminalRenderer>,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    clipboard: Option<arboard::Clipboard>,
    modifiers: ModifiersState,
    drag_start: Option<(usize, usize)>,
    mouse_pos: PhysicalPosition<f64>,
    mouse_down: bool,
    cursor_timer: Instant,
    /// Fase actual del parpadeo del cursor (true = visible)
    cursor_on: bool,
    /// Peticiones en banda al bash remoto aún sin respuesta
    pending_remote: Vec<remote_files::PendingRemote>,
    /// Resultados de los hilos SSH (`ssh_jobs`); cada hilo envía el suyo y avisa con `AxonEvent::SshDone`
    ssh_tx: std::sync::mpsc::Sender<ssh_jobs::SshDone>,
    ssh_rx: std::sync::mpsc::Receiver<ssh_jobs::SshDone>,
    next_remote_id: u32,
    /// Instante a partir del cual se aplica el tamaño final de rejilla al PTY (antirrebote)
    resize_deadline: Option<Instant>,
    is_focused: bool,
    // Barra de desplazamiento interactiva y auto-ocultable
    scrollbar_dragging: bool,
    scrollbar_drag_pane_id: Option<usize>,
    scrollbar_drag_offset_y: f32,
    sidebar_scrollbar_dragging: bool,
    sidebar_scrollbar_drag_offset_y: f32,
    pub sidebar_scrollbar_hovered: bool,
    explorer_scrollbar_dragging: bool,
    explorer_scrollbar_drag_offset_y: f32,
    viewer_scrollbar_dragging: bool,
    viewer_scrollbar_drag_offset_y: f32,
    viewer_hscrollbar_dragging: bool,
    settings_scrollbar_dragging: bool,
    settings_scrollbar_drag_offset_y: f32,
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
    last_window_move: Instant,
    last_scroll_time: Instant,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    last_titlebar_click: Instant,
    last_system_theme_check: Instant,
    /// Ventana minimizada/tapada: no se renderiza (el PTY se sigue procesando)
    occluded: bool,
    scroll_hide_armed: bool,
    last_window_title: std::cell::RefCell<String>,
    pub last_focus_gained: Instant,
    pub sidebar_scroll_offset: usize,
    /// Identificador de esta ventana en `session.toml`
    session_id: u32,
    /// La ventana debe cerrarse: el anfitrión (`AxonHost`) la retira tras el evento
    closing: bool,
    /// Se pidió abrir otra ventana (la crea el anfitrión, que posee el bucle de eventos)
    new_window_request: bool,
    /// Divisor entre paneles que se está arrastrando
    divider_drag: Option<Divider>,
    /// Pestaña (índice) que se arrastra para reordenarla
    tab_drag: Option<usize>,
}

/// Los ids de panel son únicos entre ventanas: `AxonEvent::PtyData` los usa para encontrar su dueña
static NEXT_PANE_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static NEXT_WINDOW_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn alloc_pane_id() -> usize {
    NEXT_PANE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl AxonApp {
    /// Ventana con una sola pestaña nueva
    pub fn new(
        config: AxonConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    ) -> Self {
        Self::from_session(config, proxy, None)
    }

    /// Ventana restaurada desde `session` (sin sesión, o sin pestañas, una pestaña nueva)
    pub fn from_session(
        config: AxonConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
        session: Option<WindowSession>,
    ) -> Self {
        let theme = if config.appearance.theme_mode.eq_ignore_ascii_case("system") {
            if crate::theme::is_windows_system_light_mode() {
                Theme::catppuccin_latte()
            } else {
                Theme::from_name(&config.appearance.theme)
            }
        } else if config.appearance.theme_mode.eq_ignore_ascii_case("light") {
            Theme::catppuccin_latte()
        } else {
            Theme::from_name(&config.appearance.theme)
        };

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
                .map(|(id, t)| spawn.restore_tab(id, t))
                .collect(),
            None => {
                let pane = spawn.pane(None);
                vec![TerminalTab::new(0, "Terminal 1".to_string(), pane)]
            }
        };
        let next_tab_id = tabs.len();
        let active_tab_index = active_tab_index.min(tabs.len() - 1);

        let current_dir = std::env::current_dir().ok();
        let file_explorer = crate::explorer::FileExplorer::new(current_dir, explorer_visible);

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
            settings_page_state: SettingsPageState::default(),
            context_menu: None,
            file_explorer,
            hovered_files_btn: false,
            last_terminal_tab_index: 0,
            renderer: None,
            window: None,
            context: None,
            surface: None,
            clipboard,
            modifiers: ModifiersState::default(),
            drag_start: None,
            mouse_pos: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            cursor_timer: Instant::now(),
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
            explorer_scrollbar_drag_offset_y: 0.0,
            viewer_scrollbar_dragging: false,
            viewer_scrollbar_drag_offset_y: 0.0,
            viewer_hscrollbar_dragging: false,
            settings_scrollbar_dragging: false,
            settings_scrollbar_drag_offset_y: 0.0,
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
            last_window_move: Instant::now(),
            last_scroll_time: Instant::now(),
            hovered_win_min: false,
            hovered_win_max: false,
            hovered_win_close: false,
            last_titlebar_click: Instant::now(),
            last_system_theme_check: Instant::now(),
            occluded: false,
            scroll_hide_armed: false,
            last_window_title: Default::default(),
            last_focus_gained: Instant::now(),
            sidebar_scroll_offset: 0,
            session_id: NEXT_WINDOW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            closing: false,
            new_window_request: false,
            divider_drag: None,
            tab_drag: None,
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
            tabs,
        }
    }

    pub fn save_session(&self) {
        crate::config::SessionState::save_window(self.session_id, self.session_snapshot());
    }

    /// Sesión de la ventana que abre `Ctrl+Shift+N`: una pestaña en la carpeta del panel activo
    pub(super) fn new_window_session(&self) -> WindowSession {
        WindowSession {
            active_tab_index: 0,
            sidebar_visible: self.sidebar_visible,
            file_explorer_visible: self.file_explorer.visible,
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
        self.save_session();
        self.closing = true;
    }

    pub fn owns_pane(&self, pane_id: usize) -> bool {
        self.tabs.iter().any(|t| t.find_pane(pane_id).is_some())
    }

    /// Carpeta local del panel activo (la ventana nueva arranca ahí)
    pub fn active_cwd(&self) -> Option<std::path::PathBuf> {
        let pane = self.tabs.get(self.active_tab_index)?.active_pane()?;
        let dir = pane.terminal.current_dir.clone();
        dir.filter(|d| pane.terminal.ssh_session.is_none() && d.is_dir())
    }

    pub fn is_horizontal_tabs(&self) -> bool {
        self.config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal")
    }

    pub fn sidebar_offset(&self) -> usize {
        if self.is_horizontal_tabs() {
            0
        } else if self.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    pub fn top_offset(&self) -> usize {
        if self.is_horizontal_tabs() {
            TopTabBarRects::HEIGHT
        } else {
            TopTitleBarRects::HEIGHT
        }
    }

    pub fn status_bar_height(&self) -> usize {
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

    pub fn file_explorer_offset(&self) -> usize {
        if self.file_explorer.visible {
            crate::explorer::FILE_EXPLORER_WIDTH + 16
        } else {
            0
        }
    }

    fn update_window_title(&self) {
        if let (Some(w), Some(tab)) = (&self.window, self.tabs.get(self.active_tab_index)) {
            let full_title = if tab.is_settings {
                "Axon - ⚙ Ajustes".to_string()
            } else if let Some(ref v) = tab.file_viewer {
                format!("Axon - ◈ {}", v.file_name)
            } else if tab.panes.len() > 1 {
                format!(
                    "Axon - {} ({} paneles)",
                    tab.display_title(),
                    tab.panes.len()
                )
            } else {
                format!("Axon - {}", tab.display_title())
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
            let offset = self.sidebar_offset();
            let right_offset = self.file_explorer_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
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
    config: &'a AxonConfig,
    proxy: &'a Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    theme: &'a Theme,
}

impl SpawnCtx<'_> {
    /// Panel con un shell nuevo en `cwd`; el tamaño real lo fija `AxonApp::init_window`
    fn pane(&self, cwd: Option<&std::path::Path>) -> TerminalPane {
        let (cols, rows) = (100, 30);
        let id = alloc_pane_id();
        let shell = &self.config.terminal.default_shell;
        let pty = PtyManager::new(
            id,
            shell,
            self.config.terminal.ghostty_prompt,
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
        terminal.current_dir = cwd.map(std::path::Path::to_path_buf);
        TerminalPane::new(id, pty, terminal, (cols, rows))
    }

    fn restore_tab(&self, id: usize, tab: &TabSession) -> TerminalTab {
        let mut panes = Vec::new();
        let layout = self.restore_layout(&tab.layout, &mut panes);
        let active = panes
            .get(tab.active_pane)
            .or(panes.first())
            .map_or(0, |p| p.id);
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
