use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::renderer::{
    AiAgentStatus, SettingsPageRects, SettingsPageState, SettingsSection, SidebarTabItem,
    TerminalRenderer, TopTabBarRects, TopTitleBarRects,
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
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy)]
pub enum AxonEvent {
    PtyData(usize),
    PtyClosed(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(usize),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct PaneRect {
    pub pane_id: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct TerminalPane {
    pub id: usize,
    pub pty: PtyManager,
    pub terminal: TerminalState,
    pub last_grid_size: (usize, usize),
    pub cached_git_branch: Option<String>,
    pub cached_git_dir: Option<std::path::PathBuf>,
}

impl TerminalPane {
    pub fn new(
        id: usize,
        pty: PtyManager,
        terminal: TerminalState,
        last_grid_size: (usize, usize),
    ) -> Self {
        Self {
            id,
            pty,
            terminal,
            last_grid_size,
            cached_git_branch: None,
            cached_git_dir: None,
        }
    }

    pub fn get_git_branch(&mut self) -> Option<String> {
        if self.terminal.current_dir != self.cached_git_dir {
            self.cached_git_branch = get_git_branch(self.terminal.current_dir.as_deref());
            self.cached_git_dir = self.terminal.current_dir.clone();
        }
        self.cached_git_branch.clone()
    }
}

pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    pub panes: Vec<TerminalPane>,
    pub active_pane_id: usize,
    pub layout: PaneNode,
    pub has_unread: bool,
    pub is_settings: bool,
}

impl TerminalTab {
    pub fn new(id: usize, title: String, first_pane: TerminalPane) -> Self {
        let active_pane_id = first_pane.id;
        let layout = PaneNode::Leaf(active_pane_id);
        Self {
            id,
            title,
            panes: vec![first_pane],
            active_pane_id,
            layout,
            has_unread: false,
            is_settings: false,
        }
    }

    pub fn new_settings(id: usize) -> Self {
        Self {
            id,
            title: "Ajustes".to_string(),
            panes: Vec::new(),
            active_pane_id: 0,
            layout: PaneNode::Leaf(0),
            has_unread: false,
            is_settings: true,
        }
    }

    pub fn display_title(&self) -> String {
        if self.is_settings {
            return "Ajustes".to_string();
        }
        if let Some(active) = self.active_pane() {
            if let Some(ref ssh) = active.terminal.ssh_session {
                if ssh.is_active {
                    return format!("ssh: {}", ssh.destination);
                }
            }
            let raw = active.terminal.window_title.as_deref().unwrap_or(&self.title);
            return Self::clean_display_title(raw, active.terminal.current_dir.as_deref());
        }
        Self::clean_display_title(&self.title, None)
    }

    pub fn clean_display_title(raw: &str, cwd: Option<&std::path::Path>) -> String {
        let raw_lower = raw.trim().to_lowercase();
        let is_shell_or_path = raw_lower.ends_with("pwsh.exe")
            || raw_lower.ends_with("powershell.exe")
            || raw_lower.ends_with("cmd.exe")
            || raw_lower == "powershell"
            || raw_lower == "pwsh"
            || raw_lower == "cmd"
            || raw_lower == "windows powershell"
            || raw_lower.contains("powershell\\7\\pwsh.exe")
            || raw_lower.contains("system32\\cmd.exe")
            || raw_lower.contains("windowspowershell");

        if is_shell_or_path {
            if let Some(path) = cwd {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
                return path.display().to_string();
            }
            return "PowerShell".to_string();
        }

    if raw.contains('\\') || raw.contains('/') {
        let p = std::path::Path::new(raw);
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if !stem.is_empty() {
                return stem.to_string();
            }
        }
    }

    raw.trim().to_string()
}

    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.panes
            .iter()
            .find(|p| p.id == self.active_pane_id)
            .or_else(|| self.panes.first())
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        let active_id = self.active_pane_id;
        if let Some(pos) = self.panes.iter().position(|p| p.id == active_id) {
            Some(&mut self.panes[pos])
        } else {
            self.panes.first_mut()
        }
    }

    pub fn find_pane(&self, pane_id: usize) -> Option<&TerminalPane> {
        self.panes.iter().find(|p| p.id == pane_id)
    }

    pub fn find_pane_mut(&mut self, pane_id: usize) -> Option<&mut TerminalPane> {
        self.panes.iter_mut().find(|p| p.id == pane_id)
    }

    pub fn compute_pane_rects(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<PaneRect> {
        if self.is_settings || self.panes.is_empty() {
            return Vec::new();
        }
        let mut rects = Vec::new();
        self.collect_rects(&self.layout, total_x, total_y, total_w, total_h, &mut rects);
        rects
    }

    fn collect_rects(
        &self,
        node: &PaneNode,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        out: &mut Vec<PaneRect>,
    ) {
        const DIVIDER: usize = 2;
        match node {
            PaneNode::Leaf(pane_id) => {
                out.push(PaneRect {
                    pane_id: *pane_id,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => match direction {
                SplitDirection::Horizontal => {
                    let avail_h = h.saturating_sub(DIVIDER);
                    let h1 = ((avail_h as f32) * ratio).round() as usize;
                    let h1 = h1.min(avail_h);
                    let h2 = avail_h.saturating_sub(h1);
                    self.collect_rects(first, x, y, w, h1, out);
                    self.collect_rects(second, x, y + h1 + DIVIDER, w, h2, out);
                }
                SplitDirection::Vertical => {
                    let avail_w = w.saturating_sub(DIVIDER);
                    let w1 = ((avail_w as f32) * ratio).round() as usize;
                    let w1 = w1.min(avail_w);
                    let w2 = avail_w.saturating_sub(w1);
                    self.collect_rects(first, x, y, w1, h, out);
                    self.collect_rects(second, x + w1 + DIVIDER, y, w2, h, out);
                }
            },
        }
    }

    pub fn compute_dividers(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<(usize, usize, usize, usize)> {
        if self.is_settings || self.panes.is_empty() {
            return Vec::new();
        }
        let mut dividers = Vec::new();
        self.collect_dividers(
            &self.layout,
            total_x,
            total_y,
            total_w,
            total_h,
            &mut dividers,
        );
        dividers
    }

    fn collect_dividers(
        &self,
        node: &PaneNode,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        out: &mut Vec<(usize, usize, usize, usize)>,
    ) {
        const DIVIDER: usize = 2;
        match node {
            PaneNode::Leaf(_) => {}
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => match direction {
                SplitDirection::Horizontal => {
                    let avail_h = h.saturating_sub(DIVIDER);
                    let h1 = ((avail_h as f32) * ratio).round() as usize;
                    let h1 = h1.min(avail_h);
                    let h2 = avail_h.saturating_sub(h1);
                    out.push((x, y + h1, w, DIVIDER));
                    self.collect_dividers(first, x, y, w, h1, out);
                    self.collect_dividers(second, x, y + h1 + DIVIDER, w, h2, out);
                }
                SplitDirection::Vertical => {
                    let avail_w = w.saturating_sub(DIVIDER);
                    let w1 = ((avail_w as f32) * ratio).round() as usize;
                    let w1 = w1.min(avail_w);
                    let w2 = avail_w.saturating_sub(w1);
                    out.push((x + w1, y, DIVIDER, h));
                    self.collect_dividers(first, x, y, w1, h, out);
                    self.collect_dividers(second, x + w1 + DIVIDER, y, w2, h, out);
                }
            },
        }
    }

    pub fn split_active_pane(
        &mut self,
        new_pane_id: usize,
        new_pane: TerminalPane,
        direction: SplitDirection,
    ) {
        let active_id = self.active_pane_id;
        self.layout =
            Self::split_leaf_in_tree(self.layout.clone(), active_id, new_pane_id, direction);
        self.panes.push(new_pane);
        self.active_pane_id = new_pane_id;
    }

    fn split_leaf_in_tree(
        node: PaneNode,
        target_id: usize,
        new_id: usize,
        direction: SplitDirection,
    ) -> PaneNode {
        match node {
            PaneNode::Leaf(id) => {
                if id == target_id {
                    PaneNode::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(PaneNode::Leaf(target_id)),
                        second: Box::new(PaneNode::Leaf(new_id)),
                    }
                } else {
                    PaneNode::Leaf(id)
                }
            }
            PaneNode::Split {
                direction: d,
                ratio,
                first,
                second,
            } => PaneNode::Split {
                direction: d,
                ratio,
                first: Box::new(Self::split_leaf_in_tree(
                    *first, target_id, new_id, direction,
                )),
                second: Box::new(Self::split_leaf_in_tree(
                    *second, target_id, new_id, direction,
                )),
            },
        }
    }

    pub fn close_pane(&mut self, pane_id: usize) {
        self.panes.retain(|p| p.id != pane_id);
        if let Some(new_tree) = Self::remove_leaf_from_tree(self.layout.clone(), pane_id) {
            self.layout = new_tree;
        }
        if self.active_pane_id == pane_id {
            if let Some(p) = self.panes.last() {
                self.active_pane_id = p.id;
            }
        }
    }

    fn remove_leaf_from_tree(node: PaneNode, target_id: usize) -> Option<PaneNode> {
        match node {
            PaneNode::Leaf(id) => {
                if id == target_id {
                    None
                } else {
                    Some(PaneNode::Leaf(id))
                }
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let f = Self::remove_leaf_from_tree(*first, target_id);
                let s = Self::remove_leaf_from_tree(*second, target_id);
                match (f, s) {
                    (Some(f_node), Some(s_node)) => Some(PaneNode::Split {
                        direction,
                        ratio,
                        first: Box::new(f_node),
                        second: Box::new(s_node),
                    }),
                    (Some(f_node), None) => Some(f_node),
                    (None, Some(s_node)) => Some(s_node),
                    (None, None) => None,
                }
            }
        }
    }
}

pub const SIDEBAR_WIDTH: usize = 190;

pub struct AxonApp {
    pub config: AxonConfig,
    pub theme: Theme,
    proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: usize,
    next_pane_id: usize,
    pub sidebar_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_settings_btn: bool,
    hovered_toggle_btn: bool,
    hovered_floating_settings: bool,
    pub settings_page_state: SettingsPageState,
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
    is_focused: bool,
    // Barra de desplazamiento interactiva y auto-ocultable
    scrollbar_dragging: bool,
    scrollbar_drag_pane_id: Option<usize>,
    scrollbar_drag_offset_y: f32,
    last_scroll_time: Instant,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    last_titlebar_click: Instant,
    last_system_theme_check: Instant,
}

impl AxonApp {
    pub fn new(
        config: AxonConfig,
        proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    ) -> Self {
        let cols = 100;
        let rows = 30;
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

        // Restaurar estado de sesiones previas solo si está habilitado en configuración
        let saved_session = if config.terminal.restore_session {
            crate::config::SessionState::load()
        } else {
            None
        };
        let explorer_visible = saved_session
            .as_ref()
            .map(|s| s.file_explorer_visible)
            .unwrap_or(config.appearance.show_file_explorer);
        let mut next_pane_id = 0;
        let (tabs, active_tab_index, next_tab_id, sidebar_visible) = if let Some(session) =
            saved_session
        {
            let cwd_path = session
                .tabs
                .into_iter()
                .next()
                .and_then(|t| t.cwd.map(std::path::PathBuf::from));
            let cwd_ref = cwd_path.as_deref();

            let pane_id = next_pane_id;
            next_pane_id += 1;
            let pty = PtyManager::new(
                pane_id,
                &config.terminal.default_shell,
                config.terminal.ghostty_prompt,
                cols as u16,
                rows as u16,
                cwd_ref,
                proxy.clone(),
            )
            .expect("Error al inicializar Windows ConPTY");

            let mut terminal =
                TerminalState::new(cols, rows, config.terminal.scrollback_lines, theme.clone());
            terminal.current_dir = cwd_path;

            let pane = TerminalPane::new(pane_id, pty, terminal, (cols, rows));
            (
                vec![TerminalTab::new(0, "Terminal 1".to_string(), pane)],
                0,
                1,
                session.sidebar_visible,
            )
        } else {
            let pane_id = next_pane_id;
            next_pane_id += 1;
            let pty = PtyManager::new(
                pane_id,
                &config.terminal.default_shell,
                config.terminal.ghostty_prompt,
                cols as u16,
                rows as u16,
                None,
                proxy.clone(),
            )
            .expect("Error al inicializar Windows ConPTY");
            let terminal =
                TerminalState::new(cols, rows, config.terminal.scrollback_lines, theme.clone());
            let pane = TerminalPane::new(pane_id, pty, terminal, (cols, rows));
            (
                vec![TerminalTab::new(0, "Terminal 1".to_string(), pane)],
                0,
                1,
                true,
            )
        };

        let current_dir = std::env::current_dir().ok();
        let file_explorer = crate::explorer::FileExplorer::new(current_dir, explorer_visible);

        let clipboard = arboard::Clipboard::new().ok();

        Self {
            config,
            theme,
            proxy,
            tabs,
            active_tab_index,
            next_tab_id,
            next_pane_id,
            sidebar_visible,
            hovered_tab: None,
            hovered_close: None,
            hovered_new_tab: false,
            hovered_settings_btn: false,
            hovered_toggle_btn: false,
            hovered_floating_settings: false,
            settings_page_state: SettingsPageState::default(),
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
            is_focused: true,
            scrollbar_dragging: false,
            scrollbar_drag_pane_id: None,
            scrollbar_drag_offset_y: 0.0,
            last_scroll_time: Instant::now(),
            hovered_win_min: false,
            hovered_win_max: false,
            hovered_win_close: false,
            last_titlebar_click: Instant::now(),
            last_system_theme_check: Instant::now(),
        }
    }

    pub fn save_session(&self) {
        let tab_sessions = self
            .tabs
            .iter()
            .filter(|tab| !tab.is_settings)
            .map(|tab| {
                let cwd = tab
                    .active_pane()
                    .and_then(|p| p.terminal.current_dir.as_ref())
                    .map(|p| p.to_string_lossy().to_string())
                    .or_else(|| {
                        std::env::current_dir()
                            .ok()
                            .map(|p| p.to_string_lossy().to_string())
                    });
                crate::config::TabSession {
                    title: tab.title.clone(),
                    cwd,
                }
            })
            .collect();
        let state = crate::config::SessionState {
            active_tab_index: self.last_terminal_tab_index.min(
                self.tabs
                    .iter()
                    .filter(|t| !t.is_settings)
                    .count()
                    .saturating_sub(1),
            ),
            sidebar_visible: self.sidebar_visible,
            file_explorer_visible: self.file_explorer.visible,
            tabs: tab_sessions,
        };
        let _ = state.save();
    }

    pub fn open_or_switch_settings_tab(&mut self) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            if active_tab.is_settings {
                let target = if self.last_terminal_tab_index < self.tabs.len()
                    && !self.tabs[self.last_terminal_tab_index].is_settings
                {
                    self.last_terminal_tab_index
                } else {
                    self.tabs.iter().position(|t| !t.is_settings).unwrap_or(0)
                };
                self.switch_to_tab(target);
                return;
            }
        }

        self.last_terminal_tab_index = self.active_tab_index;

        if !self.is_horizontal_tabs() && !self.sidebar_visible {
            self.sidebar_visible = true;
        }

        if let Some(pos) = self.tabs.iter().position(|t| t.is_settings) {
            self.switch_to_tab(pos);
        } else {
            let tab_id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs.push(TerminalTab::new_settings(tab_id));
            self.switch_to_tab(self.tabs.len() - 1);
        }
    }

    pub fn toggle_restore_session(&mut self) {
        self.config.terminal.restore_session = !self.config.terminal.restore_session;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
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
                .map_or(true, |t| !t.is_settings)
        {
            26
        } else {
            0
        }
    }

    pub fn file_explorer_offset(&self) -> usize {
        if self.file_explorer.visible {
            crate::explorer::FILE_EXPLORER_WIDTH
        } else {
            0
        }
    }

    pub fn toggle_file_explorer(&mut self) {
        self.file_explorer.toggle();
        self.recompute_grid_for_all_tabs();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn create_new_tab(&mut self) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let (cols, rows) = if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            let avail_w = (size.width as usize)
                .saturating_sub(self.sidebar_offset() + self.file_explorer_offset());
            let avail_h =
                (size.height as usize).saturating_sub(self.top_offset() + self.status_bar_height());
            renderer.compute_pane_grid_size(avail_w, avail_h)
        } else {
            (100, 30)
        };

        let pty = match PtyManager::new(
            pane_id,
            &self.config.terminal.default_shell,
            self.config.terminal.ghostty_prompt,
            cols as u16,
            rows as u16,
            None,
            self.proxy.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                crate::logger::log(&format!("Error al spawnear nuevo PTY: {:?}", e));
                return;
            }
        };

        let title = format!("Terminal {}", self.tabs.len() + 1);
        let terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );

