//! Intérprete de secuencias VT/ANSI: traduce los eventos de `vte` en cambios de `TerminalState`.

use super::*;

pub(super) struct Performer<'a> {
    pub(super) state: &'a mut TerminalState,
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
            let iter = params.iter();
            for param in iter {
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
                    flags: 0,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
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
                                    grid[cy][x] = erase_cell;
                                }
                            }
                            for row in grid.iter_mut().skip(cy + 1) {
                                for x in 0..cols.min(row.len()) {
                                    row[x] = erase_cell;
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
                                    grid[y][x] = erase_cell;
                                }
                            }
                        }
                        if cy < grid.len() {
                            for x in 0..=cx.min(cols.saturating_sub(1)) {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell;
                                }
                            }
                        }
                    }
                    2 => {
                        // Borrar pantalla completa activa
                        let grid = self.state.active_grid_mut();
                        for row in grid.iter_mut() {
                            for cell in row.iter_mut() {
                                *cell = erase_cell;
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
                                *cell = default_cell;
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
                    flags: 0,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
                };
                let grid = self.state.active_grid_mut();
                if cy < grid.len() {
                    match mode {
                        0 => {
                            for x in cx..cols {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell;
                                }
                            }
                        }
                        1 => {
                            for x in 0..=cx.min(cols.saturating_sub(1)) {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell;
                                }
                            }
                        }
                        2 => {
                            for x in 0..cols {
                                if x < grid[cy].len() {
                                    grid[cy][x] = erase_cell;
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
                    flags: 0,
                    fg: self.state.current_fg,
                    bg: self.state.current_bg,
                };
                let grid = self.state.active_grid_mut();
                if cy < grid.len() {
                    let end_x = (cx + count).min(cols);
                    for x in cx..end_x {
                        if x < grid[cy].len() {
                            grid[cy][x] = erase_cell;
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
                            row.push(default_cell);
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
                            row.insert(cx, default_cell);
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
                        if crate::pty::is_safe_ssh_destination(&clean_dest) {
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
                    if crate::pty::is_safe_ssh_destination(&dest) {
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
                    if !host.is_empty()
                        && host != "localhost"
                        && host != "127.0.0.1"
                        && crate::pty::is_safe_ssh_destination(host)
                    {
                        // Host remoto vía OSC 7 (el directorio solo se guarda si es seguro de reutilizar)
                        let remote_dir =
                            crate::pty::safe_remote_dir(&decoded_path).map(str::to_string);
                        if let Some(sess) = &mut self.state.ssh_session {
                            sess.remote_dir = remote_dir;
                        } else {
                            self.state.ssh_session = Some(SshSession {
                                destination: host.to_string(),
                                args: vec![host.to_string()],
                                remote_dir,
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
            b"9" if params.len() > 1 && params[1] == b"9" && params.len() > 2 => {
                let raw = String::from_utf8_lossy(params[2]);
                let clean = raw.trim().trim_matches('"');
                if !clean.is_empty() {
                    self.state.current_dir = Some(std::path::PathBuf::from(clean));
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
                        let max_cols =
                            (self.state.cols.saturating_sub(self.state.cursor_x + 2)).clamp(10, 80);
                        let cols = ((w as f32 / 8.0).round() as usize).clamp(10, max_cols);
                        let aspect = h as f32 / w as f32;
                        let rows =
                            ((cols as f32 * (8.0 / 18.0) * aspect).round() as usize).clamp(3, 30);

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
                    self.state.push_command_block(prev);
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
                    self.state.push_command_block(block);
                }
            }
            _ => {}
        }
    }
}
