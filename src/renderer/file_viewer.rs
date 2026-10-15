//! Renderizado del visor de archivos y código en pestaña dedicada (File Viewer).

use super::*;
use crate::app::FileViewerState;

impl TerminalRenderer {
    /// Renderiza el visor de archivos/código en el área del terminal sin procesos ConPTY
    pub fn render_file_viewer(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        viewer: &FileViewerState,
        theme: &Theme,
    ) {
        if w == 0 || h == 0 || width == 0 || height == 0 {
            return;
        }

        let clip: Clip = (x, y, (x + w).min(width), (y + h).min(height));

        // 1. Fondo completo del área de trabajo
        fill_rect(buffer, width, clip, x, y, w, h, theme.background.to_u32_xrgb());

        // 2. Barra de encabezado superior (38px)
        let header_h = 38;
        let header_bg = theme.surface_bg();
        fill_rect(buffer, width, clip, x, y, w, header_h, header_bg.to_u32_xrgb());

        // Línea divisoria inferior del encabezado
        fill_rect(
            buffer,
            width,
            clip,
            x,
            y + header_h - 1,
            w,
            1,
            theme.border_color().to_u32_xrgb(),
        );

        // Icono representativo del tipo de archivo
        let ext = viewer
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let file_icon = match ext.as_str() {
            "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "cs" | "html" | "css" => {
                crate::ui::LucideIcon::FileCode
            }
            "toml" | "json" | "yaml" | "yml" => crate::ui::LucideIcon::FileJson,
            "md" | "txt" | "log" => crate::ui::LucideIcon::FileText,
            _ => crate::ui::LucideIcon::File,
        };

