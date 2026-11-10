//! Geometría del visor/editor de archivos, diff lado a lado y barra de búsqueda interna.

use super::{rect_contains, Rect, ScrollbarRects, SidebarRects};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerHeaderRects {
    pub mode_btn: (usize, usize, usize, usize),
    pub md_btn: Option<(usize, usize, usize, usize)>,
    /// «Cambios»: abre el diff contra HEAD (solo si el archivo está en un repo)
    pub diff_btn: Option<(usize, usize, usize, usize)>,
    /// Selector de modo de diff (Lado a lado / Unificado), visible mientras el diff está activo
    pub diff_mode_btn: Option<(usize, usize, usize, usize)>,
}

impl FileViewerHeaderRects {
    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        _h: usize,
        is_markdown: bool,
        has_diff: bool,
        is_diff: bool,
    ) -> Self {
        let mut right_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 12);
        let btn_y = y
            + SidebarRects::CARD_INSET
            + (FileViewerLayout::HEADER_H.saturating_sub(SidebarRects::CARD_INSET + 24)) / 2;

        // 1. Botón Vista previa / Código si es .md (icono + texto)
        let md_btn = if is_markdown {
            let md_w = 136;
            let md_x = right_x.saturating_sub(md_w);
            right_x = md_x.saturating_sub(8);
            Some((md_x, btn_y, md_w, 24))
        } else {
            None
        };

        // 2. Botón Lectura / Edición (icono + texto; el atajo Ctrl+E va en la ayuda del menú)
        let mode_w = 104;
        let mode_x = right_x.saturating_sub(mode_w);
        let mode_btn = (mode_x, btn_y, mode_w, 24);

        // 3. Botón Cambios (diff antes vs ahora), a la izquierda del de modo
        let diff_btn = has_diff.then(|| {
            let diff_w = 108;
            (mode_x.saturating_sub(8 + diff_w), btn_y, diff_w, 24)
        });

        // 4. Selector de modo (Lado a lado / Unificado), solo cuando el diff está abierto
        let diff_mode_btn = if is_diff && has_diff {
            diff_btn.map(|d_btn| {
                let diff_mode_w = 118;
                (
                    d_btn.0.saturating_sub(8 + diff_mode_w),
                    btn_y,
                    diff_mode_w,
                    24,
                )
            })
        } else {
            None
        };

        Self {
            mode_btn,
            md_btn,
            diff_btn,
            diff_mode_btn,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect_contains(rect, mx, my)
    }
}

/// Geometría del área de texto del visor de archivos (una sola fuente para dibujo, clics y scroll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerLayout {
    pub content_y: usize,
    pub content_h: usize,
    pub row_h: usize,
    pub char_w: usize,
    pub gutter_w: usize,
    pub sep_x: usize,
    /// X del primer carácter de cada línea (con scroll horizontal 0)
    pub text_x: usize,
    pub text_max_x: usize,
    pub visible_cols: usize,
}

