//! Panel de terminal: celdas, cursor, imágenes, scrollbar y divisores.

use super::*;

pub(super) fn is_cell_selected(col: usize, row: usize, sel: SelectionRange) -> bool {
    let (mut p1, mut p2) = (sel.start, sel.end);
    if p1.1 > p2.1 || (p1.1 == p2.1 && p1.0 > p2.0) {
        std::mem::swap(&mut p1, &mut p2);
    }
    row >= p1.1 && row <= p2.1 && (row != p1.1 || col >= p1.0) && (row != p2.1 || col <= p2.0)
}

pub(super) fn compute_cell_fg(cell: &Cell, theme: &Theme) -> Color {
    let base = if cell.inverse() {
        if cell.bg != Color::TRANSPARENT {
            cell.bg
        } else {
            theme.background
        }
    } else if cell.fg != Color::TRANSPARENT {
        // Garantizar contraste mínimo contra el fondo de la terminal
        let dr = (cell.fg.r as i32 - theme.background.r as i32).abs();
        let dg = (cell.fg.g as i32 - theme.background.g as i32).abs();
        let db = (cell.fg.b as i32 - theme.background.b as i32).abs();
        if dr + dg + db < 18 {
            theme.foreground
        } else {
            cell.fg
        }
    } else {
        theme.foreground
    };

    if cell.dim() {
        Color::from_rgb(
            (base.r as f32 * 0.65) as u8,
            (base.g as f32 * 0.65) as u8,
            (base.b as f32 * 0.65) as u8,
        )
    } else {
        base
    }
}

impl TerminalRenderer {
    pub fn pane_scrollbar_geometry(
        &self,
        rect_x: usize,
        rect_y: usize,
        rect_width: usize,
        rect_height: usize,
        terminal: &TerminalState,
    ) -> Option<(usize, usize, usize, usize)> {
        if terminal.scrollback.is_empty() {
            return None;
        }
        let track_w = 6;
        let track_x = (rect_x + rect_width).saturating_sub(track_w + 3);
        let track_y = rect_y + self.margin_y;
        let track_h = rect_height.saturating_sub(self.margin_y * 2);
        if track_h == 0 {
            return None;
        }

        let total_history = terminal.scrollback.len() + terminal.rows;
        let thumb_h = ((terminal.rows as f32 / total_history as f32) * track_h as f32)
            .max(18.0)
            .min(track_h as f32) as usize;
        let available_travel = track_h.saturating_sub(thumb_h);
        let ratio = 1.0 - (terminal.scroll_offset as f32 / terminal.scrollback.len() as f32);
        let thumb_y = track_y + (ratio * available_travel as f32) as usize;

        Some((track_x, thumb_y, track_w, thumb_h))
    }

