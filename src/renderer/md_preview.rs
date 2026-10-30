//! Vista previa de Markdown del visor de archivos: bloques de código con resaltado, títulos con
//! subrayado, listas con iconos Lucide, enlaces, imágenes y tablas (sin símbolos Unicode decorativos).

use super::*;
use crate::ui::LucideIcon;

/// Rol de cada línea; los bloques de código necesitan el estado de las líneas anteriores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MdBlock {
    Text,
    FenceOpen(String),
    Code(String),
    FenceClose,
    /// Fila de tabla con los anchos (en caracteres) de las columnas de toda la tabla
    Table {
        widths: std::rc::Rc<[usize]>,
        header: bool,
    },
}

fn table_cells(line: &str) -> impl Iterator<Item = &str> {
    line.trim().trim_matches('|').split('|').map(str::trim)
}

fn is_table_rule(line: &str) -> bool {
    line.trim()
        .chars()
        .all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Convierte las rachas de filas `| a | b |` en `Table` con columnas alineadas
fn mark_tables(lines: &[String], blocks: &mut [MdBlock]) {
    let is_row = |i: usize, blocks: &[MdBlock]| {
        blocks[i] == MdBlock::Text && lines[i].trim_start().starts_with('|')
    };
    let mut i = 0;
    while i < lines.len() {
        if !is_row(i, blocks) {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && is_row(i, blocks) {
            i += 1;
        }
        let mut widths: Vec<usize> = Vec::with_capacity(8);
        for line in lines[start..i].iter().filter(|l| !is_table_rule(l)) {
            for (c, cell) in table_cells(line).enumerate() {
                let w = cell.chars().count();
                match widths.get_mut(c) {
                    Some(m) => *m = (*m).max(w),
                    None => widths.push(w),
                }
            }
        }
        let widths: std::rc::Rc<[usize]> = std::rc::Rc::from(widths);
        for (n, block) in blocks[start..i].iter_mut().enumerate() {
            *block = MdBlock::Table {
                widths: widths.clone(),
                header: n == 0,
            };
        }
    }
}

pub(super) fn classify_blocks(lines: &[String]) -> Vec<MdBlock> {
    let mut out = Vec::with_capacity(lines.len());
    let mut open: Option<(&'static str, String)> = None;
    for line in lines {
        let t = line.trim_start();
        match &open {
            None => {
                let marker = ["```", "~~~"].into_iter().find(|m| t.starts_with(m));
                if let Some(m) = marker {
                    let lang = t[3..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_lowercase();
                    out.push(MdBlock::FenceOpen(lang.clone()));
                    open = Some((m, lang));
                } else {
                    out.push(MdBlock::Text);
                }
            }
            Some((m, lang)) => {
                if t.starts_with(m) {
                    out.push(MdBlock::FenceClose);
                    open = None;
                } else {
                    out.push(MdBlock::Code(lang.clone()));
                }
            }
        }
    }
    mark_tables(lines, &mut out);
    out
}

/// Extensión que entiende el resaltador para el lenguaje indicado en la valla del bloque.
fn fence_ext(lang: &str) -> &str {
    match lang {
        "rust" => "rs",
        "python" => "py",
        "javascript" | "node" => "js",
        "typescript" => "ts",
        "bash" | "shell" | "zsh" | "console" => "sh",
        "powershell" | "pwsh" => "ps1",
        "yml" => "yaml",
        "" | "text" | "plaintext" => "txt",
        other => other,
    }
}

fn find_from(chars: &[char], from: usize, target: char) -> Option<usize> {
    chars
        .get(from..)?
        .iter()
        .position(|c| *c == target)
        .map(|p| p + from)
}

/// Etiqueta HTML reconocida dentro de una línea de Markdown.
struct HtmlTag {
    name: String,
    closing: bool,
    /// Texto alternativo de `<img alt="...">`
    alt: String,
    /// Índice del `>` que la cierra
    end: usize,
}

/// Solo etiquetas conocidas: un `<T>` suelto en el texto se dibuja tal cual.
fn html_tag(chars: &[char], i: usize) -> Option<HtmlTag> {
    const KNOWN: &[&str] = &[
        "div", "p", "h1", "h2", "h3", "h4", "h5", "h6", "img", "em", "i", "strong", "b", "code",
        "kbd", "s", "del", "a", "br", "hr", "sup", "sub", "span", "details", "summary", "picture",
        "source", "center", "u",
    ];
    let end = find_from(chars, i + 1, '>')?;
    let inner: String = chars[i + 1..end].iter().collect();
    let (closing, body) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner.as_str()),
    };
    let name: String = body
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if !KNOWN.contains(&name.as_str()) {
        return None;
    }
    let alt = body
        .split_once("alt=\"")
        .and_then(|(_, r)| r.split_once('"'))
        .map(|(a, _)| a.to_string())
        .unwrap_or_default();
    Some(HtmlTag {
        name,
        closing,
        alt,
        end,
    })
}

/// Sangría horizontal en píxeles para filas continuadas de listas o citas en vista previa de Markdown
pub(crate) fn md_continuation_indent(full_line: &str, char_w: usize) -> i32 {
    let full_trimmed = full_line.trim_start();
    let indent = full_line.len() - full_trimmed.len();
    let cw = char_w as i32;
    let base = indent as i32 * cw;

    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = full_trimmed.strip_prefix(prefix) {
            let offset = if rest.starts_with("[x] ") || rest.starts_with("[X] ") || rest.starts_with("[ ] ") {
                prefix.len() + 4
            } else {
                2
            };
            return base + offset as i32 * cw;
        }
    }

    let digits = full_trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && full_trimmed[digits..].starts_with(". ") {
        let offset = digits + 2;
        return base + offset as i32 * cw;
    }

    if full_trimmed.starts_with('>') {
        let mut q_count = 0;
        let mut r = full_trimmed;
        while let Some(stripped) = r.strip_prefix('>') {
            q_count += 1;
            r = stripped.trim_start();
        }
        return base + q_count as i32 * 12;
    }

    base
}

/// Estado de formato en línea (código, negrita, cursiva, tachado) alcanzado en la columna `at`.
pub(crate) fn md_inline_state_at(full_line: &str, at: usize) -> (bool, bool, bool, bool) {
    let mut code = false;
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let chars: Vec<char> = full_line.chars().take(at).collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '`' {
            code = !code;
            i += 1;
            continue;
        }
        if !code {
            if ch == '*' && chars.get(i + 1) == Some(&'*') {
                bold = !bold;
                i += 2;
                continue;
            }
            if ch == '~' && chars.get(i + 1) == Some(&'~') {
                strike = !strike;
                i += 2;
                continue;
            }
            if ch == '*' {
                italic = !italic;
                i += 1;
                continue;
            }
        }
        i += 1;
    }
    (code, bold, italic, strike)
}

