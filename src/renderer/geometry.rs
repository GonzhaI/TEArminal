//! Geometría compartida entre el dibujo y la detección de clics/hover (una sola fuente de verdad).

/// Rectángulo (x, y, ancho, alto) en píxeles
pub type Rect = (usize, usize, usize, usize);

/// Convierte la posición Y deseada de la parte superior del pulgar (`thumb_top_y`) en un `scroll_offset`
/// dentro de `0..=max_offset` para una barra de desplazamiento vertical `(track, thumb)`.
#[inline]
pub fn scroll_offset_for_thumb_y(
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
pub fn pane_scroll_to_bottom_btn_rect(
    rect_x: usize,
    rect_y: usize,
    rect_width: usize,
    rect_height: usize,
) -> Rect {
    let size = 28;
    let margin_right = 26;
    let margin_bottom = 16;
    let x = (rect_x + rect_width).saturating_sub(size + margin_right);
    let y = (rect_y + rect_height).saturating_sub(size + margin_bottom);
    (x, y, size, size)
}

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

        let files_btn = (width.saturating_sub(ctrl_w * 3 + 34), 5, 28, 28);
        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 68), 5, 28, 28);

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
        let files_btn = (width.saturating_sub(ctrl_w * 3 + 34), 5, 28, 28);
        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 68), 5, 28, 28);
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

    pub theme_slot_dark_btn: (usize, usize, usize, usize),
    pub theme_slot_light_btn: (usize, usize, usize, usize),
    pub theme_search_box: (usize, usize, usize, usize),
    pub theme_list: (usize, usize, usize, usize),
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
    pub shell_integration_btn: (usize, usize, usize, usize),
    pub copy_on_select_btn: (usize, usize, usize, usize),
    pub status_bar_toggle_btn: (usize, usize, usize, usize),
    pub file_explorer_toggle_btn: (usize, usize, usize, usize),
    pub restore_session_btn: (usize, usize, usize, usize),
    pub auto_save_btn: (usize, usize, usize, usize),
    pub word_wrap_btn: (usize, usize, usize, usize),
    pub ssh_integration_btn: (usize, usize, usize, usize),
    pub ligatures_btn: (usize, usize, usize, usize),
    pub open_file_btn: (usize, usize, usize, usize),
    pub reset_defaults_btn: (usize, usize, usize, usize),
    pub term_reset_btn: (usize, usize, usize, usize),
    pub card_w: usize,
    pub content_x: usize,
}

impl SettingsPageRects {
    /// Y (relativa a la página) donde empieza el contenido desplazable, bajo el encabezado y la navegación.
    pub const CONTENT_TOP: usize = 128;
    /// Filas visibles de la lista de temas, alto de cada una y alto de su tarjeta
    pub const THEME_ROWS: usize = 6;
    pub const THEME_ROW_H: usize = 26;
    pub const THEME_CARD_H: usize = 306;
    /// Alto de la tarjeta «Comportamiento del terminal»
    pub const TERMINAL_CARD_H: usize = 430;

    /// Alto total del contenido desplazable de una sección (desde `CONTENT_TOP`).
    pub fn content_height(section: super::state::SettingsSection) -> usize {
        use super::state::SettingsSection::*;
        let cards = match section {
            Appearance => 478 + Self::THEME_CARD_H - 150,
            Terminal => Self::TERMINAL_CARD_H + 14 + 110,
            Shortcuts => 434,
            About => 280,
        };
        cards + 24
    }

    /// Máximo desplazamiento vertical para un viewport de `viewport_h` píxeles.
    pub fn max_scroll(section: super::state::SettingsSection, viewport_h: usize) -> usize {
        Self::content_height(section).saturating_sub(viewport_h)
    }

    /// Pista y pulgar de la barra de scroll de Ajustes (compartidos por dibujo, clic y arrastre)
    pub fn scrollbar(
        section: super::state::SettingsSection,
        page_y: usize,
        win_w: usize,
        viewport_h: usize,
        scroll: usize,
    ) -> Option<(Rect, Rect)> {
        let content_h = Self::content_height(section);
        if content_h <= viewport_h || viewport_h == 0 {
            return None;
        }
        let track = (
            win_w.saturating_sub(9),
            page_y + Self::CONTENT_TOP,
            6,
            viewport_h,
        );
        let thumb_h = (viewport_h * viewport_h / content_h).clamp(18.min(viewport_h), viewport_h);
        let max_scroll = content_h - viewport_h;
        let thumb_y = track.1 + scroll.min(max_scroll) * (viewport_h - thumb_h) / max_scroll;
        Some((track, (track.0, thumb_y, track.2, thumb_h)))
    }