    /// Renderiza un panel de terminal en un rectángulo específico del framebuffer
    pub fn render_pane(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
        theme: &Theme,
        cursor_visible: bool,
        is_active_pane: bool,
        has_multiple_panes: bool,
    ) {
        if rect_w == 0 || rect_h == 0 || width == 0 || height == 0 {
            return;
        }

        let bg_color = theme.background.to_u32_xrgb();
        let baseline_y = self.baseline_y;

        // 1. Limpieza del área del panel con el fondo oficial Ghostty (#161618)
        let max_y = (rect_y + rect_h).min(height);
        let max_x = (rect_x + rect_w).min(width);
        let clip: Clip = (rect_x, rect_y, max_x, max_y);
        fill_rect(
            buffer, width, clip, rect_x, rect_y, rect_w, rect_h, bg_color,
        );

        let base_x = rect_x + self.margin_x;
        let base_y = rect_y + self.margin_y;
        let visible_rows = terminal.visible_rows();
        let view_start = terminal.visible_line_start();

        // 2. Renderizar filas y celdas
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let cell_y = base_y + (row_idx * self.row_height);
            if cell_y + self.row_height > max_y {
                break;
            }

            // PASADA 1: Dibujar todos los fondos de celda y selecciones
            for (col_idx, cell) in row.iter().enumerate() {
                let cell_x = base_x + (col_idx * self.char_width);
                if cell_x + self.char_width > max_x {
                    break;
                }

                let is_selected = if let Some(sel) = terminal.selection {
                    is_cell_selected(col_idx, row_idx, sel)
                } else {
                    false
                };

                let effective_bg = if is_selected {
                    theme.selection
                } else if cell.inverse() {
                    if cell.fg != Color::TRANSPARENT {
                        cell.fg
                    } else {
                        theme.foreground
                    }
                } else if cell.bg != Color::TRANSPARENT {
                    cell.bg
                } else {
                    theme.background
                };

                // Dibujar fondo de celda si difiere del fondo global
                if effective_bg != theme.background {
                    fill_rect(
                        buffer,
                        width,
                        clip,
                        cell_x,
                        cell_y,
                        self.char_width,
                        self.row_height,
                        effective_bg.to_u32_xrgb(),
                    );
                }
            }

            // PASADA 2: Dibujar decoraciones y rasterizar/mezclar glifos
            for (col_idx, cell) in row.iter().enumerate() {
                let cell_x = base_x + (col_idx * self.char_width);
                if cell_x + self.char_width > max_x {
                    break;
                }

                // Espaciador de carácter ancho o espacio sin decoración: nada más que pintar
                let decorated = cell.underline() || cell.strikethrough();
                if cell.width == 0 || (cell.c == ' ' && !decorated) {
                    continue;
                }

                let fg_color = compute_cell_fg(cell, theme);

                // Subrayado (SGR 4) y tachado (SGR 9)
                if cell.underline() {
                    let line_y = cell_y + self.row_height - 2;
                    fill_rect(
                        buffer,
                        width,
                        clip,
                        cell_x,
                        line_y,
                        self.char_width,
                        1,
                        fg_color.to_u32_xrgb(),
                    );
                }
                if cell.strikethrough() {
                    let line_y = cell_y + (self.row_height / 2);
                    fill_rect(
                        buffer,
                        width,
                        clip,
                        cell_x,
                        line_y,
                        self.char_width,
                        1,
                        fg_color.to_u32_xrgb(),
                    );
                }

                if cell.c == ' ' {
                    continue;
                }

                // Obtener y mezclar glifo (con soporte completo de Nerd Fonts)
                let glyph = self.get_glyph(cell.c, cell.bold());
                let glyph_x = cell_x as i32 + glyph.x_offset;
                let glyph_y = (cell_y + baseline_y) as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, glyph_x, glyph_y, fg_color);
            }
        }

        // 3. Renderizar cursor sólido al estilo Ghostty
        if terminal.scroll_offset == 0
            && terminal.cursor_visible
            && cursor_visible
            && terminal.cursor_y < terminal.rows
            && terminal.cursor_x < terminal.cols
        {
            let cx = base_x + (terminal.cursor_x * self.char_width);
            let cy = base_y + (terminal.cursor_y * self.row_height);

            let active_grid = if terminal.is_alt_screen {
                &terminal.alt_grid
            } else {
                &terminal.grid
            };

            let cell_under = if terminal.cursor_y < active_grid.len()
                && terminal.cursor_x < active_grid[terminal.cursor_y].len()
            {
                Some(&active_grid[terminal.cursor_y][terminal.cursor_x])
            } else {
                None
            };

            let c_width = if let Some(cell) = cell_under {
                if cell.width == 2 {
                    self.char_width * 2
                } else {
                    self.char_width
                }
            } else {
                self.char_width
            };

            if is_active_pane {
                fill_rect(
                    buffer,
                    width,
                    clip,
                    cx,
                    cy,
                    c_width,
                    self.row_height,
                    theme.cursor.to_u32_xrgb(),
                );

                // Dibujar carácter invertido debajo del cursor para legibilidad perfecta
                if let Some(cell) = cell_under {
                    if cell.c != ' ' && cell.width > 0 {
                        let glyph = self.get_glyph(cell.c, cell.bold());
                        let glyph_x = cx as i32 + glyph.x_offset;
                        let glyph_y =
                            (cy + baseline_y) as i32 - glyph.y_offset - glyph.height as i32;
                        blit_glyph(
                            buffer,
                            width,
                            clip,
                            glyph,
                            glyph_x,
                            glyph_y,
                            theme.background,
                        );
                    }
                }
            } else {
                // Cursor hueco para terminal inactiva en split
                let border_c = theme.ansi[8].to_u32_xrgb();
                let rh = self.row_height;
                fill_rect(buffer, width, clip, cx, cy, c_width, 1, border_c);
                fill_rect(buffer, width, clip, cx, cy + rh - 1, c_width, 1, border_c);
                fill_rect(buffer, width, clip, cx, cy, 1, rh, border_c);
                fill_rect(buffer, width, clip, cx + c_width - 1, cy, 1, rh, border_c);
            }
        }

        // 4. Renderizar imágenes inline (Protocolo iTerm2 / OSC 1337)
        let view_end = view_start + visible_rows.len();
        for img in &terminal.images {
            if img.line_index + img.height_rows > view_start && img.line_index < view_end {
                let row_in_view = img.line_index as isize - view_start as isize;
                let ix = (base_x + (img.col * self.char_width)) as i32;
                let iy = (base_y + (row_in_view as usize * self.row_height)) as i32;
                let target_w = img.width_cols * self.char_width;
                let target_h = img.height_rows * self.row_height;

                for dy in 0..target_h {
                    let py = iy + dy as i32;
                    if py < rect_y as i32 || py >= max_y as i32 {
                        continue;
                    }
                    let src_y = (dy * img.height_px as usize) / target_h;

                    for dx in 0..target_w {
                        let px = ix + dx as i32;
                        if px < rect_x as i32 || px >= max_x as i32 {
                            continue;
                        }
                        let src_x = (dx * img.width_px as usize) / target_w;

                        let src_idx = (src_y * img.width_px as usize + src_x) * 4;
                        if src_idx + 3 < img.rgba_data.len() {
                            let r = img.rgba_data[src_idx];
                            let g = img.rgba_data[src_idx + 1];
                            let b = img.rgba_data[src_idx + 2];
                            let a = img.rgba_data[src_idx + 3];

                            let buf_idx = (py as usize) * width + (px as usize);
                            if a == 255 {
                                buffer[buf_idx] =
                                    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                            } else if a > 0 {
                                let bg = Color::from_rgb(
                                    ((buffer[buf_idx] >> 16) & 0xFF) as u8,
                                    ((buffer[buf_idx] >> 8) & 0xFF) as u8,
                                    (buffer[buf_idx] & 0xFF) as u8,
                                );
                                buffer[buf_idx] = bg.blend_over(Color::from_rgb(r, g, b), a);
                            }
                        }
                    }
                }
            }
        }

        // 5. Scrollbar del panel si tiene historial
        if !terminal.scrollback.is_empty() {
            if let Some((thumb_x, thumb_y, thumb_w, thumb_h)) =
                self.pane_scrollbar_geometry(rect_x, rect_y, rect_w, rect_h, terminal)
            {
                let track_y = rect_y + self.margin_y;
                let track_h = rect_h.saturating_sub(self.margin_y * 2);

                Self::draw_scrollbar(
                    buffer,
                    width,
                    height,
                    thumb_x,
                    track_y,
                    thumb_w,
                    track_h,
                    thumb_y,
                    thumb_h,
                    terminal.scrollbar_hovered,
                    self.scrollbars_awake,
                    theme,
                );
            }
        }

        // 6. Si hay múltiples paneles, dibujar un borde sutil indicador en el panel activo
        if has_multiple_panes && is_active_pane {
            let active_border = theme.ansi[2]; // Verde Ghostty sutil
                                               // Línea superior fina indicadora de foco
            Self::draw_rect(
                buffer,
                width,
                height,
                rect_x,
                rect_y,
                rect_w,
                2,
                active_border,
            );
        }
    }

    /// Renderiza un divisor entre paneles de terminal
    pub fn render_pane_divider(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        divider_color: Color,
    ) {
        Self::draw_rect(buffer, width, height, x, y, w, h, divider_color);
    }

    /// Renderiza directamente la cuadrícula del terminal al framebuffer de píxeles 32-bit de Windows
    #[cfg(test)]
    pub fn render(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_offset: usize,
        terminal: &TerminalState,
        theme: &Theme,
        cursor_visible: bool,
        _scrollbar_visible: bool,
        _scrollbar_hovered: bool,
    ) {
        let pane_w = width.saturating_sub(sidebar_offset);
        self.render_pane(
            buffer,
            width,
            height,
            sidebar_offset,
            0,
            pane_w,
            height,
            terminal,
            theme,
            cursor_visible,
            true,
            false,
        );
    }
}
