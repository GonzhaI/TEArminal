//! Renderizado de la terminal y de toda la interfaz: la CPU graba cada fotograma en una `Scene` (`Canvas`) y
//! `gpu/` la dibuja con Direct3D 11 (GPU o el rasterizador por software WARP), ver `AGENTS.md`.
//! `TerminalRenderer` es el único dueño de las fuentes y cachés; cada pantalla vive en su archivo.

// Las funciones de dibujo reciben el `Canvas` y la geometría explícitos (coordenadas, tamaños, estado de
// hover): son primitivas planas, y agruparlas en structs solo trasladaría el ruido a cada llamada.
#![allow(clippy::too_many_arguments)]

mod blocks;
mod canvas;
mod context_menu;
mod draw;
mod explorer_panel;
mod file_viewer;
mod files_panel;
mod geometry;
mod glyphs;
mod gpu;
mod md_preview;
mod palette;
mod pane;
mod scene;
mod settings_page;
mod sidebar;
mod state;
mod status_bar;
#[cfg(test)]
mod test_support;
pub(crate) mod tokens;
mod top_bar;

use draw::Clip;
use glyphs::{
    font_settings, FastMap, EMBEDDED_INTER_MEDIUM, EMBEDDED_INTER_SEMIBOLD,
    EMBEDDED_JETBRAINS_REGULAR, EMBEDDED_LUCIDE,
};

pub(crate) use blocks::BlockCardButton;
pub(crate) use canvas::Canvas;
pub(crate) use geometry::*;
pub(crate) use glyphs::{is_ligature_candidate, CachedGlyph};
pub(crate) use gpu::{GpuSurface, Presented};
pub(crate) use scene::ImageRef;
pub(crate) use state::*;

use crate::config::TearminalConfig;
use crate::terminal::{Cell, TerminalState};
use crate::theme::{Color, Theme};
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

pub(crate) struct TerminalRenderer {
    regular_font: fontdue::Font,
    /// Negrita embebida, cargada al primer uso. Con fuente personalizada no existe (se usa `regular_font`).
    bold_font: OnceCell<fontdue::Font>,
    custom_font: bool,
    /// ~10 000 iconos Nerd (PUA): ~45 MB de contornos en RAM, cada bloque se parsea al primer icono
    nerd_fonts: [OnceCell<fontdue::Font>; 2],
    /// Fuentes de fallback del sistema Windows: solo se parsean perezosamente bajo demanda
    fallback_symbols: OnceCell<Option<fontdue::Font>>,
    fallback_emoji: OnceCell<Option<fontdue::Font>>,
    fallback_cjk: OnceCell<Option<fontdue::Font>>,
    pub inter_font: fontdue::Font,
    pub inter_semibold_font: fontdue::Font,
    pub lucide_font: fontdue::Font,
    pub font_size: f32,
    pub ui_font_size: f32,
    pub scale_factor: f64,
    pub char_width: usize,
    pub row_height: usize,
    pub baseline_y: usize,
    glyph_cache: FastMap<(char, bool), Rc<CachedGlyph>>,
    ascii_cache: Box<[Option<Rc<CachedGlyph>>; 512]>,
    ui_glyph_cache: FastMap<(char, u32, bool), (Rc<CachedGlyph>, f32)>, // (c, size_bits, semibold) -> (glyph, advance)
    ui_advance_cache: RefCell<FastMap<(char, u32, bool), f32>>, // medición de texto sin rasterizar
    icon_glyph_cache: FastMap<(char, u32), Rc<CachedGlyph>>,    // (c, size_bits) -> glyph
    pub margin_x: usize,
    pub margin_y: usize,
    /// Las barras de scroll solo se ven mientras se hace scroll (el hover las muestra siempre)
    pub scrollbars_awake: bool,
    pub ligatures: bool,
    #[allow(dead_code)]
    pub show_dimensions: bool,
    regular_face: Option<rustybuzz::Face<'static>>,
    bold_face_rb: OnceCell<Option<rustybuzz::Face<'static>>>,
    ligature_cache: FastMap<(u16, bool), Rc<CachedGlyph>>,
    shaped_ligatures: FastMap<glyphs::PunctKey, glyphs::ShapedGids>,
    md_blocks_cache: Option<(std::path::PathBuf, u64, Rc<[md_preview::MdBlock]>)>,
}

