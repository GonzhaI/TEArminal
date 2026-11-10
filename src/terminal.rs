//! Emulador de terminal: estado (`TerminalState`), rejilla con scrollback y manejo de OSC (133, 7, 52, 1337).
//!
//! `performer` traduce las secuencias VT/ANSI de `vte` en cambios de este estado.

mod cell;
pub(crate) mod kitty;
mod links;
pub(crate) mod performer;
pub(crate) mod search;
pub(crate) use performer::clean_listing;

pub(crate) use cell::*;
pub(crate) use kitty::*;
pub(crate) use links::*;
pub(crate) use performer::percent_decode;
use performer::Performer;

use crate::theme::{Color, Theme};
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use vte::{Params, Parser, Perform};

/// Tamaño máximo de una secuencia OSC. El de vte por defecto (1 KB) descarta el resto en silencio: cortaba
/// los listados remotos (`TearminalLs`), los trozos de `TearminalFile` (32 KB) y OSC 52 (hasta 100 KB).
pub(crate) const OSC_BUF_SIZE: usize = 128 * 1024;
/// Codepoint base en el plano de uso privado Unicode para mapear índices de grafemas complejos en celdas.
pub(crate) const GRAPHEME_BASE_CODEPOINT: u32 = 0x0010_0000;
type OscParser = Parser<OSC_BUF_SIZE>;

pub(crate) struct TerminalState {
    pub cols: usize,
    pub rows: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_shape: CursorShape,
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
    /// Token secreto del panel (`PtyManager::token`, variable `TEARMINAL_PANE_TOKEN` del shell): autentica los
    /// marcadores `TearminalSSH=start` que emite el wrapper `ssh` de `init.ps1`. Vacío = ningún marcador es válido.
    pub pane_token: String,
    /// El wrapper de agentes de `init.ps1` avisó (OSC 1337 TearminalAgent) de que hay un agente en marcha
    pub agent_running: bool,
    pub ai_agent: crate::renderer::AiAgentStatus,
    pub window_title: Option<String>,
    /// El bash remoto lanzado por el wrapper `ssh` de TEArminal informa de rama git (OSC 1337 TearminalGit)
    pub remote_integration: bool,
    /// El prompt remoto está esperando entrada (llega `TearminalGit` en cada prompt; Enter lo apaga)
    pub remote_at_prompt: bool,
    /// Respuestas de archivo/carpeta remotas completas (`OSC 1337;TearminalFile=<id>;status=...`) listas para la app
    pub remote_file_results: Vec<RemoteFileResult>,
    /// Trozos (`part=`) recibidos por `id` a la espera del paquete final con `status=`
    pub(super) remote_file_parts: FxHashMap<String, RemoteFileAsm>,

    // Bracketed paste dinámico negociado con agentes (?2004h / ?2004l)
    pub bracketed_paste_mode: bool,

    // Modo de teclas de cursor de aplicación (DECCKM: ?1h / ?1l)
    pub cursor_key_mode: bool,

    // Soporte para protocolos interactivos TUI (Herdr, Ratatui, Lazygit)
    pub mouse_reporting_normal: bool, // Modo 1000: Reporte de clics
    pub mouse_reporting_button_event: bool, // Modo 1002: Clics y arrastre
    pub mouse_reporting_any_event: bool, // Modo 1003: Todo movimiento
    pub mouse_reporting_sgr: bool,    // Modo 1006: Coordenadas SGR extendidas (\x1b[<...M/m)
    pub focus_event_reporting: bool,  // Modo 1004: Foco ganado/perdido (\x1b[I, \x1b[O)
    pub sync_update_mode: bool,       // Modo 2026: Renderizado sincronizado (DEC 2026)
    pub sync_update_start: Option<std::time::Instant>, // Marca de tiempo de inicio del renderizado sincronizado
    pub report_color_scheme_mode: bool, // Modo 2031: Notificación de modo claro/oscuro (DEC 2031)

    // Kitty Keyboard Protocol (CSI u)
    pub kitty_keyboard_flags: u8,
    pub kitty_keyboard_stack: Vec<u8>,

    // Imágenes renderizadas en terminal (Protocolos iTerm2 / OSC 1337 / Visión IA)
    pub images: Vec<InlineImage>,
    pub next_image_id: u64,
    pub total_lines_count: usize,

