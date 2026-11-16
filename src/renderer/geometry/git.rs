//! Geometría del módulo y panel de Git (Fase F2).
//!
//! Centraliza todos los rectángulos interactivos del panel de Git:
//! rama, formulario de commit, secciones colapsables (Preparados y Cambios),
//! botones de acción individual y grupal, y diálogo de confirmación de descarte.

use super::{rect_contains, Rect};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitBulkActionKind {
    StageAll,
    UnstageAll,
    DiscardAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitClickTarget {
    BranchRow,
    CommitInput,
    CommitBtn,
    PushBtn,
    AmendToggle,
    StagedHeaderToggle,
    UnstageAll,
    StagedItem(usize),
    UnstageItem(usize),
    UnstagedHeaderToggle,
    StageAll,
    DiscardAllPrompt,
    UnstagedItem(usize),
    StageItem(usize),
    DiscardItemPrompt(usize),
    DiscardConfirmCancel,
    DiscardConfirmExecute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GitPanelRects {
    pub panel_rect: Rect,
    pub branch_row: Rect,
    pub commit_box: Rect,
    pub commit_input: Rect,
    pub commit_btn: Rect,
    pub push_btn: Rect,
    pub char_count: Rect,
    pub amend_toggle: Rect,
    pub list_rect: Rect,
    pub staged_header: Rect,
    pub unstage_all_btn: Rect,
    pub unstaged_header: Rect,
    pub stage_all_btn: Rect,
    pub discard_all_btn: Rect,
    pub discard_banner: Option<Rect>,
    pub discard_cancel_btn: Option<Rect>,
    pub discard_execute_btn: Option<Rect>,
}

impl GitPanelRects {
    pub(crate) const BRANCH_ROW_H: usize = 36;
    pub(crate) const COMMIT_INPUT_H: usize = 68;
    pub(crate) const ACTION_BTN_H: usize = 28;
    pub(crate) const META_ROW_H: usize = 18;
    pub(crate) const SECTION_HEADER_H: usize = 30;
    pub(crate) const ITEM_ROW_H: usize = 30;
    pub(crate) const ICON_BTN_SIZE: usize = 20;

    /// Calcula todos los rectángulos del panel de Git
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute(
        panel_x: usize,
        content_y: usize,
        panel_w: usize,
        panel_h: usize,
        staged_count: usize,
        staged_collapsed: bool,
        _unstaged_count: usize,
        _unstaged_collapsed: bool,
        has_discard_confirm: bool,
    ) -> Self {
        let branch_row = (panel_x, content_y, panel_w, Self::BRANCH_ROW_H);

        // Formulario de commit
        let commit_y = content_y + Self::BRANCH_ROW_H;
        let pad = 12;
        let inner_w = panel_w.saturating_sub(2 * pad);
        let commit_input = (panel_x + pad, commit_y + pad, inner_w, Self::COMMIT_INPUT_H);

        let action_y = commit_input.1 + Self::COMMIT_INPUT_H + 8;
        let push_w = 64;
        let gap = 8;
        let commit_w = inner_w.saturating_sub(push_w + gap);
        let commit_btn = (panel_x + pad, action_y, commit_w, Self::ACTION_BTN_H);
        let push_btn = (
            panel_x + pad + commit_w + gap,
            action_y,
            push_w,
            Self::ACTION_BTN_H,
        );

        let meta_y = action_y + Self::ACTION_BTN_H + 8;
        let char_count_w = 110;
        let char_count = (panel_x + pad, meta_y, char_count_w, Self::META_ROW_H);
        let amend_toggle_w = inner_w.saturating_sub(char_count_w);
        let amend_toggle = (
            panel_x + pad + char_count_w,
            meta_y,
            amend_toggle_w,
            Self::META_ROW_H,
        );

        let commit_box = (
            panel_x,
            commit_y,
            panel_w,
            (meta_y + Self::META_ROW_H + pad).saturating_sub(commit_y),
        );

        // Área desplazable de archivos
        let list_top = commit_box.1 + commit_box.3;
        let list_h = (content_y + panel_h).saturating_sub(list_top);
        let list_rect = (panel_x, list_top, panel_w, list_h);

        // Cabecera de Preparados
        let staged_header = (panel_x, list_top, panel_w, Self::SECTION_HEADER_H);
        let unstage_all_btn = (
            panel_x + panel_w.saturating_sub(Self::ICON_BTN_SIZE + 10),
            list_top + (Self::SECTION_HEADER_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );

        // Cabecera de Cambios
        let staged_rows_h = if staged_collapsed {
            0
        } else {
            staged_count * Self::ITEM_ROW_H
        };
        let unstaged_y = list_top + Self::SECTION_HEADER_H + staged_rows_h;
        let unstaged_header = (panel_x, unstaged_y, panel_w, Self::SECTION_HEADER_H);

        let discard_all_btn = (
            panel_x + panel_w.saturating_sub(Self::ICON_BTN_SIZE + 10),
            unstaged_y + (Self::SECTION_HEADER_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );
        let stage_all_btn = (
            discard_all_btn.0.saturating_sub(Self::ICON_BTN_SIZE + 4),
            discard_all_btn.1,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );

        // Banner o diálogo de confirmación de descarte
        let (discard_banner, discard_cancel_btn, discard_execute_btn) = if has_discard_confirm {
            let banner_h = 76;
            let by = (content_y + panel_h).saturating_sub(banner_h);
            let b_rect = (panel_x, by, panel_w, banner_h);
            let btn_h = 24;
            let btn_w = 76;
            let btn_y = by + banner_h.saturating_sub(btn_h + 8);
            let cancel = (
                panel_x + panel_w.saturating_sub(2 * btn_w + 16),
                btn_y,
                btn_w,
                btn_h,
            );
            let execute = (
                panel_x + panel_w.saturating_sub(btn_w + 8),
                btn_y,
                btn_w,
                btn_h,
            );
            (Some(b_rect), Some(cancel), Some(execute))
        } else {
            (None, None, None)
        };

        Self {
            panel_rect: (panel_x, content_y, panel_w, panel_h),
            branch_row,
            commit_box,
            commit_input,
            commit_btn,
            push_btn,
            char_count,
            amend_toggle,
            list_rect,
            staged_header,
            unstage_all_btn,
            unstaged_header,
            stage_all_btn,
            discard_all_btn,
            discard_banner,
            discard_cancel_btn,
            discard_execute_btn,
        }
    }

    /// Número total de filas lógicas dentro de la lista desplegable de Git
    pub(crate) fn total_list_rows(
        staged_count: usize,
        staged_collapsed: bool,
        unstaged_count: usize,
        unstaged_collapsed: bool,
    ) -> usize {
        let staged = if staged_collapsed { 0 } else { staged_count };
        let unstaged = if unstaged_collapsed {
            0
        } else {
            unstaged_count
        };
        2 + staged + unstaged
    }

    fn scrolled_y(
        &self,
        virtual_offset: usize,
        row_h: usize,
        scroll_offset: usize,
    ) -> Option<usize> {
        let scroll_px = scroll_offset.saturating_mul(Self::ITEM_ROW_H);
        let y = self.list_rect.1 as isize + virtual_offset as isize - scroll_px as isize;
        let max_y = self.list_rect.1 + self.list_rect.3;
        if y < self.list_rect.1 as isize {
            return None;
        }
        let yu = y as usize;
        if yu >= max_y || yu + row_h > max_y {
            return None;
        }
        Some(yu)
    }

    /// Rectángulo visible de la cabecera «PREPARADOS» (incluye botón quitar todo)
    pub(crate) fn staged_header_rect(&self, scroll_offset: usize) -> Option<(Rect, Rect)> {
        let y = self.scrolled_y(0, Self::SECTION_HEADER_H, scroll_offset)?;
        let header = (
            self.panel_rect.0,
            y,
            self.panel_rect.2,
            Self::SECTION_HEADER_H,
        );
        let btn = (
            self.panel_rect.0 + self.panel_rect.2.saturating_sub(Self::ICON_BTN_SIZE + 10),
            y + (Self::SECTION_HEADER_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );
        Some((header, btn))
    }

    /// Calcula la posición Y visible de una fila de elemento preparado
    pub(crate) fn staged_item_y(&self, idx: usize, scroll_offset: usize) -> Option<usize> {
        let v_offset = Self::SECTION_HEADER_H + idx.saturating_mul(Self::ITEM_ROW_H);
        self.scrolled_y(v_offset, Self::ITEM_ROW_H, scroll_offset)
    }

    /// Rectángulo visible de la cabecera «CAMBIOS» (incluye botones preparar todo y descartar todo)
    pub(crate) fn unstaged_header_rect(
        &self,
        staged_count: usize,
        staged_collapsed: bool,
        scroll_offset: usize,
    ) -> Option<(Rect, Rect, Rect)> {
        let staged_rows_h = if staged_collapsed {
            0
        } else {
            staged_count * Self::ITEM_ROW_H
        };
        let v_offset = Self::SECTION_HEADER_H + staged_rows_h;
        let y = self.scrolled_y(v_offset, Self::SECTION_HEADER_H, scroll_offset)?;
        let header = (
            self.panel_rect.0,
            y,
            self.panel_rect.2,
            Self::SECTION_HEADER_H,
        );
        let discard_all = (
            self.panel_rect.0 + self.panel_rect.2.saturating_sub(Self::ICON_BTN_SIZE + 10),
            y + (Self::SECTION_HEADER_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );
        let stage_all = (
            discard_all.0.saturating_sub(Self::ICON_BTN_SIZE + 4),
            discard_all.1,
            Self::ICON_BTN_SIZE,
            Self::ICON_BTN_SIZE,
        );
        Some((header, stage_all, discard_all))
    }

    /// Calcula la posición Y visible de una fila de elemento sin preparar
    pub(crate) fn unstaged_item_y(
        &self,
        idx: usize,
        staged_count: usize,
        staged_collapsed: bool,
        scroll_offset: usize,
    ) -> Option<usize> {
        let staged_rows_h = if staged_collapsed {
            0
        } else {
            staged_count * Self::ITEM_ROW_H
        };
        let v_offset = Self::SECTION_HEADER_H
            + staged_rows_h
            + Self::SECTION_HEADER_H
            + idx.saturating_mul(Self::ITEM_ROW_H);
        self.scrolled_y(v_offset, Self::ITEM_ROW_H, scroll_offset)
    }

    /// Comprueba si `(mx, my)` cae en algún elemento interactivo del panel de Git
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn hit_test(
        &self,
        mx: usize,
        my: usize,
        staged_count: usize,
        staged_collapsed: bool,
        unstaged_count: usize,
        unstaged_collapsed: bool,
        scroll_offset: usize,
    ) -> Option<GitClickTarget> {
        // 1. Confirmación de descarte si está activa
        if let (Some(banner), Some(cancel), Some(exec)) = (
            self.discard_banner,
            self.discard_cancel_btn,
            self.discard_execute_btn,
        ) {
            if rect_contains(exec, mx, my) {
                return Some(GitClickTarget::DiscardConfirmExecute);
            }
            if rect_contains(cancel, mx, my) {
                return Some(GitClickTarget::DiscardConfirmCancel);
            }
            if rect_contains(banner, mx, my) {
                return None; // Absorbe clics en el fondo del banner
            }
        }

        // 2. Fila de rama
        if rect_contains(self.branch_row, mx, my) {
            return Some(GitClickTarget::BranchRow);
        }

        // 3. Formulario de commit
        if rect_contains(self.commit_btn, mx, my) {
            return Some(GitClickTarget::CommitBtn);
        }
        if rect_contains(self.push_btn, mx, my) {
            return Some(GitClickTarget::PushBtn);
        }
        if rect_contains(self.amend_toggle, mx, my) {
            return Some(GitClickTarget::AmendToggle);
        }
        if rect_contains(self.commit_input, mx, my) {
            return Some(GitClickTarget::CommitInput);
        }

        // 4. Si está fuera de la lista de archivos, terminar
        if !rect_contains(self.list_rect, mx, my) {
            return None;
        }

        // 5. Cabecera de preparados
        if let Some((staged_hdr, unstage_all_btn)) = self.staged_header_rect(scroll_offset) {
            if rect_contains(staged_hdr, mx, my) {
                if rect_contains(unstage_all_btn, mx, my) {
                    return Some(GitClickTarget::UnstageAll);
                }
                return Some(GitClickTarget::StagedHeaderToggle);
            }
        }

        // 6. Filas de preparados si no están colapsadas
        if !staged_collapsed {
            for i in 0..staged_count {
                let Some(iy) = self.staged_item_y(i, scroll_offset) else {
                    continue;
                };
                if my >= iy && my < iy + Self::ITEM_ROW_H {
                    let unstage_btn = (
                        self.panel_rect.0
                            + self.panel_rect.2.saturating_sub(Self::ICON_BTN_SIZE + 10),
                        iy + (Self::ITEM_ROW_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
                        Self::ICON_BTN_SIZE,
                        Self::ICON_BTN_SIZE,
                    );
                    if rect_contains(unstage_btn, mx, my) {
                        return Some(GitClickTarget::UnstageItem(i));
                    }
                    return Some(GitClickTarget::StagedItem(i));
                }
            }
        }

        // 7. Cabecera de cambios
        if let Some((unstaged_hdr_rect, stage_all_btn, discard_all_btn)) =
            self.unstaged_header_rect(staged_count, staged_collapsed, scroll_offset)
        {
            if rect_contains(unstaged_hdr_rect, mx, my) {
                if rect_contains(stage_all_btn, mx, my) {
                    return Some(GitClickTarget::StageAll);
                }
                if rect_contains(discard_all_btn, mx, my) {
                    return Some(GitClickTarget::DiscardAllPrompt);
                }
                return Some(GitClickTarget::UnstagedHeaderToggle);
            }
        }

        // 8. Filas de cambios si no están colapsadas
        if !unstaged_collapsed {
            for i in 0..unstaged_count {
                let Some(iy) =
                    self.unstaged_item_y(i, staged_count, staged_collapsed, scroll_offset)
                else {
                    continue;
                };
                if my >= iy && my < iy + Self::ITEM_ROW_H {
                    let discard_btn = (
                        self.panel_rect.0
                            + self.panel_rect.2.saturating_sub(Self::ICON_BTN_SIZE + 10),
                        iy + (Self::ITEM_ROW_H.saturating_sub(Self::ICON_BTN_SIZE)) / 2,
                        Self::ICON_BTN_SIZE,
                        Self::ICON_BTN_SIZE,
                    );
                    let stage_btn = (
                        discard_btn.0.saturating_sub(Self::ICON_BTN_SIZE + 4),
                        discard_btn.1,
                        Self::ICON_BTN_SIZE,
                        Self::ICON_BTN_SIZE,
                    );
                    if rect_contains(stage_btn, mx, my) {
                        return Some(GitClickTarget::StageItem(i));
                    }
                    if rect_contains(discard_btn, mx, my) {
                        return Some(GitClickTarget::DiscardItemPrompt(i));
                    }
                    return Some(GitClickTarget::UnstagedItem(i));
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
    fn test_git_panel_rects_compute_and_hit_test() {
        let panel_x = 100;
        let content_y = 50;
        let panel_w = 300;
        let panel_h = 600;

        let rects = GitPanelRects::compute(
            panel_x, content_y, panel_w, panel_h, 2,     // staged_count
            false, // staged_collapsed
            3,     // unstaged_count
            false, // unstaged_collapsed
            false, // has_discard_confirm
        );

        assert_eq!(rects.panel_rect, (100, 50, 300, 600));
        assert_eq!(rects.branch_row.1, 50);

        // Branch row hit test
        let br_hit = rects.hit_test(150, 60, 2, false, 3, false, 0);
        assert_eq!(br_hit, Some(GitClickTarget::BranchRow));

        // Commit input hit test
        let ci = rects.commit_input;
        let ci_hit = rects.hit_test(ci.0 + 5, ci.1 + 5, 2, false, 3, false, 0);
        assert_eq!(ci_hit, Some(GitClickTarget::CommitInput));

        // Commit button hit test
        let cb = rects.commit_btn;
        let cb_hit = rects.hit_test(cb.0 + 5, cb.1 + 5, 2, false, 3, false, 0);
        assert_eq!(cb_hit, Some(GitClickTarget::CommitBtn));

        // Push button hit test
        let pb = rects.push_btn;
        let pb_hit = rects.hit_test(pb.0 + 5, pb.1 + 5, 2, false, 3, false, 0);
        assert_eq!(pb_hit, Some(GitClickTarget::PushBtn));

        // Amend toggle hit test
        let at = rects.amend_toggle;
        let at_hit = rects.hit_test(at.0 + 5, at.1 + 5, 2, false, 3, false, 0);
        assert_eq!(at_hit, Some(GitClickTarget::AmendToggle));

        // Staged header hit test
        let sh = rects.staged_header;
        let sh_hit = rects.hit_test(sh.0 + 20, sh.1 + 5, 2, false, 3, false, 0);
        assert_eq!(sh_hit, Some(GitClickTarget::StagedHeaderToggle));

        // Unstage all button hit test
        let uab = rects.unstage_all_btn;
        let uab_hit = rects.hit_test(uab.0 + 2, uab.1 + 2, 2, false, 3, false, 0);
        assert_eq!(uab_hit, Some(GitClickTarget::UnstageAll));

        // Staged item row hit test
        let s0_y = rects.staged_item_y(0, 0).unwrap();
        let s0_hit = rects.hit_test(panel_x + 50, s0_y + 10, 2, false, 3, false, 0);
        assert_eq!(s0_hit, Some(GitClickTarget::StagedItem(0)));

        // Unstage item button hit test
        let unstage_btn_x = panel_x + panel_w - 20;
        let s0_btn_hit = rects.hit_test(unstage_btn_x, s0_y + 10, 2, false, 3, false, 0);
        assert_eq!(s0_btn_hit, Some(GitClickTarget::UnstageItem(0)));

        // Unstaged header hit test
        let uh = rects.unstaged_header;
        let uh_hit = rects.hit_test(uh.0 + 20, uh.1 + 5, 2, false, 3, false, 0);
        assert_eq!(uh_hit, Some(GitClickTarget::UnstagedHeaderToggle));

        // Stage all button hit test
        let sab = rects.stage_all_btn;
        let sab_hit = rects.hit_test(sab.0 + 2, sab.1 + 2, 2, false, 3, false, 0);
        assert_eq!(sab_hit, Some(GitClickTarget::StageAll));

        // Discard all button hit test
        let dab = rects.discard_all_btn;
        let dab_hit = rects.hit_test(dab.0 + 2, dab.1 + 2, 2, false, 3, false, 0);
        assert_eq!(dab_hit, Some(GitClickTarget::DiscardAllPrompt));

        // Unstaged item row hit test
        let u0_y = rects.unstaged_item_y(0, 2, false, 0).unwrap();
        let u0_hit = rects.hit_test(panel_x + 50, u0_y + 10, 2, false, 3, false, 0);
        assert_eq!(u0_hit, Some(GitClickTarget::UnstagedItem(0)));

        // Scrolled view: item 0 scrolled off top returns None; unstaged item shifts up cleanly
        assert!(rects.staged_header_rect(2).is_none());
        assert!(rects.staged_item_y(0, 2).is_none());
        let u0_scrolled_y = rects.unstaged_item_y(0, 2, false, 2).unwrap();
        assert_eq!(u0_scrolled_y, u0_y - 2 * GitPanelRects::ITEM_ROW_H);
        assert_eq!(
            rects.hit_test(panel_x + 50, u0_scrolled_y + 10, 2, false, 3, false, 2),
            Some(GitClickTarget::UnstagedItem(0))
        );
    }

    #[test]
    fn test_git_panel_discard_confirm_banner_hit_test() {
        let rects = GitPanelRects::compute(
            0, 0, 300, 500, 1, false, 1, false, true, // has_discard_confirm
        );

        assert!(rects.discard_banner.is_some());
        assert!(rects.discard_cancel_btn.is_some());
        assert!(rects.discard_execute_btn.is_some());

        let cancel = rects.discard_cancel_btn.unwrap();
        let exec = rects.discard_execute_btn.unwrap();

        assert_eq!(
            rects.hit_test(cancel.0 + 2, cancel.1 + 2, 1, false, 1, false, 0),
            Some(GitClickTarget::DiscardConfirmCancel)
        );
        assert_eq!(
            rects.hit_test(exec.0 + 2, exec.1 + 2, 1, false, 1, false, 0),
            Some(GitClickTarget::DiscardConfirmExecute)
        );
    }
}
