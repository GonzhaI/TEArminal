//! Geometría compartida para diálogos del catálogo, avisos flotantes (toasts) y vistas de estado.
//!
//! Única fuente de verdad de rectángulos para dibujo, clics y hover (DISENO §5, pantallas 11 y 12).

use super::{rect_contains, Rect};

/// Elemento interactivo del diálogo bajo el cursor del ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogHit {
    Close,
    Input,
    Button(usize),
    CopySql,
    OptionCheckbox(usize),
}

/// Rectángulos de un diálogo modal centrado en la ventana.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DialogRects {
    pub card: Rect,
    pub header: Rect,
    pub close_btn: Rect,
    pub header_meta_rect: Option<Rect>,
    pub title_pos: (usize, usize),
    pub body_rect: Rect,
    pub subtitle_pos: Option<(usize, usize)>,
    pub mono_block: Option<Rect>,
    pub sql_block: Option<Rect>,
    pub impact_table: Option<Rect>,
    pub impact_header: Option<Rect>,
    pub impact_rows: Vec<Rect>,
    pub option_checkboxes: Vec<Rect>,
    pub confirm_label_y: Option<usize>,
    pub confirm_input: Option<Rect>,
    pub confirm_status: Option<Rect>,
    pub footer: Rect,
    pub copy_sql_btn: Option<Rect>,
    pub buttons: Vec<Rect>,
    pub esc_hint_pos: Option<(usize, usize)>,
}

impl DialogRects {
    pub(crate) const HEADER_H: usize = 42;
    pub(crate) const FOOTER_H: usize = 50;
    pub(crate) const BTN_H: usize = 32;
    pub(crate) const BTN_PAD_X: usize = 14;
    pub(crate) const CARD_PAD_X: usize = 16;
    pub(crate) const CARD_PAD_Y: usize = 14;