    // Estado SGR actual
    pub theme: Theme,
    pub(super) current_fg: Color,
    pub(super) current_bg: Color,
    pub(super) current_flags: u8,

    // Selección de texto
    pub selection: Option<SelectionRange>,
    /// Barra de búsqueda en el historial (`Ctrl + Shift + F`), si está abierta
    pub(crate) search: Option<search::TermSearch>,

    // Hover del scrollbar del panel y botón de ir al fondo
    pub scrollbar_hovered: bool,
    pub scroll_to_bottom_hovered: bool,

    parser: Option<Box<OscParser>>,
    pub(super) parser_in_ground: bool,

    // Soporte para clústeres de grafemas extendidos (UAX #29)
    pub graphemes: Vec<Box<str>>,
    pub grapheme_map: FxHashMap<Box<str>, u32>,

    // Soporte para hipervínculos explícitos (OSC 8)
    pub current_hyperlink: u16,
    pub hyperlinks: Vec<Box<str>>,
    pub hyperlink_map: FxHashMap<Box<str>, u16>,

    // Soporte para color de subrayado explícito (SGR 58 / SGR 59)
    pub current_underline_color: u8,
    pub underline_colors: Vec<Color>,
    pub underline_color_map: FxHashMap<u32, u8>,

    // Protocolo Kitty Graphics (APC \x1b_G...) y cola de respuesta PTY
    pub pty_write_queue: Vec<Vec<u8>>,
    pub(super) kitty_apc_buf: Option<Vec<u8>>,
    pub(super) kitty_esc_prefix: Vec<u8>,
    pub(super) kitty_chunk_data: Option<KittyChunkState>,
    pub(super) kitty_stored_images: FxHashMap<u64, InlineImage>,
    /// Ids de `kitty_stored_images` de más antiguo a más reciente (orden de expulsión)
    pub(super) kitty_stored_order: Vec<u64>,
}

impl TerminalState {
    pub(crate) fn new(cols: usize, rows: usize, max_scrollback: usize, theme: Theme) -> Self {
        let default_cell = Cell::default_with_theme(&theme);
        let grid = (0..rows).map(|_| vec![default_cell; cols]).collect();
        let alt_grid = VecDeque::new();
        let fg = theme.foreground;

        Self {
            cols,
            rows,
            cursor_x: 0,
            cursor_y: 0,
            cursor_shape: CursorShape::Block,
            grid,
            scrollback: VecDeque::with_capacity(max_scrollback.min(4096)),
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
            ai_agent: crate::renderer::AiAgentStatus::None,
            window_title: None,
            remote_integration: false,
            remote_at_prompt: false,
            remote_file_results: Vec::new(),
            remote_file_parts: FxHashMap::default(),
            bracketed_paste_mode: false,
            cursor_key_mode: false,
            mouse_reporting_normal: false,
            mouse_reporting_button_event: false,
            mouse_reporting_any_event: false,
            mouse_reporting_sgr: false,
            focus_event_reporting: false,
            sync_update_mode: false,
            sync_update_start: None,
            report_color_scheme_mode: false,
            kitty_keyboard_flags: 0,
            kitty_keyboard_stack: Vec::new(),
            images: Vec::new(),
            next_image_id: 1,
            total_lines_count: 0,
            theme,
            current_fg: fg,
            current_bg: Color::TRANSPARENT,
            current_flags: 0,
            selection: None,
            search: None,
            scrollbar_hovered: false,
            scroll_to_bottom_hovered: false,
            parser: Some(Box::default()),
            parser_in_ground: true,
            graphemes: Vec::new(),
            grapheme_map: FxHashMap::default(),
            current_hyperlink: 0,
            hyperlinks: Vec::new(),
            hyperlink_map: FxHashMap::default(),
            current_underline_color: 0,
            underline_colors: Vec::new(),
            underline_color_map: FxHashMap::default(),
            pty_write_queue: Vec::new(),
            kitty_apc_buf: None,
            kitty_esc_prefix: Vec::new(),
            kitty_chunk_data: None,
            kitty_stored_images: FxHashMap::default(),
            kitty_stored_order: Vec::new(),
        }
    }

