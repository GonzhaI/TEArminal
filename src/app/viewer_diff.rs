//! Cálculo de diferencias (diff) y estructuras para el visor de archivos.

use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ViewerSaveError {
    #[error("los archivos remotos se guardan por SSH en segundo plano")]
    RemoteFile,
    #[error("{0}")]
    ExternalModification(&'static str),
    #[error("error de E/S al guardar archivo: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitLineStatus {
    Added,
    Modified,
    Deleted,
}

/// Modo de visualización del diff de git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DiffMode {
    #[default]
    None,
    SideBySide,
    Unified,
}

/// Tipo de cambio en una celda de diff lado a lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffCellKind {
    Normal,
    Added,
    Deleted,
    Modified,
    Empty,
    Separator,
}

/// Celda de una línea en una columna del diff lado a lado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SideBySideCell {
    pub(crate) line_num: Option<usize>,
    pub(crate) text: String,
    pub(crate) kind: DiffCellKind,
}

/// Fila sincronizada del diff lado a lado (columna izquierda = HEAD, columna derecha = disco).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SideBySideRow {
    pub(crate) left: SideBySideCell,
    pub(crate) right: SideBySideCell,
}

/// Estado del visor mientras se muestra el diff (se restaura al volver al archivo)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffStash {
    pub(crate) lines: Vec<String>,
    pub(crate) original_lines: Vec<String>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) scroll_offset_x: usize,
    pub(crate) selection: Option<((usize, usize), (usize, usize))>,
    pub(crate) folded: BTreeSet<usize>,
    pub(crate) render_markdown: bool,
}

pub(crate) type GitMarks = Rc<[Option<GitLineStatus>]>;

/// Máximo de celdas de la tabla LCS; por encima, todo el tramo cambiado se marca como modificado.
const LCS_MAX_CELLS: usize = 4_000_000;

/// Tramo cambiado: `del` líneas de la base desde `b` sustituidas por `add` líneas de la actual desde `c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Hunk {
    b: usize,
    del: usize,
    c: usize,
    add: usize,
}

