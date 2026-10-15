//! Emulador de terminal: estado (`TerminalState`), rejilla con scrollback y manejo de OSC (133, 7, 52, 1337).
//!
//! `performer` traduce las secuencias VT/ANSI de `vte` en cambios de este estado.

mod cell;
pub(crate) mod performer;
pub(crate) use performer::clean_listing;

pub use cell::*;
use performer::Performer;

use crate::theme::{Color, Theme};
use std::collections::VecDeque;
use vte::{Params, Parser, Perform};

/// Respuesta a una petición en banda al bash remoto (leer/escribir archivo, listar carpeta)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFileResult {
    pub id: u32,
    pub ok: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RemoteFileAsm {
    id: u32,
    ok: bool,
    parts: Vec<Option<String>>,
}

pub struct TerminalState {
    pub cols: usize,
    pub rows: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub grid: VecDeque<Vec<Cell>>,
    pub scrollback: VecDeque<Vec<Cell>>,
    pub max_scrollback: usize,
    pub scroll_offset: usize,

    // Alternate Screen Buffer (Soporte nativo para tmux, btop, nvtop, vim)
    pub is_alt_screen: bool,
    pub alt_grid: VecDeque<Vec<Cell>>,
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
    /// Token secreto del panel (`PtyManager::token`, variable `AXON_PANE_TOKEN` del shell): autentica los
    /// marcadores `AxonSSH=start` que emite el wrapper `ssh` de `init.ps1`. Vacío = ningún marcador es válido.
    pub pane_token: String,
    /// El wrapper de agentes de `init.ps1` avisó (OSC 1337 AxonAgent) de que hay un agente en marcha
    pub agent_running: bool,
    pub window_title: Option<String>,
    /// El bash remoto lanzado por el wrapper `ssh` de Axon informa de rama git (OSC 1337 AxonGit) y listado
    pub remote_integration: bool,
    /// Entradas de la carpeta remota actual (`ls -1Ap`, las carpetas acaban en `/`), no confiables
    pub remote_listing: Option<Vec<String>>,
    /// El prompt remoto está esperando entrada (llega `AxonGit` en cada prompt; Enter lo apaga)
    pub remote_at_prompt: bool,
    /// Respuestas completas a peticiones en banda (`AxonFile`), pendientes de que la app las recoja
    pub remote_file_results: Vec<RemoteFileResult>,
    remote_file_parts: Vec<RemoteFileAsm>,

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

    // Hover del scrollbar del panel
    pub scrollbar_hovered: bool,

