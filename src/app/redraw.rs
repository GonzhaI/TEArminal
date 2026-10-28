//! Repintado de un fotograma completo.

use super::*;
use crate::renderer::{Canvas, Presented, SidebarRects};

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
    hovered_pane_btn: Option<(PaneId, PaneHeaderButton)>,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    hovered_ai_badge: bool,
    sidebar_scroll_offset: usize,
    sidebar_scrollbar_hovered: bool,
    palette_scrollbar_hovered: bool,
    active_idx: usize,
    tab_items: &'a [SidebarTabItem],
    header: &'a SidebarHeader,
    panel_right: bool,
    panel_width: usize,
}

impl TearminalApp {
    /// Dibuja un fotograma completo (`WindowEvent::RedrawRequested`).
    pub(super) fn on_redraw(&mut self) {
        if self.occluded {
            return;
        }
        // El explorador sigue a la terminal enfocada: un solo punto cubre clic, atajo, división y cierre
        // de paneles, que cambian el foco sin pasar por `switch_to_tab`.
        let focus = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| (t.id, t.active_pane_id));
        if focus != self.explorer_focus {
            self.explorer_focus = focus;
            self.sync_file_explorer_root();
        }
        let is_horizontal = self.is_horizontal_tabs();
        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let explorer_visible = self.file_explorer.visible;
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let sidebar_visible = self.panel_shown();
        let active_idx = self.active_tab_index;
        let hovered_tab = self.hovered_tab;
        let hovered_close = self.hovered_close;
        let hovered_new_tab = self.hovered_new_tab;
        let hovered_files_btn = self.hovered_files_btn;
        let hovered_settings_btn = self.hovered_settings_btn;
        let hovered_toggle_btn = self.hovered_toggle_btn;
        let cursor_visible = !self.config.terminal.cursor_blink || self.cursor_on;
        let panel_right = self.panel_on_right();
        let panel_width = self.panel_width();

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

        let (win_w, win_h) = self.window.as_ref().map_or((0, 0), |w| {
            let size = w.inner_size();
            (size.width as usize, size.height as usize)
        });
        self.sync_explorer_dock(win_w, win_h);
        let header = if !sidebar_visible {
            SidebarHeader::default()
        } else {
            self.sidebar_header(win_h.saturating_sub(status_bar_h))
        };
        let tab_items: Vec<SidebarTabItem> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(idx, t)| SidebarTabItem {
                index: idx,
                status: super::panel::session_status(t),
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

        let palette_scrollbar_hovered = self.palette.as_ref().is_some_and(|p| {
            let rects = PaletteRects::compute_with_scroll(
                size.width as usize,
                size.height as usize,
                p.filtered.len(),
                p.scroll_offset,
            );
            rects.scrollbar.is_some_and(|(track, _)| {
                crate::renderer::ScrollbarRects::hit(
                    track,
                    rects.card.0 + rects.card.2,
                    self.mouse_pos.x as usize,
                    self.mouse_pos.y as usize,
                )
            })
        });

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
            palette_scrollbar_hovered,
            active_idx,
            tab_items: &tab_items,
            header: &header,
            panel_right,
            panel_width,
        };

