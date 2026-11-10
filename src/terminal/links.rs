//! Extracción de texto, selección en rejilla, hipervínculos OSC 8, URLs en texto plano y candidatos de rutas de archivo.

use super::*;

/// Candidato a ruta de archivo interactiva extraída de la rejilla de la terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilePathCandidate {
    pub raw_path: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub start_col: usize,
    pub end_col: usize,
}

impl TerminalState {
    pub(crate) fn register_hyperlink(&mut self, url: &str) -> u16 {
        // Rechazar URLs vacías, excesivamente largas (> 2048) o con caracteres de control
        if url.is_empty()
            || url.len() > 2048
            || url.chars().any(|c| (c as u32) < 0x20 || c == '\x7f')
        {
            return 0;
        }

        if let Some(&id) = self.hyperlink_map.get(url) {
            return id;
        }
        if self.hyperlinks.len() < (Cell::HYPERLINK_MASK as usize) {
            let id = (self.hyperlinks.len() + 1) as u16;
            let boxed: Box<str> = url.into();
            self.hyperlink_map.insert(boxed.clone(), id);
            self.hyperlinks.push(boxed);
            id
        } else {
            0
        }
    }

    pub(crate) fn hyperlink_of(&self, cell: &Cell) -> Option<&str> {
        let h_id = cell.raw_hyperlink_id();
        if h_id == 0 {
            None
        } else {
            let idx = (h_id as usize) - 1;
            self.hyperlinks.get(idx).map(|s| &**s)
        }
    }

    pub(crate) fn append_cell_text(&self, cell: &Cell, out: &mut String) {
        if cell.is_grapheme() {
            if let Some(s) = self.grapheme_of(cell) {
                out.push_str(s);
                return;
            }
        }
        out.push(cell.c);
    }

    pub(crate) fn append_row_text(&self, row: &[Cell], buf: &mut String) {
        for cell in row.iter().filter(|c| c.width > 0) {
            self.append_cell_text(cell, buf);
        }
    }

    pub(crate) fn row_text_to_string(&self, row: &[Cell]) -> String {
        let mut s = String::with_capacity(row.len());
        self.append_row_text(row, &mut s);
        s
    }

