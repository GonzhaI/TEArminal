//! Rueda del ratón: scroll del historial, listas y TUIs.

use super::*;

impl TearminalApp {
    /// Rueda del ratón: scroll del historial, listas y TUIs.
    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let bypass = self.modifiers.shift_key();
        let offset = self.sidebar_offset();
        let mx = self.mouse_pos.x as usize;
        let my = self.mouse_pos.y as usize;

        let (win_w, win_h) = self.window_size();
        let scroll_multiplier = platform::wheel_scroll_lines();
        let (scroll_x_lines, scroll_y_lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / 20.0, pos.y as f32 / 20.0),
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
                let rects = SettingsPageRects::compute(
                    offset,
                    top_offset,
                    win_w.saturating_sub(offset + self.panel_right_inset()),
                    win_h.saturating_sub(top_offset),
                );
                let page_y = if my >= top_offset + SettingsPageRects::CONTENT_TOP {
                    my + self.settings_scroll()
                } else {
                    my
                };
                if rects.theme_row_at(mx, page_y).is_some() {
                    let rows = (-scroll_y_lines * scroll_multiplier).round() as isize;
                    self.scroll_theme_list(rows);
                    return;
                }
            }
            let dy = (scroll_y_lines * 48.0).round() as isize;
            let viewport_h =
                win_h.saturating_sub(top_offset + SettingsPageRects::CONTENT_TOP + status_bar_h);
            let max = SettingsPageRects::max_scroll(self.settings_page_state.section, viewport_h);
            self.settings_page_state.scroll_y =
                (self.settings_scroll() as isize - dy).clamp(0, max as isize) as usize;
            self.request_redraw();
            return;
        }

        // Scroll en la lista de pestañas verticales de la barra lateral
        if self.panel_local_x(mx, my, win_w).is_some() {
            let scroll_steps = scroll_y_lines.round() as isize;
            let usable_h = win_h.saturating_sub(status_bar_h);
            let (total, visible) = self.panel_scroll_extent(usable_h);
            let max_offset = total.saturating_sub(visible);

            if scroll_steps > 0 {
                self.sidebar_scroll_offset = self
                    .sidebar_scroll_offset
                    .saturating_sub(scroll_steps as usize);
            } else if scroll_steps < 0 {
                self.sidebar_scroll_offset =
                    (self.sidebar_scroll_offset + scroll_steps.unsigned_abs()).min(max_offset);
            }
            self.request_redraw();
            return;
        }

        if self.file_explorer.visible && !self.tabs[self.active_tab_index].is_settings {
            let (panel_x, panel_y, panel_w, panel_h) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                let lines = (scroll_y_lines * scroll_multiplier).round() as isize;
                let list_h = panel_h.saturating_sub(
                    crate::explorer::FileExplorer::HEADER_HEIGHT
                        + crate::explorer::FileExplorer::SUBHEADER_HEIGHT,
                );
                let max_lines = (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);
                self.file_explorer.scroll(lines, max_lines);
                self.request_redraw();
                return;
            }
        }

        // Scroll en pestaña de visor de archivos (soporte vertical y horizontal)
        if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
            let (dx, dy) = (
                (scroll_x_lines * scroll_multiplier).round() as isize,
                (scroll_y_lines * scroll_multiplier).round() as isize,
            );
            let max_x = if viewer.wrap_active() {
                0
            } else {
                viewer.max_scroll_x()
            };
            if self.modifiers.shift_key() {
                // Shift + rueda vertical -> scroll horizontal
                if dy > 0 {
                    viewer.scroll_offset_x =
                        viewer.scroll_offset_x.saturating_sub(dy.unsigned_abs() * 3);
                } else if dy < 0 {
                    viewer.scroll_offset_x =
                        (viewer.scroll_offset_x + dy.unsigned_abs() * 3).min(max_x);
                }
            } else if dx != 0 {
                // Rueda horizontal nativa: winit da dx > 0 cuando el CONTENIDO se mueve a la
                // derecha (la vista va a la izquierda), igual que dy > 0 sube la vista
                if dx > 0 {
                    viewer.scroll_offset_x =
                        viewer.scroll_offset_x.saturating_sub(dx.unsigned_abs() * 3);
                } else if dx < 0 {
                    viewer.scroll_offset_x =
                        (viewer.scroll_offset_x + dx.unsigned_abs() * 3).min(max_x);
                }
            } else {
                // Scroll vertical estándar
                if dy > 0 {
                    viewer.scroll_offset = viewer.scroll_offset.saturating_sub(dy.unsigned_abs());
                } else if dy < 0 {
                    viewer.scroll_offset = (viewer.scroll_offset + dy.unsigned_abs())
                        .min(viewer.rows().len().saturating_sub(1));
                }
            }
            self.request_redraw();
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
