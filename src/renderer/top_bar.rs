//! Barra superior: pestañas en isla flotante, título y controles de ventana.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra horizontal de pestañas superiores con estilo de cápsula flotante (Floating Island)
    pub fn render_top_tab_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_files: bool,
        hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let rects = TopTabBarRects::compute(width, tabs.len());
        let bar_h = rects.bar_h;
        let bg = theme.background;
        let border_c = theme.border_color();

        // 1. Limpiar la franja superior con el fondo de la ventana
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, bg);

        // 2. Logo sutil a la izquierda: Icono Terminal / Axon
        let logo_center_x = 24;
        let logo_center_y = bar_h / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            logo_center_x,
            logo_center_y,
            15.0,
            theme.accent_color(),
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            logo_center_x + 12,
            logo_center_y + 4,
            rects.tabs_start_x.saturating_sub(8),
            theme.foreground,
            12.5,
            true,
        );

        // 3. Cápsula flotante central (Floating Island)
        let island_x = rects.tabs_start_x;
        let island_y = 4;
        let island_w = (rects.plus_btn.0 + rects.plus_btn.2 + 4).saturating_sub(island_x);
        let island_h = 30;

        // Fondo de la cápsula y borde sutil
        let island_bg = Color::from_rgba(
            theme.surface_bg().r(),
            theme.surface_bg().g(),
            theme.surface_bg().b(),
            200,
        );
        crate::ui::draw_rounded_rect(
            buffer, width, height, island_x, island_y, island_w, island_h, 14, island_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, island_x, island_y, island_w, island_h, 14, border_c,
        );

        // Renderizado de cada pestaña dentro de la cápsula
        for (idx, tab) in tabs.iter().enumerate() {
            let r = rects.tab_rect(idx);
            // Evitar desbordar la cápsula o pintar encima del botón '+'
            if r.0 + r.2 > rects.plus_btn.0 {
                break;
            }

            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                // Pestaña activa: cápsula interna iluminada y elevada acorde al tema
                let active_tab_bg = theme.surface_hover();
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 1,
                    r.1 + 2,
                    r.2.saturating_sub(2),
                    r.3.saturating_sub(4),
                    8,
                    active_tab_bg,
                );
            } else if is_hovered {
                let hover_c = theme.surface_hover();
                let hover_tab_bg = Color::from_rgba(hover_c.r(), hover_c.g(), hover_c.b(), 140);
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 1,
                    r.1 + 2,
                    r.2.saturating_sub(2),
                    r.3.saturating_sub(4),
                    8,
                    hover_tab_bg,
                );
            }

            // Icono representativo de la pestaña
            let icon = tab_icon(tab);

            let is_compact = rects.tab_w < 55;
            let icon_center_x = if is_compact {
                r.0 + (r.2 / 2)
            } else {
                r.0 + 16
            };
            let icon_center_y = r.1 + (r.3 / 2);
            let icon_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };
            let icon_sz = if rects.tab_w < 26 { 10.0 } else { 11.5 };

            self.draw_lucide_icon(
                buffer,
                width,
                height,
                icon,
                icon_center_x,
                icon_center_y,
                icon_sz,
                icon_color,
            );

            // Título de la pestaña con Inter solo si hay espacio suficiente
            if !is_compact {
                let text_color = if tab.is_active {
                    theme.foreground
                } else if is_hovered {
                    theme.ansi[7]
                } else {
                    theme.ansi[8]
                };

                let title_str = if tab.pane_count > 1 {
                    format!("{} [{}]", tab.title, tab.pane_count)
                } else {
                    tab.title.clone()
                };

                let text_x = icon_center_x + 12;
                let text_y = icon_center_y + 4;
                let text_max_x = r.0
                    + r.2.saturating_sub(if tabs.len() > 1 && rects.tab_w >= 48 {
                        24
                    } else {
                        6
                    });
                let title_end = self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &title_str,
                    text_x,
                    text_y,
                    text_max_x,
                    text_color,
                    11.5,
                    tab.is_active,
                );
                if tab.is_preview {
                    Self::draw_rect(
                        buffer,
                        width,
                        height,
                        text_x,
                        text_y + 3,
                        title_end.saturating_sub(text_x),
                        1,
                        text_color,
                    );
                }
            }

            // Botón de cerrar pestaña con Lucide 'X' (solo si la pestaña no es ultra compacta)
            if rects.tab_w >= 48 && tabs.len() > 1 && (tab.is_active || is_hovered) {
                let close_r = rects.close_btn_rect(idx);
                let is_close_hovered = hovered_close == Some(idx);
                let close_color = if is_close_hovered {
                    theme.ansi[1]
                } else {
                    theme.ansi[8]
                };

                let close_cx = close_r.0 + (close_r.2 / 2);
                let close_cy = close_r.1 + (close_r.3 / 2);
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_cx,
                    close_cy,
                    11.0,
                    close_color,
                );
            }
        }

        // 4. Botón '+' para nueva pestaña dentro de la cápsula
        let plus_r = rects.plus_btn;
        if hovered_new_tab {
            let plus_hover_bg = theme.surface_hover();
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                plus_r.0,
                plus_r.1 + 2,
                plus_r.2,
                plus_r.3.saturating_sub(4),
                8,
                plus_hover_bg,
            );
        }
        let plus_fg = if hovered_new_tab {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let plus_cx = plus_r.0 + (plus_r.2 / 2);
        let plus_cy = plus_r.1 + (plus_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            plus_cx,
            plus_cy,
            13.0,
            plus_fg,
        );

        // 5. Botón 'Explorador de archivos' a la derecha
        let files_r = rects.files_btn;
        let files_bg = if explorer_visible || hovered_files {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 8, files_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 8, border_c,
        );

        let files_fg = if explorer_visible || hovered_files {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let files_cx = files_r.0 + (files_r.2 / 2);
        let files_cy = files_r.1 + (files_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::FolderTree,
            files_cx,
            files_cy,
            13.5,
            files_fg,
        );

        // 6. Botón 'Ajustes' a la derecha
        let set_r = rects.settings_btn;
        let set_bg = if hovered_settings {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 8, set_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 8, border_c,
        );

        let set_fg = if hovered_settings {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let set_cx = set_r.0 + (set_r.2 / 2);
        let set_cy = set_r.1 + (set_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            set_cx,
            set_cy,
            13.5,
            set_fg,
        );

        // 7. Botones de ventana cliente (Minimizar, Maximizar, Cerrar)
        self.draw_window_controls(
            buffer,
            width,
            height,
            rects.win_min_btn,
            rects.win_max_btn,
            rects.win_close_btn,
            hovered_win_min,
            hovered_win_max,
            hovered_win_close,
            is_maximized,
            theme,
        );

        // 8. Línea divisoria inferior
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, border_c);
    }

    /// Dibuja los botones de control de ventana estándar (Minimizar, Maximizar, Cerrar)
    pub fn draw_window_controls(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        min_btn: (usize, usize, usize, usize),
        max_btn: (usize, usize, usize, usize),
        close_btn: (usize, usize, usize, usize),
        hovered_min: bool,
        hovered_max: bool,
        hovered_close: bool,
        is_maximized: bool,
        theme: &Theme,
    ) {
        // Minimizar
        if hovered_min {
            Self::draw_rect(
                buffer,
                width,
                height,
                min_btn.0,
                min_btn.1,
                min_btn.2,
                min_btn.3,
                theme.surface_hover(),
            );
        }
        let min_fg = if hovered_min {
            theme.foreground
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Minus,
            min_btn.0 + (min_btn.2 / 2),
            min_btn.1 + (min_btn.3 / 2),
            13.0,
            min_fg,
        );

        // Maximizar / Restaurar
        if hovered_max {
            Self::draw_rect(
                buffer,
                width,
                height,
                max_btn.0,
                max_btn.1,
                max_btn.2,
                max_btn.3,
                theme.surface_hover(),
            );
        }
        let max_fg = if hovered_max {
            theme.foreground
        } else {
            theme.ansi[7]
        };
        let max_icon = if is_maximized {
            crate::ui::LucideIcon::Copy
        } else {
            crate::ui::LucideIcon::Square
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            max_icon,
            max_btn.0 + (max_btn.2 / 2),
            max_btn.1 + (max_btn.3 / 2),
            12.0,
            max_fg,
        );

        // Cerrar (con hover rojo nativo Windows)
        if hovered_close {
            let close_bg = Color::from_rgb(224, 76, 76);
            Self::draw_rect(
                buffer,
                width,
                height,
                close_btn.0,
                close_btn.1,
                close_btn.2,
                close_btn.3,
                close_bg,
            );
        }
        let close_fg = if hovered_close {
            Color::from_rgb(255, 255, 255)
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::X,
            close_btn.0 + (close_btn.2 / 2),
            close_btn.1 + (close_btn.3 / 2),
            13.0,
            close_fg,
        );
    }

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar)
    pub fn render_top_title_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        title: &str,
        git_branch: Option<&str>,
        hovered_files: bool,
        hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let rects = TopTitleBarRects::compute(width);
        let bar_h = rects.bar_h;
        let bg = theme.background;
        let border_c = theme.border_color();

        // 1. Limpiar fondo de barra de título
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, bg);

        // 2. Logo sutil a la izquierda
        let logo_cx = 24;
        let logo_cy = bar_h / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            logo_cx,
            logo_cy,
            14.0,
            theme.accent_color(),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            logo_cx + 12,
            logo_cy + 4,
            120,
            theme.foreground,
            12.0,
            true,
        );

        // 3. Título centralizado
        let branch_suffix = if let Some(b) = git_branch {
            format!("  ⎇ {}", b)
        } else {
            String::new()
        };
        let center_title = format!("◈ {}{}", title, branch_suffix);
        let title_w = self.measure_ui_text(&center_title, 11.5, false);
        let center_x = if width > title_w + 300 {
            (width - title_w) / 2
        } else {
            140
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &center_title,
            center_x,
            logo_cy + 4,
            width.saturating_sub(210),
            theme.ansi[7],
            11.5,
            false,
        );

        // 4. Botón Explorador de archivos
        let files_r = rects.files_btn;
        let files_bg = if explorer_visible || hovered_files {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 6, files_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 6, border_c,
        );
        let files_fg = if explorer_visible || hovered_files {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::FolderTree,
            files_r.0 + (files_r.2 / 2),
            files_r.1 + (files_r.3 / 2),
            13.5,
            files_fg,
        );

        // 5. Botón Ajustes
        let set_r = rects.settings_btn;
        let set_bg = if hovered_settings {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 6, set_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 6, border_c,
        );
        let set_fg = if hovered_settings {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            set_r.0 + (set_r.2 / 2),
            set_r.1 + (set_r.3 / 2),
            13.0,
            set_fg,
        );

        // 5. Botones de ventana cliente
        self.draw_window_controls(
            buffer,
            width,
            height,
            rects.win_min_btn,
            rects.win_max_btn,
            rects.win_close_btn,
            hovered_win_min,
            hovered_win_max,
            hovered_win_close,
            is_maximized,
            theme,
        );

        // 6. Borde inferior
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, border_c);
    }
}
