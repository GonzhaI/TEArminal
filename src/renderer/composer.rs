//! Compositor: marco de acento alrededor de la entrada del prompt, con chips de contexto, atajos y «enviar».
//!
//! Es decoración sobre la rejilla: las teclas siguen yendo al shell (PSReadLine) y no se reescribe ni se
//! interpreta nada. Solo aparece cuando el panel enfocado está en el prompt (OSC 133 entre «prompt listo» y
//! «comando ejecutándose»), fuera de la pantalla alterna y sin desplazamiento. La fila de chips usa la fila
//! vacía bajo la entrada: si el shell dibuja algo ahí (menú de completado, predicciones) no se pinta.

use super::blocks::row_has_content;
use super::*;
use crate::ui::LucideIcon;

const FRAME_INSET_X: usize = 8;
const FRAME_RADIUS: usize = 8;
const CHIP_H: usize = 16;
const SEND_D: usize = 18;
/// Ancho mínimo del panel para dibujar el compositor.
const MIN_WIDTH: usize = 360;
/// Ancho mínimo de la fila de chips para mostrar además los atajos.
const HINTS_MIN_WIDTH: usize = 700;

/// Rectángulos del compositor ya situados en el panel (geometría única para dibujo y clics).
#[derive(Clone, Copy, Debug)]
pub struct ComposerLayout {
    pub frame: (usize, usize, usize, usize),
    /// Fila inferior del marco (chips, atajos y botón). `None` si no hay una fila libre bajo la entrada.
    pub chips_row: Option<(usize, usize, usize, usize)>,
    pub send_btn: Option<(usize, usize, usize, usize)>,
}

impl ComposerLayout {
    pub fn send_at(&self, mx: usize, my: usize) -> bool {
        self.send_btn
            .is_some_and(|(x, y, w, h)| mx >= x && mx < x + w && my >= y && my < y + h)
    }
}

/// Datos de contexto que muestran los chips.
#[derive(Clone, Copy, Default)]
pub struct ComposerChips<'a> {
    pub branch: Option<&'a str>,
    /// Icono, etiqueta y color del agente activo (misma etiqueta que la barra de estado).
    pub agent: Option<(LucideIcon, &'a str, Color)>,
}

impl TerminalRenderer {
    /// Compositor del panel, o `None` si el panel no está en el prompt.
    pub fn composer_layout(
        &self,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
    ) -> Option<ComposerLayout> {
        if terminal.is_alt_screen || terminal.scroll_offset != 0 || rect_w < MIN_WIDTH {
            return None;
        }
        let block = terminal
            .active_block
            .as_ref()
            .filter(|b| b.output_start.is_none())?;
        let view_start = terminal.visible_line_start();
        let visible = terminal.visible_rows().len();
        let view_end = view_start + visible;
        let cursor_line = terminal.current_global_line();
        if visible == 0 || cursor_line < view_start || cursor_line >= view_end {
            return None;
        }
        let first_line = block.prompt_line.clamp(view_start, cursor_line);
        let base_y = rect_y + self.margin_y;
        let rh = self.row_height;
        let top = base_y + (first_line - view_start) * rh + 1;
        let mut bottom = base_y + (cursor_line - view_start + 1) * rh;

        let below = cursor_line + 1;
        let has_room = below < view_end
            && base_y + (below - view_start + 1) * rh <= rect_y + rect_h
            && !row_has_content(terminal.get_buffer_line(below));
        let x = (rect_x + self.margin_x).saturating_sub(FRAME_INSET_X);
        let right = (rect_x + rect_w).saturating_sub(8);
        if right <= x + 80 {
            return None;
        }
        let w = right - x;
        let (chips_row, send_btn) = if has_room {
            let row_y = bottom;
            bottom += rh;
            let btn_x = right.saturating_sub(10 + SEND_D);
            let btn_y = row_y + (rh.saturating_sub(SEND_D)) / 2;
            (
                Some((x, row_y, w, rh)),
                Some((btn_x, btn_y, SEND_D, SEND_D)),
            )
        } else {
            (None, None)
        };
        let bottom = bottom.saturating_sub(1).min(rect_y + rect_h);
        if bottom <= top + 4 {
            return None;
        }
        Some(ComposerLayout {
            frame: (x, top, w, bottom - top),
            chips_row,
            send_btn,
        })
    }

