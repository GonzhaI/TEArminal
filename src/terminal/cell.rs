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
            | (((underline_color_id as u16) & Self::UNDERLINE_COLOR_MASK)
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
        if self.frames.is_empty() {
            return &self.rgba_data;
        }
        let total_ms: u64 = self.frames.iter().map(|f| f.delay_ms as u64).sum();
        if total_ms == 0 {
            return &self.frames[0].rgba_data;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut rem = now_ms % total_ms;
        for frame in &self.frames {
            let d = frame.delay_ms as u64;
            if rem < d {
                return &frame.rgba_data;
            }
            rem -= d;
        }
        self.frames
            .last()
            .map_or(&self.rgba_data, |frame| &frame.rgba_data)
    }

    /// Retorna el retraso en milisegundos restante para el próximo fotograma si es animada
    pub(crate) fn next_frame_delay_ms(&self) -> Option<u32> {
        if self.frames.len() <= 1 {
            return None;
        }
        let total_ms: u64 = self.frames.iter().map(|f| f.delay_ms as u64).sum();
        if total_ms == 0 {
            return None;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut rem = now_ms % total_ms;
        for frame in &self.frames {
            let d = frame.delay_ms as u64;
            if rem < d {
                let remaining = d - rem;
                return Some(remaining.max(10) as u32);
            }
            rem -= d;
        }
        Some(40)
    }
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
    pub(crate) remote_dir: Option<String>,
    pub(crate) remote_git_branch: Option<String>,
    pub(crate) is_active: bool,
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