    /// Calcula la geometría para cualquier especificación declarativa de diálogo.
    pub(crate) fn compute_for_spec(
        win_w: usize,
        win_h: usize,
        spec: &crate::app::DialogSpec,
        button_widths: &[usize],
    ) -> Self {
        if !spec.is_db_confirm() {
            return Self::compute(
                win_w,
                win_h,
                spec.mono_detail.is_some(),
                spec.confirm_keyword.is_some(),
                button_widths,
            );
        }

        // -------------------------------------------------------------------------
        // Geometría especializada para DbConfirm (DISENO §5 pantalla 07 y 10)
        // -------------------------------------------------------------------------
        let card_w = 680.min(win_w.saturating_sub(32));
        let pad_x = 20;
        let body_w = card_w.saturating_sub(pad_x * 2);

        let header_h = 46;
        let title_h = 24;
        let subtitle_h = if spec.subtitle.is_some() { 22 } else { 0 };

        let sql_lines = spec
            .sql_code
            .as_deref()
            .unwrap_or("")
            .lines()
            .count()
            .clamp(2, 6);
        let sql_box_h = 16 + sql_lines * 20;
        let sql_section_h = if spec.sql_code.is_some() {
            20 + sql_box_h + 12
        } else {
            0
        };

        let impact_count = spec.impact_rows.len();
        let impact_rows_total_h = impact_count * 34;
        let impact_section_h = if impact_count > 0 {
            20 + 30 + impact_rows_total_h + 12
        } else {
            0
        };

        let options_section_h = if spec.has_db_options { 3 * 24 + 10 } else { 0 };

        let confirm_section_h = if spec.confirm_keyword.is_some() {
            22 + 36 + 14
        } else {
            0
        };

        let footer_h = 58;

        let total_h = header_h
            + 18
            + title_h
            + subtitle_h
            + sql_section_h
            + impact_section_h
            + options_section_h
            + confirm_section_h
            + footer_h;

        let card_x = win_w.saturating_sub(card_w) / 2;
        let card_y = (win_h.saturating_sub(total_h) / 2).max(24);

        let header = (card_x, card_y, card_w, header_h);
        let close_btn = (
            card_x + card_w.saturating_sub(36),
            card_y + (header_h.saturating_sub(24)) / 2,
            24,
            24,
        );
        let header_meta_rect = if spec.header_meta.is_some() {
            Some((
                card_x + card_w.saturating_sub(220),
                card_y + (header_h.saturating_sub(20)) / 2,
                180,
                20,
            ))
        } else {
            None
        };
        let title_pos = (card_x + 44, card_y + header_h / 2);

        let body_x = card_x + pad_x;
        let mut cur_y = card_y + header_h + 18;

        let body_rect = (body_x, cur_y, body_w, title_h);
        cur_y += title_h;

        let subtitle_pos = if spec.subtitle.is_some() {
            let p = (body_x, cur_y + 10);
            cur_y += subtitle_h + 6;
            Some(p)
        } else {
            None
        };

        let sql_block = if spec.sql_code.is_some() {
            cur_y += 20; // Etiqueta SQL
            let r = (body_x, cur_y, body_w, sql_box_h);
            cur_y += sql_box_h + 14;
            Some(r)
        } else {
            None
        };

        let (impact_table, impact_header, impact_rows) = if impact_count > 0 {
            cur_y += 20; // Etiqueta Impacto
            let table_start_y = cur_y;
            let hdr_r = (body_x, cur_y, body_w, 30);
            cur_y += 30;

            let mut rows = Vec::with_capacity(impact_count);
            for _ in 0..impact_count {
                rows.push((body_x, cur_y, body_w, 34));
                cur_y += 34;
            }
            let tbl_r = (body_x, table_start_y, body_w, cur_y - table_start_y);
            cur_y += 14;
            (Some(tbl_r), Some(hdr_r), rows)
        } else {
            (None, None, Vec::new())
        };

        let option_checkboxes = if spec.has_db_options {
            let mut opts = Vec::with_capacity(3);
            for _ in 0..3 {
                opts.push((body_x, cur_y, body_w, 24));
                cur_y += 24;
            }
            cur_y += 10;
            opts
        } else {
            Vec::new()
        };

        let (confirm_label_y, confirm_input, confirm_status) = if spec.confirm_keyword.is_some() {
            let label_y = cur_y + 10;
            cur_y += 22;
            let status_w = 76;
            let input_w = body_w.saturating_sub(status_w + 10);
            let input_r = (body_x, cur_y, input_w, 36);
            let status_r = (body_x + input_w + 10, cur_y, status_w, 36);
            (Some(label_y), Some(input_r), Some(status_r))
        } else {
            (None, None, None)
        };

        let footer_y = card_y + total_h.saturating_sub(footer_h);
        let footer = (card_x, footer_y, card_w, footer_h);

        let copy_sql_btn = if spec.can_copy_sql {
            Some((body_x, footer_y + (footer_h.saturating_sub(34)) / 2, 88, 34))
        } else {
            None
        };

        let mut btn_right = card_x + card_w.saturating_sub(pad_x);
        let btn_y = footer_y + (footer_h.saturating_sub(34)) / 2;
        let mut buttons = Vec::with_capacity(button_widths.len());

        for &bw in button_widths.iter().rev() {
            let bx = btn_right.saturating_sub(bw.max(88));
            buttons.push((bx, btn_y, bw.max(88), 34));
            btn_right = bx.saturating_sub(10);
        }
        buttons.reverse();

        let esc_hint_pos = Some((btn_right.saturating_sub(84), footer_y + footer_h / 2));

        Self {
            card: (card_x, card_y, card_w, total_h),
            header,
            close_btn,
            header_meta_rect,
            title_pos,
            body_rect,
            subtitle_pos,
            mono_block: None,
            sql_block,
            impact_table,
            impact_header,
            impact_rows,
            option_checkboxes,
            confirm_label_y,
            confirm_input,
            confirm_status,
            footer,
            copy_sql_btn,
            buttons,
            esc_hint_pos,
        }
    }

