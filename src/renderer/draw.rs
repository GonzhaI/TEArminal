//! Primitivas de dibujo sobre el framebuffer: rellenos, texto de terminal, texto de UI (Inter) e iconos.

use super::*;

/// Rectángulo de recorte (x0, y0, x1, y1), extremo superior exclusivo
pub(super) type Clip = (usize, usize, usize, usize);

/// Rellena un rectángulo recortado con un color opaco usando `slice::fill` por fila
pub(super) fn fill_rect(
    buffer: &mut [u32],
    buf_w: usize,
    clip: Clip,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    let (x0, x1) = (x.max(clip.0), (x + w).min(clip.2));
    let (y0, y1) = (y.max(clip.1), (y + h).min(clip.3));
    if x0 >= x1 {
        return;
    }
    for py in y0..y1 {
        buffer[py * buf_w + x0..py * buf_w + x1].fill(color);
    }
}

/// Mezcla un rectángulo con color y alpha sobre el framebuffer existente
#[allow(dead_code)]
pub(super) fn blend_rect(
    buffer: &mut [u32],
    buf_w: usize,
    clip: Clip,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: Color,
) {
    if color.a == 0 {
        return;
    }
    if color.a == 255 {
        fill_rect(buffer, buf_w, clip, x, y, w, h, color.to_u32_xrgb());
        return;
    }
    let (x0, x1) = (x.max(clip.0), (x + w).min(clip.2));
    let (y0, y1) = (y.max(clip.1), (y + h).min(clip.3));
    if x0 >= x1 {
        return;
    }
    let a = color.a as u32;
    let inv_a = 255 - a;
    let (cr, cg, cb) = (color.r as u32 * a, color.g as u32 * a, color.b as u32 * a);

    for py in y0..y1 {
        let row_start = py * buf_w + x0;
        let row_end = py * buf_w + x1;
        for px in &mut buffer[row_start..row_end] {
            let orig = *px;
            let r = (cr + ((orig >> 16) & 0xFF) * inv_a) / 255;
            let g = (cg + ((orig >> 8) & 0xFF) * inv_a) / 255;
            let b = (cb + (orig & 0xFF) * inv_a) / 255;
            *px = (r << 16) | (g << 8) | b;
        }
    }
}

