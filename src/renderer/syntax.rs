//! Resaltado sintáctico multilenguaje de alto rendimiento para el visor de archivos.

use std::rc::Rc;

use super::*;
use crate::theme::Theme;

/// Especificación sintáctica ligera de un lenguaje para delimitación de bloques de comentarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LanguageSpec {
    pub(crate) block_comment: Option<(&'static str, &'static str)>,
}

/// Identifica delimitadores de comentarios de bloque según la extensión del archivo.
pub(crate) fn language_spec(ext: &str) -> LanguageSpec {
    let block_comment = match ext {
        "rs" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "cs" | "java" | "js" | "jsx" | "mjs"
        | "cjs" | "ts" | "tsx" | "go" | "swift" | "kt" | "kts" | "css" | "scss" | "sass"
        | "less" | "php" | "dart" | "scala" | "zig" | "sql" => Some(("/*", "*/")),
        "html" | "htm" | "xml" | "svg" | "vue" | "svelte" => Some(("<!--", "-->")),
        "ps1" | "psm1" | "psd1" => Some(("<#", "#>")),
        "lua" => Some(("--[[", "]]")),
        "hs" | "lhs" => Some(("{-", "-}")),
        "py" | "pyw" => Some(("\"\"\"", "\"\"\"")),
        _ => None,
    };
    LanguageSpec { block_comment }
}

impl TerminalRenderer {
    /// Obtiene o calcula los bloques de comentario multilínea de un archivo en caché.
    pub(crate) fn get_syntax_multiline_blocks(
        &mut self,
        path: &std::path::Path,
        gen: u64,
        lines: &[String],
        spec: LanguageSpec,
    ) -> Rc<[bool]> {
        if let Some((ref cached_path, cached_gen, ref cached_blocks)) = self.syntax_blocks_cache {
            if cached_gen == gen && cached_path == path {
                return cached_blocks.clone();
            }
        }

        let (open_tag, close_tag) = match spec.block_comment {
            Some(tags) => tags,
            None => {
                let empty: Rc<[bool]> = Rc::from(vec![false; lines.len()].into_boxed_slice());
                return empty;
            }
        };

        let mut in_block = false;
        let mut blocks = Vec::with_capacity(lines.len());

        for line in lines {
            let trimmed = line.trim();
            if in_block {
                blocks.push(true);
                if trimmed.contains(close_tag) {
                    in_block = false;
                }
            } else if let Some(open_pos) = trimmed.find(open_tag) {
                let rest = &trimmed[open_pos + open_tag.len()..];
                if rest.contains(close_tag) {
                    // Abierto y cerrado en la misma línea: no se propaga a líneas posteriores
                    blocks.push(trimmed.starts_with(open_tag));
                } else {
                    // Abierto en esta línea y continúa en las siguientes
                    in_block = true;
                    blocks.push(trimmed.starts_with(open_tag));
                }
            } else {
                blocks.push(false);
            }
        }

        let rc: Rc<[bool]> = Rc::from(blocks.into_boxed_slice());
        self.syntax_blocks_cache = Some((path.to_path_buf(), gen, rc.clone()));
        rc
    }

