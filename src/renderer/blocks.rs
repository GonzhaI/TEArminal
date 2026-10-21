//! Tarjetas de bloque de comando (OSC 133): marco, cabecera con estado y acciones, y franja de contexto.
//!
//! Las tarjetas se dibujan *sobre la rejilla existente*: no reflotan el contenido ni cambian la
//! correspondencia entre píxeles y filas, así que los clics, el scroll y la selección siguen intactos.
//! La fila del prompt se oculta (la sustituye la cabecera) y las filas de salida se pintan dentro del
//! marco como siempre.

use super::*;
use crate::terminal::CommandBlock;
use crate::ui::LucideIcon;

const CARD_INSET_X: usize = 8;
const CARD_INSET_TOP: usize = 3;
const CARD_INSET_BOTTOM: usize = 1;
const CARD_RADIUS: usize = 8;
const BUTTON_W: usize = 26;
const BUTTON_H: usize = 22;
const BUTTON_GAP: usize = 4;
const EXPLAIN_W: usize = 96;
const IGNORE_W: usize = 74;

/// Rectángulos de una tarjeta de bloque ya situada en el panel (geometría única para dibujo y clics).
#[derive(Clone, Copy, Debug)]
pub struct BlockCardLayout {
    /// Índice real en `TerminalState::command_blocks`.
    pub block_index: usize,
    /// Fila global (de la rejilla) del prompt; la cabecera se dibuja encima.
    pub header_line: usize,
    pub header_visible: bool,
    pub failed: bool,
    pub card: (usize, usize, usize, usize),
    /// Banda de la cabecera (icono, comando, metadatos y botones) para los clics.
    pub header: (usize, usize, usize, usize),
    pub copy_btn: Option<(usize, usize, usize, usize)>,
    pub repeat_btn: Option<(usize, usize, usize, usize)>,
    pub explain_btn: Option<(usize, usize, usize, usize)>,
    pub ignore_btn: Option<(usize, usize, usize, usize)>,
}

impl BlockCardLayout {
    pub fn button_at(&self, mx: usize, my: usize) -> Option<BlockCardButton> {
        let inside = |r: (usize, usize, usize, usize)| {
            mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
        };
        if self.copy_btn.is_some_and(inside) {
            Some(BlockCardButton::Copy)
        } else if self.repeat_btn.is_some_and(inside) {
            Some(BlockCardButton::Repeat)
        } else if self.explain_btn.is_some_and(inside) {
            Some(BlockCardButton::Explain)
        } else if self.ignore_btn.is_some_and(inside) {
            Some(BlockCardButton::Ignore)
        } else {
            None
        }
    }

    /// Zona de la cabecera (icono, comando, metadatos y botones) para no iniciar una selección.
    pub fn header_at(&self, mx: usize, my: usize) -> bool {
        let (x, y, w, h) = self.header;
        self.header_visible && mx >= x && mx < x + w && my >= y && my < y + h
    }
}

/// Acción de una tarjeta de bloque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCardButton {
    Copy,
    Repeat,
    Explain,
    Ignore,
}

/// Duración legible: `22,9 s`, `1 min 06 s`, `0,1 s`.
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs_f32();
    if secs < 60.0 {
        format!("{:.1} s", secs).replace('.', ",")
    } else {
        let total = duration.as_secs();
        format!("{} min {:02} s", total / 60, total % 60)
    }
}

/// Fila con contenido visible (texto, fondo o decoración). Las filas vacías del final de un bloque
/// (el salto de línea anterior al OSC 133;D) no agrandan la tarjeta.
pub(super) fn row_has_content(row: Option<&[Cell]>) -> bool {
    row.is_some_and(|row| {
        row.iter().any(|c| {
            c.c != ' '
                || c.bg != Color::TRANSPARENT
                || (c.flags
                    & (Cell::FLAG_UNDERLINE | Cell::FLAG_STRIKETHROUGH | Cell::FLAG_INVERSE))
                    != 0
        })
    })
}

