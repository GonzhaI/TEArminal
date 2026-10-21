//! `Canvas`: graba el dibujo en una `Scene` (no escribe píxeles). Traslada las coordenadas locales de
//! `with_offset`, intersecta el recorte efectivo y guarda cada operación con coordenadas absolutas;
//! `raster_cpu::rasterize` la reproduce en orden con las primitivas de siempre.

use super::scene::{ImageRef, Op, RealClip, Scene};
use super::{CachedGlyph, Clip};
use crate::theme::Color;
use std::rc::Rc;

fn intersect(a: RealClip, b: RealClip) -> RealClip {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn is_empty(c: RealClip) -> bool {
    c.0 >= c.2 || c.1 >= c.3
}

#[derive(Default)]
pub struct Canvas {
    scene: Scene,
    /// Origen local en el búfer real (`with_offset`); (0, 0) fuera de él.
    ox: i32,
    oy: i32,
    /// Tamaño local que ven las pantallas (`size()`); igual al real fuera de `with_offset`.
    lw: usize,
    lh: usize,
    /// Recorte efectivo en coordenadas reales: dentro del búfer y de `with_offset`.
    clip: RealClip,
}

impl Canvas {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empieza un fotograma `w` x `h`: vacía la escena conservando su memoria y reinicia
    /// desplazamiento y recorte.
    pub fn begin(&mut self, w: u32, h: u32) {
        self.scene.reset(w, h);
        self.ox = 0;
        self.oy = 0;
        self.lw = w as usize;
        self.lh = h as usize;
        self.clip = (0, 0, w as i32, h as i32);
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Dibuja lo grabado hasta ahora en la GPU (WARP) sobre negro (los tests comprueban lo dibujado
    /// sin soltar el lienzo).
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Vec<u32> {
        super::test_support::render_scene(&self.scene)
    }

    /// Tamaño local: el del fotograma, o el declarado por el `with_offset` en curso.
    pub fn size(&self) -> (usize, usize) {
        (self.lw, self.lh)
    }

    /// Ejecuta `f` con las coordenadas trasladadas `(dx, dy)` al búfer real, `size()` igual a
    /// `local_size` y el recorte `clip` (coordenadas reales) intersecado con el recorte vigente, con
    /// el área local trasladada y con el recorte explícito de cada operación. Sustituye a dibujar
    /// en un búfer aparte y copiar la ventana visible: una forma que sale de la ventana se recorta
    /// con la misma cobertura que tendría entera. Restaura el estado al volver.
    pub fn with_offset(
        &mut self,
        dx: i32,
        dy: i32,
        local_size: (usize, usize),
        clip: Clip,
        f: impl FnOnce(&mut Canvas),
    ) {
        let saved = (self.ox, self.oy, self.lw, self.lh, self.clip);
        let (ox, oy) = (self.ox + dx, self.oy + dy);
        let local = (ox, oy, ox + local_size.0 as i32, oy + local_size.1 as i32);
        let abs = (clip.0 as i32, clip.1 as i32, clip.2 as i32, clip.3 as i32);
        self.clip = intersect(intersect(self.clip, abs), local);
        (self.ox, self.oy, self.lw, self.lh) = (ox, oy, local_size.0, local_size.1);
        f(self);
        (self.ox, self.oy, self.lw, self.lh, self.clip) = saved;
    }

    /// Recorte explícito local (usize) a coordenadas reales, intersecado con el efectivo.
    fn real_clip(&self, clip: Clip) -> RealClip {
        let local = (
            clip.0 as i32 + self.ox,
            clip.1 as i32 + self.oy,
            clip.2 as i32 + self.ox,
            clip.3 as i32 + self.oy,
        );
        intersect(self.clip, local)
    }

    pub fn clear(&mut self, color: Color) {
        let c = self.clip;
        if c == (0, 0, self.scene.w as i32, self.scene.h as i32) {
            self.scene.ops.push(Op::Clear(color));
        } else if !is_empty(c) {
            self.fill_real(c, c.0, c.1, c.2 - c.0, c.3 - c.1, color);
        }
    }

    /// Relleno sólido en coordenadas reales `(x, y)` con origen posiblemente negativo.
    fn fill_real(&mut self, clip: RealClip, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if is_empty(clip) {
            return;
        }
        self.scene.ops.push(Op::Fill {
            clip,
            x,
            y,
            w: w as u32,
            h: h as u32,
            color,
        });
    }

    pub fn fill_rect(&mut self, clip: Clip, x: usize, y: usize, w: usize, h: usize, color: Color) {
        let c = self.real_clip(clip);
        self.fill_real(
            c,
            x as i32 + self.ox,
            y as i32 + self.oy,
            w as i32,
            h as i32,
            color,
        );
    }

    pub fn glyph(&mut self, clip: Clip, glyph: &Rc<CachedGlyph>, gx: i32, gy: i32, fg: Color) {
        let c = self.real_clip(clip);
        if is_empty(c) {
            return;
        }
        let glyph = self.scene.intern_glyph(glyph);
        self.scene.ops.push(Op::Glyph {
            clip: c,
            glyph,
            x: gx + self.ox,
            y: gy + self.oy,
            color: fg,
        });
    }

    fn rounded(
        &mut self,
        (x, y, w, h): (usize, usize, usize, usize),
        radius: usize,
        color: Color,
        stroke: bool,
    ) {
        if is_empty(self.clip) {
            return;
        }
        self.scene.ops.push(Op::Rounded {
            clip: self.clip,
            x: x as i32 + self.ox,
            y: y as i32 + self.oy,
            w: w as u32,
            h: h as u32,
            radius: radius as u32,
            color,
            stroke,
        });
    }

    pub fn rounded_fill(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: Color,
    ) {
        self.rounded((x, y, w, h), radius, color, false);
    }

    pub fn rounded_stroke(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: Color,
    ) {
        self.rounded((x, y, w, h), radius, color, true);
    }

    /// Copia `img` escalada (vecino más próximo) a `dst = (x, y, ancho, alto)`, recortada a `clip`.
    /// Sustituye al bucle inline que tenía `pane.rs` (misma salida salvo con `rgba` truncado, donde no dibuja nada).
    pub fn image(&mut self, clip: Clip, img: ImageRef<'_>, dst: (i32, i32, usize, usize)) {
        let (iw, ih) = (img.width as usize, img.height as usize);
        if iw == 0 || ih == 0 || img.rgba.len() < iw * ih * 4 {
            return;
        }
        let (ix, iy, tw, th) = dst;
        // El recorte efectivo nunca sale del búfer
        let c = self.real_clip(clip);
        if is_empty(c) || tw == 0 || th == 0 {
            return;
        }
        let image = self.scene.push_image(&img);
        self.scene.ops.push(Op::Image {
            clip: c,
            image,
            x: ix + self.ox,
            y: iy + self.oy,
            w: tw as u32,
            h: th as u32,
        });
    }

    pub fn card(
        &mut self,
        region: (usize, usize, usize, usize),
        inset: usize,
        radius: usize,
        outer: Color,
    ) {
        debug_assert!(
            (self.ox, self.oy) == (0, 0),
            "card no admite desplazamiento"
        );
        self.scene.ops.push(Op::Card {
            region: (
                region.0 as u32,
                region.1 as u32,
                region.2 as u32,
                region.3 as u32,
            ),
            inset: inset as u32,
            radius: radius as u32,
            outer,
        });
    }
}
