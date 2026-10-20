//! Panel lateral del modo de pestañas superiores: la lista de proyectos y, debajo, el explorador.

use super::*;

impl TerminalRenderer {
    /// Dibuja el panel de proyectos en un búfer propio de ancho `panel_w` (como `render_sidebar`).
    /// `entries` son los proyectos; tras ellos va la fila «Agregar proyecto».
    #[allow(clippy::too_many_arguments)]
    pub fn render_projects_panel(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        panel_w: usize,
        status_bar_h: usize,
        entries: &[SidebarProject],
        header: &SidebarHeader,
        hovered_row: Option<usize>,
        scroll_offset: usize,
        hovered_toggle_btn: bool,
        theme: &Theme,
    ) {
        if panel_w == 0 || width == 0 || height == 0 {
            return;
        }
        let usable_h = height.saturating_sub(status_bar_h);
        let top = SidebarRects::TOP;
        let line = theme.chrome_line();
        let body_h = usable_h.saturating_sub(top);

        // 1. Fondo y borde del lado del contenido.
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            top,
            panel_w,
            body_h,
            theme.chrome_bg(),
        );
        let edge_x = if header.on_right { 0 } else { panel_w - 1 };
        Self::draw_rect(buffer, width, height, edge_x, top, 1, body_h, line);

        // 2. Cabecera: título y número de proyectos.
        let title_end = self.draw_ui_text(
            buffer,
            width,
            height,
            "Proyectos",
            16,
            top + 25,
            panel_w.saturating_sub(40),
            theme.foreground,
            tokens::SECTION,
            true,
        );
        let count = entries.iter().filter(|p| !p.is_loose).count().to_string();
        self.draw_mono_text(
            buffer,
            width,
            height,
            &count,
            title_end + 8,
            top + 25,
            panel_w.saturating_sub(24),
            theme.text_muted(),
            tokens::MICRO + 0.5,
            false,
        );

        // 3. Filas: cada proyecto y, al final, «Agregar proyecto».
        let total = entries.len() + 1;
        let visible = ProjectsPanelRects::visible_rows(usable_h);
        for (slot, i) in (scroll_offset..total).take(visible).enumerate() {
            let (rx, ry, rw, rh) = ProjectsPanelRects::row_rect(slot);
            let hovered = hovered_row == Some(i);
            let Some(project) = entries.get(i) else {
                self.draw_add_project_row((rx, ry, rw, rh), hovered, buffer, width, height, theme);
                continue;
            };
            if project.is_active {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    rx,
                    ry,
                    rw,
                    rh,
                    10,
                    theme.background,
                );
            } else if hovered {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    rx,
                    ry,
                    rw,
                    rh,
                    10,
                    theme.surface_hover(),
                );
            }

            // Monograma de color (o terminal para las sesiones sueltas)
            let sq = (rx + 10, ry + (rh - 22) / 2, 22, 22);
            if project.is_loose {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    sq.0,
                    sq.1,
                    sq.2,
                    sq.3,
                    7,
                    theme.chrome_line_strong(),
                );
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::Terminal,
                    sq.0 + 11,
                    sq.1 + 11,
                    13.0,
                    theme.foreground,
                );
            } else {
                let color = theme.ansi[project.color_slot.min(15)];
                crate::ui::draw_rounded_rect(
                    buffer, width, height, sq.0, sq.1, sq.2, sq.3, 7, color,
                );
                let label = project.initial.to_string();
                let lw = self.measure_ui_text(&label, 12.0, true);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &label,
                    sq.0 + sq.2.saturating_sub(lw) / 2,
                    sq.1 + 16,
                    sq.0 + sq.2,
                    theme.background,
                    12.0,
                    true,
                );
            }
            if project.has_news {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    sq.0 + 16,
                    sq.1 - 4,
                    11,
                    11,
                    5,
                    theme.chrome_bg(),
                );
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    sq.0 + 18,
                    sq.1 - 2,
                    7,
                    7,
                    3,
                    theme.ansi[3],
                );
            }

            // Estado a la derecha; nombre y rama a la izquierda.
            let right_edge = rx + rw - 12;
            let mut name_max = right_edge;
            if let Some(status) = project.status {
                let label = status.label();
                let w = self.measure_ui_text(label, tokens::DETAIL, false);
                let x = right_edge.saturating_sub(w);
                let color = match status {
                    SessionStatus::Running => theme.accent_color(),
                    SessionStatus::Done => theme.ansi[2],
                    SessionStatus::Failed => theme.ansi[1],
                    SessionStatus::Idle => theme.text_muted(),
                };
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    label,
                    x,
                    ry + 20,
                    right_edge,
                    color,
                    tokens::DETAIL,
                    false,
                );
                name_max = x.saturating_sub(10);
            }
            let text_x = sq.0 + sq.2 + 12;
            self.draw_ui_text(
                buffer,
                width,
                height,
                &project.name,
                text_x,
                ry + 20,
                name_max,
                if project.is_active || hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
                tokens::TITLE,
                true,
            );
            if let Some(branch) = &project.branch {
                self.draw_mono_text(
                    buffer,
                    width,
                    height,
                    branch,
                    text_x,
                    ry + 36,
                    right_edge,
                    theme.text_muted(),
                    tokens::MICRO + 0.5,
                    false,
                );
            }
        }

        // 4. El explorador va alojado bajo la lista: una hairline lo separa de los proyectos.
        if header.explorer_visible {
            Self::draw_rect(
                buffer,
                width,
                height,
                8,
                header.list_end + 3,
                panel_w.saturating_sub(17),
                1,
                line,
            );
        }

        // 5. Pie: ocultar el panel, con atajo como dato.
        let btn = ProjectsPanelRects::toggle_btn_rect(usable_h);
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            btn.1.saturating_sub(8),
            panel_w.saturating_sub(1),
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

    /// Última fila de la lista: cuadrado discontinuo con «+» y el texto «Agregar proyecto».
    fn draw_add_project_row(
        &mut self,
        row: Rect,
        hovered: bool,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        theme: &Theme,
    ) {
        let (rx, ry, rw, rh) = row;
        if hovered {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                rx,
                ry,
                rw,
                rh,
                10,
                theme.surface_hover(),
            );
        }
        let sq = (rx + 10, ry + (rh - 22) / 2, 22, 22);
        let color = if hovered {
            theme.accent_color()
        } else {
            theme.chrome_line_strong()
        };
        crate::ui::stroke_rounded_rect(buffer, width, height, sq.0, sq.1, sq.2, sq.3, 7, color);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            sq.0 + 11,
            sq.1 + 11,
            13.0,
            if hovered {
                theme.accent_color()
            } else {
                theme.text_muted()
            },
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Agregar proyecto",
            sq.0 + sq.2 + 12,
            ry + rh / 2 + 4,
            rx + rw - 8,
            if hovered {
                theme.foreground
            } else {
                theme.text_muted()
            },
            tokens::TITLE,
            false,
        );
    }
}
