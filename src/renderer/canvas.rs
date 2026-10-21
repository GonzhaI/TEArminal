//! `Canvas`: envoltorio inmediato sobre un framebuffer XRGB. Cada método delega en las primitivas
//! existentes (misma salida bit a bit); es la API a la que migra el dibujo y desaparece en la tarea 11.

// Aún sin consumidores: las tareas 3-9 migran el dibujo a `Canvas` y este permiso se retira.
#![allow(dead_code)]

use super::{blit_glyph, fill_rect, CachedGlyph, Clip};
use crate::theme::Color;

/// Imagen RGBA de 8 bits por canal (`width * height * 4` bytes).
pub struct ImageRef<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

pub struct Canvas<'a> {
    buf: &'a mut [u32],
    w: usize,
    h: usize,
}

impl<'a> Canvas<'a> {
    pub fn over(buf: &'a mut [u32], w: usize, h: usize) -> Self {
        Self { buf, w, h }
    }

    pub fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    /// Píxeles crudos del búfer. Temporal: la copia de Ajustes desde su búfer fuera de pantalla lo
    /// necesita hasta la tarea 10, que la sustituye.
    pub(super) fn pixels_mut(&mut self) -> &mut [u32] {
        self.buf
    }

    pub fn clear(&mut self, color: Color) {
        self.buf.fill(color.to_u32_xrgb());
    }

    pub fn fill_rect(&mut self, clip: Clip, x: usize, y: usize, w: usize, h: usize, color: Color) {
        fill_rect(self.buf, self.w, clip, x, y, w, h, color.to_u32_xrgb());
    }

    pub fn glyph(&mut self, clip: Clip, glyph: &CachedGlyph, gx: i32, gy: i32, fg: Color) {
        blit_glyph(self.buf, self.w, clip, glyph, gx, gy, fg);
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
        crate::ui::draw_rounded_rect(self.buf, self.w, self.h, x, y, w, h, radius, color);
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
        crate::ui::stroke_rounded_rect(self.buf, self.w, self.h, x, y, w, h, radius, color);
    }

    /// Copia `img` escalada (vecino más próximo) a `dst = (x, y, ancho, alto)`, recortada a `clip`.
    /// Sustituye al bucle inline que tenía `pane.rs` (misma salida salvo con `rgba` truncado, donde no dibuja nada).
    pub fn image(&mut self, clip: Clip, img: ImageRef<'_>, dst: (i32, i32, usize, usize)) {
        let (iw, ih) = (img.width as usize, img.height as usize);
        if iw == 0 || ih == 0 || img.rgba.len() < iw * ih * 4 {
            return;
        }
        let (ix, iy, tw, th) = dst;
        // El recorte nunca sale del búfer
        let (x0, y0) = (clip.0 as i32, clip.1 as i32);
        let (x1, y1) = (clip.2.min(self.w) as i32, clip.3.min(self.h) as i32);
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
        crate::ui::frame_card(self.buf, self.w, self.h, region, inset, radius, outer);
    }
}