        self.canvas.begin(size.width, size.height);
        draw_full_scene(
            &mut self.canvas,
            renderer,
            &mut self.tabs,
            &self.theme,
            &self.config,
            &self.file_explorer,
            &self.settings_page_state,
            &self.ai_usage.snapshot,
            &self.context_menu,
            &self.palette,
            &params,
            size.width as usize,
            size.height as usize,
            is_maximized,
        );
        let Some(gpu) = &mut self.gpu_surface else {
            return;
        };
        gpu.set_render_tuning(1.0, false);
        match gpu.present(self.canvas.scene()) {
            Ok(Presented::Ok) => {}
            Ok(Presented::Recovered) => {
                crate::logger::log("GPU: dispositivo recuperado; se repinta la escena");
                window.request_redraw();
            }
            Err(e) => {
                // Un fallo persistente no debe inundar el registro: una línea cada 5 s.
                let due = self
                    .last_present_error_log
                    .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(5));
                if due {
                    self.last_present_error_log = Some(Instant::now());
                    crate::logger::log(&format!("GPU: error al presentar: {e}"));
                }
            }
        }
        self.update_ime_cursor_area();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_full_scene(
    canvas: &mut Canvas,
    renderer: &mut TerminalRenderer,
    tabs: &mut [TerminalTab],
    theme: &Theme,
    config: &TearminalConfig,
    file_explorer: &crate::explorer::FileExplorer,
    settings_page_state: &SettingsPageState,
    ai_snapshot: &ai_usage::AiUsageSnapshot,
    context_menu: &Option<ContextMenu>,
    palette: &Option<PaletteState>,
    p: &SceneParams,
    width: usize,
    height: usize,
    is_maximized: bool,
) {
    canvas.clear(theme.background);

    let active_tab = &mut tabs[p.active_idx];

    if p.is_horizontal {
        renderer.render_top_tab_bar(
            canvas,
            p.tab_items,
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
            canvas,
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
        draw_panel(canvas, renderer, theme, p, width, height);
    }

    if active_tab.is_settings {
        renderer.render_settings_page(
            canvas,
            p.offset,
            if p.panel_right && p.sidebar_visible {
                p.panel_width
            } else {
                0
            },
            p.top_offset,
            p.status_bar_h,
            config,
            settings_page_state,
            p.cursor_visible,
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
            canvas,
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
                canvas,
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
        let focused_pane_id = active_tab.active_pane_id;
        let zoomed = active_tab.zoomed_pane;
        let tab_title = active_tab.title.clone();

        for rect in &pane_rects {
            if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                let is_active = rect.pane_id == focused_pane_id;
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
                        canvas,
                        (rect.x, rect.slot_y, rect.width),
                        &pane.header_title(&tab_title),
                        pane.header_subtitle().as_deref(),
                        icon,
                        icon_color,
                        is_active,
                        zoomed == Some(rect.pane_id),
                        hovered,
                        theme,
                    );
                }
                renderer.render_pane(
                    canvas,
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
            TerminalRenderer::render_pane_divider(canvas, dx, dy, dw, dh, divider_color);
        }

        if file_explorer.visible {
            renderer.render_file_explorer(
                canvas,
                p.top_offset,
                p.status_bar_h,
                file_explorer,
                theme,
            );
        }
    }

    // Tarjetas del panel y del contenido: esquinas redondeadas y contorno sobre el tono de las barras. Van
    // después del explorador alojado, que pinta hasta el borde del panel.
    let cards = card_regions(
        width,
        height,
        p.status_bar_h,
        p.offset,
        p.right_offset,
        p.top_offset,
        p.sidebar_visible,
        p.panel_right,
    );
    for region in cards.panel.into_iter().chain(Some(cards.content)) {
        canvas.outlined_card(
            region,
            SidebarRects::CARD_INSET,
            SidebarRects::CARD_RADIUS,
            theme.bar_bg(),
            theme.card_outline(),
        );
    }

    if !p.is_horizontal && !p.sidebar_visible && p.hovered_toggle_btn {
        renderer.render_floating_sidebar_toggle(
            canvas,
            p.status_bar_h,
            p.hovered_toggle_btn,
            p.panel_right,
            theme,
        );
    }

    if config.appearance.show_status_bar && !active_tab.is_settings {
        let (cwd, git_branch, ssh_host, cols, rows, ai_status) =
            if let Some(ref viewer) = active_tab.file_viewer {
                let path_str = shell_context::shorten_home(&viewer.path).into_owned();
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
                    .map(|d| shell_context::shorten_home(d).into_owned());
                let g_branch = pane.get_git_branch();
                let s_host = pane
                    .terminal
                    .ssh_session
                    .as_ref()
                    .filter(|s| s.is_active)
                    .map(|s| s.destination.clone());
                let ai = pane.terminal.active_ai_agent();
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
            canvas,
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
    }

    if let Some(menu) = context_menu {
        renderer.render_context_menu(canvas, menu, theme);
    }

    if let Some(palette) = palette {
        renderer.render_palette(
            canvas,
            palette,
            p.cursor_visible,
            p.palette_scrollbar_hovered,
            theme,
        );
    }
}

/// Dibuja el panel lateral. A la izquierda lo hace sobre el fotograma; a la derecha, en un búfer propio
/// del ancho del panel que se copia a su sitio (la geometría del panel es local a él).
type Region = (usize, usize, usize, usize);

/// Regiones `(x, y, ancho, alto)` de las tarjetas, en coordenadas de ventana.
pub(super) struct CardRegions {
    /// Panel lateral (con cualquier disposición de pestañas y a cualquier lado), si está a la vista.
    pub(super) panel: Option<Region>,
    /// Contenido (terminales, visor, Ajustes). Con el panel a la vista se estira bajo el margen de la tarjeta
    /// vecina, de modo que entre las dos queda un solo margen y no dos.
    pub(super) content: Region,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn card_regions(
    width: usize,
    height: usize,
    status_bar_h: usize,
    offset: usize,
    right_offset: usize,
    top_offset: usize,
    panel_visible: bool,
    panel_right: bool,
) -> CardRegions {
    let usable = height.saturating_sub(status_bar_h);
    let top = SidebarRects::TOP;
    let panel_w = if panel_right { right_offset } else { offset };
    let panel_w = if panel_w > 0 { panel_w } else { SIDEBAR_WIDTH };
    let panel = panel_visible.then(|| {
        let x0 = if panel_right {
            width.saturating_sub(panel_w)
        } else {
            0
        };
        (x0, top, panel_w, usable.saturating_sub(top))
    });
    let stretch = if panel_visible {
        SidebarRects::CARD_INSET
    } else {
        0
    };
    let left = if offset > 0 {
        offset - stretch.min(offset)
    } else {
        0
    };
    let right = width.saturating_sub(right_offset) + if right_offset > 0 { stretch } else { 0 };
    CardRegions {
        panel,
        content: (
            left,
            top_offset,
            right.min(width).saturating_sub(left),
            usable.saturating_sub(top_offset),
        ),
    }
}

fn draw_panel(
    canvas: &mut Canvas,
    renderer: &mut TerminalRenderer,
    theme: &Theme,
    p: &SceneParams,
    width: usize,
    height: usize,
) {
    let panel_w = p.panel_width;
    macro_rules! render {
        ($canvas:expr) => {
            if p.is_horizontal {
                renderer.render_files_panel(
                    $canvas,
                    panel_w,
                    p.status_bar_h,
                    p.header.on_right,
                    theme,
                )
            } else {
                renderer.render_sidebar(
                    $canvas,
                    panel_w,
                    p.status_bar_h,
                    p.header,
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
        render!(canvas);
        return;
    }
    let x0 = width.saturating_sub(panel_w);
    let usable = height.saturating_sub(p.status_bar_h);
    let top = SidebarRects::TOP;
    canvas.fill_rect(
        (0, 0, width, height),
        x0,
        top,
        panel_w,
        usable.saturating_sub(top),
        theme.background,
    );
    canvas.with_offset(
        x0 as i32,
        0,
        (panel_w, height),
        (x0, top, x0 + panel_w, usable),
        |c| render!(c),
    );
}
