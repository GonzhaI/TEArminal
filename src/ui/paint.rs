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
    if w == 0 || h == 0 || buf_width == 0 || buf_height == 0 || color.a() == 0 {
        return;
    }
    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);
    if x >= x_end || y >= y_end {
        return;
    }
    if radius == 0 {
        let ca = color.a();
        for py in y..y_end {
            let row_idx = py * buf_width;
            blend_span_const_alpha(&mut buffer[row_idx + x..row_idx + x_end], color, ca);
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

    for py in y..y_end {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;

        // Filas centrales fuera de las esquinas: toda la fila tiene dist <= -0.5 (alpha = color.a())
        if qy <= -0.5 {
            blend_span_const_alpha(&mut buffer[row_idx + x..row_idx + x_end], color, ca);
            continue;
        }

        let ext_y = qy.max(0.0);
        let ext_y_sq = ext_y * ext_y;

        for px in x..x_end {
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
    if w == 0 || h == 0 || buf_width == 0 || buf_height == 0 || border_color.a() == 0 {
        return;
    }
    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);
    if x >= x_end || y >= y_end {
        return;
    }

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
    let skip_x0 = ((cx - (hw - 1.5)).ceil() as usize).max(x);
    let skip_x1 = ((cx + (hw - 1.5)).floor() as usize).min(x_end);

    for py in y..y_end {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;
        let ext_y = qy.max(0.0);
        let ext_y_sq = ext_y * ext_y;
        let can_skip_interior = qy <= 0.0 && (qy - r) <= -1.5 && skip_x0 < skip_x1;

        let mut px = x;
        while px < x_end {
            if can_skip_interior && px == skip_x0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_draw_rounded_rect(
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
        let hw = w as f32 / 2.0;
        let hh = h as f32 / 2.0;
        let cx = x as f32 + hw;
        let cy = y as f32 + hh;
        let r = (radius as f32).min(hw).min(hh);
        let inner_hw = hw - r;
        let inner_hh = hh - r;
        for py in y..(y + h).min(buf_height) {
            let qy = ((py as f32 + 0.5) - cy).abs() - inner_hh;
            for px in x..(x + w).min(buf_width) {
                let qx = ((px as f32 + 0.5) - cx).abs() - inner_hw;
                let ext_x = qx.max(0.0);
                let ext_y = qy.max(0.0);
                let dist = (ext_x * ext_x + ext_y * ext_y).sqrt() + qx.max(qy).min(0.0) - r;
                let alpha = if dist <= -0.5 {
                    color.a()
                } else if dist >= 0.5 {
                    0
                } else {
                    ((0.5 - dist) * color.a() as f32).round() as u8
                };
                if alpha > 0 {
                    let idx = py * buf_width + px;
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

    fn reference_stroke_rounded_rect(
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
        let hw = w as f32 / 2.0;
        let hh = h as f32 / 2.0;
        let cx = x as f32 + hw;
        let cy = y as f32 + hh;
        let r = (radius as f32).min(hw).min(hh);
        let inner_hw = hw - r;
        let inner_hh = hh - r;
        for py in y..(y + h).min(buf_height) {
            let qy = ((py as f32 + 0.5) - cy).abs() - inner_hh;
            for px in x..(x + w).min(buf_width) {
                let qx = ((px as f32 + 0.5) - cx).abs() - inner_hw;
                let ext_x = qx.max(0.0);
                let ext_y = qy.max(0.0);
                let dist = (ext_x * ext_x + ext_y * ext_y).sqrt() + qx.max(qy).min(0.0) - r;
                let stroke_dist = (dist + 0.5).abs() - 0.5;
                let alpha = if stroke_dist <= -0.5 {
                    border_color.a()
                } else if stroke_dist >= 0.5 {
                    0
                } else {
                    ((0.5 - stroke_dist) * border_color.a() as f32).round() as u8
                };
                if alpha > 0 {
                    let idx = py * buf_width + px;
                    let bg = Color::from_rgb(
                        ((buffer[idx] >> 16) & 0xFF) as u8,
                        ((buffer[idx] >> 8) & 0xFF) as u8,
                        (buffer[idx] & 0xFF) as u8,
                    );
                    buffer[idx] = bg.blend_over(border_color, alpha);
                }
            }
        }
    }

    #[test]
    fn test_sdf_rounded_rect_and_stroke_bit_exact() {
        let (bw, bh) = (320, 200);
        let mut fast = vec![0x00161618u32; bw * bh];
        let mut reference = fast.clone();
        for &(x, y, w, h, r, c) in &[
            (10, 10, 280, 140, 12, Color::from_rgba(34, 34, 39, 255)),
            (15, 18, 200, 85, 10, Color::from_rgba(81, 226, 122, 180)),
            (40, 30, 121, 33, 0, Color::from_rgba(78, 146, 249, 70)),
            (60, 50, 17, 19, 5, Color::from_rgba(244, 56, 65, 255)),
            (5, 5, 315, 195, 8, Color::from_rgba(255, 255, 255, 24)),
        ] {
            draw_rounded_rect(&mut fast, bw, bh, x, y, w, h, r, c);
            reference_draw_rounded_rect(&mut reference, bw, bh, x, y, w, h, r, c);
            stroke_rounded_rect(&mut fast, bw, bh, x, y, w, h, r, c);
            reference_stroke_rounded_rect(&mut reference, bw, bh, x, y, w, h, r, c);
        }
        assert_eq!(fast, reference);
    }
}
