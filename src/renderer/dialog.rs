//! Diálogo modal (`app::Dialog`): velo sobre la ventana, tarjeta centrada y botones Cancelar/Aceptar.

use super::*;
use crate::app::Dialog;

impl TerminalRenderer {
    /// Parte `text` en líneas de como mucho `max_w` px, cortando por palabras (`max_lines` como tope;
    /// la última se recorta con «…» si sobra texto).
    fn wrap_ui_text(&self, text: &str, max_w: usize, size: f32, max_lines: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let candidate = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if cur.is_empty() || self.measure_ui_text(&candidate, size, false) <= max_w {
                cur = candidate;
            } else {
                lines.push(std::mem::replace(&mut cur, word.to_string()));
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        if lines.len() > max_lines {
            lines.truncate(max_lines);
            let last = lines.pop().unwrap_or_default();
            lines.push(self.fit_ui_text(&format!("{last}…"), max_w, size, false));
        }
        lines
    }

    pub fn render_dialog(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        dialog: &Dialog,
        theme: &Theme,
    ) {
        use crate::renderer::{DialogButton, DialogRects};
        // Velo: el resto de la interfaz queda atenuado y sin respuesta.
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            0,
            0,
            width,
            height,
            0,
            Color::from_rgba(0, 0, 0, 130),
        );
        let rects = DialogRects::compute(width, height);
        let (x, y, w, h) = rects.panel;
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x,
            y + 6,
            w,
            h,
            12,
            Color::from_rgba(0, 0, 0, 90),
        );
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x,
            y,
            w,
            h,
            12,
            theme.chrome_bg_elevated(),
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            x,
            y,
            w,
            h,
            12,
            theme.chrome_line_strong(),
        );

        let text_x = x + DialogRects::PAD;
        let text_w = w.saturating_sub(DialogRects::PAD * 2);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &dialog.title,
            text_x,
            y + DialogRects::PAD + 12,
            text_x + text_w,
            theme.foreground,
            tokens::SECTION + 1.0,
            true,
        );
        // Entre el título y los botones caben tres líneas de mensaje.
        for (i, line) in self
            .wrap_ui_text(&dialog.message, text_w, tokens::BODY, 3)
            .iter()
            .enumerate()
        {
            self.draw_ui_text(
                buffer,
                width,
                height,
                line,
                text_x,
                y + DialogRects::PAD + 42 + i * 19,
                text_x + text_w,
                theme.text_secondary(),
                tokens::BODY,
                false,
            );
        }

        for (rect, label, kind) in [
            (rects.cancel, &dialog.cancel, DialogButton::Cancel),
            (rects.accept, &dialog.accept, DialogButton::Accept),
        ] {
            let hovered = dialog.hovered == Some(kind);
            let (bx, by, bw, bh) = rect;
            let text_color = if kind == DialogButton::Accept {
                let fill = if hovered {
                    theme.mix(theme.accent_color(), theme.foreground, 0.18)
                } else {
                    theme.accent_color()
                };
                crate::ui::draw_rounded_rect(buffer, width, height, bx, by, bw, bh, 8, fill);
                theme.background
            } else {
                if hovered {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        bx,
                        by,
                        bw,
                        bh,
                        8,
                        theme.surface_hover(),
                    );
                }
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    bx,
                    by,
                    bw,
                    bh,
                    8,
                    theme.chrome_line_strong(),
                );
                if hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                }
            };
            let lw = self.measure_ui_text(label, tokens::TITLE, true);
            self.draw_ui_text(
                buffer,
                width,
                height,
                label,
                bx + bw.saturating_sub(lw) / 2,
                by + bh / 2 + 4,
                bx + bw,
                text_color,
                tokens::TITLE,
                true,
            );
        }
    }
}
