//! Barra superior: pestañas, marca, título de ventana, acciones y controles de ventana.

use super::*;

impl TerminalRenderer {
    /// Marca de Axon: el prompt `>_` en acento y el wordmark. Sin decoración fuera de la marca.
    fn draw_brand(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        bar_h: usize,
        theme: &Theme,
    ) {
        let cy = bar_h / 2;
        self.draw_mono_text(
            buffer,
            width,
            height,
            ">_",
            16,
            cy + 4,
            40,
            theme.accent_color(),
            tokens::BODY,
            true,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            42,
            cy + 5,
            TopTabBarRects::BRAND_W,
            theme.foreground,
            tokens::TITLE,
            true,
        );
    }

    /// Píldora de búsqueda centrada del modo vertical: icono, invitación y atajo Ctrl K.
    fn draw_search_pill(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect: Rect,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w < 40 || h == 0 {
            return;
        }
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x,
            y,
            w,
            h,
            9,
            theme.mix(theme.bar_bg(), theme.background, 0.4),
        );
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 9, theme.chrome_bg());
        self.draw_lucide_icon(
            buffer,
            width,
            height,
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
        self.draw_keycap(buffer, width, height, keycap_x, keycap_y, keycap, theme);
        self.draw_ui_text(
            buffer,
            width,
            height,
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
        if w == 0 || h == 0 {
            return;
        }
        if active || hovered {
            let bg = if active {
                theme.surface_bg()
            } else {
                theme.surface_hover()
            };
            crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 8, bg);
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

    /// Pestaña activa: superficie del lienzo con esquinas superiores redondeadas.
    fn draw_active_tab_surface(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        r: Rect,
        theme: &Theme,
    ) {
        let y = 5;
        let h = r.3.saturating_sub(y);
        crate::ui::draw_rounded_rect(buffer, width, height, r.0, y, r.2, h, 6, theme.background);
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
    }

    /// Spinner de ocho segmentos con estela: la pestaña que ejecuta un comando.
    fn draw_tab_spinner(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        cx: usize,
        cy: usize,
        color: Color,
    ) {
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
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
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

        // 1. Franja superior
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, theme.bar_bg());

        // 2. Marca
        self.draw_brand(buffer, width, height, bar_h, theme);

        // 3. Pestañas
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
                    5,
                    r.2,
                    r.3.saturating_sub(5),
                    6,
                    theme.surface_hover(),
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

            // La pestaña que ejecuta un comando muestra el spinner en lugar del icono de tipo: es el
            // único indicador animado del chrome y sale de OSC 133 (`SessionStatus`), nunca inventado.
            if tab.status == Some(SessionStatus::Running) {
                let color = if tab.is_active {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                self.draw_tab_spinner(buffer, width, height, icon_x, r.1 + (r.3 / 2), color);
            } else {
                let icon_color = tab_icon_color(tab, tab.is_active, is_hovered, theme);
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
            }

            let show_close = rects.tab_w >= 76 && (tab.is_active || is_hovered);

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
                // El prefijo «ssh: » y el usuario son del título de la sesión, no de la pestaña: la
                // barra muestra solo el destino («staging-01»), como en el bosquejo.
                let title = match tab.title.strip_prefix("ssh: ") {
                    Some(target) => target.rsplit('@').next().unwrap_or(target),
                    None => tab.title.as_str(),
                };
                let title_end = self.draw_ui_text(
                    buffer,
                    width,
                    height,
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

        // 5. Acciones de la derecha: buscar · panel · ajustes
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.search_btn,
            crate::ui::LucideIcon::Search,
            false,
            false,
            16.0,
            theme,
        );
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.files_btn,
            crate::ui::LucideIcon::PanelLeft,
            explorer_visible,
            hovered_files,
            16.0,
            theme,
        );
        self.draw_chrome_action(
            buffer,
            width,
            height,
            rects.settings_btn,
            crate::ui::LucideIcon::Sliders,
            false,
            hovered_settings,
            16.0,
            theme,
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
            crate::ui::stroke_rounded_rect(buffer, width, height, x + 3, y, 8, 8, 2, max_fg);
            crate::ui::draw_rounded_rect(buffer, width, height, x, y + 3, 8, 8, 2, bg);
            crate::ui::stroke_rounded_rect(buffer, width, height, x, y + 3, 8, 8, 2, max_fg);
        } else {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Square,
                cx,
                cy,
                12.0,
                max_fg,
            );
        }

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

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar): marca,
    /// píldora de búsqueda centrada y controles de ventana. El título y la rama viven en la barra de estado.
    #[allow(clippy::too_many_arguments)]
    pub fn render_top_title_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
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
        if width == 0 || height == 0 {
            return;
        }
        let rects = TopTitleBarRects::compute(width);
        let bar_h = rects.bar_h;

        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, theme.bar_bg());
        self.draw_brand(buffer, width, height, bar_h, theme);
        self.draw_search_pill(buffer, width, height, rects.search_box, theme);
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
        crate::ui::LucideIcon::Sparkles => theme.ansi[5],
        crate::ui::LucideIcon::Server => theme.ansi[6],
        crate::ui::LucideIcon::Terminal => theme.ansi[4],
        _ => theme.text_muted(),
    }
}
