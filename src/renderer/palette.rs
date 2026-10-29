//! Paleta de comandos / búsqueda central (Ctrl + K, WP5).

use super::*;
use crate::app::{PaletteCategory, PaletteState};
use crate::renderer::geometry::PaletteRects;

impl TerminalRenderer {
    pub(crate) fn render_palette(
        &mut self,
        canvas: &mut Canvas,
        state: &PaletteState,
        cursor_visible: bool,
        scrollbar_hovered: bool,
        theme: &Theme,
    ) {
        let (win_w, win_h) = canvas.size();
        let items_len = state.filtered.len();
        let rects = PaletteRects::compute_with_scroll(win_w, win_h, items_len, state.scroll_offset);

        // Velo translúcido que oscurece suavemente el fondo sin ocultar la ventana en negro opaco
        canvas.rounded_fill(0, 0, win_w, win_h, 0, Color::from_rgba(0, 0, 0, 125));

        let (cx, cy, cw, ch) = rects.card;

        // Sombra suave de la tarjeta
        canvas.rounded_fill(
            cx,
            cy + 6,
            cw,
            ch,
            PaletteRects::CARD_RADIUS + 2,
            Color::from_rgba(0, 0, 0, 140),
        );
        // Fondo de la tarjeta (superficie elevada)
        canvas.rounded_fill(
            cx,
            cy,
            cw,
            ch,
            PaletteRects::CARD_RADIUS,
            theme.chrome_bg_elevated(),
        );
        // Borde fino nítido
        canvas.rounded_stroke(
            cx,
            cy,
            cw,
            ch,
            PaletteRects::CARD_RADIUS,
            theme.chrome_line_strong(),
        );

        let accent = theme.accent_color();

        // 1. Fila de entrada de texto
        let (ix, iy, iw, ih) = rects.input;
        let icon_cy = iy + ih / 2;
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Search,
            ix + 18,
            icon_cy,
            16.0,
            accent,
        );

        // Keycap ESC a la derecha
        let esc_key = "ESC";
        let esc_w = self.measure_mono_text(esc_key, tokens::MICRO, false) + 12;
        let esc_x = ix + iw - 16 - esc_w;
        let esc_y = iy + (ih.saturating_sub(tokens::KEYCAP_H)) / 2;
        self.draw_keycap(canvas, esc_x, esc_y, esc_key, theme);

        // Texto o placeholder con barra de escritura (cursor indicador)
        let text_x = ix + 40;
        let text_max_x = esc_x.saturating_sub(8);
        let input_baseline = self.centered_baseline(icon_cy);
        self.render_text_input_content(
            canvas,
            &state.input,
            text_x,
            input_baseline,
            text_max_x,
            iy + 6,
            ih.saturating_sub(12),
            cursor_visible,
            "Buscar sesiones, archivos y comandos...",
            true,
            theme,
        );

        let query_text = state.input.text();

        // Si no hay resultados
        if items_len == 0 {
            if !query_text.is_empty() {
                canvas.rounded_fill(
                    cx + 16,
                    rects.divider_y,
                    cw.saturating_sub(32),
                    1,
                    0,
                    theme.chrome_line(),
                );
                self.draw_ui_text(
                    canvas,
                    "Sin resultados para la búsqueda",
                    cx + 20,
                    rects.divider_y + 26,
                    cw.saturating_sub(40),
                    theme.text_secondary(),
                    tokens::BODY,
                    false,
                );
            }
            return;
        }

        // Divisor entre input y lista
        canvas.rounded_fill(
            cx + 16,
            rects.divider_y,
            cw.saturating_sub(32),
            1,
            0,
            theme.chrome_line(),
        );

        // 2. Elementos de la lista con ventana de desplazamiento
        let visible_count = items_len.min(PaletteRects::MAX_VISIBLE);
        let max_scroll = items_len.saturating_sub(visible_count);
        let scroll = state.scroll_offset.min(max_scroll);

