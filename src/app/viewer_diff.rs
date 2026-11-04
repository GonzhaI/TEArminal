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

/// Calcula las filas alineadas y sincronizadas para el visor de diff lado a lado.
pub(crate) fn compute_side_by_side(base: &[String], cur: &[String]) -> Vec<SideBySideRow> {
    let hunks = diff_hunks(base, cur);
    let mut rows = Vec::new();
    let mut base_idx = 0;
    let mut cur_idx = 0;

    for h in hunks {
        while base_idx < h.b && cur_idx < h.c {
            rows.push(SideBySideRow {
                left: SideBySideCell {
                    line_num: Some(base_idx + 1),
                    text: base[base_idx].clone(),
                    kind: DiffCellKind::Normal,
                },
                right: SideBySideCell {
                    line_num: Some(cur_idx + 1),
                    text: cur[cur_idx].clone(),
                    kind: DiffCellKind::Normal,
                },
            });
            base_idx += 1;
            cur_idx += 1;
        }

        let count = h.del.max(h.add);
        for k in 0..count {
            let left = if k < h.del {
                let idx = h.b + k;
                let kind = if k < h.add {
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

            let right = if k < h.add {
                let idx = h.c + k;
                let kind = if k < h.del {
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

        base_idx = h.b + h.del;
        cur_idx = h.c + h.add;
    }

    while base_idx < base.len() || cur_idx < cur.len() {
        let left_text = base.get(base_idx).cloned().unwrap_or_default();
        let right_text = cur.get(cur_idx).cloned().unwrap_or_default();
        let has_left = base_idx < base.len();
        let has_right = cur_idx < cur.len();

        let (left_kind, right_kind) = if has_left && has_right && left_text == right_text {
            (DiffCellKind::Normal, DiffCellKind::Normal)
        } else {
            (
                if has_left {
                    DiffCellKind::Deleted
                } else {
                    DiffCellKind::Empty
                },
                if has_right {
                    DiffCellKind::Added
                } else {
                    DiffCellKind::Empty
                },
            )
        };

        rows.push(SideBySideRow {
            left: SideBySideCell {
                line_num: has_left.then_some(base_idx + 1),
                text: left_text,
                kind: left_kind,
            },
            right: SideBySideCell {
                line_num: has_right.then_some(cur_idx + 1),
                text: right_text,
                kind: right_kind,
            },
        });

        if has_left {
            base_idx += 1;
        }
        if has_right {
            cur_idx += 1;
        }
    }

    if rows.is_empty() {
        rows.push(SideBySideRow {
            left: SideBySideCell {
                line_num: None,
                text: String::new(),
                kind: DiffCellKind::Normal,
            },
            right: SideBySideCell {
                line_num: None,
                text: String::new(),
                kind: DiffCellKind::Normal,
            },
        });
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
