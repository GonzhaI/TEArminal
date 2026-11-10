//! Panel izquierdo de Spaces: marca, búsqueda, lista de Spaces (este equipo y equipos remotos) y pie con
//! Ajustes y Add machine.

use super::*;

/// Un Space tal como lo dibuja el panel.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpaceItem {
    pub name: String,
    pub is_active: bool,
    /// Pestañas del Space (se muestra como contador si hay más de una).
    pub tab_count: usize,
    /// Posición del Space en la lista de la aplicación (lo que devuelven el clic y el hover).
    pub index: usize,
    /// Space de un equipo remoto (va bajo «Equipos remotos»); los locales, bajo «Este equipo».
    pub remote: bool,
    /// Edición en curso del nombre del Space (input activo).
    pub editing_input: Option<crate::ui::TextInput>,
}

/// Elemento del panel bajo el ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SpacesHover {
    #[default]
    None,
    Toggle,
    Search,
    /// «+» de Este equipo: un Space nuevo
    Plus,
    /// «+» de Equipos remotos: abre «Añadir máquina»
    MachinePlus,
    /// Space con esa posición en la lista de la aplicación
    Row(usize),
    Settings,
    Machine,
}

impl SpaceItem {
    /// Letra de la insignia: la primera alfanumérica del nombre, en mayúscula.
    pub(crate) fn letter(&self) -> char {
        self.name
            .chars()
            .find(|c| c.is_alphanumeric())
            .map_or('?', |c| c.to_uppercase().next().unwrap_or(c))
    }

    /// Color de la insignia (índice ANSI 1..=6): estable por nombre, así un Space conserva su color.
    pub(crate) fn color_index(&self) -> usize {
        let h = self
            .name
            .bytes()
            .fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize));
        1 + h % 6
    }
}

impl TerminalRenderer {
    pub(crate) fn render_spaces_panel(
        &mut self,
        canvas: &mut Canvas,
        items: &[SpaceItem],
        collapsed: bool,
        hover: SpacesHover,
        scroll: usize,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (_, height) = canvas.size();
        let rects = SpacesPanelRects::compute(collapsed, height);
        if height == 0 {
            return;
        }

        // Fondo del marco y línea que lo separa de la barra superior y de las tarjetas.
        Self::draw_rect(canvas, 0, 0, rects.width, height, theme.bar_bg());
        Self::draw_rect(canvas, rects.width - 1, 0, 1, height, theme.chrome_line());

        // Cabecera: marca (solo expandido), búsqueda y alternar.
        if !collapsed {
            let base = self.centered_baseline(SpacesPanelRects::HEADER_H / 2);
            self.draw_ui_text(
                canvas,
                "TEArminal",
                18,
                base,
                rects.search_btn.0.saturating_sub(8),
                theme.foreground,
                tokens::TITLE,
                true,
            );
            self.draw_chrome_action(
                canvas,
                rects.search_btn,
                crate::ui::LucideIcon::Search,
                false,
                hover == SpacesHover::Search,
                15.0,
                theme,
            );
        }
        self.draw_chrome_action(
            canvas,
            rects.toggle_btn,
            crate::ui::LucideIcon::PanelLeft,
            false,
            hover == SpacesHover::Toggle,
            15.0,
            theme,
        );
        if collapsed {
            self.draw_chrome_action(
                canvas,
                rects.plus_btn,
                crate::ui::LucideIcon::Plus,
                false,
                hover == SpacesHover::Plus,
                14.0,
                theme,
            );
        }

        // Lista: «Este equipo» y, si los hay, «Equipos remotos»
        let remote: Vec<bool> = items.iter().map(|i| i.remote).collect();
        for (slot, r) in rects.visible_slots(&remote, scroll) {
            match slot {
                SpacesSlot::Header { remote: is_remote } => {
                    if collapsed {
                        Self::draw_rect(canvas, r.0, r.1, r.2, 1, theme.chrome_line());
                        continue;
                    }
                    let label = if is_remote {
                        "Equipos remotos"
                    } else {
                        "Este equipo"
                    };
                    let plus = SpacesPanelRects::header_plus(r);
                    self.draw_ui_text(
                        canvas,
                        label,
                        r.0 + 8,
                        self.centered_baseline(r.1 + r.3 / 2),
                        plus.0,
                        theme.text_muted(),
                        tokens::BODY,
                        false,
                    );
                    let plus_hover = if is_remote {
                        SpacesHover::MachinePlus
                    } else {
                        SpacesHover::Plus
                    };
                    self.draw_chrome_action(
                        canvas,
                        plus,
                        crate::ui::LucideIcon::Plus,
                        false,
                        hover == plus_hover,
                        14.0,
                        theme,
                    );
                }
                SpacesSlot::Space(pos) => {
                    if let Some(item) = items.get(pos) {
                        self.draw_space_row(
                            canvas,
                            item,
                            r,
                            collapsed,
                            hover,
                            cursor_visible,
                            theme,
                        );
                    }
                }
            }
        }

        // Pie: línea, Ajustes y Add machine.
        Self::draw_rect(
            canvas,
            SpacesPanelRects::PAD_X,
            rects.settings_btn.1.saturating_sub(6),
            rects.width - 2 * SpacesPanelRects::PAD_X,
            1,
            theme.chrome_line(),
        );
        self.draw_footer_row(
            canvas,
            rects.settings_btn,
            crate::ui::LucideIcon::Settings,
            "Ajustes",
            collapsed,
            hover == SpacesHover::Settings,
            theme,
        );
        self.draw_footer_row(
            canvas,
            rects.machine_btn,
            crate::ui::LucideIcon::Plus,
            "Añadir máquina",
            collapsed,
            hover == SpacesHover::Machine,
            theme,
        );
    }

