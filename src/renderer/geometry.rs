//! Geometría compartida entre el dibujo y la detección de clics/hover (una sola fuente de verdad).

mod db;
mod dialog;
mod git;
mod modals;
mod settings;
mod system;
mod viewer;

pub(crate) use db::*;
pub(crate) use dialog::*;
pub(crate) use git::*;
pub(crate) use modals::*;
pub(crate) use settings::*;
pub(crate) use system::*;
pub(crate) use viewer::*;

use crate::config::AppModule;

/// Rectángulo (x, y, ancho, alto) en píxeles
pub(crate) type Rect = (usize, usize, usize, usize);

/// Comprueba si las coordenadas `(x, y)` caen dentro del rectángulo `(rx, ry, rw, rh)`.
#[inline]
pub(crate) fn rect_contains(rect: Rect, x: usize, y: usize) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// Convierte la posición Y deseada de la parte superior del pulgar (`thumb_top_y`) en un `scroll_offset`
/// dentro de `0..=max_offset` para una barra de desplazamiento vertical `(track, thumb)`.
#[inline]
pub(crate) fn scroll_offset_for_thumb_y(
    track: Rect,
    thumb: Rect,
    thumb_top_y: f32,
    max_offset: usize,
) -> Option<usize> {
    let available_travel = track.3.saturating_sub(thumb.3);
    if available_travel == 0 || max_offset == 0 {
        return None;
    }
    let rel_y = (thumb_top_y - track.1 as f32).clamp(0.0, available_travel as f32);
    let ratio = rel_y / available_travel as f32;
    Some(((ratio * max_offset as f32).round() as usize).min(max_offset))
}

/// Rectángulo del botón flotante "ir hasta abajo" en el panel de la terminal
#[inline]
pub(crate) fn pane_scroll_to_bottom_btn_rect(
    rect_x: usize,
    rect_y: usize,
    rect_width: usize,
    rect_height: usize,
) -> Rect {
    let size = 26;
    let margin_right = 20;
    let margin_bottom = 16;
    let x = (rect_x + rect_width).saturating_sub(size + margin_right);
    let y = (rect_y + rect_height).saturating_sub(size + margin_bottom);
    (x, y, size, size)
}

/// Rectángulo de la barra de búsqueda del historial (`Ctrl + Shift + F`), arriba a la derecha del panel
#[inline]
pub(crate) fn pane_search_bar_rect(rect_x: usize, rect_y: usize, rect_width: usize) -> Rect {
    let w = 280.min(rect_width.saturating_sub(40));
    let h = 32;
    let x = (rect_x + rect_width).saturating_sub(w + 20);
    (x, rect_y + 10, w, h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TopTabBarRects {
    pub bar_h: usize,
    /// Borde izquierdo de la barra: riel + panel lateral ocupan lo que hay a su izquierda.
    pub left_x: usize,
    /// Rectángulo del título de marca «TEARMINAL» (DISENO §4)
    pub brand_rect: Rect,
    /// Buscador «Buscar o ejecutar… Ctrl+K» (290×26 px)
    pub search_btn: Rect,
    pub tabs_start_x: usize,
    pub tab_w: usize,
    pub tab_gap: usize,
    pub plus_btn: Rect,
    /// Píldora «Files»: retrocompatibilidad de tipos
    pub files_btn: Rect,
    /// Píldora «Changes»: retrocompatibilidad de tipos
    pub changes_btn: Rect,
    pub win_min_btn: Rect,
    pub win_max_btn: Rect,
    pub win_close_btn: Rect,
}

impl TopTabBarRects {
    pub(crate) const HEIGHT: usize = 40;
    pub(crate) const TAB_GAP: usize = 0;
    pub(crate) const CLOSE_W: usize = 24;
    pub(crate) const PLUS_W: usize = 32;
    pub(crate) const SEARCH_W: usize = 290;
    pub(crate) const SEARCH_H: usize = 26;
    pub(crate) const LEFT_PAD: usize = 140;

    /// `width` es el ancho de la ventana y `left_x` el borde izquierdo de la barra (ancho de riel + panel lateral);
    /// las pestañas empiezan alineadas a continuación del nombre de marca.
    pub(crate) fn compute(width: usize, left_x: usize, tabs_count: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 46;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);

        let left_x = left_x.max(Self::LEFT_PAD);
        let brand_rect = (0, 0, left_x, bar_h);
        let tabs_start_x = left_x;

        // Buscador de 290×26 px centrado verticalmente a 10 px a la izquierda de los controles de ventana
        let search_y = (bar_h.saturating_sub(Self::SEARCH_H)) / 2;
        let right_limit = win_min_btn.0.saturating_sub(10);
        let search_btn = if right_limit > tabs_start_x + Self::PLUS_W + 120 + Self::SEARCH_W {
            (
                right_limit - Self::SEARCH_W,
                search_y,
                Self::SEARCH_W,
                Self::SEARCH_H,
            )
        } else if right_limit > tabs_start_x + Self::PLUS_W + 120 + 120 {
            let sw = (right_limit - (tabs_start_x + Self::PLUS_W + 120)).min(Self::SEARCH_W);
            (right_limit - sw, search_y, sw, Self::SEARCH_H)
        } else {
            (0, 0, 0, 0)
        };

        let tabs_end_x = if search_btn.2 > 0 {
            search_btn.0
        } else {
            win_min_btn.0.saturating_sub(10)
        };

        let available = tabs_end_x.saturating_sub(tabs_start_x + Self::PLUS_W + 12);
        let tab_w = available
            .checked_div(tabs_count)
            .map_or(177, |w| w.clamp(16, 220));

        let plus_x = if tabs_count > 0 {
            (tabs_start_x + tabs_count * (tab_w + Self::TAB_GAP) + 4)
                .min(tabs_end_x.saturating_sub(Self::PLUS_W + 4))
        } else {
            tabs_start_x + 4
        };
        let plus_btn = (
            plus_x,
            (bar_h.saturating_sub(Self::PLUS_W)) / 2,
            Self::PLUS_W,
            Self::PLUS_W,
        );

        Self {
            bar_h,
            left_x,
            brand_rect,
            search_btn,
            tabs_start_x,
            tab_w,
            tab_gap: Self::TAB_GAP,
            plus_btn,
            files_btn: (0, 0, 0, 0),
            changes_btn: (0, 0, 0, 0),
            win_min_btn,
            win_max_btn,
            win_close_btn,
        }
    }

    #[inline]
    pub(crate) fn tab_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        (
            self.tabs_start_x + idx * (self.tab_w + self.tab_gap),
            0,
            self.tab_w,
            Self::HEIGHT,
        )
    }

    #[inline]
    pub(crate) fn close_btn_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        let tab = self.tab_rect(idx);
        if self.tab_w >= 76 {
            (
                tab.0 + tab.2.saturating_sub(Self::CLOSE_W + 6),
                (Self::HEIGHT - Self::CLOSE_W) / 2,
                Self::CLOSE_W,
                Self::CLOSE_W,
            )
        } else {
            (0, 0, 0, 0)
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect_contains(rect, mx, my)
    }
}