    /// Calcula la geometría completa de un diálogo estándar a partir del tamaño de la ventana y sus elementos.
    pub(crate) fn compute(
        win_w: usize,
        win_h: usize,
        has_mono: bool,
        has_confirm: bool,
        button_widths: &[usize],
    ) -> Self {
        let card_w = 520.min(win_w.saturating_sub(32));

        // Altura del cuerpo según bloques presentes
        let mut body_content_h = 44; // Texto principal (2-3 líneas)
        if has_mono {
            body_content_h += 42; // Bloque de código / huella
        }
        if has_confirm {
            body_content_h += 66; // Etiqueta + caja de texto + espacio
        }

        let total_h = Self::HEADER_H + Self::CARD_PAD_Y * 2 + body_content_h + Self::FOOTER_H;
        let card_x = win_w.saturating_sub(card_w) / 2;
        let card_y = win_h.saturating_sub(total_h) / 2;

        let header = (card_x, card_y, card_w, Self::HEADER_H);
        let close_btn = (
            card_x + card_w.saturating_sub(36),
            card_y + (Self::HEADER_H.saturating_sub(24)) / 2,
            24,
            24,
        );
        let title_pos = (card_x + 40, card_y + Self::HEADER_H / 2);

        let body_x = card_x + Self::CARD_PAD_X;
        let body_y = card_y + Self::HEADER_H + Self::CARD_PAD_Y;
        let body_w = card_w.saturating_sub(Self::CARD_PAD_X * 2);
        let body_rect = (body_x, body_y, body_w, 42);

        let mut cur_y = body_y + 42;

        let mono_block = if has_mono {
            let r = (body_x, cur_y + 4, body_w, 36);
            cur_y += 44;
            Some(r)
        } else {
            None
        };

        let (confirm_label_y, confirm_input, confirm_status) = if has_confirm {
            let label_y = cur_y + 14;
            let input_y = label_y + 14;
            let status_w = 70;
            let input_w = body_w.saturating_sub(status_w + 8);
            let input_r = (body_x, input_y, input_w, 32);
            let status_r = (body_x + input_w + 8, input_y, status_w, 32);
            (Some(label_y), Some(input_r), Some(status_r))
        } else {
            (None, None, None)
        };

        let footer_y = card_y + total_h.saturating_sub(Self::FOOTER_H);
        let footer = (card_x, footer_y, card_w, Self::FOOTER_H);
        let esc_hint_pos = Some((body_x, footer_y + Self::FOOTER_H / 2));

        // Botones alineados a la derecha del footer
        let mut btn_right = card_x + card_w.saturating_sub(Self::CARD_PAD_X);
        let btn_y = footer_y + (Self::FOOTER_H.saturating_sub(Self::BTN_H)) / 2;
        let mut buttons = Vec::with_capacity(button_widths.len());

        for &bw in button_widths.iter().rev() {
            let bx = btn_right.saturating_sub(bw);
            buttons.push((bx, btn_y, bw, Self::BTN_H));
            btn_right = bx.saturating_sub(8);
        }
        buttons.reverse(); // Restaurar orden original

        Self {
            card: (card_x, card_y, card_w, total_h),
            header,
            close_btn,
            header_meta_rect: None,
            title_pos,
            body_rect,
            subtitle_pos: None,
            mono_block,
            sql_block: None,
            impact_table: None,
            impact_header: None,
            impact_rows: Vec::new(),
            option_checkboxes: Vec::new(),
            confirm_label_y,
            confirm_input,
            confirm_status,
            footer,
            copy_sql_btn: None,
            buttons,
            esc_hint_pos,
        }
    }

    /// Comprueba si las coordenadas `(mx, my)` caen sobre algún elemento interactivo del diálogo.
    pub(crate) fn hit_test(&self, mx: usize, my: usize) -> Option<DialogHit> {
        if rect_contains(self.close_btn, mx, my) {
            return Some(DialogHit::Close);
        }
        if let Some(c_sql) = self.copy_sql_btn {
            if rect_contains(c_sql, mx, my) {
                return Some(DialogHit::CopySql);
            }
        }
        for (i, &chk) in self.option_checkboxes.iter().enumerate() {
            if rect_contains(chk, mx, my) {
                return Some(DialogHit::OptionCheckbox(i));
            }
        }
        if let Some(inp) = self.confirm_input {
            if rect_contains(inp, mx, my) {
                return Some(DialogHit::Input);
            }
        }
        for (i, &btn) in self.buttons.iter().enumerate() {
            if rect_contains(btn, mx, my) {
                return Some(DialogHit::Button(i));
            }
        }
        None
    }
}

/// Geometría para la pila de avisos (toasts) en la esquina inferior derecha.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ToastRects {
    pub cards: Vec<Rect>,
    pub close_btns: Vec<Rect>,
}

impl ToastRects {
    pub(crate) const TOAST_W: usize = 360;
    pub(crate) const TOAST_H: usize = 44;
    pub(crate) const GAP_Y: usize = 8;
    pub(crate) const MARGIN_RIGHT: usize = 16;
    pub(crate) const MARGIN_BOTTOM: usize = 36; // Sobre la barra de estado (28 px + holgura)

    /// Calcula las posiciones de los toasts visibles en pantalla.
    pub(crate) fn compute(win_w: usize, win_h: usize, toast_count: usize) -> Self {
        if toast_count == 0 {
            return Self::default();
        }

        let w = Self::TOAST_W.min(win_w.saturating_sub(32));
        let x = win_w.saturating_sub(w + Self::MARGIN_RIGHT);
        let mut cur_bottom = win_h.saturating_sub(Self::MARGIN_BOTTOM);

        let mut cards = Vec::with_capacity(toast_count);
        let mut close_btns = Vec::with_capacity(toast_count);

        for _ in 0..toast_count {
            let y = cur_bottom.saturating_sub(Self::TOAST_H);
            let card = (x, y, w, Self::TOAST_H);
            let close = (
                x + w.saturating_sub(28),
                y + (Self::TOAST_H - 20) / 2,
                20,
                20,
            );
            cards.push(card);
            close_btns.push(close);
            cur_bottom = y.saturating_sub(Self::GAP_Y);
        }

        Self { cards, close_btns }
    }