/// Diff por líneas (prefijo/sufijo comunes y LCS en el medio), tramos en orden.
fn diff_hunks(base: &[String], cur: &[String]) -> Vec<Hunk> {
    let pre = base.iter().zip(cur).take_while(|(a, b)| a == b).count();
    let suf = base[pre..]
        .iter()
        .rev()
        .zip(cur[pre..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (b, c) = (&base[pre..base.len() - suf], &cur[pre..cur.len() - suf]);
    let (n, m) = (b.len(), c.len());
    let mut hunks = Vec::new();
    if n * m > LCS_MAX_CELLS {
        if n + m > 0 {
            hunks.push(Hunk {
                b: pre,
                del: n,
                c: pre,
                add: m,
            });
        }
        return hunks;
    }
    // t[i][j] = longitud de la subsecuencia común de b[i..] y c[j..]
    let w = m + 1;
    let mut t = vec![0u16; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * w + j] = if b[i] == c[j] {
                t[(i + 1) * w + j + 1] + 1
            } else {
                t[(i + 1) * w + j].max(t[i * w + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut gap_b, mut gap_c, mut del, mut add) = (0, 0, 0, 0);
    let flush = |gap_b, gap_c, del, add, hunks: &mut Vec<Hunk>| {
        if del + add > 0 {
            hunks.push(Hunk {
                b: pre + gap_b,
                del,
                c: pre + gap_c,
                add,
            });
        }
    };
    while i < n || j < m {
        if i < n && j < m && b[i] == c[j] {
            flush(gap_b, gap_c, del, add, &mut hunks);
            (del, add) = (0, 0);
            i += 1;
            j += 1;
            (gap_b, gap_c) = (i, j);
        } else if j >= m || (i < n && t[(i + 1) * w + j] >= t[i * w + j + 1]) {
            del += 1;
            i += 1;
        } else {
            add += 1;
            j += 1;
        }
    }
    flush(gap_b, gap_c, del, add, &mut hunks);
    hunks
}

/// Calcula las filas alineadas y sincronizadas para el visor de diff lado a lado,
/// mostrando únicamente los fragmentos cambiados con líneas de contexto (estilo git diff).
pub(crate) fn compute_side_by_side(base: &[String], cur: &[String]) -> Vec<SideBySideRow> {
    const CONTEXT: usize = 3;
    let hunks = diff_hunks(base, cur);
    if hunks.is_empty() {
        return vec![SideBySideRow {
            left: SideBySideCell {
                line_num: None,
                text: "Sin cambios".to_string(),
                kind: DiffCellKind::Normal,
            },
            right: SideBySideCell {
                line_num: None,
                text: "Sin cambios".to_string(),
                kind: DiffCellKind::Normal,
            },
        }];
    }

    let mut rows = Vec::new();
    let mut k = 0;
    while k < hunks.len() {
        let mut end = k;
        while end + 1 < hunks.len()
            && hunks[end + 1]
                .c
                .saturating_sub(hunks[end].c + hunks[end].add)
                <= 2 * CONTEXT
        {
            end += 1;
        }

        let first = &hunks[k];
        let last = &hunks[end];

        let lead = CONTEXT.min(first.b).min(first.c);
        let b_start = first.b - lead;
        let c_start = first.c - lead;

        let trail = CONTEXT
            .min(base.len().saturating_sub(last.b + last.del))
            .min(cur.len().saturating_sub(last.c + last.add));
        let b_end = last.b + last.del + trail;
        let c_end = last.c + last.add + trail;

        let b_len = b_end - b_start;
        let c_len = c_end - c_start;

        // Separador de fragmento cuando se omiten líneas antes de este tramo
        if k > 0 || b_start > 0 || c_start > 0 {
            let at = |start: usize, len: usize| if len == 0 { start } else { start + 1 };
            let header = format!(
                "@@ -{},{} +{},{} @@",
                at(b_start, b_len),
                b_len,
                at(c_start, c_len),
                c_len
            );
            rows.push(SideBySideRow {
                left: SideBySideCell {
                    line_num: None,
                    text: header.clone(),
                    kind: DiffCellKind::Separator,
                },
                right: SideBySideCell {
                    line_num: None,
                    text: header,
                    kind: DiffCellKind::Separator,
                },
            });
        }

        // Líneas de contexto inicial
        for i in 0..lead {
            let bi = b_start + i;
            let ci = c_start + i;
            rows.push(SideBySideRow {
                left: SideBySideCell {
                    line_num: Some(bi + 1),
                    text: base[bi].clone(),
                    kind: DiffCellKind::Normal,
                },
                right: SideBySideCell {
                    line_num: Some(ci + 1),
                    text: cur[ci].clone(),
                    kind: DiffCellKind::Normal,
                },
            });
        }

        // Tramo con cambios y líneas intermedias comunes
        let mut cur_b = first.b;
        let mut cur_c = first.c;

        for h in &hunks[k..=end] {
            while cur_b < h.b && cur_c < h.c {
                rows.push(SideBySideRow {
                    left: SideBySideCell {
                        line_num: Some(cur_b + 1),
                        text: base[cur_b].clone(),
                        kind: DiffCellKind::Normal,
                    },
                    right: SideBySideCell {
                        line_num: Some(cur_c + 1),
                        text: cur[cur_c].clone(),
                        kind: DiffCellKind::Normal,
                    },
                });
                cur_b += 1;
                cur_c += 1;
            }

            let count = h.del.max(h.add);
            for m in 0..count {
                let left = if m < h.del {
                    let idx = h.b + m;
                    let kind = if m < h.add {
                        DiffCellKind::Modified
                    } else {
                        DiffCellKind::Deleted
                    };
                    SideBySideCell {
                        line_num: Some(idx + 1),
                        text: base.get(idx).cloned().unwrap_or_default(),
                        kind,
                    }
                } else {
                    SideBySideCell {
                        line_num: None,
                        text: String::new(),
                        kind: DiffCellKind::Empty,
                    }
                };

                let right = if m < h.add {
                    let idx = h.c + m;
                    let kind = if m < h.del {
                        DiffCellKind::Modified
                    } else {
                        DiffCellKind::Added
                    };
                    SideBySideCell {
                        line_num: Some(idx + 1),
                        text: cur.get(idx).cloned().unwrap_or_default(),
                        kind,
                    }
                } else {
                    SideBySideCell {
                        line_num: None,
                        text: String::new(),
                        kind: DiffCellKind::Empty,
                    }
                };

                rows.push(SideBySideRow { left, right });
            }

            cur_b = h.b + h.del;
            cur_c = h.c + h.add;
        }

        // Líneas de contexto final
        for i in 0..trail {
            let bi = cur_b + i;
            let ci = cur_c + i;
            rows.push(SideBySideRow {
                left: SideBySideCell {
                    line_num: Some(bi + 1),
                    text: base[bi].clone(),
                    kind: DiffCellKind::Normal,
                },
                right: SideBySideCell {
                    line_num: Some(ci + 1),
                    text: cur[ci].clone(),
                    kind: DiffCellKind::Normal,
                },
            });
        }

        k = end + 1;
    }

    rows
}

/// Marca cada línea de `cur` frente a `base`: añadida, modificada o (en la línea siguiente) con borrado.
pub(crate) fn diff_marks(base: &[String], cur: &[String]) -> Vec<Option<GitLineStatus>> {
    let mut marks = vec![None; cur.len()];
    for h in diff_hunks(base, cur) {
        if h.add > 0 {
            for k in 0..h.add {
                marks[h.c + k] = Some(if k < h.del {
                    GitLineStatus::Modified
                } else {
                    GitLineStatus::Added
                });
            }
        } else if !cur.is_empty() {
            marks[h.c.min(cur.len() - 1)].get_or_insert(GitLineStatus::Deleted);
        }
    }
    marks
}

/// Diff unificado (`@@`, ` contexto`, `-antes`, `+ahora`) con `ctx` líneas de contexto por tramo.
pub(crate) fn unified_diff(base: &[String], cur: &[String], ctx: usize) -> Vec<String> {
    let hunks = diff_hunks(base, cur);
    let mut out = Vec::with_capacity(hunks.len() * (2 * ctx + 4));
    let mut k = 0;
    while k < hunks.len() {
        // Une los tramos cuyo contexto se solapa
        let mut end = k;
        while end + 1 < hunks.len() && hunks[end + 1].c - (hunks[end].c + hunks[end].add) <= 2 * ctx
        {
            end += 1;
        }
        let (first, last) = (&hunks[k], &hunks[end]);
        let lead = ctx.min(first.c);
        let trail = ctx.min(cur.len() - (last.c + last.add));
        let (b_start, c_start) = (first.b - lead, first.c - lead);
        let (b_len, c_len) = (
            last.b + last.del + trail - b_start,
            last.c + last.add + trail - c_start,
        );
        // Como git: con 0 líneas el inicio no se desplaza
        let at = |start: usize, len: usize| if len == 0 { start } else { start + 1 };
        out.push(format!(
            "@@ -{},{} +{},{} @@",
            at(b_start, b_len),
            b_len,
            at(c_start, c_len),
            c_len
        ));
        let mut pos = c_start;
        for h in &hunks[k..=end] {
            out.extend(cur[pos..h.c].iter().map(|l| format!(" {l}")));
            out.extend(base[h.b..h.b + h.del].iter().map(|l| format!("-{l}")));
            out.extend(cur[h.c..h.c + h.add].iter().map(|l| format!("+{l}")));
            pos = h.c + h.add;
        }
        out.extend(cur[pos..c_start + c_len].iter().map(|l| format!(" {l}")));
        k = end + 1;
    }
    out
}

impl super::viewer::FileViewerState {
    pub(crate) fn is_diff(&self) -> bool {
        self.diff_mode != DiffMode::None || self.diff_stash.is_some()
    }

    pub(crate) fn is_side_by_side(&self) -> bool {
        self.diff_mode == DiffMode::SideBySide
    }

    #[cfg(test)]
    pub(crate) fn is_unified_diff(&self) -> bool {
        self.diff_mode == DiffMode::Unified || self.diff_stash.is_some()
    }

    /// Indica si el visor puede mostrar diff (tiene versión base en HEAD, es un archivo nuevo de Git, o ya está en diff)
    pub(crate) fn has_diff_capability(&self) -> bool {
        self.git_base.is_some() || self.git_new_file || self.is_diff()
    }

    pub(crate) fn side_by_side_rows(&self) -> Rc<[SideBySideRow]> {
        if let Some((gen, ref rows)) = *self.side_by_side_cache.borrow() {
            if gen == self.edit_gen {
                return Rc::clone(rows);
            }
        }
        let empty = Vec::new();
        let base = if let Some(base) = &self.git_base {
            base
        } else if self.git_new_file {
            &empty
        } else {
            &self.original_lines
        };
        let rows: Rc<[SideBySideRow]> = Rc::from(compute_side_by_side(base, &self.lines));
        *self.side_by_side_cache.borrow_mut() = Some((self.edit_gen, Rc::clone(&rows)));
        rows
    }

    pub(crate) fn side_by_side_stats(&self) -> (usize, usize) {
        let rows = self.side_by_side_rows();
        let mut dels = 0;
        let mut adds = 0;
        for r in rows.iter() {
            match r.left.kind {
                DiffCellKind::Deleted | DiffCellKind::Modified => dels += 1,
                _ => {}
            }
            match r.right.kind {
                DiffCellKind::Added | DiffCellKind::Modified => adds += 1,
                _ => {}
            }
        }
        (dels, adds)
    }

    pub(crate) fn total_display_rows(&self) -> usize {
        if self.is_side_by_side() {
            self.side_by_side_rows().len()
        } else {
            self.rows().len()
        }
    }

    /// Alterna el modo de visualización del diff (Lado a lado <-> Unificado)
    pub(crate) fn toggle_diff_mode(&mut self) {
        if self.diff_mode == DiffMode::SideBySide {
            self.apply_unified_diff();
            self.diff_mode = DiffMode::Unified;
        } else if self.diff_mode == DiffMode::Unified || self.diff_stash.is_some() {
            self.restore_unified_diff();
            self.diff_mode = DiffMode::SideBySide;
        } else {
            self.diff_mode = DiffMode::SideBySide;
        }
        self.edit_gen = self.edit_gen.wrapping_add(1);
        self.fold_gen = self.fold_gen.wrapping_add(1);
    }

    fn ensure_git_marks(&self) {
        let mut cache = self.git_marks.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(self.edit_gen) {
            let base = self.git_base.as_ref().unwrap_or(&self.original_lines);
            let marks = diff_marks(base, &self.lines);
            let has_changes = marks.iter().any(Option::is_some);
            *cache = Some((self.edit_gen, has_changes, Rc::from(marks)));
        }
    }

    /// El texto (el real, aunque se esté viendo el diff) difiere de `HEAD`
    pub(crate) fn has_git_changes(&self) -> bool {
        let Some(base) = &self.git_base else {
            return false;
        };
        if let Some(st) = &self.diff_stash {
            return base != &st.lines;
        }
        self.ensure_git_marks();
        self.git_marks.borrow().as_ref().is_some_and(|c| c.1)
    }

    /// Alterna entre el archivo y su visor de cambios (por defecto modo Lado a lado)
    pub(crate) fn toggle_diff(&mut self) {
        if self.is_diff() {
            if self.diff_stash.is_some() {
                self.restore_unified_diff();
            }
            self.diff_mode = DiffMode::None;
        } else {
            self.diff_mode = DiffMode::SideBySide;
        }
        self.edit_gen = self.edit_gen.wrapping_add(1);
        self.fold_gen = self.fold_gen.wrapping_add(1);
    }

    /// Alterna directamente el diff unificado (mantiene retrocompatibilidad y pruebas)
    #[cfg(test)]
    pub(crate) fn toggle_unified_diff(&mut self) {
        if self.diff_stash.is_some() {
            self.restore_unified_diff();
            self.diff_mode = DiffMode::None;
        } else {
            self.apply_unified_diff();
            self.diff_mode = DiffMode::Unified;
        }
        self.edit_gen = self.edit_gen.wrapping_add(1);
        self.fold_gen = self.fold_gen.wrapping_add(1);
    }

    fn apply_unified_diff(&mut self) {
        if self.diff_stash.is_some() {
            return;
        }
        let empty = Vec::new();
        let base = if let Some(base) = &self.git_base {
            base
        } else if self.git_new_file {
            &empty
        } else {
            &self.original_lines
        };
        let mut diff = unified_diff(base, &self.lines, 3);
        if diff.is_empty() {
            diff.push("Sin cambios respecto a HEAD".to_string());
        }
        self.diff_stash = Some(Box::new(DiffStash {
            lines: std::mem::replace(&mut self.lines, diff),
            original_lines: Vec::new(),
            cursor_line: std::mem::take(&mut self.cursor_line),
            cursor_col: std::mem::take(&mut self.cursor_col),
            scroll_offset: std::mem::take(&mut self.scroll_offset),
            scroll_offset_x: std::mem::take(&mut self.scroll_offset_x),
            selection: self.selection.take(),
            folded: std::mem::take(&mut self.folded),
            render_markdown: std::mem::replace(&mut self.render_markdown, false),
        }));
        let shown = self.lines.clone();
        if let Some(st) = self.diff_stash.as_mut() {
            st.original_lines = std::mem::replace(&mut self.original_lines, shown);
        }
    }

    fn restore_unified_diff(&mut self) {
        if let Some(st) = self.diff_stash.take() {
            let st = *st;
            self.lines = st.lines;
            self.original_lines = st.original_lines;
            self.cursor_line = st.cursor_line;
            self.cursor_col = st.cursor_col;
            self.scroll_offset = st.scroll_offset;
            self.scroll_offset_x = st.scroll_offset_x;
            self.selection = st.selection;
            self.folded = st.folded;
            self.render_markdown = st.render_markdown;
        }
    }

    pub(crate) fn get_line_git_diff_status(&self, line_idx: usize) -> Option<GitLineStatus> {
        if self.is_diff() {
            return None;
        }
        self.ensure_git_marks();
        self.git_marks
            .borrow()
            .as_ref()?
            .2
            .get(line_idx)
            .copied()
            .flatten()
    }
}
