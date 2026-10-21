//! Soporte de los tests de regresión visual (`test_golden_scene_hashes`): hash de fotogramas, modo `AXON_BLESS`,
//! fase fija del spinner y un árbol de archivos fijo que sustituye al directorio de trabajo real.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    /// Fase fija del spinner de pestañas para las escenas (`None` = reloj real, como en la app).
    static SPINNER_PHASE: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(super) fn set_spinner_phase(phase: Option<usize>) {
    SPINNER_PHASE.with(|c| c.set(phase));
}

pub(super) fn spinner_phase_override() -> Option<usize> {
    SPINNER_PHASE.with(|c| c.get())
}

/// FNV-1a de 64 bits sobre los bytes (little-endian) de cada `u32` del framebuffer, en orden.
pub(super) fn frame_hash(buf: &[u32]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for px in buf {
        for byte in px.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}

/// `AXON_BLESS=1`: el test de hashes reescribe `golden/hashes.txt` en vez de compararlo.
pub(super) fn bless_requested() -> bool {
    std::env::var("AXON_BLESS").is_ok_and(|v| v == "1")
}

const PANE_RS: &str = "\
//! Fixture fijo del visor de archivos (no es el código real).

use std::path::PathBuf;

/// Ruta de ejemplo para la escena del visor.
pub fn example_path() -> PathBuf {
    let base = PathBuf::from(\"C:\\\\Users\\\\dev\\\\axon\");
    base.join(\"src\").join(\"renderer\").join(\"pane.rs\")
}

pub fn sum_cells(cells: &[u32]) -> u64 {
    cells.iter().map(|&c| u64::from(c)).sum()
}
";

/// Árbol fijo `%TEMP%/axon-golden/axon` (fuera de cualquier repositorio git) que usan las escenas del explorador
/// y del visor. Así el dibujo no depende del directorio de trabajo, del estado de git ni de la máquina.
pub(super) fn fixture_root() -> PathBuf {
    let root = std::env::temp_dir().join("axon-golden").join("axon");
    let files: [(&str, &str); 4] = [
        ("Cargo.toml", "[package]\nname = \"axon\"\n"),
        ("README.md", "# Axon\n"),
        ("src/main.rs", "fn main() {}\n"),
        ("src/renderer/pane.rs", PANE_RS),
    ];
    for (rel, content) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("ruta con carpeta padre")).unwrap();
        std::fs::write(&path, content).unwrap();
    }
    root
}