    /// Devuelve el índice del toast cuyo botón de cierre fue alcanzado.
    pub(crate) fn hit_close(&self, mx: usize, my: usize) -> Option<usize> {
        self.close_btns
            .iter()
            .position(|&btn| rect_contains(btn, mx, my))
    }

    /// Devuelve el índice del toast cuya tarjeta contiene al cursor.
    #[allow(dead_code)]
    pub(crate) fn hit_card(&self, mx: usize, my: usize) -> Option<usize> {
        self.cards
            .iter()
            .position(|&card| rect_contains(card, mx, my))
    }
}

/// Geometría para vistas de estado vacías, cargando, de error o sin permisos.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct StateViewRects {
    pub container: Rect,
    pub header: Option<Rect>,
    pub center_y: usize,
    pub title_pos: (usize, usize),
    pub desc_rect: Rect,
    pub action_buttons: Vec<Rect>,
}

#[allow(dead_code)]
impl StateViewRects {
    pub(crate) const BTN_H: usize = 32;

    pub(crate) fn compute(container: Rect, has_header: bool, button_widths: &[usize]) -> Self {
        let (cx, cy, cw, ch) = container;
        let header = if has_header {
            Some((cx, cy, cw, 34))
        } else {
            None
        };

        let content_y = cy + if has_header { 34 } else { 0 };
        let content_h = ch.saturating_sub(if has_header { 34 } else { 0 });
        let center_y = content_y + content_h / 2;

        let title_pos = (cx + cw / 2, center_y.saturating_sub(40));
        let desc_w = 340.min(cw.saturating_sub(32));
        let desc_x = cx + (cw.saturating_sub(desc_w)) / 2;
        let desc_rect = (desc_x, center_y.saturating_sub(14), desc_w, 36);

        // Botones centrados horizontalmente debajo de la descripción
        let btn_y = center_y + 30;
        let total_btn_w: usize =
            button_widths.iter().sum::<usize>() + button_widths.len().saturating_sub(1) * 8;
        let mut cur_x = cx + (cw.saturating_sub(total_btn_w)) / 2;
        let mut action_buttons = Vec::with_capacity(button_widths.len());

        for &bw in button_widths {
            action_buttons.push((cur_x, btn_y, bw, Self::BTN_H));
            cur_x += bw + 8;
        }

        Self {
            container,
            header,
            center_y,
            title_pos,
            desc_rect,
            action_buttons,
        }
    }

