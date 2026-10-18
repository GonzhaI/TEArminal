//! Renderizado del visor de archivos y código en pestaña dedicada (File Viewer).

use super::*;
use crate::app::FileViewerState;

impl TerminalRenderer {
    /// Renderiza el visor de archivos/código en el área del terminal sin procesos ConPTY
    pub fn render_file_viewer(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        right_inset: usize,
        cursor_visible: bool,
        viewer: &FileViewerState,
        theme: &Theme,
    ) {
        if w == 0 || h == 0 || width == 0 || height == 0 {
            return;
        }

        // Fondo y cabecera ocupan todo el ancho (también bajo el explorador flotante, sin corte abrupto);
        // texto, controles y barras de scroll se quedan en el área libre de `w - right_inset`.
        let full_w = w;
        let w = w.saturating_sub(right_inset);
        let clip: Clip = (x, y, (x + full_w).min(width), (y + h).min(height));

        // 1. Fondo completo del área de trabajo
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y,
            full_w,
            h,
            theme.background.to_u32_xrgb(),
        );

        // 2. Barra de encabezado superior (38px)
        let header_h = 38;
        let header_bg = theme.surface_bg();
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y,
            full_w,
            header_h,
            header_bg.to_u32_xrgb(),
        );

        // Línea divisoria inferior del encabezado
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y + header_h - 1,
            full_w,
            1,
            theme.border_color().to_u32_xrgb(),
        );

        // Icono representativo del tipo de archivo
        let ext = viewer
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let file_icon = match ext.as_str() {
            "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "cs" | "html" | "css" => {
                crate::ui::LucideIcon::FileCode
            }
            "toml" | "json" | "yaml" | "yml" => crate::ui::LucideIcon::FileJson,
            "md" | "txt" | "log" => crate::ui::LucideIcon::FileText,
            _ => crate::ui::LucideIcon::File,
        };

        let header_center_y = y + (header_h / 2);
        let header_base = header_center_y + 4;

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            file_icon,
            x + 22,
            header_center_y,
            13.0,
            theme.accent_color(),
        );

        // Botones (modo y vista previa), chips informativos y, con el espacio sobrante, el nombre del archivo
        let is_markdown = ext == "md" || ext == "markdown";
        let header_rects = crate::renderer::geometry::FileViewerHeaderRects::compute(
            x,
            y,
            w,
            h,
            is_markdown,
            viewer.git_base.is_some(),
        );
        let accent = theme.accent_color();
        let chip_bg = Color::from_rgba(255, 255, 255, 8);
        let tint = |c: Color, a: u8| Color::from_rgba(c.r(), c.g(), c.b(), a);

        if let Some(rect) = header_rects.md_btn {
            let hov = viewer.hovered_md_btn;
            let (icon, label, fg, bg) = if viewer.render_markdown {
                (
                    crate::ui::LucideIcon::Eye,
                    "Vista previa",
                    accent,
                    tint(accent, if hov { 40 } else { 20 }),
                )
            } else {
                (
                    crate::ui::LucideIcon::Code,
                    "Código",
                    if hov { theme.foreground } else { theme.ansi[7] },
                    if hov { theme.surface_hover() } else { chip_bg },
                )
            };
            self.draw_header_button(
                buffer,
                width,
                height,
                rect,
                icon,
                label,
                fg,
                bg,
                header_base,
            );
        }

        if let Some(rect) = header_rects.diff_btn {
            let hov = viewer.hovered_diff_btn;
            let changed = viewer.has_git_changes();
            let showing = viewer.is_diff();
            let color = if showing {
                accent
            } else if changed {
                theme.ansi[3]
            } else {
                theme.ansi[8]
            };
            self.draw_header_button(
                buffer,
                width,
                height,
                rect,
                crate::ui::LucideIcon::GitBranch,
                "Cambios",
                if hov { accent } else { color },
                if hov {
                    theme.surface_hover()
                } else {
                    tint(color, if showing || changed { 20 } else { 8 })
                },
                header_base,
            );
        }

        let hov = viewer.hovered_mode_btn;
        let (icon, label, fg, bg) = if viewer.is_read_only {
            (
                crate::ui::LucideIcon::Lock,
                "Lectura",
                if hov { accent } else { theme.ansi[8] },
                if hov { theme.surface_hover() } else { chip_bg },
            )
        } else {
            (
                crate::ui::LucideIcon::Pencil,
                "Edición",
                theme.ansi[2],
                tint(theme.ansi[2], if hov { 40 } else { 20 }),
            )
        };
        self.draw_header_button(
            buffer,
            width,
            height,
            header_rects.mode_btn,
            icon,
            label,
            fg,
            bg,
            header_base,
        );

        // Chips en orden de prioridad (de derecha a izquierda); los que no caben junto al nombre se omiten
        let size_text = if viewer.file_size < 1024 {
            format!("{} B", viewer.file_size)
        } else if viewer.file_size < 1024 * 1024 {
            format!("{:.1} KB", viewer.file_size as f64 / 1024.0)
        } else {
            format!("{:.2} MB", viewer.file_size as f64 / (1024.0 * 1024.0))
        };
        let mut chips: Vec<(String, Color, Color)> = Vec::new();
        if viewer.save_error.is_some() {
            chips.push((
                "ERROR AL GUARDAR".into(),
                theme.ansi[1],
                tint(theme.ansi[1], 24),
            ));
        }
        if viewer.non_utf8 {
            chips.push(("NO UTF-8".into(), theme.ansi[3], tint(theme.ansi[3], 24)));
        }
        if viewer.is_dirty {
            chips.push(("MODIFICADO".into(), theme.ansi[3], tint(theme.ansi[3], 24)));
        }
        if !viewer.is_binary {
            chips.push((
                format!(
                    "Ln {}, Col {}",
                    viewer.cursor_line + 1,
                    viewer.cursor_col + 1
                ),
                theme.ansi[7],
                chip_bg,
            ));
            if let Some(s) = viewer.selection_stats() {
                chips.push((
                    if s.lines > 1 {
                        format!("{} sel. · {} líneas", s.chars, s.lines)
                    } else {
                        format!("{} sel.", s.chars)
                    },
                    accent,
                    tint(accent, 24),
                ));
            }
        }
        chips.push((size_text, theme.ansi[7], chip_bg));
        chips.push((
            format!("{} líneas", viewer.lines.len()),
            theme.ansi[7],
            chip_bg,
        ));

        let name_start_x = x + 38;
        let name_end_x = name_start_x + self.measure_ui_text(&viewer.file_name, 12.5, true);
        let mut right_x = header_rects.mode_btn.0;
        for r in [header_rects.md_btn, header_rects.diff_btn]
            .into_iter()
            .flatten()
        {
            right_x = right_x.min(r.0);
        }
        right_x = right_x.saturating_sub(8);
        for (text, chip_fg, bg) in &chips {
            let chip_w = self.measure_ui_text(text, 10.5, false) + 14;
            let chip_x = right_x.saturating_sub(chip_w);
            if chip_x < name_end_x + 16 {
                break;
            }
            crate::ui::draw_rounded_rect(buffer, width, height, chip_x, y + 8, chip_w, 22, 4, *bg);
            self.draw_ui_text(
                buffer,
                width,
                height,
                text,
                chip_x + 7,
                header_base - 1,
                right_x,
                *chip_fg,
                10.5,
                false,
            );
            right_x = chip_x.saturating_sub(8);
        }

        // Solo el nombre del archivo: la ruta completa ya se muestra en la barra de estado
        self.draw_ui_text(
            buffer,
            width,
            height,
            &viewer.file_name,
            name_start_x,
            header_base,
            right_x + 4,
            theme.foreground,
            12.5,
            true,
        );

        // 3. Área de contenido del archivo
        let content_y = y + header_h;
        let content_h = h.saturating_sub(header_h);
        if content_h == 0 {
            return;
        }

        // Si es archivo binario o no visualizable:
        if viewer.is_binary {
            let msg = viewer
                .lines
                .first()
                .map(|s| s.as_str())
                .unwrap_or("Archivo binario");
            let card_w = 420.min(w.saturating_sub(40));
            let card_h = 100;
            let card_x = x + (w.saturating_sub(card_w)) / 2;
            let card_y = content_y + (content_h.saturating_sub(card_h)) / 3;

            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                card_x,
                card_y,
                card_w,
                card_h,
                10,
                theme.surface_bg(),
            );
            crate::ui::stroke_rounded_rect(
                buffer,
                width,
                height,
                card_x,
                card_y,
                card_w,
                card_h,
                10,
                theme.border_color(),
            );

            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Info,
                card_x + 36,
                card_y + 50,
                20.0,
                theme.accent_color(),
            );

            self.draw_ui_text(
                buffer,
                width,
                height,
                "Vista previa no disponible",
                card_x + 64,
                card_y + 42,
                card_x + card_w.saturating_sub(20),
                theme.foreground,
                13.0,
                true,
            );

            self.draw_ui_text(
                buffer,
                width,
                height,
                msg,
                card_x + 64,
                card_y + 64,
                card_x + card_w.saturating_sub(20),
                theme.ansi[8],
                11.0,
                false,
            );
            return;
        }

        let total_lines = viewer.lines.len();
        let lay = crate::renderer::geometry::FileViewerLayout::compute(
            x,
            y,
            w,
            h,
            self.row_height,
            self.char_width,
            total_lines,
        );
        let (row_h, char_w, gutter_w, sep_x) = (lay.row_h, lay.char_w, lay.gutter_w, lay.sep_x);
        let visible_lines = content_h / row_h;
        let max_scroll_x = viewer.max_scroll_x();
        let scroll_x = viewer.scroll_offset_x.min(max_scroll_x);

        // Fondo sutil del gutter
        fill_rect(
            buffer,
            width,
            clip,
            x,
            content_y,
            gutter_w,
            content_h,
            theme.surface_bg().to_u32_xrgb(),
        );

        // Línea vertical separadora del gutter
        fill_rect(
            buffer,
            width,
            clip,
            sep_x,
            content_y,
            1,
            content_h,
            theme.border_color().to_u32_xrgb(),
        );

        let baseline_offset = ((row_h as f32 * 0.76) as usize).max(char_w);

        let gutter_clip: Clip = (x, content_y, sep_x, content_y + content_h);
        let text_max_x = lay.text_max_x;
        let text_clip: Clip = (sep_x + 1, content_y, text_max_x, content_y + content_h);

        let md_blocks = (viewer.render_markdown && is_markdown).then(|| {
            if let Some((ref path, gen, ref blocks)) = self.md_blocks_cache {
                if gen == viewer.edit_gen && path == &viewer.path {
                    return blocks.clone();
                }
            }
            let blocks =
                std::rc::Rc::new(crate::renderer::md_preview::classify_blocks(&viewer.lines));
            self.md_blocks_cache = Some((viewer.path.clone(), viewer.edit_gen, blocks.clone()));
            blocks
        });

        // 4. Renderizado por filas visuales (ajuste de línea y plegado ya resueltos en `viewer.rows()`)
        let rows = viewer.rows();
        let total_rows = rows.len();
        let wrap = viewer.wrap_active();
        let scroll_x = if wrap { 0 } else { scroll_x };
        let start_row = viewer.scroll_offset.min(total_rows.saturating_sub(1));
        let selection = viewer.selection_ordered().filter(|(a, b)| a != b);
        let accent = theme.accent_color();
        let sel_bg = Color::from_rgba(accent.r(), accent.g(), accent.b(), 70);
        let mark_color = theme.ansi[8].to_u32_xrgb();
        let tab_adv = (char_w * 4) as i32;

        for (row_idx, row) in rows
            .iter()
            .enumerate()
            .skip(start_row)
            .take(visible_lines + 1)
        {
            let row_y = content_y + (row_idx - start_row) * row_h;
            if row_y + row_h > content_y + content_h {
                break;
            }
            let line_idx = row.line;
            let first_segment = row.start == 0;
            let is_last_segment = row.end == usize::MAX;
            let is_cursor_line = !viewer.is_binary && line_idx == viewer.cursor_line;
            let baseline_y = row_y + baseline_offset;
            let seg = viewer.row_text(row);
            let cur_x = (sep_x + 14) as i32 - (scroll_x * char_w) as i32;

            // Indicador de Git en el gutter si la línea fue modificada, añadida o eliminada
            if let Some(git_status) = viewer.get_line_git_diff_status(line_idx) {
                let git_color = match git_status {
                    crate::app::GitLineStatus::Added => theme.ansi[2], // Verde
                    crate::app::GitLineStatus::Modified => theme.ansi[3], // Amarillo
                    crate::app::GitLineStatus::Deleted => theme.ansi[1], // Rojo
                };
                if git_status == crate::app::GitLineStatus::Deleted {
                    fill_rect(
                        buffer,
                        width,
                        gutter_clip,
                        sep_x.saturating_sub(4),
                        row_y,
                        4,
                        3,
                        git_color.to_u32_xrgb(),
                    );
                } else {
                    fill_rect(
                        buffer,
                        width,
                        gutter_clip,
                        sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        git_color.to_u32_xrgb(),
                    );
                }
            }

            // Número de línea (solo en la primera fila de cada línea) y flecha de plegado
            if first_segment {
                let mut num_buf = [0u8; 20];
                let mut n = line_idx + 1;
                let mut pos = num_buf.len();
                while n > 0 {
                    pos -= 1;
                    num_buf[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                let digits = &num_buf[pos..];
                let num_w = digits.len() * char_w;
                let num_x = sep_x.saturating_sub(num_w + 10);
                let num_fg = if is_cursor_line {
                    theme.foreground
                } else {
                    theme.ansi[8]
                };
                for (ci, &b) in digits.iter().enumerate() {
                    let glyph = self.get_glyph(b as char, false);
                    let gx = (num_x + ci * char_w) as i32 + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, gutter_clip, glyph, gx, gy, num_fg);
                }

                let folded = viewer.folded.contains(&line_idx);
                if (folded || viewer.hovered_gutter)
                    && (folded || viewer.fold_end(line_idx).is_some())
                {
                    let icon = if folded {
                        crate::ui::LucideIcon::ChevronRight
                    } else {
                        crate::ui::LucideIcon::ChevronDown
                    };
                    let color = if folded { accent } else { theme.ansi[8] };
                    self.draw_lucide_icon(
                        buffer,
                        width,
                        height,
                        icon,
                        x + 10,
                        row_y + row_h / 2,
                        11.0,
                        color,
                    );
                }
            }

            // Diff: fondo verde/rojo en líneas añadidas/quitadas y tenue en el encabezado de tramo
            if viewer.is_diff() {
                let tone = match viewer.lines[line_idx].as_bytes().first() {
                    Some(b'+') => Some(theme.ansi[2]),
                    Some(b'-') => Some(theme.ansi[1]),
                    Some(b'@') => Some(accent),
                    _ => None,
                };
                if let Some(c) = tone {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        sep_x + 1,
                        row_y,
                        text_max_x.saturating_sub(sep_x + 1),
                        row_h,
                        0,
                        Color::from_rgba(c.r(), c.g(), c.b(), 36),
                    );
                }
            }

            // Selección: rectángulo sin bordes redondeados (filas contiguas sin huecos) y marcas de
            // espacios y tabuladores dentro de lo seleccionado para poder contar la sangría
            let mut marks: Vec<(i32, bool)> = Vec::new();
            if let Some(((l1, c1), (l2, c2))) = selection {
                if line_idx >= l1 && line_idx <= l2 {
                    let mut acc = cur_x;
                    let mut sx: Option<i32> = None;
                    let mut ex = cur_x;
                    for (ci, ch) in seg.chars().enumerate() {
                        let col = row.start + ci;
                        let adv = if ch == '\t' { tab_adv } else { char_w as i32 };
                        if (line_idx > l1 || col >= c1) && (line_idx < l2 || col < c2) {
                            sx.get_or_insert(acc);
                            ex = acc + adv;
                            match ch {
                                ' ' => marks.push((acc + adv / 2, false)),
                                '\t' => marks.push((acc, true)),
                                _ => {}
                            }
                        }
                        acc += adv;
                    }
                    // El salto de línea también está seleccionado en todas las líneas menos la última
                    if is_last_segment && line_idx < l2 {
                        sx.get_or_insert(acc);
                        ex = acc + char_w as i32;
                    }
                    if let Some(sx) = sx {
                        let draw_sx = sx.max(text_clip.0 as i32) as usize;
                        let draw_ex = ex.min(text_clip.2 as i32).max(draw_sx as i32) as usize;
                        if draw_ex > draw_sx {
                            crate::ui::draw_rounded_rect(
                                buffer,
                                width,
                                height,
                                draw_sx,
                                row_y,
                                draw_ex - draw_sx,
                                row_h,
                                0,
                                sel_bg,
                            );
                        }
                    }
                }
            }

            match md_blocks.as_ref().and_then(|b| b.get(line_idx)) {
                Some(block) => self.render_md_line(
                    buffer, width, text_clip, seg, block, cur_x, row_y, row_h, baseline_y,
                    text_max_x, char_w, theme,
                ),
                None => self.render_highlighted_line(
                    buffer,
                    width,
                    text_clip,
                    seg,
                    cur_x,
                    baseline_y,
                    text_max_x,
                    char_w,
                    theme,
                    if viewer.is_diff() { "" } else { &ext },
                ),
            }

            for (mx, is_tab) in marks {
                let mid = row_y + row_h / 2;
                if is_tab {
                    fill_rect(
                        buffer,
                        width,
                        text_clip,
                        (mx + 4).max(0) as usize,
                        mid,
                        (tab_adv - 8).max(1) as usize,
                        1,
                        mark_color,
                    );
                } else if mx > 0 {
                    fill_rect(
                        buffer,
                        width,
                        text_clip,
                        mx as usize - 1,
                        mid - 1,
                        2,
                        2,
                        mark_color,
                    );
                }
            }

            // Región plegada: indica cuántas líneas quedan ocultas tras el texto
            if viewer.folded.contains(&line_idx) {
                if let Some(end) = viewer.fold_end(line_idx) {
                    let text_w: i32 = seg
                        .chars()
                        .map(|c| if c == '\t' { tab_adv } else { char_w as i32 })
                        .sum();
                    let label = format!("{} líneas", end - line_idx);
                    let chip_w = self.measure_ui_text(&label, 10.5, false) + 14;
                    let chip_x = (cur_x + text_w + char_w as i32).max(text_clip.0 as i32) as usize;
                    if chip_x + chip_w < text_max_x {
                        crate::ui::draw_rounded_rect(
                            buffer,
                            width,
                            height,
                            chip_x,
                            row_y + 2,
                            chip_w,
                            row_h.saturating_sub(4),
                            4,
                            Color::from_rgba(accent.r(), accent.g(), accent.b(), 28),
                        );
                        self.draw_ui_text(
                            buffer,
                            width,
                            height,
                            &label,
                            chip_x + 7,
                            baseline_y,
                            text_max_x,
                            accent,
                            10.5,
                            false,
                        );
                    }
                }
            }

            // Cursor de texto (parpadea con el resto de cursores)
            let col = viewer.cursor_col;
            if is_cursor_line
                && cursor_visible
                && col >= row.start
                && (is_last_segment || col < row.end)
            {
                let offset: i32 = seg
                    .chars()
                    .take(col - row.start)
                    .map(|c| if c == '\t' { tab_adv } else { char_w as i32 })
                    .sum();
                let cursor_x = cur_x + offset;
                if cursor_x >= (sep_x + 1) as i32 && cursor_x < text_max_x as i32 {
                    let cursor_h = (row_h as f32 * 0.85).round() as usize;
                    fill_rect(
                        buffer,
                        width,
                        text_clip,
                        cursor_x as usize,
                        row_y + (row_h.saturating_sub(cursor_h)) / 2,
                        2,
                        cursor_h,
                        accent.to_u32_xrgb(),
                    );
                }
            }
        }

        // 5. Barra de desplazamiento vertical unificada
        if let Some((track, thumb)) =
            Self::file_viewer_scrollbar_geometry(x, y, w, h, viewer, self.row_height)
        {
            Self::draw_scrollbar_rects(
                buffer,
                width,
                height,
                track,
                thumb,
                viewer.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }

        // 6. Barra de desplazamiento horizontal (solo si alguna línea excede el ancho visible)
        if let Some((track, thumb)) = lay.hscrollbar(max_scroll_x, scroll_x) {
            Self::draw_scrollbar_rects(
                buffer,
                width,
                height,
                track,
                thumb,
                viewer.hovered_hscrollbar,
                self.scrollbars_awake,
                theme,
            );
        }
    }

    /// Botón de la cabecera del visor: icono Lucide + texto sobre un fondo redondeado
    #[allow(clippy::too_many_arguments)]
    fn draw_header_button(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        label: &str,
        fg: Color,
        bg: Color,
        baseline: usize,
    ) {
        crate::ui::draw_rounded_rect(buffer, width, height, rect.0, rect.1, rect.2, rect.3, 4, bg);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            icon,
            rect.0 + 14,
            rect.1 + rect.3 / 2,
            11.0,
            fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            label,
            rect.0 + 26,
            baseline - 1,
            rect.0 + rect.2,
            fg,
            10.5,
            true,
        );
    }

    /// Calcula la geometría exacta de la barra de desplazamiento del visor de archivos
    pub fn file_viewer_scrollbar_geometry(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        viewer: &FileViewerState,
        row_height: usize,
    ) -> Option<(Rect, Rect)> {
        let header_h = 38;
        let content_y = y + header_h;
        let content_h = h.saturating_sub(header_h);
        let row_h = row_height.max(16);
        let visible_lines = content_h / row_h;
        let total_lines = viewer.rows().len();

        if total_lines <= visible_lines || visible_lines == 0 {
            return None;
        }

        let track_w = 6;
        let track_margin = 3;
        let track_x = (x + w).saturating_sub(track_w + track_margin);
        let track_y = content_y + 4;
        let track_h = content_h.saturating_sub(8);
        let min_thumb_h = 18;

        let thumb_h = (((visible_lines as f32) / (total_lines as f32)) * track_h as f32)
            .max(min_thumb_h as f32) as usize;
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let travel = track_h.saturating_sub(thumb_h);
        let thumb_y = if max_scroll > 0 {
            track_y + ((viewer.scroll_offset as f32 / max_scroll as f32) * travel as f32) as usize
        } else {
            track_y
        };

        Some((
            (track_x, track_y, track_w, track_h),
            (track_x, thumb_y, track_w, thumb_h),
        ))
    }

    /// Resaltado sintáctico multilenguaje de tokens ligero y de alto rendimiento
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_highlighted_line(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        line: &str,
        mut cur_x: i32,
        baseline_y: usize,
        max_x: usize,
        char_w: usize,
        theme: &Theme,
        ext: &str,
    ) {
        let max_x_i32 = max_x as i32;
        let char_w_i32 = char_w as i32;
        let tab_adv_i32 = char_w_i32 * 4;

        let trimmed = line.trim_start();

        // Comentarios completos de línea
        let is_full_comment = match ext {
            "py" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "toml" | "yaml" | "yml" | "rb"
            | "r" | "jl" | "dockerfile" | "makefile" | "mk" => trimmed.starts_with('#'),
            "bat" | "cmd" => {
                trimmed.to_ascii_uppercase().starts_with("REM ") || trimmed.starts_with("::")
            }
            "sql" | "lua" | "hs" | "elm" | "vhd" | "vhdl" => trimmed.starts_with("--"),
            "html" | "xml" | "svg" | "vue" | "svelte" => trimmed.starts_with("<!--"),
            "ini" | "cfg" | "conf" | "properties" => {
                trimmed.starts_with(';') || trimmed.starts_with('#')
            }
            _ => trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'),
        };

        if is_full_comment {
            let comment_fg = theme.ansi[8];
            for ch in line.chars() {
                if cur_x >= max_x_i32 {
                    break;
                }
                if ch != ' ' && ch != '\t' {
                    let glyph = self.get_glyph(ch, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, comment_fg);
                }
                cur_x += if ch == '\t' { tab_adv_i32 } else { char_w_i32 };
            }
            return;
        }

        // Tokenizador sin alojamientos sobre la cadena UTF-8
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        #[inline(always)]
        fn decode_at(line: &str, bytes: &[u8], i: usize) -> (char, usize) {
            let b = bytes[i];
            if b < 128 {
                (b as char, i + 1)
            } else {
                let c = line[i..].chars().next().unwrap();
                (c, i + c.len_utf8())
            }
        }

        while i < len {
            if cur_x >= max_x_i32 {
                break;
            }

            let (ch, next_i) = decode_at(line, bytes, i);

            // Tabulaciones: 4 espacios
            if ch == '\t' {
                cur_x += tab_adv_i32;
                i = next_i;
                continue;
            }

            // Espacios en blanco
            if ch == ' ' {
                cur_x += char_w_i32;
                i = next_i;
                continue;
            }

            // Comentario al final de la línea
            let is_inline_comment = match ext {
                "py" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "toml" | "yaml" | "yml" | "rb"
                | "r" | "jl" | "dockerfile" | "makefile" | "mk" => ch == '#',
                "sql" | "lua" | "hs" | "elm" => line[i..].starts_with("--"),
                "html" | "xml" | "svg" | "vue" | "svelte" => line[i..].starts_with("<!--"),
                "ini" | "cfg" | "conf" | "properties" => ch == ';' || ch == '#',
                _ => line[i..].starts_with("//"),
            };

            if is_inline_comment {
                let comment_fg = theme.ansi[8];
                for c in line[i..].chars() {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    if c != ' ' && c != '\t' {
                        let glyph = self.get_glyph(c, false);
                        let gx = cur_x + glyph.x_offset;
                        let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                        blit_glyph(buffer, width, clip, glyph, gx, gy, comment_fg);
                    }
                    cur_x += if c == '\t' { tab_adv_i32 } else { char_w_i32 };
                }
                break;
            }

            // Cadenas literales: "...", '...' o `...`
            if ch == '"' || ch == '\'' || ch == '`' {
                let quote = ch;
                let str_fg = theme.ansi[2]; // Verde esmeralda
                let glyph = self.get_glyph(ch, false);
                let gx = cur_x + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, gx, gy, str_fg);
                cur_x += char_w_i32;
                i = next_i;

                let mut prev_char = ch;
                while i < len && cur_x < max_x_i32 {
                    let (c, ni) = decode_at(line, bytes, i);
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, str_fg);
                    cur_x += char_w_i32;
                    i = ni;
                    if c == quote && prev_char != '\\' {
                        break;
                    }
                    prev_char = c;
                }
                continue;
            }

            // Atributos, decoradores o directivas (#![...], #include, @decorator, $variable)
            if ch == '@'
                || (ch == '$'
                    && matches!(
                        ext,
                        "ps1" | "php" | "sh" | "bash" | "zsh" | "fish" | "pl" | "rb"
                    ))
                || (ch == '#' && matches!(ext, "c" | "cpp" | "h" | "hpp" | "cs"))
            {
                let decorator_fg = theme.ansi[5]; // Magenta / Púrpura
                let glyph = self.get_glyph(ch, false);
                let gx = cur_x + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, gx, gy, decorator_fg);
                cur_x += char_w_i32;
                i = next_i;

                while i < len {
                    let (c, ni) = decode_at(line, bytes, i);
                    if !(c.is_alphanumeric() || c == '_' || c == ':') {
                        break;
                    }
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, decorator_fg);
                    cur_x += char_w_i32;
                    i = ni;
                }
                continue;
            }

            // Identificadores, palabras clave o llamadas a función
            if ch.is_alphabetic() || ch == '_' {
                let start_idx = i;
                i = next_i;
                while i < len {
                    let (c, ni) = decode_at(line, bytes, i);
                    if c.is_alphanumeric() || c == '_' {
                        i = ni;
                    } else {
                        break;
                    }
                }
                let word = &line[start_idx..i];

                // Verificar si va seguido de paréntesis '(' para colorear como invocación de función
                let is_func_call = line[i..].trim_start_matches(' ').starts_with('(');

                let token_fg = match word {
                    // Palabras clave de control, estructura y definición: Acento del tema
                    "fn" | "pub" | "struct" | "enum" | "impl" | "let" | "mut" | "const" | "use"
                    | "mod" | "type" | "return" | "if" | "else" | "elif" | "elsif" | "match"
                    | "for" | "while" | "loop" | "async" | "await" | "trait" | "where" | "self"
                    | "Self" | "function" | "def" | "class" | "import" | "from" | "export"
                    | "var" | "val" | "interface" | "package" | "go" | "defer" | "errdefer"
                    | "select" | "switch" | "case" | "default" | "break" | "continue" | "try"
                    | "catch" | "finally" | "throw" | "throws" | "new" | "delete" | "in" | "is"
                    | "as" | "yield" | "lambda" | "with" | "pass" | "raise" | "assert"
                    | "global" | "nonlocal" | "static" | "public" | "private" | "protected"
                    | "virtual" | "override" | "abstract" | "extends" | "implements"
                    | "namespace" | "using" | "template" | "typename" | "constexpr" | "sizeof"
                    | "param" | "process" | "begin" | "end" | "unsafe" | "dyn" | "ref" | "move"
                    | "fun" | "guard" | "when" | "protocol" | "extension" | "mutating"
                    | "comptime" | "unreachable" | "echo" | "source" | "alias" | "local"
                    | "then" | "fi" | "do" | "done" | "esac" | "SELECT" | "FROM" | "WHERE"
                    | "INSERT" | "INTO" | "VALUES" | "UPDATE" | "SET" | "DELETE" | "CREATE"
                    | "TABLE" | "DROP" | "ALTER" | "INDEX" | "JOIN" | "INNER" | "LEFT"
                    | "RIGHT" | "OUTER" | "GROUP" | "BY" | "HAVING" | "ORDER" | "LIMIT"
                    | "OFFSET" | "UNION" | "ALL" | "DISTINCT" | "PRIMARY" | "KEY" | "FOREIGN"
                    | "REFERENCES" | "CONSTRAINT" | "DATABASE" | "SCHEMA" | "VIEW" | "TRIGGER"
                    | "RUN" | "CMD" | "LABEL" | "EXPOSE" | "ENV" | "ADD" | "COPY"
                    | "ENTRYPOINT" | "VOLUME" | "USER" | "WORKDIR" | "ARG" | "ONBUILD" => {
                        theme.accent_color()
                    }

                    // Booleanos y valores nulos: Amarillo
                    "true" | "false" | "TRUE" | "FALSE" | "None" | "null" | "nil" | "undefined"
                    | "NULL" | "nullptr" | "NaN" | "Infinity" => theme.ansi[3],

                    // Tipos primitivos y contenedores estándar: Cian brillante (ANSI 6)
                    "String" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8"
                    | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" | "bool"
                    | "char" | "Option" | "Result" | "Vec" | "Some" | "Ok" | "Err" | "Box"
                    | "Rc" | "Arc" | "Cell" | "RefCell" | "Mutex" | "RwLock" | "HashMap"
                    | "HashSet" | "BTreeMap" | "BTreeSet" | "Map" | "Set" | "Array" | "List"
                    | "int" | "float" | "double" | "dict" | "list" | "set" | "tuple" | "void"
                    | "int8" | "int16" | "int32" | "int64" | "uint8" | "uint16" | "uint32"
                    | "uint64" | "byte" | "rune" | "any" | "unknown" | "never" | "number"
                    | "boolean" | "object" | "bigint" | "symbol" | "this" => theme.ansi[6],

                    _ if is_func_call => theme.ansi[4], // Azul moderno para llamadas a función
                    _ if word
                        .chars()
                        .all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
                        && word.chars().nth(1).is_some() =>
                    {
                        theme.ansi[5] // Constantes en mayúsculas: Magenta
                    }
                    _ if ch.is_uppercase() => {
                        theme.ansi[6] // Clases o Tipos PascalCase: Cian
                    }
                    _ => theme.foreground,
                };

                let is_bold = token_fg == theme.accent_color();
                for wc in word.chars() {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let glyph = self.get_glyph(wc, is_bold);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, token_fg);
                    cur_x += char_w_i32;
                }
                continue;
            }

            // Números literales (enteros, hexadecimales 0x..., flotantes)
            if ch.is_ascii_digit() {
                let num_fg = theme.ansi[3]; // Amarillo oro
                while i < len {
                    let b = bytes[i];
                    if !(b.is_ascii_alphanumeric() || b == b'.' || b == b'_') {
                        break;
                    }
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let c = b as char;
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, num_fg);
                    cur_x += char_w_i32;
                    i += 1;
                }
                continue;
            }

            // Símbolos, delimitadores y operadores
            let symbol_fg = if matches!(ch, '{' | '}' | '(' | ')' | '[' | ']') {
                theme.ansi[7]
            } else if matches!(
                ch,
                ':' | ';'
                    | ','
                    | '.'
                    | '='
                    | '+'
                    | '-'
                    | '*'
                    | '/'
                    | '&'
                    | '|'
                    | '!'
                    | '?'
                    | '<'
                    | '>'
                    | '%'
                    | '^'
                    | '~'
            ) {
                theme.ansi[6]
            } else {
                theme.foreground
            };

            let glyph = self.get_glyph(ch, false);
            let gx = cur_x + glyph.x_offset;
            let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
            blit_glyph(buffer, width, clip, glyph, gx, gy, symbol_fg);
            cur_x += char_w_i32;
            i = next_i;
        }
    }
}
