//! Soporte de los tests de regresión visual: dibujo de una `Scene` en la GPU (WARP), referencias PNG y firmas,
//! modo `TEARMINAL_BLESS`, fase fija del spinner y un árbol de archivos fijo que sustituye al directorio de trabajo real.

use super::gpu::{DriverKind, GpuDevice, GpuRenderer};
use super::scene::Scene;
use super::Canvas;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_10_0;

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

/// `TEARMINAL_BLESS=1`: los tests de referencias REESCRIBEN los PNG (y `signatures.bin`) con lo que dibuja la GPU en vez
/// de compararlos.
///
/// **Bendecir congela lo que salga como correcto.** Hazlo solo tras mirar las imágenes (`preview_ui_scenes` las
/// vuelca a `%TEMP%`; abre los PNG de `golden/`) y siempre con `-- --test-threads=1`, porque los tests de escenas
/// escriben los mismos archivos. Una referencia que "necesita" cambiar sin un cambio de diseño deliberado es un
/// fallo del dibujo, no de la referencia.
pub(super) fn bless_requested() -> bool {
    std::env::var("TEARMINAL_BLESS").is_ok_and(|v| v == "1")
}

/// Ruta de la referencia PNG de `name` en `golden/<kind>/` (`kind` = `refs` para primitivas, `scenes` para las
/// escenas completas). El nombre se sanea a `[a-z0-9_-]` (sin acentos) para que sea un nombre de archivo válido.
pub(super) fn reference_path(kind: &str, name: &str) -> PathBuf {
    let file: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' => c,
            _ => '_',
        })
        .collect();
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/renderer/golden/"))
        .join(kind)
        .join(format!("{file}.png"))
}

/// Guarda `pixels` (`0x00RRGGBB`, `w` x `h`) como PNG RGB de 8 bits, sin pérdida.
pub(super) fn write_reference(path: &std::path::Path, w: usize, h: usize, pixels: &[u32]) {
    assert_eq!(pixels.len(), w * h, "{}: tamaño", path.display());
    let rgb: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8])
        .collect();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_raw(w as u32, h as u32, rgb)
        .unwrap()
        .save(path)
        .unwrap();
}

/// Lee una referencia PNG: `(píxeles 0x00RRGGBB, ancho, alto)`. Entra en pánico si falta.
pub(super) fn read_reference(path: &std::path::Path) -> (Vec<u32>, usize, usize) {
    let img = image::open(path)
        .unwrap_or_else(|e| {
            panic!(
                "referencia {} no disponible ({e}): genérala con TEARMINAL_BLESS=1 tras revisarla a ojo",
                path.display()
            )
        })
        .to_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let px = img
        .pixels()
        .map(|p| u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2]))
        .collect();
    (px, w, h)
}

/// Diferencia máxima por canal (0-255) que admiten las comparaciones con las referencias PNG: las referencias se
/// congelaron con la mezcla entera truncada de la antigua CPU y la GPU mezcla en coma flotante con redondeo, así
/// que ±1 es normal. Una diferencia mayor se corrige en el shader o en la codificación, nunca subiendo este valor.
pub(super) const PARITY_TOLERANCE: u8 = 2;

/// Tolerancia de escenas COMPLETAS GPU/referencia (mediciones en el documento base, "Etapa 2"). `PARITY_TOLERANCE` solo
/// vale para primitivas con <= 2 capas translúcidas apiladas (el redondeo de la GPU frente al truncado de la CPU
/// suma ~1 por capa), así que en una escena entera el criterio es estadístico: firma, fracción de atípicos y tope.
/// Medido en WARP (FL 11_1 y 10_0), 10 escenas de 1440x900: distancia de firma <= 0,0164; diferencia máxima por
/// canal 2; 0 píxeles por encima de 2; media <= 0,017. Umbrales = medido con margen explícito:
/// distancia de firma: 0,03 (~2x el peor caso medido).
pub(super) const SIGNATURE_TOLERANCE: f32 = 0.03;
/// Fracción máxima de píxeles con diferencia por canal > `PARITY_TOLERANCE`: 0,1 % (medido: 0).
pub(super) const SCENE_OUTLIER_FRACTION: f64 = 0.001;
/// Diferencia por canal que ningún píxel puede superar: medido 2, +1 por una capa translúcida más. Más que esto es
/// un error de shader o de codificación, no de redondeo.
pub(super) const SCENE_MAX_DIFF: u8 = 3;

