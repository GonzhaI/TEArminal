//! `Canvas`: envoltorio inmediato sobre un framebuffer XRGB. Cada método delega en las primitivas
//! existentes (misma salida bit a bit); es la API a la que migra el dibujo y desaparece en la tarea 11.

use super::{blit_glyph, CachedGlyph, Clip};
use crate::theme::Color;

/// Imagen RGBA de 8 bits por canal (`width * height * 4` bytes).
pub struct ImageRef<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// Recorte absoluto `(x0, y0, x1, y1)` en el búfer real (límite superior exclusivo).
type RealClip = (i32, i32, i32, i32);

fn intersect(a: RealClip, b: RealClip) -> RealClip {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

pub struct Canvas<'a> {
    buf: &'a mut [u32],
    w: usize,
    h: usize,
    /// Origen local en el búfer real (`with_offset`); (0, 0) fuera de él.
    ox: i32,
    oy: i32,
    /// Tamaño local que ven las pantallas (`size()`); igual al real fuera de `with_offset`.
    lw: usize,
    lh: usize,
    /// Recorte efectivo en coordenadas reales: dentro del búfer y de `with_offset`.
    clip: RealClip,
}

impl<'a> Canvas<'a> {
    pub fn over(buf: &'a mut [u32], w: usize, h: usize) -> Self {
        Self {
            buf,
            w,
            h,
            ox: 0,
            oy: 0,
            lw: w,
            lh: h,
            clip: (0, 0, w as i32, h as i32),
        }
    }

    /// Tamaño local: el del búfer, o el declarado por el `with_offset` en curso.
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

    /// Píxeles del búfer, solo lectura (los tests comprueban lo dibujado sin soltar el lienzo).
    #[cfg(test)]
    pub(crate) fn pixels(&self) -> &[u32] {
        self.buf
    }

    pub fn clear(&mut self, color: Color) {
        let c = self.clip;
        if c == (0, 0, self.w as i32, self.h as i32) {
            self.buf.fill(color.to_u32_xrgb());
        } else {
            self.fill_real(c, c.0, c.1, c.2 - c.0, c.3 - c.1, color);
        }
    }

    /// Relleno sólido en coordenadas reales `(x, y)` con origen posiblemente negativo.
    fn fill_real(&mut self, clip: RealClip, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let (x0, x1) = (x.max(clip.0), (x + w).min(clip.2));
        let (y0, y1) = (y.max(clip.1), (y + h).min(clip.3));
        if x0 >= x1 {
            return;
        }
        let px = color.to_u32_xrgb();
        for py in y0..y1 {
            let row = py as usize * self.w;
            self.buf[row + x0 as usize..row + x1 as usize].fill(px);
        }
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

    pub fn glyph(&mut self, clip: Clip, glyph: &CachedGlyph, gx: i32, gy: i32, fg: Color) {
        let c = self.real_clip(clip);
        if c.0 >= c.2 || c.1 >= c.3 {
            return;
        }
        // El recorte efectivo está dentro del búfer: no es negativo
        let clip = (c.0 as usize, c.1 as usize, c.2 as usize, c.3 as usize);
        blit_glyph(
            self.buf,
            self.w,
            clip,
            glyph,
            gx + self.ox,
            gy + self.oy,
            fg,
        );
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
        crate::ui::draw_rounded_rect_clipped(
            self.buf,
            self.w,
            self.clip,
            x as i32 + self.ox,
            y as i32 + self.oy,
            w,
            h,
            radius,
            color,
        );
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
        crate::ui::stroke_rounded_rect_clipped(
            self.buf,
            self.w,
            self.clip,
            x as i32 + self.ox,
            y as i32 + self.oy,
            w,
            h,
            radius,
            color,
        );
    }

    /// Copia `img` escalada (vecino más próximo) a `dst = (x, y, ancho, alto)`, recortada a `clip`.
    /// Sustituye al bucle inline que tenía `pane.rs` (misma salida salvo con `rgba` truncado, donde no dibuja nada).
    pub fn image(&mut self, clip: Clip, img: ImageRef<'_>, dst: (i32, i32, usize, usize)) {
        let (iw, ih) = (img.width as usize, img.height as usize);
        if iw == 0 || ih == 0 || img.rgba.len() < iw * ih * 4 {
            return;
        }
        let (ix, iy, tw, th) = dst;
        let (ix, iy) = (ix + self.ox, iy + self.oy);
        // El recorte efectivo nunca sale del búfer
        let (x0, y0, x1, y1) = self.real_clip(clip);
        for dy in 0..th {
            let py = iy + dy as i32;
            if py < y0 || py >= y1 {
                continue;
            }
            let src_y = dy * ih / th;
            for dx in 0..tw {
                let px = ix + dx as i32;
                if px < x0 || px >= x1 {
                    continue;
                }
                let i = (src_y * iw + dx * iw / tw) * 4;
                let (r, g, b, a) = (
                    img.rgba[i],
                    img.rgba[i + 1],
                    img.rgba[i + 2],
                    img.rgba[i + 3],
                );
                let d = &mut self.buf[py as usize * self.w + px as usize];
                if a == 255 {
                    *d = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                } else if a > 0 {
                    let bg = Color::from_rgb((*d >> 16) as u8, (*d >> 8) as u8, *d as u8);
                    *d = bg.blend_over(Color::from_rgb(r, g, b), a);
                }
            }
        }
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
        crate::ui::frame_card(self.buf, self.w, self.h, region, inset, radius, outer);
    }
}
