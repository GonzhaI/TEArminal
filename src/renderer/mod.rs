//! Renderizado por software (CPU) de la terminal y de toda la interfaz sobre un framebuffer `u32` (XRGB).
//!
//! Un fotograma completo cuesta ~1 ms a 1080p (ver `AGENTS.md`, sección de rendimiento), por eso no hay GPU.
//! `TerminalRenderer` es el único dueño de las fuentes y cachés; cada pantalla vive en su archivo.

// Las funciones de dibujo reciben el framebuffer y la geometría explícitos (coordenadas, tamaños, estado de
// hover): son primitivas planas, y agruparlas en structs solo trasladaría el ruido a cada llamada.
#![allow(clippy::too_many_arguments)]

mod blocks;
mod canvas;
mod context_menu;
pub mod d3d11;
mod draw;
mod explorer_panel;
mod file_viewer;
mod files_panel;
mod geometry;
mod glyphs;
mod md_preview;
mod pane;
mod settings_page;
mod sidebar;
mod state;
mod status_bar;
#[cfg(test)]
mod test_support;
mod tokens;
mod top_bar;

#[cfg(test)]
use draw::fill_rect;
use draw::{blit_glyph, Clip};
use glyphs::{
    font_settings, FastMap, EMBEDDED_INTER_MEDIUM, EMBEDDED_INTER_SEMIBOLD,
    EMBEDDED_JETBRAINS_REGULAR, EMBEDDED_LUCIDE,
};

pub use blocks::BlockCardButton;
pub use canvas::{Canvas, ImageRef};
pub use d3d11::D3D11Presenter;
pub use geometry::*;
pub use glyphs::{is_ligature_candidate, CachedGlyph};
pub use state::*;

use crate::config::AxonConfig;
use crate::terminal::{Cell, TerminalState};
use crate::theme::{Color, Theme};
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub struct TerminalRenderer {
    regular_font: fontdue::Font,
    /// Negrita embebida, cargada al primer uso. Con fuente personalizada no existe (se usa `regular_font`).
    bold_font: OnceCell<fontdue::Font>,
    custom_font: bool,
    /// ~10 000 iconos Nerd (PUA): ~45 MB de contornos en RAM, cada bloque se parsea al primer icono
    nerd_fonts: [OnceCell<fontdue::Font>; 2],
    /// Segoe UI Symbol del sistema: solo se parsea si aparece un glifo que ninguna otra fuente tiene
    fallback_font: OnceCell<Option<fontdue::Font>>,
    pub inter_font: fontdue::Font,
    pub inter_semibold_font: fontdue::Font,
    pub lucide_font: fontdue::Font,
    pub font_size: f32,
    pub ui_font_size: f32,
    pub scale_factor: f64,
    pub char_width: usize,
    pub row_height: usize,
    pub baseline_y: usize,
    glyph_cache: FastMap<(char, bool), CachedGlyph>,
    ascii_cache: Box<[Option<CachedGlyph>; 512]>,
    ui_glyph_cache: FastMap<(char, u32, bool), (CachedGlyph, f32)>, // (c, size_bits, semibold) -> (glyph, advance)
    ui_advance_cache: RefCell<FastMap<(char, u32, bool), f32>>, // medición de texto sin rasterizar
    /// Valores de máquina (rutas, ramas, atajos): JetBrains Mono con su propia caché.
    mono_glyph_cache: FastMap<(char, u32, bool), (CachedGlyph, f32)>,
    mono_advance_cache: RefCell<FastMap<(char, u32, bool), f32>>,
    icon_glyph_cache: FastMap<(char, u32), CachedGlyph>, // (c, size_bits) -> glyph
    pub margin_x: usize,
    pub margin_y: usize,
    /// Las barras de scroll solo se ven mientras se hace scroll (el hover las muestra siempre)
    pub scrollbars_awake: bool,
    pub ligatures: bool,
    regular_face: Option<rustybuzz::Face<'static>>,
    bold_face_rb: OnceCell<Option<rustybuzz::Face<'static>>>,
    ligature_cache: FastMap<(u16, bool), CachedGlyph>,
    shaped_ligatures: FastMap<glyphs::PunctKey, glyphs::ShapedGids>,
    md_blocks_cache: Option<(
        std::path::PathBuf,
        u64,
        std::rc::Rc<Vec<md_preview::MdBlock>>,
    )>,
}