/// Destino interactivo en el riel de módulos
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RailTarget {
    Module(AppModule),
    Settings,
}

/// Geometría del riel izquierdo de módulos (56 px de ancho, de y = 40 a win_h - status_bar_h)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RailRects {
    pub(crate) rect: Rect,
    pub(crate) spaces_btn: Rect,
    pub(crate) files_btn: Rect,
    pub(crate) git_btn: Rect,
    pub(crate) data_btn: Rect,
    pub(crate) system_btn: Rect,
    pub(crate) settings_btn: Rect,
}

impl RailRects {
    pub(crate) const WIDTH: usize = 68;
    pub(crate) const BTN_H: usize = 58;

    pub(crate) fn compute(win_h: usize, status_bar_h: usize) -> Self {
        let top = TopTabBarRects::HEIGHT;
        let total_h = win_h.saturating_sub(top + status_bar_h);
        let w = Self::WIDTH;
        let h = Self::BTN_H;

        let spaces_btn = (0, top, w, h);
        let files_btn = (0, top + h, w, h);
        let git_btn = (0, top + 2 * h, w, h);
        let data_btn = (0, top + 3 * h, w, h);
        let system_btn = (0, top + 4 * h, w, h);

        let settings_y = (top + total_h).saturating_sub(h);
        let settings_btn = (0, settings_y, w, h);

        Self {
            rect: (0, top, w, total_h),
            spaces_btn,
            files_btn,
            git_btn,
            data_btn,
            system_btn,
            settings_btn,
        }
    }

    #[inline]
    pub(crate) fn module_btn(&self, module: AppModule) -> Rect {
        match module {
            AppModule::Spaces => self.spaces_btn,
            AppModule::Files => self.files_btn,
            AppModule::Git => self.git_btn,
            AppModule::Data => self.data_btn,
            AppModule::System => self.system_btn,
        }
    }

    #[inline]
    pub(crate) fn button_at(&self, mx: usize, my: usize) -> Option<RailTarget> {
        if !rect_contains(self.rect, mx, my) {
            return None;
        }
        if rect_contains(self.spaces_btn, mx, my) {
            Some(RailTarget::Module(AppModule::Spaces))
        } else if rect_contains(self.files_btn, mx, my) {
            Some(RailTarget::Module(AppModule::Files))
        } else if rect_contains(self.git_btn, mx, my) {
            Some(RailTarget::Module(AppModule::Git))
        } else if rect_contains(self.data_btn, mx, my) {
            Some(RailTarget::Module(AppModule::Data))
        } else if rect_contains(self.system_btn, mx, my) {
            Some(RailTarget::Module(AppModule::System))
        } else if rect_contains(self.settings_btn, mx, my) {
            Some(RailTarget::Settings)
        } else {
            None
        }
    }
}

/// Geometría del panel lateral contextual (264 px de ancho, anclado a x = 56)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ContextualSidebarRects {
    pub(crate) width: usize,
    pub(crate) collapsed: bool,
    pub(crate) rect: Rect,
    pub(crate) header: Rect,
    pub(crate) toggle_btn: Rect,
    pub(crate) action_btn: Rect,
    pub(crate) content: Rect,
    pub(crate) footer: Option<Rect>,
    pub(crate) terminal_btn: Option<Rect>,
    pub(crate) add_machine_btn: Option<Rect>,
}

impl ContextualSidebarRects {
    pub(crate) const WIDTH: usize = 264;
    pub(crate) const HEADER_H: usize = 40;
    pub(crate) const BTN_SIZE: usize = 26;
    pub(crate) const FOOTER_H: usize = 48;

    pub(crate) fn compute(
        collapsed: bool,
        module: AppModule,
        win_h: usize,
        status_bar_h: usize,
    ) -> Self {
        if collapsed {
            return Self {
                width: 0,
                collapsed: true,
                rect: (RailRects::WIDTH, TopTabBarRects::HEIGHT, 0, 0),
                header: (0, 0, 0, 0),
                toggle_btn: (0, 0, 0, 0),
                action_btn: (0, 0, 0, 0),
                content: (0, 0, 0, 0),
                footer: None,
                terminal_btn: None,
                add_machine_btn: None,
            };
        }

        let x = RailRects::WIDTH;
        let y = TopTabBarRects::HEIGHT;
        let w = Self::WIDTH;
        let total_h = win_h.saturating_sub(y + status_bar_h);

        let header = (x, y, w, Self::HEADER_H);
        let btn_y = y + (Self::HEADER_H.saturating_sub(Self::BTN_SIZE)) / 2;
        let toggle_btn = (
            x + w.saturating_sub(Self::BTN_SIZE + 8),
            btn_y,
            Self::BTN_SIZE,
            Self::BTN_SIZE,
        );
        let action_btn = if matches!(
            module,
            AppModule::Spaces | AppModule::Files | AppModule::Git
        ) {
            (
                toggle_btn.0.saturating_sub(Self::BTN_SIZE + 4),
                btn_y,
                Self::BTN_SIZE,
                Self::BTN_SIZE,
            )
        } else {
            (0, 0, 0, 0)
        };

        let (content, footer, terminal_btn, add_machine_btn) = if module == AppModule::Spaces {
            let foot_y = (y + total_h).saturating_sub(Self::FOOTER_H);
            let footer_rect = (x, foot_y, w, Self::FOOTER_H);
            let pad = 12;
            let gap = 8;
            let btn_w = (w.saturating_sub(2 * pad + gap)) / 2;
            let t_btn = (x + pad, foot_y + 10, btn_w, 28);
            let m_btn = (x + pad + btn_w + gap, foot_y + 10, btn_w, 28);
            let cnt = (
                x,
                y + Self::HEADER_H,
                w,
                foot_y.saturating_sub(y + Self::HEADER_H),
            );
            (cnt, Some(footer_rect), Some(t_btn), Some(m_btn))
        } else {
            let cnt = (
                x,
                y + Self::HEADER_H,
                w,
                total_h.saturating_sub(Self::HEADER_H),
            );
            (cnt, None, None, None)
        };

        Self {
            width: w,
            collapsed: false,
            rect: (x, y, w, total_h),
            header,
            toggle_btn,
            action_btn,
            content,
            footer,
            terminal_btn,
            add_machine_btn,
        }
    }
}

