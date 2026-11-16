//! Geometría del módulo y panel de Sistema / Monitor de rendimiento (Fase F3).
//!
//! Centraliza todos los rectángulos interactivos de la vista de Sistema:
//! tarjetas de métricas, gráfico de procesos, filtros, CPU por núcleo,
//! almacenamiento, carpetas de proyecto y barra lateral de fuentes.

use super::{rect_contains, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SystemProcessFilter {
    All,
    Ai,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SystemClickTarget {
    MetricCard(usize),
    ProcessFilter(SystemProcessFilter),
    ProcessRow(usize),
    ViewInTerminal,
    KillProcess,
    FolderAction(usize),
    SidebarSource(usize),
    SidebarAddMachine,
    SidebarThresholds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SystemLayoutRects {
    pub work_area: Rect,
    pub metric_cards: [Rect; 5],
    pub processes_card: Rect,
    pub filter_all_btn: Rect,
    pub filter_ai_btn: Rect,
    pub filter_build_btn: Rect,
    pub process_list_rect: Rect,
    pub view_in_terminal_btn: Option<Rect>,
    pub kill_process_btn: Option<Rect>,
    pub cores_card: Rect,
    pub storage_card: Rect,
}

impl SystemLayoutRects {
    pub(crate) const TOP_CARDS_H: usize = 156;
    pub(crate) const GAP: usize = 8;
    pub(crate) const CARD_HEADER_H: usize = 36;
    pub(crate) const PROCESS_HEADER_H: usize = 34;
    pub(crate) const PROCESS_ROW_H: usize = 38;
    pub(crate) const PROCESS_FOOTER_H: usize = 44;
    pub(crate) const CORES_CARD_H: usize = 200;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute(
        sidebar_offset: usize,
        top_offset: usize,
        win_w: usize,
        win_h: usize,
        status_bar_h: usize,
        has_selected_process: bool,
    ) -> Self {
        let pad = Self::GAP;
        let work_x = sidebar_offset + pad;
        let work_y = top_offset + pad;
        let work_w = win_w.saturating_sub(sidebar_offset + 2 * pad);
        let work_h = win_h.saturating_sub(top_offset + status_bar_h + 2 * pad);
        let work_area = (work_x, work_y, work_w, work_h);

        // 1. Cinco tarjetas superiores de métricas (CPU, RAM, GPU, DISCO, RED)
        let total_gaps = 4 * Self::GAP;
        let card_w = work_w.saturating_sub(total_gaps) / 5;
        let mut metric_cards = [(0, 0, 0, 0); 5];
        for (i, card) in metric_cards.iter_mut().enumerate() {
            let cx = work_x + i * (card_w + Self::GAP);
            *card = (cx, work_y, card_w, Self::TOP_CARDS_H);
        }

        // 2. Área inferior (bajo las 5 tarjetas)
        let bottom_y = work_y + Self::TOP_CARDS_H + Self::GAP;
        let bottom_h = work_h.saturating_sub(Self::TOP_CARDS_H + Self::GAP);

        // Reparto horizontal: izquierda Procesos (flex 1.3), derecha Cores + Almacenamiento (flex 1.0)
        let left_w = (work_w.saturating_sub(Self::GAP) * 13) / 23;
        let right_x = work_x + left_w + Self::GAP;
        let right_w = work_w.saturating_sub(left_w + Self::GAP);

        // Tarjeta izquierda: Procesos del espacio
        let processes_card = (work_x, bottom_y, left_w, bottom_h);

        // Botones de filtro por segmento en la cabecera de la tarjeta
        let segm_h = 26;
        let segm_w = 68;
        let segm_y = bottom_y + (Self::CARD_HEADER_H.saturating_sub(segm_h)) / 2;
        let segm_right = work_x + left_w - 10;
        let filter_build_btn = (segm_right.saturating_sub(segm_w), segm_y, segm_w, segm_h);
        let filter_ai_btn = (
            filter_build_btn.0.saturating_sub(segm_w),
            segm_y,
            segm_w,
            segm_h,
        );
        let filter_all_btn = (
            filter_ai_btn.0.saturating_sub(segm_w),
            segm_y,
            segm_w,
            segm_h,
        );

        let list_top = bottom_y + Self::CARD_HEADER_H + Self::PROCESS_HEADER_H;
        let list_h = bottom_h
            .saturating_sub(Self::CARD_HEADER_H + Self::PROCESS_HEADER_H + Self::PROCESS_FOOTER_H);
        let process_list_rect = (work_x, list_top, left_w, list_h);

        let footer_y = bottom_y + bottom_h.saturating_sub(Self::PROCESS_FOOTER_H);
        let (view_in_terminal_btn, kill_process_btn) = if has_selected_process {
            let btn_h = 28;
            let btn_y = footer_y + (Self::PROCESS_FOOTER_H.saturating_sub(btn_h)) / 2;
            let kill_w = 140;
            let kill = (
                work_x + left_w.saturating_sub(kill_w + 14),
                btn_y,
                kill_w,
                btn_h,
            );
            let view_w = 120;
            let view = (kill.0.saturating_sub(view_w + 8), btn_y, view_w, btn_h);
            (Some(view), Some(kill))
        } else {
            (None, None)
        };

        // Columna derecha: CPU por núcleo + Almacenamiento
        let cores_card = (right_x, bottom_y, right_w, Self::CORES_CARD_H);
        let storage_y = bottom_y + Self::CORES_CARD_H + Self::GAP;
        let storage_h = bottom_h.saturating_sub(Self::CORES_CARD_H + Self::GAP);
        let storage_card = (right_x, storage_y, right_w, storage_h);

        Self {
            work_area,
            metric_cards,
            processes_card,
            filter_all_btn,
            filter_ai_btn,
            filter_build_btn,
            process_list_rect,
            view_in_terminal_btn,
            kill_process_btn,
            cores_card,
            storage_card,
        }
    }

    pub(crate) fn process_row_y(&self, rel_idx: usize) -> usize {
        self.process_list_rect.1 + rel_idx * Self::PROCESS_ROW_H
    }

    pub(crate) fn max_visible_process_rows(&self) -> usize {
        self.process_list_rect.3 / Self::PROCESS_ROW_H
    }

    /// Coordenada Y inicial de las filas de carpetas en la tarjeta de Almacenamiento.
    pub(crate) fn folders_start_y(storage_card: Rect, volume_count: usize) -> usize {
        storage_card.1 + Self::CARD_HEADER_H + 12 + volume_count.min(2) * 36 + 6 + 18
    }

    pub(crate) fn folder_action_btn_rect(
        storage_card: Rect,
        row_y: usize,
        btn_w: usize,
        btn_h: usize,
    ) -> Rect {
        let bx = (storage_card.0 + storage_card.2).saturating_sub(btn_w + 14);
        let by = row_y + (30usize.saturating_sub(btn_h)) / 2;
        (bx, by, btn_w, btn_h)
    }

    pub(crate) fn hit_test(
        &self,
        mx: usize,
        my: usize,
        visible_proc_count: usize,
        folder_count: usize,
        volume_count: usize,
    ) -> Option<SystemClickTarget> {
        // Tarjetas de métricas
        for (i, card) in self.metric_cards.iter().enumerate() {
            if rect_contains(*card, mx, my) {
                return Some(SystemClickTarget::MetricCard(i));
            }
        }

        // Filtros de procesos
        if rect_contains(self.filter_all_btn, mx, my) {
            return Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::All));
        }
        if rect_contains(self.filter_ai_btn, mx, my) {
            return Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::Ai));
        }
        if rect_contains(self.filter_build_btn, mx, my) {
            return Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::Build));
        }

        // Botones de acción de proceso
        if let Some(btn) = self.view_in_terminal_btn {
            if rect_contains(btn, mx, my) {
                return Some(SystemClickTarget::ViewInTerminal);
            }
        }
        if let Some(btn) = self.kill_process_btn {
            if rect_contains(btn, mx, my) {
                return Some(SystemClickTarget::KillProcess);
            }
        }

        // Filas de procesos (limitadas al área visible real)
        if rect_contains(self.process_list_rect, mx, my) {
            let shown_procs = visible_proc_count.min(self.max_visible_process_rows());
            for i in 0..shown_procs {
                let ry = self.process_row_y(i);
                if my >= ry && my < ry + Self::PROCESS_ROW_H {
                    return Some(SystemClickTarget::ProcessRow(i));
                }
            }
        }

        // Acciones de carpetas de almacenamiento
        if rect_contains(self.storage_card, mx, my) {
            let folders_start_y = Self::folders_start_y(self.storage_card, volume_count);
            let card_bottom = self.storage_card.1 + self.storage_card.3;
            for i in 0..folder_count {
                let fy = folders_start_y + i * 30;
                if fy + 30 > card_bottom {
                    break;
                }
                let btn = Self::folder_action_btn_rect(self.storage_card, fy, 80, 24);
                if rect_contains(btn, mx, my) {
                    return Some(SystemClickTarget::FolderAction(i));
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SystemSidebarRects {
    pub sidebar_rect: Rect,
    pub add_machine_btn: Rect,
    pub thresholds_btn: Rect,
    pub list_rect: Rect,
}

impl SystemSidebarRects {
    pub(crate) const HEADER_H: usize = 40;
    pub(crate) const ITEM_H: usize = 58;

    pub(crate) fn compute(panel_x: usize, panel_y: usize, panel_w: usize, panel_h: usize) -> Self {
        let add_btn_size = 28;
        let add_machine_btn = (
            panel_x + panel_w.saturating_sub(add_btn_size + 8),
            panel_y + (Self::HEADER_H.saturating_sub(add_btn_size)) / 2,
            add_btn_size,
            add_btn_size,
        );
        let thresh_h = 28;
        let thresh_pad = 10;
        let thresh_y = (panel_y + panel_h).saturating_sub(thresh_h + thresh_pad);
        let thresholds_btn = (panel_x + 12, thresh_y, panel_w.saturating_sub(24), thresh_h);

        let list_y = panel_y + Self::HEADER_H;
        let list_h = thresh_y.saturating_sub(list_y + 4);
        let list_rect = (panel_x, list_y, panel_w, list_h);

        Self {
            sidebar_rect: (panel_x, panel_y, panel_w, panel_h),
            add_machine_btn,
            thresholds_btn,
            list_rect,
        }
    }

    pub(crate) fn source_item_rect(&self, index: usize) -> Rect {
        if index == 0 {
            (
                self.sidebar_rect.0 + 6,
                self.list_rect.1 + 28,
                self.sidebar_rect.2.saturating_sub(12),
                Self::ITEM_H,
            )
        } else {
            let ry = self.list_rect.1 + 122 + (index - 1) * (Self::ITEM_H + 8);
            (
                self.sidebar_rect.0 + 6,
                ry,
                self.sidebar_rect.2.saturating_sub(12),
                Self::ITEM_H,
            )
        }
    }

    pub(crate) fn hit_test(
        &self,
        mx: usize,
        my: usize,
        source_count: usize,
    ) -> Option<SystemClickTarget> {
        if rect_contains(self.add_machine_btn, mx, my) {
            return Some(SystemClickTarget::SidebarAddMachine);
        }
        if rect_contains(self.thresholds_btn, mx, my) {
            return Some(SystemClickTarget::SidebarThresholds);
        }
        if rect_contains(self.list_rect, mx, my) {
            for i in 0..source_count {
                let ir = self.source_item_rect(i);
                if rect_contains(ir, mx, my) {
                    return Some(SystemClickTarget::SidebarSource(i));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_layout_rects_compute_and_hit_test() {
        let rects = SystemLayoutRects::compute(56, 40, 1440, 900, 28, true);

        // 5 tarjetas de métricas
        assert_eq!(rects.metric_cards.len(), 5);
        for i in 0..5 {
            let hit = rects.hit_test(
                rects.metric_cards[i].0 + 5,
                rects.metric_cards[i].1 + 5,
                5,
                3,
                1,
            );
            assert_eq!(hit, Some(SystemClickTarget::MetricCard(i)));
        }

        // Filtros
        assert_eq!(
            rects.hit_test(
                rects.filter_all_btn.0 + 2,
                rects.filter_all_btn.1 + 2,
                5,
                3,
                1
            ),
            Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::All))
        );
        assert_eq!(
            rects.hit_test(
                rects.filter_ai_btn.0 + 2,
                rects.filter_ai_btn.1 + 2,
                5,
                3,
                1
            ),
            Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::Ai))
        );
        assert_eq!(
            rects.hit_test(
                rects.filter_build_btn.0 + 2,
                rects.filter_build_btn.1 + 2,
                5,
                3,
                1
            ),
            Some(SystemClickTarget::ProcessFilter(SystemProcessFilter::Build))
        );

        // Botones de acción
        let view = rects.view_in_terminal_btn.unwrap();
        let kill = rects.kill_process_btn.unwrap();
        assert_eq!(
            rects.hit_test(view.0 + 2, view.1 + 2, 5, 3, 1),
            Some(SystemClickTarget::ViewInTerminal)
        );
        assert_eq!(
            rects.hit_test(kill.0 + 2, kill.1 + 2, 5, 3, 1),
            Some(SystemClickTarget::KillProcess)
        );

        // Fila de proceso
        let row0_y = rects.process_row_y(0);
        assert_eq!(
            rects.hit_test(rects.process_list_rect.0 + 20, row0_y + 10, 5, 3, 1),
            Some(SystemClickTarget::ProcessRow(0))
        );
    }

    #[test]
    fn test_system_sidebar_rects_hit_test() {
        let sb = SystemSidebarRects::compute(56, 40, 264, 832);
        assert_eq!(
            sb.hit_test(sb.add_machine_btn.0 + 2, sb.add_machine_btn.1 + 2, 3),
            Some(SystemClickTarget::SidebarAddMachine)
        );
        assert_eq!(
            sb.hit_test(sb.thresholds_btn.0 + 2, sb.thresholds_btn.1 + 2, 3),
            Some(SystemClickTarget::SidebarThresholds)
        );
        let s0 = sb.source_item_rect(0);
        assert_eq!(
            sb.hit_test(s0.0 + 20, s0.1 + 20, 3),
            Some(SystemClickTarget::SidebarSource(0))
        );
        let s1 = sb.source_item_rect(1);
        assert_eq!(
            sb.hit_test(s1.0 + 20, s1.1 + 20, 3),
            Some(SystemClickTarget::SidebarSource(1))
        );
    }
}
