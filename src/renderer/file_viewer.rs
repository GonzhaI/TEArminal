//! Renderizado del visor de archivos y código en pestaña dedicada (File Viewer).

use super::*;
use crate::app::FileViewerState;

impl TerminalRenderer {
    /// Renderiza el visor de archivos/código en el área del terminal sin procesos ConPTY
    pub(crate) fn render_file_viewer(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        right_inset: usize,
        cursor_visible: bool,
        viewer: &FileViewerState,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if w == 0 || h == 0 || width == 0 || height == 0 {
            return;
        }

        // Fondo y cabecera ocupan todo el ancho (también bajo el explorador flotante, sin corte abrupto);
        // texto, controles y barras de scroll se quedan en el área libre de `w - right_inset`.
        let full_w = w;
        let w = w.saturating_sub(right_inset);
        let clip: Clip = (x, y, (x + full_w).min(width), (y + h).min(height));

        // 1. Fondo completo del área de trabajo
        canvas.fill_rect(clip, x, y, full_w, h, theme.background);

        // 2. Barra de encabezado superior (38 px), plana y con hairline inferior.
        let header_h = crate::renderer::geometry::FileViewerLayout::HEADER_H;
        let header_bg = theme.chrome_bg();
        canvas.fill_rect(clip, x, y, full_w, header_h, header_bg);

        // Línea divisoria inferior del encabezado
        canvas.fill_rect(clip, x, y + header_h - 1, full_w, 1, theme.chrome_line());

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

        let header_center_y =
            y + SidebarRects::CARD_INSET + (header_h.saturating_sub(SidebarRects::CARD_INSET)) / 2;
        let header_base = self.centered_baseline(header_center_y);

        self.draw_lucide_icon(
            canvas,
            file_icon,
            x + SidebarRects::CARD_INSET + 16,
            header_center_y,
            13.0,
            theme.text_muted(),
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
        let chip_bg = theme.chrome_bg();
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
            self.draw_header_button(canvas, rect, icon, label, fg, bg, header_base);
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
                canvas,
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
            canvas,
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
        let mut chips: smallvec::SmallVec<[(String, Color, Color); 4]> = smallvec::SmallVec::new();
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

        let name_start_x = x + SidebarRects::CARD_INSET + 34;
        let name_end_x =
            name_start_x + self.measure_ui_text(&viewer.file_name, tokens::SECTION, true);
        let mut right_x = header_rects.mode_btn.0;
        for r in [header_rects.md_btn, header_rects.diff_btn]
            .into_iter()
            .flatten()
        {
            right_x = right_x.min(r.0);
        }
        right_x = right_x.saturating_sub(10);
        let sep_y = header_center_y.saturating_sub(6);
        for (text, chip_fg, _bg) in &chips {
            let chip_w = self.measure_mono_text(text, tokens::MICRO, false);
            let chip_x = right_x.saturating_sub(chip_w);
            if chip_x < name_end_x + 16 {
                break;
            }
            self.draw_mono_text(
                canvas,
                text,
                chip_x,
                header_base,
                right_x,
                *chip_fg,
                tokens::MICRO,
                false,
            );
            right_x = chip_x.saturating_sub(9);
            Self::draw_rect(canvas, right_x, sep_y, 1, 12, theme.chrome_line());
            right_x = right_x.saturating_sub(9);
        }

        // Carpeta contenedora: dato de máquina, en mono y en segundo plano tras el nombre.
        let folder_label = viewer
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().replace('\\', "/"));
        if let Some(folder) = folder_label.as_deref() {
            let folder_x = name_end_x + 14;
            if folder_x + 24 < right_x {
                Self::draw_rect(canvas, name_end_x + 7, sep_y, 1, 12, theme.chrome_line());
                self.draw_mono_text(
                    canvas,
                    folder,
                    folder_x,
                    header_base,
                    right_x,
                    theme.text_muted(),
                    tokens::DETAIL,
                    false,
                );
            }
        }

        // Solo el nombre del archivo: la ruta completa ya se muestra en la barra de estado
        self.draw_ui_text(
            canvas,
            &viewer.file_name,
            name_start_x,
            header_base,
            right_x + 4,
            theme.foreground,
            tokens::SECTION,
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

            canvas.rounded_fill(
                card_x,
                card_y,
                card_w,
                card_h,
                tokens::RADIUS_CONTROL,
                theme.chrome_bg_elevated(),
            );
            canvas.rounded_stroke(
                card_x,
                card_y,
                card_w,
                card_h,
                tokens::RADIUS_CONTROL,
                theme.card_outline(),
            );

            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Info,
                card_x + 36,
                card_y + 50,
                20.0,
                theme.accent_color(),
            );

            self.draw_ui_text(
                canvas,
                "Vista previa no disponible",
                card_x + 64,
                card_y + 42,
                card_x + card_w.saturating_sub(20),
                theme.foreground,
                13.0,
                true,
            );

            self.draw_ui_text(
                canvas,
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
        canvas.fill_rect(
            clip,
            x,
            content_y,
            gutter_w,
            content_h,
            theme.mix(theme.background, theme.foreground, 0.03),
        );

        // Línea vertical separadora del gutter
        canvas.fill_rect(clip, sep_x, content_y, 1, content_h, theme.chrome_line());

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
            let blocks: std::rc::Rc<[crate::renderer::md_preview::MdBlock]> =
                std::rc::Rc::from(crate::renderer::md_preview::classify_blocks(&viewer.lines));
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
        let mark_color = theme.text_muted();
        let tab_adv = (char_w * 4) as i32;
        let indent_unit = viewer.indent_unit();
        let guide_color = theme.mix(theme.background, theme.foreground, 0.10);

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

            // Línea actual: banda de 3 % de tinta, apenas perceptible sobre el lienzo.
            if is_cursor_line {
                canvas.fill_rect(
                    clip,
                    x,
                    row_y,
                    text_max_x.saturating_sub(x),
                    row_h,
                    theme.mix(theme.background, theme.foreground, 0.035),
                );
            }

            // Indicador de Git en el gutter si la línea fue modificada, añadida o eliminada
            if let Some(git_status) = viewer.get_line_git_diff_status(line_idx) {
                let git_color = match git_status {
                    crate::app::GitLineStatus::Added => theme.ansi[2], // Verde
                    crate::app::GitLineStatus::Modified => theme.ansi[3], // Amarillo
                    crate::app::GitLineStatus::Deleted => theme.ansi[1], // Rojo
                };
                if git_status == crate::app::GitLineStatus::Deleted {
                    canvas.fill_rect(gutter_clip, sep_x.saturating_sub(4), row_y, 4, 3, git_color);
                } else {
                    canvas.fill_rect(
                        gutter_clip,
                        sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        git_color,
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
                    theme.text_muted()
                };
                for (ci, &b) in digits.iter().enumerate() {
                    let glyph = self.get_glyph(b as char, false);
                    let gx = (num_x + ci * char_w) as i32 + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(gutter_clip, glyph, gx, gy, num_fg);
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
                    let color = if folded { accent } else { theme.text_muted() };
                    self.draw_lucide_icon(canvas, icon, x + 10, row_y + row_h / 2, 11.0, color);
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
                    canvas.fill_rect(
                        clip,
                        sep_x + 1,
                        row_y,
                        text_max_x.saturating_sub(sep_x + 1),
                        row_h,
                        // `fill_rect` es opaco: el tinte se mezcla con el lienzo, no se pasa con alfa
                        Color::from_u32_xrgb(theme.background.blend_over(c, 34)),
                    );
                    // Marca sólida en el borde de las líneas añadidas y quitadas
                    if c != accent {
                        canvas.fill_rect(clip, sep_x + 1, row_y, 3, row_h, c);
                    }
                }
            }

            // Guías de alineación e indentación verticales
            if !viewer.is_diff() && (!viewer.render_markdown || !is_markdown) && first_segment {
                let line_str = viewer.lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
                let indent_col = if !line_str.trim().is_empty() {
                    crate::app::indent_of(line_str).unwrap_or(0)
                } else {
                    viewer
                        .lines
                        .get(line_idx + 1..)
                        .unwrap_or(&[])
                        .iter()
                        .take(100)
                        .find_map(|l| crate::app::indent_of(l))
                        .or_else(|| {
                            viewer
                                .lines
                                .get(..line_idx)
                                .unwrap_or(&[])
                                .iter()
                                .rev()
                                .take(100)
                                .find_map(|l| crate::app::indent_of(l))
                        })
                        .unwrap_or(0)
                };
                if indent_col > indent_unit {
                    let mut col = indent_unit;
                    while col < indent_col {
                        let gx = cur_x + (col * char_w) as i32;
                        if gx >= text_clip.0 as i32 && (gx as usize) < text_clip.2 {
                            canvas.fill_rect(text_clip, gx as usize, row_y, 1, row_h, guide_color);
                        }
                        col += indent_unit;
                    }
                }
            }

            // Selección: rectángulo sin bordes redondeados (filas contiguas sin huecos) y marcas de
            // espacios y tabuladores dentro de lo seleccionado para poder contar la sangría
            let mut marks: smallvec::SmallVec<[(i32, bool); 32]> = smallvec::SmallVec::new();
            if let Some(((l1, c1), (l2, c2))) = selection {
                if line_idx >= l1 && line_idx <= l2 {
                    let mut acc = cur_x;
                    let mut sx: Option<i32> = None;
                    let mut ex = cur_x;
                    use unicode_segmentation::UnicodeSegmentation;
                    for (ci, g) in seg.graphemes(true).enumerate() {
                        let col = row.start + ci;
                        let adv = if g == "\t" {
                            tab_adv
                        } else {
                            use unicode_width::UnicodeWidthStr;
                            char_w as i32 * g.width().max(1) as i32
                        };
                        if (line_idx > l1 || col >= c1) && (line_idx < l2 || col < c2) {
                            sx.get_or_insert(acc);
                            ex = acc + adv;
                            match g {
                                " " => marks.push((acc + adv / 2, false)),
                                "\t" => marks.push((acc, true)),
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
                            canvas.rounded_fill(
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
                    canvas, text_clip, seg, block, cur_x, row_y, row_h, baseline_y, text_max_x,
                    char_w, theme,
                ),
                None => self.render_highlighted_line(
                    canvas,
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
                    canvas.fill_rect(
                        text_clip,
                        (mx + 4).max(0) as usize,
                        mid,
                        (tab_adv - 8).max(1) as usize,
                        1,
                        mark_color,
                    );
                } else if mx > 0 {
                    canvas.fill_rect(text_clip, mx as usize - 1, mid - 1, 2, 2, mark_color);
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
                        canvas.rounded_fill(
                            chip_x,
                            row_y + 2,
                            chip_w,
                            row_h.saturating_sub(4),
                            4,
                            theme.accent_soft(28),
                        );
                        self.draw_ui_text(
                            canvas,
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
                use unicode_segmentation::UnicodeSegmentation;
                let offset: i32 = seg
                    .graphemes(true)
                    .take(col - row.start)
                    .map(|g| {
                        if g == "\t" {
                            tab_adv
                        } else {
                            use unicode_width::UnicodeWidthStr;
                            char_w as i32 * g.width().max(1) as i32
                        }
                    })
                    .sum();
                let cursor_x = cur_x + offset;
                if cursor_x >= (sep_x + 1) as i32 && cursor_x < text_max_x as i32 {
                    let cursor_h = (row_h as f32 * 0.85).round() as usize;
                    canvas.fill_rect(
                        text_clip,
                        cursor_x as usize,
                        row_y + (row_h.saturating_sub(cursor_h)) / 2,
                        2,
                        cursor_h,
                        accent,
                    );
                }
            }
        }

        // 5. Barra de desplazamiento vertical unificada
        if let Some((track, thumb)) =
            Self::file_viewer_scrollbar_geometry(x, y, w, h, viewer, self.row_height)
        {
            Self::draw_scrollbar_rects(
                canvas,
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
                canvas,
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
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        label: &str,
        fg: Color,
        bg: Color,
        _baseline: usize,
    ) {
        canvas.rounded_fill(rect.0, rect.1, rect.2, rect.3, 4, bg);
        let cy = rect.1 + rect.3 / 2;
        let text_w = self.measure_ui_text(label, tokens::DETAIL, true);
        let content_w = 22 + text_w;
        let start_x = rect.0 + rect.2.saturating_sub(content_w) / 2;
        self.draw_lucide_icon(canvas, icon, start_x + 7, cy, 11.5, fg);
        self.draw_ui_text(
            canvas,
            label,
            start_x + 21,
            self.centered_baseline(cy),
            rect.0 + rect.2 - 4,
            fg,
            tokens::DETAIL,
            true,
        );
    }

    /// Calcula la geometría exacta de la barra de desplazamiento del visor de archivos
    pub(crate) fn file_viewer_scrollbar_geometry(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        viewer: &FileViewerState,
        row_height: usize,
    ) -> Option<(Rect, Rect)> {
        let header_h = crate::renderer::geometry::FileViewerLayout::HEADER_H;
        let content_y = y + header_h;
        let content_h = h.saturating_sub(header_h);
        let row_h = row_height.max(16);
        let visible_lines = content_h / row_h;
        let total_lines = viewer.rows().len();

        ScrollbarRects::vertical(
            x + w,
            content_y + 6,
            content_h.saturating_sub(6 + SidebarRects::CARD_INSET + 4),
            total_lines,
            visible_lines,
            viewer.scroll_offset,
        )
    }

    /// Resaltado sintáctico multilenguaje de tokens ligero y de alto rendimiento
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_highlighted_line(
        &mut self,
        canvas: &mut Canvas,
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
                    canvas.glyph(clip, glyph, gx, gy, comment_fg);
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
                let c = line
                    .get(i..)
                    .and_then(|s| s.chars().next())
                    .unwrap_or('\u{FFFD}');
                (c, (i + c.len_utf8()).min(bytes.len()))
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
                        canvas.glyph(clip, glyph, gx, gy, comment_fg);
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
                canvas.glyph(clip, glyph, gx, gy, str_fg);
                cur_x += char_w_i32;
                i = next_i;

                let mut escaped = false;
                while i < len && cur_x < max_x_i32 {
                    let (c, ni) = decode_at(line, bytes, i);
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, str_fg);
                    cur_x += char_w_i32;
                    i = ni;
                    if c == quote && !escaped {
                        break;
                    }
                    if c == '\\' {
                        escaped = !escaped;
                    } else {
                        escaped = false;
                    }
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
                canvas.glyph(clip, glyph, gx, gy, decorator_fg);
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
                    canvas.glyph(clip, glyph, gx, gy, decorator_fg);
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
                    canvas.glyph(clip, glyph, gx, gy, token_fg);
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
                    canvas.glyph(clip, glyph, gx, gy, num_fg);
                    cur_x += char_w_i32;
                    i += 1;
                }
                continue;
            }

            // Símbolos, delimitadores y operadores
            if self.ligatures && is_ligature_candidate(ch) {
                let mut op_buf = [0u8; 8];
                op_buf[0] = ch as u8;
                let mut run_len = 1usize;
                let mut peek_i = next_i;
                while run_len < 8 && peek_i < len {
                    let b = bytes[peek_i];
                    if b < 128 && is_ligature_candidate(b as char) {
                        op_buf[run_len] = b;
                        run_len += 1;
                        peek_i += 1;
                    } else {
                        break;
                    }
                }

                if run_len >= 2 {
                    let shaped = self.shape_punctuation_run(&op_buf[..run_len], false);
                    let symbol_fg = theme.ansi[6];
                    for &gid in &shaped.gids[..run_len] {
                        if cur_x >= max_x_i32 {
                            break;
                        }
                        let glyph = self.get_ligature_glyph(gid, false);
                        if glyph.width > 0 {
                            let gx = cur_x + glyph.x_offset;
                            let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                            canvas.glyph(clip, glyph, gx, gy, symbol_fg);
                        }
                        cur_x += char_w_i32;
                    }
                    i = peek_i;
                    continue;
                }
            }

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
            canvas.glyph(clip, glyph, gx, gy, symbol_fg);
            cur_x += char_w_i32;
            i = next_i;
        }
    }
}