impl TerminalRenderer {
    /// Tarjetas visibles del panel, con la misma geometría que usa el dibujo.
    pub fn block_cards(
        &self,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
    ) -> Vec<BlockCardLayout> {
        if terminal.is_alt_screen || rect_w == 0 || rect_h == 0 {
            return Vec::new();
        }
        let view_start = terminal.visible_line_start();
        let visible = terminal.visible_rows().len();
        if visible == 0 {
            return Vec::new();
        }
        let view_end = view_start + visible;
        let base_x = rect_x + self.margin_x;
        let base_y = rect_y + self.margin_y;
        let max_x = rect_x + rect_w;
        let max_y = rect_y + rect_h;
        let mut cards = Vec::new();
        let mut entries: Vec<(usize, &CommandBlock)> =
            terminal.command_blocks.iter().enumerate().collect();
        if let Some(active) = terminal
            .active_block
            .as_ref()
            .filter(|b| b.output_start.is_some())
        {
            entries.push((terminal.command_blocks.len(), active));
        }

        for (position, (block_index, block)) in entries.iter().copied().enumerate() {
            let Some(output_start) = block.output_start else {
                continue;
            };
            let start = block.prompt_line;
            let next_prompt = entries
                .get(position + 1)
                .map(|(_, next)| next.prompt_line)
                .unwrap_or(usize::MAX);
            let mut end = block
                .output_end
                .unwrap_or_else(|| terminal.current_global_line())
                .max(output_start)
                .min(next_prompt.saturating_sub(1));
            // El salto de línea previo a OSC 133;D deja filas vacías al final del bloque.
            while end > output_start && !row_has_content(terminal.get_buffer_line(end)) {
                end -= 1;
            }
            if end < output_start || end < view_start || start >= view_end {
                continue;
            }

            let header_visible = start >= view_start && start < view_end;
            let header_row_y = base_y + start.saturating_sub(view_start) * self.row_height;
            let last_visible = end.min(view_end.saturating_sub(1));
            let bottom_y = base_y + (last_visible.saturating_sub(view_start) + 1) * self.row_height;

            let card_x = base_x.saturating_sub(CARD_INSET_X);
            let card_right = max_x.saturating_sub(8);
            if card_right <= card_x + 80 {
                continue;
            }
            let top = if header_visible {
                (header_row_y + CARD_INSET_TOP).max(rect_y)
            } else {
                rect_y
            };
            let bottom = (bottom_y.saturating_sub(CARD_INSET_BOTTOM)).min(max_y);
            if bottom <= top + self.row_height / 2 {
                continue;
            }
            let card = (card_x, top, card_right - card_x, bottom - top);

            let complete = block.output_end.is_some();
            let btn_y = header_row_y + (self.row_height.saturating_sub(BUTTON_H)) / 2;
            let repeat_x = card_right.saturating_sub(10 + BUTTON_W);
            let copy_x = repeat_x.saturating_sub(BUTTON_GAP + BUTTON_W);
            let (copy_btn, repeat_btn) = if header_visible && complete {
                (
                    Some((copy_x, btn_y, BUTTON_W, BUTTON_H)),
                    Some((repeat_x, btn_y, BUTTON_W, BUTTON_H)),
                )
            } else {
                (None, None)
            };

            // «Explicar» e «Ignorar» van en la fila de la cabecera, a la izquierda de copiar/repetir:
            // la salida ocupa filas de la rejilla y un banner propio se pisaría con ella.
            let failed = block.exit_code.is_some_and(|code| code != 0);
            let (explain_btn, ignore_btn) =
                if failed && !block.error_dismissed && header_visible && complete && card.2 > 560 {
                    let ignore_x = copy_x.saturating_sub(10 + IGNORE_W);
                    let explain_x = ignore_x.saturating_sub(6 + EXPLAIN_W);
                    (
                        Some((explain_x, btn_y, EXPLAIN_W, BUTTON_H)),
                        Some((ignore_x, btn_y, IGNORE_W, BUTTON_H)),
                    )
                } else {
                    (None, None)
                };

            cards.push(BlockCardLayout {
                block_index,
                header_line: start,
                header_visible,
                failed,
                card,
                header: (card.0, header_row_y, card.2, self.row_height),
                copy_btn,
                repeat_btn,
                explain_btn,
                ignore_btn,
            });
        }
        cards
    }

    /// Filas de prompt que la cabecera de la tarjeta sustituye (no se dibuja su texto).
    pub(super) fn block_header_lines(
        &self,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
    ) -> Vec<usize> {
        self.block_cards(rect_x, rect_y, rect_w, rect_h, terminal)
            .into_iter()
            .filter(|c| c.header_visible)
            .map(|c| c.header_line)
            .collect()
    }

    /// Fondos y bordes de las tarjetas. Va antes de dibujar las celdas para que la salida quede encima.
    pub(super) fn draw_block_card_frames(
        &self,
        canvas: &mut Canvas,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
        theme: &Theme,
    ) {
        for card in self.block_cards(rect_x, rect_y, rect_w, rect_h, terminal) {
            let (x, y, w, h) = card.card;
            let bg = if card.failed {
                theme.mix(theme.surface_bg(), theme.ansi[1], 0.06)
            } else {
                theme.surface_bg()
            };
            canvas.rounded_fill(x, y, w, h, CARD_RADIUS, bg);
            let border = if card.failed {
                theme.mix(theme.background, theme.ansi[1], 0.55)
            } else {
                theme.chrome_line()
            };
            canvas.rounded_stroke(x, y, w, h, CARD_RADIUS, border);
        }
    }

