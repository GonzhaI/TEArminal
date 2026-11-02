//! Fuentes monoespaciadas instaladas en Windows, para el selector de Ajustes.
//!
//! Se leen las carpetas de fuentes del sistema y del usuario y se conservan los `.ttf`/`.otf` regulares
//! (ni negrita ni cursiva) con ancho fijo. Sin COM ni dependencias nuevas: `rustybuzz` ya trae `ttf_parser`.

use rustybuzz::ttf_parser::{name_id, Face};
use std::path::{Path, PathBuf};

/// Una fuente instalada: nombre de familia y archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledFont {
    pub(crate) family: String,
    pub(crate) path: PathBuf,
}

/// Las fuentes de ancho fijo son pequeñas; saltarse las grandes (CJK) evita leer cientos de MB.
const MAX_FONT_BYTES: u64 = 8 * 1024 * 1024;

/// Carpetas donde Windows guarda fuentes (sistema y por usuario).
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    dirs.push(Path::new(&windir).join("Fonts"));
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(Path::new(&local).join("Microsoft/Windows/Fonts"));
    }
    dirs
}

/// Nombre de familia de una fuente regular de ancho fijo, o `None` si no lo es.
fn monospace_family(bytes: &[u8]) -> Option<String> {
    let face = Face::parse(bytes, 0).ok()?;
    if !face.is_monospaced() || face.is_bold() || face.is_italic() || face.is_oblique() {
        return None;
    }
    let find = |id: u16| {
        face.names()
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    };
    find(name_id::TYPOGRAPHIC_FAMILY).or_else(|| find(name_id::FAMILY))
}

/// Recorre las carpetas de fuentes (lento: lee archivos; llamar desde un hilo). Ordenadas por nombre y sin
/// familias repetidas.
pub(crate) fn scan_installed_monospace() -> Vec<InstalledFont> {
    let mut found: Vec<InstalledFont> = Vec::new();
    for dir in font_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(ext.as_deref(), Some("ttf" | "otf")) {
                continue;
            }
            if entry.metadata().map_or(true, |m| m.len() > MAX_FONT_BYTES) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if let Some(family) = monospace_family(&bytes) {
                found.push(InstalledFont { family, path });
            }
        }
    }
    found.sort_by_cached_key(|f| f.family.to_lowercase());
    found.dedup_by(|a, b| a.family.eq_ignore_ascii_case(&b.family));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monospace_detection_on_system_fonts() {
        let fonts = std::path::Path::new(&std::env::var("WINDIR").unwrap_or_default()).join("Fonts");
        // Las fuentes embebidas están reducidas (sin tabla de nombres): se prueba con las del sistema.
        if let Ok(mono) = std::fs::read(fonts.join("consola.ttf")) {
            assert_eq!(monospace_family(&mono).as_deref(), Some("Consolas"));
        }
        let prop = include_bytes!("../assets/fonts/Inter-500.ttf");
        assert!(monospace_family(prop).is_none());
    }

    #[test]
    fn test_scan_lists_sorted_unique_families() {
        let fonts = scan_installed_monospace();
        let names: Vec<_> = fonts.iter().map(|f| f.family.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted);
    }
}
