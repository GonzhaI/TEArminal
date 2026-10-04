use crate::config::AxonConfig;
use crate::terminal::{Cell, SelectionRange, TerminalState};
use crate::theme::{Color, Theme};
use std::collections::HashMap;
use std::path::Path;

const EMBEDDED_JETBRAINS_REGULAR: &[u8] = include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");
const EMBEDDED_JETBRAINS_BOLD: &[u8] = include_bytes!("../assets/JetBrainsMonoNerdFont-Bold.ttf");

pub struct CachedGlyph {
    pub width: usize,
    pub height: usize,
    pub x_offset: i32,
    pub y_offset: i32,
    pub bitmap: Vec<u8>, // Grayscale 8-bit alpha (0-255)
}

pub struct TerminalRenderer {
    regular_font: fontdue::Font,
    bold_font: fontdue::Font,
    fallback_font: Option<fontdue::Font>,
    pub font_size: f32,
    pub char_width: usize,
    pub row_height: usize,
    pub baseline_y: usize,
    glyph_cache: HashMap<(char, bool), CachedGlyph>,
    pub margin_x: usize,
    pub margin_y: usize,
}

impl TerminalRenderer {
    pub fn new(config: &AxonConfig, scale_factor: f64) -> Self {
        // En Ghostty (macOS y Linux), font-size predeterminado es 13pt con espaciado natural compacto
        let font_size = (config.appearance.font_size * scale_factor as f32).round().max(10.0);
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let sys_fonts = Path::new(&windir).join("Fonts");

        // 1. Cargar fuentes oficiales de Ghostty (JetBrains Mono Nerd Font Regular + Bold)
        let (regular_font, bold_font) = Self::load_fonts(config);

        // 2. Cargar fuente de fallback para símbolos adicionales (Segoe UI Symbol)
        let fallback_font = {
            let sym_path = sys_fonts.join("seguisym.ttf");
            if sym_path.exists() {
                std::fs::read(&sym_path)
                    .ok()
                    .and_then(|bytes| fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok())
            } else {
                None
            }
        };

        // Medir dimensiones estándar de celda monoespaciada al estilo Ghostty
        let metrics = regular_font.metrics('M', font_size);
        let char_width = (metrics.advance_width.round() as usize).max(7);
        let line_height_raw = regular_font
            .horizontal_line_metrics(font_size)
            .map(|m| m.new_line_size)
            .unwrap_or(font_size * 1.25);
        let row_height = ((line_height_raw * config.appearance.line_spacing).round() as usize).max(15);
        let baseline_y = regular_font
            .horizontal_line_metrics(font_size)
            .map(|m| m.ascent.ceil() as usize)
            .unwrap_or(font_size.ceil() as usize);

        let margin_x = (10.0 * scale_factor as f32).round() as usize;
        let margin_y = (8.0 * scale_factor as f32).round() as usize;

        Self {
            regular_font,
            bold_font,
            fallback_font,
            font_size,
            char_width,
            row_height,
            baseline_y,
            glyph_cache: HashMap::new(),
            margin_x,
            margin_y,
        }
    }

    pub fn update_scale(&mut self, config: &AxonConfig, scale_factor: f64) {
        self.font_size = (config.appearance.font_size * scale_factor as f32).round().max(10.0);
        let metrics = self.regular_font.metrics('M', self.font_size);
        self.char_width = (metrics.advance_width.round() as usize).max(7);
        let line_height_raw = self.regular_font
            .horizontal_line_metrics(self.font_size)
            .map(|m| m.new_line_size)
            .unwrap_or(self.font_size * 1.25);
        self.row_height = ((line_height_raw * config.appearance.line_spacing).round() as usize).max(15);
        self.baseline_y = self.regular_font
            .horizontal_line_metrics(self.font_size)
            .map(|m| m.ascent.ceil() as usize)
            .unwrap_or(self.font_size.ceil() as usize);
        self.margin_x = (10.0 * scale_factor as f32).round() as usize;
        self.margin_y = (8.0 * scale_factor as f32).round() as usize;
        self.glyph_cache.clear();
    }

