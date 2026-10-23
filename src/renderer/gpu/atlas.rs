//! Atlas de glifos (R8) con empaquetado por estantes y texturas de imagen (BGRA).

use super::GpuDevice;
use crate::renderer::glyphs::FastMap;
use crate::renderer::CachedGlyph;
use std::rc::Rc;
use windows::core::Error;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// Empaquetador por estantes: cada estante mide lo que el glifo más alto de su fila; 1 px de separación
/// entre entradas (evita el sangrado del muestreo).
pub(crate) struct ShelfPacker {
    w: u32,
    h: u32,
    shelves: Vec<Shelf>,
}

struct Shelf {
    y: u32,
    h: u32,
    /// Primera x libre
    x: u32,
}

impl ShelfPacker {
    pub(crate) fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            shelves: Vec::new(),
        }
    }

    /// Esquina superior izquierda de un hueco de `w`×`h`, o `None` si no cabe.
    pub(crate) fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + 1, h + 1);
        if pw > self.w || ph > self.h {
            return None;
        }
        let total = self.w;
        if let Some(s) = self
            .shelves
            .iter_mut()
            .find(|s| s.h >= ph && s.x + pw <= total)
        {
            s.x += pw;
            return Some((s.x - pw, s.y));
        }
        // El último estante puede crecer si el glifo es más alto
        let max_h = self.h;
        if let Some(s) = self.shelves.last_mut() {
            if s.h < ph && s.y + ph <= max_h && s.x + pw <= total {
                s.h = ph;
                s.x += pw;
                return Some((s.x - pw, s.y));
            }
        }
        let bottom = self.shelves.last().map_or(0, |s| s.y + s.h);
        if bottom + ph <= self.h {
            self.shelves.push(Shelf {
                y: bottom,
                h: ph,
                x: pw,
            });
            return Some((0, bottom));
        }
        None
    }

    pub(crate) fn reset(&mut self) {
        self.shelves.clear();
    }

    /// Agranda el área conservando lo ya repartido.
    fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AtlasRect {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) w: u16,
    pub(crate) h: u16,
}

impl From<(u16, u16, u16, u16)> for AtlasRect {
    #[inline]
    fn from((x, y, w, h): (u16, u16, u16, u16)) -> Self {
        Self { x, y, w, h }
    }
}

impl From<AtlasRect> for (u16, u16, u16, u16) {
    #[inline]
    fn from(r: AtlasRect) -> Self {
        (r.x, r.y, r.w, r.h)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AtlasError {
    /// No cabe ni creciendo: el llamador vacía el atlas (`reset`) y reintenta.
    #[error("el atlas de texturas está lleno")]
    Full,
    #[error("error del dispositivo Direct3D: {0}")]
    Device(#[from] Error),
}

const INITIAL_SIZE: u32 = 1024;
const MAX_SIZE: u32 = 4096;

pub(crate) struct GlyphAtlas {
    tex: ID3D11Texture2D,
    srv: ID3D11ShaderResourceView,
    size: u32,
    max: u32,
    packer: ShelfPacker,
    /// Dirección del `Rc` -> rect. La entrada retiene el `Rc` para que la dirección no se reutilice
    /// mientras exista (la caché de glifos del renderer se vacía al llegar a 4096).
    map: FastMap<usize, (AtlasRect, Rc<CachedGlyph>)>,
}

fn create_texture(
    dev: &GpuDevice,
    w: u32,
    h: u32,
    format: DXGI_FORMAT,
    init: Option<&D3D11_SUBRESOURCE_DATA>,
) -> windows::core::Result<(ID3D11Texture2D, ID3D11ShaderResourceView)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        ..Default::default()
    };
    let mut tex = None;
    let mut srv = None;
    unsafe {
        dev.device
            .CreateTexture2D(&desc, init.map(|d| d as *const _), Some(&mut tex))?;
        let tex = tex.ok_or_else(|| Error::from(E_FAIL))?;
        dev.device
            .CreateShaderResourceView(&tex, None, Some(&mut srv))?;
        let srv = srv.ok_or_else(|| Error::from(E_FAIL))?;
        Ok((tex, srv))
    }
}

impl GlyphAtlas {
    pub(crate) fn new(dev: &GpuDevice) -> Result<Self, AtlasError> {
        Self::with_limits(dev, INITIAL_SIZE, MAX_SIZE)
    }

