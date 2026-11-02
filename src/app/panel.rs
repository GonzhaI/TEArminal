//! Colocación del panel lateral: lado de la ventana, coordenadas locales y el explorador alojado dentro.
//!
//! El panel mide siempre `SIDEBAR_WIDTH` y su geometría (`SidebarRects`) es local a él. A la izquierda
//! las coordenadas locales y las de ventana coinciden; a la derecha el panel se dibuja en un búfer propio
//! y los manejadores del ratón restan `panel_x`.

use super::*;
use crate::renderer::{FilesPanelRects, SessionStatus};

/// Estado de la sesión según OSC 133. `None` en Ajustes y visores de archivo.
pub(super) fn session_status(tab: &TerminalTab) -> Option<SessionStatus> {
    let terminal = &tab.active_pane()?.terminal;
    if terminal
        .active_block
        .as_ref()
        .is_some_and(|b| b.output_start.is_some())
    {
        return Some(SessionStatus::Running);
    }
    Some(
        match terminal
            .command_blocks
            .iter()
            .rev()
            .find_map(|b| b.exit_code)
        {
            Some(0) => SessionStatus::Done,
            Some(_) => SessionStatus::Failed,
            None => SessionStatus::Idle,
        },
    )
}

impl TearminalApp {
    /// El explorador siempre va a la derecha.
    pub(super) fn panel_on_right(&self) -> bool {
        true
    }

    /// ¿Se ve el panel lateral? Con pestañas verticales lista las sesiones; con las superiores, solo el
    /// explorador (las sesiones son las pestañas).
    pub(super) fn panel_shown(&self) -> bool {
        self.sidebar_visible
    }

    /// Ancho actual del panel lateral / explorador de archivos.
    pub(crate) fn panel_width(&self) -> usize {
        self.file_explorer.width
    }

    /// X de la ventana donde empieza el panel.
    pub(super) fn panel_x(&self, win_w: usize) -> usize {
        if self.panel_on_right() {
            win_w.saturating_sub(self.panel_width())
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
        let pw = self.panel_width();
        (mx >= x0 && mx < x0 + pw).then(|| mx - x0)
    }

    /// Botón flotante que devuelve la barra lateral cuando está oculta, en coordenadas de ventana.
    /// Fin (y) de la lista de sesiones del panel vertical: con el explorador visible la lista cede
    /// espacio. Con pestañas superiores el panel no tiene lista.
    /// Pestaña de la fila del panel vertical bajo `my`.
    /// Cabecera de la lista de sesiones: título fijo y, de la sesión a la vista, su rama y su carpeta.
    /// Carpeta local de la sesión (la del visor de archivos o la del shell); `None` por SSH.
    /// Segunda línea de la fila de sesión: agente, modelo y minutos si corre un agente de IA; si no,
    /// el contexto real de la pestaña (comando, carpeta o SSH).
    /// Filas de la lista de sesiones y cuántas caben: base del desplazamiento con la rueda. Sin
    /// pestañas verticales no hay lista que desplazar.
    /// Coloca el explorador: dentro del panel (bajo la lista en vertical, ocupándolo entero con pestañas
    /// superiores) y acoplado al borde derecho con el panel oculto.
    pub(super) fn sync_explorer_dock(&mut self, win_w: usize, win_h: usize) {
        self.file_explorer.dock = if self.panel_shown() && self.file_explorer.visible {
            let usable_h = win_h.saturating_sub(self.status_bar_height());
            let pw = self.panel_width();
            let (x, y, w, h) = FilesPanelRects::explorer_rect_with_width(pw, usable_h);
            Some((self.panel_x(win_w) + x, y, w, h))
        } else {
            None
        };
    }

    /// ¿Está el ratón `(mx, my)` sobre el borde que separa el panel / explorador de archivos y la terminal?
    pub(crate) fn on_panel_resize_edge(
        &self,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        if self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(|t| t.is_settings)
        {
            return false;
        }
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let usable_h = win_h.saturating_sub(status_bar_h);
        if my < top_offset || my >= usable_h {
            return false;
        }

        if self.panel_shown() {
            let pw = self.panel_width();
            let edge_x = if self.panel_on_right() {
                win_w.saturating_sub(pw)
            } else {
                pw
            };
            mx + 6 >= edge_x && mx <= edge_x + 5
        } else if self.file_explorer.visible {
            self.file_explorer
                .on_resize_edge(mx, my, win_w, win_h, top_offset, status_bar_h)
        } else {
            false
        }
    }

    /// Redimensiona el ancho del panel / explorador de archivos al arrastrar su borde con el ratón.
    pub(crate) fn resize_panel_from_mouse(&mut self, mx: usize, win_w: usize) {
        let on_right = if self.panel_shown() {
            self.panel_on_right()
        } else {
            true
        };
        let new_w = if on_right {
            win_w.saturating_sub(mx)
        } else {
            mx
        };
        self.file_explorer.width = crate::explorer::FileExplorer::clamp_width(new_w, win_w);
        if let Some(ref mut cfg_pw) = self.config.appearance.panel_width {
            *cfg_pw = self.file_explorer.width;
        }
        if self.panel_shown() {
            let win_h = self.window_size().1;
            self.sync_explorer_dock(win_w, win_h);
        }
    }
}
