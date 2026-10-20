//! Celda de la rejilla (16 bytes) y tipos de datos asociados.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub c: char,
    pub width: u8,
    pub flags: u8,
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

    pub fn default_with_theme(theme: &Theme) -> Self {
        Self {
            c: ' ',
            width: 1,
            flags: 0,
            fg: theme.foreground,
            bg: Color::TRANSPARENT,
        }
    }
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
        }
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
