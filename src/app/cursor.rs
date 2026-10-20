//! Movimiento del ratón: hover, arrastres, selección y reporte de ratón a TUIs.

use super::*;

impl AxonApp {
    /// Movimiento del ratón: hover, arrastres, selección y reporte de ratón a TUIs.
    pub(super) fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
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
                let pane_rects =
                    active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);
                if let Some(target_rect) = pane_rects.iter().find(|r| r.pane_id == pid) {
                    if let Some(pane) = active_tab.find_pane_mut(pid) {
                        if let Some(renderer) = &self.renderer {
                            if !pane.terminal.scrollback.is_empty() {
                                let track_y = target_rect.y + renderer.margin_y;
                                let track_h =
                                    target_rect.height.saturating_sub(renderer.margin_y * 2);
                                let total_history =
                                    pane.terminal.scrollback.len() + pane.terminal.rows;
                                let thumb_h = ((pane.terminal.rows as f32 / total_history as f32)
                                    * track_h as f32)
                                    .max(24.0)
                                    .min(track_h as f32)
                                    as usize;
                                let available_travel = track_h.saturating_sub(thumb_h);
                                if available_travel > 0 {
                                    let target_thumb_top =
                                        (my as f32 - self.scrollbar_drag_offset_y - track_y as f32)
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

        // Manejo de arrastre de la barra de desplazamiento de pestañas verticales (sidebar)
        if self.sidebar_scrollbar_dragging {
            let status_bar_h = self.status_bar_height();
            let usable_h = win_h.saturating_sub(status_bar_h);
            let list_len = self.panel_tab_indices().len();
            let list_end = self.panel_list_end(usable_h);
            if let Some((track, thumb)) = TerminalRenderer::sidebar_scrollbar_geometry(
                SIDEBAR_WIDTH,
                list_end,
                list_len,
                self.sidebar_scroll_offset,
            ) {
                let visible_items = crate::renderer::SidebarRects::visible_items(list_end);
                let max_offset = list_len.saturating_sub(visible_items);
                if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                    track,
                    thumb,
                    my as f32 - self.sidebar_scrollbar_drag_offset_y,
                    max_offset,
                ) {
                    self.sidebar_scroll_offset = off;
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Pointer);
            }
            return;
        }

        // Manejo de arrastre de la barra de desplazamiento del explorador de archivos
        if self.explorer_scrollbar_dragging {
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            if let Some((track, thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                win_w,
                win_h,
                top_offset,
                status_bar_h,
                &self.file_explorer,
            ) {
                let (_panel_x, panel_y, _panel_w, panel_h) =
                    self.file_explorer
                        .panel_geometry(win_w, win_h, top_offset, status_bar_h);
                let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
                let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
                let list_y = panel_y + header_h + sub_h + 4;
                let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                let list_max_y = panel_y + panel_h.saturating_sub(6);
                let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
                let max_offset = self.file_explorer.items.len().saturating_sub(visible_items);
                if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                    track,
                    thumb,
                    my as f32 - self.explorer_scrollbar_drag_offset_y,
                    max_offset,
                ) {
                    self.file_explorer.scroll_offset = off;
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Pointer);
            }
            return;
        }

        // Manejo de arrastre de la barra de desplazamiento del visor de archivos
        if self.viewer_scrollbar_dragging {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let right_offset = self.file_explorer_offset();
            let viewer_w = win_w.saturating_sub(offset + right_offset);
            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);

            if let Some(ref mut tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut viewer) = tab.file_viewer {
                    if let Some((track, thumb)) = TerminalRenderer::file_viewer_scrollbar_geometry(
                        offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                    ) {
                        let content_h = viewer_h.saturating_sub(38);
                        let visible_lines = (content_h / row_h.max(16)).max(1);
                        let max_offset = viewer.rows().len().saturating_sub(visible_lines);

                        if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                            track,
                            thumb,
                            my as f32 - self.viewer_scrollbar_drag_offset_y,
                            max_offset,
                        ) {
                            viewer.scroll_offset = off;
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
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

        // Arrastre de la barra de desplazamiento de Ajustes
        if self.settings_scrollbar_dragging {
            let top = (my as f32 - self.settings_scrollbar_drag_offset_y).max(0.0) as usize;
            self.settings_scroll_to_thumb(top);
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Pointer);
                w.request_redraw();
            }
            return;
        }

        // Arrastre de la barra de desplazamiento horizontal del visor de archivos
        if self.viewer_hscrollbar_dragging {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let viewer_w = win_w.saturating_sub(offset + self.file_explorer_offset());
            let viewer_h = win_h.saturating_sub(top_offset + self.status_bar_height());
            let (row_h, char_w) = self
                .renderer
                .as_ref()
                .map(|r| (r.row_height, r.char_width))
                .unwrap_or((20, 8));
            let grab = self.viewer_hscrollbar_drag_offset_x;
            if let Some(viewer) = self
                .tabs
                .get_mut(self.active_tab_index)
                .and_then(|t| t.file_viewer.as_mut())
            {
                let lay = crate::renderer::FileViewerLayout::compute(
                    offset,
                    top_offset,
                    viewer_w,
                    viewer_h,
                    row_h,
                    char_w,
                    viewer.lines.len(),
                );
                let max_x = viewer.max_scroll_x();
                if let Some((track, thumb)) = lay.hscrollbar(max_x, viewer.scroll_offset_x) {
                    let travel = track.2.saturating_sub(thumb.2);
                    if travel > 0 {
                        let thumb_left =
                            (mx as f32 - grab - track.0 as f32).clamp(0.0, travel as f32);
                        viewer.scroll_offset_x = ((thumb_left / travel as f32 * max_x as f32)
                            .round() as usize)
                            .min(max_x);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Pointer);
            }
            return;
        }

        // Arrastre del borde izquierdo del explorador: el PTY se redimensiona al soltar o al detenerse
        if self.explorer_resizing {
            self.file_explorer.set_width_from_edge(mx, win_w);
            self.schedule_pty_resize();
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::ColResize);
                w.request_redraw();
            }
            return;
        }

        // Arrastre de un divisor entre paneles
        if let Some(icon) = self.divider_drag.as_ref().map(Divider::cursor_icon) {
            self.drag_divider(mx, my);
            if let Some(w) = &self.window {
                w.set_cursor(icon);
            }
            return;
        }

        // Reordenar pestañas: la arrastrada salta a la posición bajo el ratón solo dentro de la barra
        if let Some(from) = self.tab_drag {
            let in_tab_bar = if self.is_horizontal_tabs() {
                my < TopTabBarRects::HEIGHT + 10 && mx < win_w
            } else {
                {
                    let x0 = self.panel_x(win_w);
                    mx + 10 >= x0 && mx < x0 + SIDEBAR_WIDTH + 10 && my < win_h
                }
            };
            if in_tab_bar {
                if let Some(to) = self.tab_slot_at(mx, my).filter(|&to| to != from) {
                    self.move_tab(from, to);
                }
            }
        }

        // Con el menú contextual abierto solo se resalta su opción bajo el ratón
        if let Some(menu) = &mut self.context_menu {
            let hovered = menu.item_at(mx, my);
            if menu.hovered != hovered {
                menu.hovered = hovered;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Default);
            }
            return;
        }

        let mut new_hovered_min = false;
        let mut new_hovered_max = false;
        let mut new_hovered_close_win = false;

        if self.is_horizontal_tabs() {
            let list = self.panel_tab_indices();
            let top_rects = TopTabBarRects::compute(win_w, list.len());
            let mut new_hovered_switcher = false;
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

                if TopTabBarRects::inside(mx, my, top_rects.switcher_btn) {
                    new_hovered_switcher = true;
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
                for (pos, &idx) in list.iter().enumerate() {
                    let tr = top_rects.tab_rect(pos);
                    if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                        new_hovered_tab = Some(idx);
                        is_pointer = true;
                        if list.len() > 1 {
                            let cr = top_rects.close_btn_rect(pos);
                            if mx >= cr.0 && mx < cr.0 + cr.2 && my >= cr.1 && my < cr.1 + cr.3 {
                                new_hovered_close = Some(idx);
                            }
                        }
                        break;
                    }
                }
            }

            if self.hovered_switcher != new_hovered_switcher {
                self.hovered_switcher = new_hovered_switcher;
                needs_redraw = true;
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
            let mut new_hovered_files = false;
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
                if TopTabBarRects::inside(mx, my, title_rects.files_btn) {
                    new_hovered_files = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, title_rects.settings_btn) {
                    new_hovered_settings = true;
                    is_pointer = true;
                }
            }
            if self.hovered_files_btn != new_hovered_files {
                self.hovered_files_btn = new_hovered_files;
                needs_redraw = true;
            }
            if self.hovered_settings_btn != new_hovered_settings {
                self.hovered_settings_btn = new_hovered_settings;
                needs_redraw = true;
            }
            // Comprobar hover en botón inferior de alternar barra lateral
            let status_bar_h = self.status_bar_height();
            let usable_h = win_h.saturating_sub(status_bar_h);
            let local = self.panel_local_x(mx, my, win_w);
            let list_end = self.panel_list_end(usable_h);
            let is_hover_toggle = if self.sidebar_visible {
                local.is_some_and(|lx| {
                    TopTabBarRects::inside(
                        lx,
                        my,
                        crate::renderer::SidebarRects::settings_btn_rect(usable_h),
                    )
                })
            } else {
                TopTabBarRects::inside(mx, my, self.floating_toggle_rect(win_w, usable_h))
            };

            if is_hover_toggle {
                is_pointer = true;
            }

            if self.hovered_toggle_btn != is_hover_toggle {
                self.hovered_toggle_btn = is_hover_toggle;
                needs_redraw = true;
            }

            if self.sidebar_visible {
                let is_over_sidebar_sb = if let Some((track, _)) =
                    TerminalRenderer::sidebar_scrollbar_geometry(
                        SIDEBAR_WIDTH,
                        list_end,
                        self.panel_tab_indices().len(),
                        self.sidebar_scroll_offset,
                    ) {
                    let lx = local.unwrap_or(usize::MAX);
                    lx >= track.0
                        && lx < track.0 + track.2
                        && my >= track.1
                        && my < track.1 + track.3
                } else {
                    false
                };
                if is_over_sidebar_sb {
                    is_pointer = true;
                }
                if self.sidebar_scrollbar_hovered != is_over_sidebar_sb {
                    self.sidebar_scrollbar_hovered = is_over_sidebar_sb;
                    needs_redraw = true;
                }
            }

            // 1. Detección de hover en la barra lateral si está visible
            if let (true, Some(lx)) = (self.sidebar_visible, local) {
                // Dentro del panel todas las coordenadas son locales a él
                let mx = lx;
                // Hover en botón "+"
                let is_new_tab = crate::renderer::SidebarRects::inside(
                    mx,
                    my,
                    crate::renderer::SidebarRects::plus_btn(),
                );
                if is_new_tab {
                    is_pointer = true;
                }
                if self.hovered_new_tab != is_new_tab {
                    self.hovered_new_tab = is_new_tab;
                    needs_redraw = true;
                }

                // Hover en el riel de proyectos
                let new_hovered_project = if mx < crate::renderer::SidebarRects::RAIL_W {
                    crate::renderer::SidebarRects::rail_at(mx, my, self.panel_rail().len() + 1)
                } else {
                    None
                };
                if new_hovered_project.is_some() {
                    is_pointer = true;
                }
                if self.hovered_project != new_hovered_project {
                    self.hovered_project = new_hovered_project;
                    needs_redraw = true;
                }

                // Hover en pestañas y botón de cerrar "x" (misma retícula que el dibujo y el clic)
                let mut new_hovered_tab = None;
                let mut new_hovered_close = None;

                if let Some((pos, tab_idx)) = self.panel_tab_at(my, usable_h) {
                    let row =
                        crate::renderer::SidebarRects::tab_rect(pos, self.sidebar_scroll_offset);
                    if crate::renderer::SidebarRects::inside(mx, my, row) {
                        new_hovered_tab = Some(tab_idx);
                        is_pointer = true;

                        if self.tabs.len() > 1 {
                            let close = crate::renderer::SidebarRects::close_btn_rect(row);
                            if crate::renderer::SidebarRects::inside(mx, my, close) {
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
                if self.hovered_project.is_some() {
                    self.hovered_project = None;
                    needs_redraw = true;
                }
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
            }
        }

        // Hover en el panel de proyectos (modo de pestañas superiores)
        if self.is_horizontal_tabs() {
            let usable_h = win_h.saturating_sub(self.status_bar_height());
            let mut new_row = None;
            if let Some(lx) = self.panel_local_x(mx, my, win_w) {
                new_row = crate::renderer::ProjectsPanelRects::row_at(
                    my,
                    usable_h,
                    self.panel_rail().len() + 1,
                    self.sidebar_scroll_offset,
                );
                is_pointer |= new_row.is_some() || self.panel_change_at(lx, my, usable_h).is_some();
            }
            if self.hovered_project != new_row {
                self.hovered_project = new_row;
                needs_redraw = true;
            }
        }

        // Detección de hover en la pestaña de Ajustes (si está activa)
        if is_settings_active && mx >= offset && my >= top_offset {
            let page_w = win_w.saturating_sub(offset + self.panel_right_inset());
            let page_h = win_h.saturating_sub(top_offset);
            let rects = SettingsPageRects::compute(offset, top_offset, page_w, page_h);
            let over_sb = self
                .settings_scrollbar()
                .is_some_and(|(t, _)| mx + 6 >= t.0 && my >= t.1 && my < t.1 + t.3);
            if over_sb {
                is_pointer = true;
            }
            if self.settings_page_state.hovered_scrollbar != over_sb {
                self.settings_page_state.hovered_scrollbar = over_sb;
                needs_redraw = true;
            }
            let my = if my >= top_offset + SettingsPageRects::CONTENT_TOP {
                my + self.settings_scroll()
            } else {
                my
            };

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
            let mut h_ts = None;
            let mut h_tm = None;
            let mut h_tl = None;
            let mut h_fm = false;
            let mut h_fp = false;
            let mut h_fb = false;
            let mut h_uif_m = false;
            let mut h_uif_p = false;
            let mut h_uif_b = false;
            let mut h_sm = false;
            let mut h_sp = false;
            let mut h_sb_val = false;
            let mut h_om = false;
            let mut h_op = false;
            let mut h_ob = false;
            let mut h_cb = false;
            let mut h_gp = false;
            let mut h_cs = false;
            let mut h_sb = false;
            let mut h_fe = false;
            let mut h_rs = false;
            let mut h_as = false;
            let mut h_ww = false;
            let mut h_si = false;
            let mut h_lig = false;
            let mut h_of = false;
            let mut h_reset = SettingsPageRects::inside(mx, my, rects.reset_defaults_btn);

            match self.settings_page_state.section {
                SettingsSection::Appearance => {
                    h_theme = rects.theme_row_at(mx, my);
                    h_ts = if SettingsPageRects::inside(mx, my, rects.theme_slot_dark_btn) {
                        Some(0)
                    } else if SettingsPageRects::inside(mx, my, rects.theme_slot_light_btn) {
                        Some(1)
                    } else {
                        None
                    };
                    if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                        h_tm = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.theme_mode_dark_btn) {
                        h_tm = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.theme_mode_light_btn) {
                        h_tm = Some(2);
                    }

                    if SettingsPageRects::inside(mx, my, rects.tab_layout_vertical_btn) {
                        h_tl = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.tab_layout_horizontal_btn) {
                        h_tl = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.panel_side_left_btn) {
                        h_tl = Some(2);
                    } else if SettingsPageRects::inside(mx, my, rects.panel_side_right_btn) {
                        h_tl = Some(3);
                    }

                    h_fm = SettingsPageRects::inside(mx, my, rects.font_minus_btn);
                    h_fp = SettingsPageRects::inside(mx, my, rects.font_plus_btn);
                    h_fb = SettingsPageRects::inside(mx, my, rects.font_value_box);
                    h_uif_m = SettingsPageRects::inside(mx, my, rects.ui_font_minus_btn);
                    h_uif_p = SettingsPageRects::inside(mx, my, rects.ui_font_plus_btn);
                    h_uif_b = SettingsPageRects::inside(mx, my, rects.ui_font_value_box);
                    h_sm = SettingsPageRects::inside(mx, my, rects.spacing_minus_btn);
                    h_sp = SettingsPageRects::inside(mx, my, rects.spacing_plus_btn);
                    h_sb_val = SettingsPageRects::inside(mx, my, rects.spacing_value_box);
                    h_om = SettingsPageRects::inside(mx, my, rects.opacity_minus_btn);
                    h_op = SettingsPageRects::inside(mx, my, rects.opacity_plus_btn);
                    h_ob = SettingsPageRects::inside(mx, my, rects.opacity_value_box);
                }
                SettingsSection::Terminal => {
                    h_cb = SettingsPageRects::inside(mx, my, rects.cursor_blink_btn);
                    h_gp = SettingsPageRects::inside(mx, my, rects.shell_integration_btn);
                    h_cs = SettingsPageRects::inside(mx, my, rects.copy_on_select_btn);
                    h_sb = SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn);
                    h_fe = SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn);
                    h_rs = SettingsPageRects::inside(mx, my, rects.restore_session_btn);
                    h_as = SettingsPageRects::inside(mx, my, rects.auto_save_btn);
                    h_ww = SettingsPageRects::inside(mx, my, rects.word_wrap_btn);
                    h_si = SettingsPageRects::inside(mx, my, rects.ssh_integration_btn);
                    h_lig = SettingsPageRects::inside(mx, my, rects.ligatures_btn);
                    h_of = SettingsPageRects::inside(mx, my, rects.open_file_btn);
                    h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.term_reset_btn);
                }
                SettingsSection::About => {
                    h_of = SettingsPageRects::inside(mx, my, rects.about_open_file_btn);
                    h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.about_reset_btn);
                }
                SettingsSection::Shortcuts => {}
            }

            if h_theme.is_some()
                || h_ts.is_some()
                || h_tm.is_some()
                || h_tl.is_some()
                || h_fm
                || h_fp
                || h_fb
                || h_uif_m
                || h_uif_p
                || h_uif_b
                || h_sm
                || h_sp
                || h_sb_val
                || h_om
                || h_op
                || h_ob
                || h_cb
                || h_gp
                || h_cs
                || h_sb
                || h_fe
                || h_rs
                || h_as
                || h_ww
                || h_si
                || h_lig
                || h_of
                || h_reset
            {
                is_pointer = true;
            }

            let st = &mut self.settings_page_state;
            if st.hovered_section != h_sec
                || st.hovered_theme != h_theme
                || st.hovered_theme_slot != h_ts
                || st.hovered_theme_mode != h_tm
                || st.hovered_tab_layout != h_tl
                || st.hovered_font_minus != h_fm
                || st.hovered_font_plus != h_fp
                || st.hovered_font_box != h_fb
                || st.hovered_ui_font_minus != h_uif_m
                || st.hovered_ui_font_plus != h_uif_p
                || st.hovered_ui_font_box != h_uif_b
                || st.hovered_spacing_minus != h_sm
                || st.hovered_spacing_plus != h_sp
                || st.hovered_spacing_box != h_sb_val
                || st.hovered_opacity_minus != h_om
                || st.hovered_opacity_plus != h_op
                || st.hovered_opacity_box != h_ob
                || st.hovered_cursor_blink != h_cb
                || st.hovered_shell_integration != h_gp
                || st.hovered_copy_on_select != h_cs
                || st.hovered_status_bar != h_sb
                || st.hovered_file_explorer != h_fe
                || st.hovered_restore_session != h_rs
                || st.hovered_auto_save != h_as
                || st.hovered_word_wrap != h_ww
                || st.hovered_ssh_integration != h_si
                || st.hovered_ligatures != h_lig
                || st.hovered_open_file != h_of
                || st.hovered_reset_defaults != h_reset
            {
                st.hovered_section = h_sec;
                st.hovered_theme = h_theme;
                st.hovered_theme_slot = h_ts;
                st.hovered_theme_mode = h_tm;
                st.hovered_tab_layout = h_tl;
                st.hovered_font_minus = h_fm;
                st.hovered_font_plus = h_fp;
                st.hovered_font_box = h_fb;
                st.hovered_ui_font_minus = h_uif_m;
                st.hovered_ui_font_plus = h_uif_p;
                st.hovered_ui_font_box = h_uif_b;
                st.hovered_spacing_minus = h_sm;
                st.hovered_spacing_plus = h_sp;
                st.hovered_spacing_box = h_sb_val;
                st.hovered_opacity_minus = h_om;
                st.hovered_opacity_plus = h_op;
                st.hovered_opacity_box = h_ob;
                st.hovered_cursor_blink = h_cb;
                st.hovered_shell_integration = h_gp;
                st.hovered_copy_on_select = h_cs;
                st.hovered_status_bar = h_sb;
                st.hovered_file_explorer = h_fe;
                st.hovered_restore_session = h_rs;
                st.hovered_auto_save = h_as;
                st.hovered_word_wrap = h_ww;
                st.hovered_ssh_integration = h_si;
                st.hovered_ligatures = h_lig;
                st.hovered_open_file = h_of;
                st.hovered_reset_defaults = h_reset;
                needs_redraw = true;
            }
        } else if is_settings_active && self.settings_page_state.clear_hover() {
            needs_redraw = true;
        }

        // Detección de hover dentro del explorador lateral de archivos (si no es settings)
        if !is_settings_active && self.file_explorer.visible {
            let status_bar_h = self.status_bar_height();
            let (panel_x, panel_y, panel_w, panel_h) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                let is_over_scrollbar = if let Some((track, _thumb)) =
                    TerminalRenderer::explorer_scrollbar_geometry(
                        win_w,
                        win_h,
                        top_offset,
                        status_bar_h,
                        &self.file_explorer,
                    ) {
                    mx >= track.0
                        && mx < track.0 + track.2
                        && my >= track.1
                        && my < track.1 + track.3
                } else {
                    false
                };

                let item_hover = if is_over_scrollbar {
                    None
                } else {
                    self.file_explorer.item_at_y(my, panel_y)
                };

                if item_hover.is_some() {
                    is_pointer = true;
                }

                if self.file_explorer.hovered_index != item_hover
                    || self.file_explorer.hovered_scrollbar != is_over_scrollbar
                {
                    self.file_explorer.hovered_index = item_hover;
                    self.file_explorer.hovered_scrollbar = is_over_scrollbar;
                    needs_redraw = true;
                }
            } else if self.file_explorer.hovered_index.is_some()
                || self.file_explorer.hovered_scrollbar
            {
                self.file_explorer.hovered_index = None;
                self.file_explorer.hovered_scrollbar = false;
                needs_redraw = true;
            }
        }

        // Coordenadas relativas a paneles en la terminal activa
        if !is_settings_active && self.tabs[self.active_tab_index].file_viewer.is_none() {
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

                    let is_over_scroll_to_bottom =
                        if pane.terminal.scroll_offset > 0 && !pane.terminal.is_alt_screen {
                            let (bx, by, bw, bh) = crate::renderer::pane_scroll_to_bottom_btn_rect(
                                rect.x,
                                rect.y,
                                rect.width,
                                rect.height,
                            );
                            mx >= bx && mx < bx + bw && my >= by && my < by + bh
                        } else {
                            false
                        };

                    if is_over_scroll_to_bottom {
                        is_pointer = true;
                        if !pane.terminal.scroll_to_bottom_hovered {
                            pane.terminal.scroll_to_bottom_hovered = true;
                            needs_redraw = true;
                        }
                    } else if pane.terminal.scroll_to_bottom_hovered {
                        pane.terminal.scroll_to_bottom_hovered = false;
                        needs_redraw = true;
                    }

                    if is_over_scrollbar {
                        is_pointer = true;
                        if !pane.terminal.scrollbar_hovered {
                            pane.terminal.scrollbar_hovered = true;
                            needs_redraw = true;
                        }
                    } else {
                        if pane.terminal.scrollbar_hovered {
                            pane.terminal.scrollbar_hovered = false;
                            needs_redraw = true;
                        }
                        if !pane.terminal.is_mouse_reporting_active() || bypass {
                            let mut is_link = false;
                            if self.modifiers.control_key() {
                                if let Some(renderer) = &self.renderer {
                                    let term_x = mx.saturating_sub(rect.x);
                                    let term_y = my.saturating_sub(rect.y);
                                    let col = term_x.saturating_sub(renderer.margin_x)
                                        / renderer.char_width;
                                    let col = col.min(pane.terminal.cols.saturating_sub(1));
                                    let row_in_view = term_y.saturating_sub(renderer.margin_y)
                                        / renderer.row_height;
                                    let row_in_view =
                                        row_in_view.min(pane.terminal.rows.saturating_sub(1));
                                    let global_line =
                                        pane.terminal.visible_line_start() + row_in_view;
                                    is_link = pane.terminal.url_at(col, global_line).is_some();
                                }
                            }
                            if is_link {
                                is_pointer = true;
                            } else {
                                is_text = true;
                            }
                        }
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
                            let col =
                                term_x.saturating_sub(renderer.margin_x) / renderer.char_width;
                            let col = col.min(pane.terminal.cols.saturating_sub(1));

                            // Auto-scroll si el cursor se arrastra por encima o por debajo del panel
                            let mut scrolled = false;
                            if my < rect.y + renderer.margin_y
                                && pane.terminal.scroll_offset < pane.terminal.scrollback.len()
                            {
                                pane.terminal.scroll_up(1);
                                scrolled = true;
                            } else if my >= rect.y + rect.height && pane.terminal.scroll_offset > 0
                            {
                                pane.terminal.scroll_down(1);
                                scrolled = true;
                            }
                            if scrolled {
                                self.last_scroll_time = Instant::now();
                            }

                            let row_in_view =
                                term_y.saturating_sub(renderer.margin_y) / renderer.row_height;
                            let row_in_view = row_in_view.min(pane.terminal.rows.saturating_sub(1));
                            let global_line = pane.terminal.visible_line_start() + row_in_view;

                            let new_sel = if let Some(anchor) = self.drag_anchor {
                                // Tras doble/triple clic: extiende por palabra/línea.
                                pane.terminal.selection = Some(anchor);
                                if self.terminal_click_count >= 3 {
                                    pane.terminal.select_line_at(global_line);
                                } else {
                                    pane.terminal.select_word_at(col, global_line);
                                }
                                pane.terminal
                                    .selection
                                    .map(|unit| {
                                        let key = |p: (usize, usize)| (p.1, p.0);
                                        let (a, b) = (anchor.start, anchor.end);
                                        let (u0, u1) = (unit.start, unit.end);
                                        if key(u0) < key(a) {
                                            SelectionRange { start: u0, end: b }
                                        } else {
                                            let end = if key(u1) > key(b) { u1 } else { b };
                                            SelectionRange { start: a, end }
                                        }
                                    })
                                    .or_else(|| {
                                        pane.terminal.selection = Some(anchor);
                                        None
                                    })
                            } else {
                                self.drag_start.map(|start| SelectionRange {
                                    start,
                                    end: (col, global_line),
                                })
                            };
                            if let Some(new_sel) = new_sel {
                                let new_sel = Some(new_sel);
                                if pane.terminal.selection != new_sel {
                                    pane.terminal.selection = new_sel;
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

        // Si la pestaña activa es un visor de archivos, gestionar arrastre de selección y cursor de texto (I-Beam)
        if !is_settings_active && self.tabs[self.active_tab_index].file_viewer.is_some() {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let right_offset = self.file_explorer_offset();
            let viewer_w = win_w.saturating_sub(offset + right_offset);
            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
            let content_y = top_offset + 38;
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let char_w = self.renderer.as_ref().map(|r| r.char_width).unwrap_or(8);

            if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                let ext = viewer
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let is_markdown = ext == "md" || ext == "markdown";
                let header_rects = crate::renderer::FileViewerHeaderRects::compute(
                    offset,
                    top_offset,
                    viewer_w,
                    viewer_h,
                    is_markdown,
                    viewer.git_base.is_some(),
                );

                let is_hover_diff = header_rects
                    .diff_btn
                    .map(|r| crate::renderer::FileViewerHeaderRects::inside(mx, my, r))
                    .unwrap_or(false);
                if viewer.hovered_diff_btn != is_hover_diff {
                    viewer.hovered_diff_btn = is_hover_diff;
                    needs_redraw = true;
                }
                let is_hover_mode =
                    crate::renderer::FileViewerHeaderRects::inside(mx, my, header_rects.mode_btn);
                let is_hover_md = header_rects
                    .md_btn
                    .map(|r| crate::renderer::FileViewerHeaderRects::inside(mx, my, r))
                    .unwrap_or(false);

                if is_hover_mode || is_hover_md || is_hover_diff {
                    is_pointer = true;
                }
                if viewer.hovered_mode_btn != is_hover_mode {
                    viewer.hovered_mode_btn = is_hover_mode;
                    needs_redraw = true;
                }
                if viewer.hovered_md_btn != is_hover_md {
                    viewer.hovered_md_btn = is_hover_md;
                    needs_redraw = true;
                }

                let is_over_viewer_scrollbar = if let Some((track, _)) =
                    TerminalRenderer::file_viewer_scrollbar_geometry(
                        offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                    ) {
                    mx >= track.0
                        && mx < track.0 + track.2
                        && my >= track.1
                        && my < track.1 + track.3
                } else {
                    false
                };
                if is_over_viewer_scrollbar {
                    is_pointer = true;
                }
                if viewer.hovered_scrollbar != is_over_viewer_scrollbar {
                    viewer.hovered_scrollbar = is_over_viewer_scrollbar;
                    needs_redraw = true;
                }

                let lay = crate::renderer::FileViewerLayout::compute(
                    offset,
                    top_offset,
                    viewer_w,
                    viewer_h,
                    row_h,
                    char_w,
                    viewer.lines.len(),
                );
                let gutter_w = lay.gutter_w;
                let over_gutter =
                    mx >= offset && mx < lay.sep_x && my >= content_y && my < top_offset + viewer_h;
                if viewer.hovered_gutter != over_gutter {
                    viewer.hovered_gutter = over_gutter;
                    needs_redraw = true;
                }

                let over_hscroll = lay
                    .hscrollbar(viewer.max_scroll_x(), viewer.scroll_offset_x)
                    .is_some_and(|(track, _)| {
                        mx >= track.0
                            && mx < track.0 + track.2
                            && my + 4 >= track.1
                            && my < track.1 + track.3 + 4
                    });
                if over_hscroll {
                    is_pointer = true;
                }
                if viewer.hovered_hscrollbar != over_hscroll {
                    viewer.hovered_hscrollbar = over_hscroll;
                    needs_redraw = true;
                }

                if self.viewer_drag_selecting && !viewer.is_binary && !viewer.lines.is_empty() {
                    let visible_lines = lay.content_h / lay.row_h;
                    let total_rows = viewer.rows().len();

                    if my < content_y && viewer.scroll_offset > 0 {
                        viewer.scroll_offset = viewer.scroll_offset.saturating_sub(1);
                    } else if my >= top_offset + viewer_h {
                        let max_scroll = total_rows.saturating_sub(visible_lines);
                        if viewer.scroll_offset < max_scroll {
                            viewer.scroll_offset += 1;
                        }
                    }

                    let clicked_row = viewer.scroll_offset
                        + if my < content_y {
                            0
                        } else {
                            (my - content_y) / row_h.max(16)
                        };
                    let (target_line, target_col) = viewer
                        .hit_test(clicked_row, mx, &lay)
                        .unwrap_or((viewer.cursor_line, viewer.cursor_col));

                    viewer.cursor_line = target_line;
                    viewer.cursor_col = target_col;

                    if let Some(start) = self.viewer_drag_start {
                        viewer.selection = Some((start, (target_line, target_col)));
                    }
                    let content_h = viewer_h.saturating_sub(38);
                    let visible_lines = (content_h / row_h.max(16)).max(1);
                    viewer.ensure_cursor_visible(visible_lines);
                    needs_redraw = true;
                }

                if !viewer.is_binary
                    && mx >= offset
                    && mx < offset + viewer_w
                    && my >= content_y
                    && my < top_offset + viewer_h
                {
                    // Si no está sobre la barra de desplazamiento lateral derecha
                    let scrollbar_margin = 16;
                    let text_max_x = (offset + viewer_w).saturating_sub(scrollbar_margin);
                    if mx >= (offset + gutter_w) && mx < text_max_x {
                        let clicked_row = viewer.scroll_offset + (my - content_y) / row_h.max(16);
                        let is_link = if viewer.render_markdown || self.modifiers.control_key() {
                            viewer
                                .hit_test(clicked_row, mx, &lay)
                                .and_then(|(l, c)| viewer.link_at(l, c))
                                .is_some()
                        } else {
                            false
                        };
                        if is_link {
                            is_pointer = true;
                        } else {
                            is_text = true;
                        }
                    }
                }
            }
        }

        // Hover en el indicador de modelo/agente de IA y de actualización en la barra de estado inferior
        let (new_hovered_ai, new_hovered_update) =
            if self.config.appearance.show_status_bar && !is_settings_active {
                let active_tab = &self.tabs[self.active_tab_index];
                let (ssh_host, ai_status) = if let Some(ref viewer) = active_tab.file_viewer {
                    (viewer.remote_destination.as_deref(), AiAgentStatus::None)
                } else if let Some(pane) = active_tab
                    .panes
                    .iter()
                    .find(|p| p.id == active_tab.active_pane_id)
                {
                    let s_host = pane
                        .terminal
                        .ssh_session
                        .as_ref()
                        .filter(|s| s.is_active)
                        .map(|s| s.destination.as_str());
                    let ai = detect_ai_agent(pane.terminal.window_title.as_deref());
                    (s_host, ai)
                } else {
                    (None, AiAgentStatus::None)
                };
                let active_model = self.ai_usage.snapshot.model_for(ai_status);
                let (_, badge_label, _) = TerminalRenderer::status_bar_badge_info(
                    ai_status,
                    ssh_host,
                    active_model,
                    &self.theme,
                );
                let label_w = self
                    .renderer
                    .as_ref()
                    .map(|r| r.measure_ui_text(&badge_label, 11.0, true))
                    .unwrap_or(badge_label.len() * 7);

                let update_info = crate::updater::get_available_update();
                let update_text = update_info
                    .as_ref()
                    .map(|u| format!("v{} disponible", u.latest_version));
                let update_w = update_text.as_ref().map(|t| {
                    self.renderer
                        .as_ref()
                        .map(|r| r.measure_ui_text(t, 11.0, true))
                        .unwrap_or(t.len() * 7)
                });

                let sb_rects = StatusBarRects::compute(win_w, win_h, label_w, update_w);
                let ai_hov = if self.hovered_ai_badge {
                    let popup_h = TerminalRenderer::ai_usage_popup_height(&self.ai_usage.snapshot);
                    let popup_rect = StatusBarRects::popup_rect(win_w, win_h, popup_h);
                    StatusBarRects::inside_badge_or_popup(mx, my, sb_rects.ai_badge, popup_rect)
                } else {
                    StatusBarRects::inside(mx, my, sb_rects.ai_badge)
                };
                let update_hov = sb_rects
                    .update_badge
                    .is_some_and(|r| StatusBarRects::inside(mx, my, r));
                (ai_hov, update_hov)
            } else {
                (false, false)
            };

        if new_hovered_ai {
            is_pointer = true;
            let just_entered = !self.hovered_ai_badge;
            let active_tab = &self.tabs[self.active_tab_index];
            let (term_ref, agents) = if active_tab.file_viewer.is_some() {
                (None, Vec::new())
            } else if let Some(pane) = active_tab
                .panes
                .iter()
                .find(|p| p.id == active_tab.active_pane_id)
            {
                (Some(&pane.terminal), active_agents(active_tab))
            } else {
                (None, Vec::new())
            };
            if self.ai_usage.refresh_if_needed(term_ref, just_entered) {
                self.spawn_ai_local_scan(agents);
            }
        }
        if new_hovered_update {
            is_pointer = true;
        }
        if self.hovered_ai_badge != new_hovered_ai {
            self.hovered_ai_badge = new_hovered_ai;
            needs_redraw = true;
        }
        if self.hovered_update_badge != new_hovered_update {
            self.hovered_update_badge = new_hovered_update;
            needs_redraw = true;
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

        // Cursor de redimensionamiento en los bordes (solo en ventana flotante, no maximizada ni pantalla completa).
        let can_resize = self
            .window
            .as_ref()
            .is_some_and(|w| !w.is_maximized() && w.fullscreen().is_none());
        let border_resize_cursor = if can_resize {
            use winit::window::ResizeDirection;
            self.compute_border_resize_direction(mx, my, win_w, win_h)
                .map(|dir| match dir {
                    ResizeDirection::NorthEast | ResizeDirection::SouthWest => {
                        winit::window::CursorIcon::NeswResize
                    }
                    ResizeDirection::NorthWest | ResizeDirection::SouthEast => {
                        winit::window::CursorIcon::NwseResize
                    }
                    ResizeDirection::West | ResizeDirection::East => {
                        winit::window::CursorIcon::EwResize
                    }
                    ResizeDirection::North | ResizeDirection::South => {
                        winit::window::CursorIcon::NsResize
                    }
                })
        } else {
            None
        };

        let divider_cursor = self.divider_at(mx, my).map(|d| d.cursor_icon());
        let explorer_edge_cursor = (!is_settings_active
            && self.file_explorer.on_resize_edge(
                mx,
                my,
                win_w,
                win_h,
                top_offset,
                self.status_bar_height(),
            ))
        .then_some(winit::window::CursorIcon::ColResize);
        let cursor_icon = if let Some(resize_cur) = border_resize_cursor
            .or(explorer_edge_cursor)
            .or(divider_cursor)
        {
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
}