/// Geometría de la barra lateral: riel de ajustes y columna de sesiones (una sola fuente para dibujo,
/// clic, hover y arrastre).
///
/// La barra empieza en `TOP` (bajo la barra de título) para no tapar la marca. Las coordenadas de sus
/// métodos son locales a la barra, que se dibuja en su propio búfer y se coloca a izquierda o derecha:
/// clic y dibujo no repiten la retícula.
pub(crate) struct SidebarRects;

impl SidebarRects {
    pub(crate) const WIDTH: usize = 328;
    /// Ancho del riel (solo lleva el botón de ajustes); la columna de sesiones ocupa el resto.
    #[cfg(test)]
    pub(crate) const RAIL_W: usize = 60;
    pub(crate) const TOP: usize = 38;
    /// Marco del panel de pestañas superiores: margen y radio de la tarjeta (`Canvas::card`).
    pub(crate) const CARD_INSET: usize = 6;
    pub(crate) const CARD_RADIUS: usize = super::tokens::RADIUS_CARD;
    /// Margen derecho del contenido del panel (contadores, estados, insignias de git): deja aire
    /// dentro de la tarjeta y alinea los bordes de todas las secciones.
    pub(crate) const PAD_R: usize = 20;
    /// Margen derecho de las filas de sesión: el recorte de la tarjeta del panel más 8 px de aire.
    #[cfg(test)]
    pub(crate) const ROW_PAD_R: usize = Self::CARD_INSET + 8;
    /// Título, rama y carpeta.
    #[cfg(test)]
    pub(crate) const HEADER_H: usize = 66;
    /// Y de la primera fila.
    #[cfg(test)]
    pub(crate) const LIST_START_Y: usize = Self::TOP + Self::HEADER_H + 8;
    #[cfg(test)]
    pub(crate) const TAB_ITEM_H: usize = 48;
    #[cfg(test)]
    pub(crate) const TAB_SPACING: usize = 2;
    /// X de las filas de sesión (junto al riel).
    #[cfg(test)]
    pub(crate) const PAD_X: usize = Self::RAIL_W + 8;

    /// Rectángulo del explorador alojado bajo la lista de sesiones.
    #[cfg(test)]
    pub(crate) fn explorer_rect_after(usable_h: usize, list_end: usize) -> Rect {
        Self::explorer_rect_after_with_width(Self::WIDTH, usable_h, list_end)
    }

    /// Rectángulo del explorador alojado bajo la lista de sesiones para un ancho de panel dado.
    #[cfg(test)]
    pub(crate) fn explorer_rect_after_with_width(
        panel_w: usize,
        usable_h: usize,
        list_end: usize,
    ) -> Rect {
        let top = list_end + 8;
        let bottom = Self::content_bottom(usable_h);
        (
            Self::RAIL_W,
            top,
            // Termina en el borde interior de la tarjeta del panel, no bajo su margen.
            panel_w.saturating_sub(Self::RAIL_W + Self::CARD_INSET),
            bottom.saturating_sub(top),
        )
    }

    /// Y absoluta de la fila `vis_idx` (índice dentro de la ventana visible).
    #[cfg(test)]
    #[inline]
    pub(crate) fn row_y(vis_idx: usize) -> usize {
        Self::LIST_START_Y + vis_idx * (Self::TAB_ITEM_H + Self::TAB_SPACING)
    }

    /// Borde inferior del contenido del panel (sin el pie, que ya no existe).
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn content_bottom(usable_h: usize) -> usize {
        usable_h.saturating_sub(8)
    }

    /// Rectángulo de la fila `idx` de la lista, dado el desplazamiento actual.
    #[cfg(test)]
    #[inline]
    pub(crate) fn tab_rect(idx: usize, scroll_offset: usize) -> Rect {
        Self::tab_rect_with_width(idx, scroll_offset, Self::WIDTH)
    }

    /// Rectángulo de la fila `idx` de la lista para un ancho de panel dado.
    #[cfg(test)]
    #[inline]
    pub(crate) fn tab_rect_with_width(idx: usize, scroll_offset: usize, panel_w: usize) -> Rect {
        (
            Self::PAD_X,
            Self::row_y(idx.saturating_sub(scroll_offset)),
            panel_w.saturating_sub(Self::PAD_X + Self::ROW_PAD_R),
            Self::TAB_ITEM_H,
        )
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect_contains(rect, mx, my)
    }
}

/// Un elemento de la lista del panel de Spaces: la cabecera de un grupo o un Space (posición en `items`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacesSlot {
    Header { remote: bool },
    Space(usize),
}

