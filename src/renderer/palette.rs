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
        let progress = self.palette_anim_progress.unwrap_or(1.0).clamp(0.0, 1.0);
        let offset_y = if progress < 1.0 {
            ((1.0 - progress) * -16.0).round() as isize
        } else {
            0
        };
        let rects = PaletteRects::compute_with_scroll_offset_y(
            win_w,
            win_h,
            items_len,
            state.scroll_offset,
            offset_y,
        );

        // Velo translúcido modulado por el progreso de apertura
        let overlay_alpha = (125.0 * progress).round() as u8;
        canvas.rounded_fill(
            0,
            0,
            win_w,
            win_h,
            0,
            Color::from_rgba(0, 0, 0, overlay_alpha),
        );

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
        let card_bg = theme.chrome_bg_elevated();
        // Texto legible sobre `bg`: `fg` si contrasta lo suficiente; si no, el extremo del tema que más contraste da.
        let readable = |fg: Color, bg: Color| {
            if crate::theme::contrast_ratio(fg, bg) >= 4.5 {
                fg
            } else if crate::theme::contrast_ratio(theme.foreground, bg)
                >= crate::theme::contrast_ratio(theme.background, bg)
            {
                theme.foreground
            } else {
                theme.background
            }
        };
        // Etiquetas de categoría: ancho común a las tres, el del texto más largo (ya no se corta «Comando»).
        let badge_w = [
            PaletteCategory::Session,
            PaletteCategory::Command,
            PaletteCategory::File,
        ]
        .iter()
        .map(|c| self.measure_ui_text(c.label(), 9.5, true))
        .max()
        .unwrap_or(0)
            + 16;

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

            let sel_bg = theme.mix(theme.surface_hover(), theme.accent_color(), 0.28);
            let row_bg = if selected { sel_bg } else { card_bg };
            if selected {
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
            match &item.file {
                Some(path) => {
                    self.draw_file_icon(canvas, path, rx + 14, item_cy, tokens::BODY, theme)
                }
                None => self.draw_lucide_icon(canvas, item.icon, rx + 14, item_cy, 14.0, icon_col),
            }

            // Categoría badge con ancho común y texto centrado para alinear todos los elementos: un tinte
            // suave del color (`mix` va de la superficie hacia el color) con el texto en ese color si se lee.
            let cat_label = item.category.label();
            let cat_ansi = match item.category {
                PaletteCategory::Session => theme.ansi[6],
                PaletteCategory::Command => theme.ansi[4],
                PaletteCategory::File => theme.ansi[2],
            };
            let cat_bg = theme.mix(row_bg, cat_ansi, 0.22);
            let cat_fg = readable(cat_ansi, cat_bg);
            let badge_x = rx + 30;
            let badge_y = item_cy.saturating_sub(10);
            canvas.rounded_fill(badge_x, badge_y, badge_w, 20, 4, cat_bg);
            let cat_w = self.measure_ui_text(cat_label, 9.5, true);
            let cat_text_x = badge_x + (badge_w.saturating_sub(cat_w)) / 2;
            self.draw_ui_text(
                canvas,
                cat_label,
                cat_text_x,
                item_baseline,
                badge_x + badge_w,
                cat_fg,
                9.5,
                true,
            );

            // Columna de título fija: TODOS los títulos arrancan exactamente en la misma posición, tras la etiqueta
            let title_x = badge_x + badge_w + 10;
            let title_fg = readable(
                if selected {
                    theme.foreground
                } else {
                    theme.mix(theme.foreground, theme.background, 0.08)
                },
                row_bg,
            );
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
                // Elementos con ruta/rama/subtítulo: columna fija bien alineada (147 px tras el título)
                // y corte suave con puntos suspensivos ("...") si alguno de los dos excede su ancho
                let sub_x = title_x + 147;
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

                let sub_fg = readable(
                    if selected {
                        theme.foreground
                    } else {
                        theme.mix(theme.foreground, theme.background, 0.25)
                    },
                    row_bg,
                );
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
