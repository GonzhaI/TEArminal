//! Rueda del ratón: scroll del historial, listas y TUIs.

use super::*;

impl TearminalApp {
    /// Rueda del ratón: scroll del historial, listas y TUIs.
    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let bypass = self.modifiers.shift_key();
        let mx = self.mouse_pos.x as usize;
        let my = self.mouse_pos.y as usize;

        let (win_w, win_h) = self.window_size();
        let scroll_multiplier = platform::wheel_scroll_lines();
        let (row_h, char_w) = self.renderer.as_ref().map_or((20.0, 9.0), |r| {
            (r.row_height.max(1) as f32, r.char_width.max(1) as f32)
        });
        let (scroll_x_lines, scroll_y_lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / char_w, pos.y as f32 / row_h),
        };

        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();

        // Rueda con la paleta de comandos abierta: navega por los resultados
        if let Some(palette) = &mut self.palette {
            if scroll_y_lines.abs() > 0.01 {
                let dir = if scroll_y_lines > 0.0 { -1 } else { 1 };
                palette.scroll_by(dir);
                self.request_redraw();
            }
            return;
        }

        // Cualquier rueda despierta las barras de scroll (se auto-ocultan tras un instante)
        // Rueda sobre el menú contextual: no actúa; fuera de él lo cierra y sigue su curso
        if let Some(menu) = &self.context_menu {
            let (x, y, w, h) = menu.bounds();
            if mx >= x && mx < x + w && my >= y && my < y + h {
                return;
            }
            self.context_menu = None;
        }
        self.last_scroll_time = Instant::now();

        if self.tabs[self.active_tab_index].is_settings {
            // Sobre la lista de temas la rueda la desplaza a ella, no a la página
            if self.settings_page_state.section == SettingsSection::Appearance {
                let (modal_x, modal_y, modal_w, modal_h) = self.settings_modal();
                let rects = SettingsPageRects::compute_modal(0, 0, modal_w, modal_h);
                let (lx, ly) = (mx.wrapping_sub(modal_x), my.wrapping_sub(modal_y));
                let page_y = if ly >= SettingsPageRects::CONTENT_TOP {
                    ly + self.settings_scroll()
                } else {
                    ly
                };
                if lx < modal_w && ly < modal_h && rects.theme_row_at(lx, page_y).is_some() {
                    self.wheel_remainder += scroll_y_lines * scroll_multiplier;
                    let rows = (-self.wheel_remainder.trunc()) as isize;
                    if rows != 0 {
                        self.wheel_remainder += rows as f32;
                        self.scroll_theme_list(rows);
                    }
                    return;
                }
            }
            let dy = (scroll_y_lines * 48.0).round() as isize;
            let viewport_h = self
                .settings_modal()
                .3
                .saturating_sub(SettingsPageRects::CONTENT_TOP);
            let max = SettingsPageRects::max_scroll(self.settings_page_state.section, viewport_h);
            self.settings_page_state.scroll_y =
                (self.settings_scroll() as isize - dy).clamp(0, max as isize) as usize;
            self.request_redraw();
            return;
        }

        // Scroll en la lista de Spaces
        if self.spaces_rects(win_h).contains(mx, my, win_h) {
            self.wheel_remainder += scroll_y_lines;
            let steps = self.wheel_remainder.trunc() as isize;
            if steps != 0 {
                self.wheel_remainder -= steps as f32;
                self.spaces_scroll = if steps > 0 {
                    self.spaces_scroll.saturating_sub(steps as usize)
                } else {
                    self.spaces_scroll + steps.unsigned_abs()
                };
                self.clamp_spaces_scroll();
                self.request_redraw();
            }
            return;
        }

        if self.file_explorer.visible && !self.tabs[self.active_tab_index].is_settings {
            let (panel_x, panel_y, panel_w, panel_h) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                self.wheel_remainder += scroll_y_lines * scroll_multiplier;
                let lines = self.wheel_remainder.trunc() as isize;
                if lines != 0 {
                    self.wheel_remainder -= lines as f32;
                    let list_h = panel_h.saturating_sub(
                        crate::explorer::FileExplorer::HEADER_HEIGHT
                            + self.file_explorer.subheader_height(),
                    );
                    let max_lines = (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);
                    self.file_explorer.scroll(lines, max_lines);
                    self.request_redraw();
                }
                return;
            }
        }

        // Scroll en pestaña de visor de archivos (soporte vertical y horizontal)
        if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
            let max_x = if viewer.wrap_active() {
                0
            } else {
                viewer.max_scroll_x()
            };

            let mut changed = false;
            if self.modifiers.shift_key() {
                // Shift + rueda vertical -> scroll horizontal
                self.wheel_remainder_x += scroll_y_lines * scroll_multiplier * 3.0;
            } else {
                self.wheel_remainder += scroll_y_lines * scroll_multiplier;
                self.wheel_remainder_x += scroll_x_lines * scroll_multiplier * 3.0;
            }

            let dy = self.wheel_remainder.trunc() as isize;
            if dy != 0 {
                self.wheel_remainder -= dy as f32;
                if dy > 0 {
                    viewer.scroll_offset = viewer.scroll_offset.saturating_sub(dy.unsigned_abs());
                } else {
                    viewer.scroll_offset = (viewer.scroll_offset + dy.unsigned_abs())
                        .min(viewer.total_display_rows().saturating_sub(1));
                }
                changed = true;
            }

            let dx = self.wheel_remainder_x.trunc() as isize;
            if dx != 0 {
                self.wheel_remainder_x -= dx as f32;
                if dx > 0 {
                    viewer.scroll_offset_x =
                        viewer.scroll_offset_x.saturating_sub(dx.unsigned_abs());
                } else {
                    viewer.scroll_offset_x =
                        (viewer.scroll_offset_x + dx.unsigned_abs()).min(max_x);
                }
                changed = true;
            }

            if changed {
                self.request_redraw();
            }
            return;
        }

        let (cx, cy, _, _) = self.content_area_rect(win_w, win_h);
        let pane_rects = self.active_tab_pane_rects(win_w, win_h);
        let active_tab = &mut self.tabs[self.active_tab_index];

        let target_pane_rect = pane_rects
            .iter()
            .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
            .copied();

        let target_pane_id = target_pane_rect
            .map(|r| r.pane_id)
            .unwrap_or(active_tab.active_pane_id);

        if let Some(pane) = active_tab.find_pane_mut(target_pane_id) {
            let rect_x = target_pane_rect.map(|r| r.x).unwrap_or(cx);
            let rect_y = target_pane_rect.map(|r| r.y).unwrap_or(cy);

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

                    let is_up = scroll_y_lines > 0.0;

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
                // Líneas por muesca según configuración del SO; el resto fraccionario se acumula para que un touchpad
                // (muchos eventos con y < 1) avance al ritmo de sus dedos y no a saltos bruscos
                self.wheel_remainder += scroll_y_lines * scroll_multiplier;
                let lines = self.wheel_remainder.trunc();
                self.wheel_remainder -= lines;
                let lines = lines as isize;

                if lines > 0 {
                    pane.terminal.scroll_up(lines as usize);
                } else if lines < 0 {
                    pane.terminal.scroll_down(lines.unsigned_abs());
                }

                self.last_scroll_time = Instant::now();
                self.request_redraw();
            }
        }
    }
}