/// Panel izquierdo de Spaces, a altura completa y anclado a x = 0 (coordenadas de ventana): cabecera con
/// marca, búsqueda y alternar; la lista, dividida en «Este equipo» y «Equipos remotos» (cada grupo con su
/// `+`); y el pie con Ajustes y Add machine. Colapsado es un riel de `RAIL_W` con los mismos controles solo
/// con icono. Dibujo, clic y hover parten de `compute` y `visible_slots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SpacesPanelRects {
    pub width: usize,
    pub collapsed: bool,
    pub toggle_btn: Rect,
    /// Lupa de la cabecera (abre la paleta); vacía en el riel.
    pub search_btn: Rect,
    /// «+» de Este equipo en el riel; expandido cada cabecera de grupo lleva el suyo (`header_plus`).
    pub plus_btn: Rect,
    pub list_top: usize,
    pub list_bottom: usize,
    pub settings_btn: Rect,
    pub machine_btn: Rect,
    pub terminal_btn: Rect,
}

impl SpacesPanelRects {
    pub(crate) const WIDTH: usize = RailRects::WIDTH + ContextualSidebarRects::WIDTH;
    pub(crate) const RAIL_W: usize = RailRects::WIDTH;
    /// La cabecera mide como la barra superior para que las dos filas queden alineadas.
    pub(crate) const HEADER_H: usize = TopTabBarRects::HEIGHT;
    pub(crate) const ROW_H: usize = 32;
    pub(crate) const ROW_GAP: usize = 2;
    pub(crate) const PAD_X: usize = 12;
    pub(crate) const BTN: usize = 26;
    /// Alto de la cabecera de un grupo y lado de su «+».
    pub(crate) const GROUP_H: usize = 34;
    pub(crate) const GROUP_BTN: usize = 24;
    /// Lado de la insignia de letra de cada Space.
    pub(crate) const BADGE: usize = 18;
    const FOOT_H: usize = 48;

    pub(crate) fn width_for(collapsed: bool) -> usize {
        if collapsed {
            Self::RAIL_W
        } else {
            Self::WIDTH
        }
    }

    pub(crate) fn compute(collapsed: bool, win_h: usize) -> Self {
        let width = Self::width_for(collapsed);
        let none = (0, 0, 0, 0);
        let settings_y = win_h.saturating_sub(28 + 58);
        let settings_btn = (0, settings_y, Self::RAIL_W, 58);

        if collapsed {
            Self {
                width,
                collapsed,
                toggle_btn: none,
                search_btn: none,
                plus_btn: none,
                list_top: 0,
                list_bottom: 0,
                settings_btn,
                machine_btn: none,
                terminal_btn: none,
            }
        } else {
            let x = Self::RAIL_W;
            let sidebar_w = Self::WIDTH - Self::RAIL_W;
            let btn_y = Self::HEADER_H + (Self::HEADER_H.saturating_sub(Self::BTN)) / 2;
            let toggle_btn = (
                x + sidebar_w.saturating_sub(Self::BTN + 8),
                btn_y,
                Self::BTN,
                Self::BTN,
            );
            let plus_btn = (
                toggle_btn.0.saturating_sub(Self::BTN + 4),
                btn_y,
                Self::BTN,
                Self::BTN,
            );

            let foot_y = win_h.saturating_sub(28 + Self::FOOT_H);
            let pad = 12;
            let gap = 8;
            let btn_w = (sidebar_w.saturating_sub(2 * pad + gap)) / 2;
            let terminal_btn = (x + pad, foot_y + 10, btn_w, 28);
            let machine_btn = (x + pad + btn_w + gap, foot_y + 10, btn_w, 28);

            Self {
                width,
                collapsed,
                toggle_btn,
                search_btn: none,
                plus_btn,
                list_top: Self::HEADER_H + 40 + 4,
                list_bottom: foot_y.saturating_sub(6),
                settings_btn,
                machine_btn,
                terminal_btn,
            }
        }
    }

    pub(crate) fn contains(&self, mx: usize, my: usize, win_h: usize) -> bool {
        mx < self.width && my < win_h
    }

    /// «+» de la cabecera de un grupo (solo expandido).
    pub(crate) fn header_plus(header: Rect) -> Rect {
        (
            header.0 + header.2 - Self::GROUP_BTN,
            header.1 + (header.3.saturating_sub(Self::GROUP_BTN)) / 2,
            Self::GROUP_BTN,
            Self::GROUP_BTN,
        )
    }

    /// Todos los elementos de la lista con su alto, en orden: `Este equipo`, sus Spaces y, si hay remotos,
    /// `Equipos remotos` con los suyos. `remote[i]` dice si el Space `i` es remoto; los locales van antes.
    fn slot_heights(&self, remote: &[bool]) -> Vec<(SpacesSlot, usize)> {
        let mut out = Vec::with_capacity(remote.len() + 2);
        let row = Self::ROW_H + Self::ROW_GAP;
        if !self.collapsed {
            out.push((SpacesSlot::Header { remote: false }, Self::GROUP_H + 2));
        }
        let mut added_remote_header = false;
        for (i, &is_remote) in remote.iter().enumerate() {
            if is_remote && !added_remote_header {
                added_remote_header = true;
                // Expandido: aire y cabecera; en el riel solo una raya que separa los grupos
                let h = if self.collapsed {
                    12
                } else {
                    Self::GROUP_H + 12
                };
                out.push((SpacesSlot::Header { remote: true }, h));
            }
            out.push((SpacesSlot::Space(i), row));
        }
        out
    }

    /// Cuántos elementos tiene la lista (para acotar el desplazamiento).
    pub(crate) fn slot_count(&self, remote: &[bool]) -> usize {
        self.slot_heights(remote).len()
    }

    /// Elementos que caben a la vez (una fila de Space de media).
    pub(crate) fn capacity(&self) -> usize {
        (self.list_bottom.saturating_sub(self.list_top) / (Self::ROW_H + Self::ROW_GAP)).max(1)
    }

