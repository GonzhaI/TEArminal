//! Extracción de campos sueltos de JSON sin parser: los consumidores solo leen un par de claves
//! de respuestas pequeñas (GitHub Releases, transcripciones, `settings.json`).

/// Valor de la primera clave `"key": "..."` del texto, con las secuencias `\x` reducidas a `x`
pub fn extract_json_str(text: &str, key: &str) -> Option<String> {
    let rest = value_after_key(text, key)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            '"' => return Some(out),
            _ => out.push(c),
        }
    }
    None
}

/// Valor entero sin signo de la primera clave `"key": 123` del texto
pub fn extract_json_u64(text: &str, key: &str) -> Option<u64> {
    let rest = value_after_key(text, key)?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn value_after_key<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{key}\"");
    let idx = text.find(&pattern)?;
    Some(
        text[idx + pattern.len()..]
            .trim_start()
            .strip_prefix(':')?
            .trim_start(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_str() {
        let json = r#"{"tag_name": "v0.8.0", "q": "a\"b", "n": 42}"#;
        assert_eq!(
            extract_json_str(json, "tag_name").as_deref(),
            Some("v0.8.0")
        );
        assert_eq!(extract_json_str(json, "q").as_deref(), Some("a\"b"));
        assert_eq!(extract_json_str(json, "non_existent"), None);
        assert_eq!(extract_json_str(json, "n"), None);
    }

    #[test]
    fn test_extract_json_u64() {
        let json = r#"{"input_tokens": 1234, "s": "x"}"#;
        assert_eq!(extract_json_u64(json, "input_tokens"), Some(1234));
        assert_eq!(extract_json_u64(json, "s"), None);
    }
}
