//! Soporte de los tests de regresión visual (`test_golden_scene_hashes`): hash de fotogramas, modo `AXON_BLESS`,
//! fase fija del spinner y un árbol de archivos fijo que sustituye al directorio de trabajo real.

use super::Canvas;
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

/// Lado (px) de los bloques de la firma.
const SIGNATURE_BLOCK: usize = 48;

/// Firma gruesa del fotograma (`0x00RRGGBB`): media de R, G y B por bloque de 48x48 px, en orden de filas, 3 bytes
/// por bloque. Los bloques del borde derecho/inferior son más pequeños y se promedian con su número real de
/// píxeles. La media se redondea al entero más cercano (mitades hacia arriba).
pub(super) fn signature(buf: &[u32], w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for by in (0..h).step_by(SIGNATURE_BLOCK) {
        for bx in (0..w).step_by(SIGNATURE_BLOCK) {
            let (x1, y1) = ((bx + SIGNATURE_BLOCK).min(w), (by + SIGNATURE_BLOCK).min(h));
            let mut sum = [0u64; 3];
            for y in by..y1 {
                for &p in &buf[y * w + bx..y * w + x1] {
                    sum[0] += u64::from((p >> 16) & 0xFF);
                    sum[1] += u64::from((p >> 8) & 0xFF);
                    sum[2] += u64::from(p & 0xFF);
                }
            }
            let n = ((x1 - bx) * (y1 - by)) as u64;
            out.extend(sum.iter().map(|&s| ((s + n / 2) / n) as u8));
        }
    }
    out
}

/// Diferencia media absoluta por canal (0-255) entre dos firmas. Entra en pánico si sus largos difieren:
/// comparar firmas de tamaños distintos es un error del test, no una diferencia visual.
pub(super) fn signature_distance(a: &[u8], b: &[u8]) -> f32 {
    assert_eq!(a.len(), b.len(), "firmas de distinto tamaño");
    if a.is_empty() {
        return 0.0;
    }
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| u64::from(x.abs_diff(y)))
        .sum();
    sum as f32 / a.len() as f32
}

/// Lienzo `w` x `h` listo para grabar (el fotograma parte de un búfer a cero).
pub(super) fn new_canvas(w: usize, h: usize) -> Canvas {
    let mut canvas = Canvas::new();
    canvas.begin(w as u32, h as u32);
    canvas
}

/// Dibuja sobre un lienzo `w` x `h` inicialmente en cero y devuelve los píxeles rasterizados.
pub(super) fn with_canvas(w: usize, h: usize, f: impl FnOnce(&mut Canvas)) -> Vec<u32> {
    let mut canvas = new_canvas(w, h);
    f(&mut canvas);
    canvas.snapshot()
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