    /// Comprueba si el terminal está en modo de renderizado sincronizado activo (DEC 2026).
    /// Incluye un tiempo de seguridad de 200 ms para evitar que el terminal quede congelado.
    pub(crate) fn is_in_sync_update(&self) -> bool {
        if !self.sync_update_mode {
            return false;
        }
        if let Some(start) = self.sync_update_start {
            if start.elapsed().as_millis() > 200 {
                return false;
            }
        }
        true
    }

    /// El marcador trae el token de este panel (comparación en tiempo constante). Un token vacío nunca vale.
    pub(crate) fn pane_token_matches(&self, token: &str) -> bool {
        let (a, b) = (self.pane_token.as_bytes(), token.as_bytes());
        !a.is_empty()
            && a.len() == b.len()
            && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
    }

    /// El usuario pulsó Enter: lo que se escriba ya no llega a un prompt sino a un programa
    pub(crate) fn note_enter_sent(&mut self) {
        self.remote_at_prompt = false;
    }

    /// Petición al manejador readline (`bind -x`) del bash remoto: `\e[1337~` la dispara sin hacer eco ni
    /// tocar la línea de edición y el manejador lee `op id payload\n` con `read -r`. El salto de línea no pasa por
    /// `note_enter_sent`: el prompt sigue activo al terminar.
    pub(crate) fn remote_request_bytes(op: &str, id: &str, payload: &str) -> Vec<u8> {
        format!("\x1b[1337~{op} {id} {payload}\n").into_bytes()
    }

    pub(crate) fn is_mouse_reporting_active(&self) -> bool {
        self.mouse_reporting_normal
            || self.mouse_reporting_button_event
            || self.mouse_reporting_any_event
    }

    pub(crate) fn current_global_line(&self) -> usize {
        self.scrollback.len() + self.cursor_y
    }

    pub(crate) fn active_ai_agent(&self) -> crate::renderer::AiAgentStatus {
        if self.ai_agent != crate::renderer::AiAgentStatus::None {
            return self.ai_agent;
        }
        crate::app::detect_ai_agent(self.window_title.as_deref())
    }

    #[inline]
    pub(crate) fn grapheme_of(&self, cell: &Cell) -> Option<&str> {
        if cell.is_grapheme() {
            let id = (cell.c as u32).checked_sub(GRAPHEME_BASE_CODEPOINT)? as usize;
            self.graphemes.get(id).map(|s| &**s)
        } else {
            None
        }
    }

    pub(crate) fn register_grapheme(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.grapheme_map.get(s) {
            return id;
        }
        let id = self.graphemes.len() as u32;
        if id < 0xFE00 {
            let boxed: Box<str> = s.into();
            self.grapheme_map.insert(boxed.clone(), id);
            self.graphemes.push(boxed);
            id
        } else {
            0
        }
    }

    #[inline]
    pub(crate) fn total_buffer_lines(&self) -> usize {
        if self.is_alt_screen {
            self.alt_grid.len()
        } else {
            self.scrollback.len() + self.grid.len()
        }
    }

    #[inline]
    pub(crate) fn get_buffer_line(&self, line_idx: usize) -> Option<&[Cell]> {
        if self.is_alt_screen {
            self.alt_grid.get(line_idx).map(|r| r.as_slice())
        } else if line_idx < self.scrollback.len() {
            self.scrollback.get(line_idx).map(|r| r.as_slice())
        } else {
            let grid_idx = line_idx - self.scrollback.len();
            self.grid.get(grid_idx).map(|r| r.as_slice())
        }
    }

    pub(crate) fn visible_line_start(&self) -> usize {
        if self.is_alt_screen {
            0
        } else {
            self.scrollback.len().saturating_sub(self.scroll_offset)
        }
    }

