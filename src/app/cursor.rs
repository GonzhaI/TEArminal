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
            if let Some((track, thumb)) = TerminalRenderer::sidebar_scrollbar_geometry(
                SIDEBAR_WIDTH,
                usable_h,
                self.tabs.len(),
                self.sidebar_scroll_offset,
            ) {
                let track_y = track.1;
                let track_h = track.3;
                let thumb_h = thumb.3;
                let available_travel = track_h.saturating_sub(thumb_h);
                let tab_item_h = 36;
                let tab_spacing = 4;
                let list_start_y = 42;
                let toggle_h = 30;
                let toggle_y = usable_h.saturating_sub(toggle_h + 8);
                let list_max_y = toggle_y.saturating_sub(8);
                let visible_items = if list_max_y > list_start_y {
                    (list_max_y - list_start_y) / (tab_item_h + tab_spacing)
                } else {
                    1
                };
                let max_offset = self.tabs.len().saturating_sub(visible_items);
                if available_travel > 0 && max_offset > 0 {
                    let target_thumb_top =
                        (my as f32 - self.sidebar_scrollbar_drag_offset_y - track_y as f32)
                            .clamp(0.0, available_travel as f32);
                    let ratio = target_thumb_top / available_travel as f32;
                    self.sidebar_scroll_offset =
                        ((ratio * max_offset as f32).round() as usize).min(max_offset);
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
                let track_y = track.1;
                let track_h = track.3;
                let thumb_h = thumb.3;
                let available_travel = track_h.saturating_sub(thumb_h);
                let (_panel_x, panel_y, _panel_w, panel_h) =
                    crate::explorer::FileExplorer::panel_geometry(
                        win_w,
                        win_h,
                        top_offset,
                        status_bar_h,
                    );
                let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
                let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
                let list_y = panel_y + header_h + sub_h + 4;
                let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                let list_max_y = panel_y + panel_h.saturating_sub(6);
                let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
                let max_offset = self.file_explorer.items.len().saturating_sub(visible_items);
                if available_travel > 0 && max_offset > 0 {
                    let target_thumb_top =
                        (my as f32 - self.explorer_scrollbar_drag_offset_y - track_y as f32)
                            .clamp(0.0, available_travel as f32);
                    let ratio = target_thumb_top / available_travel as f32;
                    self.file_explorer.scroll_offset =
                        ((ratio * max_offset as f32).round() as usize).min(max_offset);
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
                        let track_y = track.1;
                        let track_h = track.3;
                        let thumb_h = thumb.3;
                        let available_travel = track_h.saturating_sub(thumb_h);
                        let content_h = viewer_h.saturating_sub(38);
                        let visible_lines = (content_h / row_h.max(16)).max(1);
                        let max_offset = viewer.rows().len().saturating_sub(visible_lines);

                        if available_travel > 0 && max_offset > 0 {
                            let target_thumb_top =
                                (my as f32 - self.viewer_scrollbar_drag_offset_y - track_y as f32)
                                    .clamp(0.0, available_travel as f32);
                            let ratio = target_thumb_top / available_travel as f32;
                            viewer.scroll_offset =
                                ((ratio * max_offset as f32).round() as usize).min(max_offset);
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

        // Arrastre de un divisor entre paneles
        if let Some(icon) = self.divider_drag.as_ref().map(Divider::cursor_icon) {
            self.drag_divider(mx, my);
            if let Some(w) = &self.window {
                w.set_cursor(icon);
            }
            return;
        }

        // Reordenar pestañas: la arrastrada salta a la posición bajo el ratón
        if let Some(from) = self.tab_drag {
            if let Some(to) = self.tab_slot_at(mx, my).filter(|&to| to != from) {
                self.move_tab(from, to);
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
                            if mx >= cr.0 && mx < cr.0 + cr.2 && my >= cr.1 && my < cr.1 + cr.3 {
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

            if self.sidebar_visible {
                let usable_h = win_h.saturating_sub(status_bar_h);
                let is_over_sidebar_sb = if let Some((track, _)) =
                    TerminalRenderer::sidebar_scrollbar_geometry(
                        SIDEBAR_WIDTH,
                        usable_h,
                        self.tabs.len(),
                        self.sidebar_scroll_offset,
                    ) {
                    mx >= track.0
                        && mx < track.0 + track.2
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
                    mx >= settings_btn_x && mx < settings_btn_x + 24 && (8..32).contains(&my);
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
                    let vis_idx = rel_y / item_total;
                    let tab_idx = vis_idx + self.sidebar_scroll_offset;
                    let in_item = (rel_y % item_total) < tab_item_h;

                    if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w {
                        new_hovered_tab = Some(tab_idx);
                        is_pointer = true;

                        if self.tabs.len() > 1 {
                            let tab_y = list_start_y + vis_idx * item_total;
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
                    } else if SettingsPageRects::inside(mx, my, rects.tab_layout_horizontal_btn) {
                        h_tl = Some(1);
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
                    h_gp = SettingsPageRects::inside(mx, my, rects.ghostty_prompt_btn);
                    h_cs = SettingsPageRects::inside(mx, my, rects.copy_on_select_btn);
                    h_sb = SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn);
                    h_fe = SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn);
                    h_rs = SettingsPageRects::inside(mx, my, rects.restore_session_btn);
                    h_as = SettingsPageRects::inside(mx, my, rects.auto_save_btn);
                    h_ww = SettingsPageRects::inside(mx, my, rects.word_wrap_btn);
                    h_si = SettingsPageRects::inside(mx, my, rects.ssh_integration_btn);
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
                || st.hovered_ghostty_prompt != h_gp
                || st.hovered_copy_on_select != h_cs
                || st.hovered_status_bar != h_sb
                || st.hovered_file_explorer != h_fe
                || st.hovered_restore_session != h_rs
                || st.hovered_auto_save != h_as
                || st.hovered_word_wrap != h_ww
                || st.hovered_ssh_integration != h_si
                || st.hovered_open_file != h_of
                || st.hovered_reset_defaults != h_reset
            {
                st.hovered_section = h_sec;
                st.hovered_theme = h_theme;
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
                st.hovered_ghostty_prompt = h_gp;
                st.hovered_copy_on_select = h_cs;
                st.hovered_status_bar = h_sb;
                st.hovered_file_explorer = h_fe;
                st.hovered_restore_session = h_rs;
                st.hovered_auto_save = h_as;
                st.hovered_word_wrap = h_ww;
                st.hovered_ssh_integration = h_si;
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
                || st.hovered_font_box
                || st.hovered_spacing_minus
                || st.hovered_spacing_plus
                || st.hovered_spacing_box
                || st.hovered_opacity_minus
                || st.hovered_opacity_plus
                || st.hovered_opacity_box
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
                st.hovered_font_box = false;
                st.hovered_spacing_minus = false;
                st.hovered_spacing_plus = false;
                st.hovered_spacing_box = false;
                st.hovered_opacity_minus = false;
                st.hovered_opacity_plus = false;
                st.hovered_opacity_box = false;
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
            let status_bar_h = self.status_bar_height();
            let (panel_x, panel_y, panel_w, panel_h) =
                crate::explorer::FileExplorer::panel_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                );
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                let sub_y = panel_y + crate::explorer::FileExplorer::HEADER_HEIGHT;
                let back_x = panel_x + 8;
                let is_back_hover =
                    mx >= back_x && mx < back_x + 22 && my >= sub_y + 4 && my < sub_y + 24;

                let fwd_x = back_x + 24;
                let is_fwd_hover =
                    mx >= fwd_x && mx < fwd_x + 22 && my >= sub_y + 4 && my < sub_y + 24;

                let up_x = fwd_x + 24;
                let is_up_hover =
                    mx >= up_x && mx < up_x + 22 && my >= sub_y + 4 && my < sub_y + 24;

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

                if is_back_hover || is_fwd_hover || is_up_hover || item_hover.is_some() {
                    is_pointer = true;
                }

                if self.file_explorer.hovered_back_btn != is_back_hover
                    || self.file_explorer.hovered_forward_btn != is_fwd_hover
                    || self.file_explorer.hovered_up_btn != is_up_hover
                    || self.file_explorer.hovered_index != item_hover
                    || self.file_explorer.hovered_scrollbar != is_over_scrollbar
                {
                    self.file_explorer.hovered_back_btn = is_back_hover;
                    self.file_explorer.hovered_forward_btn = is_fwd_hover;
                    self.file_explorer.hovered_up_btn = is_up_hover;
                    self.file_explorer.hovered_index = item_hover;
                    self.file_explorer.hovered_scrollbar = is_over_scrollbar;
                    needs_redraw = true;
                }
            } else if self.file_explorer.hovered_back_btn
                || self.file_explorer.hovered_forward_btn
                || self.file_explorer.hovered_up_btn
                || self.file_explorer.hovered_index.is_some()
                || self.file_explorer.hovered_scrollbar
            {
                self.file_explorer.hovered_back_btn = false;
                self.file_explorer.hovered_forward_btn = false;
                self.file_explorer.hovered_up_btn = false;
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
                            is_text = true;
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
                        is_text = true;
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
            } else if (at_top && at_right) || (at_bottom && at_left) {
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

        let divider_cursor = self.divider_at(mx, my).map(|d| d.cursor_icon());
        let cursor_icon = if let Some(resize_cur) = border_resize_cursor.or(divider_cursor) {
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
