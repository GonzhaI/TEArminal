//! Modal «Añadir máquina»: hosts de `~/.ssh/config` y campo `usuario@host`.

use super::*;

impl TerminalRenderer {
    pub(crate) fn render_machine_modal(
        &mut self,
        canvas: &mut Canvas,
        hosts: &[(String, String)],
        input: &crate::ui::TextInput,
        hovered: Option<MachineHover>,
        selected: Option<usize>,
        error: Option<&str>,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        let rects = MachineModalRects::compute(width, height, hosts.len());
        let (x, y, w, h) = rects.card;
        if w < 280 || h == 0 {
            return;
        }
        let pad = MachineModalRects::PAD;
        let right = x + w - pad;

        // 1. Tarjeta con fondo mantle y contorno fuerte
        canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CARD, theme.mantle());
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CARD, theme.chrome_line_strong());

        // 2. Franja superior tintada (estilo común de diálogo F4)
        let header_h = 40;
        let blue = theme.accent_color();
        let tint_bg = theme.mix(theme.mantle(), blue, 0.16);
        let clip: Clip = (0, 0, width, height);
        canvas.fill_rect(clip, x, y, w, header_h, tint_bg);
        canvas.fill_rect(clip, x, y + header_h - 1, w, 1, theme.chrome_line());

        // Ícono, título y botón de cierre en la franja
        let header_cy = y + header_h / 2;
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Server,
            x + 18,
            header_cy,
            15.0,
            blue,
        );
        let title_base = self.centered_baseline(header_cy);
        self.draw_ui_text_tracked(
            canvas,
            "Añadir una máquina",
            x + 38,
            title_base,
            right - 40,
            theme.foreground,
            tokens::TITLE,
            true,
            tokens::TRACKING,
        );
        let esc_w = self.measure_ui_text("Esc", tokens::MICRO, false);
        self.draw_ui_text(
            canvas,
            "Esc",
            right - esc_w,
            title_base,
            right,
            theme.text_secondary(),
            tokens::MICRO,
            false,
        );

        // 3. Descripción
        self.draw_ui_text(
            canvas,
            "Un VPS, un Mac mini o un equipo en casa, por ssh.",
            x + pad,
            self.centered_baseline(y + 60),
            right,
            theme.text_secondary(),
            tokens::BODY,
            false,
        );

        // 4. Hosts de la configuración de ssh
        if !rects.rows.is_empty() {
            self.draw_ui_text_tracked(
                canvas,
                "Desde tu ssh config",
                x + pad,
                self.centered_baseline(rects.list_label_y + 8),
                right,
                theme.text_muted(),
                tokens::MICRO,
                true,
                tokens::TRACKING,
            );
        }
        for (i, ((row, btn), (alias, target))) in rects
            .rows
            .iter()
            .zip(&rects.row_buttons)
            .zip(hosts)
            .enumerate()
        {
            let is_hovered = hovered == Some(MachineHover::Row(i)) || selected == Some(i);
            // Tarjeta de host
            let row_bg = if is_hovered {
                theme.surface_hover()
            } else {
                theme.chrome_bg()
            };
            canvas.fill_rect(clip, row.0, row.1, row.2, row.3, row_bg);
            canvas.rounded_stroke(
                row.0,
                row.1,
                row.2,
                row.3,
                tokens::RADIUS_ROW,
                theme.chrome_line(),
            );

            let cy = row.1 + row.3 / 2;
            let base = self.centered_baseline(cy);
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Server,
                row.0 + 16,
                cy,
                14.0,
                theme.text_secondary(),
            );
            let alias_end = self.draw_ui_text(
                canvas,
                alias,
                row.0 + 34,
                base,
                btn.0 - 12,
                theme.foreground,
                tokens::BODY,
                true,
            );
            self.draw_mono_text(
                canvas,
                target,
                alias_end + 10,
                base,
                btn.0 - 12,
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );
            self.draw_machine_button(canvas, *btn, "Añadir", is_hovered, true, theme);
        }

        // 5. Campo `usuario@host`
        self.draw_ui_text_tracked(
            canvas,
            "O escribe cómo llega ssh",
            x + pad,
            self.centered_baseline(rects.typed_label_y + 10),
            right,
            theme.text_muted(),
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );
        let (ix, iy, iw, ih) = rects.input;
        canvas.fill_rect(clip, ix, iy, iw, ih, theme.chrome_bg());
        canvas.rounded_stroke(
            ix,
            iy,
            iw,
            ih,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );
        self.render_text_input_content(
            canvas,
            input,
            ix + 12,
            self.centered_baseline(iy + ih / 2),
            ix + iw - 10,
            iy,
            ih,
            cursor_visible,
            "usuario@host",
            true,
            theme,
        );
        let can_add = !input.text().trim().is_empty();
        self.draw_machine_button(
            canvas,
            rects.add_btn,
            "Añadir",
            hovered == Some(MachineHover::TypedAdd) && can_add,
            can_add,
            theme,
        );
        if let Some(err) = error {
            self.draw_ui_text(
                canvas,
                err,
                x + pad,
                self.centered_baseline(rects.error_y + 8),
                right,
                theme.ansi[1],
                tokens::DETAIL,
                false,
            );
        }
    }

    fn draw_machine_button(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        label: &str,
        hovered: bool,
        enabled: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if hovered {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_ROW, theme.surface_hover());
        }
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_ROW, theme.chrome_line_strong());
        let fg = if !enabled {
            theme.text_muted()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let lw = self.measure_ui_text(label, tokens::BODY, false);
        self.draw_ui_text(
            canvas,
            label,
            x + w.saturating_sub(lw) / 2,
            self.centered_baseline(y + h / 2),
            x + w - 4,
            fg,
            tokens::BODY,
            false,
        );
    }
}
