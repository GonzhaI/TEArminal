//! Repintado de un fotograma completo.

use super::*;
use crate::renderer::{Canvas, Presented, SidebarRects};

#[allow(clippy::too_many_arguments)]
struct SceneParams<'a> {
    spaces_collapsed: bool,
    spaces_hover: crate::renderer::SpacesHover,
    spaces_scroll: usize,
    space_items: &'a [crate::renderer::SpaceItem],
    hovered_changes_btn: bool,
    changes_active: bool,
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
    hovered_pane_btn: Option<(PaneId, PaneHeaderButton)>,
    hovered_pane_header: Option<PaneId>,
    hovered_url_span: Option<(PaneId, usize, usize, usize)>,
    hovered_win_min: bool,
    hovered_win_max: bool,
    hovered_win_close: bool,
    hovered_ai_badge: bool,
    hovered_keep_awake: bool,
    keep_awake_popup_open: bool,
    hovered_keep_awake_option: Option<usize>,
    keep_awake: KeepAwakeMode,
    keep_awake_working: bool,
    palette_scrollbar_hovered: bool,
    active_idx: usize,
    /// Pestaña cuyo contenido se dibuja: la activa, o con Ajustes abierto la terminal de detrás
    content_idx: usize,
    tab_items: &'a [SidebarTabItem],
    panel_right: bool,
    panel_width: usize,
    tab_anim_rect: Option<(f32, f32)>,
    palette_anim_progress: f32,
    settings_modal_anim_progress: f32,
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
        // Perder el foco (clic fuera, cambio de pestaña) cancela el nombre que se estaba editando
        if !self.file_explorer.is_focused {
            self.file_explorer.cancel_edit();
        }
        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let explorer_visible = self.file_explorer.visible;
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let sidebar_visible = self.panel_shown();
        let active_idx = self.active_tab_index;
        let content_idx = if self.tabs[active_idx].is_settings {
            let ok = |i: usize| self.tabs.get(i).is_some_and(|t| !t.is_settings);
            if ok(self.last_terminal_tab_index) {
                self.last_terminal_tab_index
            } else {
                self.tabs
                    .iter()
                    .position(|t| !t.is_settings)
                    .unwrap_or(active_idx)
            }
        } else {
            active_idx
        };
        let hovered_tab = self.hovered_tab;
        let hovered_close = self.hovered_close;
        let hovered_new_tab = self.hovered_new_tab;
        let hovered_files_btn = self.hovered_files_btn;
        let space_items = self.space_items();
        let visible_tabs = self.visible_tab_indices();
        let cursor_visible = !self.config.terminal.cursor_blink || self.cursor_on;
        let panel_right = self.panel_on_right();
        let panel_width = self.panel_width();

        // Editar una vista previa la fija (si no, la siguiente vista previa borraría el trabajo)
        for t in &mut self.tabs {
            if t.is_preview && t.file_viewer.as_ref().is_some_and(|v| v.is_dirty) {
                t.is_preview = false;
            }
        }

        let (win_w, win_h) = self.window.as_ref().map_or((0, 0), |w| {
            let size = w.inner_size();
            (size.width as usize, size.height as usize)
        });
        self.sync_explorer_dock(win_w, win_h);
        let tab_items: Vec<SidebarTabItem> = visible_tabs
            .iter()
            .map(|&idx| (idx, &self.tabs[idx]))
            .map(|(idx, t)| SidebarTabItem {
                index: idx,
                status: t.session_status(),
                title: t.display_title(),
                detail: None,
                is_active: idx == content_idx,
                has_unread: t.has_unread,
                pane_count: t.panes.len(),
                is_settings: t.is_settings,
                file_path: t.file_viewer.as_ref().map(|v| v.path.clone()),
                is_preview: t.is_preview,
            })
            .collect();

        let keep_awake_working = self.has_working_agent();

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
            spaces_collapsed: self.spaces_collapsed,
            spaces_hover: self.spaces_hover,
            spaces_scroll: self.spaces_scroll,
            space_items: &space_items,
            hovered_changes_btn: self.hovered_changes_btn,
            changes_active: self.file_explorer.changes_only,
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
            hovered_pane_btn: self.hovered_pane_btn,
            hovered_pane_header: self.hovered_pane_header,
            hovered_url_span: self.hovered_url_span,
            hovered_win_min: self.hovered_win_min,
            hovered_win_max: self.hovered_win_max,
            hovered_win_close: self.hovered_win_close,
            hovered_ai_badge: self.hovered_ai_badge,
            hovered_keep_awake: self.hovered_keep_awake,
            keep_awake_popup_open: self.keep_awake_popup_open,
            hovered_keep_awake_option: self.hovered_keep_awake_option,
            keep_awake: self.config.terminal.keep_awake,
            keep_awake_working,
            palette_scrollbar_hovered,
            active_idx,
            content_idx,
            tab_items: &tab_items,
            panel_right,
            panel_width,
            tab_anim_rect: self.tab_anim_rect,
            palette_anim_progress: self.palette_anim_progress,
            settings_modal_anim_progress: self.settings_modal_anim_progress,
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
            &self.machine_modal,
            &params,
            size.width as usize,
            size.height as usize,
            is_maximized,
        );
        let Some(gpu) = &mut self.gpu_surface else {
            return;
        };
        gpu.set_render_tuning(1.0, false);
        gpu.set_vsync(self.config.terminal.vsync);
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
    machine: &Option<MachineModal>,
    p: &SceneParams,
    width: usize,
    height: usize,
    is_maximized: bool,
) {
    canvas.clear(theme.background);

    let settings_open = tabs[p.active_idx].is_settings;
    let active_tab = &mut tabs[p.content_idx];

    renderer.tab_indicator_visual = p.tab_anim_rect;
    renderer.render_top_tab_bar(
        canvas,
        p.tab_items,
        p.hovered_tab,
        p.hovered_close,
        p.hovered_new_tab,
        p.hovered_files_btn,
        p.hovered_changes_btn,
        p.hovered_win_min,
        p.hovered_win_max,
        p.hovered_win_close,
        is_maximized,
        p.explorer_visible,
        p.changes_active,
        p.offset,
        theme,
    );
    renderer.tab_indicator_visual = None;
    renderer.render_spaces_panel(
        canvas,
        p.space_items,
        p.spaces_collapsed,
        p.spaces_hover,
        p.spaces_scroll,
        p.cursor_visible,
        theme,
    );

    if p.sidebar_visible {
        draw_panel(canvas, renderer, theme, p, width, height);
    }

    if let Some(ref mut viewer) = active_tab.file_viewer {
        let term_area_w = width.saturating_sub(p.offset + p.right_offset);
        let term_area_h = height.saturating_sub(p.top_offset + p.status_bar_h);
        viewer.word_wrap = config.terminal.word_wrap;
        viewer.visible_cols = if viewer.is_side_by_side() {
            let total_rows = viewer.side_by_side_rows().len();
            let max_line = viewer
                .lines
                .len()
                .max(viewer.git_base.as_ref().map_or(0, |b| b.len()))
                .max(total_rows);
            crate::renderer::SideBySideLayout::compute(
                p.offset,
                p.top_offset,
                term_area_w,
                term_area_h,
                renderer.row_height,
                renderer.char_width,
                max_line,
            )
            .visible_cols
        } else {
            crate::renderer::FileViewerLayout::compute(
                p.offset,
                p.top_offset,
                term_area_w,
                term_area_h,
                renderer.row_height,
                renderer.char_width,
                viewer.lines.len(),
            )
            .visible_cols
        };

        renderer.scroll_visual_offset = Some(viewer.scroll_offset as f32);
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
        renderer.scroll_visual_offset = None;

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
                        rect.has_buttons,
                        p.hovered_pane_header == Some(rect.pane_id),
                        hovered,
                        theme,
                    );
                }
                renderer.hovered_url_span = p
                    .hovered_url_span
                    .filter(|(pane_id, _, _, _)| *pane_id == rect.pane_id)
                    .map(|(_, line, sc, ec)| (line, sc, ec));
                renderer.cursor_visual = pane.cursor_visual_pos;
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
                renderer.cursor_visual = None;
                renderer.hovered_url_span = None;
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
    let terminal_focused =
        !file_explorer.is_focused && !active_tab.is_settings && active_tab.file_viewer.is_none();
    let outline = |_focused: bool| theme.card_outline();
    if let Some(region) = cards.panel {
        canvas.outlined_card(
            region,
            SidebarRects::CARD_INSET,
            SidebarRects::CARD_RADIUS,
            theme.bar_bg(),
            outline(file_explorer.is_focused),
        );
    }
    canvas.outlined_card(
        cards.content,
        SidebarRects::CARD_INSET,
        SidebarRects::CARD_RADIUS,
        theme.bar_bg(),
        outline(terminal_focused),
    );

    if file_explorer.visible {
        renderer.render_file_explorer_hover_overlay(
            canvas,
            p.top_offset,
            p.status_bar_h,
            file_explorer,
            theme,
        );
    }

    if config.appearance.show_status_bar && !active_tab.is_settings {
        let (cwd, git_branch, ssh_host, cols, rows, ai_status) =
            if let Some(ref viewer) = active_tab.file_viewer {
                let path_str = shell_context::display_path(&viewer.path).into_owned();
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
                    .map(|d| shell_context::display_path(d).into_owned());
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
        renderer.render_status_bar_full(
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
            p.keep_awake,
            p.keep_awake_working,
            p.hovered_keep_awake,
            p.hovered_ai_badge,
            p.explorer_visible,
            theme,
        );
        if p.keep_awake_popup_open {
            renderer.render_keep_awake_popup(
                canvas,
                p.keep_awake,
                p.hovered_keep_awake_option,
                p.keep_awake_working,
                theme,
            );
        }
    }

    // Modales: un velo translúcido sobre toda la ventana y la tarjeta encima
    if settings_open || machine.is_some() {
        canvas.rounded_fill(
            0,
            0,
            width,
            height,
            0,
            crate::theme::Color::from_rgba(0, 0, 0, 120),
        );
    }
    if settings_open {
        let (mx0, my0, mw, mh) = crate::renderer::SettingsPageRects::modal_region(width, height);
        renderer.modal_anim_progress = Some(p.settings_modal_anim_progress);
        canvas.with_offset(
            mx0 as i32,
            my0 as i32,
            (mw, mh),
            (mx0, my0, mx0 + mw, my0 + mh),
            |c| {
                renderer.render_settings_page(
                    c,
                    0,
                    0,
                    0,
                    0,
                    config,
                    settings_page_state,
                    p.cursor_visible,
                    theme,
                )
            },
        );
        renderer.modal_anim_progress = None;
    }
    if let Some(m) = machine {
        let hosts: Vec<(String, String)> = m
            .hosts
            .iter()
            .map(|h| (h.alias.clone(), h.target.clone()))
            .collect();
        renderer.render_machine_modal(
            canvas,
            &hosts,
            &m.input,
            m.hovered,
            m.selected,
            m.error.as_deref(),
            p.cursor_visible,
            theme,
        );
    }

    if let Some(menu) = context_menu {
        renderer.render_context_menu(canvas, menu, theme);
    }

    if let Some(palette) = palette {
        renderer.palette_anim_progress = Some(p.palette_anim_progress);
        renderer.render_palette(
            canvas,
            palette,
            p.cursor_visible,
            p.palette_scrollbar_hovered,
            theme,
        );
        renderer.palette_anim_progress = None;
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
    // El panel de Spaces es marco plano (no tarjeta): el contenido arranca justo en su borde. A la derecha,
    // con el explorador a la vista, el contenido se estira bajo el margen de su tarjeta: un solo margen.
    let stretch = if panel_visible {
        SidebarRects::CARD_INSET
    } else {
        0
    };
    let left = offset;
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
        |c| renderer.render_files_panel(c, panel_w, p.status_bar_h, true, theme),
    );
}
