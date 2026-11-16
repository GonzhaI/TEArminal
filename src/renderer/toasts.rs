//! Renderizado de avisos emergentes (toasts) con franja de gravedad semántica.
//!
//! Visualización no intrusiva en la esquina inferior derecha (DISENO §5, pantalla 11).

use super::*;
use crate::app::Toast;
use crate::renderer::geometry::ToastRects;

impl TerminalRenderer {
    /// Dibuja la pila de avisos (toasts) activos sobre la esquina inferior derecha.
    pub(crate) fn render_toasts(
        &mut self,
        canvas: &mut Canvas,
        toasts: &[Toast],
        hovered_close: Option<usize>,
        theme: &Theme,
    ) {
        if toasts.is_empty() {
            return;
        }

        let (win_w, win_h) = canvas.size();
        let clip: Clip = (0, 0, win_w, win_h);
        let rects = ToastRects::compute(win_w, win_h, toasts.len());

        for (i, (&(cx, cy, cw, ch), &(cl_x, cl_y, cl_w, cl_h))) in
            rects.cards.iter().zip(&rects.close_btns).enumerate()
        {
            let Some(toast) = toasts.get(i) else {
                continue;
            };

            // Fondo mantle con sombra discreta y contorno fuerte
            canvas.rounded_fill(cx, cy, cw, ch, tokens::RADIUS_CONTROL, theme.mantle());
            canvas.rounded_stroke(
                cx,
                cy,
                cw,
                ch,
                tokens::RADIUS_CONTROL,
                theme.chrome_line_strong(),
            );

            // Borde izquierdo de 4 px tintado por gravedad
            let bar_col = toast.severity.color(theme);
            canvas.fill_rect(clip, cx, cy, 4, ch, bar_col);

            // Texto del mensaje (1 o 2 líneas con elipsis si es largo)
            let text_x = cx + 14;
            let text_max = cl_x.saturating_sub(8);
            let avail_w = text_max.saturating_sub(text_x);
            let full_w = self.measure_ui_text(&toast.message, tokens::DETAIL, false);
            if full_w <= avail_w || avail_w < 40 {
                let baseline = self.centered_baseline(cy + ch / 2);
                self.draw_ui_text(
                    canvas,
                    &toast.message,
                    text_x,
                    baseline,
                    text_max,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            } else {
                let mut split_at = 0;
                for (idx, ch_char) in toast.message.char_indices() {
                    if ch_char.is_whitespace()
                        && self.measure_ui_text(&toast.message[..idx], tokens::DETAIL, false)
                            <= avail_w
                    {
                        split_at = idx;
                    }
                }
                let (line1, rest) = if split_at > 0 {
                    (
                        &toast.message[..split_at],
                        toast.message[split_at..].trim_start(),
                    )
                } else {
                    (toast.message.as_str(), "")
                };
                let b1 = self.centered_baseline(cy + ch / 2 - 8);
                let b2 = self.centered_baseline(cy + ch / 2 + 8);
                let line1_fit = self.truncate_ui_text(line1, avail_w, tokens::DETAIL, false);
                self.draw_ui_text(
                    canvas,
                    &line1_fit,
                    text_x,
                    b1,
                    text_max,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
                if !rest.is_empty() {
                    let line2_fit = self.truncate_ui_text(rest, avail_w, tokens::DETAIL, false);
                    self.draw_ui_text(
                        canvas,
                        &line2_fit,
                        text_x,
                        b2,
                        text_max,
                        theme.text_secondary(),
                        tokens::DETAIL,
                        false,
                    );
                }
            }

            // Botón de cierre `×`
            let is_hovered = hovered_close == Some(i);
            if is_hovered {
                canvas.rounded_fill(
                    cl_x,
                    cl_y,
                    cl_w,
                    cl_h,
                    tokens::RADIUS_ROW,
                    theme.surface_hover(),
                );
            }
            let close_col = if is_hovered {
                theme.foreground
            } else {
                theme.text_muted()
            };
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::X,
                cl_x + cl_w / 2,
                cl_y + cl_h / 2,
                12.0,
                close_col,
            );
        }
    }
}
