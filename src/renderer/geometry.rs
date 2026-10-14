//! Geometría compartida entre el dibujo y la detección de clics/hover (una sola fuente de verdad).

/// Rectángulo (x, y, ancho, alto) en píxeles
pub type Rect = (usize, usize, usize, usize);

#[derive(Debug, Clone, Copy)]
pub struct TopTabBarRects {
    pub bar_h: usize,
    pub tabs_start_x: usize,
    pub tab_w: usize,
    pub tab_gap: usize,
    pub plus_btn: (usize, usize, usize, usize),
    pub files_btn: (usize, usize, usize, usize),
    pub settings_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
}

impl TopTabBarRects {
    pub const HEIGHT: usize = 38;

    pub fn compute(width: usize, tabs_count: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 42;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);

        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 34), 5, 28, 28);
        let files_btn = (width.saturating_sub(ctrl_w * 3 + 68), 5, 28, 28);

        let right_reserved = ctrl_w * 3 + 76;
        let left_reserved = 76;
        let available_space = width.saturating_sub(left_reserved + right_reserved);

        let plus_w = 28;
        let plus_margin = 4;
        let overhead = plus_w + plus_margin * 2;
        let gap = if tabs_count > 16 { 2 } else { 4 };

        let tab_w = if tabs_count > 0 {
            let usable =
                available_space.saturating_sub(overhead + tabs_count.saturating_sub(1) * gap);
            (usable / tabs_count).clamp(18, 160)
        } else {
            160
        };

        let raw_tabs_w = tabs_count * (tab_w + gap);
        let max_tabs_w = available_space.saturating_sub(overhead);
        let effective_tabs_w = raw_tabs_w.min(max_tabs_w);

        let total_capsule_w = (effective_tabs_w + overhead).min(available_space);
        let tabs_start_x = if available_space > total_capsule_w {
            left_reserved + (available_space - total_capsule_w) / 2
        } else {
            left_reserved
        };
        let plus_x = (tabs_start_x + effective_tabs_w + plus_margin)
            .min(left_reserved + available_space.saturating_sub(plus_w));
        let plus_btn = (plus_x, 5, plus_w, 28);

        Self {
            bar_h,
            tabs_start_x,
            tab_w,
            tab_gap: gap,
            plus_btn,
            files_btn,
            settings_btn,
            win_min_btn,
            win_max_btn,
            win_close_btn,
        }
    }

    #[inline]
    pub fn tab_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        (
            self.tabs_start_x + idx * (self.tab_w + self.tab_gap),
            4,
            self.tab_w,
            30,
        )
    }

    #[inline]
    pub fn close_btn_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        let tab = self.tab_rect(idx);
        if self.tab_w >= 48 {
            (tab.0 + tab.2.saturating_sub(20), tab.1 + 6, 16, 16)
        } else {
            (0, 0, 0, 0)
        }
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TopTitleBarRects {
    pub bar_h: usize,
    pub files_btn: (usize, usize, usize, usize),
    pub settings_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
}

impl TopTitleBarRects {
    pub const HEIGHT: usize = 38;

