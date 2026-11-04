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

    /// Píldora de la barra superior: icono y etiqueta; la activa lleva relleno, el hover un tono más suave.
    fn draw_top_pill(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        label: &str,
        active: bool,
        hovered: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        if active {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.surface_bg());
        } else if hovered {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.surface_hover());
        }
        let fg = if active || hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        self.draw_lucide_icon(canvas, icon, x + 15, y + h / 2, 14.0, fg);
        let baseline = self.centered_baseline(y + h / 2);
        self.draw_ui_text(
            canvas,
            label,
            x + 29,
            baseline,
            x + w - 6,
            fg,
            tokens::BODY,
            active,
        );
    }

    /// Chip de pestaña dentro de su `tab_rect`, recortado 4 px por arriba y por abajo: la activa lleva el
    /// color de la tarjeta de contenido (con el que se funde y sobre el que el texto ya tiene el contraste
    /// del propio tema) y contorno; una inactiva solo relleno al pasar el ratón.
    fn draw_tab_chip(canvas: &mut Canvas, r: Rect, active: bool, hovered: bool, theme: &Theme) {
        let (x, y, w, h) = (r.0, r.1 + 4, r.2, r.3.saturating_sub(8));
        if active {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.background);
            canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CONTROL, theme.card_outline());
        } else if hovered {
            canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.surface_hover());
        }
    }

    /// Renderiza la barra horizontal de pestañas: franja plana, pestañas y regla de acento.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_top_tab_bar(
        &mut self,
        canvas: &mut Canvas,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_files: bool,
        hovered_changes: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        explorer_visible: bool,
        changes_active: bool,
        left_x: usize,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }

        let rects = TopTabBarRects::compute(width, left_x, tabs.len());
        let bar_h = rects.bar_h;

        // 1. Franja superior: a la derecha del panel de Spaces, que ocupa todo el alto.
        Self::draw_rect(
            canvas,
            left_x,
            0,
            width.saturating_sub(left_x),
            bar_h,
            theme.bar_bg(),
        );

        // 2. Chip activo animado (si hay animación activa) o estático por pestaña
        let anim_active_drawn = if let Some((ix, iw)) = self.tab_indicator_visual {
            let active_y = 4;
            let active_h = bar_h.saturating_sub(8);
            let rx = ix.round() as usize;
            let rw = iw.round() as usize;
            canvas.rounded_fill(
                rx,
                active_y,
                rw,
                active_h,
                tokens::RADIUS_CONTROL,
                theme.background,
            );
            canvas.rounded_stroke(
                rx,
                active_y,
                rw,
                active_h,
                tokens::RADIUS_CONTROL,
                theme.card_outline(),
            );
            true
        } else {
            false
        };

        // 3. Pestañas
        for (pos, tab) in tabs.iter().enumerate() {
            let idx = tab.index;
            let r = rects.tab_rect(pos);
            if r.0 + r.2 > rects.plus_btn.0 {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            if anim_active_drawn {
                if !tab.is_active && is_hovered {
                    Self::draw_tab_chip(canvas, r, false, true, theme);
                }
            } else {
                Self::draw_tab_chip(canvas, r, tab.is_active, is_hovered, theme);
            }

            let show_close = rects.tab_w >= 76 && (tab.is_active || is_hovered);

            let text_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let mut title_x = r.0 + 14;
            if let Some(path) = &tab.file_path {
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
            let title_max = r.0 + r.2 - if show_close { 34 } else { 14 };
            // El prefijo «ssh: » y el usuario son del título de la sesión, no de la pestaña: la
            // barra muestra solo el destino («staging-01»), como en el bosquejo.
            let title = match tab.title.strip_prefix("ssh: ") {
                Some(target) => target.rsplit('@').next().unwrap_or(target),
                None => tab.title.as_str(),
            };
            let tab_base = self.centered_baseline(r.1 + (r.3 / 2));
            let title_end = self.draw_ui_text(
                canvas,
                title,
                title_x,
                tab_base,
                title_max,
                text_color,
                tokens::TITLE,
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
                    title_end + 7,
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
                    canvas.rounded_fill(
                        close_r.0,
                        close_r.1,
                        close_r.2,
                        close_r.3,
                        tokens::RADIUS_ROW,
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
        }

        // 4. Nueva pestaña
        self.draw_chrome_action(
            canvas,
            rects.plus_btn,
            crate::ui::LucideIcon::Plus,
            false,
            hovered_new_tab,
            14.0,
            theme,
        );

        // 5. Píldoras de la tarjeta derecha: Files y Changes (la activa, con relleno).
        self.draw_top_pill(
            canvas,
            rects.files_btn,
            crate::ui::LucideIcon::File,
            "Archivos",
            explorer_visible && !changes_active,
            hovered_files,
            theme,
        );
        self.draw_top_pill(
            canvas,
            rects.changes_btn,
            crate::ui::LucideIcon::GitBranch,
            "Cambios",
            explorer_visible && changes_active,
            hovered_changes,
            theme,
        );

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
            canvas.rounded_stroke(x + 3, y, 8, 8, 2, max_fg);
            canvas.rounded_fill(x, y + 3, 8, 8, 2, bg);
            canvas.rounded_stroke(x, y + 3, 8, 8, 2, max_fg);
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
