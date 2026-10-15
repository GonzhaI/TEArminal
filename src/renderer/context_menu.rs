//! Menú contextual flotante (clic derecho).

use super::*;
use crate::app::ContextMenu;

impl TerminalRenderer {
    /// Ancho que necesita el menú: etiqueta más larga + atajo, sin que se solapen nunca
    pub fn context_menu_width(&self, items: &[crate::app::MenuItem]) -> usize {
        let content = items
            .iter()
            .map(|i| {
                let label = self.measure_ui_text(i.label, 12.0, false);
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

    pub fn render_context_menu(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        menu: &ContextMenu,
        theme: &Theme,
    ) {
        let (x, y, w, h) = menu.bounds();
        // Sombra suave, fondo y borde
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x + 2,
            y + 4,
            w,
            h,
            10,
            Color::from_rgba(0, 0, 0, 70),
        );
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.surface_bg());
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.border_color());

        let accent = theme.accent_color();
        for (i, item) in menu.items.iter().enumerate() {
            let r = menu.item_rect(i);
            if item.action.is_none() {
                let clip: Clip = (0, 0, width, height);
                fill_rect(
                    buffer,
                    width,
                    clip,
                    r.0 + 8,
                    r.1 + r.3 / 2,
                    r.2.saturating_sub(16),
                    1,
                    theme.border_color().to_u32_xrgb(),
                );
                continue;
            }

            let hovered = item.enabled && menu.hovered == Some(i);
            if hovered {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0,
                    r.1,
                    r.2,
                    r.3,
                    6,
                    Color::from_rgba(accent.r(), accent.g(), accent.b(), 34),
                );
            }
            let (label_fg, icon_fg) = match (item.enabled, hovered) {
                (false, _) => (theme.ansi[8], theme.ansi[8]),
                (true, true) => (theme.foreground, accent),
                (true, false) => (theme.foreground, theme.ansi[7]),
            };
            let cy = r.1 + r.3 / 2;
            if let Some(icon) = item.icon {
                self.draw_lucide_icon(buffer, width, height, icon, r.0 + 18, cy, 12.5, icon_fg);
            }
            self.draw_ui_text(
                buffer,
                width,
                height,
                item.label,
                r.0 + 36,
                cy + 4,
                r.0 + r.2 - 8,
                label_fg,
                12.0,
                false,
            );
            if !item.shortcut.is_empty() {
                let sw = self.measure_ui_text(item.shortcut, 10.5, false);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    item.shortcut,
                    (r.0 + r.2).saturating_sub(sw + 10),
                    cy + 4,
                    r.0 + r.2,
                    theme.ansi[8],
                    10.5,
                    false,
                );
            }
        }
    }
}
