//! Renderizado del diálogo modal común con franja de gravedad, detalle y confirmación escrita.
//!
//! Implementa las especificaciones visuales de DISENO §5, pantalla `design/Dialogs.dc.html`
//! y pantalla especializada `design/DbConfirm.dc.html`.

use super::*;
use crate::app::{DialogButtonKind, DialogState};
use crate::renderer::geometry::DialogRects;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlTokenKind {
    Keyword,
    StringLit,
    NumberLit,
    Plain,
}

struct SqlToken<'a> {
    text: &'a str,
    kind: SqlTokenKind,
}

fn tokenize_sql_line(line: &str) -> Vec<SqlToken<'_>> {
    let mut tokens = Vec::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut idx = 0;

    while idx < chars.len() {
        let (start, ch) = chars[idx];
        if ch == '\'' || ch == '"' {
            let quote = ch;
            idx += 1;
            while idx < chars.len() && chars[idx].1 != quote {
                idx += 1;
            }
            if idx < chars.len() {
                idx += 1;
            }
            let end = if idx < chars.len() {
                chars[idx].0
            } else {
                line.len()
            };
            tokens.push(SqlToken {
                text: &line[start..end],
                kind: SqlTokenKind::StringLit,
            });
        } else if ch.is_alphabetic() || ch == '_' {
            while idx < chars.len() && (chars[idx].1.is_alphanumeric() || chars[idx].1 == '_') {
                idx += 1;
            }
            let end = if idx < chars.len() {
                chars[idx].0
            } else {
                line.len()
            };
            let word = &line[start..end];
            let upper = word.to_uppercase();
            let is_kw = matches!(
                upper.as_str(),
                "SELECT"
                    | "INSERT"
                    | "UPDATE"
                    | "DELETE"
                    | "FROM"
                    | "WHERE"
                    | "SET"
                    | "INTO"
                    | "VALUES"
                    | "JOIN"
                    | "INNER"
                    | "LEFT"
                    | "RIGHT"
                    | "ON"
                    | "GROUP"
                    | "BY"
                    | "ORDER"
                    | "ASC"
                    | "DESC"
                    | "LIMIT"
                    | "OFFSET"
                    | "AND"
                    | "OR"
                    | "NOT"
                    | "IN"
                    | "IS"
                    | "NULL"
                    | "TRUE"
                    | "FALSE"
                    | "BEGIN"
                    | "COMMIT"
                    | "ROLLBACK"
                    | "DROP"
                    | "TRUNCATE"
                    | "ALTER"
                    | "CREATE"
                    | "TABLE"
                    | "EXPLAIN"
                    | "ANALYZE"
            );
            tokens.push(SqlToken {
                text: word,
                kind: if is_kw {
                    SqlTokenKind::Keyword
                } else {
                    SqlTokenKind::Plain
                },
            });
        } else if ch.is_ascii_digit() {
            while idx < chars.len() && (chars[idx].1.is_ascii_digit() || chars[idx].1 == '.') {
                idx += 1;
            }
            let end = if idx < chars.len() {
                chars[idx].0
            } else {
                line.len()
            };
            tokens.push(SqlToken {
                text: &line[start..end],
                kind: SqlTokenKind::NumberLit,
            });
        } else {
            while idx < chars.len()
                && !chars[idx].1.is_alphabetic()
                && chars[idx].1 != '_'
                && !chars[idx].1.is_ascii_digit()
                && chars[idx].1 != '\''
                && chars[idx].1 != '"'
            {
                idx += 1;
            }
            let end = if idx < chars.len() {
                chars[idx].0
            } else {
                line.len()
            };
            tokens.push(SqlToken {
                text: &line[start..end],
                kind: SqlTokenKind::Plain,
            });
        }
    }

    tokens
}

impl TerminalRenderer {
    /// Dibuja el diálogo modal activo centrado sobre un velo translúcido.
    pub(crate) fn render_dialog(
        &mut self,
        canvas: &mut Canvas,
        dialog: &DialogState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);

        // Medir ancho de botones con padding simétrico (14 px por lado)
        let button_widths: Vec<usize> = dialog
            .spec
            .buttons
            .iter()
            .map(|btn| {
                let text_w =
                    self.measure_ui_text_tracked(&btn.label, tokens::DETAIL, tokens::TRACKING);
                text_w + DialogRects::BTN_PAD_X * 2
            })
            .collect();

        let is_db = dialog.spec.is_db_confirm();
        let rects = DialogRects::compute_for_spec(win_w, win_h, &dialog.spec, &button_widths);

