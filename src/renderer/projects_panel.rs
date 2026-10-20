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
        _hovered_toggle_btn: bool,
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

        // 2. Cabecera: icono, título y contador de proyectos en una píldora.
        let header_cy = top + ProjectsPanelRects::HEADER_H / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Folder,
            24,
            header_cy,
            14.0,
            theme.text_secondary(),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Proyectos",
            38,
            header_cy + 4,
            panel_w.saturating_sub(70),
            theme.foreground,
            tokens::SECTION,
            true,
        );
        let count = entries.iter().filter(|p| !p.is_loose).count().to_string();
        let cw = self.measure_mono_text(&count, tokens::MICRO, false);
        let count_w = (cw + 16).max(22);
        let badge_x = panel_w.saturating_sub(count_w + SidebarRects::PAD_R);
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            badge_x,
            header_cy.saturating_sub(10),
            count_w,
            20,
            10,
            theme.background,
        );
        self.draw_mono_text(
            buffer,
            width,
            height,
            &count,
            badge_x + (count_w.saturating_sub(cw)) / 2,
            header_cy + 5,
            badge_x + count_w,
            theme.text_secondary(),
            tokens::MICRO,
            false,
        );

        // 3. Filas: cada proyecto y, al final, «Agregar proyecto».
        let total = entries.len() + usize::from(header.suggestion.is_some()) + 1;
        let visible = ProjectsPanelRects::visible_rows(usable_h);
        for (slot, i) in (scroll_offset..total).take(visible).enumerate() {
            let (rx, ry, rw, rh) = ProjectsPanelRects::row_rect(slot);
            let hovered = hovered_row == Some(i);
            let Some(project) = entries.get(i) else {
                // Tras los proyectos: la sugerencia (si hay) y, al final, «Agregar proyecto».
                let suggestion = header.suggestion.as_deref().filter(|_| i == entries.len());
                self.draw_add_project_row(
                    (rx, ry, rw, rh),
                    hovered,
                    suggestion,
                    buffer,
                    width,
                    height,
                    theme,
                );
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

            // Estado a la derecha (anillo en curso, punto si terminó/falló/en espera); nombre y rama a la izquierda.
            let right_edge = rx + rw - 12;
            let mut name_max = right_edge;
            if let Some(status) = project.status {
                let label = status.label();
                let color = match status {
                    SessionStatus::Running => theme.accent_color(),
                    SessionStatus::Done => theme.ansi[2],
                    SessionStatus::Failed => theme.ansi[1],
                    SessionStatus::Idle => theme.text_muted(),
                };
                let text_w = self.measure_ui_text(label, tokens::DETAIL, false);
                let group_x = right_edge.saturating_sub(text_w + 14);
                if status == SessionStatus::Running {
                    crate::ui::stroke_rounded_rect(
                        buffer,
                        width,
                        height,
                        group_x,
                        ry + 12,
                        10,
                        10,
                        5,
                        color,
                    );
                } else {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        group_x + 1,
                        ry + 13,
                        8,
                        8,
                        4,
                        color,
                    );
                }
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    label,
                    group_x + 14,
                    ry + 20,
                    right_edge,
                    color,
                    tokens::DETAIL,
                    false,
                );
                name_max = group_x.saturating_sub(10);
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
    }

    /// Fila de agregar: cuadrado con «+» y el texto «Agregar proyecto». Con `suggestion` (el repositorio
    /// de la sesión a la vista) es la oferta «Agregar "repo"», resaltada con el acento.
    #[allow(clippy::too_many_arguments)]
    fn draw_add_project_row(
        &mut self,
        row: Rect,
        hovered: bool,
        suggestion: Option<&str>,
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
        let color = if hovered || suggestion.is_some() {
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
            if hovered || suggestion.is_some() {
                theme.accent_color()
            } else {
                theme.text_muted()
            },
        );
        let text_x = sq.0 + sq.2 + 12;
        let label = match suggestion {
            Some(name) => format!("Agregar «{name}»"),
            None => "Agregar proyecto".to_string(),
        };
        let label = self.fit_ui_text(
            &label,
            (rx + rw).saturating_sub(text_x + 12),
            tokens::TITLE,
            false,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &label,
            text_x,
            ry + rh / 2 + 4,
            rx + rw - 8,
            if hovered {
                theme.foreground
            } else if suggestion.is_some() {
                theme.accent_color()
            } else {
                theme.text_muted()
            },
            tokens::TITLE,
            suggestion.is_some(),
        );
    }
}
