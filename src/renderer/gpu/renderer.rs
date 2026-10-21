//! Ejecuta una `Scene` en Direct3D 11: `encode` (pura) la convierte en instancias de quad y lotes por
//! textura; `GpuRenderer` las sube a un buffer dinámico y las dibuja en orden, vaciando el atlas de
//! glifos (tras volcar lo ya codificado) cuando se llena.

use super::{AtlasError, AtlasRect, GlyphAtlas, GpuDevice, ImageTexture, Instance, Kind, Pipeline};
use crate::renderer::scene::{GlyphId, ImageId, Op, RealClip, Scene};
use crate::theme::Color;
use windows::core::{Error, Result};
use windows::Win32::Foundation::E_FAIL;
#[cfg(test)]
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use windows::Win32::Graphics::Direct3D11::*;
#[cfg(test)]
use windows::Win32::Graphics::Dxgi::Common::*;

/// Instancias consecutivas que comparten enlace de textura: `image == None` es «sin textura o solo atlas».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch {
    pub first: u32,
    pub count: u32,
    pub image: Option<ImageId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeStop {
    Done,
    /// El atlas no admite el glifo de la operación `at_op`: lo codificado hasta ahí (sin incluirla) es válido.
    AtlasFull {
        at_op: usize,
    },
}

/// Rect del glifo en el atlas (lo sube si hace falta). `Full` = vaciar el atlas y reintentar.
pub type RectFor<'a> =
    dyn FnMut(&Scene, GlyphId) -> std::result::Result<AtlasRect, AtlasError> + 'a;

fn color_bits(c: Color) -> u32 {
    (c.a as u32) << 24 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

/// Recorte de la escena acotado al destino; `None` si queda vacío.
fn clamp_clip(c: RealClip, tw: i32, th: i32) -> Option<[u16; 4]> {
    let (x0, y0) = (c.0.max(0), c.1.max(0));
    let (x1, y1) = (c.2.min(tw), c.3.min(th));
    (x0 < x1 && y0 < y1).then_some([x0 as u16, y0 as u16, x1 as u16, y1 as u16])
}

/// ¿Corta el rectángulo `(x, y, w, h)` con el recorte? (`i64`: sin desbordes con tamaños absurdos.)
fn visible(x: i32, y: i32, w: u32, h: u32, clip: [u16; 4]) -> bool {
    let (x, y) = (x as i64, y as i64);
    w > 0
        && h > 0
        && x < clip[2] as i64
        && y < clip[3] as i64
        && x + w as i64 > clip[0] as i64
        && y + h as i64 > clip[1] as i64
}

/// El rect cabe en `i16` (los campos de `Instance`). Con la forma visible solo falla si es descomunal.
// ponytail: una forma de más de 32767 px se omite; el destino máximo mide 16384
fn rect_i16(x: i32, y: i32, w: u32, h: u32) -> Option<[i16; 4]> {
    Some([
        i16::try_from(x).ok()?,
        i16::try_from(y).ok()?,
        i16::try_from(w).ok()?,
        i16::try_from(h).ok()?,
    ])
}

struct Sink<'a> {
    out: &'a mut Vec<Instance>,
    batches: &'a mut Vec<Batch>,
}

impl Sink<'_> {
    fn push(&mut self, image: Option<ImageId>, inst: Instance) {
        let idx = self.out.len() as u32;
        self.out.push(inst);
        match self.batches.last_mut() {
            Some(b) if b.image == image && b.first + b.count == idx => b.count += 1,
            _ => self.batches.push(Batch {
                first: idx,
                count: 1,
                image,
            }),
        }
    }

    /// Relleno opaco de `(x0, y0)-(x1, y1)` (ya dentro del destino); vacío = nada.
    fn solid(&mut self, (x0, y0, x1, y1): (i32, i32, i32, i32), color: Color) {
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let clip = [x0 as u16, y0 as u16, x1 as u16, y1 as u16];
        let rect = [x0 as i16, y0 as i16, (x1 - x0) as i16, (y1 - y0) as i16];
        self.push(
            None,
            Instance::new(Kind::Solid, rect, clip, color_bits(color), [0, 0], 0, 0),
        );
    }
}

