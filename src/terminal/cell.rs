//! Celda de la rejilla (16 bytes) y tipos de datos asociados.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    pub(crate) c: char,
    pub(crate) width: u8,
    pub(crate) flags: u8,
    pub(crate) hyperlink_id: u16,
    pub(crate) fg: Color,
    pub(crate) bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            width: 1,
            flags: 0,
            hyperlink_id: 0,
            fg: Color::TRANSPARENT,
            bg: Color::TRANSPARENT,
        }
    }
}

impl From<char> for Cell {
    fn from(c: char) -> Self {
        Self {
            c,
            width: 1,
            ..Self::default()
        }
    }
}

impl Cell {
    pub(crate) const FLAG_BOLD: u8 = 1 << 0;
    pub(crate) const FLAG_DIM: u8 = 1 << 1;
    pub(crate) const FLAG_ITALIC: u8 = 1 << 2;
    pub(crate) const FLAG_UNDERLINE: u8 = 1 << 3;
    pub(crate) const FLAG_STRIKETHROUGH: u8 = 1 << 4;
    pub(crate) const FLAG_INVERSE: u8 = 1 << 5;
    pub(crate) const FLAG_GRAPHEME: u8 = 1 << 6;

    pub(crate) const HYPERLINK_MASK: u16 = 0x03FF; // 10 bits: 0..=1023 (hasta 1023 URLs simultáneas)
    pub(crate) const UNDERLINE_COLOR_SHIFT: u16 = 10;
    pub(crate) const UNDERLINE_COLOR_MASK: u16 = 0x003F; // 6 bits: 0..=63 (hasta 63 colores RGB de subrayado)

    #[inline(always)]
    pub(crate) fn bold(&self) -> bool {
        self.flags & Self::FLAG_BOLD != 0
    }

    #[inline(always)]
    pub(crate) fn dim(&self) -> bool {
        self.flags & Self::FLAG_DIM != 0
    }

    #[inline(always)]
    pub(crate) fn underline(&self) -> bool {
        self.flags & Self::FLAG_UNDERLINE != 0
    }

    #[inline(always)]
    pub(crate) fn strikethrough(&self) -> bool {
        self.flags & Self::FLAG_STRIKETHROUGH != 0
    }

    #[inline(always)]
    pub(crate) fn inverse(&self) -> bool {
        self.flags & Self::FLAG_INVERSE != 0
    }

    #[inline(always)]
    pub(crate) fn is_grapheme(&self) -> bool {
        self.flags & Self::FLAG_GRAPHEME != 0
    }

    #[inline(always)]
    pub(crate) fn raw_hyperlink_id(&self) -> u16 {
        self.hyperlink_id & Self::HYPERLINK_MASK
    }

    #[inline(always)]
    pub(crate) fn underline_color_id(&self) -> u8 {
        ((self.hyperlink_id >> Self::UNDERLINE_COLOR_SHIFT) & Self::UNDERLINE_COLOR_MASK) as u8
    }

    #[inline(always)]
    pub(crate) fn pack_meta(hyperlink_id: u16, underline_color_id: u8) -> u16 {
        (hyperlink_id & Self::HYPERLINK_MASK)
            | ((u16::from(underline_color_id) & Self::UNDERLINE_COLOR_MASK)
                << Self::UNDERLINE_COLOR_SHIFT)
    }