    /// Tamaño inicial y tope (acotado a la textura máxima del dispositivo).
    pub(crate) fn with_limits(dev: &GpuDevice, initial: u32, max: u32) -> Result<Self, AtlasError> {
        let max = max.min(dev.max_texture_dim());
        let size = initial.min(max);
        let (tex, srv) = create_texture(dev, size, size, DXGI_FORMAT_R8_UNORM, None)?;
        Ok(Self {
            tex,
            srv,
            size,
            max,
            packer: ShelfPacker::new(size, size),
            map: FastMap::default(),
        })
    }

    pub(crate) fn srv(&self) -> &ID3D11ShaderResourceView {
        &self.srv
    }

    /// Lado actual de la textura cuadrada.
    pub(crate) fn size(&self) -> u32 {
        self.size
    }

    /// Vacía el atlas (el tamaño alcanzado se conserva) y suelta los `Rc` retenidos.
    pub(crate) fn reset(&mut self) {
        self.packer.reset();
        self.map.clear();
    }

    /// Rect del glifo en el atlas; lo sube si aún no está. Un glifo vacío (o con el bitmap truncado)
    /// da el rect vacío sin reservar nada. `Full` = no cabe ni creciendo.
    pub(crate) fn rect_for(
        &mut self,
        dev: &GpuDevice,
        glyph: &Rc<CachedGlyph>,
    ) -> Result<AtlasRect, AtlasError> {
        let key = Rc::as_ptr(glyph) as usize;
        if let Some((r, _)) = self.map.get(&key) {
            return Ok(*r);
        }
        let (w, h) = (glyph.width, glyph.height);
        if w == 0 || h == 0 || glyph.bitmap.len() < w * h {
            return Ok(AtlasRect::default());
        }
        if w >= self.max as usize || h >= self.max as usize {
            return Err(AtlasError::Full);
        }
        let (x, y) = loop {
            if let Some(p) = self.packer.alloc(w as u32, h as u32) {
                break p;
            }
            if self.size >= self.max {
                return Err(AtlasError::Full);
            }
            self.grow(dev)?;
        };
        let region = D3D11_BOX {
            left: x,
            top: y,
            front: 0,
            right: x + w as u32,
            bottom: y + h as u32,
            back: 1,
        };
        unsafe {
            dev.context.UpdateSubresource(
                &self.tex,
                0,
                Some(&region),
                glyph.bitmap.as_ptr().cast(),
                w as u32,
                0,
            );
        }
        let rect = AtlasRect {
            x: x as u16,
            y: y as u16,
            w: w as u16,
            h: h as u16,
        };
        self.map.insert(key, (rect, Rc::clone(glyph)));
        Ok(rect)
    }

    /// Duplica el lado (hasta el tope) copiando en la GPU lo ya subido: los rects siguen valiendo.
    fn grow(&mut self, dev: &GpuDevice) -> windows::core::Result<()> {
        let size = (self.size * 2).min(self.max);
        let (tex, srv) = create_texture(dev, size, size, DXGI_FORMAT_R8_UNORM, None)?;
        let old = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: self.size,
            bottom: self.size,
            back: 1,
        };
        unsafe {
            dev.context
                .CopySubresourceRegion(&tex, 0, 0, 0, 0, &self.tex, 0, Some(&old));
        }
        self.tex = tex;
        self.srv = srv;
        self.size = size;
        self.packer.resize(size, size);
        Ok(())
    }
}

/// Imagen RGBA subida como textura BGRA propia (una por imagen).
pub(crate) struct ImageTexture {
    srv: ID3D11ShaderResourceView,
    #[cfg(test)]
    width: u32,
    #[cfg(test)]
    height: u32,
}

impl ImageTexture {
    /// `Ok(None)` (con aviso en el log) si la imagen es vacía, está truncada o excede la textura máxima.
    pub(crate) fn new(
        dev: &GpuDevice,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> windows::core::Result<Option<ImageTexture>> {
        let len = width as usize * height as usize * 4;
        let max = dev.max_texture_dim();
        if width == 0 || height == 0 || rgba.len() < len || width > max || height > max {
            crate::logger::log(&format!(
                "GPU: imagen descartada ({width}x{height}, {} bytes, máximo {max})",
                rgba.len()
            ));
            return Ok(None);
        }
        let mut bgra = rgba[..len].to_vec();
        for px in bgra.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let init = D3D11_SUBRESOURCE_DATA {
            pSysMem: bgra.as_ptr().cast(),
            SysMemPitch: width * 4,
            SysMemSlicePitch: 0,
        };
        let (_, srv) = create_texture(dev, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, Some(&init))?;
        Ok(Some(ImageTexture {
            srv,
            #[cfg(test)]
            width,
            #[cfg(test)]
            height,
        }))
    }

    pub(crate) fn srv(&self) -> &ID3D11ShaderResourceView {
        &self.srv
    }

    #[cfg(test)]
    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
