//! Rueda del ratón: scroll del historial, listas y TUIs.

use super::*;

impl AxonApp {
    /// Rueda del ratón: scroll del historial, listas y TUIs.
    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
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

        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();

        // Scroll en la lista de pestañas verticales de la barra lateral
        if !self.is_horizontal_tabs() && self.sidebar_visible && mx < SIDEBAR_WIDTH {
            let scroll_steps = match delta {
                MouseScrollDelta::LineDelta(_, y) => y.round() as isize,
                MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0).round() as isize,
            };
            let tab_item_h = 36;
            let tab_spacing = 4;
            let list_start_y = 42;
            let toggle_h = 30;
            let toggle_y = win_h.saturating_sub(status_bar_h + toggle_h + 8);
            let list_max_y = toggle_y.saturating_sub(8);
            let visible_items = if list_max_y > list_start_y {
                (list_max_y - list_start_y) / (tab_item_h + tab_spacing)
            } else {
                1
            };
            let max_offset = self.tabs.len().saturating_sub(visible_items);

            if scroll_steps > 0 {
                self.sidebar_scroll_offset = self
                    .sidebar_scroll_offset
                    .saturating_sub(scroll_steps as usize);
            } else if scroll_steps < 0 {
                self.sidebar_scroll_offset =
                    (self.sidebar_scroll_offset + scroll_steps.unsigned_abs()).min(max_offset);
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }

        if self.file_explorer.visible && !self.tabs[self.active_tab_index].is_settings {
            let (panel_x, panel_y, panel_w, panel_h) =
                crate::explorer::FileExplorer::panel_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                );
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * 3.0).round() as isize,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0).round() as isize,
                };
                let list_h = panel_h.saturating_sub(
                    crate::explorer::FileExplorer::HEADER_HEIGHT
                        + crate::explorer::FileExplorer::SUBHEADER_HEIGHT,
                );
                let max_lines = (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);
                self.file_explorer.scroll(lines, max_lines);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        }

        let right_offset = self.file_explorer_offset();
        let term_area_w = win_w.saturating_sub(offset + right_offset);
        let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
        let active_tab = &mut self.tabs[self.active_tab_index];
        let pane_rects =
            active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);

        let target_pane_rect = pane_rects
            .iter()
            .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
            .copied();

        let target_pane_id = target_pane_rect
            .map(|r| r.pane_id)
            .unwrap_or(active_tab.active_pane_id);

        if let Some(pane) = active_tab.find_pane_mut(target_pane_id) {
            let rect_x = target_pane_rect.map(|r| r.x).unwrap_or(offset);
            let rect_y = target_pane_rect.map(|r| r.y).unwrap_or(top_offset);

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

                    let is_up = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                        MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                    };

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
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1) * 3,
                    MouseScrollDelta::PixelDelta(pos) => {
                        ((pos.y.abs() / 20.0).ceil() as usize).max(1) * 3
                    }
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
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
        }
    }
}
