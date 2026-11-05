//! Renderizado sincronizado del diff lado a lado (side-by-side) para el visor de archivos.

use super::*;
use crate::app::FileViewerState;
use crate::theme::Theme;

impl TerminalRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_side_by_side(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        full_w: usize,
        w: usize,
        h: usize,
        _right_inset: usize,
        clip: Clip,
        viewer: &FileViewerState,
        theme: &Theme,
        ext: &str,
    ) {
        let rows = viewer.side_by_side_rows();
        let total_rows = rows.len();
        let max_line = viewer
            .lines
            .len()
            .max(viewer.git_base.as_ref().map_or(0, |b| b.len()))
            .max(total_rows);
        let lay = crate::renderer::geometry::SideBySideLayout::compute(
            x,
            y,
            w,
            h,
            self.row_height,
            self.char_width,
            max_line,
        );
        let (row_h, char_w) = (lay.row_h, lay.char_w);
        let content_y = lay.content_y;
        let content_h = lay.content_h;
        if content_h == 0 {
            return;
        }

        // 1. Barra de encabezado de columnas (HEAD vs Disco)
        let col_header_bg = theme.chrome_bg_elevated();
        canvas.fill_rect(
            clip,
            x,
            lay.col_header_y,
            full_w,
            lay.col_header_h,
            col_header_bg,
        );
        canvas.fill_rect(
            clip,
            x,
            lay.col_header_y + lay.col_header_h - 1,
            full_w,
            1,
            theme.chrome_line(),
        );

        let col_base = self.centered_baseline(lay.col_header_y + lay.col_header_h / 2);
        let (dels, adds) = viewer.side_by_side_stats();

        // Encabezado columna izquierda (HEAD / Base)
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::GitBranch,
            lay.left_x + SidebarRects::CARD_INSET + 16,
            lay.col_header_y + lay.col_header_h / 2,
            tokens::MICRO,
            theme.ansi[3],
        );
        let left_title = "HEAD (versión base)";
        self.draw_ui_text(
            canvas,
            left_title,
            lay.left_x + SidebarRects::CARD_INSET + 34,
            col_base,
            lay.divider_x.saturating_sub(60),
            theme.foreground,
            tokens::SECTION,
            true,
        );
        if dels > 0 {
            let del_badge = format!("–{}", dels);
            let badge_w = self.measure_mono_text(&del_badge, tokens::MICRO, true) + 12;
            let badge_x = lay.divider_x.saturating_sub(badge_w + 12);
            if badge_x > lay.left_x + 180 {
                canvas.rounded_fill(
                    badge_x,
                    lay.col_header_y + 4,
                    badge_w,
                    lay.col_header_h.saturating_sub(8),
                    tokens::RADIUS_ROW,
                    Color::from_rgba(theme.ansi[1].r(), theme.ansi[1].g(), theme.ansi[1].b(), 35),
                );
                self.draw_mono_text(
                    canvas,
                    &del_badge,
                    badge_x + 6,
                    col_base,
                    badge_x + badge_w,
                    theme.ansi[1],
                    tokens::MICRO,
                    true,
                );
            }
        }

        // Separador vertical entre columnas en el encabezado
        canvas.fill_rect(
            clip,
            lay.divider_x,
            lay.col_header_y,
            1,
            lay.col_header_h,
            theme.chrome_line(),
        );

        // Encabezado columna derecha (Disco / Actual)
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::FileDiff,
            lay.right_x + SidebarRects::CARD_INSET + 16,
            lay.col_header_y + lay.col_header_h / 2,
            tokens::MICRO,
            theme.ansi[2],
        );
        let right_title = "Disco (cambios locales)";
        self.draw_ui_text(
            canvas,
            right_title,
            lay.right_x + SidebarRects::CARD_INSET + 34,
            col_base,
            (x + w).saturating_sub(60),
            theme.foreground,
            tokens::SECTION,
            true,
        );
        if adds > 0 {
            let add_badge = format!("+{}", adds);
            let badge_w = self.measure_mono_text(&add_badge, tokens::MICRO, true) + 12;
            let badge_x = (x + w).saturating_sub(badge_w + 24);
            if badge_x > lay.right_x + 180 {
                canvas.rounded_fill(
                    badge_x,
                    lay.col_header_y + 4,
                    badge_w,
                    lay.col_header_h.saturating_sub(8),
                    tokens::RADIUS_ROW,
                    Color::from_rgba(theme.ansi[2].r(), theme.ansi[2].g(), theme.ansi[2].b(), 35),
                );
                self.draw_mono_text(
                    canvas,
                    &add_badge,
                    badge_x + 6,
                    col_base,
                    badge_x + badge_w,
                    theme.ansi[2],
                    tokens::MICRO,
                    true,
                );
            }
        }

        // 2. Fondos de los gutters y líneas divisorias del cuerpo
        let gutter_bg = theme.mix(theme.background, theme.foreground, 0.03);
        let chrome_line = theme.chrome_line();

        canvas.fill_rect(
            clip,
            lay.left_x,
            content_y,
            lay.left_gutter_w,
            content_h,
            gutter_bg,
        );
        canvas.fill_rect(clip, lay.left_sep_x, content_y, 1, content_h, chrome_line);

        canvas.fill_rect(clip, lay.divider_x, content_y, 1, content_h, chrome_line);

        canvas.fill_rect(
            clip,
            lay.right_x,
            content_y,
            lay.right_gutter_w,
            content_h,
            gutter_bg,
        );
        canvas.fill_rect(clip, lay.right_sep_x, content_y, 1, content_h, chrome_line);

        let left_gutter_clip: Clip = (lay.left_x, content_y, lay.left_sep_x, content_y + content_h);
        let left_text_clip: Clip = (
            lay.left_sep_x + 1,
            content_y,
            lay.left_text_max_x,
            content_y + content_h,
        );
        let right_gutter_clip: Clip = (
            lay.right_x,
            content_y,
            lay.right_sep_x,
            content_y + content_h,
        );
        let right_text_clip: Clip = (
            lay.right_sep_x + 1,
            content_y,
            lay.right_text_max_x,
            content_y + content_h,
        );

        let baseline_offset = ((row_h as f32 * 0.76) as usize).max(char_w);
        let visible_rows = content_h / row_h;
        let start_row = viewer.scroll_offset.min(total_rows.saturating_sub(1));
        let end_row = (start_row + visible_rows + 1).min(total_rows);

        let del_tint = Color::from_u32_xrgb(theme.background.blend_over(theme.ansi[1], 34));
        let add_tint = Color::from_u32_xrgb(theme.background.blend_over(theme.ansi[2], 34));
        let mod_left_tint = Color::from_u32_xrgb(theme.background.blend_over(theme.ansi[3], 34));
        let mod_right_tint = Color::from_u32_xrgb(theme.background.blend_over(theme.ansi[3], 34));
        let empty_tint = theme.mix(theme.background, theme.foreground, 0.02);

        // 3. Dibujo de filas sincronizadas
        for (idx, r) in rows[start_row..end_row].iter().enumerate() {
            let row_y = content_y + idx * row_h;
            let baseline_y = row_y + baseline_offset;

            // Fila separadora de fragmentos (hunk @@)
            if r.left.kind == crate::app::DiffCellKind::Separator {
                let sep_bg = theme.mix(theme.background, theme.foreground, 0.05);
                let sep_line = theme.chrome_line();
                let sep_fg = theme.ansi[4]; // Cyan característico de @@

                // Franja horizontal en todo el ancho con borde sutil
                canvas.fill_rect(clip, lay.left_x, row_y, w, row_h, sep_bg);
                canvas.fill_rect(clip, lay.left_x, row_y, w, 1, sep_line);
                canvas.fill_rect(clip, lay.left_x, row_y + row_h - 1, w, 1, sep_line);
                canvas.fill_rect(clip, lay.divider_x, row_y, 1, row_h, sep_line);

                let text = &r.left.text;
                if !text.is_empty() {
                    self.draw_mono_text(
                        canvas,
                        text,
                        lay.left_text_x,
                        baseline_y,
                        lay.left_text_max_x,
                        sep_fg,
                        tokens::MICRO,
                        true,
                    );
                    self.draw_mono_text(
                        canvas,
                        text,
                        lay.right_text_x,
                        baseline_y,
                        lay.right_text_max_x,
                        sep_fg,
                        tokens::MICRO,
                        true,
                    );
                }
                continue;
            }

            // --- COLUMNA IZQUIERDA ---
            let left_row_clip: Clip = (lay.left_x, row_y, lay.divider_x, row_y + row_h);
            match r.left.kind {
                crate::app::DiffCellKind::Deleted => {
                    canvas.fill_rect(
                        left_row_clip,
                        lay.left_sep_x + 1,
                        row_y,
                        lay.divider_x.saturating_sub(lay.left_sep_x + 1),
                        row_h,
                        del_tint,
                    );
                    canvas.fill_rect(
                        left_gutter_clip,
                        lay.left_sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        theme.ansi[1],
                    );
                }
                crate::app::DiffCellKind::Modified => {
                    canvas.fill_rect(
                        left_row_clip,
                        lay.left_sep_x + 1,
                        row_y,
                        lay.divider_x.saturating_sub(lay.left_sep_x + 1),
                        row_h,
                        mod_left_tint,
                    );
                    canvas.fill_rect(
                        left_gutter_clip,
                        lay.left_sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        theme.ansi[3],
                    );
                }
                crate::app::DiffCellKind::Empty => {
                    canvas.fill_rect(
                        left_row_clip,
                        lay.left_sep_x + 1,
                        row_y,
                        lay.divider_x.saturating_sub(lay.left_sep_x + 1),
                        row_h,
                        empty_tint,
                    );
                }
                crate::app::DiffCellKind::Normal
                | crate::app::DiffCellKind::Added
                | crate::app::DiffCellKind::Separator => {}
            }

            if let Some(line_num) = r.left.line_num {
                let num_str = line_num.to_string();
                let num_w = num_str.len() * char_w;
                let num_x = lay.left_sep_x.saturating_sub(num_w + 10);
                let num_color = match r.left.kind {
                    crate::app::DiffCellKind::Deleted => theme.ansi[1],
                    crate::app::DiffCellKind::Modified => theme.ansi[3],
                    _ => theme.text_muted(),
                };
                for (ci, &b) in num_str.as_bytes().iter().enumerate() {
                    let glyph = self.get_glyph(b as char, false);
                    let gx = (num_x + ci * char_w) as i32 + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(left_gutter_clip, glyph, gx, gy, num_color);
                }

                self.render_highlighted_line(
                    canvas,
                    left_text_clip,
                    &r.left.text,
                    lay.left_text_x as i32,
                    baseline_y,
                    lay.left_text_max_x,
                    char_w,
                    theme,
                    ext,
                    false,
                );
            }

            // --- COLUMNA DERECHA ---
            let right_row_clip: Clip = (lay.right_x, row_y, x + w, row_y + row_h);
            match r.right.kind {
                crate::app::DiffCellKind::Added => {
                    canvas.fill_rect(
                        right_row_clip,
                        lay.right_sep_x + 1,
                        row_y,
                        (x + w).saturating_sub(lay.right_sep_x + 1),
                        row_h,
                        add_tint,
                    );
                    canvas.fill_rect(
                        right_gutter_clip,
                        lay.right_sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        theme.ansi[2],
                    );
                }
                crate::app::DiffCellKind::Modified => {
                    canvas.fill_rect(
                        right_row_clip,
                        lay.right_sep_x + 1,
                        row_y,
                        (x + w).saturating_sub(lay.right_sep_x + 1),
                        row_h,
                        mod_right_tint,
                    );
                    canvas.fill_rect(
                        right_gutter_clip,
                        lay.right_sep_x.saturating_sub(3),
                        row_y,
                        3,
                        row_h,
                        theme.ansi[3],
                    );
                }
                crate::app::DiffCellKind::Empty => {
                    canvas.fill_rect(
                        right_row_clip,
                        lay.right_sep_x + 1,
                        row_y,
                        (x + w).saturating_sub(lay.right_sep_x + 1),
                        row_h,
                        empty_tint,
                    );
                }
                crate::app::DiffCellKind::Normal
                | crate::app::DiffCellKind::Deleted
                | crate::app::DiffCellKind::Separator => {}
            }

            if let Some(line_num) = r.right.line_num {
                let num_str = line_num.to_string();
                let num_w = num_str.len() * char_w;
                let num_x = lay.right_sep_x.saturating_sub(num_w + 10);
                let num_color = match r.right.kind {
                    crate::app::DiffCellKind::Added => theme.ansi[2],
                    crate::app::DiffCellKind::Modified => theme.ansi[3],
                    _ => theme.text_muted(),
                };
                for (ci, &b) in num_str.as_bytes().iter().enumerate() {
                    let glyph = self.get_glyph(b as char, false);
                    let gx = (num_x + ci * char_w) as i32 + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(right_gutter_clip, glyph, gx, gy, num_color);
                }

                self.render_highlighted_line(
                    canvas,
                    right_text_clip,
                    &r.right.text,
                    lay.right_text_x as i32,
                    baseline_y,
                    lay.right_text_max_x,
                    char_w,
                    theme,
                    ext,
                    false,
                );
            }
        }

        // 4. Barra de desplazamiento vertical sincronizada
        if let Some((track, thumb)) =
            Self::file_viewer_scrollbar_geometry(x, y, w, h, viewer, self.row_height)
        {
            Self::draw_scrollbar_rects(
                canvas,
                track,
                thumb,
                viewer.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }
    }
}
