//! Primitivas de dibujo sobre el `Canvas`: texto de terminal, texto de UI (Inter), iconos y barras de scroll.

use super::*;

/// Rectángulo de recorte (x0, y0, x1, y1), extremo superior exclusivo
pub(super) type Clip = (usize, usize, usize, usize);

impl TerminalRenderer {
    /// Rellena un rectángulo de color sólido directamente en el buffer
    pub(crate) fn draw_rect(
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Color,
    ) {
        let (buf_width, buf_height) = canvas.size();
        canvas.fill_rect((0, 0, buf_width, buf_height), x, y, w, h, color);
    }

    /// Ratio de escalado para la tipografía e iconografía de la UI basado en ui_font_size
    #[inline]
    pub(crate) fn ui_scale_ratio(&self) -> f32 {
        let base = 14.0 * self.scale_factor as f32;
        if base > 0.0 {
            (self.ui_font_size / base).clamp(0.5, 3.0)
        } else {
            1.0
        }
    }

    /// Calcula la línea base (`baseline_y`) para centrar verticalmente texto alrededor de `center_y`.
    #[inline]
    pub(crate) fn centered_baseline(&self, center_y: usize) -> usize {
        let cap_h = self
            .inter_font
            .metrics('M', self.ui_font_size.max(6.0))
            .height;
        center_y + cap_h.div_ceil(2)
    }

    /// Fuente proporcional de UI (Inter Medium / SemiBold) con fallback del sistema si falta el glifo.
    fn ui_font_for(&self, c: char, semibold: bool) -> &fontdue::Font {
        if semibold && self.inter_semibold_font.has_glyph(c) {
            &self.inter_semibold_font
        } else if self.inter_font.has_glyph(c) {
            &self.inter_font
        } else if let Some(fb) = self.fallback_font_for(c) {
            fb
        } else if semibold {
            &self.inter_semibold_font
        } else {
            &self.inter_font
        }
    }

    /// Mide el ancho en píxeles de un texto de UI (Inter Medium/SemiBold)
    pub(crate) fn measure_ui_text(&self, text: &str, size_px: f32, semibold: bool) -> usize {
        self.measure_ui_text_tracked(text, size_px, semibold, 0.0)
    }

    /// Mide texto con tracking adicional entre caracteres (micro-etiquetas en mayúsculas).
    pub(crate) fn measure_ui_text_tracked(
        &self,
        text: &str,
        _size_px: f32,
        semibold: bool,
        tracking: f32,
    ) -> usize {
        let effective_size = self.ui_font_size.max(6.0);
        let size_bits = effective_size.to_bits();
        let mut cache = self.ui_advance_cache.borrow_mut();
        let width: f32 = text
            .chars()
            .map(|c| {
                *cache.entry((c, size_bits, semibold)).or_insert_with(|| {
                    self.ui_font_for(c, semibold)
                        .metrics(c, effective_size)
                        .advance_width
                })
            })
            .sum();
        let gaps = text.chars().count().saturating_sub(1) as f32;
        (width + gaps * tracking * self.ui_scale_ratio()).round() as usize
    }

    /// Dibuja texto de UI utilizando la tipografía Inter (Medium como peso base y SemiBold en énfasis)
    pub(crate) fn draw_ui_text(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        size_px: f32,
        semibold: bool,
    ) -> usize {
        self.draw_ui_text_tracked(
            canvas, text, start_x, baseline_y, max_x, color, size_px, semibold, 0.0,
        )
    }

