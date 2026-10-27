//! Menú contextual flotante (clic derecho).

use super::*;
use crate::app::ContextMenu;

impl TerminalRenderer {
    /// Ancho que necesita el menú: etiqueta más larga + atajo, sin que se solapen nunca
    pub(crate) fn context_menu_width(&self, items: &[crate::app::MenuItem]) -> usize {
        let content = items
            .iter()
            .map(|i| {
                let label = self.measure_ui_text(&i.label, 12.0, false);
                if i.shortcut.is_empty() {
                    label
                } else {
                    label + 28 + self.measure_ui_text(i.shortcut, 10.5, false)
                }
            })
            .max()
            .unwrap_or(0);
        // relleno + icono (36) + contenido + margen derecho
        (ContextMenu::PAD * 2 + 36 + content + 14).max(200)
    }

    pub(crate) fn render_context_menu(
        &mut self,
        canvas: &mut Canvas,
        menu: &ContextMenu,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        let (x, y, w, h) = menu.bounds();
        // Sombra corta, superficie elevada y hairline: un menú, no una isla.
        canvas.rounded_fill(
            x + 1,
            y + 3,
            w,
            h,
            tokens::RADIUS_CONTROL,
            Color::from_rgba(0, 0, 0, 80),
        );
        canvas.rounded_fill(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_bg_elevated(),
        );
        canvas.rounded_stroke(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );

        let accent = theme.accent_color();
        for (i, item) in menu.items.iter().enumerate() {
            let r = menu.item_rect(i);
            if item.action.is_none() {
                let clip: Clip = (0, 0, width, height);
                canvas.fill_rect(
                    clip,
                    r.0 + 8,
                    r.1 + r.3 / 2,
                    r.2.saturating_sub(16),
                    1,
                    theme.chrome_line(),
                );
                continue;
            }

            let hovered = item.enabled && menu.hovered == Some(i);
            if hovered {
                canvas.rounded_fill(
                    r.0 + 4,
                    r.1,
                    r.2.saturating_sub(8),
                    r.3,
                    3,
                    theme.accent_soft(30),
                );
            }
            let (label_fg, icon_fg) = match (item.enabled, hovered) {
                (false, _) => (theme.text_muted(), theme.text_muted()),
                (true, true) => (theme.foreground, accent),
                (true, false) => (theme.foreground, theme.text_secondary()),
            };
            let cy = r.1 + r.3 / 2;
            if let Some(icon) = item.icon {
                self.draw_lucide_icon(canvas, icon, r.0 + 18, cy, 12.0, icon_fg);
            }
            self.draw_ui_text(
                canvas,
                &item.label,
                r.0 + 36,
                cy + 4,
                r.0 + r.2 - 8,
                label_fg,
                11.5,
                false,
            );
            if !item.shortcut.is_empty() {
                let sw = self.measure_ui_text(item.shortcut, 10.0, false);
                self.draw_ui_text(
                    canvas,
                    item.shortcut,
                    (r.0 + r.2).saturating_sub(sw + 10),
                    cy + 4,
                    r.0 + r.2,
                    theme.text_muted(),
                    10.0,
                    false,
                );
            }
        }
    }
}
