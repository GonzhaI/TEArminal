//! Barra lateral vertical de sesiones y botón flotante para mostrarla.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra lateral: riel de ajustes a la izquierda y, a su lado, la lista de sesiones.
    /// Dibuja en un búfer propio de ancho `sidebar_width`: `app` lo coloca a izquierda o derecha.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_sidebar(
        &mut self,
        canvas: &mut Canvas,
        sidebar_width: usize,
        status_bar_h: usize,
        header: &SidebarHeader,
        tabs: &[SidebarTabItem],
        scroll_offset: usize,
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_toggle_btn: bool,
        is_scrollbar_hovered: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if sidebar_width == 0 || width == 0 || height == 0 {
            return;
        }

        let usable_h = height.saturating_sub(status_bar_h);
        let line = theme.chrome_line();
        let top = SidebarRects::TOP;
        let rail_w = SidebarRects::RAIL_W;
        let body_h = usable_h.saturating_sub(top);

        // 1. Riel (fondo del lienzo) y columna de sesiones (superficie de chrome).
        Self::draw_rect(canvas, 0, top, rail_w, body_h, theme.background);
        Self::draw_rect(
            canvas,
            rail_w,
            top,
            sidebar_width.saturating_sub(rail_w),
            body_h,
            theme.chrome_bg(),
        );
        let edge_x = if header.on_right {
            0
        } else {
            sidebar_width - 1
        };
        Self::draw_rect(canvas, edge_x, top, 1, body_h, line);

        // 2. Cabecera: título, rama y carpeta de la sesión a la vista. El botón «+» abre una sesión nueva.
        let col_x = rail_w + 14;
        let plus = SidebarRects::plus_btn_with_width(sidebar_width);
        // La rama va como píldora alineada a la derecha, junto al «+».
        let chip = header.branch.as_ref().map(|branch| {
            let text_w = self.measure_mono_text(branch, tokens::MICRO + 0.5, false);
            let chip_w = text_w + 30;
            let x = plus.0.saturating_sub(8).saturating_sub(chip_w);
            (branch.clone(), x, chip_w)
        });
        let title_max = chip
            .as_ref()
            .map_or(plus.0.saturating_sub(8), |(_, x, _)| x.saturating_sub(8));
        let header_cy = top + 22;
        let header_base = self.centered_baseline(header_cy);
        let _ = self.draw_ui_text(
            canvas,
            &header.title,
            col_x,
            header_base,
            title_max,
            theme.foreground,
            tokens::SECTION + 1.5,
            true,
        );
        if let Some((branch, chip_x, chip_w)) = &chip {
            canvas.rounded_fill(
                *chip_x,
                top + 11,
                *chip_w,
                22,
                tokens::RADIUS_CONTROL,
                theme.surface_bg(),
            );
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::GitBranch,
                chip_x + 12,
                header_cy,
                11.0,
                theme.accent_color(),
            );
            self.draw_mono_text(
                canvas,
                branch,
                chip_x + 23,
                header_base,
                chip_x + chip_w - 4,
                theme.text_secondary(),
                tokens::MICRO + 0.5,
                false,
            );
        }
        if let Some(path) = &header.path {
            self.draw_mono_text(
                canvas,
                path,
                col_x,
                top + 49,
                sidebar_width.saturating_sub(12),
                theme.text_muted(),
                tokens::MICRO + 0.5,
                false,
            );
        }
        if hovered_new_tab {
            canvas.rounded_fill(
                plus.0,
                plus.1,
                plus.2,
                plus.3,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Plus,
            plus.0 + plus.2 / 2,
            plus.1 + plus.3 / 2,
            14.0,
            if hovered_new_tab {
                theme.foreground
            } else {
                theme.text_secondary()
            },
        );

        // 3. Lista de sesiones.
        let tab_item_h = SidebarRects::TAB_ITEM_H;
        let list_max_y = header.list_end;
        let tab_x = SidebarRects::PAD_X;
        let tab_w = sidebar_width.saturating_sub(tab_x + SidebarRects::ROW_PAD_R);

        for (vis_idx, tab) in tabs.iter().skip(scroll_offset).enumerate() {
            let tab_y = SidebarRects::row_y(vis_idx);
            if tab_y + tab_item_h > list_max_y {
                break;
            }
            let idx = tab.index;
            let is_hovered = hovered_tab == Some(idx);
            let row = (tab_x, tab_y, tab_w, tab_item_h);

            if tab.is_active {
                let radius = tokens::RADIUS_ROW;
                canvas.rounded_fill(row.0, row.1, row.2, row.3, radius, theme.background);
                canvas.rounded_stroke(
                    row.0,
                    row.1,
                    row.2,
                    row.3,
                    radius,
                    theme.chrome_line_strong(),
                );
            } else if is_hovered {
                canvas.rounded_fill(
                    row.0,
                    row.1,
                    row.2,
                    row.3,
                    tokens::RADIUS_ROW,
                    theme.surface_hover(),
                );
            }

            let item_cy = tab_y + tab_item_h / 2;
            let show_close = tabs.len() > 1 && is_hovered;

            let maybe_icon = tab_icon(tab);
            let has_leading = maybe_icon.is_some();
            let has_detail = tab.detail.is_some();

            if let Some(icon) = maybe_icon {
                let icon_cy = if has_detail { tab_y + 16 } else { item_cy };
                self.draw_lucide_icon(
                    canvas,
                    icon,
                    tab_x + 18,
                    icon_cy,
                    14.0,
                    if tab.is_active {
                        theme.accent_color()
                    } else {
                        theme.text_muted()
                    },
                );
            }

            // Al pasar el ratón se reserva espacio para el botón de cerrar.
            let right_edge = tab_x + tab_w - 12;
            let title_max = if show_close {
                right_edge.saturating_sub(26)
            } else {
                right_edge
            };

            let title_x = tab_x + if has_leading { 32 } else { 16 };
            let title_color = if tab.is_active || tab.has_unread {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let title_base = if has_detail {
                self.centered_baseline(tab_y + 16)
            } else {
                self.centered_baseline(item_cy)
            };
            let title_end = self.draw_ui_text(
                canvas,
                &tab.title,
                title_x,
                title_base,
                title_max,
                title_color,
                tokens::TITLE,
                tab.is_active,
            );
            if tab.is_preview {
                Self::draw_rect(
                    canvas,
                    title_x,
                    title_base + 2,
                    title_end.saturating_sub(title_x),
                    1,
                    title_color,
                );
            }
            if let Some(detail) = &tab.detail {
                self.draw_ui_text(
                    canvas,
                    detail,
                    title_x,
                    self.centered_baseline(tab_y + 34),
                    tab_x + tab_w - 10,
                    theme.text_muted(),
                    tokens::BODY,
                    false,
                );
            }

            if show_close {
                let close = SidebarRects::close_btn_rect(row);
                let is_close_hovered = hovered_close == Some(idx);
                if is_close_hovered {
                    canvas.rounded_fill(
                        close.0,
                        close.1,
                        close.2,
                        close.3,
                        tokens::RADIUS_ROW,
                        theme.surface_bg(),
                    );
                }
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::X,
                    close.0 + close.2 / 2,
                    close.1 + close.3 / 2,
                    11.5,
                    if is_close_hovered {
                        theme.ansi[1]
                    } else {
                        theme.text_muted()
                    },
                );
            }
        }

        // 5. Barra de desplazamiento de la lista
        if let Some((track, thumb)) = Self::sidebar_scrollbar_geometry(
            sidebar_width,
            header.list_end,
            tabs.len(),
            scroll_offset,
        ) {
            Self::draw_scrollbar_rects(
                canvas,
                track,
                thumb,
                is_scrollbar_hovered,
                self.scrollbars_awake,
                theme,
            );
        }

        // 4. Ajustes al fondo del riel (el pie «Ocultar panel» ya no existe; el panel se alterna con Ctrl B).
        let btn = SidebarRects::settings_btn_rect(usable_h);
        if hovered_toggle_btn {
            canvas.rounded_fill(
                btn.0,
                btn.1,
                btn.2,
                btn.3,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Sliders,
            btn.0 + btn.2 / 2,
            btn.1 + btn.3 / 2,
            16.0,
            if hovered_toggle_btn {
                theme.foreground
            } else {
                theme.text_secondary()
            },
        );
    }

    /// Resalte de una fila de lista del panel (Archivos y Cambios): inset de 8 px, esquinas redondeadas
    /// y, si está seleccionada, una marca de acento (gris cuando la lista no tiene el foco).
    pub(crate) fn draw_list_row_highlight(
        canvas: &mut Canvas,
        row: Rect,
        selected: bool,
        focused: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = row;
        let fill = if selected && !focused {
            theme.surface_bg()
        } else {
            theme.surface_hover()
        };
        let row_w = w.saturating_sub(16);
        canvas.rounded_fill(x + 8, y, row_w, h, tokens::RADIUS_ROW, fill);
        if selected && focused {
            canvas.rounded_stroke(x + 8, y, row_w, h, tokens::RADIUS_ROW, theme.card_outline());
        }
        if selected {
            let bar = if focused {
                theme.accent_color()
            } else {
                theme.chrome_line_strong()
            };
            canvas.rounded_fill(x + 12, y + 6, 2, h.saturating_sub(12), 1, bar);
        }
    }

    /// Botón flotante para recuperar la barra lateral cuando está oculta.
    pub(crate) fn render_floating_sidebar_toggle(
        &mut self,
        canvas: &mut Canvas,
        status_bar_h: usize,
        hovered_toggle: bool,
        on_right: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }
        let btn_w = 30;
        let btn_h = 30;
        let btn_x = if on_right {
            width.saturating_sub(38)
        } else {
            8
        };
        let btn_y = height.saturating_sub(btn_h + 8 + status_bar_h);
        let bg = if hovered_toggle {
            theme.surface_hover()
        } else {
            theme.chrome_bg_elevated()
        };
        canvas.rounded_fill(btn_x, btn_y, btn_w, btn_h, tokens::RADIUS_CONTROL, bg);
        canvas.rounded_stroke(
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            tokens::RADIUS_CONTROL,
            theme.chrome_line(),
        );
        self.draw_lucide_icon(
            canvas,
            if on_right {
                crate::ui::LucideIcon::PanelRight
            } else {
                crate::ui::LucideIcon::PanelLeft
            },
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

    pub(crate) fn sidebar_scrollbar_geometry(
        sidebar_width: usize,
        list_end: usize,
        total_tabs: usize,
        scroll_offset: usize,
    ) -> Option<(Rect, Rect)> {
        let list_start_y = SidebarRects::LIST_START_Y;
        ScrollbarRects::vertical(
            sidebar_width,
            list_start_y,
            list_end.saturating_sub(list_start_y),
            total_tabs,
            SidebarRects::visible_items(list_end),
            scroll_offset,
        )
    }
}
