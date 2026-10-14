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
        let bg = if active_accent || hovered {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 7, bg);
        let b_border = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.ansi[8]
        } else {
            theme.border_color()
        };
        crate::ui::stroke_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 7, b_border);

        let fg = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.ansi[7]
        };
        let text_w = self.measure_ui_text(text, 11.5, active_accent);
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
            11.5,
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
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, span_start, box_y, span_w, box_h, 6, bg,
        );

        let border_c = if is_editing {
            theme.accent_color()
        } else if is_hovered {
            theme.ansi[8]
        } else {
            theme.border_color()
        };
        crate::ui::stroke_rounded_rect(
            buffer, width, height, span_start, box_y, span_w, box_h, 6, border_c,
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
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.surface_bg());
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.border_color());

        // Barra superior de encabezado de la tarjeta
        Self::draw_rect(buffer, width, height, x, y + 30, w, 1, theme.border_color());
        let baseline = y + 20;
        self.draw_ui_text(
            buffer,
            width,
            height,
            title,
            x + 14,
            baseline,
            x + w,
            theme.ansi[8],
            11.0,
            true,
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

        // 2. Encabezado principal con icono Lucide Sliders y tipografía Inter
        let content_x = rects.nav_appearance.0;
        let card_w = 680.min(page_w.saturating_sub(72));

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Sliders,
            content_x + 8,
            page_y + 38,
            15.0,
            theme.accent_color(),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ajustes de Axon",
            content_x + 24,
            page_y + 43,
            content_x + card_w.saturating_sub(215),
            theme.foreground,
            14.5,
            true,
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Personaliza apariencia, comportamiento del terminal y atajos de teclado",
            content_x,
            page_y + 62,
            content_x + card_w.saturating_sub(215),
            theme.ansi[8],
            11.5,
            false,
        );

        // Botón Restablecer valores por defecto en el encabezado
        let r_btn = rects.reset_defaults_btn;
        let r_bg = if state.hovered_reset_defaults {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, r_btn.0, r_btn.1, r_btn.2, r_btn.3, 6, r_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            r_btn.0,
            r_btn.1,
            r_btn.2,
            r_btn.3,
            6,
            theme.border_color(),
        );
        let r_fg = if state.hovered_reset_defaults {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        let r_cy = r_btn.1 + (r_btn.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::RefreshCw,
            r_btn.0 + 16,
            r_cy,
            12.0,
            r_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Restablecer por defecto",
            r_btn.0 + 28,
            r_cy + 4,
            r_btn.0 + r_btn.2,
            r_fg,
            11.5,
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
            theme.border_color(),
        );

        // 3. Barra de navegación de secciones (Pills redondeadas con iconos Lucide e Inter)
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
                "Atajos de Teclado",
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
            let bg = if is_active {
                theme.surface_bg()
            } else if is_hovered {
                theme.surface_hover()
            } else {
                theme.sidebar_bg()
            };
            crate::ui::draw_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 8, bg);

            let border_c = if is_active {
                theme.accent_color()
            } else {
                theme.border_color()
            };
            crate::ui::stroke_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 8, border_c);

            let fg = if is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };
            let cy = r.1 + (r.3 / 2);
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                icon,
                r.0 + 16,
                cy,
                12.5,
                if is_active { theme.accent_color() } else { fg },
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                title,
                r.0 + 28,
                cy + 4,
                r.0 + r.2,
                fg,
                11.5,
                is_active,
            );
        }

        // Línea divisoria bajo navegación
        Self::draw_rect(
            buffer,
            width,
            height,
            content_x,
            page_y + 116,
            card_w,
            1,
            theme.border_color(),
        );

        // 4. Renderizar contenido según la sección activa
        let card_y = page_y + 128;

        match state.section {
            SettingsSection::Appearance => self.draw_settings_appearance(
                buffer, width, height, content_x, card_w, card_y, config, state, &rects, theme,
            ),
            SettingsSection::Terminal => self.draw_settings_terminal(
                buffer, width, height, content_x, card_w, card_y, config, state, &rects, theme,
            ),
            SettingsSection::Shortcuts => self
                .draw_settings_shortcuts(buffer, width, height, content_x, card_w, card_y, theme),
            SettingsSection::About => self.draw_settings_about(
                buffer, width, height, content_x, card_w, card_y, state, &rects, theme,
            ),
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
        let th_card_h = 150;
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

        for (idx, &theme_name) in Theme::AVAILABLE_THEMES.iter().enumerate() {
            let r = rects.theme_buttons[idx];
            let is_current = Theme::is_same_theme(&config.appearance.theme, theme_name);
            let is_hover = state.hovered_theme == Some(idx);
            let btn_bg = if is_current || is_hover {
                theme.surface_hover()
            } else {
                theme.surface_bg()
            };
            crate::ui::draw_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 7, btn_bg);

            let border_c = if is_current {
                theme.accent_color()
            } else if is_hover {
                theme.ansi[8]
            } else {
                theme.border_color()
            };
            crate::ui::stroke_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 7, border_c);

            let fg = if is_current {
                theme.accent_color()
            } else if is_hover {
                theme.foreground
            } else {
                theme.ansi[7]
            };

            let display_name = if is_current {
                format!("● {}", theme_name)
            } else {
                theme_name.to_string()
            };
            let cy = r.1 + (r.3 / 2);
            let base_y = cy + 4;
            let text_w = self.measure_ui_text(&display_name, 11.0, is_current);
            let tx = r.0 + (r.2.saturating_sub(text_w) / 2);
            self.draw_ui_text(
                buffer,
                width,
                height,
                &display_name,
                tx,
                base_y,
                r.0 + r.2,
                fg,
                11.0,
                is_current,
            );
        }

        // Paleta de chips de color del tema activo
        let swatch_y = card_y + 116;
        let swatch_w = 26;
        let swatch_h = 18;
        let swatch_start_x = content_x + 20;
        for i in 0..16 {
            let sx = swatch_start_x + i * (swatch_w + 4);
            if sx + swatch_w < content_x + card_w {
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    sx,
                    swatch_y,
                    swatch_w,
                    swatch_h,
                    4,
                    theme.ansi[i],
                );
            }
        }

        // Tarjeta 2: Modo de Tema y Disposición de Pestañas
        let card2_y = card_y + 160;
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
        let term_card_h = 278;
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
            rects.ghostty_prompt_btn.1,
            content_x + 250,
            "Prompt Ghostty nativo (OSC 133):",
            theme,
        );
        let prompt_str = if config.terminal.ghostty_prompt {
            "Activado"
        } else {
            "Desactivado"
        };
        self.draw_button(
            buffer,
            width,
            height,
            rects.ghostty_prompt_btn,
            prompt_str,
            state.hovered_ghostty_prompt,
            config.terminal.ghostty_prompt,
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
            theme.ansi[8],
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
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon Terminal",
            content_x + 20,
            start_y + 16,
            content_x + card_w,
            theme.foreground,
            14.0,
            true,
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Emulador de terminal nativo de alto rendimiento para Windows en Rust.",
            content_x + 20,
            start_y + 36,
            content_x + card_w,
            theme.ansi[8],
            11.5,
            false,
        );

        let features = [
            "• Zero-runtime web: Renderizado softbuffer + rasterizador fontdue de alta fidelidad.",
            "• Windows ConPTY asíncrono con multihilo y anclaje de prompt para agentes CLI.",
            "• Protocolo iTerm2 / OSC 1337 para renderizado inline de imágenes PNG/JPG.",
            "• Integración completa para sesiones remotas SSH (clonación de splits y SCP).",
            "• Portapapeles remoto bidireccional mediante OSC 52 en base64.",
            "• Temas oficiales Catppuccin Frappé y Latte, con sincronización de tema del sistema.",
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
                theme.ansi[7],
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
