//! Barra superior: pestañas, marca, título de ventana, acciones y controles de ventana.

use super::*;

impl TerminalRenderer {
    /// Marca de Axon: icono + wordmark con tracking. Sin decoración fuera de la marca.
    fn draw_brand(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        bar_h: usize,
        theme: &Theme,
    ) {
        let cy = bar_h / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            18,
            cy,
            15.0,
            theme.accent_color(),
        );
        self.draw_ui_text_tracked(
            buffer,
            width,
            height,
            "AXON",
            30,
            cy + 4,
            TopTabBarRects::BRAND_W.saturating_sub(14),
            theme.foreground,
            tokens::MICRO,
            true,
            1.5,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            TopTabBarRects::BRAND_W.saturating_sub(10),
            10,
            1,
            bar_h.saturating_sub(20),
            theme.chrome_line(),
        );
    }

    /// Selector de proyecto de la barra de pestañas: monograma, nombre y, si otro proyecto tiene novedades,
    /// un punto. Un clic pasa al siguiente proyecto.
    #[allow(clippy::too_many_arguments)]
    fn draw_project_switcher(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect: Rect,
        project: &TopBarProject,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        let bg = if hovered {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 9, bg);
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 9, theme.chrome_line());

        let sq = (x + 6, y + (h - 22) / 2, 22, 22);
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
            crate::ui::draw_rounded_rect(buffer, width, height, sq.0, sq.1, sq.2, sq.3, 7, color);
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
        self.draw_ui_text(
            buffer,
            width,
            height,
            &project.title,
            x + 36,
            y + h / 2 + 4,
            x + w - 24,
            theme.foreground,
            tokens::TITLE,
            true,
        );
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::ChevronDown,
            x + w - 13,
            y + h / 2,
            12.0,
            theme.text_muted(),
        );
        if project.has_news {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                x + w - 7,
                y - 3,
                11,
                11,
                5,
                theme.chrome_bg(),
            );
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                x + w - 5,
                y - 1,
                7,
                7,
                3,
                theme.ansi[3],
            );
        }
    }

    /// Botón de icono del chrome: plano, 30×30, radio 4, sin borde salvo estado activo.
    fn draw_chrome_action(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        active: bool,
        hovered: bool,
        icon_size: f32,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if active || hovered {
            let bg = if active {
                theme.accent_soft(30)
            } else {
                theme.surface_hover()
            };
            crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 4, bg);
        }
        let fg = if active {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            icon,
            x + w / 2,
            y + h / 2,
            icon_size,
            fg,
        );
    }

    /// Pestaña activa: superficie del lienzo con esquinas superiores redondeadas y regla de acento.
    fn draw_active_tab_surface(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        r: Rect,
        theme: &Theme,
    ) {
        let y = 5;
        let h = r.3.saturating_sub(y);
        crate::ui::draw_rounded_rect(buffer, width, height, r.0, y, r.2, h, 5, theme.background);
        // Base cuadrada: la pestaña se funde con el lienzo en el borde inferior.
        Self::draw_rect(
            buffer,
            width,
            height,
            r.0,
            y + 6,
            r.2,
            h.saturating_sub(6),
            theme.background,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            r.0 + 8,
            y,
            r.2.saturating_sub(16),
            2,
            theme.accent_color(),
        );
    }

    /// Renderiza la barra horizontal de pestañas: franja plana, pestañas y regla de acento.
    #[allow(clippy::too_many_arguments)]
    pub fn render_top_tab_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        tabs: &[SidebarTabItem],
        project: &TopBarProject,
        hovered_switcher: bool,
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
        let line = theme.chrome_line();

        // 1. Franja superior
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, theme.chrome_bg());

        // 2. Selector de proyecto
        self.draw_project_switcher(
            buffer,
            width,
            height,
            rects.switcher_btn,
            project,
            hovered_switcher,
            theme,
        );

        // 3. Pestañas del proyecto (`tab.index` es la posición real; la lista está filtrada)
        for (pos, tab) in tabs.iter().enumerate() {
            let idx = tab.index;
            let r = rects.tab_rect(pos);
            if r.0 + r.2 > rects.plus_btn.0 {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                Self::draw_active_tab_surface(buffer, width, height, r, theme);
            } else if is_hovered {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0,
                    6,
                    r.2,
                    r.3.saturating_sub(6),
                    5,
                    theme.surface_hover(),
                );
            } else {
                // Separador solo entre pestañas inactivas: una hairline, nunca una cápsula.
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    r.0 + r.2,
                    12,
                    1,
                    bar_h.saturating_sub(24),
                    line,
                );
            }

            let compact = rects.tab_w < 110;
            let mut icon_x = if compact { r.0 + (r.2 / 2) } else { r.0 + 18 };
            if tab.has_unread && !tab.is_active {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 7,
                    r.1 + (r.3 / 2) - 2,
                    4,
                    4,
                    2,
                    theme.ansi[6],
                );
                icon_x += 5;
            }

            let icon_color = if tab.is_active {
                theme.foreground
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
                icon_x,
                r.1 + (r.3 / 2),
                14.0,
                icon_color,
            );

            let show_close = rects.tab_w >= 76 && tabs.len() > 1 && (tab.is_active || is_hovered);

            if !compact {
                let text_color = if tab.is_active {
                    theme.foreground
                } else if is_hovered {
                    theme.text_secondary()
                } else {
                    theme.text_muted()
                };
                let title_x = r.0 + 34;
                let title_max = r.0 + r.2 - if show_close { 34 } else { 14 };
                let title_end = self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &tab.title,
                    title_x,
                    r.1 + (r.3 / 2) + 4,
                    title_max,
                    text_color,
                    tokens::TITLE,
                    tab.is_active,
                );
                if tab.is_preview {
                    Self::draw_rect(
                        buffer,
                        width,
                        height,
                        title_x,
                        r.1 + (r.3 / 2) + 7,
                        title_end.saturating_sub(title_x),
                        1,
                        text_color,
                    );
                }
                if tab.pane_count > 1 {
                    let badge = format!("×{}", tab.pane_count);
                    self.draw_mono_text(
                        buffer,
                        width,
                        height,
                        &badge,
                        title_end + 7,
                        r.1 + (r.3 / 2) + 4,
                        title_max,
                        theme.text_muted(),
                        tokens::MICRO,
                        false,
                    );
                }
            }

            if show_close {
                let close_r = rects.close_btn_rect(pos);
                let is_close_hovered = hovered_close == Some(idx);
                if is_close_hovered {
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        close_r.0,
                        close_r.1,
                        close_r.2,
                        close_r.3,
                        4,
                        theme.surface_hover(),
                    );
                }
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_r.0 + close_r.2 / 2,
                    close_r.1 + close_r.3 / 2,
                    12.0,
                    if is_close_hovered {
                        theme.ansi[1]
                    } else {
                        theme.text_muted()
                    },
                );
            }
        }

        // 4. Nueva pestaña
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.plus_btn,
            crate::ui::LucideIcon::Plus,
            false,
            hovered_new_tab,
            14.0,
            theme,
        );

        // 5. Acciones de la derecha
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.files_btn,
            crate::ui::LucideIcon::FolderTree,
            explorer_visible,
            hovered_files,
            15.0,
            theme,
        );
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.settings_btn,
            crate::ui::LucideIcon::Settings,
            false,
            hovered_settings,
            15.0,
            theme,
        );

        // Separador antes de los controles de ventana
        Self::draw_rect(
            buffer,
            width,
            height,
            rects.win_min_btn.0.saturating_sub(4),
            10,
            1,
            bar_h.saturating_sub(20),
            line,
        );

        // 6. Controles de ventana
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

        // 7. Hairline inferior; la pestaña activa la interrumpe para fundirse con el lienzo.
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, line);
        if let Some(tab) = tabs.iter().position(|t| t.is_active) {
            let r = rects.tab_rect(tab);
            if r.0 + r.2 <= rects.plus_btn.0 {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    r.0,
                    bar_h - 1,
                    r.2,
                    1,
                    theme.background,
                );
            }
        }
    }

    /// Dibuja los botones de control de ventana estándar (Minimizar, Maximizar, Cerrar)
    #[allow(clippy::too_many_arguments)]
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
        let hover_bg = theme.surface_hover();
        let fg = theme.text_secondary();

        if hovered_min {
            Self::draw_rect(
                buffer, width, height, min_btn.0, min_btn.1, min_btn.2, min_btn.3, hover_bg,
            );
        }
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Minus,
            min_btn.0 + (min_btn.2 / 2),
            min_btn.1 + (min_btn.3 / 2),
            13.0,
            if hovered_min { theme.foreground } else { fg },
        );

        if hovered_max {
            Self::draw_rect(
                buffer, width, height, max_btn.0, max_btn.1, max_btn.2, max_btn.3, hover_bg,
            );
        }
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
            if hovered_max { theme.foreground } else { fg },
        );

        // Cerrar: el único estado rojo del chrome, como en Windows.
        if hovered_close {
            Self::draw_rect(
                buffer,
                width,
                height,
                close_btn.0,
                close_btn.1,
                close_btn.2,
                close_btn.3,
                Color::from_rgb(0xe5, 0x48, 0x4d),
            );
        }
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::X,
            close_btn.0 + (close_btn.2 / 2),
            close_btn.1 + (close_btn.3 / 2),
            13.0,
            if hovered_close {
                Color::from_rgb(255, 255, 255)
            } else {
                fg
            },
        );
    }

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar).
    #[allow(clippy::too_many_arguments)]
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
        let line = theme.chrome_line();

        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, theme.chrome_bg());
        self.draw_brand(buffer, width, height, bar_h, theme);

        // Título alineado a la izquierda tras la marca; la rama es contexto, no decoración.
        let title_x = TopTabBarRects::BRAND_W;
        let actions_left = rects.settings_btn.0.saturating_sub(20);
        let branch_w = git_branch
            .map(|b| self.measure_mono_text(b, tokens::DETAIL, false) + 20)
            .unwrap_or(0);
        let title_max = actions_left.saturating_sub(branch_w + 16);
        let title_end = self.draw_ui_text(
            buffer,
            width,
            height,
            title,
            title_x,
            bar_h / 2 + 4,
            title_max,
            theme.foreground,
            tokens::SECTION,
            true,
        );
        if let Some(branch) = git_branch {
            let bx = title_end + 12;
            if bx + branch_w < actions_left {
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::GitBranch,
                    bx + 5,
                    bar_h / 2,
                    11.0,
                    theme.accent_color(),
                );
                self.draw_mono_text(
                    buffer,
                    width,
                    height,
                    branch,
                    bx + 15,
                    bar_h / 2 + 4,
                    actions_left,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
            }
        }

        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.files_btn,
            crate::ui::LucideIcon::FolderTree,
            explorer_visible,
            hovered_files,
            15.0,
            theme,
        );
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.settings_btn,
            crate::ui::LucideIcon::Settings,
            false,
            hovered_settings,
            15.0,
            theme,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            rects.win_min_btn.0.saturating_sub(4),
            10,
            1,
            bar_h.saturating_sub(20),
            line,
        );
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
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, line);
    }
}
