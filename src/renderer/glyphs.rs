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

/// Ajustes de fuente comunes. Carga sustituciones GSUB para soportar ligaduras de programación.
pub(super) fn font_settings() -> fontdue::FontSettings {
    fontdue::FontSettings {
        load_substitutions: true,
        ..Default::default()
    }
}

/// Clave compacta (10 bytes, Copy) para la caché de secuencias de ligadura puntuadas
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PunctKey {
    pub bytes: [u8; 8],
    pub len: u8,
    pub bold: bool,
}

impl PunctKey {
    #[inline]
    pub fn new(slice: &[u8], bold: bool) -> Self {
        let mut bytes = [0u8; 8];
        let len = slice.len().min(8);
        bytes[..len].copy_from_slice(&slice[..len]);
        Self {
            bytes,
            len: len as u8,
            bold,
        }
    }
}

/// GIDs de una secuencia con ligadura dada forma (hasta 8 glifos, 16 bytes, Copy)
#[derive(Clone, Copy)]
pub struct ShapedGids {
    pub gids: [u16; 8],
}

/// Comprueba si un carácter puede formar parte de una ligadura de programación
#[inline]
pub fn is_ligature_candidate(c: char) -> bool {
    matches!(
        c,
        '!' | '#'
            | '$'
            | '%'
            | '&'
            | '*'
            | '+'
            | '-'
            | '.'
            | '/'
            | ':'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '\\'
            | '^'
            | '|'
            | '~'
    )
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

    /// Obtiene o rasteriza un glifo; tabla plana O(1) para ASCII + Latin-1 y caché rápida para el resto
    #[inline]
    pub fn get_glyph(&mut self, c: char, bold: bool) -> &CachedGlyph {
        let code = c as u32;
        if code < 256 {
            let idx = code as usize + if bold { 256 } else { 0 };
            if self.ascii_cache[idx].is_none() {
                self.ascii_cache[idx] = Some(self.rasterize_glyph(c, bold));
            }
            return self.ascii_cache[idx].as_ref().unwrap();
        }

        if !self.glyph_cache.contains_key(&(c, bold)) {
            // Límite de glifos dinámicos (CJK, símbolos) para evitar crecimiento ilimitado de memoria
            if self.glyph_cache.len() >= 4096 {
                self.glyph_cache.clear();
            }
            let glyph = self.rasterize_glyph(c, bold);
            self.glyph_cache.insert((c, bold), glyph);
        }
        &self.glyph_cache[&(c, bold)]
    }

    pub(super) fn get_regular_face(&self) -> Option<&rustybuzz::Face<'static>> {
        self.regular_face.as_ref()
    }

    pub(super) fn get_bold_face_rb(&self) -> Option<&rustybuzz::Face<'static>> {
        if self.custom_font {
            return None;
        }
        self.bold_face_rb
            .get_or_init(|| rustybuzz::Face::from_slice(EMBEDDED_JETBRAINS_BOLD, 0))
            .as_ref()
    }

    /// Obtiene o rasteriza un glifo de ligadura por su GID de OpenType
    pub fn get_ligature_glyph(&mut self, gid: u16, bold: bool) -> &CachedGlyph {
        if !self.ligature_cache.contains_key(&(gid, bold)) {
            let font = if bold {
                self.bold_font()
            } else {
                &self.regular_font
            };
            let (metrics, bitmap) = font.rasterize_indexed(gid, self.font_size);
            let glyph = CachedGlyph {
                width: metrics.width,
                height: metrics.height,
                x_offset: metrics.xmin,
                y_offset: metrics.ymin,
                bitmap,
            };
            self.ligature_cache.insert((gid, bold), glyph);
        }
        &self.ligature_cache[&(gid, bold)]
    }

    /// Da forma a una secuencia de puntuación ASCII (hasta 8 bytes).
    /// Devuelve un `ShapedGids` que contiene los GIDs de cada carácter.
    pub fn shape_punctuation_run(&mut self, slice: &[u8], bold: bool) -> ShapedGids {
        let key = PunctKey::new(slice, bold);
        if let Some(&cached) = self.shaped_ligatures.get(&key) {
            return cached;
        }

        let face_opt = if bold {
            self.get_bold_face_rb()
        } else {
            self.get_regular_face()
        };

        let mut gids = [0u16; 8];
        let len = slice.len().min(8);

        if let Some(face) = face_opt {
            if let Ok(text) = std::str::from_utf8(&slice[..len]) {
                let mut buf = rustybuzz::UnicodeBuffer::new();
                buf.push_str(text);
                let glyphs = rustybuzz::shape(face, &[], buf);
                let infos = glyphs.glyph_infos();
                if infos.len() == len {
                    for (i, info) in infos.iter().take(8).enumerate() {
                        gids[i] = info.glyph_id as u16;
                    }
                } else {
                    // Si el moldeado devuelve menos glifos que caracteres (p. ej. compresión no monospaciada),
                    // recurrir al glifo estándar de cada carácter para no dejar GID 0 (.notdef) en celdas intermedias
                    let font = if bold {
                        self.bold_font()
                    } else {
                        &self.regular_font
                    };
                    for (i, &b) in slice[..len].iter().enumerate() {
                        gids[i] = font.lookup_glyph_index(b as char);
                    }
                }
            }
        } else {
            let font = if bold {
                self.bold_font()
            } else {
                &self.regular_font
            };
            for (i, &b) in slice[..len].iter().enumerate() {
                gids[i] = font.lookup_glyph_index(b as char);
            }
        }

        let shaped = ShapedGids { gids };
        if self.shaped_ligatures.len() >= 2048 {
            self.shaped_ligatures.clear();
        }
        self.shaped_ligatures.insert(key, shaped);
        shaped
    }
}