    pub(crate) fn default_with_theme(theme: &Theme) -> Self {
        Self {
            c: ' ',
            width: 1,
            flags: 0,
            hyperlink_id: 0,
            fg: theme.foreground,
            bg: Color::TRANSPARENT,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct ImageFrame {
    pub(crate) rgba_data: Vec<u8>,
    pub(crate) delay_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InlineImage {
    pub(crate) id: u64,
    pub(crate) line_index: usize,
    pub(crate) col: usize,
    pub(crate) width_cols: usize,
    pub(crate) height_rows: usize,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) rgba_data: Vec<u8>,
    pub(crate) frames: Vec<ImageFrame>,
}

impl InlineImage {
    /// Imagen ya escalada a `cols` x `rows` celdas, anclada en (`col`, `line_index`)
    pub(crate) fn new(
        id: u64,
        (col, line_index): (usize, usize),
        (cols, rows): (usize, usize),
        rgba: image::RgbaImage,
    ) -> Self {
        let (width_px, height_px) = rgba.dimensions();
        Self {
            id,
            line_index,
            col,
            width_cols: cols,
            height_rows: rows,
            width_px,
            height_px,
            rgba_data: rgba.into_raw(),
            frames: Vec::new(),
        }
    }

    /// Imagen con múltiples fotogramas para animación continua (Kitty Graphics / GIFs)
    pub(crate) fn with_frames(
        id: u64,
        (col, line_index): (usize, usize),
        (cols, rows): (usize, usize),
        rgba: image::RgbaImage,
        frames: Vec<ImageFrame>,
    ) -> Self {
        let (width_px, height_px) = rgba.dimensions();
        Self {
            id,
            line_index,
            col,
            width_cols: cols,
            height_rows: rows,
            width_px,
            height_px,
            rgba_data: rgba.into_raw(),
            frames,
        }
    }

    /// Retorna los bytes RGBA del fotograma activo según el tiempo del sistema
    pub(crate) fn active_rgba(&self) -> &[u8] {
        match current_frame(&self.frames) {
            Some((i, _)) => &self.frames[i].rgba_data,
            None => self
                .frames
                .first()
                .map_or(&self.rgba_data, |f| &f.rgba_data),
        }
    }

    /// Retorna el retraso en milisegundos restante para el próximo fotograma si es animada
    pub(crate) fn next_frame_delay_ms(&self) -> Option<u32> {
        if self.frames.len() <= 1 {
            return None;
        }
        current_frame(&self.frames).map(|(_, remaining)| remaining.max(10))
    }
}

/// Fotograma que toca ahora en una animación en bucle y los ms que le quedan, medidos con el reloj del
/// sistema (sin estado: cualquier vista de la misma animación va al compás). `None` si no hay fotogramas
/// o ninguno tiene duración.
pub(crate) fn current_frame(frames: &[ImageFrame]) -> Option<(usize, u32)> {
    let total_ms: u64 = frames.iter().map(|f| u64::from(f.delay_ms)).sum();
    if total_ms == 0 {
        return None;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut rem = now_ms % total_ms;
    for (i, frame) in frames.iter().enumerate() {
        let d = u64::from(frame.delay_ms);
        if rem < d {
            return Some((i, (d - rem) as u32));
        }
        rem -= d;
    }
    None
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct CommandBlock {
    pub(crate) prompt_line: usize,
    pub(crate) command_line: Option<usize>,
    pub(crate) output_start: Option<usize>,
    pub(crate) output_end: Option<usize>,
    pub(crate) exit_code: Option<i32>,
    /// Texto del comando sin el prompt (desde OSC 133;C), para copiarlo o repetirlo.
    pub(crate) command: Option<String>,
    /// Columna en la que empieza el comando dentro de `command_line`.
    pub(crate) command_col: usize,
    /// Marca de tiempo del inicio de la ejecución (OSC 133;C).
    pub(crate) started: Option<std::time::Instant>,
    /// Marca de tiempo del fin (OSC 133;D); su diferencia con `started` es la duración.
    pub(crate) finished: Option<std::time::Instant>,
    /// El usuario descartó los botones «Explicar» / «Ignorar» de esta tarjeta de error.
    pub(crate) error_dismissed: bool,
}

impl CommandBlock {
    /// Duración real de la ejecución, si el shell reportó inicio y fin.
    pub(crate) fn duration(&self) -> Option<std::time::Duration> {
        Some(self.finished?.duration_since(self.started?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct SshSession {
    pub(crate) destination: String,
    pub(crate) identity_file: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) remote_dir: Option<String>,
    pub(crate) remote_git_branch: Option<String>,
    pub(crate) is_active: bool,
    pub(crate) remote_listing: Option<Vec<String>>,
    pub(crate) remote_git_statuses:
        Option<rustc_hash::FxHashMap<std::path::PathBuf, crate::explorer::GitStatusInfo>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteFileResult {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) data: Vec<u8>,
}

/// Respuesta `TearminalFile` que llega en varios `OSC 1337;TearminalFile=<id>;part=<b64>` antes del `;status=...`
#[derive(Clone, Debug, Default)]
pub(crate) struct RemoteFileAsm {
    pub(crate) data: Vec<u8>,
    pub(crate) overflow: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct SelectionRange {
    pub(crate) start: (usize, usize), // (col, global_line)
    pub(crate) end: (usize, usize),
}

impl From<((usize, usize), (usize, usize))> for SelectionRange {
    fn from((start, end): ((usize, usize), (usize, usize))) -> Self {
        Self { start, end }
    }
}

impl From<SelectionRange> for ((usize, usize), (usize, usize)) {
    fn from(r: SelectionRange) -> Self {
        (r.start, r.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CursorShape {
    #[default]
    Block,
    Beam,
    Underline,
}

impl CursorShape {
    pub(crate) fn from_str_name(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "beam" | "bar" | "line" => CursorShape::Beam,
            "underline" => CursorShape::Underline,
            _ => CursorShape::Block,
        }
    }
}

impl TerminalState {
    /// Notificación de esquema de color (`CSI ?997;Ps n`, 1 = oscuro, 2 = claro): respuesta a
    /// `CSI ?996n` y aviso de cambio de tema con el modo 2031 activo.
    pub(crate) fn color_scheme_report_bytes(&self) -> Vec<u8> {
        let code = if self.theme.is_dark() { 1 } else { 2 };
        format!("\x1b[?997;{code}n").into_bytes()
    }

    pub(crate) fn register_underline_color(&mut self, color: Color) -> u8 {
        let key = color.to_u32_xrgb();
        if let Some(&id) = self.underline_color_map.get(&key) {
            return id;
        }
        if self.underline_colors.len() < (Cell::UNDERLINE_COLOR_MASK as usize) {
            let id = (self.underline_colors.len() + 1) as u8;
            self.underline_color_map.insert(key, id);
            self.underline_colors.push(color);
            id
        } else {
            self.underline_colors.len() as u8
        }
    }

    pub(crate) fn underline_color_of(&self, cell: &Cell) -> Option<Color> {
        let u_id = cell.underline_color_id();
        if u_id == 0 {
            None
        } else {
            let idx = (u_id as usize) - 1;
            self.underline_colors.get(idx).copied()
        }
    }

    /// Actualiza el tema de la terminal migrando coherentemente el estado del emulador y las celdas
    /// existentes en la rejilla, historial y pantalla alternativa.
    pub(crate) fn update_theme(&mut self, new_theme: Theme) {
        if self.theme == new_theme {
            return;
        }
        let old_theme = std::mem::replace(&mut self.theme, new_theme);

        // 1. Actualizar colores activos de escritura
        if self.current_fg == old_theme.foreground || self.current_fg == Color::TRANSPARENT {
            self.current_fg = self.theme.foreground;
        } else {
            for i in 0..16 {
                if self.current_fg == old_theme.ansi[i] {
                    self.current_fg = self.theme.ansi[i];
                    break;
                }
            }
        }

        if self.current_bg == old_theme.background || self.current_bg == Color::TRANSPARENT {
            self.current_bg = Color::TRANSPARENT;
        } else {
            for i in 0..16 {
                if self.current_bg == old_theme.ansi[i] {
                    self.current_bg = self.theme.ansi[i];
                    break;
                }
            }
        }

        // 2. Mapear colores de subrayado si coincidían
        for col in &mut self.underline_colors {
            if *col == old_theme.foreground {
                *col = self.theme.foreground;
            } else {
                for i in 0..16 {
                    if *col == old_theme.ansi[i] {
                        *col = self.theme.ansi[i];
                        break;
                    }
                }
            }
        }

        // 3. Mapear celdas en grid, scrollback y alt_grid
        let is_mode_switch = old_theme.is_dark() != self.theme.is_dark();
        let remap_cell = |cell: &mut Cell| {
            // Foreground
            if cell.fg == old_theme.foreground || cell.fg == Color::TRANSPARENT {
                cell.fg = self.theme.foreground;
            } else {
                let mut matched = false;
                for i in 0..16 {
                    if cell.fg == old_theme.ansi[i] {
                        let new_col = self.theme.ansi[i];
                        if is_mode_switch
                            && crate::theme::contrast_ratio(new_col, self.theme.background) < 2.0
                        {
                            cell.fg = self.theme.foreground;
                        } else {
                            cell.fg = new_col;
                        }
                        matched = true;
                        break;
                    }
                }
                if !matched && is_mode_switch {
                    let effective_bg =
                        if cell.bg == Color::TRANSPARENT || cell.bg == old_theme.background {
                            self.theme.background
                        } else {
                            cell.bg
                        };
                    let cr = crate::theme::contrast_ratio(cell.fg, effective_bg);
                    if cr < 2.2 {
                        if effective_bg != self.theme.background
                            && crate::theme::luminance(effective_bg) < 0.3
                        {
                            cell.fg = Color::from_rgb(255, 255, 255);
                        } else {
                            cell.fg = self.theme.foreground;
                        }
                    }
                }
            }

            // Background
            if cell.bg == old_theme.background {
                cell.bg = Color::TRANSPARENT;
            } else if cell.bg != Color::TRANSPARENT {
                for i in 0..16 {
                    if cell.bg == old_theme.ansi[i] {
                        cell.bg = self.theme.ansi[i];
                        break;
                    }
                }
            }
        };

        for row in &mut self.grid {
            for cell in row {
                remap_cell(cell);
            }
        }
        for row in &mut self.scrollback {
            for cell in row {
                remap_cell(cell);
            }
        }
        for row in &mut self.alt_grid {
            for cell in row {
                remap_cell(cell);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_cell_pack_meta_roundtrip(raw_link in 0u16..=1023u16, color_id in 0u8..=63u8) {
            let packed = Cell::pack_meta(raw_link, color_id);
            let cell = Cell { hyperlink_id: packed, ..Cell::default() };
            prop_assert_eq!(cell.raw_hyperlink_id(), raw_link);
            prop_assert_eq!(cell.underline_color_id(), color_id);
        }

        #[test]
        fn prop_cell_flags_consistency(flags: u8) {
            let cell = Cell { flags, ..Cell::default() };
            prop_assert_eq!(cell.bold(), flags & Cell::FLAG_BOLD != 0);
            prop_assert_eq!(cell.dim(), flags & Cell::FLAG_DIM != 0);
            prop_assert_eq!(cell.underline(), flags & Cell::FLAG_UNDERLINE != 0);
            prop_assert_eq!(cell.strikethrough(), flags & Cell::FLAG_STRIKETHROUGH != 0);
            prop_assert_eq!(cell.inverse(), flags & Cell::FLAG_INVERSE != 0);
            prop_assert_eq!(cell.is_grapheme(), flags & Cell::FLAG_GRAPHEME != 0);
        }
    }
}
