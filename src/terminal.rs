use crate::theme::{Color, Theme};
use std::collections::VecDeque;
use vte::{Params, Parser, Perform};

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub c: char,
    pub width: u8,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
}

impl Cell {
    pub fn default_with_theme(theme: &Theme) -> Self {
        Self {
            c: ' ',
            width: 1,
            fg: theme.foreground,
            bg: Color::TRANSPARENT,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            strikethrough: false,
            inverse: false,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct InlineImage {
    pub id: u64,
    pub line_index: usize,
    pub col: usize,
    pub width_cols: usize,
    pub height_rows: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba_data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandBlock {
    pub prompt_line: usize,
    pub command_line: Option<usize>,
    pub output_start: Option<usize>,
    pub output_end: Option<usize>,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshSession {
    pub destination: String,
    pub args: Vec<String>,
    pub remote_dir: Option<String>,
    pub is_active: bool,
}

pub struct TerminalState {
    pub cols: usize,
    pub rows: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub grid: Vec<Vec<Cell>>,
    pub scrollback: VecDeque<Vec<Cell>>,
    pub max_scrollback: usize,
    pub scroll_offset: usize,

    // Alternate Screen Buffer (Soporte nativo para tmux, btop, nvtop, vim)
    pub is_alt_screen: bool,
    pub alt_grid: Vec<Vec<Cell>>,
    pub saved_cursor_x: usize,
    pub saved_cursor_y: usize,

    // Visibilidad del cursor y márgenes de desplazamiento (DECSTBM para split panes de tmux)
    pub cursor_visible: bool,
    pub scroll_region_top: usize,
    pub scroll_region_bottom: usize,

    // Bloques semánticos y navegación por comandos (OSC 133 Shell Integration)
    pub command_blocks: Vec<CommandBlock>,
    pub active_block: Option<CommandBlock>,

    // Directorio de trabajo actual notificado vía OSC 7 u OSC 9;9
    pub current_dir: Option<std::path::PathBuf>,

    // Cola de portapapeles OSC 52 (Integración segura bidireccional para túneles SSH)
    pub clipboard_queue: Vec<String>,

    // Sesión SSH remota activa y título de ventana notificado vía OSC 0 / OSC 2
    pub ssh_session: Option<SshSession>,
    pub window_title: Option<String>,

    // Bracketed paste dinámico negociado con agentes (?2004h / ?2004l)
    pub bracketed_paste_mode: bool,

    // Soporte para protocolos interactivos TUI (Herdr, Ratatui, Lazygit)
    pub mouse_reporting_normal: bool, // Modo 1000: Reporte de clics
    pub mouse_reporting_button_event: bool, // Modo 1002: Clics y arrastre
    pub mouse_reporting_any_event: bool, // Modo 1003: Todo movimiento
    pub mouse_reporting_sgr: bool,    // Modo 1006: Coordenadas SGR extendidas (\x1b[<...M/m)
    pub focus_event_reporting: bool,  // Modo 1004: Foco ganado/perdido (\x1b[I, \x1b[O)
    pub sync_update_mode: bool,       // Modo 2026: Renderizado sincronizado (DEC 2026)

    // Imágenes renderizadas en terminal (Protocolos iTerm2 / OSC 1337 / Visión IA)
    pub images: Vec<InlineImage>,
    pub next_image_id: u64,
    pub total_lines_count: usize,

    // Estado SGR actual
    pub theme: Theme,
    current_fg: Color,
    current_bg: Color,
    current_bold: bool,
    current_dim: bool,
    current_italic: bool,
    current_underline: bool,
    current_strikethrough: bool,
    current_inverse: bool,

    // Selección de texto
    pub selection: Option<SelectionRange>,

    parser: Option<Parser>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRange {
    pub start: (usize, usize), // (col, row_in_view)
    pub end: (usize, usize),
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize, max_scrollback: usize, theme: Theme) -> Self {
        let default_cell = Cell::default_with_theme(&theme);
        let grid = vec![vec![default_cell.clone(); cols]; rows];
        let alt_grid = vec![vec![default_cell; cols]; rows];
        let fg = theme.foreground;

        Self {
            cols,
            rows,
            cursor_x: 0,
            cursor_y: 0,
            grid,
            scrollback: VecDeque::with_capacity(max_scrollback.min(1000)),
            max_scrollback,
            scroll_offset: 0,
            is_alt_screen: false,
            alt_grid,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            cursor_visible: true,
            scroll_region_top: 0,
            scroll_region_bottom: rows.saturating_sub(1),
            command_blocks: Vec::new(),
            active_block: None,
            current_dir: None,
            clipboard_queue: Vec::new(),
            ssh_session: None,
            window_title: None,
            bracketed_paste_mode: false,
            mouse_reporting_normal: false,
            mouse_reporting_button_event: false,
            mouse_reporting_any_event: false,
            mouse_reporting_sgr: false,
            focus_event_reporting: false,
            sync_update_mode: false,
            images: Vec::new(),
            next_image_id: 1,
            total_lines_count: 0,
            theme,
            current_fg: fg,
            current_bg: Color::TRANSPARENT,
            current_bold: false,
            current_dim: false,
            current_italic: false,
            current_underline: false,
            current_strikethrough: false,
            current_inverse: false,
            selection: None,
            parser: Some(Parser::new()),
        }
    }

    pub fn is_mouse_reporting_active(&self) -> bool {
        self.mouse_reporting_normal
            || self.mouse_reporting_button_event
            || self.mouse_reporting_any_event
    }

    pub fn current_global_line(&self) -> usize {
        self.scrollback.len() + self.cursor_y
    }

    pub fn get_last_output_text(&self) -> Option<String> {
        let block = self.command_blocks.last()?;
        let start = block.output_start?;
        let end = block
            .output_end
            .unwrap_or_else(|| self.current_global_line());
        if start > end {
            return None;
        }

        let mut lines = Vec::new();
        let scroll_len = self.scrollback.len();
        for line_idx in start..=end {
            if line_idx < scroll_len {
                if let Some(row) = self.scrollback.get(line_idx) {
                    let s: String = row.iter().filter(|c| c.width > 0).map(|c| c.c).collect();
                    let trimmed = s.trim_end();
                    if !trimmed.is_empty() {
                        lines.push(trimmed.to_string());
                    }
                }
            } else {
                let grid_idx = line_idx.saturating_sub(scroll_len);
                if grid_idx < self.grid.len() {
                    let row = &self.grid[grid_idx];
                    let s: String = row.iter().filter(|c| c.width > 0).map(|c| c.c).collect();
                    let trimmed = s.trim_end();
                    if !trimmed.is_empty() {
                        lines.push(trimmed.to_string());
                    }
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

    pub fn visible_line_start(&self) -> usize {
        self.scrollback.len().saturating_sub(self.scroll_offset)
    }

    pub fn active_grid_mut(&mut self) -> &mut Vec<Vec<Cell>> {
        if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        }
    }

    pub fn enter_alt_screen(&mut self) {
        if !self.is_alt_screen {
            self.is_alt_screen = true;
            self.saved_cursor_x = self.cursor_x;
            self.saved_cursor_y = self.cursor_y;
            let default_cell = Cell::default_with_theme(&self.theme);
            self.alt_grid = vec![vec![default_cell; self.cols]; self.rows];
            self.cursor_x = 0;
            self.cursor_y = 0;
        }
    }

    pub fn exit_alt_screen(&mut self) {
        if self.is_alt_screen {
            self.is_alt_screen = false;
            self.cursor_x = self.saved_cursor_x.min(self.cols.saturating_sub(1));
            self.cursor_y = self.saved_cursor_y.min(self.rows.saturating_sub(1));
        }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        if let Some(mut parser) = self.parser.take() {
            let mut performer = Performer { state: self };
            parser.advance(&mut performer, bytes);
            performer.state.parser = Some(parser);
        }
    }

    pub fn newline(&mut self) {
        self.total_lines_count += 1;
        let default_cell = Cell::default_with_theme(&self.theme);

        if self.is_alt_screen {
            if self.cursor_y >= self.scroll_region_bottom {
                if self.scroll_region_top < self.alt_grid.len() {
                    self.alt_grid.remove(self.scroll_region_top);
                    let insert_pos = self.scroll_region_bottom.min(self.alt_grid.len());
                    self.alt_grid
                        .insert(insert_pos, vec![default_cell; self.cols]);
                }
            } else {
                self.cursor_y += 1;
            }
            return;
        }

        if self.cursor_y >= self.scroll_region_bottom {
            if self.scroll_region_top == 0
                && self.scroll_region_bottom == self.rows.saturating_sub(1)
            {
                // Desplazamiento fullscreen a O(1) con VecDeque
                let top_row = self.grid.remove(0);
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                }
                self.scrollback.push_back(top_row);
                self.grid.push(vec![default_cell; self.cols]);
            } else {
                // Sub-región de desplazamiento (split panes de tmux)
                if self.scroll_region_top < self.grid.len() {
                    self.grid.remove(self.scroll_region_top);
                    let insert_pos = self.scroll_region_bottom.min(self.grid.len());
                    self.grid.insert(insert_pos, vec![default_cell; self.cols]);
                }
            }
        } else {
            self.cursor_y += 1;
        }
    }

    pub fn put_char(&mut self, c: char) {
        use unicode_width::UnicodeWidthChar;
        let char_w = c.width().unwrap_or(1);

        // Caracteres de control no imprimibles
        if char_w == 0 && c.is_control() {
            return;
        }

        let w = if char_w == 0 { 1 } else { char_w.min(2) as u8 };

        // Si es de doble ancho (emoji, CJK) y no cabe al final de la línea, envolver automáticamente
        if w == 2 && self.cursor_x + 1 >= self.cols {
            self.cursor_x = 0;
            self.newline();
        } else if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }

        let grid = if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        };

        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            grid[self.cursor_y][self.cursor_x] = Cell {
                c,
                width: w,
                fg: self.current_fg,
                bg: self.current_bg,
                bold: self.current_bold,
                dim: self.current_dim,
                italic: self.current_italic,
                underline: self.current_underline,
                strikethrough: self.current_strikethrough,
                inverse: self.current_inverse,
            };

            // Para caracteres de 2 columnas, la celda contigua es un espaciador de continuación (width: 0)
            if w == 2 && self.cursor_x + 1 < self.cols {
                grid[self.cursor_y][self.cursor_x + 1] = Cell {
                    c: ' ',
                    width: 0,
                    fg: self.current_fg,
                    bg: self.current_bg,
                    bold: self.current_bold,
                    dim: self.current_dim,
                    italic: self.current_italic,
                    underline: self.current_underline,
                    strikethrough: self.current_strikethrough,
                    inverse: self.current_inverse,
                };
                self.cursor_x += 2;
            } else {
                self.cursor_x += 1;
            }
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        if self.is_alt_screen {
            return;
        }
        let max_offset = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        if self.is_alt_screen {
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    /// Obtiene las filas que deben renderizarse en pantalla según `scroll_offset`
    pub fn visible_rows(&self) -> Vec<&Vec<Cell>> {
        if self.is_alt_screen {
            return self.alt_grid.iter().collect();
        }

        if self.scroll_offset == 0 {
            self.grid.iter().collect()
        } else {
            let total_history = self.scrollback.len();
            let start = total_history.saturating_sub(self.scroll_offset);
            let mut rows = Vec::with_capacity(self.rows);

            for r in self.scrollback.range(start..) {
                rows.push(r);
                if rows.len() == self.rows {
                    return rows;
                }
            }
            for r in &self.grid {
                rows.push(r);
                if rows.len() == self.rows {
                    return rows;
                }
            }
            rows
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }
        let default_cell = Cell::default_with_theme(&self.theme);

        // Ajustar columnas
        for row in self.grid.iter_mut() {
            row.resize(new_cols, default_cell.clone());
        }
        for row in self.alt_grid.iter_mut() {
            row.resize(new_cols, default_cell.clone());
        }

        // Ajustar filas
        if new_rows > self.rows {
            for _ in self.rows..new_rows {
                self.grid.push(vec![default_cell.clone(); new_cols]);
                self.alt_grid.push(vec![default_cell.clone(); new_cols]);
            }
        } else if new_rows < self.rows {
            // Solo si el cursor desborda la nueva altura de la ventana, desplazar líneas hacia el scrollback
            if self.cursor_y >= new_rows {
                let shift = (self.cursor_y + 1).saturating_sub(new_rows);
                for _ in 0..shift {
                    if !self.grid.is_empty() {
                        let r = self.grid.remove(0);
                        self.scrollback.push_back(r);
                    }
                    if !self.alt_grid.is_empty() {
                        self.alt_grid.remove(0);
                    }
                }
                self.cursor_y = self.cursor_y.saturating_sub(shift);
            }

            // Recortar el exceso de filas vacías del final de la pantalla
            while self.grid.len() > new_rows {
                self.grid.pop();
            }
            while self.alt_grid.len() > new_rows {
                self.alt_grid.pop();
            }
        }

        self.cols = new_cols;
        self.rows = new_rows;
        self.scroll_region_top = 0;
        self.scroll_region_bottom = new_rows.saturating_sub(1);
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self.cursor_y.min(new_rows.saturating_sub(1));
    }

    pub fn copy_selection_text(&self) -> Option<String> {
        let sel = self.selection?;
        let (mut p1, mut p2) = (sel.start, sel.end);
        if p1.1 > p2.1 || (p1.1 == p2.1 && p1.0 > p2.0) {
            std::mem::swap(&mut p1, &mut p2);
        }

        let visible = self.visible_rows();
        let mut result = String::new();

        for y in p1.1..=p2.1.min(visible.len().saturating_sub(1)) {
            let row = visible[y];
            let x_start = if y == p1.1 { p1.0 } else { 0 };
            let x_end = if y == p2.1 {
                p2.0.min(row.len().saturating_sub(1))
            } else {
                row.len().saturating_sub(1)
            };

            let mut line = String::new();
            for x in x_start..=x_end {
                if x < row.len() && row[x].width > 0 {
                    line.push(row[x].c);
                }
            }
            result.push_str(line.trim_end());
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
}

struct Performer<'a> {
    state: &'a mut TerminalState,
}

impl<'a> Perform for Performer<'a> {
    fn print(&mut self, c: char) {
        self.state.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.state.newline(),
            b'\r' => self.state.cursor_x = 0,
            b'\x08' => {
                if self.state.cursor_x > 0 {
                    self.state.cursor_x -= 1;
                }
            }
            b'\t' => {
                let next_tab = (self.state.cursor_x + 8) & !7;
                self.state.cursor_x = next_tab.min(self.state.cols.saturating_sub(1));
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Manejo de secuencias privadas DEC (empiezan con '?')
        if intermediates == b"?" {
            let mut iter = params.iter();
            while let Some(param) = iter.next() {
                let mode = param.first().copied().unwrap_or(0);
                match action {
                    // Set Mode (h)
                    'h' => match mode {
                        // Alternate Screen Buffer (usado por tmux, btop, nvtop, vim, less, herdr)
                        1049 | 47 => self.state.enter_alt_screen(),
                        // Mostrar cursor
                        25 => self.state.cursor_visible = true,
                        // Mouse tracking para TUIs (Herdr, Ratatui, Lazygit)
                        1000 => self.state.mouse_reporting_normal = true,
                        1002 => self.state.mouse_reporting_button_event = true,
                        1003 => self.state.mouse_reporting_any_event = true,
                        1006 => self.state.mouse_reporting_sgr = true,
                        // Reporte de foco (\x1b[I, \x1b[O)
                        1004 => self.state.focus_event_reporting = true,
                        // Renderizado sincronizado (DEC 2026 anti-flicker)
                        2026 => self.state.sync_update_mode = true,
                        // Activar Bracketed Paste Mode (negociado con Claude Code, Aider, shells)
                        2004 => self.state.bracketed_paste_mode = true,
                        _ => {}
                    },
                    // Reset Mode (l)
                    'l' => match mode {
                        // Salir de Alternate Screen Buffer y restaurar sesión original
                        1049 | 47 => self.state.exit_alt_screen(),
                        // Ocultar cursor (usado por TUIs para evitar parpadeos)
                        25 => self.state.cursor_visible = false,
                        // Desactivar Mouse tracking
                        1000 => self.state.mouse_reporting_normal = false,
                        1002 => self.state.mouse_reporting_button_event = false,
                        1003 => self.state.mouse_reporting_any_event = false,
                        1006 => self.state.mouse_reporting_sgr = false,
                        // Desactivar reporte de foco
                        1004 => self.state.focus_event_reporting = false,
                        // Finalizar renderizado sincronizado
                        2026 => self.state.sync_update_mode = false,
                        // Desactivar Bracketed Paste Mode
                        2004 => self.state.bracketed_paste_mode = false,
                        _ => {}
                    },
                    _ => {}
                }
            }
            return;
        }

        match action {
            // DECSTBM: Set Top and Bottom Margins (usado por tmux para split panes)
            'r' => {
                let mut iter = params.iter();
                let top = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1) as usize;
                let bottom = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(self.state.rows as u16)
                    .saturating_sub(1) as usize;
                if top < bottom && bottom < self.state.rows {
                    self.state.scroll_region_top = top;
                    self.state.scroll_region_bottom = bottom;
                } else {
                    self.state.scroll_region_top = 0;
                    self.state.scroll_region_bottom = self.state.rows.saturating_sub(1);
                }
                self.state.cursor_x = 0;
                self.state.cursor_y = 0;
            }
            // CUU - Cursor Up
            'A' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_y = self.state.cursor_y.saturating_sub(n.max(1));
            }
            // CUD - Cursor Down
            'B' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_y =
                    (self.state.cursor_y + n.max(1)).min(self.state.rows.saturating_sub(1));
            }
            // CUF - Cursor Forward
            'C' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_x =
                    (self.state.cursor_x + n.max(1)).min(self.state.cols.saturating_sub(1));
            }
            // CUB - Cursor Back
            'D' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_x = self.state.cursor_x.saturating_sub(n.max(1));
            }
            // CNL - Cursor Next Line
            'E' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_y =
                    (self.state.cursor_y + n.max(1)).min(self.state.rows.saturating_sub(1));
                self.state.cursor_x = 0;
            }
            // CPL - Cursor Previous Line
            'F' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.state.cursor_y = self.state.cursor_y.saturating_sub(n.max(1));
                self.state.cursor_x = 0;
            }
            // CHA - Cursor Horizontal Absolute
            'G' => {
                let col = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1) as usize;
                self.state.cursor_x = col.min(self.state.cols.saturating_sub(1));
            }
            // CUP / HVP - Cursor Position
            'H' | 'f' => {
                let mut iter = params.iter();
                let row = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1) as usize;
                let col = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1) as usize;
                self.state.cursor_y = row.min(self.state.rows.saturating_sub(1));
                self.state.cursor_x = col.min(self.state.cols.saturating_sub(1));
            }
            // ED - Erase in Display
            'J' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                let default_cell = Cell::default_with_theme(&self.state.theme);
                let erase_cell = Cell {
                    c: ' ',
                    width: 1,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    inverse: false,
                };
                match mode {
                    0 => {
                        // Borrar desde el cursor hasta el final de la pantalla
                        let cx = self.state.cursor_x;
                        let cy = self.state.cursor_y;
                        let cols = self.state.cols;
                        let grid = self.state.active_grid_mut();
                        if cy < grid.len() {
                            for x in cx..cols {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell.clone();
                                }
                            }
                            for y in (cy + 1)..grid.len() {
                                for x in 0..cols {
                                    if x < grid[y].len() {
                                        grid[y][x] = erase_cell.clone();
                                    }
                                }
                            }
                        }
                    }
                    1 => {
                        // Borrar desde el inicio de la pantalla hasta el cursor
                        let cx = self.state.cursor_x;
                        let cy = self.state.cursor_y;
                        let cols = self.state.cols;
                        let grid = self.state.active_grid_mut();
                        for y in 0..cy.min(grid.len()) {
                            for x in 0..cols {
                                if x < grid[y].len() {
                                    grid[y][x] = erase_cell.clone();
                                }
                            }
                        }
                        if cy < grid.len() {
                            for x in 0..=cx.min(cols.saturating_sub(1)) {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell.clone();
                                }
                            }
                        }
                    }
                    2 => {
                        // Borrar pantalla completa activa
                        let grid = self.state.active_grid_mut();
                        for row in grid.iter_mut() {
                            for cell in row.iter_mut() {
                                *cell = erase_cell.clone();
                            }
                        }
                        self.state.cursor_x = 0;
                        self.state.cursor_y = 0;
                        self.state.reset_scroll();
                    }
                    3 => {
                        // Borrar pantalla completa y limpiar scrollback buffer (usado por clear / cls / ConPTY)
                        let grid = self.state.active_grid_mut();
                        for row in grid.iter_mut() {
                            for cell in row.iter_mut() {
                                *cell = default_cell.clone();
                            }
                        }
                        self.state.cursor_x = 0;
                        self.state.cursor_y = 0;
                        if !self.state.is_alt_screen {
                            self.state.scrollback.clear();
                            self.state.scroll_offset = 0;
                            self.state.total_lines_count = self.state.rows;
                            self.state.command_blocks.clear();
                        }
                    }
                    _ => {}
                }
            }
            // EL - Erase in Line
            'K' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                let cy = self.state.cursor_y;
                let cx = self.state.cursor_x;
                let cols = self.state.cols;
                let erase_cell = Cell {
                    c: ' ',
                    width: 1,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    inverse: false,
                };
                let grid = self.state.active_grid_mut();
                if cy < grid.len() {
                    match mode {
                        0 => {
                            for x in cx..cols {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell.clone();
                                }
                            }
                        }
                        1 => {
                            for x in 0..=cx.min(cols.saturating_sub(1)) {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell.clone();
                                }
                            }
                        }
                        2 => {
                            for x in 0..cols {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell.clone();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // ECH - Erase Character (\x1b[PnX): borra Pn caracteres a partir del cursor
            'X' => {
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let cy = self.state.cursor_y;
                let cx = self.state.cursor_x;
                let cols = self.state.cols;
                let erase_cell = Cell {
                    c: ' ',
                    width: 1,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    inverse: false,
                };
                let grid = self.state.active_grid_mut();
                if cy < grid.len() {
                    let end_x = (cx + count).min(cols);
                    for x in cx..end_x {
                        if x < grid[cy].len() {
                            grid[cy][x] = erase_cell.clone();
                        }
                    }
                }
                crate::logger::log(&format!(
                    "CSI X (ECH): borrados {} caracteres en ({}, {})",
                    count, cx, cy
                ));
            }

            // DCH - Delete Character (\x1b[PnP): elimina Pn caracteres en el cursor desplazando hacia la izquierda
            'P' => {
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let cy = self.state.cursor_y;
                let cx = self.state.cursor_x;
                let cols = self.state.cols;
                let default_cell = Cell::default_with_theme(&self.state.theme);
                let grid = self.state.active_grid_mut();
                if cy < grid.len() && cx < cols {
                    let row = &mut grid[cy];
                    for _ in 0..count {
                        if cx < row.len() {
                            row.remove(cx);
                            row.push(default_cell.clone());
                        }
                    }
                }
                crate::logger::log(&format!(
                    "CSI P (DCH): eliminados {} caracteres en ({}, {})",
                    count, cx, cy
                ));
            }

            // ICH - Insert Character (\x1b[Pn@): inserta Pn espacios en blanco en el cursor desplazando hacia la derecha
            '@' => {
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let cy = self.state.cursor_y;
                let cx = self.state.cursor_x;
                let cols = self.state.cols;
                let default_cell = Cell::default_with_theme(&self.state.theme);
                let grid = self.state.active_grid_mut();
                if cy < grid.len() && cx < cols {
                    let row = &mut grid[cy];
                    for _ in 0..count {
                        if cx < row.len() {
                            row.insert(cx, default_cell.clone());
                            row.pop();
                        }
                    }
                }
                crate::logger::log(&format!(
                    "CSI @ (ICH): insertados {} espacios en ({}, {})",
                    count, cx, cy
                ));
            }

            // SGR - Select Graphic Rendition (Colores y Estilos)
            'm' => {
                self.handle_sgr(params);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        match params[0] {
            // Título de ventana e icono (OSC 0 y OSC 2)
            b"0" | b"2" if params.len() > 1 => {
                let title_str = String::from_utf8_lossy(params[1]).trim().to_string();
                if !title_str.is_empty() {
                    self.state.window_title = Some(title_str.clone());
                    // Inferencia automática de sesión SSH si el título lo indica (ej. "ssh: user@host")
                    if let Some(dest) = title_str.strip_prefix("ssh: ") {
                        let clean_dest = dest.trim().to_string();
                        if !clean_dest.is_empty() {
                            self.state.ssh_session = Some(SshSession {
                                destination: clean_dest.clone(),
                                args: vec![clean_dest],
                                remote_dir: None,
                                is_active: true,
                            });
                        }
                    }
                }
            }
            // Protocolo de imagen iTerm2 (OSC 1337) y señalización SSH de Axon
            b"1337" if params.len() > 1 => {
                let p1 = params[1];
                if p1.starts_with(b"AxonSSH=start") {
                    let mut dest = String::new();
                    for param in &params[1..] {
                        let text = String::from_utf8_lossy(param);
                        for part in text.split(';') {
                            if let Some(d) = part.strip_prefix("dest=") {
                                dest = d.trim().to_string();
                            }
                        }
                    }
                    if !dest.is_empty() {
                        self.state.ssh_session = Some(SshSession {
                            destination: dest.clone(),
                            args: vec![dest],
                            remote_dir: None,
                            is_active: true,
                        });
                    }
                } else if p1.starts_with(b"AxonSSH=end") {
                    if let Some(sess) = &mut self.state.ssh_session {
                        sess.is_active = false;
                    }
                    self.state.ssh_session = None;
                } else {
                    self.handle_iterm2_image(&params[1..]);
                }
            }
            // Integración de portapapeles bidireccional segura para túneles SSH (OSC 52)
            b"52" if params.len() > 1 => {
                self.handle_osc_52(&params[1..]);
            }
            // Shell Integration y Bloques Semánticos (OSC 133)
            b"133" if params.len() > 1 => {
                self.handle_osc_133(&params[1..]);
            }
            // Notificación de CWD estándar (OSC 7 file://hostname/path)
            b"7" if params.len() > 1 => {
                let uri = String::from_utf8_lossy(params[1]);
                if let Some(path_part) = uri.strip_prefix("file://") {
                    let (host, path) = if let Some(slash_pos) = path_part.find('/') {
                        (&path_part[..slash_pos], &path_part[slash_pos..])
                    } else {
                        ("", path_part)
                    };
                    let decoded_path = path.replace("%20", " ");
                    if !host.is_empty() && host != "localhost" && host != "127.0.0.1" {
                        // Host remoto vía OSC 7
                        if let Some(sess) = &mut self.state.ssh_session {
                            sess.remote_dir = Some(decoded_path.clone());
                        } else {
                            self.state.ssh_session = Some(SshSession {
                                destination: host.to_string(),
                                args: vec![host.to_string()],
                                remote_dir: Some(decoded_path.clone()),
                                is_active: true,
                            });
                        }
                    }
                    let trimmed = decoded_path.strip_prefix('/').unwrap_or(&decoded_path);
                    if !trimmed.is_empty() {
                        self.state.current_dir = Some(std::path::PathBuf::from(trimmed));
                    }
                }
            }
            // Notificación de CWD ConEmu / Windows Terminal (OSC 9;9;"path")
            b"9" if params.len() > 1 => {
                if params[1] == b"9" && params.len() > 2 {
                    let raw = String::from_utf8_lossy(params[2]);
                    let clean = raw.trim().trim_matches('"');
                    if !clean.is_empty() {
                        self.state.current_dir = Some(std::path::PathBuf::from(clean));
                    }
                }
            }
            _ => {}
        }
    }
}

impl<'a> Performer<'a> {
    fn handle_sgr(&mut self, params: &Params) {
        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            let val = param.first().copied().unwrap_or(0);
            match val {
                0 => {
                    self.state.current_fg = self.state.theme.foreground;
                    self.state.current_bg = Color::TRANSPARENT;
                    self.state.current_bold = false;
                    self.state.current_dim = false;
                    self.state.current_italic = false;
                    self.state.current_underline = false;
                    self.state.current_strikethrough = false;
                    self.state.current_inverse = false;
                }
                1 => self.state.current_bold = true,
                2 => self.state.current_dim = true,
                3 => self.state.current_italic = true,
                4 => self.state.current_underline = true,
                7 => self.state.current_inverse = true,
                9 => self.state.current_strikethrough = true,
                22 => {
                    self.state.current_bold = false;
                    self.state.current_dim = false;
                }
                23 => self.state.current_italic = false,
                24 => self.state.current_underline = false,
                27 => self.state.current_inverse = false,
                29 => self.state.current_strikethrough = false,

                // Foreground estándar ANSI (30..=37)
                30..=37 => {
                    let idx = (val - 30) as usize;
                    self.state.current_fg = self.state.theme.ansi[idx];
                }
                // TrueColor & 256 Colors Foreground (38;2;r;g;b / 38:2:r:g:b / 38;5;n / 38:5:n)
                38 => {
                    if let Some(c) = parse_sgr_color(param, &mut iter, &self.state.theme) {
                        self.state.current_fg = c;
                    }
                }
                39 => self.state.current_fg = self.state.theme.foreground,

                // Background estándar ANSI (40..=47)
                40..=47 => {
                    let idx = (val - 40) as usize;
                    self.state.current_bg = self.state.theme.ansi[idx];
                }
                // TrueColor & 256 Colors Background (48;2;r;g;b / 48:2:r:g:b / 48;5;n / 48:5:n)
                48 => {
                    if let Some(c) = parse_sgr_color(param, &mut iter, &self.state.theme) {
                        self.state.current_bg = c;
                    }
                }
                49 => self.state.current_bg = Color::TRANSPARENT,

                // Foreground brillante (90..=97)
                90..=97 => {
                    let idx = (val - 90 + 8) as usize;
                    self.state.current_fg = self.state.theme.ansi[idx];
                }
                // Background brillante (100..=107)
                100..=107 => {
                    let idx = (val - 100 + 8) as usize;
                    self.state.current_bg = self.state.theme.ansi[idx];
                }
                _ => {}
            }
        }
    }
}

