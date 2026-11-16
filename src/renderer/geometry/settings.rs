//! Geometría de la página y el modal de Ajustes.

use super::{rect_contains, Rect, ScrollbarRects, SidebarRects};
use crate::renderer::state::SettingsSection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SettingsPageRects {
    pub nav_appearance: (usize, usize, usize, usize),
    pub nav_terminal: (usize, usize, usize, usize),
    pub nav_database: (usize, usize, usize, usize),
    pub nav_performance: (usize, usize, usize, usize),
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

    // Bases de datos (Screen 15)
    pub db_prod_read_only_btn: (usize, usize, usize, usize),
    pub db_write_window_minus_btn: (usize, usize, usize, usize),
    pub db_write_window_plus_btn: (usize, usize, usize, usize),
    pub db_write_window_value_box: (usize, usize, usize, usize),
    pub db_written_confirmation_btn: (usize, usize, usize, usize),
    pub db_warn_no_where_btn: (usize, usize, usize, usize),
    pub db_enforce_transaction_btn: (usize, usize, usize, usize),
    pub db_backup_original_rows_btn: (usize, usize, usize, usize),
    pub db_warn_unsafe_pem_permissions_btn: (usize, usize, usize, usize),
    pub db_cert_warn_days_minus_btn: (usize, usize, usize, usize),
    pub db_cert_warn_days_plus_btn: (usize, usize, usize, usize),
    pub db_cert_warn_days_value_box: (usize, usize, usize, usize),
    pub db_certs_folder_btn: (usize, usize, usize, usize),

    pub db_page_size_minus_btn: (usize, usize, usize, usize),
    pub db_page_size_plus_btn: (usize, usize, usize, usize),
    pub db_page_size_value_box: (usize, usize, usize, usize),
    pub db_query_timeout_minus_btn: (usize, usize, usize, usize),
    pub db_query_timeout_plus_btn: (usize, usize, usize, usize),
    pub db_query_timeout_value_box: (usize, usize, usize, usize),
    pub db_query_row_limit_minus_btn: (usize, usize, usize, usize),
    pub db_query_row_limit_plus_btn: (usize, usize, usize, usize),
    pub db_query_row_limit_value_box: (usize, usize, usize, usize),
    pub db_slow_query_threshold_minus_btn: (usize, usize, usize, usize),
    pub db_slow_query_threshold_plus_btn: (usize, usize, usize, usize),
    pub db_slow_query_threshold_value_box: (usize, usize, usize, usize),
    pub db_autocomplete_btn: (usize, usize, usize, usize),
    pub db_save_passwords_btn: (usize, usize, usize, usize),
    pub db_save_query_history_btn: (usize, usize, usize, usize),

    // Rendimiento (Screen 14 / Brief F8)
    pub perf_show_cpu_btn: (usize, usize, usize, usize),
    pub perf_show_ram_btn: (usize, usize, usize, usize),
    pub perf_show_gpu_btn: (usize, usize, usize, usize),
    pub perf_show_disk_btn: (usize, usize, usize, usize),
    pub perf_show_net_btn: (usize, usize, usize, usize),
    pub perf_sample_interval_minus_btn: (usize, usize, usize, usize),
    pub perf_sample_interval_plus_btn: (usize, usize, usize, usize),
    pub perf_sample_interval_value_box: (usize, usize, usize, usize),
    pub perf_enable_alerts_btn: (usize, usize, usize, usize),
    pub perf_cpu_threshold_minus_btn: (usize, usize, usize, usize),
    pub perf_cpu_threshold_plus_btn: (usize, usize, usize, usize),
    pub perf_cpu_threshold_value_box: (usize, usize, usize, usize),
    pub perf_ram_threshold_minus_btn: (usize, usize, usize, usize),
    pub perf_ram_threshold_plus_btn: (usize, usize, usize, usize),
    pub perf_ram_threshold_value_box: (usize, usize, usize, usize),
    pub perf_disk_threshold_minus_btn: (usize, usize, usize, usize),
    pub perf_disk_threshold_plus_btn: (usize, usize, usize, usize),
    pub perf_disk_threshold_value_box: (usize, usize, usize, usize),
    pub perf_agent_input_box: (usize, usize, usize, usize),
    pub perf_agent_reset_btn: (usize, usize, usize, usize),
    pub shortcuts_rows: [(usize, usize, usize, usize); 16],
    pub shortcuts_reset_btn: (usize, usize, usize, usize),

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
    pub(crate) fn content_height(section: SettingsSection) -> usize {
        use SettingsSection::*;
        match section {
            Appearance => 1012,
            Terminal => 894,
            Database => 860,
            Performance => 680,
            Shortcuts => 424,
            About => 360,
        }
    }

    /// Máximo desplazamiento vertical para un viewport de `viewport_h` píxeles.
    pub(crate) fn max_scroll(section: SettingsSection, viewport_h: usize) -> usize {
        Self::content_height(section).saturating_sub(viewport_h)
    }

    /// Pista y pulgar de la barra de scroll de Ajustes (compartidos por dibujo, clic y arrastre)
    pub(crate) fn scrollbar(
        section: SettingsSection,
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

    /// Ancho de la columna de navegación lateral del modal.
    pub(crate) const NAV_W: usize = 184;
    pub(crate) const MODAL_MAX_W: usize = 980;
    pub(crate) const MODAL_MAX_H: usize = 680;

    /// Región `(x, y, w, h)` del modal de Ajustes: centrado en la ventana, con margen a los bordes.
    pub(crate) fn modal_region(win_w: usize, win_h: usize) -> Rect {
        let w = Self::MODAL_MAX_W.min(win_w.saturating_sub(80));
        let h = Self::MODAL_MAX_H.min(win_h.saturating_sub(120));
        (
            (win_w.saturating_sub(w)) / 2,
            (win_h.saturating_sub(h)) / 2,
            w,
            h,
        )
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
        r.nav_database = row(2);
        r.nav_performance = row(3);
        r.nav_shortcuts = row(4);
        r.nav_about = row(5);
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

        let nav_w = card_w / 6;
        let nav_appearance = (content_x, nav_y, nav_w, nav_h);
        let nav_terminal = (content_x + nav_w, nav_y, nav_w, nav_h);
        let nav_database = (content_x + nav_w * 2, nav_y, nav_w, nav_h);
        let nav_performance = (content_x + nav_w * 3, nav_y, nav_w, nav_h);
        let nav_shortcuts = (content_x + nav_w * 4, nav_y, nav_w, nav_h);
        let nav_about = (content_x + nav_w * 5, nav_y, nav_w, nav_h);

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

        // Dos columnas para Bases de datos y Rendimiento (Screen 14 y 15)
        let col_gap = 28;
        let col_w = (card_w.saturating_sub(col_gap)) / 2;
        let col1_x = content_x;
        let col2_x = content_x + col_w + col_gap;
        let db_row_h = 50;

        // DB Columna 1 (Seguridad en producción)
        let db_base_y = card_y + 36;
        let db_r1 = |i: usize| db_base_y + i * db_row_h;
        let db_prod_read_only_btn = (col1_x + col_w.saturating_sub(44), db_r1(0) + 14, 40, 20);
        let db_write_window_minus_btn = (col1_x + col_w.saturating_sub(96), db_r1(1) + 12, 26, 26);
        let db_write_window_value_box = (col1_x + col_w.saturating_sub(68), db_r1(1) + 12, 40, 26);
        let db_write_window_plus_btn = (col1_x + col_w.saturating_sub(26), db_r1(1) + 12, 26, 26);
        let db_written_confirmation_btn =
            (col1_x + col_w.saturating_sub(44), db_r1(2) + 14, 40, 20);
        let db_warn_no_where_btn = (col1_x + col_w.saturating_sub(44), db_r1(3) + 14, 40, 20);
        let db_enforce_transaction_btn = (col1_x + col_w.saturating_sub(44), db_r1(4) + 14, 40, 20);
        let db_backup_original_rows_btn =
            (col1_x + col_w.saturating_sub(44), db_r1(5) + 14, 40, 20);
        let db_warn_unsafe_pem_permissions_btn =
            (col1_x + col_w.saturating_sub(44), db_r1(6) + 14, 40, 20);
        let db_cert_warn_days_minus_btn =
            (col1_x + col_w.saturating_sub(96), db_r1(7) + 12, 26, 26);
        let db_cert_warn_days_value_box =
            (col1_x + col_w.saturating_sub(68), db_r1(7) + 12, 40, 26);
        let db_cert_warn_days_plus_btn = (col1_x + col_w.saturating_sub(26), db_r1(7) + 12, 26, 26);
        let db_certs_folder_btn = (col1_x + col_w.saturating_sub(110), db_r1(8) + 12, 110, 26);

        // DB Columna 2 (Consultas y Privacidad)
        let db_r2 = |i: usize| db_base_y + i * db_row_h;
        let db_page_size_minus_btn = (col2_x + col_w.saturating_sub(96), db_r2(0) + 12, 26, 26);
        let db_page_size_value_box = (col2_x + col_w.saturating_sub(68), db_r2(0) + 12, 40, 26);
        let db_page_size_plus_btn = (col2_x + col_w.saturating_sub(26), db_r2(0) + 12, 26, 26);
        let db_query_timeout_minus_btn = (col2_x + col_w.saturating_sub(96), db_r2(1) + 12, 26, 26);
        let db_query_timeout_value_box = (col2_x + col_w.saturating_sub(68), db_r2(1) + 12, 40, 26);
        let db_query_timeout_plus_btn = (col2_x + col_w.saturating_sub(26), db_r2(1) + 12, 26, 26);
        let db_query_row_limit_minus_btn =
            (col2_x + col_w.saturating_sub(96), db_r2(2) + 12, 26, 26);
        let db_query_row_limit_value_box =
            (col2_x + col_w.saturating_sub(68), db_r2(2) + 12, 40, 26);
        let db_query_row_limit_plus_btn =
            (col2_x + col_w.saturating_sub(26), db_r2(2) + 12, 26, 26);
        let db_slow_query_threshold_minus_btn =
            (col2_x + col_w.saturating_sub(96), db_r2(3) + 12, 26, 26);
        let db_slow_query_threshold_value_box =
            (col2_x + col_w.saturating_sub(68), db_r2(3) + 12, 40, 26);
        let db_slow_query_threshold_plus_btn =
            (col2_x + col_w.saturating_sub(26), db_r2(3) + 12, 26, 26);
        let db_autocomplete_btn = (col2_x + col_w.saturating_sub(44), db_r2(4) + 14, 40, 20);

        let cred_y = db_base_y + 5 * db_row_h + 34;
        let db_save_passwords_btn = (col2_x + col_w.saturating_sub(44), cred_y + 14, 40, 20);
        let db_save_query_history_btn = (
            col2_x + col_w.saturating_sub(44),
            cred_y + db_row_h + 14,
            40,
            20,
        );

        // Rendimiento (Medidores columna 1, Muestreo y alertas columna 2)
        let perf_base_y = card_y + 36;
        let perf_r1 = |i: usize| perf_base_y + i * db_row_h;
        let perf_show_cpu_btn = (col1_x + col_w.saturating_sub(44), perf_r1(0) + 14, 40, 20);
        let perf_show_ram_btn = (col1_x + col_w.saturating_sub(44), perf_r1(1) + 14, 40, 20);
        let perf_show_gpu_btn = (col1_x + col_w.saturating_sub(44), perf_r1(2) + 14, 40, 20);
        let perf_show_disk_btn = (col1_x + col_w.saturating_sub(44), perf_r1(3) + 14, 40, 20);
        let perf_show_net_btn = (col1_x + col_w.saturating_sub(44), perf_r1(4) + 14, 40, 20);

        let perf_r2 = |i: usize| perf_base_y + i * db_row_h;
        let perf_sample_interval_minus_btn =
            (col2_x + col_w.saturating_sub(96), perf_r2(0) + 12, 26, 26);
        let perf_sample_interval_value_box =
            (col2_x + col_w.saturating_sub(68), perf_r2(0) + 12, 40, 26);
        let perf_sample_interval_plus_btn =
            (col2_x + col_w.saturating_sub(26), perf_r2(0) + 12, 26, 26);
        let perf_enable_alerts_btn = (col2_x + col_w.saturating_sub(44), perf_r2(1) + 14, 40, 20);
        let perf_cpu_threshold_minus_btn =
            (col2_x + col_w.saturating_sub(96), perf_r2(2) + 12, 26, 26);
        let perf_cpu_threshold_value_box =
            (col2_x + col_w.saturating_sub(68), perf_r2(2) + 12, 40, 26);
        let perf_cpu_threshold_plus_btn =
            (col2_x + col_w.saturating_sub(26), perf_r2(2) + 12, 26, 26);
        let perf_ram_threshold_minus_btn =
            (col2_x + col_w.saturating_sub(96), perf_r2(3) + 12, 26, 26);
        let perf_ram_threshold_value_box =
            (col2_x + col_w.saturating_sub(68), perf_r2(3) + 12, 40, 26);
        let perf_ram_threshold_plus_btn =
            (col2_x + col_w.saturating_sub(26), perf_r2(3) + 12, 26, 26);
        let perf_disk_threshold_minus_btn =
            (col2_x + col_w.saturating_sub(96), perf_r2(4) + 12, 26, 26);
        let perf_disk_threshold_value_box =
            (col2_x + col_w.saturating_sub(68), perf_r2(4) + 12, 40, 26);
        let perf_disk_threshold_plus_btn =
            (col2_x + col_w.saturating_sub(26), perf_r2(4) + 12, 26, 26);
        let agent_y = perf_r1(5) + 22;
        let perf_agent_reset_btn = (col1_x + col_w.saturating_sub(78), agent_y, 78, 28);
        let perf_agent_input_box = (col1_x, agent_y, col_w.saturating_sub(84), 28);

        let sc_col_gap = 28;
        let sc_col_w = (card_w.saturating_sub(40 + sc_col_gap)) / 2;
        let sc_start_y = card_y + 56;
        let sc_row_h = 40;
        let mut shortcuts_rows = [(0, 0, 0, 0); 16];
        for (idx, slot) in shortcuts_rows.iter_mut().enumerate() {
            let col = idx / 8;
            let row = idx % 8;
            let x = content_x + 14 + col * (sc_col_w + sc_col_gap);
            let y = sc_start_y + row * sc_row_h - 3;
            *slot = (x, y, sc_col_w + 8, sc_row_h - 2);
        }
        let shortcuts_reset_btn = (content_x + card_w.saturating_sub(116), card_y + 10, 96, 26);

        Self {
            nav_appearance,
            nav_terminal,
            nav_database,
            nav_performance,
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
            db_prod_read_only_btn,
            db_write_window_minus_btn,
            db_write_window_plus_btn,
            db_write_window_value_box,
            db_written_confirmation_btn,
            db_warn_no_where_btn,
            db_enforce_transaction_btn,
            db_backup_original_rows_btn,
            db_warn_unsafe_pem_permissions_btn,
            db_cert_warn_days_minus_btn,
            db_cert_warn_days_plus_btn,
            db_cert_warn_days_value_box,
            db_certs_folder_btn,
            db_page_size_minus_btn,
            db_page_size_plus_btn,
            db_page_size_value_box,
            db_query_timeout_minus_btn,
            db_query_timeout_plus_btn,
            db_query_timeout_value_box,
            db_query_row_limit_minus_btn,
            db_query_row_limit_plus_btn,
            db_query_row_limit_value_box,
            db_slow_query_threshold_minus_btn,
            db_slow_query_threshold_plus_btn,
            db_slow_query_threshold_value_box,
            db_autocomplete_btn,
            db_save_passwords_btn,
            db_save_query_history_btn,
            perf_show_cpu_btn,
            perf_show_ram_btn,
            perf_show_gpu_btn,
            perf_show_disk_btn,
            perf_show_net_btn,
            perf_sample_interval_minus_btn,
            perf_sample_interval_plus_btn,
            perf_sample_interval_value_box,
            perf_enable_alerts_btn,
            perf_cpu_threshold_minus_btn,
            perf_cpu_threshold_plus_btn,
            perf_cpu_threshold_value_box,
            perf_ram_threshold_minus_btn,
            perf_ram_threshold_plus_btn,
            perf_ram_threshold_value_box,
            perf_disk_threshold_minus_btn,
            perf_disk_threshold_plus_btn,
            perf_disk_threshold_value_box,
            perf_agent_input_box,
            perf_agent_reset_btn,
            shortcuts_rows,
            shortcuts_reset_btn,
            card_w,
            content_x,
        }
    }

    #[inline]
    pub(crate) fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        rect_contains(rect, mx, my)
    }

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
