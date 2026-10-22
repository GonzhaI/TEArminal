//! Celda de la rejilla (16 bytes) y tipos de datos asociados.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub c: char,
    pub width: u8,
    pub flags: u8,
    pub hyperlink_id: u16,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    pub const FLAG_BOLD: u8 = 1 << 0;
    pub const FLAG_DIM: u8 = 1 << 1;
    pub const FLAG_ITALIC: u8 = 1 << 2;
    pub const FLAG_UNDERLINE: u8 = 1 << 3;
    pub const FLAG_STRIKETHROUGH: u8 = 1 << 4;
    pub const FLAG_INVERSE: u8 = 1 << 5;
    pub const FLAG_GRAPHEME: u8 = 1 << 6;

    pub const HYPERLINK_MASK: u16 = 0x03FF; // 10 bits: 0..=1023 (hasta 1023 URLs simultáneas)
    pub const UNDERLINE_COLOR_SHIFT: u16 = 10;
    pub const UNDERLINE_COLOR_MASK: u16 = 0x003F; // 6 bits: 0..=63 (hasta 63 colores RGB de subrayado)

    #[inline(always)]
    pub fn bold(&self) -> bool {
        self.flags & Self::FLAG_BOLD != 0
    }

    #[inline(always)]
    pub fn dim(&self) -> bool {
        self.flags & Self::FLAG_DIM != 0
    }

    #[inline(always)]
    pub fn underline(&self) -> bool {
        self.flags & Self::FLAG_UNDERLINE != 0
    }

    #[inline(always)]
    pub fn strikethrough(&self) -> bool {
        self.flags & Self::FLAG_STRIKETHROUGH != 0
    }

    #[inline(always)]
    pub fn inverse(&self) -> bool {
        self.flags & Self::FLAG_INVERSE != 0
    }

    #[inline(always)]
    pub fn is_grapheme(&self) -> bool {
        self.flags & Self::FLAG_GRAPHEME != 0
    }

    #[inline(always)]
    pub fn raw_hyperlink_id(&self) -> u16 {
        self.hyperlink_id & Self::HYPERLINK_MASK
    }

    #[inline(always)]
    pub fn underline_color_id(&self) -> u8 {
        ((self.hyperlink_id >> Self::UNDERLINE_COLOR_SHIFT) & Self::UNDERLINE_COLOR_MASK) as u8
    }

    #[inline(always)]
    pub fn pack_meta(hyperlink_id: u16, underline_color_id: u8) -> u16 {
        (hyperlink_id & Self::HYPERLINK_MASK)
            | (((underline_color_id as u16) & Self::UNDERLINE_COLOR_MASK)
                << Self::UNDERLINE_COLOR_SHIFT)
    }

    pub fn default_with_theme(theme: &Theme) -> Self {
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

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFrame {
    pub rgba_data: Vec<u8>,
    pub delay_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineImage {
    pub id: u64,
    pub line_index: usize,
    pub col: usize,
    pub width_cols: usize,
    pub height_rows: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba_data: Vec<u8>,
    pub frames: Vec<ImageFrame>,
}

impl InlineImage {
    /// Imagen ya escalada a `cols` x `rows` celdas, anclada en (`col`, `line_index`)
    pub fn new(
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
    pub fn with_frames(
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
    pub fn active_rgba(&self) -> &[u8] {
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
        &self.frames.last().unwrap().rgba_data
    }

    /// Retorna el retraso en milisegundos restante para el próximo fotograma si es animada
    pub fn next_frame_delay_ms(&self) -> Option<u32> {
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

#[derive(Clone, Debug, PartialEq)]
pub struct CommandBlock {
    pub prompt_line: usize,
    pub command_line: Option<usize>,
    pub output_start: Option<usize>,
    pub output_end: Option<usize>,
    pub exit_code: Option<i32>,
    /// Texto del comando sin el prompt (desde OSC 133;C), para copiarlo o repetirlo.
    pub command: Option<String>,
    /// Columna en la que empieza el comando dentro de `command_line`.
    pub command_col: usize,
    /// Marca de tiempo del inicio de la ejecución (OSC 133;C).
    pub started: Option<std::time::Instant>,
    /// Marca de tiempo del fin (OSC 133;D); su diferencia con `started` es la duración.
    pub finished: Option<std::time::Instant>,
    /// El usuario descartó los botones «Explicar» / «Ignorar» de esta tarjeta de error.
    pub error_dismissed: bool,
}

impl CommandBlock {
    /// Duración real de la ejecución, si el shell reportó inicio y fin.
    pub fn duration(&self) -> Option<std::time::Duration> {
        Some(self.finished?.duration_since(self.started?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshSession {
    pub destination: String,
    pub remote_dir: Option<String>,
    pub remote_git_branch: Option<String>,
    pub is_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: (usize, usize), // (col, global_line)
    pub end: (usize, usize),
}