impl TerminalRenderer {
    pub fn new(config: &AxonConfig, scale_factor: f64) -> Self {
        // JetBrains Mono Nerd Font Regular (o la fuente del usuario); negrita y fallback son perezosas
        let (regular_font, custom_font) = Self::load_regular_font(config);
        let regular_face = if custom_font {
            None
        } else {
            rustybuzz::Face::from_slice(EMBEDDED_JETBRAINS_REGULAR, 0)
        };

        let inter_font = fontdue::Font::from_bytes(EMBEDDED_INTER_MEDIUM, font_settings())
            .expect("Error al inicializar fuente Inter Medium");
        let inter_semibold_font =
            fontdue::Font::from_bytes(EMBEDDED_INTER_SEMIBOLD, font_settings())
                .expect("Error al inicializar fuente Inter SemiBold");
        let lucide_font = fontdue::Font::from_bytes(EMBEDDED_LUCIDE, font_settings())
            .expect("Error al inicializar fuente Lucide");

        let mut renderer = Self {
            regular_font,
            bold_font: OnceCell::new(),
            custom_font,
            nerd_fonts: [OnceCell::new(), OnceCell::new()],
            fallback_font: OnceCell::new(),
            inter_font,
            inter_semibold_font,
            lucide_font,
            font_size: 0.0,
            ui_font_size: 0.0,
            scale_factor,
            char_width: 0,
            row_height: 0,
            baseline_y: 0,
            glyph_cache: FastMap::default(),
            ascii_cache: Box::new(std::array::from_fn(|_| None)),
            ui_glyph_cache: FastMap::default(),
            ui_advance_cache: RefCell::default(),
            mono_glyph_cache: FastMap::default(),
            mono_advance_cache: RefCell::default(),
            icon_glyph_cache: FastMap::default(),
            margin_x: 0,
            margin_y: 0,
            scrollbars_awake: false,
            ligatures: config.terminal.ligatures,
            regular_face,
            bold_face_rb: OnceCell::new(),
            ligature_cache: FastMap::default(),
            shaped_ligatures: FastMap::default(),
            md_blocks_cache: None,
        };
        renderer.update_scale(config, scale_factor);
        renderer
    }

    pub fn update_scale(&mut self, config: &AxonConfig, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.font_size = (config.appearance.font_size * scale_factor as f32)
            .round()
            .max(10.0);
        self.ui_font_size = (config.appearance.ui_font_size * scale_factor as f32)
            .round()
            .max(9.0);
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
        self.margin_x = (14.0 * scale_factor as f32).round() as usize;
        self.margin_y = (10.0 * scale_factor as f32).round() as usize;
        self.glyph_cache.clear();
        for slot in self.ascii_cache.iter_mut() {
            *slot = None;
        }
        self.ui_glyph_cache.clear();
        self.ui_advance_cache.borrow_mut().clear();
        self.mono_glyph_cache.clear();
        self.mono_advance_cache.borrow_mut().clear();
        self.icon_glyph_cache.clear();
        self.ligature_cache.clear();
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
    #[cfg(test)]
    pub fn compute_grid_size(
        &self,
        window_width: usize,
        window_height: usize,
        sidebar_offset: usize,
    ) -> (usize, usize) {
        let avail_w = window_width.saturating_sub(sidebar_offset);
        self.compute_pane_grid_size(avail_w, window_height)
    }
}

#[cfg(test)]
mod tests;