/// Convierte las operaciones de `from_op` en adelante, EN ORDEN, en instancias y lotes (se añaden a
/// `out` y `batches`). `target` es el tamaño real del destino: acota los recortes. Se detiene antes
/// de la primera operación cuyo glifo no cabe en el atlas.
pub fn encode(
    scene: &Scene,
    from_op: usize,
    target: (u32, u32),
    out: &mut Vec<Instance>,
    batches: &mut Vec<Batch>,
    rect_for: &mut RectFor<'_>,
) -> std::result::Result<EncodeStop, AtlasError> {
    let (tw, th) = (target.0.min(16384) as i32, target.1.min(16384) as i32);
    let mut sink = Sink { out, batches };
    for (i, op) in scene.ops.iter().enumerate().skip(from_op) {
        match *op {
            // Un Solid a pantalla completa (no un clear del contexto): conserva el orden con el resto
            Op::Clear(color) => sink.solid((0, 0, tw, th), color),
            Op::Fill {
                clip,
                x,
                y,
                w,
                h,
                color,
            } => {
                let Some(c) = clamp_clip(clip, tw, th) else {
                    continue;
                };
                let (x, y) = (x as i64, y as i64);
                let x0 = x.max(c[0] as i64);
                let y0 = y.max(c[1] as i64);
                let x1 = (x + w as i64).min(c[2] as i64);
                let y1 = (y + h as i64).min(c[3] as i64);
                sink.solid((x0 as i32, y0 as i32, x1 as i32, y1 as i32), color);
            }
            Op::Glyph {
                clip,
                glyph,
                x,
                y,
                color,
            } => {
                let Some(c) = clamp_clip(clip, tw, th) else {
                    continue;
                };
                let g = scene.glyph(glyph);
                // Fuera del recorte no se sube al atlas
                if !visible(x, y, g.width as u32, g.height as u32, c) {
                    continue;
                }
                let r = match rect_for(scene, glyph) {
                    Ok(r) => r,
                    Err(AtlasError::Full) => return Ok(EncodeStop::AtlasFull { at_op: i }),
                    Err(e) => return Err(e),
                };
                if r.w == 0 || r.h == 0 {
                    continue;
                }
                let Some(rect) = rect_i16(x, y, r.w as u32, r.h as u32) else {
                    continue;
                };
                sink.push(
                    None,
                    Instance::new(Kind::Glyph, rect, c, color_bits(color), [r.x, r.y], 0, 0),
                );
            }
            Op::Rounded {
                clip,
                x,
                y,
                w,
                h,
                radius,
                color,
                stroke,
            } => {
                let Some(c) = clamp_clip(clip, tw, th) else {
                    continue;
                };
                if w == 0 || h == 0 || color.a() == 0 || !visible(x, y, w, h, c) {
                    continue;
                }
                let Some(rect) = rect_i16(x, y, w, h) else {
                    continue;
                };
                let kind = if stroke {
                    Kind::RoundedStroke
                } else {
                    Kind::RoundedFill
                };
                let radius = radius.min(255) as u8;
                sink.push(
                    None,
                    Instance::new(kind, rect, c, color_bits(color), [0, 0], radius, 0),
                );
            }
            Op::Image {
                clip,
                image,
                x,
                y,
                w,
                h,
            } => {
                let Some(c) = clamp_clip(clip, tw, th) else {
                    continue;
                };
                if w == 0 || h == 0 || !visible(x, y, w, h, c) {
                    continue;
                }
                let Some(rect) = rect_i16(x, y, w, h) else {
                    continue;
                };
                sink.push(
                    Some(image),
                    Instance::new(Kind::Image, rect, c, 0, [0, 0], 0, 0),
                );
            }
            Op::Card {
                region,
                inset,
                radius,
                outer,
            } => encode_card(&mut sink, (tw, th), region, inset, radius, outer),
        }
    }
    Ok(EncodeStop::Done)
}