        for visual_idx in 0..visible_count {
            let item_idx = scroll + visual_idx;
            if item_idx >= items_len {
                break;
            }
            let item = &state.filtered[item_idx];
            let (rx, ry, rw, rh) = rects.item_rect(visual_idx);
            let selected = item_idx == state.selected;

            if selected {
                let sel_bg = theme.mix(theme.surface_hover(), theme.accent_color(), 0.28);
                let sel_stroke = theme.mix(theme.surface_hover(), theme.accent_color(), 0.65);
                canvas.rounded_fill(rx, ry, rw, rh, tokens::RADIUS_ROW, sel_bg);
                canvas.rounded_stroke(rx, ry, rw, rh, tokens::RADIUS_ROW, sel_stroke);
            }

            let item_cy = ry + rh / 2;
            let item_baseline = self.centered_baseline(item_cy);
            let icon_col = if selected {
                accent
            } else {
                theme.text_secondary()
            };
            self.draw_lucide_icon(canvas, item.icon, rx + 14, item_cy, 14.0, icon_col);

            // Categoría badge con ancho fijo (68 px) y texto centrado para alinear todos los elementos
            const BADGE_W: usize = 68;
            let cat_label = item.category.label();
            let (cat_bg, cat_fg) = match item.category {
                PaletteCategory::Session => (
                    theme.mix(theme.ansi[6], theme.surface_bg(), 0.25),
                    theme.ansi[6],
                ),
                PaletteCategory::Command => (
                    theme.mix(theme.ansi[4], theme.surface_bg(), 0.25),
                    theme.ansi[4],
                ),
                PaletteCategory::File => (
                    theme.mix(theme.ansi[2], theme.surface_bg(), 0.25),
                    theme.ansi[2],
                ),
            };
            let badge_x = rx + 30;
            let badge_y = item_cy.saturating_sub(10);
            canvas.rounded_fill(badge_x, badge_y, BADGE_W, 20, 4, cat_bg);
            let cat_w = self.measure_ui_text(cat_label, 9.5, true);
            let cat_text_x = badge_x + (BADGE_W.saturating_sub(cat_w)) / 2;
            self.draw_ui_text(
                canvas,
                cat_label,
                cat_text_x,
                item_baseline,
                badge_x + BADGE_W,
                cat_fg,
                9.5,
                true,
            );

            // Columna de título fija: TODOS los títulos arrancan exactamente en la misma posición (rx + 108)
            let title_x = rx + 108;
            let title_fg = if selected {
                theme.foreground
            } else {
                theme.mix(theme.foreground, theme.background, 0.08)
            };
            let right_limit = (rx + rw).saturating_sub(8);

            // Atajos de teclado (solo comandos con atajo real)
            let is_shortcut = item.category == PaletteCategory::Command
                && (item.subtitle.starts_with("Ctrl") || item.subtitle.starts_with("Alt"));

            if is_shortcut {
                let sub_w = self.measure_mono_text(&item.subtitle, tokens::MICRO, false) + 12;
                let keycap_x = right_limit.saturating_sub(sub_w);
                let keycap_y = item_cy.saturating_sub(tokens::KEYCAP_H / 2);
                self.draw_keycap(canvas, keycap_x, keycap_y, &item.subtitle, theme);

                let title_max_x = keycap_x.saturating_sub(12);
                self.draw_ui_text_truncated(
                    canvas,
                    &item.title,
                    title_x,
                    item_baseline,
                    title_max_x,
                    title_fg,
                    tokens::BODY,
                    selected,
                );
            } else if !item.subtitle.is_empty() {
                // Elementos con ruta/rama/subtítulo: columna fija bien alineada (rx + 255)
                // y corte suave con puntos suspensivos ("...") si alguno de los dos excede su ancho
                let sub_x = rx + 255;
                let title_max_x = sub_x.saturating_sub(12);
                self.draw_ui_text_truncated(
                    canvas,
                    &item.title,
                    title_x,
                    item_baseline,
                    title_max_x,
                    title_fg,
                    tokens::BODY,
                    selected,
                );

                let sub_fg = if selected {
                    theme.foreground
                } else {
                    theme.mix(theme.foreground, theme.background, 0.25)
                };
                self.draw_ui_text_truncated(
                    canvas,
                    &item.subtitle,
                    sub_x,
                    item_baseline,
                    right_limit,
                    sub_fg,
                    tokens::MICRO,
                    false,
                );
            } else {
                // Sin subtítulo: título aprovecha todo el ancho disponible hasta el margen derecho
                self.draw_ui_text_truncated(
                    canvas,
                    &item.title,
                    title_x,
                    item_baseline,
                    right_limit,
                    title_fg,
                    tokens::BODY,
                    selected,
                );
            }
        }

        // 3. Scrollbar vertical cuando hay más elementos de los que caben
        if let Some((track, thumb)) = rects.scrollbar {
            Self::draw_scrollbar_rects(canvas, track, thumb, scrollbar_hovered, true, theme);
        }
    }
}