/// Firmas de `golden/signatures.bin`: `(nombre de escena, firma)` en el orden del archivo.
pub(super) fn read_golden_signatures() -> Vec<(String, Vec<u8>)> {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/renderer/golden/signatures.bin"
    ))
    .expect("falta golden/signatures.bin: generarlo con TEARMINAL_BLESS=1");
    let (mut pos, mut out) = (0, Vec::new());
    while pos < data.len() {
        let n = data[pos] as usize;
        let name = String::from_utf8(data[pos + 1..pos + 1 + n].to_vec()).unwrap();
        pos += 1 + n;
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        out.push((name, data[pos + 4..pos + 4 + len].to_vec()));
        pos += 4 + len;
    }
    out
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

thread_local! {
    /// Un `GpuRenderer` WARP por hilo de test: crear el dispositivo y compilar el pipeline en cada llamada es lento.
    static GPU: RefCell<Option<GpuRenderer>> = const { RefCell::new(None) };
}

/// Dibuja `scene` en la GPU (WARP, FL 10_0 como mínimo) sobre negro y devuelve `0x00RRGGBB` por píxel. El tamaño
/// es el de la escena (si la textura máxima lo recortara, el test debe usar `GpuRenderer::render_to_pixels`).
pub(super) fn render_scene(scene: &Scene) -> Vec<u32> {
    render(scene, None)
}

/// Como `render_scene`, pero sobre `base` (`scene.w * scene.h` píxeles `0x00RRGGBB`) en vez de sobre negro.
pub(super) fn render_scene_over(scene: &Scene, base: &[u32]) -> Vec<u32> {
    render(scene, Some(base))
}

fn render(scene: &Scene, base: Option<&[u32]>) -> Vec<u32> {
    GPU.with(|g| {
        let mut g = g.borrow_mut();
        let r = g.get_or_insert_with(|| {
            let dev = GpuDevice::new_with(Some(DriverKind::Warp), D3D_FEATURE_LEVEL_10_0).unwrap();
            GpuRenderer::new(dev).unwrap()
        });
        let (px, w, h) = match base {
            Some(b) => r.render_to_pixels_over(scene, b),
            None => r.render_to_pixels(scene),
        }
        .unwrap();
        assert_eq!(
            (w, h),
            (scene.w, scene.h),
            "la escena supera la textura máxima"
        );
        px
    })
}

/// Dibuja sobre un lienzo `w` x `h` inicialmente en negro y devuelve los píxeles que pinta la GPU.
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
    let base = PathBuf::from(\"C:\\\\Users\\\\dev\\\\tearminal\");
    base.join(\"src\").join(\"renderer\").join(\"pane.rs\")
}

pub fn sum_cells(cells: &[u32]) -> u64 {
    cells.iter().map(|&c| u64::from(c)).sum()
}
";

/// Árbol fijo `%TEMP%/tearminal-golden/tearminal` (fuera de cualquier repositorio git) que usan las escenas del explorador
/// y del visor. Así el dibujo no depende del directorio de trabajo, del estado de git ni de la máquina.
pub(super) fn fixture_root() -> PathBuf {
    let root = std::env::temp_dir()
        .join("tearminal-golden")
        .join("tearminal");
    let files: [(&str, &str); 4] = [
        ("Cargo.toml", "[package]\nname = \"tearminal\"\n"),
        ("README.md", "# TEArminal\n"),
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
