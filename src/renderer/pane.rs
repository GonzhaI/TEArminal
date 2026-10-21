//! Panel de terminal: celdas, cursor, imágenes, scrollbar y divisores.

use super::*;

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
        let history = terminal.scrollback.len();
        // El desplazamiento de la terminal cuenta desde abajo; la barra, desde arriba.
        let (_, (x, y, w, h)) = ScrollbarRects::vertical(
            rect_x + rect_width,
            rect_y + self.margin_y,
            rect_height.saturating_sub(self.margin_y * 2),
            history + terminal.rows,
            terminal.rows,
            history.saturating_sub(terminal.scroll_offset),
        )?;
        Some((x, y, w, h))
    }

    /// Botón de la cabecera de panel: fondo solo al pasar el ratón, icono atenuado.
    #[allow(clippy::too_many_arguments)]
    fn draw_pane_header_button(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect: (usize, usize, usize, usize),
        icon: crate::ui::LucideIcon,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if hovered {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                x,
                y,
                w,
                h,
                4,
                theme.surface_hover(),
            );
        }
        let fg = if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(buffer, width, height, icon, x + w / 2, y + h / 2, 14.0, fg);
    }

    /// Cabecera de un panel dividido (38 px): icono, título, subtítulo atenuado, acciones
    /// (dividir · maximizar · cerrar) y línea de acento en el panel enfocado.
    #[allow(clippy::too_many_arguments)]
    pub fn render_pane_header(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        slot: (usize, usize, usize),
        title: &str,
        subtitle: Option<&str>,
        icon: crate::ui::LucideIcon,
        icon_color: Color,
        focused: bool,
        zoomed: bool,
        hovered: Option<PaneHeaderButton>,
        theme: &Theme,
    ) {
        let (x, y, w) = slot;
        if w < 120 || width == 0 || height == 0 || y >= height {
            return;
        }
        let rects = PaneHeaderRects::compute(x, y, w);
        let clip: Clip = (
            x,
            y,
            (x + w).min(width),
            (y + PaneHeaderRects::CONTENT_OFFSET).min(height),
        );
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y,
            w,
            PaneHeaderRects::HEIGHT,
            theme.chrome_bg().to_u32_xrgb(),
        );
        // Línea que separa la cabecera del terminal y acento del panel enfocado sobre ella
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y + PaneHeaderRects::HEIGHT,
            w,
            PaneHeaderRects::BORDER,
            theme.chrome_line().to_u32_xrgb(),
        );
        if focused {
            fill_rect(
                buffer,
                width,
                clip,
                x,
                y + PaneHeaderRects::HEIGHT - 2,
                w,
                2,
                theme.accent_color().to_u32_xrgb(),
            );
        }

        let cy = y + PaneHeaderRects::HEIGHT / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            icon,
            x + PaneHeaderRects::ICON_X,
            cy,
            15.0,
            icon_color,
        );

        // El texto no invade los botones que se dibujan en este panel
        let buttons_left = if focused {
            rects.split_btn.0
        } else {
            rects.close_btn.0
        };
        let text_max = buttons_left.saturating_sub(12);
        let baseline = cy + 4;
        let title_end = self.draw_ui_text(
            buffer,
            width,
            height,
            title,
            x + PaneHeaderRects::TEXT_X,
            baseline,
            text_max,
            theme.foreground,
            tokens::TITLE,
            true,
        );
        if let Some(subtitle) = subtitle.filter(|s| !s.is_empty()) {
            let sx = title_end + 10;
            if sx < text_max {
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    subtitle,
                    sx,
                    baseline,
                    text_max,
                    theme.text_muted(),
                    tokens::BODY,
                    false,
                );
            }
        }

        if focused {
            self.draw_pane_header_button(
                buffer,
                width,
                height,
                rects.split_btn,
                crate::ui::LucideIcon::Columns2,
                hovered == Some(PaneHeaderButton::Split),
                theme,
            );
            self.draw_pane_header_button(
                buffer,
                width,
                height,
                rects.zoom_btn,
                if zoomed {
                    crate::ui::LucideIcon::Minimize2
                } else {
                    crate::ui::LucideIcon::Maximize2
                },
                hovered == Some(PaneHeaderButton::Zoom),
                theme,
            );
        }
        self.draw_pane_header_button(
            buffer,
            width,
            height,
            rects.close_btn,
            crate::ui::LucideIcon::X,
            hovered == Some(PaneHeaderButton::Close),
            theme,
        );
    }

    #[allow(clippy::too_many_arguments)]
    /// Renderiza un panel de terminal en un rectángulo específico del framebuffer.
    #[allow(clippy::too_many_arguments)]
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
        _has_multiple_panes: bool,
    ) {
        if rect_w == 0 || rect_h == 0 || width == 0 || height == 0 {
            return;
        }

        let bg_color = theme.background.to_u32_xrgb();
        let baseline_y = self.baseline_y;

        // 1. Limpieza del área del panel con el fondo del tema
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
        let max_cols = max_x.saturating_sub(base_x) / self.char_width;

        let norm_sel = terminal.selection_ordered();

        // Tarjetas de bloque (OSC 133): fondo antes que las celdas, cabecera después.
        let hidden_headers = self.block_header_lines(rect_x, rect_y, rect_w, rect_h, terminal);
        self.draw_block_card_frames(
            buffer, width, height, rect_x, rect_y, rect_w, rect_h, terminal, theme,
        );

        // 2. Renderizar filas y celdas
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let cell_y = base_y + (row_idx * self.row_height);
            if cell_y + self.row_height > max_y {
                break;
            }
            let global_row = view_start + row_idx;
            // La cabecera de la tarjeta sustituye el prompt y el comando de esa fila.
            if hidden_headers.contains(&global_row) {
                continue;
            }
            let cols_in_row = row.len().min(max_cols);
            let row_slice = &row[..cols_in_row];

            let line_used = row
                .iter()
                .rposition(|cell| {
                    !(cell.c == ' '
                        && cell.bg == Color::TRANSPARENT
                        && (cell.flags
                            & (Cell::FLAG_UNDERLINE
                                | Cell::FLAG_STRIKETHROUGH
                                | Cell::FLAG_INVERSE))
                            == 0)
                })
                .map_or(0, |i| i + 1);

            let row_sel = norm_sel.and_then(|(p1, p2)| {
                if global_row < p1.1 || global_row > p2.1 {
                    None
                } else if p1.1 == p2.1 {
                    if p1.0 >= line_used && p2.0 >= line_used {
                        None
                    } else {
                        let sc = p1.0.min(line_used);
                        let ec = p2.0.min(line_used.saturating_sub(1));
                        if sc <= ec {
                            Some((sc, ec))
                        } else {
                            None
                        }
                    }
                } else {
                    let sc = if global_row == p1.1 {
                        p1.0.min(line_used)
                    } else {
                        0
                    };
                    let ec = if global_row == p2.1 {
                        p2.0.min(line_used.saturating_sub(1))
                    } else {
                        line_used.max(sc).min(cols_in_row.saturating_sub(1))
                    };
                    if sc <= ec {
                        Some((sc, ec))
                    } else {
                        None
                    }
                }
            });

            // PASADA 1: Dibujar fondos de celda y selecciones fusionando tramos horizontales idénticos
            let mut run_start = 0usize;
            let mut run_color: Option<u32> = None;
            for (col_idx, cell) in row_slice.iter().enumerate() {
                let is_selected = row_sel.is_some_and(|(sc, ec)| col_idx >= sc && col_idx <= ec);

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

                let cell_bg_u32 =
                    (effective_bg != theme.background).then(|| effective_bg.to_u32_xrgb());

                if cell_bg_u32 != run_color {
                    if let Some(c_u32) = run_color {
                        fill_rect(
                            buffer,
                            width,
                            clip,
                            base_x + run_start * self.char_width,
                            cell_y,
                            (col_idx - run_start) * self.char_width,
                            self.row_height,
                            c_u32,
                        );
                    }
                    run_start = col_idx;
                    run_color = cell_bg_u32;
                }
            }
            if let Some(c_u32) = run_color {
                fill_rect(
                    buffer,
                    width,
                    clip,
                    base_x + run_start * self.char_width,
                    cell_y,
                    (cols_in_row - run_start) * self.char_width,
                    self.row_height,
                    c_u32,
                );
            }

            // PASADA 2: Dibujar decoraciones y rasterizar/mezclar glifos (con soporte para ligaduras tipográficas)
            let is_cursor_row = terminal.scroll_offset == 0
                && terminal.cursor_visible
                && cursor_visible
                && terminal.cursor_y == row_idx;

            let mut col_idx = 0;
            while col_idx < cols_in_row {
                let cell = &row_slice[col_idx];

                // Espaciador de carácter ancho o espacio sin decoración: nada más que pintar
                let decorated = cell.underline() || cell.strikethrough();
                if cell.width == 0 || (cell.c == ' ' && !decorated) {
                    col_idx += 1;
                    continue;
                }

                let cell_x = base_x + (col_idx * self.char_width);
                let fg_color = if row_sel.is_some_and(|(sc, ec)| col_idx >= sc && col_idx <= ec) {
                    theme.selection_fg
                } else {
                    compute_cell_fg(cell, theme)
                };

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
                    col_idx += 1;
                    continue;
                }

                // Obtener y mezclar glifo (con soporte para clústeres de grafemas y Nerd Fonts)
                if cell.is_grapheme() {
                    if let Some(g) = terminal.grapheme_of(cell) {
                        for ch in g.chars() {
                            let glyph = self.get_glyph(ch, cell.bold());
                            let glyph_x = cell_x as i32 + glyph.x_offset;
                            let glyph_y =
                                (cell_y + baseline_y) as i32 - glyph.y_offset - glyph.height as i32;
                            blit_glyph(buffer, width, clip, glyph, glyph_x, glyph_y, fg_color);
                        }
                    }
                    col_idx += 1;
                    continue;
                }

                // Ligaduras tipográficas (operadores y puntuación de programación: ->, !=, ==, ===, =>, etc.)
                if self.ligatures
                    && is_ligature_candidate(cell.c)
                    && (!is_cursor_row || col_idx != terminal.cursor_x)
                {
                    let mut run_len = 1usize;
                    let mut p_buf = [0u8; 8];
                    p_buf[0] = cell.c as u8;

                    while run_len < 8 && col_idx + run_len < cols_in_row {
                        let next_col = col_idx + run_len;
                        if is_cursor_row && next_col == terminal.cursor_x {
                            break;
                        }
                        let next_cell = &row_slice[next_col];
                        if next_cell.width != 1
                            || !is_ligature_candidate(next_cell.c)
                            || next_cell.bold() != cell.bold()
                        {
                            break;
                        }
                        let next_fg =
                            if row_sel.is_some_and(|(sc, ec)| next_col >= sc && next_col <= ec) {
                                theme.selection_fg
                            } else {
                                compute_cell_fg(next_cell, theme)
                            };
                        if next_fg != fg_color {
                            break;
                        }
                        p_buf[run_len] = next_cell.c as u8;
                        run_len += 1;
                    }

                    if run_len >= 2 {
                        let shaped = self.shape_punctuation_run(&p_buf[..run_len], cell.bold());
                        for k in 0..run_len {
                            let cur_col = col_idx + k;
                            let cur_cell_x = base_x + (cur_col * self.char_width);
                            let cur_cell = &row_slice[cur_col];

                            if k > 0 {
                                if cur_cell.underline() {
                                    let line_y = cell_y + self.row_height - 2;
                                    fill_rect(
                                        buffer,
                                        width,
                                        clip,
                                        cur_cell_x,
                                        line_y,
                                        self.char_width,
                                        1,
                                        fg_color.to_u32_xrgb(),
                                    );
                                }
                                if cur_cell.strikethrough() {
                                    let line_y = cell_y + (self.row_height / 2);
                                    fill_rect(
                                        buffer,
                                        width,
                                        clip,
                                        cur_cell_x,
                                        line_y,
                                        self.char_width,
                                        1,
                                        fg_color.to_u32_xrgb(),
                                    );
                                }
                            }

                            let gid = shaped.gids[k];
                            let glyph = self.get_ligature_glyph(gid, cell.bold());
                            if glyph.width > 0 {
                                let glyph_x = cur_cell_x as i32 + glyph.x_offset;
                                let glyph_y = (cell_y + baseline_y) as i32
                                    - glyph.y_offset
                                    - glyph.height as i32;
                                blit_glyph(buffer, width, clip, glyph, glyph_x, glyph_y, fg_color);
                            }
                        }
                        col_idx += run_len;
                        continue;
                    }
                }

                let glyph = self.get_glyph(cell.c, cell.bold());
                let glyph_x = cell_x as i32 + glyph.x_offset;
                let glyph_y = (cell_y + baseline_y) as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, glyph_x, glyph_y, fg_color);
                col_idx += 1;
            }
        }

        // 3. Canal de bloques de comando (OSC 133): tarjetas con cabecera, estado y acciones.
        self.draw_block_card_chrome(
            buffer, width, height, rect_x, rect_y, rect_w, rect_h, terminal, theme,
        );

        // 4. Renderizar cursor sólido
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
                        if cell.is_grapheme() {
                            if let Some(g) = terminal.grapheme_of(cell) {
                                for ch in g.chars() {
                                    let glyph = self.get_glyph(ch, cell.bold());
                                    let glyph_x = cx as i32 + glyph.x_offset;
                                    let glyph_y = (cy + baseline_y) as i32
                                        - glyph.y_offset
                                        - glyph.height as i32;
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

        // 5. Renderizar imágenes inline (Protocolo iTerm2 / OSC 1337)
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

        // 6. Scrollbar del panel si tiene historial
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

        // 7. Foco: la cabecera de cada panel (arriba) marca el panel activo con su línea de acento.
        //    El contenido no lleva marco para no competir con la rejilla del terminal.

        // 8. Botón "ir hasta abajo" mientras la vista está desplazada.
        if terminal.scroll_offset > 0 && !terminal.is_alt_screen {
            let (btn_x, btn_y, btn_w, btn_h) =
                crate::renderer::pane_scroll_to_bottom_btn_rect(rect_x, rect_y, rect_w, rect_h);
            if btn_x + btn_w <= width && btn_y + btn_h <= height {
                let bg_color = if terminal.scroll_to_bottom_hovered {
                    theme.surface_hover()
                } else {
                    theme.chrome_bg_elevated()
                };
                let border_color = theme.chrome_line();
                let icon_color = if terminal.scroll_to_bottom_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };

                crate::ui::draw_rounded_rect(
                    buffer, width, height, btn_x, btn_y, btn_w, btn_h, 4, bg_color,
                );
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    btn_x,
                    btn_y,
                    btn_w,
                    btn_h,
                    4,
                    border_color,
                );
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::ChevronDown,
                    btn_x + btn_w / 2,
                    btn_y + btn_h / 2,
                    13.0,
                    icon_color,
                );
            }
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