    /// Igual que `draw_ui_text`, con separación extra entre caracteres.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_ui_text_tracked(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        _size_px: f32,
        semibold: bool,
        tracking: f32,
    ) -> usize {
        let (buf_width, buf_height) = canvas.size();
        let effective_size = self.ui_font_size.max(6.0);
        let size_bits = effective_size.to_bits();
        let tracking_px = tracking * self.ui_scale_ratio();
        let mut cur_x = start_x as f32;
        let mut cache = std::mem::take(&mut self.ui_glyph_cache);

        for c in text.chars() {
            if cur_x as usize >= max_x {
                break;
            }

            let key = (c, size_bits, semibold);
            let font = self.ui_font_for(c, semibold);
            let (glyph, advance) = cache.entry(key).or_insert_with(|| {
                let (metrics, bitmap) = font.rasterize(c, effective_size);
                (
                    Rc::new(CachedGlyph {
                        width: metrics.width,
                        height: metrics.height,
                        x_offset: metrics.xmin,
                        y_offset: metrics.ymin,
                        bitmap,
                    }),
                    metrics.advance_width,
                )
            });

            let glyph_x = (cur_x.round() as i32) + glyph.x_offset;
            let glyph_y = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
            canvas.glyph(
                (0, 0, max_x.min(buf_width), buf_height),
                glyph,
                glyph_x,
                glyph_y,
                color,
            );
            cur_x += *advance + tracking_px;
        }

        self.ui_glyph_cache = cache;
        cur_x.round() as usize
    }

    /// Trunca un texto de UI para que al dibujarlo con `font_size` y `semibold` quepa en `max_w` píxeles.
    /// Si no cabe, añade puntos suspensivos ("...") al final.
    pub(crate) fn truncate_ui_text(
        &mut self,
        text: &str,
        max_w: usize,
        font_size: f32,
        semibold: bool,
    ) -> String {
        let total_w = self.measure_ui_text(text, font_size, semibold);
        if total_w <= max_w {
            return text.to_string();
        }
        let ellipsis = "...";
        let ellipsis_w = self.measure_ui_text(ellipsis, font_size, semibold);
        if max_w <= ellipsis_w {
            return String::new();
        }
        let avail_w = max_w.saturating_sub(ellipsis_w);
        let mut cur_w = 0;
        let mut char_count = 0;
        for c in text.chars() {
            let mut buf = [0u8; 4];
            let c_str = c.encode_utf8(&mut buf);
            let cw = self.measure_ui_text(c_str, font_size, semibold);
            if cur_w + cw > avail_w {
                break;
            }
            cur_w += cw;
            char_count += c.len_utf8();
        }
        let mut result = format!("{}{}", &text[..char_count], ellipsis);
        while char_count > 0 && self.measure_ui_text(&result, font_size, semibold) > max_w {
            if let Some(c) = text[..char_count].chars().last() {
                char_count -= c.len_utf8();
                result = format!("{}{}", &text[..char_count], ellipsis);
            } else {
                break;
            }
        }
        result
    }

    /// Dibuja texto de UI recortado con puntos suspensivos ("...") hacia la derecha si excede `max_x`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_ui_text_truncated(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        font_size: f32,
        semibold: bool,
    ) -> usize {
        if x >= max_x {
            return x;
        }
        let max_w = max_x - x;
        let truncated = self.truncate_ui_text(text, max_w, font_size, semibold);
        if !truncated.is_empty() {
            self.draw_ui_text(
                canvas, &truncated, x, baseline_y, max_x, color, font_size, semibold,
            )
        } else {
            x
        }
    }

    /// Mide un valor del chrome con la fuente de interfaz Inter.
    pub(crate) fn measure_mono_text(&self, text: &str, size_px: f32, bold: bool) -> usize {
        self.measure_ui_text(text, size_px, bold)
    }

    /// Dibuja un valor del chrome (ruta, rama, dimensión, atajo o versión) con la fuente de interfaz Inter.
    pub(crate) fn draw_mono_text(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        start_x: usize,
        baseline_y: usize,
        max_x: usize,
        color: Color,
        size_px: f32,
        bold: bool,
    ) -> usize {
        self.draw_ui_text(
            canvas, text, start_x, baseline_y, max_x, color, size_px, bold,
        )
    }

    /// Tecla individual del chrome: caja de 18 px, borde hairline y glifo mono centrado.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_keycap(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        label: &str,
        theme: &Theme,
    ) -> usize {
        let h = tokens::KEYCAP_H;
        let text_w = self.measure_mono_text(label, tokens::MICRO, false);
        let w = (text_w + 12).max(20);
        canvas.rounded_fill(
            x,
            y,
            w,
            h,
            3,
            theme.mix(theme.background, theme.foreground, 0.07),
        );
        canvas.rounded_stroke(x, y, w, h, 3, theme.chrome_line());
        let baseline = self.centered_baseline(y + h / 2);
        let tx = x + w.saturating_sub(text_w) / 2;
        self.draw_mono_text(
            canvas,
            label,
            tx,
            baseline,
            x + w,
            theme.text_secondary(),
            tokens::MICRO,
            false,
        );
        w
    }

    /// Dibuja un icono Lucide vectorial perfectamente centrado en una caja
    pub(crate) fn draw_lucide_icon(
        &mut self,
        canvas: &mut Canvas,
        icon: crate::ui::LucideIcon,
        center_x: usize,
        center_y: usize,
        size_px: f32,
        color: Color,
    ) {
        let (buf_width, buf_height) = canvas.size();
        let effective_size = if size_px <= 9.5 {
            (10.5 * self.ui_scale_ratio()).round().max(6.0)
        } else {
            (self.ui_font_size * 1.15).round().max(6.0)
        };
        let c = icon.as_char();
        let size_bits = effective_size.to_bits();
        let lucide_ref = &self.lucide_font;

        let glyph = self
            .icon_glyph_cache
            .entry((c, size_bits))
            .or_insert_with(|| {
                let (metrics, bitmap) = lucide_ref.rasterize(c, effective_size);
                Rc::new(CachedGlyph {
                    width: metrics.width,
                    height: metrics.height,
                    x_offset: metrics.xmin,
                    y_offset: metrics.ymin,
                    bitmap,
                })
            });

        let glyph_x = (center_x as i32) - (glyph.width as i32 / 2);
        let glyph_y = (center_y as i32) - (glyph.height as i32 / 2);
        canvas.glyph(
            (0, 0, buf_width, buf_height),
            glyph,
            glyph_x,
            glyph_y,
            color,
        );
    }

    /// Dibuja una barra de desplazamiento vertical unificada con pista translúcida y pulgar redondeado
    pub(crate) fn draw_scrollbar(
        canvas: &mut Canvas,
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
            canvas,
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
    pub(crate) fn draw_scrollbar_rects(
        canvas: &mut Canvas,
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
        canvas.rounded_fill(track.0, track.1, track.2, track.3, 3, track_bg);
        let thumb_color = if is_hovered {
            theme.accent_color()
        } else {
            theme.ansi[8]
        };
        canvas.rounded_fill(thumb.0, thumb.1, thumb.2, thumb.3, 3, thumb_color);
    }

    /// Dibuja el contenido de un `TextInput` (texto, placeholder, selección de fondo y cursor parpadeante).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_text_input_content(
        &mut self,
        canvas: &mut Canvas,
        input: &crate::ui::TextInput,
        text_start_x: usize,
        text_y: usize,
        text_max_x: usize,
        box_y: usize,
        box_h: usize,
        cursor_visible: bool,
        placeholder: &str,
        is_active: bool,
        theme: &Theme,
    ) {
        let text = input.text();
        if text.is_empty() && !is_active {
            self.draw_ui_text(
                canvas,
                placeholder,
                text_start_x,
                text_y,
                text_max_x,
                theme.text_muted(),
                tokens::BODY,
                false,
            );
            return;
        }

        let cur_h = 16.min(box_h.saturating_sub(4)).max(12);
        let cur_y = box_y + (box_h.saturating_sub(cur_h)) / 2;
        let cursor_color = if theme.cursor != theme.background {
            theme.cursor
        } else {
            theme.accent_color()
        };

        if text.is_empty() && is_active {
            if cursor_visible {
                canvas.rounded_fill(
                    text_start_x,
                    cur_y,
                    2,
                    cur_h,
                    1,
                    cursor_color,
                );
            }
            self.draw_ui_text(
                canvas,
                placeholder,
                text_start_x + 8,
                text_y,
                text_max_x,
                theme.mix(theme.foreground, theme.background, 0.42),
                tokens::BODY,
                false,
            );
            return;
        }

        // Selección activa: color nítido y alto contraste
        let selection = input.selection();
        let (sel_bg, sel_fg) = if theme.selection != theme.background {
            (theme.selection, theme.selection_fg)
        } else {
            (theme.mix(theme.background, theme.accent_color(), 0.55), theme.foreground)
        };

        if let Some((start, end)) = selection {
            let x_start = text_start_x + self.measure_ui_text(&text[..start], tokens::BODY, true);
            let x_end = text_start_x + self.measure_ui_text(&text[..end], tokens::BODY, true);
            let sel_x = x_start.min(text_max_x);
            let sel_w = (x_end.saturating_sub(x_start)).min(text_max_x.saturating_sub(sel_x));
            if sel_w > 0 {
                let sel_h = 18.min(box_h.saturating_sub(2)).max(14);
                let sel_y = box_y + (box_h.saturating_sub(sel_h)) / 2;
                canvas.rounded_fill(
                    sel_x,
                    sel_y,
                    sel_w,
                    sel_h,
                    3,
                    sel_bg,
                );
            }

            // Dibujar los tramos de texto: antes, seleccionado (en sel_fg) y después
            let prefix = &text[..start];
            let selected = &text[start..end];
            let suffix = &text[end..];

            if !prefix.is_empty() {
                self.draw_ui_text(
                    canvas,
                    prefix,
                    text_start_x,
                    text_y,
                    x_start.min(text_max_x),
                    theme.foreground,
                    tokens::BODY,
                    true,
                );
            }
            if !selected.is_empty() {
                self.draw_ui_text(
                    canvas,
                    selected,
                    x_start,
                    text_y,
                    x_end.min(text_max_x),
                    sel_fg,
                    tokens::BODY,
                    true,
                );
            }
            if !suffix.is_empty() {
                self.draw_ui_text(
                    canvas,
                    suffix,
                    x_end,
                    text_y,
                    text_max_x,
                    theme.foreground,
                    tokens::BODY,
                    true,
                );
            }
        } else {
            self.draw_ui_text(
                canvas,
                text,
                text_start_x,
                text_y,
                text_max_x,
                theme.foreground,
                tokens::BODY,
                true,
            );
        }

        if is_active && cursor_visible {
            let cur_offset = self.measure_ui_text(&text[..input.cursor], tokens::BODY, true);
            let cur_x = (text_start_x + cur_offset).min(text_max_x);
            canvas.rounded_fill(
                cur_x,
                cur_y,
                2,
                cur_h,
                1,
                cursor_color,
            );
        }
    }
}