fn parse_sgr_color<'a, I>(param: &[u16], iter: &mut I, theme: &Theme) -> Option<Color>
where
    I: Iterator<Item = &'a [u16]>,
{
    // Formato con subparámetros en el mismo grupo delimitados por dos puntos:
    // ej: [38, 2, r, g, b] o [38, 2, cs, r, g, b] o [38, 5, idx]
    if param.len() > 1 {
        let mode = param[1];
        if mode == 5 && param.len() >= 3 {
            return Some(theme.resolve_256_color(param[2] as u8));
        } else if mode == 2 {
            if param.len() >= 6 {
                // Formato ISO: 38:2:colorspace:r:g:b
                return Some(Color::from_rgb(
                    param[3] as u8,
                    param[4] as u8,
                    param[5] as u8,
                ));
            } else if param.len() >= 5 {
                // Formato corto: 38:2:r:g:b
                return Some(Color::from_rgb(
                    param[2] as u8,
                    param[3] as u8,
                    param[4] as u8,
                ));
            }
        }
        return None;
    }

    // Formato estándar delimitado por punto y coma (38;2;r;g;b o 38;5;idx)
    if let Some(next_param) = iter.next() {
        let mode = next_param.first().copied().unwrap_or(0);
        if mode == 5 {
            if let Some(idx_p) = iter.next() {
                let idx = idx_p.first().copied().unwrap_or(0) as u8;
                return Some(theme.resolve_256_color(idx));
            }
        } else if mode == 2 {
            let r = iter.next().and_then(|p| p.first()).copied().unwrap_or(0) as u8;
            let g = iter.next().and_then(|p| p.first()).copied().unwrap_or(0) as u8;
            let b = iter.next().and_then(|p| p.first()).copied().unwrap_or(0) as u8;
            return Some(Color::from_rgb(r, g, b));
        }
    }
    None
}

