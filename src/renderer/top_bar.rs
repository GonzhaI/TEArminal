//! Barra superior: pestañas, marca, título de ventana, acciones y controles de ventana.

use super::*;

impl TerminalRenderer {
    /// Píldora de búsqueda centrada del modo vertical: icono, invitación y atajo Ctrl K.
    fn draw_search_pill(&mut self, canvas: &mut Canvas, rect: Rect, theme: &Theme) {
        let (x, y, w, h) = rect;
        if w < 40 || h == 0 {
            return;
        }
        canvas.rounded_fill(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.mix(theme.bar_bg(), theme.background, 0.4),
        );
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CONTROL, theme.chrome_bg());
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Search,
            x + 16,
            y + h / 2,
            13.0,
            theme.text_muted(),
        );
        let keycap = "Ctrl K";
        let keycap_w = self.measure_mono_text(keycap, tokens::MICRO, false) + 12;
        let keycap_x = x + w - 8 - keycap_w;
        let keycap_y = y + (h.saturating_sub(tokens::KEYCAP_H)) / 2;
        self.draw_keycap(canvas, keycap_x, keycap_y, keycap, theme);
        self.draw_ui_text(
            canvas,
            "Buscar sesiones, archivos y comandos",
            x + 30,
            y + h / 2 + 4,
            keycap_x.saturating_sub(8),
            theme.text_secondary(),
            tokens::BODY,
            false,
        );
    }

    /// Botón de icono del chrome: 32×32, radio 8, relleno solo con hover o estado activo.
    fn draw_chrome_action(
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

    /// Spinner de ocho segmentos con estela: la pestaña que ejecuta un comando.
    fn draw_tab_spinner(&mut self, canvas: &mut Canvas, cx: usize, cy: usize, color: Color) {
        const SEGMENTS: usize = 8;
        const ALPHA: [u8; SEGMENTS] = [255, 205, 158, 116, 82, 56, 36, 22];
        let phase = Self::spinner_phase();
        let radius = 5.0f32;
        for i in 0..SEGMENTS {
            let angle =
                (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let px = cx as f32 + angle.cos() * radius;
            let py = cy as f32 + angle.sin() * radius;
            let step = (i + SEGMENTS - phase % SEGMENTS) % SEGMENTS;
            let dot = Color::from_rgba(color.r, color.g, color.b, ALPHA[step]);
            canvas.rounded_fill(
                (px - 1.0).round().max(0.0) as usize,
                (py - 1.0).round().max(0.0) as usize,
                2,
                2,
                1,
                dot,
            );
        }
    }

    /// Fase del spinner en pasos de 110 ms.
    fn spinner_phase() -> usize {
        #[cfg(test)]
        if let Some(phase) = super::test_support::spinner_phase_override() {
            return phase;
        }
        use std::time::{SystemTime, UNIX_EPOCH};
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        ((ms / 110) % 8) as usize
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
        hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }

        let rects = TopTabBarRects::compute(width, tabs.len());
        let bar_h = rects.bar_h;

        // 1. Franja superior
        Self::draw_rect(canvas, 0, 0, width, bar_h, theme.bar_bg());

        // 2. Pestañas
        for (pos, tab) in tabs.iter().enumerate() {
            let idx = tab.index;
            let r = rects.tab_rect(pos);
            if r.0 + r.2 > rects.plus_btn.0 {
                break;
            }
            let is_hovered = hovered_tab == Some(idx);

            Self::draw_tab_chip(canvas, r, tab.is_active, is_hovered, theme);

            let has_spinner = tab.status == Some(SessionStatus::Running);
            let maybe_icon = tab_icon(tab);
            let has_leading = has_spinner || maybe_icon.is_some();

            let compact = rects.tab_w < 110;
            let mut icon_x = if compact { r.0 + (r.2 / 2) } else { r.0 + 18 };
            if tab.has_unread && !tab.is_active {
                canvas.rounded_fill(r.0 + 7, r.1 + (r.3 / 2) - 2, 4, 4, 2, theme.ansi[6]);
                icon_x += 5;
            }

            // La pestaña que ejecuta un comando muestra el spinner en lugar del icono de tipo: es el
            // único indicador animado del chrome y sale de OSC 133 (`SessionStatus`), nunca inventado.
            if has_spinner {
                let color = if tab.is_active {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                self.draw_tab_spinner(canvas, icon_x, r.1 + (r.3 / 2), color);
            } else if let Some(icon) = maybe_icon {
                let icon_color = tab_icon_color(tab, tab.is_active, is_hovered, theme);
                self.draw_lucide_icon(canvas, icon, icon_x, r.1 + (r.3 / 2), 14.0, icon_color);
            }

            let show_close = rects.tab_w >= 76 && (tab.is_active || is_hovered);

            if !compact || !has_leading {
                let text_color = if tab.is_active {
                    theme.foreground
                } else if is_hovered {
                    theme.text_secondary()
                } else {
                    theme.text_muted()
                };
                let title_x = if has_leading {
                    r.0 + 34
                } else if tab.has_unread && !tab.is_active {
                    r.0 + 18
                } else {
                    r.0 + 14
                };
                let title_max = r.0 + r.2 - if show_close { 34 } else { 14 };
                // El prefijo «ssh: » y el usuario son del título de la sesión, no de la pestaña: la
                // barra muestra solo el destino («staging-01»), como en el bosquejo.
                let title = match tab.title.strip_prefix("ssh: ") {
                    Some(target) => target.rsplit('@').next().unwrap_or(target),
                    None => tab.title.as_str(),
                };
                let title_end = self.draw_ui_text(
                    canvas,
                    title,
                    title_x,
                    r.1 + (r.3 / 2) + 4,
                    title_max,
                    text_color,
                    tokens::TITLE,
                    tab.is_active,
                );
                if tab.is_preview {
                    Self::draw_rect(
                        canvas,
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
                        canvas,
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

        // 5. Acciones de la derecha: buscar · panel · ajustes, separadas de las pestañas por una línea fina.
        let sep_x = rects.search_btn.0.saturating_sub(8);
        if sep_x >= rects.plus_btn.0 + rects.plus_btn.2 + 8 {
            Self::draw_rect(canvas, sep_x, (bar_h - 16) / 2, 1, 16, theme.chrome_line());
        }
        self.draw_chrome_action(
            canvas,
            rects.search_btn,
            crate::ui::LucideIcon::Search,
            false,
            false,
            16.0,
            theme,
        );
        self.draw_chrome_action(
            canvas,
            rects.files_btn,
            crate::ui::LucideIcon::PanelLeft,
            explorer_visible,
            hovered_files,
            16.0,
            theme,
        );
        self.draw_chrome_action(
            canvas,
            rects.settings_btn,
            crate::ui::LucideIcon::Sliders,
            false,
            hovered_settings,
            16.0,
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

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar): marca,
    /// píldora de búsqueda centrada y controles de ventana. El título y la rama viven en la barra de estado.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_top_title_bar(
        &mut self,
        canvas: &mut Canvas,
        _title: &str,
        _git_branch: Option<&str>,
        _hovered_files: bool,
        _hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        _explorer_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }
        let rects = TopTitleBarRects::compute(width);
        let bar_h = rects.bar_h;

        Self::draw_rect(canvas, 0, 0, width, bar_h, theme.bar_bg());
        self.draw_search_pill(canvas, rects.search_box, theme);
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
}

/// Color del icono de la pestaña según su tipo: terminal, agente o servidor. Es información, así que
/// el color sigue al tipo y no al estado de la pestaña.
fn tab_icon_color(tab: &SidebarTabItem, active: bool, hovered: bool, theme: &Theme) -> Color {
    if tab.is_settings || tab.is_file_viewer {
        return if active {
            theme.foreground
        } else if hovered {
            theme.text_secondary()
        } else {
            theme.text_muted()
        };
    }
    // La pestaña activa no colorea sus iconos: el título ya manda y el tipo es información secundaria.
    if active {
        return theme.foreground;
    }
    match tab_icon(tab) {
        Some(crate::ui::LucideIcon::Sparkles) => theme.ansi[5],
        Some(crate::ui::LucideIcon::Server) => theme.ansi[6],
        Some(crate::ui::LucideIcon::Terminal) => theme.ansi[4],
        _ => theme.text_muted(),
    }
}
