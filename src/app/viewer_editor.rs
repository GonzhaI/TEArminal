//! Edición de texto, historial deshacer/rehacer, búsqueda interna y guardado en disco del visor de archivos.

use super::viewer::{char_byte_idx, char_count, FileViewerState, ViewerSnapshot};
use super::viewer_diff::ViewerSaveError;
use super::TearminalApp;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

impl FileViewerState {
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
            self.edited_line = Some(l1);
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
            self.edited_line = None;
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
            self.edited_line = None;
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
            self.edited_line = None;
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

    /// Abre la barra de búsqueda flotante (`Ctrl + F`). Si hay texto seleccionado en una sola línea,
    /// lo usa como consulta inicial y lo selecciona.
    pub(crate) fn open_search(&mut self, visible_lines: usize) {
        if self.goto_mode {
            self.goto_mode = false;
            self.search_input = crate::ui::TextInput::new("");
        }
        self.search_open = true;
        if let Some(text) = self.selected_text() {
            let first = text.lines().next().unwrap_or("").trim();
            if !first.is_empty() {
                self.search_input = crate::ui::TextInput::new(first);
                self.search_input.select_all();
            }
        } else if !self.search_input.text().is_empty() {
            self.search_input.select_all();
        }
        self.update_search_matches();
        if self.search_active_idx.is_some() {
            self.jump_to_active_match(visible_lines);
        }
    }

    /// Cierra la búsqueda flotante y limpia las coincidencias activas.
    pub(crate) fn close_search(&mut self) {
        self.search_open = false;
        self.goto_mode = false;
        self.search_matches.clear();
        self.search_active_idx = None;
    }

    /// Abre la barra en modo «Ir a línea» (`Ctrl + G`), vacía.
    pub(crate) fn open_goto(&mut self) {
        self.search_matches.clear();
        self.search_active_idx = None;
        self.search_input = crate::ui::TextInput::new("");
        self.search_open = true;
        self.goto_mode = true;
    }

    /// Salta a `línea[:columna]` (base 1; fuera de rango se ajusta al final) y cierra la barra.
    /// Con una entrada que no es un número la deja abierta y devuelve `false`.
    pub(crate) fn goto_line(&mut self, visible_lines: usize) -> bool {
        let text = self.search_input.text().trim().to_string();
        let (line_s, col_s) = text.split_once(':').unwrap_or((text.as_str(), ""));
        let Ok(line) = line_s.trim().parse::<usize>() else {
            return false;
        };
        let col = col_s.trim().parse::<usize>().unwrap_or(1);
        let line_idx = line
            .saturating_sub(1)
            .min(self.lines.len().saturating_sub(1));
        let max_col = self.lines.get(line_idx).map_or(0, |l| {
            unicode_segmentation::UnicodeSegmentation::graphemes(l.as_str(), true).count()
        });
        self.cursor_line = line_idx;
        self.cursor_col = col.saturating_sub(1).min(max_col);
        self.clear_selection();
        self.reveal_line(line_idx);
        self.ensure_cursor_visible(visible_lines.max(1));
        self.close_search();
        true
    }

    /// Recalcula todas las coincidencias de búsqueda sobre el contenido actual.
    pub(crate) fn update_search_matches(&mut self) {
        self.search_matches.clear();
        let query = self.search_input.text().trim();
        if query.is_empty() {
            self.search_active_idx = None;
            return;
        }

        let is_ascii = query.is_ascii();
        let q_lower = query.to_lowercase();
        let q_chars: Vec<char> = q_lower.chars().collect();

        for (line_idx, line) in self.lines.iter().enumerate() {
            if is_ascii && line.is_ascii() {
                let line_lower = line.to_ascii_lowercase();
                let q_str = q_lower.as_str();
                let q_len = q_str.len();
                let mut offset = 0;
                while let Some(pos) = line_lower[offset..].find(q_str) {
                    let start = offset + pos;
                    self.search_matches.push((line_idx, start, start + q_len));
                    offset = start + q_len.max(1);
                }
            } else {
                let graphemes: Vec<&str> = line.graphemes(true).collect();
                let g_lowers: Vec<String> = graphemes.iter().map(|g| g.to_lowercase()).collect();
                let total_g = graphemes.len();
                let mut i = 0;
                while i < total_g {
                    let mut matched_chars = 0;
                    let mut j = i;
                    let mut is_match = false;
                    while j < total_g {
                        let g_str = &g_lowers[j];
                        let mut all_matched = true;
                        for c in g_str.chars() {
                            if matched_chars < q_chars.len() && q_chars[matched_chars] == c {
                                matched_chars += 1;
                            } else {
                                all_matched = false;
                                break;
                            }
                        }
                        if !all_matched {
                            break;
                        }
                        if matched_chars == q_chars.len() {
                            is_match = true;
                            j += 1;
                            break;
                        }
                        j += 1;
                    }
                    if is_match {
                        self.search_matches.push((line_idx, i, j));
                        i = j;
                    } else {
                        i += 1;
                    }
                }
            }
        }

        if self.search_matches.is_empty() {
            self.search_active_idx = None;
        } else {
            let cur = (self.cursor_line, self.cursor_col);
            let nearest = self
                .search_matches
                .iter()
                .position(|&(l, c, _)| (l, c) >= cur)
                .unwrap_or(0);
            self.search_active_idx = Some(nearest);
        }
    }