impl<'a> Performer<'a> {
    fn handle_iterm2_image(&mut self, params: &[&[u8]]) {
        // Concatenar parámetros del comando OSC
        let mut full_payload = Vec::new();
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                full_payload.push(b';');
            }
            full_payload.extend_from_slice(p);
        }

        // Buscar prefijo "File="
        if let Some(pos) = full_payload.windows(5).position(|w| w == b"File=") {
            let file_data = &full_payload[pos + 5..];
            // Separador ':' entre argumentos y los bytes en base64 de la imagen
            if let Some(colon_pos) = file_data.iter().position(|&b| b == b':') {
                let args_bytes = &file_data[..colon_pos];
                let b64_bytes = &file_data[colon_pos + 1..];

                let args_str = String::from_utf8_lossy(args_bytes);
                if !args_str.contains("inline=1") {
                    return;
                }

                // Filtrar espacios y saltos de línea de la cadena base64
                let clean_b64: Vec<u8> = b64_bytes
                    .iter()
                    .copied()
                    .filter(|b| !b.is_ascii_whitespace())
                    .collect();

                use base64::prelude::*;
                if let Ok(decoded) = BASE64_STANDARD.decode(&clean_b64) {
                    if let Ok(dyn_img) = image::load_from_memory(&decoded) {
                        let rgba = dyn_img.to_rgba8();
                        let (w, h) = rgba.dimensions();

                        // Dimensiones en celdas de terminal (aprox. 8px ancho, 18px alto por celda)
                        let max_cols = (self.state.cols.saturating_sub(self.state.cursor_x + 2))
                            .min(80)
                            .max(10);
                        let cols = ((w as f32 / 8.0).round() as usize).clamp(10, max_cols);
                        let aspect = h as f32 / w as f32;
                        let rows = ((cols as f32 * (8.0 / 18.0) * aspect).round() as usize)
                            .max(3)
                            .min(30);

                        let img_id = self.state.next_image_id;
                        self.state.next_image_id += 1;

                        let img_entry = InlineImage {
                            id: img_id,
                            line_index: self.state.total_lines_count + self.state.cursor_y,
                            col: self.state.cursor_x,
                            width_cols: cols,
                            height_rows: rows,
                            width_px: w,
                            height_px: h,
                            rgba_data: rgba.into_raw(),
                        };

                        self.state.images.push(img_entry);

                        // Dejar espacio vertical en el grid para que el texto siguiente continúe abajo
                        for _ in 0..rows {
                            self.state.newline();
                        }
                    }
                }
            }
        }
    }

    fn handle_osc_52(&mut self, params: &[&[u8]]) {
        let mut full = Vec::new();
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                full.push(b';');
            }
            full.extend_from_slice(p);
        }

        let parts: Vec<&[u8]> = full.splitn(2, |&b| b == b';').collect();
        if parts.len() == 2 {
            let b64_payload = parts[1];
            if b64_payload == b"?" {
                // Consulta de lectura remota OSC 52 (bloqueada por seguridad)
                return;
            }

            let clean_b64: Vec<u8> = b64_payload
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_whitespace())
                .collect();

            use base64::prelude::*;
            if let Ok(decoded_bytes) = BASE64_STANDARD.decode(&clean_b64) {
                if let Ok(text) = String::from_utf8(decoded_bytes) {
                    self.state.clipboard_queue.push(text);
                }
            }
        }
    }

    fn handle_osc_133(&mut self, params: &[&[u8]]) {
        if params.is_empty() {
            return;
        }
        let code = params[0].first().copied().unwrap_or(0);
        let cur_line = self.state.current_global_line();

        match code {
            b'A' => {
                // Prompt Start (OSC 133;A)
                if let Some(mut prev) = self.state.active_block.take() {
                    if prev.output_end.is_none() {
                        prev.output_end = Some(cur_line.saturating_sub(1));
                    }
                    self.state.command_blocks.push(prev);
                }
                self.state.active_block = Some(CommandBlock {
                    prompt_line: cur_line,
                    command_line: None,
                    output_start: None,
                    output_end: None,
                    exit_code: None,
                });
            }
            b'B' => {
                // Command Start (OSC 133;B)
                if let Some(block) = &mut self.state.active_block {
                    block.command_line = Some(cur_line);
                }
            }
            b'C' => {
                // Output Start (OSC 133;C)
                if let Some(block) = &mut self.state.active_block {
                    block.output_start = Some(cur_line);
                }
            }
            b'D' => {
                // Command Finished (OSC 133;D[;exit_code])
                let exit_code = if params.len() > 1 {
                    String::from_utf8_lossy(params[1])
                        .trim()
                        .parse::<i32>()
                        .ok()
                } else {
                    None
                };
                if let Some(mut block) = self.state.active_block.take() {
                    block.output_end = Some(cur_line);
                    block.exit_code = exit_code;
                    self.state.command_blocks.push(block);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_ghostty_palette() {
        let theme = Theme::ghostty_dark();
        assert_eq!(theme.background, Color::from_rgb(22, 22, 24));
        assert_eq!(theme.cursor, Color::from_rgb(255, 255, 255));
        assert_eq!(theme.ansi.len(), 16);
    }

    #[test]
    fn test_truecolor_parsing() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);
        // Secuencia TrueColor: RGB(120, 200, 250)
        let seq = b"\x1b[38;2;120;200;250mHola TrueColor\x1b[0m";
        term.process_bytes(seq);

        let cell = &term.grid[0][0];
        assert_eq!(cell.c, 'H');
        assert_eq!(cell.fg, Color::from_rgb(120, 200, 250));
    }

    #[test]
    fn test_cursor_and_clear_screen() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Escribir en posición (10, 5)
        term.process_bytes(b"\x1b[5;10HTest");
        assert_eq!(term.cursor_y, 4);
        assert_eq!(term.cursor_x, 13);

        // Limpiar pantalla
        term.process_bytes(b"\x1b[2J");
        assert_eq!(term.cursor_x, 0);
        assert_eq!(term.cursor_y, 0);
        assert_eq!(term.grid[0][0].c, ' ');
    }

    #[test]
    fn test_scrollback_retention() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 10, 500, theme);

        // Insertar 50 líneas (más que el tamaño de pantalla)
        for i in 0..50 {
            let line = format!("Línea de log {}\r\n", i);
            term.process_bytes(line.as_bytes());
        }

        assert_eq!(term.scrollback.len(), 41);
        term.scroll_up(20);
        assert_eq!(term.scroll_offset, 20);
        term.reset_scroll();
        assert_eq!(term.scroll_offset, 0);
    }

    #[test]
    fn bench_throughput_massive_stream() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(120, 40, 50_000, theme);

        // Generar un bloque representativo de logs de agentes/compilador con colores ANSI
        let sample = b"\x1b[32m[INFO]\x1b[0m \x1b[1mClaude/Agy Agent:\x1b[0m Analizando archivo \x1b[36msrc/main.rs\x1b[0m [OK]\r\n";
        let iterations = 20_000;
        let total_bytes = sample.len() * iterations;

        let start = Instant::now();
        for _ in 0..iterations {
            term.process_bytes(sample);
        }
        let elapsed = start.elapsed();

        let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
        let lines_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "\n⚡ [Axon Benchmark] Procesados {} bytes ({} líneas) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
            total_bytes, iterations, elapsed, throughput_mb, lines_per_sec
        );

        assert!(
            throughput_mb > 3.0,
            "El throughput debe superar los 3 MB/s en debug y decenas de MB/s en release"
        );
    }

    #[test]
    fn test_iterm2_image_protocol() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // PNG 1x1 codificado en Base64
        let b64_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let osc_command = format!("\x1b]1337;File=inline=1:{}\x07", b64_png);

        term.process_bytes(osc_command.as_bytes());

        assert_eq!(
            term.images.len(),
            1,
            "Debe decodificar y registrar exactamente una imagen inline"
        );
        let img = &term.images[0];
        assert_eq!(img.width_px, 1);
        assert_eq!(img.height_px, 1);
        assert_eq!(img.width_cols >= 5, true);
        assert_eq!(img.height_rows >= 2, true);
    }

    #[test]
    fn test_alt_screen_buffer_tmux() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // 1. Escribir en buffer principal
        term.process_bytes(b"Comando normal en PowerShell\r\n");
        assert_eq!(term.scrollback.len(), 0);
        assert_eq!(term.is_alt_screen, false);

        // 2. Entrar en modo Alternate Screen (CSI ? 1049 h usado por tmux/btop/vim)
        term.process_bytes(b"\x1b[?1049h");
        assert_eq!(term.is_alt_screen, true);

        // 3. Escribir datos dentro de tmux / btop
        for i in 0..30 {
            let log = format!("Panel tmux fila {}\r\n", i);
            term.process_bytes(log.as_bytes());
        }

        // El scrollback principal NO debe ser contaminado por la pantalla alterna
        assert_eq!(
            term.scrollback.len(),
            0,
            "Alternate screen buffer no debe contaminar el scrollback principal"
        );

        // 4. Salir de tmux (CSI ? 1049 l)
        term.process_bytes(b"\x1b[?1049l");
        assert_eq!(
            term.is_alt_screen, false,
            "Debe restaurar el buffer principal"
        );
    }

    #[test]
    fn test_decstbm_tmux_split_margins() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 20, 1000, theme);

        // Definir margen de desplazamiento de la línea 5 a la 10 (split pane de tmux)
        term.process_bytes(b"\x1b[5;10r");
        assert_eq!(term.scroll_region_top, 4);
        assert_eq!(term.scroll_region_bottom, 9);
    }

    #[test]
    fn bench_massive_scrollback_100k_lines() {
        let theme = Theme::ghostty_dark();
        // 100.000 líneas de scrollback en memoria
        let mut term = TerminalState::new(120, 40, 100_000, theme);

        let line = b"[PyTorch/vLLM INFO] Step 12450/50000 | Loss: 0.0412 | GPU-0: 98% VRAM: 22.4/24.0 GB | 185 tokens/sec\r\n";
        let count = 100_000;
        let total_bytes = line.len() * count;

        let start = Instant::now();
        for _ in 0..count {
            term.process_bytes(line);
        }
        let elapsed = start.elapsed();

        let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
        let lines_per_sec = count as f64 / elapsed.as_secs_f64();

        println!(
            "\n🚀 [Axon Massive Stream Test] Procesadas {} líneas ({} MB) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
            count, (total_bytes / (1024 * 1024)), elapsed, throughput_mb, lines_per_sec
        );

        assert_eq!(
            term.scrollback.len() + term.rows - 1,
            count,
            "Debe retener las 100.000 líneas entre scrollback y pantalla activa"
        );
        assert!(
            lines_per_sec > 50_000.0,
            "Debe superar las 50.000 líneas por segundo en debug y >500.000 en release"
        );
    }

    #[test]
    fn test_osc_52_clipboard() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Enviar secuencia OSC 52 con texto "Hello from SSH!" en base64 (SGVsbG8gZnJvbSBTU0gh)
        term.process_bytes(b"\x1b]52;c;SGVsbG8gZnJvbSBTU0gh\x07");

        assert_eq!(
            term.clipboard_queue.len(),
            1,
            "Debe agregar el texto decodificado a la cola de portapapeles"
        );
        assert_eq!(term.clipboard_queue[0], "Hello from SSH!");
    }

    #[test]
    fn test_osc_133_semantic_prompts() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Simulación de secuencia Shell Integration OSC 133
        // 1. Inicio de prompt (A)
        term.process_bytes(b"\x1b]133;A\x07");
        // 2. Inicio de comando (B)
        term.process_bytes(b"\x1b]133;B\x07git status\r\n");
        // 3. Inicio de salida (C)
        term.process_bytes(b"\x1b]133;C\x07On branch main\r\nnothing to commit\r\n");
        // 4. Fin de comando con exit code 0 (D;0)
        term.process_bytes(b"\x1b]133;D;0\x07");

        assert_eq!(
            term.command_blocks.len(),
            1,
            "Debe registrar un bloque de comando semántico"
        );
        let block = &term.command_blocks[0];
        assert_eq!(block.exit_code, Some(0), "El código de salida debe ser 0");

        let last_output = term.get_last_output_text();
        assert!(
            last_output.is_some(),
            "Debe poder extraer la salida del comando sin mouse"
        );
        assert!(last_output.unwrap().contains("On branch main"));
    }

    #[test]
    fn test_bracketed_paste_mode() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        assert_eq!(term.bracketed_paste_mode, false);
        // Activar modo bracketed paste (?2004h)
        term.process_bytes(b"\x1b[?2004h");
        assert_eq!(term.bracketed_paste_mode, true);
        // Desactivar modo bracketed paste (?2004l)
        term.process_bytes(b"\x1b[?2004l");
        assert_eq!(term.bracketed_paste_mode, false);
    }

    #[test]
    fn test_powershell_full_startup_stream() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);
        let raw = b"\x1b[?9001h\x1b[?1004h\x1b[?25l\x1b[2J\x1b[m\x1b[HWindows PowerShell\r\nCopyright (C) Microsoft Corporation. All rights reserved.\x1b]0;C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\x07\x1b[?25h\x1b[?25l\x1b[4;1H\x1b[?25hPS C:\\Users\\gmora> ";
        term.process_bytes(raw);

        for (i, row) in term.grid.iter().enumerate() {
            let s: String = row.iter().map(|c| c.c).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                println!(
                    "ROW {}: {:?} (cell[0].fg: {:?}, bg: {:?})",
                    i, trimmed, row[0].fg, row[0].bg
                );
            }
        }
        println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);

        // Simular redimensionamiento de ventana al inicio (de 24 a 20 filas)
        println!("--- AFTER RESIZE to 20 rows ---");
        term.resize(80, 20);
        for (i, row) in term.grid.iter().enumerate() {
            let s: String = row.iter().map(|c| c.c).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                println!("GRID ROW {}: {:?}", i, trimmed);
            }
        }
        for (i, row) in term.scrollback.iter().enumerate() {
            let s: String = row.iter().map(|c| c.c).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                println!("SCROLLBACK ROW {}: {:?}", i, trimmed);
            }
        }
        println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);
        assert!(term.cursor_x > 0);
    }

    #[test]
    fn test_unicode_double_width_emojis_and_cjk() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Imprimir emoji robot (🤖) y texto "AI"
        let text = "🤖AI";
        term.process_bytes(text.as_bytes());

        // El emoji '🤖' ocupa 2 columnas:
        // Columna 0: carácter '🤖' con width = 2
        // Columna 1: celda de continuación espaciadora con width = 0
        assert_eq!(term.grid[0][0].c, '🤖');
        assert_eq!(term.grid[0][0].width, 2);
        assert_eq!(term.grid[0][1].width, 0);

        // Columna 2: 'A' con width = 1
        assert_eq!(term.grid[0][2].c, 'A');
        assert_eq!(term.grid[0][2].width, 1);

        // Columna 3: 'I' con width = 1
        assert_eq!(term.grid[0][3].c, 'I');
        assert_eq!(term.grid[0][3].width, 1);

        // El cursor debe estar en la columna 4
        assert_eq!(term.cursor_x, 4);
    }

    #[test]
    fn test_unicode_box_drawing_and_braille_patterns() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Caracteres de diagramación (Box Drawing) y patrones Braille (utilizados por btop / spinners)
        let tui_chars = "┌─┐⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
        term.process_bytes(tui_chars.as_bytes());

        assert_eq!(term.grid[0][0].c, '┌');
        assert_eq!(term.grid[0][1].c, '─');
        assert_eq!(term.grid[0][2].c, '┐');
        assert_eq!(term.grid[0][3].c, '⠋');
        assert_eq!(term.grid[0][4].c, '⠙');
        assert_eq!(term.cursor_x, 13);
    }

    #[test]
    fn test_truecolor_24bit_rgb_semicolon_and_colon() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // 1. TrueColor formato punto y coma estándar (38;2;R;G;B y 48;2;R;G;B)
        term.process_bytes(b"\x1b[38;2;250;120;40;48;2;15;25;35mX\x1b[0m");
        let cell_x = &term.grid[0][0];
        assert_eq!(cell_x.c, 'X');
        assert_eq!(cell_x.fg, Color::from_rgb(250, 120, 40));
        assert_eq!(cell_x.bg, Color::from_rgb(15, 25, 35));

        // 2. TrueColor formato delimitado por dos puntos (38:2:R:G:B)
        term.process_bytes(b"\x1b[38:2:180:90:220mY\x1b[0m");
        let cell_y = &term.grid[0][1];
        assert_eq!(cell_y.c, 'Y');
        assert_eq!(cell_y.fg, Color::from_rgb(180, 90, 220));

        // 3. TrueColor formato ISO con espacio de color (38:2:0:70:140:210)
        term.process_bytes(b"\x1b[38:2::70:140:210mZ\x1b[0m");
        let cell_z = &term.grid[0][2];
        assert_eq!(cell_z.c, 'Z');
        assert_eq!(cell_z.fg, Color::from_rgb(70, 140, 210));
    }

    #[test]
    fn test_sgr_text_styles_dim_bold_underline_strikethrough() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Estilos combinados: 1 (bold), 2 (dim), 4 (underline), 9 (strikethrough)
        term.process_bytes(b"\x1b[1;2;4;9mA\x1b[0mB");

        let cell_a = &term.grid[0][0];
        assert_eq!(cell_a.c, 'A');
        assert!(cell_a.bold);
        assert!(cell_a.dim);
        assert!(cell_a.underline);
        assert!(cell_a.strikethrough);

        // Tras el reset (SGR 0), 'B' debe tener estilos limpios
        let cell_b = &term.grid[0][1];
        assert_eq!(cell_b.c, 'B');
        assert!(!cell_b.bold);
        assert!(!cell_b.dim);
        assert!(!cell_b.underline);
        assert!(!cell_b.strikethrough);
    }

    #[test]
    fn test_tui_mouse_and_sync_protocols() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        assert!(!term.is_mouse_reporting_active());
        assert!(!term.mouse_reporting_sgr);
        assert!(!term.focus_event_reporting);
        assert!(!term.sync_update_mode);

        // Herdr / Ratatui activa mouse tracking (1000, 1002, 1006) y focus (1004)
        term.process_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[?2026h");
        assert!(term.is_mouse_reporting_active());
        assert!(term.mouse_reporting_normal);
        assert!(term.mouse_reporting_button_event);
        assert!(term.mouse_reporting_sgr);
        assert!(term.focus_event_reporting);
        assert!(term.sync_update_mode);

        // Desactivación al salir del TUI
        term.process_bytes(b"\x1b[?1000l\x1b[?1002l\x1b[?1006l\x1b[?1004l\x1b[?2026l");
        assert!(!term.is_mouse_reporting_active());
        assert!(!term.mouse_reporting_sgr);
        assert!(!term.focus_event_reporting);
        assert!(!term.sync_update_mode);
    }

    #[test]
    fn test_pwsh_startup_prompt() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);
        let raw = b"\x1b[?9001h\x1b[?1004h\x1b[2J\x1b[?25l\x1b[2J\x1b[m\x1b[H\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[H\x1b]0;pwsh.exe\x07\x1b[?25h\x1b[38;2;78;146;249m\xef\x81\xbb ~\x1b[38;2;81;226;122m\r\n\xef\x90\xb2 ";
        term.process_bytes(raw);
        for (i, row) in term.grid.iter().enumerate() {
            let s: String = row.iter().map(|c| c.c).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                println!("ROW {}: {:?}", i, trimmed);
            }
        }
        println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);
        assert!(term.cursor_x > 0);
    }

    #[test]
    fn test_ech_erase_characters() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Escribir "hello world"
        term.process_bytes(b"hello world");
        assert_eq!(term.grid[0][0].c, 'h');
        assert_eq!(term.grid[0][6].c, 'w');

        // Mover cursor a columna 6 (índice 5: ' ') y borrar 6 caracteres con ECH (\x1b[6X)
        term.process_bytes(b"\x1b[1;6H\x1b[6X");
        assert_eq!(term.cursor_x, 5); // El cursor no debe moverse
        assert_eq!(term.grid[0][0].c, 'h');
        assert_eq!(term.grid[0][4].c, 'o');
        assert_eq!(term.grid[0][5].c, ' ');
        assert_eq!(term.grid[0][6].c, ' ');
        assert_eq!(term.grid[0][10].c, ' ');
    }

    #[test]
    fn test_bce_erase_in_line() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Establecer color de fondo azul (44) y borrar línea con \x1b[2K
        term.process_bytes(b"\x1b[44m\x1b[2K");
        assert_eq!(term.grid[0][0].bg, term.theme.ansi[4]);
        assert_eq!(term.grid[0][79].bg, term.theme.ansi[4]);
    }

    #[test]
    fn test_osc_cwd_reporting() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Test OSC 7
        term.process_bytes(b"\x1b]7;file://localhost/C:/Users/gmora/dev/axon\x07");
        assert_eq!(
            term.current_dir.as_deref(),
            Some(std::path::Path::new("C:/Users/gmora/dev/axon"))
        );

        // Test OSC 9;9
        term.process_bytes(b"\x1b]9;9;\"C:\\Users\\gmora\\dev\"\x07");
        assert_eq!(
            term.current_dir.as_deref(),
            Some(std::path::Path::new("C:\\Users\\gmora\\dev"))
        );
    }

    #[test]
    fn test_osc_window_title_and_ssh_inference() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Envío de título de ventana tradicional OSC 0
        term.process_bytes(b"\x1b]0;My Custom Window\x07");
        assert_eq!(term.window_title.as_deref(), Some("My Custom Window"));
        assert!(term.ssh_session.is_none());

        // Inferencia de sesión SSH mediante título "ssh: user@host"
        term.process_bytes(b"\x1b]0;ssh: dev@server.example.com\x07");
        assert_eq!(
            term.window_title.as_deref(),
            Some("ssh: dev@server.example.com")
        );
        let sess = term
            .ssh_session
            .as_ref()
            .expect("Debe inferir la sesión SSH desde el título");
        assert_eq!(sess.destination, "dev@server.example.com");
        assert!(sess.is_active);
    }

    #[test]
    fn test_osc_axon_ssh_signaling() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // Inicio de sesión SSH mediante protocolo OSC 1337 AxonSSH=start
        term.process_bytes(b"\x1b]1337;AxonSSH=start;dest=ubuntu@production-node-1\x07");
        let sess = term
            .ssh_session
            .as_ref()
            .expect("Debe registrar la sesión SSH activa");
        assert_eq!(sess.destination, "ubuntu@production-node-1");
        assert!(sess.is_active);

        // Finalización de sesión SSH mediante OSC 1337 AxonSSH=end
        term.process_bytes(b"\x1b]1337;AxonSSH=end\x07");
        assert!(
            term.ssh_session.is_none(),
            "Debe limpiar la sesión SSH al desconectarse"
        );
    }

    #[test]
    fn test_osc_7_remote_host_ssh() {
        let theme = Theme::ghostty_dark();
        let mut term = TerminalState::new(80, 24, 1000, theme);

        // OSC 7 emitido por una shell remota con hostname no local
        term.process_bytes(b"\x1b]7;file://bastion.corp.internal/home/admin/projects\x07");
        let sess = term
            .ssh_session
            .as_ref()
            .expect("Debe detectar host remoto vía OSC 7");
        assert_eq!(sess.destination, "bastion.corp.internal");
        assert_eq!(sess.remote_dir.as_deref(), Some("/home/admin/projects"));
        assert!(sess.is_active);
    }
}
