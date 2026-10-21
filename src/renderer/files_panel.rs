//! Panel lateral del modo de pestañas superiores: solo el fondo y el borde; el explorador (Archivos ·
//! Cambios) se dibuja encima.

use super::*;

impl TerminalRenderer {
    /// Dibuja el fondo del panel en un búfer propio de ancho `panel_w` (como `render_sidebar`).
    /// `on_right`: el panel está a la derecha y el borde que lo separa del contenido es el izquierdo.
    #[allow(clippy::too_many_arguments)]
    pub fn render_files_panel(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        panel_w: usize,
        status_bar_h: usize,
        on_right: bool,
        theme: &Theme,
    ) {
        if panel_w == 0 || width == 0 || height == 0 {
            return;
        }
        let top = SidebarRects::TOP;
        let body_h = height.saturating_sub(status_bar_h).saturating_sub(top);
        Self::draw_rect(
            &mut Canvas::over(buffer, width, height),
            0,
            top,
            panel_w,
            body_h,
            theme.chrome_bg(),
        );
        let edge_x = if on_right { 0 } else { panel_w - 1 };
        Self::draw_rect(
            &mut Canvas::over(buffer, width, height),
            edge_x,
            top,
            1,
            body_h,
            theme.chrome_line(),
        );
    }
}
