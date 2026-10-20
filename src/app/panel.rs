//! Colocación del panel lateral: lado de la ventana, coordenadas locales y el explorador alojado dentro.
//!
//! El panel mide siempre `SIDEBAR_WIDTH` y su geometría (`SidebarRects`) es local a él. A la izquierda
//! las coordenadas locales y las de ventana coinciden; a la derecha el panel se dibuja en un búfer propio
//! y los manejadores del ratón restan `panel_x`.

use super::*;
use crate::renderer::{FilesPanelRects, Rect, SidebarRects};

impl AxonApp {
    pub(super) fn panel_on_right(&self) -> bool {
        self.config.appearance.panel_on_right()
    }

    /// ¿Se ve el panel lateral? Con pestañas verticales lista proyectos y sesiones; con las superiores,
    /// solo el explorador (los proyectos están en el selector de la barra y las sesiones son las pestañas).
    pub(super) fn panel_shown(&self) -> bool {
        self.sidebar_visible
    }

    /// X de la ventana donde empieza el panel.
    pub(super) fn panel_x(&self, win_w: usize) -> usize {
        if self.panel_on_right() {
            win_w.saturating_sub(SIDEBAR_WIDTH)
        } else {
            0
        }
    }

    /// Coordenada x local al panel si `(mx, my)` cae en su parte propia. El explorador alojado dentro
    /// atiende su propio ratón, así que su rectángulo queda fuera.
    pub(super) fn panel_local_x(&self, mx: usize, my: usize, win_w: usize) -> Option<usize> {
        if !self.panel_shown() {
            return None;
        }
        let in_dock = self
            .file_explorer
            .dock
            .is_some_and(|(x, y, w, h)| mx >= x && mx < x + w && my >= y && my < y + h);
        if in_dock {
            return None;
        }
        let x0 = self.panel_x(win_w);
        (mx >= x0 && mx < x0 + SIDEBAR_WIDTH).then(|| mx - x0)
    }

    /// Ancho que el panel quita a la derecha cuando va a ese lado (Ajustes lo respeta al centrar su contenido).
    pub(super) fn panel_right_inset(&self) -> usize {
        if self.panel_shown() && self.panel_on_right() {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    /// Botón flotante que devuelve la barra lateral cuando está oculta, en coordenadas de ventana.
    pub(super) fn floating_toggle_rect(&self, win_w: usize, usable_h: usize) -> Rect {
        let x = if self.panel_on_right() {
            win_w.saturating_sub(38)
        } else {
            8
        };
        (x, usable_h.saturating_sub(38), 30, 30)
    }

    /// Fin (y) de la lista de sesiones del panel vertical: con el explorador visible la lista cede
    /// espacio. Con pestañas superiores el panel no tiene lista.
    pub(super) fn panel_list_end(&self, usable_h: usize) -> usize {
        SidebarRects::list_end(
            usable_h,
            self.panel_tab_indices().len(),
            self.file_explorer.visible,
        )
    }

    /// Filas de la lista de sesiones y cuántas caben: base del desplazamiento con la rueda. Sin
    /// pestañas verticales no hay lista que desplazar.
    pub(super) fn panel_scroll_extent(&self, usable_h: usize) -> (usize, usize) {
        if self.is_horizontal_tabs() {
            (0, 0)
        } else {
            (
                self.panel_tab_indices().len(),
                SidebarRects::visible_items(self.panel_list_end(usable_h)),
            )
        }
    }

    /// Coloca el explorador: dentro del panel (bajo la lista en vertical, ocupándolo entero con pestañas
    /// superiores) y acoplado al borde derecho con el panel oculto.
    pub(super) fn sync_explorer_dock(&mut self, win_w: usize, win_h: usize) {
        self.file_explorer.dock = if self.panel_shown() && self.file_explorer.visible {
            let usable_h = win_h.saturating_sub(self.status_bar_height());
            let (x, y, w, h) = if self.is_horizontal_tabs() {
                FilesPanelRects::explorer_rect(usable_h)
            } else {
                SidebarRects::explorer_rect_after(usable_h, self.panel_list_end(usable_h))
            };
            Some((self.panel_x(win_w) + x, y, w, h))
        } else {
            None
        };
    }
}
