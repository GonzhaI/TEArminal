//! Búsqueda en el scrollback (`Ctrl + Shift + F`): coincidencias sobre el historial y la pantalla.

use super::TerminalState;
use crate::ui::TextInput;

/// Tope de coincidencias guardadas: una consulta de un carácter sobre 10 000 líneas no debe crecer sin límite.
const MAX_MATCHES: usize = 10_000;

/// Barra de búsqueda abierta de un panel. Cada coincidencia es `(línea global, columna inicial, columna final
/// exclusiva)`, ordenadas por línea (los resaltados usan `partition_point`).
pub(crate) struct TermSearch {
    pub(crate) input: TextInput,
    pub(crate) matches: Vec<(usize, usize, usize)>,
    pub(crate) active: Option<usize>,
}

impl TerminalState {
    /// Abre la barra (o la deja abierta con la consulta anterior seleccionada).
    pub(crate) fn open_search(&mut self) {
        match &mut self.search {
            Some(s) => s.input.select_all(),
            None => {
                self.search = Some(TermSearch {
                    input: TextInput::new(""),
                    matches: Vec::new(),
                    active: None,
                })
            }
        }
        self.update_search_matches();
    }

    pub(crate) fn close_search(&mut self) {
        self.search = None;
    }

    /// Recalcula las coincidencias (sin distinción de mayúsculas) y activa la primera desde la vista actual.
    pub(crate) fn update_search_matches(&mut self) {
        let Some(query) = self.search.as_ref().map(|s| s.input.text().to_lowercase()) else {
            return;
        };
        let q: Vec<char> = query.chars().collect();
        let mut matches = Vec::new();
        if !q.is_empty() {
            let mut chars: Vec<char> = Vec::new();
            let mut cols: Vec<(usize, usize)> = Vec::new(); // (columna, ancho)
            'lines: for line in 0..self.total_buffer_lines() {
                let Some(row) = self.get_buffer_line(line) else {
                    continue;
                };
                chars.clear();
                cols.clear();
                for (col, cell) in row.iter().enumerate() {
                    if cell.width > 0 {
                        chars.push(cell.c.to_lowercase().next().unwrap_or(cell.c));
                        cols.push((col, cell.width as usize));
                    }
                }
                if chars.len() < q.len() {
                    continue;
                }
                for i in 0..=chars.len() - q.len() {
                    if chars[i..i + q.len()] == q[..] {
                        let (last_col, last_w) = cols[i + q.len() - 1];
                        matches.push((line, cols[i].0, last_col + last_w));
                        if matches.len() >= MAX_MATCHES {
                            break 'lines;
                        }
                    }
                }
            }
        }
        let start = self.visible_line_start();
        let active = if matches.is_empty() {
            None
        } else {
            let i = matches.partition_point(|&(l, _, _)| l < start);
            Some(if i < matches.len() { i } else { matches.len() - 1 })
        };
        if let Some(s) = &mut self.search {
            s.matches = matches;
            s.active = active;
        }
        self.reveal_active_match();
    }

    /// Salta a la coincidencia siguiente (`forward`) o anterior, dando la vuelta al llegar al extremo.
    pub(crate) fn search_step(&mut self, forward: bool) {
        if let Some(s) = &mut self.search {
            let n = s.matches.len();
            if n == 0 {
                return;
            }
            s.active = Some(match (s.active, forward) {
                (None, _) => 0,
                (Some(i), true) => (i + 1) % n,
                (Some(i), false) => (i + n - 1) % n,
            });
        }
        self.reveal_active_match();
    }

    /// Desplaza el historial para que la coincidencia activa quede visible (centrada si estaba fuera).
    fn reveal_active_match(&mut self) {
        if self.is_alt_screen {
            return;
        }
        let Some(line) = self
            .search
            .as_ref()
            .and_then(|s| s.active.map(|i| s.matches[i].0))
        else {
            return;
        };
        let rows = self.grid.len().max(1);
        let start = self.visible_line_start();
        if line >= start && line < start + rows {
            return;
        }
        let target = line.saturating_sub(rows / 2);
        self.scroll_offset = self.scrollback.len().saturating_sub(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn term_with(lines: &[&str]) -> TerminalState {
        let mut t = TerminalState::new(20, 3, 100, Theme::default());
        t.process_bytes(lines.join("\r\n").as_bytes());
        t
    }

    #[test]
    fn test_search_finds_matches_in_scrollback_and_grid_ignoring_case() {
        let mut t = term_with(&["Hola", "mundo", "HOLA otra", "x", "hola"]);
        assert!(!t.scrollback.is_empty(), "las primeras líneas pasaron al historial");
        t.open_search();
        t.search.as_mut().unwrap().input = TextInput::new("hola");
        t.update_search_matches();
        let s = t.search.as_ref().unwrap();
        assert_eq!(s.matches.len(), 3);
        assert_eq!(s.matches[0], (0, 0, 4));
    }

    #[test]
    fn test_search_step_wraps_and_scrolls_to_match() {
        let mut t = term_with(&["uno", "dos", "tres", "cuatro", "cinco", "uno"]);
        t.open_search();
        t.search.as_mut().unwrap().input = TextInput::new("uno");
        t.update_search_matches();
        assert_eq!(t.search.as_ref().unwrap().active, Some(1), "empieza en la vista actual");
        t.search_step(true);
        assert_eq!(t.search.as_ref().unwrap().active, Some(0), "da la vuelta");
        assert!(t.scroll_offset > 0, "la primera coincidencia está en el historial");
        t.close_search();
        assert!(t.search.is_none());
    }
}