    /// Elementos visibles tras desplazar `scroll` elementos, con su rectángulo. La cabecera de un grupo
    /// remoto lleva su aire por encima: el rectángulo es solo la franja de la etiqueta.
    pub(crate) fn visible_slots(&self, remote: &[bool], scroll: usize) -> Vec<(SpacesSlot, Rect)> {
        let mut y = self.list_top;
        let mut out = Vec::new();
        let (x, w) = if self.collapsed {
            (Self::PAD_X, Self::BTN)
        } else {
            (
                Self::RAIL_W + Self::PAD_X,
                (Self::WIDTH - Self::RAIL_W) - 2 * Self::PAD_X,
            )
        };
        for (slot, h) in self.slot_heights(remote).into_iter().skip(scroll) {
            if y + h > self.list_bottom {
                break;
            }
            let rect = match slot {
                SpacesSlot::Space(_) => (x, y, w, Self::ROW_H),
                SpacesSlot::Header { remote: true } if self.collapsed => (x, y + 5, w, 1),
                SpacesSlot::Header { remote: true } => (x, y + 12, w, Self::GROUP_H),
                SpacesSlot::Header { remote: false } => (x, y, w, Self::GROUP_H),
            };
            out.push((slot, rect));
            y += h;
        }
        out
    }
}

/// Barra de scroll vertical de toda la interfaz (panel, explorador, terminal, visor y Ajustes): un solo
/// carril, ancho y posición, para que todas se vean y respondan igual. Dibujo, clic, hover y arrastre
/// parten de `vertical`.
pub(crate) struct ScrollbarRects;

impl ScrollbarRects {
    /// Ancho del carril y del pulgar.
    pub(crate) const WIDTH: usize = 8;
    /// Aire entre el carril y el borde de la tarjeta.
    const GAP: usize = 2;
    /// Pulgar mínimo, para que siga siendo agarrable con listas muy largas.
    const MIN_THUMB: usize = 18;
    /// Espacio reservado a la derecha para el carril de la barra de scroll y el borde de la tarjeta.
    pub(crate) const RESERVED_RIGHT: usize = Self::WIDTH + Self::GAP + SidebarRects::CARD_INSET;

    /// X del carril de un contenido que termina en `right`: junto al borde de la tarjeta
    /// (`SidebarRects::CARD_INSET`), que recorta ese margen.
    #[inline]
    pub(crate) fn track_x(right: usize) -> usize {
        right.saturating_sub(Self::WIDTH + Self::GAP + SidebarRects::CARD_INSET)
    }

    /// Carril `(x, y, WIDTH, h)` y pulgar de una vista de `visible` unidades sobre `total`, desplazada
    /// `offset` unidades desde arriba. `None` si todo cabe.
    pub(crate) fn vertical(
        right: usize,
        y: usize,
        h: usize,
        total: usize,
        visible: usize,
        offset: usize,
    ) -> Option<(Rect, Rect)> {
        if h == 0 || visible == 0 || total <= visible {
            return None;
        }
        let x = Self::track_x(right);
        let thumb_h = (visible * h / total).max(Self::MIN_THUMB).min(h);
        let max_offset = total - visible;
        let thumb_y = y + offset.min(max_offset) * (h - thumb_h) / max_offset;
        Some(((x, y, Self::WIDTH, h), (x, thumb_y, Self::WIDTH, thumb_h)))
    }

    /// Zona que responde como barra (clic y hover): el carril, 8 px a su izquierda y el margen hasta el
    /// borde, para no agarrar por error lo que queda detrás.
    #[inline]
    pub(crate) fn hit(track: Rect, right: usize, mx: usize, my: usize) -> bool {
        mx + 8 >= track.0 && mx < right && my >= track.1 && my < track.1 + track.3
    }
}

/// Geometría del panel del modo de pestañas superiores: el explorador ocupa todo el panel, que mide
/// `SidebarRects::WIDTH`. Es local al panel.
#[allow(dead_code)]
pub(crate) struct FilesPanelRects;

#[allow(dead_code)]
impl FilesPanelRects {
    /// Rectángulo del explorador alojado: el interior de la tarjeta del panel (sin su margen), hasta el fondo.
    #[cfg(test)]
    pub(crate) fn explorer_rect(usable_h: usize) -> Rect {
        Self::explorer_rect_with_width(SidebarRects::WIDTH, usable_h)
    }

    /// Rectángulo del explorador alojado para un ancho de panel dado.
    pub(crate) fn explorer_rect_with_width(panel_w: usize, usable_h: usize) -> Rect {
        let inset = SidebarRects::CARD_INSET;
        let top = SidebarRects::TOP + inset;
        let bottom = SidebarRects::content_bottom(usable_h);
        (
            inset,
            top,
            panel_w.saturating_sub(2 * inset),
            bottom.saturating_sub(top),
        )
    }
}

/// Acciones rápidas de Git disponibles en las filas de la pestaña «Cambios»
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitActionKind {
    Stage,
    Unstage,
    Discard,
}

/// Geometría de los botones de acción rápida de Git en la lista de cambios
pub(crate) struct GitActionBtnRects;

impl GitActionBtnRects {
    pub(crate) const BTN_SIZE: usize = 18;
    pub(crate) const BTN_GAP: usize = 3;
    pub(crate) const RIGHT_PAD: usize = 10;

    /// Calcula los rectángulos de cada botón de acción aplicable a una fila
    pub(crate) fn buttons_for_item(
        panel_x: usize,
        panel_w: usize,
        item_y: usize,
        item_h: usize,
        is_staged: bool,
        is_unstaged: bool,
    ) -> smallvec::SmallVec<[(GitActionKind, Rect); 3]> {
        let mut btns = smallvec::SmallVec::<[GitActionKind; 3]>::new();
        if is_staged {
            btns.push(GitActionKind::Unstage);
        }
        if is_unstaged {
            btns.push(GitActionKind::Stage);
            btns.push(GitActionKind::Discard);
        }
        if btns.is_empty() {
            return smallvec::SmallVec::new();
        }
        let total_w = btns.len() * Self::BTN_SIZE + (btns.len() - 1) * Self::BTN_GAP;
        let start_x = panel_x + panel_w.saturating_sub(Self::RIGHT_PAD + total_w);
        let btn_y = item_y + (item_h.saturating_sub(Self::BTN_SIZE)) / 2;

        let mut res = smallvec::SmallVec::<[(GitActionKind, Rect); 3]>::new();
        for (i, action) in btns.into_iter().enumerate() {
            let bx = start_x + i * (Self::BTN_SIZE + Self::BTN_GAP);
            res.push((action, (bx, btn_y, Self::BTN_SIZE, Self::BTN_SIZE)));
        }
        res
    }

