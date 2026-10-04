use egui::Color32;
use vte::{Params, Parser, Perform};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Cell {
    pub c: char,
    pub fg: Color32,
    pub bg: Color32,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color32::from_rgb(220, 220, 225),
            bg: Color32::TRANSPARENT,
            bold: false,
        }
    }
}

pub struct TerminalState {
    pub cols: usize,
    pub rows: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub grid: Vec<Vec<Cell>>,
    pub scrollback: Vec<Vec<Cell>>,
    pub max_scrollback: usize,
    current_fg: Color32,
    current_bg: Color32,
    current_bold: bool,
    parser: Option<Parser>,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize, max_scrollback: usize) -> Self {
        let grid = vec![vec![Cell::default(); cols]; rows];
        Self {
            cols,
            rows,
            cursor_x: 0,
            cursor_y: 0,
            grid,
            scrollback: Vec::with_capacity(max_scrollback),
            max_scrollback,
            current_fg: Color32::from_rgb(225, 230, 240),
            current_bg: Color32::TRANSPARENT,
            current_bold: false,
            parser: Some(Parser::new()),
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
        if self.cursor_y + 1 >= self.rows {
            // Empujar fila superior al scrollback
            let top_row = self.grid.remove(0);
            if self.scrollback.len() >= self.max_scrollback {
                self.scrollback.remove(0);
            }
            self.scrollback.push(top_row);
            self.grid.push(vec![Cell::default(); self.cols]);
        } else {
            self.cursor_y += 1;
        }
    }

    pub fn put_char(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }

        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            self.grid[self.cursor_y][self.cursor_x] = Cell {
                c,
                fg: self.current_fg,
                bg: self.current_bg,
                bold: self.current_bold,
            };
            self.cursor_x += 1;
        }
    }

    pub fn get_recent_text(&self, max_lines: usize) -> String {
        let mut lines = Vec::new();

        // Agregar desde scrollback
        let scroll_take = max_lines.saturating_sub(self.rows);
        if scroll_take > 0 && !self.scrollback.is_empty() {
            let start = self.scrollback.len().saturating_sub(scroll_take);
            for row in &self.scrollback[start..] {
                let s: String = row.iter().map(|cell| cell.c).collect();
                lines.push(s.trim_end().to_string());
            }
        }

        // Agregar de la pantalla activa
        for row in &self.grid {
            let s: String = row.iter().map(|cell| cell.c).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }

        lines.join("\n")
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
                // Backspace
                if self.state.cursor_x > 0 {
                    self.state.cursor_x -= 1;
                }
            }
            b'\t' => {
                // Tabulación (cada 4 o 8 columnas)
                let next_tab = (self.state.cursor_x + 8) & !7;
                self.state.cursor_x = next_tab.min(self.state.cols.saturating_sub(1));
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        match action {
            // Mover cursor arriba (CUU)
            'A' => {
                let n = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.state.cursor_y = self.state.cursor_y.saturating_sub(n.max(1));
            }
            // Mover cursor abajo (CUD)
            'B' => {
                let n = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.state.cursor_y = (self.state.cursor_y + n.max(1)).min(self.state.rows.saturating_sub(1));
            }
            // Mover cursor adelante (CUF)
            'C' => {
                let n = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.state.cursor_x = (self.state.cursor_x + n.max(1)).min(self.state.cols.saturating_sub(1));
            }
            // Mover cursor atrás (CUB)
            'D' => {
                let n = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.state.cursor_x = self.state.cursor_x.saturating_sub(n.max(1));
            }
            // Posicionar cursor (CUP / HVP)
            'H' | 'f' => {
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.first()).copied().unwrap_or(1).saturating_sub(1) as usize;
                let col = iter.next().and_then(|p| p.first()).copied().unwrap_or(1).saturating_sub(1) as usize;
                self.state.cursor_y = row.min(self.state.rows.saturating_sub(1));
                self.state.cursor_x = col.min(self.state.cols.saturating_sub(1));
            }
            // Borrar en pantalla (ED)
            'J' => {
                let mode = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(0);
                if mode == 2 || mode == 3 {
                    for row in self.state.grid.iter_mut() {
                        for cell in row.iter_mut() {
                            *cell = Cell::default();
                        }
                    }
                    self.state.cursor_x = 0;
                    self.state.cursor_y = 0;
                }
            }
            // Borrar en línea (EL)
            'K' => {
                let mode = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(0);
                let cy = self.state.cursor_y;
                let cx = self.state.cursor_x;
                if cy < self.state.rows {
                    match mode {
                        0 => {
                            // Desde cursor hasta el final
                            for x in cx..self.state.cols {
                                self.state.grid[cy][x] = Cell::default();
                            }
                        }
                        1 => {
                            // Desde inicio hasta el cursor
                            for x in 0..=cx.min(self.state.cols.saturating_sub(1)) {
                                self.state.grid[cy][x] = Cell::default();
                            }
                        }
                        2 => {
                            // Toda la línea
                            for x in 0..self.state.cols {
                                self.state.grid[cy][x] = Cell::default();
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Select Graphic Rendition (SGR - Colores y Estilos)
            'm' => {
                self.handle_sgr(params);
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
                    self.state.current_fg = Color32::from_rgb(225, 230, 240);
                    self.state.current_bg = Color32::TRANSPARENT;
                    self.state.current_bold = false;
                }
                1 => self.state.current_bold = true,
                22 => self.state.current_bold = false,
                // Colores estándar ANSI FG
                30 => self.state.current_fg = Color32::from_rgb(40, 44, 52),     // Negro
                31 => self.state.current_fg = Color32::from_rgb(235, 95, 95),    // Rojo
                32 => self.state.current_fg = Color32::from_rgb(90, 215, 130),   // Verde
                33 => self.state.current_fg = Color32::from_rgb(245, 200, 75),   // Amarillo
                34 => self.state.current_fg = Color32::from_rgb(90, 160, 250),   // Azul
                35 => self.state.current_fg = Color32::from_rgb(200, 120, 245),  // Magenta
                36 => self.state.current_fg = Color32::from_rgb(90, 220, 220),   // Cian
                37 => self.state.current_fg = Color32::from_rgb(220, 220, 225),  // Blanco
                39 => self.state.current_fg = Color32::from_rgb(225, 230, 240),  // Reset FG
                // Colores brillantes ANSI FG
                90 => self.state.current_fg = Color32::from_rgb(90, 95, 110),
                91 => self.state.current_fg = Color32::from_rgb(255, 115, 115),
                92 => self.state.current_fg = Color32::from_rgb(110, 240, 150),
                93 => self.state.current_fg = Color32::from_rgb(255, 220, 100),
                94 => self.state.current_fg = Color32::from_rgb(115, 185, 255),
                95 => self.state.current_fg = Color32::from_rgb(225, 145, 255),
                96 => self.state.current_fg = Color32::from_rgb(115, 245, 245),
                97 => self.state.current_fg = Color32::WHITE,
                _ => {}
            }
        }
    }
}
