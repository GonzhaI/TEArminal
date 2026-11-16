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
        let baseline = self.centered_baseline(by + bh / 2);
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
        let baseline = self.centered_baseline(box_y + box_h / 2);
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_stepper_row(
        &mut self,
        canvas: &mut Canvas,
        label_x: usize,
        label_max: usize,
        label: &str,
        minus_rect: Rect,
        plus_rect: Rect,
        val_str: &str,
        is_editing: bool,
        hovered_minus: bool,
        hovered_plus: bool,
        hovered_box: bool,
        theme: &Theme,
    ) {
        self.draw_label(canvas, label_x, minus_rect.1, label_max, label, theme);
        self.draw_button(canvas, minus_rect, "−", hovered_minus, false, theme);
        self.draw_stepper_value(
            canvas,
            minus_rect,
            plus_rect,
            val_str,
            is_editing,
            hovered_box,
            theme,
        );
        self.draw_button(canvas, plus_rect, "+", hovered_plus, false, theme);
    }

    pub(super) fn draw_segmented_group(
        &mut self,
        canvas: &mut Canvas,
        label_x: usize,
        label_max: usize,
        label: &str,
        buttons: &[(Rect, &str, bool, bool)],
        theme: &Theme,
    ) {
        if let Some((first_rect, _, _, _)) = buttons.first() {
            self.draw_label(canvas, label_x, first_rect.1, label_max, label, theme);
        }
        for &(rect, btn_text, hovered, active) in buttons {
            self.draw_button(canvas, rect, btn_text, hovered, active, theme);
        }
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
            self.centered_baseline(y + 15),
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
            self.centered_baseline(y + 20),
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
        let baseline = self.centered_baseline(cy);
        self.draw_ui_text(
            canvas,
            label,
            label_x,
            baseline,
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
            baseline,
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

    /// Control de interruptor (switch deslizante estilo Screen 14/15 de 40×20 con pastilla de 14×14).
    pub(super) fn draw_switch_control(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        checked: bool,
        hovered: bool,
        theme: &Theme,
    ) {
        let (bx, by, bw, bh) = rect;
        let bg = if checked {
            theme.accent_color()
        } else if hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
        let border = if checked {
            theme.accent_color()
        } else if hovered {
            theme.chrome_line_strong()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_ROW, border);

        let knob_w = 14;
        let knob_h = 14;
        let knob_y = by + (bh.saturating_sub(knob_h)) / 2;
        let knob_x = if checked {
            bx + bw.saturating_sub(knob_w + 3)
        } else {
            bx + 3
        };
        let knob_c = if checked {
            theme.background
        } else {
            theme.text_secondary()
        };
        canvas.rounded_fill(knob_x, knob_y, knob_w, knob_h, tokens::RADIUS_ROW, knob_c);
    }

    /// Fila de ajuste con interruptor switch, título, descripción y línea inferior.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_settings_row_switch(
        &mut self,
        canvas: &mut Canvas,
        col_x: usize,
        col_w: usize,
        y: usize,
        label: &str,
        hint: &str,
        switch_rect: Rect,
        checked: bool,
        hovered: bool,
        theme: &Theme,
    ) {
        let row_h = 50;
        let label_max = col_x + col_w.saturating_sub(54);
        let baseline = self.centered_baseline(y + 16);
        self.draw_ui_text(
            canvas,
            label,
            col_x,
            baseline,
            label_max,
            if hovered {
                theme.foreground
            } else {
                theme.text_secondary()
            },
            tokens::BODY,
            false,
        );
        let hint_baseline = self.centered_baseline(y + 34);
        self.draw_ui_text(
            canvas,
            hint,
            col_x,
            hint_baseline,
            label_max,
            theme.text_muted(),
            tokens::DETAIL,
            false,
        );

        self.draw_switch_control(canvas, switch_rect, checked, hovered, theme);

        Self::draw_rect(canvas, col_x, y + row_h - 1, col_w, 1, theme.chrome_line());
    }

    /// Fila de ajuste con selector numérico stepper [-] [valor] [+], título, descripción y línea inferior.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_settings_row_stepper(
        &mut self,
        canvas: &mut Canvas,
        col_x: usize,
        col_w: usize,
        y: usize,
        label: &str,
        hint: &str,
        minus_rect: Rect,
        val_rect: Rect,
        plus_rect: Rect,
        value_str: &str,
        hovered_minus: bool,
        hovered_plus: bool,
        theme: &Theme,
    ) {
        let row_h = 50;
        let label_max = col_x + col_w.saturating_sub(106);
        let baseline = self.centered_baseline(y + 16);
        self.draw_ui_text(
            canvas,
            label,
            col_x,
            baseline,
            label_max,
            theme.foreground,
            tokens::BODY,
            false,
        );
        let hint_baseline = self.centered_baseline(y + 34);
        self.draw_ui_text(
            canvas,
            hint,
            col_x,
            hint_baseline,
            label_max,
            theme.text_muted(),
            tokens::DETAIL,
            false,
        );

        self.draw_button(canvas, minus_rect, "−", hovered_minus, false, theme);

        let (vx, vy, vw, vh) = val_rect;
        canvas.rounded_fill(
            vx,
            vy,
            vw,
            vh,
            tokens::RADIUS_ROW,
            theme.mix(theme.background, theme.foreground, 0.05),
        );
        canvas.rounded_stroke(vx, vy, vw, vh, tokens::RADIUS_ROW, theme.chrome_line());
        let val_w = self.measure_mono_text(value_str, tokens::DETAIL, false);
        let tx = vx + (vw.saturating_sub(val_w)) / 2;
        let v_baseline = self.centered_baseline(vy + vh / 2);
        self.draw_mono_text(
            canvas,
            value_str,
            tx,
            v_baseline,
            vx + vw.saturating_sub(2),
            theme.foreground,
            tokens::DETAIL,
            false,
        );

        self.draw_button(canvas, plus_rect, "+", hovered_plus, false, theme);

        Self::draw_rect(canvas, col_x, y + row_h - 1, col_w, 1, theme.chrome_line());
    }

    /// Fila de ajuste con botón de valor monoespaciado (p. ej. ruta de carpeta de certificados).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_settings_row_value(
        &mut self,
        canvas: &mut Canvas,
        col_x: usize,
        col_w: usize,
        y: usize,
        label: &str,
        hint: &str,
        btn_rect: Rect,
        value_str: &str,
        hovered: bool,
        theme: &Theme,
    ) {
        let row_h = 50;
        let label_max = col_x + col_w.saturating_sub(btn_rect.2 + 10);
        let baseline = self.centered_baseline(y + 16);
        self.draw_ui_text(
            canvas,
            label,
            col_x,
            baseline,
            label_max,
            if hovered {
                theme.foreground
            } else {
                theme.text_secondary()
            },
            tokens::BODY,
            false,
        );
        let hint_baseline = self.centered_baseline(y + 34);
        self.draw_ui_text(
            canvas,
            hint,
            col_x,
            hint_baseline,
            label_max,
            theme.text_muted(),
            tokens::DETAIL,
            false,
        );

        let (bx, by, bw, bh) = btn_rect;
        let bg = if hovered {
            theme.surface_hover()
        } else {
            theme.mix(theme.background, theme.foreground, 0.05)
        };
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
        canvas.rounded_stroke(
            bx,
            by,
            bw,
            bh,
            tokens::RADIUS_ROW,
            if hovered {
                theme.chrome_line_strong()
            } else {
                theme.chrome_line()
            },
        );
        let val_w = self.measure_mono_text(value_str, tokens::DETAIL, false);
        let tx = bx + (bw.saturating_sub(val_w)) / 2;
        let v_baseline = self.centered_baseline(by + bh / 2);
        self.draw_mono_text(
            canvas,
            value_str,
            tx,
            v_baseline,
            bx + bw.saturating_sub(4),
            theme.foreground,
            tokens::DETAIL,
            false,
        );

        Self::draw_rect(canvas, col_x, y + row_h - 1, col_w, 1, theme.chrome_line());
    }

    /// Caja de aviso informativo estilo Screen 15 ("Todo es local: sin nube ni telemetría").
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_info_notice(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        text: &str,
        theme: &Theme,
    ) {
        canvas.rounded_fill(x, y, w, h, tokens::RADIUS_ROW, theme.accent_soft(20));
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_ROW, theme.accent_soft(90));
        let cy = y + h / 2;
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Info,
            x + 16,
            cy,
            14.0,
            theme.accent_color(),
        );
        let baseline = self.centered_baseline(cy);
        self.draw_ui_text(
            canvas,
            text,
            x + 34,
            baseline,
            x + w.saturating_sub(12),
            theme.foreground,
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
        _status_bar_h: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width <= page_x + right_inset || height <= page_y {
            return;
        }

        let progress = self.modal_anim_progress.unwrap_or(1.0).clamp(0.0, 1.0);
        let offset_y = if progress < 1.0 {
            ((1.0 - progress) * -14.0).round() as isize
        } else {
            0
        };
        let page_y = (page_y as isize + offset_y).max(0) as usize;
        let page_w = width - page_x - right_inset;
        let page_h = height.saturating_sub(page_y);
        let rects = SettingsPageRects::compute_modal(page_x, page_y, page_w, page_h);

        // 1. Fondo del modal con esquinas redondeadas; la columna de navegación se extiende bajo el contenido
        //    (que la tapa) para que solo las esquinas izquierdas sean suyas.
        let radius = tokens::RADIUS_CARD;
        let nav_w = SettingsPageRects::NAV_W;
        canvas.rounded_fill(page_x, page_y, page_w, page_h, radius, theme.background);
        canvas.rounded_fill(
            page_x,
            page_y,
            nav_w + radius,
            page_h,
            radius,
            theme.chrome_bg(),
        );
        canvas.rounded_fill(
            page_x + nav_w,
            page_y,
            page_w - nav_w,
            page_h,
            radius,
            theme.background,
        );
        Self::draw_rect(
            canvas,
            page_x + nav_w - 1,
            page_y + radius,
            1,
            page_h.saturating_sub(2 * radius),
            theme.chrome_line(),
        );

        // 2. Título en la columna de navegación; restablecer y buscar encabezan el contenido.
        let content_x = rects.content_x;
        let card_w = rects.card_w;
        self.draw_ui_text(
            canvas,
            "Ajustes",
            page_x + 24,
            page_y + 46,
            page_x + nav_w - 16,
            theme.foreground,
            tokens::DISPLAY,
            true,
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
        let r_content_w = 22 + r_label_w;
        let r_start_x = r_btn.0 + r_btn.2.saturating_sub(r_content_w) / 2;
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::RefreshCw,
            r_start_x + 7,
            r_cy,
            11.0,
            r_fg,
        );
        self.draw_ui_text(
            canvas,
            r_label,
            r_start_x + 22,
            self.centered_baseline(r_cy),
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
        let sb_text_y = self.centered_baseline(sb.1 + sb.3 / 2);
        let sb_text_max_x = sb.0 + sb.2.saturating_sub(8);
        self.render_text_input_content(
            canvas,
            &state.search_input,
            sb_text_x,
            sb_text_y,
            sb_text_max_x,
            sb.1,
            sb.3,
            cursor_visible,
            "Buscar ajustes...",
            is_searching,
            theme,
        );

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
                SettingsSection::Database,
                crate::ui::LucideIcon::Database,
                "Bases de datos",
                rects.nav_database,
            ),
            (
                SettingsSection::Performance,
                crate::ui::LucideIcon::Sliders,
                "Rendimiento",
                rects.nav_performance,
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
            if is_active {
                canvas.rounded_fill(r.0, r.1, r.2, r.3, tokens::RADIUS_ROW, theme.surface_bg());
            } else if is_hovered {
                canvas.rounded_fill(
                    r.0,
                    r.1,
                    r.2,
                    r.3,
                    tokens::RADIUS_ROW,
                    theme.surface_hover(),
                );
            }
            let fg = if is_active {
                theme.foreground
            } else if is_hovered {
                theme.text_secondary()
            } else {
                theme.text_muted()
            };
            let cy = r.1 + r.3 / 2;
            self.draw_lucide_icon(
                canvas,
                icon,
                r.0 + 16,
                cy,
                13.0,
                if is_active { theme.accent_color() } else { fg },
            );
            self.draw_ui_text(
                canvas,
                title,
                r.0 + 34,
                self.centered_baseline(cy),
                r.0 + r.2 - 8,
                fg,
                tokens::BODY,
                is_active,
            );
        }

        // 4. Contenido: se dibuja en coordenadas locales (`width` x `top + content_h`) desplazadas por
        // el scroll y recortadas a la ventana visible; el fondo se pinta antes en esa ventana.
        let top = SettingsPageRects::CONTENT_TOP;
        let bottom_pad = 16;
        let content_h = SettingsPageRects::content_height(state.section);
        let viewport_h = page_h.saturating_sub(top + bottom_pad);
        let scroll = state
            .scroll_y
            .min(SettingsPageRects::max_scroll(state.section, viewport_h));
        let tmp_h = top + content_h;
        let v = SettingsPageRects::compute_modal(page_x, 0, page_w, page_h);
        let win_y = page_y + top;
        let win_end =
            (page_y + top + viewport_h.min(content_h - scroll)).min(page_y + page_h - bottom_pad);
        let window = canvas.real_clip_of((page_x + nav_w, win_y, page_x + page_w, win_end));
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
                SettingsSection::Database => {
                    self.draw_settings_database(c, content_x, card_w, top, config, state, &v, theme)
                }
                SettingsSection::Performance => self.draw_settings_performance(
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
                SettingsSection::Shortcuts => self
                    .draw_settings_shortcuts(c, content_x, card_w, top, config, state, &v, theme),
                SettingsSection::About => {
                    self.draw_settings_about(c, content_x, card_w, top, state, &v, theme)
                }
            },
        );

        if let Some((track, thumb)) = SettingsPageRects::scrollbar(
            state.section,
            page_y,
            page_x + page_w - 4,
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
        canvas.rounded_stroke(
            page_x,
            page_y,
            page_w,
            page_h,
            tokens::RADIUS_CARD,
            theme.card_outline(),
        );
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
        .zip(&slot_names)
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
        let count = format!("{}/{}", matches.len(), Theme::all_count());
        let count_w = self.measure_mono_text(&count, tokens::MICRO, false);
        let text_y = self.centered_baseline(sb.1 + sb.3 / 2);
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
        let current = slot_names[usize::from(state.theme_slot_light)].as_ref();
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
        for (i, name) in matches
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
                canvas.rounded_fill(cx, r.1 + 8, chip_w, 14, tokens::RADIUS_ROW, c);
                canvas.rounded_stroke(
                    cx,
                    r.1 + 8,
                    chip_w,
                    14,
                    tokens::RADIUS_ROW,
                    theme.chrome_line(),
                );
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
                self.centered_baseline(r.1 + r.3 / 2),
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
            canvas.rounded_fill(
                l.0 + l.2 - 4,
                thumb_y,
                3,
                thumb_h,
                tokens::RADIUS_ROW,
                theme.border_color(),
            );
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
            canvas.rounded_fill(sx, swatch_y, swatch_w, 16, tokens::RADIUS_ROW, ansi_color);
        }

        // Tarjeta 2: Interfaz (modo y opacidad)
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

        let label_max_x = rects.theme_mode_system_btn.0.saturating_sub(12);
        let is_sys = config.appearance.theme_mode.eq_ignore_ascii_case("system");
        let is_dark = config.appearance.theme_mode.eq_ignore_ascii_case("dark");
        let is_light = config.appearance.theme_mode.eq_ignore_ascii_case("light");
        self.draw_segmented_group(
            canvas,
            content_x + 20,
            label_max_x,
            "Modo de color",
            &[
                (
                    rects.theme_mode_system_btn,
                    "Sistema",
                    state.hovered_theme_mode == Some(0),
                    is_sys,
                ),
                (
                    rects.theme_mode_dark_btn,
                    "Oscuro",
                    state.hovered_theme_mode == Some(1),
                    is_dark,
                ),
                (
                    rects.theme_mode_light_btn,
                    "Claro",
                    state.hovered_theme_mode == Some(2),
                    is_light,
                ),
            ],
            theme,
        );

        let is_editing_opacity = state.editing_setting == Some(EditingSettingField::WindowOpacity);
        let op_str = if is_editing_opacity {
            state.setting_input_buffer.clone()
        } else {
            format!("{:.0}%", (config.appearance.window_opacity * 100.0).round())
        };
        self.draw_stepper_row(
            canvas,
            content_x + 20,
            label_max_x,
            "Opacidad de ventana",
            rects.opacity_minus_btn,
            rects.opacity_plus_btn,
            &op_str,
            is_editing_opacity,
            state.hovered_opacity_minus,
            state.hovered_opacity_plus,
            state.hovered_opacity_box,
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
                rects.ui_font_minus_btn,
                rects.ui_font_plus_btn,
                "Tamaño de fuente UI",
                state.editing_setting == Some(EditingSettingField::UiFontSize),
                state.hovered_ui_font_minus,
                state.hovered_ui_font_plus,
                state.hovered_ui_font_box,
                format!("{:.1} pt", config.appearance.ui_font_size),
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
                minus.0.saturating_sub(12),
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

        // Familia tipográfica: ◀ nombre ▶ entre las fuentes monoespaciadas instaladas
        {
            let (prev, next) = (rects.font_family_prev_btn, rects.font_family_next_btn);
            self.draw_label(
                canvas,
                content_x + 20,
                prev.1,
                prev.0.saturating_sub(12),
                "Familia tipográfica",
                theme,
            );
            self.draw_button(
                canvas,
                prev,
                "‹",
                state.hovered_font_family_prev,
                false,
                theme,
            );
            let name = if config.appearance.font_path.is_some() {
                config.appearance.font_family.as_str()
            } else {
                "Predeterminada (JetBrains Mono)"
            };
            self.draw_stepper_value(canvas, prev, next, name, false, false, theme);
            self.draw_button(
                canvas,
                next,
                "›",
                state.hovered_font_family_next,
                false,
                theme,
            );
        }

        // Muestra: valores de máquina en mono con el tamaño de fuente configurado.
        let preview = (content_x + 20, card3_y + 224, card_w.saturating_sub(40), 66);
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
            preview.0 + 18,
            self.centered_baseline(preview.1 + preview.3 / 2),
            preview.0 + 54,
            theme.accent_text(),
            self.font_size,
            true,
        );
        self.draw_mono_text(
            canvas,
            "fn main() -> Result<(), Error>",
            preview.0 + 56,
            self.centered_baseline(preview.1 + 22),
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
                    config.appearance.font_family.as_str()
                } else {
                    "JetBrains Mono"
                },
                config.appearance.font_size
            ),
            preview.0 + 56,
            self.centered_baseline(preview.1 + 44),
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
        let label_max = rects.cursor_blink_btn.0.saturating_sub(12);

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

        let cs = config.terminal.cursor_shape.to_ascii_lowercase();
        let is_beam = cs == "beam" || cs == "bar" || cs == "line";
        let is_under = cs == "underline";
        let is_block = !is_beam && !is_under;
        self.draw_segmented_group(
            canvas,
            label_x,
            label_max,
            "Forma del cursor",
            &[
                (
                    rects.cursor_shape_block_btn,
                    "Bloque",
                    state.hovered_cursor_shape == Some(0),
                    is_block,
                ),
                (
                    rects.cursor_shape_beam_btn,
                    "Barra",
                    state.hovered_cursor_shape == Some(1),
                    is_beam,
                ),
                (
                    rects.cursor_shape_underline_btn,
                    "Subrayado",
                    state.hovered_cursor_shape == Some(2),
                    is_under,
                ),
            ],
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

        self.draw_group_label(canvas, label_x, card_y + 220, label_max, "SHELL", theme);
        let sh = config.terminal.default_shell.to_ascii_lowercase();
        let is_pwsh = sh.contains("pwsh");
        let is_cmd = sh.ends_with("cmd.exe") || sh == "cmd";
        let is_winps = !is_pwsh && !is_cmd;
        self.draw_segmented_group(
            canvas,
            label_x,
            label_max,
            "Shell por defecto",
            &[
                (
                    rects.shell_pwsh_btn,
                    "PowerShell 7",
                    state.hovered_default_shell == Some(0),
                    is_pwsh,
                ),
                (
                    rects.shell_powershell_btn,
                    "PowerShell",
                    state.hovered_default_shell == Some(1),
                    is_winps,
                ),
                (
                    rects.shell_cmd_btn,
                    "CMD",
                    state.hovered_default_shell == Some(2),
                    is_cmd,
                ),
            ],
            theme,
        );

        let is_editing_sb = state.editing_setting == Some(EditingSettingField::ScrollbackLines);
        let sb_str = if is_editing_sb {
            state.setting_input_buffer.clone()
        } else {
            format!("{}", config.terminal.scrollback_lines)
        };
        self.draw_stepper_row(
            canvas,
            label_x,
            label_max,
            "Historial (scrollback)",
            rects.scrollback_minus_btn,
            rects.scrollback_plus_btn,
            &sb_str,
            is_editing_sb,
            state.hovered_scrollback_minus,
            state.hovered_scrollback_plus,
            state.hovered_scrollback_box,
            theme,
        );

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
            rects.osc52_btn,
            "Portapapeles remoto (OSC 52)",
            config.terminal.osc52,
            state.hovered_osc52,
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

        self.draw_group_label(canvas, label_x, card_y + 446, label_max, "INTERFAZ", theme);
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
            rects.show_dimensions_btn,
            "Dimensiones de la rejilla (cols × filas)",
            config.appearance.show_dimensions,
            state.hovered_show_dimensions,
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
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.ai_limits_btn,
            "Consultar cuotas de IA (Claude Code)",
            config.terminal.ai_limits,
            state.hovered_ai_limits,
            theme,
        );
        self.draw_toggle_row(
            canvas,
            label_x,
            label_max,
            rects.auto_check_updates_btn,
            "Buscar actualizaciones al iniciar",
            config.terminal.auto_check_updates,
            state.hovered_auto_check_updates,
            theme,
        );

        self.draw_label(
            canvas,
            label_x,
            rects.keep_awake_off_btn.1,
            label_max,
            "Pantalla encendida",
            theme,
        );
        let kw = config.terminal.keep_awake;
        self.draw_button(
            canvas,
            rects.keep_awake_off_btn,
            "Apagado",
            state.hovered_keep_awake == Some(0),
            kw == crate::config::KeepAwakeMode::Off,
            theme,
        );
        self.draw_button(
            canvas,
            rects.keep_awake_agent_btn,
            "Agente",
            state.hovered_keep_awake == Some(1),
            kw == crate::config::KeepAwakeMode::Agent,
            theme,
        );
        self.draw_button(
            canvas,
            rects.keep_awake_active_btn,
            "Activo",
            state.hovered_keep_awake == Some(2),
            kw == crate::config::KeepAwakeMode::Active,
            theme,
        );

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

    /// Sección «Bases de datos» (Screen 15: seguridad en PROD, consultas, privacidad y certificados).
    #[allow(clippy::too_many_arguments)]
    fn draw_settings_database(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        top: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        let col_gap = 28;
        let col_w = (card_w.saturating_sub(col_gap)) / 2;
        let col1_x = content_x;
        let col2_x = content_x + col_w + col_gap;
        let db_base_y = top + 36;
        let db = &config.database;

        // Columna 1 Header: SEGURIDAD EN PRODUCCIÓN
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Lock,
            col1_x + 8,
            top + 14,
            14.0,
            theme.ansi[1],
        );
        self.draw_group_label(
            canvas,
            col1_x + 22,
            top + 6,
            col1_x + col_w,
            "SEGURIDAD EN PRODUCCIÓN",
            theme,
        );
        Self::draw_rect(canvas, col1_x, top + 28, col_w, 1, theme.chrome_line());

        // Columna 1 Rows
        let r1 = |i: usize| db_base_y + i * 50;
        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(0),
            "Abrir conexiones PROD en solo lectura",
            "La escritura se habilita por pestaña y caduca sola.",
            rects.db_prod_read_only_btn,
            db.prod_read_only,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::ProdReadOnly),
            theme,
        );

        let dur_str = format!("{} min", db.write_window_minutes);
        self.draw_settings_row_stepper(
            canvas,
            col1_x,
            col_w,
            r1(1),
            "Duración de la escritura habilitada",
            "Después vuelve a solo lectura.",
            rects.db_write_window_minus_btn,
            rects.db_write_window_value_box,
            rects.db_write_window_plus_btn,
            &dur_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::WriteWindowMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::WriteWindowPlus),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(2),
            "Pedir confirmación escrita",
            "Hay que escribir el nombre de la conexión para aplicar.",
            rects.db_written_confirmation_btn,
            db.written_confirmation,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::WrittenConfirmation),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(3),
            "Avisar de DELETE y UPDATE sin WHERE",
            "Muestra las filas afectadas antes de ejecutar.",
            rects.db_warn_no_where_btn,
            db.warn_no_where,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::WarnNoWhere),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(4),
            "Aplicar cambios dentro de una transacción",
            "Si una sentencia falla, se revierte todo.",
            rects.db_enforce_transaction_btn,
            db.enforce_transaction,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::EnforceTransaction),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(5),
            "Guardar las filas originales antes de aplicar",
            "Archivo local para poder deshacer.",
            rects.db_backup_original_rows_btn,
            db.backup_original_rows,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::BackupOriginalRows),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(6),
            "Avisar si una clave .pem es legible por otros",
            "Comprueba los permisos de Windows al abrir la conexión.",
            rects.db_warn_unsafe_pem_permissions_btn,
            db.warn_unsafe_pem_permissions,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::WarnUnsafePem),
            theme,
        );

        let cert_str = format!("{} días", db.cert_warn_days);
        self.draw_settings_row_stepper(
            canvas,
            col1_x,
            col_w,
            r1(7),
            "Aviso de certificados por vencer",
            "Con esta antelación al vencimiento.",
            rects.db_cert_warn_days_minus_btn,
            rects.db_cert_warn_days_value_box,
            rects.db_cert_warn_days_plus_btn,
            &cert_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::CertWarnDaysMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::CertWarnDaysPlus),
            theme,
        );

        let folder_str = if db.certs_folder.is_empty() {
            "…\\certs"
        } else {
            &db.certs_folder
        };
        self.draw_settings_row_value(
            canvas,
            col1_x,
            col_w,
            r1(8),
            "Carpeta de certificados .pem",
            "Solo se guarda la ruta, nunca el contenido.",
            rects.db_certs_folder_btn,
            folder_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::CertsFolder),
            theme,
        );

        // Subheader: ETIQUETAS DE ENTORNO
        let env_y = r1(8) + 60;
        self.draw_group_label(
            canvas,
            col1_x,
            env_y,
            col1_x + col_w,
            "ETIQUETAS DE ENTORNO",
            theme,
        );
        let tags = [
            ("LOCAL", theme.ansi[2]),
            ("STG", theme.ansi[3]),
            ("PROD", theme.ansi[1]),
        ];
        let mut cur_tag_x = col1_x;
        for (name, color) in tags {
            let tag_w = 76;
            let tag_h = 28;
            canvas.rounded_fill(
                cur_tag_x,
                env_y + 22,
                tag_w,
                tag_h,
                tokens::RADIUS_ROW,
                theme.mix(theme.background, theme.foreground, 0.05),
            );
            canvas.rounded_stroke(
                cur_tag_x,
                env_y + 22,
                tag_w,
                tag_h,
                tokens::RADIUS_ROW,
                theme.chrome_line(),
            );
            canvas.rounded_fill(
                cur_tag_x + 10,
                env_y + 31,
                10,
                10,
                tokens::RADIUS_ROW,
                color,
            );
            let baseline = self.centered_baseline(env_y + 22 + tag_h / 2);
            self.draw_ui_text(
                canvas,
                name,
                cur_tag_x + 26,
                baseline,
                cur_tag_x + tag_w,
                theme.foreground,
                tokens::DETAIL,
                true,
            );
            cur_tag_x += tag_w + 10;
        }

        // Columna 2 Header: CONSULTAS
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Database,
            col2_x + 8,
            top + 14,
            14.0,
            theme.accent_color(),
        );
        self.draw_group_label(
            canvas,
            col2_x + 22,
            top + 6,
            col2_x + col_w,
            "CONSULTAS",
            theme,
        );
        Self::draw_rect(canvas, col2_x, top + 28, col_w, 1, theme.chrome_line());

        let r2 = |i: usize| db_base_y + i * 50;
        let page_str = format!("{}", db.page_size);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(0),
            "Filas por página",
            "Vista de datos de tablas.",
            rects.db_page_size_minus_btn,
            rects.db_page_size_value_box,
            rects.db_page_size_plus_btn,
            &page_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::PageSizeMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::PageSizePlus),
            theme,
        );

        let timeout_str = format!("{} s", db.query_timeout_seconds);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(1),
            "Tiempo máximo de consulta",
            "Cancela al superarlo.",
            rects.db_query_timeout_minus_btn,
            rects.db_query_timeout_value_box,
            rects.db_query_timeout_plus_btn,
            &timeout_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::QueryTimeoutMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::QueryTimeoutPlus),
            theme,
        );

        let limit_str = format!("{}", db.query_row_limit);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(2),
            "Límite de filas devueltas",
            "Evita cargar resultados enormes.",
            rects.db_query_row_limit_minus_btn,
            rects.db_query_row_limit_value_box,
            rects.db_query_row_limit_plus_btn,
            &limit_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::QueryRowLimitMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::QueryRowLimitPlus),
            theme,
        );

        let slow_str = format!("{} ms", db.slow_query_threshold_ms);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(3),
            "Avisar de consultas lentas desde",
            "Muestra un aviso y la marca LENTA.",
            rects.db_slow_query_threshold_minus_btn,
            rects.db_slow_query_threshold_value_box,
            rects.db_slow_query_threshold_plus_btn,
            &slow_str,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::SlowQueryThresholdMinus),
            state.hovered_db == Some(crate::renderer::HoveredDbControl::SlowQueryThresholdPlus),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col2_x,
            col_w,
            r2(4),
            "Autocompletado de tablas y columnas",
            "Usa el esquema leído de la conexión.",
            rects.db_autocomplete_btn,
            db.autocomplete,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::Autocomplete),
            theme,
        );

        // Columna 2 Header: CREDENCIALES Y PRIVACIDAD
        let cred_y = r2(4) + 60;
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Lock,
            col2_x + 8,
            cred_y + 8,
            14.0,
            theme.ansi[3],
        );
        self.draw_group_label(
            canvas,
            col2_x + 22,
            cred_y,
            col2_x + col_w,
            "CREDENCIALES Y PRIVACIDAD",
            theme,
        );
        Self::draw_rect(canvas, col2_x, cred_y + 22, col_w, 1, theme.chrome_line());

        let cred_r = |i: usize| cred_y + 30 + i * 50;
        self.draw_settings_row_switch(
            canvas,
            col2_x,
            col_w,
            cred_r(0),
            "Guardar contraseñas en el Administrador de Windows",
            "Nunca en tearminal.toml ni en el historial.",
            rects.db_save_passwords_btn,
            db.save_passwords_in_credential_manager,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::SavePasswords),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col2_x,
            col_w,
            cred_r(1),
            "Guardar historial de consultas",
            "Solo texto y duración, sin valores de parámetros.",
            rects.db_save_query_history_btn,
            db.save_query_history,
            state.hovered_db == Some(crate::renderer::HoveredDbControl::SaveQueryHistory),
            theme,
        );

        // Info Notice Card al pie
        let notice_y = cred_r(1) + 66;
        self.draw_info_notice(
            canvas,
            col2_x,
            notice_y,
            col_w,
            46,
            "Todo es local: sin nube ni telemetría. Conexión directa o por túnel SSH.",
            theme,
        );
    }

    /// Sección «Rendimiento» (Screen 14 / Brief F8: medidores barra de estado, muestreo y alertas).
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn draw_settings_performance(
        &mut self,
        canvas: &mut Canvas,
        content_x: usize,
        card_w: usize,
        top: usize,
        config: &TearminalConfig,
        state: &SettingsPageState,
        cursor_visible: bool,
        rects: &SettingsPageRects,
        theme: &Theme,
    ) {
        let col_gap = 28;
        let col_w = (card_w.saturating_sub(col_gap)) / 2;
        let col1_x = content_x;
        let col2_x = content_x + col_w + col_gap;
        let perf_base_y = top + 36;
        let perf = &config.performance;

        // Columna 1 Header: MEDIDORES EN LA BARRA DE ESTADO
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Sliders,
            col1_x + 8,
            top + 14,
            14.0,
            theme.accent_color(),
        );
        self.draw_group_label(
            canvas,
            col1_x + 22,
            top + 6,
            col1_x + col_w,
            "MEDIDORES EN LA BARRA DE ESTADO",
            theme,
        );
        Self::draw_rect(canvas, col1_x, top + 28, col_w, 1, theme.chrome_line());

        let r1 = |i: usize| perf_base_y + i * 50;
        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(0),
            "Uso de CPU",
            "Muestra el porcentaje total de uso de CPU en la barra.",
            rects.perf_show_cpu_btn,
            perf.show_cpu,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::ShowCpu),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(1),
            "Memoria RAM",
            "Muestra el porcentaje de memoria ocupada en la barra.",
            rects.perf_show_ram_btn,
            perf.show_ram,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::ShowRam),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(2),
            "Uso de GPU",
            "Muestra el porcentaje de GPU 3D en la barra.",
            rects.perf_show_gpu_btn,
            perf.show_gpu,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::ShowGpu),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(3),
            "Espacio en disco",
            "Muestra el porcentaje de disco usado en la barra.",
            rects.perf_show_disk_btn,
            perf.show_disk,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::ShowDisk),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col1_x,
            col_w,
            r1(4),
            "Actividad de red",
            "Muestra el tráfico de red en la barra.",
            rects.perf_show_net_btn,
            perf.show_net,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::ShowNet),
            theme,
        );

        // Perfiles de agentes CLI IA (Columna 1)
        let agents_header_y = r1(5) + 2;
        self.draw_group_label(
            canvas,
            col1_x,
            agents_header_y,
            col1_x + col_w,
            "AGENTES CLI CONFIGURABLES (COMA)",
            theme,
        );
        let ab = rects.perf_agent_input_box;
        let editing_agents = state.editing_setting == Some(EditingSettingField::AgentProfiles);
        let hover_agents =
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::AgentProfilesBox);
        canvas.rounded_fill(
            ab.0,
            ab.1,
            ab.2,
            ab.3,
            tokens::RADIUS_ROW,
            theme.mix(theme.background, theme.foreground, 0.05),
        );
        canvas.rounded_stroke(
            ab.0,
            ab.1,
            ab.2,
            ab.3,
            tokens::RADIUS_ROW,
            if editing_agents {
                theme.accent_color()
            } else if hover_agents {
                theme.foreground
            } else {
                theme.chrome_line()
            },
        );
        let ab_text_y = self.centered_baseline(ab.1 + ab.3 / 2);
        if editing_agents {
            self.render_text_input_content(
                canvas,
                &state.agent_profiles_input,
                ab.0 + 10,
                ab_text_y,
                ab.0 + ab.2.saturating_sub(10),
                ab.1,
                ab.3,
                cursor_visible,
                "claude, agy, gemini, codex…",
                true,
                theme,
            );
        } else {
            let csv = perf.agent_executables_csv();
            let shown = if csv.is_empty() {
                "Ninguno (clic para añadir)"
            } else {
                &csv
            };
            self.draw_mono_text(
                canvas,
                shown,
                ab.0 + 10,
                ab_text_y,
                ab.0 + ab.2.saturating_sub(10),
                if csv.is_empty() {
                    theme.text_muted()
                } else {
                    theme.foreground
                },
                tokens::DETAIL,
                false,
            );
        }
        self.draw_button(
            canvas,
            rects.perf_agent_reset_btn,
            "Defecto",
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::AgentProfilesReset),
            false,
            theme,
        );

        // Columna 2 Header: MUESTREO Y ALERTAS
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Sliders,
            col2_x + 8,
            top + 14,
            14.0,
            theme.accent_color(),
        );
        self.draw_group_label(
            canvas,
            col2_x + 22,
            top + 6,
            col2_x + col_w,
            "MUESTREO Y ALERTAS",
            theme,
        );
        Self::draw_rect(canvas, col2_x, top + 28, col_w, 1, theme.chrome_line());

        let r2 = |i: usize| perf_base_y + i * 50;
        let int_str = format!("{} s", perf.sample_interval_ms / 1000);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(0),
            "Intervalo de muestreo",
            "Frecuencia de lectura del monitor del sistema.",
            rects.perf_sample_interval_minus_btn,
            rects.perf_sample_interval_value_box,
            rects.perf_sample_interval_plus_btn,
            &int_str,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::SampleIntervalMinus),
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::SampleIntervalPlus),
            theme,
        );

        self.draw_settings_row_switch(
            canvas,
            col2_x,
            col_w,
            r2(1),
            "Alertas visuales",
            "Avisa si un recurso supera el umbral crítico.",
            rects.perf_enable_alerts_btn,
            perf.enable_alerts,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::EnableAlerts),
            theme,
        );

        let cpu_th_str = format!("{}%", perf.cpu_alert_threshold);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(2),
            "Umbral de alerta CPU",
            "Marca en amarillo o rojo al superarlo.",
            rects.perf_cpu_threshold_minus_btn,
            rects.perf_cpu_threshold_value_box,
            rects.perf_cpu_threshold_plus_btn,
            &cpu_th_str,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::CpuThresholdMinus),
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::CpuThresholdPlus),
            theme,
        );

        let ram_th_str = format!("{}%", perf.ram_alert_threshold);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(3),
            "Umbral de alerta RAM",
            "Marca en amarillo o rojo al superarlo.",
            rects.perf_ram_threshold_minus_btn,
            rects.perf_ram_threshold_value_box,
            rects.perf_ram_threshold_plus_btn,
            &ram_th_str,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::RamThresholdMinus),
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::RamThresholdPlus),
            theme,
        );

        let disk_th_str = format!("{}%", perf.disk_alert_threshold);
        self.draw_settings_row_stepper(
            canvas,
            col2_x,
            col_w,
            r2(4),
            "Umbral de alerta Disco",
            "Marca en amarillo o rojo al superarlo.",
            rects.perf_disk_threshold_minus_btn,
            rects.perf_disk_threshold_value_box,
            rects.perf_disk_threshold_plus_btn,
            &disk_th_str,
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::DiskThresholdMinus),
            state.hovered_perf == Some(crate::renderer::HoveredPerfControl::DiskThresholdPlus),
            theme,
        );

        // Info Notice al pie
        let perf_notice_y = r2(4) + 66;
        self.draw_info_notice(
            canvas,
            col2_x,
            perf_notice_y,
            col_w,
            46,
            "Reposo absoluto: el monitor se detiene cuando la ventana no es visible.",
            theme,
        );
    }

    /// Sección «Atajos»: dos columnas con keycaps individuales y edición interactiva.
    #[allow(clippy::too_many_arguments)]
    fn draw_settings_shortcuts(
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
            SettingsPageRects::SHORTCUTS_CARD_H,
            "ATAJOS DE TECLADO · CLIC PARA EDITAR",
            theme,
        );
        self.draw_button(
            canvas,
            rects.shortcuts_reset_btn,
            "Restablecer",
            state.hovered_shortcuts_reset,
            false,
            theme,
        );

        let col_gap = 28;
        let col_w = (card_w.saturating_sub(40 + col_gap)) / 2;
        let start_y = card_y + 56;
        let row_h = 40;
        for idx in 0..16 {
            let (chord, desc, action_opt) = config.shortcuts.slot_info(idx);
            let col = idx / 8;
            let row = idx % 8;
            let x = content_x + 20 + col * (col_w + col_gap);
            let y = start_y + row * row_h;
            let row_rect = rects.shortcuts_rows[idx];
            let is_editing = action_opt.is_some() && state.editing_shortcut == action_opt;
            let is_hover = action_opt.is_some() && state.hovered_shortcut_idx == Some(idx);

            if is_editing {
                canvas.rounded_fill(
                    row_rect.0,
                    row_rect.1,
                    row_rect.2,
                    row_rect.3,
                    tokens::RADIUS_ROW,
                    theme.accent_soft(26),
                );
                canvas.rounded_stroke(
                    row_rect.0,
                    row_rect.1,
                    row_rect.2,
                    row_rect.3,
                    tokens::RADIUS_ROW,
                    theme.accent_color(),
                );
                self.draw_mono_text(
                    canvas,
                    "Pulsa combinación… (Esc cancela)",
                    x,
                    self.centered_baseline(y + tokens::KEYCAP_H / 2),
                    x + col_w,
                    theme.accent_color(),
                    tokens::MICRO,
                    true,
                );
            } else {
                if is_hover {
                    canvas.rounded_fill(
                        row_rect.0,
                        row_rect.1,
                        row_rect.2,
                        row_rect.3,
                        tokens::RADIUS_ROW,
                        theme.surface_hover(),
                    );
                }
                let mut kx = x;
                let key_max = x + col_w;
                for (ki, key) in chord.split(" + ").enumerate() {
                    if ki > 0 {
                        let plus_w = self.measure_mono_text("+", tokens::MICRO, false);
                        let plus_base = self.centered_baseline(y + tokens::KEYCAP_H / 2);
                        self.draw_mono_text(
                            canvas,
                            "+",
                            kx + 4,
                            plus_base,
                            key_max,
                            theme.text_muted(),
                            tokens::MICRO,
                            false,
                        );
                        kx += plus_w + 8;
                    }
                    if kx >= key_max {
                        break;
                    }
                    let kw = self.draw_keycap(canvas, kx, y, key, theme);
                    kx += kw + 4;
                }
            }
            self.draw_ui_text(
                canvas,
                desc,
                x,
                y + 32,
                x + col_w,
                if is_editing {
                    theme.foreground
                } else if action_opt.is_none() {
                    theme.text_muted()
                } else {
                    theme.text_secondary()
                },
                tokens::BODY,
                is_editing,
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
            theme.accent_text(),
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
        let themes = Theme::all_count().to_string();
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
                theme.accent_text(),
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
