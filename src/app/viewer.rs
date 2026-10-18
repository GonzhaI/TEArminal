//! Estado del visor/editor de archivos en pestaña: texto, selección, deshacer y diff de git.

#[derive(Debug, Clone)]
pub struct ViewerSnapshot {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionStats {
    pub chars: usize,
    pub lines: usize,
}

/// Fila visual del visor: un tramo [start, end) de caracteres de una línea (`end == usize::MAX` = hasta el final).
/// El ajuste de línea parte una línea en varias filas y el plegado oculta líneas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowRef {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct RowsCache {
    key: (u64, u64, usize, bool),
    rows: Rc<Vec<RowRef>>,
    max_cols_gen: u64,
    max_cols: usize,
}

#[inline]
fn char_byte_idx(s: &str, col: usize) -> usize {
    if s.is_ascii() {
        col.min(s.len())
    } else {
        s.char_indices().nth(col).map_or(s.len(), |(b, _)| b)
    }
}

#[inline]
fn char_count(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

#[inline]
fn line_col_width(s: &str) -> usize {
    if s.is_ascii() {
        let tabs = s.bytes().filter(|&b| b == b'\t').count();
        s.len() + tabs * 3
    } else {
        s.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let n = line.chars().take_while(|c| *c == '#').count();
    ((1..=6).contains(&n) && line[n..].starts_with(' ')).then_some(n)
}

fn indent_of(line: &str) -> Option<usize> {
    (!line.trim().is_empty()).then(|| {
        line.chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum()
    })
}

/// Parte `line` en tramos de como mucho `cols` columnas, prefiriendo cortar tras un espacio.
fn wrap_line(line: &str, cols: usize, line_idx: usize, out: &mut Vec<RowRef>) {
    let cols = cols.max(8);
    if line_col_width(line) <= cols {
        out.push(RowRef {
            line: line_idx,
            start: 0,
            end: usize::MAX,
        });
        return;
    }
    let (mut start, mut width, mut last_break) = (0usize, 0usize, None::<(usize, usize)>);
    for (i, ch) in line.chars().enumerate() {
        let adv = if ch == '\t' { 4 } else { 1 };
        if width + adv > cols && i > start {
            let (at, rem_width) = match last_break {
                Some((b, w_at_b)) if b > start + cols / 2 => (b, width - w_at_b),
                _ => (i, 0),
            };
            out.push(RowRef {
                line: line_idx,
                start,
                end: at,
            });
            start = at;
            last_break = None;
            width = rem_width;
        }
        width += adv;
        if ch.is_whitespace() {
            last_break = Some((i + 1, width));
        }
    }
    out.push(RowRef {
        line: line_idx,
        start,
        end: usize::MAX,
    });
}

#[derive(Debug, Clone)]
pub struct FileViewerState {
    pub path: std::path::PathBuf,
    pub file_name: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
    pub scroll_offset_x: usize,
    pub is_binary: bool,
    pub file_size: u64,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub is_dirty: bool,
    pub undo_stack: Vec<ViewerSnapshot>,
    pub redo_stack: Vec<ViewerSnapshot>,
    pub original_lines: Vec<String>,
    /// Contenido en `HEAD` (base del diff del margen). `None` fuera de un repo: se compara con el texto al abrir/guardar.
    pub git_base: Option<Vec<String>>,
    /// Con el diff «HEAD vs ahora» a la vista: guarda el estado real del visor para volver a él
    pub diff_stash: Option<Box<DiffStash>>,
    pub hovered_diff_btn: bool,
    /// Marca de cada línea frente a la base y si existen cambios, en caché por `edit_gen`
    pub git_marks: RefCell<Option<(u64, bool, GitMarks)>>,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub remote_destination: Option<String>,
    pub remote_branch: Option<String>,
    pub is_read_only: bool,
    /// Fin de línea y salto final del archivo al abrirlo: se conservan al guardar
    pub crlf: bool,
    pub final_newline: bool,
    /// El archivo no era UTF-8: se muestra con sustitutos y nunca se escribe de vuelta
    pub non_utf8: bool,
    /// Último fallo de guardado (se muestra en la cabecera hasta que un guardado funcione)
    pub save_error: Option<String>,
    pub render_markdown: bool,
    pub hovered_mode_btn: bool,
    pub hovered_md_btn: bool,
    pub hovered_scrollbar: bool,
    pub hovered_hscrollbar: bool,
    pub hovered_gutter: bool,
    /// Columnas de texto visibles (las fija el render según el ancho real de la pestaña)
    pub visible_cols: usize,
    /// Ajuste de línea (lo sincroniza el render desde la configuración global)
    pub word_wrap: bool,
    /// Líneas plegadas (primera línea de cada región oculta)
    pub folded: BTreeSet<usize>,
    pub needs_autosave: bool,
    pub last_edit_time: Instant,
    pub edit_gen: u64,
    pub fold_gen: u64,
    pub rows_cache: RefCell<Option<RowsCache>>,
}

/// Texto de un archivo tal como lo ve el visor, más lo necesario para escribirlo igual que se leyó
pub struct DecodedText {
    pub lines: Vec<String>,
    pub crlf: bool,
    pub final_newline: bool,
    pub non_utf8: bool,
}

impl DecodedText {
    pub fn decode(bytes: &[u8]) -> Self {
        let (text, non_utf8) = match std::str::from_utf8(bytes) {
            Ok(t) => (std::borrow::Cow::Borrowed(t), false),
            Err(_) => (String::from_utf8_lossy(bytes), true),
        };
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            crlf: text.find('\n').is_some_and(|i| text[..i].ends_with('\r')),
            final_newline: text.ends_with('\n'),
            non_utf8,
        }
    }
}

impl FileViewerState {
    /// Única puerta de las mutaciones: en modo lectura (o binario) el texto no puede cambiar por ninguna vía.
    pub fn is_editable(&self) -> bool {
        !self.is_binary && !self.is_read_only && !self.non_utf8 && !self.is_diff()
    }

    /// Alterna lectura/edición; un archivo que no es UTF-8 no puede pasar a edición (se corrompería al guardar)
    pub fn toggle_read_only(&mut self) {
        if !self.non_utf8 {
            self.is_read_only = !self.is_read_only;
        }
    }

    pub fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
    }

    /// El ajuste de línea no se aplica a la vista previa de Markdown (se dibuja línea a línea)
    pub fn wrap_active(&self) -> bool {
        self.word_wrap && !(self.render_markdown && self.is_markdown())
    }

    /// Filas visuales (ajuste de línea + plegado), en caché mientras no cambien texto, pliegues, ancho ni modo.
    pub fn rows(&self) -> Rc<Vec<RowRef>> {
        let wrap = self.wrap_active();
        let key = (self.edit_gen, self.fold_gen, self.visible_cols, wrap);
        let prev_max_cols = {
            let borrow = self.rows_cache.borrow();
            if let Some(c) = &*borrow {
                if c.key == key {
                    return c.rows.clone();
                }
                (c.max_cols_gen == self.edit_gen).then_some(c.max_cols)
            } else {
                None
            }
        };
        let mut rows = Vec::with_capacity(self.lines.len());
        let mut i = 0;
        while i < self.lines.len() {
            if wrap {
                wrap_line(&self.lines[i], self.visible_cols, i, &mut rows);
            } else {
                rows.push(RowRef {
                    line: i,
                    start: 0,
                    end: usize::MAX,
                });
            }
            i = match self.folded.contains(&i).then(|| self.fold_end(i)).flatten() {
                Some(end) => end + 1,
                None => i + 1,
            };
        }
        if rows.is_empty() {
            rows.push(RowRef {
                line: 0,
                start: 0,
                end: usize::MAX,
            });
        }
        let max_cols = prev_max_cols.unwrap_or_else(|| {
            self.lines
                .iter()
                .map(|l| line_col_width(l))
                .max()
                .unwrap_or(0)
        });
        let rows = Rc::new(rows);
        *self.rows_cache.borrow_mut() = Some(RowsCache {
            key,
            rows: rows.clone(),
            max_cols_gen: self.edit_gen,
            max_cols,
        });
        rows
    }

    /// Texto del tramo de una fila visual
    pub fn row_text(&self, row: &RowRef) -> &str {
        let line = self.lines.get(row.line).map_or("", |l| l.as_str());
        if row.start == 0 && row.end == usize::MAX {
            return line;
        }
        let from = char_byte_idx(line, row.start);
        let to = if row.end == usize::MAX {
            line.len()
        } else {
            char_byte_idx(line, row.end)
        };
        &line[from..to]
    }

    /// (línea, columna) bajo la fila visual `row_idx` y la coordenada X del ratón
    pub fn hit_test(
        &self,
        row_idx: usize,
        mx: usize,
        lay: &crate::renderer::FileViewerLayout,
    ) -> Option<(usize, usize)> {
        let rows = self.rows();
        let row = rows.get(row_idx).or(rows.last())?;
        let seg = self.row_text(row);
        let scroll_x = if self.wrap_active() {
            0
        } else {
            self.scroll_offset_x
        };
        let mut col = lay.col_at(seg, mx, scroll_x);
        if row.end != usize::MAX {
            col = col.min(char_count(seg).saturating_sub(1));
        }
        Some((row.line, row.start + col))
    }

    /// Índice de la fila visual que contiene (línea, columna); `None` si la línea está plegada
    pub fn row_of(&self, line: usize, col: usize) -> Option<usize> {
        self.rows()
            .iter()
            .rposition(|r| r.line == line && r.start <= col)
    }

    /// Última línea de la región plegable que empieza en `start` (títulos en Markdown, sangría en el resto)
    pub fn fold_end(&self, start: usize) -> Option<usize> {
        let line = self.lines.get(start)?;
        let n = self.lines.len();
        if self.is_markdown() {
            if let Some(level) = heading_level(line) {
                let mut end = start;
                for i in start + 1..n {
                    if heading_level(&self.lines[i]).is_some_and(|l| l <= level) {
                        break;
                    }
                    if !self.lines[i].trim().is_empty() {
                        end = i;
                    }
                }
                return (end > start).then_some(end);
            }
        }
        let base = indent_of(line)?;
        let mut end = None;
        for i in start + 1..n {
            match indent_of(&self.lines[i]) {
                None => continue,
                Some(ind) if ind > base => end = Some(i),
                _ => break,
            }
        }
        end
    }

    /// Plega o despliega la región que empieza en `line`
    pub fn toggle_fold(&mut self, line: usize) {
        if !self.folded.remove(&line) {
            let Some(end) = self.fold_end(line) else {
                return;
            };
            self.folded.insert(line);
            if self.cursor_line > line && self.cursor_line <= end {
                self.cursor_line = line;
                self.cursor_col = 0;
            }
            self.selection = None;
        }
        self.fold_gen += 1;
    }

    pub fn fold_all(&mut self) {
        let starts: Vec<usize> = (0..self.lines.len())
            .filter(|&i| self.fold_end(i).is_some())
            .collect();
        self.folded.extend(starts);
        self.fold_gen += 1;
        self.selection = None;
        // si el cursor quedó oculto, sube al encabezado visible más externo que lo contiene
        let hidden_by = self.folded.iter().copied().find(|&s| {
            s < self.cursor_line && self.fold_end(s).is_some_and(|e| self.cursor_line <= e)
        });
        if let Some(top) = hidden_by {
            self.cursor_line = top;
            self.cursor_col = 0;
        }
        self.scroll_offset = self.scroll_offset.min(self.rows().len().saturating_sub(1));
    }

    pub fn unfold_all(&mut self) {
        if !self.folded.is_empty() {
            self.folded.clear();
            self.fold_gen += 1;
        }
    }

    /// Despliega las regiones que ocultan `line` (el cursor nunca queda en una línea oculta)
    pub fn reveal_line(&mut self, line: usize) {
        let hiding: Vec<usize> = self
            .folded
            .iter()
            .copied()
            .filter(|&s| s < line && self.fold_end(s).is_some_and(|e| line <= e))
            .collect();
        if !hiding.is_empty() {
            for s in hiding {
                self.folded.remove(&s);
            }
            self.fold_gen += 1;
        }
    }

    pub fn selection_ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let (p1, p2) = self.selection?;
        if p1 <= p2 {
            Some((p1, p2))
        } else {
            Some((p2, p1))
        }
    }

    /// Ancho en columnas de la línea más larga (tabulador = 4), para el scroll horizontal.
    pub fn max_cols(&self) -> usize {
        if let Some(c) = &*self.rows_cache.borrow() {
            if c.max_cols_gen == self.edit_gen {
                return c.max_cols;
            }
        }
        let max = self
            .lines
            .iter()
            .map(|l| line_col_width(l))
            .max()
            .unwrap_or(0);
        if let Some(c) = &mut *self.rows_cache.borrow_mut() {
            c.max_cols_gen = self.edit_gen;
            c.max_cols = max;
        }
        max
    }

    /// Máximo `scroll_offset_x`: deja 2 columnas libres para el cursor al final de la línea más larga.
    pub fn max_scroll_x(&self) -> usize {
        (self.max_cols() + 2).saturating_sub(self.visible_cols.max(1))
    }

    /// Resumen de la selección: caracteres y líneas (sin alojar memoria)
    pub fn selection_stats(&self) -> Option<SelectionStats> {
        let ((l1, c1), (l2, c2)) = self.selection_ordered()?;
        if (l1 == l2 && c1 == c2) || l1 >= self.lines.len() {
            return None;
        }
        let mut chars = 0usize;
        let mut parts = 0usize;
        for line_idx in l1..=l2.min(self.lines.len().saturating_sub(1)) {
            let line = &self.lines[line_idx];
            let char_len = char_count(line);
            let start = if line_idx == l1 { c1.min(char_len) } else { 0 };
            let end = if line_idx == l2 {
                c2.min(char_len)
            } else {
                char_len
            };
            if start < end {
                chars += end - start;
                parts += 1;
            } else if line_idx != l2 {
                parts += 1;
            }
        }
        chars += parts.saturating_sub(1);
        Some(SelectionStats {
            chars,
            lines: l2 - l1 + 1,
        })
    }

    pub fn has_selection(&self) -> bool {
        if let Some((p1, p2)) = self.selection_ordered() {
            p1 != p2
        } else {
            false
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selected_text(&self) -> Option<String> {
        let ((l1, c1), (l2, c2)) = self.selection_ordered()?;
        if l1 == l2 && c1 == c2 {
            return None;
        }
        if l1 >= self.lines.len() {
            return None;
        }

        let mut res = Vec::new();
        for line_idx in l1..=l2.min(self.lines.len().saturating_sub(1)) {
            let line = &self.lines[line_idx];
            let len = char_count(line);
            let start = if line_idx == l1 { c1.min(len) } else { 0 };
            let end = if line_idx == l2 { c2.min(len) } else { len };

            if start < end {
                let s = &line[char_byte_idx(line, start)..char_byte_idx(line, end)];
                res.push(s);
            } else if line_idx != l2 {
                res.push("");
            }
        }
        Some(res.join("\n"))
    }

    pub fn delete_selection(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let Some(((l1, c1), (l2, c2))) = self.selection_ordered() else {
            return false;
        };
        if l1 == l2 && c1 == c2 {
            self.selection = None;
            return false;
        }
        self.push_undo();

        if l1 == l2 {
            if l1 < self.lines.len() {
                let line = &mut self.lines[l1];
                let start_byte = char_byte_idx(line, c1);
                let end_byte = char_byte_idx(line, c2);
                line.drain(start_byte..end_byte);
            }
        } else {
            let prefix_end = char_byte_idx(&self.lines[l1], c1);
            let mut prefix = self.lines[l1][..prefix_end].to_string();

            if l2 < self.lines.len() {
                let suffix_start = char_byte_idx(&self.lines[l2], c2);
                prefix.push_str(&self.lines[l2][suffix_start..]);
            }

            self.lines[l1] = prefix;

            let drain_end = (l2 + 1).min(self.lines.len());
            if l1 + 1 < drain_end {
                self.lines.drain((l1 + 1)..drain_end);
            }
        }

        self.cursor_line = l1.min(self.lines.len().saturating_sub(1));
        self.cursor_col = c1;
        self.selection = None;
        self.is_dirty = true;
        true
    }

    pub fn select_all(&mut self) {
        if self.lines.is_empty() {
            self.selection = None;
            return;
        }
        let last_line = self.lines.len() - 1;
        let last_col = char_count(&self.lines[last_line]);
        self.selection = Some(((0, 0), (last_line, last_col)));
        self.cursor_line = last_line;
        self.cursor_col = last_col;
    }

    pub fn select_word_at_cursor(&mut self) {
        if self.lines.is_empty() || self.cursor_line >= self.lines.len() {
            return;
        }
        let line = &self.lines[self.cursor_line];
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return;
        }
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
        let target_type = is_word_char(chars[col]);

        let mut start_col = col;
        while start_col > 0
            && is_word_char(chars[start_col - 1]) == target_type
            && !chars[start_col - 1].is_whitespace()
        {
            start_col -= 1;
        }

        let mut end_col = col;
        while end_col < chars.len()
            && is_word_char(chars[end_col]) == target_type
            && !chars[end_col].is_whitespace()
        {
            end_col += 1;
        }

        self.selection = Some(((self.cursor_line, start_col), (self.cursor_line, end_col)));
        self.cursor_col = end_col;
    }

    pub fn select_line_at_cursor(&mut self) {
        if self.lines.is_empty() || self.cursor_line >= self.lines.len() {
            return;
        }
        let line_len = char_count(&self.lines[self.cursor_line]);
        self.selection = Some(((self.cursor_line, 0), (self.cursor_line, line_len)));
        self.cursor_col = line_len;
    }
    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(ViewerSnapshot {
            lines: self.lines.clone(),
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
        });
        self.redo_stack.clear();
        self.edit_gen += 1;
    }

    pub fn undo(&mut self) {
        if !self.is_editable() {
            return;
        }
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(ViewerSnapshot {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
            });
            self.lines = prev.lines;
            self.cursor_line = prev.cursor_line.min(self.lines.len().saturating_sub(1));
            self.cursor_col = prev.cursor_col;
            self.is_dirty = self.lines != self.original_lines;
            self.edit_gen += 1;
        }
    }

    pub fn redo(&mut self) {
        if !self.is_editable() {
            return;
        }
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(ViewerSnapshot {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
            });
            self.lines = next.lines;
            self.cursor_line = next.cursor_line.min(self.lines.len().saturating_sub(1));
            self.cursor_col = next.cursor_col;
            self.is_dirty = self.lines != self.original_lines;
            self.edit_gen += 1;
        }
    }

    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        self.reveal_line(self.cursor_line);
        if let Some(row) = self.row_of(self.cursor_line, self.cursor_col) {
            if row < self.scroll_offset {
                self.scroll_offset = row;
            } else if row >= self.scroll_offset + visible_lines {
                self.scroll_offset = row + 1 - visible_lines.max(1);
            }
        }

        // Scroll horizontal (solo sin ajuste de línea)
        if self.wrap_active() {
            self.scroll_offset_x = 0;
        } else if self.cursor_col < self.scroll_offset_x {
            self.scroll_offset_x = self.cursor_col;
        } else if self.cursor_col >= self.scroll_offset_x + self.visible_cols {
            self.scroll_offset_x = self.cursor_col.saturating_sub(self.visible_cols - 1);
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if !self.is_editable() {
            return;
        }
        if !self.delete_selection() {
            self.push_undo();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &mut self.lines[self.cursor_line];
        let byte_pos = char_byte_idx(line, self.cursor_col);
        line.insert(byte_pos, ch);
        self.cursor_col += 1;
        self.clear_selection();
        self.is_dirty = true;
    }

    pub fn insert_text(&mut self, text: &str) {
        if !self.is_editable() || text.is_empty() {
            return;
        }
        if !self.delete_selection() {
            self.push_undo();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let text_clean = text.replace("\r\n", "\n").replace('\r', "\n");
        let parts: Vec<&str> = text_clean.split('\n').collect();

        if parts.len() == 1 {
            let line = &mut self.lines[self.cursor_line];
            let byte_pos = char_byte_idx(line, self.cursor_col);
            line.insert_str(byte_pos, parts[0]);
            self.cursor_col += char_count(parts[0]);
        } else {
            let line = &self.lines[self.cursor_line];
            let byte_pos = char_byte_idx(line, self.cursor_col);

            let prefix = line[..byte_pos].to_string();
            let suffix = line[byte_pos..].to_string();

            self.lines[self.cursor_line] = format!("{}{}", prefix, parts[0]);

            for (idx, &part) in parts.iter().enumerate().skip(1) {
                if idx == parts.len() - 1 {
                    self.lines
                        .insert(self.cursor_line + idx, format!("{}{}", part, suffix));
                    self.cursor_col = char_count(part);
                } else {
                    self.lines.insert(self.cursor_line + idx, part.to_string());
                }
            }
            self.cursor_line += parts.len() - 1;
        }
        self.is_dirty = true;
    }

    pub fn insert_newline(&mut self) {
        if !self.is_editable() {
            return;
        }
        if !self.delete_selection() {
            self.push_undo();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &mut self.lines[self.cursor_line];
        let byte_pos = char_byte_idx(line, self.cursor_col);
        let remainder = line[byte_pos..].to_string();
        line.truncate(byte_pos);

        self.cursor_line += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_line, remainder);
        self.clear_selection();
        self.is_dirty = true;
    }

    pub fn insert_tab(&mut self) {
        self.insert_text("    ");
    }

    pub fn backspace(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let end_byte = char_byte_idx(line, self.cursor_col);
            let start_byte = char_byte_idx(line, self.cursor_col - 1);
            if start_byte < end_byte {
                line.drain(start_byte..end_byte);
                self.cursor_col -= 1;
                self.is_dirty = true;
            }
        } else if self.cursor_line > 0 {
            // Unir la línea actual con la anterior
            let cur_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = char_count(&self.lines[self.cursor_line]);
            self.lines[self.cursor_line].push_str(&cur_line);
            self.cursor_col = prev_len;
            self.is_dirty = true;
        }
    }

    pub fn delete(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &mut self.lines[self.cursor_line];
        let start_byte = char_byte_idx(line, self.cursor_col);

        if start_byte < line.len() {
            let ch_len = line[start_byte..].chars().next().map_or(0, char::len_utf8);
            line.drain(start_byte..start_byte + ch_len);
            self.is_dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            // Unir la línea siguiente a la actual
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.is_dirty = true;
        }
    }

    /// Borrado de palabra hacia la izquierda (Ctrl + Backspace)
    pub fn delete_word_left(&mut self) {
        if !self.is_editable() || self.lines.is_empty() {
            return;
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }
        if self.cursor_col == 0 {
            self.backspace();
            return;
        }
        self.push_undo();
        let target_col = self.find_word_boundary_left();
        let line = &mut self.lines[self.cursor_line];
        let start_byte = char_byte_idx(line, target_col);
        let end_byte = char_byte_idx(line, self.cursor_col);
        line.drain(start_byte..end_byte);
        self.cursor_col = target_col;
        self.is_dirty = true;
    }

    /// Borrado de palabra hacia la derecha (Ctrl + Delete)
    pub fn delete_word_right(&mut self) {
        if !self.is_editable() || self.lines.is_empty() {
            return;
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }
        let line_len = char_count(&self.lines[self.cursor_line]);
        if self.cursor_col >= line_len {
            self.delete();
            return;
        }
        self.push_undo();
        let target_col = self.find_word_boundary_right();
        let line = &mut self.lines[self.cursor_line];
        let start_byte = char_byte_idx(line, self.cursor_col);
        let end_byte = char_byte_idx(line, target_col);
        line.drain(start_byte..end_byte);
        self.is_dirty = true;
    }

    pub fn find_word_boundary_left(&self) -> usize {
        if self.cursor_col == 0 || self.cursor_line >= self.lines.len() {
            return 0;
        }
        let chars: Vec<char> = self.lines[self.cursor_line].chars().collect();
        let mut idx = self.cursor_col.min(chars.len());

        // Saltar espacios a la izquierda
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        if idx == 0 {
            return 0;
        }

        // Si es caracter alfanumérico o guion bajo, buscar el inicio de la palabra
        let is_alphanumeric = chars[idx - 1].is_alphanumeric() || chars[idx - 1] == '_';
        if is_alphanumeric {
            while idx > 0 && (chars[idx - 1].is_alphanumeric() || chars[idx - 1] == '_') {
                idx -= 1;
            }
        } else {
            // Símbolos puntuación
            while idx > 0 && !chars[idx - 1].is_alphanumeric() && !chars[idx - 1].is_whitespace() {
                idx -= 1;
            }
        }
        idx
    }

    pub fn find_word_boundary_right(&self) -> usize {
        if self.cursor_line >= self.lines.len() {
            return 0;
        }
        let chars: Vec<char> = self.lines[self.cursor_line].chars().collect();
        let len = chars.len();
        if self.cursor_col >= len {
            return len;
        }
        let mut idx = self.cursor_col;

        let is_alphanumeric = chars[idx].is_alphanumeric() || chars[idx] == '_';
        if is_alphanumeric {
            while idx < len && (chars[idx].is_alphanumeric() || chars[idx] == '_') {
                idx += 1;
            }
        } else if !chars[idx].is_whitespace() {
            while idx < len && !chars[idx].is_alphanumeric() && !chars[idx].is_whitespace() {
                idx += 1;
            }
        }
        // Saltar espacios posteriores
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }
        idx
    }

    pub fn is_diff(&self) -> bool {
        self.diff_stash.is_some()
    }

    fn ensure_git_marks(&self) {
        let mut cache = self.git_marks.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(self.edit_gen) {
            let base = self.git_base.as_ref().unwrap_or(&self.original_lines);
            let marks = diff_marks(base, &self.lines);
            let has_changes = marks.iter().any(Option::is_some);
            *cache = Some((self.edit_gen, has_changes, Rc::new(marks)));
        }
    }

    /// El texto (el real, aunque se esté viendo el diff) difiere de `HEAD`
    pub fn has_git_changes(&self) -> bool {
        let Some(base) = &self.git_base else {
            return true;
        };
        if let Some(st) = &self.diff_stash {
            return base != &st.lines;
        }
        self.ensure_git_marks();
        self.git_marks.borrow().as_ref().is_some_and(|c| c.1)
    }

    /// Alterna entre el archivo y su diff unificado contra `HEAD` (la vista de cambios es de solo lectura)
    pub fn toggle_diff(&mut self) {
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
        } else if let Some(base) = &self.git_base {
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
            // Sin cambios propios: así `is_dirty` y el margen no ven el diff como una edición
            let shown = self.lines.clone();
            if let Some(st) = self.diff_stash.as_mut() {
                st.original_lines = std::mem::replace(&mut self.original_lines, shown);
            }
        } else {
            return;
        }
        self.edit_gen += 1;
        self.fold_gen += 1;
    }

    pub fn get_line_git_diff_status(&self, line_idx: usize) -> Option<GitLineStatus> {
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

    /// Programa el autoguardado para ejecutarse con retardo (antirrebote) tras dejar de teclear
    pub fn schedule_autosave(&mut self) {
        if self.remote_destination.is_none() {
            self.needs_autosave = true;
            self.last_edit_time = Instant::now();
        }
    }

    /// Autoguardado tras editar: solo archivos locales, sin recortar saltos en curso ni borrar el historial de deshacer
    pub fn autosave_local(&mut self) {
        if self.remote_destination.is_none() {
            self.needs_autosave = false;
            let _ = self.save_to_disk_inner(false);
        }
    }

    /// Recorta las líneas vacías finales dejando como máximo una (estándar de los IDE) y mantiene el cursor dentro de rango
    pub fn trim_trailing_blank_lines(&mut self) {
        while self.lines.len() > 1
            && self.lines.last().map(|s| s.is_empty()).unwrap_or(false)
            && self.lines[self.lines.len() - 2].is_empty()
        {
            self.lines.pop();
            self.edit_gen += 1;
        }
        let last = self.lines.len().saturating_sub(1);
        if self.cursor_line > last {
            self.cursor_line = last;
            self.cursor_col = self.lines.get(last).map_or(0, |l| char_count(l));
        }
    }

    /// Contenido de un archivo remoto tal como se escribe al guardar (siempre con `\n` final)
    pub fn remote_content(&mut self) -> String {
        self.trim_trailing_blank_lines();
        let mut content = self.lines.join("\n");
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content
    }

    /// El servidor confirmó la escritura: el texto actual pasa a ser el original
    pub fn mark_remote_saved(&mut self) {
        self.is_dirty = false;
        self.file_size = self.lines.iter().map(|l| l.len() + 1).sum::<usize>() as u64;
        self.original_lines = self.lines.clone();
        self.edit_gen += 1;
    }

    /// Escribe en `<archivo>.axon-tmp` y renombra encima: un fallo (disco lleno…) deja el original intacto
    fn write_local(&self) -> Result<(), std::io::Error> {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        let mut content = self.lines.join(eol);
        if self.final_newline && !content.is_empty() && !content.ends_with(eol) {
            content.push_str(eol);
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".axon-tmp");
        let tmp = std::path::PathBuf::from(tmp);
        // Un temporal viejo (o un enlace plantado) se borra y se crea de nuevo con `create_new`: nunca se escribe a través de él
        let _ = std::fs::remove_file(&tmp);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .and_then(|mut f| std::io::Write::write_all(&mut f, content.as_bytes()))
            .and_then(|()| std::fs::rename(&tmp, &self.path))
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&tmp);
            })
    }

    pub fn save_to_disk(&mut self) -> Result<(), std::io::Error> {
        self.needs_autosave = false;
        self.save_to_disk_inner(true)
    }

    fn save_to_disk_inner(&mut self, is_explicit_save: bool) -> Result<(), std::io::Error> {
        // En la vista de cambios `lines` es el diff, nunca debe llegar a disco
        if self.is_binary || self.non_utf8 || self.is_diff() {
            return Ok(());
        }

        if is_explicit_save {
            self.trim_trailing_blank_lines();
        }

        if self.remote_destination.is_some() {
            // El guardado remoto es asíncrono (`AxonApp::ssh_save_viewer`): nunca bloquea aquí
            return Err(std::io::Error::other(
                "los archivos remotos se guardan por SSH en segundo plano",
            ));
        }

        if let Err(e) = self.write_local() {
            self.save_error = Some(e.to_string());
            return Err(e);
        }
        self.save_error = None;
        self.file_size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if is_explicit_save {
            self.is_dirty = false;
            self.original_lines = self.lines.clone();
        }
        self.edit_gen += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitLineStatus {
    Added,
    Modified,
    Deleted,
}

/// Estado del visor mientras se muestra el diff (se restaura al volver al archivo)
#[derive(Debug, Clone)]
pub struct DiffStash {
    lines: Vec<String>,
    original_lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    scroll_offset: usize,
    scroll_offset_x: usize,
    selection: Option<((usize, usize), (usize, usize))>,
    folded: BTreeSet<usize>,
    render_markdown: bool,
}

type GitMarks = Rc<Vec<Option<GitLineStatus>>>;

/// Máximo de celdas de la tabla LCS; por encima, todo el tramo cambiado se marca como modificado.
const LCS_MAX_CELLS: usize = 4_000_000;

/// Tramo cambiado: `del` líneas de la base desde `b` sustituidas por `add` líneas de la actual desde `c`.
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
    let mut out = Vec::new();
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
