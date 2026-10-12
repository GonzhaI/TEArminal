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

#[derive(Debug, Clone)]
pub struct SidebarTabItem {
    #[allow(dead_code)]
    pub id: usize,
    pub title: String,
    pub is_active: bool,
    pub has_unread: bool,
    #[allow(dead_code)]
    pub pane_count: usize,
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

    /// Rellena un rectángulo de color sólido directamente en el buffer
    pub fn draw_rect(
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Color,
    ) {
        let c = color.to_u32_xrgb();
        let x_end = (x + w).min(buf_width);
        let y_end = (y + h).min(buf_height);
        for py in y..y_end {
            let row_start = py * buf_width;
            for px in x..x_end {
                buffer[row_start + px] = c;
            }
        }
    }

    /// Dibuja una cadena de texto en el buffer con rasterización y mezcla alpha
    pub fn draw_text(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        bold: bool,
    ) -> usize {
        let mut cur_x = start_x;
        for c in text.chars() {
            if cur_x >= max_x {
                break;
            }
            let glyph = self.get_glyph(c, bold);
            let glyph_w = glyph.width;
            let glyph_h = glyph.height;
            let glyph_x = cur_x as i32 + glyph.x_offset;
            let glyph_y = baseline_y as i32 - glyph.y_offset - glyph_h as i32;

            for gy in 0..glyph_h {
                let py = glyph_y + gy as i32;
                if py < 0 || py as usize >= buf_height {
                    continue;
                }
                for gx in 0..glyph_w {
                    let px = glyph_x + gx as i32;
                    if px < 0 || px as usize >= buf_width || px as usize >= max_x {
                        continue;
                    }

                    let alpha = glyph.bitmap[gy * glyph_w + gx];
                    if alpha > 0 {
                        let idx = (py as usize) * buf_width + (px as usize);
                        let bg = Color::from_rgb(
                            ((buffer[idx] >> 16) & 0xFF) as u8,
                            ((buffer[idx] >> 8) & 0xFF) as u8,
                            (buffer[idx] & 0xFF) as u8,
                        );
                        buffer[idx] = bg.blend_over(color, alpha);
                    }
                }
            }
            cur_x += self.char_width;
        }
        cur_x
    }

