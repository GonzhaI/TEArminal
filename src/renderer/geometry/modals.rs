//! Geometría de los modales superpuestos: «Añadir máquina» y Paleta de Comandos (`Ctrl + K`).

use super::{rect_contains, Rect, ScrollbarRects};
use crate::renderer::tokens::RADIUS_CONTROL;

/// Elemento del modal «Añadir máquina» bajo el ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MachineHover {
    /// Fila de la lista de hosts de `~/.ssh/config`
    Row(usize),
    Input,
    /// Botón «Añadir» del campo `usuario@host`
    TypedAdd,
}

/// Geometría del modal «Añadir máquina», centrado en la ventana. La comparten dibujo, clic y hover.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MachineModalRects {
    pub card: Rect,
    /// Filas de hosts (las `MAX_ROWS` primeras) y su botón «Añadir»
    pub rows: Vec<Rect>,
    pub row_buttons: Vec<Rect>,
    pub list_label_y: usize,
    pub typed_label_y: usize,
    pub input: Rect,
    pub add_btn: Rect,
    pub error_y: usize,
}

impl MachineModalRects {
    pub(crate) const W: usize = 560;
    pub(crate) const PAD: usize = 20;
    const HEAD_H: usize = 88;
    pub(crate) const ROW_H: usize = 40;
    pub(crate) const MAX_ROWS: usize = 5;

    pub(crate) fn compute(win_w: usize, win_h: usize, host_count: usize) -> Self {
        let w = Self::W.min(win_w.saturating_sub(32));
        let n = host_count.min(Self::MAX_ROWS);
        let list_block = if n > 0 { 26 + n * Self::ROW_H + 14 } else { 0 };
        let h = Self::HEAD_H + list_block + 8 + 26 + 36 + 24 + 14;
        let x = (win_w.saturating_sub(w)) / 2;
        let y = (win_h.saturating_sub(h)) / 2;
        let list_label_y = y + Self::HEAD_H;
        let rows_y = list_label_y + 26;
        let rows: Vec<Rect> = (0..n)
            .map(|i| {
                (
                    x + Self::PAD - 8,
                    rows_y + i * Self::ROW_H,
                    w - 2 * Self::PAD + 16,
                    Self::ROW_H,
                )
            })
            .collect();
        let row_buttons = rows
            .iter()
            .map(|r| (r.0 + r.2 - 8 - 76, r.1 + (Self::ROW_H - 28) / 2, 76, 28))
            .collect();
        let typed_label_y = y + Self::HEAD_H + list_block + 8;
        let input = (
            x + Self::PAD,
            typed_label_y + 26,
            w.saturating_sub(2 * Self::PAD + 100),
            36,
        );
        let add_btn = (input.0 + input.2 + 8, input.1, 92, 36);
        Self {
            card: (x, y, w, h),
            rows,
            row_buttons,
            list_label_y,
            typed_label_y,
            input,
            add_btn,
            error_y: input.1 + 36 + 6,
        }
    }

    pub(crate) fn inside(mx: usize, my: usize, r: Rect) -> bool {
        rect_contains(r, mx, my)
    }

    pub(crate) fn hit(&self, mx: usize, my: usize) -> Option<MachineHover> {
        if let Some(i) = self.rows.iter().position(|r| Self::inside(mx, my, *r)) {
            Some(MachineHover::Row(i))
        } else if Self::inside(mx, my, self.add_btn) {
            Some(MachineHover::TypedAdd)
        } else if Self::inside(mx, my, self.input) {
            Some(MachineHover::Input)
        } else {
            None
        }
    }
}

/// Geometría de la paleta de comandos y búsqueda central (Ctrl + K, WP5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaletteRects {
    pub(crate) card: Rect,
    pub(crate) input: Rect,
    pub(crate) divider_y: usize,
    pub(crate) list: Rect,
    pub(crate) items_count: usize,
    pub(crate) item_h: usize,
    pub(crate) scrollbar: Option<(Rect, Rect)>,
}

impl PaletteRects {
    pub(crate) const WIDTH: usize = 560;
    pub(crate) const INPUT_H: usize = 46;
    pub(crate) const ITEM_H: usize = 40;
    pub(crate) const MAX_VISIBLE: usize = 8;
    pub(crate) const CARD_RADIUS: usize = RADIUS_CONTROL;

    #[cfg(test)]
    pub(crate) fn compute(win_w: usize, win_h: usize, items_count: usize) -> Self {
        Self::compute_with_scroll(win_w, win_h, items_count, 0)
    }

    pub(crate) fn compute_with_scroll(
        win_w: usize,
        win_h: usize,
        items_count: usize,
        scroll_offset: usize,
    ) -> Self {
        Self::compute_with_scroll_offset_y(win_w, win_h, items_count, scroll_offset, 0)
    }

    pub(crate) fn compute_with_scroll_offset_y(
        win_w: usize,
        win_h: usize,
        items_count: usize,
        scroll_offset: usize,
        offset_y: isize,
    ) -> Self {
        let card_w = Self::WIDTH.min(win_w.saturating_sub(40));
        let card_x = (win_w.saturating_sub(card_w)) / 2;
        let card_y = (54.min(win_h / 6) as isize + offset_y).max(0) as usize;
        let visible_items = items_count.min(Self::MAX_VISIBLE);
        let list_h = visible_items * Self::ITEM_H;
        let card_h = Self::INPUT_H
            + if visible_items > 0 {
                list_h + 8
            } else if items_count == 0 {
                44
            } else {
                0
            };

        let card = (card_x, card_y, card_w, card_h);
        let input = (card_x, card_y, card_w, Self::INPUT_H);
        let divider_y = card_y + Self::INPUT_H;
        let list = (card_x, divider_y + 4, card_w, list_h);

        let scrollbar = if items_count > Self::MAX_VISIBLE {
            ScrollbarRects::vertical(
                card_x + card_w,
                divider_y + 6,
                list_h.saturating_sub(4),
                items_count,
                Self::MAX_VISIBLE,
                scroll_offset,
            )
        } else {
            None
        };

        Self {
            card,
            input,
            divider_y,
            list,
            items_count,
            item_h: Self::ITEM_H,
            scrollbar,
        }
    }

    pub(crate) fn item_rect(&self, idx: usize) -> Rect {
        let y = self.list.1 + idx * self.item_h;
        let w = if self.scrollbar.is_some() {
            self.card.2.saturating_sub(26)
        } else {
            self.card.2.saturating_sub(12)
        };
        (self.card.0 + 6, y, w, self.item_h)
    }

    pub(crate) fn item_at_with_offset(
        &self,
        mx: usize,
        my: usize,
        scroll_offset: usize,
    ) -> Option<usize> {
        if let Some((track, _)) = self.scrollbar {
            let right = self.card.0 + self.card.2;
            if ScrollbarRects::hit(track, right, mx, my) {
                return None;
            }
        }
        let (lx, ly, lw, lh) = self.list;
        let list_w = if self.scrollbar.is_some() {
            lw.saturating_sub(26)
        } else {
            lw
        };
        if mx >= lx && mx < lx + list_w && my >= ly && my < ly + lh {
            let visual_idx = (my - ly) / self.item_h;
            if visual_idx < self.items_count.min(Self::MAX_VISIBLE) {
                let real_idx = scroll_offset + visual_idx;
                if real_idx < self.items_count {
                    return Some(real_idx);
                }
            }
        }
        None
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect_contains(rect, mx, my)
    }
}
