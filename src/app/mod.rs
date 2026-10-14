//! Aplicación: estado global (`AxonApp`), pestañas/paneles y bucle de eventos de winit.
//!
//! `AxonApp` reparte sus métodos por responsabilidad en submódulos (`tabs`, `settings`, `clipboard`, ...);
//! los eventos de winit entran por `handler` y se despachan a `redraw`, `keyboard`, `cursor`, `click` y `wheel`.

mod click;
mod clipboard;
mod cursor;
mod handler;
mod keyboard;
mod keys;
mod layout;
mod platform;
mod redraw;
mod settings;
mod shell_context;
mod tabs;
mod wheel;

pub use keys::{key_code_to_char, key_is_char};
pub use layout::*;
pub(crate) use shell_context::detect_ai_agent;

use platform::{
    apply_window_opacity_native, create_native_icon, get_physical_modifiers, update_windows_theme,
};
use shell_context::get_git_branch;

use crate::config::AxonConfig;
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
    sidebar_scrollbar_dragging: bool,
    sidebar_scrollbar_drag_offset_y: f32,
    explorer_scrollbar_dragging: bool,
    explorer_scrollbar_drag_offset_y: f32,
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
            .unwrap_or_else(|e| crate::pty::fatal_spawn_error(&config.terminal.default_shell, &e));

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
            .unwrap_or_else(|e| crate::pty::fatal_spawn_error(&config.terminal.default_shell, &e));
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
            sidebar_scrollbar_dragging: false,
            sidebar_scrollbar_drag_offset_y: 0.0,
            explorer_scrollbar_dragging: false,
            explorer_scrollbar_drag_offset_y: 0.0,
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
}

#[cfg(test)]
mod tests;