    pub(crate) fn get_last_output_text(&self) -> Option<String> {
        let block = self.command_blocks.last()?;
        let start = block.output_start?;
        let end = block
            .output_end
            .unwrap_or_else(|| self.current_global_line());
        if start > end {
            return None;
        }

        let line_count = (end + 1).saturating_sub(start);
        let mut lines = Vec::with_capacity(line_count);
        let mut line_buf = String::with_capacity(self.cols.max(80));
        for line_idx in start..=end {
            if let Some(row) = self.get_buffer_line(line_idx) {
                line_buf.clear();
                self.append_row_text(row, &mut line_buf);
                let trimmed = line_buf.trim_end();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
        }

        let text = lines.join("\n").trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub(crate) fn selection_ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let sel = self.selection?;
        let (mut p1, mut p2) = (sel.start, sel.end);
        if p1.1 > p2.1 || (p1.1 == p2.1 && p1.0 > p2.0) {
            std::mem::swap(&mut p1, &mut p2);
        }
        if p1 == p2 {
            None
        } else {
            Some((p1, p2))
        }
    }

    #[inline]
    pub(crate) fn has_selection(&self) -> bool {
        self.selection_ordered().is_some()
    }

    // `y` se usa como número de línea global en el buffer
    #[allow(clippy::needless_range_loop)]
    pub(crate) fn copy_selection_text(&self) -> Option<String> {
        let (p1, p2) = self.selection_ordered()?;

        let total_lines = self.total_buffer_lines();
        if total_lines == 0 {
            return None;
        }

        let max_line = total_lines.saturating_sub(1);
        let end_line = p2.1.min(max_line);
        let num_lines = (end_line + 1).saturating_sub(p1.1);
        let mut result = String::with_capacity(num_lines * (self.cols.max(80) + 1));
        let mut line_buf = String::with_capacity(self.cols.max(80));

        for y in p1.1..=end_line {
            let Some(row) = self.get_buffer_line(y) else {
                continue;
            };
            let line_used = row
                .iter()
                .rposition(|cell| {
                    !(cell.c == ' '
                        && cell.bg == Color::TRANSPARENT
                        && (cell.flags
                            & (Cell::FLAG_UNDERLINE
                                | Cell::FLAG_STRIKETHROUGH
                                | Cell::FLAG_INVERSE))
                            == 0)
                })
                .map_or(0, |i| i + 1);

            let x_start = if y == p1.1 { p1.0 } else { 0 };
            let x_end = if y == p2.1 {
                p2.0.min(row.len().saturating_sub(1))
            } else {
                line_used.saturating_sub(1)
            };

            line_buf.clear();
            if x_start <= x_end && x_start < row.len() {
                for x in x_start..=x_end {
                    if x < row.len() && row[x].width > 0 {
                        self.append_cell_text(&row[x], &mut line_buf);
                    }
                }
            }
            result.push_str(line_buf.trim_end());
            if y < p2.1 {
                result.push('\n');
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Extrae la URL y el rango de columnas `(url, start_col, end_col)` de un hipervínculo
    /// (explícito OSC 8 o reconocido por patrón de texto plano) bajo (col, global_line).
    pub(crate) fn url_and_span_at(
        &self,
        col: usize,
        global_line: usize,
    ) -> Option<(String, usize, usize)> {
        let row = self.get_buffer_line(global_line)?;
        if row.is_empty() || col >= row.len() {
            return None;
        }

        // 1. Enlace explícito OSC 8 en la celda
        let link_id = row[col].raw_hyperlink_id();
        if link_id > 0 {
            if let Some(url) = self.hyperlink_of(&row[col]) {
                let mut start_col = col;
                while start_col > 0 && row[start_col - 1].raw_hyperlink_id() == link_id {
                    start_col -= 1;
                }
                let mut end_col = col;
                while end_col + 1 < row.len() && row[end_col + 1].raw_hyperlink_id() == link_id {
                    end_col += 1;
                }
                return Some((url.to_string(), start_col, end_col));
            }
        }

        // 2. Reconocimiento de URLs en texto plano (http://, https://, localhost:PORT, 127.0.0.1:PORT)
        Self::find_plain_url_in_row(row, col)
    }

    fn find_plain_url_in_row(row: &[Cell], col: usize) -> Option<(String, usize, usize)> {
        let mut chars = Vec::with_capacity(row.len());
        let mut char_to_col = Vec::with_capacity(row.len());

        for (c_idx, cell) in row.iter().enumerate() {
            if cell.width == 0 {
                continue;
            }
            chars.push(cell.c);
            char_to_col.push(c_idx);
        }

        if chars.is_empty() {
            return None;
        }

        let n = chars.len();
        let mut i = 0;
        while i < n {
            // Un enlace solo puede empezar si es el inicio o el carácter anterior no es alfanumérico ni delimitador interno
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_alphanumeric() || prev == '/' || prev == '.' || prev == '@' {
                    i += 1;
                    continue;
                }
            }

            let mut prefix_len = 0;
            let mut is_local = false;

            if Self::chars_match(&chars[i..], "https://") {
                prefix_len = 8;
            } else if Self::chars_match(&chars[i..], "http://") {
                prefix_len = 7;
            } else if Self::chars_match(&chars[i..], "localhost:") {
                let rest = &chars[i + 10..];
                if rest.first().is_some_and(|c| c.is_ascii_digit()) {
                    prefix_len = 10;
                    is_local = true;
                }
            } else if Self::chars_match(&chars[i..], "127.0.0.1:") {
                let rest = &chars[i + 10..];
                if rest.first().is_some_and(|c| c.is_ascii_digit()) {
                    prefix_len = 10;
                    is_local = true;
                }
            }

            if prefix_len == 0 {
                i += 1;
                continue;
            }

            // Extender hasta el final de la URL
            let mut j = i + prefix_len;
            while j < n {
                let c = chars[j];
                if c.is_whitespace()
                    || (c as u32) <= 32
                    || c == '"'
                    || c == '\''
                    || c == '`'
                    || c == '<'
                    || c == '>'
                {
                    break;
                }
                j += 1;
            }

            // Recortar signos de puntuación finales si no están balanceados
            while j > i + prefix_len {
                let last = chars[j - 1];
                if last == '.'
                    || last == ','
                    || last == ';'
                    || last == ':'
                    || last == '!'
                    || last == '?'
                {
                    j -= 1;
                } else if last == ')' {
                    let open_count = chars[i..j].iter().filter(|&&c| c == '(').count();
                    let close_count = chars[i..j].iter().filter(|&&c| c == ')').count();
                    if close_count > open_count {
                        j -= 1;
                    } else {
                        break;
                    }
                } else if last == ']' {
                    let open_count = chars[i..j].iter().filter(|&&c| c == '[').count();
                    let close_count = chars[i..j].iter().filter(|&&c| c == ']').count();
                    if close_count > open_count {
                        j -= 1;
                    } else {
                        break;
                    }
                } else if last == '}' {
                    let open_count = chars[i..j].iter().filter(|&&c| c == '{').count();
                    let close_count = chars[i..j].iter().filter(|&&c| c == '}').count();
                    if close_count > open_count {
                        j -= 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            if j > i + prefix_len {
                let start_col = char_to_col[i];
                let end_col = char_to_col[j - 1];
                if col >= start_col && col <= end_col {
                    let matched_str: String = chars[i..j].iter().collect();
                    let url = if is_local {
                        format!("http://{matched_str}")
                    } else {
                        matched_str
                    };
                    return Some((url, start_col, end_col));
                }
                i = j;
            } else {
                i += prefix_len;
            }
        }

        None
    }

    #[inline]
    fn chars_match(slice: &[char], prefix: &str) -> bool {
        if slice.len() < prefix.len() {
            return false;
        }
        prefix
            .chars()
            .zip(slice)
            .all(|(p, &c)| p.eq_ignore_ascii_case(&c))
    }

    /// Extrae la URL del hipervínculo (OSC 8 o regex de texto plano) bajo (col, global_line), si existe.
    pub(crate) fn url_at(&self, col: usize, global_line: usize) -> Option<String> {
        self.url_and_span_at(col, global_line).map(|(s, _, _)| s)
    }

    /// Extrae un candidato a ruta de archivo con posible sufijo de línea/columna bajo (col, global_line).
    pub(crate) fn file_path_candidate_at(
        &self,
        col: usize,
        global_line: usize,
    ) -> Option<FilePathCandidate> {
        let row = self.get_buffer_line(global_line)?;
        if row.is_empty() || col >= row.len() {
            return None;
        }
        Self::find_file_path_in_row(row, col)
    }

    fn find_file_path_in_row(row: &[Cell], target_col: usize) -> Option<FilePathCandidate> {
        let mut chars = Vec::with_capacity(row.len());
        let mut char_to_col = Vec::with_capacity(row.len());

        for (c_idx, cell) in row.iter().enumerate() {
            if cell.width == 0 {
                continue;
            }
            chars.push(cell.c);
            char_to_col.push(c_idx);
        }

        if chars.is_empty() {
            return None;
        }

        let target_char_idx = match char_to_col.binary_search(&target_col) {
            Ok(idx) => idx,
            Err(idx) if idx > 0 && idx <= chars.len() => idx - 1,
            _ => return None,
        };

        let c = chars[target_char_idx];
        if c.is_whitespace() || (c as u32) <= 32 {
            return None;
        }

        let is_quote = |ch: char| matches!(ch, '"' | '\'' | '`');
        let is_path_boundary = |ch: char| -> bool {
            ch.is_whitespace()
                || (ch as u32) <= 32
                || matches!(ch, '"' | '\'' | '`' | '<' | '>' | '|' | '\0')
        };

        // Comprobar si target_col está dentro de un rango delimitado por comillas
        let mut quoted_range = None;
        if let Some(left_quote) = chars[..target_char_idx]
            .iter()
            .rposition(|&ch| is_quote(ch))
        {
            let q = chars[left_quote];
            if let Some(right_offset) = chars[target_char_idx..].iter().position(|&ch| ch == q) {
                let right_quote = target_char_idx + right_offset;
                if left_quote + 1 < right_quote {
                    quoted_range = Some((left_quote + 1, right_quote - 1));
                }
            }
        }

        let (mut start_idx, mut end_idx) = if let Some(range) = quoted_range {
            range
        } else {
            let mut s = target_char_idx;
            while s > 0 && !is_path_boundary(chars[s - 1]) {
                s -= 1;
            }
            let mut e = target_char_idx;
            while e + 1 < chars.len() && !is_path_boundary(chars[e + 1]) {
                e += 1;
            }
            (s, e)
        };

        // Recortar caracteres delimitadores externos comunes del inicio
        while start_idx < end_idx && matches!(chars[start_idx], '(' | '[' | '{' | ',' | ';') {
            start_idx += 1;
        }

        // Recortar puntuación final no deseada (comas, punto y coma, exclamación, interrogación, corchetes o punto final)
        while end_idx > start_idx
            && matches!(chars[end_idx], ',' | ';' | '!' | '?' | '.' | ']' | '}')
        {
            end_idx -= 1;
        }
        // Si termina con ':' recortarlo si no le siguen dígitos
        while end_idx > start_idx && chars[end_idx] == ':' {
            end_idx -= 1;
        }

        if start_idx > end_idx {
            return None;
        }

        let token: String = chars[start_idx..=end_idx].iter().collect();
        let (raw_path_slice, line, col) = Self::parse_line_col_suffix(&token);
        let raw_path = raw_path_slice.trim().to_string();

        if !Self::is_plausible_file_path(&raw_path) {
            return None;
        }

        let start_col = char_to_col[start_idx];
        let end_col = char_to_col[end_idx];

        Some(FilePathCandidate {
            raw_path,
            line,
            col,
            start_col,
            end_col,
        })
    }

    fn parse_line_col_suffix(s: &str) -> (&str, Option<usize>, Option<usize>) {
        if s.ends_with(')') {
            if let Some(open_paren) = s.rfind('(') {
                let inner = &s[open_paren + 1..s.len() - 1];
                let mut parts = inner.split(',');
                if let Some(line_str) = parts.next() {
                    if let Ok(line) = line_str.trim().parse::<usize>() {
                        let col = parts.next().and_then(|c| c.trim().parse::<usize>().ok());
                        let path = &s[..open_paren];
                        return (path, Some(line), col);
                    }
                }
            }
        }

        if let Some(last_colon) = s.rfind(':') {
            if last_colon > 1 {
                let after_last = &s[last_colon + 1..];
                if !after_last.is_empty() && after_last.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(num2) = after_last.parse::<usize>() {
                        let before_last = &s[..last_colon];
                        if let Some(second_colon) = before_last.rfind(':') {
                            if second_colon > 1 {
                                let middle = &before_last[second_colon + 1..];
                                if !middle.is_empty() && middle.chars().all(|c| c.is_ascii_digit())
                                {
                                    if let Ok(num1) = middle.parse::<usize>() {
                                        let path = &before_last[..second_colon];
                                        return (path, Some(num1), Some(num2));
                                    }
                                }
                            }
                        }
                        return (before_last, Some(num2), None);
                    }
                }
            }
        }

        (s, None, None)
    }

    fn is_plausible_file_path(path: &str) -> bool {
        if path.is_empty() || path.len() > 1024 {
            return false;
        }
        if path.starts_with("http://") || path.starts_with("https://") {
            return false;
        }
        let has_sep = path.contains('/') || path.contains('\\');
        let starts_dot = path.starts_with('.');
        let has_drive = path.len() >= 3
            && path.as_bytes()[0].is_ascii_alphabetic()
            && path.as_bytes()[1] == b':'
            && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/');
        let has_ext = if let Some(dot) = path.rfind('.') {
            let ext = &path[dot + 1..];
            !ext.is_empty() && ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric())
        } else {
            false
        };
        let is_special_name = matches!(
            path.to_ascii_lowercase().as_str(),
            "makefile" | "dockerfile" | "license"
        );

        has_sep || starts_dot || has_drive || has_ext || is_special_name
    }

    /// Selecciona la palabra o bloque delimitado bajo la columna `col` en `global_line`.
    pub(crate) fn select_word_at(&mut self, col: usize, global_line: usize) {
        let Some(row) = self.get_buffer_line(global_line) else {
            self.selection = None;
            return;
        };
        if row.is_empty() {
            self.selection = None;
            return;
        }
        let col = col.min(row.len().saturating_sub(1));
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
        let is_ws_char = |c: char| c == ' ' || c == '\t';

        let target_char = row[col].c;
        let target_is_word = is_word_char(target_char);
        let target_is_ws = is_ws_char(target_char);

        let mut start_col = col;
        while start_col > 0 {
            let prev_c = row[start_col - 1].c;
            if target_is_ws {
                if !is_ws_char(prev_c) {
                    break;
                }
            } else if target_is_word {
                if !is_word_char(prev_c) {
                    break;
                }
            } else if is_word_char(prev_c) || is_ws_char(prev_c) {
                break;
            }
            start_col -= 1;
        }

        let mut end_col = col;
        while end_col + 1 < row.len() {
            let next_c = row[end_col + 1].c;
            if target_is_ws {
                if !is_ws_char(next_c) {
                    break;
                }
            } else if target_is_word {
                if !is_word_char(next_c) {
                    break;
                }
            } else if is_word_char(next_c) || is_ws_char(next_c) {
                break;
            }
            end_col += 1;
        }

        self.selection = Some(SelectionRange {
            start: (start_col, global_line),
            end: (end_col, global_line),
        });
    }

    /// Selecciona toda la línea visible ocupada en `global_line`.
    pub(crate) fn select_line_at(&mut self, global_line: usize) {
        let Some(row) = self.get_buffer_line(global_line) else {
            self.selection = None;
            return;
        };
        if row.is_empty() {
            self.selection = None;
            return;
        }
        let line_used = row
            .iter()
            .rposition(|cell| {
                !(cell.c == ' '
                    && cell.bg == Color::TRANSPARENT
                    && (cell.flags
                        & (Cell::FLAG_UNDERLINE | Cell::FLAG_STRIKETHROUGH | Cell::FLAG_INVERSE))
                        == 0)
            })
            .map_or(0, |i| i + 1);

        let end_col = if line_used > 0 {
            line_used.saturating_sub(1)
        } else {
            0
        };

        self.selection = Some(SelectionRange {
            start: (0, global_line),
            end: (end_col, global_line),
        });
    }
}
