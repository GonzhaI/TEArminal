//! Barra lateral vertical de sesiones y botón flotante para mostrarla.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra lateral: riel de proyectos a la izquierda y, a su lado, las sesiones del proyecto
    /// seleccionado. Dibuja en un búfer propio de ancho `sidebar_width`: `app` lo coloca a izquierda o derecha.
    #[allow(clippy::too_many_arguments)]
    pub fn render_sidebar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_width: usize,
        status_bar_h: usize,
        projects: &[SidebarProject],
        header: &SidebarHeader,
        hovered_project: Option<usize>,
        tabs: &[SidebarTabItem],
        scroll_offset: usize,
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_toggle_btn: bool,
        is_scrollbar_hovered: bool,
        theme: &Theme,
    ) {
        if sidebar_width == 0 || width == 0 || height == 0 {
            return;
        }

        let usable_h = height.saturating_sub(status_bar_h);
        let line = theme.chrome_line();
        let top = SidebarRects::TOP;
        let rail_w = SidebarRects::RAIL_W;
        let body_h = usable_h.saturating_sub(top);

        // 1. Riel (fondo del lienzo) y columna de sesiones (superficie de chrome).
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            top,
            rail_w,
            body_h,
            theme.background,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
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
        Self::draw_rect(buffer, width, height, edge_x, top, 1, body_h, line);

        // 2. Riel: un cuadrado por proyecto, sesiones sueltas si las hay y «Agregar proyecto».
        for (i, project) in projects.iter().enumerate() {
            self.draw_rail_project(
                buffer,
                width,
                height,
                i,
                project,
                hovered_project == Some(i),
                theme,
            );
        }
        let add = SidebarRects::rail_item_rect(projects.len());
        let add_hover = hovered_project == Some(projects.len());
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            add.0,
            add.1,
            add.2,
            add.3,
            12,
            if add_hover {
                theme.accent_color()
            } else {
                theme.chrome_line_strong()
            },
        );
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            add.0 + add.2 / 2,
            add.1 + add.3 / 2,
            15.0,
            if add_hover {
                theme.accent_color()
            } else {
                theme.text_muted()
            },
        );

        // 3. Cabecera del proyecto: nombre, rama y carpeta. El botón «+» abre una sesión nueva.
        let col_x = rail_w + 14;
        let plus = SidebarRects::plus_btn();
        let title_end = self.draw_ui_text(
            buffer,
            width,
            height,
            &header.title,
            col_x,
            top + 26,
            plus.0.saturating_sub(8),
            theme.foreground,
            tokens::SECTION + 1.5,
            true,
        );
        if let Some(branch) = &header.branch {
            let chip_x = title_end + 10;
            let text_w = self.measure_mono_text(branch, tokens::MICRO + 0.5, false);
            let chip_w = text_w + 28;
            if chip_x + chip_w < plus.0 {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    chip_x,
                    top + 12,
                    chip_w,
                    20,
                    6,
                    theme.surface_bg(),
                );
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::GitBranch,
                    chip_x + 11,
                    top + 22,
                    11.0,
                    theme.accent_color(),
                );
                self.draw_mono_text(
                    buffer,
                    width,
                    height,
                    branch,
                    chip_x + 22,
                    top + 26,
                    chip_x + chip_w,
                    theme.text_secondary(),
                    tokens::MICRO + 0.5,
                    false,
                );
            }
        }
        if let Some(path) = &header.path {
            self.draw_mono_text(
                buffer,
                width,
                height,
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
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                plus.0,
                plus.1,
                plus.2,
                plus.3,
                6,
                theme.surface_hover(),
            );
        }
        self.draw_lucide_icon(
            buffer,
            width,
            height,
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

        // 4. Lista de sesiones del proyecto.
        let tab_item_h = SidebarRects::TAB_ITEM_H;
        let list_max_y = header.list_end;
        let tab_x = SidebarRects::PAD_X;
        let tab_w = sidebar_width.saturating_sub(tab_x + 8);

        for (vis_idx, tab) in tabs.iter().skip(scroll_offset).enumerate() {
            let tab_y = SidebarRects::row_y(vis_idx);
            if tab_y + tab_item_h > list_max_y {
                break;
            }
            let idx = tab.index;
            let is_hovered = hovered_tab == Some(idx);
            let row = (tab_x, tab_y, tab_w, tab_item_h);

            if tab.is_active {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    row.0,
                    row.1,
                    row.2,
                    row.3,
                    10,
                    theme.background,
                );
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    row.0,
                    row.1,
                    row.2,
                    row.3,
                    10,
                    theme.chrome_line_strong(),
                );
            } else if is_hovered {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    row.0,
                    row.1,
                    row.2,
                    row.3,
                    10,
                    theme.surface_hover(),
                );
            }

            let item_cy = tab_y + tab_item_h / 2;
            let show_close = tabs.len() > 1 && is_hovered;
            let status_color = match tab.status {
                Some(SessionStatus::Running) => theme.accent_color(),
                Some(SessionStatus::Done) => theme.ansi[2],
                Some(SessionStatus::Failed) => theme.ansi[1],
                _ => theme.text_muted(),
            };

            // Indicador: punto de estado en terminales; icono en Ajustes y visores.
            if tab.status.is_some() {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x + 14,
                    tab_y + 13,
                    8,
                    8,
                    4,
                    status_color,
                );
            } else {
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    tab_icon(tab),
                    tab_x + 18,
                    tab_y + 17,
                    14.0,
                    if tab.is_active {
                        theme.accent_color()
                    } else {
                        theme.text_muted()
                    },
                );
            }

            // Estado a la derecha; al pasar el ratón lo sustituye el botón de cerrar.
            let right_edge = tab_x + tab_w - 12;
            let mut title_max = right_edge;
            if show_close {
                title_max = right_edge.saturating_sub(26);
            } else if let Some(status) = tab.status {
                let label = status.label();
                let w = self.measure_ui_text(label, tokens::DETAIL, false);
                let x = right_edge.saturating_sub(w);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    label,
                    x,
                    tab_y + 22,
                    right_edge,
                    status_color,
                    tokens::DETAIL,
                    false,
                );
                title_max = x.saturating_sub(10);
            }

            let title_x = tab_x + 32;
            let title_color = if tab.is_active || tab.has_unread {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let has_detail = tab.detail.is_some();
            let title_base = if has_detail { tab_y + 22 } else { item_cy + 4 };
            let title_end = self.draw_ui_text(
                buffer,
                width,
                height,
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
                self.draw_mono_text(
                    buffer,
                    width,
                    height,
                    detail,
                    title_x,
                    tab_y + 38,
                    tab_x + tab_w - 10,
                    theme.text_muted(),
                    tokens::MICRO + 0.5,
                    false,
                );
            }

            if show_close {
                let close = SidebarRects::close_btn_rect(row);
                let is_close_hovered = hovered_close == Some(idx);
                if is_close_hovered {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        close.0,
                        close.1,
                        close.2,
                        close.3,
                        6,
                        theme.surface_bg(),
                    );
                }
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
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

        // El explorador va alojado bajo la lista: una hairline lo separa de las sesiones.
        if header.explorer_visible {
            Self::draw_rect(
                buffer,
                width,
                height,
                rail_w,
                list_max_y + 3,
                sidebar_width.saturating_sub(rail_w + 1),
                1,
                line,
            );
        }

        // 6. Pie: ocultar la barra lateral, con atajo como dato.
        let btn = SidebarRects::toggle_btn_rect(usable_h);
        Self::draw_rect(
            buffer,
            width,
            height,
            rail_w,
            btn.1.saturating_sub(8),
            sidebar_width.saturating_sub(rail_w + 1),
            1,
            line,
        );
        if hovered_toggle_btn {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                btn.0,
                btn.1,
                btn.2,
                btn.3,
                6,
                theme.surface_hover(),
            );
        }
        let fg = if hovered_toggle_btn {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let cy = btn.1 + btn.3 / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            if header.on_right {
                crate::ui::LucideIcon::PanelRight
            } else {
                crate::ui::LucideIcon::PanelLeft
            },
            btn.0 + 12,
            cy,
            13.0,
            fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ocultar panel",
            btn.0 + 24,
            cy + 4,
            btn.0 + btn.2.saturating_sub(76),
            fg,
            tokens::BODY,
            false,
        );

        let b_w = self.measure_mono_text("B", tokens::MICRO, false) + 12;
        let ctrl_w = self.measure_mono_text("Ctrl", tokens::MICRO, false) + 12;
        let bx = btn.0 + btn.2 - b_w - 8;
        let cx = bx - ctrl_w - 4;
        self.draw_keycap(
            buffer,
            width,
            height,
            cx,
            cy - tokens::KEYCAP_H / 2,
            "Ctrl",
            theme,
        );
        self.draw_keycap(
            buffer,
            width,
            height,
            bx,
            cy - tokens::KEYCAP_H / 2,
            "B",
            theme,
        );
    }

    /// Cuadrado de proyecto del riel: color propio, inicial y marca lateral del activo.
    #[allow(clippy::too_many_arguments)]
    fn draw_rail_project(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        i: usize,
        project: &SidebarProject,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = SidebarRects::rail_item_rect(i);
        let color = theme.ansi[project.color_slot.min(15)];
        let fill = if project.is_loose {
            theme.surface_bg()
        } else if project.is_active {
            color
        } else {
            theme.mix(theme.background, color, if hovered { 0.8 } else { 0.55 })
        };
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 12, fill);
        if project.is_loose {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Terminal,
                x + w / 2,
                y + h / 2,
                17.0,
                if project.is_active {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
            );
        } else {
            let label = project.initial.to_string();
            let lw = self.measure_ui_text(&label, 15.0, true);
            self.draw_ui_text(
                buffer,
                width,
                height,
                &label,
                x + w.saturating_sub(lw) / 2,
                y + h / 2 + 5,
                x + w,
                theme.background,
                15.0,
                true,
            );
        }
        if project.is_active {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                0,
                y + 10,
                3,
                h - 20,
                1,
                theme.foreground,
            );
        }
        if project.has_news {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                x + w - 9,
                y - 3,
                12,
                12,
                6,
                theme.background,
            );
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                x + w - 7,
                y - 1,
                8,
                8,
                4,
                theme.ansi[3],
            );
        }
    }

    /// Botón flotante para recuperar la barra lateral cuando está oculta.
    pub fn render_floating_sidebar_toggle(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        status_bar_h: usize,
        hovered_toggle: bool,
        on_right: bool,
        theme: &Theme,
    ) {
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

    pub fn sidebar_scrollbar_geometry(
        sidebar_width: usize,
        list_end: usize,
        total_tabs: usize,
        scroll_offset: usize,
    ) -> Option<(Rect, Rect)> {
        let list_start_y = SidebarRects::LIST_START_Y;
        let list_max_y = list_end;
        if list_max_y <= list_start_y {
            return None;
        }
        let visible_items = SidebarRects::visible_items(list_end);

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