    fn load_fonts(config: &AxonConfig) -> (fontdue::Font, fontdue::Font) {
        // A. Si el usuario configuró una ruta personalizada en config
        if let Some(custom) = &config.appearance.font_path {
            if let Ok(bytes) = std::fs::read(custom) {
                if let Ok(font) = fontdue::Font::from_bytes(bytes.clone(), fontdue::FontSettings::default()) {
                    let bold = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap_or_else(|_| font.clone());
                    return (font, bold);
                }
            }
        }

        // B. Cargar JetBrains Mono Nerd Font nativo embebido (Ghostty official font & icons)
        let regular = fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_REGULAR, fontdue::FontSettings::default())
            .expect("Error al inicializar fuente JetBrains Mono Nerd Font Regular");
        let bold = fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_BOLD, fontdue::FontSettings::default())
            .expect("Error al inicializar fuente JetBrains Mono Nerd Font Bold");

        (regular, bold)
    }

    /// Obtiene o rasteriza un glifo a través de la caché SIMD de fontdue con soporte completo de Nerd Fonts
    pub fn get_glyph(&mut self, c: char, bold: bool) -> &CachedGlyph {
        let (font_size, has_regular_glyph, has_bold_glyph, has_fallback_glyph) = (
            self.font_size,
            self.regular_font.has_glyph(c),
            self.bold_font.has_glyph(c),
            self.fallback_font.as_ref().map_or(false, |f| f.has_glyph(c)),
        );

        let regular_ref = &self.regular_font;
        let bold_ref = &self.bold_font;
        let fallback_ref = self.fallback_font.as_ref();

        self.glyph_cache.entry((c, bold)).or_insert_with(|| {
            let font = if bold && has_bold_glyph {
                bold_ref
            } else if has_regular_glyph {
                regular_ref
            } else if has_fallback_glyph {
                fallback_ref.unwrap()
            } else if bold {
                bold_ref
            } else {
                regular_ref
            };

            let (metrics, bitmap) = font.rasterize(c, font_size);
            CachedGlyph {
                width: metrics.width,
                height: metrics.height,
                x_offset: metrics.xmin,
                y_offset: metrics.ymin,
                bitmap,
            }
        })
    }

    /// Calcula las columnas y filas que caben en el tamaño actual de la ventana
    pub fn compute_grid_size(&self, window_width: usize, window_height: usize) -> (usize, usize) {
        let avail_w = window_width.saturating_sub(self.margin_x * 2);
        let avail_h = window_height.saturating_sub(self.margin_y * 2);
        let cols = (avail_w / self.char_width).max(20);
        let rows = (avail_h / self.row_height).max(5);
        (cols, rows)
    }

    /// Renderiza directamente la cuadrícula del terminal al framebuffer de píxeles 32-bit de Windows
    pub fn render(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        terminal: &TerminalState,
        theme: &Theme,
        cursor_visible: bool,
    ) {
        let bg_color = theme.background.to_u32_xrgb();
        let baseline_y = self.baseline_y;

        // 1. Limpieza del framebuffer con el fondo oficial Ghostty (#161618)
        buffer.fill(bg_color);

        let visible_rows = terminal.visible_rows();
        let view_start = terminal.visible_line_start();

        // 2. Renderizar filas y celdas
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let cell_y = self.margin_y + (row_idx * self.row_height);
            if cell_y + self.row_height > height {
                break;
            }

            // Indicador semántico de comandos de agentes CLI (OSC 133 / Estilo Ghostty)
            let global_row = view_start + row_idx;
            if let Some(block) = terminal.command_blocks.iter().find(|b| b.prompt_line == global_row) {
                let dot_color = match block.exit_code {
                    Some(0) => Color::from_rgb(81, 226, 122).to_u32_xrgb(),  // Verde Ghostty #51e27a
                    Some(_) => Color::from_rgb(244, 56, 65).to_u32_xrgb(),   // Rojo Ghostty #f43841
                    None => Color::from_rgb(78, 146, 249).to_u32_xrgb(),     // Azul Ghostty #4e92f9
                };
                let bar_x = self.margin_x.saturating_sub(7);
                for y in (cell_y + 3)..(cell_y + self.row_height - 3) {
                    for x in bar_x..(bar_x + 3) {
                        if x < width && y < height {
                            buffer[y * width + x] = dot_color;
                        }
                    }
                }
            }

            for (col_idx, cell) in row.iter().enumerate() {
                let cell_x = self.margin_x + (col_idx * self.char_width);
                if cell_x + self.char_width > width {
                    break;
                }

                let is_selected = if let Some(sel) = terminal.selection {
                    is_cell_selected(col_idx, row_idx, sel)
                } else {
                    false
                };

                let effective_bg = if is_selected {
                    theme.selection
                } else if cell.bg != Color::TRANSPARENT {
                    cell.bg
                } else {
                    theme.background
                };

                // Dibujar fondo de celda si difiere del fondo global
                if effective_bg != theme.background {
                    let fill = effective_bg.to_u32_xrgb();
                    for y in cell_y..(cell_y + self.row_height) {
                        for x in cell_x..(cell_x + self.char_width) {
                            if x < width && y < height {
                                buffer[y * width + x] = fill;
                            }
                        }
                    }
                }

                // Subrayado (SGR 4)
                if cell.underline {
                    let line_y = cell_y + self.row_height - 2;
                    let fg_u32 = cell.fg.to_u32_xrgb();
                    for x in cell_x..(cell_x + self.char_width) {
                        if x < width && line_y < height {
                            buffer[line_y * width + x] = fg_u32;
                        }
                    }
                }

                // Tachado (SGR 9)
                if cell.strikethrough {
                    let line_y = cell_y + (self.row_height / 2);
                    let fg_u32 = cell.fg.to_u32_xrgb();
                    for x in cell_x..(cell_x + self.char_width) {
                        if x < width && line_y < height {
                            buffer[line_y * width + x] = fg_u32;
                        }
                    }
                }

                // Si es celda de espaciado para carácter ancho o espacio vacío
                if cell.width == 0 || cell.c == ' ' {
                    continue;
                }

                // Calcular color final de texto
                let fg_color = compute_cell_fg(cell, theme);

                // Obtener y mezclar glifo (con soporte completo de Nerd Fonts)
                let glyph = self.get_glyph(cell.c, cell.bold);
                let glyph_w = glyph.width;
                let glyph_h = glyph.height;
                let glyph_x = cell_x as i32 + glyph.x_offset;
                let glyph_y = (cell_y + baseline_y) as i32 - glyph.y_offset - glyph_h as i32;

                for gy in 0..glyph_h {
                    let py = glyph_y + gy as i32;
                    if py < 0 || py as usize >= height {
                        continue;
                    }
                    for gx in 0..glyph_w {
                        let px = glyph_x + gx as i32;
                        if px < 0 || px as usize >= width {
                            continue;
                        }

                        let alpha = glyph.bitmap[gy * glyph_w + gx];
                        if alpha > 0 {
                            let buf_idx = (py as usize) * width + (px as usize);
                            let bg = Color::from_rgb(
                                ((buffer[buf_idx] >> 16) & 0xFF) as u8,
                                ((buffer[buf_idx] >> 8) & 0xFF) as u8,
                                (buffer[buf_idx] & 0xFF) as u8,
                            );
                            buffer[buf_idx] = bg.blend_over(fg_color, alpha);
                        }
                    }
                }
            }
        }

        // 3. Renderizar cursor sólido al estilo Ghostty
        if terminal.scroll_offset == 0
            && terminal.cursor_visible
            && cursor_visible
            && terminal.cursor_y < terminal.rows
            && terminal.cursor_x < terminal.cols
        {
            let cx = self.margin_x + (terminal.cursor_x * self.char_width);
            let cy = self.margin_y + (terminal.cursor_y * self.row_height);

            let active_grid = if terminal.is_alt_screen {
                &terminal.alt_grid
            } else {
                &terminal.grid
            };

            let cell_under = if terminal.cursor_y < active_grid.len()
                && terminal.cursor_x < active_grid[terminal.cursor_y].len()
            {
                Some(&active_grid[terminal.cursor_y][terminal.cursor_x])
            } else {
                None
            };

            let c_width = if let Some(cell) = cell_under {
                if cell.width == 2 { self.char_width * 2 } else { self.char_width }
            } else {
                self.char_width
            };

            let cursor_color = theme.cursor.to_u32_xrgb();

            // Dibujar rectángulo del cursor
            for y in cy..(cy + self.row_height) {
                for x in cx..(cx + c_width) {
                    if x < width && y < height {
                        buffer[y * width + x] = cursor_color;
                    }
                }
            }

            // Dibujar carácter invertido debajo del cursor para legibilidad perfecta
            if let Some(cell) = cell_under {
                if cell.c != ' ' && cell.width > 0 {
                    let inv_fg = theme.background;
                    let glyph = self.get_glyph(cell.c, cell.bold);
                    let glyph_w = glyph.width;
                    let glyph_h = glyph.height;
                    let glyph_x = cx as i32 + glyph.x_offset;
                    let glyph_y = (cy + baseline_y) as i32 - glyph.y_offset - glyph_h as i32;

                    for gy in 0..glyph_h {
                        let py = glyph_y + gy as i32;
                        if py < 0 || py as usize >= height {
                            continue;
                        }
                        for gx in 0..glyph_w {
                            let px = glyph_x + gx as i32;
                            if px < 0 || px as usize >= width {
                                continue;
                            }

                            let alpha = glyph.bitmap[gy * glyph_w + gx];
                            if alpha > 0 {
                                let buf_idx = (py as usize) * width + (px as usize);
                                let bg = theme.cursor;
                                buffer[buf_idx] = bg.blend_over(inv_fg, alpha);
                            }
                        }
                    }
                }
            }
        }

        // 4. Renderizar imágenes inline (Protocolo iTerm2 / OSC 1337)
        let view_end = view_start + visible_rows.len();
        for img in &terminal.images {
            if img.line_index + img.height_rows > view_start && img.line_index < view_end {
                let row_in_view = img.line_index as isize - view_start as isize;
                let ix = (self.margin_x + (img.col * self.char_width)) as i32;
                let iy = (self.margin_y + (row_in_view as usize * self.row_height)) as i32;
                let target_w = img.width_cols * self.char_width;
                let target_h = img.height_rows * self.row_height;

                for dy in 0..target_h {
                    let py = iy + dy as i32;
                    if py < 0 || py as usize >= height {
                        continue;
                    }
                    let src_y = (dy * img.height_px as usize) / target_h;

                    for dx in 0..target_w {
                        let px = ix + dx as i32;
                        if px < 0 || px as usize >= width {
                            continue;
                        }
                        let src_x = (dx * img.width_px as usize) / target_w;

                        let src_idx = (src_y * img.width_px as usize + src_x) * 4;
                        if src_idx + 3 < img.rgba_data.len() {
                            let r = img.rgba_data[src_idx];
                            let g = img.rgba_data[src_idx + 1];
                            let b = img.rgba_data[src_idx + 2];
                            let a = img.rgba_data[src_idx + 3];

                            let buf_idx = (py as usize) * width + (px as usize);
                            if a == 255 {
                                buffer[buf_idx] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                            } else if a > 0 {
                                let bg = Color::from_rgb(
                                    ((buffer[buf_idx] >> 16) & 0xFF) as u8,
                                    ((buffer[buf_idx] >> 8) & 0xFF) as u8,
                                    (buffer[buf_idx] & 0xFF) as u8,
                                );
                                buffer[buf_idx] = bg.blend_over(Color::from_rgb(r, g, b), a);
                            }
                        }
                    }
                }
            }
        }

        // 5. Barra indicadora sutil de scrollback en el margen derecho
        if terminal.scroll_offset > 0 {
            let total_history = terminal.scrollback.len() + terminal.rows;
            let bar_height = ((terminal.rows as f32 / total_history as f32) * height as f32).max(24.0) as usize;
            let ratio = 1.0 - (terminal.scroll_offset as f32 / terminal.scrollback.len() as f32);
            let bar_y = self.margin_y + (ratio * (height - self.margin_y * 2 - bar_height) as f32) as usize;
            let bar_x = width.saturating_sub(5);
            let bar_color = Color::from_rgb(100, 110, 130).to_u32_xrgb();

            for y in bar_y..(bar_y + bar_height) {
                for x in bar_x..width {
                    if x < width && y < height {
                        buffer[y * width + x] = bar_color;
                    }
                }
            }
        }
    }
}

fn is_cell_selected(col: usize, row: usize, sel: SelectionRange) -> bool {
    let (mut p1, mut p2) = (sel.start, sel.end);
    if p1.1 > p2.1 || (p1.1 == p2.1 && p1.0 > p2.0) {
        std::mem::swap(&mut p1, &mut p2);
    }
    row >= p1.1 && row <= p2.1 && (row != p1.1 || col >= p1.0) && (row != p2.1 || col <= p2.0)
}

fn compute_cell_fg(cell: &Cell, theme: &Theme) -> Color {
    let base = if cell.inverse {
        if cell.bg != Color::TRANSPARENT { cell.bg } else { theme.background }
    } else {
        cell.fg
    };

    if cell.dim {
        Color::from_rgb(
            (base.r as f32 * 0.65) as u8,
            (base.g as f32 * 0.65) as u8,
            (base.b as f32 * 0.65) as u8,
        )
    } else {
        base
    }
}