impl FileViewerLayout {
    pub(crate) const HEADER_H: usize = 42;

    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        row_height: usize,
        char_width: usize,
        total_lines: usize,
        show_gutter: bool,
    ) -> Self {
        let row_h = row_height.max(16);
        let char_w = char_width.max(7);
        let content_h = h.saturating_sub(Self::HEADER_H);
        let (gutter_w, sep_x, text_x) = if show_gutter {
            let digits = total_lines.to_string().len().max(3);
            let gw = SidebarRects::CARD_INSET + digits * char_w + 24;
            let sx = x + gw;
            let tx = sx + 14;
            (gw, sx, tx)
        } else {
            // Modo vista previa (estilo GitHub / documento): sin canaleta de números,
            // margen de lectura limpio
            let left_pad = SidebarRects::CARD_INSET + 20;
            (0, x, x + left_pad)
        };
        let text_max_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 16);
        Self {
            content_y: y + Self::HEADER_H,
            content_h,
            row_h,
            char_w,
            gutter_w,
            sep_x,
            text_x,
            text_max_x,
            visible_cols: (text_max_x.saturating_sub(text_x) / char_w).max(1),
        }
    }

    /// Columna (índice de grafema) de la línea bajo la coordenada X, con el scroll horizontal aplicado.
    pub(crate) fn col_at(&self, line: &str, mx: usize, scroll_x: usize) -> usize {
        if mx < self.text_x {
            return 0;
        }
        let rel_x = (mx - self.text_x) + scroll_x * self.char_w;
        let mut acc = 0;
        use unicode_segmentation::UnicodeSegmentation;
        for (ci, g) in line.graphemes(true).enumerate() {
            let adv = if g == "\t" {
                self.char_w * 4
            } else {
                use unicode_width::UnicodeWidthStr;
                self.char_w * g.width().max(1)
            };
            if acc + adv / 2 >= rel_x {
                return ci;
            }
            acc += adv;
        }
        line.graphemes(true).count()
    }

    /// Pista y pulgar de la barra horizontal; `None` si todo el ancho cabe. `max_scroll` = `FileViewerState::max_scroll_x`.
    pub(crate) fn hscrollbar(&self, max_scroll: usize, scroll_x: usize) -> Option<(Rect, Rect)> {
        if max_scroll == 0 || self.content_h < 32 {
            return None;
        }
        let track_x = self.sep_x + 8;
        let track_w = self.text_max_x.saturating_sub(track_x);
        let track_h = ScrollbarRects::WIDTH;
        let track_y = (self.content_y + self.content_h)
            .saturating_sub(track_h + 4 + SidebarRects::CARD_INSET);
        let total = max_scroll + self.visible_cols;
        let thumb_w = (track_w * self.visible_cols / total).clamp(24.min(track_w), track_w);
        let travel = track_w - thumb_w;
        let thumb_x = track_x + travel * scroll_x.min(max_scroll) / max_scroll;
        Some((
            (track_x, track_y, track_w, track_h),
            (thumb_x, track_y, thumb_w, track_h),
        ))
    }
}

/// Geometría para el visor de diff lado a lado (dos columnas sincronizadas: base HEAD vs disco).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SideBySideLayout {
    pub col_header_y: usize,
    pub col_header_h: usize,
    pub content_y: usize,
    pub content_h: usize,
    pub row_h: usize,
    pub char_w: usize,
    // Columna izquierda (HEAD / Base)
    pub left_x: usize,
    pub left_w: usize,
    pub left_gutter_w: usize,
    pub left_sep_x: usize,
    pub left_text_x: usize,
    pub left_text_max_x: usize,
    // Separador central
    pub divider_x: usize,
    // Columna derecha (Disco / Actual)
    pub right_x: usize,
    pub right_w: usize,
    pub right_gutter_w: usize,
    pub right_sep_x: usize,
    pub right_text_x: usize,
    pub right_text_max_x: usize,
    pub visible_cols: usize,
}

impl SideBySideLayout {
    pub(crate) const COL_HEADER_H: usize = 28;

    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        row_height: usize,
        char_width: usize,
        total_lines: usize,
    ) -> Self {
        let row_h = row_height.max(16);
        let char_w = char_width.max(7);
        let col_header_y = y + FileViewerLayout::HEADER_H;
        let col_header_h = Self::COL_HEADER_H;
        let content_y = col_header_y + col_header_h;
        let content_h = h.saturating_sub(FileViewerLayout::HEADER_H + col_header_h);

        let half_w = w.saturating_sub(1) / 2;
        let left_x = x;
        let left_w = half_w;
        let divider_x = x + half_w;
        let right_x = divider_x + 1;
        let right_w = (x + w).saturating_sub(right_x);

        let digits = total_lines.to_string().len().max(3);
        let gutter_w = SidebarRects::CARD_INSET + digits * char_w + 20;

        let left_gutter_w = gutter_w.min(left_w / 3);
        let left_sep_x = left_x + left_gutter_w;
        let left_text_x = left_sep_x + 8;
        let left_text_max_x = divider_x.saturating_sub(4);

        let right_gutter_w = gutter_w.min(right_w / 3);
        let right_sep_x = right_x + right_gutter_w;
        let right_text_x = right_sep_x + 8;
        let right_text_max_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 16);

        let visible_cols = (left_text_max_x.saturating_sub(left_text_x) / char_w).max(1);

        Self {
            col_header_y,
            col_header_h,
            content_y,
            content_h,
            row_h,
            char_w,
            left_x,
            left_w,
            left_gutter_w,
            left_sep_x,
            left_text_x,
            left_text_max_x,
            divider_x,
            right_x,
            right_w,
            right_gutter_w,
            right_sep_x,
            right_text_x,
            right_text_max_x,
            visible_cols,
        }
    }

    /// Pista y pulgar de la barra horizontal sincronizada; `None` si todo cabe.
    pub(crate) fn hscrollbar(&self, max_scroll: usize, scroll_x: usize) -> Option<(Rect, Rect)> {
        if max_scroll == 0 || self.content_h < 32 {
            return None;
        }
        let track_x = self.left_text_x;
        let track_w = self.right_text_max_x.saturating_sub(track_x);
        let track_h = ScrollbarRects::WIDTH;
        let track_y = (self.content_y + self.content_h)
            .saturating_sub(track_h + 4 + SidebarRects::CARD_INSET);
        let total = max_scroll + self.visible_cols;
        let thumb_w = (track_w * self.visible_cols / total).clamp(24.min(track_w), track_w);
        let travel = track_w - thumb_w;
        let thumb_x = track_x + travel * scroll_x.min(max_scroll) / max_scroll;
        Some((
            (track_x, track_y, track_w, track_h),
            (thumb_x, track_y, thumb_w, track_h),
        ))
    }
}