        let pane = TerminalPane::new(pane_id, pty, terminal, (cols, rows));

        self.tabs.push(TerminalTab::new(tab_id, title, pane));

        self.active_tab_index = self.tabs.len() - 1;
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_tab(&mut self, index: usize, event_loop: &ActiveEventLoop) {
        if index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando Axon");
            self.save_session();
            event_loop.exit();
            return;
        }

        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        self.tabs[self.active_tab_index].has_unread = false;
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn split_active_pane(&mut self, direction: SplitDirection) {
        if self.tabs[self.active_tab_index].is_settings {
            return;
        }
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let active_pane = self
            .tabs
            .get(self.active_tab_index)
            .and_then(|t| t.active_pane());
        let ssh_session = active_pane.and_then(|p| p.terminal.ssh_session.clone());

        // Heredar el directorio de trabajo del pane activo
        let cwd_path = active_pane.and_then(|p| p.terminal.current_dir.clone());
        let cwd_ref = cwd_path.as_deref();

        let (cols, rows) = (80, 24);

        let (pty, initial_ssh) = if let Some(ref ssh) = ssh_session {
            if ssh.is_active {
                crate::logger::log(&format!(
                    "Split pane: reconectando automáticamente vía SSH a {}",
                    ssh.destination
                ));
                match PtyManager::new_ssh(
                    new_pane_id,
                    &ssh.destination,
                    ssh.remote_dir.as_deref(),
                    cols as u16,
                    rows as u16,
                    self.proxy.clone(),
                ) {
                    Ok(p) => (p, Some(ssh.clone())),
                    Err(e) => {
                        crate::logger::log(&format!(
                            "Fallo al conectar SSH para split: {:?}, usando local",
                            e
                        ));
                        let local_p = match PtyManager::new(
                            new_pane_id,
                            &self.config.terminal.default_shell,
                            self.config.terminal.ghostty_prompt,
                            cols as u16,
                            rows as u16,
                            cwd_ref,
                            self.proxy.clone(),
                        ) {
                            Ok(p) => p,
                            Err(e2) => {
                                crate::logger::log(&format!(
                                    "Error al spawnear nuevo PTY para split: {:?}",
                                    e2
                                ));
                                return;
                            }
                        };
                        (local_p, None)
                    }
                }
            } else {
                let local_p = match PtyManager::new(
                    new_pane_id,
                    &self.config.terminal.default_shell,
                    self.config.terminal.ghostty_prompt,
                    cols as u16,
                    rows as u16,
                    cwd_ref,
                    self.proxy.clone(),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        crate::logger::log(&format!(
                            "Error al spawnear nuevo PTY para split: {:?}",
                            e
                        ));
                        return;
                    }
                };
                (local_p, None)
            }
        } else {
            let local_p = match PtyManager::new(
                new_pane_id,
                &self.config.terminal.default_shell,
                self.config.terminal.ghostty_prompt,
                cols as u16,
                rows as u16,
                cwd_ref,
                self.proxy.clone(),
            ) {
                Ok(p) => p,
                Err(e) => {
                    crate::logger::log(&format!("Error al spawnear nuevo PTY para split: {:?}", e));
                    return;
                }
            };
            (local_p, None)
        };