impl TerminalRenderer {
    pub(crate) fn new(config: &TearminalConfig, scale_factor: f64) -> Self {
        // JetBrains Mono Nerd Font Regular (o la fuente del usuario); negrita y fallback son perezosas
        let (regular_font, custom_font) = Self::load_regular_font(config);
        let regular_face = if custom_font {
            None
        } else {
            rustybuzz::Face::from_slice(EMBEDDED_JETBRAINS_REGULAR, 0)
        };

        let inter_font = fontdue::Font::from_bytes(EMBEDDED_INTER_MEDIUM, font_settings())
            .expect("invariante: fuente embebida Inter Medium válida en tiempo de compilación");
        let inter_semibold_font =
            fontdue::Font::from_bytes(EMBEDDED_INTER_SEMIBOLD, font_settings()).expect(
                "invariante: fuente embebida Inter SemiBold válida en tiempo de compilación",
            );
        let lucide_font = fontdue::Font::from_bytes(EMBEDDED_LUCIDE, font_settings())
            .expect("invariante: fuente embebida Lucide válida en tiempo de compilación");

        let mut renderer = Self {
            regular_font,
            bold_font: OnceCell::new(),
            custom_font,
            nerd_fonts: [OnceCell::new(), OnceCell::new()],
            fallback_symbols: OnceCell::new(),
            fallback_emoji: OnceCell::new(),
            fallback_cjk: OnceCell::new(),
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
            icon_glyph_cache: FastMap::default(),
            margin_x: 0,
            margin_y: 0,
            scrollbars_awake: false,
            ligatures: config.terminal.ligatures,
            show_dimensions: config.appearance.show_dimensions,
            regular_face,
            bold_face_rb: OnceCell::new(),
            ligature_cache: FastMap::default(),
            shaped_ligatures: FastMap::default(),
            md_blocks_cache: None,
        };
        renderer.update_scale(config, scale_factor);
        renderer
    }

    pub(crate) fn update_scale(&mut self, config: &TearminalConfig, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.show_dimensions = config.appearance.show_dimensions;
        self.font_size = (config.appearance.font_size * scale_factor as f32)
            .round()
            .max(10.0);
        self.ui_font_size = (config.appearance.ui_font_size * scale_factor as f32)
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
        // La tarjeta de contenido recorta `CARD_INSET` px del borde del panel: los márgenes cuentan ese recorte
        // y dejan 14 px laterales y 8 verticales de aire visible hasta su borde interior.
        self.margin_x = (20.0 * scale_factor as f32).round() as usize;
        self.margin_y = (14.0 * scale_factor as f32).round() as usize;
        self.glyph_cache.clear();
        for slot in self.ascii_cache.iter_mut() {
            *slot = None;
        }
        self.ui_glyph_cache.clear();
        self.ui_advance_cache.borrow_mut().clear();
        self.icon_glyph_cache.clear();
        self.ligature_cache.clear();
    }

    /// Calcula las columnas y filas que caben en un panel según sus dimensiones en píxeles.
    /// Reserva el margen izquierdo (`margin_x`), el margen simétrico a la derecha del texto,
    /// y el espacio de la barra de scroll vertical y marco de tarjeta (`ScrollbarRects::RESERVED_RIGHT`),
    /// evitando que el texto o tablas formateadas por CLIs/agentes se solapen con la barra de scroll
    /// o queden cortados por el borde derecho.
    pub(crate) fn compute_pane_grid_size(
        &self,
        pane_width: usize,
        pane_height: usize,
    ) -> (usize, usize) {
        let h_margins = self.margin_x * 2 + ScrollbarRects::RESERVED_RIGHT;
        let avail_w = pane_width.saturating_sub(h_margins);
        let avail_h = pane_height.saturating_sub(self.margin_y * 2);
        let cols = (avail_w / self.char_width).max(10);
        let rows = (avail_h / self.row_height).max(3);
        (cols, rows)
    }

    /// Calcula las columnas y filas que caben en el tamaño actual de la ventana
    #[cfg(test)]
    pub(crate) fn compute_grid_size(
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