/// Rectángulos de la barra flotante de búsqueda interna en el visor de archivos (`Ctrl + F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerSearchRects {
    pub bar: (usize, usize, usize, usize),
    pub input: (usize, usize, usize, usize),
    pub text_start_x: usize,
    pub counter: (usize, usize, usize, usize),
    pub prev_btn: (usize, usize, usize, usize),
    pub next_btn: (usize, usize, usize, usize),
    pub close_btn: (usize, usize, usize, usize),
}

impl FileViewerSearchRects {
    pub(crate) fn compute(lay: &FileViewerLayout) -> Self {
        let max_w = lay.text_max_x.saturating_sub(lay.sep_x);
        let bar_w = 296.min(max_w).max(180);
        let bar_h: usize = 32;
        let bar_x = lay.text_max_x.saturating_sub(bar_w + 8);
        let bar_y = lay.content_y + 8;
        let bar = (bar_x, bar_y, bar_w, bar_h);

        let input_x = bar_x + 28;
        let input_w = if bar_w >= 296 {
            110
        } else {
            bar_w.saturating_sub(186)
        };
        let input = (input_x, bar_y + 4, input_w, 24);
        let text_start_x = input_x + 4;

        let counter_x = input_x + input_w + 4;
        let counter_w = 48;
        let counter = (counter_x, bar_y + 4, counter_w, 24);

        let btn_y = bar_y + (bar_h.saturating_sub(22)) / 2;
        let prev_x = counter_x + counter_w + 2;
        let prev_btn = (prev_x, btn_y, 22, 22);

        let next_x = prev_x + 22 + 2;
        let next_btn = (next_x, btn_y, 22, 22);

        let close_x = next_x + 22 + 6;
        let close_btn = (close_x, btn_y, 22, 22);

        Self {
            bar,
            input,
            text_start_x,
            counter,
            prev_btn,
            next_btn,
            close_btn,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect_contains(rect, mx, my)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_by_side_layout_gutter_width_and_no_clipping() {
        let char_w = 9;
        let row_h = 18;
        let lay = SideBySideLayout::compute(0, 0, 1000, 800, row_h, char_w, 1500);
        assert!(lay.left_gutter_w >= SidebarRects::CARD_INSET + 4 * char_w + 20);

        let num_w = 4 * char_w;
        let num_x = lay.left_sep_x.saturating_sub(num_w + 10);
        assert!(
            num_x >= lay.left_x + SidebarRects::CARD_INSET,
            "num_x ({num_x}) no debe colisionar con el borde izquierdo del card ({})",
            lay.left_x + SidebarRects::CARD_INSET
        );
    }

    #[test]
    fn test_side_by_side_layout_hscrollbar() {
        let char_w = 9;
        let row_h = 18;
        let lay = SideBySideLayout::compute(0, 0, 1000, 800, row_h, char_w, 100);
        assert!(lay.hscrollbar(0, 0).is_none());
        let (track, thumb) = lay.hscrollbar(50, 0).expect("hscrollbar presente");
        assert!(track.2 > 0);
        assert_eq!(thumb.0, track.0);
        let (_, thumb_end) = lay.hscrollbar(50, 50).expect("hscrollbar al final");
        assert!(thumb_end.0 > thumb.0);
    }
}
