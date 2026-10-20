//! Barra lateral vertical de sesiones y botón flotante para mostrarla.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra lateral vertical: cabecera de marca, índice de sesiones y pie.
    #[allow(clippy::too_many_arguments)]
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
        let line = theme.chrome_line();

        // 1. Fondo y borde derecho
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            0,
            sidebar_width,
            usable_h,
            theme.chrome_bg(),
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            sidebar_width.saturating_sub(1),
            0,
            1,
            usable_h,
            line,
        );

        // 2. Cabecera: marca y nueva sesión
        let header_cy = 22;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            18,
            header_cy,
            15.0,
            theme.accent_color(),
        );
        self.draw_ui_text_tracked(
            buffer,
            width,
            height,
            "AXON",
            30,
            header_cy + 4,
            sidebar_width.saturating_sub(44),
            theme.foreground,
            10.0,
            true,
            1.5,
        );

        let plus_x = sidebar_width.saturating_sub(38);
        let plus_y = 8;
        let plus = (plus_x, plus_y, 28, 28);
        if hovered_new_tab {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                plus.0,
                plus.1,
                plus.2,
                plus.3,
                4,
                theme.surface_hover(),
            );
        }
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            plus.0 + 14,
            plus.1 + 14,
            14.0,
            if hovered_new_tab {
                theme.foreground
            } else {
                theme.text_secondary()
            },
        );
        Self::draw_rect(buffer, width, height, 0, 43, sidebar_width - 1, 1, line);

        // 3. Encabezado de la lista
        self.draw_ui_text_tracked(
            buffer,
            width,
            height,
            "SESIONES",
            12,
            58,
            sidebar_width.saturating_sub(28),
            theme.text_muted(),
            9.5,
            true,
            1.2,
        );
        let count = format!("{}", tabs.len());
        let count_w = self.measure_ui_text(&count, 9.5, true);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &count,
            sidebar_width.saturating_sub(12 + count_w),
            58,
            sidebar_width,
            theme.text_muted(),
            9.5,
            true,
        );

        // 4. Lista de sesiones
        let tab_item_h = SidebarRects::TAB_ITEM_H;
        let tab_spacing = SidebarRects::TAB_SPACING;
        let list_start_y = SidebarRects::LIST_START_Y;
        let toggle_h = SidebarRects::TOGGLE_H;
        let toggle_y = usable_h.saturating_sub(toggle_h + 8);
        let list_max_y = SidebarRects::list_max_y(usable_h);
        let tab_x = 8;
        let tab_w = sidebar_width.saturating_sub(16);

        for (vis_idx, (idx, tab)) in tabs.iter().enumerate().skip(scroll_offset).enumerate() {
            let tab_y = list_start_y + vis_idx * (tab_item_h + tab_spacing);
            if tab_y + tab_item_h > list_max_y {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x,
                    tab_y,
                    tab_w,
                    tab_item_h,
                    3,
                    theme.surface_bg(),
                );
                // Indicador de 1 px: el foco se ve sin convertir la fila en una tarjeta.
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    tab_x,
                    tab_y + 8,
                    1,
                    tab_item_h.saturating_sub(16),
                    theme.accent_color(),
                );
            } else if is_hovered {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x,
                    tab_y,
                    tab_w,
                    tab_item_h,
                    3,
                    theme.surface_hover(),
                );
            }

            let item_cy = tab_y + (tab_item_h / 2);
            let icon_color = if tab.is_active {
                theme.accent_color()
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                tab_icon(tab),
                tab_x + 18,
                item_cy,
                13.0,
                icon_color,
            );

            let show_close = tabs.len() > 1 && (tab.is_active || is_hovered);
            let title_x = tab_x + 32;
            let title_max = tab_x + tab_w - if show_close { 30 } else { 12 };
            let title_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let has_detail = tab.detail.is_some();
            let title_base = if has_detail { tab_y + 18 } else { item_cy + 4 };
            let title_end = self.draw_ui_text(
                buffer,
                width,
                height,
                &tab.title,
                title_x,
                title_base,
                title_max,
                title_color,
                11.5,
                tab.is_active,
            );
            if tab.is_preview {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    title_x,
                    title_base + 2,
                    title_end.saturating_sub(title_x),
                    1,
                    title_color,
                );
            }
            if let Some(detail) = &tab.detail {
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    detail,
                    title_x,
                    tab_y + 32,
                    title_max,
                    theme.text_muted(),
                    10.0,
                    false,
                );
            }

            if tab.has_unread && !tab.is_active && !show_close {
                let dot = theme.ansi[6];
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x + tab_w - 16,
                    item_cy - 2,
                    4,
                    4,
                    2,
                    dot,
                );
            }

            if show_close {
                let close_x = tab_x + tab_w - 26;
                let close_y = tab_y + 11;
                let is_close_hovered = hovered_close == Some(idx);
                if is_close_hovered {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        close_x,
                        close_y,
                        20,
                        20,
                        3,
                        theme.surface_hover(),
                    );
                }
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_x + 10,
                    close_y + 10,
                    11.0,
                    if is_close_hovered {
                        theme.ansi[1]
                    } else {
                        theme.text_muted()
                    },
                );
            }
        }

        // 5. Barra de desplazamiento de la lista
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

        // 6. Pie: ocultar barra lateral
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            toggle_y.saturating_sub(8),
            sidebar_width - 1,
            1,
            line,
        );
        let btn = (8, toggle_y, sidebar_width.saturating_sub(16), toggle_h);
        if hovered_toggle_btn {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                btn.0,
                btn.1,
                btn.2,
                btn.3,
                3,
                theme.surface_hover(),
            );
        }
        let fg = if hovered_toggle_btn {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let cy = toggle_y + (toggle_h / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            btn.0 + 10,
            cy,
            13.0,
            fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ocultar",
            btn.0 + 22,
            cy + 4,
            btn.0 + btn.2.saturating_sub(44),
            fg,
            11.0,
            false,
        );
        let shortcut = "Ctrl+B";
        let sw = self.measure_ui_text(shortcut, 9.5, false);
        self.draw_ui_text(
            buffer,
            width,
            height,
            shortcut,
            btn.0 + btn.2.saturating_sub(sw + 8),
            cy + 4,
            btn.0 + btn.2,
            theme.text_muted(),
            9.5,
            false,
        );
    }

    /// Botón flotante para recuperar la barra lateral cuando está oculta.
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
        let btn_w = 30;
        let btn_h = 30;
        let btn_x = 8;
        let btn_y = height.saturating_sub(btn_h + 8 + status_bar_h);
        let bg = if hovered_toggle {
            theme.surface_hover()
        } else {
            theme.chrome_bg_elevated()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, btn_x, btn_y, btn_w, btn_h, 4, bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            4,
            theme.chrome_line(),
        );
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            btn_x + (btn_w / 2),
            btn_y + (btn_h / 2),
            13.0,
            if hovered_toggle {
                theme.accent_color()
            } else {
                theme.text_secondary()
            },
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

        let track_w = 4;
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
