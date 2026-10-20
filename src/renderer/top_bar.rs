//! Barra superior: pestañas contiguas, marca, acciones y controles de ventana.

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
            68,
            theme.foreground,
            10.0,
            true,
            1.5,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            68,
            10,
            1,
            bar_h - 20,
            theme.chrome_line(),
        );
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
        self.draw_lucide_icon(buffer, width, height, icon, x + w / 2, y + h / 2, 14.0, fg);
    }

    /// Renderiza la barra horizontal de pestañas: franja plana, pestañas contiguas y regla de acento.
    #[allow(clippy::too_many_arguments)]
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
        let line = theme.chrome_line();

        // 1. Franja superior
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, theme.chrome_bg());

        // 2. Marca
        self.draw_brand(buffer, width, height, bar_h, theme);

        // 3. Pestañas
        for (idx, tab) in tabs.iter().enumerate() {
            let r = rects.tab_rect(idx);
            if r.0 + r.2 > rects.plus_btn.0 {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                // La pestaña activa se funde con el lienzo: sin borde inferior y con regla de acento.
                Self::draw_rect(buffer, width, height, r.0, r.1, r.2, r.3, theme.background);
                Self::draw_rect(buffer, width, height, r.0, 0, r.2, 2, theme.accent_color());
            } else {
                if is_hovered {
                    Self::draw_rect(
                        buffer,
                        width,
                        height,
                        r.0,
                        r.1,
                        r.2,
                        r.3,
                        theme.surface_hover(),
                    );
                }
                // Separador solo entre pestañas inactivas: una hairline, nunca una cápsula.
                Self::draw_rect(buffer, width, height, r.0 + r.2, 10, 1, bar_h - 20, line);
            }

            let compact = rects.tab_w < 104;
            let mut icon_x = if compact { r.0 + (r.2 / 2) } else { r.0 + 16 };
            if tab.has_unread && !tab.is_active {
                let dot = theme.ansi[6];
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 6,
                    r.1 + (r.3 / 2) - 2,
                    4,
                    4,
                    2,
                    dot,
                );
                icon_x += 4;
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
                13.0,
                icon_color,
            );

            let show_close = rects.tab_w >= 72 && tabs.len() > 1 && (tab.is_active || is_hovered);

            if !compact {
                let text_color = if tab.is_active {
                    theme.foreground
                } else if is_hovered {
                    theme.text_secondary()
                } else {
                    theme.text_muted()
                };
                let title_x = r.0 + 30;
                let title_max = r.0 + r.2 - if show_close { 30 } else { 12 };
                let title_end = self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &tab.title,
                    title_x,
                    r.1 + (r.3 / 2) + 4,
                    title_max,
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
                        r.1 + (r.3 / 2) + 7,
                        title_end.saturating_sub(title_x),
                        1,
                        text_color,
                    );
                }
                if tab.pane_count > 1 {
                    let badge = format!("×{}", tab.pane_count);
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        &badge,
                        title_end + 6,
                        r.1 + (r.3 / 2) + 4,
                        title_max,
                        theme.text_muted(),
                        9.5,
                        false,
                    );
                }
            }

            if show_close {
                let close_r = rects.close_btn_rect(idx);
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
            bar_h - 20,
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

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar)
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

        // Título centrado + rama como dato, no como glifo decorativo.
        let title_w = self.measure_ui_text(title, 12.0, true);
        let branch_w = git_branch
            .map(|b| self.measure_ui_text(b, 10.5, false) + 18)
            .unwrap_or(0);
        let total_w = title_w + if branch_w > 0 { branch_w + 10 } else { 0 };
        let center_x = if width > total_w + 360 {
            (width - total_w) / 2
        } else {
            88
        };
        let mut cursor = self.draw_ui_text(
            buffer,
            width,
            height,
            title,
            center_x,
            bar_h / 2 + 4,
            width.saturating_sub(200),
            theme.foreground,
            12.0,
            true,
        );
        if let Some(branch) = git_branch {
            cursor += 10;
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::GitBranch,
                cursor + 6,
                bar_h / 2,
                11.0,
                theme.accent_color(),
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                branch,
                cursor + 16,
                bar_h / 2 + 4,
                width.saturating_sub(210),
                theme.text_secondary(),
                10.5,
                false,
            );
        }

        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.files_btn,
            crate::ui::LucideIcon::FolderTree,
            explorer_visible,
            hovered_files,
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
            theme,
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            rects.win_min_btn.0.saturating_sub(4),
            10,
            1,
            bar_h - 20,
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
