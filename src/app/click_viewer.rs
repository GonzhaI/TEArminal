//! Gestión de eventos de clic dentro del visor de archivos, divisores y cabeceras de panel.

use super::*;
use crate::app::SplitDirection;
use crate::renderer::{PaneHeaderButton, PaneHeaderRects};

impl TearminalApp {
    pub(super) fn click_file_viewer_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        // Clics dentro del visor de archivos si la pestaña activa es un file viewer
        let is_viewer_active = self.tabs[self.active_tab_index].file_viewer.is_some();
        if is_viewer_active {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let right_offset = self.file_explorer_offset();
            let viewer_w = win_w.saturating_sub(offset + right_offset);
            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let char_w = self.renderer.as_ref().map(|r| r.char_width).unwrap_or(8);

            if mx >= offset
                && mx < offset + viewer_w
                && my >= top_offset
                && my < top_offset + viewer_h
            {
                self.file_explorer.is_focused = false;
                if state == ElementState::Pressed {
                    if button == MouseButton::Middle {
                        // Clic central: pegar contenido del portapapeles
                        self.paste_from_clipboard();
                        return true;
                    }
                    if button == MouseButton::Left {
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
                                viewer.has_diff_capability(),
                                viewer.is_diff(),
                            );

                            if let Some(rect) = header_rects.diff_mode_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, rect) {
                                    viewer.toggle_diff_mode();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

                            if let Some(rect) = header_rects.diff_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, rect) {
                                    viewer.toggle_diff();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

                            if crate::renderer::FileViewerHeaderRects::inside(
                                mx,
                                my,
                                header_rects.mode_btn,
                            ) {
                                viewer.toggle_read_only();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return true;
                            }

                            if let Some(md_rect) = header_rects.md_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, md_rect) {
                                    viewer.render_markdown = !viewer.render_markdown;
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
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

                            // Interacción con la barra flotante de búsqueda (`Ctrl + F`)
                            if viewer.search_open {
                                let s_rects = crate::renderer::FileViewerSearchRects::compute(&lay);
                                if crate::renderer::FileViewerSearchRects::inside(
                                    mx,
                                    my,
                                    s_rects.bar,
                                ) {
                                    let visible_lines = (lay.content_h / lay.row_h.max(16)).max(1);
                                    if crate::renderer::FileViewerSearchRects::inside(
                                        mx,
                                        my,
                                        s_rects.close_btn,
                                    ) {
                                        viewer.close_search();
                                    } else if !viewer.goto_mode
                                        && crate::renderer::FileViewerSearchRects::inside(
                                            mx,
                                            my,
                                            s_rects.prev_btn,
                                        )
                                    {
                                        viewer.search_prev(visible_lines);
                                    } else if !viewer.goto_mode
                                        && crate::renderer::FileViewerSearchRects::inside(
                                            mx,
                                            my,
                                            s_rects.next_btn,
                                        )
                                    {
                                        viewer.search_next(visible_lines);
                                    } else if crate::renderer::FileViewerSearchRects::inside(
                                        mx,
                                        my,
                                        s_rects.input,
                                    ) {
                                        let mut input = std::mem::take(&mut viewer.search_input);
                                        self.handle_text_input_click(
                                            &mut input,
                                            mx,
                                            s_rects.text_start_x,
                                        );
                                        if let Some(ref mut v) =
                                            self.tabs[self.active_tab_index].file_viewer
                                        {
                                            v.search_input = input;
                                        }
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

                            // Barra de desplazamiento horizontal
                            let max_x = viewer.max_scroll_x();
                            if let Some((track, thumb)) =
                                lay.hscrollbar(max_x, viewer.scroll_offset_x)
                            {
                                if mx >= track.0
                                    && mx < track.0 + track.2
                                    && my + 4 >= track.1
                                    && my < track.1 + track.3 + 4
                                {
                                    if mx >= thumb.0 && mx < thumb.0 + thumb.2 {
                                        self.scrollbar_drag = Some(ScrollbarDragState {
                                            target: ScrollbarTarget::ViewerHorizontal,
                                            grab_offset: (mx - thumb.0) as f32,
                                        });
                                    } else {
                                        let rel = mx.saturating_sub(track.0 + thumb.2 / 2);
                                        if let Some(x) = (rel * max_x)
                                            .checked_div(track.2.saturating_sub(thumb.2))
                                        {
                                            viewer.scroll_offset_x = x.min(max_x);
                                        }
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

                            // Comprobar clic en la barra de desplazamiento del visor
                            if let Some((track, thumb)) =
                                TerminalRenderer::file_viewer_scrollbar_geometry(
                                    offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                                )
                            {
                                if crate::renderer::ScrollbarRects::hit(
                                    track,
                                    offset + viewer_w,
                                    mx,
                                    my,
                                ) {
                                    if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                        self.scrollbar_drag = Some(ScrollbarDragState {
                                            target: ScrollbarTarget::ViewerVertical,
                                            grab_offset: (my - thumb.1) as f32,
                                        });
                                        self.last_scroll_time = Instant::now();
                                    } else {
                                        let visible_lines = lay.content_h / lay.row_h;
                                        let max_offset =
                                            viewer.rows().len().saturating_sub(visible_lines);
                                        if let Some(off) =
                                            crate::renderer::scroll_offset_for_thumb_y(
                                                track,
                                                thumb,
                                                my.saturating_sub(thumb.3 / 2) as f32,
                                                max_offset,
                                            )
                                        {
                                            viewer.scroll_offset = off;
                                            self.last_scroll_time = Instant::now();
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                        }
                                    }
                                    return true;
                                }
                            }

                            // Clic dentro del contenido de texto para posicionar el cursor y seleccionar
                            let content_y =
                                top_offset + crate::renderer::FileViewerLayout::HEADER_H;
                            if my >= content_y && !viewer.is_binary && !viewer.lines.is_empty() {
                                let clicked_row =
                                    viewer.scroll_offset + (my - content_y) / row_h.max(16);

                                // Clic en el margen: pliega o despliega la región de esa línea
                                if mx < lay.sep_x {
                                    if let Some(row) = viewer.rows().get(clicked_row).copied() {
                                        if row.start == 0
                                            && (viewer.folded.contains(&row.line)
                                                || viewer.fold_end(row.line).is_some())
                                        {
                                            viewer.toggle_fold(row.line);
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                            return true;
                                        }
                                    }
                                }
                                let Some((target_line, target_col)) =
                                    viewer.hit_test(clicked_row, mx, &lay)
                                else {
                                    return true;
                                };

                                // Si es enlace externo (en modo Markdown preview o con Ctrl pulsado): abrir en navegador
                                if (viewer.render_markdown || self.modifiers.control_key())
                                    && !self.modifiers.shift_key()
                                {
                                    if let Some(url) = viewer.link_at(target_line, target_col) {
                                        crate::app::platform::shell_open_url(&url);
                                        return true;
                                    }
                                }

                                if self.modifiers.shift_key() {
                                    let start = viewer
                                        .selection
                                        .map(|s| s.0)
                                        .unwrap_or((viewer.cursor_line, viewer.cursor_col));
                                    viewer.cursor_line = target_line;
                                    viewer.cursor_col = target_col;
                                    viewer.selection = Some((start, (target_line, target_col)));
                                } else {
                                    let now = Instant::now();
                                    let is_quick_click = now
                                        .duration_since(self.last_viewer_click_time)
                                        <= platform::double_click_time()
                                        && self.last_viewer_click_pos.is_some_and(|(l, c)| {
                                            l == target_line
                                                && (c as isize - target_col as isize).abs() <= 1
                                        });

                                    viewer.cursor_line = target_line;
                                    viewer.cursor_col = target_col;

                                    if is_quick_click && self.viewer_click_count == 1 {
                                        viewer.select_word_at_cursor();
                                        self.viewer_click_count = 2;
                                    } else if is_quick_click && self.viewer_click_count == 2 {
                                        viewer.select_line_at_cursor();
                                        self.viewer_click_count = 3;
                                    } else {
                                        viewer.clear_selection();
                                        self.viewer_click_count = 1;
                                        self.viewer_drag_selecting = true;
                                        self.viewer_drag_start = Some((target_line, target_col));
                                    }
                                    self.last_viewer_click_time = now;
                                    self.last_viewer_click_pos = Some((target_line, target_col));
                                }

                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        }
                    }
                }
                return true;
            }
        }

        // 2. Divisor entre paneles: agarrarlo para redimensionar
        if button == MouseButton::Left && state == ElementState::Pressed {
            if let Some(divider) = self.divider_at(mx, my) {
                self.divider_drag = Some(divider);
                return true;
            }
        }

        // 2b. Cabecera de un panel dividido: foco y botones dividir · maximizar · cerrar
        if button == MouseButton::Left && state == ElementState::Pressed {
            let tab = &self.tabs[self.active_tab_index];
            let hit = if tab.is_settings || tab.file_viewer.is_some() {
                None
            } else {
                self.active_tab_pane_rects(win_w, win_h)
                    .iter()
                    .find_map(|rect| {
                        if !rect.has_header {
                            return None;
                        }
                        let header = PaneHeaderRects::compute(rect.x, rect.slot_y, rect.width);
                        if !PaneHeaderRects::inside(mx, my, header.header) {
                            return None;
                        }
                        let focused = tab.active_pane_id == rect.pane_id;
                        Some((rect.pane_id, focused, header.button_at(mx, my)))
                    })
            };
            if let Some((pane_id, focused, hit)) = hit {
                if !focused {
                    self.tabs[self.active_tab_index].active_pane_id = pane_id;
                }
                match hit {
                    Some(PaneHeaderButton::Split) => {
                        self.split_active_pane(SplitDirection::Vertical);
                    }
                    Some(PaneHeaderButton::SplitHorizontal) => {
                        self.split_active_pane(SplitDirection::Horizontal);
                    }
                    Some(PaneHeaderButton::Zoom) => {
                        if self.tabs[self.active_tab_index].toggle_pane_zoom(pane_id) {
                            self.recompute_grid_for_all_tabs();
                            self.save_session();
                        }
                    }
                    Some(PaneHeaderButton::Close) => self.close_pane_by_id(pane_id),
                    None => {}
                }
                self.request_redraw();
                return true;
            }
        }

        false
    }
}
