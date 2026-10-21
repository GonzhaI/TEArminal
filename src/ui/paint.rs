// Primitivas planas de dibujo: coordenadas y color explícitos (ver el mismo criterio en `renderer`).
#![allow(clippy::too_many_arguments)]

use crate::theme::Color;

#[inline(always)]
fn blend_span_const_alpha(row: &mut [u32], color: Color, alpha: u8) {
    if alpha == 0 {
        return;
    }
    if alpha == 255 {
        row.fill(color.to_u32_xrgb());
        return;
    }
    let a = alpha as u32;
    let inv_a = 255 - a;
    let (cr, cg, cb) = (color.r as u32 * a, color.g as u32 * a, color.b as u32 * a);
    for px in row {
        let orig = *px;
        let r = (cr + ((orig >> 16) & 0xFF) * inv_a) / 255;
        let g = (cg + ((orig >> 8) & 0xFF) * inv_a) / 255;
        let b = (cb + (orig & 0xFF) * inv_a) / 255;
        *px = (r << 16) | (g << 8) | b;
    }
}

/// Convierte `region` en una tarjeta: lo ya dibujado queda dentro de un rectángulo con esquinas
/// redondeadas a `inset` px del borde y el resto (el margen y las esquinas) se pinta de `outer`.
/// Va al final del fotograma porque recorta lo que dibujaron otros.
pub fn frame_card(
    buffer: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    region: (usize, usize, usize, usize),
    inset: usize,
    radius: usize,
    outer: Color,
) {
    let (x, y, w, h) = region;
    let (x_end, y_end) = ((x + w).min(buf_width), (y + h).min(buf_height));
    if w <= inset * 2 || h <= inset * 2 || x >= x_end || y >= y_end {
        return;
    }
    let (cx0, cy0) = (x + inset, y + inset);
    let (cx1, cy1) = (x + w - inset, y + h - inset);
    let r = radius.min((cx1 - cx0) / 2).min((cy1 - cy0) / 2);
    let paint = |buffer: &mut [u32], py: usize, from: usize, to: usize| {
        if py < buf_height && from < to.min(buf_width) {
            let row = py * buf_width;
            buffer[row + from..row + to.min(buf_width)].fill(outer.to_u32_xrgb());
        }
    };
    for py in y..y_end {
        if py < cy0 || py >= cy1 {
            paint(buffer, py, x, x_end);
        } else {
            paint(buffer, py, x, cx0);
            paint(buffer, py, cx1, x_end);
        }
    }
    // Esquinas: el cuarto de círculo de la tarjeta; fuera de él, `outer` con la cobertura del borde.
    for (corner_x, corner_y, cx, cy) in [
        (cx0, cy0, cx0 + r, cy0 + r),
        (cx1 - r, cy0, cx1 - r, cy0 + r),
        (cx0, cy1 - r, cx0 + r, cy1 - r),
        (cx1 - r, cy1 - r, cx1 - r, cy1 - r),
    ] {
        for py in corner_y..corner_y + r {
            for px in corner_x..corner_x + r {
                if px >= buf_width || py >= buf_height {
                    continue;
                }
                let (dx, dy) = (px as f32 + 0.5 - cx as f32, py as f32 + 0.5 - cy as f32);
                let outside = ((dx * dx + dy * dy).sqrt() - r as f32 + 0.5).clamp(0.0, 1.0);
                let i = py * buf_width + px;
                blend_span_const_alpha(&mut buffer[i..i + 1], outer, (outside * 255.0) as u8);
            }
        }
    }
}

/// Dibuja un rectángulo con esquinas redondeadas y antialiasing subpíxel perfecto (SDF 2D)
pub fn draw_rounded_rect(
    buffer: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: Color,
) {
    let clip = (0, 0, buf_width as i32, buf_height as i32);
    draw_rounded_rect_clipped(
        buffer, buf_width, clip, x as i32, y as i32, w, h, radius, color,
    );
}