    fn draw_space_row(
        &mut self,
        canvas: &mut Canvas,
        item: &SpaceItem,
        r: Rect,
        collapsed: bool,
        hover: SpacesHover,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let hovered = hover == SpacesHover::Row(item.index);
        if item.is_active {
            canvas.rounded_fill(r.0, r.1, r.2, r.3, tokens::RADIUS_ROW, theme.surface_bg());
        } else if hovered {
            canvas.rounded_fill(
                r.0,
                r.1,
                r.2,
                r.3,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        let badge = SpacesPanelRects::BADGE;
        let bx = if collapsed {
            r.0 + (r.2 - badge) / 2
        } else {
            r.0 + 8
        };
        let by = r.1 + (r.3 - badge) / 2;
        canvas.rounded_fill(
            bx,
            by,
            badge,
            badge,
            tokens::RADIUS_ROW.saturating_sub(2),
            theme.ansi[item.color_index()],
        );
        let letter = item
            .editing_input
            .as_ref()
            .and_then(|inp| {
                inp.text()
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map(|c| c.to_uppercase().next().unwrap_or(c))
            })
            .unwrap_or_else(|| item.letter())
            .to_string();
        let lw = self.measure_ui_text(&letter, 11.0, true);
        let lb = self.centered_baseline(by + badge / 2);
        self.draw_ui_text(
            canvas,
            &letter,
            bx + badge.saturating_sub(lw) / 2,
            lb,
            bx + badge,
            theme.background,
            11.0,
            true,
        );
        if collapsed {
            return;
        }
        let fg = if item.is_active || hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let tx = bx + badge + 10;
        let base = self.centered_baseline(r.1 + r.3 / 2);
        let end = r.0 + r.2 - 8;

        if let Some(ref input) = item.editing_input {
            let input_x = tx.saturating_sub(4);
            let input_y = r.1 + 3;
            let input_w = end.saturating_sub(input_x);
            let input_h = r.3.saturating_sub(6);
            canvas.rounded_fill(
                input_x,
                input_y,
                input_w,
                input_h,
                tokens::RADIUS_ROW.saturating_sub(2),
                theme.surface_bg(),
            );
            canvas.rounded_stroke(
                input_x,
                input_y,
                input_w,
                input_h,
                tokens::RADIUS_ROW.saturating_sub(2),
                theme.accent_color(),
            );
            self.render_text_input_content(
                canvas,
                input,
                input_x + 6,
                base,
                input_x + input_w.saturating_sub(6),
                input_y,
                input_h,
                cursor_visible,
                "Nombre del espacio",
                true,
                theme,
            );
            return;
        }

        let counter = (item.tab_count > 1).then(|| item.tab_count.to_string());
        let name_end = match &counter {
            Some(c) => end.saturating_sub(self.measure_mono_text(c, tokens::MICRO, false) + 8),
            None => end,
        };
        self.draw_ui_text(
            canvas,
            &item.name,
            tx,
            base,
            name_end,
            fg,
            tokens::BODY,
            item.is_active,
        );
        if let Some(c) = counter {
            let cw = self.measure_mono_text(&c, tokens::MICRO, false);
            self.draw_mono_text(
                canvas,
                &c,
                end - cw,
                base,
                end,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );
        }
    }

    fn draw_footer_row(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        label: &str,
        collapsed: bool,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if hovered {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_ROW, theme.surface_hover());
        }
        let fg = if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let icon_x = if collapsed { x + w / 2 } else { x + 16 };
        self.draw_lucide_icon(canvas, icon, icon_x, y + h / 2, 14.0, fg);
        if !collapsed {
            let base = self.centered_baseline(y + h / 2);
            self.draw_ui_text(
                canvas,
                label,
                x + 34,
                base,
                x + w - 8,
                fg,
                tokens::BODY,
                false,
            );
        }
    }
}