    parser: Option<Parser>,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize, max_scrollback: usize, theme: Theme) -> Self {
        let default_cell = Cell::default_with_theme(&theme);
        let grid = (0..rows).map(|_| vec![default_cell; cols]).collect();
        let alt_grid = VecDeque::new();
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
            pane_token: String::new(),
            agent_running: false,
            window_title: None,
            remote_integration: false,
            remote_listing: None,
            remote_at_prompt: false,
            remote_file_results: Vec::new(),
            remote_file_parts: Vec::new(),
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
            scrollbar_hovered: false,
            parser: Some(Parser::new()),
        }
    }

    /// El marcador trae el token de este panel (comparación en tiempo constante). Un token vacío nunca vale.
    pub fn pane_token_matches(&self, token: &str) -> bool {
        let (a, b) = (self.pane_token.as_bytes(), token.as_bytes());
        !a.is_empty()
            && a.len() == b.len()
            && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
    }

    /// Bytes que Axon escribe en el bash remoto para pedir algo sin abrir otra conexión: escribe la
    /// petición tras `#AXR#` en la línea de edición y pulsa una combinación que el gancho de Axon
    /// tiene enlazada (`bind -x`); el gancho borra la petición y responde con `OSC 1337 AxonFile`.
    /// Solo con integración activa y el prompt esperando entrada (si no, los bytes irían a otro programa).
    pub fn remote_request_bytes(&self, op: &str, id: u32, payload: &str) -> Option<Vec<u8>> {
        let safe = |s: &str, extra: &[char]| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
        };
        if !self.remote_integration
            || !self.remote_at_prompt
            || !matches!(op, "get" | "put" | "ls")
            || !safe(payload, &['%', '-', '_', '.', '~', '/', ' '])
        {
            return None;
        }
        Some(format!("\x05#AXR#{id} {op} {payload}\x1b[1337~").into_bytes())
    }

    /// El usuario pulsó Enter: lo que se escriba ya no llega a un prompt sino a un programa
    pub fn note_enter_sent(&mut self) {
        self.remote_at_prompt = false;
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

    pub fn active_grid_mut(&mut self) -> &mut VecDeque<Vec<Cell>> {
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
            if self.alt_grid.len() != self.rows {
                self.alt_grid = (0..self.rows)
                    .map(|_| vec![default_cell; self.cols])
                    .collect();
            } else {
                for row in &mut self.alt_grid {
                    row.clear();
                    row.resize(self.cols, default_cell);
                }
            }
            self.cursor_x = 0;
            self.cursor_y = 0;
        }
    }

    pub fn exit_alt_screen(&mut self) {
        if self.is_alt_screen {
            self.is_alt_screen = false;
            self.cursor_x = self.saved_cursor_x.min(self.cols.saturating_sub(1));
            self.cursor_y = self.saved_cursor_y.min(self.rows.saturating_sub(1));
            self.scroll_region_top = 0;
            self.scroll_region_bottom = self.rows.saturating_sub(1);
            self.current_fg = Color::TRANSPARENT;
            self.current_bg = Color::TRANSPARENT;
            self.current_bold = false;
            self.current_dim = false;
            self.current_italic = false;
            self.current_underline = false;
            self.current_strikethrough = false;
            self.current_inverse = false;
        }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        if let Some(mut parser) = self.parser.take() {
            let mut performer = Performer { state: self };
            parser.advance(&mut performer, bytes);
            performer.state.parser = Some(parser);
        }
    }

    /// Registra un bloque OSC 133 descartando los más antiguos (el render los recorre en cada fotograma)
    fn push_command_block(&mut self, block: CommandBlock) {
        const MAX_COMMAND_BLOCKS: usize = 512;
        if self.command_blocks.len() >= MAX_COMMAND_BLOCKS {
            self.command_blocks.drain(..MAX_COMMAND_BLOCKS / 2);
        }
        self.command_blocks.push(block);
    }

    /// Registra una imagen inline; con más de `MAX_IMAGE_BYTES` en total se descartan las más antiguas
    fn push_image(&mut self, img: InlineImage) {
        const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
        self.images.push(img);
        let mut total: usize = self.images.iter().map(|i| i.rgba_data.len()).sum();
        while total > MAX_IMAGE_BYTES && self.images.len() > 1 {
            total -= self.images.remove(0).rgba_data.len();
        }
    }

    /// El historial perdió su línea más antigua: las imágenes se reindexan y se liberan las que salen con ella
    fn shift_images_after_eviction(&mut self) {
        self.images
            .retain_mut(|img| match img.line_index.checked_sub(1) {
                Some(line) => {
                    img.line_index = line;
                    true
                }
                None => false,
            });
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
                let mut top_row = self
                    .grid
                    .pop_front()
                    .unwrap_or_else(|| vec![default_cell; self.cols]);

                // Recortar celdas vacías por la derecha: el historial guarda solo lo escrito (-70..85 % RAM)
                let used = top_row
                    .iter()
                    .rposition(|last| {
                        !(last.c == ' '
                            && last.bg == Color::TRANSPARENT
                            && (last.flags
                                & (Cell::FLAG_UNDERLINE
                                    | Cell::FLAG_STRIKETHROUGH
                                    | Cell::FLAG_INVERSE))
                                == 0)
                    })
                    .map_or(0, |i| i + 1);

                // Una sola asignación de tamaño exacto por línea; el buffer completo de la fila que sale
                // se reutiliza como nueva fila inferior y, al llenar el historial, la más antigua se libera.
                let stored = top_row[..used].to_vec();
                top_row.clear();
                top_row.resize(self.cols, default_cell);
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                    self.shift_images_after_eviction();
                }
                self.scrollback.push_back(stored);
                self.grid.push_back(top_row);
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

    #[inline(always)]
    pub fn current_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.current_bold {
            flags |= Cell::FLAG_BOLD;
        }
        if self.current_dim {
            flags |= Cell::FLAG_DIM;
        }
        if self.current_italic {
            flags |= Cell::FLAG_ITALIC;
        }
        if self.current_underline {
            flags |= Cell::FLAG_UNDERLINE;
        }
        if self.current_strikethrough {
            flags |= Cell::FLAG_STRIKETHROUGH;
        }
        if self.current_inverse {
            flags |= Cell::FLAG_INVERSE;
        }
        flags
    }

    pub fn put_char(&mut self, c: char) {
        // Fast-path para caracteres imprimibles ASCII estándar (ancho 1 fijo sin queries Unicode)
        let w = if c.is_ascii() {
            if (c as u8) < 0x20 || c == '\x7f' {
                return;
            }
            1u8
        } else {
            use unicode_width::UnicodeWidthChar;
            let char_w = c.width().unwrap_or(1);
            if char_w == 0 && c.is_control() {
                return;
            }
            if char_w == 0 {
                1
            } else {
                char_w.min(2) as u8
            }
        };

        // Si es de doble ancho (emoji, CJK) y no cabe al final de la línea, envolver automáticamente
        if (w == 2 && self.cursor_x + 1 >= self.cols) || self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }

        let flags = self.current_flags();
        let grid = if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        };

        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            grid[self.cursor_y][self.cursor_x] = Cell {
                c,
                width: w,
                flags,
                fg: self.current_fg,
                bg: self.current_bg,
            };

            // Para caracteres de 2 columnas, la celda contigua es un espaciador de continuación (width: 0)
            if w == 2 && self.cursor_x + 1 < self.cols {
                grid[self.cursor_y][self.cursor_x + 1] = Cell {
                    c: ' ',
                    width: 0,
                    flags,
                    fg: self.current_fg,
                    bg: self.current_bg,
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
            row.resize(new_cols, default_cell);
        }
        for row in self.alt_grid.iter_mut() {
            row.resize(new_cols, default_cell);
        }

        // Ajustar filas
        if new_rows > self.rows {
            for _ in self.rows..new_rows {
                self.grid.push_back(vec![default_cell; new_cols]);
                if !self.alt_grid.is_empty() {
                    self.alt_grid.push_back(vec![default_cell; new_cols]);
                }
            }
        } else if new_rows < self.rows {
            // Solo si el cursor desborda la nueva altura de la ventana, desplazar líneas hacia el scrollback
            if self.cursor_y >= new_rows {
                let shift = (self.cursor_y + 1).saturating_sub(new_rows);
                for _ in 0..shift {
                    if let Some(r) = self.grid.pop_front() {
                        if self.scrollback.len() >= self.max_scrollback {
                            self.scrollback.pop_front();
                            self.shift_images_after_eviction();
                        }
                        self.scrollback.push_back(r);
                    }
                    if !self.alt_grid.is_empty() {
                        self.alt_grid.pop_front();
                    }
                }
                self.cursor_y = self.cursor_y.saturating_sub(shift);
            }

            // Recortar el exceso de filas vacías del final de la pantalla
            while self.grid.len() > new_rows {
                self.grid.pop_back();
            }
            while self.alt_grid.len() > new_rows {
                self.alt_grid.pop_back();
            }
        }

        self.cols = new_cols;
        self.rows = new_rows;
        self.scroll_region_top = 0;
        self.scroll_region_bottom = new_rows.saturating_sub(1);
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self.cursor_y.min(new_rows.saturating_sub(1));
    }

    // `y` se usa también como número de fila de la selección, no solo como índice
    #[allow(clippy::needless_range_loop)]
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

#[cfg(test)]
mod tests;