/// Misma geometría que `ui::frame_card`: márgenes sólidos (franja superior, inferior, columnas laterales)
/// y un cuarto de círculo de `outer` en cada esquina de la tarjeta.
fn encode_card(
    sink: &mut Sink<'_>,
    (bw, bh): (i32, i32),
    (x, y, w, h): (u32, u32, u32, u32),
    inset: u32,
    radius: u32,
    outer: Color,
) {
    let (bw, bh) = (bw as i64, bh as i64);
    let (x, y, w, h, inset) = (x as i64, y as i64, w as i64, h as i64, inset as i64);
    let (x_end, y_end) = ((x + w).min(bw), (y + h).min(bh));
    if w <= inset * 2 || h <= inset * 2 || x >= x_end || y >= y_end {
        return;
    }
    let (cx0, cy0) = (x + inset, y + inset);
    let (cx1, cy1) = (x + w - inset, y + h - inset);
    let r = (radius as i64).min((cx1 - cx0) / 2).min((cy1 - cy0) / 2);
    let mut solid = |x0: i64, y0: i64, x1: i64, y1: i64| {
        sink.solid((x0 as i32, y0 as i32, x1 as i32, y1 as i32), outer);
    };
    let mid_end = cy1.min(y_end);
    solid(x, y, x_end, cy0.min(y_end)); // franja superior
    solid(x, cy1.max(y), x_end, y_end); // franja inferior
    solid(x, cy0, cx0.min(x_end), mid_end); // columna izquierda
    solid(cx1, cy0, x_end, mid_end); // columna derecha

    // ponytail: una esquina de más de 255 px se acota a 255 (el radio de la instancia es u8)
    let r = r.min(255);
    if r == 0 {
        return;
    }
    let clip = [0, 0, bw as u16, bh as u16];
    for (corner, (px, py)) in [
        (0u8, (cx0, cy0)),
        (1, (cx1 - r, cy0)),
        (2, (cx0, cy1 - r)),
        (3, (cx1 - r, cy1 - r)),
    ] {
        if px >= bw || py >= bh {
            continue;
        }
        let rect = [px as i16, py as i16, r as i16, r as i16];
        sink.push(
            None,
            Instance::new(
                Kind::CardCorner,
                rect,
                clip,
                color_bits(outer),
                [0, 0],
                r as u8,
                corner,
            ),
        );
    }
}

fn into_windows(e: AtlasError) -> Error {
    match e {
        AtlasError::Device(e) => e,
        AtlasError::Full => Error::from(E_FAIL),
    }
}

const INSTANCE_STRIDE: u32 = std::mem::size_of::<Instance>() as u32;
const INITIAL_INSTANCES: u32 = 4096;

pub struct GpuRenderer {
    dev: GpuDevice,
    pipeline: Pipeline,
    atlas: GlyphAtlas,
    instance_buf: ID3D11Buffer,
    /// Instancias que caben en `instance_buf` (crece por duplicación)
    capacity: u32,
    // Memoria de trabajo reutilizada entre fotogramas
    instances: Vec<Instance>,
    batches: Vec<Batch>,
}

fn create_instance_buffer(dev: &GpuDevice, count: u32) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: count * INSTANCE_STRIDE,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        ..Default::default()
    };
    let mut buf = None;
    unsafe { dev.device.CreateBuffer(&desc, None, Some(&mut buf))? };
    buf.ok_or_else(|| Error::from(E_FAIL))
}

impl GpuRenderer {
    pub fn new(dev: GpuDevice) -> Result<Self> {
        let atlas = GlyphAtlas::new(&dev).map_err(into_windows)?;
        Self::build(dev, atlas)
    }

    /// Atlas de tamaño inicial y tope dados (los tests lo llenan con pocos glifos).
    #[cfg(test)]
    pub fn with_atlas_limits(dev: GpuDevice, initial: u32, max: u32) -> Result<Self> {
        let atlas = GlyphAtlas::with_limits(&dev, initial, max).map_err(into_windows)?;
        Self::build(dev, atlas)
    }

    fn build(dev: GpuDevice, atlas: GlyphAtlas) -> Result<Self> {
        let pipeline = Pipeline::new(&dev)?;
        let instance_buf = create_instance_buffer(&dev, INITIAL_INSTANCES)?;
        Ok(Self {
            dev,
            pipeline,
            atlas,
            instance_buf,
            capacity: INITIAL_INSTANCES,
            instances: Vec::new(),
            batches: Vec::new(),
        })
    }

    pub fn device(&self) -> &GpuDevice {
        &self.dev
    }

