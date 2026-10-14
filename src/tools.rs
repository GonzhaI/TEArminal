//! Herramientas CLI opcionales (eza, bat, rg, fd, zoxide, fzf) distribuidas junto a Axon en `bin\`.
//! Axon no descarga nada en ejecución: el ZIP/instalador las trae y `assets/shell/init.ps1` las usa
//! si están en PATH (con alternativas propias cuando faltan).
use std::path::{Path, PathBuf};

/// `bin\` junto al ejecutable (ZIP portable o carpeta de instalación), si existe.
pub fn bundled_bin_dir() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("bin");
    dir.is_dir().then_some(dir)
}

/// PATH para los procesos hijos. `bin\` de Axon va AL FINAL: las versiones que el usuario ya tiene
/// instaladas tienen prioridad y las incluidas sirven de respaldo.
pub fn path_with_bundled_tools() -> String {
    append_dir(
        &std::env::var("PATH").unwrap_or_default(),
        bundled_bin_dir().as_deref(),
    )
}

fn append_dir(path: &str, dir: Option<&Path>) -> String {
    match dir {
        Some(d) if path.is_empty() => d.display().to_string(),
        Some(d) => format!("{};{}", path.trim_end_matches(';'), d.display()),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_tools_go_last_and_user_path_is_untouched() {
        let bin = Path::new(r"C:\Axon\bin");
        assert_eq!(
            append_dir(r"C:\a;C:\b", Some(bin)),
            r"C:\a;C:\b;C:\Axon\bin"
        );
        assert_eq!(append_dir(r"C:\a;", Some(bin)), r"C:\a;C:\Axon\bin");
        assert_eq!(append_dir("", Some(bin)), r"C:\Axon\bin");
        assert_eq!(append_dir(r"C:\a", None), r"C:\a");
    }
}
