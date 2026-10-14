//! Fuentes embebidas, cachés de glifos y rasterización (fontdue guarda todos los contornos en RAM: ver `tools/subset-fonts`).

use super::*;

// Fuentes reducidas con `tools/subset-fonts`: core (sin PUA) siempre; iconos Nerd bajo demanda
pub(super) const EMBEDDED_JETBRAINS_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular-Core.ttf");

pub(super) const EMBEDDED_JETBRAINS_BOLD: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMono-Bold-Core.ttf");

pub(super) const EMBEDDED_NERD_ICONS: [&[u8]; 2] = [
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular-Nerd.ttf"), // U+E000..=F8FF
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular-NerdMd.ttf"), // U+F0000.. (Material Design)
];

pub(super) const EMBEDDED_INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-500.ttf");

pub(super) const EMBEDDED_INTER_SEMIBOLD: &[u8] =
    include_bytes!("../../assets/fonts/Inter-600.ttf");

pub(super) const EMBEDDED_LUCIDE: &[u8] = include_bytes!("../../assets/fonts/lucide.ttf");

/// Hasher multiplicativo (FxHash) para las cachés de glifos: claves minúsculas (char, tamaño, bool)
/// consultadas decenas de miles de veces por fotograma; SipHash es innecesariamente caro aquí.
#[derive(Default, Clone, Copy)]
pub(super) struct FastHasher(u64);

impl FastHasher {
    #[inline(always)]
    fn add(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        bytes.iter().for_each(|&b| self.add(b as u64));
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

pub(super) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// Ajustes de fuente comunes. Sin shaping (rejilla monoespaciada, un glifo por carácter),
/// así que no se cargan las sustituciones GSUB.
pub(super) fn font_settings() -> fontdue::FontSettings {
    fontdue::FontSettings {
        load_substitutions: false,
        ..Default::default()
    }
}

pub struct CachedGlyph {
    pub width: usize,
    pub height: usize,
    pub x_offset: i32,
    pub y_offset: i32,
    pub bitmap: Vec<u8>, // Grayscale 8-bit alpha (0-255)
}

impl TerminalRenderer {
    /// Devuelve la fuente regular y si es una fuente personalizada del usuario
    pub(super) fn load_regular_font(config: &AxonConfig) -> (fontdue::Font, bool) {
        if let Some(custom) = &config.appearance.font_path {
            if let Some(font) = std::fs::read(custom)
                .ok()
                .and_then(|bytes| fontdue::Font::from_bytes(bytes, font_settings()).ok())
            {
                return (font, true);
            }
        }
        let regular = fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_REGULAR, font_settings())
            .expect("Error al inicializar fuente JetBrains Mono Nerd Font Regular");
        (regular, false)
    }

    pub(super) fn bold_font(&self) -> &fontdue::Font {
        if self.custom_font {
            return &self.regular_font;
        }
        self.bold_font.get_or_init(|| {
            fontdue::Font::from_bytes(EMBEDDED_JETBRAINS_BOLD, font_settings())
                .expect("Error al inicializar fuente JetBrains Mono Nerd Font Bold")
        })
    }

    /// Fuente de iconos Nerd que contiene `c` (solo para PUA), cargándola si hace falta
    pub(super) fn nerd_font(&self, c: char) -> &fontdue::Font {
        let block = (c as u32 >= 0xF0000) as usize;
        self.nerd_fonts[block].get_or_init(|| {
            fontdue::Font::from_bytes(EMBEDDED_NERD_ICONS[block], font_settings())
                .expect("Error al inicializar iconos Nerd Font")
        })
    }

    pub(super) fn fallback_font(&self) -> Option<&fontdue::Font> {
        self.fallback_font
            .get_or_init(|| {
                let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
                let path = std::path::Path::new(&windir)
                    .join("Fonts")
                    .join("seguisym.ttf");
                std::fs::read(path)
                    .ok()
                    .and_then(|bytes| fontdue::Font::from_bytes(bytes, font_settings()).ok())
            })
            .as_ref()
    }

    pub(super) fn rasterize_glyph(&self, c: char, bold: bool) -> CachedGlyph {
        // Orden: negrita (si aplica) -> regular -> iconos Nerd (PUA) -> fallback del sistema -> lo que haya
        let is_pua = matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..);
        let font = if bold && self.bold_font().has_glyph(c) {
            self.bold_font()
        } else if self.regular_font.has_glyph(c) {
            &self.regular_font
        } else if is_pua && self.nerd_font(c).has_glyph(c) {
            self.nerd_font(c)
        } else if let Some(fb) = self.fallback_font().filter(|f| f.has_glyph(c)) {
            fb
        } else if bold {
            self.bold_font()
        } else {
            &self.regular_font
        };

        let (metrics, bitmap) = font.rasterize(c, self.font_size);
        CachedGlyph {
            width: metrics.width,
            height: metrics.height,
            x_offset: metrics.xmin,
            y_offset: metrics.ymin,
            bitmap,
        }
    }

    /// Obtiene o rasteriza un glifo; tabla plana O(1) para ASCII y caché rápida para el resto
    pub fn get_glyph(&mut self, c: char, bold: bool) -> &CachedGlyph {
        let code = c as u32;
        if code < 128 {
            let idx = code as usize + if bold { 128 } else { 0 };
            if self.ascii_cache[idx].is_none() {
                self.ascii_cache[idx] = Some(self.rasterize_glyph(c, bold));
            }
            return self.ascii_cache[idx].as_ref().unwrap();
        }

        if !self.glyph_cache.contains_key(&(c, bold)) {
            let glyph = self.rasterize_glyph(c, bold);
            self.glyph_cache.insert((c, bold), glyph);
        }
        &self.glyph_cache[&(c, bold)]
    }
}
