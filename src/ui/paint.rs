use crate::theme::Color;

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
    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let cx = x as f32 + hw;
    let cy = y as f32 + hh;
    let r = (radius as f32).min(hw).min(hh);
    let inner_hw = hw - r;
    let inner_hh = hh - r;

    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);

    for py in y..y_end {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;

        for px in x..x_end {
            let px_f = (px as f32 + 0.5) - cx;
            let qx = px_f.abs() - inner_hw;

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
    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let cx = x as f32 + hw;
    let cy = y as f32 + hh;
    let r = (radius as f32).min(hw).min(hh);
    let inner_hw = hw - r;
    let inner_hh = hh - r;

    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);

    for py in y..y_end {
        let py_f = (py as f32 + 0.5) - cy;
        let qy = py_f.abs() - inner_hh;
        let row_idx = py * buf_width;

        for px in x..x_end {
            let px_f = (px as f32 + 0.5) - cx;
            let qx = px_f.abs() - inner_hw;

            let ext_x = qx.max(0.0);
            let ext_y = qy.max(0.0);
            let dist = (ext_x * ext_x + ext_y * ext_y).sqrt() + qx.max(qy).min(0.0) - r;

            // Stroke alineado hacia el interior del borde (grosor 1.0px exacto)
            let stroke_dist = (dist + 0.5).abs() - 0.5;
            let alpha = if stroke_dist <= -0.5 {
                border_color.a()
            } else if stroke_dist >= 0.5 {
                0
            } else {
                ((0.5 - stroke_dist) * border_color.a() as f32).round() as u8
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
        }
    }
}