    /// Dibuja `scene` en `target` (de `target_size` píxeles, acotado a la textura máxima del dispositivo).
    /// No limpia el destino: la escena empieza con su `Op::Clear` si quiere fondo.
    pub fn render(
        &mut self,
        scene: &Scene,
        target: &ID3D11RenderTargetView,
        target_size: (u32, u32),
    ) -> Result<()> {
        let max = self.dev.max_texture_dim();
        let size = (target_size.0.min(max), target_size.1.min(max));
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }
        self.bind_frame(target, size)?;
        let mut next_op = 0;
        // Operación tras cuyo vaciado del atlas se reintenta: si vuelve a fallar, el glifo no cabe nunca
        let mut reset_for: Option<usize> = None;
        loop {
            self.instances.clear();
            self.batches.clear();
            let Self {
                dev,
                atlas,
                instances,
                batches,
                ..
            } = self;
            let stop = encode(scene, next_op, size, instances, batches, &mut |sc, id| {
                atlas.rect_for(dev, sc.glyph_rc(id))
            })
            .map_err(into_windows)?;
            // Se dibuja lo codificado ANTES de tocar el atlas: el vaciado reutiliza sus huecos
            self.flush(scene)?;
            match stop {
                EncodeStop::Done => return Ok(()),
                EncodeStop::AtlasFull { at_op } if reset_for == Some(at_op) => {
                    crate::logger::log(&format!(
                        "GPU: glifo de la operación {at_op} no cabe en el atlas ({}); se omite",
                        self.atlas.size()
                    ));
                    reset_for = None;
                    next_op = at_op + 1;
                }
                EncodeStop::AtlasFull { at_op } => {
                    self.atlas.reset();
                    reset_for = Some(at_op);
                    next_op = at_op;
                }
            }
        }
    }

    /// Estado fijo del fotograma: destino, viewport, shaders, mezcla y tamaño en el constant buffer.
    fn bind_frame(&self, target: &ID3D11RenderTargetView, (w, h): (u32, u32)) -> Result<()> {
        let (ctx, p) = (&self.dev.context, &self.pipeline);
        let constants = [w as f32, h as f32, 0.0, 0.0];
        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: w as f32,
            Height: h as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        unsafe {
            ctx.UpdateSubresource(&p.constants, 0, None, constants.as_ptr().cast(), 0, 0);
            ctx.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            ctx.RSSetViewports(Some(&[viewport]));
            ctx.RSSetState(&p.raster);
            ctx.OMSetBlendState(&p.blend, None, 0xFFFF_FFFF);
            ctx.IASetInputLayout(&p.layout);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            ctx.VSSetShader(&p.vs, None);
            ctx.PSSetShader(&p.ps, None);
            ctx.VSSetConstantBuffers(0, Some(&[Some(p.constants.clone())]));
            ctx.PSSetSamplers(0, Some(&[Some(p.sampler.clone())]));
        }
        Ok(())
    }

    /// Sube `self.instances` al buffer dinámico (duplicando su capacidad si no caben).
    fn upload(&mut self) -> Result<()> {
        let n = self.instances.len() as u32;
        if n > self.capacity {
            let mut cap = self.capacity;
            while cap < n {
                cap *= 2;
            }
            self.instance_buf = create_instance_buffer(&self.dev, cap)?;
            self.capacity = cap;
        }
        let bytes = n as usize * INSTANCE_STRIDE as usize;
        let mut map = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            let ctx = &self.dev.context;
            ctx.Map(
                &self.instance_buf,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut map),
            )?;
            std::ptr::copy_nonoverlapping(
                self.instances.as_ptr().cast::<u8>(),
                map.pData.cast::<u8>(),
                bytes,
            );
            ctx.Unmap(&self.instance_buf, 0);
        }
        Ok(())
    }

    /// Dibuja las instancias codificadas: una llamada instanciada por lote.
    fn flush(&mut self, scene: &Scene) -> Result<()> {
        if self.instances.is_empty() {
            return Ok(());
        }
        self.upload()?;
        let ctx = &self.dev.context;
        unsafe {
            let (buf, stride, offset) = (Some(self.instance_buf.clone()), INSTANCE_STRIDE, 0u32);
            ctx.IASetVertexBuffers(0, 1, Some(&buf), Some(&stride), Some(&offset));
        }
        for b in &self.batches {
            // ponytail: una textura por imagen y fotograma; cachear por identidad si pesa
            let tex = match b.image {
                Some(id) => {
                    let im = scene.image(id);
                    match ImageTexture::new(&self.dev, im.rgba, im.width, im.height)? {
                        Some(t) => Some(t),
                        None => continue, // vacía, truncada o demasiado grande: no se dibuja
                    }
                }
                None => None,
            };
            let views = [
                Some(self.atlas.srv().clone()),
                tex.as_ref().map(|t| t.srv().clone()),
            ];
            unsafe {
                ctx.PSSetShaderResources(0, Some(&views));
                ctx.DrawInstanced(4, b.count, 0, b.first);
            }
        }
        Ok(())
    }

    /// Dibuja `scene` en una textura sin ventana que empieza en negro y devuelve `0x00RRGGBB` por píxel
    /// con el tamaño real (acotado a la textura máxima). Solo tests y capturas.
    #[cfg(test)]
    pub fn render_to_pixels(&mut self, scene: &Scene) -> Result<(Vec<u32>, u32, u32)> {
        self.render_offscreen(scene, None)
    }

    /// Como `render_to_pixels`, con `base` (`scene.w * scene.h` píxeles `0x00RRGGBB`) como contenido inicial.
    #[cfg(test)]
    pub fn render_to_pixels_over(
        &mut self,
        scene: &Scene,
        base: &[u32],
    ) -> Result<(Vec<u32>, u32, u32)> {
        self.render_offscreen(scene, Some(base))
    }

    #[cfg(test)]
    fn render_offscreen(
        &mut self,
        scene: &Scene,
        base: Option<&[u32]>,
    ) -> Result<(Vec<u32>, u32, u32)> {
        let max = self.dev.max_texture_dim();
        let (w, h) = (scene.w.min(max), scene.h.min(max));
        if w == 0 || h == 0 {
            return Ok((Vec::new(), w, h));
        }
        let (fw, wz, hz) = (scene.w as usize, w as usize, h as usize);
        // Contenido inicial, recortado al tamaño acotado (alfa 255: no importa para la mezcla)
        let init: Option<Vec<u32>> = match base {
            Some(b) if b.len() < fw * scene.h as usize => return Err(Error::from(E_INVALIDARG)),
            Some(b) => Some(
                (0..hz)
                    .flat_map(|y| b[y * fw..y * fw + wz].iter().map(|p| p | 0xFF00_0000))
                    .collect(),
            ),
            None => None,
        };
        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let data = init.as_ref().map(|v| D3D11_SUBRESOURCE_DATA {
            pSysMem: v.as_ptr().cast(),
            SysMemPitch: w * 4,
            SysMemSlicePitch: 0,
        });
        let (tex, rtv) = unsafe {
            let mut tex = None;
            self.dev.device.CreateTexture2D(
                &desc,
                data.as_ref().map(|d| d as *const _),
                Some(&mut tex),
            )?;
            let tex = tex.ok_or_else(|| Error::from(E_FAIL))?;
            let mut rtv = None;
            self.dev
                .device
                .CreateRenderTargetView(&tex, None, Some(&mut rtv))?;
            (tex, rtv.ok_or_else(|| Error::from(E_FAIL))?)
        };
        if init.is_none() {
            unsafe {
                self.dev
                    .context
                    .ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 1.0]);
            }
        }
        self.render(scene, &rtv, (w, h))?;

        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let ctx = &self.dev.context;
        let mut pixels = Vec::with_capacity(wz * hz);
        unsafe {
            let mut staging = None;
            self.dev
                .device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.ok_or_else(|| Error::from(E_FAIL))?;
            ctx.CopyResource(&staging, &tex);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
            for row in 0..hz {
                let src = std::slice::from_raw_parts(
                    map.pData
                        .cast::<u8>()
                        .add(row * map.RowPitch as usize)
                        .cast::<u32>(),
                    wz,
                );
                pixels.extend(src.iter().map(|p| p & 0x00FF_FFFF));
            }
            ctx.Unmap(&staging, 0);
        }
        Ok((pixels, w, h))
    }
}
