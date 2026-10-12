use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::renderer::{SidebarTabItem, TerminalRenderer};
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
}

pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    pub panes: Vec<TerminalPane>,
    pub active_pane_id: usize,
    pub layout: PaneNode,
    pub has_unread: bool,
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
        }
    }

    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.panes.iter().find(|p| p.id == self.active_pane_id)
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

    pub fn compute_pane_rects(&self, total_x: usize, total_y: usize, total_w: usize, total_h: usize) -> Vec<PaneRect> {
        let mut rects = Vec::new();
        self.collect_rects(&self.layout, total_x, total_y, total_w, total_h, &mut rects);
        rects
    }

    fn collect_rects(&self, node: &PaneNode, x: usize, y: usize, w: usize, h: usize, out: &mut Vec<PaneRect>) {
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
            PaneNode::Split { direction, ratio, first, second } => {
                match direction {
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
                }
            }
        }
    }

    pub fn compute_dividers(&self, total_x: usize, total_y: usize, total_w: usize, total_h: usize) -> Vec<(usize, usize, usize, usize)> {
        let mut dividers = Vec::new();
        self.collect_dividers(&self.layout, total_x, total_y, total_w, total_h, &mut dividers);
        dividers
    }

    fn collect_dividers(&self, node: &PaneNode, x: usize, y: usize, w: usize, h: usize, out: &mut Vec<(usize, usize, usize, usize)>) {
        const DIVIDER: usize = 2;
        match node {
            PaneNode::Leaf(_) => {}
            PaneNode::Split { direction, ratio, first, second } => {
                match direction {
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
                }
            }
        }
    }

    pub fn split_active_pane(&mut self, new_pane_id: usize, new_pane: TerminalPane, direction: SplitDirection) {
        let active_id = self.active_pane_id;
        self.layout = Self::split_leaf_in_tree(self.layout.clone(), active_id, new_pane_id, direction);
        self.panes.push(new_pane);
        self.active_pane_id = new_pane_id;
    }

    fn split_leaf_in_tree(node: PaneNode, target_id: usize, new_id: usize, direction: SplitDirection) -> PaneNode {
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
            PaneNode::Split { direction: d, ratio, first, second } => {
                PaneNode::Split {
                    direction: d,
                    ratio,
                    first: Box::new(Self::split_leaf_in_tree(*first, target_id, new_id, direction)),
                    second: Box::new(Self::split_leaf_in_tree(*second, target_id, new_id, direction)),
                }
            }
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
            PaneNode::Split { direction, ratio, first, second } => {
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
    config: AxonConfig,
    theme: Theme,
    proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: usize,
    next_pane_id: usize,
    pub sidebar_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_toggle_btn: bool,
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
    #[allow(dead_code)]
    hovered_scrollbar: bool,
}

impl AxonApp {
    pub fn new(config: AxonConfig, proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>) -> Self {
        let cols = 100;
        let rows = 30;
        let theme = Theme::from_name(&config.appearance.theme);

        // Restaurar estado de sesiones previas si existe
        let saved_session = crate::config::SessionState::load();
        let mut next_pane_id = 0;
        let (tabs, active_tab_index, next_tab_id, sidebar_visible) = if let Some(session) = saved_session {
            if !session.tabs.is_empty() {
                let mut restored_tabs = Vec::new();
                for (idx, tab_info) in session.tabs.into_iter().enumerate() {
                    let pane_id = next_pane_id;
                    next_pane_id += 1;
                    let cwd_path = tab_info.cwd.as_ref().map(std::path::PathBuf::from);
                    let cwd_ref = cwd_path.as_deref();
                    let pty = PtyManager::new(
                        pane_id,
                        &config.terminal.default_shell,
                        config.terminal.ghostty_prompt,
                        cols as u16,
                        rows as u16,
                        cwd_ref,
                        proxy.clone(),
                    ).expect("Error al inicializar Windows ConPTY");

                    let mut terminal = TerminalState::new(
                        cols,
                        rows,
                        config.terminal.scrollback_lines,
                        theme.clone(),
                    );
                    terminal.current_dir = cwd_path;

                    let pane = TerminalPane {
                        id: pane_id,
                        pty,
                        terminal,
                        last_grid_size: (cols, rows),
                    };

                    restored_tabs.push(TerminalTab::new(idx, tab_info.title, pane));
                }
                let active = session.active_tab_index.min(restored_tabs.len() - 1);
                let next_id = restored_tabs.len();
                (restored_tabs, active, next_id, session.sidebar_visible)
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
                ).expect("Error al inicializar Windows ConPTY");
                let terminal = TerminalState::new(cols, rows, config.terminal.scrollback_lines, theme.clone());
                let pane = TerminalPane {
                    id: pane_id,
                    pty,
                    terminal,
                    last_grid_size: (cols, rows),
                };
                (vec![TerminalTab::new(0, "Terminal 1".to_string(), pane)], 0, 1, true)
            }
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
            ).expect("Error al inicializar Windows ConPTY");
            let terminal = TerminalState::new(cols, rows, config.terminal.scrollback_lines, theme.clone());
            let pane = TerminalPane {
                id: pane_id,
                pty,
                terminal,
                last_grid_size: (cols, rows),
            };
            (vec![TerminalTab::new(0, "Terminal 1".to_string(), pane)], 0, 1, true)
        };

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
            hovered_toggle_btn: false,
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
            hovered_scrollbar: false,
        }
    }

    pub fn save_session(&self) {
        let tab_sessions = self.tabs.iter().map(|tab| {
            let cwd = tab.active_pane()
                .and_then(|p| p.terminal.current_dir.as_ref())
                .map(|p| p.to_string_lossy().to_string())
                .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()));
            crate::config::TabSession {
                title: tab.title.clone(),
                cwd,
            }
        }).collect();
        let state = crate::config::SessionState {
            active_tab_index: self.active_tab_index,
            sidebar_visible: self.sidebar_visible,
            tabs: tab_sessions,
        };
        let _ = state.save();
    }

    pub fn sidebar_offset(&self) -> usize {
        if self.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    pub fn create_new_tab(&mut self) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let (cols, rows) = if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            renderer.compute_grid_size(size.width as usize, size.height as usize, self.sidebar_offset())
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

        let pane = TerminalPane {
            id: pane_id,
            pty,
            terminal,
            last_grid_size: (cols, rows),
        };

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
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        // Heredar el directorio de trabajo del pane activo
        let cwd_path = self.tabs.get(self.active_tab_index)
            .and_then(|t| t.active_pane())
            .and_then(|p| p.terminal.current_dir.clone());
        let cwd_ref = cwd_path.as_deref();

        let (cols, rows) = (80, 24);

        let pty = match PtyManager::new(
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

        let mut terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );
        terminal.current_dir = cwd_path;

        let new_pane = TerminalPane {
            id: new_pane_id,
            pty,
            terminal,
            last_grid_size: (cols, rows),
        };

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
        if let (Some(window), Some(_renderer)) = (&self.window, &self.renderer) {
            let size = window.inner_size();
            let offset = self.sidebar_offset();
            let total_w = (size.width as usize).saturating_sub(offset);
            let total_h = size.height as usize;
            if total_w == 0 || total_h == 0 {
                return;
            }

            let active_tab = &mut self.tabs[self.active_tab_index];
            let rects = active_tab.compute_pane_rects(offset, 0, total_w, total_h);
            if rects.len() <= 1 {
                return;
            }

            let cur_rect = match rects.iter().find(|r| r.pane_id == active_tab.active_pane_id) {
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
            let pane_info = if tab.panes.len() > 1 {
                format!(" ({} paneles)", tab.panes.len())
            } else {
                String::new()
            };
            w.set_title(&format!("Axon - {}{}", tab.title, pane_info));
        }
    }

    fn recompute_grid_for_all_tabs(&mut self) {
        if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            let offset = self.sidebar_offset();
            let total_w = (size.width as usize).saturating_sub(offset);
            let total_h = size.height as usize;
            if total_w == 0 || total_h == 0 {
                return;
            }

            for tab in &mut self.tabs {
                let pane_rects = tab.compute_pane_rects(offset, 0, total_w, total_h);
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
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = &mut self.clipboard else { return; };

        // 1. Si hay texto en el portapapeles (código, comandos, texto plano), pegar directamente
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                self.paste_text(&text);
                return;
            }
        }

        // 2. Si no hay texto, comprobar si hay una imagen (captura de pantalla, recorte, imagen web)
        if let Ok(img) = cb.get_image() {
            // Si Claude Code (u otro agente CLI compatible) está activo, enviamos la secuencia
            // nativa de pegado de imágenes (Alt + V: \x1bv). Esto hace que Claude Code
            // procese la imagen nativamente y muestre el chip interactivo [Image 1] en el prompt.
            if is_claude_running() {
                if let Some(active_pane) = self.tabs[self.active_tab_index].active_pane() {
                    active_pane.pty.write_input(b"\x1bv");
                }
                return;
            }

            // Si Claude Code no está corriendo (ej: prompt PowerShell estándar u otras herramientas),
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
                crate::logger::log(&format!("PtyClosed recibido para pane_id {}: cerrando pane", pane_id));
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
            .with_window_icon(Some(icon));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Error al crear ventana nativa de Windows"),
        );

        // Habilitar barra de título oscura inmersiva de Windows 11 (DWM Dark Mode)
        // y fijar el pincel de fondo de clase Win32 al tema oscuro para evitar flash blanco
        enable_windows_dark_mode(&window);

        let context = softbuffer::Context::new(window.clone())
            .expect("Error al inicializar contexto de aceleración gráfica de pantalla");
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .expect("Error al inicializar superficie de renderizado nativo");

        let scale_factor = window.scale_factor();
        let mut renderer = TerminalRenderer::new(&self.config, scale_factor);

        let initial_size = window.inner_size();
        if let (Some(w), Some(h)) = (NonZeroU32::new(initial_size.width), NonZeroU32::new(initial_size.height)) {
            let _ = surface.resize(w, h);
        }

        let offset = self.sidebar_offset();
        let total_w = (initial_size.width as usize).saturating_sub(offset);
        let total_h = initial_size.height as usize;
        for tab in &mut self.tabs {
            let pane_rects = tab.compute_pane_rects(offset, 0, total_w, total_h);
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
            if self.sidebar_visible {
                let sidebar_tabs: Vec<SidebarTabItem> = self.tabs.iter().enumerate().map(|(idx, t)| SidebarTabItem {
                    id: t.id,
                    title: t.title.clone(),
                    is_active: idx == self.active_tab_index,
                    has_unread: t.has_unread,
                    pane_count: t.panes.len(),
                }).collect();

                renderer.render_sidebar(
                    &mut buffer,
                    initial_size.width as usize,
                    initial_size.height as usize,
                    SIDEBAR_WIDTH,
                    &sidebar_tabs,
                    self.hovered_tab,
                    self.hovered_close,
                    self.hovered_new_tab,
                    self.hovered_toggle_btn,
                    &self.theme,
                );
            }

            let active_tab = &self.tabs[self.active_tab_index];
            let pane_rects = active_tab.compute_pane_rects(offset, 0, total_w, total_h);
            let dividers = active_tab.compute_dividers(offset, 0, total_w, total_h);
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
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
                    if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        let _ = surface.resize(w, h);
                    }
                }
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let offset = self.sidebar_offset();
                let sidebar_visible = self.sidebar_visible;
                let active_idx = self.active_tab_index;
                let hovered_tab = self.hovered_tab;
                let hovered_close = self.hovered_close;
                let hovered_new_tab = self.hovered_new_tab;
                let hovered_toggle_btn = self.hovered_toggle_btn;
                let cursor_visible = !self.config.terminal.cursor_blink
                    || (self.cursor_timer.elapsed().as_millis() % 1000) < 600;

                let sidebar_tabs: Vec<SidebarTabItem> = if sidebar_visible {
                    self.tabs.iter().enumerate().map(|(idx, t)| SidebarTabItem {
                        id: t.id,
                        title: t.title.clone(),
                        is_active: idx == active_idx,
                        has_unread: t.has_unread,
                        pane_count: t.panes.len(),
                    }).collect()
                } else {
                    Vec::new()
                };

                let active_tab = &self.tabs[active_idx];

                if let (Some(surface), Some(renderer), Some(window)) =
                    (&mut self.surface, &mut self.renderer, &self.window)
                {
                    let size = window.inner_size();
                    if size.width == 0 || size.height == 0 {
                        return;
                    }

                    if let Ok(mut buffer) = surface.buffer_mut() {
                        if sidebar_visible {
                            renderer.render_sidebar(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                SIDEBAR_WIDTH,
                                &sidebar_tabs,
                                hovered_tab,
                                hovered_close,
                                hovered_new_tab,
                                hovered_toggle_btn,
                                &self.theme,
                            );
                        }

                        let term_area_w = (size.width as usize).saturating_sub(offset);
                        let term_area_h = size.height as usize;
                        let pane_rects = active_tab.compute_pane_rects(offset, 0, term_area_w, term_area_h);
                        let dividers = active_tab.compute_dividers(offset, 0, term_area_w, term_area_h);
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

                        let divider_color = crate::theme::Color::from_rgb(38, 38, 43);
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

                        if !sidebar_visible && hovered_toggle_btn {
                            renderer.render_floating_sidebar_toggle(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                hovered_toggle_btn,
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
                // procesar exclusivamente como entrada de texto y nunca como atajo de control
                if altgr {
                    if let Some(t) = text {
                        if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                            crate::logger::log(&format!("-> AltGr text: {:?}", t));
                            if let Some(active_pane) = self.tabs[self.active_tab_index].active_pane() {
                                active_pane.pty.write_input(t.as_bytes());
                            }
                        }
                    }
                    return;
                }

                // Alt + Tab es el atajo de cambio de tarea de Windows; NUNCA debe enviar \t al shell
                if alt && logical_key == Key::Named(NamedKey::Tab) {
                    crate::logger::log("-> Ignorado Alt+Tab");
                    return;
                }

                // --- ATAJOS DE PANELES DIVIDIDOS (SPLIT PANES) ---
                // 1. División Horizontal: Ctrl + Alt + D
                if ctrl && alt && key_is_char(&logical_key, 'd') {
                    crate::logger::log("-> Hotkey: Split Horizontal (Ctrl+Alt+D)");
                    self.split_active_pane(SplitDirection::Horizontal);
                    return;
                }

                // 2. División Vertical: Ctrl + Alt + V o Ctrl + Alt + E
                if ctrl && alt && (key_is_char(&logical_key, 'v') || key_is_char(&logical_key, 'e')) {
                    crate::logger::log("-> Hotkey: Split Vertical (Ctrl+Alt+V / Ctrl+Alt+E)");
                    self.split_active_pane(SplitDirection::Vertical);
                    return;
                }

                // 3. Cerrar panel activo en split: Ctrl + Alt + W
                if ctrl && alt && key_is_char(&logical_key, 'w') {
                    crate::logger::log("-> Hotkey: Cerrar panel activo (Ctrl+Alt+W)");
                    self.close_active_pane(event_loop);
                    return;
                }

                // 4. Navegación direccional entre paneles: Ctrl + Alt + Flechas
                if ctrl && alt {
                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            self.navigate_pane(PaneDirection::Up);
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            self.navigate_pane(PaneDirection::Down);
                            return;
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.navigate_pane(PaneDirection::Left);
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.navigate_pane(PaneDirection::Right);
                            return;
                        }
                        _ => {}
                    }
                }

                // --- ATAJOS DE GESTIÓN DE PESTAÑAS (TABS & SIDEBAR) ---
                // 1. Nueva pestaña / terminal: Ctrl + T (o Ctrl + Shift + T)
                if ctrl && !alt && key_is_char(&logical_key, 't') {
                    crate::logger::log("-> Hotkey: Nueva terminal (Ctrl+T)");
                    self.create_new_tab();
                    return;
                }

                // 2. Cerrar pestaña activa: Ctrl + W (o Ctrl + Shift + W)
                if ctrl && !alt && key_is_char(&logical_key, 'w') {
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
                        _ => None,
                    };
                    if let Some(idx) = tab_num {
                        if idx < self.tabs.len() {
                            crate::logger::log(&format!("-> Hotkey: Ir a pestaña {}", idx + 1));
                            self.switch_to_tab(idx);
                            return;
                        }
                    }
                }

                // 5. Alternar barra lateral (Sidebar toggle): Ctrl + B o Ctrl + Shift + B
                if ctrl && key_is_char(&logical_key, 'b') {
                    crate::logger::log("-> Hotkey: Alternar barra lateral (Ctrl+B)");
                    self.toggle_sidebar();
                    return;
                }

                // 6. Pegar desde el portapapeles (Ctrl + Shift + V, Ctrl + V, Shift + Insert)
                if (ctrl && shift && key_is_char(&logical_key, 'v'))
                    || (ctrl && !shift && key_is_char(&logical_key, 'v'))
                    || (shift && logical_key == Key::Named(NamedKey::Insert))
                {
                    self.paste_from_clipboard();
                    return;
                }

                // 7. Zoom dinámico de tipografía (Ctrl + +, Ctrl + -, Ctrl + 0)
                if ctrl && !shift {
                    if key_is_char(&logical_key, '+') || key_is_char(&logical_key, '=') {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '-') || key_is_char(&logical_key, '_') {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '0') {
                        self.reset_font_size();
                        return;
                    }
                } else if ctrl && shift {
                    if key_is_char(&logical_key, '+') || key_is_char(&logical_key, '=') {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '_') || key_is_char(&logical_key, '-') {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                }

                // 8. Recarga dinámica de configuración Ghostty (Ctrl + Shift + ,)
                if ctrl && shift && key_is_char(&logical_key, ',') {
                    self.reload_config();
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
                    && !(shift && (logical_key == Key::Named(NamedKey::Home) || logical_key == Key::Named(NamedKey::End)))
                {
                    active.terminal.reset_scroll();
                }

                // Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
                if (ctrl && shift && key_is_char(&logical_key, 'c'))
                    || (ctrl && !shift && key_is_char(&logical_key, 'c') && active.terminal.selection.is_some())
                {
                    if let Some(sel_text) = active.terminal.copy_selection_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(sel_text).ok();
                        }
                        active.terminal.selection = None;
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                    return;
                }

                // Combinaciones con Alt (Alt + V para imágenes en Claude Code, Alt + Flechas)
                if alt && !ctrl {
                    if key_is_char(&logical_key, 'v') {
                        active.pty.write_input(b"\x1bv");
                        return;
                    }

                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            active.pty.write_input(b"\x1bb"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            active.pty.write_input(b"\x1bf"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            active.pty.write_input(b"\x1bd"); // Borrado de palabra adelante
                            return;
                        }
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
                        _ => {}
                    }
                }

                // Copiar salida del último comando del agente (Ctrl + Shift + O)
                if ctrl && shift && key_is_char(&logical_key, 'o') {
                    if let Some(output) = active.terminal.get_last_output_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(output).ok();
                        }
                    }
                    return;
                }

                // Limpiar pantalla y scrollback buffer al estilo Ghostty (Ctrl + Shift + K)
                if ctrl && shift && (key_is_char(&logical_key, 'k') || key_is_char(&logical_key, 'l')) {
                    active.terminal.scrollback.clear();
                    active.terminal.reset_scroll();
                    active.terminal.command_blocks.clear();
                    active.pty.write_input(b"\x0c");
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // Navegación semántica entre comandos de agentes OSC 133 (Ctrl + Shift + Arriba/Abajo)
                if ctrl && shift {
                    if let Key::Named(NamedKey::ArrowUp) = logical_key {
                        if let Some(block) = active.terminal.command_blocks.last() {
                            let target = active.terminal.total_lines_count.saturating_sub(block.prompt_line);
                            active.terminal.scroll_offset = target.min(active.terminal.scrollback.len());
                            if let Some(w) = &self.window { w.request_redraw(); }
                        }
                        return;
                    }
                    if let Key::Named(NamedKey::ArrowDown) = logical_key {
                        active.terminal.reset_scroll();
                        if let Some(w) = &self.window { w.request_redraw(); }
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
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let half = (active.terminal.rows / 2).max(1);
                            active.terminal.scroll_down(half);
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            active.terminal.scroll_offset = active.terminal.scrollback.len();
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            active.terminal.reset_scroll();
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window { w.request_redraw(); }
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
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }
                if logical_key == Key::Named(NamedKey::PageDown) {
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_down(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
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
                let mx = position.x as usize;
                let my = position.y as usize;

                let (win_w, win_h) = if let Some(w) = &self.window {
                    let s = w.inner_size();
                    (s.width as usize, s.height as usize)
                } else {
                    (800, 600)
                };

                let mut needs_redraw = false;

                // Manejo de arrastre activo del pulgar de la barra de desplazamiento en el panel seleccionado
                if self.scrollbar_dragging {
                    if let Some(pid) = self.scrollbar_drag_pane_id {
                        let active_tab = &mut self.tabs[self.active_tab_index];
                        let term_area_w = win_w.saturating_sub(offset);
                        let pane_rects = active_tab.compute_pane_rects(offset, 0, term_area_w, win_h);
                        if let Some(target_rect) = pane_rects.iter().find(|r| r.pane_id == pid) {
                            if let Some(pane) = active_tab.find_pane_mut(pid) {
                                if let Some(renderer) = &self.renderer {
                                    if !pane.terminal.scrollback.is_empty() {
                                        let track_y = target_rect.y + renderer.margin_y;
                                        let track_h = target_rect.height.saturating_sub(renderer.margin_y * 2);
                                        let total_history = pane.terminal.scrollback.len() + pane.terminal.rows;
                                        let thumb_h = ((pane.terminal.rows as f32 / total_history as f32) * track_h as f32).max(24.0).min(track_h as f32) as usize;
                                        let available_travel = track_h.saturating_sub(thumb_h);
                                        if available_travel > 0 {
                                            let target_thumb_top = (my as f32 - self.scrollbar_drag_offset_y - track_y as f32).clamp(0.0, available_travel as f32);
                                            let ratio = target_thumb_top / available_travel as f32;
                                            let new_offset = ((1.0 - ratio) * pane.terminal.scrollback.len() as f32).round() as usize;
                                            pane.terminal.scroll_offset = new_offset.min(pane.terminal.scrollback.len());
                                            self.last_scroll_time = Instant::now();
                                            if let Some(w) = &self.window { w.request_redraw(); }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }

                // Comprobar hover en botón inferior de alternar barra lateral
                let toggle_btn_w = if self.sidebar_visible { SIDEBAR_WIDTH.saturating_sub(16) } else { 32 };
                let toggle_btn_h = 30;
                let toggle_btn_x = 8;
                let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8);

                let is_hover_toggle = mx >= toggle_btn_x
                    && mx < toggle_btn_x + toggle_btn_w
                    && my >= toggle_btn_y
                    && my < toggle_btn_y + toggle_btn_h;

                if self.hovered_toggle_btn != is_hover_toggle {
                    self.hovered_toggle_btn = is_hover_toggle;
                    needs_redraw = true;
                }

                // 1. Detección de hover en la barra lateral si está visible
                if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                    // Hover en botón "+"
                    let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                    let plus_btn_y = 8;
                    let plus_btn_w = 24;
                    let plus_btn_h = 24;
                    let is_new_tab = mx >= plus_btn_x && mx < plus_btn_x + plus_btn_w && my >= plus_btn_y && my < plus_btn_y + plus_btn_h;
                    if self.hovered_new_tab != is_new_tab {
                        self.hovered_new_tab = is_new_tab;
                        needs_redraw = true;
                    }

                    // Hover en pestañas y botón de cerrar "x"
                    let tab_item_h = 34;
                    let tab_spacing = 4;
                    let list_start_y = 42;
                    let tab_x = 8;
                    let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                    let list_max_y = win_h.saturating_sub(44);

                    let mut new_hovered_tab = None;
                    let mut new_hovered_close = None;

                    if my >= list_start_y && my < list_max_y {
                        let rel_y = my - list_start_y;
                        let item_total = tab_item_h + tab_spacing;
                        let tab_idx = rel_y / item_total;
                        let in_item = (rel_y % item_total) < tab_item_h;

                        if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w {
                            new_hovered_tab = Some(tab_idx);

                            if self.tabs.len() > 1 {
                                let tab_y = list_start_y + tab_idx * item_total;
                                let close_x = tab_x + tab_w.saturating_sub(22);
                                let close_y = tab_y + 6;
                                let close_size = 20;

                                if mx >= close_x && mx < close_x + close_size && my >= close_y && my < close_y + close_size {
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

                    if needs_redraw {
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                    return;
                } else if self.sidebar_visible {
                    if self.hovered_tab.is_some() { self.hovered_tab = None; needs_redraw = true; }
                    if self.hovered_close.is_some() { self.hovered_close = None; needs_redraw = true; }
                    if self.hovered_new_tab { self.hovered_new_tab = false; needs_redraw = true; }
                    if needs_redraw {
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                } else if needs_redraw {
                    if let Some(w) = &self.window { w.request_redraw(); }
                }

                // 2. Coordenadas relativas a paneles en la terminal activa
                let bypass = self.modifiers.shift_key();
                let active_tab = &mut self.tabs[self.active_tab_index];
                let active_pane_id = active_tab.active_pane_id;
                let term_area_w = win_w.saturating_sub(offset);
                let pane_rects = active_tab.compute_pane_rects(offset, 0, term_area_w, win_h);

                let hovered_pane_rect = pane_rects.iter().find(|r| {
                    mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height
                }).copied();

                if let Some(rect) = hovered_pane_rect {
                    if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                        if pane.terminal.is_mouse_reporting_active() && !bypass {
                            if (pane.terminal.mouse_reporting_button_event && self.mouse_down) || pane.terminal.mouse_reporting_any_event {
                                if let Some(renderer) = &self.renderer {
                                    let term_x = mx.saturating_sub(rect.x);
                                    let term_y = my.saturating_sub(rect.y);
                                    let col = if term_x >= renderer.margin_x {
                                        ((term_x - renderer.margin_x) / renderer.char_width) + 1
                                    } else { 1 }.min(pane.terminal.cols);
                                    let row = if term_y >= renderer.margin_y {
                                        ((term_y - renderer.margin_y) / renderer.row_height) + 1
                                    } else { 1 }.min(pane.terminal.rows);

                                    let mut code = 32;
                                    if self.modifiers.shift_key() { code += 4; }
                                    if self.modifiers.alt_key() { code += 8; }
                                    if self.modifiers.control_key() { code += 16; }

                                    if pane.terminal.mouse_reporting_sgr {
                                        let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                        pane.pty.write_input(seq.as_bytes());
                                    } else {
                                        let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
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
                                        if let Some(w) = &self.window { w.request_redraw(); }
                                    }
                                }
                            }
                        }
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

                // Si soltamos el botón izquierdo, cancelar arrastre de la barra de desplazamiento
                if button == MouseButton::Left && state == ElementState::Released {
                    if self.scrollbar_dragging {
                        self.scrollbar_dragging = false;
                        self.scrollbar_drag_pane_id = None;
                        self.last_scroll_time = Instant::now();
                        if let Some(w) = &self.window { w.request_redraw(); }
                        return;
                    }
                }

                // Comprobar clic en botón inferior de alternar barra lateral (en ambas modalidades)
                let toggle_btn_w = if self.sidebar_visible { SIDEBAR_WIDTH.saturating_sub(16) } else { 32 };
                let toggle_btn_h = 30;
                let toggle_btn_x = 8;
                let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8);

                let is_click_toggle = mx >= toggle_btn_x
                    && mx < toggle_btn_x + toggle_btn_w
                    && my >= toggle_btn_y
                    && my < toggle_btn_y + toggle_btn_h;

                if is_click_toggle && state == ElementState::Pressed && button == MouseButton::Left {
                    crate::logger::log("-> Clic en botón inferior de alternar barra lateral");
                    self.toggle_sidebar();
                    return;
                }

                // 1. Clics en la barra lateral abierta
                if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                    if state == ElementState::Pressed && button == MouseButton::Left {
                        // Clic en botón "+"
                        let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                        let plus_btn_y = 8;
                        let plus_btn_w = 24;
                        let plus_btn_h = 24;
                        if mx >= plus_btn_x && mx < plus_btn_x + plus_btn_w && my >= plus_btn_y && my < plus_btn_y + plus_btn_h {
                            crate::logger::log("-> Clic en [+] sidebar: nueva terminal");
                            self.create_new_tab();
                            return;
                        }

                        // Clic en pestañas o botón cerrar "x"
                        let tab_item_h = 34;
                        let tab_spacing = 4;
                        let list_start_y = 42;
                        let tab_x = 8;
                        let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                        let list_max_y = win_h.saturating_sub(44);

                        if my >= list_start_y && my < list_max_y {
                            let rel_y = my - list_start_y;
                            let item_total = tab_item_h + tab_spacing;
                            let tab_idx = rel_y / item_total;
                            let in_item = (rel_y % item_total) < tab_item_h;

                            if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w {
                                let tab_y = list_start_y + tab_idx * item_total;
                                let close_x = tab_x + tab_w.saturating_sub(22);
                                let close_y = tab_y + 6;
                                let close_size = 20;

                                if self.tabs.len() > 1 && mx >= close_x && mx < close_x + close_size && my >= close_y && my < close_y + close_size {
                                    crate::logger::log(&format!("-> Clic en [x] pestaña {}", tab_idx));
                                    self.close_tab(tab_idx, event_loop);
                                } else {
                                    crate::logger::log(&format!("-> Clic en pestaña {}", tab_idx));
                                    self.switch_to_tab(tab_idx);
                                }
                                return;
                            }
                        }
                    }
                    return;
                }

                // 2. Clics en paneles de terminal activa
                let bypass = self.modifiers.shift_key();
                let offset = self.sidebar_offset();
                let term_area_w = win_w.saturating_sub(offset);
                let active_tab = &mut self.tabs[self.active_tab_index];
                let pane_rects = active_tab.compute_pane_rects(offset, 0, term_area_w, win_h);

                let clicked_pane_rect = pane_rects.iter().find(|r| {
                    mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height
                }).copied();

                if let Some(rect) = clicked_pane_rect {
                    // Cambiar foco al panel clickeado si no está activo
                    if active_tab.active_pane_id != rect.pane_id {
                        active_tab.active_pane_id = rect.pane_id;
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }

                    if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                        // Clic en scrollbar de este panel
                        if let Some(renderer) = &self.renderer {
                            if let Some((_thumb_x, thumb_y, _thumb_w, thumb_h)) = renderer.pane_scrollbar_geometry(rect.x, rect.y, rect.width, rect.height, &pane.terminal) {
                                let scroll_margin = 16;
                                if mx >= (rect.x + rect.width).saturating_sub(scroll_margin) && button == MouseButton::Left && state == ElementState::Pressed {
                                    if my >= thumb_y && my < thumb_y + thumb_h {
                                        self.scrollbar_dragging = true;
                                        self.scrollbar_drag_pane_id = Some(pane.id);
                                        self.scrollbar_drag_offset_y = (my - thumb_y) as f32;
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window { w.request_redraw(); }
                                        return;
                                    } else if my < thumb_y {
                                        pane.terminal.scroll_up(pane.terminal.rows);
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window { w.request_redraw(); }
                                        return;
                                    } else {
                                        pane.terminal.scroll_down(pane.terminal.rows);
                                        self.last_scroll_time = Instant::now();
                                        if let Some(w) = &self.window { w.request_redraw(); }
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
                                } else { 1 }.min(pane.terminal.cols);
                                let row = if term_y >= renderer.margin_y {
                                    ((term_y - renderer.margin_y) / renderer.row_height) + 1
                                } else { 1 }.min(pane.terminal.rows);

                                let base_code = match button {
                                    MouseButton::Left => 0,
                                    MouseButton::Middle => 1,
                                    MouseButton::Right => 2,
                                    _ => 0,
                                };

                                let mut code = base_code;
                                if self.modifiers.shift_key() { code += 4; }
                                if self.modifiers.alt_key() { code += 8; }
                                if self.modifiers.control_key() { code += 16; }

                                let is_pressed = state == ElementState::Pressed;
                                self.mouse_down = is_pressed && button == MouseButton::Left;

                                if pane.terminal.mouse_reporting_sgr {
                                    let suffix = if is_pressed { 'M' } else { 'm' };
                                    let seq = format!("\x1b[<{};{};{}{}", code, col, row, suffix);
                                    pane.pty.write_input(seq.as_bytes());
                                } else {
                                    let btn_byte = if is_pressed { 32 + code as u8 } else { 32 + 3 };
                                    let seq = [0x1b, b'[', b'M', btn_byte, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                                    pane.pty.write_input(&seq);
                                }
                            }
                        } else {
                            match button {
                                MouseButton::Left => {
                                    match state {
                                        ElementState::Pressed => {
                                            self.mouse_down = true;
                                            if let Some(renderer) = &self.renderer {
                                                let term_x = mx.saturating_sub(rect.x);
                                                let term_y = my.saturating_sub(rect.y);
                                                if term_x >= renderer.margin_x && term_y >= renderer.margin_y {
                                                    let col = (term_x - renderer.margin_x) / renderer.char_width;
                                                    let row = (term_y - renderer.margin_y) / renderer.row_height;
                                                    self.drag_start = Some((col, row));
                                                    pane.terminal.selection = None;
                                                    if let Some(w) = &self.window { w.request_redraw(); }
                                                }
                                            }
                                        }
                                        ElementState::Released => {
                                            self.mouse_down = false;
                                            self.drag_start = None;
                                        }
                                    }
                                }
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

                let active_tab = &mut self.tabs[self.active_tab_index];
                let term_area_w = win_w.saturating_sub(offset);
                let pane_rects = active_tab.compute_pane_rects(offset, 0, term_area_w, win_h);

                let target_pane_rect = pane_rects.iter().find(|r| {
                    mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height
                }).copied();

                let target_pane_id = target_pane_rect.map(|r| r.pane_id).unwrap_or(active_tab.active_pane_id);

                if let Some(pane) = active_tab.find_pane_mut(target_pane_id) {
                    let rect_x = target_pane_rect.map(|r| r.x).unwrap_or(offset);
                    let rect_y = target_pane_rect.map(|r| r.y).unwrap_or(0);

                    if pane.terminal.is_mouse_reporting_active() && !bypass {
                        if let Some(renderer) = &self.renderer {
                            let term_x = mx.saturating_sub(rect_x);
                            let term_y = my.saturating_sub(rect_y);
                            let col = if term_x >= renderer.margin_x {
                                ((term_x - renderer.margin_x) / renderer.char_width) + 1
                            } else { 1 }.min(pane.terminal.cols);
                            let row = if term_y >= renderer.margin_y {
                                ((term_y - renderer.margin_y) / renderer.row_height) + 1
                            } else { 1 }.min(pane.terminal.rows);

                            let is_up = match delta {
                                MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                                MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                            };

                            let mut code = if is_up { 64 } else { 65 };
                            if self.modifiers.shift_key() { code += 4; }
                            if self.modifiers.alt_key() { code += 8; }
                            if self.modifiers.control_key() { code += 16; }

                            if pane.terminal.mouse_reporting_sgr {
                                let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                pane.pty.write_input(seq.as_bytes());
                            } else {
                                let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                                pane.pty.write_input(&seq);
                            }
                        }
                    } else {
                        let lines = match delta {
                            MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1) * 3,
                            MouseScrollDelta::PixelDelta(pos) => ((pos.y.abs() / 20.0).ceil() as usize).max(1) * 3,
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
                        if let Some(w) = &self.window { w.request_redraw(); }
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
            crate::logger::log(&format!("Proceso hijo de pane {} terminado: cerrando pane", pane_id));
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

        // 4. Temporizador de parpadeo de cursor cuando la terminal está inactiva
        if self.config.terminal.cursor_blink {
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
            let in_arrow_top = x >= 9 && x <= 16 && (y as i32 - x as i32).abs() <= 1 && y >= 9 && y <= 16;
            let in_arrow_bot = x >= 9 && x <= 16 && (y as i32 + x as i32 - 32).abs() <= 1 && y >= 16 && y <= 23;
            let in_cursor = x >= 18 && x <= 24 && (y == 21 || y == 22);

            if in_arrow_top || in_arrow_bot || in_cursor {
                rgba[idx] = 0x51;     // Verde Ghostty #51e27a
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
fn enable_windows_dark_mode(window: &Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                // 1. Dark Mode inmersivo
                let dark_mode: i32 = 1;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    20, // DWMWA_USE_IMMERSIVE_DARK_MODE
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                // 2. Color de fondo del titlebar idéntico a Ghostty (#161618 -> BGR: 0x00181616)
                let caption_color: u32 = 0x00181616;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    35, // DWMWA_CAPTION_COLOR
                    &caption_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 3. Color del texto del titlebar Ghostty (#e6e6eb -> BGR: 0x00ebe6e6)
                let text_color: u32 = 0x00ebe6e6;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    36, // DWMWA_TEXT_COLOR
                    &text_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 4. Color del borde sutil de la ventana (#28282c -> BGR: 0x002c2828)
                let border_color: u32 = 0x002c2828;
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

                // 6. Efecto de fondo Mica nativo de Windows 11 (estilo Ghostty)
                let backdrop_type: u32 = 2; // 2 = DWMSBT_MAINWINDOW (Mica)
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    38, // DWMWA_SYSTEMBACKDROP_TYPE
                    &backdrop_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 7. Establecer el pincel de fondo de la clase Win32 al color oscuro de Ghostty (#161618)
                // Esto previene que Windows pinte el fondo blanco por defecto (COLOR_WINDOW / WHITE_BRUSH)
                // al crear la ventana o al procesar eventos WM_ERASEBKGND durante redimensionado.
                let dark_brush = windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(0x00181616);
                if dark_brush != 0 {
                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::GCLP_HBRBACKGROUND,
                        dark_brush as isize,
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_physical_modifiers() -> (bool, bool, bool, bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_RCONTROL, VK_RMENU, VK_SHIFT,
    };

    unsafe {
        let is_down = |vk: u16| -> bool {
            (GetAsyncKeyState(vk as i32) as i16) < 0
        };

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

fn key_is_char(key: &Key, target: char) -> bool {
    match key {
        Key::Character(s) => {
            if let Some(c) = s.chars().next() {
                if c.to_ascii_lowercase() == target.to_ascii_lowercase() {
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
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
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
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
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
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
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