        let header_center_y = y + (header_h / 2);
        let header_base = header_center_y + 4;

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            file_icon,
            x + 22,
            header_center_y,
            13.0,
            theme.accent_color(),
        );

        // Nombre del archivo (Inter Semibold)
        let name_end_x = self.draw_ui_text(
            buffer,
            width,
            height,
            &viewer.file_name,
            x + 38,
            header_base,
            (x + w).saturating_sub(240),
            theme.foreground,
            12.5,
            true,
        );

        // Ruta relativa o del directorio padre
        let parent_str = viewer
            .path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if !parent_str.is_empty() && name_end_x + 16 < (x + w).saturating_sub(260) {
            let path_display = format!("—  {}", parent_str);
            self.draw_ui_text(
                buffer,
                width,
                height,
                &path_display,
                name_end_x + 12,
                header_base,
                (x + w).saturating_sub(260),
                theme.ansi[8],
                11.0,
                false,
            );
        }

        // Badges informativos a la derecha
        let mut badge_right_x = (x + w).saturating_sub(16);

        // Badge de estado de edición ("MODIFICADO" o "SOLO LECTURA" si binario o "EDITABLE")
        let (status_text, status_fg, status_bg) = if viewer.is_binary {
            ("SOLO LECTURA", theme.ansi[8], theme.surface_hover())
        } else if viewer.is_dirty {
            ("MODIFICADO", theme.ansi[3], Color::from_rgba(255, 180, 0, 24))
        } else {
            ("EDITABLE", theme.ansi[2], Color::from_rgba(0, 255, 128, 16))
        };
        let status_w = self.measure_ui_text(status_text, 10.0, true) + 14;
        let status_x = badge_right_x.saturating_sub(status_w);
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            status_x,
            y + 8,
            status_w,
            22,
            4,
            status_bg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            status_text,
            status_x + 7,
            header_base - 1,
            badge_right_x,
            status_fg,
            10.0,
            true,
        );
        badge_right_x = status_x.saturating_sub(8);

        // Badge de cursor (Ln X, Col Y) si no es binario
        if !viewer.is_binary {
            let cursor_text = format!("Ln {}, Col {}", viewer.cursor_line + 1, viewer.cursor_col + 1);
            let cursor_w = self.measure_ui_text(&cursor_text, 10.5, false) + 14;
            let cursor_x = badge_right_x.saturating_sub(cursor_w);
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                cursor_x,
                y + 8,
                cursor_w,
                22,
                4,
                Color::from_rgba(255, 255, 255, 8),
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                &cursor_text,
                cursor_x + 7,
                header_base - 1,
                badge_right_x,
                theme.ansi[7],
                10.5,
                false,
            );
            badge_right_x = cursor_x.saturating_sub(8);
        }

        // Badge de tamaño del archivo
        let size_text = if viewer.file_size < 1024 {
            format!("{} B", viewer.file_size)
        } else if viewer.file_size < 1024 * 1024 {
            format!("{:.1} KB", viewer.file_size as f64 / 1024.0)
        } else {
            format!("{:.2} MB", viewer.file_size as f64 / (1024.0 * 1024.0))
        };
        let size_w = self.measure_ui_text(&size_text, 10.5, false) + 14;
        let size_x = badge_right_x.saturating_sub(size_w);
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            size_x,
            y + 8,
            size_w,
            22,
            4,
            Color::from_rgba(255, 255, 255, 8),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &size_text,
            size_x + 7,
            header_base - 1,
            badge_right_x,
            theme.ansi[7],
            10.5,
            false,
        );
        badge_right_x = size_x.saturating_sub(8);

        // Badge de recuento de líneas
        let lines_text = format!("{} líneas", viewer.lines.len());
        let lines_w = self.measure_ui_text(&lines_text, 10.5, false) + 14;
        let lines_x = badge_right_x.saturating_sub(lines_w);
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            lines_x,
            y + 8,
            lines_w,
            22,
            4,
            Color::from_rgba(255, 255, 255, 8),
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &lines_text,
            lines_x + 7,
            header_base - 1,
            badge_right_x,
            theme.ansi[7],
            10.5,
            false,
        );

        // 3. Área de contenido del archivo
        let content_y = y + header_h;
        let content_h = h.saturating_sub(header_h);
        if content_h == 0 {
            return;
        }

        // Si es archivo binario o no visualizable:
        if viewer.is_binary {
            let msg = viewer
                .lines
                .first()
                .map(|s| s.as_str())
                .unwrap_or("Archivo binario");
            let card_w = 420.min(w.saturating_sub(40));
            let card_h = 100;
            let card_x = x + (w.saturating_sub(card_w)) / 2;
            let card_y = content_y + (content_h.saturating_sub(card_h)) / 3;

            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                card_x,
                card_y,
                card_w,
                card_h,
                10,
                theme.surface_bg(),
            );
            crate::ui::stroke_rounded_rect(
                buffer,
                width,
                height,
                card_x,
                card_y,
                card_w,
                card_h,
                10,
                theme.border_color(),
            );

            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Info,
                card_x + 36,
                card_y + 50,
                20.0,
                theme.accent_color(),
            );

            self.draw_ui_text(
                buffer,
                width,
                height,
                "Vista previa no disponible",
                card_x + 64,
                card_y + 42,
                card_x + card_w.saturating_sub(20),
                theme.foreground,
                13.0,
                true,
            );

            self.draw_ui_text(
                buffer,
                width,
                height,
                msg,
                card_x + 64,
                card_y + 64,
                card_x + card_w.saturating_sub(20),
                theme.ansi[8],
                11.0,
                false,
            );
            return;
        }

        let row_h = self.row_height.max(16);
        let char_w = self.char_width.max(7);
        let visible_lines = content_h / row_h;
        let total_lines = viewer.lines.len();

        // Columna de números de línea (gutter)
        let digits = format!("{}", total_lines).len().max(3);
        let gutter_w = digits * char_w + 24;
        let sep_x = x + gutter_w;

        // Fondo sutil del gutter
        fill_rect(
            buffer,
            width,
            clip,
            x,
            content_y,
            gutter_w,
            content_h,
            theme.surface_bg().to_u32_xrgb(),
        );

        // Línea vertical separadora del gutter
        fill_rect(
            buffer,
            width,
            clip,
            sep_x,
            content_y,
            1,
            content_h,
            theme.border_color().to_u32_xrgb(),
        );

        let baseline_offset = ((row_h as f32 * 0.76) as usize).max(char_w);
        let start_line = viewer.scroll_offset;
        let end_line = (start_line + visible_lines + 1).min(total_lines);

        let gutter_clip: Clip = (x, content_y, sep_x, content_y + content_h);
        let text_max_x = (x + w).saturating_sub(18);
        let text_clip: Clip = (sep_x + 1, content_y, text_max_x, content_y + content_h);

        // 4. Renderizado línea por línea
        for line_idx in start_line..end_line {
            let row_idx = line_idx - start_line;
            let row_y = content_y + row_idx * row_h;
            if row_y + row_h > content_y + content_h {
                break;
            }

            let is_cursor_line = !viewer.is_binary && line_idx == viewer.cursor_line;
            let baseline_y = row_y + baseline_offset;

            // Indicador de Git en el gutter si la línea fue modificada o añadida
            if let Some(git_status) = viewer.get_line_git_diff_status(line_idx) {
                let git_color = match git_status {
                    crate::app::GitLineStatus::Added => theme.ansi[2],    // Verde
                    crate::app::GitLineStatus::Modified => theme.ansi[3], // Amarillo
                };
                fill_rect(
                    buffer,
                    width,
                    gutter_clip,
                    sep_x.saturating_sub(3),
                    row_y,
                    3,
                    row_h,
                    git_color.to_u32_xrgb(),
                );
            }

            // Número de línea (alineado a la derecha en el gutter)
            let line_num_str = format!("{}", line_idx + 1);
            let num_w = line_num_str.len() * char_w;
            let num_x = sep_x.saturating_sub(num_w + 10);
            let num_fg = if is_cursor_line {
                theme.foreground
            } else {
                theme.ansi[8]
            };

            for (ci, ch) in line_num_str.chars().enumerate() {
                let glyph = self.get_glyph(ch, false);
                let gx = (num_x + ci * char_w) as i32 + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, gutter_clip, glyph, gx, gy, num_fg);
            }

            // Contenido de la línea de código con resaltado sintáctico completo y desplazamiento horizontal
            let line_text = &viewer.lines[line_idx];
            let start_x_offset = (viewer.scroll_offset_x * char_w) as i32;
            let cur_x = (sep_x + 14) as i32 - start_x_offset;

            self.render_highlighted_line(
                buffer,
                width,
                text_clip,
                line_text,
                cur_x,
                baseline_y,
                text_max_x,
                char_w,
                theme,
                &ext,
            );

            // Renderizado del cursor de texto en la línea activa
            if is_cursor_line {
                let col = viewer.cursor_col;
                let mut cursor_x_offset: i32 = 0;
                for (ci, ch) in line_text.chars().enumerate() {
                    if ci >= col {
                        break;
                    }
                    cursor_x_offset += if ch == '\t' { (char_w * 4) as i32 } else { char_w as i32 };
                }
                let cursor_x = cur_x + cursor_x_offset;
                if cursor_x >= (sep_x + 1) as i32 && cursor_x < text_max_x as i32 {
                    let cursor_w = 2;
                    let cursor_h = (row_h as f32 * 0.85).round() as usize;
                    let cursor_y = row_y + (row_h.saturating_sub(cursor_h)) / 2;

                    fill_rect(
                        buffer,
                        width,
                        text_clip,
                        cursor_x as usize,
                        cursor_y,
                        cursor_w,
                        cursor_h,
                        theme.accent_color().to_u32_xrgb(),
                    );
                }
            }
        }

        // 5. Barra de desplazamiento vertical unificada
        if total_lines > visible_lines {
            let track_w = 6;
            let track_margin = 3;
            let track_x = (x + w).saturating_sub(track_w + track_margin);
            let track_y = content_y + 4;
            let track_h = content_h.saturating_sub(8);
            let min_thumb_h = 18;

            let thumb_h = (((visible_lines as f32) / (total_lines as f32)) * track_h as f32)
                .max(min_thumb_h as f32) as usize;
            let max_scroll = total_lines.saturating_sub(visible_lines);
            let scroll_ratio = if max_scroll > 0 {
                (viewer.scroll_offset as f32) / (max_scroll as f32)
            } else {
                0.0
            };
            let thumb_y = track_y
                + (scroll_ratio * (track_h.saturating_sub(thumb_h)) as f32).round() as usize;

            // Pista sutil
            let track_bg = Color::from_rgba(
                theme.surface_hover().r(),
                theme.surface_hover().g(),
                theme.surface_hover().b(),
                80,
            );
            crate::ui::draw_rounded_rect(
                buffer, width, height, track_x, track_y, track_w, track_h, 3, track_bg,
            );

            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                track_x,
                thumb_y,
                track_w,
                thumb_h,
                3,
                theme.surface_hover(),
            );
        }
    }

    /// Calcula la geometría exacta de la barra de desplazamiento del visor de archivos
    pub fn file_viewer_scrollbar_geometry(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        viewer: &FileViewerState,
        row_height: usize,
    ) -> Option<(Rect, Rect)> {
        let header_h = 38;
        let content_y = y + header_h;
        let content_h = h.saturating_sub(header_h);
        let row_h = row_height.max(16);
        let visible_lines = content_h / row_h;
        let total_lines = viewer.lines.len();

        if total_lines <= visible_lines || visible_lines == 0 {
            return None;
        }

        let track_w = 6;
        let track_margin = 3;
        let track_x = (x + w).saturating_sub(track_w + track_margin);
        let track_y = content_y + 4;
        let track_h = content_h.saturating_sub(8);
        let min_thumb_h = 18;

        let thumb_h = (((visible_lines as f32) / (total_lines as f32)) * track_h as f32)
            .max(min_thumb_h as f32) as usize;
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let travel = track_h.saturating_sub(thumb_h);
        let thumb_y = if max_scroll > 0 {
            track_y + ((viewer.scroll_offset as f32 / max_scroll as f32) * travel as f32) as usize
        } else {
            track_y
        };

        Some((
            (track_x, track_y, track_w, track_h),
            (track_x, thumb_y, track_w, thumb_h),
        ))
    }

    /// Resaltado sintáctico multilenguaje de tokens ligero y de alto rendimiento
    #[allow(clippy::too_many_arguments)]
    fn render_highlighted_line(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        clip: Clip,
        line: &str,
        mut cur_x: i32,
        baseline_y: usize,
        max_x: usize,
        char_w: usize,
        theme: &Theme,
        ext: &str,
    ) {
        let max_x_i32 = max_x as i32;
        let char_w_i32 = char_w as i32;
        let tab_adv_i32 = char_w_i32 * 4;

        let trimmed = line.trim_start();
        // Comentarios completos de línea
        let is_full_comment = match ext {
            "py" | "sh" | "bash" | "ps1" | "toml" | "yaml" | "yml" => trimmed.starts_with('#'),
            "bat" | "cmd" => trimmed.to_ascii_uppercase().starts_with("REM ") || trimmed.starts_with("::"),
            "sql" | "lua" => trimmed.starts_with("--"),
            _ => trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'),
        };

        if is_full_comment {
            let comment_fg = theme.ansi[8];
            for ch in line.chars() {
                if cur_x >= max_x_i32 {
                    break;
                }
                if ch != ' ' && ch != '\t' {
                    let glyph = self.get_glyph(ch, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, comment_fg);
                }
                cur_x += if ch == '\t' { tab_adv_i32 } else { char_w_i32 };
            }
            return;
        }

        // Tokenizador por caracteres
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if cur_x >= max_x_i32 {
                break;
            }

            let ch = chars[i];

            // Tabulaciones: 4 espacios
            if ch == '\t' {
                cur_x += tab_adv_i32;
                i += 1;
                continue;
            }

            // Espacios en blanco
            if ch == ' ' {
                cur_x += char_w_i32;
                i += 1;
                continue;
            }

            // Comentario al final de la línea
            let is_inline_comment = match ext {
                "py" | "sh" | "bash" | "ps1" | "toml" | "yaml" | "yml" => ch == '#',
                "sql" | "lua" => ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-',
                _ => ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/',
            };

            if is_inline_comment {
                let comment_fg = theme.ansi[8];
                while i < chars.len() && cur_x < max_x_i32 {
                    let c = chars[i];
                    if c != ' ' && c != '\t' {
                        let glyph = self.get_glyph(c, false);
                        let gx = cur_x + glyph.x_offset;
                        let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                        blit_glyph(buffer, width, clip, glyph, gx, gy, comment_fg);
                    }
                    cur_x += if c == '\t' { tab_adv_i32 } else { char_w_i32 };
                    i += 1;
                }
                break;
            }

            // Cadenas literales: "...", '...' o `...`
            if ch == '"' || ch == '\'' || ch == '`' {
                let quote = ch;
                let str_fg = theme.ansi[2]; // Verde esmeralda
                let glyph = self.get_glyph(ch, false);
                let gx = cur_x + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, gx, gy, str_fg);
                cur_x += char_w_i32;
                i += 1;

                while i < chars.len() && cur_x < max_x_i32 {
                    let c = chars[i];
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, str_fg);
                    cur_x += char_w_i32;
                    i += 1;
                    if c == quote && (i == 1 || chars[i - 2] != '\\') {
                        break;
                    }
                }
                continue;
            }

            // Atributos, decoradores o directivas (#![...] o @decorator o $variable)
            if ch == '@' || (ch == '$' && matches!(ext, "ps1" | "php" | "sh" | "bash")) {
                let decorator_fg = theme.ansi[5]; // Magenta / Púrpura
                let glyph = self.get_glyph(ch, false);
                let gx = cur_x + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                blit_glyph(buffer, width, clip, glyph, gx, gy, decorator_fg);
                cur_x += char_w_i32;
                i += 1;

                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == ':') {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let c = chars[i];
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, decorator_fg);
                    cur_x += char_w_i32;
                    i += 1;
                }
                continue;
            }

            // Identificadores, palabras clave o llamadas a función
            if ch.is_alphabetic() || ch == '_' {
                let start_idx = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start_idx..i].iter().collect();

                // Verificar si va seguido de paréntesis '(' para colorear como invocación de función
                let mut peek_idx = i;
                while peek_idx < chars.len() && chars[peek_idx] == ' ' {
                    peek_idx += 1;
                }
                let is_func_call = peek_idx < chars.len() && chars[peek_idx] == '(';

                let token_fg = match word.as_str() {
                    // Palabras clave de control, estructura y definición: Acento del tema
                    "fn" | "pub" | "struct" | "enum" | "impl" | "let" | "mut" | "const"
                    | "use" | "mod" | "type" | "return" | "if" | "else" | "match" | "for"
                    | "while" | "loop" | "async" | "await" | "trait" | "where" | "self"
                    | "Self" | "function" | "def" | "class" | "import" | "from" | "export"
                    | "var" | "val" | "interface" | "package" | "go" | "defer" | "select"
                    | "switch" | "case" | "default" | "break" | "continue" | "try" | "catch"
                    | "finally" | "throw" | "throws" | "new" | "delete" | "in" | "is" | "as"
                    | "yield" | "lambda" | "with" | "pass" | "raise" | "assert" | "global"
                    | "nonlocal" | "static" | "public" | "private" | "protected" | "virtual"
                    | "override" | "abstract" | "extends" | "implements" | "namespace"
                    | "using" | "template" | "typename" | "constexpr" | "sizeof"
                    | "param" | "process" | "begin" | "end" => theme.accent_color(),

                    // Booleanos y valores nulos: Amarillo
                    "true" | "false" | "None" | "null" | "nil" | "undefined" => theme.ansi[3],

                    // Tipos primitivos y contenedores estándar: Cian brillante (ANSI 6)
                    "String" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                    | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64"
                    | "bool" | "char" | "Option" | "Result" | "Vec" | "Some"
                    | "Ok" | "Err" | "Box" | "Rc" | "Arc" | "Cell" | "RefCell" | "HashMap"
                    | "HashSet" | "BTreeMap" | "BTreeSet" | "int" | "float" | "dict" | "list"
                    | "set" | "tuple" | "void" | "int8" | "int16" | "int32" | "int64"
                    | "uint8" | "uint16" | "uint32" | "uint64" | "byte" | "rune"
                    | "any" | "unknown" | "never" | "number" | "boolean" | "object" => theme.ansi[6],

                    _ if is_func_call => theme.ansi[4], // Azul moderno para llamadas a función
                    _ if word.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric()) && word.len() > 1 => {
                        theme.ansi[5] // Constantes en mayúsculas: Magenta
                    }
                    _ if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) => {
                        theme.ansi[6] // Clases o Tipos PascalCase: Cian
                    }
                    _ => theme.foreground,
                };

                let is_bold = token_fg == theme.accent_color();
                for &wc in &chars[start_idx..i] {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let glyph = self.get_glyph(wc, is_bold);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, token_fg);
                    cur_x += char_w_i32;
                }
                continue;
            }

            // Números literales (enteros, hexadecimales 0x..., flotantes)
            if ch.is_ascii_digit() {
                let num_fg = theme.ansi[3]; // Amarillo oro
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let c = chars[i];
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    blit_glyph(buffer, width, clip, glyph, gx, gy, num_fg);
                    cur_x += char_w_i32;
                    i += 1;
                }
                continue;
            }

            // Símbolos, delimitadores y operadores
            let symbol_fg = if matches!(ch, '{' | '}' | '(' | ')' | '[' | ']') {
                theme.ansi[7]
            } else if matches!(ch, ':' | ';' | ',' | '.' | '=' | '+' | '-' | '*' | '/' | '&' | '|' | '!' | '?' | '<' | '>' | '%' | '^' | '~') {
                theme.ansi[6]
            } else {
                theme.foreground
            };

            let glyph = self.get_glyph(ch, false);
            let gx = cur_x + glyph.x_offset;
            let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
            blit_glyph(buffer, width, clip, glyph, gx, gy, symbol_fg);
            cur_x += char_w_i32;
            i += 1;
        }
    }
}
