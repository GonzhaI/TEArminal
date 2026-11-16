//! Vistas de estado reutilizables: vacío, cargando (esqueleto), error y sin permisos.
//!
//! Implementa las pantallas de referencia de DISENO §5 y `design/States.dc.html`.

use super::*;
use crate::ui::LucideIcon;

/// Insignia de motor o tecnología (por ejemplo PG, MY, MS, MA en bases de datos).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TechBadge {
    pub code: &'static str,
    pub color: Color,
}

/// Estado de una verificación técnica individual (ej. túnel ssh activo, certificado leído).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateCheck {
    pub label: String,
    pub ok: bool,
}

#[allow(dead_code)]
impl TerminalRenderer {
    /// Renderiza un estado vacío con mensaje descriptivo y botones de acción principales.
    pub(crate) fn render_empty_state(
        &mut self,
        canvas: &mut Canvas,
        container: Rect,
        header_title: Option<&str>,
        badges: &[TechBadge],
        title: &str,
        description: &str,
        action_labels: &[&str],
        hovered_button: Option<usize>,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);
        let (cx, cy, cw, ch) = container;

        // Fondo base y contorno
        canvas.fill_rect(clip, cx, cy, cw, ch, theme.chrome_bg());
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.chrome_line());

        // Cabecera opcional
        let mut content_top = cy;
        if let Some(ht) = header_title {
            canvas.fill_rect(clip, cx, cy, cw, 34, theme.mantle());
            canvas.fill_rect(clip, cx, cy + 33, cw, 1, theme.chrome_line());
            self.draw_ui_text_tracked(
                canvas,
                ht,
                cx + 14,
                self.centered_baseline(cy + 17),
                cx + cw - 14,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                tokens::TRACKING,
            );
            content_top += 34;
        }

        let content_h = (cy + ch).saturating_sub(content_top);
        let center_y = content_top + content_h / 2;

        // 1. Insignias tecnológicas opcionales
        let mut cur_y = center_y.saturating_sub(60);
        if !badges.is_empty() {
            let total_w = badges.len() * 34 + badges.len().saturating_sub(1) * 6;
            let mut bx = cx + (cw.saturating_sub(total_w)) / 2;
            for b in badges {
                canvas.fill_rect(
                    clip,
                    bx,
                    cur_y,
                    34,
                    34,
                    Color::from_rgba(b.color.r, b.color.g, b.color.b, 40),
                );
                canvas.rounded_stroke(bx, cur_y, 34, 34, tokens::RADIUS_ROW, b.color);
                self.draw_mono_text(
                    canvas,
                    b.code,
                    bx + 8,
                    self.centered_baseline(cur_y + 17),
                    bx + 34,
                    theme.foreground,
                    tokens::DETAIL,
                    true,
                );
                bx += 40;
            }
            cur_y += 46;
        }

        // 2. Título en mayúsculas
        let title_w = self.measure_ui_text_tracked(title, tokens::DISPLAY, tokens::TRACKING);
        let title_x = cx + (cw.saturating_sub(title_w)) / 2;
        self.draw_ui_text_tracked(
            canvas,
            title,
            title_x,
            self.centered_baseline(cur_y + 10),
            cx + cw - 14,
            theme.foreground,
            tokens::DISPLAY,
            true,
            tokens::TRACKING,
        );
        cur_y += 28;

        // 3. Descripción
        let desc_w = self.measure_ui_text(description, tokens::BODY, false);
        let desc_x = cx + (cw.saturating_sub(desc_w)) / 2;
        self.draw_ui_text(
            canvas,
            description,
            desc_x.max(cx + 16),
            self.centered_baseline(cur_y + 10),
            cx + cw - 16,
            theme.text_secondary(),
            tokens::BODY,
            false,
        );
        cur_y += 34;

        // 4. Botones de acción
        let button_widths: Vec<usize> = action_labels
            .iter()
            .map(|lbl| self.measure_ui_text_tracked(lbl, tokens::DETAIL, tokens::TRACKING) + 28)
            .collect();

        let total_btn_w: usize =
            button_widths.iter().sum::<usize>() + button_widths.len().saturating_sub(1) * 8;
        let mut btn_x = cx + (cw.saturating_sub(total_btn_w)) / 2;

        for (i, (&bw, &lbl)) in button_widths.iter().zip(action_labels).enumerate() {
            let is_hovered = hovered_button == Some(i);
            let is_primary = i == 0;

            let (bg, border, fg) = if is_primary {
                let col = theme.accent_color();
                let bg = if is_hovered {
                    theme.accent_soft(120)
                } else {
                    col
                };
                (bg, col, theme.on_accent())
            } else {
                let bg = if is_hovered {
                    theme.surface_hover()
                } else {
                    Color::TRANSPARENT
                };
                (bg, theme.chrome_line_strong(), theme.foreground)
            };

            if bg != Color::TRANSPARENT {
                canvas.fill_rect(clip, btn_x, cur_y, bw, 32, bg);
            }
            canvas.rounded_stroke(btn_x, cur_y, bw, 32, tokens::RADIUS_CONTROL, border);

            let txt_w = self.measure_ui_text_tracked(lbl, tokens::DETAIL, tokens::TRACKING);
            let tx = btn_x + (bw.saturating_sub(txt_w)) / 2;
            self.draw_ui_text_tracked(
                canvas,
                lbl,
                tx,
                self.centered_baseline(cur_y + 16),
                btn_x + bw,
                fg,
                tokens::DETAIL,
                true,
                tokens::TRACKING,
            );

            btn_x += bw + 8;
        }
    }

    /// Renderiza un estado de carga con filas de esqueleto (skeleton) y botón de cancelación opcional.
    pub(crate) fn render_loading_state(
        &mut self,
        canvas: &mut Canvas,
        container: Rect,
        title: &str,
        cancel_hovered: bool,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);
        let (cx, cy, cw, ch) = container;

        canvas.fill_rect(clip, cx, cy, cw, ch, theme.chrome_bg());
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.chrome_line());

        // Cabecera de estado
        let title_baseline = self.centered_baseline(cy + 24);
        self.draw_ui_text_tracked(
            canvas,
            title,
            cx + 20,
            title_baseline,
            cx + cw - 120,
            theme.foreground,
            tokens::TITLE,
            true,
            tokens::TRACKING,
        );

        // Botón Cancelar en la esquina superior derecha
        let cancel_w = 84;
        let cancel_x = cx + cw.saturating_sub(cancel_w + 20);
        let cancel_y = cy + 10;
        let cancel_bg = if cancel_hovered {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        if cancel_bg != Color::TRANSPARENT {
            canvas.fill_rect(clip, cancel_x, cancel_y, cancel_w, 28, cancel_bg);
        }
        canvas.rounded_stroke(
            cancel_x,
            cancel_y,
            cancel_w,
            28,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );
        let ctxt_w = self.measure_ui_text_tracked("Cancelar", tokens::MICRO, tokens::TRACKING);
        let ctxt_x = cancel_x + (cancel_w.saturating_sub(ctxt_w)) / 2;
        self.draw_ui_text_tracked(
            canvas,
            "Cancelar",
            ctxt_x,
            self.centered_baseline(cancel_y + 14),
            cancel_x + cancel_w,
            theme.foreground,
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );

        // Filas esqueleto simuladas
        let row_w = cw.saturating_sub(40);
        let fractions = [0.38, 0.28, 0.44, 0.32, 0.40, 0.24];
        let mut row_y = cy + 54;
        for &f in &fractions {
            if row_y + 24 > cy + ch {
                break;
            }
            let w1 = ((row_w as f32) * f).round() as usize;
            let w2 = ((row_w as f32) * (0.22)).round() as usize;
            canvas.fill_rect(clip, cx + 20, row_y, w1, 16, theme.surface_level(0));
            canvas.fill_rect(
                clip,
                cx + 20 + w1 + 14,
                row_y,
                w2,
                16,
                theme.surface_level(0),
            );
            row_y += 32;
        }
    }

    /// Renderiza un estado de error con verificaciones (checks ✓ / ×) y acciones sugeridas.
    pub(crate) fn render_error_state(
        &mut self,
        canvas: &mut Canvas,
        container: Rect,
        header_title: &str,
        error_title: &str,
        detail_msg: &str,
        checks: &[StateCheck],
        action_labels: &[&str],
        hovered_button: Option<usize>,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);
        let (cx, cy, cw, ch) = container;

        canvas.fill_rect(clip, cx, cy, cw, ch, theme.chrome_bg());
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.red());

        // Cabecera tintada en rojo (fill_rect ignora alfa, premezclamos sobre chrome_bg)
        let red_tint = Color::from_u32_xrgb(theme.chrome_bg().blend_over(theme.red(), 40));
        canvas.fill_rect(clip, cx, cy, cw, 34, red_tint);
        canvas.fill_rect(clip, cx, cy + 33, cw, 1, theme.chrome_line());
        self.draw_ui_text_tracked(
            canvas,
            header_title,
            cx + 14,
            self.centered_baseline(cy + 17),
            cx + cw - 14,
            theme.foreground,
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );

        let mut cur_y = cy + 48;
        // Título del error
        self.draw_ui_text(
            canvas,
            error_title,
            cx + 16,
            self.centered_baseline(cur_y),
            cx + cw - 16,
            theme.foreground,
            tokens::TITLE,
            true,
        );
        cur_y += 24;

        // Mensaje de detalle
        self.draw_ui_text(
            canvas,
            detail_msg,
            cx + 16,
            self.centered_baseline(cur_y),
            cx + cw - 16,
            theme.text_secondary(),
            tokens::BODY,
            false,
        );
        cur_y += 28;

        // Lista de checks con ✓ o ×
        for check in checks {
            if cur_y + 24 > cy + ch.saturating_sub(44) {
                break;
            }
            let (icon, bg) = if check.ok {
                (crate::ui::LucideIcon::Check, theme.green())
            } else {
                (crate::ui::LucideIcon::X, theme.red())
            };
            canvas.fill_rect(clip, cx + 16, cur_y - 2, 18, 18, bg);
            self.draw_lucide_icon(
                canvas,
                icon,
                cx + 16 + 9,
                cur_y - 2 + 9,
                11.0,
                theme.on_accent(),
            );
            self.draw_ui_text(
                canvas,
                &check.label,
                cx + 42,
                self.centered_baseline(cur_y + 7),
                cx + cw - 16,
                theme.foreground,
                tokens::BODY,
                false,
            );
            cur_y += 26;
        }

        // Botones de acción al pie
        let btn_y = (cy + ch).saturating_sub(42);
        let button_widths: Vec<usize> = action_labels
            .iter()
            .map(|lbl| self.measure_ui_text_tracked(lbl, tokens::DETAIL, tokens::TRACKING) + 24)
            .collect();

        let mut bx = cx + 16;
        for (i, (&bw, &lbl)) in button_widths.iter().zip(action_labels).enumerate() {
            let is_hovered = hovered_button == Some(i);
            let is_primary = i == 0;
            let (bg, border, fg) = if is_primary {
                let col = theme.accent_color();
                let bg = if is_hovered {
                    theme.accent_soft(120)
                } else {
                    col
                };
                (bg, col, theme.on_accent())
            } else {
                let bg = if is_hovered {
                    theme.surface_hover()
                } else {
                    Color::TRANSPARENT
                };
                (bg, theme.chrome_line_strong(), theme.foreground)
            };

            if bg != Color::TRANSPARENT {
                canvas.fill_rect(clip, bx, btn_y, bw, 30, bg);
            }
            canvas.rounded_stroke(bx, btn_y, bw, 30, tokens::RADIUS_CONTROL, border);

            let tw = self.measure_ui_text_tracked(lbl, tokens::DETAIL, tokens::TRACKING);
            let tx = bx + (bw.saturating_sub(tw)) / 2;
            self.draw_ui_text_tracked(
                canvas,
                lbl,
                tx,
                self.centered_baseline(btn_y + 15),
                bx + bw,
                fg,
                tokens::DETAIL,
                true,
                tokens::TRACKING,
            );
            bx += bw + 8;
        }
    }

    /// Renderiza un estado de falta de permisos con candado amarillo.
    pub(crate) fn render_no_permission_state(
        &mut self,
        canvas: &mut Canvas,
        container: Rect,
        title: &str,
        message: &str,
        copy_hovered: bool,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);
        let (cx, cy, cw, ch) = container;

        canvas.fill_rect(clip, cx, cy, cw, ch, theme.chrome_bg());
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.chrome_line());

        let center_y = cy + ch / 2;

        // Candado en amarillo
        self.draw_lucide_icon(
            canvas,
            LucideIcon::Lock,
            cx + cw / 2,
            center_y.saturating_sub(44),
            32.0,
            theme.yellow(),
        );

        // Título
        let tw = self.measure_ui_text_tracked(title, tokens::SECTION, tokens::TRACKING);
        let tx = cx + (cw.saturating_sub(tw)) / 2;
        self.draw_ui_text_tracked(
            canvas,
            title,
            tx,
            self.centered_baseline(center_y),
            cx + cw - 16,
            theme.foreground,
            tokens::SECTION,
            true,
            tokens::TRACKING,
        );

        // Mensaje
        let mw = self.measure_ui_text(message, tokens::BODY, false);
        let mx = cx + (cw.saturating_sub(mw)) / 2;
        self.draw_ui_text(
            canvas,
            message,
            mx.max(cx + 16),
            self.centered_baseline(center_y + 26),
            cx + cw - 16,
            theme.text_secondary(),
            tokens::BODY,
            false,
        );

        // Botón "Copiar el error"
        let btn_w = 140;
        let bx = cx + (cw.saturating_sub(btn_w)) / 2;
        let by = center_y + 54;
        let bg = if copy_hovered {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        if bg != Color::TRANSPARENT {
            canvas.fill_rect(clip, bx, by, btn_w, 30, bg);
        }
        canvas.rounded_stroke(
            bx,
            by,
            btn_w,
            30,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );
        let l_w = self.measure_ui_text_tracked("Copiar el error", tokens::MICRO, tokens::TRACKING);
        let lx = bx + (btn_w.saturating_sub(l_w)) / 2;
        self.draw_ui_text_tracked(
            canvas,
            "Copiar el error",
            lx,
            self.centered_baseline(by + 15),
            bx + btn_w,
            theme.foreground,
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );
    }
}
