//! Componente de entrada de texto reutilizable (buscador de paleta, ajustes, etc.)
//!
//! Soporta cursor en cualquier posición, navegación por caracteres y palabras (Ctrl + Flechas),
//! selección con teclado (Shift) y ratón (clic, arrastre, doble clic para palabra, triple clic para todo),
//! eliminación de palabras completas (Ctrl+Backspace, Alt+Backspace, Ctrl+Delete), portapapeles (Copiar/Cortar/Pegar)
//! y dibujo preciso con cursor parpadeante y resaltado de selección.

use crate::app::key_is_char;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextInputKeyResult {
    /// No fue procesada por el input (tecla reservada o no relevante)
    Ignored,
    /// El cursor o selección se movió sin alterar el contenido (flechas, Home, End, Copiar)
    Navigated,
    /// El texto cambió (escritura, borrado, corte, pegado)
    Changed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TextInput {
    pub text: String,
    /// Posición del cursor en bytes (siempre alineado a límite de char UTF-8)
    pub cursor: usize,
    /// Ancla de selección en bytes. Si `Some(a)` y `a != cursor`, hay texto seleccionado.
    pub select_anchor: Option<usize>,
    /// Indica si se está arrastrando con el ratón para seleccionar
    pub is_dragging: bool,
    undo_stack: Vec<(String, usize)>,
    redo_stack: Vec<(String, usize)>,
}

impl TextInput {
    pub(crate) fn new(initial: impl Into<String>) -> Self {
        let text = initial.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            select_anchor: None,
            is_dragging: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Guarda una instantánea del estado actual en el historial de deshacer
    fn push_undo(&mut self) {
        if self.undo_stack.last().map(|(t, _)| t.as_str()) != Some(&self.text) {
            if self.undo_stack.len() >= 50 {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push((self.text.clone(), self.cursor));
        }
        self.redo_stack.clear();
    }

    /// Deshace la última edición de texto (Ctrl+Z)
    pub(crate) fn undo(&mut self) -> bool {
        if let Some((prev_text, prev_cursor)) = self.undo_stack.pop() {
            self.redo_stack.push((self.text.clone(), self.cursor));
            self.text = prev_text;
            self.cursor = prev_cursor.min(self.text.len());
            self.select_anchor = None;
            self.clamp_cursor();
            true
        } else {
            false
        }
    }

    /// Rehace la última edición deshecha (Ctrl+Y / Ctrl+Shift+Z)
    pub(crate) fn redo(&mut self) -> bool {
        if let Some((next_text, next_cursor)) = self.redo_stack.pop() {
            self.undo_stack.push((self.text.clone(), self.cursor));
            self.text = next_text;
            self.cursor = next_cursor.min(self.text.len());
            self.select_anchor = None;
            self.clamp_cursor();
            true
        } else {
            false
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn clear(&mut self) {
        self.push_undo();
        self.text.clear();
        self.cursor = 0;
        self.select_anchor = None;
        self.is_dragging = false;
    }

    pub(crate) fn set_text(&mut self, s: impl Into<String>) {
        self.push_undo();
        self.text = s.into();
        self.cursor = self.text.len();
        self.select_anchor = None;
        self.is_dragging = false;
    }

    /// Rango de selección normalizado `(inicio_bytes, fin_bytes)`, si existe
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        match self.select_anchor {
            Some(a) if a != self.cursor => Some((a.min(self.cursor), a.max(self.cursor))),
            _ => None,
        }
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    pub(crate) fn selected_text(&self) -> Option<&str> {
        self.selection().map(|(start, end)| &self.text[start..end])
    }

    pub(crate) fn clear_selection(&mut self) {
        self.select_anchor = None;
    }

    pub(crate) fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.select_anchor = Some(0);
            self.cursor = self.text.len();
        }
    }

    /// Borra el rango seleccionado si lo hay. Devuelve `true` si se borró algo.
    pub(crate) fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection() {
            self.push_undo();
            self.text.drain(start..end);
            self.cursor = start;
            self.select_anchor = None;
            return true;
        }
        false
    }

    /// Inserta texto en la posición del cursor (reemplazando la selección si existe)
    pub(crate) fn insert_text(&mut self, s: &str) {
        self.push_undo();
        self.delete_selection();
        self.clamp_cursor();
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.select_anchor = None;
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_text(s);
    }

    /// Backspace: borra la selección o el carácter anterior al cursor
    pub(crate) fn delete_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor > 0 {
            self.push_undo();
            let prev = self.prev_char_index(self.cursor);
            self.text.drain(prev..self.cursor);
            self.cursor = prev;
            return true;
        }
        false
    }

    /// Delete: borra la selección o el carácter siguiente al cursor
    pub(crate) fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor < self.text.len() {
            self.push_undo();
            let next = self.next_char_index(self.cursor);
            self.text.drain(self.cursor..next);
            self.cursor = self.cursor.min(self.text.len());
            return true;
        }
        false
    }

    /// Ctrl+Backspace / Alt+Backspace: borra la palabra anterior al cursor
    pub(crate) fn delete_word_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor > 0 {
            self.push_undo();
            let target = self.word_boundary_left(self.cursor);
            self.text.drain(target..self.cursor);
            self.cursor = target;
            return true;
        }
        false
    }

    /// Ctrl+Delete: borra la palabra siguiente al cursor
    pub(crate) fn delete_word_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor < self.text.len() {
            self.push_undo();
            let target = self.word_boundary_right(self.cursor);
            self.text.drain(self.cursor..target);
            return true;
        }
        false
    }

    /// Mueve el cursor a la izquierda (1 carácter o 1 palabra), opcionalmente extendiendo la selección
    pub(crate) fn move_left(&mut self, extend_selection: bool, by_word: bool) {
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
            self.cursor = if by_word {
                self.word_boundary_left(self.cursor)
            } else {
                self.prev_char_index(self.cursor)
            };
        } else if let Some((start, _)) = self.selection() {
            self.cursor = start;
            self.select_anchor = None;
        } else {
            self.cursor = if by_word {
                self.word_boundary_left(self.cursor)
            } else {
                self.prev_char_index(self.cursor)
            };
            self.select_anchor = None;
        }
    }

    /// Mueve el cursor a la derecha (1 carácter o 1 palabra), opcionalmente extendiendo la selección
    pub(crate) fn move_right(&mut self, extend_selection: bool, by_word: bool) {
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
            self.cursor = if by_word {
                self.word_boundary_right(self.cursor)
            } else {
                self.next_char_index(self.cursor)
            };
        } else if let Some((_, end)) = self.selection() {
            self.cursor = end;
            self.select_anchor = None;
        } else {
            self.cursor = if by_word {
                self.word_boundary_right(self.cursor)
            } else {
                self.next_char_index(self.cursor)
            };
            self.select_anchor = None;
        }
    }

    pub(crate) fn move_home(&mut self, extend_selection: bool) {
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = 0;
    }

    pub(crate) fn move_end(&mut self, extend_selection: bool) {
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = self.text.len();
    }

    /// Localiza el carácter más cercano a una coordenada horizontal `click_x`
    pub(crate) fn hit_test(
        &self,
        click_x: usize,
        text_start_x: usize,
        measure: impl Fn(&str) -> usize,
    ) -> usize {
        if click_x <= text_start_x {
            return 0;
        }
        let mut prev_idx = 0;
        let mut prev_w = 0;
        for (idx, _) in self.text.char_indices() {
            if idx == 0 {
                continue;
            }
            let w = measure(&self.text[..idx]);
            let mid = text_start_x + prev_w + (w.saturating_sub(prev_w)) / 2;
            if click_x < mid {
                return prev_idx;
            }
            prev_idx = idx;
            prev_w = w;
        }
        let total_w = measure(&self.text);
        let mid = text_start_x + prev_w + (total_w.saturating_sub(prev_w)) / 2;
        if click_x < mid {
            prev_idx
        } else {
            self.text.len()
        }
    }

    pub(crate) fn mouse_down(&mut self, char_idx: usize, extend_selection: bool) {
        let target = char_idx.min(self.text.len());
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = Some(target);
        }
        self.cursor = target;
        self.clamp_cursor();
        self.is_dragging = true;
    }

    pub(crate) fn mouse_drag(&mut self, char_idx: usize) {
        if self.is_dragging {
            self.cursor = char_idx.min(self.text.len());
            self.clamp_cursor();
        }
    }

    pub(crate) fn mouse_up(&mut self) {
        self.is_dragging = false;
        if self.select_anchor == Some(self.cursor) {
            self.select_anchor = None;
        }
    }

    pub(crate) fn select_word_around(&mut self, char_idx: usize) {
        let (start, end) = self.word_range_around(char_idx);
        self.select_anchor = Some(start);
        self.cursor = end;
        self.is_dragging = false;
    }

    /// Procesa una pulsación de tecla sobre el input (navegación, borrado, escritura, selección y portapapeles).
    /// Devuelve `(resultado, texto_a_copiar_si_aplica)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_key(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: Option<&str>,
        ctrl: bool,
        shift: bool,
        alt: bool,
        clipboard_paste: Option<&str>,
    ) -> (TextInputKeyResult, Option<String>) {
        if ctrl && !alt {
            if key_is_char(logical_key, physical_key, 'z') {
                if shift {
                    if self.redo() {
                        return (TextInputKeyResult::Changed, None);
                    }
                } else if self.undo() {
                    return (TextInputKeyResult::Changed, None);
                }
                return (TextInputKeyResult::Navigated, None);
            }
            if key_is_char(logical_key, physical_key, 'y') {
                if self.redo() {
                    return (TextInputKeyResult::Changed, None);
                }
                return (TextInputKeyResult::Navigated, None);
            }
            if key_is_char(logical_key, physical_key, 'a') {
                self.select_all();
                return (TextInputKeyResult::Navigated, None);
            }
            if key_is_char(logical_key, physical_key, 'c') {
                let to_copy = self.selected_text().map(|s| s.to_string());
                return (TextInputKeyResult::Navigated, to_copy);
            }
            if key_is_char(logical_key, physical_key, 'x') {
                let to_copy = self.selected_text().map(|s| s.to_string());
                if to_copy.is_some() {
                    self.delete_selection();
                    return (TextInputKeyResult::Changed, to_copy);
                }
                return (TextInputKeyResult::Navigated, None);
            }
            if key_is_char(logical_key, physical_key, 'v') {
                if let Some(paste) = clipboard_paste {
                    self.insert_text(paste);
                    return (TextInputKeyResult::Changed, None);
                }
                return (TextInputKeyResult::Navigated, None);
            }
        }

        let is_backspace = matches!(logical_key, Key::Named(NamedKey::Backspace))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::Backspace)
            )
            || matches!(logical_key, Key::Character(s) if s == "\x17" || s == "\x08" || s == "\x7f");

        let is_delete = matches!(logical_key, Key::Named(NamedKey::Delete))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::Delete)
            );

        let is_left = matches!(logical_key, Key::Named(NamedKey::ArrowLeft))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::ArrowLeft)
            );

        let is_right = matches!(logical_key, Key::Named(NamedKey::ArrowRight))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::ArrowRight)
            );

        let is_home = matches!(logical_key, Key::Named(NamedKey::Home))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::Home)
            );

        let is_end = matches!(logical_key, Key::Named(NamedKey::End))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::End)
            );

        if is_left {
            self.move_left(shift, ctrl);
            return (TextInputKeyResult::Navigated, None);
        }
        if is_right {
            self.move_right(shift, ctrl);
            return (TextInputKeyResult::Navigated, None);
        }
        if is_home {
            self.move_home(shift);
            return (TextInputKeyResult::Navigated, None);
        }
        if is_end {
            self.move_end(shift);
            return (TextInputKeyResult::Navigated, None);
        }
        if is_backspace {
            let changed = if ctrl || alt {
                self.delete_word_backward()
            } else {
                self.delete_backward()
            };
            return if changed {
                (TextInputKeyResult::Changed, None)
            } else {
                (TextInputKeyResult::Navigated, None)
            };
        }
        if is_delete {
            let changed = if ctrl {
                self.delete_word_forward()
            } else {
                self.delete_forward()
            };
            return if changed {
                (TextInputKeyResult::Changed, None)
            } else {
                (TextInputKeyResult::Navigated, None)
            };
        }
        if matches!(logical_key, Key::Named(NamedKey::Escape)) && self.has_selection() {
            self.clear_selection();
            return (TextInputKeyResult::Navigated, None);
        }

        if matches!(
            logical_key,
            Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
        ) {
            return (TextInputKeyResult::Ignored, None);
        }

        if !ctrl && !alt {
            let input_str = text.or(match logical_key {
                Key::Character(s) => Some(s.as_str()),
                _ => None,
            });
            if let Some(t) = input_str {
                let mut changed = false;
                for ch in t.chars().filter(|c| !c.is_control()) {
                    self.insert_char(ch);
                    changed = true;
                }
                if changed {
                    return (TextInputKeyResult::Changed, None);
                }
            }
        }

        (TextInputKeyResult::Ignored, None)
    }

    // --- Métodos internos de cálculo de límites ---

    fn clamp_cursor(&mut self) {
        if self.cursor > self.text.len() {
            self.cursor = self.text.len();
        }
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor = self.cursor.saturating_sub(1);
        }
    }

    fn prev_char_index(&self, idx: usize) -> usize {
        if idx == 0 {
            return 0;
        }
        let mut p = idx - 1;
        while p > 0 && !self.text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn next_char_index(&self, idx: usize) -> usize {
        if idx >= self.text.len() {
            return self.text.len();
        }
        let mut n = idx + 1;
        while n < self.text.len() && !self.text.is_char_boundary(n) {
            n += 1;
        }
        n
    }

    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Límite de palabra hacia la izquierda desde `from`
    fn word_boundary_left(&self, from: usize) -> usize {
        let from = from.min(self.text.len());
        let s = &self.text[..from];
        let mut chars: Vec<(usize, char)> = s.char_indices().collect();
        if chars.is_empty() {
            return 0;
        }

        // 1. Saltar espacios a la izquierda
        while let Some(&(_, c)) = chars.last() {
            if c.is_whitespace() {
                chars.pop();
            } else {
                break;
            }
        }
        // 2. Determinar la clase del carácter inicial
        let Some(&(mut target, last_c)) = chars.last() else {
            return 0;
        };
        let word_class = Self::is_word_char(last_c);

        // 3. Retroceder mientras sea de la misma clase
        while let Some(&(idx, c)) = chars.last() {
            if c.is_whitespace() || (Self::is_word_char(c) != word_class) {
                break;
            }
            target = idx;
            chars.pop();
        }
        target
    }

    /// Límite de palabra hacia la derecha desde `from`
    fn word_boundary_right(&self, from: usize) -> usize {
        let from = from.min(self.text.len());
        let s = &self.text[from..];
        let mut chars = s.char_indices();
        let Some((_, first_c)) = chars.next() else {
            return self.text.len();
        };

        if first_c.is_whitespace() {
            // Si arranca en espacio, saltar todos los espacios
            for (idx, c) in chars {
                if !c.is_whitespace() {
                    return from + idx;
                }
            }
            return self.text.len();
        }

        let word_class = Self::is_word_char(first_c);
        let mut end = self.text.len();
        for (idx, c) in chars {
            if c.is_whitespace() || (Self::is_word_char(c) != word_class) {
                end = from + idx;
                break;
            }
        }
        end
    }

    /// Rango de palabra alrededor de `char_idx` (para doble clic)
    fn word_range_around(&self, char_idx: usize) -> (usize, usize) {
        if self.text.is_empty() {
            return (0, 0);
        }
        let pos = char_idx.min(self.text.len());
        // Encontrar el char en esa posición o el anterior
        let mut probe = pos;
        if probe >= self.text.len() && probe > 0 {
            probe = self.prev_char_index(probe);
        }
        let c = self.text[probe..].chars().next().unwrap_or(' ');
        let is_word = Self::is_word_char(c);

        // Buscar inicio
        let mut start = probe;
        let left = &self.text[..probe];
        for (idx, ch) in left.char_indices().rev() {
            if Self::is_word_char(ch) == is_word && !ch.is_whitespace() {
                start = idx;
            } else {
                break;
            }
        }

        // Buscar fin
        let mut end = probe + c.len_utf8();
        let right = &self.text[end..];
        for (idx, ch) in right.char_indices() {
            if Self::is_word_char(ch) == is_word && !ch.is_whitespace() {
                end = probe + c.len_utf8() + idx + ch.len_utf8();
            } else {
                break;
            }
        }

        (start, end)
    }

    /// Calcula la posición de línea y columna del cursor (1-indexed para la interfaz de usuario).
    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        let prefix = &self.text[..self.cursor.min(self.text.len())];
        let line = prefix.matches('\n').count() + 1;
        let col = match prefix.rfind('\n') {
            Some(last_nl) => prefix[last_nl + 1..].chars().count() + 1,
            None => prefix.chars().count() + 1,
        };
        (line, col)
    }

    /// Número total de líneas en el texto.
    #[allow(dead_code)]
    pub(crate) fn line_count(&self) -> usize {
        let count = self.text.matches('\n').count() + 1;
        count
    }

    /// Inserta un salto de línea en la posición actual del cursor (multilínea).
    pub(crate) fn insert_newline(&mut self) {
        self.delete_selection();
        self.push_undo();
        self.text.insert(self.cursor, '\n');
        self.cursor += 1;
        self.clamp_cursor();
    }

    /// Mueve el cursor al inicio de la línea actual.
    pub(crate) fn move_line_start(&mut self, extend_selection: bool) {
        let prefix = &self.text[..self.cursor.min(self.text.len())];
        let target = match prefix.rfind('\n') {
            Some(last_nl) => last_nl + 1,
            None => 0,
        };
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = target;
    }

    /// Mueve el cursor al final de la línea actual.
    pub(crate) fn move_line_end(&mut self, extend_selection: bool) {
        let suffix = &self.text[self.cursor.min(self.text.len())..];
        let target = match suffix.find('\n') {
            Some(next_nl) => self.cursor + next_nl,
            None => self.text.len(),
        };
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = target;
    }

    /// Mueve el cursor hacia la línea anterior manteniendo la columna relativa.
    pub(crate) fn move_up(&mut self, extend_selection: bool) {
        let (_, cur_col) = self.cursor_line_col();
        let prefix = &self.text[..self.cursor.min(self.text.len())];
        let Some(prev_nl) = prefix.rfind('\n') else {
            // Ya estamos en la primera línea: mover al inicio de la línea
            self.move_line_start(extend_selection);
            return;
        };
        // Inicio de la línea anterior
        let line_before_prefix = &self.text[..prev_nl];
        let prev_line_start = line_before_prefix.rfind('\n').map_or(0, |nl| nl + 1);
        let prev_line_len = prev_nl - prev_line_start;

        // Calcular posición de destino basada en número de caracteres
        let prev_line_str = &self.text[prev_line_start..prev_nl];
        let target_byte_offset = prev_line_str
            .char_indices()
            .nth(cur_col.saturating_sub(1))
            .map_or(prev_line_len, |(idx, _)| idx);

        let target = prev_line_start + target_byte_offset;
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = target;
        self.clamp_cursor();
    }

    /// Mueve el cursor hacia la siguiente línea manteniendo la columna relativa.
    pub(crate) fn move_down(&mut self, extend_selection: bool) {
        let (_, cur_col) = self.cursor_line_col();
        let suffix = &self.text[self.cursor.min(self.text.len())..];
        let Some(next_nl) = suffix.find('\n') else {
            // Ya estamos en la última línea: mover al final
            self.move_line_end(extend_selection);
            return;
        };
        let next_line_start = self.cursor + next_nl + 1;
        if next_line_start >= self.text.len() {
            let target = self.text.len();
            if extend_selection {
                if self.select_anchor.is_none() {
                    self.select_anchor = Some(self.cursor);
                }
            } else {
                self.select_anchor = None;
            }
            self.cursor = target;
            return;
        }

        let next_line_rest = &self.text[next_line_start..];
        let next_line_len = next_line_rest.find('\n').unwrap_or(next_line_rest.len());
        let next_line_str = &next_line_rest[..next_line_len];

        let target_byte_offset = next_line_str
            .char_indices()
            .nth(cur_col.saturating_sub(1))
            .map_or(next_line_len, |(idx, _)| idx);

        let target = next_line_start + target_byte_offset;
        if extend_selection {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
        self.cursor = target;
        self.clamp_cursor();
    }

    /// Posiciona el cursor a partir de una línea y columna (1-indexed).
    pub(crate) fn set_cursor_from_line_col(&mut self, target_line: usize, target_col: usize) {
        let mut cur_line = 1;
        let mut line_start = 0;
        for (idx, ch) in self.text.char_indices() {
            if cur_line == target_line {
                break;
            }
            if ch == '\n' {
                cur_line += 1;
                line_start = idx + 1;
            }
        }

        if cur_line < target_line {
            self.cursor = self.text.len();
            self.select_anchor = None;
            return;
        }

        let line_rest = &self.text[line_start..];
        let line_len = line_rest.find('\n').unwrap_or(line_rest.len());
        let line_str = &line_rest[..line_len];

        let offset = line_str
            .char_indices()
            .nth(target_col.saturating_sub(1))
            .map_or(line_len, |(idx, _)| idx);

        self.cursor = line_start + offset;
        self.select_anchor = None;
        self.clamp_cursor();
    }

    /// Procesa pulsaciones de teclas para editores multilínea (consola SQL, notas).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_multiline_key(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: Option<&str>,
        ctrl: bool,
        shift: bool,
        alt: bool,
        clipboard_paste: Option<&str>,
    ) -> (TextInputKeyResult, Option<String>) {
        let is_up = matches!(logical_key, Key::Named(NamedKey::ArrowUp))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::ArrowUp)
            );
        if is_up {
            self.move_up(shift);
            return (TextInputKeyResult::Navigated, None);
        }

        let is_down = matches!(logical_key, Key::Named(NamedKey::ArrowDown))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::ArrowDown)
            );
        if is_down {
            self.move_down(shift);
            return (TextInputKeyResult::Navigated, None);
        }

        let is_enter = matches!(logical_key, Key::Named(NamedKey::Enter))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::Enter)
            );
        if is_enter && !ctrl && !alt {
            self.insert_newline();
            return (TextInputKeyResult::Changed, None);
        }

        let is_home = matches!(logical_key, Key::Named(NamedKey::Home))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::Home)
            );
        if is_home {
            if ctrl {
                self.move_home(shift);
            } else {
                self.move_line_start(shift);
            }
            return (TextInputKeyResult::Navigated, None);
        }

        let is_end = matches!(logical_key, Key::Named(NamedKey::End))
            || matches!(
                physical_key,
                PhysicalKey::Code(winit::keyboard::KeyCode::End)
            );
        if is_end {
            if ctrl {
                self.move_end(shift);
            } else {
                self.move_line_end(shift);
            }
            return (TextInputKeyResult::Navigated, None);
        }

        self.handle_key(
            logical_key,
            physical_key,
            text,
            ctrl,
            shift,
            alt,
            clipboard_paste,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_insert_and_delete() {
        let mut input = TextInput::new("");
        input.insert_text("hola mundo");
        assert_eq!(input.text(), "hola mundo");
        assert_eq!(input.cursor, 10);

        // Backspace
        input.delete_backward();
        assert_eq!(input.text(), "hola mund");
        assert_eq!(input.cursor, 9);

        // Word delete backward
        input.delete_word_backward();
        assert_eq!(input.text(), "hola ");
        assert_eq!(input.cursor, 5);

        input.delete_word_backward();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_text_input_selection_and_replace() {
        let mut input = TextInput::new("hola mundo");
        input.select_anchor = Some(5);
        input.cursor = 10;
        assert_eq!(input.selected_text(), Some("mundo"));

        input.insert_text("amigo");
        assert_eq!(input.text(), "hola amigo");
        assert_eq!(input.cursor, 10);
        assert!(!input.has_selection());
    }

    #[test]
    fn test_text_input_word_boundary_navigation() {
        let mut input = TextInput::new("primera_palabra segunda tercera");
        input.cursor = input.len();

        input.move_left(false, true);
        assert_eq!(&input.text[input.cursor..], "tercera");

        input.move_left(false, true);
        assert_eq!(&input.text[input.cursor..], "segunda tercera");

        input.move_right(true, true);
        assert_eq!(input.selected_text(), Some("segunda"));
    }

    #[test]
    fn test_text_input_utf8_accents() {
        let mut input = TextInput::new("configuración rápida");
        input.delete_word_backward();
        assert_eq!(input.text(), "configuración ");
        input.delete_word_backward();
        assert_eq!(input.text(), "");
    }

    #[test]
    fn test_text_input_hit_test() {
        let input = TextInput::new("abc");
        // Medida simulada: 10 px por carácter
        let measure = |s: &str| s.len() * 10;
        assert_eq!(input.hit_test(0, 10, measure), 0);
        assert_eq!(input.hit_test(14, 10, measure), 0);
        assert_eq!(input.hit_test(16, 10, measure), 1);
        assert_eq!(input.hit_test(24, 10, measure), 1);
        assert_eq!(input.hit_test(26, 10, measure), 2);
        assert_eq!(input.hit_test(36, 10, measure), 3);
        assert_eq!(input.hit_test(100, 10, measure), 3);
    }

    #[test]
    fn test_text_input_undo_redo() {
        let mut input = TextInput::new("hola");
        input.insert_text(" mundo");
        assert_eq!(input.text(), "hola mundo");

        assert!(input.undo());
        assert_eq!(input.text(), "hola");

        assert!(input.redo());
        assert_eq!(input.text(), "hola mundo");
    }

    #[test]
    fn test_text_input_shift_click_selection() {
        let mut input = TextInput::new("hola mundo");
        input.mouse_down(0, false);
        input.mouse_up();
        assert_eq!(input.cursor, 0);
        assert!(!input.has_selection());

        input.mouse_down(4, true);
        assert_eq!(input.cursor, 4);
        assert_eq!(input.selected_text(), Some("hola"));
    }

    #[test]
    fn test_text_input_escape_clears_selection() {
        let mut input = TextInput::new("texto");
        input.select_all();
        assert!(input.has_selection());

        let (res, _) = input.handle_key(
            &Key::Named(NamedKey::Escape),
            PhysicalKey::Code(winit::keyboard::KeyCode::Escape),
            None,
            false,
            false,
            false,
            None,
        );
        assert_eq!(res, TextInputKeyResult::Navigated);
        assert!(!input.has_selection());
    }

    #[test]
    fn test_text_input_multiline_navigation_and_lines() {
        let mut input = TextInput::new("SELECT *\nFROM users\nWHERE id = 1;");
        assert_eq!(input.line_count(), 3);

        // Al inicio, cursor está al final: "WHERE id = 1;" tiene 13 chars -> col 14
        assert_eq!(input.cursor_line_col(), (3, 14));

        // Subir a línea 2: "FROM users" tiene 10 chars -> col 11 (al final de línea)
        input.move_up(false);
        assert_eq!(input.cursor_line_col(), (2, 11));

        // Subir a línea 1: "SELECT *" tiene 8 chars -> col 9 (al final de línea)
        input.move_up(false);
        assert_eq!(input.cursor_line_col(), (1, 9));

        // Bajar a línea 2 manteniendo col 9
        input.move_down(false);
        assert_eq!(input.cursor_line_col(), (2, 9));

        // Set cursor explícito
        input.set_cursor_from_line_col(1, 1);
        assert_eq!(input.cursor_line_col(), (1, 1));
        assert_eq!(input.cursor, 0);

        // Insertar salto de línea
        input.insert_newline();
        assert_eq!(input.line_count(), 4);
        assert_eq!(input.cursor_line_col(), (2, 1));
    }
}
