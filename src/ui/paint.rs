use crate::theme::Color;

/// Dibuja un rectángulo con esquinas redondeadas y antialiasing sobre el framebuffer XRGB
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
    if w == 0 || h == 0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let r = radius.min(w / 2).min(h / 2) as f32;
    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);

    for py in y..y_end {
        let row_idx = py * buf_width;
        let dy = if py < y + radius {
            (y + radius) as f32 - py as f32 - 0.5
        } else if py >= y + h - radius {
            py as f32 - (y + h - radius - 1) as f32 + 0.5
        } else {
            0.0
        };

        for px in x..x_end {
            let dx = if px < x + radius {
                (x + radius) as f32 - px as f32 - 0.5
            } else if px >= x + w - radius {
                px as f32 - (x + w - radius - 1) as f32 + 0.5
            } else {
                0.0
            };

            let alpha = if dx > 0.0 && dy > 0.0 {
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > (r + 0.5) * (r + 0.5) {
                    0u8
                } else if dist_sq <= (r - 0.5) * (r - 0.5) {
                    color.a()
                } else {
                    let dist = dist_sq.sqrt();
                    let coverage = ((r + 0.5 - dist).clamp(0.0, 1.0) * (color.a() as f32)) as u8;
                    coverage
                }
            } else {
                color.a()
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

/// Dibuja el contorno (stroke) de un rectángulo con esquinas redondeadas
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
    if w == 0 || h == 0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let r = radius.min(w / 2).min(h / 2) as f32;
    let x_end = (x + w).min(buf_width);
    let y_end = (y + h).min(buf_height);

    for py in y..y_end {
        let row_idx = py * buf_width;
        let dy = if py < y + radius {
            (y + radius) as f32 - py as f32 - 0.5
        } else if py >= y + h - radius {
            py as f32 - (y + h - radius - 1) as f32 + 0.5
        } else {
            0.0
        };

        let is_border_y = py == y || py == y + h - 1;

        for px in x..x_end {
            let dx = if px < x + radius {
                (x + radius) as f32 - px as f32 - 0.5
            } else if px >= x + w - radius {
                px as f32 - (x + w - radius - 1) as f32 + 0.5
            } else {
                0.0
            };

            let is_border_x = px == x || px == x + w - 1;

            let alpha = if dx > 0.0 && dy > 0.0 {
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > r + 0.5 || dist < r - 1.5 {
                    0u8
                } else if dist >= r - 0.5 && dist <= r + 0.5 {
                    let cov = (1.0 - (dist - r).abs()).clamp(0.0, 1.0);
                    (cov * (border_color.a() as f32)) as u8
                } else {
                    border_color.a()
                }
            } else if is_border_x || is_border_y {
                border_color.a()
            } else {
                0u8
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