        let mut terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );
        terminal.current_dir = cwd_path;
        terminal.ssh_session = initial_ssh;

        let new_pane = TerminalPane::new(new_pane_id, pty, terminal, (cols, rows));

        let active_tab = &mut self.tabs[self.active_tab_index];
        active_tab.split_active_pane(new_pane_id, new_pane, direction);

        self.recompute_grid_for_all_tabs();
        self.update_window_title();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_active_pane(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            if active_tab.is_settings {
                self.close_tab(self.active_tab_index, event_loop);
                return;
            }
            let active_pane_id = active_tab.active_pane_id;
            self.close_pane_by_id(active_pane_id, event_loop);
        }
    }

    pub fn close_pane_by_id(&mut self, pane_id: usize, event_loop: &ActiveEventLoop) {
        let mut tab_to_close = None;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if tab.panes.iter().any(|p| p.id == pane_id) {
                if tab.panes.len() <= 1 {
                    tab_to_close = Some(idx);
                } else {
                    tab.close_pane(pane_id);
                    self.recompute_grid_for_all_tabs();
                    self.update_window_title();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                break;
            }
        }

        if let Some(idx) = tab_to_close {
            self.close_tab(idx, event_loop);
        }
    }

    pub fn navigate_pane(&mut self, dir: PaneDirection) {
        if self.tabs[self.active_tab_index].is_settings {
            return;
        }
        if let (Some(window), Some(_renderer)) = (&self.window, &self.renderer) {
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

            let active_tab = &mut self.tabs[self.active_tab_index];
            let rects = active_tab.compute_pane_rects(offset, top_offset, total_w, total_h);
            if rects.len() <= 1 {
                return;
            }

            let cur_rect = match rects
                .iter()
                .find(|r| r.pane_id == active_tab.active_pane_id)
            {
                Some(r) => *r,
                None => return,
            };

            let cx = cur_rect.x as isize + (cur_rect.width as isize / 2);
            let cy = cur_rect.y as isize + (cur_rect.height as isize / 2);

            let mut best_candidate: Option<(usize, isize)> = None;

            for r in &rects {
                if r.pane_id == active_tab.active_pane_id {
                    continue;
                }
                let rcx = r.x as isize + (r.width as isize / 2);
                let rcy = r.y as isize + (r.height as isize / 2);

                let is_valid = match dir {
                    PaneDirection::Up => rcy < cy,
                    PaneDirection::Down => rcy > cy,
                    PaneDirection::Left => rcx < cx,
                    PaneDirection::Right => rcx > cx,
                };

                if is_valid {
                    let dist = match dir {
                        PaneDirection::Up => (cy - rcy) * 2 + (cx - rcx).abs(),
                        PaneDirection::Down => (rcy - cy) * 2 + (cx - rcx).abs(),
                        PaneDirection::Left => (cx - rcx) * 2 + (cy - rcy).abs(),
                        PaneDirection::Right => (rcx - cx) * 2 + (cy - rcy).abs(),
                    };
                    if best_candidate.map_or(true, |(_, best)| dist < best) {
                        best_candidate = Some((r.pane_id, dist));
                    }
                }
            }

            if let Some((next_id, _)) = best_candidate {
                active_tab.active_pane_id = next_id;
                window.request_redraw();
            }
        }
    }

    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab_index {
            if !self.tabs[self.active_tab_index].is_settings {
                self.last_terminal_tab_index = self.active_tab_index;
            }
            self.active_tab_index = index;
            self.tabs[index].has_unread = false;
            self.update_window_title();
            self.save_session();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            self.switch_to_tab(next);
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            self.switch_to_tab(prev);
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.recompute_grid_for_all_tabs();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn update_window_title(&self) {
        if let (Some(w), Some(tab)) = (&self.window, self.tabs.get(self.active_tab_index)) {
            if tab.is_settings {
                w.set_title("Axon - ⚙ Ajustes");
                return;
            }
            let title_text = tab.display_title();

            let pane_info = if tab.panes.len() > 1 {
                format!(" ({} paneles)", tab.panes.len())
            } else {
                String::new()
            };

            let full_title = format!("Axon - {}{}", title_text, pane_info);

            w.set_title(&full_title);
        }
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
                if tab.is_settings {
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

    fn paste_text(&self, text: &str) {
        let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
        let active_tab = &self.tabs[self.active_tab_index];
        if let Some(active_pane) = active_tab.active_pane() {
            if active_pane.terminal.bracketed_paste_mode {
                let mut payload = Vec::with_capacity(normalized.len() + 16);
                payload.extend_from_slice(b"\x1b[200~");
                payload.extend_from_slice(normalized.as_bytes());
                payload.extend_from_slice(b"\x1b[201~");
                active_pane.pty.write_input(&payload);
            } else {
                active_pane.pty.write_input(normalized.as_bytes());
            }
        }
    }

    /// Pegar desde el portapapeles con soporte inteligente para texto, imágenes de IA y archivos.
    /// Si el portapapeles contiene una imagen (captura Snipping Tool, Win+Shift+S, recorte web),
    /// la guarda automáticamente en un archivo PNG temporal y pega la ruta entrecomillada
    /// en el prompt de la terminal para consumo directo por agentes de IA (Claude Code, Aider, etc.).
    fn paste_from_clipboard(&mut self) {
        if self.tabs[self.active_tab_index].is_settings {
            return;
        }
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = &mut self.clipboard else {
            return;
        };

        // 1. Si hay texto en el portapapeles (código, comandos, texto plano), pegar directamente
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                self.paste_text(&text);
                return;
            }
        }

        // 2. Si no hay texto, comprobar si hay una imagen (captura de pantalla, recorte, imagen web)
        if let Ok(img) = cb.get_image() {
            let active_pane = self
                .tabs
                .get(self.active_tab_index)
                .and_then(|t| t.active_pane());
            let ssh_session = active_pane.and_then(|p| p.terminal.ssh_session.clone());
            let is_ssh = ssh_session.as_ref().map(|s| s.is_active).unwrap_or(false);

            let is_claude = is_claude_running() || {
                active_pane
                    .and_then(|p| p.terminal.window_title.as_ref())
                    .map(|title| title.to_lowercase().contains("claude"))
                    .unwrap_or(false)
            };

            // Si Claude Code (local o remoto vía SSH) está activo, enviamos la secuencia
            // nativa de pegado de imágenes (Alt + V: \x1bv).
            if is_claude {
                if let Some(pane) = active_pane {
                    pane.pty.write_input(b"\x1bv");
                }
                return;
            }

            // Si estamos en una sesión remota SSH, transferimos la imagen mediante scp
            // y pegamos la ruta remota /tmp/axon_img_... en lugar de una ruta local de Windows
            if is_ssh {
                if let Some(session) = ssh_session {
                    if let Some(local_path) = self.save_clipboard_image(&img) {
                        if let Some(remote_path) =
                            self.upload_clipboard_image_ssh(&local_path, &session.destination)
                        {
                            let formatted = format!("\"{}\" ", remote_path);
                            self.paste_text(&formatted);
                            return;
                        }
                    }
                }
            }

            // Si Claude Code no está corriendo y es una sesión local,
            // guardamos la captura en el directorio temporal y pegamos la ruta formateada.
            if let Some(path_str) = self.save_clipboard_image(&img) {
                let formatted = format!("\"{}\" ", path_str);
                self.paste_text(&formatted);
                return;
            }
        }

        // 3. Si no hay texto ni bitmap directo, comprobar si se copiaron archivos desde el Explorador de Windows (CF_HDROP)
        if let Some(files) = get_clipboard_files() {
            let formatted = files
                .into_iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(" ");
            if !formatted.is_empty() {
                let with_space = format!("{} ", formatted);
                self.paste_text(&with_space);
            }
        }
    }

    /// Guarda una imagen del portapapeles en la carpeta temporal de Axon como archivo PNG.
    /// Realiza limpieza pasiva de archivos temporales con más de 24 horas de antigüedad.
    pub(crate) fn save_clipboard_image(&self, img: &arboard::ImageData) -> Option<String> {
        let clip_dir = std::env::temp_dir().join("axon_clipboard");
        let _ = std::fs::create_dir_all(&clip_dir);

        // Limpieza pasiva de capturas antiguas (>24 horas)
        if let Ok(entries) = std::fs::read_dir(&clip_dir) {
            let now = std::time::SystemTime::now();
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age.as_secs() > 86400 {
                                let _ = std::fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_path = clip_dir.join(format!("axon_clip_{}.png", timestamp));

        let width = img.width as u32;
        let height = img.height as u32;
        let raw_bytes = img.bytes.to_vec();

        if let Some(rgba_img) = image::RgbaImage::from_raw(width, height, raw_bytes) {
            if rgba_img.save(&file_path).is_ok() {
                return Some(file_path.to_string_lossy().to_string());
            }
        }

        None
    }

    /// Transfiere de forma segura y transparente una captura de pantalla a un host remoto SSH vía scp
    pub(crate) fn upload_clipboard_image_ssh(
        &self,
        local_path: &str,
        destination: &str,
    ) -> Option<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let remote_filename = format!("axon_img_{}.png", timestamp);
        let remote_target = format!("{}:/tmp/{}", destination, remote_filename);

        crate::logger::log(&format!(
            "SSH Upload: transfiriendo captura a {} -> /tmp/{}",
            destination, remote_filename
        ));

        let status = std::process::Command::new("scp.exe")
            .arg("-q")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("ConnectTimeout=3")
            .arg(local_path)
            .arg(&remote_target)
            .status();

        match status {
            Ok(s) if s.success() => {
                crate::logger::log(&format!("SSH Upload exitoso: /tmp/{}", remote_filename));
                Some(format!("/tmp/{}", remote_filename))
            }
            Ok(s) => {
                crate::logger::log(&format!("SSH Upload falló con código: {:?}", s.code()));
                None
            }
            Err(e) => {
                crate::logger::log(&format!("Error al ejecutar scp.exe: {:?}", e));
                None
            }
        }
    }

    fn adjust_font_size(&mut self, delta: f32) {
        let new_size = (self.config.appearance.font_size + delta).clamp(8.0, 32.0);
        if (new_size - self.config.appearance.font_size).abs() > 0.01 {
            self.config.appearance.font_size = new_size;
            if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                let scale_factor = window.scale_factor();
                renderer.update_scale(&self.config, scale_factor);
            }
            self.recompute_grid_for_all_tabs();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn reset_font_size(&mut self) {
        let default_cfg = AxonConfig::load_or_default();
        let target_size = default_cfg.appearance.font_size;
        self.config.appearance.font_size = target_size;
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn reload_config(&mut self) {
        self.config = AxonConfig::load_or_default();
        self.theme = Theme::from_name(&self.config.appearance.theme);
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    #[allow(dead_code)]
    pub fn toggle_settings(&mut self) {
        self.open_or_switch_settings_tab();
    }

    #[allow(dead_code)]
    pub fn cycle_theme(&mut self, next: bool) {
        let themes = crate::theme::Theme::AVAILABLE_THEMES;
        let current = &self.config.appearance.theme;
        let current_idx = themes
            .iter()
            .position(|&t| t.eq_ignore_ascii_case(current))
            .unwrap_or(0);
        let new_idx = if next {
            (current_idx + 1) % themes.len()
        } else {
            (current_idx + themes.len() - 1) % themes.len()
        };
        let new_theme = themes[new_idx];
        self.apply_theme(new_theme);
    }

    pub fn apply_theme(&mut self, theme_name: &str) {
        self.config.appearance.theme = theme_name.to_string();
        self.theme = Theme::from_name(theme_name);
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        let _ = self.config.save();
        if let Some(w) = &self.window {
            update_windows_theme(w, &self.theme);
            w.request_redraw();
        }
    }

    pub fn set_theme_mode(&mut self, mode: &str) {
        self.config.appearance.theme_mode = mode.to_string();
        if mode.eq_ignore_ascii_case("system") {
            let is_light = crate::theme::is_windows_system_light_mode();
            self.apply_theme(if is_light { "latte" } else { "ghostty" });
        } else if mode.eq_ignore_ascii_case("light") {
            self.apply_theme("latte");
        } else {
            if self.theme.is_light() {
                self.apply_theme("ghostty");
            }
        }
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn set_tab_layout(&mut self, layout: &str) {
        self.config.appearance.tab_layout = layout.to_string();
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_status_bar(&mut self) {
        self.config.appearance.show_status_bar = !self.config.appearance.show_status_bar;
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn adjust_opacity(&mut self, delta: f32) {
        let new_op = (self.config.appearance.window_opacity + delta).clamp(0.50, 1.0);
        self.config.appearance.window_opacity = (new_op * 100.0).round() / 100.0;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn adjust_line_spacing(&mut self, delta: f32) {
        let new_sp = (self.config.appearance.line_spacing + delta).clamp(0.8, 2.0);
        self.config.appearance.line_spacing = (new_sp * 10.0).round() / 10.0;
        let _ = self.config.save();
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_cursor_blink(&mut self) {
        self.config.terminal.cursor_blink = !self.config.terminal.cursor_blink;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_ghostty_prompt(&mut self) {
        self.config.terminal.ghostty_prompt = !self.config.terminal.ghostty_prompt;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_copy_on_select(&mut self) {
        self.config.terminal.copy_on_select = !self.config.terminal.copy_on_select;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn open_config_file(&self) {
        let path = AxonConfig::config_path();
        let _ = self.config.save();
        let path_str = path.to_string_lossy().to_string();
        crate::logger::log(&format!(
            "-> Abriendo archivo de configuración: {}",
            path_str
        ));
        let res = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path_str])
            .spawn();
        if res.is_err() {
            let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
        }
    }

    pub fn reset_to_defaults(&mut self) {
        crate::logger::log("-> Restableciendo configuración a valores por defecto");
        self.config = AxonConfig::default();
        let _ = self.config.save();
        self.theme = if self.config.appearance.theme_mode.eq_ignore_ascii_case("system") {
            if crate::theme::is_windows_system_light_mode() {
                Theme::catppuccin_latte()
            } else {
                Theme::from_name(&self.config.appearance.theme)
            }
        } else if self.config.appearance.theme_mode.eq_ignore_ascii_case("light") {
            Theme::catppuccin_latte()
        } else {
            Theme::from_name(&self.config.appearance.theme)
        };
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
            update_windows_theme(window, &self.theme);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler<AxonEvent> for AxonApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AxonEvent) {
        match event {
            AxonEvent::PtyData(pane_id) => {
                for (tab_idx, tab) in self.tabs.iter_mut().enumerate() {
                    if let Some(pane) = tab.find_pane_mut(pane_id) {
                        let mut got_bytes = false;
                        while let Some(bytes) = pane.pty.try_recv_output() {
                            got_bytes = true;
                            pane.terminal.process_bytes(&bytes);
                        }
                        for text in pane.terminal.clipboard_queue.drain(..) {
                            if let Some(cb) = &mut self.clipboard {
                                cb.set_text(text).ok();
                            }
                        }
                        if got_bytes {
                            if tab_idx != self.active_tab_index {
                                tab.has_unread = true;
                            } else {
                                self.update_window_title();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        break;
                    }
                }
            }
            AxonEvent::PtyClosed(pane_id) => {
                crate::logger::log(&format!(
                    "PtyClosed recibido para pane_id {}: cerrando pane",
                    pane_id
                ));
                self.close_pane_by_id(pane_id, event_loop);
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Crear icono nativo de Axon (Ghostty Green '>' sin fondo)
        let icon = create_native_icon();

        let window_attributes = WindowAttributes::default()
            .with_title("Axon")
            .with_visible(false)
            .with_inner_size(PhysicalSize::new(1080, 680))
            .with_min_inner_size(PhysicalSize::new(540, 320))
            .with_decorations(false)
            .with_resizable(true)
            .with_window_icon(Some(icon));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Error al crear ventana nativa de Windows"),
        );

        // Configurar tema nativo inmersivo de Windows 11 (DWM Dark/Light Mode y Mica)
        update_windows_theme(&window, &self.theme);

        let context = softbuffer::Context::new(window.clone())
            .expect("Error al inicializar contexto de aceleración gráfica de pantalla");
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .expect("Error al inicializar superficie de renderizado nativo");

        let scale_factor = window.scale_factor();
        let mut renderer = TerminalRenderer::new(&self.config, scale_factor);

        let target_cols = 100;
        let target_rows = 35;
        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();

        let target_pane_w = (target_cols * renderer.char_width) + (renderer.margin_x * 2);
        let target_pane_h = (target_rows * renderer.row_height) + (renderer.margin_y * 2);
        let init_win_w = (target_pane_w + offset + right_offset) as u32;
        let init_win_h = (target_pane_h + top_offset + status_bar_h) as u32;

        let _ = window.request_inner_size(PhysicalSize::new(init_win_w, init_win_h));
        let initial_size = window.inner_size();
        if let (Some(w), Some(h)) = (
            NonZeroU32::new(initial_size.width),
            NonZeroU32::new(initial_size.height),
        ) {
            let _ = surface.resize(w, h);
        }

        let total_w = (initial_size.width as usize).saturating_sub(offset + right_offset);
        let total_h = (initial_size.height as usize).saturating_sub(top_offset + status_bar_h);
        for tab in &mut self.tabs {
            let pane_rects = tab.compute_pane_rects(offset, top_offset, total_w, total_h);
            for rect in pane_rects {
                if let Some(pane) = tab.find_pane_mut(rect.pane_id) {
                    let (cols, rows) = renderer.compute_pane_grid_size(rect.width, rect.height);
                    pane.last_grid_size = (cols, rows);
                    pane.terminal.resize(cols, rows);
                    pane.pty.resize(cols as u16, rows as u16);
                }
            }
        }

        // Pre-renderizar el primer frame oscuro en la superficie antes de hacer visible la ventana.
        // Esto garantiza que el usuario vea la terminal oscura de inmediato sin ningún destello blanco.
        if let Ok(mut buffer) = surface.buffer_mut() {
            buffer.fill(self.theme.background.to_u32_xrgb());
            if self.sidebar_visible {
                let sidebar_tabs: Vec<SidebarTabItem> = self
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| SidebarTabItem {
                        id: t.id,
                        title: t.display_title(),
                        is_active: idx == self.active_tab_index,
                        has_unread: t.has_unread,
                        pane_count: t.panes.len(),
                        is_settings: t.is_settings,
                    })
                    .collect();

                renderer.render_sidebar(
                    &mut buffer,
                    initial_size.width as usize,
                    initial_size.height as usize,
                    SIDEBAR_WIDTH,
                    status_bar_h,
                    &sidebar_tabs,
                    self.hovered_tab,
                    self.hovered_close,
                    self.hovered_new_tab,
                    self.hovered_settings_btn,
                    self.hovered_toggle_btn,
                    &self.theme,
                );
            }

            let active_tab = &self.tabs[self.active_tab_index];
            let pane_rects = active_tab.compute_pane_rects(offset, top_offset, total_w, total_h);
            let dividers = active_tab.compute_dividers(offset, top_offset, total_w, total_h);
            let has_multiple = pane_rects.len() > 1;

            for rect in &pane_rects {
                if let Some(pane) = active_tab.find_pane(rect.pane_id) {
                    let is_active = rect.pane_id == active_tab.active_pane_id;
                    renderer.render_pane(
                        &mut buffer,
                        initial_size.width as usize,
                        initial_size.height as usize,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        &pane.terminal,
                        &self.theme,
                        true,
                        is_active,
                        has_multiple,
                    );
                }
            }

            let divider_color = crate::theme::Color::from_rgb(38, 38, 43);
            for (dx, dy, dw, dh) in dividers {
                TerminalRenderer::render_pane_divider(
                    &mut buffer,
                    initial_size.width as usize,
                    initial_size.height as usize,
                    dx,
                    dy,
                    dw,
                    dh,
                    divider_color,
                );
            }

            buffer.present().ok();
        }

        // Mostrar la ventana una vez que el primer frame oscuro ya está listo y dibujado
        window.set_visible(true);

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.save_session();
                event_loop.exit();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(_window)) = (&mut self.renderer, &self.window) {
                    renderer.update_scale(&self.config, scale_factor);
                }
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    if let (Some(w), Some(h)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    {
                        let _ = surface.resize(w, h);
                    }
                }
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let is_horizontal = self.is_horizontal_tabs();
                let offset = self.sidebar_offset();
                let right_offset = self.file_explorer_offset();
                let explorer_visible = self.file_explorer.visible;
                let top_offset = self.top_offset();
                let status_bar_h = self.status_bar_height();
                let sidebar_visible = self.sidebar_visible && !is_horizontal;
                let active_idx = self.active_tab_index;
                let hovered_tab = self.hovered_tab;
                let hovered_close = self.hovered_close;
                let hovered_new_tab = self.hovered_new_tab;
                let hovered_files_btn = self.hovered_files_btn;
                let hovered_settings_btn = self.hovered_settings_btn;
                let hovered_toggle_btn = self.hovered_toggle_btn;
                let cursor_visible = !self.config.terminal.cursor_blink
                    || (self.cursor_timer.elapsed().as_millis() % 1000) < 600;

                let tab_items: Vec<SidebarTabItem> = self
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| SidebarTabItem {
                        id: t.id,
                        title: t.display_title(),
                        is_active: idx == active_idx,
                        has_unread: t.has_unread,
                        pane_count: t.panes.len(),
                        is_settings: t.is_settings,
                    })
                    .collect();

                let active_tab = &mut self.tabs[active_idx];

                if let (Some(surface), Some(renderer), Some(window)) =
                    (&mut self.surface, &mut self.renderer, &self.window)
                {
                    let size = window.inner_size();
                    if size.width == 0 || size.height == 0 {
                        return;
                    }

                    let is_maximized = window.is_maximized();
                    if let Ok(mut buffer) = surface.buffer_mut() {
                        buffer.fill(self.theme.background.to_u32_xrgb());

                        if is_horizontal {
                            renderer.render_top_tab_bar(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                &tab_items,
                                hovered_tab,
                                hovered_close,
                                hovered_new_tab,
                                hovered_files_btn,
                                hovered_settings_btn,
                                self.hovered_win_min,
                                self.hovered_win_max,
                                self.hovered_win_close,
                                is_maximized,
                                explorer_visible,
                                &self.theme,
                            );
                        } else {
                            let active_title = active_tab.display_title();
                            let git_branch = active_tab.active_pane_mut().and_then(|p| p.get_git_branch());
                            renderer.render_top_title_bar(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                &active_title,
                                git_branch.as_deref(),
                                hovered_settings_btn,
                                self.hovered_win_min,
                                self.hovered_win_max,
                                self.hovered_win_close,
                                is_maximized,
                                &self.theme,
                            );

                            if sidebar_visible {
                                renderer.render_sidebar(
                                    &mut buffer,
                                    size.width as usize,
                                    size.height as usize,
                                    SIDEBAR_WIDTH,
                                    status_bar_h,
                                    &tab_items,
                                    hovered_tab,
                                    hovered_close,
                                    hovered_new_tab,
                                    hovered_settings_btn,
                                    hovered_toggle_btn,
                                    &self.theme,
                                );
                            }
                        }

                        if active_tab.is_settings {
                            renderer.render_settings_page(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                offset,
                                top_offset,
                                &self.config,
                                &self.settings_page_state,
                                &self.theme,
                            );
                        } else {
                            let term_area_w =
                                (size.width as usize).saturating_sub(offset + right_offset);
                            let term_area_h =
                                (size.height as usize).saturating_sub(top_offset + status_bar_h);
                            let pane_rects = active_tab.compute_pane_rects(
                                offset,
                                top_offset,
                                term_area_w,
                                term_area_h,
                            );
                            let dividers = active_tab.compute_dividers(
                                offset,
                                top_offset,
                                term_area_w,
                                term_area_h,
                            );
                            let has_multiple = pane_rects.len() > 1;

                            for rect in &pane_rects {
                                if let Some(pane) = active_tab.find_pane(rect.pane_id) {
                                    let is_active = rect.pane_id == active_tab.active_pane_id;
                                    renderer.render_pane(
                                        &mut buffer,
                                        size.width as usize,
                                        size.height as usize,
                                        rect.x,
                                        rect.y,
                                        rect.width,
                                        rect.height,
                                        &pane.terminal,
                                        &self.theme,
                                        cursor_visible,
                                        is_active,
                                        has_multiple,
                                    );
                                }
                            }

                            let divider_color = self.theme.border_color();
                            for (dx, dy, dw, dh) in dividers {
                                TerminalRenderer::render_pane_divider(
                                    &mut buffer,
                                    size.width as usize,
                                    size.height as usize,
                                    dx,
                                    dy,
                                    dw,
                                    dh,
                                    divider_color,
                                );
                            }

                            if self.file_explorer.visible {
                                renderer.render_file_explorer(
                                    &mut buffer,
                                    size.width as usize,
                                    size.height as usize,
                                    top_offset,
                                    status_bar_h,
                                    &self.file_explorer,
                                    &self.theme,
                                );
                            }
                        }

                        if !is_horizontal
                            && !sidebar_visible
                            && (hovered_toggle_btn || self.hovered_floating_settings)
                        {
                            renderer.render_floating_sidebar_toggle(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                status_bar_h,
                                hovered_toggle_btn,
                                self.hovered_floating_settings,
                                &self.theme,
                            );
                        }

                        if self.config.appearance.show_status_bar && !active_tab.is_settings {
                            let (cwd, git_branch, ssh_host, cols, rows, ai_status) =
                                if let Some(pane) = active_tab.active_pane_mut() {
                                    let c_dir = pane.terminal.current_dir.as_ref().map(|p| {
                                        let s = p.to_string_lossy().to_string();
                                        if let Ok(userprofile) = std::env::var("USERPROFILE") {
                                            if s.starts_with(&userprofile) {
                                                return format!("~{}", &s[userprofile.len()..]);
                                            }
                                        }
                                        s
                                    });
                                    let g_branch = if pane.terminal.current_dir != pane.cached_git_dir {
                                        pane.cached_git_branch =
                                            get_git_branch(pane.terminal.current_dir.as_deref());
                                        pane.cached_git_dir = pane.terminal.current_dir.clone();
                                        pane.cached_git_branch.clone()
                                    } else {
                                        pane.cached_git_branch.clone()
                                    };
                                    let s_host = pane
                                        .terminal
                                        .ssh_session
                                        .as_ref()
                                        .filter(|s| s.is_active)
                                        .map(|s| s.destination.clone());
                                    let ai = detect_ai_agent(pane.terminal.window_title.as_deref());
                                    (
                                        c_dir,
                                        g_branch,
                                        s_host,
                                        pane.terminal.cols,
                                        pane.terminal.rows,
                                        ai,
                                    )
                                } else {
                                    (None, None, None, 100, 30, AiAgentStatus::None)
                                };

                            renderer.render_status_bar(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                offset,
                                cwd.as_deref(),
                                git_branch.as_deref(),
                                ssh_host.as_deref(),
                                cols,
                                rows,
                                ai_status,
                                explorer_visible,
                                &self.theme,
                            );
                        }

                        buffer.present().ok();
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        text,
                        state: ElementState::Pressed,
                        physical_key,
                        ..
                    },
                ..
            } => {
                // Si la terminal no tiene el foco activo, ignorar la entrada de teclado
                if !self.is_focused {
                    return;
                }

                #[cfg(target_os = "windows")]
                let (ctrl, shift, alt, altgr) = get_physical_modifiers();

                #[cfg(not(target_os = "windows"))]
                let (ctrl, shift, alt, altgr) = (
                    self.modifiers.control_key(),
                    self.modifiers.shift_key(),
                    self.modifiers.alt_key(),
                    false,
                );

                crate::logger::log(&format!(
                    "KEY: phys={:?}, log={:?}, text={:?}, ctrl={}, shift={}, alt={}, altgr={}",
                    physical_key, logical_key, text, ctrl, shift, alt, altgr
                ));

                // Si AltGr está presionado (teclados en español para @, #, \, ~, etc.),
                // procesar exclusivamente como entrada de texto si genera caracteres imprimibles
                let mut alt = alt;
                let mut ctrl = ctrl;
                if altgr {
                    if let Some(ref t) = text {
                        if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                            crate::logger::log(&format!("-> AltGr text: {:?}", t));
                            if let Some(active_pane) =
                                self.tabs[self.active_tab_index].active_pane()
                            {
                                active_pane.pty.write_input(t.as_bytes());
                            }
                            return;
                        }
                    }
                    // Si AltGr no generó texto imprimible (teclas como D, V, W, etc.),
                    // en Windows AltGr emula por hardware/driver la combinación Ctrl + Alt.
                    // Permitir que atajos como Ctrl + Alt + D se activen limpiamente con Right Alt / AltGr.
                    alt = true;
                    ctrl = true;
                }

                // Alt + Tab es el atajo de cambio de tarea de Windows; NUNCA debe enviar \t al shell
                if alt && logical_key == Key::Named(NamedKey::Tab) {
                    crate::logger::log("-> Ignorado Alt+Tab");
                    return;
                }

                // Si estamos en la pestaña de Ajustes, capturar Escape, Ctrl+, o Ctrl+W para cerrarla/alternarla
                let active_is_settings = self
                    .tabs
                    .get(self.active_tab_index)
                    .map(|t| t.is_settings)
                    .unwrap_or(false);
                if active_is_settings {
                    if matches!(&logical_key, Key::Named(NamedKey::Escape))
                        || (ctrl && !shift && key_is_char(&logical_key, physical_key, ','))
                    {
                        crate::logger::log("-> Cerrando / alternando pestaña de Ajustes");
                        self.open_or_switch_settings_tab();
                        return;
                    }
                    if ctrl && !shift && key_is_char(&logical_key, physical_key, 'w') {
                        crate::logger::log("-> Cerrando pestaña de Ajustes con Ctrl+W");
                        self.close_tab(self.active_tab_index, event_loop);
                        return;
                    }
                }

                // Abrir / alternar pestaña de Ajustes (Ctrl + ,)
                if ctrl && !shift && key_is_char(&logical_key, physical_key, ',') {
                    crate::logger::log("-> Hotkey: Abrir / alternar Ajustes (Ctrl+,)");
                    self.open_or_switch_settings_tab();
                    return;
                }

                // --- ATAJOS DE PANELES DIVIDIDOS (SPLIT PANES) CANÓNICOS ---
                let is_d = key_is_char(&logical_key, physical_key, 'd');
                let is_v = key_is_char(&logical_key, physical_key, 'v');
                let is_w = key_is_char(&logical_key, physical_key, 'w');

                // 1. División Horizontal: Ctrl + Alt + D -> Lanzar terminal debajo
                if ctrl && alt && is_d {
                    crate::logger::log("-> Hotkey: Split Horizontal (Ctrl+Alt+D)");
                    self.split_active_pane(SplitDirection::Horizontal);
                    return;
                }

                // 2. División Vertical: Ctrl + Alt + V -> Lanzar terminal al lado
                if ctrl && alt && is_v {
                    crate::logger::log("-> Hotkey: Split Vertical (Ctrl+Alt+V)");
                    self.split_active_pane(SplitDirection::Vertical);
                    return;
                }

                // 3. Cerrar panel activo en split: Ctrl + Alt + W
                if ctrl && alt && is_w {
                    crate::logger::log("-> Hotkey: Cerrar panel activo (Ctrl+Alt+W)");
                    self.close_active_pane(event_loop);
                    return;
                }

                // 4. Navegación direccional entre paneles: Ctrl + Alt + Flechas
                if ctrl && alt {
                    let is_up = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowUp))
                        || logical_key == Key::Named(NamedKey::ArrowUp);
                    let is_down = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowDown))
                        || logical_key == Key::Named(NamedKey::ArrowDown);
                    let is_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                        || logical_key == Key::Named(NamedKey::ArrowLeft);
                    let is_right = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                        || logical_key == Key::Named(NamedKey::ArrowRight);

                    if is_up {
                        self.navigate_pane(PaneDirection::Up);
                        return;
                    }
                    if is_down {
                        self.navigate_pane(PaneDirection::Down);
                        return;
                    }
                    if is_left {
                        self.navigate_pane(PaneDirection::Left);
                        return;
                    }
                    if is_right {
                        self.navigate_pane(PaneDirection::Right);
                        return;
                    }
                }

                // --- ATAJOS DE GESTIÓN DE PESTAÑAS (TABS & SIDEBAR) CANÓNICOS ---
                // 1. Nueva pestaña / terminal: Ctrl + T
                if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 't') {
                    crate::logger::log("-> Hotkey: Nueva terminal (Ctrl+T)");
                    self.create_new_tab();
                    return;
                }

                // 2. Cerrar pestaña activa: Ctrl + W
                if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 'w') {
                    crate::logger::log("-> Hotkey: Cerrar pestaña activa (Ctrl+W)");
                    self.close_tab(self.active_tab_index, event_loop);
                    return;
                }

                // 3. Conmutar pestaña siguiente / anterior: Ctrl + Tab / Ctrl + Shift + Tab
                if ctrl && logical_key == Key::Named(NamedKey::Tab) {
                    if shift {
                        crate::logger::log("-> Hotkey: Pestaña anterior");
                        self.prev_tab();
                    } else {
                        crate::logger::log("-> Hotkey: Pestaña siguiente");
                        self.next_tab();
                    }
                    return;
                }

                // 4. Cambiar directamente a pestaña 1-9: Ctrl + 1..9
                if ctrl && !shift && !alt {
                    let tab_num = match &logical_key {
                        Key::Character(s) if s.len() == 1 => {
                            let ch = s.chars().next().unwrap();
                            if ch >= '1' && ch <= '9' {
                                Some((ch as usize) - ('1' as usize))
                            } else {
                                None
                            }
                        }
                        _ => match physical_key {
                            PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => Some(0),
                            PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => Some(1),
                            PhysicalKey::Code(KeyCode::Digit3 | KeyCode::Numpad3) => Some(2),
                            PhysicalKey::Code(KeyCode::Digit4 | KeyCode::Numpad4) => Some(3),
                            PhysicalKey::Code(KeyCode::Digit5 | KeyCode::Numpad5) => Some(4),
                            PhysicalKey::Code(KeyCode::Digit6 | KeyCode::Numpad6) => Some(5),
                            PhysicalKey::Code(KeyCode::Digit7 | KeyCode::Numpad7) => Some(6),
                            PhysicalKey::Code(KeyCode::Digit8 | KeyCode::Numpad8) => Some(7),
                            PhysicalKey::Code(KeyCode::Digit9 | KeyCode::Numpad9) => Some(8),
                            _ => None,
                        },
                    };
                    if let Some(idx) = tab_num {
                        if idx < self.tabs.len() {
                            crate::logger::log(&format!("-> Hotkey: Ir a pestaña {}", idx + 1));
                            self.switch_to_tab(idx);
                            return;
                        }
                    }
                }

                // 5. Alternar barra lateral (Sidebar toggle): Ctrl + B
                if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 'b') {
                    crate::logger::log("-> Hotkey: Alternar barra lateral (Ctrl+B)");
                    self.toggle_sidebar();
                    return;
                }

                // Alternar explorador lateral de archivos: Ctrl + Shift + E
                if ctrl && shift && !alt && key_is_char(&logical_key, physical_key, 'e') {
                    crate::logger::log("-> Hotkey: Alternar explorador de archivos (Ctrl+Shift+E)");
                    self.toggle_file_explorer();
                    return;
                }

                // 6. Pegar desde el portapapeles: Ctrl + V
                if ctrl && !shift && key_is_char(&logical_key, physical_key, 'v') {
                    self.paste_from_clipboard();
                    return;
                }

                // 7. Zoom dinámico de tipografía (Ctrl + +, Ctrl + -, Ctrl + 0)
                if ctrl && !shift {
                    if key_is_char(&logical_key, physical_key, '+')
                        || key_is_char(&logical_key, physical_key, '=')
                    {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, physical_key, '-')
                        || key_is_char(&logical_key, physical_key, '_')
                    {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                    if key_is_char(&logical_key, physical_key, '0') {
                        self.reset_font_size();
                        return;
                    }
                } else if ctrl && shift {
                    if key_is_char(&logical_key, physical_key, '+')
                        || key_is_char(&logical_key, physical_key, '=')
                    {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, physical_key, '_')
                        || key_is_char(&logical_key, physical_key, '-')
                    {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                }

                // 8. Recarga dinámica de configuración Ghostty (Ctrl + Shift + ,)
                if ctrl && shift && key_is_char(&logical_key, physical_key, ',') {
                    self.reload_config();
                    return;
                }

                // Si estamos en la pestaña de Ajustes, absorber el resto de teclas
                if active_is_settings {
                    return;
                }

                // Obtener panel activo para entrada de teclado
                let active = match self.tabs[self.active_tab_index].active_pane_mut() {
                    Some(p) => p,
                    None => return,
                };

                // Detección robusta de tecla Backspace (por physical_key independiente de layout/Alt y por logical_key)
                let is_backspace = matches!(physical_key, PhysicalKey::Code(KeyCode::Backspace))
                    || matches!(&logical_key, Key::Named(NamedKey::Backspace));

                // 1. Alt + Backspace: Secuencia estándar VT (ESC + DEL: \x1b\x7f).
                if alt && is_backspace {
                    crate::logger::log("-> Enviando \\x1b\\x7f (Alt+Backspace)");
                    active.pty.write_input(b"\x1b\x7f");
                    return;
                }

                // 2. Ctrl + Backspace: Byte estándar werase (Ctrl+W: 0x17) utilizado por Windows Terminal.
                if ctrl && is_backspace {
                    crate::logger::log("-> Enviando 0x17 (Ctrl+Backspace / Ctrl+W)");
                    active.pty.write_input(&[0x17]);
                    return;
                }

                // 3. Backspace normal (o Shift+Backspace):
                if is_backspace {
                    crate::logger::log("-> Enviando 0x7f (borrado de caracter)");
                    active.pty.write_input(&[0x7f]);
                    return;
                }

                // Siempre devolver scroll al prompt activo al pulsar teclas normales (excepto PageUp/Down y Shift+Home/End)
                if logical_key != Key::Named(NamedKey::PageUp)
                    && logical_key != Key::Named(NamedKey::PageDown)
                    && !(shift
                        && (logical_key == Key::Named(NamedKey::Home)
                            || logical_key == Key::Named(NamedKey::End)))
                {
                    active.terminal.reset_scroll();
                }

                // Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
                if (ctrl && shift && key_is_char(&logical_key, physical_key, 'c'))
                    || (ctrl
                        && !shift
                        && key_is_char(&logical_key, physical_key, 'c')
                        && active.terminal.selection.is_some())
                {
                    if let Some(sel_text) = active.terminal.copy_selection_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(sel_text).ok();
                        }
                        active.terminal.selection = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    return;
                }

                // Combinaciones con Alt (Alt + V para imágenes en Claude Code, Alt + Flechas)
                if alt && !ctrl {
                    if key_is_char(&logical_key, physical_key, 'v') {
                        active.pty.write_input(b"\x1bv");
                        return;
                    }

                    let is_alt_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                        || logical_key == Key::Named(NamedKey::ArrowLeft);
                    let is_alt_right =
                        matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                            || logical_key == Key::Named(NamedKey::ArrowRight);
                    let is_alt_delete = matches!(physical_key, PhysicalKey::Code(KeyCode::Delete))
                        || logical_key == Key::Named(NamedKey::Delete);

                    if is_alt_left {
                        active.pty.write_input(b"\x1bb"); // Salto palabra atrás
                        return;
                    }
                    if is_alt_right {
                        active.pty.write_input(b"\x1bf"); // Salto palabra adelante
                        return;
                    }
                    if is_alt_delete {
                        active.pty.write_input(b"\x1bd"); // Borrado de palabra adelante
                        return;
                    }

                    match logical_key {
                        Key::Named(NamedKey::Tab) => {
                            return; // Prevenir cualquier propagación de Alt+Tab
                        }
                        Key::Named(NamedKey::F4) => {
                            return; // Alt+F4 manejado por el sistema
                        }
                        Key::Character(ref s) => {
                            if let Some(c) = s.chars().next() {
                                if c.is_ascii_alphabetic() {
                                    let mut buf = [0u8; 5];
                                    buf[0] = 0x1b;
                                    let len = c.encode_utf8(&mut buf[1..]).len();
                                    active.pty.write_input(&buf[..1 + len]);
                                    return;
                                }
                            }
                        }
                        _ => {
                            if let PhysicalKey::Code(code) = physical_key {
                                if let Some(c) = key_code_to_char(code) {
                                    if c.is_ascii_alphabetic() {
                                        let mut buf = [0u8; 5];
                                        buf[0] = 0x1b;
                                        let len = c.encode_utf8(&mut buf[1..]).len();
                                        active.pty.write_input(&buf[..1 + len]);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }

                // Copiar salida del último comando del agente (Ctrl + Shift + O)
                if ctrl && shift && key_is_char(&logical_key, physical_key, 'o') {
                    if let Some(output) = active.terminal.get_last_output_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(output).ok();
                        }
                    }
                    return;
                }

                // Limpiar pantalla y scrollback buffer al estilo Ghostty (Ctrl + Shift + K)
                if ctrl && shift && key_is_char(&logical_key, physical_key, 'k') {
                    active.terminal.scrollback.clear();
                    active.terminal.reset_scroll();
                    active.terminal.command_blocks.clear();
                    active.pty.write_input(b"\x0c");
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Navegación semántica entre comandos de agentes OSC 133 (Ctrl + Shift + Arriba/Abajo)
                if ctrl && shift {
                    if let Key::Named(NamedKey::ArrowUp) = logical_key {
                        if let Some(block) = active.terminal.command_blocks.last() {
                            let target = active
                                .terminal
                                .total_lines_count
                                .saturating_sub(block.prompt_line);
                            active.terminal.scroll_offset =
                                target.min(active.terminal.scrollback.len());
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }
                    if let Key::Named(NamedKey::ArrowDown) = logical_key {
                        active.terminal.reset_scroll();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }

                // Navegación de historial y scrollback (Shift + PageUp / PageDown / Home / End)
                if shift && !ctrl {
                    match logical_key {
                        Key::Named(NamedKey::PageUp) => {
                            let half = (active.terminal.rows / 2).max(1);
                            active.terminal.scroll_up(half);
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let half = (active.terminal.rows / 2).max(1);
                            active.terminal.scroll_down(half);
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            active.terminal.scroll_offset = active.terminal.scrollback.len();
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            active.terminal.reset_scroll();
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                // Salto de palabras y edición en línea de comandos (Ctrl + Flechas, Ctrl + Delete)
                if ctrl && !shift {
                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            active.pty.write_input(b"\x1b[1;5D"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            active.pty.write_input(b"\x1b[1;5C"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            active.pty.write_input(b"\x1b[1;5A");
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            active.pty.write_input(b"\x1b[1;5B");
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            active.pty.write_input(b"\x1b[3;5~"); // Borrar palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            active.pty.write_input(b"\x1b[1;5H");
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            active.pty.write_input(b"\x1b[1;5F");
                            return;
                        }
                        _ => {}
                    }
                }

                // Combinaciones de control clásicas de terminal (Ctrl+C, Ctrl+D, Ctrl+L, Ctrl+Z, Ctrl+R, etc.)
                if ctrl && !shift {
                    match logical_key {
                        Key::Character(ref s) => {
                            let c = s.chars().next().unwrap_or(' ').to_ascii_lowercase();
                            if c >= 'a' && c <= 'z' {
                                let code = (c as u8) - b'a' + 1;
                                active.pty.write_input(&[code]);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // Scroll por página directo sin Shift
                if logical_key == Key::Named(NamedKey::PageUp) {
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_up(half);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                if logical_key == Key::Named(NamedKey::PageDown) {
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_down(half);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Teclas especiales y flechas VT estándar
                match logical_key {
                    Key::Named(NamedKey::Enter) => active.pty.write_input(b"\r"),
                    Key::Named(NamedKey::Backspace) => active.pty.write_input(&[0x7f]),
                    Key::Named(NamedKey::Tab) => {
                        if alt {
                            return;
                        }
                        if shift {
                            active.pty.write_input(b"\x1b[Z");
                        } else {
                            active.pty.write_input(b"\t");
                        }
                    }
                    Key::Named(NamedKey::Escape) => active.pty.write_input(&[0x1b]),
                    Key::Named(NamedKey::Delete) => active.pty.write_input(b"\x1b[3~"),
                    Key::Named(NamedKey::Home) => active.pty.write_input(b"\x1b[H"),
                    Key::Named(NamedKey::End) => active.pty.write_input(b"\x1b[F"),
                    Key::Named(NamedKey::ArrowUp) => active.pty.write_input(b"\x1b[A"),
                    Key::Named(NamedKey::ArrowDown) => active.pty.write_input(b"\x1b[B"),
                    Key::Named(NamedKey::ArrowRight) => active.pty.write_input(b"\x1b[C"),
                    Key::Named(NamedKey::ArrowLeft) => active.pty.write_input(b"\x1b[D"),
                    Key::Named(NamedKey::F1) => active.pty.write_input(b"\x1bOP"),
                    Key::Named(NamedKey::F2) => active.pty.write_input(b"\x1bOQ"),
                    Key::Named(NamedKey::F3) => active.pty.write_input(b"\x1bOR"),
                    Key::Named(NamedKey::F4) => active.pty.write_input(b"\x1bOS"),
                    Key::Named(NamedKey::F5) => active.pty.write_input(b"\x1b[15~"),
                    Key::Named(NamedKey::F6) => active.pty.write_input(b"\x1b[17~"),
                    Key::Named(NamedKey::F7) => active.pty.write_input(b"\x1b[18~"),
                    Key::Named(NamedKey::F8) => active.pty.write_input(b"\x1b[19~"),
                    Key::Named(NamedKey::F9) => active.pty.write_input(b"\x1b[20~"),
                    Key::Named(NamedKey::F10) => active.pty.write_input(b"\x1b[21~"),
                    Key::Named(NamedKey::F11) => active.pty.write_input(b"\x1b[23~"),
                    Key::Named(NamedKey::F12) => active.pty.write_input(b"\x1b[24~"),
                    _ => {
                        // Entrada de texto regular (letras, números, símbolos, emojis)
                        if let Some(t) = text {
                            if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                                active.pty.write_input(t.as_bytes());
                            }
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = position;
                let offset = self.sidebar_offset();
                let top_offset = self.top_offset();
                let mx = position.x as usize;
                let my = position.y as usize;

                let (win_w, win_h) = if let Some(w) = &self.window {
                    let s = w.inner_size();
                    (s.width as usize, s.height as usize)
                } else {
                    (800, 600)
                };

                let mut needs_redraw = false;
                let mut is_pointer = false;
                let mut is_text = false;

                let is_settings_active = self.tabs[self.active_tab_index].is_settings;

                // Manejo de arrastre activo del pulgar de la barra de desplazamiento en el panel seleccionado
                if self.scrollbar_dragging {
                    if let Some(pid) = self.scrollbar_drag_pane_id {
                        let status_bar_h = self.status_bar_height();
                        let right_offset = self.file_explorer_offset();
                        let active_tab = &mut self.tabs[self.active_tab_index];
                        let term_area_w = win_w.saturating_sub(offset + right_offset);
                        let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
                        let pane_rects = active_tab.compute_pane_rects(
                            offset,
                            top_offset,
                            term_area_w,
                            term_area_h,
                        );
                        if let Some(target_rect) = pane_rects.iter().find(|r| r.pane_id == pid) {
                            if let Some(pane) = active_tab.find_pane_mut(pid) {
                                if let Some(renderer) = &self.renderer {
                                    if !pane.terminal.scrollback.is_empty() {
                                        let track_y = target_rect.y + renderer.margin_y;
                                        let track_h = target_rect
                                            .height
                                            .saturating_sub(renderer.margin_y * 2);
                                        let total_history =
                                            pane.terminal.scrollback.len() + pane.terminal.rows;
                                        let thumb_h = ((pane.terminal.rows as f32
                                            / total_history as f32)
                                            * track_h as f32)
                                            .max(24.0)
                                            .min(track_h as f32)
                                            as usize;
                                        let available_travel = track_h.saturating_sub(thumb_h);
                                        if available_travel > 0 {
                                            let target_thumb_top = (my as f32
                                                - self.scrollbar_drag_offset_y
                                                - track_y as f32)
                                                .clamp(0.0, available_travel as f32);
                                            let ratio = target_thumb_top / available_travel as f32;
                                            let new_offset = ((1.0 - ratio)
                                                * pane.terminal.scrollback.len() as f32)
                                                .round()
                                                as usize;
                                            pane.terminal.scroll_offset =
                                                new_offset.min(pane.terminal.scrollback.len());
                                            self.last_scroll_time = Instant::now();
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(w) = &self.window {
                        w.set_cursor(winit::window::CursorIcon::Pointer);
                    }
                    return;
                }

                let mut new_hovered_min = false;
                let mut new_hovered_max = false;
                let mut new_hovered_close_win = false;

                if self.is_horizontal_tabs() {
                    let top_rects = TopTabBarRects::compute(win_w, self.tabs.len());
                    let mut new_hovered_tab = None;
                    let mut new_hovered_close = None;
                    let mut new_hovered_new_tab = false;
                    let mut new_hovered_settings = false;
                    let mut new_hovered_files = false;

                    if my < TopTabBarRects::HEIGHT {
                        // Check window controls
                        if TopTabBarRects::inside(mx, my, top_rects.win_min_btn) {
                            new_hovered_min = true;
                            is_pointer = true;
                        }
                        if TopTabBarRects::inside(mx, my, top_rects.win_max_btn) {
                            new_hovered_max = true;
                            is_pointer = true;
                        }
                        if TopTabBarRects::inside(mx, my, top_rects.win_close_btn) {
                            new_hovered_close_win = true;
                            is_pointer = true;
                        }

                        // Check plus button
                        if mx >= top_rects.plus_btn.0
                            && mx < top_rects.plus_btn.0 + top_rects.plus_btn.2
                            && my >= top_rects.plus_btn.1
                            && my < top_rects.plus_btn.1 + top_rects.plus_btn.3
                        {
                            new_hovered_new_tab = true;
                            is_pointer = true;
                        }
                        // Check files button
                        if mx >= top_rects.files_btn.0
                            && mx < top_rects.files_btn.0 + top_rects.files_btn.2
                            && my >= top_rects.files_btn.1
                            && my < top_rects.files_btn.1 + top_rects.files_btn.3
                        {
                            new_hovered_files = true;
                            is_pointer = true;
                        }
                        // Check settings button
                        if mx >= top_rects.settings_btn.0
                            && mx < top_rects.settings_btn.0 + top_rects.settings_btn.2
                            && my >= top_rects.settings_btn.1
                            && my < top_rects.settings_btn.1 + top_rects.settings_btn.3
                        {
                            new_hovered_settings = true;
                            is_pointer = true;
                        }
                        // Check tabs
                        for idx in 0..self.tabs.len() {
                            let tr = top_rects.tab_rect(idx);
                            if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                                new_hovered_tab = Some(idx);
                                is_pointer = true;
                                if self.tabs.len() > 1 {
                                    let cr = top_rects.close_btn_rect(idx);
                                    if mx >= cr.0
                                        && mx < cr.0 + cr.2
                                        && my >= cr.1
                                        && my < cr.1 + cr.3
                                    {
                                        new_hovered_close = Some(idx);
                                    }
                                }
                                break;
                            }
                        }
                    }

                    if self.hovered_tab != new_hovered_tab {
                        self.hovered_tab = new_hovered_tab;
                        needs_redraw = true;
                    }
                    if self.hovered_close != new_hovered_close {
                        self.hovered_close = new_hovered_close;
                        needs_redraw = true;
                    }
                    if self.hovered_new_tab != new_hovered_new_tab {
                        self.hovered_new_tab = new_hovered_new_tab;
                        needs_redraw = true;
                    }
                    if self.hovered_files_btn != new_hovered_files {
                        self.hovered_files_btn = new_hovered_files;
                        needs_redraw = true;
                    }
                    if self.hovered_settings_btn != new_hovered_settings {
                        self.hovered_settings_btn = new_hovered_settings;
                        needs_redraw = true;
                    }
                } else {
                    let title_rects = TopTitleBarRects::compute(win_w);
                    let mut new_hovered_settings = false;
                    if my < TopTitleBarRects::HEIGHT {
                        if TopTabBarRects::inside(mx, my, title_rects.win_min_btn) {
                            new_hovered_min = true;
                            is_pointer = true;
                        }
                        if TopTabBarRects::inside(mx, my, title_rects.win_max_btn) {
                            new_hovered_max = true;
                            is_pointer = true;
                        }
                        if TopTabBarRects::inside(mx, my, title_rects.win_close_btn) {
                            new_hovered_close_win = true;
                            is_pointer = true;
                        }
                        if TopTabBarRects::inside(mx, my, title_rects.settings_btn) {
                            new_hovered_settings = true;
                            is_pointer = true;
                        }
                    }
                    if self.hovered_settings_btn != new_hovered_settings {
                        self.hovered_settings_btn = new_hovered_settings;
                        needs_redraw = true;
                    }
                    // Comprobar hover en botón inferior de alternar barra lateral
                    let status_bar_h = self.status_bar_height();
                    let toggle_btn_w = if self.sidebar_visible {
                        SIDEBAR_WIDTH.saturating_sub(16)
                    } else {
                        32
                    };
                    let toggle_btn_h = 30;
                    let toggle_btn_x = 8;
                    let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8 + status_bar_h);

                    let is_hover_toggle = mx >= toggle_btn_x
                        && mx < toggle_btn_x + toggle_btn_w
                        && my >= toggle_btn_y
                        && my < toggle_btn_y + toggle_btn_h;

                    if is_hover_toggle {
                        is_pointer = true;
                    }

                    if self.hovered_toggle_btn != is_hover_toggle {
                        self.hovered_toggle_btn = is_hover_toggle;
                        needs_redraw = true;
                    }

                    if !self.sidebar_visible {
                        let float_set_x = 44;
                        let float_set_w = 32;
                        let is_hover_float_set = mx >= float_set_x
                            && mx < float_set_x + float_set_w
                            && my >= toggle_btn_y
                            && my < toggle_btn_y + toggle_btn_h;
                        if is_hover_float_set {
                            is_pointer = true;
                        }
                        if self.hovered_floating_settings != is_hover_float_set {
                            self.hovered_floating_settings = is_hover_float_set;
                            needs_redraw = true;
                        }
                    }

                    // 1. Detección de hover en la barra lateral si está visible
                    if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                        // Hover en botón "+"
                        let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                        let plus_btn_y = 8;
                        let plus_btn_w = 24;
                        let plus_btn_h = 24;
                        let is_new_tab = mx >= plus_btn_x
                            && mx < plus_btn_x + plus_btn_w
                            && my >= plus_btn_y
                            && my < plus_btn_y + plus_btn_h;
                        if is_new_tab {
                            is_pointer = true;
                        }
                        if self.hovered_new_tab != is_new_tab {
                            self.hovered_new_tab = is_new_tab;
                            needs_redraw = true;
                        }

                        // Hover en botón "⚙" en el header del sidebar
                        let settings_btn_x = SIDEBAR_WIDTH.saturating_sub(62);
                        let is_hover_set =
                            mx >= settings_btn_x && mx < settings_btn_x + 24 && my >= 8 && my < 32;
                        if is_hover_set {
                            is_pointer = true;
                        }
                        if self.hovered_settings_btn != is_hover_set {
                            self.hovered_settings_btn = is_hover_set;
                            needs_redraw = true;
                        }

                        // Hover en pestañas y botón de cerrar "x"
                        let tab_item_h = 36;
                        let tab_spacing = 4;
                        let list_start_y = 42;
                        let tab_x = 8;
                        let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                        let list_max_y = toggle_btn_y.saturating_sub(8);

                        let mut new_hovered_tab = None;
                        let mut new_hovered_close = None;

                        if my >= list_start_y && my < list_max_y {
                            let rel_y = my - list_start_y;
                            let item_total = tab_item_h + tab_spacing;
                            let tab_idx = rel_y / item_total;
                            let in_item = (rel_y % item_total) < tab_item_h;

                            if in_item
                                && tab_idx < self.tabs.len()
                                && mx >= tab_x
                                && mx < tab_x + tab_w
                            {
                                new_hovered_tab = Some(tab_idx);
                                is_pointer = true;

                                if self.tabs.len() > 1 {
                                    let tab_y = list_start_y + tab_idx * item_total;
                                    let close_x = tab_x + tab_w.saturating_sub(22);
                                    let close_y = tab_y + 6;
                                    let close_size = 20;

                                    if mx >= close_x
                                        && mx < close_x + close_size
                                        && my >= close_y
                                        && my < close_y + close_size
                                    {
                                        new_hovered_close = Some(tab_idx);
                                    }
                                }
                            }
                        }

                        if self.hovered_tab != new_hovered_tab {
                            self.hovered_tab = new_hovered_tab;
                            needs_redraw = true;
                        }
                        if self.hovered_close != new_hovered_close {
                            self.hovered_close = new_hovered_close;
                            needs_redraw = true;
                        }
                    } else if self.sidebar_visible {
                        if self.hovered_tab.is_some() {
                            self.hovered_tab = None;
                            needs_redraw = true;
                        }
                        if self.hovered_close.is_some() {
                            self.hovered_close = None;
                            needs_redraw = true;
                        }
                        if self.hovered_new_tab {
                            self.hovered_new_tab = false;
                            needs_redraw = true;
                        }
                        if self.hovered_settings_btn {
                            self.hovered_settings_btn = false;
                            needs_redraw = true;
                        }
                    }
                }

                // Detección de hover en la pestaña de Ajustes (si está activa)
                if is_settings_active && mx >= offset && my >= top_offset {
                    let page_w = win_w.saturating_sub(offset);
                    let page_h = win_h.saturating_sub(top_offset);
                    let rects = SettingsPageRects::compute(offset, top_offset, page_w, page_h);

                    let h_sec = if SettingsPageRects::inside(mx, my, rects.nav_appearance) {
                        Some(SettingsSection::Appearance)
                    } else if SettingsPageRects::inside(mx, my, rects.nav_terminal) {
                        Some(SettingsSection::Terminal)
                    } else if SettingsPageRects::inside(mx, my, rects.nav_shortcuts) {
                        Some(SettingsSection::Shortcuts)
                    } else if SettingsPageRects::inside(mx, my, rects.nav_about) {
                        Some(SettingsSection::About)
                    } else {
                        None
                    };

                    if h_sec.is_some() {
                        is_pointer = true;
                    }

                    let mut h_theme = None;
                    let mut h_tm = None;
                    let mut h_tl = None;
                    let mut h_fm = false;
                    let mut h_fp = false;
                    let mut h_sm = false;
                    let mut h_sp = false;
                    let mut h_om = false;
                    let mut h_op = false;
                    let mut h_cb = false;
                    let mut h_gp = false;
                    let mut h_cs = false;
                    let mut h_sb = false;
                    let mut h_fe = false;
                    let mut h_rs = false;
                    let mut h_of = false;
                    let mut h_reset = SettingsPageRects::inside(mx, my, rects.reset_defaults_btn);

                    match self.settings_page_state.section {
                        SettingsSection::Appearance => {
                            for (idx, r) in rects.theme_buttons.iter().enumerate() {
                                if SettingsPageRects::inside(mx, my, *r) {
                                    h_theme = Some(idx);
                                    break;
                                }
                            }
                            if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                                h_tm = Some(0);
                            } else if SettingsPageRects::inside(mx, my, rects.theme_mode_dark_btn) {
                                h_tm = Some(1);
                            } else if SettingsPageRects::inside(mx, my, rects.theme_mode_light_btn) {
                                h_tm = Some(2);
                            }

                            if SettingsPageRects::inside(mx, my, rects.tab_layout_vertical_btn) {
                                h_tl = Some(0);
                            } else if SettingsPageRects::inside(
                                mx,
                                my,
                                rects.tab_layout_horizontal_btn,
                            ) {
                                h_tl = Some(1);
                            }

                            h_fm = SettingsPageRects::inside(mx, my, rects.font_minus_btn);
                            h_fp = SettingsPageRects::inside(mx, my, rects.font_plus_btn);
                            h_sm = SettingsPageRects::inside(mx, my, rects.spacing_minus_btn);
                            h_sp = SettingsPageRects::inside(mx, my, rects.spacing_plus_btn);
                            h_om = SettingsPageRects::inside(mx, my, rects.opacity_minus_btn);
                            h_op = SettingsPageRects::inside(mx, my, rects.opacity_plus_btn);
                        }
                        SettingsSection::Terminal => {
                            h_cb = SettingsPageRects::inside(mx, my, rects.cursor_blink_btn);
                            h_gp = SettingsPageRects::inside(mx, my, rects.ghostty_prompt_btn);
                            h_cs = SettingsPageRects::inside(mx, my, rects.copy_on_select_btn);
                            h_sb = SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn);
                            h_fe = SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn);
                            h_rs = SettingsPageRects::inside(mx, my, rects.restore_session_btn);
                            h_of = SettingsPageRects::inside(mx, my, rects.open_file_btn);
                            h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.term_reset_btn);
                        }
                        SettingsSection::About => {
                            h_of = SettingsPageRects::inside(mx, my, rects.open_file_btn);
                            h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.term_reset_btn);
                        }
                        SettingsSection::Shortcuts => {}
                    }

                    if h_theme.is_some()
                        || h_tm.is_some()
                        || h_tl.is_some()
                        || h_fm
                        || h_fp
                        || h_sm
                        || h_sp
                        || h_om
                        || h_op
                        || h_cb
                        || h_gp
                        || h_cs
                        || h_sb
                        || h_fe
                        || h_rs
                        || h_of
                        || h_reset
                    {
                        is_pointer = true;
                    }

                    let st = &mut self.settings_page_state;
                    if st.hovered_section != h_sec
                        || st.hovered_theme != h_theme
                        || st.hovered_theme_mode != h_tm
                        || st.hovered_tab_layout != h_tl
                        || st.hovered_font_minus != h_fm
                        || st.hovered_font_plus != h_fp
                        || st.hovered_spacing_minus != h_sm
                        || st.hovered_spacing_plus != h_sp
                        || st.hovered_opacity_minus != h_om
                        || st.hovered_opacity_plus != h_op
                        || st.hovered_cursor_blink != h_cb
                        || st.hovered_ghostty_prompt != h_gp
                        || st.hovered_copy_on_select != h_cs
                        || st.hovered_status_bar != h_sb
                        || st.hovered_file_explorer != h_fe
                        || st.hovered_restore_session != h_rs
                        || st.hovered_open_file != h_of
                        || st.hovered_reset_defaults != h_reset
                    {
                        st.hovered_section = h_sec;
                        st.hovered_theme = h_theme;
                        st.hovered_theme_mode = h_tm;
                        st.hovered_tab_layout = h_tl;
                        st.hovered_font_minus = h_fm;
                        st.hovered_font_plus = h_fp;
                        st.hovered_spacing_minus = h_sm;
                        st.hovered_spacing_plus = h_sp;
                        st.hovered_opacity_minus = h_om;
                        st.hovered_opacity_plus = h_op;
                        st.hovered_cursor_blink = h_cb;
                        st.hovered_ghostty_prompt = h_gp;
                        st.hovered_copy_on_select = h_cs;
                        st.hovered_status_bar = h_sb;
                        st.hovered_file_explorer = h_fe;
                        st.hovered_restore_session = h_rs;
                        st.hovered_open_file = h_of;
                        st.hovered_reset_defaults = h_reset;
                        needs_redraw = true;
                    }
                } else if is_settings_active {
                    let st = &mut self.settings_page_state;
                    if st.hovered_section.is_some()
                        || st.hovered_theme.is_some()
                        || st.hovered_theme_mode.is_some()
                        || st.hovered_tab_layout.is_some()
                        || st.hovered_font_minus
                        || st.hovered_font_plus
                        || st.hovered_spacing_minus
                        || st.hovered_spacing_plus
                        || st.hovered_opacity_minus
                        || st.hovered_opacity_plus
                        || st.hovered_cursor_blink
                        || st.hovered_ghostty_prompt
                        || st.hovered_copy_on_select
                        || st.hovered_status_bar
                        || st.hovered_file_explorer
                        || st.hovered_open_file
                        || st.hovered_reset_defaults
                    {
                        st.hovered_section = None;
                        st.hovered_theme = None;
                        st.hovered_theme_mode = None;
                        st.hovered_tab_layout = None;
                        st.hovered_font_minus = false;
                        st.hovered_font_plus = false;
                        st.hovered_spacing_minus = false;
                        st.hovered_spacing_plus = false;
                        st.hovered_opacity_minus = false;
                        st.hovered_opacity_plus = false;
                        st.hovered_cursor_blink = false;
                        st.hovered_ghostty_prompt = false;
                        st.hovered_copy_on_select = false;
                        st.hovered_status_bar = false;
                        st.hovered_file_explorer = false;
                        st.hovered_open_file = false;
                        st.hovered_reset_defaults = false;
                        needs_redraw = true;
                    }
                }

                // Detección de hover dentro del explorador lateral de archivos (si no es settings)
                if !is_settings_active && self.file_explorer.visible {
                    let panel_w = crate::explorer::FILE_EXPLORER_WIDTH;
                    if mx >= win_w.saturating_sub(panel_w) {
                        let panel_x = win_w.saturating_sub(panel_w);
                        let close_x = panel_x + panel_w.saturating_sub(28);
                        let is_close_hover = mx >= close_x
                            && mx < close_x + 20
                            && my >= top_offset + 8
                            && my < top_offset + 28;

                        let refresh_x = panel_x + panel_w.saturating_sub(56);
                        let is_refresh_hover = mx >= refresh_x
                            && mx < refresh_x + 20
                            && my >= top_offset + 8
                            && my < top_offset + 28;

                        let sub_y = top_offset + crate::explorer::FileExplorer::HEADER_HEIGHT;
                        let up_x = panel_x + 10;
                        let is_up_hover =
                            mx >= up_x && mx < up_x + 28 && my >= sub_y + 4 && my < sub_y + 24;

                        let item_hover = self.file_explorer.item_at_y(my, top_offset);

                        if is_close_hover || is_refresh_hover || is_up_hover || item_hover.is_some() {
                            is_pointer = true;
                        }

                        if self.file_explorer.hovered_close_btn != is_close_hover
                            || self.file_explorer.hovered_refresh_btn != is_refresh_hover
                            || self.file_explorer.hovered_up_btn != is_up_hover
                            || self.file_explorer.hovered_index != item_hover
                        {
                            self.file_explorer.hovered_close_btn = is_close_hover;
                            self.file_explorer.hovered_refresh_btn = is_refresh_hover;
                            self.file_explorer.hovered_up_btn = is_up_hover;
                            self.file_explorer.hovered_index = item_hover;
                            needs_redraw = true;
                        }
                    } else if self.file_explorer.hovered_close_btn
                        || self.file_explorer.hovered_refresh_btn
                        || self.file_explorer.hovered_up_btn
                        || self.file_explorer.hovered_index.is_some()
                    {
                        self.file_explorer.hovered_close_btn = false;
                        self.file_explorer.hovered_refresh_btn = false;
                        self.file_explorer.hovered_up_btn = false;
                        self.file_explorer.hovered_index = None;
                        needs_redraw = true;
                    }
                }


                // Coordenadas relativas a paneles en la terminal activa
                if !is_settings_active {
                    let bypass = self.modifiers.shift_key();
                    let status_bar_h = self.status_bar_height();
                    let right_offset = self.file_explorer_offset();
                    let term_area_w = win_w.saturating_sub(offset + right_offset);
                    let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
                    let active_tab = &mut self.tabs[self.active_tab_index];
                    let active_pane_id = active_tab.active_pane_id;
                    let pane_rects =
                        active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);

                    let hovered_pane_rect = pane_rects
                        .iter()
                        .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
                        .copied();

                    if let Some(rect) = hovered_pane_rect {
                        if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                            let is_over_scrollbar = if let Some(renderer) = &self.renderer {
                                if let Some((_tx, ty, _tw, th)) = renderer.pane_scrollbar_geometry(
                                    rect.x,
                                    rect.y,
                                    rect.width,
                                    rect.height,
                                    &pane.terminal,
                                ) {
                                    let scroll_margin = 16;
                                    mx >= (rect.x + rect.width).saturating_sub(scroll_margin)
                                        && my >= ty
                                        && my < ty + th
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if is_over_scrollbar {
                                is_pointer = true;
                            } else if !pane.terminal.is_mouse_reporting_active() || bypass {
                                is_text = true;
                            }

                            if pane.terminal.is_mouse_reporting_active() && !bypass {
                                if (pane.terminal.mouse_reporting_button_event && self.mouse_down)
                                    || pane.terminal.mouse_reporting_any_event
                                {
                                    if let Some(renderer) = &self.renderer {
                                        let term_x = mx.saturating_sub(rect.x);
                                        let term_y = my.saturating_sub(rect.y);
                                        let col = if term_x >= renderer.margin_x {
                                            ((term_x - renderer.margin_x) / renderer.char_width) + 1
                                        } else {
                                            1
                                        }
                                        .min(pane.terminal.cols);
                                        let row = if term_y >= renderer.margin_y {
                                            ((term_y - renderer.margin_y) / renderer.row_height) + 1
                                        } else {
                                            1
                                        }
                                        .min(pane.terminal.rows);

                                        let mut code = 32;
                                        if self.modifiers.shift_key() {
                                            code += 4;
                                        }
                                        if self.modifiers.alt_key() {
                                            code += 8;
                                        }
                                        if self.modifiers.control_key() {
                                            code += 16;
                                        }

                                        if pane.terminal.mouse_reporting_sgr {
                                            let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                            pane.pty.write_input(seq.as_bytes());
                                        } else {
                                            let seq = [
                                                0x1b,
                                                b'[',
                                                b'M',
                                                32 + code as u8,
                                                32 + col.min(223) as u8,
                                                32 + row.min(223) as u8,
                                            ];
                                            pane.pty.write_input(&seq);
                                        }
                                    }
                                }
                            } else if self.mouse_down && rect.pane_id == active_pane_id {
                                if let Some(renderer) = &self.renderer {
                                    let term_x = mx.saturating_sub(rect.x);
                                    let term_y = my.saturating_sub(rect.y);
                                    if term_x >= renderer.margin_x && term_y >= renderer.margin_y {
                                        let col = (term_x - renderer.margin_x) / renderer.char_width;
                                        let row = (term_y - renderer.margin_y) / renderer.row_height;
                                        if let Some(start) = self.drag_start {
                                            pane.terminal.selection = Some(SelectionRange {
                                                start,
                                                end: (col, row),
                                            });
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if self.hovered_win_min != new_hovered_min {
                    self.hovered_win_min = new_hovered_min;
                    needs_redraw = true;
                }
                if self.hovered_win_max != new_hovered_max {
                    self.hovered_win_max = new_hovered_max;
                    needs_redraw = true;
                }
                if self.hovered_win_close != new_hovered_close_win {
                    self.hovered_win_close = new_hovered_close_win;
                    needs_redraw = true;
                }

                // Detección de bordes para cursor de redimensionamiento (ventana frameless)
                let border_resize_cursor = {
                    let border_margin = 6;
                    let at_left = mx < border_margin;
                    let at_right = mx >= win_w.saturating_sub(border_margin);
                    let at_top = my < border_margin;
                    let at_bottom = my >= win_h.saturating_sub(border_margin);

                    if at_top && at_left {
                        Some(winit::window::CursorIcon::NwseResize)
                    } else if at_top && at_right {
                        Some(winit::window::CursorIcon::NeswResize)
                    } else if at_bottom && at_left {
                        Some(winit::window::CursorIcon::NeswResize)
                    } else if at_bottom && at_right {
                        Some(winit::window::CursorIcon::NwseResize)
                    } else if at_left || at_right {
                        Some(winit::window::CursorIcon::EwResize)
                    } else if at_top || at_bottom {
                        Some(winit::window::CursorIcon::NsResize)
                    } else {
                        None
                    }
                };

                let cursor_icon = if let Some(resize_cur) = border_resize_cursor {
                    resize_cur
                } else if is_pointer {
                    winit::window::CursorIcon::Pointer
                } else if is_text {
                    winit::window::CursorIcon::Text
                } else {
                    winit::window::CursorIcon::Default
                };
                if let Some(w) = &self.window {
                    w.set_cursor(cursor_icon);
                    if needs_redraw {
                        w.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let mx = self.mouse_pos.x as usize;
                let my = self.mouse_pos.y as usize;

                let (win_w, win_h) = if let Some(w) = &self.window {
                    let s = w.inner_size();
                    (s.width as usize, s.height as usize)
                } else {
                    (800, 600)
                };

                let offset = self.sidebar_offset();
                let is_settings_active = self.tabs[self.active_tab_index].is_settings;

                // Si soltamos el botón izquierdo, cancelar arrastre de la barra de desplazamiento
                if button == MouseButton::Left && state == ElementState::Released {
                    if self.scrollbar_dragging {
                        self.scrollbar_dragging = false;
                        self.scrollbar_drag_pane_id = None;
                        self.last_scroll_time = Instant::now();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }

                // Redimensionamiento de ventana al arrastrar bordes (ventana frameless)
                if button == MouseButton::Left && state == ElementState::Pressed {
                    let border_margin = 6;
                    let at_left = mx < border_margin;
                    let at_right = mx >= win_w.saturating_sub(border_margin);
                    let at_top = my < border_margin;
                    let at_bottom = my >= win_h.saturating_sub(border_margin);

                    let resize_direction = if at_top && at_left {
                        Some(winit::window::ResizeDirection::NorthWest)
                    } else if at_top && at_right {
                        Some(winit::window::ResizeDirection::NorthEast)
                    } else if at_bottom && at_left {
                        Some(winit::window::ResizeDirection::SouthWest)
                    } else if at_bottom && at_right {
                        Some(winit::window::ResizeDirection::SouthEast)
                    } else if at_left {
                        Some(winit::window::ResizeDirection::West)
                    } else if at_right {
                        Some(winit::window::ResizeDirection::East)
                    } else if at_top {
                        Some(winit::window::ResizeDirection::North)
                    } else if at_bottom {
                        Some(winit::window::ResizeDirection::South)
                    } else {
                        None
                    };

                    if let Some(dir) = resize_direction {
                        if let Some(w) = &self.window {
                            let _ = w.drag_resize_window(dir);
                            return;
                        }
                    }
                }

                // Si estamos en modo de pestañas horizontales superiores
                if self.is_horizontal_tabs() && my < TopTabBarRects::HEIGHT {
                    if state == ElementState::Pressed && button == MouseButton::Left {
                        let top_rects = TopTabBarRects::compute(win_w, self.tabs.len());

                        // Clic en [-] Minimizar
                        if TopTabBarRects::inside(mx, my, top_rects.win_min_btn) {
                            if let Some(w) = &self.window {
                                w.set_minimized(true);
                            }
                            return;
                        }

                        // Clic en [□] Maximizar / Restaurar
                        if TopTabBarRects::inside(mx, my, top_rects.win_max_btn) {
                            if let Some(w) = &self.window {
                                w.set_maximized(!w.is_maximized());
                            }
                            return;
                        }

                        // Clic en [✕] Cerrar ventana
                        if TopTabBarRects::inside(mx, my, top_rects.win_close_btn) {
                            self.save_session();
                            event_loop.exit();
                            return;
                        }

                        // Clic en [+] nueva pestaña
                        if TopTabBarRects::inside(mx, my, top_rects.plus_btn) {
                            crate::logger::log("-> Clic en [+] top bar: nueva terminal");
                            self.create_new_tab();
                            return;
                        }

                        // Clic en [◈] Explorador de archivos
                        if TopTabBarRects::inside(mx, my, top_rects.files_btn) {
                            crate::logger::log("-> Clic en [◈] top bar: Explorador de archivos");
                            self.toggle_file_explorer();
                            return;
                        }

                        // Clic en [⚙] Ajustes
                        if TopTabBarRects::inside(mx, my, top_rects.settings_btn) {
                            crate::logger::log("-> Clic en [⚙] top bar: Ajustes");
                            self.open_or_switch_settings_tab();
                            return;
                        }

                        // Clic en pestañas o en botón cerrar 'x'
                        for idx in 0..self.tabs.len() {
                            let tr = top_rects.tab_rect(idx);
                            if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                                if self.tabs.len() > 1 {
                                    let cr = top_rects.close_btn_rect(idx);
                                    if mx >= cr.0
                                        && mx < cr.0 + cr.2
                                        && my >= cr.1
                                        && my < cr.1 + cr.3
                                    {
                                        crate::logger::log(&format!(
                                            "-> Clic en [x] pestaña {}",
                                            idx
                                        ));
                                        self.close_tab(idx, event_loop);
                                        return;
                                    }
                                }
                                crate::logger::log(&format!("-> Clic en pestaña {}", idx));
                                self.switch_to_tab(idx);
                                return;
                            }
                        }

                        // Arrastrar ventana o doble clic para maximizar
                            let now = Instant::now();
                            if now.duration_since(self.last_titlebar_click).as_millis() < 300 {
                                if let Some(w) = &self.window {
                                    w.set_maximized(!w.is_maximized());
                                }
                            } else {
                                self.last_titlebar_click = now;
                                if let Some(w) = &self.window {
                                    let _ = w.drag_window();
                                }
                            }
                    }
                    return;
                }

                // Modalidad vertical: comprobación de barra de título superior
                if !self.is_horizontal_tabs() && my < TopTitleBarRects::HEIGHT {
                    if state == ElementState::Pressed && button == MouseButton::Left {
                        let title_rects = TopTitleBarRects::compute(win_w);

                        if TopTabBarRects::inside(mx, my, title_rects.win_min_btn) {
                            if let Some(w) = &self.window {
                                w.set_minimized(true);
                            }
                            return;
                        }

                        if TopTabBarRects::inside(mx, my, title_rects.win_max_btn) {
                            if let Some(w) = &self.window {
                                w.set_maximized(!w.is_maximized());
                            }
                            return;
                        }

                        if TopTabBarRects::inside(mx, my, title_rects.win_close_btn) {
                            self.save_session();
                            event_loop.exit();
                            return;
                        }

                        if TopTabBarRects::inside(mx, my, title_rects.settings_btn) {
                            self.open_or_switch_settings_tab();
                            return;
                        }

                        // Arrastrar ventana o doble clic para maximizar
                        let now = Instant::now();
                        if now.duration_since(self.last_titlebar_click).as_millis() < 300 {
                            if let Some(w) = &self.window {
                                w.set_maximized(!w.is_maximized());
                            }
                        } else {
                            self.last_titlebar_click = now;
                            if let Some(w) = &self.window {
                                let _ = w.drag_window();
                            }
                        }
                    }
                    return;
                }

                // Modalidad vertical: comprobación de botones de alternar barra lateral y clics en el sidebar
                if !self.is_horizontal_tabs() {
                    let status_bar_h = self.status_bar_height();
                    let toggle_btn_w = if self.sidebar_visible {
                        SIDEBAR_WIDTH.saturating_sub(16)
                    } else {
                        32
                    };
                    let toggle_btn_h = 30;
                    let toggle_btn_x = 8;
                    let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8 + status_bar_h);

                    let is_click_toggle = mx >= toggle_btn_x
                        && mx < toggle_btn_x + toggle_btn_w
                        && my >= toggle_btn_y
                        && my < toggle_btn_y + toggle_btn_h;

                    if is_click_toggle
                        && state == ElementState::Pressed
                        && button == MouseButton::Left
                    {
                        crate::logger::log("-> Clic en botón inferior de alternar barra lateral");
                        self.toggle_sidebar();
                        return;
                    }

                    // Si la barra lateral está colapsada, clic en el botón flotante de Ajustes
                    if !self.sidebar_visible {
                        let float_set_x = 44;
                        let float_set_w = 32;
                        let is_click_float_set = mx >= float_set_x
                            && mx < float_set_x + float_set_w
                            && my >= toggle_btn_y
                            && my < toggle_btn_y + toggle_btn_h;
                        if is_click_float_set
                            && state == ElementState::Pressed
                            && button == MouseButton::Left
                        {
                            crate::logger::log("-> Clic en botón flotante de Ajustes");
                            self.open_or_switch_settings_tab();
                            return;
                        }
                    }

                    // Clics en la barra lateral abierta
                    if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                        if state == ElementState::Pressed && button == MouseButton::Left {
                            // Clic en botón "+"
                            let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                            let plus_btn_y = 8;
                            let plus_btn_w = 24;
                            let plus_btn_h = 24;
                            if mx >= plus_btn_x
                                && mx < plus_btn_x + plus_btn_w
                                && my >= plus_btn_y
                                && my < plus_btn_y + plus_btn_h
                            {
                                crate::logger::log("-> Clic en [+] sidebar: nueva terminal");
                                self.create_new_tab();
                                return;
                            }

                            // Clic en botón "⚙" en el header del sidebar
                            let settings_btn_x = SIDEBAR_WIDTH.saturating_sub(62);
                            if mx >= settings_btn_x
                                && mx < settings_btn_x + 24
                                && my >= 8
                                && my < 32
                            {
                                crate::logger::log("-> Clic en [⚙] header sidebar: Ajustes");
                                self.open_or_switch_settings_tab();
                                return;
                            }

                            // Clic en pestañas o botón cerrar "x"
                            let tab_item_h = 36;
                            let tab_spacing = 4;
                            let list_start_y = 42;
                            let tab_x = 8;
                            let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                            let list_max_y = toggle_btn_y.saturating_sub(8);

                            if my >= list_start_y && my < list_max_y {
                                let rel_y = my - list_start_y;
                                let item_total = tab_item_h + tab_spacing;
                                let tab_idx = rel_y / item_total;
                                let in_item = (rel_y % item_total) < tab_item_h;

                                if in_item
                                    && tab_idx < self.tabs.len()
                                    && mx >= tab_x
                                    && mx < tab_x + tab_w
                                {
                                    let tab_y = list_start_y + tab_idx * item_total;
                                    let close_x = tab_x + tab_w.saturating_sub(22);
                                    let close_y = tab_y + 6;
                                    let close_size = 20;

                                    if self.tabs.len() > 1
                                        && mx >= close_x
                                        && mx < close_x + close_size
                                        && my >= close_y
                                        && my < close_y + close_size
                                    {
                                        crate::logger::log(&format!(
                                            "-> Clic en [x] pestaña {}",
                                            tab_idx
                                        ));
                                        self.close_tab(tab_idx, event_loop);
                                    } else {
                                        crate::logger::log(&format!(
                                            "-> Clic en pestaña {}",
                                            tab_idx
                                        ));
                                        self.switch_to_tab(tab_idx);
                                    }
                                    return;
                                }
                            }
                        }
                        return;
                    }
                }

                // 2. Clics en la pestaña de Ajustes (si está activa)
                if is_settings_active {
                    let top_offset = self.top_offset();
                    if mx >= offset
                        && my >= top_offset
                        && state == ElementState::Pressed
                        && button == MouseButton::Left
                    {
                        let page_w = win_w.saturating_sub(offset);
                        let page_h = win_h.saturating_sub(top_offset);
                        let rects = SettingsPageRects::compute(offset, top_offset, page_w, page_h);

                        // Botón Restablecer valores por defecto (Header)
                        if SettingsPageRects::inside(mx, my, rects.reset_defaults_btn) {
                            crate::logger::log("-> Clic en restablecer valores por defecto (Header)");
                            self.reset_to_defaults();
                            return;
                        }

                        // Pestañas de navegación
                        if SettingsPageRects::inside(mx, my, rects.nav_appearance) {
                            self.settings_page_state.section = SettingsSection::Appearance;
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.nav_terminal) {
                            self.settings_page_state.section = SettingsSection::Terminal;
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.nav_shortcuts) {
                            self.settings_page_state.section = SettingsSection::Shortcuts;
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.nav_about) {
                            self.settings_page_state.section = SettingsSection::About;
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }

                        // Contenido de la sección activa
                        match self.settings_page_state.section {
                            SettingsSection::Appearance => {
                                for (idx, r) in rects.theme_buttons.iter().enumerate() {
                                    if SettingsPageRects::inside(mx, my, *r) {
                                        if let Some(th_name) = Theme::AVAILABLE_THEMES.get(idx) {
                                            crate::logger::log(&format!(
                                                "-> Clic en tema: {}",
                                                th_name
                                            ));
                                            self.apply_theme(th_name);
                                            return;
                                        }
                                    }
                                }
                                if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                                    crate::logger::log("-> Clic en modo de tema: Sistema");
                                    self.set_theme_mode("system");
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.theme_mode_dark_btn,
                                ) {
                                    crate::logger::log("-> Clic en modo de tema: Oscuro");
                                    self.set_theme_mode("dark");
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.theme_mode_light_btn,
                                ) {
                                    crate::logger::log("-> Clic en modo de tema: Claro");
                                    self.set_theme_mode("light");
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.tab_layout_vertical_btn,
                                ) {
                                    crate::logger::log("-> Clic en disposición: Vertical");
                                    self.set_tab_layout("vertical");
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.tab_layout_horizontal_btn,
                                ) {
                                    crate::logger::log("-> Clic en disposición: Horizontal");
                                    self.set_tab_layout("horizontal");
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.font_minus_btn) {
                                    crate::logger::log("-> Clic en fuente -");
                                    self.adjust_font_size(-1.0);
                                    let _ = self.config.save();
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.font_plus_btn) {
                                    crate::logger::log("-> Clic en fuente +");
                                    self.adjust_font_size(1.0);
                                    let _ = self.config.save();
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.spacing_minus_btn)
                                {
                                    crate::logger::log("-> Clic en espaciado -");
                                    self.adjust_line_spacing(-0.1);
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.spacing_plus_btn)
                                {
                                    crate::logger::log("-> Clic en espaciado +");
                                    self.adjust_line_spacing(0.1);
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.opacity_minus_btn)
                                {
                                    crate::logger::log("-> Clic en opacidad -");
                                    self.adjust_opacity(-0.05);
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.opacity_plus_btn)
                                {
                                    crate::logger::log("-> Clic en opacidad +");
                                    self.adjust_opacity(0.05);
                                    return;
                                }
                            }
                            SettingsSection::Terminal => {
                                if SettingsPageRects::inside(mx, my, rects.cursor_blink_btn) {
                                    crate::logger::log("-> Clic en parpadeo del cursor");
                                    self.toggle_cursor_blink();
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.ghostty_prompt_btn,
                                ) {
                                    crate::logger::log("-> Clic en prompt Ghostty");
                                    self.toggle_ghostty_prompt();
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.copy_on_select_btn,
                                ) {
                                    crate::logger::log("-> Clic en copiar al seleccionar");
                                    self.toggle_copy_on_select();
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.status_bar_toggle_btn,
                                ) {
                                    crate::logger::log("-> Clic en alternar barra de estado");
                                    self.toggle_status_bar();
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.file_explorer_toggle_btn,
                                ) {
                                    crate::logger::log("-> Clic en alternar explorador lateral de archivos (Settings)");
                                    self.toggle_file_explorer();
                                    return;
                                } else if SettingsPageRects::inside(
                                    mx,
                                    my,
                                    rects.restore_session_btn,
                                ) {
                                    crate::logger::log("-> Clic en alternar restaurar pestañas al iniciar");
                                    self.toggle_restore_session();
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.open_file_btn) {
                                    crate::logger::log("-> Clic en abrir archivo axon.toml");
                                    self.open_config_file();
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.term_reset_btn) {
                                    crate::logger::log("-> Clic en restablecer valores por defecto (Terminal)");
                                    self.reset_to_defaults();
                                    return;
                                }
                            }
                            SettingsSection::About => {
                                if SettingsPageRects::inside(mx, my, rects.open_file_btn) {
                                    crate::logger::log(
                                        "-> Clic en abrir archivo axon.toml (About)",
                                    );
                                    self.open_config_file();
                                    return;
                                } else if SettingsPageRects::inside(mx, my, rects.term_reset_btn) {
                                    crate::logger::log("-> Clic en restablecer valores por defecto (About)");
                                    self.reset_to_defaults();
                                    return;
                                }
                            }
                            SettingsSection::Shortcuts => {}
                        }
                    }
                    return;
                }

                // Clics dentro del panel del explorador lateral de archivos (a la derecha)
                if self.file_explorer.visible && !is_settings_active {
                    let panel_w = crate::explorer::FILE_EXPLORER_WIDTH;
                    if mx >= win_w.saturating_sub(panel_w) {
                        if state == ElementState::Pressed && button == MouseButton::Left {
                            let panel_x = win_w.saturating_sub(panel_w);
                            let top_offset = self.top_offset();
                            let close_x = panel_x + panel_w.saturating_sub(28);
                            if mx >= close_x
                                && mx < close_x + 20
                                && my >= top_offset + 8
                                && my < top_offset + 28
                            {
                                crate::logger::log("-> Clic en [x] cerrar explorador de archivos");
                                self.toggle_file_explorer();
                                return;
                            }

                            let refresh_x = panel_x + panel_w.saturating_sub(56);
                            if mx >= refresh_x
                                && mx < refresh_x + 20
                                && my >= top_offset + 8
                                && my < top_offset + 28
                            {
                                crate::logger::log(
                                    "-> Clic en [↻] refrescar explorador de archivos",
                                );
                                self.file_explorer.refresh();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }

                            let sub_y = top_offset + crate::explorer::FileExplorer::HEADER_HEIGHT;
                            let up_x = panel_x + 10;
                            if mx >= up_x && mx < up_x + 28 && my >= sub_y + 4 && my < sub_y + 24 {
                                crate::logger::log("-> Clic en [..] subir nivel en explorador");
                                self.file_explorer.navigate_up();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }

                            if let Some(idx) = self.file_explorer.item_at_y(my, top_offset) {
                                if idx < self.file_explorer.items.len() {
                                    let item = &self.file_explorer.items[idx];
                                    if item.is_dir {
                                        crate::logger::log(&format!(
                                            "-> Clic en carpeta: {}",
                                            item.name
                                        ));
                                        self.file_explorer.toggle_expand(idx);
                                    } else {
                                        crate::logger::log(&format!(
                                            "-> Clic en archivo: {}",
                                            item.name
                                        ));
                                        let path_str = if let Ok(rel) =
                                            item.path.strip_prefix(&self.file_explorer.root_dir)
                                        {
                                            rel.to_string_lossy().replace('\\', "/")
                                        } else {
                                            item.path.to_string_lossy().to_string()
                                        };
                                        let to_insert = if path_str.contains(' ') {
                                            format!("\"{}\" ", path_str)
                                        } else {
                                            format!("{} ", path_str)
                                        };
                                        if let Some(active_pane) =
                                            self.tabs[self.active_tab_index].active_pane()
                                        {
                                            active_pane.pty.write_input(to_insert.as_bytes());
                                        }
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }
                        return;
                    }
                }

                // 2. Clics en paneles de terminal activa
                let bypass = self.modifiers.shift_key();
                let offset = self.sidebar_offset();
                let top_offset = self.top_offset();
                let status_bar_h = self.status_bar_height();
                let right_offset = self.file_explorer_offset();
                let term_area_w = win_w.saturating_sub(offset + right_offset);
                let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
                let active_tab = &mut self.tabs[self.active_tab_index];
                let pane_rects =
                    active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);

                let clicked_pane_rect = pane_rects
                    .iter()
                    .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
                    .copied();

                if let Some(rect) = clicked_pane_rect {
                    // Cambiar foco al panel clickeado si no está activo
                    if active_tab.active_pane_id != rect.pane_id {
                        active_tab.active_pane_id = rect.pane_id;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }

                    if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                        // Clic en scrollbar de este panel
                        if let Some(renderer) = &self.renderer {
                            if let Some((_thumb_x, thumb_y, _thumb_w, thumb_h)) = renderer
                                .pane_scrollbar_geometry(
                                    rect.x,
                                    rect.y,
                                    rect.width,
                                    rect.height,
                                    &pane.terminal,
                                )
                            {
                                let scroll_margin = 16;
                                if mx >= (rect.x + rect.width).saturating_sub(scroll_margin)
                                    && button == MouseButton::Left
                                    && state == ElementState::Pressed
                                {
                                    if my >= thumb_y && my < thumb_y + thumb_h {
                                        self.scrollbar_dragging = true;
                                        self.scrollbar_drag_pane_id = Some(pane.id);
                                        self.scrollbar_drag_offset_y = (my - thumb_y) as f32;
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window {
                                            w.request_redraw();
                                        }
                                        return;
                                    } else if my < thumb_y {
                                        pane.terminal.scroll_up(pane.terminal.rows);
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window {
                                            w.request_redraw();
                                        }
                                        return;
                                    } else {
                                        pane.terminal.scroll_down(pane.terminal.rows);
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window {
                                            w.request_redraw();
                                        }
                                        return;
                                    }
                                }
                            }
                        }

                        if pane.terminal.is_mouse_reporting_active() && !bypass {
                            if let Some(renderer) = &self.renderer {
                                let term_x = mx.saturating_sub(rect.x);
                                let term_y = my.saturating_sub(rect.y);
                                let col = if term_x >= renderer.margin_x {
                                    ((term_x - renderer.margin_x) / renderer.char_width) + 1
                                } else {
                                    1
                                }
                                .min(pane.terminal.cols);
                                let row = if term_y >= renderer.margin_y {
                                    ((term_y - renderer.margin_y) / renderer.row_height) + 1
                                } else {
                                    1
                                }
                                .min(pane.terminal.rows);

                                let base_code = match button {
                                    MouseButton::Left => 0,
                                    MouseButton::Middle => 1,
                                    MouseButton::Right => 2,
                                    _ => 0,
                                };

                                let mut code = base_code;
                                if self.modifiers.shift_key() {
                                    code += 4;
                                }
                                if self.modifiers.alt_key() {
                                    code += 8;
                                }
                                if self.modifiers.control_key() {
                                    code += 16;
                                }

                                let is_pressed = state == ElementState::Pressed;
                                self.mouse_down = is_pressed && button == MouseButton::Left;

                                if pane.terminal.mouse_reporting_sgr {
                                    let suffix = if is_pressed { 'M' } else { 'm' };
                                    let seq = format!("\x1b[<{};{};{}{}", code, col, row, suffix);
                                    pane.pty.write_input(seq.as_bytes());
                                } else {
                                    let btn_byte =
                                        if is_pressed { 32 + code as u8 } else { 32 + 3 };
                                    let seq = [
                                        0x1b,
                                        b'[',
                                        b'M',
                                        btn_byte,
                                        32 + col.min(223) as u8,
                                        32 + row.min(223) as u8,
                                    ];
                                    pane.pty.write_input(&seq);
                                }
                            }
                        } else {
                            match button {
                                MouseButton::Left => match state {
                                    ElementState::Pressed => {
                                        self.mouse_down = true;
                                        if let Some(renderer) = &self.renderer {
                                            let term_x = mx.saturating_sub(rect.x);
                                            let term_y = my.saturating_sub(rect.y);
                                            if term_x >= renderer.margin_x
                                                && term_y >= renderer.margin_y
                                            {
                                                let col = (term_x - renderer.margin_x)
                                                    / renderer.char_width;
                                                let row = (term_y - renderer.margin_y)
                                                    / renderer.row_height;
                                                self.drag_start = Some((col, row));
                                                pane.terminal.selection = None;
                                                if let Some(w) = &self.window {
                                                    w.request_redraw();
                                                }
                                            }
                                        }
                                    }
                                    ElementState::Released => {
                                        self.mouse_down = false;
                                        self.drag_start = None;
                                    }
                                },
                                MouseButton::Right | MouseButton::Middle => {
                                    if state == ElementState::Pressed {
                                        self.paste_from_clipboard();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let bypass = self.modifiers.shift_key();
                let offset = self.sidebar_offset();
                let mx = self.mouse_pos.x as usize;
                let my = self.mouse_pos.y as usize;

                let (win_w, win_h) = if let Some(w) = &self.window {
                    let s = w.inner_size();
                    (s.width as usize, s.height as usize)
                } else {
                    (800, 600)
                };

                let top_offset = self.top_offset();
                let status_bar_h = self.status_bar_height();

                if self.file_explorer.visible && !self.tabs[self.active_tab_index].is_settings {
                    let panel_w = crate::explorer::FILE_EXPLORER_WIDTH;
                    if mx >= win_w.saturating_sub(panel_w) {
                        let lines = match delta {
                            MouseScrollDelta::LineDelta(_, y) => (y * 3.0).round() as isize,
                            MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0).round() as isize,
                        };
                        let list_h = win_h.saturating_sub(
                            top_offset
                                + status_bar_h
                                + crate::explorer::FileExplorer::HEADER_HEIGHT
                                + crate::explorer::FileExplorer::SUBHEADER_HEIGHT,
                        );
                        let max_lines =
                            (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);
                        self.file_explorer.scroll(lines, max_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }

                let right_offset = self.file_explorer_offset();
                let term_area_w = win_w.saturating_sub(offset + right_offset);
                let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
                let active_tab = &mut self.tabs[self.active_tab_index];
                let pane_rects =
                    active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);

                let target_pane_rect = pane_rects
                    .iter()
                    .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
                    .copied();

                let target_pane_id = target_pane_rect
                    .map(|r| r.pane_id)
                    .unwrap_or(active_tab.active_pane_id);

                if let Some(pane) = active_tab.find_pane_mut(target_pane_id) {
                    let rect_x = target_pane_rect.map(|r| r.x).unwrap_or(offset);
                    let rect_y = target_pane_rect.map(|r| r.y).unwrap_or(top_offset);

                    if pane.terminal.is_mouse_reporting_active() && !bypass {
                        if let Some(renderer) = &self.renderer {
                            let term_x = mx.saturating_sub(rect_x);
                            let term_y = my.saturating_sub(rect_y);
                            let col = if term_x >= renderer.margin_x {
                                ((term_x - renderer.margin_x) / renderer.char_width) + 1
                            } else {
                                1
                            }
                            .min(pane.terminal.cols);
                            let row = if term_y >= renderer.margin_y {
                                ((term_y - renderer.margin_y) / renderer.row_height) + 1
                            } else {
                                1
                            }
                            .min(pane.terminal.rows);

                            let is_up = match delta {
                                MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                                MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                            };

                            let mut code = if is_up { 64 } else { 65 };
                            if self.modifiers.shift_key() {
                                code += 4;
                            }
                            if self.modifiers.alt_key() {
                                code += 8;
                            }
                            if self.modifiers.control_key() {
                                code += 16;
                            }

                            if pane.terminal.mouse_reporting_sgr {
                                let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                pane.pty.write_input(seq.as_bytes());
                            } else {
                                let seq = [
                                    0x1b,
                                    b'[',
                                    b'M',
                                    32 + code as u8,
                                    32 + col.min(223) as u8,
                                    32 + row.min(223) as u8,
                                ];
                                pane.pty.write_input(&seq);
                            }
                        }
                    } else {
                        let lines = match delta {
                            MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1) * 3,
                            MouseScrollDelta::PixelDelta(pos) => {
                                ((pos.y.abs() / 20.0).ceil() as usize).max(1) * 3
                            }
                        };

                        let is_up = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                            MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                        };

                        if is_up {
                            pane.terminal.scroll_up(lines);
                        } else {
                            pane.terminal.scroll_down(lines);
                        }

                        self.last_scroll_time = Instant::now();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::Focused(focused) => {
                crate::logger::log(&format!("FOCUS: {}", focused));
                self.is_focused = focused;
                if !focused {
                    self.modifiers = ModifiersState::empty();
                    self.mouse_down = false;
                    self.drag_start = None;
                } else {
                    self.modifiers = ModifiersState::empty();
                }

                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = active.active_pane_mut() {
                        if pane.terminal.focus_event_reporting {
                            if focused {
                                pane.pty.write_input(b"\x1b[I");
                            } else {
                                pane.pty.write_input(b"\x1b[O");
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 1. Revisar si algún panel tiene su proceso terminado
        let mut closed_panes = Vec::new();
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                if pane.pty.is_child_exited() {
                    closed_panes.push(pane.id);
                }
            }
        }
        for pane_id in closed_panes {
            crate::logger::log(&format!(
                "Proceso hijo de pane {} terminado: cerrando pane",
                pane_id
            ));
            self.close_pane_by_id(pane_id, event_loop);
        }
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando Axon");
            event_loop.exit();
            return;
        }

        // 2. Drenar salida de ConPTY de todos los paneles por si quedó algún byte residual
        let mut got_bytes = false;
        let active_idx = self.active_tab_index;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let mut tab_got_bytes = false;
            for pane in &mut tab.panes {
                let mut pane_got_bytes = false;
                while let Some(bytes) = pane.pty.try_recv_output() {
                    pane_got_bytes = true;
                    pane.terminal.process_bytes(&bytes);
                }
                for text in pane.terminal.clipboard_queue.drain(..) {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set_text(text).ok();
                    }
                }
                if pane_got_bytes {
                    tab_got_bytes = true;
                }
            }
            if tab_got_bytes {
                got_bytes = true;
                if idx != active_idx {
                    tab.has_unread = true;
                }
            }
        }

        // 3. Solicitar repintado si llegaron nuevos bytes del PTY
        if got_bytes {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        // 4. Temporizador de parpadeo de cursor cuando la terminal está inactiva y tiene foco
        if self.config.terminal.cursor_blink && self.is_focused {
            let elapsed = self.cursor_timer.elapsed().as_millis();
            if elapsed >= 500 {
                self.cursor_timer = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                    std::time::Duration::from_millis(500),
                ));
            } else {
                let remain = (500 - elapsed).max(16) as u64;
                event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                    std::time::Duration::from_millis(remain),
                ));
            }
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }

        // 5. Temporizador de auto-ocultamiento de la barra de desplazamiento
        let scroll_elapsed = self.last_scroll_time.elapsed().as_millis();
        if scroll_elapsed < 1250 {
            let remain = (1250 - scroll_elapsed) as u64;
            event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                std::time::Duration::from_millis(remain),
            ));
        }

        // 6. Verificación periódica del tema del sistema (cada 2 segundos) si theme_mode es "system"
        if self
            .config
            .appearance
            .theme_mode
            .eq_ignore_ascii_case("system")
        {
            if self.last_system_theme_check.elapsed().as_secs() >= 2 {
                self.last_system_theme_check = Instant::now();
                let is_sys_light = crate::theme::is_windows_system_light_mode();
                let currently_light = self.theme.is_light();
                if is_sys_light != currently_light {
                    crate::logger::log(&format!("-> Cambio de tema del sistema Windows detectado (light={}): actualizando interfaz", is_sys_light));
                    let new_theme = if is_sys_light {
                        Theme::catppuccin_latte()
                    } else {
                        Theme::from_name(&self.config.appearance.theme)
                    };
                    self.theme = new_theme.clone();
                    for tab in &mut self.tabs {
                        for pane in &mut tab.panes {
                            pane.terminal.theme = new_theme.clone();
                        }
                    }
                    if let Some(w) = &self.window {
                        update_windows_theme(w, &self.theme);
                        w.request_redraw();
                    }
                }
            }

            match event_loop.control_flow() {
                winit::event_loop::ControlFlow::Wait => {
                    event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                        std::time::Duration::from_secs(2),
                    ));
                }
                winit::event_loop::ControlFlow::WaitUntil(instant) => {
                    let max_target = Instant::now() + std::time::Duration::from_secs(2);
                    if instant > max_target {
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                            std::time::Duration::from_secs(2),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
}

fn create_native_icon() -> Icon {
    const ICON_BYTES: &[u8] = include_bytes!("../assets/icon-256.png");
    if let Ok(img) = image::load_from_memory(ICON_BYTES) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        if let Ok(icon) = Icon::from_rgba(rgba.into_raw(), w, h) {
            return icon;
        }
    }

    let size = 32;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let in_arrow_top =
                x >= 9 && x <= 16 && (y as i32 - x as i32).abs() <= 1 && y >= 9 && y <= 16;
            let in_arrow_bot =
                x >= 9 && x <= 16 && (y as i32 + x as i32 - 32).abs() <= 1 && y >= 16 && y <= 23;
            let in_cursor = x >= 18 && x <= 24 && (y == 21 || y == 22);

            if in_arrow_top || in_arrow_bot || in_cursor {
                rgba[idx] = 0x51; // Verde Ghostty #51e27a
                rgba[idx + 1] = 0xe2;
                rgba[idx + 2] = 0x7a;
                rgba[idx + 3] = 255;
            } else {
                // Totalmente transparente (sin fondo oscuro)
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32).expect("Error al crear icono de ventana")
}

#[cfg(target_os = "windows")]
fn update_windows_theme(window: &Window, theme: &Theme) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                let is_light = theme.is_light();
                // 1. Dark Mode inmersivo (0 = Light, 1 = Dark)
                let dark_mode: i32 = if is_light { 0 } else { 1 };
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    20, // DWMWA_USE_IMMERSIVE_DARK_MODE
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                let to_bgr = |c: crate::theme::Color| -> u32 {
                    let r = c.r as u32;
                    let g = c.g as u32;
                    let b = c.b as u32;
                    (b << 16) | (g << 8) | r
                };

                // 2. Color de fondo del titlebar acorde al sidebar/pestañas
                let caption_color: u32 = to_bgr(theme.sidebar_bg());
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    35, // DWMWA_CAPTION_COLOR
                    &caption_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 3. Color del texto del titlebar
                let text_color: u32 = to_bgr(theme.foreground);
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    36, // DWMWA_TEXT_COLOR
                    &text_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 4. Color del borde sutil de la ventana
                let border_color: u32 = to_bgr(theme.border_color());
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    34, // DWMWA_BORDER_COLOR
                    &border_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 5. Esquinas redondeadas nativas de Windows 11
                let corner_pref: u32 = 2; // DWMWCP_ROUND
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    33, // DWMWA_WINDOW_CORNER_PREFERENCE
                    &corner_pref as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 6. Efecto de fondo Mica nativo de Windows 11
                let backdrop_type: u32 = 2; // 2 = DWMSBT_MAINWINDOW (Mica)
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    38, // DWMWA_SYSTEMBACKDROP_TYPE
                    &backdrop_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 7. Establecer el pincel de fondo de la clase Win32
                let brush =
                    windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(to_bgr(theme.background));
                if brush != 0 {
                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::GCLP_HBRBACKGROUND,
                        brush as isize,
                    );
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn update_windows_theme(_window: &Window, _theme: &Theme) {}

#[cfg(target_os = "windows")]
fn get_physical_modifiers() -> (bool, bool, bool, bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_RCONTROL, VK_RMENU, VK_SHIFT,
    };

    unsafe {
        let is_down = |vk: u16| -> bool { (GetAsyncKeyState(vk as i32) as i16) < 0 };

        let shift = is_down(VK_SHIFT);
        let rmenu = is_down(VK_RMENU); // AltGr (Right Alt)
        let lmenu = is_down(VK_LMENU); // Left Alt
        let lctrl = is_down(VK_LCONTROL);
        let rctrl = is_down(VK_RCONTROL);

        // En Windows, presionar AltGr (RMENU) inyecta automáticamente un LCONTROL sintético.
        // Por tanto, Ctrl solo es genuino si RCONTROL está presionado, o si LCONTROL
        // está presionado SIN que RMENU (AltGr) esté activo.
        let ctrl = rctrl || (lctrl && !rmenu);
        let alt = lmenu;
        let altgr = rmenu;

        (ctrl, shift, alt, altgr)
    }
}

#[cfg(not(target_os = "windows"))]
fn get_physical_modifiers() -> (bool, bool, bool, bool) {
    (false, false, false, false)
}

pub fn key_code_to_char(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Equal => Some('='),
        KeyCode::NumpadAdd => Some('+'),
        KeyCode::Comma => Some(','),
        _ => None,
    }
}

pub fn key_is_char(key: &Key, physical: PhysicalKey, target: char) -> bool {
    let target_lower = target.to_ascii_lowercase();

    // 1. Comprobar tecla física (PhysicalKey) por hardware KeyCode.
    // En Windows, cuando Alt está presionado (WM_SYSKEYDOWN), ToUnicodeEx no genera
    // caracteres y winit reporta logical_key como Key::Unidentified.
    // Por tanto, verificar physical_key garantiza que los atajos con Alt y Ctrl
    // funcionen de manera 100% confiable independientemente de la plataforma o del estado SYSKEY.
    if let PhysicalKey::Code(code) = physical {
        if let Some(c) = key_code_to_char(code) {
            if c == target_lower {
                return true;
            }
        }
    }

    // 2. Comprobar logical_key (para layouts no QWERTY donde el carácter semántico coincide)
    match key {
        Key::Character(s) => {
            if let Some(c) = s.chars().next() {
                if c.to_ascii_lowercase() == target_lower {
                    return true;
                }
                let target_lower = target.to_ascii_lowercase();
                if target_lower >= 'a' && target_lower <= 'z' {
                    let code = (target_lower as u8) - b'a' + 1;
                    if c as u32 == code as u32 {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Obtiene rutas de archivos copiados al portapapeles desde el Explorador de Windows (CF_HDROP)
#[cfg(target_os = "windows")]
fn get_clipboard_files() -> Option<Vec<String>> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;
    const CF_HDROP: u32 = 15;

    unsafe {
        if OpenClipboard(0) == 0 {
            return None;
        }

        let handle = GetClipboardData(CF_HDROP);
        if handle == 0 {
            CloseClipboard();
            return None;
        }

        let count = DragQueryFileW(handle as _, 0xFFFFFFFF, std::ptr::null_mut(), 0);
        let mut paths = Vec::with_capacity(count as usize);

        for i in 0..count {
            let len = DragQueryFileW(handle as _, i, std::ptr::null_mut(), 0);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                DragQueryFileW(handle as _, i, buf.as_mut_ptr(), len + 1);
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                paths.push(path);
            }
        }

        CloseClipboard();

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_clipboard_files() -> Option<Vec<String>> {
    None
}

/// Comprueba si Claude Code (o un agente CLI compatible con imágenes) se encuentra activo en el sistema
#[cfg(target_os = "windows")]
pub(crate) fn is_claude_running() -> bool {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == 0 || snapshot == -1isize as _ {
            return false;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                if name.contains("claude") {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        windows_sys::Win32::Foundation::CloseHandle(snapshot);
        found
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_claude_running() -> bool {
    false
}

fn get_git_branch(start_dir: Option<&std::path::Path>) -> Option<String> {
    let mut cur = start_dir?.to_path_buf();
    for _ in 0..8 {
        let git_dir = cur.join(".git");
        if git_dir.is_dir() {
            let head_path = git_dir.join("HEAD");
            if let Ok(content) = std::fs::read_to_string(head_path) {
                let line = content.lines().next()?.trim();
                if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                } else if line.len() >= 7 {
                    return Some(line[..7].to_string());
                }
            }
            return None;
        } else if git_dir.is_file() {
            if let Ok(content) = std::fs::read_to_string(&git_dir) {
                if let Some(path_str) = content
                    .lines()
                    .next()
                    .and_then(|l| l.strip_prefix("gitdir: "))
                {
                    let actual_git_dir = cur.join(path_str.trim());
                    let head_path = actual_git_dir.join("HEAD");
                    if let Ok(head_content) = std::fs::read_to_string(head_path) {
                        let line = head_content.lines().next()?.trim();
                        if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                            return Some(branch.to_string());
                        }
                    }
                }
            }
            return None;
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

pub(crate) fn detect_ai_agent(title: Option<&str>) -> AiAgentStatus {
    if let Some(t) = title {
        let lower = t.to_lowercase();
        if lower.contains("claude") {
            return AiAgentStatus::Claude;
        }
        if lower.contains("agy") || lower.contains("antigravity") {
            return AiAgentStatus::Agy;
        }
        if lower.contains("gemini") {
            return AiAgentStatus::Gemini;
        }
        if lower.contains("aider") {
            return AiAgentStatus::Aider;
        }
        if lower.contains("copilot") {
            return AiAgentStatus::Copilot;
        }
        if lower.contains("ollama") {
            return AiAgentStatus::Ollama;
        }
    }
    AiAgentStatus::None
}
