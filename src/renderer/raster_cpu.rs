//! Rasterizador CPU de referencia: reproduce una `Scene` en orden sobre un framebuffer XRGB con las
//! primitivas de dibujo de siempre (misma salida bit a bit que el dibujo inmediato anterior).

use super::blit_glyph;
use super::scene::{Op, Scene};
use crate::theme::Color;

/// Dibuja `scene` sobre `buf` (`scene.w * scene.h` píxeles XRGB). El búfer no se limpia antes:
/// la escena empieza con su propio `Op::Clear` si quiere fondo.
pub fn rasterize(scene: &Scene, buf: &mut [u32]) {
    let (w, h) = (scene.w as usize, scene.h as usize);
    debug_assert!(buf.len() >= w * h, "búfer menor que la escena");
    if buf.len() < w * h {
        return;
    }
    for op in &scene.ops {
        match *op {
            Op::Clear(color) => buf.fill(color.to_u32_xrgb()),
            Op::Fill {
                clip,
                x,
                y,
                w: fw,
                h: fh,
                color,
            } => {
                let (x0, x1) = (x.max(clip.0), (x + fw as i32).min(clip.2));
                let (y0, y1) = (y.max(clip.1), (y + fh as i32).min(clip.3));
                if x0 >= x1 {
                    continue;
                }
                let px = color.to_u32_xrgb();
                for py in y0..y1 {
                    let row = py as usize * w;
                    buf[row + x0 as usize..row + x1 as usize].fill(px);
                }
            }
            Op::Glyph {
                clip,
                glyph,
                x,
                y,
                color,
            } => {
                if clip.0 >= clip.2 || clip.1 >= clip.3 {
                    continue;
                }
                // El recorte efectivo está dentro del búfer: no es negativo
                let clip = (
                    clip.0 as usize,
                    clip.1 as usize,
                    clip.2 as usize,
                    clip.3 as usize,
                );
                blit_glyph(buf, w, clip, scene.glyph(glyph), x, y, color);
            }
            Op::Rounded {
                clip,
                x,
                y,
                w: rw,
                h: rh,
                radius,
                color,
                stroke,
            } => {
                let (rw, rh, radius) = (rw as usize, rh as usize, radius as usize);
                if stroke {
                    crate::ui::stroke_rounded_rect_clipped(
                        buf, w, clip, x, y, rw, rh, radius, color,
                    );
                } else {
                    crate::ui::draw_rounded_rect_clipped(buf, w, clip, x, y, rw, rh, radius, color);
                }
            }
            Op::Image {
                clip,
                image,
                x: ix,
                y: iy,
                w: tw,
                h: th,
            } => {
                let img = scene.image(image);
                let (iw, ih) = (img.width as usize, img.height as usize);
                let (tw, th) = (tw as usize, th as usize);
                let (x0, y0, x1, y1) = clip;
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
                        let d = &mut buf[py as usize * w + px as usize];
                        if a == 255 {
                            *d = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                        } else if a > 0 {
                            let bg = Color::from_rgb((*d >> 16) as u8, (*d >> 8) as u8, *d as u8);
                            *d = bg.blend_over(Color::from_rgb(r, g, b), a);
                        }
                    }
                }
            }
            Op::Card {
                region: (rx, ry, rw, rh),
                inset,
                radius,
                outer,
            } => crate::ui::frame_card(
                buf,
                w,
                h,
                (rx as usize, ry as usize, rw as usize, rh as usize),
                inset as usize,
                radius as usize,
                outer,
            ),
        }
    }
}
