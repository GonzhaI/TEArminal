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
    /// «+» de Este equipo: un Space nuevo
    Plus,
    /// «+» de Equipos remotos: abre «Añadir máquina»
    MachinePlus,
    /// Space con esa posición en la lista de la aplicación
    Row(usize),
    Settings,
    Machine,
    Terminal,
    RailModule(crate::config::AppModule),
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
    /// Dibuja el riel de navegación izquierdo (DISENO §4):
    /// 56 px de ancho, fondo crust, borde s1 a la derecha, 5 botones de módulos de 58 px de alto con icono 20 px
    /// y etiqueta mayúscula de 10 px, y el botón de Ajustes anclado al pie del riel.
    pub(crate) fn render_rail(
        &mut self,
        canvas: &mut Canvas,
        active_module: crate::config::AppModule,
        git_changes: usize,
        hover: SpacesHover,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width < RailRects::WIDTH || height < TopTabBarRects::HEIGHT + StatusBarRects::BAR_H {
            return;
        }

        let rects = RailRects::compute(height, StatusBarRects::BAR_H);
        let (rx, ry, rw, rh) = rects.rect;

        // Fondo del riel (crust) y borde derecho (s1)
        Self::draw_rect(canvas, rx, ry, rw, rh, theme.rail_bg());
        Self::draw_rect(canvas, rx + rw - 1, ry, 1, rh, theme.chrome_line());

        // 5 Botones de módulos en el riel
        let modules = [
            (
                crate::config::AppModule::Spaces,
                crate::ui::LucideIcon::Terminal,
                "ESPACIOS",
            ),
            (
                crate::config::AppModule::Files,
                crate::ui::LucideIcon::Folder,
                "ARCHIVOS",
            ),
            (
                crate::config::AppModule::Git,
                crate::ui::LucideIcon::GitBranch,
                "GIT",
            ),
            (
                crate::config::AppModule::Data,
                crate::ui::LucideIcon::Database,
                "DATOS",
            ),
            (
                crate::config::AppModule::System,
                crate::ui::LucideIcon::Sliders,
                "SISTEMA",
            ),
        ];

        for (module, icon, label) in modules {
            let btn_rect = rects.module_btn(module);
            let is_active = active_module == module;
            let is_hovered = hover == SpacesHover::RailModule(module);

            if is_active {
                // Fondo mantle y barra izquierda de 2px blue (DISENO §3.3)
                Self::draw_rect(
                    canvas,
                    btn_rect.0,
                    btn_rect.1,
                    btn_rect.2,
                    btn_rect.3,
                    theme.bar_bg(),
                );
                Self::draw_rect(
                    canvas,
                    btn_rect.0,
                    btn_rect.1,
                    2,
                    btn_rect.3,
                    theme.accent_color(),
                );
            } else if is_hovered {
                Self::draw_rect(
                    canvas,
                    btn_rect.0,
                    btn_rect.1,
                    btn_rect.2,
                    btn_rect.3,
                    theme.surface_hover(),
                );
            }

            let fg = if is_active || is_hovered {
                theme.foreground
            } else {
                theme.text_secondary()
            };

            // Icono de 20 px centrado
            let icon_cx = btn_rect.0 + btn_rect.2 / 2;
            let icon_cy = btn_rect.1 + 18;
            self.draw_lucide_icon(canvas, icon, icon_cx, icon_cy, 20.0, fg);

            // Etiqueta mayúscula de 10 px
            let label_w = self.measure_ui_text(label, 10.0, true);
            let label_x = (btn_rect.0 + btn_rect.2).saturating_sub(label_w) / 2;
            let label_base = self.centered_baseline(btn_rect.1 + 43);
            self.draw_ui_text(
                canvas,
                label,
                label_x.max(btn_rect.0 + 2),
                label_base,
                btn_rect.0 + btn_rect.2 - 2,
                fg,
                10.0,
                true,
            );

            // Insignia de cambios de Git (DISENO §4)
            if module == crate::config::AppModule::Git && git_changes > 0 {
                let badge_str = if git_changes > 99 {
                    "99+".to_string()
                } else {
                    git_changes.to_string()
                };
                let bw = (self.measure_mono_text(&badge_str, tokens::MICRO, true) + 8).max(18);
                let bx = (btn_rect.0 + btn_rect.2).saturating_sub(bw + 4);
                let by = btn_rect.1 + 4;
                Self::draw_rect(canvas, bx, by, bw, 16, theme.accent_color());
                let badge_base = self.centered_baseline(by + 8);
                self.draw_mono_text(
                    canvas,
                    &badge_str,
                    bx + 4,
                    badge_base,
                    bx + bw,
                    theme.background,
                    tokens::MICRO,
                    true,
                );
            }
        }

        // Botón Ajustes al pie del riel (DISENO §4)
        let s_rect = rects.settings_btn;
        let s_hover = hover == SpacesHover::Settings;
        Self::draw_rect(canvas, s_rect.0, s_rect.1, s_rect.2, 1, theme.chrome_line());
        if s_hover {
            Self::draw_rect(
                canvas,
                s_rect.0,
                s_rect.1 + 1,
                s_rect.2,
                s_rect.3 - 1,
                theme.surface_hover(),
            );
        }
        let s_fg = if s_hover {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Settings,
            s_rect.0 + s_rect.2 / 2,
            s_rect.1 + 18,
            20.0,
            s_fg,
        );
        let s_label = "AJUSTES";
        let s_label_w = self.measure_ui_text(s_label, 10.0, true);
        let s_label_x = (s_rect.0 + s_rect.2).saturating_sub(s_label_w) / 2;
        let s_base = self.centered_baseline(s_rect.1 + 43);
        self.draw_ui_text(
            canvas,
            s_label,
            s_label_x.max(s_rect.0 + 2),
            s_base,
            s_rect.0 + s_rect.2 - 2,
            s_fg,
            10.0,
            true,
        );
    }

    /// Cabecera contextual del panel lateral (DISENO §4)
    pub(crate) fn render_sidebar_header(
        &mut self,
        canvas: &mut Canvas,
        module: crate::config::AppModule,
        rects: ContextualSidebarRects,
        hover: SpacesHover,
        theme: &Theme,
    ) {
        if rects.collapsed || rects.width == 0 {
            return;
        }

        let (hx, hy, hw, hh) = rects.header;

        // Fondo mantle y línea separadora inferior (s1)
        Self::draw_rect(canvas, hx, hy, hw, hh, theme.bar_bg());
        Self::draw_rect(canvas, hx, hy + hh - 1, hw, 1, theme.chrome_line());
        // Línea separadora derecha con el área de trabajo (s1)
        Self::draw_rect(canvas, hx + hw - 1, hy, 1, hh, theme.chrome_line());

        // Título del módulo
        let title = match module {
            crate::config::AppModule::Spaces => "ESPACIOS",
            crate::config::AppModule::Files => "ARCHIVOS",
            crate::config::AppModule::Git => "CONTROL DE CÓDIGO",
            crate::config::AppModule::Data => "BASES DE DATOS",
            crate::config::AppModule::System => "SISTEMA",
        };

        let base = self.centered_baseline(hy + hh / 2);
        self.draw_ui_text(
            canvas,
            title,
            hx + 14,
            base,
            rects.action_btn.0.saturating_sub(6),
            theme.foreground,
            tokens::TITLE,
            true,
        );

        // Botón de acción contextual en cabecera (+ para Espacios/Archivos, refrescar para Git)
        if rects.action_btn.2 > 0 {
            let act_icon = match module {
                crate::config::AppModule::Spaces => crate::ui::LucideIcon::Plus,
                crate::config::AppModule::Files => crate::ui::LucideIcon::Plus,
                crate::config::AppModule::Git => crate::ui::LucideIcon::RefreshCw,
                _ => crate::ui::LucideIcon::Plus,
            };
            self.draw_chrome_action(
                canvas,
                rects.action_btn,
                act_icon,
                false,
                hover == SpacesHover::Plus,
                14.0,
                theme,
            );
        }

        // Botón contraer panel contextual (PanelLeft)
        if rects.toggle_btn.2 > 0 {
            self.draw_chrome_action(
                canvas,
                rects.toggle_btn,
                crate::ui::LucideIcon::PanelLeft,
                false,
                hover == SpacesHover::Toggle,
                15.0,
                theme,
            );
        }
    }

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
        if height == 0 || collapsed {
            return;
        }

        let sidebar_x = SpacesPanelRects::RAIL_W;
        let sidebar_w = SpacesPanelRects::WIDTH - SpacesPanelRects::RAIL_W;
        let status_h = StatusBarRects::BAR_H;
        let total_h = height.saturating_sub(TopTabBarRects::HEIGHT + status_h);

        // Fondo del panel lateral contextual (mantle) y línea derecha (s1)
        Self::draw_rect(
            canvas,
            sidebar_x,
            TopTabBarRects::HEIGHT,
            sidebar_w,
            total_h,
            theme.bar_bg(),
        );
        Self::draw_rect(
            canvas,
            sidebar_x + sidebar_w - 1,
            TopTabBarRects::HEIGHT,
            1,
            total_h,
            theme.chrome_line(),
        );

        // Cabecera contextual del panel de Spaces
        let sidebar_rects = ContextualSidebarRects::compute(
            collapsed,
            crate::config::AppModule::Spaces,
            height,
            status_h,
        );
        self.render_sidebar_header(
            canvas,
            crate::config::AppModule::Spaces,
            sidebar_rects,
            hover,
            theme,
        );

        // Lista: «Este equipo» y, si los hay, «Equipos remotos»
        let remote: Vec<bool> = items.iter().map(|i| i.remote).collect();
        for (slot, r) in rects.visible_slots(&remote, scroll) {
            match slot {
                SpacesSlot::Header { remote: is_remote } => {
                    let label = if is_remote {
                        "EQUIPOS REMOTOS"
                    } else {
                        "ESTE EQUIPO"
                    };
                    let plus = SpacesPanelRects::header_plus(r);
                    self.draw_ui_text(
                        canvas,
                        label,
                        r.0 + 8,
                        self.centered_baseline(r.1 + r.3 / 2),
                        plus.0,
                        theme.text_muted(),
                        tokens::MICRO,
                        true,
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

        // Pie: línea separadora superior y botones «Terminal» y «Añadir máquina» (DISENO §4)
        if rects.terminal_btn.2 > 0 {
            Self::draw_rect(
                canvas,
                sidebar_x,
                rects.terminal_btn.1.saturating_sub(10),
                sidebar_w,
                1,
                theme.chrome_line(),
            );
            self.draw_footer_btn(
                canvas,
                rects.terminal_btn,
                crate::ui::LucideIcon::Terminal,
                "Terminal",
                hover == SpacesHover::Terminal,
                theme,
            );
            self.draw_footer_btn(
                canvas,
                rects.machine_btn,
                crate::ui::LucideIcon::Plus,
                "Añadir máquina",
                hover == SpacesHover::Machine,
                theme,
            );
        }
    }

    /// Botón rectangular con contorno de 1 px para el pie del panel contextual (DISENO §4)
    pub(crate) fn draw_footer_btn(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        label: &str,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        let border = if hovered {
            theme.accent_color()
        } else {
            theme.card_outline()
        };
        let bg = if hovered {
            theme.surface_hover()
        } else {
            theme.bar_bg()
        };
        Self::draw_rect(canvas, x, y, w, h, bg);
        Self::draw_rect(canvas, x, y, w, 1, border);
        Self::draw_rect(canvas, x, y + h - 1, w, 1, border);
        Self::draw_rect(canvas, x, y, 1, h, border);
        Self::draw_rect(canvas, x + w - 1, y, 1, h, border);

        let fg = if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let label_w = self.measure_ui_text(label, 11.0, true);
        let total_content = 14 + 6 + label_w;
        let start_x = x + (w.saturating_sub(total_content)) / 2;
        self.draw_lucide_icon(canvas, icon, start_x + 7, y + h / 2, 13.0, fg);
        let base = self.centered_baseline(y + h / 2);
        self.draw_ui_text(canvas, label, start_x + 18, base, x + w - 4, fg, 11.0, true);
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
            Self::draw_rect(canvas, r.0, r.1, r.2, r.3, theme.surface_bg());
            Self::draw_rect(canvas, r.0, r.1, 2, r.3, theme.accent_color());
        } else if hovered {
            Self::draw_rect(canvas, r.0, r.1, r.2, r.3, theme.surface_hover());
        }
        let badge = SpacesPanelRects::BADGE;
        let bx = if collapsed {
            r.0 + (r.2 - badge) / 2
        } else {
            r.0 + 8
        };
        let by = r.1 + (r.3 - badge) / 2;
        Self::draw_rect(canvas, bx, by, badge, badge, theme.ansi[item.color_index()]);
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
}