        // 1. Velo translúcido sobre toda la ventana: crust al 68%
        let crust = theme.crust();
        canvas.fill_rect(
            clip,
            0,
            0,
            win_w,
            win_h,
            Color::from_rgba(crust.r, crust.g, crust.b, 174),
        );

        // 2. Tarjeta del diálogo: fondo mantle y contorno fuerte (rojo si es peligroso/DbConfirm)
        let (cx, cy, cw, ch) = rects.card;
        canvas.rounded_fill(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.mantle());
        let border_col = if is_db || dialog.spec.severity == crate::app::DialogSeverity::Danger {
            theme.red()
        } else {
            theme.chrome_line_strong()
        };
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, border_col);

        // 3. Franja superior tintada por gravedad
        let (hx, hy, hw, hh) = rects.header;
        let tint_bg = dialog.spec.severity.tint(theme);
        canvas.fill_rect(clip, hx, hy, hw, hh, tint_bg);
        canvas.fill_rect(
            clip,
            hx,
            hy + hh.saturating_sub(1),
            hw,
            1,
            theme.chrome_line(),
        );

        // Ícono de gravedad en la franja
        let icon_cy = hy + hh / 2;
        let icon_col = dialog.spec.severity.color(theme);
        self.draw_lucide_icon(canvas, dialog.spec.icon, hx + 18, icon_cy, 15.0, icon_col);

        // Título del diálogo en mayúsculas con tracking
        let title_baseline = self.centered_baseline(icon_cy);
        self.draw_ui_text_tracked(
            canvas,
            &dialog.spec.title,
            hx + 38,
            title_baseline,
            rects.close_btn.0.saturating_sub(180),
            theme.foreground,
            tokens::TITLE,
            true,
            tokens::TRACKING,
        );

        // Meta de cabecera a la derecha (ej. pg-main · acme para DbConfirm)
        if let (Some(meta_str), Some((mx, my, mw, mh))) =
            (&dialog.spec.header_meta, rects.header_meta_rect)
        {
            self.draw_mono_text(
                canvas,
                meta_str,
                mx,
                self.centered_baseline(my + mh / 2),
                mx + mw,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
        }

        // Botón de cierre `×`
        let (cl_x, cl_y, cl_w, cl_h) = rects.close_btn;
        if dialog.hovered_close {
            canvas.rounded_fill(
                cl_x,
                cl_y,
                cl_w,
                cl_h,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        let close_col = if dialog.hovered_close {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::X,
            cl_x + cl_w / 2,
            cl_y + cl_h / 2,
            13.0,
            close_col,
        );

        if is_db {
            // =========================================================================
            // RENDERIZADO ESPECIALIZADO DbConfirm (DISENO §5 pantalla 07)
            // =========================================================================
            let (bx, by, bw, _bh) = rects.body_rect;

            // Título principal destacado (18px semibold)
            self.draw_ui_text(
                canvas,
                &dialog.spec.body,
                bx,
                self.centered_baseline(by + 12),
                bx + bw,
                theme.foreground,
                tokens::TITLE,
                true,
            );

            // Subtítulo explicativo
            if let (Some(sub), Some((sx, sy))) = (&dialog.spec.subtitle, rects.subtitle_pos) {
                self.draw_ui_text(
                    canvas,
                    sub,
                    sx,
                    self.centered_baseline(sy + 8),
                    sx + bw,
                    theme.text_secondary(),
                    tokens::BODY,
                    false,
                );
            }

            // Bloque «SQL que se ejecutará»
            if let (Some(sql_str), Some((sx, sy, sw, sh))) =
                (&dialog.spec.sql_code, rects.sql_block)
            {
                self.draw_ui_text_tracked(
                    canvas,
                    "SQL QUE SE EJECUTARÁ",
                    sx,
                    self.centered_baseline(sy.saturating_sub(10)),
                    sx + sw,
                    theme.text_muted(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );

                canvas.fill_rect(clip, sx, sy, sw, sh, theme.chrome_bg());
                canvas.rounded_stroke(sx, sy, sw, sh, tokens::RADIUS_ROW, theme.chrome_line());

                let mut cur_line_y = sy + 10;
                let max_x = sx + sw.saturating_sub(14);

                for line in sql_str.lines().take(6) {
                    let mut cur_token_x = sx + 14;
                    let tokens = tokenize_sql_line(line);

                    for tok in tokens {
                        let col = match tok.kind {
                            SqlTokenKind::Keyword => theme.mauve(),
                            SqlTokenKind::StringLit => theme.green(),
                            SqlTokenKind::NumberLit => theme.peach(),
                            SqlTokenKind::Plain => theme.foreground,
                        };
                        cur_token_x = self.draw_mono_text(
                            canvas,
                            tok.text,
                            cur_token_x,
                            self.centered_baseline(cur_line_y + 10),
                            max_x,
                            col,
                            tokens::DETAIL,
                            false,
                        );
                        if cur_token_x >= max_x {
                            break;
                        }
                    }
                    cur_line_y += 20;
                }
            }

            // Bloque «Impacto estimado»
            if let (Some((tx, ty, tw, th)), Some((hx, hy, hw, hh))) =
                (rects.impact_table, rects.impact_header)
            {
                self.draw_ui_text_tracked(
                    canvas,
                    "IMPACTO ESTIMADO",
                    tx,
                    self.centered_baseline(ty.saturating_sub(10)),
                    tx + tw,
                    theme.text_muted(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );

                // Marco exterior de la tabla
                canvas.rounded_stroke(tx, ty, tw, th, tokens::RADIUS_ROW, theme.chrome_line());

                // Cabecera
                canvas.fill_rect(clip, hx, hy, hw, hh, theme.chrome_bg());
                let hdr_base = self.centered_baseline(hy + hh / 2);
                self.draw_ui_text_tracked(
                    canvas,
                    "#",
                    hx + 12,
                    hdr_base,
                    hx + 44,
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );
                self.draw_ui_text_tracked(
                    canvas,
                    "OPERACIÓN",
                    hx + 56,
                    hdr_base,
                    hx + tw.saturating_sub(230),
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );
                self.draw_ui_text_tracked(
                    canvas,
                    "TABLA",
                    hx + tw.saturating_sub(230),
                    hdr_base,
                    hx + tw.saturating_sub(120),
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );
                let aff_w = self.measure_ui_text_tracked(
                    "FILAS AFECTADAS",
                    tokens::MICRO,
                    tokens::TRACKING,
                );
                let aff_x = (hx + tw.saturating_sub(12)).saturating_sub(aff_w);
                self.draw_ui_text_tracked(
                    canvas,
                    "FILAS AFECTADAS",
                    aff_x,
                    hdr_base,
                    hx + tw,
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );

                // Filas de impacto
                for (idx, (im, &(rx, ry, rw, rh))) in dialog
                    .spec
                    .impact_rows
                    .iter()
                    .zip(&rects.impact_rows)
                    .enumerate()
                {
                    canvas.fill_rect(clip, rx, ry, rw, 1, theme.chrome_line());
                    let row_base = self.centered_baseline(ry + rh / 2);

                    self.draw_mono_text(
                        canvas,
                        &im.n,
                        rx + 12,
                        row_base,
                        rx + 44,
                        theme.text_muted(),
                        tokens::DETAIL,
                        false,
                    );
                    self.draw_ui_text(
                        canvas,
                        &im.op,
                        rx + 56,
                        row_base,
                        rx + tw.saturating_sub(230),
                        theme.foreground,
                        tokens::BODY,
                        false,
                    );
                    self.draw_mono_text(
                        canvas,
                        &im.table,
                        rx + tw.saturating_sub(230),
                        row_base,
                        rx + tw.saturating_sub(120),
                        theme.foreground,
                        tokens::DETAIL,
                        false,
                    );

                    let rows_w = self.measure_mono_text(&im.rows, tokens::DETAIL, true);
                    let rows_x = (rx + tw.saturating_sub(14)).saturating_sub(rows_w);
                    self.draw_mono_text(
                        canvas,
                        &im.rows,
                        rows_x,
                        row_base,
                        rx + tw,
                        theme.foreground,
                        tokens::DETAIL,
                        true,
                    );
                    let _ = idx;
                }
            }

            // Bloque Opciones (Checkboxes)
            if dialog.spec.has_db_options {
                let options_data = [
                    (
                        "Ejecutar en una transacción y revertir si una sentencia falla",
                        dialog.db_options.in_transaction,
                    ),
                    (
                        "Guardar las filas originales en un archivo local antes de aplicar",
                        dialog.db_options.backup_rows,
                    ),
                    (
                        "No volver a pedir confirmación en esta pestaña",
                        dialog.db_options.remember_tab,
                    ),
                ];

                for (idx, ((label, is_checked), &(ox, oy, ow, oh))) in options_data
                    .iter()
                    .zip(&rects.option_checkboxes)
                    .enumerate()
                {
                    if dialog.hovered_option == Some(idx) {
                        canvas.rounded_fill(
                            ox,
                            oy,
                            ow,
                            oh,
                            tokens::RADIUS_ROW,
                            theme.surface_hover(),
                        );
                    }

                    // Cuadro de checkbox 16x16
                    let chk_x = ox;
                    let chk_y = oy + (oh.saturating_sub(16)) / 2;
                    let chk_size = 16;

                    if *is_checked {
                        canvas.rounded_fill(
                            chk_x,
                            chk_y,
                            chk_size,
                            chk_size,
                            tokens::RADIUS_ROW,
                            theme.accent_color(),
                        );
                        self.draw_lucide_icon(
                            canvas,
                            crate::ui::LucideIcon::Check,
                            chk_x + chk_size / 2,
                            chk_y + chk_size / 2,
                            11.0,
                            theme.on_accent(),
                        );
                    } else {
                        canvas.rounded_stroke(
                            chk_x,
                            chk_y,
                            chk_size,
                            chk_size,
                            tokens::RADIUS_ROW,
                            theme.chrome_line_strong(),
                        );
                    }

                    // Texto de la opción
                    self.draw_ui_text(
                        canvas,
                        label,
                        chk_x + chk_size + 10,
                        self.centered_baseline(oy + oh / 2),
                        ox + ow,
                        theme.foreground,
                        tokens::BODY,
                        false,
                    );
                }
            }

            // Confirmación escrita
            if let (Some(kw), Some(lbl_y), Some((ix, iy, iw, ih)), Some((sx, sy, sw, sh))) = (
                &dialog.spec.confirm_keyword,
                rects.confirm_label_y,
                rects.confirm_input,
                rects.confirm_status,
            ) {
                let lbl_base = self.centered_baseline(lbl_y);
                let end_lbl = self.draw_ui_text(
                    canvas,
                    "Para confirmar, escribe el nombre de la conexión: ",
                    bx,
                    lbl_base,
                    bx + bw,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
                self.draw_mono_text(
                    canvas,
                    kw,
                    end_lbl + 4,
                    lbl_base,
                    bx + bw,
                    theme.red(),
                    tokens::DETAIL,
                    true,
                );

                // Caja de entrada
                canvas.fill_rect(clip, ix, iy, iw, ih, theme.chrome_bg());
                let input_border = if dialog.is_confirmed() {
                    theme.green()
                } else {
                    theme.accent_color()
                };
                canvas.rounded_stroke(ix, iy, iw, ih, tokens::RADIUS_CONTROL, input_border);

                self.render_text_input_content(
                    canvas,
                    &dialog.input,
                    ix + 10,
                    self.centered_baseline(iy + ih / 2),
                    ix + iw - 10,
                    iy,
                    ih,
                    cursor_visible,
                    kw,
                    true,
                    theme,
                );

                // Indicador de coincidencia a la derecha
                let (status_text, status_col) = if dialog.is_confirmed() {
                    ("Coincide", theme.green())
                } else {
                    ("Pendiente", theme.text_muted())
                };
                self.draw_ui_text_tracked(
                    canvas,
                    status_text,
                    sx + 6,
                    self.centered_baseline(sy + sh / 2),
                    sx + sw,
                    status_col,
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );
            }
        } else {
            // =========================================================================
            // RENDERIZADO DIÁLOGOS ESTÁNDAR DEL CATÁLOGO (DISENO §5 pantalla 11)
            // =========================================================================
            // 4. Cuerpo: texto explicativo
            let (bx, by, bw, _bh) = rects.body_rect;
            self.draw_ui_text(
                canvas,
                &dialog.spec.body,
                bx,
                self.centered_baseline(by + 16),
                bx + bw,
                theme.text_secondary(),
                tokens::BODY,
                false,
            );

            // 5. Bloque monoespaciado (si existe)
            if let (Some(mono_str), Some((mx, my, mw, mh))) =
                (&dialog.spec.mono_detail, rects.mono_block)
            {
                canvas.fill_rect(clip, mx, my, mw, mh, theme.chrome_bg());
                canvas.rounded_stroke(mx, my, mw, mh, tokens::RADIUS_ROW, theme.chrome_line());
                self.draw_mono_text(
                    canvas,
                    mono_str,
                    mx + 10,
                    self.centered_baseline(my + mh / 2),
                    mx + mw - 10,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            // 6. Campo de confirmación escrita (si la requiere)
            if let (Some(kw), Some(lbl_y), Some((ix, iy, iw, ih)), Some((sx, sy, sw, sh))) = (
                &dialog.spec.confirm_keyword,
                rects.confirm_label_y,
                rects.confirm_input,
                rects.confirm_status,
            ) {
                let lbl_base = self.centered_baseline(lbl_y);
                let end_lbl = self.draw_ui_text(
                    canvas,
                    "Para confirmar, escribe: ",
                    bx,
                    lbl_base,
                    bx + bw,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
                self.draw_mono_text(
                    canvas,
                    kw,
                    end_lbl + 4,
                    lbl_base,
                    bx + bw,
                    dialog.spec.severity.color(theme),
                    tokens::DETAIL,
                    true,
                );

                // Caja de entrada
                canvas.fill_rect(clip, ix, iy, iw, ih, theme.chrome_bg());
                let input_border = if dialog.is_confirmed() {
                    theme.green()
                } else {
                    theme.chrome_line_strong()
                };
                canvas.rounded_stroke(ix, iy, iw, ih, tokens::RADIUS_CONTROL, input_border);

                self.render_text_input_content(
                    canvas,
                    &dialog.input,
                    ix + 8,
                    self.centered_baseline(iy + ih / 2),
                    ix + iw - 8,
                    iy,
                    ih,
                    cursor_visible,
                    kw,
                    true,
                    theme,
                );

                // Indicador de coincidencia a la derecha
                let (status_text, status_col) = if dialog.is_confirmed() {
                    ("Coincide", theme.green())
                } else {
                    ("Pendiente", theme.text_muted())
                };
                self.draw_ui_text_tracked(
                    canvas,
                    status_text,
                    sx + 4,
                    self.centered_baseline(sy + sh / 2),
                    sx + sw,
                    status_col,
                    tokens::MICRO,
                    true,
                    tokens::TRACKING,
                );
            }
        }

        // =========================================================================
        // 7. Pie de diálogo: fondo base y borde superior
        // =========================================================================
        let (fx, fy, fw, fh) = rects.footer;
        canvas.fill_rect(clip, fx, fy, fw, fh, theme.chrome_bg());
        canvas.fill_rect(clip, fx, fy, fw, 1, theme.chrome_line());

        // Botón «Copiar SQL» a la izquierda en DbConfirm
        if let Some((cs_x, cs_y, cs_w, cs_h)) = rects.copy_sql_btn {
            let cs_col = if dialog.hovered_copy_sql {
                theme.foreground
            } else {
                theme.text_secondary()
            };
            let cs_base = self.centered_baseline(cs_y + cs_h / 2);
            self.draw_ui_text(
                canvas,
                "Copiar SQL",
                cs_x,
                cs_base,
                cs_x + cs_w,
                cs_col,
                tokens::BODY,
                false,
            );
            // Subrayado del enlace
            canvas.fill_rect(clip, cs_x, cs_base + 2, 64, 1, cs_col);
        }

        // Atajo "Esc cancela" en el footer
        if let Some((esc_x, esc_y)) = rects.esc_hint_pos {
            self.draw_ui_text(
                canvas,
                "Esc cancela",
                esc_x,
                self.centered_baseline(esc_y),
                esc_x + 120,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );
        }

        // 8. Botones del diálogo
        for (i, (&(bx, by, bw, bh), btn)) in
            rects.buttons.iter().zip(&dialog.spec.buttons).enumerate()
        {
            let is_hovered = dialog.hovered_button == Some(i);
            let is_enabled = dialog.is_button_enabled(i);

            let (bg, border, fg) = if !is_enabled {
                (
                    theme.surface_level(0),
                    theme.chrome_line(),
                    theme.text_muted(),
                )
            } else {
                match btn.kind {
                    DialogButtonKind::Ghost => {
                        let bg = if is_hovered {
                            theme.surface_hover()
                        } else {
                            Color::TRANSPARENT
                        };
                        (bg, theme.chrome_line_strong(), theme.foreground)
                    }
                    DialogButtonKind::Primary => {
                        let col = theme.accent_color();
                        let bg = if is_hovered {
                            theme.accent_soft(120)
                        } else {
                            col
                        };
                        (bg, col, theme.on_accent())
                    }
                    DialogButtonKind::Danger => {
                        let red = theme.red();
                        let bg = if is_hovered {
                            Color::from_rgba(red.r, red.g, red.b, 200)
                        } else {
                            red
                        };
                        (bg, red, theme.on_accent())
                    }
                }
            };

            if bg != Color::TRANSPARENT {
                canvas.fill_rect(clip, bx, by, bw, bh, bg);
            }
            canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_CONTROL, border);

            let btn_text_w =
                self.measure_ui_text_tracked(&btn.label, tokens::DETAIL, tokens::TRACKING);
            let btn_text_x = bx + (bw.saturating_sub(btn_text_w)) / 2;
            let btn_base = self.centered_baseline(by + bh / 2);

            self.draw_ui_text_tracked(
                canvas,
                &btn.label,
                btn_text_x,
                btn_base,
                bx + bw,
                fg,
                tokens::DETAIL,
                true,
                tokens::TRACKING,
            );
        }
    }
}