/// Ajusta los anchos de columna para que la tabla quepa en las columnas disponibles (`available_cols`).
/// Si la suma cabe holgadamente, se usan los anchos naturales; si excede, se reducen proporcionalmente
/// las columnas más anchas respetando un mínimo por columna.
pub(crate) fn fit_table_column_widths(widths: &[usize], available_cols: usize) -> Vec<usize> {
    if widths.is_empty() {
        return Vec::new();
    }
    let k = widths.len();
    let overhead = k + 1;
    let net_available = available_cols.saturating_sub(overhead).max(k * 4);
    let natural_total: usize = widths.iter().sum();

    if natural_total <= net_available {
        return widths.to_vec();
    }

    let min_widths: Vec<usize> = widths.iter().map(|&w| w.min(4).max(1)).collect();
    let min_total: usize = min_widths.iter().sum();
    if min_total >= net_available {
        return min_widths;
    }

    let extra_budget = net_available - min_total;
    let excess_total: usize = widths
        .iter()
        .zip(&min_widths)
        .map(|(&w, &m)| w.saturating_sub(m))
        .sum();

    if excess_total == 0 {
        return min_widths;
    }

    let mut out: Vec<usize> = Vec::with_capacity(k);
    let mut distributed = 0;
    for (i, (&w, &min_w)) in widths.iter().zip(&min_widths).enumerate() {
        let excess = w.saturating_sub(min_w);
        let share = if i == k - 1 {
            extra_budget.saturating_sub(distributed)
        } else {
            (excess * extra_budget) / excess_total
        };
        distributed += share;
        out.push(min_w + share);
    }
    out
}

