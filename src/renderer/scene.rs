//! `Scene`: lista ordenada de operaciones de dibujo ya resueltas (coordenadas absolutas y recorte
//! efectivo finales) más las tablas de glifos e imágenes que usan. `Canvas` la graba; el rasterizador
//! CPU (`raster_cpu`) la reproduce y, más adelante, el backend GPU la subirá como quads.

use super::glyphs::FastMap;
use super::CachedGlyph;
use crate::theme::Color;
use std::rc::Rc;

/// Recorte absoluto `(x0, y0, x1, y1)` en el búfer real (límite superior exclusivo).
pub type RealClip = (i32, i32, i32, i32);

/// Imagen RGBA de 8 bits por canal (`width * height * 4` bytes).
pub struct ImageRef<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// Índice de un glifo en `Scene::glyphs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphId(u32);

/// Índice de una imagen en `Scene::images`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageId(u32);

/// Una operación de dibujo. Todas llevan coordenadas absolutas y el recorte ya intersecado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Rellena todo el búfer.
    Clear(Color),
    Fill {
        clip: RealClip,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: Color,
    },
    Glyph {
        clip: RealClip,
        glyph: GlyphId,
        x: i32,
        y: i32,
        color: Color,
    },
    Rounded {
        clip: RealClip,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        radius: u32,
        color: Color,
        /// `true` = solo el borde de 1 px; `false` = relleno.
        stroke: bool,
    },
    Image {
        clip: RealClip,
        image: ImageId,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    },
    /// Recorta el fotograma a una tarjeta redondeada y pinta el resto con `outer`.
    Card {
        region: (u32, u32, u32, u32),
        inset: u32,
        radius: u32,
        outer: Color,
    },
}

struct ImageEntry {
    offset: usize,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub struct Scene {
    pub w: u32,
    pub h: u32,
    pub ops: Vec<Op>,
    glyphs: Vec<Rc<CachedGlyph>>,
    /// Identidad (dirección del `Rc`) -> índice en `glyphs`. Las `Rc` de la tabla viven lo que la
    /// escena, así que la dirección no se reutiliza mientras el mapa la recuerda.
    glyph_ids: FastMap<usize, u32>,
    /// Último glifo registrado: el texto repite el mismo glifo muy a menudo.
    last_glyph: Option<(usize, GlyphId)>,
    images: Vec<ImageEntry>,
    image_bytes: Vec<u8>,
}

impl Scene {
    /// Vacía la escena conservando la memoria de las tablas y de `ops`.
    pub fn reset(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.ops.clear();
        self.glyphs.clear();
        self.glyph_ids.clear();
        self.last_glyph = None;
        self.images.clear();
        self.image_bytes.clear();
    }

    pub fn glyph(&self, id: GlyphId) -> &CachedGlyph {
        &self.glyphs[id.0 as usize]
    }

    /// El `Rc` del glifo: su dirección es la identidad estable que usa el atlas de la GPU.
    // Lo usa el backend GPU (tarea 16); hasta entonces solo los tests.
    #[allow(dead_code)]
    pub fn glyph_rc(&self, id: GlyphId) -> &Rc<CachedGlyph> {
        &self.glyphs[id.0 as usize]
    }

    pub fn image(&self, id: ImageId) -> ImageRef<'_> {
        let e = &self.images[id.0 as usize];
        let len = e.width as usize * e.height as usize * 4;
        ImageRef {
            rgba: &self.image_bytes[e.offset..e.offset + len],
            width: e.width,
            height: e.height,
        }
    }

    /// Número de glifos distintos que usa la escena.
    #[cfg(test)]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Registra `glyph` (una sola vez por identidad) y devuelve su id.
    pub(super) fn intern_glyph(&mut self, glyph: &Rc<CachedGlyph>) -> GlyphId {
        let key = Rc::as_ptr(glyph) as usize;
        if let Some((k, id)) = self.last_glyph {
            if k == key {
                return id;
            }
        }
        let next = self.glyphs.len() as u32;
        let id = GlyphId(*self.glyph_ids.entry(key).or_insert(next));
        if id.0 == next {
            self.glyphs.push(Rc::clone(glyph));
        }
        self.last_glyph = Some((key, id));
        id
    }

    /// Copia los píxeles de `img` a la escena. `img` debe estar completa (`width * height * 4` bytes).
    // ponytail: copia por fotograma; el backend GPU podrá cachear por identidad más adelante.
    pub(super) fn push_image(&mut self, img: &ImageRef<'_>) -> ImageId {
        let len = img.width as usize * img.height as usize * 4;
        self.images.push(ImageEntry {
            offset: self.image_bytes.len(),
            width: img.width,
            height: img.height,
        });
        self.image_bytes.extend_from_slice(&img.rgba[..len]);
        ImageId(self.images.len() as u32 - 1)
    }
}
