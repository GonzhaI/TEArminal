//! Página de ajustes (pestaña dedicada).

use super::*;

impl TerminalRenderer {
    pub(super) fn draw_button(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        rect: (usize, usize, usize, usize),
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
            theme.mix(theme.background, theme.foreground, 0.06)
        };
        crate::ui::draw_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 4, bg);
        let b_border = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        crate::ui::stroke_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 4, b_border);

        let fg = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.text_secondary()
        };
        let text_w = self.measure_ui_text(text, 11.0, active_accent);
        let tx = bx + (bw.saturating_sub(text_w) / 2);
        let baseline = by + (bh / 2) + 4;
        self.draw_ui_text(
            buffer,
            buf_width,
            buf_height,
            text,
            tx,
            baseline,
            bx + bw,
            fg,
            11.0,
            active_accent,
        );
    }

    pub(super) fn draw_stepper_value(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        minus_rect: (usize, usize, usize, usize),
        plus_rect: (usize, usize, usize, usize),
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
            theme.mix(theme.background, theme.foreground, 0.06)
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, span_start, box_y, span_w, box_h, 4, bg,
        );

        let border_c = if is_editing {
            theme.accent_color()
        } else if is_hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        crate::ui::stroke_rounded_rect(
            buffer, width, height, span_start, box_y, span_w, box_h, 4, border_c,
        );

        let disp_text = if is_editing {
            format!("{}|", text)
        } else {
            text.to_string()
        };

        let text_w = self.measure_ui_text(&disp_text, 11.5, true);
        let tx = span_start + (span_w.saturating_sub(text_w) / 2);
        let baseline = box_y + (box_h / 2) + 4;
        let fg = if is_editing {
            theme.foreground
        } else {
            theme.accent_color()
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &disp_text,
            tx,
            baseline,
            span_end.saturating_sub(2),
            fg,
            11.5,
            true,
        );
    }

    pub(super) fn draw_label(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        x: usize,
        y: usize,
        max_x: usize,
        label: &str,
        theme: &Theme,
    ) {
        let baseline = y + 18;
        self.draw_ui_text(
            buffer,
            buf_width,
            buf_height,
            label,
            x,
            baseline,
            max_x,
            theme.foreground,
            11.5,
            false,
        );
    }

    pub(super) fn draw_card(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        title: &str,
        theme: &Theme,
    ) {
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 6, theme.surface_bg());
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 6, theme.chrome_line());

        // Barra superior de encabezado de la tarjeta
        Self::draw_rect(buffer, width, height, x, y + 30, w, 1, theme.chrome_line());
        let baseline = y + 20;
        self.draw_ui_text_tracked(
            buffer,
            width,
            height,
            title,
            x + 16,
            baseline,
            x + w,
            theme.text_muted(),
            9.5,
            true,
            1.1,
        );
    }

    /// Renderiza la página completa de la pestaña de Ajustes y Personalización de Axon
    pub fn render_settings_page(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        page_x: usize,
        page_y: usize,
        status_bar_h: usize,
        config: &AxonConfig,
        state: &SettingsPageState,
        theme: &Theme,
    ) {
        if width <= page_x || height <= page_y {
            return;
        }

        let page_w = width - page_x;
        let page_h = height - page_y;
        let rects = SettingsPageRects::compute(page_x, page_y, page_w, page_h);

        // 1. Fondo de la página de ajustes a partir de page_y (para no sobreescribir barra superior)
        Self::draw_rect(
            buffer,
            width,
            height,
            page_x,
            page_y,
            page_w,
            page_h,
            theme.background,
        );

        // 2. Encabezado: título y una línea de contexto. Sin iconos decorativos.
        let content_x = rects.content_x;
        let card_w = rects.card_w;

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ajustes",
            content_x,
            page_y + 42,
            content_x + card_w.saturating_sub(215),
            theme.foreground,
            15.0,
            true,
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Apariencia, terminal y atajos de teclado",
            content_x,
            page_y + 61,
            content_x + card_w.saturating_sub(215),
            theme.text_muted(),
            10.5,
            false,
        );

        // Botón Restablecer valores por defecto en el encabezado
        let r_btn = rects.reset_defaults_btn;
        if state.hovered_reset_defaults {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                r_btn.0,
                r_btn.1,
                r_btn.2,
                r_btn.3,
                3,
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
        let r_label_w = self.measure_ui_text(r_label, 10.5, false);
        let r_text_x = r_btn.0 + r_btn.2.saturating_sub(r_label_w + 8);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::RefreshCw,
            r_text_x.saturating_sub(12),
            r_cy,
            10.5,
            r_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            r_label,
            r_text_x,
            r_cy + 4,
            r_btn.0 + r_btn.2,
            r_fg,
            10.5,
            false,
        );

        // Línea divisoria superior
        Self::draw_rect(
            buffer,
            width,
            height,
            content_x,
            page_y + 72,
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
                "Atajos de teclado",
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
                buffer,
                width,
                height,
                icon,
                r.0 + 14,
                cy,
                11.5,
                if is_active { theme.accent_color() } else { fg },
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                title,
                r.0 + 26,
                cy + 4,
                r.0 + r.2,
                fg,
                11.0,
                is_active,
            );
            if is_active {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    r.0,
                    r.1 + r.3 - 2,
                    r.2,
                    2,
                    theme.accent_color(),
                );
            }
        }

        // Línea divisoria bajo navegación
        Self::draw_rect(
            buffer,
            width,
            height,
            content_x,
            rects.nav_appearance.1 + rects.nav_appearance.3,
            card_w,
            1,
            theme.chrome_line(),
        );

        // 4. Contenido de la sección activa: se dibuja completo fuera de pantalla (coordenadas de
        //    página, page_y = 0) y se copia solo la ventana visible según el scroll.
        let top = SettingsPageRects::CONTENT_TOP;
        let content_h = SettingsPageRects::content_height(state.section);
        let viewport_h = page_h.saturating_sub(top + status_bar_h);
        let scroll = state
            .scroll_y
            .min(SettingsPageRects::max_scroll(state.section, viewport_h));
        let tmp_h = top + content_h;
        let mut tmp = std::mem::take(&mut self.settings_scratch);
        tmp.resize(width * tmp_h, 0);
        tmp.fill(theme.background.to_u32_xrgb());
        let v = SettingsPageRects::compute(page_x, 0, page_w, page_h);
        match state.section {
            SettingsSection::Appearance => self.draw_settings_appearance(
                &mut tmp, width, tmp_h, content_x, card_w, top, config, state, &v, theme,
            ),
            SettingsSection::Terminal => self.draw_settings_terminal(
                &mut tmp, width, tmp_h, content_x, card_w, top, config, state, &v, theme,
            ),
            SettingsSection::Shortcuts => {
                self.draw_settings_shortcuts(&mut tmp, width, tmp_h, content_x, card_w, top, theme)
            }
            SettingsSection::About => self.draw_settings_about(
                &mut tmp, width, tmp_h, content_x, card_w, top, state, &v, theme,
            ),
        }
        let rows = viewport_h.min(content_h - scroll);
        for row in 0..rows {
            let dst_y = page_y + top + row;
            if dst_y >= height {
                break;
            }
            let src = (top + scroll + row) * width + page_x;
            let dst = dst_y * width + page_x;
            buffer[dst..dst + page_w].copy_from_slice(&tmp[src..src + page_w]);
        }
        self.settings_scratch = tmp;

        if let Some((track, thumb)) =
            SettingsPageRects::scrollbar(state.section, page_y, width, viewport_h, scroll)
        {
            Self::draw_scrollbar_rects(
                buffer,
                width,
                height,
                track,
                thumb,
                state.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }
    }

    /// Sección «Appearance» de la página de ajustes.
    fn draw_settings_appearance(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        config: &AxonConfig,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        // Tarjeta 1: Temas
        let th_card_h = SettingsPageRects::THEME_CARD_H;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            card_y,
            card_w,
            th_card_h,
            "TEMA VISUAL DE COLOR",
            theme,
        );

        // Ranuras: el tema oscuro y el claro se eligen por separado (el modo decide cuál se usa)
        let slot_names = [
            Theme::canonical_name(&config.appearance.theme),
            Theme::canonical_name(&config.appearance.theme_light),
        ];
        for (i, (rect, label)) in [
            (rects.theme_slot_dark_btn, "Oscuro"),
            (rects.theme_slot_light_btn, "Claro"),
        ]
        .into_iter()
        .enumerate()
        {
            self.draw_button(
                buffer,
                width,
                height,
                rect,
                &format!("{label}: {}", slot_names[i]),
                state.hovered_theme_slot == Some(i),
                state.theme_slot_light == (i == 1),
                theme,
            );
        }

        // Buscador
        let sb = rects.theme_search_box;
        let searching = state.editing_setting == Some(EditingSettingField::ThemeSearch);
        let matches = &state.filtered_themes;
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            sb.0,
            sb.1,
            sb.2,
            sb.3,
            4,
            theme.mix(theme.background, theme.foreground, 0.06),
        );
        let search_border = if searching {
            theme.accent_color()
        } else {
            theme.chrome_line()
        };
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            sb.0,
            sb.1,
            sb.2,
            sb.3,
            4,
            search_border,
        );
        let count = format!("{}/{}", matches.len(), Theme::names().count());
        let count_w = self.measure_ui_text(&count, 11.0, false);
        let text_y = sb.1 + sb.3 / 2 + 4;
        self.draw_ui_text(
            buffer,
            width,
            height,
            &count,
            sb.0 + sb.2 - count_w - 12,
            text_y,
            sb.0 + sb.2,
            theme.text_muted(),
            11.0,
            false,
        );
        let (query, query_color) = if state.theme_filter.is_empty() && !searching {
            ("Buscar tema...".to_string(), theme.text_muted())
        } else {
            (
                format!("{}{}", state.theme_filter, if searching { "|" } else { "" }),
                theme.foreground,
            )
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &query,
            sb.0 + 12,
            text_y,
            sb.0 + sb.2 - count_w - 20,
            query_color,
            11.0,
            false,
        );

        // Lista desplazable: el tema activo se resalta y cada fila muestra una vista previa de su paleta
        let current = slot_names[state.theme_slot_light as usize];
        let first = state
            .theme_scroll
            .min(matches.len().saturating_sub(SettingsPageRects::THEME_ROWS));
        if matches.is_empty() {
            let l = rects.theme_list;
            self.draw_ui_text(
                buffer,
                width,
                height,
                "Sin resultados",
                l.0 + 12,
                l.1 + 18,
                l.0 + l.2,
                theme.text_muted(),
                11.0,
                false,
            );
        }
        for i in 0..SettingsPageRects::THEME_ROWS {
            let Some(&name) = matches.get(first + i) else {
                break;
            };
            let r = rects.theme_row(i);
            let is_current = name == current;
            let is_hover = state.hovered_theme == Some(i);
            if is_current || is_hover {
                let bg = if is_current {
                    theme.accent_soft(26)
                } else {
                    theme.surface_hover()
                };
                crate::ui::draw_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3 - 2, 3, bg);
            }
            let fg = if is_current {
                theme.accent_color()
            } else if is_hover {
                theme.foreground
            } else {
                theme.text_secondary()
            };
            let preview = Theme::from_name(name);
            let chips = [preview.background, preview.foreground]
                .into_iter()
                .chain(preview.ansi[1..=6].iter().copied());
            let chip_w = 10;
            let chips_x = r.0 + r.2 - 14 - 8 * (chip_w + 3);
            for (k, c) in chips.enumerate() {
                let cx = chips_x + k * (chip_w + 3);
                crate::ui::draw_rounded_rect(buffer, width, height, cx, r.1 + 6, chip_w, 12, 2, c);
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    cx,
                    r.1 + 6,
                    chip_w,
                    12,
                    2,
                    theme.chrome_line(),
                );
            }
            self.draw_ui_text(
                buffer,
                width,
                height,
                name,
                r.0 + 12,
                r.1 + r.3 / 2 + 3,
                chips_x - 8,
                fg,
                11.0,
                is_current,
            );
        }
        if matches.len() > SettingsPageRects::THEME_ROWS {
            let l = rects.theme_list;
            let thumb_h = (l.3 * SettingsPageRects::THEME_ROWS / matches.len()).max(18);
            let max_first = matches.len() - SettingsPageRects::THEME_ROWS;
            let thumb_y = l.1 + first * (l.3 - thumb_h) / max_first;
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                l.0 + l.2 - 5,
                thumb_y,
                3,
                thumb_h,
                1,
                theme.border_color(),
            );
        }

        // Paleta de chips de color del tema activo (ocupa todo el ancho de la tarjeta)
        let swatch_y = card_y + SettingsPageRects::THEME_CARD_H - 28;
        let swatch_h = 18;
        let pad_x = 20;
        let avail_w = card_w.saturating_sub(pad_x * 2);
        let gap = 4;
        let total_gaps = 15 * gap;
        let swatch_w = (avail_w.saturating_sub(total_gaps)) / 16;
        let extra = (avail_w.saturating_sub(total_gaps)) % 16;
        let start_x = content_x + pad_x + extra / 2;
        for i in 0..16 {
            let sx = start_x + i * (swatch_w + gap);
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                sx,
                swatch_y,
                swatch_w,
                swatch_h,
                2,
                theme.ansi[i],
            );
        }

        // Tarjeta 2: Modo de Tema y Disposición de Pestañas
        let card2_y = card_y + SettingsPageRects::THEME_CARD_H + 10;
        let card2_h = 114;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            card2_y,
            card_w,
            card2_h,
            "MODO Y DISPOSICIÓN DE PESTAÑAS",
            theme,
        );

        // Modo de tema (system, dark, light)
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.theme_mode_system_btn.1,
            content_x + 175,
            "Modo de color:",
            theme,
        );
        let is_sys = config.appearance.theme_mode.eq_ignore_ascii_case("system");
        let is_dark = config.appearance.theme_mode.eq_ignore_ascii_case("dark");
        let is_light = config.appearance.theme_mode.eq_ignore_ascii_case("light");

        self.draw_button(
            buffer,
            width,
            height,
            rects.theme_mode_system_btn,
            "Sistema (Auto)",
            state.hovered_theme_mode == Some(0),
            is_sys,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.theme_mode_dark_btn,
            "Oscuro",
            state.hovered_theme_mode == Some(1),
            is_dark,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.theme_mode_light_btn,
            "Claro",
            state.hovered_theme_mode == Some(2),
            is_light,
            theme,
        );

        // Disposición de pestañas (vertical, horizontal)
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.tab_layout_vertical_btn.1,
            content_x + 175,
            "Pestañas:",
            theme,
        );
        let is_vert = !config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal");
        let is_horiz = config
            .appearance
            .tab_layout
            .eq_ignore_ascii_case("horizontal");

        self.draw_button(
            buffer,
            width,
            height,
            rects.tab_layout_vertical_btn,
            "Vertical (Sidebar)",
            state.hovered_tab_layout == Some(0),
            is_vert,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.tab_layout_horizontal_btn,
            "Superior (Tabs)",
            state.hovered_tab_layout == Some(1),
            is_horiz,
            theme,
        );

        // Tarjeta 3: Tipografía y Ventana
        let typo_y = card2_y + card2_h + 14;
        let typo_h = 190;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            typo_y,
            card_w,
            typo_h,
            "TIPOGRAFÍA Y VENTANA",
            theme,
        );

        // Tamaño de fuente de terminal
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.font_minus_btn.1,
            content_x + 230,
            "Fuente de terminal:",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.font_minus_btn,
            "-",
            state.hovered_font_minus,
            false,
            theme,
        );
        let is_editing_font = state.editing_setting == Some(EditingSettingField::FontSize);
        let font_str = if is_editing_font {
            state.setting_input_buffer.clone()
        } else {
            format!("{:.1} pt", config.appearance.font_size)
        };
        self.draw_stepper_value(
            buffer,
            width,
            height,
            rects.font_minus_btn,
            rects.font_plus_btn,
            &font_str,
            is_editing_font,
            state.hovered_font_box,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.font_plus_btn,
            "+",
            state.hovered_font_plus,
            false,
            theme,
        );

        // Tamaño de fuente de interfaz (UI)
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.ui_font_minus_btn.1,
            content_x + 230,
            "Fuente de interfaz (UI):",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.ui_font_minus_btn,
            "-",
            state.hovered_ui_font_minus,
            false,
            theme,
        );
        let is_editing_ui_font = state.editing_setting == Some(EditingSettingField::UiFontSize);
        let ui_font_str = if is_editing_ui_font {
            state.setting_input_buffer.clone()
        } else {
            format!("{:.1} pt", config.appearance.ui_font_size)
        };
        self.draw_stepper_value(
            buffer,
            width,
            height,
            rects.ui_font_minus_btn,
            rects.ui_font_plus_btn,
            &ui_font_str,
            is_editing_ui_font,
            state.hovered_ui_font_box,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.ui_font_plus_btn,
            "+",
            state.hovered_ui_font_plus,
            false,
            theme,
        );

        // Interlineado
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.spacing_minus_btn.1,
            content_x + 230,
            "Interlineado (Spacing):",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.spacing_minus_btn,
            "-",
            state.hovered_spacing_minus,
            false,
            theme,
        );
        let is_editing_spacing = state.editing_setting == Some(EditingSettingField::LineSpacing);
        let sp_str = if is_editing_spacing {
            state.setting_input_buffer.clone()
        } else {
            format!("{:.1}", config.appearance.line_spacing)
        };
        self.draw_stepper_value(
            buffer,
            width,
            height,
            rects.spacing_minus_btn,
            rects.spacing_plus_btn,
            &sp_str,
            is_editing_spacing,
            state.hovered_spacing_box,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.spacing_plus_btn,
            "+",
            state.hovered_spacing_plus,
            false,
            theme,
        );

        // Opacidad
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.opacity_minus_btn.1,
            content_x + 230,
            "Opacidad de ventana:",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.opacity_minus_btn,
            "-",
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
            buffer,
            width,
            height,
            rects.opacity_minus_btn,
            rects.opacity_plus_btn,
            &op_str,
            is_editing_opacity,
            state.hovered_opacity_box,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.opacity_plus_btn,
            "+",
            state.hovered_opacity_plus,
            false,
            theme,
        );
    }

    /// Sección «Terminal» de la página de ajustes.
    fn draw_settings_terminal(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        config: &AxonConfig,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        // Tarjeta 1: Comportamiento
        let term_card_h = SettingsPageRects::TERMINAL_CARD_H;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            card_y,
            card_w,
            term_card_h,
            "COMPORTAMIENTO DEL TERMINAL",
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.cursor_blink_btn.1,
            content_x + 250,
            "Cursor parpadeante:",
            theme,
        );
        let blink_str = if config.terminal.cursor_blink {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.cursor_blink_btn,
            blink_str,
            state.hovered_cursor_blink,
            config.terminal.cursor_blink,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.shell_integration_btn.1,
            content_x + 250,
            "Integración de shell (OSC 133):",
            theme,
        );
        let prompt_str = if config.terminal.shell_integration {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.shell_integration_btn,
            prompt_str,
            state.hovered_shell_integration,
            config.terminal.shell_integration,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.copy_on_select_btn.1,
            content_x + 250,
            "Copiar texto al seleccionar:",
            theme,
        );
        let copy_str = if config.terminal.copy_on_select {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.copy_on_select_btn,
            copy_str,
            state.hovered_copy_on_select,
            config.terminal.copy_on_select,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.status_bar_toggle_btn.1,
            content_x + 250,
            "Barra inferior (Status Bar):",
            theme,
        );
        let status_str = if config.appearance.show_status_bar {
            "Visible"
        } else {
            "Oculta"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.status_bar_toggle_btn,
            status_str,
            state.hovered_status_bar,
            config.appearance.show_status_bar,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.file_explorer_toggle_btn.1,
            content_x + 250,
            "Explorador de archivos lateral:",
            theme,
        );
        let explorer_str = if config.appearance.show_file_explorer {
            "Visible"
        } else {
            "Oculto"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.file_explorer_toggle_btn,
            explorer_str,
            state.hovered_file_explorer,
            config.appearance.show_file_explorer,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.restore_session_btn.1,
            content_x + 250,
            "Restaurar pestañas al iniciar:",
            theme,
        );
        let restore_str = if config.terminal.restore_session {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.restore_session_btn,
            restore_str,
            state.hovered_restore_session,
            config.terminal.restore_session,
            theme,
        );

        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.auto_save_btn.1,
            content_x + 250,
            "Auto-guardar archivos editados:",
            theme,
        );
        let auto_save_str = if config.terminal.auto_save {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.auto_save_btn,
            auto_save_str,
            state.hovered_auto_save,
            config.terminal.auto_save,
            theme,
        );

        let on_off = |on: bool| if on { "Activado" } else { "Desactivado" };
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.word_wrap_btn.1,
            content_x + 250,
            "Ajuste de línea en el visor:",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.word_wrap_btn,
            on_off(config.terminal.word_wrap),
            state.hovered_word_wrap,
            config.terminal.word_wrap,
            theme,
        );
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.ssh_integration_btn.1,
            content_x + 250,
            "Integración SSH (rama y carpeta):",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.ssh_integration_btn,
            on_off(config.terminal.ssh_integration),
            state.hovered_ssh_integration,
            config.terminal.ssh_integration,
            theme,
        );
        self.draw_label(
            buffer,
            width,
            height,
            content_x + 20,
            rects.ligatures_btn.1,
            content_x + 250,
            "Ligaduras de código (->, !=, ==):",
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.ligatures_btn,
            on_off(config.terminal.ligatures),
            state.hovered_ligatures,
            config.terminal.ligatures,
            theme,
        );

        // Tarjeta 2: Configuración en disco
        let cfg_y = card_y + term_card_h + 14;
        let cfg_h = 110;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            cfg_y,
            card_w,
            cfg_h,
            "CONFIGURACIÓN EN DISCO",
            theme,
        );

        self.draw_button(
            buffer,
            width,
            height,
            rects.open_file_btn,
            "Abrir axon.toml en editor",
            state.hovered_open_file,
            state.hovered_open_file,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.term_reset_btn,
            "Restablecer por defecto",
            state.hovered_reset_defaults,
            false,
            theme,
        );
        let hint_base = rects.open_file_btn.1 + rects.open_file_btn.3 + 18;
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ruta: %LOCALAPPDATA%\\axon\\axon.toml (se recarga automáticamente)",
            content_x + 20,
            hint_base,
            content_x + card_w,
            theme.text_muted(),
            11.0,
            false,
        );
    }

    /// Sección «Shortcuts» de la página de ajustes.
    fn draw_settings_shortcuts(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        theme: &Theme,
    ) {
        let sh_h = 434;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            card_y,
            card_w,
            sh_h,
            "ATAJOS DE TECLADO CANÓNICOS (Unificados)",
            theme,
        );

        let shortcuts = [
            ("Ctrl + T", "Nueva terminal / pestaña"),
            ("Ctrl + W", "Cerrar pestaña o terminal activa"),
            (
                "Ctrl + Tab",
                "Conmutar a la siguiente pestaña (Shift: anterior)",
            ),
            ("Ctrl + 1..9", "Cambiar directamente a pestaña 1-9"),
            (
                "Alt + Shift + D",
                "Dividir terminal horizontalmente (debajo)",
            ),
            (
                "Alt + Shift + V",
                "Dividir terminal verticalmente (al lado)",
            ),
            ("Alt + Shift + W", "Cerrar panel dividido activo"),
            (
                "Alt + Shift + Flechas",
                "Navegar foco entre paneles divididos",
            ),
            ("Ctrl + B", "Mostrar / Ocultar barra lateral"),
            (
                "Ctrl + Shift + E",
                "Mostrar / Ocultar explorador de archivos lateral",
            ),
            ("Ctrl + ,", "Abrir o cerrar pestaña de Ajustes"),
            (
                "Ctrl + V",
                "Pegar portapapeles (texto plano o imágenes IA / SSH)",
            ),
            (
                "Ctrl + Shift + C",
                "Copiar texto seleccionado al portapapeles",
            ),
            ("Ctrl + Shift + K", "Limpiar pantalla y scrollback buffer"),
            (
                "Ctrl + Shift + O",
                "Copiar salida del último comando del agente",
            ),
            ("Ctrl + Backspace", "Borrar palabra atrás (0x17 / werase)"),
        ];

        let start_y = card_y + 40;
        for (idx, (chord, desc)) in shortcuts.iter().enumerate() {
            let y = start_y + idx * 24;
            let chord_w = 170;
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                content_x + 20,
                y,
                chord_w,
                20,
                4,
                theme.sidebar_bg(),
            );
            crate::ui::stroke_rounded_rect(
                buffer,
                width,
                height,
                content_x + 20,
                y,
                chord_w,
                20,
                4,
                theme.border_color(),
            );
            let baseline = y + 14;
            self.draw_ui_text(
                buffer,
                width,
                height,
                chord,
                content_x + 28,
                baseline,
                content_x + 20 + chord_w,
                theme.accent_color(),
                11.0,
                true,
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                desc,
                content_x + 204,
                baseline,
                content_x + card_w.saturating_sub(10),
                theme.foreground,
                11.5,
                false,
            );
        }
    }

    /// Sección «About» de la página de ajustes.
    fn draw_settings_about(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        content_x: usize,
        card_w: usize,
        card_y: usize,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        let ab_h = 280;
        self.draw_card(
            buffer,
            width,
            height,
            content_x,
            card_y,
            card_w,
            ab_h,
            "ACERCA DE AXON TERMINAL",
            theme,
        );

        let start_y = card_y + 42;
        let version_title = format!("Axon Terminal  v{}", env!("CARGO_PKG_VERSION"));
        self.draw_ui_text(
            buffer,
            width,
            height,
            &version_title,
            content_x + 20,
            start_y + 16,
            content_x + card_w,
            theme.foreground,
            14.0,
            true,
        );

        if let Some(info) = crate::updater::get_available_update() {
            let note = format!(
                "• ¡Nueva versión v{} disponible! Haz clic en la barra o menú para actualizar",
                info.latest_version
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                &note,
                content_x + 200,
                start_y + 16,
                content_x + card_w,
                theme.accent_color(),
                11.0,
                true,
            );
        }

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Emulador de terminal nativo de alto rendimiento para Windows en Rust.",
            content_x + 20,
            start_y + 36,
            content_x + card_w,
            theme.text_muted(),
            11.5,
            false,
        );

        let features = [
            "• Zero-runtime web: Renderizado softbuffer + rasterizador fontdue de alta fidelidad.",
            "• Windows ConPTY asíncrono con multihilo y anclaje de prompt para agentes CLI.",
            "• Protocolo iTerm2 / OSC 1337 para renderizado inline de imágenes PNG/JPG.",
            "• Integración completa para sesiones remotas SSH (clonación de splits y SCP).",
            "• Portapapeles remoto bidireccional mediante OSC 52 en base64.",
            "• 819 temas de iterm2-color-schemes, con sincronización de tema del sistema.",
            "• Barra de estado inferior informativa con rama Git y detección de agentes de IA.",
        ];

        for (idx, feat) in features.iter().enumerate() {
            let fy = start_y + 54 + idx * 22;
            let f_base = fy + 14;
            self.draw_ui_text(
                buffer,
                width,
                height,
                feat,
                content_x + 20,
                f_base,
                content_x + card_w,
                theme.text_secondary(),
                11.0,
                false,
            );
        }

        self.draw_button(
            buffer,
            width,
            height,
            rects.open_file_btn,
            "Abrir axon.toml en editor",
            state.hovered_open_file,
            state.hovered_open_file,
            theme,
        );
        self.draw_button(
            buffer,
            width,
            height,
            rects.term_reset_btn,
            "Restablecer por defecto",
            state.hovered_reset_defaults,
            false,
            theme,
        );
    }
}
