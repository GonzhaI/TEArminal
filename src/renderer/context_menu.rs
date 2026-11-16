//! Menú contextual flotante (clic derecho).

use super::*;
use crate::app::ContextMenu;

impl TerminalRenderer {
    /// Ancho que necesita el menú: etiqueta más larga + atajo, sin que se solapen nunca
    pub(crate) fn context_menu_width(&self, items: &[crate::app::MenuItem]) -> usize {
        let content = items
            .iter()
            .map(|i| {
                let label = self.measure_ui_text(&i.label, tokens::TITLE, false);
                if i.shortcut.is_empty() {
                    label
                } else {
                    label + 28 + self.measure_mono_text(i.shortcut, tokens::DETAIL, false)
                }
            })
            .max()
            .unwrap_or(0);
        // relleno + icono (36) + contenido + margen derecho
        (ContextMenu::PAD * 2 + 36 + content + 18).max(220)
    }

    pub(crate) fn render_context_menu(
        &mut self,
        canvas: &mut Canvas,
        menu: &ContextMenu,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        let clip: Clip = (0, 0, width, height);
        let (x, y, w, h) = menu.bounds();

        // Sombra corta, superficie mantle y borde fuerte (esquinas rectas según tokens::RADIUS_CONTROL)
        canvas.rounded_fill(
            x + 1,
            y + 3,
            w,
            h,
            tokens::RADIUS_CONTROL,
            Color::from_rgba(0, 0, 0, 80),
        );
        canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.mantle());
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
                canvas.fill_rect(
                    clip,
                    r.0 + 3,
                    r.1,
                    r.2.saturating_sub(6),
                    r.3,
                    theme.surface_level(0),
                );
            }

            let (label_fg, icon_fg) = if item.danger {
                (theme.red(), theme.red())
            } else {
                match (item.enabled, hovered) {
                    (false, _) => (theme.text_muted(), theme.text_muted()),
                    (true, true) => (theme.foreground, accent),
                    (true, false) => (theme.foreground, theme.text_secondary()),
                }
            };

            let cy = r.1 + r.3 / 2;
            let baseline = self.centered_baseline(cy);
            if let Some(icon) = item.icon {
                self.draw_lucide_icon(canvas, icon, r.0 + 18, cy, 13.0, icon_fg);
            }
            self.draw_ui_text(
                canvas,
                &item.label,
                r.0 + 36,
                baseline,
                r.0 + r.2 - 8,
                label_fg,
                tokens::TITLE,
                false,
            );
            if !item.shortcut.is_empty() {
                let sw = self.measure_mono_text(item.shortcut, tokens::DETAIL, false);
                self.draw_mono_text(
                    canvas,
                    item.shortcut,
                    (r.0 + r.2).saturating_sub(sw + 12),
                    baseline,
                    r.0 + r.2,
                    theme.text_muted(),
                    tokens::DETAIL,
                    false,
                );
            }
        }
    }
}
