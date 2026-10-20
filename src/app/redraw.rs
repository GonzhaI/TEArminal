//! Repintado de un fotograma completo.

use super::*;
use crate::renderer::SidebarRects;

#[allow(clippy::too_many_arguments)]
struct SceneParams<'a> {
    is_horizontal: bool,
    offset: usize,
    right_offset: usize,
    top_offset: usize,
    status_bar_h: usize,
    sidebar_visible: bool,
    explorer_visible: bool,
    cursor_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_files_btn: bool,
    hovered_settings_btn: bool,
    hovered_toggle_btn: bool,
    hovered_pane_btn: Option<(usize, PaneHeaderButton)>,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    hovered_ai_badge: bool,
    sidebar_scroll_offset: usize,
    sidebar_scrollbar_hovered: bool,
    active_idx: usize,
    tab_items: &'a [SidebarTabItem],
    projects: &'a [SidebarProject],
    header: &'a SidebarHeader,
    hovered_project: Option<usize>,
    panel_right: bool,
    top_project: &'a TopBarProject,
    hovered_switcher: bool,
}

impl AxonApp {
    /// Dibuja un fotograma completo (`WindowEvent::RedrawRequested`).
    pub(super) fn on_redraw(&mut self) {
        if self.occluded {
            return;
        }
        let is_horizontal = self.is_horizontal_tabs();
        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let explorer_visible = self.file_explorer.visible;
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let sidebar_visible = self.sidebar_visible;
        let active_idx = self.active_tab_index;
        let hovered_tab = self.hovered_tab;
        let hovered_close = self.hovered_close;
        let hovered_new_tab = self.hovered_new_tab;
        let hovered_files_btn = self.hovered_files_btn;
        let hovered_settings_btn = self.hovered_settings_btn;
        let hovered_toggle_btn = self.hovered_toggle_btn;
        let cursor_visible = !self.config.terminal.cursor_blink || self.cursor_on;
        let panel_right = self.panel_on_right();
        let top_project = if is_horizontal {
            self.top_bar_project()
        } else {
            TopBarProject::default()
        };

        // Editar una vista previa la fija (si no, la siguiente vista previa borraría el trabajo)
        for t in &mut self.tabs {
            if t.is_preview
                && t.file_viewer
                    .as_ref()
                    .is_some_and(|v| v.is_dirty || !v.is_read_only)
            {
                t.is_preview = false;
            }
        }

        // En vertical el panel solo lista las sesiones del proyecto a la vista
        let panel_indices = self.panel_tab_indices();
        let (win_w, win_h) = self.window.as_ref().map_or((0, 0), |w| {
            let size = w.inner_size();
            (size.width as usize, size.height as usize)
        });
        self.sync_explorer_dock(win_w, win_h);
        let (projects, header) = if !sidebar_visible {
            (Vec::new(), SidebarHeader::default())
        } else {
            let usable_h = win_h.saturating_sub(status_bar_h);
            (self.sidebar_projects(), self.sidebar_header(usable_h))
        };
        let tab_items: Vec<SidebarTabItem> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(idx, _)| panel_indices.contains(idx))
            .map(|(idx, t)| SidebarTabItem {
                index: idx,
                status: super::projects::session_status(t),
                title: t.display_title(),
                detail: self.session_detail(t),
                is_active: idx == active_idx,
                has_unread: t.has_unread,
                pane_count: t.panes.len(),
                is_settings: t.is_settings,
                is_file_viewer: t.file_viewer.is_some(),
                is_preview: t.is_preview,
            })
            .collect();

