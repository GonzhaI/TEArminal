//! Página de ajustes (pestaña dedicada).

use super::*;

impl TerminalRenderer {
    pub(super) fn draw_button(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        text: &str,
        hovered: bool,
        active_accent: bool,
        theme: &Theme,
    ) {
        let (bx, by, bw, bh) = rect;
        let bg = if active_accent {
            theme.accent_soft(32)
        } else if hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
        let border = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_ROW, border);

        let fg = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let text_w = self.measure_ui_text(text, tokens::BODY, active_accent);
        let tx = bx + (bw.saturating_sub(text_w) / 2);
        let baseline = by + (bh / 2) + 4;
        self.draw_ui_text(
            canvas,
            text,
            tx,
            baseline,
            bx + bw,
            fg,
            tokens::BODY,
            active_accent,
        );
    }

    /// Botón de opción con icono Lucide y etiqueta (disposición de pestañas).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_icon_button(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        icon: crate::ui::LucideIcon,
        text: &str,
        hovered: bool,
        active_accent: bool,
        theme: &Theme,
    ) {
        let (bx, by, bw, bh) = rect;
        let bg = if active_accent {
            theme.accent_soft(32)
        } else if hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
        let border = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_ROW, border);

        let fg = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let text_w = self.measure_ui_text(text, tokens::BODY, active_accent);
        let content_w = 14 + 6 + text_w;
        let start = bx + (bw.saturating_sub(content_w) / 2);
        let cy = by + bh / 2;
        self.draw_lucide_icon(canvas, icon, start + 7, cy, 13.0, fg);
        self.draw_ui_text(
            canvas,
            text,
            start + 20,
            cy + 4,
            bx + bw,
            fg,
            tokens::BODY,
            active_accent,
        );
    }

    pub(super) fn draw_stepper_value(
        &mut self,
        canvas: &mut Canvas,
        minus_rect: Rect,
        plus_rect: Rect,
        text: &str,
        is_editing: bool,
        is_hovered: bool,
        theme: &Theme,
    ) {
        let span_start = minus_rect.0 + minus_rect.2 + 4;
        let span_end = plus_rect.0.saturating_sub(4);
        let span_w = span_end.saturating_sub(span_start);
        let box_y = minus_rect.1;
        let box_h = minus_rect.3;

        let bg = if is_editing || is_hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(span_start, box_y, span_w, box_h, tokens::RADIUS_ROW, bg);

        let border_c = if is_editing {
            theme.accent_color()
        } else if is_hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(
            span_start,
            box_y,
            span_w,
            box_h,
            tokens::RADIUS_ROW,
            border_c,
        );

        let disp_text = if is_editing {
            format!("{}|", text)
        } else {
            text.to_string()
        };

        let text_w = self.measure_mono_text(&disp_text, tokens::BODY, true);
        let tx = span_start + (span_w.saturating_sub(text_w) / 2);
        let baseline = box_y + (box_h / 2) + 4;
        let fg = if is_editing {
            theme.foreground
        } else {
            theme.accent_color()
        };
        self.draw_mono_text(
            canvas,
            &disp_text,
            tx,
            baseline,
            span_end.saturating_sub(2),
            fg,
            tokens::BODY,
            true,
        );
    }

    pub(super) fn draw_label(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        max_x: usize,
        label: &str,
        theme: &Theme,
    ) {
        self.draw_ui_text(
            canvas,
            label,
            x,
            y + 18,
            max_x,
            theme.foreground,
            tokens::BODY,
            false,
        );
    }

    /// Etiqueta de grupo: micro-mayúsculas con tracking, sin iconos.
    pub(super) fn draw_group_label(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        baseline: usize,
        max_x: usize,
        label: &str,
        theme: &Theme,
    ) {
        self.draw_ui_text_tracked(
            canvas,
            label,
            x,
            baseline,
            max_x,
            theme.text_muted(),
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );
    }

    pub(super) fn draw_card(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        title: &str,
        theme: &Theme,
    ) {
        canvas.rounded_fill(x, y, w, h, tokens::RADIUS_CONTROL, theme.surface_bg());
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CONTROL, theme.chrome_line());

        // Cabecera editorial de la tarjeta: 40 px, micro-etiqueta con tracking.
        Self::draw_rect(canvas, x, y + 40, w, 1, theme.chrome_line());
        self.draw_ui_text_tracked(
            canvas,
            title,
            x + 20,
            y + 26,
            x + w,
            theme.text_muted(),
            tokens::MICRO,
            true,
            tokens::TRACKING,
        );
    }

    /// Fila de opción booleana: etiqueta a la izquierda y checkbox de 18 px a la derecha.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_toggle_row(
        &mut self,
        canvas: &mut Canvas,
        label_x: usize,
        label_max: usize,
        rect: Rect,
        label: &str,
        on: bool,
        hovered: bool,
        theme: &Theme,
    ) {
        let cy = rect.1 + rect.3 / 2;
        self.draw_ui_text(
            canvas,
            label,
            label_x,
            cy + 4,
            label_max,
            if hovered {
                theme.foreground
            } else {
                theme.text_secondary()
            },
            tokens::BODY,
            false,
        );

        let size = tokens::CHECKBOX;
        let bx = rect.0;
        let by = cy.saturating_sub(size / 2);
        let bg = if on {
            theme.accent_soft(if hovered { 44 } else { 32 })
        } else if hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(bx, by, size, size, tokens::RADIUS_ROW, bg);
        let border = if on {
            theme.accent_color()
        } else if hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(bx, by, size, size, tokens::RADIUS_ROW, border);
        if on {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Check,
                bx + size / 2,
                by + size / 2,
                12.0,
                theme.accent_color(),
            );
        }
        self.draw_ui_text(
            canvas,
            if on { "Activado" } else { "Desactivado" },
            bx + size + 8,
            cy + 4,
            rect.0 + rect.2,
            if on {
                theme.text_secondary()
            } else {
                theme.text_muted()
            },
            tokens::DETAIL,
            false,
        );
    }

    /// Renderiza la página completa de la pestaña de Ajustes y Personalización de TEArminal
    pub(crate) fn render_settings_page(
        &mut self,
        canvas: &mut Canvas,
        page_x: usize,
        right_inset: usize,
        page_y: usize,
        status_bar_h: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width <= page_x + right_inset || height <= page_y {
            return;
        }

        let page_w = width - page_x - right_inset;
        let page_h = height - page_y;
        let rects = SettingsPageRects::compute(page_x, page_y, page_w, page_h);

        // 1. Fondo de la página
        Self::draw_rect(canvas, page_x, page_y, page_w, page_h, theme.background);

        // 2. Encabezado editorial: título de 20 px, contexto y restablecer.
        let content_x = rects.content_x;
        let card_w = rects.card_w;
        self.draw_ui_text(
            canvas,
            "Ajustes",
            content_x,
            page_y + 48,
            content_x + card_w.saturating_sub(190),
            theme.foreground,
            tokens::DISPLAY,
            true,
        );
        self.draw_ui_text(
            canvas,
            "Apariencia, terminal y atajos de teclado",
            content_x,
            page_y + 70,
            content_x + card_w.saturating_sub(190),
            theme.text_muted(),
            tokens::DETAIL,
            false,
        );

        let r_btn = rects.reset_defaults_btn;
        if state.hovered_reset_defaults {
            canvas.rounded_fill(
                r_btn.0,
                r_btn.1,
                r_btn.2,
                r_btn.3,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        let r_fg = if state.hovered_reset_defaults {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let r_cy = r_btn.1 + (r_btn.3 / 2);
        let r_label = "Restablecer";
        let r_label_w = self.measure_ui_text(r_label, tokens::BODY, false);
        let r_text_x = r_btn.0 + r_btn.2.saturating_sub(r_label_w + 8);
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::RefreshCw,
            r_text_x.saturating_sub(13),
            r_cy,
            11.0,
            r_fg,
        );
        self.draw_ui_text(
            canvas,
            r_label,
            r_text_x,
            r_cy + 4,
            r_btn.0 + r_btn.2,
            r_fg,
            tokens::BODY,
            false,
        );

        // Barra de búsqueda rápida de ajustes estilo JetBrains / DataGrip
        let sb = rects.search_box;
        let is_searching = state.editing_setting == Some(EditingSettingField::GeneralSearch);
        let sb_bg = if is_searching || state.hovered_search_box {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(sb.0, sb.1, sb.2, sb.3, tokens::RADIUS_ROW, sb_bg);
        let sb_border = if is_searching {
            theme.accent_color()
        } else if state.hovered_search_box {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(sb.0, sb.1, sb.2, sb.3, tokens::RADIUS_ROW, sb_border);

        let icon_c = if is_searching {
            theme.accent_color()
        } else {
            theme.text_muted()
        };
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Search,
            sb.0 + 14,
            sb.1 + sb.3 / 2,
            11.5,
            icon_c,
        );

        let sb_text_x = sb.0 + 28;
        let sb_text_y = sb.1 + sb.3 / 2 + 4;
        let query = &state.search_input.text;
        if query.is_empty() {
            self.draw_ui_text(
                canvas,
                "Buscar ajustes...",
                sb_text_x,
                sb_text_y,
                sb.0 + sb.2.saturating_sub(8),
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );
        } else {
            let cursor_str = if is_searching && cursor_visible {
                "|"
            } else {
                ""
            };
            let disp = format!("{query}{cursor_str}");
            self.draw_ui_text(
                canvas,
                &disp,
                sb_text_x,
                sb_text_y,
                sb.0 + sb.2.saturating_sub(8),
                theme.foreground,
                tokens::BODY,
                false,
            );
        }

        Self::draw_rect(
            canvas,
            content_x,
            page_y + 84,
            card_w,
            1,
            theme.chrome_line(),
        );

        // 3. Navegación de secciones: pestañas planas con subrayado de acento.
        let sections = [
            (
                SettingsSection::Appearance,
                crate::ui::LucideIcon::Palette,
                "Apariencia",
                rects.nav_appearance,
            ),
            (
                SettingsSection::Terminal,
                crate::ui::LucideIcon::Terminal,
                "Terminal",
                rects.nav_terminal,
            ),
            (
                SettingsSection::Shortcuts,
                crate::ui::LucideIcon::Keyboard,
                "Atajos",
                rects.nav_shortcuts,
            ),
            (
                SettingsSection::About,
                crate::ui::LucideIcon::Info,
                "Acerca de",
                rects.nav_about,
            ),
        ];

        for (sec, icon, title, r) in sections {
            let is_active = state.section == sec;
            let is_hovered = state.hovered_section == Some(sec);
            let fg = if is_active {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let cy = r.1 + (r.3 / 2) - 1;
            self.draw_lucide_icon(
                canvas,
                icon,
                r.0 + 14,
                cy,
                12.0,
                if is_active { theme.accent_color() } else { fg },
            );
            self.draw_ui_text(
                canvas,
                title,
                r.0 + 28,
                cy + 4,
                r.0 + r.2,
                fg,
                tokens::BODY,
                is_active,
            );
            if is_active {
                Self::draw_rect(canvas, r.0, r.1 + r.3 - 2, r.2, 2, theme.accent_color());
            }
        }
        Self::draw_rect(
            canvas,
            content_x,
            rects.nav_appearance.1 + rects.nav_appearance.3,
            card_w,
            1,
            theme.chrome_line(),
        );

        // 4. Contenido: se dibuja en coordenadas locales (`width` x `top + content_h`) desplazadas por
        // el scroll y recortadas a la ventana visible; el fondo se pinta antes en esa ventana.
        let top = SettingsPageRects::CONTENT_TOP;
        let content_h = SettingsPageRects::content_height(state.section);
        let viewport_h = page_h.saturating_sub(top + status_bar_h);
        let scroll = state
            .scroll_y
            .min(SettingsPageRects::max_scroll(state.section, viewport_h));
        let tmp_h = top + content_h;
        let v = SettingsPageRects::compute(page_x, 0, page_w, page_h);
        let win_y = page_y + top;
        let win_end = (win_y + viewport_h.min(content_h - scroll)).min(height);
        let window = (page_x, win_y, page_x + page_w, win_end);
        Self::draw_rect(
            canvas,
            page_x,
            win_y,
            page_w,
            win_end.saturating_sub(win_y),
            theme.background,
        );
        canvas.with_offset(
            0,
            page_y as i32 - scroll as i32,
            (width, tmp_h),
            window,
            |c| match state.section {
                SettingsSection::Appearance => self.draw_settings_appearance(
                    c,
                    content_x,
                    card_w,
                    top,
                    config,
                    state,
                    cursor_visible,
                    &v,
                    theme,
                ),
                SettingsSection::Terminal => {
                    self.draw_settings_terminal(c, content_x, card_w, top, config, state, &v, theme)
                }
                SettingsSection::Shortcuts => {
                    self.draw_settings_shortcuts(c, content_x, card_w, top, theme)
                }
                SettingsSection::About => {
                    self.draw_settings_about(c, content_x, card_w, top, state, &v, theme)
                }
            },
        );

        if let Some((track, thumb)) = SettingsPageRects::scrollbar(
            state.section,
            page_y,
            width.saturating_sub(right_inset),
            viewport_h,
            scroll,
        ) {
            Self::draw_scrollbar_rects(
                canvas,
                track,
                thumb,
                state.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }
    }

    /// Sección «Apariencia»: temas, interfaz y tipografía.
    fn draw_settings_appearance(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        cursor_visible: bool,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        // Tarjeta 1: Temas
        self.draw_card(
            canvas,
            content_x,
            card_y,
            card_w,
            SettingsPageRects::THEME_CARD_H,
            "TEMA VISUAL",
            theme,
        );

        let slot_names = [
            Theme::canonical_name(&config.appearance.theme),
            Theme::canonical_name(&config.appearance.theme_light),
        ];
        for (i, ((rect, label), slot_name)) in [
            (rects.theme_slot_dark_btn, "Oscuro"),
            (rects.theme_slot_light_btn, "Claro"),
        ]
        .into_iter()
        .zip(slot_names)
        .enumerate()
        {
            self.draw_button(
                canvas,
                rect,
                &format!("{label} · {slot_name}"),
                state.hovered_theme_slot == Some(i),
                state.theme_slot_light == (i == 1),
                theme,
            );
        }

        // Buscador
        let sb = rects.theme_search_box;
        let searching = state.editing_setting == Some(EditingSettingField::ThemeSearch);
        let matches = &state.filtered_themes;
        canvas.rounded_fill(
            sb.0,
            sb.1,
            sb.2,
            sb.3,
            tokens::RADIUS_ROW,
            theme.mix(theme.background, theme.foreground, 0.05),
        );
        canvas.rounded_stroke(
            sb.0,
            sb.1,
            sb.2,
            sb.3,
            tokens::RADIUS_ROW,
            if searching {
                theme.accent_color()
            } else {
                theme.chrome_line()
            },
        );
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Search,
            sb.0 + 14,
            sb.1 + sb.3 / 2,
            12.0,
            if searching {
                theme.accent_color()
            } else {
                theme.text_muted()
            },
        );
        let count = format!("{}/{}", matches.len(), Theme::names().count());
        let count_w = self.measure_mono_text(&count, tokens::MICRO, false);
        let text_y = sb.1 + sb.3 / 2 + 4;
        self.draw_mono_text(
            canvas,
            &count,
            sb.0 + sb.2 - count_w - 12,
            text_y,
            sb.0 + sb.2,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );
        let text_start_x = sb.0 + 30;
        let text_max_x = sb.0 + sb.2 - count_w - 20;
        self.render_text_input_content(
            canvas,
            &state.theme_search_input,
            text_start_x,
            text_y,
            text_max_x,
            sb.1,
            sb.3,
            cursor_visible,
            "Buscar tema…",
            searching,
            theme,
        );

        // Lista desplazable de temas
        let current = slot_names[state.theme_slot_light as usize];
        let first = state
            .theme_scroll
            .min(matches.len().saturating_sub(SettingsPageRects::THEME_ROWS));
        if matches.is_empty() {
            let l = rects.theme_list;
            self.draw_ui_text(
                canvas,
                "Sin resultados",
                l.0 + 12,
                l.1 + 20,
                l.0 + l.2,
                theme.text_muted(),
                tokens::BODY,
                false,
            );
        }
        for (i, &name) in matches
            .iter()
            .skip(first)
            .take(SettingsPageRects::THEME_ROWS)
            .enumerate()
        {
            let r = rects.theme_row(i);
            let is_current = name == current;
            let is_hover = state.hovered_theme == Some(i);
            if is_current || is_hover {
                canvas.rounded_fill(
                    r.0,
                    r.1,
                    r.2,
                    r.3 - 2,
                    tokens::RADIUS_ROW,
                    if is_current {
                        theme.accent_soft(26)
                    } else {
                        theme.surface_hover()
                    },
                );
            }
            let fg = if is_current {
                theme.accent_color()
            } else if is_hover {
                theme.foreground
            } else {
                theme.text_secondary()
            };
            let preview = Theme::from_name(name);
            let chip_right = r.0 + r.2 - if is_current { 26 } else { 12 };
            let chips = [preview.background, preview.foreground]
                .into_iter()
                .chain(preview.ansi[1..=6].iter().copied());
            let chip_w = 10;
            let chips_x = chip_right - 8 * (chip_w + 3);
            for (k, c) in chips.enumerate() {
                let cx = chips_x + k * (chip_w + 3);
                canvas.rounded_fill(cx, r.1 + 8, chip_w, 14, 2, c);
                canvas.rounded_stroke(cx, r.1 + 8, chip_w, 14, 2, theme.chrome_line());
            }
            if is_current {
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Check,
                    r.0 + r.2 - 14,
                    r.1 + r.3 / 2,
                    12.0,
                    theme.accent_color(),
                );
            }
            self.draw_ui_text(
                canvas,
                name,
                r.0 + 12,
                r.1 + r.3 / 2 + 4,
                chips_x - 8,
                fg,
                tokens::BODY,
                is_current,
            );
        }
        if matches.len() > SettingsPageRects::THEME_ROWS {
            let l = rects.theme_list;
            let thumb_h = (l.3 * SettingsPageRects::THEME_ROWS / matches.len()).max(18);
            let max_first = matches.len() - SettingsPageRects::THEME_ROWS;
            let thumb_y = l.1 + first * (l.3 - thumb_h) / max_first;
            canvas.rounded_fill(l.0 + l.2 - 4, thumb_y, 3, thumb_h, 1, theme.border_color());
        }

        self.draw_ui_text(
            canvas,
            "Vista previa en vivo en toda la interfaz",
            content_x + 20,
            card_y + 370,
            content_x + card_w - 20,
            theme.text_muted(),
            tokens::DETAIL,
            false,
        );

        // Paleta completa del tema activo
        let swatch_y = card_y + SettingsPageRects::THEME_CARD_H - 34;
        let pad_x = 20;
        let avail_w = card_w.saturating_sub(pad_x * 2);
        let gap = 4;
        let swatch_w = (avail_w.saturating_sub(15 * gap)) / 16;
        let extra = (avail_w.saturating_sub(15 * gap)) % 16;
        let start_x = content_x + pad_x + extra / 2;
        for (i, &ansi_color) in theme.ansi.iter().take(16).enumerate() {
            let sx = start_x + i * (swatch_w + gap);
            canvas.rounded_fill(sx, swatch_y, swatch_w, 16, 2, ansi_color);
        }

        // Tarjeta 2: Interfaz (modo, disposición y opacidad)
        let card2_y = card_y + SettingsPageRects::THEME_CARD_H + 14;
        self.draw_card(
            canvas,
            content_x,
            card2_y,
            card_w,
            SettingsPageRects::INTERFACE_CARD_H,
            "INTERFAZ",
            theme,
        );

        self.draw_label(
            canvas,
            content_x + 20,
            rects.theme_mode_system_btn.1,
            content_x + 200,
            "Modo de color",
            theme,
        );
        let is_sys = config.appearance.theme_mode.eq_ignore_ascii_case("system");
        let is_dark = config.appearance.theme_mode.eq_ignore_ascii_case("dark");
        let is_light = config.appearance.theme_mode.eq_ignore_ascii_case("light");
        self.draw_button(
            canvas,
            rects.theme_mode_system_btn,
            "Sistema",
            state.hovered_theme_mode == Some(0),
            is_sys,
            theme,
        );
        self.draw_button(
            canvas,
            rects.theme_mode_dark_btn,
            "Oscuro",
            state.hovered_theme_mode == Some(1),
            is_dark,
            theme,
        );
        self.draw_button(
            canvas,
            rects.theme_mode_light_btn,
            "Claro",
            state.hovered_theme_mode == Some(2),
            is_light,
            theme,
        );

        self.draw_label(
            canvas,
            content_x + 20,
            rects.tab_layout_vertical_btn.1,
            content_x + 200,
            "Disposición de pestañas",
            theme,
        );
        let is_vert = !config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal");
        let is_horiz = !is_vert;
        self.draw_icon_button(
            canvas,
            rects.tab_layout_vertical_btn,
            crate::ui::LucideIcon::PanelLeft,
            "Vertical",
            state.hovered_tab_layout == Some(0),
            is_vert,
            theme,
        );
        self.draw_icon_button(
            canvas,
            rects.tab_layout_horizontal_btn,
            crate::ui::LucideIcon::Rows2,
            "Superior",
            state.hovered_tab_layout == Some(1),
            is_horiz,
            theme,
        );

        self.draw_label(
            canvas,
            content_x + 20,
            rects.panel_side_left_btn.1,
            content_x + 200,
            "Lado del panel",
            theme,
        );
        let on_right = config.appearance.panel_on_right();
        self.draw_icon_button(
            canvas,
            rects.panel_side_left_btn,
            crate::ui::LucideIcon::PanelLeft,
            "Izquierda",
            state.hovered_tab_layout == Some(2),
            !on_right,
            theme,
        );
        self.draw_icon_button(
            canvas,
            rects.panel_side_right_btn,
            crate::ui::LucideIcon::PanelRight,
            "Derecha",
            state.hovered_tab_layout == Some(3),
            on_right,
            theme,
        );

        self.draw_label(
            canvas,
            content_x + 20,
            rects.opacity_minus_btn.1,
            content_x + 200,
            "Opacidad de ventana",
            theme,
        );
        self.draw_button(
            canvas,
            rects.opacity_minus_btn,
            "−",
            state.hovered_opacity_minus,
            false,
            theme,
        );
        let is_editing_opacity = state.editing_setting == Some(EditingSettingField::WindowOpacity);
        let op_str = if is_editing_opacity {
            state.setting_input_buffer.clone()
        } else {
            format!("{:.0}%", (config.appearance.window_opacity * 100.0).round())
        };
        self.draw_stepper_value(
            canvas,
            rects.opacity_minus_btn,
            rects.opacity_plus_btn,
            &op_str,
            is_editing_opacity,
            state.hovered_opacity_box,
            theme,
        );
        self.draw_button(
            canvas,
            rects.opacity_plus_btn,
            "+",
            state.hovered_opacity_plus,
            false,
            theme,
        );

        // Tarjeta 3: Tipografía con muestra en vivo
        let card3_y = card2_y + SettingsPageRects::INTERFACE_CARD_H + 14;
        self.draw_card(
            canvas,
            content_x,
            card3_y,
            card_w,
            SettingsPageRects::TYPOGRAPHY_CARD_H,
            "TIPOGRAFÍA",
            theme,
        );

        let typography = [
            (
                rects.font_minus_btn,
                rects.font_plus_btn,
                "Tamaño de fuente",
                state.editing_setting == Some(EditingSettingField::FontSize),
                state.hovered_font_minus,
                state.hovered_font_plus,
                state.hovered_font_box,
                format!("{:.1} pt", config.appearance.font_size),
            ),
            (
                rects.spacing_minus_btn,
                rects.spacing_plus_btn,
                "Interlineado",
                state.editing_setting == Some(EditingSettingField::LineSpacing),
                state.hovered_spacing_minus,
                state.hovered_spacing_plus,
                state.hovered_spacing_box,
                format!("{:.1}", config.appearance.line_spacing),
            ),
        ];
        for (minus, plus, label, editing, h_minus, h_plus, h_box, value) in typography {
            self.draw_label(
                canvas,
                content_x + 20,
                minus.1,
                content_x + 230,
                label,
                theme,
            );
            self.draw_button(canvas, minus, "−", h_minus, false, theme);
            let text = if editing {
                state.setting_input_buffer.clone()
            } else {
                value
            };
            self.draw_stepper_value(canvas, minus, plus, &text, editing, h_box, theme);
            self.draw_button(canvas, plus, "+", h_plus, false, theme);
        }

        // Muestra: valores de máquina en mono con el tamaño de fuente configurado.
        let preview = (content_x + 20, card3_y + 140, card_w.saturating_sub(40), 66);
        canvas.rounded_fill(
            preview.0,
            preview.1,
            preview.2,
            preview.3,
            tokens::RADIUS_ROW,
            theme.mix(theme.background, theme.foreground, 0.04),
        );
        self.draw_mono_text(
            canvas,
            "Aa",
            preview.0 + 16,
            preview.1 + 42,
            preview.0 + 54,
            theme.accent_color(),
            self.font_size,
            false,
        );
        self.draw_mono_text(
            canvas,
            "fn main() -> Result<(), Error>",
            preview.0 + 56,
            preview.1 + 28,
            preview.0 + preview.2 - 12,
            theme.foreground,
            self.font_size,
            false,
        );
        self.draw_mono_text(
            canvas,
            &format!(
                "{} · {:.1} pt",
                if config.appearance.font_path.is_some() {
                    "Fuente personalizada"
                } else {
                    "JetBrains Mono"
                },
                config.appearance.font_size
            ),
            preview.0 + 56,
            preview.1 + 50,
            preview.0 + preview.2 - 12,
            theme.text_muted(),
            self.font_size,
            false,
        );
    }

    /// Sección «Terminal»: comportamiento agrupado y configuración en disco.
    fn draw_settings_terminal(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        self.draw_card(
            canvas,
            content_x,
            card_y,
            card_w,
            SettingsPageRects::TERMINAL_CARD_H,
            "COMPORTAMIENTO DEL TERMINAL",
            theme,
        );

        let label_x = content_x + 20;
        let label_max = content_x + card_w / 2 + 40;
        let on_off = |on: bool| if on { "Activado" } else { "Desactivado" };

        self.draw_group_label(canvas, label_x, card_y + 62, label_max, "ENTRADA", theme);
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.cursor_blink_btn,
            "Cursor parpadeante",
            config.terminal.cursor_blink,
            state.hovered_cursor_blink,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.copy_on_select_btn,
            "Copiar al seleccionar",
            config.terminal.copy_on_select,
            state.hovered_copy_on_select,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.word_wrap_btn,
            "Ajuste de línea en el visor",
            config.terminal.word_wrap,
            state.hovered_word_wrap,
            theme,
        );

        self.draw_group_label(canvas, label_x, card_y + 186, label_max, "SHELL", theme);
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.shell_integration_btn,
            "Integración de shell (OSC 133)",
            config.terminal.shell_integration,
            state.hovered_shell_integration,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.ssh_integration_btn,
            "Integración SSH (rama y carpeta)",
            config.terminal.ssh_integration,
            state.hovered_ssh_integration,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.ligatures_btn,
            "Ligaduras de código (->, !=, ==)",
            config.terminal.ligatures,
            state.hovered_ligatures,
            theme,
        );

        self.draw_group_label(canvas, label_x, card_y + 310, label_max, "INTERFAZ", theme);
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.status_bar_toggle_btn,
            "Barra inferior (Status Bar)",
            config.appearance.show_status_bar,
            state.hovered_status_bar,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.file_explorer_toggle_btn,
            "Explorador de archivos lateral",
            config.appearance.show_file_explorer,
            state.hovered_file_explorer,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.restore_session_btn,
            "Restaurar pestañas al iniciar",
            config.terminal.restore_session,
            state.hovered_restore_session,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.auto_save_btn,
            "Auto-guardar archivos editados",
            config.terminal.auto_save,
            state.hovered_auto_save,
            theme,
        );

        let _ = on_off;

        // Tarjeta 2: configuración en disco
        let cfg_y = card_y + SettingsPageRects::TERMINAL_CARD_H + 14;
        self.draw_card(
            canvas,
            content_x,
            cfg_y,
            card_w,
            SettingsPageRects::DISK_CARD_H,
            "CONFIGURACIÓN EN DISCO",
            theme,
        );
        self.draw_button(
            canvas,
            rects.open_file_btn,
            "Abrir tearminal.toml",
            state.hovered_open_file,
            false,
            theme,
        );
        self.draw_button(
            canvas,
            rects.term_reset_btn,
            "Restablecer por defecto",
            state.hovered_reset_defaults,
            false,
            theme,
        );
        let path = TearminalConfig::config_path();
        self.draw_mono_text(
            canvas,
            &path.to_string_lossy(),
            content_x + 20,
            cfg_y + 100,
            content_x + card_w - 20,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );
    }

    /// Sección «Atajos»: dos columnas con keycaps individuales.
    fn draw_settings_shortcuts(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        theme: &Theme,
    ) {
        self.draw_card(
            canvas,
            content_x,
            card_y,
            card_w,
            SettingsPageRects::SHORTCUTS_CARD_H,
            "ATAJOS DE TECLADO",
            theme,
        );

        let shortcuts = [
            ("Ctrl + T", "Nueva terminal o pestaña"),
            ("Ctrl + W", "Cerrar pestaña o panel activo"),
            ("Ctrl + Tab", "Siguiente pestaña (Shift: anterior)"),
            ("Ctrl + 1..9", "Ir a la pestaña 1–9"),
            ("Ctrl + B", "Mostrar u ocultar la barra lateral"),
            ("Ctrl + ,", "Abrir o cerrar Ajustes"),
            ("Ctrl + Shift + E", "Mostrar u ocultar el explorador"),
            ("Ctrl + Shift + C", "Copiar la selección"),
            ("Alt + Shift + D", "Dividir debajo"),
            ("Alt + Shift + V", "Dividir al lado"),
            ("Alt + Shift + W", "Cerrar panel dividido"),
            ("Alt + Shift + Flechas", "Mover el foco entre paneles"),
            ("Ctrl + V", "Pegar texto o imágenes"),
            ("Ctrl + Shift + K", "Limpiar pantalla y scrollback"),
            ("Ctrl + Shift + O", "Copiar salida del último comando"),
            ("Ctrl + Backspace", "Borrar palabra atrás"),
        ];

        let col_gap = 28;
        let col_w = (card_w.saturating_sub(40 + col_gap)) / 2;
        let start_y = card_y + 56;
        let row_h = 40;
        for (idx, (chord, desc)) in shortcuts.iter().enumerate() {
            let col = idx / 8;
            let row = idx % 8;
            let x = content_x + 20 + col * (col_w + col_gap);
            let y = start_y + row * row_h;
            let mut kx = x;
            let key_max = x + col_w;
            for (ki, key) in chord.split(" + ").enumerate() {
                if ki > 0 {
                    self.draw_mono_text(
                        canvas,
                        "+",
                        kx + 3,
                        y + 15,
                        key_max,
                        theme.text_muted(),
                        tokens::MICRO,
                        false,
                    );
                    kx += 12;
                }
                if kx >= key_max {
                    break;
                }
                let kw = self.draw_keycap(canvas, kx, y, key, theme);
                kx += kw + 4;
            }
            self.draw_ui_text(
                canvas,
                desc,
                x,
                y + 32,
                x + col_w,
                theme.text_secondary(),
                tokens::BODY,
                false,
            );
        }
    }

    /// Sección «Acerca de»: ficha editorial con datos reales.
    fn draw_settings_about(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        self.draw_card(
            canvas,
            content_x,
            card_y,
            card_w,
            SettingsPageRects::ABOUT_CARD_H,
            "ACERCA DE",
            theme,
        );

        let left = content_x + 20;
        let right = content_x + card_w - 20;
        self.draw_ui_text(
            canvas,
            "TEArminal",
            left,
            card_y + 76,
            right,
            theme.foreground,
            tokens::DISPLAY,
            true,
        );
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_w = self.measure_mono_text(&version, tokens::DETAIL, false);
        self.draw_mono_text(
            canvas,
            &version,
            right.saturating_sub(version_w),
            card_y + 74,
            right,
            theme.accent_color(),
            tokens::DETAIL,
            false,
        );
        self.draw_ui_text(
            canvas,
            "Terminal nativo para Windows escrito en Rust. Render por software y ConPTY.",
            left,
            card_y + 98,
            right,
            theme.text_secondary(),
            tokens::BODY,
            false,
        );

        Self::draw_rect(
            canvas,
            left,
            card_y + 114,
            card_w.saturating_sub(40),
            1,
            theme.chrome_line(),
        );

        let config_path = TearminalConfig::config_path();
        let themes = Theme::names().count().to_string();
        let data = [
            ("VERSIÓN", version.clone()),
            ("TEMAS", themes),
            ("MOTOR", "Rust · ConPTY".to_string()),
            ("PLATAFORMA", "Windows 10 / 11".to_string()),
            ("RENDER", "CPU + Direct3D 11".to_string()),
            ("CONFIGURACIÓN", config_path.to_string_lossy().to_string()),
        ];
        let col_w = (card_w.saturating_sub(56)) / 2;
        for (i, (label, value)) in data.iter().enumerate() {
            let col = i % 2;
            let row = i / 2;
            let x = left + col * (col_w + 16);
            let y = card_y + 136 + row * 42;
            self.draw_group_label(canvas, x, y, x + col_w, label, theme);
            self.draw_mono_text(
                canvas,
                value,
                x,
                y + 20,
                x + col_w,
                theme.foreground,
                tokens::DETAIL,
                false,
            );
        }

        if let Some(info) = crate::updater::get_available_update() {
            self.draw_mono_text(
                canvas,
                &format!("v{} disponible", info.latest_version),
                right.saturating_sub(120),
                card_y + 52,
                right,
                theme.accent_color(),
                tokens::MICRO,
                false,
            );
        }

        self.draw_button(
            canvas,
            rects.about_open_file_btn,
            "Abrir tearminal.toml",
            state.hovered_open_file,
            false,
            theme,
        );
        self.draw_button(
            canvas,
            rects.about_reset_btn,
            "Restablecer",
            state.hovered_reset_defaults,
            false,
            theme,
        );
    }
}