    pub(super) fn draw_composer(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
        chips: ComposerChips<'_>,
        theme: &Theme,
    ) {
        let Some(layout) = self.composer_layout(rect_x, rect_y, rect_w, rect_h, terminal) else {
            return;
        };
        let (fx, fy, fw, fh) = layout.frame;
        let accent = theme.mix(theme.background, theme.accent_color(), 0.75);
        crate::ui::stroke_rounded_rect(buffer, width, height, fx, fy, fw, fh, FRAME_RADIUS, accent);
        let (Some((rx, ry, rw, rh)), Some(send)) = (layout.chips_row, layout.send_btn) else {
            return;
        };

        let cy = ry + rh / 2;
        let baseline = cy + (tokens::DETAIL * self.ui_scale_ratio() * 0.36) as usize;
        let right = send.0.saturating_sub(10);
        let mut x = rx + 12;

        // Chips de contexto: carpeta, rama y agente.
        let folder = terminal
            .current_dir
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        if let Some(name) = folder.filter(|n| !n.is_empty()) {
            x = self.draw_composer_chip(
                buffer,
                width,
                height,
                x,
                cy,
                right,
                LucideIcon::Folder,
                &name,
                theme.text_secondary(),
                theme.chrome_bg_elevated(),
                theme,
            );
        }
        if let Some(branch) = chips.branch.filter(|b| !b.is_empty()) {
            x = self.draw_composer_chip(
                buffer,
                width,
                height,
                x + 6,
                cy,
                right,
                LucideIcon::GitBranch,
                branch,
                theme.text_secondary(),
                theme.chrome_bg_elevated(),
                theme,
            );
        }
        if let Some((icon, label, color)) = chips.agent {
            x = self.draw_composer_chip(
                buffer,
                width,
                height,
                x + 6,
                cy,
                right,
                icon,
                label,
                color,
                theme.accent_soft(40),
                theme,
            );
        }

        // Atajos (key-caps + etiqueta), de derecha a izquierda; solo si caben tras los chips.
        if rw >= HINTS_MIN_WIDTH {
            let hints: [(&[&str], &str); 3] = [
                (&["Alt", "Enter"], "línea nueva"),
                (&["Ctrl", "R"], "historial"),
                (&["Tab"], "completar"),
            ];
            let mut hx = right;
            for (keys, label) in hints {
                let label_w = self.measure_ui_text(label, tokens::DETAIL, false);
                let keys_w: usize = keys
                    .iter()
                    .map(|k| self.measure_mono_text(k, tokens::DETAIL, false) + 10 + 3)
                    .sum();
                let total = keys_w + 5 + label_w;
                if hx < x + 24 + total {
                    break;
                }
                let start = hx - total;
                let mut kx = start;
                for key in keys {
                    let w = self.measure_mono_text(key, tokens::DETAIL, false) + 10;
                    crate::ui::draw_rounded_rect(
                        buffer,
                        width,
                        height,
                        kx,
                        cy - 7,
                        w,
                        14,
                        4,
                        theme.chrome_bg_elevated(),
                    );
                    self.draw_mono_text(
                        buffer,
                        width,
                        height,
                        key,
                        kx + 5,
                        baseline,
                        kx + w,
                        theme.text_muted(),
                        tokens::DETAIL,
                        false,
                    );
                    kx += w + 3;
                }
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    label,
                    kx + 2,
                    baseline,
                    hx,
                    theme.text_muted(),
                    tokens::DETAIL,
                    false,
                );
                hx = start.saturating_sub(16);
            }
        }

        // Botón enviar.
        let (sx, sy, sw, sh) = send;
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            sx,
            sy,
            sw,
            sh,
            sw / 2,
            theme.accent_color(),
        );
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            LucideIcon::ArrowUp,
            sx + sw / 2,
            sy + sh / 2,
            11.0,
            theme.background,
        );
    }

    /// Chip de contexto (icono + texto) que empieza en `x`; devuelve el `x` donde acaba.
    fn draw_composer_chip(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        cy: usize,
        max_x: usize,
        icon: LucideIcon,
        text: &str,
        fg: Color,
        bg: Color,
        _theme: &Theme,
    ) -> usize {
        let text_w = self.measure_mono_text(text, tokens::DETAIL, false);
        let w = 8 + 12 + 5 + text_w + 8;
        if x + w > max_x {
            return x;
        }
        crate::ui::draw_rounded_rect(buffer, width, height, x, cy - CHIP_H / 2, w, CHIP_H, 5, bg);
        self.draw_lucide_icon(buffer, width, height, icon, x + 8 + 6, cy, 11.0, fg);
        let baseline = cy + (tokens::DETAIL * self.ui_scale_ratio() * 0.36) as usize;
        self.draw_mono_text(
            buffer,
            width,
            height,
            text,
            x + 8 + 12 + 5,
            baseline,
            x + w,
            fg,
            tokens::DETAIL,
            false,
        );
        x + w
    }
}
