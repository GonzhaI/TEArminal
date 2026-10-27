//! Estado del visor/editor de archivos en pestaña: texto, selección, deshacer y diff de git.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ViewerSnapshot {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
}

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SelectionStats {
    pub(crate) chars: usize,
    pub(crate) lines: usize,
}

/// Fila visual del visor: un tramo [start, end) de caracteres de una línea (`end == usize::MAX` = hasta el final).
/// El ajuste de línea parte una línea en varias filas y el plegado oculta líneas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RowRef {
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RowsCache {
    key: (u64, u64, usize, bool),
    rows: Rc<[RowRef]>,
    max_cols_gen: u64,
    max_cols: usize,
}

use unicode_segmentation::UnicodeSegmentation;

#[inline]
fn char_byte_idx(s: &str, col: usize) -> usize {
    if s.is_ascii() {
        col.min(s.len())
    } else {
        s.grapheme_indices(true)
            .nth(col)
            .map_or(s.len(), |(b, _)| b)
    }
}

#[inline]
fn char_count(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.graphemes(true).count()
    }
}

#[inline]
fn line_col_width(s: &str) -> usize {
    if s.is_ascii() {
        let tabs = s.bytes().filter(|&b| b == b'\t').count();
        s.len() + tabs * 3
    } else {
        use unicode_width::UnicodeWidthStr;
        s.graphemes(true)
            .map(|g| if g == "\t" { 4 } else { g.width().max(1) })
            .sum()
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let n = line.chars().take_while(|c| *c == '#').count();
    ((1..=6).contains(&n) && line[n..].starts_with(' ')).then_some(n)
}

pub(crate) fn indent_of(line: &str) -> Option<usize> {
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
    for (i, g) in line.graphemes(true).enumerate() {
        let adv = if g == "\t" {
            4
        } else {
            use unicode_width::UnicodeWidthStr;
            g.width().max(1)
        };
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
        if g.chars().all(char::is_whitespace) {
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
pub(crate) struct FileViewerState {
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
    /// Fecha de modificación del archivo local al abrirlo o guardarlo (`None` si es remoto o aún no existe):
    /// si en disco es otra, un agente u otro programa lo cambió y no se sobrescribe sin avisar
    pub disk_mtime: Option<std::time::SystemTime>,
    pub needs_autosave: bool,
    pub last_edit_time: Instant,
    pub edit_gen: u64,
    pub fold_gen: u64,
    pub rows_cache: RefCell<Option<RowsCache>>,
}

/// Texto de un archivo tal como lo ve el visor, más lo necesario para escribirlo igual que se leyó
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DecodedText {
    pub lines: Vec<String>,
    pub crlf: bool,
    pub final_newline: bool,
    pub non_utf8: bool,
}

impl DecodedText {
    /// Decodifica texto desde cualquier flujo de lectura genérico (std::io::Read)
    #[cfg(test)]
    pub(crate) fn from_reader(mut reader: impl std::io::Read) -> std::io::Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(Self::decode(&bytes))
    }

    pub(crate) fn decode(bytes: &[u8]) -> Self {
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

impl From<&[u8]> for DecodedText {
    #[inline]
    fn from(bytes: &[u8]) -> Self {
        Self::decode(bytes)
    }
}

impl FileViewerState {
    /// Única puerta de las mutaciones: en modo lectura (o binario) el texto no puede cambiar por ninguna vía.
    pub(crate) fn is_editable(&self) -> bool {
        !self.is_binary && !self.is_read_only && !self.non_utf8 && !self.is_diff()
    }

    /// Alterna lectura/edición; un archivo que no es UTF-8 no puede pasar a edición (se corrompería al guardar)
    pub(crate) fn toggle_read_only(&mut self) {
        if !self.non_utf8 {
            self.is_read_only = !self.is_read_only;
        }
    }

    pub(crate) fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
    }

    /// El ajuste de línea no se aplica a la vista previa de Markdown (se dibuja línea a línea)
    pub(crate) fn wrap_active(&self) -> bool {
        self.word_wrap && !(self.render_markdown && self.is_markdown())
    }

    /// Detecta si el archivo utiliza sangría de 2 o 4 espacios (por defecto 4).
    pub(crate) fn indent_unit(&self) -> usize {
        let mut count_2 = 0;
        let mut count_4 = 0;
        for line in self.lines.iter().take(150) {
            if let Some(ind) = indent_of(line) {
                if ind > 0 {
                    if ind % 4 != 0 && ind % 2 == 0 {
                        count_2 += 1;
                    } else if ind % 4 == 0 {
                        count_4 += 1;
                    }
                    if count_4 >= 20 || count_2 >= 20 {
                        break;
                    }
                }
            }
        }
        if count_2 > count_4 {
            2
        } else {
            4
        }
    }

    /// Filas visuales (ajuste de línea + plegado), en caché mientras no cambien texto, pliegues, ancho ni modo.
    pub(crate) fn rows(&self) -> Rc<[RowRef]> {
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
        let rows: Rc<[RowRef]> = Rc::from(rows);
        *self.rows_cache.borrow_mut() = Some(RowsCache {
            key,
            rows: rows.clone(),
            max_cols_gen: self.edit_gen,
            max_cols,
        });
        rows
    }

    /// Texto del tramo de una fila visual
    pub(crate) fn row_text(&self, row: &RowRef) -> &str {
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
    pub(crate) fn hit_test(
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
    pub(crate) fn row_of(&self, line: usize, col: usize) -> Option<usize> {
        self.rows()
            .iter()
            .rposition(|r| r.line == line && r.start <= col)
    }

    /// Última línea de la región plegable que empieza en `start` (títulos en Markdown, sangría en el resto)
    pub(crate) fn fold_end(&self, start: usize) -> Option<usize> {
        let line = self.lines.get(start)?;
        if self.is_markdown() {
            if let Some(level) = heading_level(line) {
                let mut end = start;
                for (i, cur_line) in self.lines.iter().enumerate().skip(start + 1) {
                    if heading_level(cur_line).is_some_and(|l| l <= level) {
                        break;
                    }
                    if !cur_line.trim().is_empty() {
                        end = i;
                    }
                }
                return (end > start).then_some(end);
            }
        }
        let base = indent_of(line)?;
        let mut end = None;
        for (i, cur_line) in self.lines.iter().enumerate().skip(start + 1) {
            match indent_of(cur_line) {
                None => continue,
                Some(ind) if ind > base => end = Some(i),
                _ => break,
            }
        }
        end
    }

    /// Plega o despliega la región que empieza en `line`
    pub(crate) fn toggle_fold(&mut self, line: usize) {
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

    pub(crate) fn fold_all(&mut self) {
        let starts: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, _)| self.fold_end(i).is_some().then_some(i))
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

    pub(crate) fn unfold_all(&mut self) {
        if !self.folded.is_empty() {
            self.folded.clear();
            self.fold_gen += 1;
        }
    }

    /// Despliega las regiones que ocultan `line` (el cursor nunca queda en una línea oculta)
    pub(crate) fn reveal_line(&mut self, line: usize) {
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

    pub(crate) fn selection_ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let (p1, p2) = self.selection?;
        if p1 <= p2 {
            Some((p1, p2))
        } else {
            Some((p2, p1))
        }
    }

    /// Ancho en columnas de la línea más larga (tabulador = 4), para el scroll horizontal.
    pub(crate) fn max_cols(&self) -> usize {
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
    pub(crate) fn max_scroll_x(&self) -> usize {
        (self.max_cols() + 2).saturating_sub(self.visible_cols.max(1))
    }

    /// Resumen de la selección: caracteres y líneas (sin alojar memoria)
    pub(crate) fn selection_stats(&self) -> Option<SelectionStats> {
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

    pub(crate) fn has_selection(&self) -> bool {
        if let Some((p1, p2)) = self.selection_ordered() {
            p1 != p2
        } else {
            false
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let ((l1, c1), (l2, c2)) = self.selection_ordered()?;
        if l1 == l2 && c1 == c2 {
            return None;
        }
        if l1 >= self.lines.len() {
            return None;
        }

        let end_idx = l2.min(self.lines.len().saturating_sub(1));
        let num_lines = (end_idx + 1).saturating_sub(l1);
        let mut res = Vec::with_capacity(num_lines);
        for line_idx in l1..=end_idx {
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

    pub(crate) fn delete_selection(&mut self) -> bool {
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

    pub(crate) fn select_all(&mut self) {
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

    #[inline]
    pub(crate) fn line_len(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, |l| char_count(l))
    }

    pub(crate) fn select_word_at_cursor(&mut self) {
        if self.lines.is_empty() || self.cursor_line >= self.lines.len() {
            return;
        }
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        if graphemes.is_empty() {
            return;
        }
        let col = self.cursor_col.min(graphemes.len().saturating_sub(1));
        let is_word_grapheme = |g: &str| g.chars().all(|c| c.is_alphanumeric() || c == '_');
        let is_ws_grapheme = |g: &str| g.chars().all(char::is_whitespace);
        let target_type = is_word_grapheme(graphemes[col]);

        let mut start_col = col;
        while start_col > 0
            && is_word_grapheme(graphemes[start_col - 1]) == target_type
            && !is_ws_grapheme(graphemes[start_col - 1])
        {
            start_col -= 1;
        }

        let mut end_col = col;
        while end_col < graphemes.len()
            && is_word_grapheme(graphemes[end_col]) == target_type
            && !is_ws_grapheme(graphemes[end_col])
        {
            end_col += 1;
        }

        self.selection = Some(((self.cursor_line, start_col), (self.cursor_line, end_col)));
        self.cursor_col = end_col;
    }

    pub(crate) fn select_line_at_cursor(&mut self) {
        if self.lines.is_empty() || self.cursor_line >= self.lines.len() {
            return;
        }
        let line_len = char_count(&self.lines[self.cursor_line]);
        self.selection = Some(((self.cursor_line, 0), (self.cursor_line, line_len)));
        self.cursor_col = line_len;
    }

    /// Extrae un enlace web externo (Markdown `[texto](url)` o URL cruda `http://`/`https://`)
    /// bajo la línea y columna especificadas.
    pub(crate) fn link_at(&self, line_idx: usize, col: usize) -> Option<String> {
        let line = self.lines.get(line_idx)?;
        let chars: Vec<char> = line.chars().collect();
        let col = col.min(chars.len());

        // 1. Detectar enlaces Markdown: [texto](url)
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '[' {
                if let Some(close_bracket) = chars[i + 1..]
                    .iter()
                    .position(|&c| c == ']')
                    .map(|p| i + 1 + p)
                {
                    if chars.get(close_bracket + 1) == Some(&'(') {
                        if let Some(close_paren) = chars[close_bracket + 2..]
                            .iter()
                            .position(|&c| c == ')')
                            .map(|p| close_bracket + 2 + p)
                        {
                            let span_end = if self.render_markdown {
                                close_bracket
                            } else {
                                close_paren
                            };
                            if col >= i && col <= span_end {
                                let url: String =
                                    chars[close_bracket + 2..close_paren].iter().collect();
                                let clean_url = url.trim();
                                if clean_url.starts_with("http://")
                                    || clean_url.starts_with("https://")
                                {
                                    return Some(clean_url.to_string());
                                }
                            }
                            i = close_paren + 1;
                            continue;
                        }
                    }
                }
            }
            i += 1;
        }

        // 2. Detectar URLs sueltas http:// o https://
        let text = line.as_str();
        let mut search_from = 0;
        while search_from < text.len() {
            let rem = &text[search_from..];
            let http_pos = rem.find("http://");
            let https_pos = rem.find("https://");
            let rel_pos = match (http_pos, https_pos) {
                (Some(p1), Some(p2)) => Some(p1.min(p2)),
                (Some(p), None) | (None, Some(p)) => Some(p),
                (None, None) => None,
            };
            let Some(pos) = rel_pos else {
                break;
            };
            let abs_start = search_from + pos;
            let url_rem = &text[abs_start..];
            let end_offset = url_rem
                .find(|c: char| {
                    c.is_whitespace()
                        || matches!(c, '<' | '>' | '"' | '\'' | '`' | '{' | '}' | ')' | ']')
                })
                .unwrap_or(url_rem.len());
            let mut raw_url = &url_rem[..end_offset];
            while raw_url.ends_with(['.', ',', ';', ':', '!', '?']) {
                raw_url = &raw_url[..raw_url.len() - 1];
            }
            let start_char_idx = text[..abs_start].chars().count();
            let end_char_idx = start_char_idx + raw_url.chars().count();
            if col >= start_char_idx && col <= end_char_idx {
                return Some(raw_url.to_string());
            }
            search_from = abs_start + end_offset.max(1);
        }

        None
    }
    pub(crate) fn push_undo(&mut self) {
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

    pub(crate) fn undo(&mut self) {
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

    pub(crate) fn redo(&mut self) {
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

    pub(crate) fn ensure_cursor_visible(&mut self, visible_lines: usize) {
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

    pub(crate) fn insert_char(&mut self, ch: char) {
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

    pub(crate) fn insert_text(&mut self, text: &str) {
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

    pub(crate) fn insert_newline(&mut self) {
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

    pub(crate) fn insert_tab(&mut self) {
        self.insert_text("    ");
    }

    pub(crate) fn backspace(&mut self) {
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

    pub(crate) fn delete(&mut self) {
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
            let g_len = line[start_byte..]
                .graphemes(true)
                .next()
                .map_or(0, str::len);
            line.drain(start_byte..start_byte + g_len);
            self.is_dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            // Unir la línea siguiente a la actual
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.is_dirty = true;
        }
    }

    /// Borrado de palabra hacia la izquierda (Ctrl + Backspace)
    pub(crate) fn delete_word_left(&mut self) {
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
    pub(crate) fn delete_word_right(&mut self) {
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

    pub(crate) fn find_word_boundary_left(&self) -> usize {
        if self.cursor_col == 0 || self.cursor_line >= self.lines.len() {
            return 0;
        }
        let graphemes: Vec<&str> = self.lines[self.cursor_line].graphemes(true).collect();
        let mut idx = self.cursor_col.min(graphemes.len());

        let is_ws = |g: &str| g.chars().all(char::is_whitespace);
        let is_word = |g: &str| g.chars().all(|c| c.is_alphanumeric() || c == '_');

        // Saltar espacios a la izquierda
        while idx > 0 && is_ws(graphemes[idx - 1]) {
            idx -= 1;
        }
        if idx == 0 {
            return 0;
        }

        // Si es caracter alfanumérico o guion bajo, buscar el inicio de la palabra
        let is_alphanumeric = is_word(graphemes[idx - 1]);
        if is_alphanumeric {
            while idx > 0 && is_word(graphemes[idx - 1]) {
                idx -= 1;
            }
        } else {
            // Símbolos puntuación
            while idx > 0 && !is_word(graphemes[idx - 1]) && !is_ws(graphemes[idx - 1]) {
                idx -= 1;
            }
        }
        idx
    }

    pub(crate) fn find_word_boundary_right(&self) -> usize {
        if self.cursor_line >= self.lines.len() {
            return 0;
        }
        let graphemes: Vec<&str> = self.lines[self.cursor_line].graphemes(true).collect();
        let len = graphemes.len();
        if self.cursor_col >= len {
            return len;
        }
        let mut idx = self.cursor_col;

        let is_ws = |g: &str| g.chars().all(char::is_whitespace);
        let is_word = |g: &str| g.chars().all(|c| c.is_alphanumeric() || c == '_');

        let is_alphanumeric = is_word(graphemes[idx]);
        if is_alphanumeric {
            while idx < len && is_word(graphemes[idx]) {
                idx += 1;
            }
        } else if !is_ws(graphemes[idx]) {
            while idx < len && !is_word(graphemes[idx]) && !is_ws(graphemes[idx]) {
                idx += 1;
            }
        }
        // Saltar espacios posteriores
        while idx < len && is_ws(graphemes[idx]) {
            idx += 1;
        }
        idx
    }

    pub(crate) fn is_diff(&self) -> bool {
        self.diff_stash.is_some()
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
            return true;
        };
        if let Some(st) = &self.diff_stash {
            return base != &st.lines;
        }
        self.ensure_git_marks();
        self.git_marks.borrow().as_ref().is_some_and(|c| c.1)
    }

    /// Alterna entre el archivo y su diff unificado contra `HEAD` (la vista de cambios es de solo lectura)
    pub(crate) fn toggle_diff(&mut self) {
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

    /// Programa el autoguardado para ejecutarse con retardo (antirrebote) tras dejar de teclear
    pub(crate) fn schedule_autosave(&mut self) {
        if self.remote_destination.is_none() {
            self.needs_autosave = true;
            self.last_edit_time = Instant::now();
        }
    }

    /// Autoguardado tras editar: solo archivos locales, sin recortar saltos en curso ni borrar el historial de deshacer
    pub(crate) fn autosave_local(&mut self) {
        if self.remote_destination.is_none() {
            self.needs_autosave = false;
            let _ = self.save_to_disk_inner(false);
        }
    }

    /// Recorta las líneas vacías finales dejando como máximo una (estándar de los IDE) y mantiene el cursor dentro de rango
    pub(crate) fn trim_trailing_blank_lines(&mut self) {
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

    /// Contenido de un archivo remoto tal como se escribe al guardar (con el fin de línea y el salto final
    /// que tenía al abrirlo, igual que el guardado local)
    pub(crate) fn remote_content(&mut self) -> String {
        self.trim_trailing_blank_lines();
        self.text_for_disk()
    }

    /// Texto del visor con el fin de línea y el salto final originales del archivo
    fn text_for_disk(&self) -> String {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        let mut content = self.lines.join(eol);
        if self.final_newline && !content.is_empty() && !content.ends_with(eol) {
            content.push_str(eol);
        }
        content
    }

    /// Fecha de modificación actual en disco, si el archivo existe
    fn disk_mtime_now(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok()
    }

    /// Fecha en disco distinta de la que vio el visor al abrir o guardar: otro programa (un agente, git…) escribió
    /// el archivo y guardar ahora pisaría esos cambios
    fn disk_changed_externally(&self) -> Option<std::time::SystemTime> {
        let now = self.disk_mtime_now()?;
        (self.disk_mtime? != now).then_some(now)
    }

    /// El servidor confirmó la escritura: el texto actual pasa a ser el original
    pub(crate) fn mark_remote_saved(&mut self) {
        self.is_dirty = false;
        self.file_size = self.lines.iter().map(|l| l.len() + 1).sum::<usize>() as u64;
        self.original_lines = self.lines.clone();
        self.edit_gen += 1;
    }

    /// Escribe el contenido en cualquier flujo de salida genérico (std::io::Write)
    pub(crate) fn write_to(&self, mut writer: impl std::io::Write) -> Result<(), std::io::Error> {
        let content = self.text_for_disk();
        writer.write_all(content.as_bytes())
    }

    /// Escribe en `<archivo>.tearminal-tmp` y renombra encima: un fallo (disco lleno…) deja el original intacto
    fn write_local(&self) -> Result<(), std::io::Error> {
        // Un enlace simbólico se conserva: se reemplaza su destino, no el enlace
        let target = if std::fs::symlink_metadata(&self.path).is_ok_and(|m| m.is_symlink()) {
            std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone())
        } else {
            self.path.clone()
        };
        let mut tmp = target.clone().into_os_string();
        tmp.push(".tearminal-tmp");
        let tmp = std::path::PathBuf::from(tmp);
        // Un temporal viejo (o un enlace plantado) se borra y se crea de nuevo con `create_new`: nunca se escribe a través de él
        let _ = std::fs::remove_file(&tmp);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .and_then(|f| {
                use std::io::Write;
                let mut writer = std::io::BufWriter::new(f);
                self.write_to(&mut writer)?;
                writer.flush()
            })
            .and_then(|()| std::fs::rename(&tmp, &target))
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&tmp);
            })
    }

    pub(crate) fn save_to_disk(&mut self) -> Result<(), ViewerSaveError> {
        self.needs_autosave = false;
        self.save_to_disk_inner(true)
    }

    fn save_to_disk_inner(&mut self, is_explicit_save: bool) -> Result<(), ViewerSaveError> {
        // En la vista de cambios `lines` es el diff, nunca debe llegar a disco
        if self.is_binary || self.non_utf8 || self.is_diff() {
            return Ok(());
        }

        if is_explicit_save {
            self.trim_trailing_blank_lines();
        }

        if self.remote_destination.is_some() {
            // El guardado remoto es asíncrono (`TearminalApp::ssh_save_viewer`): nunca bloquea aquí
            return Err(ViewerSaveError::RemoteFile);
        }

        // Otro programa escribió el archivo desde que se abrió: el autoguardado se detiene y un guardado
        // explícito avisa y exige repetirlo para sobrescribir
        if let Some(now) = self.disk_changed_externally() {
            if is_explicit_save {
                self.disk_mtime = Some(now);
            }
            let msg = if is_explicit_save {
                "El archivo cambió en disco: guarda de nuevo para sobrescribir"
            } else {
                "El archivo cambió en disco: autoguardado detenido (guarda con Ctrl+S para sobrescribir)"
            };
            self.save_error = Some(msg.to_string());
            return Err(ViewerSaveError::ExternalModification(msg));
        }
        if let Err(e) = self.write_local() {
            self.save_error = Some(e.to_string());
            return Err(ViewerSaveError::Io(e));
        }
        self.save_error = None;
        self.disk_mtime = self.disk_mtime_now();
        self.file_size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if is_explicit_save {
            self.is_dirty = false;
            self.original_lines = self.lines.clone();
        }
        self.edit_gen += 1;
        Ok(())
    }
}

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

/// Estado del visor mientras se muestra el diff (se restaura al volver al archivo)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffStash {
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

type GitMarks = Rc<[Option<GitLineStatus>]>;

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