    pub fn compute(page_x: usize, page_y: usize, page_w: usize, _page_h: usize) -> Self {
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
        let reset_defaults_btn = (content_x + card_w.saturating_sub(210), page_y + 36, 210, 28);

        let nav_y = page_y + 80;
        let nav_h = 32;

        let nav_w = (card_w.saturating_sub(24)) / 4;
        let nav_appearance = (content_x, nav_y, nav_w, nav_h);
        let nav_terminal = (content_x + nav_w + 8, nav_y, nav_w, nav_h);
        let nav_shortcuts = (content_x + (nav_w + 8) * 2, nav_y, nav_w, nav_h);
        let nav_about = (content_x + (nav_w + 8) * 3, nav_y, nav_w, nav_h);

        let card_y = page_y + 128;

        // Buscador y lista desplazable de temas
        let slot_w = card_w.saturating_sub(48) / 2;
        let theme_slot_dark_btn = (content_x + 20, card_y + 40, slot_w, 28);
        let theme_slot_light_btn = (content_x + 28 + slot_w, card_y + 40, slot_w, 28);
        let theme_search_box = (content_x + 20, card_y + 76, card_w.saturating_sub(40), 28);
        let theme_list = (
            content_x + 20,
            card_y + 112,
            card_w.saturating_sub(40),
            Self::THEME_ROWS * Self::THEME_ROW_H,
        );

        // Tarjeta 2: Modo y Disposición de Pestañas
        let card2_y = card_y + Self::THEME_CARD_H + 10;
        let label_col_w = (card_w as f32 * 0.35).clamp(180.0, 260.0) as usize;
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
        let shell_integration_btn = (ctrl_x, card_y + 80, toggle_w, 28);
        let copy_on_select_btn = (ctrl_x, card_y + 118, toggle_w, 28);
        let status_bar_toggle_btn = (ctrl_x, card_y + 156, toggle_w, 28);
        let file_explorer_toggle_btn = (ctrl_x, card_y + 194, toggle_w, 28);
        let restore_session_btn = (ctrl_x, card_y + 232, toggle_w, 28);
        let auto_save_btn = (ctrl_x, card_y + 270, toggle_w, 28);
        let word_wrap_btn = (ctrl_x, card_y + 308, toggle_w, 28);
        let ssh_integration_btn = (ctrl_x, card_y + 346, toggle_w, 28);
        let ligatures_btn = (ctrl_x, card_y + 384, toggle_w, 28);

        let cfg_y = card_y + Self::TERMINAL_CARD_H + 14;
        let cfg_btn_w = (card_w.saturating_sub(50)) / 2;
        let open_file_btn = (content_x + 20, cfg_y + 44, cfg_btn_w, 32);
        let term_reset_btn = (content_x + 30 + cfg_btn_w, cfg_y + 44, cfg_btn_w, 32);

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
            shell_integration_btn,
            copy_on_select_btn,
            status_bar_toggle_btn,
            file_explorer_toggle_btn,
            restore_session_btn,
            auto_save_btn,
            word_wrap_btn,
            ssh_integration_btn,
            ligatures_btn,
            open_file_btn,
            reset_defaults_btn,
            term_reset_btn,
            card_w,
            content_x,
        }
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileViewerHeaderRects {
    pub mode_btn: (usize, usize, usize, usize),
    pub md_btn: Option<(usize, usize, usize, usize)>,
    /// «Cambios»: abre el diff contra HEAD (solo si el archivo está en un repo)
    pub diff_btn: Option<(usize, usize, usize, usize)>,
}

impl FileViewerHeaderRects {
    pub fn compute(
        x: usize,
        y: usize,
        w: usize,
        _h: usize,
        is_markdown: bool,
        has_diff: bool,
    ) -> Self {
        let mut right_x = (x + w).saturating_sub(16);

        // 1. Botón Vista previa / Código si es .md (icono + texto)
        let md_btn = if is_markdown {
            let md_w = 118;
            let md_x = right_x.saturating_sub(md_w);
            right_x = md_x.saturating_sub(8);
            Some((md_x, y + 8, md_w, 22))
        } else {
            None
        };

        // 2. Botón Lectura / Edición (icono + texto; el atajo Ctrl+E va en la ayuda del menú)
        let mode_w = 98;
        let mode_x = right_x.saturating_sub(mode_w);
        let mode_btn = (mode_x, y + 8, mode_w, 22);

        // 3. Botón Cambios (diff antes vs ahora), a la izquierda del de modo
        let diff_btn = has_diff.then(|| {
            let diff_w = 92;
            (mode_x.saturating_sub(8 + diff_w), y + 8, diff_w, 22)
        });

        Self {
            mode_btn,
            md_btn,
            diff_btn,
        }
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

/// Geometría del área de texto del visor de archivos (una sola fuente para dibujo, clics y scroll).
#[derive(Debug, Clone, Copy)]
pub struct FileViewerLayout {
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
    pub const HEADER_H: usize = 38;

    pub fn compute(
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
        let gutter_w = digits * char_w + 24;
        let sep_x = x + gutter_w;
        let text_x = sep_x + 14;
        let text_max_x = (x + w).saturating_sub(18);
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
    pub fn col_at(&self, line: &str, mx: usize, scroll_x: usize) -> usize {
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
    pub fn hscrollbar(&self, max_scroll: usize, scroll_x: usize) -> Option<(Rect, Rect)> {
        if max_scroll == 0 || self.content_h < 24 {
            return None;
        }
        let track_x = self.sep_x + 6;
        let track_w = self.text_max_x.saturating_sub(track_x);
        let track_h = 6;
        let track_y = self.content_y + self.content_h - track_h - 3;
        let total = max_scroll + self.visible_cols;
        let thumb_w = (track_w * self.visible_cols / total).clamp(18.min(track_w), track_w);
        let travel = track_w - thumb_w;
        let thumb_x = track_x + travel * scroll_x.min(max_scroll) / max_scroll;
        Some((
            (track_x, track_y, track_w, track_h),
            (thumb_x, track_y, thumb_w, track_h),
        ))
    }
}

/// Geometría de la lista vertical de pestañas en la barra lateral (una sola fuente de verdad).
pub struct SidebarRects;

impl SidebarRects {
    pub const TAB_ITEM_H: usize = 36;
    pub const TAB_SPACING: usize = 4;
    pub const LIST_START_Y: usize = 42;
    pub const TOGGLE_H: usize = 30;

    #[inline]
    pub fn list_max_y(usable_h: usize) -> usize {
        usable_h
            .saturating_sub(Self::TOGGLE_H + 8)
            .saturating_sub(8)
    }

    #[inline]
    pub fn visible_items(usable_h: usize) -> usize {
        let max_y = Self::list_max_y(usable_h);
        if max_y > Self::LIST_START_Y {
            (max_y - Self::LIST_START_Y) / (Self::TAB_ITEM_H + Self::TAB_SPACING)
        } else {
            1
        }
    }

    #[inline]
    pub fn tab_at_y(
        my: usize,
        usable_h: usize,
        scroll_offset: usize,
        total_tabs: usize,
    ) -> Option<usize> {
        let max_y = Self::list_max_y(usable_h);
        if my < Self::LIST_START_Y || my >= max_y {
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
}

#[derive(Debug, Clone, Copy)]
pub struct StatusBarRects {
    pub ai_badge: Rect,
    pub update_badge: Option<Rect>,
}

impl StatusBarRects {
    pub const BAR_H: usize = 26;
    pub const POPUP_W: usize = 392;

    /// Rectángulos de la barra de estado para un ancho de etiqueta `label_w` (en px),
    /// incluyendo el indicador opcional de nueva versión disponible
    pub fn compute(width: usize, height: usize, label_w: usize, update_w: Option<usize>) -> Self {
        let bar_y = height.saturating_sub(Self::BAR_H);
        let text_x = width.saturating_sub(label_w + 32);
        let badge_x = text_x.saturating_sub(6);
        let badge_w = (label_w + 30).min(width.saturating_sub(badge_x + 4));
        let ai_badge = (badge_x, bar_y, badge_w, Self::BAR_H);

        let update_badge = update_w.map(|uw| {
            let ub_w = uw + 26;
            let ub_x = badge_x.saturating_sub(ub_w + 8);
            (ub_x, bar_y, ub_w, Self::BAR_H)
        });

        Self {
            ai_badge,
            update_badge,
        }
    }

    /// Rectángulo del menú flotante de consumo encima de la barra de estado.
    pub fn popup_rect(width: usize, height: usize, popup_h: usize) -> Rect {
        let w = Self::POPUP_W.min(width.saturating_sub(16));
        let x = width.saturating_sub(w + 10);
        let bar_y = height.saturating_sub(Self::BAR_H);
        let y = bar_y.saturating_sub(popup_h + 6);
        (x, y, w, popup_h)
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: Rect) -> bool {
        rect.2 > 0
            && rect.3 > 0
            && mx >= rect.0
            && mx < rect.0 + rect.2
            && my >= rect.1
            && my < rect.1 + rect.3
    }

    /// Mantiene abierto el menú flotante si el ratón está en el badge, en la tarjeta o en el puente de 6px entre ambos.
    #[inline]
    pub fn inside_badge_or_popup(mx: usize, my: usize, badge: Rect, popup: Rect) -> bool {
        let bridge = (popup.0, popup.1, popup.2, popup.3 + 8);
        Self::inside(mx, my, badge) || Self::inside(mx, my, bridge)
    }
}

impl SettingsPageRects {
    /// Rectángulo de la fila `i` (0..THEME_ROWS) de la lista de temas
    pub fn theme_row(&self, i: usize) -> (usize, usize, usize, usize) {
        let l = self.theme_list;
        (l.0, l.1 + i * Self::THEME_ROW_H, l.2, Self::THEME_ROW_H)
    }

    /// Fila de la lista de temas bajo el ratón, si hay alguna
    pub fn theme_row_at(&self, mx: usize, my: usize) -> Option<usize> {
        Self::inside(mx, my, self.theme_list).then(|| (my - self.theme_list.1) / Self::THEME_ROW_H)
    }
}
