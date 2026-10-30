//! Geometría compartida entre el dibujo y la detección de clics/hover (una sola fuente de verdad).

/// Rectángulo (x, y, ancho, alto) en píxeles
pub(crate) type Rect = (usize, usize, usize, usize);

/// Convierte la posición Y deseada de la parte superior del pulgar (`thumb_top_y`) en un `scroll_offset`
/// dentro de `0..=max_offset` para una barra de desplazamiento vertical `(track, thumb)`.
#[inline]
pub(crate) fn scroll_offset_for_thumb_y(
    track: Rect,
    thumb: Rect,
    thumb_top_y: f32,
    max_offset: usize,
) -> Option<usize> {
    let available_travel = track.3.saturating_sub(thumb.3);
    if available_travel == 0 || max_offset == 0 {
        return None;
    }
    let rel_y = (thumb_top_y - track.1 as f32).clamp(0.0, available_travel as f32);
    let ratio = rel_y / available_travel as f32;
    Some(((ratio * max_offset as f32).round() as usize).min(max_offset))
}

/// Rectángulo del botón flotante "ir hasta abajo" en el panel de la terminal
#[inline]
pub(crate) fn pane_scroll_to_bottom_btn_rect(
    rect_x: usize,
    rect_y: usize,
    rect_width: usize,
    rect_height: usize,
) -> Rect {
    let size = 26;
    let margin_right = 20;
    let margin_bottom = 16;
    let x = (rect_x + rect_width).saturating_sub(size + margin_right);
    let y = (rect_y + rect_height).saturating_sub(size + margin_bottom);
    (x, y, size, size)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TopTabBarRects {
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
    /// Buscar (paleta Ctrl K, WP5): solo se dibuja; el clic no hace nada todavía.
    pub search_btn: (usize, usize, usize, usize),
}

impl TopTabBarRects {
    pub(crate) const HEIGHT: usize = 38;
    /// Las pestañas empiezan alineadas con el borde izquierdo de las tarjetas (no hay marca).
    pub(crate) const BRAND_W: usize = SidebarRects::CARD_INSET;
    pub(crate) const TAB_GAP: usize = 3;
    pub(crate) const CLOSE_W: usize = 24;
    pub(crate) const PLUS_W: usize = 28;
    /// Lado de los botones de acción del chrome (buscar, panel, ajustes).
    pub(crate) const ACTION: usize = 32;

    #[allow(dead_code)]
    pub(crate) fn compute(width: usize, tabs_count: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 46;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);

        // Acciones a la izquierda de los controles de ventana: buscar · panel · ajustes.
        let actions_right = width.saturating_sub(ctrl_w * 3 + 8);
        let action_y = (bar_h.saturating_sub(Self::ACTION)) / 2;
        let settings_btn = (
            actions_right.saturating_sub(Self::ACTION),
            action_y,
            Self::ACTION,
            Self::ACTION,
        );
        let files_btn = (
            settings_btn.0.saturating_sub(Self::ACTION + 2),
            action_y,
            Self::ACTION,
            Self::ACTION,
        );
        let search_btn = (
            files_btn.0.saturating_sub(Self::ACTION + 2),
            action_y,
            Self::ACTION,
            Self::ACTION,
        );

        let left_reserved = Self::BRAND_W;
        let right_reserved = ctrl_w * 3 + 112;
        let available_space = width.saturating_sub(left_reserved + right_reserved);

        let plus_w = Self::PLUS_W;
        let gap = Self::TAB_GAP;
        let usable = available_space
            .saturating_sub(plus_w + 8)
            .saturating_sub(tabs_count.saturating_mul(gap));
        let tab_w = usable
            .checked_div(tabs_count)
            .map_or(177, |w| w.clamp(20, 177));

        let tabs_start_x = left_reserved;
        let plus_x = (tabs_start_x + tabs_count * (tab_w + gap))
            .min(left_reserved + available_space.saturating_sub(plus_w));
        let plus_btn = (plus_x, 5, plus_w, plus_w);

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
            search_btn,
        }
    }

    #[inline]
    pub(crate) fn tab_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        (
            self.tabs_start_x + idx * (self.tab_w + self.tab_gap),
            0,
            self.tab_w,
            Self::HEIGHT,
        )
    }

    #[inline]
    pub(crate) fn close_btn_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        let tab = self.tab_rect(idx);
        if self.tab_w >= 76 {
            (
                tab.0 + tab.2.saturating_sub(Self::CLOSE_W + 6),
                (Self::HEIGHT - Self::CLOSE_W) / 2,
                Self::CLOSE_W,
                Self::CLOSE_W,
            )
        } else {
            (0, 0, 0, 0)
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TopTitleBarRects {
    pub bar_h: usize,
    pub files_btn: (usize, usize, usize, usize),
    pub settings_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
    /// Píldora de búsqueda centrada del modo vertical (paleta Ctrl K, WP5).
    pub search_box: (usize, usize, usize, usize),
}

impl TopTitleBarRects {
    pub(crate) const HEIGHT: usize = 38;
    pub(crate) const SEARCH_W: usize = 420;
    pub(crate) const SEARCH_H: usize = 30;

    #[allow(dead_code)]
    pub(crate) fn compute(width: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 46;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);
        // En vertical los accesos de archivos y ajustes viven en el panel (riel); la barra solo lleva
        // la marca, la búsqueda y los controles de ventana.
        let search_w =
            Self::SEARCH_W.min(width.saturating_sub(TopTabBarRects::BRAND_W + ctrl_w * 3 + 80));
        let search_box = if search_w >= 200 {
            (
                (width.saturating_sub(search_w)) / 2,
                (bar_h.saturating_sub(Self::SEARCH_H)) / 2,
                search_w,
                Self::SEARCH_H,
            )
        } else {
            (0, 0, 0, 0)
        };
        Self {
            bar_h,
            files_btn: (0, 0, 0, 0),
            settings_btn: (0, 0, 0, 0),
            win_min_btn,
            win_max_btn,
            win_close_btn,
            search_box,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SettingsPageRects {
    pub nav_appearance: (usize, usize, usize, usize),
    pub nav_terminal: (usize, usize, usize, usize),
    pub nav_shortcuts: (usize, usize, usize, usize),
    pub nav_about: (usize, usize, usize, usize),

    pub theme_slot_dark_btn: (usize, usize, usize, usize),
    pub theme_slot_light_btn: (usize, usize, usize, usize),
    pub theme_search_box: (usize, usize, usize, usize),
    pub theme_list: (usize, usize, usize, usize),
    pub theme_mode_system_btn: (usize, usize, usize, usize),
    pub theme_mode_dark_btn: (usize, usize, usize, usize),
    pub theme_mode_light_btn: (usize, usize, usize, usize),

    pub tab_layout_vertical_btn: (usize, usize, usize, usize),
    pub tab_layout_horizontal_btn: (usize, usize, usize, usize),
    pub panel_side_left_btn: (usize, usize, usize, usize),
    pub panel_side_right_btn: (usize, usize, usize, usize),

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
    pub cursor_shape_block_btn: (usize, usize, usize, usize),
    pub cursor_shape_beam_btn: (usize, usize, usize, usize),
    pub cursor_shape_underline_btn: (usize, usize, usize, usize),
    pub shell_pwsh_btn: (usize, usize, usize, usize),
    pub shell_powershell_btn: (usize, usize, usize, usize),
    pub shell_cmd_btn: (usize, usize, usize, usize),
    pub scrollback_minus_btn: (usize, usize, usize, usize),
    pub scrollback_plus_btn: (usize, usize, usize, usize),
    pub scrollback_value_box: (usize, usize, usize, usize),
    pub shell_integration_btn: (usize, usize, usize, usize),
    pub copy_on_select_btn: (usize, usize, usize, usize),
    pub status_bar_toggle_btn: (usize, usize, usize, usize),
    pub show_dimensions_btn: (usize, usize, usize, usize),
    pub file_explorer_toggle_btn: (usize, usize, usize, usize),
    pub restore_session_btn: (usize, usize, usize, usize),
    pub auto_save_btn: (usize, usize, usize, usize),
    pub word_wrap_btn: (usize, usize, usize, usize),
    pub ssh_integration_btn: (usize, usize, usize, usize),
    pub osc52_btn: (usize, usize, usize, usize),
    pub ligatures_btn: (usize, usize, usize, usize),
    pub ai_limits_btn: (usize, usize, usize, usize),
    pub auto_check_updates_btn: (usize, usize, usize, usize),
    pub keep_awake_off_btn: (usize, usize, usize, usize),
    pub keep_awake_agent_btn: (usize, usize, usize, usize),
    pub keep_awake_active_btn: (usize, usize, usize, usize),
    pub open_file_btn: (usize, usize, usize, usize),
    pub reset_defaults_btn: (usize, usize, usize, usize),
    pub search_box: (usize, usize, usize, usize),
    pub term_reset_btn: (usize, usize, usize, usize),
    /// Acciones editoriales de «Acerca de» (no comparten rectángulo con Terminal)
    pub about_open_file_btn: (usize, usize, usize, usize),
    pub about_reset_btn: (usize, usize, usize, usize),
    pub card_w: usize,
    pub content_x: usize,
}

impl SettingsPageRects {
    /// Y (relativa a la página) donde empieza el contenido desplazable, bajo el encabezado y la navegación.
    pub(crate) const CONTENT_TOP: usize = 144;
    /// Filas visibles de la lista de temas, alto de cada una y alto de su tarjeta
    pub(crate) const THEME_ROWS: usize = 7;
    pub(crate) const THEME_ROW_H: usize = 30;
    pub(crate) const THEME_CARD_H: usize = 428;
    pub(crate) const INTERFACE_CARD_H: usize = 270;
    pub(crate) const TYPOGRAPHY_CARD_H: usize = 278;
    /// Alto de la tarjeta «Comportamiento del terminal»
    pub(crate) const TERMINAL_CARD_H: usize = 738;
    pub(crate) const TERMINAL_ROW_H: usize = 28;
    pub(crate) const DISK_CARD_H: usize = 118;
    pub(crate) const SHORTCUTS_CARD_H: usize = 392;
    pub(crate) const ABOUT_CARD_H: usize = 328;

    /// Alto total del contenido desplazable de una sección (desde `CONTENT_TOP`).
    pub(crate) fn content_height(section: super::state::SettingsSection) -> usize {
        use super::state::SettingsSection::*;
        match section {
            Appearance => 1012,
            Terminal => 894,
            Shortcuts => 424,
            About => 360,
        }
    }

    /// Máximo desplazamiento vertical para un viewport de `viewport_h` píxeles.
    pub(crate) fn max_scroll(section: super::state::SettingsSection, viewport_h: usize) -> usize {
        Self::content_height(section).saturating_sub(viewport_h)
    }

    /// Pista y pulgar de la barra de scroll de Ajustes (compartidos por dibujo, clic y arrastre)
    pub(crate) fn scrollbar(
        section: super::state::SettingsSection,
        page_y: usize,
        win_w: usize,
        viewport_h: usize,
        scroll: usize,
    ) -> Option<(Rect, Rect)> {
        ScrollbarRects::vertical(
            win_w,
            page_y + Self::CONTENT_TOP,
            viewport_h.saturating_sub(SidebarRects::CARD_INSET + 4),
            Self::content_height(section),
            viewport_h,
            scroll,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn compute(page_x: usize, page_y: usize, page_w: usize, _page_h: usize) -> Self {
        let max_card_w = 980;
        let side_padding = ((page_w as f32 * 0.05) as usize).clamp(24, 72);
        let card_w = (page_w.saturating_sub(side_padding * 2))
            .min(max_card_w)
            .max(460);
        let margin_x = if page_w > card_w {
            (page_w - card_w) / 2
        } else {
            side_padding
        };
        let content_x = page_x + margin_x;
        let search_w = 216.min(card_w / 3);
        let reset_w = 148;
        let reset_defaults_btn = (
            content_x + card_w.saturating_sub(reset_w),
            page_y + 26,
            reset_w,
            30,
        );
        let search_box = (
            reset_defaults_btn.0.saturating_sub(search_w + 12),
            page_y + 26,
            search_w,
            30,
        );

        let nav_y = page_y + 92;
        let nav_h = 34;

        let nav_w = card_w / 4;
        let nav_appearance = (content_x, nav_y, nav_w, nav_h);
        let nav_terminal = (content_x + nav_w, nav_y, nav_w, nav_h);
        let nav_shortcuts = (content_x + nav_w * 2, nav_y, nav_w, nav_h);
        let nav_about = (content_x + nav_w * 3, nav_y, nav_w, nav_h);

        let card_y = page_y + Self::CONTENT_TOP;

        // Buscador y lista desplazable de temas
        let slot_w = card_w.saturating_sub(52) / 2;
        let theme_slot_dark_btn = (content_x + 20, card_y + 50, slot_w, 30);
        let theme_slot_light_btn = (content_x + 32 + slot_w, card_y + 50, slot_w, 30);
        let theme_search_box = (content_x + 20, card_y + 92, card_w.saturating_sub(40), 32);
        let theme_list = (
            content_x + 20,
            card_y + 138,
            card_w.saturating_sub(40),
            Self::THEME_ROWS * Self::THEME_ROW_H,
        );

        // Tarjeta 2: Modo y Disposición de Pestañas
        let card2_y = card_y + Self::THEME_CARD_H + 14;
        let label_col_w = (card_w as f32 * 0.36).clamp(195.0, 250.0) as usize;
        let ctrl_x = content_x + 20 + label_col_w;
        let ctrl_w = card_w.saturating_sub(label_col_w + 40);

        // 3 botones para Modo de color (Sistema, Oscuro, Claro)
        let btn3_w = (ctrl_w.saturating_sub(16)) / 3;
        let theme_mode_system_btn = (ctrl_x, card2_y + 48, btn3_w, 30);
        let theme_mode_dark_btn = (ctrl_x + btn3_w + 8, card2_y + 48, btn3_w, 30);
        let theme_mode_light_btn = (ctrl_x + (btn3_w + 8) * 2, card2_y + 48, btn3_w, 30);

        // 2 botones para Disposición de pestañas (Vertical, Superior)
        let btn2_w = (ctrl_w.saturating_sub(8)) / 2;
        let tab_layout_vertical_btn = (ctrl_x, card2_y + 98, btn2_w, 30);
        let tab_layout_horizontal_btn = (ctrl_x + btn2_w + 8, card2_y + 98, btn2_w, 30);

        // 2 botones para el lado del panel de navegación (Izquierda, Derecha)
        let panel_side_left_btn = (ctrl_x, card2_y + 148, btn2_w, 30);
        let panel_side_right_btn = (ctrl_x + btn2_w + 8, card2_y + 148, btn2_w, 30);

        // Tarjeta 3: Tipografía y ventana (Font Size, UI Font Size, Spacing)
        let card3_y = card2_y + Self::INTERFACE_CARD_H + 14;
        let step_btn_w = 30;
        let step_span = 150;
        let font_minus_btn = (ctrl_x, card3_y + 50, step_btn_w, 30);
        let font_plus_btn = (ctrl_x + step_span, card3_y + 50, step_btn_w, 30);

        let ui_font_minus_btn = (ctrl_x, card3_y + 92, step_btn_w, 30);
        let ui_font_plus_btn = (ctrl_x + step_span, card3_y + 92, step_btn_w, 30);

        let spacing_minus_btn = (ctrl_x, card3_y + 134, step_btn_w, 30);
        let spacing_plus_btn = (ctrl_x + step_span, card3_y + 134, step_btn_w, 30);

        // La opacidad pertenece a «Interfaz», no a tipografía.
        let opacity_minus_btn = (ctrl_x, card2_y + 198, step_btn_w, 30);
        let opacity_plus_btn = (ctrl_x + step_span, card2_y + 198, step_btn_w, 30);

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

        // Terminal: tres grupos (Entrada · Shell · Interfaz) con filas de 28 px
        let toggle_w = 180;
        let row_h = Self::TERMINAL_ROW_H;
        let cursor_blink_btn = (ctrl_x, card_y + 72, toggle_w, row_h);
        let cursor_shape_block_btn = (ctrl_x, card_y + 106, btn3_w, row_h);
        let cursor_shape_beam_btn = (ctrl_x + btn3_w + 8, card_y + 106, btn3_w, row_h);
        let cursor_shape_underline_btn = (ctrl_x + (btn3_w + 8) * 2, card_y + 106, btn3_w, row_h);
        let copy_on_select_btn = (ctrl_x, card_y + 140, toggle_w, row_h);
        let word_wrap_btn = (ctrl_x, card_y + 174, toggle_w, row_h);

        let shell_pwsh_btn = (ctrl_x, card_y + 230, btn3_w, row_h);
        let shell_powershell_btn = (ctrl_x + btn3_w + 8, card_y + 230, btn3_w, row_h);
        let shell_cmd_btn = (ctrl_x + (btn3_w + 8) * 2, card_y + 230, btn3_w, row_h);
        let scrollback_minus_btn = (ctrl_x, card_y + 264, step_btn_w, row_h);
        let scrollback_plus_btn = (ctrl_x + step_span, card_y + 264, step_btn_w, row_h);
        let scrollback_value_box = (
            scrollback_minus_btn.0 + scrollback_minus_btn.2 + 4,
            scrollback_minus_btn.1,
            scrollback_plus_btn
                .0
                .saturating_sub(scrollback_minus_btn.0 + scrollback_minus_btn.2 + 8),
            scrollback_minus_btn.3,
        );
        let shell_integration_btn = (ctrl_x, card_y + 298, toggle_w, row_h);
        let ssh_integration_btn = (ctrl_x, card_y + 332, toggle_w, row_h);
        let osc52_btn = (ctrl_x, card_y + 366, toggle_w, row_h);
        let ligatures_btn = (ctrl_x, card_y + 400, toggle_w, row_h);

        let status_bar_toggle_btn = (ctrl_x, card_y + 456, toggle_w, row_h);
        let show_dimensions_btn = (ctrl_x, card_y + 490, toggle_w, row_h);
        let file_explorer_toggle_btn = (ctrl_x, card_y + 524, toggle_w, row_h);
        let restore_session_btn = (ctrl_x, card_y + 558, toggle_w, row_h);
        let auto_save_btn = (ctrl_x, card_y + 592, toggle_w, row_h);
        let ai_limits_btn = (ctrl_x, card_y + 626, toggle_w, row_h);
        let auto_check_updates_btn = (ctrl_x, card_y + 660, toggle_w, row_h);
        let keep_awake_off_btn = (ctrl_x, card_y + 694, btn3_w, row_h);
        let keep_awake_agent_btn = (ctrl_x + btn3_w + 8, card_y + 694, btn3_w, row_h);
        let keep_awake_active_btn = (ctrl_x + (btn3_w + 8) * 2, card_y + 694, btn3_w, row_h);

        let cfg_y = card_y + Self::TERMINAL_CARD_H + 14;
        let cfg_btn_w = (card_w.saturating_sub(50)) / 2;
        let open_file_btn = (content_x + 20, cfg_y + 46, cfg_btn_w, 32);
        let term_reset_btn = (content_x + 30 + cfg_btn_w, cfg_y + 46, cfg_btn_w, 32);

        // About: acciones propias, al pie de la tarjeta editorial
        let about_y = card_y + Self::ABOUT_CARD_H.saturating_sub(60);
        let about_open_file_btn = (content_x + 20, about_y, 190, 32);
        let about_reset_btn = (content_x + 220, about_y, 190, 32);

        Self {
            nav_appearance,
            nav_terminal,
            nav_shortcuts,
            nav_about,
            theme_slot_dark_btn,
            theme_slot_light_btn,
            theme_search_box,
            theme_list,
            theme_mode_system_btn,
            theme_mode_dark_btn,
            theme_mode_light_btn,
            tab_layout_vertical_btn,
            tab_layout_horizontal_btn,
            panel_side_left_btn,
            panel_side_right_btn,
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
            cursor_shape_block_btn,
            cursor_shape_beam_btn,
            cursor_shape_underline_btn,
            shell_pwsh_btn,
            shell_powershell_btn,
            shell_cmd_btn,
            scrollback_minus_btn,
            scrollback_plus_btn,
            scrollback_value_box,
            shell_integration_btn,
            copy_on_select_btn,
            status_bar_toggle_btn,
            show_dimensions_btn,
            file_explorer_toggle_btn,
            restore_session_btn,
            auto_save_btn,
            word_wrap_btn,
            ssh_integration_btn,
            osc52_btn,
            ligatures_btn,
            ai_limits_btn,
            auto_check_updates_btn,
            keep_awake_off_btn,
            keep_awake_agent_btn,
            keep_awake_active_btn,
            open_file_btn,
            reset_defaults_btn,
            search_box,
            term_reset_btn,
            about_open_file_btn,
            about_reset_btn,
            card_w,
            content_x,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerHeaderRects {
    pub mode_btn: (usize, usize, usize, usize),
    pub md_btn: Option<(usize, usize, usize, usize)>,
    /// «Cambios»: abre el diff contra HEAD (solo si el archivo está en un repo)
    pub diff_btn: Option<(usize, usize, usize, usize)>,
}

impl FileViewerHeaderRects {
    #[allow(dead_code)]
    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        _h: usize,
        is_markdown: bool,
        has_diff: bool,
    ) -> Self {
        let mut right_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 12);
        let btn_y = y
            + SidebarRects::CARD_INSET
            + (FileViewerLayout::HEADER_H.saturating_sub(SidebarRects::CARD_INSET + 24)) / 2;

        // 1. Botón Vista previa / Código si es .md (icono + texto)
        let md_btn = if is_markdown {
            let md_w = 136;
            let md_x = right_x.saturating_sub(md_w);
            right_x = md_x.saturating_sub(8);
            Some((md_x, btn_y, md_w, 24))
        } else {
            None
        };

        // 2. Botón Lectura / Edición (icono + texto; el atajo Ctrl+E va en la ayuda del menú)
        let mode_w = 104;
        let mode_x = right_x.saturating_sub(mode_w);
        let mode_btn = (mode_x, btn_y, mode_w, 24);

        // 3. Botón Cambios (diff antes vs ahora), a la izquierda del de modo
        let diff_btn = has_diff.then(|| {
            let diff_w = 108;
            (mode_x.saturating_sub(8 + diff_w), btn_y, diff_w, 24)
        });

        Self {
            mode_btn,
            md_btn,
            diff_btn,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

/// Geometría del área de texto del visor de archivos (una sola fuente para dibujo, clics y scroll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerLayout {
    pub content_y: usize,
    pub content_h: usize,
    pub row_h: usize,
    pub char_w: usize,
    pub gutter_w: usize,
    pub sep_x: usize,
    /// X del primer carácter de cada línea (con scroll horizontal 0)
    pub text_x: usize,
    pub text_max_x: usize,
    pub visible_cols: usize,
}

impl FileViewerLayout {
    pub(crate) const HEADER_H: usize = 42;

    #[allow(dead_code)]
    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        row_height: usize,
        char_width: usize,
        total_lines: usize,
    ) -> Self {
        let row_h = row_height.max(16);
        let char_w = char_width.max(7);
        let content_h = h.saturating_sub(Self::HEADER_H);
        let digits = total_lines.to_string().len().max(3);
        let gutter_w = SidebarRects::CARD_INSET + digits * char_w + 24;
        let sep_x = x + gutter_w;
        let text_x = sep_x + 14;
        let text_max_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 16);
        Self {
            content_y: y + Self::HEADER_H,
            content_h,
            row_h,
            char_w,
            gutter_w,
            sep_x,
            text_x,
            text_max_x,
            visible_cols: (text_max_x.saturating_sub(text_x) / char_w).max(1),
        }
    }

    /// Columna (índice de grafema) de la línea bajo la coordenada X, con el scroll horizontal aplicado.
    pub(crate) fn col_at(&self, line: &str, mx: usize, scroll_x: usize) -> usize {
        if mx < self.text_x {
            return 0;
        }
        let rel_x = (mx - self.text_x) + scroll_x * self.char_w;
        let mut acc = 0;
        use unicode_segmentation::UnicodeSegmentation;
        for (ci, g) in line.graphemes(true).enumerate() {
            let adv = if g == "\t" {
                self.char_w * 4
            } else {
                use unicode_width::UnicodeWidthStr;
                self.char_w * g.width().max(1)
            };
            if acc + adv / 2 >= rel_x {
                return ci;
            }
            acc += adv;
        }
        line.graphemes(true).count()
    }

    /// Pista y pulgar de la barra horizontal; `None` si todo el ancho cabe. `max_scroll` = `FileViewerState::max_scroll_x`.
    pub(crate) fn hscrollbar(&self, max_scroll: usize, scroll_x: usize) -> Option<(Rect, Rect)> {
        if max_scroll == 0 || self.content_h < 32 {
            return None;
        }
        let track_x = self.sep_x + 8;
        let track_w = self.text_max_x.saturating_sub(track_x);
        let track_h = ScrollbarRects::WIDTH;
        let track_y = (self.content_y + self.content_h)
            .saturating_sub(track_h + 4 + SidebarRects::CARD_INSET);
        let total = max_scroll + self.visible_cols;
        let thumb_w = (track_w * self.visible_cols / total).clamp(24.min(track_w), track_w);
        let travel = track_w - thumb_w;
        let thumb_x = track_x + travel * scroll_x.min(max_scroll) / max_scroll;
        Some((
            (track_x, track_y, track_w, track_h),
            (thumb_x, track_y, thumb_w, track_h),
        ))
    }
}

/// Geometría de la barra lateral: riel de ajustes y columna de sesiones (una sola fuente para dibujo,
/// clic, hover y arrastre).
///
/// La barra empieza en `TOP` (bajo la barra de título) para no tapar la marca. Las coordenadas de sus
/// métodos son locales a la barra, que se dibuja en su propio búfer y se coloca a izquierda o derecha:
/// clic y dibujo no repiten la retícula.
pub(crate) struct SidebarRects;

impl SidebarRects {
    pub(crate) const WIDTH: usize = 328;
    /// Ancho del riel (solo lleva el botón de ajustes); la columna de sesiones ocupa el resto.
    pub(crate) const RAIL_W: usize = 60;
    pub(crate) const TOP: usize = 38;
    /// Marco del panel de pestañas superiores: margen y radio de la tarjeta (`Canvas::card`).
    pub(crate) const CARD_INSET: usize = 6;
    pub(crate) const CARD_RADIUS: usize = super::tokens::RADIUS_CARD;
    /// Margen derecho del contenido del panel (contadores, estados, insignias de git): deja aire
    /// dentro de la tarjeta y alinea los bordes de todas las secciones.
    pub(crate) const PAD_R: usize = 20;
    /// Margen derecho de las filas de sesión: el recorte de la tarjeta del panel más 8 px de aire.
    pub(crate) const ROW_PAD_R: usize = Self::CARD_INSET + 8;
    /// Título, rama y carpeta.
    pub(crate) const HEADER_H: usize = 66;
    /// Y de la primera fila.
    pub(crate) const LIST_START_Y: usize = Self::TOP + Self::HEADER_H + 8;
    pub(crate) const TAB_ITEM_H: usize = 48;
    pub(crate) const TAB_SPACING: usize = 2;
    /// X de las filas de sesión (junto al riel).
    pub(crate) const PAD_X: usize = Self::RAIL_W + 8;
    pub(crate) const CLOSE_SIZE: usize = 24;
    pub(crate) const PLUS_SIZE: usize = 28;

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn plus_btn() -> Rect {
        Self::plus_btn_with_width(Self::WIDTH)
    }

    /// Rectángulo del botón «+» para un ancho de panel dado.
    #[inline]
    pub(crate) fn plus_btn_with_width(panel_w: usize) -> Rect {
        (
            panel_w.saturating_sub(Self::PLUS_SIZE + 10),
            Self::TOP + 8,
            Self::PLUS_SIZE,
            Self::PLUS_SIZE,
        )
    }

    #[inline]
    pub(crate) fn list_max_y(usable_h: usize) -> usize {
        Self::content_bottom(usable_h).max(Self::LIST_START_Y)
    }

    /// Fin (y) de la lista de sesiones. Sin explorador llega hasta el pie; con él la lista cede espacio:
    /// como mucho el 40 % libre, en filas enteras y con al menos una.
    pub(crate) fn list_end(usable_h: usize, rows: usize, explorer_visible: bool) -> usize {
        let max = Self::list_max_y(usable_h);
        if !explorer_visible {
            return max;
        }
        let stride = Self::TAB_ITEM_H + Self::TAB_SPACING;
        let cap_rows = ((max - Self::LIST_START_Y) * 2 / 5 / stride).max(1);
        Self::LIST_START_Y + rows.clamp(1, cap_rows) * stride
    }

    /// Rectángulo del explorador alojado bajo la lista de sesiones.
    #[allow(dead_code)]
    pub(crate) fn explorer_rect_after(usable_h: usize, list_end: usize) -> Rect {
        Self::explorer_rect_after_with_width(Self::WIDTH, usable_h, list_end)
    }

    /// Rectángulo del explorador alojado bajo la lista de sesiones para un ancho de panel dado.
    pub(crate) fn explorer_rect_after_with_width(
        panel_w: usize,
        usable_h: usize,
        list_end: usize,
    ) -> Rect {
        let top = list_end + 8;
        let bottom = Self::content_bottom(usable_h);
        (
            Self::RAIL_W,
            top,
            // Termina en el borde interior de la tarjeta del panel, no bajo su margen.
            panel_w.saturating_sub(Self::RAIL_W + Self::CARD_INSET),
            bottom.saturating_sub(top),
        )
    }

    #[inline]
    pub(crate) fn visible_items(list_end: usize) -> usize {
        if list_end > Self::LIST_START_Y {
            (list_end - Self::LIST_START_Y) / (Self::TAB_ITEM_H + Self::TAB_SPACING)
        } else {
            1
        }
    }

    /// Y absoluta de la fila `vis_idx` (índice dentro de la ventana visible).
    #[inline]
    pub(crate) fn row_y(vis_idx: usize) -> usize {
        Self::LIST_START_Y + vis_idx * (Self::TAB_ITEM_H + Self::TAB_SPACING)
    }

    /// Borde inferior del contenido del panel (sin el pie, que ya no existe).
    #[inline]
    pub(crate) fn content_bottom(usable_h: usize) -> usize {
        usable_h.saturating_sub(8)
    }

    /// Botón de ajustes al fondo del riel (modo vertical): abre Ajustes.
    #[inline]
    pub(crate) fn settings_btn_rect(usable_h: usize) -> Rect {
        let size = 30;
        (
            (Self::RAIL_W - size) / 2,
            usable_h.saturating_sub(size + 16),
            size,
            size,
        )
    }

    /// Rectángulo de la fila `idx` de la lista, dado el desplazamiento actual.
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn tab_rect(idx: usize, scroll_offset: usize) -> Rect {
        Self::tab_rect_with_width(idx, scroll_offset, Self::WIDTH)
    }

    /// Rectángulo de la fila `idx` de la lista para un ancho de panel dado.
    #[inline]
    pub(crate) fn tab_rect_with_width(idx: usize, scroll_offset: usize, panel_w: usize) -> Rect {
        (
            Self::PAD_X,
            Self::row_y(idx.saturating_sub(scroll_offset)),
            panel_w.saturating_sub(Self::PAD_X + Self::ROW_PAD_R),
            Self::TAB_ITEM_H,
        )
    }

    #[inline]
    pub(crate) fn close_btn_rect(tab: Rect) -> Rect {
        (
            tab.0 + tab.2.saturating_sub(Self::CLOSE_SIZE + 6),
            tab.1 + (tab.3.saturating_sub(Self::CLOSE_SIZE)) / 2,
            Self::CLOSE_SIZE,
            Self::CLOSE_SIZE,
        )
    }

    #[inline]
    pub(crate) fn tab_at_y(
        my: usize,
        list_end: usize,
        scroll_offset: usize,
        total_tabs: usize,
    ) -> Option<usize> {
        if my < Self::LIST_START_Y || my >= list_end {
            return None;
        }
        let rel = my - Self::LIST_START_Y;
        let stride = Self::TAB_ITEM_H + Self::TAB_SPACING;
        if rel % stride >= Self::TAB_ITEM_H {
            return None;
        }
        let idx = rel / stride + scroll_offset;
        (idx < total_tabs).then_some(idx)
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}

/// Barra de scroll vertical de toda la interfaz (panel, explorador, terminal, visor y Ajustes): un solo
/// carril, ancho y posición, para que todas se vean y respondan igual. Dibujo, clic, hover y arrastre
/// parten de `vertical`.
pub(crate) struct ScrollbarRects;

impl ScrollbarRects {
    /// Ancho del carril y del pulgar.
    pub(crate) const WIDTH: usize = 8;
    /// Aire entre el carril y el borde de la tarjeta.
    const GAP: usize = 2;
    /// Pulgar mínimo, para que siga siendo agarrable con listas muy largas.
    const MIN_THUMB: usize = 18;
    /// Espacio reservado a la derecha para el carril de la barra de scroll y el borde de la tarjeta.
    pub(crate) const RESERVED_RIGHT: usize = Self::WIDTH + Self::GAP + SidebarRects::CARD_INSET;

    /// X del carril de un contenido que termina en `right`: junto al borde de la tarjeta
    /// (`SidebarRects::CARD_INSET`), que recorta ese margen.
    #[inline]
    pub(crate) fn track_x(right: usize) -> usize {
        right.saturating_sub(Self::WIDTH + Self::GAP + SidebarRects::CARD_INSET)
    }

    /// Carril `(x, y, WIDTH, h)` y pulgar de una vista de `visible` unidades sobre `total`, desplazada
    /// `offset` unidades desde arriba. `None` si todo cabe.
    pub(crate) fn vertical(
        right: usize,
        y: usize,
        h: usize,
        total: usize,
        visible: usize,
        offset: usize,
    ) -> Option<(Rect, Rect)> {
        if h == 0 || visible == 0 || total <= visible {
            return None;
        }
        let x = Self::track_x(right);
        let thumb_h = (visible * h / total).max(Self::MIN_THUMB).min(h);
        let max_offset = total - visible;
        let thumb_y = y + offset.min(max_offset) * (h - thumb_h) / max_offset;
        Some(((x, y, Self::WIDTH, h), (x, thumb_y, Self::WIDTH, thumb_h)))
    }

    /// Zona que responde como barra (clic y hover): el carril, 8 px a su izquierda y el margen hasta el
    /// borde, para no agarrar por error lo que queda detrás.
    #[inline]
    pub(crate) fn hit(track: Rect, right: usize, mx: usize, my: usize) -> bool {
        mx + 8 >= track.0 && mx < right && my >= track.1 && my < track.1 + track.3
    }
}

/// Geometría del panel del modo de pestañas superiores: el explorador ocupa todo el panel, que mide
/// `SidebarRects::WIDTH`. Es local al panel.
pub(crate) struct FilesPanelRects;

impl FilesPanelRects {
    /// Rectángulo del explorador alojado: el interior de la tarjeta del panel (sin su margen), hasta el fondo.
    #[allow(dead_code)]
    pub(crate) fn explorer_rect(usable_h: usize) -> Rect {
        Self::explorer_rect_with_width(SidebarRects::WIDTH, usable_h)
    }

    /// Rectángulo del explorador alojado para un ancho de panel dado.
    pub(crate) fn explorer_rect_with_width(panel_w: usize, usable_h: usize) -> Rect {
        let inset = SidebarRects::CARD_INSET;
        let top = SidebarRects::TOP + inset;
        let bottom = SidebarRects::content_bottom(usable_h);
        (
            inset,
            top,
            panel_w.saturating_sub(2 * inset),
            bottom.saturating_sub(top),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StatusBarRects {
    pub(crate) ai_badge: Rect,
    pub(crate) update_badge: Option<Rect>,
    /// Medidores de límites; `0` de ancho cuando no hay sitio y no se dibujan.
    pub(crate) meters: Rect,
    /// Botón de selección de modo pantalla encendida (Activo, Agente, Apagado) en la esquina inferior derecha.
    pub(crate) keep_awake_btn: Rect,
}

impl StatusBarRects {
    pub(crate) const BAR_H: usize = 28;
    #[allow(dead_code)]
    pub(crate) const POPUP_W: usize = 392;
    /// Margen derecho del grupo de estado (las dimensiones cierran la barra).
    pub(crate) const RIGHT_PAD: usize = 14;
    /// Separación entre bloques del grupo derecho (badge, medidores, dimensiones).
    pub(crate) const GROUP_GAP: usize = 20;
    /// Ancho de la barra de un medidor y alto del carril.
    pub(crate) const METER_TRACK_W: usize = 82;
    pub(crate) const METER_TRACK_H: usize = 6;
    /// Ancho mínimo que debe quedar para la ruta y la rama: por debajo, los medidores no se dibujan.
    pub(crate) const MIN_CONTENT_W: usize = 200;

    /// Rectángulos de la barra de estado para un ancho de etiqueta `label_w` (en px),
    /// incluyendo el indicador opcional de nueva versión disponible
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn compute(
        width: usize,
        height: usize,
        label_w: usize,
        update_w: Option<usize>,
    ) -> Self {
        Self::compute_full(width, height, label_w, update_w, 0, 0, 0)
    }

    /// Como `compute_full`, con `keep_awake_w = 0`.
    #[allow(dead_code)]
    pub(crate) fn compute_with_status(
        width: usize,
        height: usize,
        label_w: usize,
        update_w: Option<usize>,
        dims_w: usize,
        meters_w: usize,
    ) -> Self {
        Self::compute_full(width, height, label_w, update_w, dims_w, meters_w, 0)
    }

    /// Como `compute`, reservando a la derecha del badge las dimensiones de la rejilla (`dims_w`), los
    /// medidores de límites (`meters_w`) y el selector de modo de pantalla encendida (`keep_awake_w`).
    /// El grupo se ancla al borde derecho y el badge queda a su izquierda.
    pub(crate) fn compute_full(
        width: usize,
        height: usize,
        label_w: usize,
        update_w: Option<usize>,
        dims_w: usize,
        meters_w: usize,
        keep_awake_w: usize,
    ) -> Self {
        let bar_y = height.saturating_sub(Self::BAR_H);
        let badge_w = (label_w + 24).min(width);

        let (kw_block, keep_awake_btn) = if keep_awake_w > 0 {
            let kw_x = width.saturating_sub(Self::RIGHT_PAD + keep_awake_w);
            (
                keep_awake_w + Self::GROUP_GAP,
                (kw_x, bar_y, keep_awake_w, Self::BAR_H),
            )
        } else {
            (0, (0, bar_y, 0, 0))
        };

        let dims_block = if dims_w > 0 {
            dims_w + Self::GROUP_GAP
        } else {
            0
        };
        let mut meters_block = if meters_w > 0 {
            meters_w + Self::GROUP_GAP
        } else {
            0
        };
        // El grupo derecho no puede comerse el espacio de la ruta y la rama: si al colocar el badge no
        // queda sitio para la izquierda, los medidores (lo opcional) se caen antes que el badge.
        let mut badge_x =
            width.saturating_sub(Self::RIGHT_PAD + kw_block + dims_block + meters_block + badge_w);
        if badge_x < Self::MIN_CONTENT_W && meters_block > 0 {
            meters_block = 0;
            badge_x = width.saturating_sub(Self::RIGHT_PAD + kw_block + dims_block + badge_w);
        }
        if badge_x < Self::MIN_CONTENT_W && dims_block > 0 {
            badge_x = width.saturating_sub(Self::RIGHT_PAD + kw_block + badge_w);
        }
        let badge_right = badge_x + badge_w;
        let ai_badge = (badge_x, bar_y, badge_w, Self::BAR_H);

        let meters = if meters_block > 0 {
            (badge_right + Self::GROUP_GAP, bar_y, meters_w, Self::BAR_H)
        } else {
            (0, bar_y, 0, Self::BAR_H)
        };

        let update_badge = update_w.map(|uw| {
            let ub_w = uw + 26;
            // 16 px hasta el badge: la cápsula del agente sobresale 6 px de su rectángulo y quedan 10 de aire.
            let ub_x = badge_x.saturating_sub(ub_w + 16);
            (ub_x, bar_y, ub_w, Self::BAR_H)
        });

        Self {
            ai_badge,
            update_badge,
            meters,
            keep_awake_btn,
        }
    }

    /// Rectángulo del menú flotante de consumo encima de la barra de estado.
    #[allow(dead_code)]
    pub(crate) fn popup_rect(width: usize, height: usize, popup_h: usize) -> Rect {
        let w = Self::POPUP_W.min(width.saturating_sub(16));
        let x = width.saturating_sub(w + 10);
        let bar_y = height.saturating_sub(Self::BAR_H);
        let y = bar_y.saturating_sub(popup_h + 6);
        (x, y, w, popup_h)
    }

    /// Ancho, alto de cada opción y margen interior del selector de pantalla encendida.
    pub(crate) const KA_POPUP_W: usize = 290;
    pub(crate) const KA_ROW_H: usize = 46;
    pub(crate) const KA_PAD: usize = 6;

    /// Rectángulo del selector de pantalla encendida: se despliega hacia arriba, pegado a la derecha.
    pub(crate) fn keep_awake_popup_rect(width: usize, height: usize) -> Rect {
        let w = Self::KA_POPUP_W.min(width.saturating_sub(16));
        let h = 3 * Self::KA_ROW_H + 2 * Self::KA_PAD;
        let x = width.saturating_sub(w + Self::RIGHT_PAD);
        let y = height.saturating_sub(Self::BAR_H + h + 6);
        (x, y, w, h)
    }

    /// Rectángulo de la opción `i` (0..3) dentro del selector.
    pub(crate) fn keep_awake_option_rect(width: usize, height: usize, i: usize) -> Rect {
        let (x, y, w, _) = Self::keep_awake_popup_rect(width, height);
        (
            x + Self::KA_PAD,
            y + Self::KA_PAD + i * Self::KA_ROW_H,
            w.saturating_sub(2 * Self::KA_PAD),
            Self::KA_ROW_H,
        )
    }

    /// Opción del selector bajo el ratón.
    pub(crate) fn keep_awake_option_at(
        width: usize,
        height: usize,
        mx: usize,
        my: usize,
    ) -> Option<usize> {
        (0..3).find(|&i| Self::inside(mx, my, Self::keep_awake_option_rect(width, height, i)))
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }

    /// Mantiene abierto el menú flotante si el ratón está en el badge, en la tarjeta o en el puente de 6px entre ambos.
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn inside_badge_or_popup(mx: usize, my: usize, badge: Rect, popup: Rect) -> bool {
        let bridge = (popup.0, popup.1, popup.2, popup.3 + 8);
        Self::inside(mx, my, badge) || Self::inside(mx, my, bridge)
    }
}

/// Botón de la cabecera de un panel dividido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneHeaderButton {
    /// Dividir el panel en dos columnas
    Split,
    /// Maximizar el panel (ocultar el resto) o restaurar el reparto
    Zoom,
    /// Cerrar el panel
    Close,
}

/// Geometría de la cabecera de un panel dividido (38 px): icono, título, subtítulo y los tres
/// botones de la derecha. Es local al hueco del panel y la comparten dibujo, clic y hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaneHeaderRects {
    pub(crate) header: Rect,
    pub(crate) split_btn: Rect,
    pub(crate) zoom_btn: Rect,
    pub(crate) close_btn: Rect,
}

impl PaneHeaderRects {
    pub(crate) const HEIGHT: usize = 38;
    /// Línea inferior que separa la cabecera del terminal
    pub(crate) const BORDER: usize = 1;
    /// Lo que hay que descontar del hueco del panel para obtener el área del terminal
    pub(crate) const CONTENT_OFFSET: usize = Self::HEIGHT + Self::BORDER;
    pub(crate) const BTN: usize = 26;
    pub(crate) const BTN_GAP: usize = 10;
    pub(crate) const PAD_RIGHT: usize = 18;
    /// Centro del icono y comienzo del título, relativos al borde izquierdo del panel
    #[allow(dead_code)]
    pub(crate) const ICON_X: usize = 24;
    pub(crate) const TEXT_X: usize = 20;

    #[allow(dead_code)]
    pub(crate) fn compute(x: usize, y: usize, w: usize) -> Self {
        let right = x + w;
        let by = y + (Self::HEIGHT.saturating_sub(Self::BTN)) / 2;
        let close_btn = (
            right.saturating_sub(Self::PAD_RIGHT + Self::BTN),
            by,
            Self::BTN,
            Self::BTN,
        );
        let zoom_btn = (
            close_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        let split_btn = (
            zoom_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        Self {
            header: (x, y, w, Self::HEIGHT),
            split_btn,
            zoom_btn,
            close_btn,
        }
    }

    /// Botón bajo el ratón. Dividir y maximizar solo existen en el panel enfocado; cerrar, en todos.
    pub(crate) fn button_at(
        &self,
        mx: usize,
        my: usize,
        focused: bool,
    ) -> Option<PaneHeaderButton> {
        if !Self::inside(mx, my, self.header) {
            return None;
        }
        if Self::inside(mx, my, self.close_btn) {
            return Some(PaneHeaderButton::Close);
        }
        if focused && Self::inside(mx, my, self.zoom_btn) {
            return Some(PaneHeaderButton::Zoom);
        }
        if focused && Self::inside(mx, my, self.split_btn) {
            return Some(PaneHeaderButton::Split);
        }
        None
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}

impl SettingsPageRects {
    /// Rectángulo de la fila `i` (0..THEME_ROWS) de la lista de temas
    pub(crate) fn theme_row(&self, i: usize) -> (usize, usize, usize, usize) {
        let l = self.theme_list;
        (l.0, l.1 + i * Self::THEME_ROW_H, l.2, Self::THEME_ROW_H)
    }

    /// Fila de la lista de temas bajo el ratón, si hay alguna
    pub(crate) fn theme_row_at(&self, mx: usize, my: usize) -> Option<usize> {
        Self::inside(mx, my, self.theme_list).then(|| (my - self.theme_list.1) / Self::THEME_ROW_H)
    }
}

/// Pestañas «Archivos» · «Cambios» que encabezan el explorador estilo DataGrip.
/// La comparten dibujo, clic y hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ExplorerTabsRects {
    pub(crate) files: Rect,
    pub(crate) changes: Rect,
}

impl ExplorerTabsRects {
    #[allow(dead_code)]
    pub(crate) fn compute(panel_x: usize, panel_y: usize, panel_w: usize, header_h: usize) -> Self {
        let inner = panel_w.saturating_sub(16);
        let half = inner / 2;
        Self {
            files: (panel_x + 8, panel_y, half, header_h),
            changes: (
                panel_x + 8 + half,
                panel_y,
                inner.saturating_sub(half),
                header_h,
            ),
        }
    }

    /// `Some(true)` sobre «Cambios», `Some(false)` sobre «Archivos».
    pub(crate) fn tab_at(&self, mx: usize, my: usize) -> Option<bool> {
        let inside = |r: Rect| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3;
        if inside(self.files) {
            Some(false)
        } else if inside(self.changes) {
            Some(true)
        } else {
            None
        }
    }
}

/// Geometría de la paleta de comandos y búsqueda central (Ctrl + K, WP5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaletteRects {
    pub(crate) card: Rect,
    pub(crate) input: Rect,
    pub(crate) divider_y: usize,
    pub(crate) list: Rect,
    pub(crate) items_count: usize,
    pub(crate) item_h: usize,
    pub(crate) scrollbar: Option<(Rect, Rect)>,
}

impl PaletteRects {
    pub(crate) const WIDTH: usize = 560;
    pub(crate) const INPUT_H: usize = 46;
    pub(crate) const ITEM_H: usize = 40;
    pub(crate) const MAX_VISIBLE: usize = 8;
    pub(crate) const CARD_RADIUS: usize = super::tokens::RADIUS_CONTROL;

    #[allow(dead_code)]
    #[allow(dead_code)]
    pub(crate) fn compute(win_w: usize, win_h: usize, items_count: usize) -> Self {
        Self::compute_with_scroll(win_w, win_h, items_count, 0)
    }

    pub(crate) fn compute_with_scroll(
        win_w: usize,
        win_h: usize,
        items_count: usize,
        scroll_offset: usize,
    ) -> Self {
        let card_w = Self::WIDTH.min(win_w.saturating_sub(40));
        let card_x = (win_w.saturating_sub(card_w)) / 2;
        let card_y = 54.min(win_h / 6);
        let visible_items = items_count.min(Self::MAX_VISIBLE);
        let list_h = visible_items * Self::ITEM_H;
        let card_h = Self::INPUT_H
            + if visible_items > 0 {
                list_h + 8
            } else if items_count == 0 {
                44
            } else {
                0
            };

        let card = (card_x, card_y, card_w, card_h);
        let input = (card_x, card_y, card_w, Self::INPUT_H);
        let divider_y = card_y + Self::INPUT_H;
        let list = (card_x, divider_y + 4, card_w, list_h);

        let scrollbar = if items_count > Self::MAX_VISIBLE {
            ScrollbarRects::vertical(
                card_x + card_w,
                divider_y + 6,
                list_h.saturating_sub(4),
                items_count,
                Self::MAX_VISIBLE,
                scroll_offset,
            )
        } else {
            None
        };

        Self {
            card,
            input,
            divider_y,
            list,
            items_count,
            item_h: Self::ITEM_H,
            scrollbar,
        }
    }

    pub(crate) fn item_rect(&self, idx: usize) -> Rect {
        let y = self.list.1 + idx * self.item_h;
        let w = if self.scrollbar.is_some() {
            self.card.2.saturating_sub(26)
        } else {
            self.card.2.saturating_sub(12)
        };
        (self.card.0 + 6, y, w, self.item_h)
    }

    #[allow(dead_code)]
    pub(crate) fn item_at(&self, mx: usize, my: usize) -> Option<usize> {
        self.item_at_with_offset(mx, my, 0)
    }

    pub(crate) fn item_at_with_offset(
        &self,
        mx: usize,
        my: usize,
        scroll_offset: usize,
    ) -> Option<usize> {
        if let Some((track, _)) = self.scrollbar {
            let right = self.card.0 + self.card.2;
            if ScrollbarRects::hit(track, right, mx, my) {
                return None;
            }
        }
        let (lx, ly, lw, lh) = self.list;
        let list_w = if self.scrollbar.is_some() {
            lw.saturating_sub(26)
        } else {
            lw
        };
        if mx >= lx && mx < lx + list_w && my >= ly && my < ly + lh {
            let visual_idx = (my - ly) / self.item_h;
            if visual_idx < self.items_count.min(Self::MAX_VISIBLE) {
                let real_idx = scroll_offset + visual_idx;
                if real_idx < self.items_count {
                    return Some(real_idx);
                }
            }
        }
        None
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}
