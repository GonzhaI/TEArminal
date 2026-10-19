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
        widths: std::rc::Rc<Vec<usize>>,
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
        let mut widths: Vec<usize> = Vec::new();
        for line in lines[start..i].iter().filter(|l| !is_table_rule(l)) {
            for (c, cell) in table_cells(line).enumerate() {
                let w = cell.chars().count();
                match widths.get_mut(c) {
                    Some(m) => *m = (*m).max(w),
                    None => widths.push(w),
                }
            }
        }
        let widths = std::rc::Rc::new(widths);
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

impl TerminalRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_md_line(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        line: &str,
        block: &MdBlock,
        cur_x: i32,
        row_y: usize,
        row_h: usize,
        baseline_y: usize,
        max_x: usize,
        char_w: usize,
        theme: &Theme,
    ) {
        let height = buffer.len() / width.max(1);
        let cw = char_w as i32;
        let max_x_i32 = max_x as i32;
        let band = theme.surface_bg().to_u32_xrgb();
        let band_w = clip.2.saturating_sub(clip.0);

        match block {
            MdBlock::Code(lang) => {
                fill_rect(buffer, width, clip, clip.0, row_y, band_w, row_h, band);
                self.render_highlighted_line(
                    buffer,
                    width,
                    clip,
                    line,
                    cur_x,
                    baseline_y,
                    max_x,
                    char_w,
                    theme,
                    fence_ext(lang),
                );
                return;
            }
            MdBlock::FenceOpen(lang) => {
                fill_rect(buffer, width, clip, clip.0, row_y, band_w, row_h, band);
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    LucideIcon::Code,
                    clip.0 + 16,
                    row_y + row_h / 2,
                    11.0,
                    theme.ansi[8],
                );
                if !lang.is_empty() {
                    self.draw_ui_text(
                        buffer,
                        width,
                        height,
                        lang,
                        clip.0 + 28,
                        baseline_y,
                        max_x,
                        theme.ansi[8],
                        10.5,
                        false,
                    );
                }
                return;
            }
            MdBlock::FenceClose => {
                fill_rect(buffer, width, clip, clip.0, row_y, band_w, row_h, band);
                return;
            }
            MdBlock::Text | MdBlock::Table { .. } => {}
        }

        // Texto Markdown: la indentación se conserva
        let indent = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<i32>();
        let mut x = cur_x + indent * cw;
        let trimmed = line.trim_start();
        let fg = theme.foreground;
        let ctx = (max_x_i32, cw);

        // Títulos: color por nivel, negrita y subrayado en h1/h2
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        let html_level = trimmed
            .strip_prefix("<h")
            .and_then(|r| r.chars().next())
            .and_then(|c| c.to_digit(10))
            .filter(|d| (1..=6).contains(d))
            .map(|d| d as usize);
        let heading = if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
            Some((level, trimmed[level..].trim()))
        } else {
            html_level.map(|l| (l, trimmed))
        };
        if let Some((level, title)) = heading {
            let color = match level {
                1 => theme.accent_color(),
                2 => theme.ansi[4],
                3 => theme.ansi[6],
                4 => theme.ansi[2],
                5 => theme.ansi[5],
                _ => theme.ansi[3],
            };
            self.draw_md_inline(
                buffer, width, clip, title, x, baseline_y, ctx, theme, color, true,
            );
            if level <= 2 && x < max_x_i32 {
                let ux = x.max(clip.0 as i32) as usize;
                let uw = (max_x_i32 - x).max(0) as usize;
                fill_rect(
                    buffer,
                    width,
                    clip,
                    ux,
                    row_y + row_h - 2,
                    uw,
                    1,
                    theme.border_color().to_u32_xrgb(),
                );
            }
            return;
        }

        // Separador horizontal
        if matches!(trimmed, "---" | "***" | "___") {
            let ux = x.max(clip.0 as i32) as usize;
            let uw = (max_x_i32 - x).max(0) as usize;
            fill_rect(
                buffer,
                width,
                clip,
                ux,
                row_y + row_h / 2,
                uw,
                1,
                theme.border_color().to_u32_xrgb(),
            );
            return;
        }

        // Cita en bloque (admite anidadas: `> > texto`)
        if trimmed.starts_with('>') {
            let mut rest = trimmed;
            let mut qx = x;
            while let Some(r) = rest.strip_prefix('>') {
                fill_rect(
                    buffer,
                    width,
                    clip,
                    qx.max(0) as usize,
                    row_y + 2,
                    3,
                    row_h.saturating_sub(4),
                    theme.ansi[4].to_u32_xrgb(),
                );
                qx += 12;
                rest = r.trim_start();
            }
            self.draw_md_inline(
                buffer,
                width,
                clip,
                rest,
                qx,
                baseline_y,
                ctx,
                theme,
                theme.ansi[7],
                false,
            );
            return;
        }

        // Tabla: filas con separadores verticales; la fila de guiones se dibuja como línea
        if trimmed.starts_with('|') {
            if trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                let ux = x.max(clip.0 as i32) as usize;
                let uw = (max_x_i32 - x).max(0) as usize;
                fill_rect(
                    buffer,
                    width,
                    clip,
                    ux,
                    row_y + row_h / 2,
                    uw,
                    1,
                    theme.border_color().to_u32_xrgb(),
                );
                return;
            }
            let (widths, header) = match block {
                MdBlock::Table { widths, header } => (Some(widths.clone()), *header),
                _ => (None, false),
            };
            for (c, cell) in table_cells(trimmed).enumerate() {
                self.draw_md_bar(buffer, width, clip, x, row_y, row_h, theme);
                let col_w = widths.as_ref().and_then(|w| w.get(c).copied());
                x += cw;
                let end = self.draw_md_inline(
                    buffer, width, clip, cell, x, baseline_y, ctx, theme, fg, header,
                );
                // Con anchos conocidos todas las filas alinean sus columnas
                x = match col_w {
                    Some(w) => x + (w as i32 + 1) * cw,
                    None => end.max(x) + cw,
                };
            }
            self.draw_md_bar(buffer, width, clip, x, row_y, row_h, theme);
            return;
        }

        // Listas: tareas, viñetas y numeradas
        let icon_cy = row_y + row_h / 2;
        let list_item = |this: &mut Self,
                         buffer: &mut [u32],
                         icon: Option<(LucideIcon, Color)>,
                         rest: &str,
                         x: i32| {
            if let Some((icon, color)) = icon {
                let cx = x + cw / 2;
                if cx > clip.0 as i32 + 6 && cx < clip.2 as i32 {
                    this.draw_lucide_icon(
                        buffer,
                        width,
                        height,
                        icon,
                        cx as usize,
                        icon_cy,
                        12.0,
                        color,
                    );
                }
            }
            this.draw_md_inline(
                buffer,
                width,
                clip,
                rest,
                x + cw * 2,
                baseline_y,
                ctx,
                theme,
                fg,
                false,
            );
        };
        for prefix in ["- ", "* ", "+ "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if let Some(done) = rest
                    .strip_prefix("[x] ")
                    .or_else(|| rest.strip_prefix("[X] "))
                {
                    list_item(
                        self,
                        buffer,
                        Some((LucideIcon::CheckSquare, theme.ansi[2])),
                        done,
                        x,
                    );
                } else if let Some(todo) = rest.strip_prefix("[ ] ") {
                    list_item(
                        self,
                        buffer,
                        Some((LucideIcon::Square, theme.ansi[8])),
                        todo,
                        x,
                    );
                } else {
                    list_item(
                        self,
                        buffer,
                        Some((LucideIcon::Dot, theme.accent_color())),
                        rest,
                        x,
                    );
                }
                return;
            }
        }
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(". ") {
            let nx = self.draw_md_inline(
                buffer,
                width,
                clip,
                &trimmed[..digits + 1],
                x,
                baseline_y,
                ctx,
                theme,
                theme.accent_color(),
                true,
            );
            self.draw_md_inline(
                buffer,
                width,
                clip,
                &trimmed[digits + 2..],
                nx + cw,
                baseline_y,
                ctx,
                theme,
                fg,
                false,
            );
            return;
        }

        self.draw_md_inline(
            buffer, width, clip, trimmed, x, baseline_y, ctx, theme, fg, false,
        );
    }

    /// Separador vertical fino de las tablas
    #[allow(clippy::too_many_arguments)]
    fn draw_md_bar(
        &self,
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        x: i32,
        row_y: usize,
        row_h: usize,
        theme: &Theme,
    ) {
        if x >= clip.0 as i32 {
            fill_rect(
                buffer,
                width,
                clip,
                x as usize,
                row_y,
                1,
                row_h,
                theme.border_color().to_u32_xrgb(),
            );
        }
    }

    /// Texto sin formato con color fijo y subrayado opcional (enlaces). Devuelve la X final.
    #[allow(clippy::too_many_arguments)]
    fn draw_md_span(
        &mut self,
        buffer: &mut [u32],
        width: usize,
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
                blit_glyph(buffer, width, clip, glyph, x + glyph.x_offset, gy, color);
            }
            x += cw;
        }
        if underline && start >= 0 && x > start {
            let w = (x.min(max_x) - start).max(0) as usize;
            fill_rect(
                buffer,
                width,
                clip,
                start as usize,
                baseline_y + 2,
                w,
                1,
                color.to_u32_xrgb(),
            );
        }
        x
    }

    /// Imagen: icono + texto alternativo (no se decodifica el archivo). Devuelve la X final.
    #[allow(clippy::too_many_arguments)]
    fn draw_md_image(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        alt: &str,
        x: i32,
        baseline_y: usize,
        ctx: (i32, i32),
        theme: &Theme,
    ) -> i32 {
        let (_, cw) = ctx;
        let height = buffer.len() / width.max(1);
        let cx = x + cw / 2;
        if cx > clip.0 as i32 + 6 && cx < clip.2 as i32 {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                LucideIcon::Image,
                cx as usize,
                baseline_y.saturating_sub(4),
                12.0,
                theme.ansi[5],
            );
        }
        let alt = if alt.is_empty() { "imagen" } else { alt };
        self.draw_md_span(
            buffer,
            width,
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
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        text: &str,
        mut x: i32,
        baseline_y: usize,
        ctx: (i32, i32),
        theme: &Theme,
        base_fg: Color,
        base_bold: bool,
    ) -> i32 {
        let (max_x, cw) = ctx;
        let chars: Vec<char> = text.chars().collect();
        let (mut i, mut code, mut bold, mut italic, mut strike) =
            (0, false, base_bold, false, false);
        let link_color = theme.ansi[4];
        let code_bg = theme.surface_hover().to_u32_xrgb();

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
                        x = self
                            .draw_md_image(buffer, width, clip, &label, x, baseline_y, ctx, theme);
                    } else {
                        x = self.draw_md_span(
                            buffer, width, clip, &label, x, baseline_y, ctx, link_color, true,
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
                            x = self.draw_md_image(
                                buffer, width, clip, &tag.alt, x, baseline_y, ctx, theme,
                            );
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
                x = self.draw_md_span(
                    buffer, width, clip, &url, x, baseline_y, ctx, link_color, true,
                );
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
                fill_rect(
                    buffer,
                    width,
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
                blit_glyph(buffer, width, clip, glyph, x + glyph.x_offset, gy, fg);
            }
            if strike && x >= 0 {
                fill_rect(
                    buffer,
                    width,
                    clip,
                    x as usize,
                    baseline_y.saturating_sub(4),
                    adv as usize,
                    1,
                    base_fg.to_u32_xrgb(),
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
        let c: Vec<char> = r#"<img src="a.png" alt="Axon" />"#.chars().collect();
        let t = html_tag(&c, 0).unwrap();
        assert_eq!(
            (t.name.as_str(), t.alt.as_str(), t.closing),
            ("img", "Axon", false)
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
        assert_eq!(**widths, vec![3, 2]);
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
}