    /// Renderiza la barra lateral vertical con la lista de instancias / pestañas al estilo Ghostty
    pub fn render_sidebar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_width: usize,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_toggle_btn: bool,
        theme: &Theme,
    ) {
        if sidebar_width == 0 || width == 0 || height == 0 {
            return;
        }

        // 1. Fondo de la barra lateral (gris oscuro profundo Ghostty)
        let sidebar_bg = Color::from_rgb(18, 18, 20);
        Self::draw_rect(buffer, width, height, 0, 0, sidebar_width, height, sidebar_bg);

        // 2. Línea divisoria vertical sutil
        let divider_color = Color::from_rgb(38, 38, 43);
        Self::draw_rect(buffer, width, height, sidebar_width.saturating_sub(1), 0, 1, height, divider_color);

        // 3. Encabezado de la barra lateral (Header: "> AXON" + Botón "+")
        let header_y = 12;
        let header_baseline = header_y + self.baseline_y.min(16);
        let title_color = theme.ansi[8]; // Gris medio Ghostty
        let logo_color = theme.ansi[2];  // Verde Ghostty sin fondo
        self.draw_text(
            buffer,
            width,
            height,
            ">",
            14,
            header_baseline,
            sidebar_width.saturating_sub(40),
            logo_color,
            true,
        );
        self.draw_text(
            buffer,
            width,
            height,
            " AXON",
            14 + self.char_width,
            header_baseline,
            sidebar_width.saturating_sub(40),
            title_color,
            true,
        );

        // Botón "+" para crear nueva terminal
        let plus_btn_x = sidebar_width.saturating_sub(34);
        let plus_btn_y = 8;
        let plus_btn_w = 24;
        let plus_btn_h = 24;
        let plus_bg = if hovered_new_tab {
            Color::from_rgb(45, 45, 52)
        } else {
            theme.ansi[0] // Negro carbón Ghostty
        };
        Self::draw_rect(buffer, width, height, plus_btn_x, plus_btn_y, plus_btn_w, plus_btn_h, plus_bg);

        let plus_fg = if hovered_new_tab {
            theme.ansi[2] // Verde Ghostty
        } else {
            theme.ansi[7] // Blanco/gris claro Ghostty
        };
        let plus_baseline = plus_btn_y + self.baseline_y.min(18);
        self.draw_text(
            buffer,
            width,
            height,
            "+",
            plus_btn_x + (plus_btn_w.saturating_sub(self.char_width) / 2),
            plus_baseline,
            plus_btn_x + plus_btn_w,
            plus_fg,
            true,
        );

        // 4. Lista de pestañas verticales
        let tab_item_h = 34;
        let tab_spacing = 4;
        let list_start_y = 42;
        let list_max_y = height.saturating_sub(44);

        for (idx, tab) in tabs.iter().enumerate() {
            let tab_y = list_start_y + idx * (tab_item_h + tab_spacing);
            if tab_y + tab_item_h > list_max_y {
                break;
            }

            let is_hovered = hovered_tab == Some(idx);
            let tab_w = sidebar_width.saturating_sub(16);
            let tab_x = 8;

            // Fondo del ítem de pestaña
            let item_bg = if tab.is_active {
                Color::from_rgb(32, 32, 36)
            } else if is_hovered {
                Color::from_rgb(25, 25, 29)
            } else {
                sidebar_bg
            };

            Self::draw_rect(buffer, width, height, tab_x, tab_y, tab_w, tab_item_h, item_bg);

            // Indicador vertical verde en la pestaña activa
            if tab.is_active {
                let accent_color = theme.ansi[2]; // Verde Ghostty
                Self::draw_rect(buffer, width, height, tab_x, tab_y + 4, 3, tab_item_h.saturating_sub(8), accent_color);
            }

            // Título de la pestaña
            let text_color = if tab.is_active {
                theme.foreground // Ghostty foreground
            } else if is_hovered {
                theme.ansi[7] // Ghostty blanco/gris claro
            } else {
                theme.ansi[8] // Ghostty gris
            };

            let text_baseline = tab_y + self.baseline_y.min(22);
            let icon_text = "> ";
            self.draw_text(
                buffer,
                width,
                height,
                icon_text,
                tab_x + 8,
                text_baseline,
                tab_x + tab_w,
                if tab.is_active { theme.ansi[2] } else { text_color },
                tab.is_active,
            );

            let display_title = if tab.pane_count > 1 {
                format!("{} [{}]", tab.title, tab.pane_count)
            } else {
                tab.title.clone()
            };

            let title_x = tab_x + 8 + (self.char_width * 2);
            let title_max_x = tab_x + tab_w.saturating_sub(26);
            self.draw_text(
                buffer,
                width,
                height,
                &display_title,
                title_x,
                text_baseline,
                title_max_x,
                text_color,
                tab.is_active,
            );

            // Indicador de actividad / no leído si la pestaña tiene salida pendiente en segundo plano
            if tab.has_unread && !tab.is_active {
                let dot_x = tab_x + tab_w.saturating_sub(32);
                let dot_y = tab_y + (tab_item_h / 2).saturating_sub(3);
                let dot_color = theme.ansi[4]; // Azul Ghostty
                Self::draw_rect(buffer, width, height, dot_x, dot_y, 6, 6, dot_color);
            }

            // Botón cerrar "x"
            if tabs.len() > 1 && (tab.is_active || is_hovered) {
                let close_x = tab_x + tab_w.saturating_sub(22);
                let close_y = tab_y + 6;
                let close_size = 20;

                let is_close_hovered = hovered_close == Some(idx);
                let close_fg = if is_close_hovered {
                    theme.ansi[1] // Rojo Ghostty
                } else {
                    theme.ansi[8]
                };

                let close_baseline = close_y + self.baseline_y.min(15);
                self.draw_text(
                    buffer,
                    width,
                    height,
                    "x",
                    close_x + 5,
                    close_baseline,
                    close_x + close_size,
                    close_fg,
                    is_close_hovered,
                );
            }
        }

        // 5. Botón inferior para alternar/ocultar la barra lateral ("< Ocultar")
        let toggle_h = 30;
        let toggle_y = height.saturating_sub(toggle_h + 8);
        let toggle_x = 8;
        let toggle_w = sidebar_width.saturating_sub(16);

        // Separador superior sutil
        Self::draw_rect(buffer, width, height, 0, toggle_y.saturating_sub(6), sidebar_width, 1, divider_color);

        let toggle_bg = if hovered_toggle_btn {
            Color::from_rgb(32, 32, 38)
        } else {
            Color::from_rgb(22, 22, 26)
        };
        Self::draw_rect(buffer, width, height, toggle_x, toggle_y, toggle_w, toggle_h, toggle_bg);

        let toggle_fg = if hovered_toggle_btn {
            theme.ansi[2] // Verde Ghostty
        } else {
            theme.ansi[7]
        };
        let toggle_baseline = toggle_y + self.baseline_y.min(20);
        self.draw_text(
            buffer,
            width,
            height,
            "< Ocultar",
            toggle_x + 10,
            toggle_baseline,
            toggle_x + toggle_w,
            toggle_fg,
            hovered_toggle_btn,
        );

        // Indicador de atajo Ctrl+B a la derecha
        self.draw_text(
            buffer,
            width,
            height,
            "Ctrl+B",
            toggle_x + toggle_w.saturating_sub(58),
            toggle_baseline,
            toggle_x + toggle_w,
            theme.ansi[8],
            false,
        );
    }

    /// Renderiza un botón flotante sutil en la esquina inferior izquierda para volver a desplegar la barra lateral
    pub fn render_floating_sidebar_toggle(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        hovered: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let btn_w = 32;
        let btn_h = 30;
        let btn_x = 8;
        let btn_y = height.saturating_sub(btn_h + 8);

        let bg_color = if hovered {
            Color::from_rgb(38, 38, 46)
        } else {
            Color::from_rgb(24, 24, 28)
        };
        Self::draw_rect(buffer, width, height, btn_x, btn_y, btn_w, btn_h, bg_color);

        // Borde fino
        let border_color = Color::from_rgb(45, 45, 54);
        Self::draw_rect(buffer, width, height, btn_x, btn_y, btn_w, 1, border_color);
        Self::draw_rect(buffer, width, height, btn_x, btn_y + btn_h - 1, btn_w, 1, border_color);
        Self::draw_rect(buffer, width, height, btn_x, btn_y, 1, btn_h, border_color);
        Self::draw_rect(buffer, width, height, btn_x + btn_w - 1, btn_y, 1, btn_h, border_color);

        let icon_fg = if hovered {
            theme.ansi[2] // Verde Ghostty
        } else {
            theme.ansi[7]
        };

        let baseline = btn_y + self.baseline_y.min(20);
        self.draw_text(
            buffer,
            width,
            height,
            ">",
            btn_x + 11,
            baseline,
            btn_x + btn_w,
            icon_fg,
            true,
        );
    }

    /// Calcula las columnas y filas que caben en un panel según sus dimensiones en píxeles
    pub fn compute_pane_grid_size(&self, pane_width: usize, pane_height: usize) -> (usize, usize) {
        let avail_w = pane_width.saturating_sub(self.margin_x * 2);
        let avail_h = pane_height.saturating_sub(self.margin_y * 2);
        let cols = (avail_w / self.char_width).max(10);
        let rows = (avail_h / self.row_height).max(3);
        (cols, rows)
    }

    /// Calcula las columnas y filas que caben en el tamaño actual de la ventana
    pub fn compute_grid_size(&self, window_width: usize, window_height: usize, sidebar_offset: usize) -> (usize, usize) {
        let avail_w = window_width.saturating_sub(sidebar_offset);
        self.compute_pane_grid_size(avail_w, window_height)
    }

    pub fn pane_scrollbar_geometry(
        &self,
        rect_x: usize,
        rect_y: usize,
        rect_width: usize,
        rect_height: usize,
        terminal: &TerminalState,
    ) -> Option<(usize, usize, usize, usize)> {
        if terminal.scrollback.is_empty() {
            return None;
        }
        let track_w = 6;
        let track_x = (rect_x + rect_width).saturating_sub(track_w + 2);
        let track_y = rect_y + self.margin_y;
        let track_h = rect_height.saturating_sub(self.margin_y * 2);
        if track_h == 0 {
            return None;
        }

        let total_history = terminal.scrollback.len() + terminal.rows;
        let thumb_h = ((terminal.rows as f32 / total_history as f32) * track_h as f32).max(16.0).min(track_h as f32) as usize;
        let available_travel = track_h.saturating_sub(thumb_h);
        let ratio = 1.0 - (terminal.scroll_offset as f32 / terminal.scrollback.len() as f32);
        let thumb_y = track_y + (ratio * available_travel as f32) as usize;

        Some((track_x, thumb_y, track_w, thumb_h))
    }

    #[allow(dead_code)]
    pub fn scrollbar_geometry(
        &self,
        width: usize,
        height: usize,
        terminal: &TerminalState,
    ) -> Option<(usize, usize, usize, usize)> {
        self.pane_scrollbar_geometry(0, 0, width, height, terminal)
    }

    /// Renderiza un panel de terminal en un rectángulo específico del framebuffer
    pub fn render_pane(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
        theme: &Theme,
        cursor_visible: bool,
        is_active_pane: bool,
        has_multiple_panes: bool,
    ) {
        if rect_w == 0 || rect_h == 0 || width == 0 || height == 0 {
            return;
        }

        let bg_color = theme.background.to_u32_xrgb();
        let baseline_y = self.baseline_y;

        // 1. Limpieza del área del panel con el fondo oficial Ghostty (#161618)
        let max_y = (rect_y + rect_h).min(height);
        let max_x = (rect_x + rect_w).min(width);
        for py in rect_y..max_y {
            let row_start = py * width;
            for px in rect_x..max_x {
                buffer[row_start + px] = bg_color;
            }
        }

        let base_x = rect_x + self.margin_x;
        let base_y = rect_y + self.margin_y;
        let visible_rows = terminal.visible_rows();
        let view_start = terminal.visible_line_start();

        // 2. Renderizar filas y celdas
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let cell_y = base_y + (row_idx * self.row_height);
            if cell_y + self.row_height > max_y {
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
                let bar_x = base_x.saturating_sub(7);
                for y in (cell_y + 3)..(cell_y + self.row_height - 3) {
                    for x in bar_x..(bar_x + 3) {
                        if x >= rect_x && x < max_x && y >= rect_y && y < max_y {
                            buffer[y * width + x] = dot_color;
                        }
                    }
                }
            }

            for (col_idx, cell) in row.iter().enumerate() {
                let cell_x = base_x + (col_idx * self.char_width);
                if cell_x + self.char_width > max_x {
                    break;
                }

                let is_selected = if let Some(sel) = terminal.selection {
                    is_cell_selected(col_idx, row_idx, sel)
                } else {
                    false
                };

                let effective_bg = if is_selected {
                    theme.selection
                } else if cell.inverse {
                    if cell.fg != Color::TRANSPARENT {
                        cell.fg
                    } else {
                        theme.foreground
                    }
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
                            if x < max_x && y < max_y {
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
                        if x < max_x && line_y < max_y {
                            buffer[line_y * width + x] = fg_u32;
                        }
                    }
                }

                // Tachado (SGR 9)
                if cell.strikethrough {
                    let line_y = cell_y + (self.row_height / 2);
                    let fg_u32 = cell.fg.to_u32_xrgb();
                    for x in cell_x..(cell_x + self.char_width) {
                        if x < max_x && line_y < max_y {
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
                    if py < rect_y as i32 || py >= max_y as i32 {
                        continue;
                    }
                    for gx in 0..glyph_w {
                        let px = glyph_x + gx as i32;
                        if px < rect_x as i32 || px >= max_x as i32 {
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
            let cx = base_x + (terminal.cursor_x * self.char_width);
            let cy = base_y + (terminal.cursor_y * self.row_height);

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

            if is_active_pane {
                let cursor_color = theme.cursor.to_u32_xrgb();

                // Dibujar rectángulo del cursor
                for y in cy..(cy + self.row_height) {
                    for x in cx..(cx + c_width) {
                        if x >= rect_x && x < max_x && y >= rect_y && y < max_y {
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
                            if py < rect_y as i32 || py >= max_y as i32 {
                                continue;
                            }
                            for gx in 0..glyph_w {
                                let px = glyph_x + gx as i32;
                                if px < rect_x as i32 || px >= max_x as i32 {
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
            } else {
                // Cursor hueco para terminal inactiva en split
                let border_c = theme.ansi[8].to_u32_xrgb();
                for y in cy..(cy + self.row_height) {
                    for x in cx..(cx + c_width) {
                        if (y == cy || y == cy + self.row_height - 1 || x == cx || x == cx + c_width - 1)
                            && x >= rect_x && x < max_x && y >= rect_y && y < max_y
                        {
                            buffer[y * width + x] = border_c;
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
                let ix = (base_x + (img.col * self.char_width)) as i32;
                let iy = (base_y + (row_in_view as usize * self.row_height)) as i32;
                let target_w = img.width_cols * self.char_width;
                let target_h = img.height_rows * self.row_height;

                for dy in 0..target_h {
                    let py = iy + dy as i32;
                    if py < rect_y as i32 || py >= max_y as i32 {
                        continue;
                    }
                    let src_y = (dy * img.height_px as usize) / target_h;

                    for dx in 0..target_w {
                        let px = ix + dx as i32;
                        if px < rect_x as i32 || px >= max_x as i32 {
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

        // 5. Scrollbar del panel si tiene historial
        if !terminal.scrollback.is_empty() {
            if let Some((thumb_x, thumb_y, thumb_w, thumb_h)) = self.pane_scrollbar_geometry(rect_x, rect_y, rect_w, rect_h, terminal) {
                let track_y = rect_y + self.margin_y;
                let track_h = rect_h.saturating_sub(self.margin_y * 2);
                let track_color = theme.ansi[0].to_u32_xrgb();

                for y in track_y..(track_y + track_h) {
                    for x in thumb_x..(thumb_x + thumb_w) {
                        if x < max_x && y < max_y {
                            buffer[y * width + x] = track_color;
                        }
                    }
                }

                let thumb_color = if is_active_pane {
                    theme.ansi[12].to_u32_xrgb()
                } else {
                    theme.ansi[8].to_u32_xrgb()
                };

                for y in thumb_y..(thumb_y + thumb_h) {
                    for x in thumb_x..(thumb_x + thumb_w) {
                        if x < max_x && y < max_y {
                            buffer[y * width + x] = thumb_color;
                        }
                    }
                }
            }
        }

        // 6. Si hay múltiples paneles, dibujar un borde sutil indicador en el panel activo
        if has_multiple_panes && is_active_pane {
            let active_border = theme.ansi[2]; // Verde Ghostty sutil
            // Línea superior fina indicadora de foco
            Self::draw_rect(buffer, width, height, rect_x, rect_y, rect_w, 2, active_border);
        }
    }

    /// Renderiza un divisor entre paneles de terminal
    pub fn render_pane_divider(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        divider_color: Color,
    ) {
        Self::draw_rect(buffer, width, height, x, y, w, h, divider_color);
    }

    /// Renderiza directamente la cuadrícula del terminal al framebuffer de píxeles 32-bit de Windows
    #[allow(dead_code)]
    pub fn render(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_offset: usize,
        terminal: &TerminalState,
        theme: &Theme,
        cursor_visible: bool,
        _scrollbar_visible: bool,
        _scrollbar_hovered: bool,
    ) {
        let pane_w = width.saturating_sub(sidebar_offset);
        self.render_pane(
            buffer,
            width,
            height,
            sidebar_offset,
            0,
            pane_w,
            height,
            terminal,
            theme,
            cursor_visible,
            true,
            false,
        );
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