impl TerminalRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_md_line(
        &mut self,
        canvas: &mut Canvas,
        clip: Clip,
        line: &str,
        full_line: &str,
        row_start: usize,
        is_last_segment: bool,
        block: &MdBlock,
        cur_x: i32,
        row_y: usize,
        row_h: usize,
        baseline_y: usize,
        max_x: usize,
        char_w: usize,
        theme: &Theme,
    ) {
        let cw = char_w as i32;
        let max_x_i32 = max_x as i32;
        let band = theme.surface_bg();
        let band_w = clip.2.saturating_sub(clip.0);
        let first_segment = row_start == 0;
        let init_state = if first_segment {
            (false, false, false, false)
        } else {
            md_inline_state_at(full_line, row_start)
        };

        match block {
            MdBlock::Code(lang) => {
                canvas.fill_rect(clip, clip.0, row_y, band_w, row_h, band);
                // Borde lateral izquierdo acentuado
                canvas.fill_rect(
                    clip,
                    clip.0,
                    row_y,
                    2,
                    row_h,
                    theme.mix(theme.background, theme.ansi[8], 0.35),
                );
                let x = if first_segment {
                    cur_x
                } else {
                    let code_indent = full_line
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .map(|c| if c == '\t' { 4 } else { 1 })
                        .sum::<i32>();
                    cur_x + code_indent * cw
                };
                self.render_highlighted_line(
                    canvas,
                    clip,
                    line,
                    x,
                    baseline_y,
                    max_x,
                    char_w,
                    theme,
                    fence_ext(lang),
                );
                return;
            }
            MdBlock::FenceOpen(lang) => {
                let header_bg = theme.mix(theme.surface_bg(), theme.background, 0.45);
                canvas.fill_rect(clip, clip.0, row_y, band_w, row_h, header_bg);
                // Borde superior y lateral
                canvas.fill_rect(clip, clip.0, row_y, band_w, 1, theme.border_color());
                canvas.fill_rect(
                    clip,
                    clip.0,
                    row_y,
                    2,
                    row_h,
                    theme.mix(theme.background, theme.ansi[8], 0.45),
                );
                self.draw_lucide_icon(
                    canvas,
                    LucideIcon::Code,
                    clip.0 + 16,
                    row_y + row_h / 2,
                    11.0,
                    theme.ansi[4],
                );
                if !lang.is_empty() {
                    self.draw_ui_text(
                        canvas,
                        lang,
                        clip.0 + 28,
                        baseline_y,
                        max_x,
                        theme.accent_color(),
                        10.5,
                        true,
                    );
                }
                return;
            }
            MdBlock::FenceClose => {
                canvas.fill_rect(clip, clip.0, row_y, band_w, row_h, band);
                canvas.fill_rect(
                    clip,
                    clip.0,
                    row_y,
                    2,
                    row_h,
                    theme.mix(theme.background, theme.ansi[8], 0.35),
                );
                // Borde inferior
                canvas.fill_rect(
                    clip,
                    clip.0,
                    row_y + row_h.saturating_sub(1),
                    band_w,
                    1,
                    theme.border_color(),
                );
                return;
            }
            MdBlock::Text | MdBlock::Table { .. } => {}
        }

        // Texto Markdown: la indentación se calcula a partir de la línea completa
        let full_trimmed = full_line.trim_start();
        let full_indent = full_line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<i32>();
        let base_x = cur_x + full_indent * cw;

        let trimmed = line.trim_start();
        let fg = theme.foreground;
        let ctx = (max_x_i32, cw);

        // Títulos: color por nivel, negrita y subrayado en h1/h2
        let level = full_trimmed.chars().take_while(|c| *c == '#').count();
        let html_level = full_trimmed
            .strip_prefix("<h")
            .and_then(|r| r.chars().next())
            .and_then(|c| c.to_digit(10))
            .filter(|d| (1..=6).contains(d))
            .map(|d| d as usize);
        let heading = if (1..=6).contains(&level) && full_trimmed[level..].starts_with(' ') {
            Some(level)
        } else {
            html_level
        };
        if let Some(level) = heading {
            let color = match level {
                1 => theme.accent_color(),
                2 => theme.ansi[4],
                3 => theme.ansi[6],
                4 => theme.ansi[2],
                5 => theme.ansi[5],
                _ => theme.ansi[3],
            };
            let title = if first_segment {
                if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
                    trimmed[level..].trim()
                } else {
                    trimmed
                }
            } else {
                trimmed
            };
            self.draw_md_inline(canvas, clip, title, base_x, baseline_y, ctx, theme, color, true, init_state);
            if level <= 2 && is_last_segment && base_x < max_x_i32 {
                let ux = base_x.max(clip.0 as i32) as usize;
                let uw = (max_x_i32 - base_x).max(0) as usize;
                canvas.fill_rect(clip, ux, row_y + row_h - 2, uw, 1, theme.border_color());
            }
            return;
        }

        // Separador horizontal
        if matches!(full_trimmed, "---" | "***" | "___") {
            let ux = base_x.max(clip.0 as i32) as usize;
            let uw = (max_x_i32 - base_x).max(0) as usize;
            canvas.fill_rect(clip, ux, row_y + row_h / 2, uw, 1, theme.border_color());
            return;
        }

        // Cita en bloque (admite anidadas: `> > texto`) y alertas estilo GitHub (`> [!NOTE]`, etc.)
        if full_trimmed.starts_with('>') {
            let mut rest_full = full_trimmed;
            let mut q_count = 0;
            while let Some(r) = rest_full.strip_prefix('>') {
                q_count += 1;
                rest_full = r.trim_start();
            }

            // Detección de alerta / callout estilo GitHub
            let alert_info = if rest_full.starts_with("[!") {
                if let Some(end) = rest_full.find(']') {
                    let tag = &rest_full[2..end];
                    let after_tag = rest_full[end + 1..].trim_start();
                    match tag.to_ascii_uppercase().as_str() {
                        "NOTE" => Some((LucideIcon::Info, theme.ansi[4], "Nota", after_tag)),
                        "TIP" => Some((LucideIcon::Sparkles, theme.ansi[2], "Consejo", after_tag)),
                        "IMPORTANT" => Some((LucideIcon::Sparkles, theme.ansi[5], "Importante", after_tag)),
                        "WARNING" => Some((LucideIcon::Info, theme.ansi[3], "Advertencia", after_tag)),
                        "CAUTION" => Some((LucideIcon::X, theme.ansi[1], "Precaución", after_tag)),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let (bar_color, bg_tint) = if let Some((_, c, _, _)) = alert_info {
                (c, Some(Color::from_u32_xrgb(theme.background.blend_over(c, 24))))
            } else {
                (theme.ansi[4], Some(Color::from_u32_xrgb(theme.background.blend_over(theme.ansi[4], 14))))
            };

            let mut qx = base_x;
            for _ in 0..q_count {
                if let Some(bg) = bg_tint {
                    canvas.fill_rect(
                        clip,
                        qx.max(0) as usize,
                        row_y,
                        (max_x_i32 - qx).max(0) as usize,
                        row_h,
                        bg,
                    );
                }
                canvas.fill_rect(
                    clip,
                    qx.max(0) as usize,
                    row_y + 1,
                    3,
                    row_h.saturating_sub(2),
                    bar_color,
                );
                qx += 14;
            }

            if let Some((icon, color, title, after_tag)) = alert_info {
                if first_segment {
                    let icon_cx = qx + cw;
                    if icon_cx > clip.0 as i32 + 6 && icon_cx < clip.2 as i32 {
                        self.draw_lucide_icon(canvas, icon, icon_cx as usize, row_y + row_h / 2, 12.0, color);
                    }
                    let title_x = icon_cx + cw + 4;
                    let nx = self.draw_md_inline(
                        canvas,
                        clip,
                        title,
                        title_x,
                        baseline_y,
                        ctx,
                        theme,
                        color,
                        true,
                        init_state,
                    );
                    if !after_tag.is_empty() {
                        self.draw_md_inline(
                            canvas,
                            clip,
                            after_tag,
                            nx + cw,
                            baseline_y,
                            ctx,
                            theme,
                            theme.foreground,
                            false,
                            init_state,
                        );
                    }
                    return;
                }
            }

            let text_to_draw = if first_segment {
                let mut r = trimmed;
                while let Some(stripped) = r.strip_prefix('>') {
                    r = stripped.trim_start();
                }
                r
            } else {
                trimmed
            };
            self.draw_md_inline(
                canvas,
                clip,
                text_to_draw,
                qx,
                baseline_y,
                ctx,
                theme,
                theme.ansi[7],
                false,
                init_state,
            );
            return;
        }

        // Tabla: filas con separadores verticales; la fila de guiones se dibuja como línea
        if trimmed.starts_with('|') {
            let (raw_widths, header) = match block {
                MdBlock::Table { widths, header } => (Some(widths.as_ref()), *header),
                _ => (None, false),
            };

            let available_cols = max_x_i32.saturating_sub(base_x).max(0) as usize / cw.max(1) as usize;
            let fitted_widths = raw_widths.map(|w| fit_table_column_widths(w, available_cols));
            let widths = fitted_widths.as_deref().or(raw_widths);

            let table_total_chars: usize = match widths {
                Some(w) => w.iter().map(|c| c + 1).sum::<usize>() + 1,
                None => (max_x_i32.saturating_sub(base_x) / cw).max(1) as usize,
            };
            let table_w = (table_total_chars as i32 * cw).min(max_x_i32 - base_x);
            let table_end_x = base_x + table_w;

            if trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                let ux = base_x.max(clip.0 as i32) as usize;
                let uw = (table_end_x.saturating_sub(base_x)).max(0) as usize;
                canvas.fill_rect(clip, ux, row_y + row_h / 2, uw, 1, theme.border_color());
                if let Some(w_list) = widths {
                    let mut cx = base_x;
                    for w in w_list {
                        self.draw_md_bar(canvas, clip, cx, row_y + row_h / 4, row_h / 2, theme);
                        cx += (w + 1) as i32 * cw;
                    }
                    self.draw_md_bar(canvas, clip, cx, row_y + row_h / 4, row_h / 2, theme);
                }
                return;
            }

            // Fondo sutil para la cabecera de la tabla
            if header {
                let ux = base_x.max(clip.0 as i32) as usize;
                let uw = (table_end_x.saturating_sub(base_x)).max(0) as usize;
                canvas.fill_rect(
                    clip,
                    ux,
                    row_y,
                    uw,
                    row_h,
                    theme.mix(theme.background, theme.foreground, 0.05),
                );
            }

            let mut x = base_x;
            for (c, cell) in table_cells(trimmed).enumerate() {
                self.draw_md_bar(canvas, clip, x, row_y, row_h, theme);
                let col_w = widths.and_then(|w| w.get(c).copied()).unwrap_or_else(|| cell.chars().count());
                let cell_col_w = (col_w as i32 + 1) * cw;
                let cell_max_x = (x + cell_col_w).min(max_x_i32);
                let cell_clip = (x.max(clip.0 as i32) as usize, clip.1, cell_max_x as usize, clip.3);

                x += cw;
                let cell_char_count = cell.chars().count();
                let cell_text = if cell_char_count > col_w && col_w >= 3 {
                    let truncated: String = cell.chars().take(col_w.saturating_sub(1)).collect();
                    format!("{truncated}…")
                } else {
                    cell.to_string()
                };

                let cell_ctx = (cell_max_x, cw);
                self.draw_md_inline(
                    canvas,
                    cell_clip,
                    &cell_text,
                    x,
                    baseline_y,
                    cell_ctx,
                    theme,
                    if header { theme.accent_color() } else { fg },
                    header,
                    (false, false, false, false),
                );

                x += col_w as i32 * cw;
            }
            self.draw_md_bar(canvas, clip, x, row_y, row_h, theme);
            return;
        }

        // Listas: tareas, viñetas y numeradas
        let icon_cy = row_y + row_h / 2;
        let mut ul_prefix = None;
        for p in ["- ", "* ", "+ "] {
            if full_trimmed.starts_with(p) {
                ul_prefix = Some(p);
                break;
            }
        }
        if let Some(prefix) = ul_prefix {
            let after_prefix = &full_trimmed[prefix.len()..];
            let (icon, color, offset_chars, is_todo) = if after_prefix.starts_with("[x] ") || after_prefix.starts_with("[X] ") {
                (LucideIcon::CheckSquare, theme.ansi[2], prefix.len() + 4, true)
            } else if after_prefix.starts_with("[ ] ") {
                (LucideIcon::Square, theme.ansi[8], prefix.len() + 4, true)
            } else {
                (LucideIcon::Dot, theme.accent_color(), 2, false)
            };
            let text_x = base_x + offset_chars as i32 * cw;
            if first_segment {
                let cx = base_x + cw / 2;
                if cx > clip.0 as i32 + 6 && cx < clip.2 as i32 {
                    self.draw_lucide_icon(canvas, icon, cx as usize, icon_cy, 12.0, color);
                }
                let rest_text = if is_todo {
                    trimmed
                        .strip_prefix(prefix)
                        .and_then(|r| r.strip_prefix("[x] ").or_else(|| r.strip_prefix("[X] ")).or_else(|| r.strip_prefix("[ ] ")))
                        .unwrap_or(trimmed)
                } else {
                    trimmed.strip_prefix(prefix).unwrap_or(trimmed)
                };
                self.draw_md_inline(canvas, clip, rest_text, text_x, baseline_y, ctx, theme, fg, false, init_state);
            } else {
                self.draw_md_inline(canvas, clip, trimmed, text_x, baseline_y, ctx, theme, fg, false, init_state);
            }
            return;
        }

        let digits = full_trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && full_trimmed[digits..].starts_with(". ") {
            let prefix_len = digits + 2;
            let text_x = base_x + prefix_len as i32 * cw;
            if first_segment {
                let nx = self.draw_md_inline(
                    canvas,
                    clip,
                    &trimmed[..digits + 1],
                    base_x,
                    baseline_y,
                    ctx,
                    theme,
                    theme.accent_color(),
                    true,
                    init_state,
                );
                let rest_text = if trimmed.len() >= prefix_len { &trimmed[prefix_len..] } else { "" };
                self.draw_md_inline(
                    canvas,
                    clip,
                    rest_text,
                    nx + cw,
                    baseline_y,
                    ctx,
                    theme,
                    fg,
                    false,
                    init_state,
                );
            } else {
                self.draw_md_inline(canvas, clip, trimmed, text_x, baseline_y, ctx, theme, fg, false, init_state);
            }
            return;
        }

        let seg_indent = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<i32>();
        let x = if first_segment { cur_x + seg_indent * cw } else { base_x };
        self.draw_md_inline(canvas, clip, trimmed, x, baseline_y, ctx, theme, fg, false, init_state);
    }

    /// Separador vertical fino de las tablas
    #[allow(clippy::too_many_arguments)]
    fn draw_md_bar(
        &self,
        canvas: &mut Canvas,
        clip: Clip,
        x: i32,
        row_y: usize,
        row_h: usize,
        theme: &Theme,
    ) {
        if x >= clip.0 as i32 {
            canvas.fill_rect(clip, x as usize, row_y, 1, row_h, theme.border_color());
        }
    }

    /// Texto sin formato con color fijo y subrayado opcional (enlaces). Devuelve la X final.
    #[allow(clippy::too_many_arguments)]
    fn draw_md_span(
        &mut self,
        canvas: &mut Canvas,
        clip: Clip,
        text: &str,
        mut x: i32,
        baseline_y: usize,
        ctx: (i32, i32),
        color: Color,
        underline: bool,
    ) -> i32 {
        let (max_x, cw) = ctx;
        let start = x;
        for c in text.chars() {
            if x >= max_x {
                break;
            }
            if c != ' ' {
                let glyph = self.get_glyph(c, false);
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                canvas.glyph(clip, glyph, x + glyph.x_offset, gy, color);
            }
            x += cw;
        }
        if underline && start >= 0 && x > start {
            let w = (x.min(max_x) - start).max(0) as usize;
            canvas.fill_rect(clip, start as usize, baseline_y + 2, w, 1, color);
        }
        x
    }

    /// Imagen: icono + texto alternativo (no se decodifica el archivo). Devuelve la X final.
    #[allow(clippy::too_many_arguments)]
    fn draw_md_image(
        &mut self,
        canvas: &mut Canvas,
        clip: Clip,
        alt: &str,
        x: i32,
        baseline_y: usize,
        ctx: (i32, i32),
        theme: &Theme,
    ) -> i32 {
        let (_, cw) = ctx;
        let cx = x + cw / 2;
        if cx > clip.0 as i32 + 6 && cx < clip.2 as i32 {
            self.draw_lucide_icon(
                canvas,
                LucideIcon::Image,
                cx as usize,
                baseline_y.saturating_sub(4),
                12.0,
                theme.ansi[5],
            );
        }
        let alt = if alt.is_empty() { "imagen" } else { alt };
        self.draw_md_span(
            canvas,
            clip,
            alt,
            x + cw * 2,
            baseline_y,
            ctx,
            theme.ansi[4],
            true,
        )
    }

    /// Texto en línea: `código`, **negrita**, *cursiva*, ~~tachado~~, [enlaces](url), ![imágenes](url)
    /// y URLs sueltas. Devuelve la X final. `ctx` = (X máxima, ancho de carácter).
    #[allow(clippy::too_many_arguments)]
    fn draw_md_inline(
        &mut self,
        canvas: &mut Canvas,
        clip: Clip,
        text: &str,
        mut x: i32,
        baseline_y: usize,
        ctx: (i32, i32),
        theme: &Theme,
        base_fg: Color,
        base_bold: bool,
        init_state: (bool, bool, bool, bool),
    ) -> i32 {
        let (max_x, cw) = ctx;
        let chars: Vec<char> = text.chars().collect();
        let (mut i, mut code, mut bold, mut italic, mut strike) = (
            0,
            init_state.0,
            init_state.1 || base_bold,
            init_state.2,
            init_state.3,
        );
        let link_color = theme.ansi[4];
        let code_bg = theme.surface_hover();

        while i < chars.len() && x < max_x {
            let ch = chars[i];

            if !code && (ch == '[' || (ch == '!' && chars.get(i + 1) == Some(&'['))) {
                let is_img = ch == '!';
                let open = i + is_img as usize;
                let link = find_from(&chars, open + 1, ']')
                    .filter(|c| chars.get(c + 1) == Some(&'('))
                    .and_then(|c| find_from(&chars, c + 2, ')').map(|e| (c, e)));
                if let Some((close, end)) = link {
                    let label: String = chars[open + 1..close].iter().collect();
                    if is_img {
                        x = self.draw_md_image(canvas, clip, &label, x, baseline_y, ctx, theme);
                    } else {
                        x = self.draw_md_span(
                            canvas, clip, &label, x, baseline_y, ctx, link_color, true,
                        );
                    }
                    i = end + 1;
                    continue;
                }
            }

            // Etiquetas HTML habituales en los README (`<img>`, `<em>`, `<h1>`…)
            if !code && ch == '<' {
                if let Some(tag) = html_tag(&chars, i) {
                    let on = !tag.closing;
                    match tag.name.as_str() {
                        "em" | "i" => italic = on,
                        "strong" | "b" => bold = on || base_bold,
                        "code" | "kbd" => code = on,
                        "s" | "del" => strike = on,
                        "img" => {
                            x = self
                                .draw_md_image(canvas, clip, &tag.alt, x, baseline_y, ctx, theme);
                        }
                        _ => {}
                    }
                    i = tag.end + 1;
                    continue;
                }
            }

            // URL suelta: se resalta como enlace hasta el siguiente espacio
            if !code
                && (ch == 'h')
                && (chars[i..].starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
                    || chars[i..].starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/']))
            {
                let end = chars[i..]
                    .iter()
                    .position(|c| c.is_whitespace() || matches!(c, ')' | '>' | '"'))
                    .map_or(chars.len(), |p| i + p);
                let url: String = chars[i..end].iter().collect();
                x = self.draw_md_span(canvas, clip, &url, x, baseline_y, ctx, link_color, true);
                i = end;
                continue;
            }

            match ch {
                '`' => {
                    code = !code;
                    i += 1;
                    continue;
                }
                '*' if !code && chars.get(i + 1) == Some(&'*') => {
                    bold = !bold;
                    i += 2;
                    continue;
                }
                '~' if !code && chars.get(i + 1) == Some(&'~') => {
                    strike = !strike;
                    i += 2;
                    continue;
                }
                '*' if !code => {
                    italic = !italic;
                    i += 1;
                    continue;
                }
                // Escape: el siguiente carácter se dibuja tal cual
                '\\' if !code && chars.get(i + 1).is_some_and(|c| c.is_ascii_punctuation()) => {
                    i += 1;
                }
                _ => {}
            }
            let ch = chars[i];
            let adv = if ch == '\t' { cw * 4 } else { cw };

            if code {
                // Fondo del código en línea
                let top = baseline_y.saturating_sub((cw * 3 / 2) as usize);
                canvas.fill_rect(
                    clip,
                    x.max(0) as usize,
                    top,
                    adv as usize,
                    (cw * 2) as usize,
                    code_bg,
                );
            }
            if ch != ' ' && ch != '\t' {
                let (fg, is_bold) = if code {
                    (theme.ansi[3], true)
                } else if italic {
                    (theme.ansi[5], bold)
                } else {
                    (base_fg, bold)
                };
                let glyph = self.get_glyph(ch, is_bold);
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                canvas.glyph(clip, glyph, x + glyph.x_offset, gy, fg);
            }
            if strike && x >= 0 {
                canvas.fill_rect(
                    clip,
                    x as usize,
                    baseline_y.saturating_sub(4),
                    adv as usize,
                    1,
                    base_fg,
                );
            }
            x += adv;
            i += 1;
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    #[test]
    fn test_html_tags_are_recognized_only_when_known() {
        let c: Vec<char> = r#"<img src="a.png" alt="TEArminal" />"#.chars().collect();
        let t = html_tag(&c, 0).unwrap();
        assert_eq!(
            (t.name.as_str(), t.alt.as_str(), t.closing),
            ("img", "TEArminal", false)
        );
        assert_eq!(t.end, c.len() - 1);
        let c: Vec<char> = "</em>".chars().collect();
        assert!(html_tag(&c, 0).unwrap().closing);
        let c: Vec<char> = "Vec<T> genérico".chars().collect();
        assert!(html_tag(&c, 3).is_none());
    }

    #[test]
    fn test_fenced_blocks_are_classified_with_language() {
        let b = classify_blocks(&lines("texto\n```rust\nfn a() {}\n```\nfin\n~~~\nx\n~~~"));
        assert_eq!(
            b,
            vec![
                MdBlock::Text,
                MdBlock::FenceOpen("rust".into()),
                MdBlock::Code("rust".into()),
                MdBlock::FenceClose,
                MdBlock::Text,
                MdBlock::FenceOpen(String::new()),
                MdBlock::Code(String::new()),
                MdBlock::FenceClose,
            ]
        );
    }

    #[test]
    fn test_other_fence_marker_does_not_close_block() {
        let b = classify_blocks(&lines("```\n~~~\n```"));
        assert_eq!(b[1], MdBlock::Code(String::new()));
        assert_eq!(b[2], MdBlock::FenceClose);
    }

    #[test]
    fn test_table_rows_share_column_widths() {
        let b = classify_blocks(&lines("| a | bb |\n|---|----|\n| ccc | d |\ntexto"));
        let MdBlock::Table { widths, header } = &b[0] else {
            panic!("la primera fila debe ser tabla");
        };
        assert_eq!(&widths[..], &[3, 2]);
        assert!(*header);
        assert!(matches!(&b[2], MdBlock::Table { header: false, .. }));
        assert_eq!(b[3], MdBlock::Text);
    }

    #[test]
    fn test_fence_language_aliases() {
        assert_eq!(fence_ext("rust"), "rs");
        assert_eq!(fence_ext("powershell"), "ps1");
        assert_eq!(fence_ext(""), "txt");
        assert_eq!(fence_ext("toml"), "toml");
    }

    #[test]
    fn test_md_continuation_indent() {
        let cw = 10;
        // Viñeta simple: 2 columnas = 20 px
        assert_eq!(md_continuation_indent("- Elemento de lista", cw), 20);
        // Tarea sin completar: 6 columnas = 60 px
        assert_eq!(md_continuation_indent("- [ ] Tarea pendiente", cw), 60);
        // Tarea completada: 6 columnas = 60 px
        assert_eq!(md_continuation_indent("- [x] Tarea hecha", cw), 60);
        // Lista numerada: 1. -> 3 columnas = 30 px
        assert_eq!(md_continuation_indent("1. Primer punto", cw), 30);
        // Lista numerada: 12. -> 4 columnas = 40 px
        assert_eq!(md_continuation_indent("12. Punto doce", cw), 40);
        // Cita simple: 12 px
        assert_eq!(md_continuation_indent("> Cita de bloque", cw), 12);
        // Cita anidada: 24 px
        assert_eq!(md_continuation_indent("> > Cita anidada", cw), 24);
        // Párrafo normal: 0 px
        assert_eq!(md_continuation_indent("Texto plano normal", cw), 0);
    }

    #[test]
    fn test_md_inline_state_at() {
        let line = "Texto con **negrita** y `código` y ~~tachado~~ y *cursiva*.";
        // Antes de la negrita
        assert_eq!(md_inline_state_at(line, 5), (false, false, false, false));
        // Dentro de la negrita
        assert_eq!(md_inline_state_at(line, 14), (false, true, false, false));
        // Después de la negrita y antes del código
        assert_eq!(md_inline_state_at(line, 22), (false, false, false, false));
        // Dentro del código
        assert_eq!(md_inline_state_at(line, 27), (true, false, false, false));
        // Dentro del tachado
        let strike_idx = line.find("tachado").unwrap();
        assert_eq!(md_inline_state_at(line, strike_idx + 2), (false, false, false, true));
        // Dentro de la cursiva
        let italic_idx = line.find("cursiva").unwrap();
        assert_eq!(md_inline_state_at(line, italic_idx + 2), (false, false, true, false));
    }

    #[test]
    fn test_fit_table_column_widths() {
        // Tabla que cabe holgadamente: se respetan los anchos originales
        let natural = [10, 15, 20];
        let fitted = fit_table_column_widths(&natural, 80);
        assert_eq!(fitted, vec![10, 15, 20]);

        // Tabla que excede el espacio disponible: se reducen las columnas anchas
        let wide = [10, 60, 20];
        let fitted_shrunk = fit_table_column_widths(&wide, 40);
        let total: usize = fitted_shrunk.iter().sum::<usize>() + 4; // 3 columnas + 4 separadores
        assert!(total <= 40, "la tabla ajustada debe caber en 40 columnas, total = {total}");
        // La columna más ancha (60) absorbió la mayor parte de la reducción
        assert!(fitted_shrunk[1] < 60);
        // Cada columna preserva un mínimo legible
        assert!(fitted_shrunk.iter().all(|&w| w >= 4));
    }
}