        let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) else {
            return;
        };

        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        renderer.scrollbars_awake =
            self.last_scroll_time.elapsed() < std::time::Duration::from_millis(1250);
        let is_maximized = window.is_maximized();

        let params = SceneParams {
            is_horizontal,
            offset,
            right_offset,
            top_offset,
            status_bar_h,
            sidebar_visible,
            explorer_visible,
            cursor_visible,
            hovered_tab,
            hovered_close,
            hovered_new_tab,
            hovered_files_btn,
            hovered_settings_btn,
            hovered_toggle_btn,
            hovered_pane_btn: self.hovered_pane_btn,
            hovered_win_min: self.hovered_win_min,
            hovered_win_max: self.hovered_win_max,
            hovered_win_close: self.hovered_win_close,
            hovered_ai_badge: self.hovered_ai_badge,
            sidebar_scroll_offset: self.sidebar_scroll_offset,
            sidebar_scrollbar_hovered: self.sidebar_scrollbar_hovered,
            active_idx,
            tab_items: &tab_items,
            projects: &projects,
            header: &header,
            hovered_project: self.hovered_project,
            panel_right,
            top_project: &top_project,
            hovered_switcher: self.hovered_switcher,
        };

        // 1. Si la aceleración por hardware GPU (Direct3D 11 DirectFlip) está activa, presentar vía VRAM
        if let Some(d3d11) = &mut self.d3d11_presenter {
            let _ = d3d11.resize(size.width, size.height); // no-op si ya coincide
            let buffer = d3d11.buffer_mut();
            draw_full_scene(
                buffer,
                renderer,
                &mut self.tabs,
                &self.theme,
                &self.config,
                &self.file_explorer,
                &self.settings_page_state,
                &self.ai_usage.snapshot,
                &self.context_menu,
                &params,
                size.width as usize,
                size.height as usize,
                is_maximized,
            );
            let _ = d3d11.present();
            return;
        }

        // 2. Si no, utilizar la superficie de software (`softbuffer`) como fallback
        if let Some(surface) = &mut self.surface {
            if let Ok(mut buffer) = surface.buffer_mut() {
                draw_full_scene(
                    &mut buffer,
                    renderer,
                    &mut self.tabs,
                    &self.theme,
                    &self.config,
                    &self.file_explorer,
                    &self.settings_page_state,
                    &self.ai_usage.snapshot,
                    &self.context_menu,
                    &params,
                    size.width as usize,
                    size.height as usize,
                    is_maximized,
                );
                buffer.present().ok();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_full_scene(
    buffer: &mut [u32],
    renderer: &mut TerminalRenderer,
    tabs: &mut [TerminalTab],
    theme: &Theme,
    config: &AxonConfig,
    file_explorer: &crate::explorer::FileExplorer,
    settings_page_state: &SettingsPageState,
    ai_snapshot: &ai_usage::AiUsageSnapshot,
    context_menu: &Option<ContextMenu>,
    p: &SceneParams,
    width: usize,
    height: usize,
    is_maximized: bool,
) {
    buffer.fill(theme.background.to_u32_xrgb());

    let active_tab = &mut tabs[p.active_idx];

    if p.is_horizontal {
        renderer.render_top_tab_bar(
            buffer,
            width,
            height,
            p.tab_items,
            p.top_project,
            p.hovered_switcher,
            p.hovered_tab,
            p.hovered_close,
            p.hovered_new_tab,
            p.hovered_files_btn,
            p.hovered_settings_btn,
            p.hovered_win_min,
            p.hovered_win_max,
            p.hovered_win_close,
            is_maximized,
            p.explorer_visible,
            theme,
        );
    } else {
        let active_title = active_tab.display_title();
        let git_branch = active_tab.get_git_branch();
        renderer.render_top_title_bar(
            buffer,
            width,
            height,
            &active_title,
            git_branch.as_deref(),
            p.hovered_files_btn,
            p.hovered_settings_btn,
            p.hovered_win_min,
            p.hovered_win_max,
            p.hovered_win_close,
            is_maximized,
            p.explorer_visible,
            theme,
        );
    }

    if p.sidebar_visible {
        draw_panel(buffer, renderer, theme, p, width, height);
    }

    if active_tab.is_settings {
        renderer.render_settings_page(
            buffer,
            width,
            height,
            p.offset,
            if p.panel_right && p.sidebar_visible {
                SIDEBAR_WIDTH
            } else {
                0
            },
            p.top_offset,
            p.status_bar_h,
            config,
            settings_page_state,
            theme,
        );
    } else if let Some(ref mut viewer) = active_tab.file_viewer {
        let term_area_w = width.saturating_sub(p.offset + p.right_offset);
        let term_area_h = height.saturating_sub(p.top_offset + p.status_bar_h);
        viewer.word_wrap = config.terminal.word_wrap;
        viewer.visible_cols = crate::renderer::FileViewerLayout::compute(
            p.offset,
            p.top_offset,
            term_area_w,
            term_area_h,
            renderer.row_height,
            renderer.char_width,
            viewer.lines.len(),
        )
        .visible_cols;

        renderer.render_file_viewer(
            buffer,
            width,
            height,
            p.offset,
            p.top_offset,
            term_area_w + p.right_offset,
            term_area_h,
            p.right_offset,
            p.cursor_visible,
            viewer,
            theme,
        );

        if file_explorer.visible {
            renderer.render_file_explorer(
                buffer,
                width,
                height,
                p.top_offset,
                p.status_bar_h,
                file_explorer,
                theme,
            );
        }
    } else {
        let term_area_w = width.saturating_sub(p.offset + p.right_offset);
        let term_area_h = height.saturating_sub(p.top_offset + p.status_bar_h);
        let pane_rects =
            active_tab.compute_pane_rects(p.offset, p.top_offset, term_area_w, term_area_h);
        let dividers =
            active_tab.compute_dividers(p.offset, p.top_offset, term_area_w, term_area_h);
        let has_multiple = pane_rects.len() > 1;

        for rect in &pane_rects {
            if let Some(pane) = active_tab.find_pane(rect.pane_id) {
                let is_active = rect.pane_id == active_tab.active_pane_id;
                if rect.has_header {
                    let ssh_pane = pane
                        .terminal
                        .ssh_session
                        .as_ref()
                        .is_some_and(|s| s.is_active);
                    let (icon, icon_color) = if pane.is_agent_pane() {
                        (crate::ui::LucideIcon::Sparkles, theme.accent_color())
                    } else if ssh_pane {
                        (crate::ui::LucideIcon::Server, theme.text_secondary())
                    } else {
                        (crate::ui::LucideIcon::Terminal, theme.text_secondary())
                    };
                    let hovered = p
                        .hovered_pane_btn
                        .filter(|(pane_id, _)| *pane_id == rect.pane_id)
                        .map(|(_, button)| button);
                    renderer.render_pane_header(
                        buffer,
                        width,
                        height,
                        (rect.x, rect.slot_y, rect.width),
                        &pane.header_title(&active_tab.title),
                        pane.header_subtitle().as_deref(),
                        icon,
                        icon_color,
                        is_active,
                        active_tab.zoomed_pane == Some(rect.pane_id),
                        hovered,
                        theme,
                    );
                }
                renderer.render_pane(
                    buffer,
                    width,
                    height,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    &pane.terminal,
                    theme,
                    p.cursor_visible,
                    is_active,
                    has_multiple,
                );
            }
        }

        let divider_color = theme.border_color();
        for Divider {
            rect: (dx, dy, dw, dh),
            ..
        } in dividers
        {
            TerminalRenderer::render_pane_divider(
                buffer,
                width,
                height,
                dx,
                dy,
                dw,
                dh,
                divider_color,
            );
        }

        if file_explorer.visible {
            renderer.render_file_explorer(
                buffer,
                width,
                height,
                p.top_offset,
                p.status_bar_h,
                file_explorer,
                theme,
            );
        }
    }

    if !p.is_horizontal && !p.sidebar_visible && p.hovered_toggle_btn {
        renderer.render_floating_sidebar_toggle(
            buffer,
            width,
            height,
            p.status_bar_h,
            p.hovered_toggle_btn,
            p.panel_right,
            theme,
        );
    }

    if config.appearance.show_status_bar && !active_tab.is_settings {
        let (cwd, git_branch, ssh_host, cols, rows, ai_status) =
            if let Some(ref viewer) = active_tab.file_viewer {
                let path_str = shell_context::shorten_home(&viewer.path);
                let branch = viewer.remote_branch.clone();
                let ssh = viewer.remote_destination.clone();
                let lines_count = viewer.lines.len();
                (
                    Some(path_str),
                    branch,
                    ssh,
                    0,
                    lines_count,
                    AiAgentStatus::None,
                )
            } else if let Some(pane) = active_tab.active_pane_mut() {
                let c_dir = pane
                    .terminal
                    .current_dir
                    .as_deref()
                    .map(shell_context::shorten_home);
                let g_branch = pane.get_git_branch();
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

        let active_model = ai_snapshot.model_for(ai_status);
        let meters = TerminalRenderer::status_meters_for(ai_snapshot, ai_status);
        renderer.render_status_bar_with_ai(
            buffer,
            width,
            height,
            p.offset,
            cwd.as_deref(),
            git_branch.as_deref(),
            ssh_host.as_deref(),
            cols,
            rows,
            ai_status,
            active_model,
            &meters,
            p.hovered_ai_badge,
            p.explorer_visible,
            theme,
        );

        if p.hovered_ai_badge && context_menu.is_none() {
            renderer.render_ai_usage_popup(buffer, width, height, ai_snapshot, theme);
        }
    }

    if let Some(menu) = context_menu {
        renderer.render_context_menu(buffer, width, height, menu, theme);
    }
}

/// Dibuja el panel lateral. A la izquierda lo hace sobre el fotograma; a la derecha, en un búfer propio
/// del ancho del panel que se copia a su sitio (la geometría del panel es local a él).
fn draw_panel(
    buffer: &mut [u32],
    renderer: &mut TerminalRenderer,
    theme: &Theme,
    p: &SceneParams,
    width: usize,
    height: usize,
) {
    macro_rules! render {
        ($buf:expr, $w:expr) => {
            if p.is_horizontal {
                renderer.render_projects_panel(
                    $buf,
                    $w,
                    height,
                    SIDEBAR_WIDTH,
                    p.status_bar_h,
                    p.projects,
                    p.header,
                    p.hovered_project,
                    p.sidebar_scroll_offset,
                    p.hovered_toggle_btn,
                    theme,
                )
            } else {
                renderer.render_sidebar(
                    $buf,
                    $w,
                    height,
                    SIDEBAR_WIDTH,
                    p.status_bar_h,
                    p.projects,
                    p.header,
                    p.hovered_project,
                    p.tab_items,
                    p.sidebar_scroll_offset,
                    p.hovered_tab,
                    p.hovered_close,
                    p.hovered_new_tab,
                    p.hovered_toggle_btn,
                    p.sidebar_scrollbar_hovered,
                    theme,
                )
            }
        };
    }
    if !p.panel_right {
        render!(buffer, width);
        return;
    }
    let mut panel = std::mem::take(&mut renderer.panel_buf);
    panel.clear();
    panel.resize(SIDEBAR_WIDTH * height, theme.background.to_u32_xrgb());
    render!(&mut panel, SIDEBAR_WIDTH);
    let x0 = width.saturating_sub(SIDEBAR_WIDTH);
    let usable = height.saturating_sub(p.status_bar_h);
    for y in SidebarRects::TOP..usable {
        let src = &panel[y * SIDEBAR_WIDTH..(y + 1) * SIDEBAR_WIDTH];
        buffer[y * width + x0..y * width + x0 + SIDEBAR_WIDTH].copy_from_slice(src);
    }
    renderer.panel_buf = panel;
}