    /// Cabeceras, metadatos y acciones de las tarjetas. Va después de las celdas.
    pub(super) fn draw_block_card_chrome(
        &mut self,
        canvas: &mut Canvas,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        terminal: &TerminalState,
        theme: &Theme,
    ) {
        let cards = self.block_cards(rect_x, rect_y, rect_w, rect_h, terminal);
        for card in cards {
            let Some(block) = block_at(terminal, card.block_index) else {
                continue;
            };
            if !card.header_visible {
                continue;
            }
            let header_y = rect_y
                + self.margin_y
                + card
                    .header_line
                    .saturating_sub(terminal.visible_line_start())
                    * self.row_height;
            let cy = header_y + self.row_height / 2;
            let baseline = cy + (tokens::TITLE * self.ui_scale_ratio() * 0.36) as usize;

            // Estado: ✓ completado, ✗ con error, punto de acento mientras corre.
            let status = match block.exit_code {
                Some(0) => theme.ansi[2],
                Some(_) => theme.ansi[1],
                None => theme.accent_color(),
            };
            let icon_x = card.card.0 + 20;
            canvas.rounded_fill(icon_x, cy - 7, 14, 14, 7, status);
            match block.exit_code {
                Some(0) => self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Check,
                    icon_x + 7,
                    cy,
                    9.0,
                    theme.background,
                ),
                Some(_) => self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::X,
                    icon_x + 7,
                    cy,
                    9.0,
                    theme.background,
                ),
                None => {
                    canvas.rounded_fill(icon_x + 4, cy - 3, 6, 6, 3, theme.background);
                }
            }

            let mut right = card.card.0 + card.card.2 - 12;
            // Botones copiar / repetir (solo bloques terminados).
            if let Some(btn) = card.repeat_btn {
                self.draw_block_action(canvas, btn, LucideIcon::RefreshCw, theme);
            }
            if let Some(btn) = card.copy_btn {
                self.draw_block_action(canvas, btn, LucideIcon::Copy, theme);
            }
            if let Some(copy) = card.copy_btn {
                right = copy.0.saturating_sub(10);
            }
            // Tarjeta de error: [✧ Explicar] [Ignorar] en la misma fila.
            if let (Some(explain), Some(ignore)) = (card.explain_btn, card.ignore_btn) {
                canvas.rounded_fill(
                    explain.0,
                    explain.1,
                    explain.2,
                    explain.3,
                    5,
                    theme.accent_soft(56),
                );
                self.draw_lucide_icon(
                    canvas,
                    LucideIcon::Sparkles,
                    explain.0 + 14,
                    explain.1 + explain.3 / 2,
                    11.0,
                    theme.accent_color(),
                );
                self.draw_ui_text(
                    canvas,
                    "Explicar",
                    explain.0 + 26,
                    baseline,
                    explain.0 + explain.2 - 6,
                    theme.accent_color(),
                    tokens::BODY,
                    true,
                );
                canvas.rounded_fill(
                    ignore.0,
                    ignore.1,
                    ignore.2,
                    ignore.3,
                    5,
                    theme.chrome_bg_elevated(),
                );
                self.draw_ui_text(
                    canvas,
                    "Ignorar",
                    ignore.0 + 12,
                    baseline,
                    ignore.0 + ignore.2 - 6,
                    theme.text_secondary(),
                    tokens::BODY,
                    false,
                );
                right = explain.0.saturating_sub(12);
            }

            // Duración y, si falló, el código de salida a su izquierda.
            if let Some(label) = block_duration_label(block) {
                let w = self.measure_mono_text(&label, tokens::DETAIL, false);
                let x = right.saturating_sub(w);
                self.draw_mono_text(
                    canvas,
                    &label,
                    x,
                    baseline,
                    right,
                    theme.text_muted(),
                    tokens::DETAIL,
                    false,
                );
                right = x.saturating_sub(12);
            }
            if let Some(code) = block.exit_code.filter(|c| *c != 0) {
                let label = format!("código {code}");
                let w = self.measure_mono_text(&label, tokens::DETAIL, false);
                let x = right.saturating_sub(w);
                self.draw_mono_text(
                    canvas,
                    &label,
                    x,
                    baseline,
                    right,
                    theme.ansi[1],
                    tokens::DETAIL,
                    false,
                );
                right = x.saturating_sub(12);
            }

            // Comando en negrita de interfaz: el prompt desaparece y manda la orden.
            let text_x = icon_x + 22;
            let command = block.command.as_deref().unwrap_or("comando");
            self.draw_ui_text(
                canvas,
                command,
                text_x,
                baseline,
                right.max(text_x + 20),
                theme.foreground,
                tokens::TITLE,
                true,
            );
        }
    }

    fn draw_block_action(
        &mut self,
        canvas: &mut Canvas,
        rect: (usize, usize, usize, usize),
        icon: LucideIcon,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        canvas.rounded_fill(x, y, w, h, 5, theme.chrome_bg());
        canvas.rounded_stroke(x, y, w, h, 5, theme.chrome_line());
        self.draw_lucide_icon(
            canvas,
            icon,
            x + w / 2,
            y + h / 2,
            12.0,
            theme.text_secondary(),
        );
    }
}

fn block_duration_label(block: &CommandBlock) -> Option<String> {
    if block.exit_code.is_none() {
        let elapsed = block.started?.elapsed();
        return Some(format!("En curso · {}", format_duration(elapsed)));
    }
    block.duration().map(format_duration)
}

/// Bloque por índice: los reales primero y, al final, el bloque activo (aún sin OSC 133;D).
fn block_at(terminal: &TerminalState, index: usize) -> Option<&CommandBlock> {
    if index < terminal.command_blocks.len() {
        terminal.command_blocks.get(index)
    } else {
        terminal.active_block.as_ref()
    }
}