    /// Salta a la siguiente coincidencia cíclica.
    pub(crate) fn search_next(&mut self, visible_lines: usize) {
        if self.search_matches.is_empty() {
            return;
        }
        let next = match self.search_active_idx {
            Some(idx) => (idx + 1) % self.search_matches.len(),
            None => 0,
        };
        self.search_active_idx = Some(next);
        self.jump_to_active_match(visible_lines);
    }

    /// Salta a la coincidencia anterior cíclica.
    pub(crate) fn search_prev(&mut self, visible_lines: usize) {
        if self.search_matches.is_empty() {
            return;
        }
        let prev = match self.search_active_idx {
            Some(0) | None => self.search_matches.len().saturating_sub(1),
            Some(idx) => idx - 1,
        };
        self.search_active_idx = Some(prev);
        self.jump_to_active_match(visible_lines);
    }

    /// Revela la línea y posiciona el cursor en la coincidencia activa.
    pub(crate) fn jump_to_active_match(&mut self, visible_lines: usize) {
        if let Some(idx) = self.search_active_idx {
            if let Some(&(line, start_col, _)) = self.search_matches.get(idx) {
                self.cursor_line = line;
                self.cursor_col = start_col;
                self.reveal_line(line);
                self.ensure_cursor_visible(visible_lines.max(1));
            }
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
        self.edited_line = Some(self.cursor_line);
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
            self.edited_line = Some(self.cursor_line);
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
            self.edited_line = None;
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
        self.edited_line = None;
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
                self.edited_line = Some(self.cursor_line);
            }
        } else if self.cursor_line > 0 {
            // Unir la línea actual con la anterior
            let cur_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = char_count(&self.lines[self.cursor_line]);
            self.lines[self.cursor_line].push_str(&cur_line);
            self.cursor_col = prev_len;
            self.is_dirty = true;
            self.edited_line = None;
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
            self.edited_line = Some(self.cursor_line);
        } else if self.cursor_line + 1 < self.lines.len() {
            // Unir la línea siguiente a la actual
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.is_dirty = true;
            self.edited_line = None;
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
        self.edited_line = Some(self.cursor_line);
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
        self.edited_line = Some(self.cursor_line);
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

impl TearminalApp {
    /// Filas de texto visibles en el visor de la pestaña activa
    pub(crate) fn viewer_visible_lines(&self) -> usize {
        let win_h = self
            .window
            .as_ref()
            .map_or(600, |w| w.inner_size().height as usize);
        let row_h = self.renderer.as_ref().map_or(20, |r| r.row_height);
        (win_h.saturating_sub(self.top_offset() + self.status_bar_height() + 38) / row_h.max(16))
            .max(1)
    }

    /// Ctrl+S / «Guardar»: escribe el archivo del visor activo y refresca el estado git del explorador
    pub(crate) fn save_active_viewer(&mut self) {
        // Archivo remoto: vía rápida en banda por el propio panel SSH; si no está en el prompt, respaldo por SshConnection
        let remote = self.tabs[self.active_tab_index]
            .file_viewer
            .as_ref()
            .and_then(|v| Some((v.remote_destination.clone()?, v.path.clone(), v.is_dirty)));
        if let Some((dest, path, dirty)) = remote {
            if dirty && !self.remote_save_active_viewer(&dest, &path) {
                self.ssh_save_viewer(&dest, &path);
            }
            return;
        }
        let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_mut() else {
            return;
        };
        match viewer.save_to_disk() {
            Ok(()) => {
                crate::logger::log(&format!("-> Archivo guardado: {}", viewer.file_name));
                if self.file_explorer.visible {
                    self.file_explorer.refresh();
                    self.pump_explorer_ssh();
                }
            }
            Err(e) => crate::logger::log(&format!("-> Error al guardar archivo: {}", e)),
        }
    }
}
