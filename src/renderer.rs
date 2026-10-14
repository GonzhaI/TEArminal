use crate::config::AxonConfig;
use crate::terminal::{Cell, SelectionRange, TerminalState};
use crate::theme::{Color, Theme};
use std::collections::HashMap;
use std::path::Path;

const EMBEDDED_JETBRAINS_REGULAR: &[u8] =
    include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");
const EMBEDDED_JETBRAINS_BOLD: &[u8] = include_bytes!("../assets/JetBrainsMonoNerdFont-Bold.ttf");
#[allow(dead_code)]
const EMBEDDED_INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-400.ttf");
const EMBEDDED_INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-500.ttf");
const EMBEDDED_INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-600.ttf");
const EMBEDDED_LUCIDE: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");

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
    pub is_settings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Terminal,
    Shortcuts,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAgentStatus {
    None,
    Claude,
    Agy,
    Gemini,
    Aider,
    Copilot,
    Ollama,
}

#[derive(Debug, Clone, Copy)]
pub struct TopTabBarRects {
    pub bar_h: usize,
    pub tabs_start_x: usize,
    pub tab_w: usize,
    pub plus_btn: (usize, usize, usize, usize),
    pub files_btn: (usize, usize, usize, usize),
    pub settings_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
}

impl TopTabBarRects {
    pub const HEIGHT: usize = 38;

