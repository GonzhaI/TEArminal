//! Todo lo que llega del servidor de base de datos (nombres de tablas y columnas, valores, mensajes de error) es
//! entrada no confiable: puede traer secuencias de escape de terminal, saltos de línea o caracteres bidireccionales
//! que falsean lo que ve el usuario. `display` los convierte en texto visible (`\x1b`, `\u{202e}`…).

use std::borrow::Cow;
use std::fmt::Write;

/// Caracteres sin control ASCII que también engañan al ojo: marcas bidireccionales, anchura cero, separadores de línea
fn is_deceptive(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}'
    )
}

fn needs_escape(c: char) -> bool {
    c.is_control() || is_deceptive(c)
}

/// Texto seguro para mostrar en una sola línea. Devuelve el original (sin copiar) si no hay nada que escapar.
pub(crate) fn display(input: &str) -> Cow<'_, str> {
    if !input.chars().any(needs_escape) {
        return Cow::Borrowed(input);
    }
    let mut out = String::with_capacity(input.len() + 8);
    for c in input.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if needs_escape(c) && (c as u32) < 0x100 => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c if needs_escape(c) => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// `true` si `display` cambiaría el texto (la interfaz lo marca como «escapado»)
pub(crate) fn has_escapes(input: &str) -> bool {
    input.chars().any(needs_escape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_borrowed() {
        assert!(matches!(display("users_2024 ñandú"), Cow::Borrowed(_)));
        assert!(!has_escapes("users"));
    }

    #[test]
    fn terminal_escape_sequences_become_visible_text() {
        assert_eq!(display("report\x1b[31m.txt"), "report\\x1b[31m.txt");
        assert_eq!(display("a\nb\tc\r"), "a\\nb\\tc\\r");
        assert_eq!(display("nul\0"), "nul\\x00");
    }

    #[test]
    fn bidi_and_zero_width_characters_are_escaped() {
        assert_eq!(display("fdp\u{202e}gpj"), "fdp\\u{202e}gpj");
        assert_eq!(display("a\u{200b}b"), "a\\u{200b}b");
        assert_eq!(display("\u{9b}31m"), "\\x9b31m"); // CSI de 8 bits
        assert!(has_escapes("x\u{2066}"));
    }

    #[test]
    fn escaped_output_has_no_control_characters() {
        let hostile = "\x07\x1b]0;titulo\x07\u{85}\u{2028}fin";
        assert!(!display(hostile).chars().any(needs_escape));
    }
}
