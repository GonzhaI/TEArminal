//! Barra lateral vertical de sesiones y botón flotante para mostrarla.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra lateral: riel de ajustes a la izquierda y, a su lado, la lista de sesiones.
    /// Dibuja en un búfer propio de ancho `sidebar_width`: `app` lo coloca a izquierda o derecha.
    #[allow(clippy::too_many_arguments)]
    /// Resalte de una fila de lista del panel (Archivos y Cambios): inset de 8 px, esquinas redondeadas
    /// y, si está seleccionada, una marca de acento (gris cuando la lista no tiene el foco).
    pub(crate) fn draw_list_row_highlight(
        canvas: &mut Canvas,
        row: Rect,
        selected: bool,
        focused: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = row;
        let fill = if selected && !focused {
            theme.surface_bg()
        } else {
            theme.surface_hover()
        };
        let row_w = w.saturating_sub(16);
        canvas.rounded_fill(x + 8, y, row_w, h, tokens::RADIUS_ROW, fill);
        if selected && focused {
            canvas.rounded_stroke(x + 8, y, row_w, h, tokens::RADIUS_ROW, theme.card_outline());
        }
        if selected {
            let bar = if focused {
                theme.accent_color()
            } else {
                theme.chrome_line_strong()
            };
            canvas.rounded_fill(x + 12, y + 6, 2, h.saturating_sub(12), 1, bar);
        }
    }
}
