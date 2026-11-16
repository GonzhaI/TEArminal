//! Barra superior: pestañas, marca, título de ventana, acciones y controles de ventana.

use super::*;

impl TerminalRenderer {
    /// Botón de icono del chrome: 32×32, radio 8, relleno solo con hover o estado activo.
    pub(super) fn draw_chrome_action(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        active: bool,
        hovered: bool,
        icon_size: f32,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        if active || hovered {
            let bg = if active {
                theme.surface_bg()
            } else {
                theme.surface_hover()
            };
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, bg);
        }
        let fg = if active {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(canvas, icon, x + w / 2, y + h / 2, icon_size, fg);
    }

    /// Renderiza la barra horizontal superior (DISENO §4):
    /// Nombre «TEARMINAL» sobre riel + panel contextual, pestañas, botón `+`, buscador 290×26 y controles de ventana.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_top_tab_bar(
        &mut self,
        canvas: &mut Canvas,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_search: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        left_x: usize,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }

        let rects = TopTabBarRects::compute(width, left_x, tabs.len());
        let bar_h = rects.bar_h;

        // 1. Fondo completo de la barra superior y línea inferior divisoria
        Self::draw_rect(canvas, 0, 0, width, bar_h, theme.bar_bg());
        Self::draw_rect(canvas, 0, bar_h - 1, width, 1, theme.chrome_line());

        // 2. Título de marca «TEARMINAL» (DISENO §4)
        if left_x > 0 {
            Self::draw_rect(canvas, left_x - 1, 0, 1, bar_h, theme.chrome_line());
            let brand_base = self.centered_baseline(bar_h / 2);
            self.draw_ui_text(
                canvas,
                "TEARMINAL",
                16,
                brand_base,
                left_x.saturating_sub(12),
                theme.foreground,
                tokens::TITLE,
                true,
            );
        }

        // 3. Pestañas
        let tabs_limit = if rects.search_btn.2 > 0 {
            rects.search_btn.0
        } else {
            rects.win_min_btn.0
        };
        for (pos, tab) in tabs.iter().enumerate() {
            let idx = tab.index;
            let r = rects.tab_rect(pos);
            if r.0 >= tabs_limit {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                // Pestaña activa: fondo base y barra superior de 2px blue (DISENO §3.3)
                Self::draw_rect(canvas, r.0, 0, r.2, bar_h - 1, theme.background);
                Self::draw_rect(canvas, r.0, 0, r.2, 2, theme.accent_color());
            } else if is_hovered {
                Self::draw_rect(canvas, r.0, 0, r.2, bar_h - 1, theme.surface_hover());
            }
            let show_close = rects.tab_w >= 76 && (tab.is_active || is_hovered);

            let text_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };

            let title_max = r.0 + r.2.saturating_sub(if show_close { 32 } else { 12 });
            let mut title_x = if r.2 < 36 {
                r.0 + (r.2.saturating_sub(8)) / 2
            } else {
                r.0 + 12
            };

            // Punto de estado o icono de archivo
            if let Some(path) = &tab.file_path {
                if title_x + 12 < r.0 + r.2 {
                    self.draw_file_icon(
                        canvas,
                        path,
                        title_x + 6,
                        r.1 + r.3 / 2,
                        tokens::BODY,
                        theme,
                    );
                    title_x += 20;
                }
            } else if title_x + 8 < r.0 + r.2 {
                let dot_color = if tab.is_active {
                    theme.accent_color()
                } else {
                    theme.text_muted()
                };
                let dot_y = (bar_h.saturating_sub(8)) / 2;
                Self::draw_rect(canvas, title_x, dot_y, 8, 8, dot_color);
                title_x += 16;
            }
            let title = match tab.title.strip_prefix("ssh: ") {
                Some(target) => target.rsplit('@').next().unwrap_or(target),
                None => tab.title.as_str(),
            };
            let tab_base = self.centered_baseline(bar_h / 2);
            let title_end = self.draw_ui_text(
                canvas,
                title,
                title_x,
                tab_base,
                title_max,
                text_color,
                tokens::BODY,
                tab.is_active,
            );

            if tab.is_preview {
                Self::draw_rect(
                    canvas,
                    title_x,
                    tab_base + 3,
                    title_end.saturating_sub(title_x),
                    1,
                    text_color,
                );
            }
            if tab.pane_count > 1 {
                let badge = format!("×{}", tab.pane_count);
                self.draw_mono_text(
                    canvas,
                    &badge,
                    title_end + 6,
                    tab_base,
                    title_max,
                    theme.text_muted(),
                    tokens::MICRO,
                    false,
                );
            }

            if show_close {
                let close_r = rects.close_btn_rect(pos);
                let is_close_hovered = hovered_close == Some(idx);
                if is_close_hovered {
                    Self::draw_rect(
                        canvas,
                        close_r.0,
                        close_r.1,
                        close_r.2,
                        close_r.3,
                        theme.surface_hover(),
                    );
                }
                self.draw_lucide_icon(
                    canvas,
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

            // Línea separadora derecha de cada pestaña
            Self::draw_rect(canvas, r.0 + r.2 - 1, 0, 1, bar_h, theme.chrome_line());
        }

        // 4. Nueva pestaña (+)
        self.draw_chrome_action(
            canvas,
            rects.plus_btn,
            crate::ui::LucideIcon::Plus,
            false,
            hovered_new_tab,
            14.0,
            theme,
        );

        // 5. Buscador de 290×26 px («Buscar o ejecutar… Ctrl+K»)
        let (sx, sy, sw, sh) = rects.search_btn;
        if sw > 0 {
            let search_border = if hovered_search {
                theme.accent_color()
            } else {
                theme.chrome_line()
            };
            Self::draw_rect(canvas, sx, sy, sw, sh, theme.background);
            Self::draw_rect(canvas, sx, sy, sw, 1, search_border);
            Self::draw_rect(canvas, sx, sy + sh - 1, sw, 1, search_border);
            Self::draw_rect(canvas, sx, sy, 1, sh, search_border);
            Self::draw_rect(canvas, sx + sw - 1, sy, 1, sh, search_border);

            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Search,
                sx + 15,
                sy + sh / 2,
                13.0,
                theme.text_muted(),
            );
            let search_base = self.centered_baseline(sy + sh / 2);
            self.draw_ui_text(
                canvas,
                "Buscar o ejecutar…",
                sx + 28,
                search_base,
                sx + sw - 60,
                theme.text_muted(),
                tokens::BODY,
                false,
            );
            let mono_w = self.measure_mono_text("Ctrl+K", tokens::MICRO, false);
            self.draw_mono_text(
                canvas,
                "Ctrl+K",
                sx + sw - 10 - mono_w,
                search_base,
                sx + sw - 6,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );
        }

        // 6. Controles de ventana
        self.draw_window_controls(
            canvas,
            rects.win_min_btn,
            rects.win_max_btn,
            rects.win_close_btn,
            hovered_win_min,
            hovered_win_max,
            hovered_win_close,
            is_maximized,
            theme,
        );
    }

    /// Dibuja los botones de control de ventana estándar (Minimizar, Maximizar, Cerrar)
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_window_controls(
        &mut self,
        canvas: &mut Canvas,
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
            Self::draw_rect(canvas, min_btn.0, min_btn.1, min_btn.2, min_btn.3, hover_bg);
        }
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Minus,
            min_btn.0 + (min_btn.2 / 2),
            min_btn.1 + (min_btn.3 / 2),
            13.0,
            if hovered_min { theme.foreground } else { fg },
        );

        if hovered_max {
            Self::draw_rect(canvas, max_btn.0, max_btn.1, max_btn.2, max_btn.3, hover_bg);
        }
        let max_fg = if hovered_max { theme.foreground } else { fg };
        let (cx, cy) = (max_btn.0 + max_btn.2 / 2, max_btn.1 + max_btn.3 / 2);
        if is_maximized {
            // Restaurar, como en Windows: el cuadrado de adelante abajo a la izquierda y el de atrás
            // arriba a la derecha (el `Copy` de Lucide está espejado).
            let bg = if hovered_max {
                hover_bg
            } else {
                theme.bar_bg()
            };
            let (x, y) = (cx.saturating_sub(5), cy.saturating_sub(4));
            canvas.rounded_stroke(x + 3, y, 8, 8, tokens::RADIUS_ROW, max_fg);
            canvas.rounded_fill(x, y + 3, 8, 8, tokens::RADIUS_ROW, bg);
            canvas.rounded_stroke(x, y + 3, 8, 8, tokens::RADIUS_ROW, max_fg);
        } else {
            self.draw_lucide_icon(canvas, crate::ui::LucideIcon::Square, cx, cy, 12.0, max_fg);
        }

        // Cerrar: el único estado rojo del chrome, como en Windows.
        if hovered_close {
            Self::draw_rect(
                canvas,
                close_btn.0,
                close_btn.1,
                close_btn.2,
                close_btn.3,
                Color::from_rgb(0xe5, 0x48, 0x4d),
            );
        }
        self.draw_lucide_icon(
            canvas,
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
}