    pub fn compute(width: usize, tabs_count: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 42;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);

        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 34), 5, 28, 28);
        let files_btn = (width.saturating_sub(ctrl_w * 3 + 68), 5, 28, 28);

        let right_reserved = ctrl_w * 3 + 76;
        let left_reserved = 76;
        let available_space = width.saturating_sub(left_reserved + right_reserved);

        let tab_w = if tabs_count > 0 {
            ((available_space.saturating_sub(36)) / tabs_count).clamp(50, 160)
        } else {
            160
        };

        let total_capsule_w = tabs_count * (tab_w + 4) + 36;
        let tabs_start_x = if available_space > total_capsule_w {
            left_reserved + (available_space - total_capsule_w) / 2
        } else {
            left_reserved
        };
        let plus_x = tabs_start_x + tabs_count * (tab_w + 4) + 4;
        let plus_btn = (plus_x, 5, 28, 28);

        Self {
            bar_h,
            tabs_start_x,
            tab_w,
            plus_btn,
            files_btn,
            settings_btn,
            win_min_btn,
            win_max_btn,
            win_close_btn,
        }
    }

    #[inline]
    pub fn tab_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        (
            self.tabs_start_x + idx * (self.tab_w + 4),
            4,
            self.tab_w,
            30,
        )
    }

    #[inline]
    pub fn close_btn_rect(&self, idx: usize) -> (usize, usize, usize, usize) {
        let tab = self.tab_rect(idx);
        (tab.0 + tab.2.saturating_sub(22), tab.1 + 6, 18, 18)
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TopTitleBarRects {
    pub bar_h: usize,
    pub settings_btn: (usize, usize, usize, usize),
    pub win_min_btn: (usize, usize, usize, usize),
    pub win_max_btn: (usize, usize, usize, usize),
    pub win_close_btn: (usize, usize, usize, usize),
}

impl TopTitleBarRects {
    pub const HEIGHT: usize = 34;

    pub fn compute(width: usize) -> Self {
        let bar_h = Self::HEIGHT;
        let ctrl_w = 42;
        let win_close_btn = (width.saturating_sub(ctrl_w), 0, ctrl_w, bar_h);
        let win_max_btn = (width.saturating_sub(ctrl_w * 2), 0, ctrl_w, bar_h);
        let win_min_btn = (width.saturating_sub(ctrl_w * 3), 0, ctrl_w, bar_h);
        let settings_btn = (width.saturating_sub(ctrl_w * 3 + 34), 3, 28, 28);
        Self {
            bar_h,
            settings_btn,
            win_min_btn,
            win_max_btn,
            win_close_btn,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsPageState {
    pub section: SettingsSection,
    pub hovered_section: Option<SettingsSection>,
    pub hovered_theme: Option<usize>,
    pub hovered_theme_mode: Option<usize>,
    pub hovered_tab_layout: Option<usize>,
    pub hovered_font_minus: bool,
    pub hovered_font_plus: bool,
    pub hovered_spacing_minus: bool,
    pub hovered_spacing_plus: bool,
    pub hovered_opacity_minus: bool,
    pub hovered_opacity_plus: bool,
    pub hovered_cursor_blink: bool,
    pub hovered_ghostty_prompt: bool,
    pub hovered_copy_on_select: bool,
    pub hovered_status_bar: bool,
    pub hovered_file_explorer: bool,
    pub hovered_restore_session: bool,
    pub hovered_open_file: bool,
    pub hovered_reset_defaults: bool,
}

impl Default for SettingsPageState {
    fn default() -> Self {
        Self {
            section: SettingsSection::Appearance,
            hovered_section: None,
            hovered_theme: None,
            hovered_theme_mode: None,
            hovered_tab_layout: None,
            hovered_font_minus: false,
            hovered_font_plus: false,
            hovered_spacing_minus: false,
            hovered_spacing_plus: false,
            hovered_opacity_minus: false,
            hovered_opacity_plus: false,
            hovered_cursor_blink: false,
            hovered_ghostty_prompt: false,
            hovered_copy_on_select: false,
            hovered_status_bar: false,
            hovered_file_explorer: false,
            hovered_restore_session: false,
            hovered_open_file: false,
            hovered_reset_defaults: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsPageRects {
    pub nav_appearance: (usize, usize, usize, usize),
    pub nav_terminal: (usize, usize, usize, usize),
    pub nav_shortcuts: (usize, usize, usize, usize),
    pub nav_about: (usize, usize, usize, usize),

    pub theme_buttons: [(usize, usize, usize, usize); 8],
    pub theme_mode_system_btn: (usize, usize, usize, usize),
    pub theme_mode_dark_btn: (usize, usize, usize, usize),
    pub theme_mode_light_btn: (usize, usize, usize, usize),

    pub tab_layout_vertical_btn: (usize, usize, usize, usize),
    pub tab_layout_horizontal_btn: (usize, usize, usize, usize),

    pub font_minus_btn: (usize, usize, usize, usize),
    pub font_plus_btn: (usize, usize, usize, usize),
    pub spacing_minus_btn: (usize, usize, usize, usize),
    pub spacing_plus_btn: (usize, usize, usize, usize),
    pub opacity_minus_btn: (usize, usize, usize, usize),
    pub opacity_plus_btn: (usize, usize, usize, usize),

    pub cursor_blink_btn: (usize, usize, usize, usize),
    pub ghostty_prompt_btn: (usize, usize, usize, usize),
    pub copy_on_select_btn: (usize, usize, usize, usize),
    pub status_bar_toggle_btn: (usize, usize, usize, usize),
    pub file_explorer_toggle_btn: (usize, usize, usize, usize),
    pub restore_session_btn: (usize, usize, usize, usize),
    pub open_file_btn: (usize, usize, usize, usize),
    pub reset_defaults_btn: (usize, usize, usize, usize),
    pub term_reset_btn: (usize, usize, usize, usize),
}

impl SettingsPageRects {
    pub fn compute(page_x: usize, page_y: usize, page_w: usize, _page_h: usize) -> Self {
        let card_w = 680.min(page_w.saturating_sub(72));
        let margin_x = if page_w > card_w {
            (page_w - card_w) / 2
        } else {
            36
        };
        let content_x = page_x + margin_x;
        let reset_defaults_btn = (content_x + card_w.saturating_sub(210), page_y + 36, 210, 28);

        let nav_y = page_y + 80;
        let nav_h = 32;

        let nav_w = (card_w.saturating_sub(24)) / 4;
        let nav_appearance = (content_x, nav_y, nav_w, nav_h);
        let nav_terminal = (content_x + nav_w + 8, nav_y, nav_w, nav_h);
        let nav_shortcuts = (content_x + (nav_w + 8) * 2, nav_y, nav_w, nav_h);
        let nav_about = (content_x + (nav_w + 8) * 3, nav_y, nav_w, nav_h);

        let card_y = page_y + 128;

        // Fila 1 y 2 temas (8 temas en 2 filas de 4 botones perfectamente alineados)
        let th_y1 = card_y + 44;
        let th_y2 = card_y + 80;
        let th_w = (card_w.saturating_sub(40 + 24)) / 4;
        let th_gap = 8;
        let theme_buttons = [
            (content_x + 20, th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap), th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 2, th_y1, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 3, th_y1, th_w, 28),
            (content_x + 20, th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap), th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 2, th_y2, th_w, 28),
            (content_x + 20 + (th_w + th_gap) * 3, th_y2, th_w, 28),
        ];

        // Tarjeta 2: Modo y Disposición de Pestañas
        let card2_y = card_y + 160;
        let label_col_w = 200;
        let ctrl_x = content_x + 20 + label_col_w;
        let ctrl_w = card_w.saturating_sub(label_col_w + 40);

        // 3 botones para Modo de color (Sistema, Oscuro, Claro)
        let btn3_w = (ctrl_w.saturating_sub(16)) / 3;
        let theme_mode_system_btn = (ctrl_x, card2_y + 40, btn3_w, 28);
        let theme_mode_dark_btn = (ctrl_x + btn3_w + 8, card2_y + 40, btn3_w, 28);
        let theme_mode_light_btn = (ctrl_x + (btn3_w + 8) * 2, card2_y + 40, btn3_w, 28);

        // 2 botones para Disposición de pestañas (Vertical, Superior)
        let btn2_w = (ctrl_w.saturating_sub(8)) / 2;
        let tab_layout_vertical_btn = (ctrl_x, card2_y + 78, btn2_w, 28);
        let tab_layout_horizontal_btn = (ctrl_x + btn2_w + 8, card2_y + 78, btn2_w, 28);

        // Tarjeta 3: Tipografía y ventana
        let card3_y = card2_y + 124;
        let step_btn_w = 32;
        let step_span = 140;
        let font_minus_btn = (ctrl_x, card3_y + 42, step_btn_w, 26);
        let font_plus_btn = (ctrl_x + step_span, card3_y + 42, step_btn_w, 26);

        let spacing_minus_btn = (ctrl_x, card3_y + 78, step_btn_w, 26);
        let spacing_plus_btn = (ctrl_x + step_span, card3_y + 78, step_btn_w, 26);

        let opacity_minus_btn = (ctrl_x, card3_y + 114, step_btn_w, 26);
        let opacity_plus_btn = (ctrl_x + step_span, card3_y + 114, step_btn_w, 26);

        // Terminal toggles (alineados a ctrl_x con ancho uniforme)
        let toggle_w = 170;
        let cursor_blink_btn = (ctrl_x, card_y + 42, toggle_w, 28);
        let ghostty_prompt_btn = (ctrl_x, card_y + 80, toggle_w, 28);
        let copy_on_select_btn = (ctrl_x, card_y + 118, toggle_w, 28);
        let status_bar_toggle_btn = (ctrl_x, card_y + 156, toggle_w, 28);
        let file_explorer_toggle_btn = (ctrl_x, card_y + 194, toggle_w, 28);
        let restore_session_btn = (ctrl_x, card_y + 232, toggle_w, 28);

        let cfg_y = card_y + 278 + 14;
        let cfg_btn_w = (card_w.saturating_sub(50)) / 2;
        let open_file_btn = (content_x + 20, cfg_y + 44, cfg_btn_w, 32);
        let term_reset_btn = (content_x + 30 + cfg_btn_w, cfg_y + 44, cfg_btn_w, 32);

        Self {
            nav_appearance,
            nav_terminal,
            nav_shortcuts,
            nav_about,
            theme_buttons,
            theme_mode_system_btn,
            theme_mode_dark_btn,
            theme_mode_light_btn,
            tab_layout_vertical_btn,
            tab_layout_horizontal_btn,
            font_minus_btn,
            font_plus_btn,
            spacing_minus_btn,
            spacing_plus_btn,
            opacity_minus_btn,
            opacity_plus_btn,
            cursor_blink_btn,
            ghostty_prompt_btn,
            copy_on_select_btn,
            status_bar_toggle_btn,
            file_explorer_toggle_btn,
            restore_session_btn,
            open_file_btn,
            reset_defaults_btn,
            term_reset_btn,
        }
    }

    #[inline]
    pub fn inside(mx: usize, my: usize, rect: (usize, usize, usize, usize)) -> bool {
        mx >= rect.0 && mx < rect.0 + rect.2 && my >= rect.1 && my < rect.1 + rect.3
    }
}

pub struct TerminalRenderer {
    regular_font: fontdue::Font,
    bold_font: fontdue::Font,
    fallback_font: Option<fontdue::Font>,
    pub inter_font: fontdue::Font,
    pub inter_semibold_font: fontdue::Font,
    pub lucide_font: fontdue::Font,
    pub font_size: f32,
    pub scale_factor: f64,
    pub char_width: usize,
    pub row_height: usize,
    pub baseline_y: usize,
    glyph_cache: HashMap<(char, bool), CachedGlyph>,
    ascii_cache: Box<[Option<CachedGlyph>; 256]>,
    ui_glyph_cache: HashMap<(char, u32, bool), (CachedGlyph, f32)>, // (c, size_bits, semibold) -> (glyph, advance)
    icon_glyph_cache: HashMap<(char, u32), CachedGlyph>,            // (c, size_bits) -> glyph
    pub margin_x: usize,
    pub margin_y: usize,
}

impl TerminalRenderer {
    pub fn new(config: &AxonConfig, scale_factor: f64) -> Self {
        // En Ghostty (macOS y Linux), font-size predeterminado es 13pt con espaciado natural compacto
        let font_size = (config.appearance.font_size * scale_factor as f32)
            .round()
            .max(10.0);
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let sys_fonts = Path::new(&windir).join("Fonts");

        // 1. Cargar fuentes oficiales de Ghostty (JetBrains Mono Nerd Font Regular + Bold)
        let (regular_font, bold_font) = Self::load_fonts(config);

        // 2. Cargar fuente de fallback para símbolos adicionales (Segoe UI Symbol)
        let fallback_font = {
            let sym_path = sys_fonts.join("seguisym.ttf");
            if sym_path.exists() {
                std::fs::read(&sym_path).ok().and_then(|bytes| {
                    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
                })
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
        let row_height =
            ((line_height_raw * config.appearance.line_spacing).round() as usize).max(15);
        let baseline_y = regular_font
            .horizontal_line_metrics(font_size)
            .map(|m| m.ascent.ceil() as usize)
            .unwrap_or(font_size.ceil() as usize);

        let margin_x = (10.0 * scale_factor as f32).round() as usize;
        let margin_y = (8.0 * scale_factor as f32).round() as usize;

        let inter_font =
            fontdue::Font::from_bytes(EMBEDDED_INTER_MEDIUM, fontdue::FontSettings::default())
                .expect("Error al inicializar fuente Inter Medium");
        let inter_semibold_font =
            fontdue::Font::from_bytes(EMBEDDED_INTER_SEMIBOLD, fontdue::FontSettings::default())
                .expect("Error al inicializar fuente Inter SemiBold");
        let lucide_font =
            fontdue::Font::from_bytes(EMBEDDED_LUCIDE, fontdue::FontSettings::default())
                .expect("Error al inicializar fuente Lucide");

        Self {
            regular_font,
            bold_font,
            fallback_font,
            inter_font,
            inter_semibold_font,
            lucide_font,
            font_size,
            scale_factor,
            char_width,
            row_height,
            baseline_y,
            glyph_cache: HashMap::new(),
            ascii_cache: Box::new([(); 256].map(|_| None)),
            ui_glyph_cache: HashMap::new(),
            icon_glyph_cache: HashMap::new(),
            margin_x,
            margin_y,
        }
    }

    pub fn update_scale(&mut self, config: &AxonConfig, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.font_size = (config.appearance.font_size * scale_factor as f32)
            .round()
            .max(10.0);
        let metrics = self.regular_font.metrics('M', self.font_size);
        self.char_width = (metrics.advance_width.round() as usize).max(7);
        let line_height_raw = self
            .regular_font
            .horizontal_line_metrics(self.font_size)
            .map(|m| m.new_line_size)
            .unwrap_or(self.font_size * 1.25);
        self.row_height =
            ((line_height_raw * config.appearance.line_spacing).round() as usize).max(15);
        self.baseline_y = self
            .regular_font
            .horizontal_line_metrics(self.font_size)
            .map(|m| m.ascent.ceil() as usize)
            .unwrap_or(self.font_size.ceil() as usize);
        self.margin_x = (10.0 * scale_factor as f32).round() as usize;
        self.margin_y = (8.0 * scale_factor as f32).round() as usize;
        self.glyph_cache.clear();
        for slot in self.ascii_cache.iter_mut() {
            *slot = None;
        }
        self.ui_glyph_cache.clear();
        self.icon_glyph_cache.clear();
    }

    fn load_fonts(config: &AxonConfig) -> (fontdue::Font, fontdue::Font) {
        // A. Si el usuario configuró una ruta personalizada en config
        if let Some(custom) = &config.appearance.font_path {
            if let Ok(bytes) = std::fs::read(custom) {
                if let Ok(font) =
                    fontdue::Font::from_bytes(bytes.clone(), fontdue::FontSettings::default())
                {
                    let bold = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
                        .unwrap_or_else(|_| font.clone());
                    return (font, bold);
                }
            }
        }

        // B. Cargar JetBrains Mono Nerd Font nativo embebido (Ghostty official font & icons)
        let regular =
            fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_REGULAR, fontdue::FontSettings::default())
                .expect("Error al inicializar fuente JetBrains Mono Nerd Font Regular");
        let bold =
            fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_BOLD, fontdue::FontSettings::default())
                .expect("Error al inicializar fuente JetBrains Mono Nerd Font Bold");

        (regular, bold)
    }

    fn rasterize_glyph(&self, c: char, bold: bool) -> CachedGlyph {
        let (font_size, has_regular_glyph, has_bold_glyph, has_fallback_glyph) = (
            self.font_size,
            self.regular_font.has_glyph(c),
            self.bold_font.has_glyph(c),
            self.fallback_font
                .as_ref()
                .map_or(false, |f| f.has_glyph(c)),
        );

        let regular_ref = &self.regular_font;
        let bold_ref = &self.bold_font;
        let fallback_ref = self.fallback_font.as_ref();

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
    }

    /// Obtiene o rasteriza un glifo a través de la caché SIMD de fontdue con soporte completo de Nerd Fonts y tabla plana O(1) para ASCII
    pub fn get_glyph(&mut self, c: char, bold: bool) -> &CachedGlyph {
        let code = c as u32;
        if code < 128 {
            let idx = code as usize + if bold { 128 } else { 0 };
            if self.ascii_cache[idx].is_none() {
                let glyph = self.rasterize_glyph(c, bold);
                self.ascii_cache[idx] = Some(glyph);
            }
            return self.ascii_cache[idx].as_ref().unwrap();
        }

        let regular_ref = &self.regular_font;
        let bold_ref = &self.bold_font;
        let fallback_ref = self.fallback_font.as_ref();
        let (font_size, has_regular_glyph, has_bold_glyph, has_fallback_glyph) = (
            self.font_size,
            regular_ref.has_glyph(c),
            bold_ref.has_glyph(c),
            fallback_ref.map_or(false, |f| f.has_glyph(c)),
        );

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
    #[allow(dead_code)]
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

    /// Mide el ancho en píxeles de un texto renderizado con Inter
    pub fn measure_ui_text(&self, text: &str, size_px: f32, semibold: bool) -> usize {
        let font = if semibold {
            &self.inter_semibold_font
        } else {
            &self.inter_font
        };
        let mut width = 0.0f32;
        for c in text.chars() {
            let metrics = font.metrics(c, size_px);
            width += metrics.advance_width;
        }
        width.round() as usize
    }

    /// Dibuja texto proporcional moderno utilizando la tipografía Inter
    pub fn draw_ui_text(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        size_px: f32,
        semibold: bool,
    ) -> usize {
        let size_bits = size_px.to_bits();
        let mut cur_x = start_x as f32;

        for c in text.chars() {
            if cur_x as usize >= max_x {
                break;
            }

            let inter_ref = &self.inter_font;
            let inter_semi_ref = &self.inter_semibold_font;
            let fallback_ref = self.fallback_font.as_ref();

            let (glyph, advance) = self
                .ui_glyph_cache
                .entry((c, size_bits, semibold))
                .or_insert_with(|| {
                    let font = if semibold && inter_semi_ref.has_glyph(c) {
                        inter_semi_ref
                    } else if inter_ref.has_glyph(c) {
                        inter_ref
                    } else if let Some(fb) = fallback_ref {
                        if fb.has_glyph(c) {
                            fb
                        } else {
                            inter_ref
                        }
                    } else {
                        inter_ref
                    };

                    let (metrics, bitmap) = font.rasterize(c, size_px);
                    (
                        CachedGlyph {
                            width: metrics.width,
                            height: metrics.height,
                            x_offset: metrics.xmin,
                            y_offset: metrics.ymin,
                            bitmap,
                        },
                        metrics.advance_width,
                    )
                });

            let glyph_w = glyph.width;
            let glyph_h = glyph.height;
            let glyph_x = (cur_x.round() as i32) + glyph.x_offset;
            let glyph_y = baseline_y as i32 - glyph.y_offset - glyph_h as i32;
            let adv = *advance;

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
            cur_x += adv;
        }

        cur_x.round() as usize
    }

    /// Dibuja un icono Lucide vectorial perfectamente centrado en una caja
    pub fn draw_lucide_icon(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        icon: crate::ui::LucideIcon,
        center_x: usize,
        center_y: usize,
        size_px: f32,
        color: Color,
    ) {
        let c = icon.as_char();
        let size_bits = size_px.to_bits();
        let lucide_ref = &self.lucide_font;

        let glyph = self
            .icon_glyph_cache
            .entry((c, size_bits))
            .or_insert_with(|| {
                let (metrics, bitmap) = lucide_ref.rasterize(c, size_px);
                CachedGlyph {
                    width: metrics.width,
                    height: metrics.height,
                    x_offset: metrics.xmin,
                    y_offset: metrics.ymin,
                    bitmap,
                }
            });

        let glyph_w = glyph.width;
        let glyph_h = glyph.height;
        let glyph_x = (center_x as i32) - (glyph_w as i32 / 2);
        let glyph_y = (center_y as i32) - (glyph_h as i32 / 2);

        for gy in 0..glyph_h {
            let py = glyph_y + gy as i32;
            if py < 0 || py as usize >= buf_height {
                continue;
            }
            for gx in 0..glyph_w {
                let px = glyph_x + gx as i32;
                if px < 0 || px as usize >= buf_width {
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
    }

    /// Renderiza la barra lateral vertical con la lista de instancias / pestañas al estilo Ghostty
    pub fn render_sidebar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        sidebar_width: usize,
        status_bar_h: usize,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_settings_btn: bool,
        hovered_toggle_btn: bool,
        theme: &Theme,
    ) {
        if sidebar_width == 0 || width == 0 || height == 0 {
            return;
        }

        let usable_h = height.saturating_sub(status_bar_h);

        // 1. Fondo de la barra lateral dinámico acorde al tema (por encima de la barra inferior)
        let sidebar_bg = theme.sidebar_bg();
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            0,
            sidebar_width,
            usable_h,
            sidebar_bg,
        );

        // 2. Línea divisoria vertical sutil
        let divider_color = theme.border_color();
        Self::draw_rect(
            buffer,
            width,
            height,
            sidebar_width.saturating_sub(1),
            0,
            1,
            usable_h,
            divider_color,
        );

        // 3. Encabezado de la barra lateral (Header: Axon + Botón Settings + Botón New Tab)
        let header_cy = 20;
        let header_base = header_cy + 4;
        let logo_color = theme.accent_color();

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            20,
            header_cy,
            14.5,
            logo_color,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            32,
            header_base,
            sidebar_width.saturating_sub(68),
            theme.foreground,
            12.5,
            true,
        );

        // Botón "⚙" (Settings) en el header
        let settings_btn_x = sidebar_width.saturating_sub(62);
        let settings_btn_y = 8;
        let settings_btn_w = 24;
        let settings_btn_h = 24;
        let settings_bg = if hovered_settings_btn {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            settings_btn_x,
            settings_btn_y,
            settings_btn_w,
            settings_btn_h,
            6,
            settings_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            settings_btn_x,
            settings_btn_y,
            settings_btn_w,
            settings_btn_h,
            6,
            theme.border_color(),
        );
        let settings_fg = if hovered_settings_btn {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            settings_btn_x + (settings_btn_w / 2),
            settings_btn_y + (settings_btn_h / 2),
            12.0,
            settings_fg,
        );

        // Botón "+" para crear nueva terminal
        let plus_btn_x = sidebar_width.saturating_sub(34);
        let plus_btn_y = 8;
        let plus_btn_w = 24;
        let plus_btn_h = 24;
        let plus_bg = if hovered_new_tab {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, plus_btn_x, plus_btn_y, plus_btn_w, plus_btn_h, 6, plus_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            plus_btn_x,
            plus_btn_y,
            plus_btn_w,
            plus_btn_h,
            6,
            theme.border_color(),
        );

        let plus_fg = if hovered_new_tab {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            plus_btn_x + (plus_btn_w / 2),
            plus_btn_y + (plus_btn_h / 2),
            12.0,
            plus_fg,
        );

        // 4. Lista de pestañas verticales
        let tab_item_h = 36;
        let tab_spacing = 4;
        let list_start_y = 42;
        let toggle_h = 30;
        let toggle_y = usable_h.saturating_sub(toggle_h + 8);
        let list_max_y = toggle_y.saturating_sub(8);

        for (idx, tab) in tabs.iter().enumerate() {
            let tab_y = list_start_y + idx * (tab_item_h + tab_spacing);
            if tab_y + tab_item_h > list_max_y {
                break;
            }

            let is_hovered = hovered_tab == Some(idx);
            let tab_w = sidebar_width.saturating_sub(16);
            let tab_x = 8;

            // Fondo del ítem de pestaña con elevación sutil de tarjeta y esquinas redondeadas
            let item_bg = if tab.is_active {
                theme.surface_bg()
            } else if is_hovered {
                theme.surface_hover()
            } else {
                sidebar_bg
            };

            crate::ui::draw_rounded_rect(
                buffer, width, height, tab_x, tab_y, tab_w, tab_item_h, 8, item_bg,
            );

            // Borde sutil si está activo o en hover
            if tab.is_active {
                crate::ui::stroke_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x,
                    tab_y,
                    tab_w,
                    tab_item_h,
                    8,
                    theme.border_color(),
                );
            }

            // Indicador vertical de acento en la pestaña activa
            if tab.is_active {
                let accent_color = theme.accent_color();
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    tab_x + 2,
                    tab_y + 8,
                    3,
                    tab_item_h.saturating_sub(16),
                    2,
                    accent_color,
                );
            }

            // Icono representativo de la pestaña
            let icon = if tab.is_settings {
                crate::ui::LucideIcon::Settings
            } else if tab.title.to_lowercase().contains("claude")
                || tab.title.to_lowercase().contains("agent")
                || tab.title.to_lowercase().contains("ai")
            {
                crate::ui::LucideIcon::Sparkles
            } else if tab.title.to_lowercase().contains("ssh") {
                crate::ui::LucideIcon::Server
            } else {
                crate::ui::LucideIcon::Terminal
            };

            let item_cy = tab_y + (tab_item_h / 2);
            let icon_cx = tab_x + 18;
            let icon_color = if tab.is_active {
                theme.accent_color()
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };
            self.draw_lucide_icon(
                buffer, width, height, icon, icon_cx, item_cy, 12.5, icon_color,
            );

            // Título de la pestaña en Inter
            let text_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };

            let display_title = if tab.pane_count > 1 {
                format!("{} [{}]", tab.title, tab.pane_count)
            } else {
                tab.title.clone()
            };

            let title_x = icon_cx + 12;
            let title_base = item_cy + 4;
            let title_max_x = tab_x + tab_w.saturating_sub(26);
            self.draw_ui_text(
                buffer,
                width,
                height,
                &display_title,
                title_x,
                title_base,
                title_max_x,
                text_color,
                11.5,
                tab.is_active,
            );

            // Indicador de actividad / no leído si la pestaña tiene salida pendiente en segundo plano
            if tab.has_unread && !tab.is_active {
                let dot_x = tab_x + tab_w.saturating_sub(32);
                let dot_y = tab_y + (tab_item_h / 2).saturating_sub(3);
                let dot_color = theme.ansi[4];
                crate::ui::draw_rounded_rect(
                    buffer, width, height, dot_x, dot_y, 6, 6, 3, dot_color,
                );
            }

            // Botón cerrar "x" con Lucide X
            if tabs.len() > 1 && (tab.is_active || is_hovered) {
                let close_x = tab_x + tab_w.saturating_sub(22);
                let close_y = tab_y + 8;
                let close_size = 20;

                let is_close_hovered = hovered_close == Some(idx);
                let close_fg = if is_close_hovered {
                    theme.ansi[1]
                } else {
                    theme.ansi[8]
                };

                let close_cx = close_x + (close_size / 2);
                let close_cy = close_y + (close_size / 2) - 1;
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_cx,
                    close_cy,
                    11.0,
                    close_fg,
                );
            }
        }

        // 5. Botón inferior para alternar/ocultar la barra lateral ("< Ocultar")
        let sidebar_btn_x = 8;
        let sidebar_btn_w = sidebar_width.saturating_sub(16);

        // Separador superior sutil
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            toggle_y.saturating_sub(6),
            sidebar_width,
            1,
            divider_color,
        );

        let toggle_bg = if hovered_toggle_btn {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            sidebar_btn_x,
            toggle_y,
            sidebar_btn_w,
            toggle_h,
            6,
            toggle_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            sidebar_btn_x,
            toggle_y,
            sidebar_btn_w,
            toggle_h,
            6,
            divider_color,
        );

        let toggle_fg = if hovered_toggle_btn {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        let toggle_cy = toggle_y + (toggle_h / 2);
        let toggle_base = toggle_cy + 4;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            sidebar_btn_x + 14,
            toggle_cy,
            12.0,
            toggle_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ocultar",
            sidebar_btn_x + 26,
            toggle_base,
            sidebar_btn_x + sidebar_btn_w.saturating_sub(54),
            toggle_fg,
            11.5,
            hovered_toggle_btn,
        );

        // Indicador de atajo Ctrl+B a la derecha en Inter
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ctrl+B",
            sidebar_btn_x + sidebar_btn_w.saturating_sub(44),
            toggle_base,
            sidebar_btn_x + sidebar_btn_w - 4,
            theme.ansi[8],
            10.5,
            false,
        );
    }

    /// Renderiza un botón flotante sutil en la esquina inferior izquierda para volver a desplegar la barra lateral o abrir ajustes
    pub fn render_floating_sidebar_toggle(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        status_bar_h: usize,
        hovered_toggle: bool,
        hovered_settings: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let btn_w = 32;
        let btn_h = 30;
        let btn_x = 8;
        let btn_y = height.saturating_sub(btn_h + 8 + status_bar_h);
        let border_color = theme.border_color();

        // Botón 1: Desplegar barra lateral [ PanelLeft ]
        let bg_color = if hovered_toggle {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, btn_x, btn_y, btn_w, btn_h, 6, bg_color,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            6,
            border_color,
        );

        let icon_fg = if hovered_toggle {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::PanelLeft,
            btn_x + (btn_w / 2),
            btn_y + (btn_h / 2),
            13.0,
            icon_fg,
        );

        // Botón 2: Abrir Ajustes flotante [ Settings ]
        let set_x = 44;
        let set_bg = if hovered_settings {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, set_x, btn_y, btn_w, btn_h, 6, set_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            set_x,
            btn_y,
            btn_w,
            btn_h,
            6,
            border_color,
        );

        let set_fg = if hovered_settings {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            set_x + (btn_w / 2),
            btn_y + (btn_h / 2),
            13.0,
            set_fg,
        );
    }

    /// Renderiza la barra horizontal de pestañas superiores con estilo de cápsula flotante (Floating Island)
    pub fn render_top_tab_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        tabs: &[SidebarTabItem],
        hovered_tab: Option<usize>,
        hovered_close: Option<usize>,
        hovered_new_tab: bool,
        hovered_files: bool,
        hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let rects = TopTabBarRects::compute(width, tabs.len());
        let bar_h = rects.bar_h;
        let bg = theme.background;
        let border_c = theme.border_color();

        // 1. Limpiar la franja superior con el fondo de la ventana
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, bg);

        // 2. Logo sutil a la izquierda: Icono Terminal / Axon
        let logo_center_x = 24;
        let logo_center_y = bar_h / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            logo_center_x,
            logo_center_y,
            15.0,
            theme.accent_color(),
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            logo_center_x + 12,
            logo_center_y + 4,
            rects.tabs_start_x.saturating_sub(8),
            theme.foreground,
            12.5,
            true,
        );

        // 3. Cápsula flotante central (Floating Island)
        let island_x = rects.tabs_start_x;
        let island_y = 4;
        let island_w = (rects.plus_btn.0 + rects.plus_btn.2 + 4).saturating_sub(island_x);
        let island_h = 30;

        // Fondo de la cápsula y borde sutil
        let island_bg = Color::from_rgba(
            theme.surface_bg().r(),
            theme.surface_bg().g(),
            theme.surface_bg().b(),
            200,
        );
        crate::ui::draw_rounded_rect(
            buffer, width, height, island_x, island_y, island_w, island_h, 14, island_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, island_x, island_y, island_w, island_h, 14, border_c,
        );

        // Renderizado de cada pestaña dentro de la cápsula
        for (idx, tab) in tabs.iter().enumerate() {
            let r = rects.tab_rect(idx);
            let is_hovered = hovered_tab == Some(idx);

            if tab.is_active {
                // Pestaña activa: cápsula interna iluminada y elevada
                let active_tab_bg = Color::from_rgba(255, 255, 255, 22);
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 2,
                    r.1 + 2,
                    r.2.saturating_sub(4),
                    r.3.saturating_sub(4),
                    10,
                    active_tab_bg,
                );
            } else if is_hovered {
                let hover_tab_bg = Color::from_rgba(255, 255, 255, 10);
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    r.0 + 2,
                    r.1 + 2,
                    r.2.saturating_sub(4),
                    r.3.saturating_sub(4),
                    10,
                    hover_tab_bg,
                );
            }

            // Icono representativo de la pestaña
            let icon = if tab.is_settings {
                crate::ui::LucideIcon::Settings
            } else if tab.title.to_lowercase().contains("claude")
                || tab.title.to_lowercase().contains("agent")
                || tab.title.to_lowercase().contains("ai")
            {
                crate::ui::LucideIcon::Sparkles
            } else if tab.title.to_lowercase().contains("ssh") {
                crate::ui::LucideIcon::Server
            } else {
                crate::ui::LucideIcon::Terminal
            };

            let icon_center_x = r.0 + 16;
            let icon_center_y = r.1 + (r.3 / 2);
            let icon_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };

            self.draw_lucide_icon(
                buffer,
                width,
                height,
                icon,
                icon_center_x,
                icon_center_y,
                13.0,
                icon_color,
            );

            // Título de la pestaña con Inter
            let text_color = if tab.is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };

            let title_str = if tab.pane_count > 1 {
                format!("{} [{}]", tab.title, tab.pane_count)
            } else {
                tab.title.clone()
            };

            let text_x = icon_center_x + 12;
            let text_y = icon_center_y + 4;
            let text_max_x = r.0 + r.2.saturating_sub(24);
            self.draw_ui_text(
                buffer,
                width,
                height,
                &title_str,
                text_x,
                text_y,
                text_max_x,
                text_color,
                11.5,
                tab.is_active,
            );

            // Botón de cerrar pestaña con Lucide 'X'
            if tabs.len() > 1 && (tab.is_active || is_hovered) {
                let close_r = rects.close_btn_rect(idx);
                let is_close_hovered = hovered_close == Some(idx);
                let close_color = if is_close_hovered {
                    theme.ansi[1]
                } else {
                    theme.ansi[8]
                };

                let close_cx = close_r.0 + (close_r.2 / 2);
                let close_cy = close_r.1 + (close_r.3 / 2);
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    crate::ui::LucideIcon::X,
                    close_cx,
                    close_cy,
                    11.0,
                    close_color,
                );
            }
        }

        // 4. Botón '+' para nueva pestaña dentro de la cápsula
        let plus_r = rects.plus_btn;
        if hovered_new_tab {
            let plus_hover_bg = Color::from_rgba(255, 255, 255, 14);
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                plus_r.0,
                plus_r.1 + 2,
                plus_r.2,
                plus_r.3.saturating_sub(4),
                8,
                plus_hover_bg,
            );
        }
        let plus_fg = if hovered_new_tab {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let plus_cx = plus_r.0 + (plus_r.2 / 2);
        let plus_cy = plus_r.1 + (plus_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Plus,
            plus_cx,
            plus_cy,
            13.0,
            plus_fg,
        );

        // 5. Botón 'Explorador de archivos' a la derecha
        let files_r = rects.files_btn;
        let files_bg = if explorer_visible || hovered_files {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 8, files_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, files_r.0, files_r.1, files_r.2, files_r.3, 8, border_c,
        );

        let files_fg = if explorer_visible || hovered_files {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let files_cx = files_r.0 + (files_r.2 / 2);
        let files_cy = files_r.1 + (files_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::FolderTree,
            files_cx,
            files_cy,
            13.5,
            files_fg,
        );

        // 6. Botón 'Ajustes' a la derecha
        let set_r = rects.settings_btn;
        let set_bg = if hovered_settings {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 8, set_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 8, border_c,
        );

        let set_fg = if hovered_settings {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        let set_cx = set_r.0 + (set_r.2 / 2);
        let set_cy = set_r.1 + (set_r.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            set_cx,
            set_cy,
            13.5,
            set_fg,
        );

        // 7. Botones de ventana cliente (Minimizar, Maximizar, Cerrar)
        self.draw_window_controls(
            buffer,
            width,
            height,
            rects.win_min_btn,
            rects.win_max_btn,
            rects.win_close_btn,
            hovered_win_min,
            hovered_win_max,
            hovered_win_close,
            is_maximized,
            theme,
        );

        // 8. Línea divisoria inferior
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, border_c);
    }

    /// Dibuja los botones de control de ventana estándar (Minimizar, Maximizar, Cerrar)
    pub fn draw_window_controls(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        min_btn: (usize, usize, usize, usize),
        max_btn: (usize, usize, usize, usize),
        close_btn: (usize, usize, usize, usize),
        hovered_min: bool,
        hovered_max: bool,
        hovered_close: bool,
        is_maximized: bool,
        theme: &Theme,
    ) {
        // Minimizar
        if hovered_min {
            Self::draw_rect(buffer, width, height, min_btn.0, min_btn.1, min_btn.2, min_btn.3, theme.surface_hover());
        }
        let min_fg = if hovered_min { theme.foreground } else { theme.ansi[7] };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Minus,
            min_btn.0 + (min_btn.2 / 2),
            min_btn.1 + (min_btn.3 / 2),
            13.0,
            min_fg,
        );

        // Maximizar / Restaurar
        if hovered_max {
            Self::draw_rect(buffer, width, height, max_btn.0, max_btn.1, max_btn.2, max_btn.3, theme.surface_hover());
        }
        let max_fg = if hovered_max { theme.foreground } else { theme.ansi[7] };
        let max_icon = if is_maximized {
            crate::ui::LucideIcon::Copy
        } else {
            crate::ui::LucideIcon::Square
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            max_icon,
            max_btn.0 + (max_btn.2 / 2),
            max_btn.1 + (max_btn.3 / 2),
            12.0,
            max_fg,
        );

        // Cerrar (con hover rojo nativo Windows)
        if hovered_close {
            let close_bg = Color::from_rgb(224, 76, 76);
            Self::draw_rect(buffer, width, height, close_btn.0, close_btn.1, close_btn.2, close_btn.3, close_bg);
        }
        let close_fg = if hovered_close {
            Color::from_rgb(255, 255, 255)
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::X,
            close_btn.0 + (close_btn.2 / 2),
            close_btn.1 + (close_btn.3 / 2),
            13.0,
            close_fg,
        );
    }

    /// Renderiza la barra superior de título cuando las pestañas son verticales (Sidebar)
    pub fn render_top_title_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        title: &str,
        git_branch: Option<&str>,
        hovered_settings: bool,
        hovered_win_min: bool,
        hovered_win_max: bool,
        hovered_win_close: bool,
        is_maximized: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let rects = TopTitleBarRects::compute(width);
        let bar_h = rects.bar_h;
        let bg = theme.background;
        let border_c = theme.border_color();

        // 1. Limpiar fondo de barra de título
        Self::draw_rect(buffer, width, height, 0, 0, width, bar_h, bg);

        // 2. Logo sutil a la izquierda
        let logo_cx = 24;
        let logo_cy = bar_h / 2;
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::SquareTerminal,
            logo_cx,
            logo_cy,
            14.0,
            theme.accent_color(),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Axon",
            logo_cx + 12,
            logo_cy + 4,
            120,
            theme.foreground,
            12.0,
            true,
        );

        // 3. Título centralizado
        let branch_suffix = if let Some(b) = git_branch {
            format!("  ⎇ {}", b)
        } else {
            String::new()
        };
        let center_title = format!("◈ {}{}", title, branch_suffix);
        let title_w = self.measure_ui_text(&center_title, 11.5, false);
        let center_x = if width > title_w + 300 {
            (width - title_w) / 2
        } else {
            140
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &center_title,
            center_x,
            logo_cy + 4,
            width.saturating_sub(180),
            theme.ansi[7],
            11.5,
            false,
        );

        // 4. Botón Ajustes
        let set_r = rects.settings_btn;
        let set_bg = if hovered_settings {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 6, set_bg);
        crate::ui::stroke_rounded_rect(buffer, width, height, set_r.0, set_r.1, set_r.2, set_r.3, 6, border_c);
        let set_fg = if hovered_settings { theme.foreground } else { theme.ansi[8] };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Settings,
            set_r.0 + (set_r.2 / 2),
            set_r.1 + (set_r.3 / 2),
            13.0,
            set_fg,
        );

        // 5. Botones de ventana cliente
        self.draw_window_controls(
            buffer,
            width,
            height,
            rects.win_min_btn,
            rects.win_max_btn,
            rects.win_close_btn,
            hovered_win_min,
            hovered_win_max,
            hovered_win_close,
            is_maximized,
            theme,
        );

        // 6. Borde inferior
        Self::draw_rect(buffer, width, height, 0, bar_h - 1, width, 1, border_c);
    }

    /// Renderiza la barra de estado informativa inferior (Status Bar) con tipografía Inter e iconos Lucide
    pub fn render_status_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        _sidebar_offset: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        _cols: usize,
        _rows: usize,
        ai_status: AiAgentStatus,
        _explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height < 26 {
            return;
        }

        let bar_h = 26;
        let bar_y = height - bar_h;
        let bar_bg = theme.sidebar_bg();
        let border_c = theme.border_color();

        let bar_x = 0;
        let bar_w = width;

        // 1. Fondo y borde superior 1px abarcando todo el ancho inferior
        Self::draw_rect(buffer, width, height, bar_x, bar_y, bar_w, bar_h, bar_bg);
        Self::draw_rect(buffer, width, height, bar_x, bar_y, bar_w, 1, border_c);

        let center_y = bar_y + (bar_h / 2);
        let baseline = center_y + 4;
        let mut cur_x = 12;

        // 2. Izquierda: CWD o SSH Host con icono Lucide
        if let Some(ssh) = ssh_host {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Server,
                cur_x + 6,
                center_y,
                12.0,
                theme.ansi[2],
            );
            cur_x += 16;
            let label = format!("ssh: {}", ssh);
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                &label,
                cur_x,
                baseline,
                width.saturating_sub(200),
                theme.ansi[2],
                11.0,
                true,
            );
            cur_x = adv + 16;
        } else if let Some(dir) = cwd {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Folder,
                cur_x + 6,
                center_y,
                12.0,
                theme.ansi[8],
            );
            cur_x += 16;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                dir,
                cur_x,
                baseline,
                (cur_x + 320).min(width.saturating_sub(200)),
                theme.ansi[7],
                11.0,
                false,
            );
            cur_x = adv + 16;
        }

        // Git branch con icono Lucide GitBranch
        if let Some(branch) = git_branch {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::GitBranch,
                cur_x + 6,
                center_y,
                12.0,
                theme.accent_color(),
            );
            cur_x += 16;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                branch,
                cur_x,
                baseline,
                width.saturating_sub(150),
                theme.accent_color(),
                11.0,
                true,
            );
            let _ = adv;
        }

        // 3. Derecha: Badge de Agente IA o estado de conexión (REMOTO / LOCAL)
        let (ai_icon, ai_name, ai_fg) = match ai_status {
            AiAgentStatus::Claude => (crate::ui::LucideIcon::Sparkles, "CLAUDE", theme.ansi[5]),
            AiAgentStatus::Agy => (crate::ui::LucideIcon::Sparkles, "AGY", theme.ansi[6]),
            AiAgentStatus::Gemini => (crate::ui::LucideIcon::Sparkles, "GEMINI", theme.ansi[4]),
            AiAgentStatus::Aider => (crate::ui::LucideIcon::Bot, "AIDER", theme.ansi[2]),
            AiAgentStatus::Copilot => (crate::ui::LucideIcon::Bot, "COPILOT", theme.ansi[6]),
            AiAgentStatus::Ollama => (crate::ui::LucideIcon::Bot, "OLLAMA", theme.ansi[3]),
            AiAgentStatus::None => {
                if ssh_host.is_some() {
                    (crate::ui::LucideIcon::Server, "REMOTO", theme.ansi[2])
                } else {
                    (crate::ui::LucideIcon::Dot, "LOCAL", theme.ansi[8])
                }
            }
        };

        let ai_text_w = self.measure_ui_text(ai_name, 11.0, true);
        let badge_x = width.saturating_sub(ai_text_w + 32);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            ai_icon,
            badge_x + 6,
            center_y,
            12.0,
            ai_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            ai_name,
            badge_x + 16,
            baseline,
            width,
            ai_fg,
            11.0,
            true,
        );
    }

    /// Renderiza el panel lateral derecho del explorador de archivos con estilo overlay translúcido (Floating Island)
    pub fn render_file_explorer(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 || !explorer.visible {
            return;
        }

        let panel_w = crate::explorer::FILE_EXPLORER_WIDTH;
        if width <= panel_w + 16 {
            return;
        }
        let margin_top = top_offset + 8;
        let margin_bottom = status_bar_h + 8;
        let panel_x = width - panel_w - 8;
        let panel_y = margin_top;
        let panel_h = height.saturating_sub(margin_top + margin_bottom);

        let border_color = theme.border_color();

        // 1. Fondo del panel con esquinas redondeadas y translucidez suave
        let bg_color = Color::from_rgba(
            theme.surface_bg().r(),
            theme.surface_bg().g(),
            theme.surface_bg().b(),
            230,
        );
        crate::ui::draw_rounded_rect(
            buffer, width, height, panel_x, panel_y, panel_w, panel_h, 12, bg_color,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            12,
            border_color,
        );

        // 2. Encabezado del explorador (36px)
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let header_center_y = panel_y + (header_h / 2);
        let header_base = header_center_y + 4;

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::FolderTree,
            panel_x + 20,
            header_center_y,
            13.5,
            theme.accent_color(),
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "ARCHIVOS",
            panel_x + 32,
            header_base,
            panel_x + panel_w.saturating_sub(60),
            theme.foreground,
            11.5,
            true,
        );

        // Botón '↻' refrescar
        let ref_x = panel_x + panel_w.saturating_sub(56);
        let ref_bg = if explorer.hovered_refresh_btn {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        crate::ui::draw_rounded_rect(buffer, width, height, ref_x, panel_y + 7, 22, 22, 6, ref_bg);
        let ref_fg = if explorer.hovered_refresh_btn {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::RefreshCw,
            ref_x + 11,
            panel_y + 18,
            12.0,
            ref_fg,
        );

        // Botón 'x' cerrar
        let close_x = panel_x + panel_w.saturating_sub(28);
        let close_bg = if explorer.hovered_close_btn {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            close_x,
            panel_y + 7,
            22,
            22,
            6,
            close_bg,
        );
        let close_fg = if explorer.hovered_close_btn {
            theme.ansi[1]
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::X,
            close_x + 11,
            panel_y + 18,
            12.0,
            close_fg,
        );

        // 3. Sub-encabezado con ruta actual y botón [..] (28px)
        let sub_y = panel_y + header_h;
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let sub_center_y = sub_y + (sub_h / 2);
        let up_base = sub_center_y + 4;

        let up_x = panel_x + 10;
        let up_bg = if explorer.hovered_up_btn {
            theme.surface_hover()
        } else {
            Color::from_rgba(255, 255, 255, 8)
        };
        crate::ui::draw_rounded_rect(buffer, width, height, up_x, sub_y + 4, 26, 20, 6, up_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            up_x,
            sub_y + 4,
            26,
            20,
            6,
            border_color,
        );

        let up_fg = if explorer.hovered_up_btn {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::ArrowUp,
            up_x + 13,
            sub_y + 14,
            11.5,
            up_fg,
        );

        // Nombre de la carpeta actual con Inter
        let dir_name = explorer
            .root_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| explorer.root_dir.to_string_lossy().to_string());
        let dir_label = format!("{}/", dir_name);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &dir_label,
            up_x + 34,
            up_base,
            panel_x + panel_w - 8,
            theme.ansi[8],
            11.0,
            false,
        );

        // 4. Lista de elementos (archivos y carpetas)
        let list_y = sub_y + sub_h + 4;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(6);

        if explorer.items.is_empty() {
            let empty_base = list_y + 14;
            self.draw_ui_text(
                buffer,
                width,
                height,
                "(Directorio vacío)",
                panel_x + 16,
                empty_base,
                panel_x + panel_w - 16,
                theme.ansi[8],
                11.0,
                false,
            );
            return;
        }

        for (rel_idx, item) in explorer
            .items
            .iter()
            .skip(explorer.scroll_offset)
            .enumerate()
        {
            let actual_idx = explorer.scroll_offset + rel_idx;
            let item_y = list_y + rel_idx * item_h;
            if item_y + item_h > list_max_y {
                break;
            }

            let is_hovered = explorer.hovered_index == Some(actual_idx);
            if is_hovered {
                let hover_row_bg = Color::from_rgba(255, 255, 255, 12);
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 6,
                    item_y,
                    panel_w.saturating_sub(12),
                    item_h,
                    6,
                    hover_row_bg,
                );
            }

            let indent = item.depth * 12;
            let item_center_y = item_y + (item_h / 2);
            let item_base = item_center_y + 4;
            let content_x = panel_x + 12 + indent;

            if item.is_dir {
                // Directorio: Lucide Chevron + Folder/FolderOpen
                let chevron_icon = if item.is_expanded {
                    crate::ui::LucideIcon::ChevronDown
                } else {
                    crate::ui::LucideIcon::ChevronRight
                };
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    chevron_icon,
                    content_x + 4,
                    item_center_y,
                    10.5,
                    theme.ansi[8],
                );

                let folder_icon = if item.is_expanded {
                    crate::ui::LucideIcon::FolderOpen
                } else {
                    crate::ui::LucideIcon::Folder
                };
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    folder_icon,
                    content_x + 18,
                    item_center_y,
                    12.5,
                    theme.accent_color(),
                );

                let dir_color = if is_hovered {
                    theme.foreground
                } else {
                    theme.ansi[7]
                };
                let label = format!("{}/", item.name);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &label,
                    content_x + 28,
                    item_base,
                    panel_x + panel_w - 8,
                    dir_color,
                    11.5,
                    is_hovered,
                );
            } else {
                // Archivo: Lucide file icons por extensión
                let ext = item.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let (file_icon, file_fg) = match ext {
                    "rs" => (crate::ui::LucideIcon::FileCode, theme.ansi[1]),
                    "toml" | "json" | "yaml" | "yml" => {
                        (crate::ui::LucideIcon::FileJson, theme.ansi[3])
                    }
                    "md" | "txt" => (crate::ui::LucideIcon::FileText, theme.ansi[6]),
                    "png" | "jpg" | "svg" => (crate::ui::LucideIcon::File, theme.ansi[5]),
                    "exe" | "bat" | "ps1" => (crate::ui::LucideIcon::FileCog, theme.ansi[2]),
                    _ => (crate::ui::LucideIcon::File, theme.ansi[8]),
                };

                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    file_icon,
                    content_x + 10,
                    item_center_y,
                    12.0,
                    file_fg,
                );

                let text_color = if is_hovered {
                    theme.foreground
                } else {
                    theme.ansi[7]
                };
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &item.name,
                    content_x + 22,
                    item_base,
                    panel_x + panel_w - 8,
                    text_color,
                    11.5,
                    false,
                );
            }
        }
    }

    fn draw_button(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        rect: (usize, usize, usize, usize),
        text: &str,
        hovered: bool,
        active_accent: bool,
        theme: &Theme,
    ) {
        let (bx, by, bw, bh) = rect;
        let bg = if active_accent {
            theme.surface_hover()
        } else if hovered {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 7, bg);
        let b_border = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.ansi[8]
        } else {
            theme.border_color()
        };
        crate::ui::stroke_rounded_rect(buffer, buf_width, buf_height, bx, by, bw, bh, 7, b_border);

        let fg = if active_accent {
            theme.accent_color()
        } else if hovered {
            theme.foreground
        } else {
            theme.ansi[7]
        };
        let text_w = self.measure_ui_text(text, 11.5, active_accent);
        let tx = bx + (bw.saturating_sub(text_w) / 2);
        let baseline = by + (bh / 2) + 4;
        self.draw_ui_text(
            buffer,
            buf_width,
            buf_height,
            text,
            tx,
            baseline,
            bx + bw,
            fg,
            11.5,
            active_accent,
        );
    }

    fn draw_stepper_value(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        minus_rect: (usize, usize, usize, usize),
        plus_rect: (usize, usize, usize, usize),
        text: &str,
        theme: &Theme,
    ) {
        let span_start = minus_rect.0 + minus_rect.2;
        let span_end = plus_rect.0;
        let span_w = span_end.saturating_sub(span_start);
        let text_w = self.measure_ui_text(text, 11.5, true);
        let tx = span_start + (span_w.saturating_sub(text_w) / 2);
        let baseline = minus_rect.1 + (minus_rect.3 / 2) + 4;
        self.draw_ui_text(
            buffer,
            width,
            height,
            text,
            tx,
            baseline,
            span_end.saturating_sub(2),
            theme.accent_color(),
            11.5,
            true,
        );
    }

    fn draw_label(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        x: usize,
        y: usize,
        max_x: usize,
        label: &str,
        theme: &Theme,
    ) {
        let baseline = y + 18;
        self.draw_ui_text(
            buffer,
            buf_width,
            buf_height,
            label,
            x,
            baseline,
            max_x,
            theme.foreground,
            11.5,
            false,
        );
    }

    fn draw_card(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        title: &str,
        theme: &Theme,
    ) {
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.surface_bg());
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.border_color());

        // Barra superior de encabezado de la tarjeta
        Self::draw_rect(buffer, width, height, x, y + 30, w, 1, theme.border_color());
        let baseline = y + 20;
        self.draw_ui_text(
            buffer,
            width,
            height,
            title,
            x + 14,
            baseline,
            x + w,
            theme.ansi[8],
            11.0,
            true,
        );
    }

    /// Renderiza la página completa de la pestaña de Ajustes y Personalización de Axon
    pub fn render_settings_page(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        page_x: usize,
        page_y: usize,
        config: &AxonConfig,
        state: &SettingsPageState,
        theme: &Theme,
    ) {
        if width <= page_x || height <= page_y {
            return;
        }

        let page_w = width - page_x;
        let page_h = height - page_y;
        let rects = SettingsPageRects::compute(page_x, page_y, page_w, page_h);

        // 1. Fondo de la página de ajustes a partir de page_y (para no sobreescribir barra superior)
        Self::draw_rect(
            buffer,
            width,
            height,
            page_x,
            page_y,
            page_w,
            page_h,
            theme.background,
        );

        // 2. Encabezado principal con icono Lucide Sliders y tipografía Inter
        let content_x = rects.nav_appearance.0;
        let card_w = 680.min(page_w.saturating_sub(72));

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Sliders,
            content_x + 8,
            page_y + 38,
            15.0,
            theme.accent_color(),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Ajustes de Axon",
            content_x + 24,
            page_y + 43,
            content_x + card_w.saturating_sub(215),
            theme.foreground,
            14.5,
            true,
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "Personaliza apariencia, comportamiento del terminal y atajos de teclado",
            content_x,
            page_y + 62,
            content_x + card_w.saturating_sub(215),
            theme.ansi[8],
            11.5,
            false,
        );

        // Botón Restablecer valores por defecto en el encabezado
        let r_btn = rects.reset_defaults_btn;
        let r_bg = if state.hovered_reset_defaults {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(
            buffer, width, height, r_btn.0, r_btn.1, r_btn.2, r_btn.3, 6, r_bg,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            r_btn.0,
            r_btn.1,
            r_btn.2,
            r_btn.3,
            6,
            theme.border_color(),
        );
        let r_fg = if state.hovered_reset_defaults {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        let r_cy = r_btn.1 + (r_btn.3 / 2);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::RefreshCw,
            r_btn.0 + 16,
            r_cy,
            12.0,
            r_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Restablecer por defecto",
            r_btn.0 + 28,
            r_cy + 4,
            r_btn.0 + r_btn.2,
            r_fg,
            11.5,
            false,
        );

        // Línea divisoria superior
        Self::draw_rect(
            buffer,
            width,
            height,
            content_x,
            page_y + 72,
            card_w,
            1,
            theme.border_color(),
        );

        // 3. Barra de navegación de secciones (Pills redondeadas con iconos Lucide e Inter)
        let sections = [
            (
                SettingsSection::Appearance,
                crate::ui::LucideIcon::Palette,
                "Apariencia",
                rects.nav_appearance,
            ),
            (
                SettingsSection::Terminal,
                crate::ui::LucideIcon::Terminal,
                "Terminal",
                rects.nav_terminal,
            ),
            (
                SettingsSection::Shortcuts,
                crate::ui::LucideIcon::Keyboard,
                "Atajos de Teclado",
                rects.nav_shortcuts,
            ),
            (
                SettingsSection::About,
                crate::ui::LucideIcon::Info,
                "Acerca de",
                rects.nav_about,
            ),
        ];

        for (sec, icon, title, r) in sections {
            let is_active = state.section == sec;
            let is_hovered = state.hovered_section == Some(sec);
            let bg = if is_active {
                theme.surface_bg()
            } else if is_hovered {
                theme.surface_hover()
            } else {
                theme.sidebar_bg()
            };
            crate::ui::draw_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 8, bg);

            let border_c = if is_active {
                theme.accent_color()
            } else {
                theme.border_color()
            };
            crate::ui::stroke_rounded_rect(buffer, width, height, r.0, r.1, r.2, r.3, 8, border_c);

            let fg = if is_active {
                theme.foreground
            } else if is_hovered {
                theme.ansi[7]
            } else {
                theme.ansi[8]
            };
            let cy = r.1 + (r.3 / 2);
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                icon,
                r.0 + 16,
                cy,
                12.5,
                if is_active { theme.accent_color() } else { fg },
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                title,
                r.0 + 28,
                cy + 4,
                r.0 + r.2,
                fg,
                11.5,
                is_active,
            );
        }

        // Línea divisoria bajo navegación
        Self::draw_rect(
            buffer,
            width,
            height,
            content_x,
            page_y + 116,
            card_w,
            1,
            theme.border_color(),
        );

        // 4. Renderizar contenido según la sección activa
        let card_y = page_y + 128;

        match state.section {
            SettingsSection::Appearance => {
                // Tarjeta 1: Temas
                let th_card_h = 150;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    card_y,
                    card_w,
                    th_card_h,
                    "TEMA VISUAL DE COLOR",
                    theme,
                );

                for (idx, &theme_name) in Theme::AVAILABLE_THEMES.iter().enumerate() {
                    let r = rects.theme_buttons[idx];
                    let is_current = config.appearance.theme.eq_ignore_ascii_case(theme_name);
                    let is_hover = state.hovered_theme == Some(idx);
                    let btn_bg = if is_current {
                        theme.surface_hover()
                    } else if is_hover {
                        theme.surface_hover()
                    } else {
                        theme.surface_bg()
                    };
                    crate::ui::draw_rounded_rect(
                        buffer, width, height, r.0, r.1, r.2, r.3, 7, btn_bg,
                    );

                    let border_c = if is_current {
                        theme.accent_color()
                    } else if is_hover {
                        theme.ansi[8]
                    } else {
                        theme.border_color()
                    };
                    crate::ui::stroke_rounded_rect(
                        buffer, width, height, r.0, r.1, r.2, r.3, 7, border_c,
                    );

                    let fg = if is_current {
                        theme.accent_color()
                    } else if is_hover {
                        theme.foreground
                    } else {
                        theme.ansi[7]
                    };

                    let cy = r.1 + (r.3 / 2);
                    let base_y = cy + 4;
                    let text_w = self.measure_ui_text(theme_name, 11.0, is_current);
                    let tx = r.0 + (r.2.saturating_sub(text_w) / 2);
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        theme_name,
                        tx,
                        base_y,
                        r.0 + r.2,
                        fg,
                        11.0,
                        is_current,
                    );
                }

                // Paleta de chips de color del tema activo
                let swatch_y = card_y + 116;
                let swatch_w = 26;
                let swatch_h = 18;
                let swatch_start_x = content_x + 20;
                for i in 0..16 {
                    let sx = swatch_start_x + i * (swatch_w + 4);
                    if sx + swatch_w < content_x + card_w {
                        crate::ui::draw_rounded_rect(
                            buffer,
                            width,
                            height,
                            sx,
                            swatch_y,
                            swatch_w,
                            swatch_h,
                            4,
                            theme.ansi[i],
                        );
                    }
                }

                // Tarjeta 2: Modo de Tema y Disposición de Pestañas
                let card2_y = card_y + 160;
                let card2_h = 114;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    card2_y,
                    card_w,
                    card2_h,
                    "MODO Y DISPOSICIÓN DE PESTAÑAS",
                    theme,
                );

                // Modo de tema (system, dark, light)
                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.theme_mode_system_btn.1,
                    content_x + 175,
                    "Modo de color:",
                    theme,
                );
                let is_sys = config.appearance.theme_mode.eq_ignore_ascii_case("system");
                let is_dark = config.appearance.theme_mode.eq_ignore_ascii_case("dark");
                let is_light = config.appearance.theme_mode.eq_ignore_ascii_case("light");

                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.theme_mode_system_btn,
                    "Sistema (Auto)",
                    state.hovered_theme_mode == Some(0),
                    is_sys,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.theme_mode_dark_btn,
                    "Oscuro",
                    state.hovered_theme_mode == Some(1),
                    is_dark,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.theme_mode_light_btn,
                    "Claro",
                    state.hovered_theme_mode == Some(2),
                    is_light,
                    theme,
                );

                // Disposición de pestañas (vertical, horizontal)
                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.tab_layout_vertical_btn.1,
                    content_x + 175,
                    "Pestañas:",
                    theme,
                );
                let is_vert = !config
                    .appearance
                    .tab_layout
                    .eq_ignore_ascii_case("horizontal");
                let is_horiz = config
                    .appearance
                    .tab_layout
                    .eq_ignore_ascii_case("horizontal");

                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.tab_layout_vertical_btn,
                    "Vertical (Sidebar)",
                    state.hovered_tab_layout == Some(0),
                    is_vert,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.tab_layout_horizontal_btn,
                    "Superior (Tabs)",
                    state.hovered_tab_layout == Some(1),
                    is_horiz,
                    theme,
                );

                // Tarjeta 3: Tipografía y Ventana
                let typo_y = card2_y + card2_h + 14;
                let typo_h = 154;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    typo_y,
                    card_w,
                    typo_h,
                    "TIPOGRAFÍA Y VENTANA",
                    theme,
                );

                // Tamaño de fuente
                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.font_minus_btn.1,
                    content_x + 230,
                    "Tamaño de fuente:",
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.font_minus_btn,
                    "-",
                    state.hovered_font_minus,
                    false,
                    theme,
                );
                let font_str = format!("{:.1} pt", config.appearance.font_size);
                self.draw_stepper_value(
                    buffer,
                    width,
                    height,
                    rects.font_minus_btn,
                    rects.font_plus_btn,
                    &font_str,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.font_plus_btn,
                    "+",
                    state.hovered_font_plus,
                    false,
                    theme,
                );

                // Interlineado
                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.spacing_minus_btn.1,
                    content_x + 230,
                    "Interlineado (Spacing):",
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.spacing_minus_btn,
                    "-",
                    state.hovered_spacing_minus,
                    false,
                    theme,
                );
                let sp_str = format!("{:.1}", config.appearance.line_spacing);
                self.draw_stepper_value(
                    buffer,
                    width,
                    height,
                    rects.spacing_minus_btn,
                    rects.spacing_plus_btn,
                    &sp_str,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.spacing_plus_btn,
                    "+",
                    state.hovered_spacing_plus,
                    false,
                    theme,
                );

                // Opacidad
                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.opacity_minus_btn.1,
                    content_x + 230,
                    "Opacidad de ventana:",
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.opacity_minus_btn,
                    "-",
                    state.hovered_opacity_minus,
                    false,
                    theme,
                );
                let op_str = format!("{:.0}%", (config.appearance.window_opacity * 100.0).round());
                self.draw_stepper_value(
                    buffer,
                    width,
                    height,
                    rects.opacity_minus_btn,
                    rects.opacity_plus_btn,
                    &op_str,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.opacity_plus_btn,
                    "+",
                    state.hovered_opacity_plus,
                    false,
                    theme,
                );
            }
            SettingsSection::Terminal => {
                // Tarjeta 1: Comportamiento
                let term_card_h = 278;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    card_y,
                    card_w,
                    term_card_h,
                    "COMPORTAMIENTO DEL TERMINAL",
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.cursor_blink_btn.1,
                    content_x + 250,
                    "Cursor parpadeante:",
                    theme,
                );
                let blink_str = if config.terminal.cursor_blink {
                    "Activado"
                } else {
                    "Desactivado"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.cursor_blink_btn,
                    blink_str,
                    state.hovered_cursor_blink,
                    config.terminal.cursor_blink,
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.ghostty_prompt_btn.1,
                    content_x + 250,
                    "Prompt Ghostty nativo (OSC 133):",
                    theme,
                );
                let prompt_str = if config.terminal.ghostty_prompt {
                    "Activado"
                } else {
                    "Desactivado"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.ghostty_prompt_btn,
                    prompt_str,
                    state.hovered_ghostty_prompt,
                    config.terminal.ghostty_prompt,
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.copy_on_select_btn.1,
                    content_x + 250,
                    "Copiar texto al seleccionar:",
                    theme,
                );
                let copy_str = if config.terminal.copy_on_select {
                    "Activado"
                } else {
                    "Desactivado"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.copy_on_select_btn,
                    copy_str,
                    state.hovered_copy_on_select,
                    config.terminal.copy_on_select,
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.status_bar_toggle_btn.1,
                    content_x + 250,
                    "Barra inferior (Status Bar):",
                    theme,
                );
                let status_str = if config.appearance.show_status_bar {
                    "Visible"
                } else {
                    "Oculta"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.status_bar_toggle_btn,
                    status_str,
                    state.hovered_status_bar,
                    config.appearance.show_status_bar,
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.file_explorer_toggle_btn.1,
                    content_x + 250,
                    "Explorador de archivos lateral:",
                    theme,
                );
                let explorer_str = if config.appearance.show_file_explorer {
                    "Visible"
                } else {
                    "Oculto"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.file_explorer_toggle_btn,
                    explorer_str,
                    state.hovered_file_explorer,
                    config.appearance.show_file_explorer,
                    theme,
                );

                self.draw_label(
                    buffer,
                    width,
                    height,
                    content_x + 20,
                    rects.restore_session_btn.1,
                    content_x + 250,
                    "Restaurar pestañas al iniciar:",
                    theme,
                );
                let restore_str = if config.terminal.restore_session {
                    "Activado"
                } else {
                    "Desactivado"
                };
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.restore_session_btn,
                    restore_str,
                    state.hovered_restore_session,
                    config.terminal.restore_session,
                    theme,
                );

                // Tarjeta 2: Configuración en disco
                let cfg_y = card_y + term_card_h + 14;
                let cfg_h = 110;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    cfg_y,
                    card_w,
                    cfg_h,
                    "CONFIGURACIÓN EN DISCO",
                    theme,
                );

                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.open_file_btn,
                    "Abrir axon.toml en editor",
                    state.hovered_open_file,
                    state.hovered_open_file,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.term_reset_btn,
                    "Restablecer por defecto",
                    state.hovered_reset_defaults,
                    false,
                    theme,
                );
                let hint_base = rects.open_file_btn.1 + rects.open_file_btn.3 + 18;
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    "Ruta: %LOCALAPPDATA%\\axon\\axon.toml (se recarga automáticamente)",
                    content_x + 20,
                    hint_base,
                    content_x + card_w,
                    theme.ansi[8],
                    11.0,
                    false,
                );
            }
            SettingsSection::Shortcuts => {
                let sh_h = 434;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    card_y,
                    card_w,
                    sh_h,
                    "ATAJOS DE TECLADO CANÓNICOS (Unificados)",
                    theme,
                );

                let shortcuts = [
                    ("Ctrl + T", "Nueva terminal / pestaña"),
                    ("Ctrl + W", "Cerrar pestaña o terminal activa"),
                    (
                        "Ctrl + Tab",
                        "Conmutar a la siguiente pestaña (Shift: anterior)",
                    ),
                    ("Ctrl + 1..9", "Cambiar directamente a pestaña 1-9"),
                    (
                        "Ctrl + Alt + D",
                        "Dividir terminal horizontalmente (debajo)",
                    ),
                    ("Ctrl + Alt + V", "Dividir terminal verticalmente (al lado)"),
                    ("Ctrl + Alt + W", "Cerrar panel dividido activo"),
                    (
                        "Ctrl + Alt + Flechas",
                        "Navegar foco entre paneles divididos",
                    ),
                    ("Ctrl + B", "Mostrar / Ocultar barra lateral"),
                    (
                        "Ctrl + Shift + E",
                        "Mostrar / Ocultar explorador de archivos lateral",
                    ),
                    ("Ctrl + ,", "Abrir o cerrar pestaña de Ajustes"),
                    (
                        "Ctrl + V",
                        "Pegar portapapeles (texto plano o imágenes IA / SSH)",
                    ),
                    (
                        "Ctrl + Shift + C",
                        "Copiar texto seleccionado al portapapeles",
                    ),
                    ("Ctrl + Shift + K", "Limpiar pantalla y scrollback buffer"),
                    (
                        "Ctrl + Shift + O",
                        "Copiar salida del último comando del agente",
                    ),
                    ("Ctrl + Backspace", "Borrar palabra atrás (0x17 / werase)"),
                ];

                let start_y = card_y + 40;
                for (idx, (chord, desc)) in shortcuts.iter().enumerate() {
                    let y = start_y + idx * 24;
                    let chord_w = 170;
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        content_x + 20,
                        y,
                        chord_w,
                        20,
                        4,
                        theme.sidebar_bg(),
                    );
                    crate::ui::stroke_rounded_rect(
                        buffer,
                        width,
                        height,
                        content_x + 20,
                        y,
                        chord_w,
                        20,
                        4,
                        theme.border_color(),
                    );
                    let baseline = y + 14;
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        chord,
                        content_x + 28,
                        baseline,
                        content_x + 20 + chord_w,
                        theme.accent_color(),
                        11.0,
                        true,
                    );
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        desc,
                        content_x + 204,
                        baseline,
                        content_x + card_w.saturating_sub(10),
                        theme.foreground,
                        11.5,
                        false,
                    );
                }
            }
            SettingsSection::About => {
                let ab_h = 280;
                self.draw_card(
                    buffer,
                    width,
                    height,
                    content_x,
                    card_y,
                    card_w,
                    ab_h,
                    "ACERCA DE AXON TERMINAL",
                    theme,
                );

                let start_y = card_y + 42;
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    "Axon Terminal",
                    content_x + 20,
                    start_y + 16,
                    content_x + card_w,
                    theme.foreground,
                    14.0,
                    true,
                );

                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    "Emulador de terminal nativo de alto rendimiento para Windows en Rust.",
                    content_x + 20,
                    start_y + 36,
                    content_x + card_w,
                    theme.ansi[8],
                    11.5,
                    false,
                );

                let features = [
                    "• Zero-runtime web: Renderizado softbuffer + rasterizador fontdue de alta fidelidad.",
                    "• Windows ConPTY asíncrono con multihilo y anclaje de prompt para agentes CLI.",
                    "• Protocolo iTerm2 / OSC 1337 para renderizado inline de imágenes PNG/JPG.",
                    "• Integración completa para sesiones remotas SSH (clonación de splits y SCP).",
                    "• Portapapeles remoto bidireccional mediante OSC 52 en base64.",
                    "• Temas oficiales Catppuccin Frappé y Latte, con sincronización de tema del sistema.",
                    "• Barra de estado inferior informativa con rama Git y detección de agentes de IA.",
                ];

                for (idx, feat) in features.iter().enumerate() {
                    let fy = start_y + 54 + idx * 22;
                    let f_base = fy + 14;
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        feat,
                        content_x + 20,
                        f_base,
                        content_x + card_w,
                        theme.ansi[7],
                        11.0,
                        false,
                    );
                }

                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.open_file_btn,
                    "Abrir axon.toml en editor",
                    state.hovered_open_file,
                    state.hovered_open_file,
                    theme,
                );
                self.draw_button(
                    buffer,
                    width,
                    height,
                    rects.term_reset_btn,
                    "Restablecer por defecto",
                    state.hovered_reset_defaults,
                    false,
                    theme,
                );
            }
        }
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
    #[allow(dead_code)]
    pub fn compute_grid_size(
        &self,
        window_width: usize,
        window_height: usize,
        sidebar_offset: usize,
    ) -> (usize, usize) {
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
        let thumb_h = ((terminal.rows as f32 / total_history as f32) * track_h as f32)
            .max(16.0)
            .min(track_h as f32) as usize;
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
            if let Some(block) = terminal
                .command_blocks
                .iter()
                .find(|b| b.prompt_line == global_row)
            {
                let dot_color = match block.exit_code {
                    Some(0) => Color::from_rgb(81, 226, 122).to_u32_xrgb(), // Verde Ghostty #51e27a
                    Some(_) => Color::from_rgb(244, 56, 65).to_u32_xrgb(),  // Rojo Ghostty #f43841
                    None => Color::from_rgb(78, 146, 249).to_u32_xrgb(),    // Azul Ghostty #4e92f9
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
                } else if cell.inverse() {
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

                // Calcular color final de texto
                let fg_color = compute_cell_fg(cell, theme);
                let fg_u32 = fg_color.to_u32_xrgb();
                let fg_r = fg_color.r as u32;
                let fg_g = fg_color.g as u32;
                let fg_b = fg_color.b as u32;

                // Subrayado (SGR 4)
                if cell.underline() {
                    let line_y = cell_y + self.row_height - 2;
                    for x in cell_x..(cell_x + self.char_width) {
                        if x < max_x && line_y < max_y {
                            buffer[line_y * width + x] = fg_u32;
                        }
                    }
                }

                // Tachado (SGR 9)
                if cell.strikethrough() {
                    let line_y = cell_y + (self.row_height / 2);
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

                // Obtener y mezclar glifo (con soporte completo de Nerd Fonts)
                let glyph = self.get_glyph(cell.c, cell.bold());
                let glyph_w = glyph.width;
                let glyph_h = glyph.height;
                let glyph_x = cell_x as i32 + glyph.x_offset;
                let glyph_y = (cell_y + baseline_y) as i32 - glyph.y_offset - glyph_h as i32;

                for gy in 0..glyph_h {
                    let py = glyph_y + gy as i32;
                    if py < rect_y as i32 || py >= max_y as i32 {
                        continue;
                    }
                    let row_buf_idx = (py as usize) * width;
                    for gx in 0..glyph_w {
                        let px = glyph_x + gx as i32;
                        if px < rect_x as i32 || px >= max_x as i32 {
                            continue;
                        }

                        let alpha = glyph.bitmap[gy * glyph_w + gx];
                        if alpha == 255 {
                            buffer[row_buf_idx + (px as usize)] = fg_u32;
                        } else if alpha > 0 {
                            let buf_idx = row_buf_idx + (px as usize);
                            let bg_val = buffer[buf_idx];
                            let bg_r = (bg_val >> 16) & 0xFF;
                            let bg_g = (bg_val >> 8) & 0xFF;
                            let bg_b = bg_val & 0xFF;
                            let a = alpha as u32;
                            let inv_a = 255 - a;
                            let r = (fg_r * a + bg_r * inv_a) / 255;
                            let g = (fg_g * a + bg_g * inv_a) / 255;
                            let b = (fg_b * a + bg_b * inv_a) / 255;
                            buffer[buf_idx] = (r << 16) | (g << 8) | b;
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
                if cell.width == 2 {
                    self.char_width * 2
                } else {
                    self.char_width
                }
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
                        let inv_fg_u32 = inv_fg.to_u32_xrgb();
                        let inv_fg_r = inv_fg.r as u32;
                        let inv_fg_g = inv_fg.g as u32;
                        let inv_fg_b = inv_fg.b as u32;
                        let bg = theme.cursor;
                        let bg_r = bg.r as u32;
                        let bg_g = bg.g as u32;
                        let bg_b = bg.b as u32;
                        let glyph = self.get_glyph(cell.c, cell.bold());
                        let glyph_w = glyph.width;
                        let glyph_h = glyph.height;
                        let glyph_x = cx as i32 + glyph.x_offset;
                        let glyph_y = (cy + baseline_y) as i32 - glyph.y_offset - glyph_h as i32;

                        for gy in 0..glyph_h {
                            let py = glyph_y + gy as i32;
                            if py < rect_y as i32 || py >= max_y as i32 {
                                continue;
                            }
                            let row_buf_idx = (py as usize) * width;
                            for gx in 0..glyph_w {
                                let px = glyph_x + gx as i32;
                                if px < rect_x as i32 || px >= max_x as i32 {
                                    continue;
                                }

                                let alpha = glyph.bitmap[gy * glyph_w + gx];
                                if alpha == 255 {
                                    buffer[row_buf_idx + (px as usize)] = inv_fg_u32;
                                } else if alpha > 0 {
                                    let buf_idx = row_buf_idx + (px as usize);
                                    let a = alpha as u32;
                                    let inv_a = 255 - a;
                                    let r = (inv_fg_r * a + bg_r * inv_a) / 255;
                                    let g = (inv_fg_g * a + bg_g * inv_a) / 255;
                                    let b = (inv_fg_b * a + bg_b * inv_a) / 255;
                                    buffer[buf_idx] = (r << 16) | (g << 8) | b;
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
                        if (y == cy
                            || y == cy + self.row_height - 1
                            || x == cx
                            || x == cx + c_width - 1)
                            && x >= rect_x
                            && x < max_x
                            && y >= rect_y
                            && y < max_y
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
                                buffer[buf_idx] =
                                    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
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
            if let Some((thumb_x, thumb_y, thumb_w, thumb_h)) =
                self.pane_scrollbar_geometry(rect_x, rect_y, rect_w, rect_h, terminal)
            {
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
            Self::draw_rect(
                buffer,
                width,
                height,
                rect_x,
                rect_y,
                rect_w,
                2,
                active_border,
            );
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
    let base = if cell.inverse() {
        if cell.bg != Color::TRANSPARENT {
            cell.bg
        } else {
            theme.background
        }
    } else if cell.fg != Color::TRANSPARENT {
        // Garantizar contraste mínimo contra el fondo de la terminal
        let dr = (cell.fg.r as i32 - theme.background.r as i32).abs();
        let dg = (cell.fg.g as i32 - theme.background.g as i32).abs();
        let db = (cell.fg.b as i32 - theme.background.b as i32).abs();
        if dr + dg + db < 18 {
            theme.foreground
        } else {
            cell.fg
        }
    } else {
        theme.foreground
    };

    if cell.dim() {
        Color::from_rgb(
            (base.r as f32 * 0.65) as u8,
            (base.g as f32 * 0.65) as u8,
            (base.b as f32 * 0.65) as u8,
        )
    } else {
        base
    }
}