/// Como `draw_rounded_rect`, con origen `i32` (puede ser negativo) y recorte `(x0, y0, x1, y1)`
/// (límite superior exclusivo, dentro del búfer). La cobertura de cada píxel solo depende de su
/// posición respecto a la forma: recortar equivale a dibujarla entera y recortar el resultado.
pub fn draw_rounded_rect_clipped(
    buffer: &mut [u32],
    buf_width: usize,
    clip: (i32, i32, i32, i32),
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    radius: usize,
    color: Color,
) {
    if w == 0 || h == 0 || buf_width == 0 || color.a() == 0 {
        return;
    }
    let x0 = x.max(clip.0);
    let x_end = (x + w as i32).min(clip.2);
    let y0 = y.max(clip.1);
    let y_end = (y + h as i32).min(clip.3);
    if x0 >= x_end || y0 >= y_end {
        return;
    }
    let (x0, x_end) = (x0 as usize, x_end as usize);
    if radius == 0 {
        let ca = color.a();
        for py in y0 as usize..y_end as usize {
            let row_idx = py * buf_width;
            blend_span_const_alpha(&mut buffer[row_idx + x0..row_idx + x_end], color, ca);
        }
        return;
    }

    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let cx = x as f32 + hw;
    let cy = y as f32 + hh;
    let r = (radius as f32).min(hw).min(hh);
    let inner_hw = hw - r;
    let inner_hh = hh - r;
    let ca = color.a();
    let ca_f = ca as f32;

    for py in y0 as usize..y_end as usize {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;

        // Filas centrales fuera de las esquinas: toda la fila tiene dist <= -0.5 (alpha = color.a())
        if qy <= -0.5 {
            blend_span_const_alpha(&mut buffer[row_idx + x0..row_idx + x_end], color, ca);
            continue;
        }

        let ext_y = qy.max(0.0);
        let ext_y_sq = ext_y * ext_y;

        for px in x0..x_end {
            let px_f = (px as f32 + 0.5) - cx;
            let qx = px_f.abs() - inner_hw;

            let dist = if qx <= 0.0 {
                if qy <= 0.0 {
                    qx.max(qy) - r
                } else {
                    ext_y - r
                }
            } else if qy <= 0.0 {
                qx - r
            } else {
                (qx * qx + ext_y_sq).sqrt() - r
            };

            let alpha = if dist <= -0.5 {
                ca
            } else if dist >= 0.5 {
                0
            } else {
                ((0.5 - dist) * ca_f).round() as u8
            };

            if alpha > 0 {
                let idx = row_idx + px;
                let bg = Color::from_rgb(
                    ((buffer[idx] >> 16) & 0xFF) as u8,
                    ((buffer[idx] >> 8) & 0xFF) as u8,
                    (buffer[idx] & 0xFF) as u8,
                );
                buffer[idx] = bg.blend_over(color, alpha);
            }
        }
    }
}

/// Dibuja el contorno (stroke) uniforme de 1px de un rectángulo con esquinas redondeadas
pub fn stroke_rounded_rect(
    buffer: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    border_color: Color,
) {
    let clip = (0, 0, buf_width as i32, buf_height as i32);
    stroke_rounded_rect_clipped(
        buffer,
        buf_width,
        clip,
        x as i32,
        y as i32,
        w,
        h,
        radius,
        border_color,
    );
}

/// Como `stroke_rounded_rect`, con origen `i32` y recorte `(x0, y0, x1, y1)` (ver `draw_rounded_rect_clipped`).
pub fn stroke_rounded_rect_clipped(
    buffer: &mut [u32],
    buf_width: usize,
    clip: (i32, i32, i32, i32),
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    radius: usize,
    border_color: Color,
) {
    if w == 0 || h == 0 || buf_width == 0 || border_color.a() == 0 {
        return;
    }
    let x0 = x.max(clip.0);
    let x_end = (x + w as i32).min(clip.2);
    let y0 = y.max(clip.1);
    let y_end = (y + h as i32).min(clip.3);
    if x0 >= x_end || y0 >= y_end {
        return;
    }
    let (x0, x_end) = (x0 as usize, x_end as usize);

    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let cx = x as f32 + hw;
    let cy = y as f32 + hh;
    let r = (radius as f32).min(hw).min(hh);
    let inner_hw = hw - r;
    let inner_hh = hh - r;
    let ba = border_color.a();
    let ba_f = ba as f32;

    // En las filas centrales donde qy - r <= -1.5, el hueco interior tiene |px - cx| < hw - 1.5
    let skip_x0 = ((cx - (hw - 1.5)).ceil() as usize).max(x0);
    let skip_x1 = ((cx + (hw - 1.5)).floor() as usize).min(x_end);

    for py in y0 as usize..y_end as usize {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;
        let ext_y = qy.max(0.0);
        let ext_y_sq = ext_y * ext_y;
        let can_skip_interior = qy <= 0.0 && (qy - r) <= -1.5 && skip_x0 < skip_x1;

        let mut px = x0;
        while px < x_end {
            if can_skip_interior && (skip_x0..skip_x1).contains(&px) {
                px = skip_x1;
                if px >= x_end {
                    break;
                }
            }
            let px_f = (px as f32 + 0.5) - cx;
            let qx = px_f.abs() - inner_hw;

            let dist = if qx <= 0.0 {
                if qy <= 0.0 {
                    qx.max(qy) - r
                } else {
                    ext_y - r
                }
            } else if qy <= 0.0 {
                qx - r
            } else {
                (qx * qx + ext_y_sq).sqrt() - r
            };

            // Stroke alineado hacia el interior del borde (grosor 1.0px exacto)
            let stroke_dist = (dist + 0.5).abs() - 0.5;
            let alpha = if stroke_dist <= -0.5 {
                ba
            } else if stroke_dist >= 0.5 {
                0
            } else {
                ((0.5 - stroke_dist) * ba_f).round() as u8
            };

            if alpha > 0 {
                let idx = row_idx + px;
                let bg = Color::from_rgb(
                    ((buffer[idx] >> 16) & 0xFF) as u8,
                    ((buffer[idx] >> 8) & 0xFF) as u8,
                    (buffer[idx] & 0xFF) as u8,
                );
                buffer[idx] = bg.blend_over(border_color, alpha);
            }
            px += 1;
        }
    }
}