    /// Comprueba si `(mx, my)` cae sobre alguno de los botones de acción de la fila
    pub(crate) fn hit_test(
        panel_x: usize,
        panel_w: usize,
        item_y: usize,
        item_h: usize,
        is_staged: bool,
        is_unstaged: bool,
        mx: usize,
        my: usize,
    ) -> Option<GitActionKind> {
        let buttons =
            Self::buttons_for_item(panel_x, panel_w, item_y, item_h, is_staged, is_unstaged);
        for (action, (bx, by, bw, bh)) in buttons {
            if mx >= bx && mx < bx + bw && my >= by && my < by + bh {
                return Some(action);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StatusBarRects {
    pub(crate) ai_badge: Rect,
    pub(crate) update_badge: Option<Rect>,
    /// Medidores de límites / ranura del monitor de rendimiento (abre Sistema al pulsar)
    pub(crate) meters: Rect,
    /// Botón de selección de modo pantalla encendida (Activo, Agente, Apagado)
    pub(crate) keep_awake_btn: Rect,
    /// Dimensiones de la cuadrícula terminal (ej. «120 × 38»)
    pub(crate) dims_rect: Rect,
    /// Entorno / destino (LOCAL o SSH)
    pub(crate) env_rect: Rect,
    /// Rama git
    pub(crate) branch_rect: Option<Rect>,
    /// Ruta activa
    pub(crate) path_rect: Option<Rect>,
}

impl StatusBarRects {
    pub(crate) const BAR_H: usize = 28;
    #[cfg(test)]
    pub(crate) const POPUP_W: usize = 392;
    pub(crate) const RIGHT_PAD: usize = 0;
    pub(crate) const METER_TRACK_W: usize = 82;
    pub(crate) const METER_TRACK_H: usize = 6;
    pub(crate) const MIN_CONTENT_W: usize = 120;

    /// Como `compute_full`, con `keep_awake_w = 0`.
    #[cfg(test)]
    pub(crate) fn compute_with_status(
        width: usize,
        height: usize,
        label_w: usize,
        update_w: Option<usize>,
        dims_w: usize,
        meters_w: usize,
    ) -> Self {
        Self::compute_full(width, height, label_w, update_w, dims_w, meters_w, 0, true)
    }

    /// Calcula la geometría en segmentos de la barra de estado (DISENO §4, DISENO §1).
    /// Segmentos a la izquierda (entorno, rama, ruta) y a la derecha (dimensiones, medidores, IA, actualización, keep-awake).
    pub(crate) fn compute_full(
        width: usize,
        height: usize,
        label_w: usize,
        update_w: Option<usize>,
        dims_w: usize,
        meters_w: usize,
        keep_awake_w: usize,
        has_branch: bool,
    ) -> Self {
        let bar_y = height.saturating_sub(Self::BAR_H);
        let mut cur_right = width;

        // 1. Selector keep-awake (si está activo/disponible)
        let (kw_btn, _kw_w) = if keep_awake_w > 0 {
            let kw_full_w = keep_awake_w + 38;
            cur_right = cur_right.saturating_sub(kw_full_w);
            ((cur_right, bar_y, kw_full_w, Self::BAR_H), kw_full_w)
        } else {
            ((0, bar_y, 0, 0), 0)
        };

        // 2. Insignia de actualización disponible (si hay una disponible)
        let update_badge = update_w.map(|uw| {
            let ub_w = uw + 32;
            cur_right = cur_right.saturating_sub(ub_w);
            (cur_right, bar_y, ub_w, Self::BAR_H)
        });

        // 3. Chip de IA (solo cuando hay un agente activo; Local/SSH ya va a la izquierda)
        let badge_w = if label_w > 0 {
            (label_w + 38).min(cur_right)
        } else {
            0
        };
        cur_right = cur_right.saturating_sub(badge_w);
        let ai_badge = (cur_right, bar_y, badge_w, Self::BAR_H);

        // 4. Medidores de rendimiento / ranura del monitor de sistema en F1/F3
        let mw = if meters_w > 0 { meters_w + 24 } else { 88 };
        let (meters, _m_w) = if cur_right > Self::MIN_CONTENT_W + mw {
            cur_right = cur_right.saturating_sub(mw);
            ((cur_right, bar_y, mw, Self::BAR_H), mw)
        } else {
            ((0, bar_y, 0, Self::BAR_H), 0)
        };

        // 5. Dimensiones (cols × rows)
        let dims_full_w = if dims_w > 0 { dims_w + 24 } else { 0 };
        let dims_rect = if dims_full_w > 0 && cur_right > Self::MIN_CONTENT_W + dims_full_w {
            cur_right = cur_right.saturating_sub(dims_full_w);
            (cur_right, bar_y, dims_full_w, Self::BAR_H)
        } else {
            (0, bar_y, 0, 0)
        };

        // Segmentos izquierdos: entorno (80 px), rama (130 px si hay), ruta
        let env_w = 80;
        let env_rect = (0, bar_y, env_w, Self::BAR_H);
        let branch_w = 130;
        let (branch_rect, path_start) = if has_branch && cur_right >= env_w + 40 {
            let bw = branch_w.min(cur_right - env_w);
            (Some((env_w, bar_y, bw, Self::BAR_H)), env_w + bw)
        } else {
            (None, env_w)
        };
        let path_w = cur_right.saturating_sub(path_start);
        let path_rect = if path_w > 30 {
            Some((path_start, bar_y, path_w, Self::BAR_H))
        } else {
            None
        };

        Self {
            ai_badge,
            update_badge,
            meters,
            keep_awake_btn: kw_btn,
            dims_rect,
            env_rect,
            branch_rect,
            path_rect,
        }
    }

    /// Rectángulo del menú flotante de consumo encima de la barra de estado.
    #[cfg(test)]
    pub(crate) fn popup_rect(width: usize, height: usize, popup_h: usize) -> Rect {
        let w = Self::POPUP_W.min(width.saturating_sub(16));
        let x = width.saturating_sub(w + 10);
        let bar_y = height.saturating_sub(Self::BAR_H);
        let y = bar_y.saturating_sub(popup_h + 6);
        (x, y, w, popup_h)
    }

    /// Ancho, alto de cada opción y margen interior del selector de pantalla encendida.
    pub(crate) const KA_POPUP_W: usize = 330;
    pub(crate) const KA_ROW_H: usize = 46;
    pub(crate) const KA_PAD: usize = 6;

    /// Rectángulo del selector de pantalla encendida: se despliega hacia arriba, pegado a la derecha.
    pub(crate) fn keep_awake_popup_rect(width: usize, height: usize) -> Rect {
        let w = Self::KA_POPUP_W.min(width.saturating_sub(16));
        let h = 3 * Self::KA_ROW_H + 2 * Self::KA_PAD;
        let x = width.saturating_sub(w + Self::RIGHT_PAD);
        let y = height.saturating_sub(Self::BAR_H + h + 6);
        (x, y, w, h)
    }

    /// Rectángulo de la opción `i` (0..3) dentro del selector.
    pub(crate) fn keep_awake_option_rect(width: usize, height: usize, i: usize) -> Rect {
        let (x, y, w, _) = Self::keep_awake_popup_rect(width, height);
        (
            x + Self::KA_PAD,
            y + Self::KA_PAD + i * Self::KA_ROW_H,
            w.saturating_sub(2 * Self::KA_PAD),
            Self::KA_ROW_H,
        )
    }

    /// Opción del selector bajo el ratón.
    pub(crate) fn keep_awake_option_at(
        width: usize,
        height: usize,
        mx: usize,
        my: usize,
    ) -> Option<usize> {
        (0..3).find(|&i| Self::inside(mx, my, Self::keep_awake_option_rect(width, height, i)))
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect_contains(rect, mx, my)
    }

    /// Mantiene abierto el menú flotante si el ratón está en el badge, en la tarjeta o en el puente de 6px entre ambos.
    #[cfg(test)]
    #[inline]
    pub(crate) fn inside_badge_or_popup(mx: usize, my: usize, badge: Rect, popup: Rect) -> bool {
        let bridge = (popup.0, popup.1, popup.2, popup.3 + 8);
        Self::inside(mx, my, badge) || Self::inside(mx, my, bridge)
    }
}

/// Botón de la cabecera de un panel dividido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneHeaderButton {
    /// Dividir el panel en dos columnas
    Split,
    /// Dividir el panel en dos filas
    SplitHorizontal,
    /// Maximizar el panel (ocultar el resto) o restaurar el reparto
    Zoom,
    /// Cerrar el panel
    Close,
}

/// Geometría de la cabecera de un panel dividido (40 px): icono, título, subtítulo y los botones
/// de la derecha. Es local al hueco del panel y la comparten dibujo, clic y hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaneHeaderRects {
    pub(crate) header: Rect,
    /// Dividir en columnas
    pub(crate) split_btn: Rect,
    /// Dividir en filas
    pub(crate) split_h_btn: Rect,
    pub(crate) zoom_btn: Rect,
    pub(crate) close_btn: Rect,
}

impl PaneHeaderRects {
    pub(crate) const HEIGHT: usize = 40;
    /// Línea inferior que separa la cabecera del terminal
    pub(crate) const BORDER: usize = 1;
    pub(crate) const BTN: usize = 26;
    pub(crate) const BTN_GAP: usize = 10;
    pub(crate) const PAD_RIGHT: usize = 18;
    /// Comienzo del título, relativo al borde izquierdo del panel
    pub(crate) const TEXT_X: usize = 20;

    /// Margen superior del hueco si la cabecera coincide con el borde superior de la tarjeta de contenido.
    /// La tarjeta recorta `SidebarRects::CARD_INSET` px de margen, de modo que la cabecera visible
    /// arranca en `y + top_pad` y mide `HEIGHT` (40 px), alineándose exactamente con el explorador y el visor.
    #[inline]
    pub(crate) fn top_pad(y: usize) -> usize {
        if y <= SidebarRects::TOP {
            SidebarRects::CARD_INSET
        } else {
            0
        }
    }

    #[inline]
    pub(crate) fn header_total_h(y: usize) -> usize {
        Self::top_pad(y) + Self::HEIGHT
    }

    #[inline]
    pub(crate) fn content_offset_for(y: usize) -> usize {
        Self::header_total_h(y) + Self::BORDER
    }

    pub(crate) fn compute(x: usize, y: usize, w: usize) -> Self {
        let pad = Self::top_pad(y);
        let right = x + w;
        let by = y + pad + (Self::HEIGHT.saturating_sub(Self::BTN)) / 2;
        let close_btn = (
            right.saturating_sub(Self::PAD_RIGHT + Self::BTN),
            by,
            Self::BTN,
            Self::BTN,
        );
        let zoom_btn = (
            close_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        let split_h_btn = (
            zoom_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        let split_btn = (
            split_h_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        Self {
            header: (x, y + pad, w, Self::HEIGHT),
            split_btn,
            split_h_btn,
            zoom_btn,
            close_btn,
        }
    }

    /// Botón bajo el ratón. Todo panel tiene los cuatro (dividir en columnas · en filas · maximizar ·
    /// cerrar); solo se ven, y por tanto solo se pueden pulsar, con el ratón sobre la cabecera.
    pub(crate) fn button_at(&self, mx: usize, my: usize) -> Option<PaneHeaderButton> {
        if !Self::inside(mx, my, self.header) {
            return None;
        }
        [
            (self.close_btn, PaneHeaderButton::Close),
            (self.zoom_btn, PaneHeaderButton::Zoom),
            (self.split_h_btn, PaneHeaderButton::SplitHorizontal),
            (self.split_btn, PaneHeaderButton::Split),
        ]
        .into_iter()
        .find(|(r, _)| Self::inside(mx, my, *r))
        .map(|(_, b)| b)
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect_contains(rect, mx, my)
    }
}

/// Marca de mira de la tarjeta con foco (DISENO §3.3): una cruz de 11 px centrada en cada esquina del borde
/// interior de la tarjeta, de modo que sobresale 6 px (la esquina más 5) hacia fuera; cabe en el recorte
/// SidebarRects::CARD_INSET. Las cruces son de 1 px de grosor.
pub(crate) struct FocusMarkRects;

impl FocusMarkRects {
    /// Largo de cada brazo completo de la cruz.
    pub(crate) const LENGTH: usize = 11;
    /// Cuánto sobresale la cruz de la esquina, contando el píxel de la esquina.
    pub(crate) const OUTWARD: usize = 6;

    /// Ocho rectángulos (x, y, ancho, alto): el brazo horizontal y el vertical de cada esquina, de la tarjeta
    /// cuya región (con recorte) es `region`.
    pub(crate) fn compute(region: Rect) -> [Rect; 8] {
        let (x, y, w, h) = region;
        let inset = SidebarRects::CARD_INSET;
        let (left, top) = (x + inset, y + inset);
        let right = (x + w).saturating_sub(inset + 1).max(left);
        let bottom = (y + h).saturating_sub(inset + 1).max(top);
        let reach = Self::OUTWARD - 1;
        let corner = |cx: usize, cy: usize| {
            [
                (cx.saturating_sub(reach), cy, Self::LENGTH, 1),
                (cx, cy.saturating_sub(reach), 1, Self::LENGTH),
            ]
        };
        let [a, b] = corner(left, top);
        let [c, d] = corner(right, top);
        let [e, f] = corner(left, bottom);
        let [g, i] = corner(right, bottom);
        [a, b, c, d, e, f, g, i]
    }
}

/// Botón interactivo dentro de la vista de reconexión SSH (Ssh.dc.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SshLostButton {
    Reconnect,
    Close,
}

/// Rectángulos de la vista de conexión perdida y reconexión SSH en un panel (pantalla 03 / Ssh.dc.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SshLostConnectionRects {
    pub(crate) title_rect: Rect,
    pub(crate) subtitle_rect: Rect,
    pub(crate) reconnect_btn: Rect,
    pub(crate) close_pane_btn: Rect,
}

impl SshLostConnectionRects {
    pub(crate) const BTN_H: usize = 28;
    pub(crate) const RECONNECT_W: usize = 140;
    pub(crate) const CLOSE_W: usize = 110;
    pub(crate) const GAP: usize = 8;

    pub(crate) fn compute(rx: usize, ry: usize, rw: usize, rh: usize) -> Self {
        let pad_x = 24.max(rw.saturating_sub(400) / 4).min(40);
        let start_x = rx + pad_x;
        let content_h = 96;
        let start_y = ry + (rh.saturating_sub(content_h)) / 2;
        let text_w = rw.saturating_sub(pad_x * 2);
        let btn_y = start_y + 64;
        Self {
            title_rect: (start_x, start_y, text_w, 24),
            subtitle_rect: (start_x, start_y + 30, text_w, 22),
            reconnect_btn: (start_x, btn_y, Self::RECONNECT_W, Self::BTN_H),
            close_pane_btn: (
                start_x + Self::RECONNECT_W + Self::GAP,
                btn_y,
                Self::CLOSE_W,
                Self::BTN_H,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_lost_connection_rects_compute() {
        let rects = SshLostConnectionRects::compute(100, 200, 500, 400);
        assert!(rects.reconnect_btn.0 >= 100);
        assert_eq!(rects.reconnect_btn.2, SshLostConnectionRects::RECONNECT_W);
        assert_eq!(rects.reconnect_btn.3, SshLostConnectionRects::BTN_H);
        assert_eq!(
            rects.close_pane_btn.0,
            rects.reconnect_btn.0
                + SshLostConnectionRects::RECONNECT_W
                + SshLostConnectionRects::GAP
        );
        assert_eq!(rects.close_pane_btn.2, SshLostConnectionRects::CLOSE_W);
        assert_eq!(rects.close_pane_btn.3, SshLostConnectionRects::BTN_H);
    }

    #[test]
    fn test_focus_marks_are_11px_crosses_centered_on_each_corner() {
        let region = (100, 50, 400, 300);
        let marks = FocusMarkRects::compute(region);
        let inset = SidebarRects::CARD_INSET;
        let (left, top) = (100 + inset, 50 + inset);
        let (right, bottom) = (100 + 400 - inset - 1, 50 + 300 - inset - 1);
        // Esquina superior izquierda: brazo horizontal y vertical de 11 px con centro en la esquina.
        assert_eq!(marks[0], (left - 5, top, 11, 1));
        assert_eq!(marks[1], (left, top - 5, 1, 11));
        // Esquina inferior derecha.
        assert_eq!(marks[6], (right - 5, bottom, 11, 1));
        assert_eq!(marks[7], (right, bottom - 5, 1, 11));
        // Sobresalen 6 px de la esquina (esquina incluida) y nunca salen de la región recortada.
        for (x, y, w, h) in marks {
            assert!(
                x >= 100 && y >= 50,
                "la mira sale por arriba o la izquierda"
            );
            assert!(
                x + w <= 100 + 400 && y + h <= 50 + 300,
                "la mira sale de la región"
            );
            assert!(w == 11 && h == 1 || w == 1 && h == 11);
        }
        assert_eq!(left - marks[0].0 + 1, FocusMarkRects::OUTWARD);
    }

    #[test]
    fn test_focus_marks_survive_tiny_regions() {
        // Sin pánico ni desbordes con regiones menores que el recorte.
        let marks = FocusMarkRects::compute((0, 0, 3, 3));
        assert_eq!(marks.len(), 8);
        let _ = FocusMarkRects::compute((0, 0, 0, 0));
    }
}