    pub(crate) fn hit_button(&self, mx: usize, my: usize) -> Option<usize> {
        self.action_buttons
            .iter()
            .position(|&btn| rect_contains(btn, mx, my))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_rects_and_hit_testing() {
        let win_w = 1200;
        let win_h = 800;
        let button_widths = [80, 100];
        let rects = DialogRects::compute(win_w, win_h, true, true, &button_widths);

        // Tarjeta centrada
        assert!(rects.card.0 > 0);
        assert!(rects.card.1 > 0);
        assert_eq!(rects.card.2, 520);
        assert!(rects.card.3 > 150);

        // Header dentro de la tarjeta
        assert_eq!(rects.header.0, rects.card.0);
        assert_eq!(rects.header.1, rects.card.1);
        assert_eq!(rects.header.2, rects.card.2);

        // Cierre dentro del header
        let (cx, cy, _, _) = rects.close_btn;
        assert_eq!(rects.hit_test(cx + 2, cy + 2), Some(DialogHit::Close));

        // Input presente y detectado
        let inp = rects.confirm_input.expect("debe existir confirm_input");
        assert_eq!(rects.hit_test(inp.0 + 4, inp.1 + 4), Some(DialogHit::Input));

        // Botones dentro del footer y detectados
        assert_eq!(rects.buttons.len(), 2);
        let b0 = rects.buttons[0];
        assert_eq!(
            rects.hit_test(b0.0 + 2, b0.1 + 2),
            Some(DialogHit::Button(0))
        );
        let b1 = rects.buttons[1];
        assert_eq!(
            rects.hit_test(b1.0 + 2, b1.1 + 2),
            Some(DialogHit::Button(1))
        );

        // Coordenada fuera de la tarjeta
        assert_eq!(rects.hit_test(10, 10), None);
    }

    #[test]
    fn test_toast_rects_stacking_and_hit() {
        let win_w = 1440;
        let win_h = 900;
        let rects = ToastRects::compute(win_w, win_h, 3);

        assert_eq!(rects.cards.len(), 3);
        assert_eq!(rects.close_btns.len(), 3);

        // Toast 0 está más abajo que Toast 1, y Toast 1 más abajo que Toast 2
        assert!(rects.cards[0].1 > rects.cards[1].1);
        assert!(rects.cards[1].1 > rects.cards[2].1);

        // Todos caben en la ventana
        for card in &rects.cards {
            assert!(card.0 + card.2 <= win_w);
            assert!(card.1 + card.3 <= win_h);
        }

        // Clic en botón de cierre del primer toast
        let (bx, by, _, _) = rects.close_btns[0];
        assert_eq!(rects.hit_close(bx + 2, by + 2), Some(0));

        let (bx2, by2, _, _) = rects.close_btns[2];
        assert_eq!(rects.hit_close(bx2 + 2, by2 + 2), Some(2));
        assert_eq!(rects.hit_close(10, 10), None);
    }

    #[test]
    fn test_state_view_rects() {
        let container = (100, 100, 600, 400);
        let button_widths = [90, 110];
        let rects = StateViewRects::compute(container, true, &button_widths);

        assert!(rects.header.is_some());
        assert_eq!(rects.action_buttons.len(), 2);

        let b0 = rects.action_buttons[0];
        assert_eq!(rects.hit_button(b0.0 + 2, b0.1 + 2), Some(0));
        let b1 = rects.action_buttons[1];
        assert_eq!(rects.hit_button(b1.0 + 2, b1.1 + 2), Some(1));
        assert_eq!(rects.hit_button(50, 50), None);
    }

    #[test]
    fn test_db_confirm_rects_and_hits() {
        let win_w = 1440;
        let win_h = 900;
        let impact = vec![
            crate::app::DbImpactRow {
                n: "1".into(),
                op: "UPDATE · active".into(),
                table: "users".into(),
                rows: "1".into(),
            },
            crate::app::DbImpactRow {
                n: "2".into(),
                op: "UPDATE · name".into(),
                table: "users".into(),
                rows: "1".into(),
            },
        ];
        let spec = crate::app::DialogSpec::db_prod_write_confirm(
            "pg-main",
            "acme",
            "Aplicar 2 cambios en users",
            "Estás conectado a una base de datos de producción.",
            "BEGIN;\nUPDATE users SET active = true WHERE id = 4;\nCOMMIT;",
            impact,
        );
        let button_widths = [90, 140];
        let rects = DialogRects::compute_for_spec(win_w, win_h, &spec, &button_widths);

        // Tarjeta con ancho 680 px según diseño
        assert_eq!(rects.card.2, 680);
        assert!(rects.sql_block.is_some());
        assert!(rects.impact_table.is_some());
        assert_eq!(rects.impact_rows.len(), 2);
        assert_eq!(rects.option_checkboxes.len(), 3);
        assert!(rects.copy_sql_btn.is_some());
        assert_eq!(rects.buttons.len(), 2);

        // Hit testing en botón Copiar SQL
        let copy_btn = rects.copy_sql_btn.unwrap();
        assert_eq!(
            rects.hit_test(copy_btn.0 + 4, copy_btn.1 + 4),
            Some(DialogHit::CopySql)
        );

        // Hit testing en checkboxes de opciones
        let opt0 = rects.option_checkboxes[0];
        assert_eq!(
            rects.hit_test(opt0.0 + 4, opt0.1 + 4),
            Some(DialogHit::OptionCheckbox(0))
        );
        let opt1 = rects.option_checkboxes[1];
        assert_eq!(
            rects.hit_test(opt1.0 + 4, opt1.1 + 4),
            Some(DialogHit::OptionCheckbox(1))
        );

        // Hit testing en input
        let inp = rects.confirm_input.unwrap();
        assert_eq!(rects.hit_test(inp.0 + 4, inp.1 + 4), Some(DialogHit::Input));

        // Hit testing en botones
        let b0 = rects.buttons[0];
        assert_eq!(
            rects.hit_test(b0.0 + 4, b0.1 + 4),
            Some(DialogHit::Button(0))
        );
        let b1 = rects.buttons[1];
        assert_eq!(
            rects.hit_test(b1.0 + 4, b1.1 + 4),
            Some(DialogHit::Button(1))
        );
    }
}