    pub fn compute(width: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 42;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);
        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 34), 5, 28, 28);
        let files_btn = (width.saturating_sub(ctrl_w * 3 + 68), 5, 28, 28);
        Self {
            bar_h,
            files_btn,
            settings_btn,
            win_min_btn,
            win_max_btn,
            win_close_btn,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsPageRects {
    pub nav_appearance: (usize, usize, usize, usize),
    pub nav_terminal: (usize, usize, usize, usize),
    pub nav_shortcuts: (usize, usize, usize, usize),
    pub nav_about: (usize, usize, usize, usize),

    pub theme_buttons: [(usize, usize, usize, usize); 8],
    pub theme_mode_system_btn: (usize, usize, usize, usize),
    pub theme_mode_dark_btn: (usize, usize, usize, usize),
    pub theme_mode_light_btn: (usize, usize, usize, usize),

    pub tab_layout_vertical_btn: (usize, usize, usize, usize),
    pub tab_layout_horizontal_btn: (usize, usize, usize, usize),

    pub font_minus_btn: (usize, usize, usize, usize),
    pub font_plus_btn: (usize, usize, usize, usize),
    pub font_value_box: (usize, usize, usize, usize),
    pub ui_font_minus_btn: (usize, usize, usize, usize),
    pub ui_font_plus_btn: (usize, usize, usize, usize),
    pub ui_font_value_box: (usize, usize, usize, usize),
    pub spacing_minus_btn: (usize, usize, usize, usize),
    pub spacing_plus_btn: (usize, usize, usize, usize),
    pub spacing_value_box: (usize, usize, usize, usize),
    pub opacity_minus_btn: (usize, usize, usize, usize),
    pub opacity_plus_btn: (usize, usize, usize, usize),
    pub opacity_value_box: (usize, usize, usize, usize),

    pub cursor_blink_btn: (usize, usize, usize, usize),
    pub ghostty_prompt_btn: (usize, usize, usize, usize),
    pub copy_on_select_btn: (usize, usize, usize, usize),
    pub status_bar_toggle_btn: (usize, usize, usize, usize),
    pub file_explorer_toggle_btn: (usize, usize, usize, usize),
    pub restore_session_btn: (usize, usize, usize, usize),
    pub open_file_btn: (usize, usize, usize, usize),
    pub reset_defaults_btn: (usize, usize, usize, usize),
    pub term_reset_btn: (usize, usize, usize, usize),
}

impl SettingsPageRects {
    pub fn compute(page_x: usize, page_y: usize, page_w: usize, _page_h: usize) -> Self {
        let card_w = 680.min(page_w.saturating_sub(72));
        let margin_x = if page_w > card_w {
            (page_w - card_w) / 2
        } else {
            36
        };
        let content_x = page_x + margin_x;
        let reset_defaults_btn = (content_x + card_w.saturating_sub(210), page_y + 36, 210, 28);

        let nav_y = page_y + 80;
        let nav_h = 32;

        let nav_w = (card_w.saturating_sub(24)) / 4;
        let nav_appearance = (content_x, nav_y, nav_w, nav_h);
        let nav_terminal = (content_x + nav_w + 8, nav_y, nav_w, nav_h);
        let nav_shortcuts = (content_x + (nav_w + 8) * 2, nav_y, nav_w, nav_h);
        let nav_about = (content_x + (nav_w + 8) * 3, nav_y, nav_w, nav_h);

        let card_y = page_y + 128;

        // Fila 1 y 2 temas (8 temas en 2 filas de 4 botones perfectamente alineados)
        let th_y1 = card_y + 44;
        let th_y2 = card_y + 80;
        let th_w = (card_w.saturating_sub(40 + 24)) / 4;
        let th_gap = 8;
        let theme_buttons = [
            (content_x + 20, th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap), th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 2, th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 3, th_y1, th_w, 28),
            (content_x + 20, th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap), th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 2, th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 3, th_y2, th_w, 28),
        ];

        // Tarjeta 2: Modo y Disposición de Pestañas
        let card2_y = card_y + 160;
        let label_col_w = 200;
        let ctrl_x = content_x + 20 + label_col_w;
        let ctrl_w = card_w.saturating_sub(label_col_w + 40);

        // 3 botones para Modo de color (Sistema, Oscuro, Claro)
        let btn3_w = (ctrl_w.saturating_sub(16)) / 3;
        let theme_mode_system_btn = (ctrl_x, card2_y + 40, btn3_w, 28);
        let theme_mode_dark_btn = (ctrl_x + btn3_w + 8, card2_y + 40, btn3_w, 28);
        let theme_mode_light_btn = (ctrl_x + (btn3_w + 8) * 2, card2_y + 40, btn3_w, 28);

        // 2 botones para Disposición de pestañas (Vertical, Superior)
        let btn2_w = (ctrl_w.saturating_sub(8)) / 2;
        let tab_layout_vertical_btn = (ctrl_x, card2_y + 78, btn2_w, 28);
        let tab_layout_horizontal_btn = (ctrl_x + btn2_w + 8, card2_y + 78, btn2_w, 28);

        // Tarjeta 3: Tipografía y ventana (Terminal Font, UI Font, Spacing, Opacity)
        let card3_y = card2_y + 124;
        let step_btn_w = 32;
        let step_span = 140;
        let font_minus_btn = (ctrl_x, card3_y + 40, step_btn_w, 26);
        let font_plus_btn = (ctrl_x + step_span, card3_y + 40, step_btn_w, 26);

        let ui_font_minus_btn = (ctrl_x, card3_y + 76, step_btn_w, 26);
        let ui_font_plus_btn = (ctrl_x + step_span, card3_y + 76, step_btn_w, 26);

        let spacing_minus_btn = (ctrl_x, card3_y + 112, step_btn_w, 26);
        let spacing_plus_btn = (ctrl_x + step_span, card3_y + 112, step_btn_w, 26);

        let opacity_minus_btn = (ctrl_x, card3_y + 148, step_btn_w, 26);
        let opacity_plus_btn = (ctrl_x + step_span, card3_y + 148, step_btn_w, 26);

        let font_value_box = (
            font_minus_btn.0 + font_minus_btn.2 + 4,
            font_minus_btn.1,
            font_plus_btn
                .0
                .saturating_sub(font_minus_btn.0 + font_minus_btn.2 + 8),
            font_minus_btn.3,
        );
        let ui_font_value_box = (
            ui_font_minus_btn.0 + ui_font_minus_btn.2 + 4,
            ui_font_minus_btn.1,
            ui_font_plus_btn
                .0
                .saturating_sub(ui_font_minus_btn.0 + ui_font_minus_btn.2 + 8),
            ui_font_minus_btn.3,
        );
        let spacing_value_box = (
            spacing_minus_btn.0 + spacing_minus_btn.2 + 4,
            spacing_minus_btn.1,
            spacing_plus_btn
                .0
                .saturating_sub(spacing_minus_btn.0 + spacing_minus_btn.2 + 8),
            spacing_minus_btn.3,
        );
        let opacity_value_box = (
            opacity_minus_btn.0 + opacity_minus_btn.2 + 4,
            opacity_minus_btn.1,
            opacity_plus_btn
                .0
                .saturating_sub(opacity_minus_btn.0 + opacity_minus_btn.2 + 8),
            opacity_minus_btn.3,
        );

        // Terminal toggles (alineados a ctrl_x con ancho uniforme)
        let toggle_w = 170;
        let cursor_blink_btn = (ctrl_x, card_y + 42, toggle_w, 28);
        let ghostty_prompt_btn = (ctrl_x, card_y + 80, toggle_w, 28);
        let copy_on_select_btn = (ctrl_x, card_y + 118, toggle_w, 28);
        let status_bar_toggle_btn = (ctrl_x, card_y + 156, toggle_w, 28);
        let file_explorer_toggle_btn = (ctrl_x, card_y + 194, toggle_w, 28);
        let restore_session_btn = (ctrl_x, card_y + 232, toggle_w, 28);

        let cfg_y = card_y + 278 + 14;
        let cfg_btn_w = (card_w.saturating_sub(50)) / 2;
        let open_file_btn = (content_x + 20, cfg_y + 44, cfg_btn_w, 32);
        let term_reset_btn = (content_x + 30 + cfg_btn_w, cfg_y + 44, cfg_btn_w, 32);

        Self {
            nav_appearance,
            nav_terminal,
            nav_shortcuts,
            nav_about,
            theme_buttons,
            theme_mode_system_btn,
            theme_mode_dark_btn,
            theme_mode_light_btn,
            tab_layout_vertical_btn,
            tab_layout_horizontal_btn,
            font_minus_btn,
            font_plus_btn,
            font_value_box,
            ui_font_minus_btn,
            ui_font_plus_btn,
            ui_font_value_box,
            spacing_minus_btn,
            spacing_plus_btn,
            spacing_value_box,
            opacity_minus_btn,
            opacity_plus_btn,
            opacity_value_box,
            cursor_blink_btn,
            ghostty_prompt_btn,
            copy_on_select_btn,
            status_bar_toggle_btn,
            file_explorer_toggle_btn,
            restore_session_btn,
            open_file_btn,
            reset_defaults_btn,
            term_reset_btn,
        }
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}