    pub(crate) fn active_grid_mut(&mut self) -> &mut VecDeque<Vec<Cell>> {
        if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        }
    }

    pub(crate) fn enter_alt_screen(&mut self) {
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
            self.current_hyperlink = 0;
            self.current_underline_color = 0;
        }
    }

    pub(crate) fn exit_alt_screen(&mut self) {
        if self.is_alt_screen {
            self.is_alt_screen = false;
            self.cursor_x = self.saved_cursor_x.min(self.cols.saturating_sub(1));
            self.cursor_y = self.saved_cursor_y.min(self.rows.saturating_sub(1));
            self.scroll_region_top = 0;
            self.scroll_region_bottom = self.rows.saturating_sub(1);
            self.current_fg = Color::TRANSPARENT;
            self.current_bg = Color::TRANSPARENT;
            self.current_flags = 0;
            self.sync_update_mode = false;
            self.sync_update_start = None;
            self.cursor_key_mode = false;
            self.current_hyperlink = 0;
            self.current_underline_color = 0;
        }
    }

    pub(crate) fn process_bytes(&mut self, bytes: &[u8]) {
        let Some(mut parser) = self.parser.take() else {
            return;
        };
        let mut i = 0;
        let len = bytes.len();

        // 1. Resolver prefijo ESC pendiente si el paquete anterior terminó a mitad de \x1b_G
        if !self.kitty_esc_prefix.is_empty() {
            if self.kitty_esc_prefix == [0x1b] {
                if bytes.starts_with(b"_G") {
                    self.kitty_esc_prefix.clear();
                    self.kitty_apc_buf = Some(Vec::new());
                    i = 2;
                } else if bytes == b"_" {
                    self.kitty_esc_prefix.push(b'_');
                    self.parser = Some(parser);
                    return;
                } else {
                    let mut performer = Performer { state: self };
                    parser.advance(&mut performer, &[0x1b]);
                    self.kitty_esc_prefix.clear();
                }
            } else if self.kitty_esc_prefix == [0x1b, b'_'] {
                if bytes.starts_with(b"G") {
                    self.kitty_esc_prefix.clear();
                    self.kitty_apc_buf = Some(Vec::new());
                    i = 1;
                } else {
                    let mut performer = Performer { state: self };
                    parser.advance(&mut performer, &[0x1b]);
                    parser.advance(&mut performer, b"_");
                    self.kitty_esc_prefix.clear();
                }
            }
        }

        while i < len {
            // Si estamos en medio de una secuencia APC Kitty (\x1b_G...)
            if let Some(buf) = &mut self.kitty_apc_buf {
                let b = bytes[i];
                i += 1;
                if b == 0x07 {
                    if let Some(payload) = self.kitty_apc_buf.take() {
                        kitty::process_kitty_apc(self, &payload);
                    }
                    self.parser_in_ground = true;
                } else if b == b'\\' && buf.ends_with(&[0x1b]) {
                    buf.pop();
                    if let Some(payload) = self.kitty_apc_buf.take() {
                        kitty::process_kitty_apc(self, &payload);
                    }
                    self.parser_in_ground = true;
                } else if buf.len() >= 64 * 1024 * 1024 {
                    self.kitty_apc_buf = None;
                    self.parser_in_ground = true;
                } else {
                    buf.push(b);
                }
                continue;
            }

            // Detección de inicio de secuencia Kitty Graphics: \x1b_G
            if bytes[i] == 0x1b {
                if i + 2 < len && bytes[i + 1] == b'_' && bytes[i + 2] == b'G' {
                    self.kitty_apc_buf = Some(Vec::new());
                    i += 3;
                    continue;
                } else if i + 1 == len {
                    self.kitty_esc_prefix.push(0x1b);
                    break;
                } else if i + 2 == len && bytes[i + 1] == b'_' {
                    self.kitty_esc_prefix.extend_from_slice(&[0x1b, b'_']);
                    break;
                }
            }

            if self.parser_in_ground {
                let b = bytes[i];
                if (0x20..=0x7e).contains(&b) {
                    if self.cursor_x >= self.cols {
                        self.cursor_x = 0;
                        self.newline();
                    }
                    let max_run = (len - i).min(self.cols.saturating_sub(self.cursor_x));
                    let run = bytes[i..i + max_run]
                        .iter()
                        .take_while(|&&ch| (0x20..=0x7e).contains(&ch))
                        .count();
                    if run > 0 && self.cursor_y < self.rows {
                        let (cx, cy, flags, fg, bg, meta) = (
                            self.cursor_x,
                            self.cursor_y,
                            self.current_flags,
                            self.current_fg,
                            self.current_bg,
                            Cell::pack_meta(self.current_hyperlink, self.current_underline_color),
                        );
                        let grid = if self.is_alt_screen {
                            &mut self.alt_grid
                        } else {
                            &mut self.grid
                        };
                        let row = &mut grid[cy][cx..cx + run];
                        for (&ch, cell) in bytes[i..i + run].iter().zip(row) {
                            *cell = Cell {
                                c: ch as char,
                                width: 1,
                                flags,
                                hyperlink_id: meta,
                                fg,
                                bg,
                            };
                        }
                        self.cursor_x += run;
                        i += run;
                        continue;
                    }
                } else if b != 0x1b && b < 0x80 {
                    match b {
                        b'\r' => self.cursor_x = 0,
                        b'\n' => self.newline(),
                        b'\x08' => {
                            if self.cursor_x > 0 {
                                self.cursor_x -= 1;
                            }
                        }
                        b'\t' => {
                            let next_tab = (self.cursor_x + 8) & !7;
                            self.cursor_x = next_tab.min(self.cols.saturating_sub(1));
                        }
                        _ => {}
                    }
                    i += 1;
                    continue;
                }
            }
            self.parser_in_ground = false;
            let mut performer = Performer { state: self };
            while i < len && !performer.state.parser_in_ground {
                if bytes[i] == 0x1b
                    && ((i + 2 < len && bytes[i + 1] == b'_' && bytes[i + 2] == b'G')
                        || i + 1 == len
                        || (i + 2 == len && bytes[i + 1] == b'_'))
                {
                    break;
                }
                parser.advance(&mut performer, &bytes[i..=i]);
                i += 1;
            }
        }
        self.parser = Some(parser);
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
    pub(crate) fn push_image(&mut self, img: InlineImage) {
        const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
        self.images.push(img);
        let mut total: usize = self.images.iter().map(|i| i.rgba_data.len()).sum();
        while total > MAX_IMAGE_BYTES && self.images.len() > 1 {
            total -= self.images.remove(0).rgba_data.len();
        }
    }

    /// Almacena una imagen Kitty pretransmitida (`a=t`/`a=T`); limita a 32 imágenes y 64 MB para evitar fugas de memoria
    pub(crate) fn store_kitty_image(&mut self, id: u64, img: InlineImage) {
        const MAX_STORED_IMAGES: usize = 32;
        const MAX_STORED_BYTES: usize = 64 * 1024 * 1024;
        // `kitty_stored_order` lleva el orden de inserción (un `HashMap` no sabe cuál es la más antigua): la que se
        // expulsa es la primera y nunca la recién guardada. Reemplazar un id lo pasa al final.
        self.kitty_stored_images.insert(id, img);
        self.kitty_stored_order.retain(|&i| i != id);
        self.kitty_stored_order.push(id);
        // Los ids borrados por `a=d` ya no están en el mapa
        let images = &self.kitty_stored_images;
        self.kitty_stored_order.retain(|i| images.contains_key(i));
        let mut total: usize = self
            .kitty_stored_images
            .values()
            .map(|i| i.rgba_data.len())
            .sum();
        while (self.kitty_stored_order.len() > MAX_STORED_IMAGES || total > MAX_STORED_BYTES)
            && self.kitty_stored_order.len() > 1
        {
            let oldest = self.kitty_stored_order.remove(0);
            if let Some(removed) = self.kitty_stored_images.remove(&oldest) {
                total = total.saturating_sub(removed.rgba_data.len());
            }
        }
    }

    /// El historial perdió su línea más antigua: imágenes y bloques OSC 133 se reindexan y se liberan los que salen con ella
    fn shift_images_after_eviction(&mut self) {
        if !self.images.is_empty() {
            self.images
                .retain_mut(|img| match img.line_index.checked_sub(1) {
                    Some(line) => {
                        img.line_index = line;
                        true
                    }
                    None => false,
                });
        }
        if !self.command_blocks.is_empty() {
            let oldest_survives = self.command_blocks[0]
                .output_end
                .or(self.command_blocks[0].output_start)
                .or(self.command_blocks[0].command_line)
                .unwrap_or(self.command_blocks[0].prompt_line)
                > 1;
            if oldest_survives {
                for b in &mut self.command_blocks {
                    b.prompt_line = b.prompt_line.saturating_sub(1);
                    if let Some(l) = &mut b.command_line {
                        *l = l.saturating_sub(1);
                    }
                    if let Some(l) = &mut b.output_start {
                        *l = l.saturating_sub(1);
                    }
                    if let Some(l) = &mut b.output_end {
                        *l = l.saturating_sub(1);
                    }
                }
            } else {
                self.command_blocks
                    .retain_mut(Self::shift_block_after_eviction);
            }
        }
        if let Some(b) = &mut self.active_block {
            if !Self::shift_block_after_eviction(b) {
                self.active_block = None;
            }
        }
        if let Some(sel) = &mut self.selection {
            if sel.start.1 == 0 && sel.end.1 == 0 {
                self.selection = None;
            } else {
                sel.start.1 = sel.start.1.saturating_sub(1);
                sel.end.1 = sel.end.1.saturating_sub(1);
            }
        }
    }

    #[inline]
    fn shift_block_after_eviction(b: &mut CommandBlock) -> bool {
        let max_line = b
            .output_end
            .or(b.output_start)
            .or(b.command_line)
            .unwrap_or(b.prompt_line);
        b.prompt_line = b.prompt_line.saturating_sub(1);
        if let Some(l) = &mut b.command_line {
            *l = l.saturating_sub(1);
        }
        if let Some(l) = &mut b.output_start {
            *l = l.saturating_sub(1);
        }
        if let Some(l) = &mut b.output_end {
            *l = l.saturating_sub(1);
        }
        max_line > 0
    }

    pub(crate) fn newline(&mut self) {
        self.total_lines_count += 1;
        let default_cell = Cell::default_with_theme(&self.theme);

        if self.is_alt_screen {
            if self.cursor_y >= self.scroll_region_bottom {
                if self.scroll_region_top < self.alt_grid.len() {
                    if let Some(mut recycled) = self.alt_grid.remove(self.scroll_region_top) {
                        recycled.fill(default_cell);
                        let insert_pos = self.scroll_region_bottom.min(self.alt_grid.len());
                        self.alt_grid.insert(insert_pos, recycled);
                    }
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

                // Si el historial está lleno, reciclar el buffer expulsado cuando su capacidad se ajuste
                let stored = if self.scrollback.len() >= self.max_scrollback {
                    let evicted = self.scrollback.pop_front();
                    self.shift_images_after_eviction();
                    match evicted {
                        Some(mut buf) if buf.capacity() >= used && buf.capacity() <= used + 32 => {
                            buf.clear();
                            buf.extend_from_slice(&top_row[..used]);
                            buf
                        }
                        _ => top_row[..used].to_vec(),
                    }
                } else {
                    top_row[..used].to_vec()
                };
                top_row.fill(default_cell);
                if self.max_scrollback > 0 {
                    self.scrollback.push_back(stored);
                    if self.scroll_offset > 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(self.max_scrollback);
                    }
                }
                self.grid.push_back(top_row);
            } else {
                // Sub-región de desplazamiento (split panes de tmux)
                if self.scroll_region_top < self.grid.len() {
                    if let Some(mut recycled) = self.grid.remove(self.scroll_region_top) {
                        recycled.fill(default_cell);
                        let insert_pos = self.scroll_region_bottom.min(self.grid.len());
                        self.grid.insert(insert_pos, recycled);
                    }
                }
            }
        } else {
            self.cursor_y += 1;
        }
    }

    #[inline(always)]
    pub(crate) fn put_char(&mut self, c: char) {
        let (cx, cy) = (self.cursor_x, self.cursor_y);
        let is_alt = self.is_alt_screen;

        // Grapheme clustering (UAX #29): comprobar si `c` extiende el grafema precedente en la línea
        if !c.is_ascii() && cx > 0 {
            let has_row = if is_alt {
                cy < self.alt_grid.len()
            } else {
                cy < self.grid.len()
            };
            if has_row {
                let row = if is_alt {
                    &self.alt_grid[cy]
                } else {
                    &self.grid[cy]
                };
                let prev_x = if cx >= 2 && row[cx - 1].width == 0 && row[cx - 2].width == 2 {
                    Some(cx - 2)
                } else if row[cx - 1].width > 0 {
                    Some(cx - 1)
                } else {
                    None
                };

                if let Some(px) = prev_x {
                    let prev_cell = row[px];
                    if prev_cell.c != ' ' {
                        let mut prev_str = String::with_capacity(8);
                        self.append_cell_text(&prev_cell, &mut prev_str);
                        prev_str.push(c);

                        use unicode_segmentation::UnicodeSegmentation;
                        if prev_str.graphemes(true).count() == 1 {
                            let id = self.register_grapheme(&prev_str);
                            let target = if is_alt {
                                &mut self.alt_grid[cy][px]
                            } else {
                                &mut self.grid[cy][px]
                            };
                            target.flags |= Cell::FLAG_GRAPHEME;
                            target.c =
                                char::from_u32(GRAPHEME_BASE_CODEPOINT + id).unwrap_or(prev_cell.c);
                            return;
                        }
                    }
                }
            }
        }

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
        if self.cursor_x >= self.cols || (w == 2 && self.cursor_x + 1 >= self.cols) {
            self.cursor_x = 0;
            self.newline();
        }

        let meta = Cell::pack_meta(self.current_hyperlink, self.current_underline_color);
        let (cx, cy, flags, fg, bg) = (
            self.cursor_x,
            self.cursor_y,
            self.current_flags,
            self.current_fg,
            self.current_bg,
        );
        let grid = if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        };

        if cy < self.rows && cx < self.cols {
            let row = &mut grid[cy];
            row[cx] = Cell {
                c,
                width: w,
                flags,
                hyperlink_id: meta,
                fg,
                bg,
            };

            // Para caracteres de 2 columnas, la celda contigua es un espaciador de continuación (width: 0)
            if w == 2 && cx + 1 < self.cols {
                row[cx + 1] = Cell {
                    c: ' ',
                    width: 0,
                    flags,
                    hyperlink_id: meta,
                    fg,
                    bg,
                };
                self.cursor_x = cx + 2;
            } else {
                self.cursor_x = cx + 1;
            }
        }
    }

    /// Devuelve verdadero si existe historial de desplazamiento con contenido visible o imágenes.
    /// Evita barras de desplazamiento fantasma cuando el historial está vacío o solo contiene líneas en blanco.
    #[inline]
    pub(crate) fn has_scrollback(&self) -> bool {
        !self.scrollback.is_empty()
            && (self.scrollback.iter().any(|r| !r.is_empty()) || !self.images.is_empty())
    }

    pub(crate) fn scroll_up(&mut self, lines: usize) {
        if self.is_alt_screen || !self.has_scrollback() {
            return;
        }
        let max_offset = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    pub(crate) fn scroll_down(&mut self, lines: usize) {
        if self.is_alt_screen {
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    /// Vacía el historial de desplazamiento, los bloques OSC 133, las imágenes inline y la selección
    pub(crate) fn clear_scrollback(&mut self) {
        self.scrollback.clear();
        self.scroll_offset = 0;
        self.total_lines_count = self.rows;
        self.command_blocks.clear();
        self.images.clear();
        self.kitty_stored_images.clear();
        self.kitty_stored_order.clear();
        self.selection = None;
    }

    /// Obtiene las filas que deben renderizarse en pantalla según `scroll_offset`
    pub(crate) fn visible_rows(&self) -> Vec<&[Cell]> {
        if self.is_alt_screen {
            return self.alt_grid.iter().map(|r| r.as_slice()).collect();
        }

        if self.scroll_offset == 0 {
            self.grid.iter().map(|r| r.as_slice()).collect()
        } else {
            let total_history = self.scrollback.len();
            let start = total_history.saturating_sub(self.scroll_offset);
            let mut rows = Vec::with_capacity(self.rows);

            for r in self.scrollback.range(start..) {
                rows.push(r.as_slice());
                if rows.len() == self.rows {
                    return rows;
                }
            }
            for r in &self.grid {
                rows.push(r.as_slice());
                if rows.len() == self.rows {
                    return rows;
                }
            }
            rows
        }
    }

    pub(crate) fn resize(&mut self, new_cols: usize, new_rows: usize) {
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
                        let used = r
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
                        if self.max_scrollback > 0 {
                            if self.scrollback.len() >= self.max_scrollback {
                                self.scrollback.pop_front();
                                self.shift_images_after_eviction();
                            }
                            self.scrollback.push_back(r[..used].to_vec());
                            if self.scroll_offset > 0 {
                                self.scroll_offset =
                                    (self.scroll_offset + 1).min(self.max_scrollback);
                            }
                        }
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
}

#[cfg(test)]
mod tests;
