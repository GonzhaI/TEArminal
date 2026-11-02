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

/// Rectángulo de la barra de búsqueda del historial (`Ctrl + Shift + F`), arriba a la derecha del panel
#[inline]
pub(crate) fn pane_search_bar_rect(rect_x: usize, rect_y: usize, rect_width: usize) -> Rect {
    let w = 280.min(rect_width.saturating_sub(40));
    let h = 32;
    let x = (rect_x + rect_width).saturating_sub(w + 20);
    (x, rect_y + 10, w, h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TopTabBarRects {
    pub bar_h: usize,
    /// Borde izquierdo de la barra: el panel de Spaces ocupa lo que hay a su izquierda.
    pub left_x: usize,
    pub tabs_start_x: usize,
    pub tab_w: usize,
    pub tab_gap: usize,
    pub plus_btn: (usize, usize, usize, usize),
    /// Píldora «Files»: muestra u oculta la tarjeta del explorador.
    pub files_btn: (usize, usize, usize, usize),
    /// Píldora «Changes»: la tarjeta en su vista de cambios de git.
    pub changes_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
}

impl TopTabBarRects {
    pub(crate) const HEIGHT: usize = 38;
    pub(crate) const TAB_GAP: usize = 3;
    pub(crate) const CLOSE_W: usize = 24;
    pub(crate) const PLUS_W: usize = 28;
    /// Ancho de las píldoras Files y Changes (icono + etiqueta).
    pub(crate) const FILES_W: usize = 124;
    pub(crate) const CHANGES_W: usize = 124;
    /// Alto de las píldoras y de los botones de acción del chrome.
    pub(crate) const ACTION: usize = 30;

    /// `width` es el ancho de la ventana y `left_x` el borde izquierdo de la barra (ancho del panel de
    /// Spaces); las pestañas empiezan alineadas con la tarjeta de contenido.
    #[allow(dead_code)]
    pub(crate) fn compute(width: usize, left_x: usize, tabs_count: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 46;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);

        // De derecha a izquierda: Changes · Files · «+», junto a los controles de ventana.
        let action_y = (bar_h.saturating_sub(Self::ACTION)) / 2;
        let changes_btn = (
            win_min_btn.0.saturating_sub(8 + Self::CHANGES_W),
            action_y,
            Self::CHANGES_W,
            Self::ACTION,
        );
        let files_btn = (
            changes_btn.0.saturating_sub(4 + Self::FILES_W),
            action_y,
            Self::FILES_W,
            Self::ACTION,
        );
        let plus_btn = (
            files_btn.0.saturating_sub(8 + Self::PLUS_W),
            (bar_h - Self::PLUS_W) / 2,
            Self::PLUS_W,
            Self::PLUS_W,
        );

        let tabs_start_x = left_x + SidebarRects::CARD_INSET;
        let available = plus_btn.0.saturating_sub(tabs_start_x + 12);
        let gap = Self::TAB_GAP;
        let usable = available.saturating_sub(tabs_count.saturating_mul(gap));
        let tab_w = usable
            .checked_div(tabs_count)
            .map_or(177, |w| w.clamp(20, 177));

        Self {
            bar_h,
            left_x,
            tabs_start_x,
            tab_w,
            tab_gap: gap,
            plus_btn,
            files_btn,
            changes_btn,
            win_min_btn,
            win_max_btn,
            win_close_btn,
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

    pub font_minus_btn: (usize, usize, usize, usize),
    pub font_plus_btn: (usize, usize, usize, usize),
    pub font_value_box: (usize, usize, usize, usize),
    /// Selector de la familia tipográfica instalada: anterior, siguiente y el nombre entre ambos
    pub font_family_prev_btn: (usize, usize, usize, usize),
    pub font_family_next_btn: (usize, usize, usize, usize),
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
    pub(crate) const CONTENT_TOP: usize = 100;
    /// Filas visibles de la lista de temas, alto de cada una y alto de su tarjeta
    pub(crate) const THEME_ROWS: usize = 7;
    pub(crate) const THEME_ROW_H: usize = 30;
    pub(crate) const THEME_CARD_H: usize = 428;
    pub(crate) const INTERFACE_CARD_H: usize = 170;
    pub(crate) const TYPOGRAPHY_CARD_H: usize = 320;
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
    /// Ancho de la columna de navegación lateral del modal.
    pub(crate) const NAV_W: usize = 184;
    pub(crate) const MODAL_MAX_W: usize = 980;
    pub(crate) const MODAL_MAX_H: usize = 680;

    /// Región `(x, y, w, h)` del modal de Ajustes: centrado en el área a la derecha de `left` (el panel de
    /// Spaces, que el modal no tapa), con margen a los bordes.
    pub(crate) fn modal_region(win_w: usize, win_h: usize, left: usize) -> Rect {
        let area_w = win_w.saturating_sub(left);
        let w = Self::MODAL_MAX_W.min(area_w.saturating_sub(80));
        let h = Self::MODAL_MAX_H.min(win_h.saturating_sub(120));
        (left + (area_w - w) / 2, (win_h - h) / 2, w, h)
    }

    /// Geometría del modal: columna de navegación a la izquierda (`NAV_W`) y el contenido de la página
    /// en el resto. `page_*` son la esquina y el tamaño del modal.
    pub(crate) fn compute_modal(
        page_x: usize,
        page_y: usize,
        page_w: usize,
        page_h: usize,
    ) -> Self {
        let mut r = Self::compute(
            page_x + Self::NAV_W,
            page_y,
            page_w.saturating_sub(Self::NAV_W),
            page_h,
        );
        let (x, w) = (page_x + 12, Self::NAV_W - 24);
        let row = |i: usize| (x, page_y + 76 + i * 38, w, 34);
        r.nav_appearance = row(0);
        r.nav_terminal = row(1);
        r.nav_shortcuts = row(2);
        r.nav_about = row(3);
        r
    }

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

        // Tarjeta 3: Tipografía y ventana (Font Size, UI Font Size, Spacing)
        let card3_y = card2_y + Self::INTERFACE_CARD_H + 14;
        let step_btn_w = 30;
        let step_span = 150;
        let font_family_prev_btn = (ctrl_x, card3_y + 50, step_btn_w, 30);
        let font_family_next_btn = (
            (ctrl_x + step_span).max((content_x + card_w).saturating_sub(20 + step_btn_w)),
            card3_y + 50,
            step_btn_w,
            30,
        );
        let font_minus_btn = (ctrl_x, card3_y + 92, step_btn_w, 30);
        let font_plus_btn = (ctrl_x + step_span, card3_y + 92, step_btn_w, 30);

        let ui_font_minus_btn = (ctrl_x, card3_y + 134, step_btn_w, 30);
        let ui_font_plus_btn = (ctrl_x + step_span, card3_y + 134, step_btn_w, 30);

        let spacing_minus_btn = (ctrl_x, card3_y + 176, step_btn_w, 30);
        let spacing_plus_btn = (ctrl_x + step_span, card3_y + 176, step_btn_w, 30);

        // La opacidad pertenece a «Interfaz», no a tipografía.
        let opacity_minus_btn = (ctrl_x, card2_y + 98, step_btn_w, 30);
        let opacity_plus_btn = (ctrl_x + step_span, card2_y + 98, step_btn_w, 30);

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
            font_minus_btn,
            font_plus_btn,
            font_value_box,
            font_family_prev_btn,
            font_family_next_btn,
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
    /// Selector de modo de diff (Lado a lado / Unificado), visible mientras el diff está activo
    pub diff_mode_btn: Option<(usize, usize, usize, usize)>,
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
        is_diff: bool,
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

        // 4. Selector de modo (Lado a lado / Unificado), solo cuando el diff está abierto
        let diff_mode_btn = if is_diff && has_diff {
            diff_btn.map(|d_btn| {
                let diff_mode_w = 118;
                (
                    d_btn.0.saturating_sub(8 + diff_mode_w),
                    btn_y,
                    diff_mode_w,
                    24,
                )
            })
        } else {
            None
        };

        Self {
            mode_btn,
            md_btn,
            diff_btn,
            diff_mode_btn,
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

/// Geometría para el visor de diff lado a lado (dos columnas sincronizadas: base HEAD vs disco).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SideBySideLayout {
    pub col_header_y: usize,
    pub col_header_h: usize,
    pub content_y: usize,
    pub content_h: usize,
    pub row_h: usize,
    pub char_w: usize,
    // Columna izquierda (HEAD / Base)
    pub left_x: usize,
    pub left_w: usize,
    pub left_gutter_w: usize,
    pub left_sep_x: usize,
    pub left_text_x: usize,
    pub left_text_max_x: usize,
    // Separador central
    pub divider_x: usize,
    // Columna derecha (Disco / Actual)
    pub right_x: usize,
    pub right_w: usize,
    pub right_gutter_w: usize,
    pub right_sep_x: usize,
    pub right_text_x: usize,
    pub right_text_max_x: usize,
    pub visible_cols: usize,
}

impl SideBySideLayout {
    pub(crate) const COL_HEADER_H: usize = 28;

    pub(crate) fn compute(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        row_height: usize,
        char_width: usize,
        total_rows: usize,
    ) -> Self {
        let row_h = row_height.max(16);
        let char_w = char_width.max(7);
        let col_header_y = y + FileViewerLayout::HEADER_H;
        let col_header_h = Self::COL_HEADER_H;
        let content_y = col_header_y + col_header_h;
        let content_h = h.saturating_sub(FileViewerLayout::HEADER_H + col_header_h);

        let half_w = w.saturating_sub(1) / 2;
        let left_x = x;
        let left_w = half_w;
        let divider_x = x + half_w;
        let right_x = divider_x + 1;
        let right_w = (x + w).saturating_sub(right_x);

        let digits = total_rows.to_string().len().max(3);
        let gutter_w = SidebarRects::CARD_INSET + digits * char_w + 16;

        let left_gutter_w = gutter_w.min(left_w / 3);
        let left_sep_x = left_x + left_gutter_w;
        let left_text_x = left_sep_x + 8;
        let left_text_max_x = divider_x.saturating_sub(4);

        let right_gutter_w = gutter_w.min(right_w / 3);
        let right_sep_x = right_x + right_gutter_w;
        let right_text_x = right_sep_x + 8;
        let right_text_max_x = (x + w).saturating_sub(SidebarRects::CARD_INSET + 16);

        let visible_cols = (left_text_max_x.saturating_sub(left_text_x) / char_w).max(1);

        Self {
            col_header_y,
            col_header_h,
            content_y,
            content_h,
            row_h,
            char_w,
            left_x,
            left_w,
            left_gutter_w,
            left_sep_x,
            left_text_x,
            left_text_max_x,
            divider_x,
            right_x,
            right_w,
            right_gutter_w,
            right_sep_x,
            right_text_x,
            right_text_max_x,
            visible_cols,
        }
    }
}

/// Rectángulos de la barra flotante de búsqueda interna en el visor de archivos (`Ctrl + F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FileViewerSearchRects {
    pub bar: (usize, usize, usize, usize),
    pub input: (usize, usize, usize, usize),
    pub text_start_x: usize,
    pub counter: (usize, usize, usize, usize),
    pub prev_btn: (usize, usize, usize, usize),
    pub next_btn: (usize, usize, usize, usize),
    pub close_btn: (usize, usize, usize, usize),
}

impl FileViewerSearchRects {
    pub(crate) fn compute(lay: &FileViewerLayout) -> Self {
        let max_w = lay.text_max_x.saturating_sub(lay.sep_x);
        let bar_w = 296.min(max_w).max(180);
        let bar_h: usize = 32;
        let bar_x = lay.text_max_x.saturating_sub(bar_w + 8);
        let bar_y = lay.content_y + 8;
        let bar = (bar_x, bar_y, bar_w, bar_h);

        let input_x = bar_x + 28;
        let input_w = if bar_w >= 296 {
            110
        } else {
            bar_w.saturating_sub(186)
        };
        let input = (input_x, bar_y + 4, input_w, 24);
        let text_start_x = input_x + 4;

        let counter_x = input_x + input_w + 4;
        let counter_w = 48;
        let counter = (counter_x, bar_y + 4, counter_w, 24);

        let btn_y = bar_y + (bar_h.saturating_sub(22)) / 2;
        let prev_x = counter_x + counter_w + 2;
        let prev_btn = (prev_x, btn_y, 22, 22);

        let next_x = prev_x + 22 + 2;
        let next_btn = (next_x, btn_y, 22, 22);

        let close_x = next_x + 22 + 6;
        let close_btn = (close_x, btn_y, 22, 22);

        Self {
            bar,
            input,
            text_start_x,
            counter,
            prev_btn,
            next_btn,
            close_btn,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
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
    pub(crate) fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }
}

/// Un elemento de la lista del panel de Spaces: la cabecera de un grupo o un Space (posición en `items`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacesSlot {
    Header { remote: bool },
    Space(usize),
}

/// Panel izquierdo de Spaces, a altura completa y anclado a x = 0 (coordenadas de ventana): cabecera con
/// marca, búsqueda y alternar; la lista, dividida en «Este equipo» y «Equipos remotos» (cada grupo con su
/// `+`); y el pie con Ajustes y Add machine. Colapsado es un riel de `RAIL_W` con los mismos controles solo
/// con icono. Dibujo, clic y hover parten de `compute` y `visible_slots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SpacesPanelRects {
    pub width: usize,
    pub collapsed: bool,
    pub toggle_btn: Rect,
    /// Lupa de la cabecera (abre la paleta); vacía en el riel.
    pub search_btn: Rect,
    /// «+» de Este equipo en el riel; expandido cada cabecera de grupo lleva el suyo (`header_plus`).
    pub plus_btn: Rect,
    pub list_top: usize,
    pub list_bottom: usize,
    pub settings_btn: Rect,
    pub machine_btn: Rect,
}

impl SpacesPanelRects {
    pub(crate) const WIDTH: usize = 264;
    pub(crate) const RAIL_W: usize = 44;
    /// La cabecera mide como la barra superior para que las dos filas queden alineadas.
    pub(crate) const HEADER_H: usize = TopTabBarRects::HEIGHT;
    pub(crate) const ROW_H: usize = 30;
    pub(crate) const ROW_GAP: usize = 2;
    pub(crate) const PAD_X: usize = 8;
    pub(crate) const BTN: usize = 28;
    /// Alto de la cabecera de un grupo y lado de su «+».
    pub(crate) const GROUP_H: usize = 26;
    pub(crate) const GROUP_BTN: usize = 24;
    /// Lado de la insignia de letra de cada Space.
    pub(crate) const BADGE: usize = 18;
    const FOOT_H: usize = 28;

    pub(crate) fn width_for(collapsed: bool) -> usize {
        if collapsed {
            Self::RAIL_W
        } else {
            Self::WIDTH
        }
    }

    pub(crate) fn compute(collapsed: bool, win_h: usize) -> Self {
        let width = Self::width_for(collapsed);
        let btn = Self::BTN;
        let head_y = (Self::HEADER_H - btn) / 2;
        let none = (0, 0, 0, 0);
        let foot_x = Self::PAD_X;
        let foot_w = width - 2 * Self::PAD_X;
        let machine_y = win_h.saturating_sub(Self::PAD_X + Self::FOOT_H);
        let settings_y = machine_y.saturating_sub(Self::FOOT_H + 2);
        let machine_btn = (foot_x, machine_y, foot_w, Self::FOOT_H);
        let settings_btn = (foot_x, settings_y, foot_w, Self::FOOT_H);
        let list_bottom = settings_y.saturating_sub(10);
        if collapsed {
            let plus_y = Self::HEADER_H + 6;
            Self {
                width,
                collapsed,
                toggle_btn: (Self::PAD_X, head_y, btn, btn),
                search_btn: none,
                plus_btn: (Self::PAD_X, plus_y, btn, btn),
                list_top: plus_y + btn + 10,
                list_bottom,
                settings_btn,
                machine_btn,
            }
        } else {
            let toggle_btn = (width - Self::PAD_X - btn, head_y, btn, btn);
            Self {
                width,
                collapsed,
                toggle_btn,
                search_btn: (toggle_btn.0 - 2 - btn, head_y, btn, btn),
                plus_btn: none,
                list_top: Self::HEADER_H + 8,
                list_bottom,
                settings_btn,
                machine_btn,
            }
        }
    }

    pub(crate) fn contains(&self, mx: usize, my: usize, win_h: usize) -> bool {
        mx < self.width && my < win_h
    }

    /// «+» de la cabecera de un grupo (solo expandido).
    pub(crate) fn header_plus(header: Rect) -> Rect {
        (
            header.0 + header.2 - Self::GROUP_BTN,
            header.1 + (header.3.saturating_sub(Self::GROUP_BTN)) / 2,
            Self::GROUP_BTN,
            Self::GROUP_BTN,
        )
    }

    /// Todos los elementos de la lista con su alto, en orden: `Este equipo`, sus Spaces y, si hay remotos,
    /// `Equipos remotos` con los suyos. `remote[i]` dice si el Space `i` es remoto; los locales van antes.
    fn slot_heights(&self, remote: &[bool]) -> Vec<(SpacesSlot, usize)> {
        let mut out = Vec::with_capacity(remote.len() + 2);
        let row = Self::ROW_H + Self::ROW_GAP;
        if !self.collapsed {
            out.push((SpacesSlot::Header { remote: false }, Self::GROUP_H + 2));
        }
        let mut added_remote_header = false;
        for (i, &is_remote) in remote.iter().enumerate() {
            if is_remote && !added_remote_header {
                added_remote_header = true;
                // Expandido: aire y cabecera; en el riel solo una raya que separa los grupos
                let h = if self.collapsed {
                    12
                } else {
                    Self::GROUP_H + 12
                };
                out.push((SpacesSlot::Header { remote: true }, h));
            }
            out.push((SpacesSlot::Space(i), row));
        }
        out
    }

    /// Cuántos elementos tiene la lista (para acotar el desplazamiento).
    pub(crate) fn slot_count(&self, remote: &[bool]) -> usize {
        self.slot_heights(remote).len()
    }

    /// Elementos que caben a la vez (una fila de Space de media).
    pub(crate) fn capacity(&self) -> usize {
        (self.list_bottom.saturating_sub(self.list_top) / (Self::ROW_H + Self::ROW_GAP)).max(1)
    }

    /// Elementos visibles tras desplazar `scroll` elementos, con su rectángulo. La cabecera de un grupo
    /// remoto lleva su aire por encima: el rectángulo es solo la franja de la etiqueta.
    pub(crate) fn visible_slots(&self, remote: &[bool], scroll: usize) -> Vec<(SpacesSlot, Rect)> {
        let mut y = self.list_top;
        let mut out = Vec::new();
        let (x, w) = if self.collapsed {
            (Self::PAD_X, Self::BTN)
        } else {
            (Self::PAD_X, self.width - 2 * Self::PAD_X)
        };
        for (slot, h) in self.slot_heights(remote).into_iter().skip(scroll) {
            if y + h > self.list_bottom {
                break;
            }
            let rect = match slot {
                SpacesSlot::Space(_) => (x, y, w, Self::ROW_H),
                SpacesSlot::Header { remote: true } if self.collapsed => (x, y + 5, w, 1),
                SpacesSlot::Header { remote: true } => (x, y + 12, w, Self::GROUP_H),
                SpacesSlot::Header { remote: false } => (x, y, w, Self::GROUP_H),
            };
            out.push((slot, rect));
            y += h;
        }
        out
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
    pub(crate) const KA_POPUP_W: usize = 330;
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
    /// Dividir el panel en dos filas
    SplitHorizontal,
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
    /// Dividir en columnas
    pub(crate) split_btn: Rect,
    /// Dividir en filas
    pub(crate) split_h_btn: Rect,
    pub(crate) zoom_btn: Rect,
    pub(crate) close_btn: Rect,
}

impl PaneHeaderRects {
    pub(crate) const HEIGHT: usize = 38;
    /// Línea inferior que separa la cabecera del terminal
    pub(crate) const BORDER: usize = 1;
    /// Lo que hay que descontar del hueco del panel para obtener el área del terminal (sin padding de tarjeta)
    #[allow(dead_code)]
    pub(crate) const CONTENT_OFFSET: usize = Self::HEIGHT + Self::BORDER;
    pub(crate) const BTN: usize = 26;
    pub(crate) const BTN_GAP: usize = 10;
    pub(crate) const PAD_RIGHT: usize = 18;
    /// Centro del icono y comienzo del título, relativos al borde izquierdo del panel
    #[allow(dead_code)]
    pub(crate) const ICON_X: usize = 24;
    pub(crate) const TEXT_X: usize = 20;

    /// Margen superior del hueco si la cabecera coincide con el borde superior de la tarjeta de contenido.
    /// La tarjeta recorta `SidebarRects::CARD_INSET` px de margen, de modo que la cabecera visible
    /// arranca en `y + top_pad` y mide `HEIGHT` (38 px), alineándose exactamente con el explorador y el visor.
    #[inline]
    pub(crate) fn top_pad(y: usize) -> usize {
        if y <= SidebarRects::TOP {
            SidebarRects::CARD_INSET
        } else {
            0
        }
    }

    #[inline]
    pub(crate) fn header_total_h(y: usize) -> usize {
        Self::top_pad(y) + Self::HEIGHT
    }

    #[inline]
    pub(crate) fn content_offset_for(y: usize) -> usize {
        Self::header_total_h(y) + Self::BORDER
    }

    #[allow(dead_code)]
    pub(crate) fn compute(x: usize, y: usize, w: usize) -> Self {
        let pad = Self::top_pad(y);
        let right = x + w;
        let by = y + pad + (Self::HEIGHT.saturating_sub(Self::BTN)) / 2;
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
        let split_h_btn = (
            zoom_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        let split_btn = (
            split_h_btn.0.saturating_sub(Self::BTN + Self::BTN_GAP),
            by,
            Self::BTN,
            Self::BTN,
        );
        Self {
            header: (x, y + pad, w, Self::HEIGHT),
            split_btn,
            split_h_btn,
            zoom_btn,
            close_btn,
        }
    }

    /// Botón bajo el ratón. Todo panel tiene los cuatro (dividir en columnas · en filas · maximizar ·
    /// cerrar); solo se ven, y por tanto solo se pueden pulsar, con el ratón sobre la cabecera.
    pub(crate) fn button_at(&self, mx: usize, my: usize) -> Option<PaneHeaderButton> {
        if !Self::inside(mx, my, self.header) {
            return None;
        }
        [
            (self.close_btn, PaneHeaderButton::Close),
            (self.zoom_btn, PaneHeaderButton::Zoom),
            (self.split_h_btn, PaneHeaderButton::SplitHorizontal),
            (self.split_btn, PaneHeaderButton::Split),
        ]
        .into_iter()
        .find(|(r, _)| Self::inside(mx, my, *r))
        .map(|(_, b)| b)
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

/// Elemento del modal «Añadir máquina» bajo el ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MachineHover {
    /// Fila de la lista de hosts de `~/.ssh/config`
    Row(usize),
    Input,
    /// Botón «Añadir» del campo `usuario@host`
    TypedAdd,
}

/// Geometría del modal «Añadir máquina», centrado en la ventana. La comparten dibujo, clic y hover.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MachineModalRects {
    pub card: Rect,
    /// Filas de hosts (las `MAX_ROWS` primeras) y su botón «Añadir»
    pub rows: Vec<Rect>,
    pub row_buttons: Vec<Rect>,
    pub list_label_y: usize,
    pub typed_label_y: usize,
    pub input: Rect,
    pub add_btn: Rect,
    pub error_y: usize,
}

impl MachineModalRects {
    pub(crate) const W: usize = 560;
    pub(crate) const PAD: usize = 20;
    const HEAD_H: usize = 88;
    pub(crate) const ROW_H: usize = 40;
    pub(crate) const MAX_ROWS: usize = 5;

    pub(crate) fn compute(win_w: usize, win_h: usize, host_count: usize) -> Self {
        let w = Self::W.min(win_w.saturating_sub(32));
        let n = host_count.min(Self::MAX_ROWS);
        let list_block = if n > 0 { 26 + n * Self::ROW_H + 14 } else { 0 };
        let h = Self::HEAD_H + list_block + 8 + 26 + 36 + 24 + 14;
        let x = (win_w.saturating_sub(w)) / 2;
        let y = (win_h.saturating_sub(h)) / 2;
        let list_label_y = y + Self::HEAD_H;
        let rows_y = list_label_y + 26;
        let rows: Vec<Rect> = (0..n)
            .map(|i| {
                (
                    x + Self::PAD - 8,
                    rows_y + i * Self::ROW_H,
                    w - 2 * Self::PAD + 16,
                    Self::ROW_H,
                )
            })
            .collect();
        let row_buttons = rows
            .iter()
            .map(|r| (r.0 + r.2 - 8 - 76, r.1 + (Self::ROW_H - 28) / 2, 76, 28))
            .collect();
        let typed_label_y = y + Self::HEAD_H + list_block + 8;
        let input = (
            x + Self::PAD,
            typed_label_y + 26,
            w.saturating_sub(2 * Self::PAD + 100),
            36,
        );
        let add_btn = (input.0 + input.2 + 8, input.1, 92, 36);
        Self {
            card: (x, y, w, h),
            rows,
            row_buttons,
            list_label_y,
            typed_label_y,
            input,
            add_btn,
            error_y: input.1 + 36 + 6,
        }
    }

    pub(crate) fn inside(mx: usize, my: usize, r: Rect) -> bool {
        r.2 > 0 && mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
    }

    pub(crate) fn hit(&self, mx: usize, my: usize) -> Option<MachineHover> {
        if let Some(i) = self.rows.iter().position(|r| Self::inside(mx, my, *r)) {
            Some(MachineHover::Row(i))
        } else if Self::inside(mx, my, self.add_btn) {
            Some(MachineHover::TypedAdd)
        } else if Self::inside(mx, my, self.input) {
            Some(MachineHover::Input)
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