/// Mezcla un glifo (mapa de cobertura 8-bit) sobre el framebuffer recortado. Único punto de
/// composición de texto: terminal, UI Inter e iconos Lucide pasan por aquí.
pub(super) fn blit_glyph(
    buffer: &mut [u32],
    buf_w: usize,
    clip: Clip,
    glyph: &CachedGlyph,
    gx: i32,
    gy: i32,
    fg: Color,
) {
    let x0 = gx.max(clip.0 as i32);
    let x1 = (gx + glyph.width as i32).min(clip.2 as i32);
    let y0 = gy.max(clip.1 as i32);
    let y1 = (gy + glyph.height as i32).min(clip.3 as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = (x1 - x0) as usize;
    let fg_u32 = fg.to_u32_xrgb();
    let (fr, fgc, fb) = (fg.r as u32, fg.g as u32, fg.b as u32);
    for py in y0..y1 {
        let src_start = (py - gy) as usize * glyph.width + (x0 - gx) as usize;
        let dst_start = py as usize * buf_w + x0 as usize;
        let src = &glyph.bitmap[src_start..src_start + n];
        let dst = &mut buffer[dst_start..dst_start + n];
        for (d, &alpha) in dst.iter_mut().zip(src) {
            match alpha {
                0 => {}
                255 => *d = fg_u32,
                _ => {
                    let a = alpha as u32;
                    let inv = 255 - a;
                    let r = (fr * a + ((*d >> 16) & 0xFF) * inv) / 255;
                    let g = (fgc * a + ((*d >> 8) & 0xFF) * inv) / 255;
                    let b = (fb * a + (*d & 0xFF) * inv) / 255;
                    *d = (r << 16) | (g << 8) | b;
                }
            }
        }
    }
}

impl TerminalRenderer {
    /// Rellena un rectángulo de color sólido directamente en el buffer
    pub fn draw_rect(
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Color,
    ) {
        fill_rect(
            buffer,
            buf_width,
            (0, 0, buf_width, buf_height),
            x,
            y,
            w,
            h,
            color.to_u32_xrgb(),
        );
    }

    /// Ratio de escalado para la tipografía e iconografía de la UI basado en ui_font_size
    #[inline]
    pub fn ui_scale_ratio(&self) -> f32 {
        let base = 13.0 * self.scale_factor as f32;
        if base > 0.0 {
            (self.ui_font_size / base).clamp(0.6, 2.5)
        } else {
            1.0
        }
    }

    /// Mide el ancho en píxeles de un texto renderizado con Inter
    pub fn measure_ui_text(&self, text: &str, size_px: f32, semibold: bool) -> usize {
        let font = if semibold {
            &self.inter_semibold_font
        } else {
            &self.inter_font
        };
        let effective_size = (size_px * self.ui_scale_ratio()).max(6.0);
        let size_bits = effective_size.to_bits();
        let mut cache = self.ui_advance_cache.borrow_mut();
        let width: f32 = text
            .chars()
            .map(|c| {
                *cache
                    .entry((c, size_bits, semibold))
                    .or_insert_with(|| font.metrics(c, effective_size).advance_width)
            })
            .sum();
        width.round() as usize
    }

    /// Dibuja texto proporcional moderno utilizando la tipografía Inter
    pub fn draw_ui_text(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        size_px: f32,
        semibold: bool,
    ) -> usize {
        let effective_size = (size_px * self.ui_scale_ratio()).max(6.0);
        let size_bits = effective_size.to_bits();
        let mut cur_x = start_x as f32;

        for c in text.chars() {
            if cur_x as usize >= max_x {
                break;
            }

            let key = (c, size_bits, semibold);
            if !self.ui_glyph_cache.contains_key(&key) {
                let font = if semibold && self.inter_semibold_font.has_glyph(c) {
                    &self.inter_semibold_font
                } else if self.inter_font.has_glyph(c) {
                    &self.inter_font
                } else {
                    self.fallback_font()
                        .filter(|f| f.has_glyph(c))
                        .unwrap_or(&self.inter_font)
                };
                let (metrics, bitmap) = font.rasterize(c, effective_size);
                let glyph = CachedGlyph {
                    width: metrics.width,
                    height: metrics.height,
                    x_offset: metrics.xmin,
                    y_offset: metrics.ymin,
                    bitmap,
                };
                self.ui_glyph_cache
                    .insert(key, (glyph, metrics.advance_width));
            }
            let (glyph, advance) = &self.ui_glyph_cache[&key];

            let glyph_x = (cur_x.round() as i32) + glyph.x_offset;
            let glyph_y = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
            blit_glyph(
                buffer,
                buf_width,
                (0, 0, max_x.min(buf_width), buf_height),
                glyph,
                glyph_x,
                glyph_y,
                color,
            );
            cur_x += *advance;
        }

        cur_x.round() as usize
    }

    /// Dibuja un icono Lucide vectorial perfectamente centrado en una caja
    pub fn draw_lucide_icon(
        &mut self,
        buffer: &mut [u32],
        buf_width: usize,
        buf_height: usize,
        icon: crate::ui::LucideIcon,
        center_x: usize,
        center_y: usize,
        size_px: f32,
        color: Color,
    ) {
        let effective_size = (size_px * self.ui_scale_ratio()).max(6.0);
        let c = icon.as_char();
        let size_bits = effective_size.to_bits();
        let lucide_ref = &self.lucide_font;

        let glyph = self
            .icon_glyph_cache
            .entry((c, size_bits))
            .or_insert_with(|| {
                let (metrics, bitmap) = lucide_ref.rasterize(c, effective_size);
                CachedGlyph {
                    width: metrics.width,
                    height: metrics.height,
                    x_offset: metrics.xmin,
                    y_offset: metrics.ymin,
                    bitmap,
                }
            });

        let glyph_x = (center_x as i32) - (glyph.width as i32 / 2);
        let glyph_y = (center_y as i32) - (glyph.height as i32 / 2);
        blit_glyph(
            buffer,
            buf_width,
            (0, 0, buf_width, buf_height),
            glyph,
            glyph_x,
            glyph_y,
            color,
        );
    }

    /// Dibuja una barra de desplazamiento vertical unificada con pista translúcida y pulgar redondeado
    pub fn draw_scrollbar(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        track_x: usize,
        track_y: usize,
        track_w: usize,
        track_h: usize,
        thumb_y: usize,
        thumb_h: usize,
        is_hovered: bool,
        awake: bool,
        theme: &Theme,
    ) {
        Self::draw_scrollbar_rects(
            buffer,
            width,
            height,
            (track_x, track_y, track_w, track_h),
            (track_x, thumb_y, track_w, thumb_h),
            is_hovered,
            awake,
            theme,
        );
    }

    /// Barra de desplazamiento genérica (vertical u horizontal) a partir de pista y pulgar.
    /// Solo se dibuja con el ratón encima (`is_hovered`) o mientras se hace scroll (`awake`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scrollbar_rects(
        buffer: &mut [u32],
        width: usize,
        height: usize,
        track: Rect,
        thumb: Rect,
        is_hovered: bool,
        awake: bool,
        theme: &Theme,
    ) {
        if track.2 == 0 || track.3 == 0 || thumb.2 == 0 || thumb.3 == 0 || !(is_hovered || awake) {
            return;
        }
        let hover = theme.surface_hover();
        let track_bg = Color::from_rgba(hover.r(), hover.g(), hover.b(), 100);
        crate::ui::draw_rounded_rect(
            buffer, width, height, track.0, track.1, track.2, track.3, 3, track_bg,
        );
        let thumb_color = if is_hovered {
            theme.accent_color()
        } else {
            theme.ansi[8]
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            thumb.0,
            thumb.1,
            thumb.2,
            thumb.3,
            3,
            thumb_color,
        );
    }
}
