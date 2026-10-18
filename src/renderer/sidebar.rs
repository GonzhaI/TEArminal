//! Barra lateral vertical de pestañas y botón flotante para mostrarla.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra lateral vertical con la lista de instancias / pestañas al estilo Ghostty
    pub fn render_sidebar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_width: usize,
        status_bar_h: usize,
        tabs: &[SidebarTabItem],
        scroll_offset: usize,
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        _hovered_settings_btn: bool,
        hovered_toggle_btn: bool,
        is_scrollbar_hovered: bool,
        theme: &Theme,
    ) {
        if sidebar_width == 0 || width == 0 || height == 0 {
            return;
        }

        let usable_h = height.saturating_sub(status_bar_h);

        // 1. Fondo de la barra lateral dinámico acorde al tema (por encima de la barra inferior)
        let sidebar_bg = theme.sidebar_bg();
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            0,
            sidebar_width,
            usable_h,
            sidebar_bg,
        );

        // 2. Línea divisoria vertical sutil
        let divider_color = theme.border_color();
        Self::draw_rect(
            buffer,
            width,
            height,
            sidebar_width.saturating_sub(1),
            0,
            1,
            usable_h,
            divider_color,
        );

        // 3. Encabezado de la barra lateral (Header: Axon + Botón Settings + Botón New Tab)
        let header_cy = 20;
        let header_base = header_cy + 4;
        let logo_color = theme.accent_color();

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            20,
            header_cy,
            14.5,
            logo_color,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            32,
            header_base,
            sidebar_width.saturating_sub(42),
            theme.foreground,
            12.5,
            true,
        );

        // Botón "+" para crear nueva terminal
        let plus_btn_x = sidebar_width.saturating_sub(34);
        let plus_btn_y = 8;
        let plus_btn_w = 24;
        let plus_btn_h = 24;
        let plus_bg = if hovered_new_tab {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, plus_btn_x, plus_btn_y, plus_btn_w, plus_btn_h, 6, plus_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            plus_btn_x,
            plus_btn_y,
            plus_btn_w,
            plus_btn_h,
            6,
            theme.border_color(),
        );

        let plus_fg = if hovered_new_tab {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            plus_btn_x + (plus_btn_w / 2),
            plus_btn_y + (plus_btn_h / 2),
            12.0,
            plus_fg,
        );

        // 4. Lista de pestañas verticales
        let tab_item_h = SidebarRects::TAB_ITEM_H;
        let tab_spacing = SidebarRects::TAB_SPACING;
        let list_start_y = SidebarRects::LIST_START_Y;
        let toggle_h = SidebarRects::TOGGLE_H;
        let toggle_y = usable_h.saturating_sub(toggle_h + 8);
        let list_max_y = SidebarRects::list_max_y(usable_h);

        for (vis_idx, (idx, tab)) in tabs.iter().enumerate().skip(scroll_offset).enumerate() {
            let tab_y = list_start_y + vis_idx * (tab_item_h + tab_spacing);
            if tab_y + tab_item_h > list_max_y {
                break;
            }

            let is_hovered = hovered_tab == Some(idx);
            let tab_w = sidebar_width.saturating_sub(16);
            let tab_x = 8;

            // Fondo del ítem de pestaña con elevación sutil de tarjeta y esquinas redondeadas
            let item_bg = if tab.is_active {
                theme.surface_bg()
            } else if is_hovered {
                theme.surface_hover()
            } else {
                sidebar_bg
            };

            crate::ui::draw_rounded_rect(
                buffer, width, height, tab_x, tab_y, tab_w, tab_item_h, 8, item_bg,
            );

            // Borde sutil si está activo o en hover
            if tab.is_active {
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x,
                    tab_y,
                    tab_w,
                    tab_item_h,
                    8,
                    theme.border_color(),
                );
            }

            // Indicador vertical de acento en la pestaña activa
            if tab.is_active {
                let accent_color = theme.accent_color();
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x + 2,
                    tab_y + 8,
                    3,
                    tab_item_h.saturating_sub(16),
                    2,
                    accent_color,
                );
            }

            // Icono representativo de la pestaña
            let icon = tab_icon(tab);

            let item_cy = tab_y + (tab_item_h / 2);
            let icon_cx = tab_x + 18;
            let icon_color = if tab.is_active {
                theme.accent_color()
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };
            self.draw_lucide_icon(
                buffer, width, height, icon, icon_cx, item_cy, 11.5, icon_color,
            );

            // Título de la pestaña en Inter
            let text_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };

            let display_title = if tab.pane_count > 1 {
                format!("{} [{}]", tab.title, tab.pane_count)
            } else {
                tab.title.clone()
            };

            let title_x = icon_cx + 12;
            let title_base = item_cy + 4;
            let title_max_x = tab_x + tab_w.saturating_sub(26);
            let title_end = self.draw_ui_text(
                buffer,
                width,
                height,
                &display_title,
                title_x,
                title_base,
                title_max_x,
                text_color,
                11.5,
                tab.is_active,
            );
            if tab.is_preview {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    title_x,
                    title_base + 3,
                    title_end.saturating_sub(title_x),
                    1,
                    text_color,
                );
            }

            // Indicador de actividad / no leído si la pestaña tiene salida pendiente en segundo plano
            if tab.has_unread && !tab.is_active {
                let dot_x = tab_x + tab_w.saturating_sub(32);
                let dot_y = tab_y + (tab_item_h / 2).saturating_sub(3);
                let dot_color = theme.ansi[4];
                crate::ui::draw_rounded_rect(
                    buffer, width, height, dot_x, dot_y, 6, 6, 3, dot_color,
                );
            }

            // Botón cerrar "x" con Lucide X
            if tabs.len() > 1 && (tab.is_active || is_hovered) {
                let close_x = tab_x + tab_w.saturating_sub(22);
                let close_y = tab_y + 8;
                let close_size = 20;

                let is_close_hovered = hovered_close == Some(idx);
                let close_fg = if is_close_hovered {
                    theme.ansi[1]
                } else {
                    theme.ansi[8]
                };

                let close_cx = close_x + (close_size / 2);
                let close_cy = close_y + (close_size / 2) - 1;
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_cx,
                    close_cy,
                    11.0,
                    close_fg,
                );
            }
        }

        // Barra de desplazamiento de pestañas si la lista excede el espacio visible
        if let Some((track, thumb)) =
            Self::sidebar_scrollbar_geometry(sidebar_width, usable_h, tabs.len(), scroll_offset)
        {
            Self::draw_scrollbar_rects(
                buffer,
                width,
                height,
                track,
                thumb,
                is_scrollbar_hovered,
                self.scrollbars_awake,
                theme,
            );
        }

        // 5. Botón inferior para alternar/ocultar la barra lateral ("< Ocultar")
        let sidebar_btn_x = 8;
        let sidebar_btn_w = sidebar_width.saturating_sub(16);

        // Separador superior sutil
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            toggle_y.saturating_sub(6),
            sidebar_width,
            1,
            divider_color,
        );

        let toggle_bg = if hovered_toggle_btn {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            sidebar_btn_x,
            toggle_y,
            sidebar_btn_w,
            toggle_h,
            6,
            toggle_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            sidebar_btn_x,
            toggle_y,
            sidebar_btn_w,
            toggle_h,
            6,
            divider_color,
        );

        let toggle_fg = if hovered_toggle_btn {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        let toggle_cy = toggle_y + (toggle_h / 2);
        let toggle_base = toggle_cy + 4;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            sidebar_btn_x + 14,
            toggle_cy,
            12.0,
            toggle_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ocultar",
            sidebar_btn_x + 26,
            toggle_base,
            sidebar_btn_x + sidebar_btn_w.saturating_sub(54),
            toggle_fg,
            11.5,
            hovered_toggle_btn,
        );

        // Indicador de atajo Ctrl+B a la derecha en Inter
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ctrl+B",
            sidebar_btn_x + sidebar_btn_w.saturating_sub(44),
            toggle_base,
            sidebar_btn_x + sidebar_btn_w - 4,
            theme.ansi[8],
            10.5,
            false,
        );
    }

    /// Renderiza un botón flotante sutil en la esquina inferior izquierda para volver a desplegar la barra lateral (los Ajustes están solo en la barra superior)
    pub fn render_floating_sidebar_toggle(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        status_bar_h: usize,
        hovered_toggle: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let btn_w = 32;
        let btn_h = 30;
        let btn_x = 8;
        let btn_y = height.saturating_sub(btn_h + 8 + status_bar_h);
        let border_color = theme.border_color();

        // Botón 1: Desplegar barra lateral [ PanelLeft ]
        let bg_color = if hovered_toggle {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, btn_x, btn_y, btn_w, btn_h, 6, bg_color,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            6,
            border_color,
        );

        let icon_fg = if hovered_toggle {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            btn_x + (btn_w / 2),
            btn_y + (btn_h / 2),
            13.0,
            icon_fg,
        );
    }

    pub fn sidebar_scrollbar_geometry(
        sidebar_width: usize,
        usable_h: usize,
        total_tabs: usize,
        scroll_offset: usize,
    ) -> Option<(Rect, Rect)> {
        let list_start_y = SidebarRects::LIST_START_Y;
        let list_max_y = SidebarRects::list_max_y(usable_h);
        if list_max_y <= list_start_y {
            return None;
        }
        let visible_items = SidebarRects::visible_items(usable_h);

        if total_tabs <= visible_items || visible_items == 0 {
            return None;
        }

        let track_w = 6;
        let track_x = sidebar_width.saturating_sub(track_w + 3);
        let track_y = list_start_y;
        let track_h = list_max_y.saturating_sub(list_start_y);

        let thumb_h = (((visible_items as f32 / total_tabs as f32) * track_h as f32).max(18.0)
            as usize)
            .min(track_h);
        let max_offset = total_tabs.saturating_sub(visible_items);
        let travel = track_h.saturating_sub(thumb_h);
        let thumb_y = if max_offset > 0 {
            track_y + ((scroll_offset as f32 / max_offset as f32) * travel as f32) as usize
        } else {
            track_y
        };

        Some((
            (track_x, track_y, track_w, track_h),
            (track_x, thumb_y, track_w, thumb_h),
        ))
    }
}