    /// Resaltado sintáctico multilenguaje de tokens ligero y de alto rendimiento
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_highlighted_line(
        &mut self,
        canvas: &mut Canvas,
        clip: Clip,
        line: &str,
        mut cur_x: i32,
        baseline_y: usize,
        max_x: usize,
        char_w: usize,
        theme: &Theme,
        ext: &str,
        in_block_comment: bool,
    ) {
        let max_x_i32 = max_x as i32;
        let char_w_i32 = char_w as i32;
        let tab_adv_i32 = char_w_i32 * 4;

        // Texto plano sin formato (bloques ```text o archivos .txt): sin coloreado sintáctico
        if matches!(ext, "txt" | "text" | "plaintext" | "") {
            for ch in line.chars() {
                if cur_x >= max_x_i32 {
                    break;
                }
                if ch != ' ' && ch != '\t' {
                    let glyph = self.get_glyph(ch, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, theme.foreground);
                }
                cur_x += if ch == '\t' { tab_adv_i32 } else { char_w_i32 };
            }
            return;
        }

        let trimmed = line.trim_start();
        let is_config_lang = matches!(ext, "toml" | "ini" | "cfg");
        let is_kebab_lang = is_config_lang
            || matches!(
                ext,
                "yaml" | "yml" | "css" | "scss" | "html" | "xml" | "vue" | "svelte"
            );

        // Cabeceras de sección/tabla en TOML/INI: [tabla], [[array_de_tablas]]
        let is_toml_table_header = is_config_lang && trimmed.starts_with('[') && {
            let after_bracket = trimmed.trim_start_matches('[');
            if let Some(close_bracket_idx) = after_bracket.rfind(']') {
                let rest = after_bracket[close_bracket_idx + 1..].trim_start();
                rest.is_empty() || rest.starts_with('#') || rest.starts_with(';')
            } else {
                false
            }
        };

        // Comentarios completos de línea
        let is_full_comment = in_block_comment
            || match ext {
                "py" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "toml" | "yaml" | "yml" | "rb"
                | "r" | "jl" | "dockerfile" | "makefile" | "mk" => trimmed.starts_with('#'),
                "bat" | "cmd" => {
                    trimmed.to_ascii_uppercase().starts_with("REM ") || trimmed.starts_with("::")
                }
                "sql" | "lua" | "hs" | "elm" | "vhd" | "vhdl" => trimmed.starts_with("--"),
                "html" | "xml" | "svg" | "vue" | "svelte" => trimmed.starts_with("<!--"),
                "ini" | "cfg" | "conf" | "properties" => {
                    trimmed.starts_with(';') || trimmed.starts_with('#')
                }
                _ => {
                    trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                }
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
                    canvas.glyph(clip, glyph, gx, gy, comment_fg);
                }
                cur_x += if ch == '\t' { tab_adv_i32 } else { char_w_i32 };
            }
            return;
        }

        // Tokenizador sin alojamientos sobre la cadena UTF-8
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        #[inline(always)]
        fn decode_at(line: &str, bytes: &[u8], i: usize) -> (char, usize) {
            let b = bytes[i];
            if b < 128 {
                (b as char, i + 1)
            } else {
                let c = line
                    .get(i..)
                    .and_then(|s| s.chars().next())
                    .unwrap_or('\u{FFFD}');
                (c, (i + c.len_utf8()).min(bytes.len()))
            }
        }

        while i < len {
            if cur_x >= max_x_i32 {
                break;
            }

            let (ch, next_i) = decode_at(line, bytes, i);

            // Tabulaciones: 4 espacios
            if ch == '\t' {
                cur_x += tab_adv_i32;
                i = next_i;
                continue;
            }

            // Espacios en blanco
            if ch == ' ' {
                cur_x += char_w_i32;
                i = next_i;
                continue;
            }

            // Comentario al final de la línea
            let is_inline_comment = match ext {
                "py" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "toml" | "yaml" | "yml" | "rb"
                | "r" | "jl" | "dockerfile" | "makefile" | "mk" => ch == '#',
                "sql" | "lua" | "hs" | "elm" => line[i..].starts_with("--"),
                "html" | "xml" | "svg" | "vue" | "svelte" => line[i..].starts_with("<!--"),
                "ini" | "cfg" | "conf" | "properties" => ch == ';' || ch == '#',
                _ => line[i..].starts_with("//"),
            };

            if is_inline_comment {
                let comment_fg = theme.ansi[8];
                for c in line[i..].chars() {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    if c != ' ' && c != '\t' {
                        let glyph = self.get_glyph(c, false);
                        let gx = cur_x + glyph.x_offset;
                        let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                        canvas.glyph(clip, glyph, gx, gy, comment_fg);
                    }
                    cur_x += if c == '\t' { tab_adv_i32 } else { char_w_i32 };
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
                canvas.glyph(clip, glyph, gx, gy, str_fg);
                cur_x += char_w_i32;
                i = next_i;

                let mut escaped = false;
                while i < len && cur_x < max_x_i32 {
                    let (c, ni) = decode_at(line, bytes, i);
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, str_fg);
                    cur_x += char_w_i32;
                    i = ni;
                    if c == quote && !escaped {
                        break;
                    }
                    if c == '\\' {
                        escaped = !escaped;
                    } else {
                        escaped = false;
                    }
                }
                continue;
            }

            // Atributos, decoradores o directivas (#![...], #include, @decorator, $variable)
            if ch == '@'
                || (ch == '$'
                    && matches!(
                        ext,
                        "ps1" | "php" | "sh" | "bash" | "zsh" | "fish" | "pl" | "rb"
                    ))
                || (ch == '#' && matches!(ext, "c" | "cpp" | "h" | "hpp" | "cs" | "rs"))
            {
                let decorator_fg = theme.ansi[5]; // Magenta / Púrpura
                let glyph = self.get_glyph(ch, false);
                let gx = cur_x + glyph.x_offset;
                let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                canvas.glyph(clip, glyph, gx, gy, decorator_fg);
                cur_x += char_w_i32;
                i = next_i;

                while i < len {
                    let (c, ni) = decode_at(line, bytes, i);
                    if !(c.is_alphanumeric() || c == '_' || c == ':') {
                        break;
                    }
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, decorator_fg);
                    cur_x += char_w_i32;
                    i = ni;
                }
                continue;
            }

            // Identificadores, palabras clave o llamadas a función
            if ch.is_alphabetic() || ch == '_' {
                let start_idx = i;
                i = next_i;
                while i < len {
                    let (c, ni) = decode_at(line, bytes, i);
                    if c.is_alphanumeric() || c == '_' {
                        i = ni;
                    } else if is_kebab_lang && c == '-' {
                        // Consumir el guión solo si va seguido de un carácter alfanumérico o '_'
                        // para evitar consumir operadores de resta como 'a - b'
                        let next_byte = bytes.get(ni).copied();
                        if next_byte.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_') {
                            i = ni;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                let word = &line[start_idx..i];

                // Verificar si va seguido de paréntesis '(' para colorear como invocación de función
                let is_func_call = line[i..].trim_start_matches(' ').starts_with('(');
                let is_yaml_key = matches!(ext, "yaml" | "yml")
                    && line[i..].trim_start_matches(' ').starts_with(':');

                let token_fg = if is_toml_table_header {
                    theme.ansi[5] // En cabeceras de tabla [profile.default], todo identificador pertenece a la cabecera
                } else if is_config_lang {
                    match word {
                        // Booleanos y valores nulos: Amarillo
                        "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "None"
                        | "null" | "nil" | "undefined" | "nan" | "inf" => theme.ansi[3],
                        // En TOML/INI, cualquier otro identificador es una clave de configuración
                        _ => theme.ansi[4],
                    }
                } else {
                    match word {
                        // Palabras clave de control, estructura y definición: Acento del tema
                        "fn" | "pub" | "struct" | "enum" | "impl" | "let" | "mut" | "const"
                        | "use" | "mod" | "type" | "return" | "if" | "else" | "elif" | "elsif"
                        | "match" | "for" | "while" | "loop" | "async" | "await" | "trait"
                        | "where" | "self" | "Self" | "function" | "def" | "class" | "import"
                        | "from" | "export" | "var" | "val" | "interface" | "package" | "go"
                        | "defer" | "errdefer" | "select" | "switch" | "case" | "default"
                        | "break" | "continue" | "try" | "catch" | "except" | "finally"
                        | "throw" | "throws" | "new" | "delete" | "in" | "is" | "as" | "yield"
                        | "lambda" | "with" | "pass" | "raise" | "assert" | "global"
                        | "nonlocal" | "static" | "public" | "private" | "protected"
                        | "virtual" | "override" | "abstract" | "extends" | "implements"
                        | "namespace" | "using" | "template" | "typename" | "constexpr"
                        | "sizeof" | "param" | "process" | "begin" | "end" | "unsafe" | "dyn"
                        | "ref" | "move" | "fun" | "guard" | "when" | "protocol" | "extension"
                        | "mutating" | "comptime" | "unreachable" | "echo" | "source" | "alias"
                        | "local" | "then" | "fi" | "do" | "done" | "esac" | "SELECT" | "FROM"
                        | "WHERE" | "INSERT" | "INTO" | "VALUES" | "UPDATE" | "SET" | "DELETE"
                        | "CREATE" | "TABLE" | "DROP" | "ALTER" | "INDEX" | "JOIN" | "INNER"
                        | "LEFT" | "RIGHT" | "OUTER" | "GROUP" | "BY" | "HAVING" | "ORDER"
                        | "LIMIT" | "OFFSET" | "UNION" | "ALL" | "DISTINCT" | "PRIMARY" | "KEY"
                        | "FOREIGN" | "REFERENCES" | "CONSTRAINT" | "DATABASE" | "SCHEMA"
                        | "VIEW" | "TRIGGER" | "RUN" | "CMD" | "LABEL" | "EXPOSE" | "ENV"
                        | "ADD" | "COPY" | "ENTRYPOINT" | "VOLUME" | "USER" | "WORKDIR" | "ARG"
                        | "ONBUILD" => theme.accent_color(),

                        // Booleanos y valores nulos: Amarillo
                        "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "None"
                        | "null" | "nil" | "undefined" | "NULL" | "nullptr" | "NaN"
                        | "Infinity" | "nan" | "inf" => theme.ansi[3],

                        // Tipos primitivos y contenedores estándar: Cian brillante (ANSI 6)
                        "String" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                        | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64"
                        | "bool" | "char" | "Option" | "Result" | "Vec" | "Some" | "Ok" | "Err"
                        | "Box" | "Rc" | "Arc" | "Cell" | "RefCell" | "Mutex" | "RwLock"
                        | "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "Map" | "Set"
                        | "Array" | "List" | "int" | "float" | "double" | "dict" | "list"
                        | "set" | "tuple" | "void" | "int8" | "int16" | "int32" | "int64"
                        | "uint8" | "uint16" | "uint32" | "uint64" | "byte" | "rune" | "any"
                        | "unknown" | "never" | "number" | "boolean" | "object" | "bigint"
                        | "symbol" | "this" => theme.ansi[6],

                        _ if is_yaml_key => theme.ansi[4], // Claves YAML en azul
                        _ if is_func_call => theme.ansi[4], // Azul moderno para llamadas a función
                        _ if word
                            .chars()
                            .all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
                            && word.chars().nth(1).is_some() =>
                        {
                            theme.ansi[5] // Constantes en mayúsculas: Magenta
                        }
                        _ if ch.is_uppercase() => {
                            theme.ansi[6] // Clases o Tipos PascalCase: Cian
                        }
                        _ => theme.foreground,
                    }
                };

                let is_bold = token_fg == theme.accent_color() || is_toml_table_header;
                for wc in word.chars() {
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let glyph = self.get_glyph(wc, is_bold);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, token_fg);
                    cur_x += char_w_i32;
                }
                continue;
            }

            // Números literales (enteros, hexadecimales 0x..., flotantes)
            if ch.is_ascii_digit() {
                let num_fg = theme.ansi[3]; // Amarillo oro
                while i < len {
                    let b = bytes[i];
                    if !(b.is_ascii_alphanumeric() || b == b'.' || b == b'_') {
                        break;
                    }
                    if cur_x >= max_x_i32 {
                        break;
                    }
                    let c = b as char;
                    let glyph = self.get_glyph(c, false);
                    let gx = cur_x + glyph.x_offset;
                    let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                    canvas.glyph(clip, glyph, gx, gy, num_fg);
                    cur_x += char_w_i32;
                    i += 1;
                }
                continue;
            }

            // Símbolos, delimitadores y operadores
            if self.ligatures && is_ligature_candidate(ch) {
                let mut op_buf = [0u8; 8];
                op_buf[0] = ch as u8;
                let mut run_len = 1usize;
                let mut peek_i = next_i;
                while run_len < 8 && peek_i < len {
                    let b = bytes[peek_i];
                    if b < 128 && is_ligature_candidate(b as char) {
                        op_buf[run_len] = b;
                        run_len += 1;
                        peek_i += 1;
                    } else {
                        break;
                    }
                }

                if run_len >= 2 {
                    let shaped = self.shape_punctuation_run(&op_buf[..run_len], false);
                    let symbol_fg = theme.ansi[6];
                    for &gid in &shaped.gids[..run_len] {
                        if cur_x >= max_x_i32 {
                            break;
                        }
                        let glyph = self.get_ligature_glyph(gid, false);
                        if glyph.width > 0 {
                            let gx = cur_x + glyph.x_offset;
                            let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
                            canvas.glyph(clip, glyph, gx, gy, symbol_fg);
                        }
                        cur_x += char_w_i32;
                    }
                    i = peek_i;
                    continue;
                }
            }

            let is_header_symbol = is_toml_table_header && (ch == '[' || ch == ']' || ch == '.');
            let symbol_fg = if is_header_symbol {
                theme.ansi[5] // Magenta para delimitadores y jerarquía de sección en TOML/INI
            } else if matches!(ch, '{' | '}' | '(' | ')' | '[' | ']') {
                theme.ansi[7]
            } else if matches!(
                ch,
                ':' | ';'
                    | ','
                    | '.'
                    | '='
                    | '+'
                    | '-'
                    | '*'
                    | '/'
                    | '&'
                    | '|'
                    | '!'
                    | '?'
                    | '<'
                    | '>'
                    | '%'
                    | '^'
                    | '~'
            ) {
                theme.ansi[6]
            } else {
                theme.foreground
            };

            let glyph = self.get_glyph(ch, is_header_symbol);
            let gx = cur_x + glyph.x_offset;
            let gy = baseline_y as i32 - glyph.y_offset - glyph.height as i32;
            canvas.glyph(clip, glyph, gx, gy, symbol_fg);
            cur_x += char_w_i32;
            i = next_i;
        }
    }
}
