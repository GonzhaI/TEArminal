//! Decodificación, escalado y gestión de fotogramas de imagen y GIF para el visor de archivos.

use std::rc::Rc;

/// Imagen decodificada para el visor de archivos (PNG, JPEG, WebP, GIF, etc.). Una imagen estática tiene
/// un solo fotograma (`delay_ms` 0); un GIF animado, todos los que caben en `MAX_ANIMATION_BYTES`.
#[derive(Debug, Clone)]
pub(crate) struct ViewerImage {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<crate::terminal::ImageFrame>,
    /// Dimensiones del archivo, antes de reducirlo
    pub source_size: (u32, u32),
    /// Fotograma dibujado la última vez: el bucle de eventos solo repinta cuando cambia
    pub shown_frame: std::cell::Cell<usize>,
}

impl ViewerImage {
    /// Lado mayor tras decodificar: acota la RAM y la copia por fotograma de la escena
    const MAX_SIDE: u32 = 2048;
    /// Tope de RAM de los fotogramas de un GIF (los que no caben se descartan: la animación se acorta)
    const MAX_ANIMATION_BYTES: usize = 96 * 1024 * 1024;
    /// Tope de tamaño de un archivo de imagen (el de texto es menor: 5 MB)
    pub(crate) const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
    const EXTENSIONS: [&'static str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "ico"];

    pub(crate) fn has_image_extension(path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| Self::EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
    }

    /// Tamaño máximo de un archivo que se abre en el visor (local o remoto)
    pub(crate) fn max_preview_bytes(path: &std::path::Path) -> u64 {
        if Self::has_image_extension(path) {
            Self::MAX_FILE_BYTES
        } else {
            5 * 1024 * 1024
        }
    }

    pub(crate) fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    /// Índice del fotograma que toca ahora y ms hasta el siguiente (0 en una imagen fija)
    pub(crate) fn current(&self) -> (usize, u32) {
        crate::terminal::current_frame(&self.frames).unwrap_or((0, 0))
    }

    /// Decodifica `bytes` si `path` tiene extensión de imagen; `None` si no lo es o está dañada
    /// (el visor cae entonces al aviso de binario).
    pub(crate) fn decode(path: &std::path::Path, bytes: &[u8]) -> Option<Rc<Self>> {
        if !Self::has_image_extension(path) {
            return None;
        }
        let gif = bytes
            .starts_with(b"GIF8")
            .then(|| Self::gif_frames(bytes))
            .flatten();
        let frames =
            gif.or_else(|| crate::terminal::decode_untrusted(bytes).map(|img| vec![(img, 0)]))?;
        let source_size = frames[0].0.dimensions();
        let frames: Vec<_> = frames
            .into_iter()
            .map(|(img, delay_ms)| crate::terminal::ImageFrame {
                rgba_data: Self::shrink(img).into_raw(),
                delay_ms,
            })
            .collect();
        let (width, height) = Self::shrunk_size(source_size);
        Some(Rc::new(Self {
            width,
            height,
            frames,
            source_size,
            shown_frame: Default::default(),
        }))
    }

    /// Fotogramas (ya compuestos a lienzo completo) con su duración en ms; los de un GIF acaban cuando
    /// se agota `MAX_ANIMATION_BYTES`. `None` si no hay ni un fotograma legible.
    fn gif_frames(bytes: &[u8]) -> Option<Vec<(image::RgbaImage, u32)>> {
        use image::{AnimationDecoder, ImageDecoder};
        let mut decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        decoder
            .set_limits(crate::terminal::untrusted_limits())
            .ok()?;
        let mut frames = Vec::new();
        let mut used = 0usize;
        for frame in decoder.into_frames() {
            let Ok(frame) = frame else { break };
            let (num, den) = frame.delay().numer_denom_ms();
            // Como los navegadores: un GIF sin duración (o de menos de 20 ms) va a 100 ms por fotograma
            let delay_ms = match num.checked_div(den) {
                Some(ms) if ms >= 20 => ms,
                _ => 100,
            };
            let img = frame.into_buffer();
            used += img.as_raw().len();
            if !frames.is_empty() && used > Self::MAX_ANIMATION_BYTES {
                break;
            }
            frames.push((img, delay_ms));
        }
        (!frames.is_empty()).then_some(frames)
    }

    fn shrunk_size((w, h): (u32, u32)) -> (u32, u32) {
        let longest = w.max(h);
        if longest <= Self::MAX_SIDE {
            return (w, h);
        }
        let scale = Self::MAX_SIDE as f32 / longest as f32;
        let scaled = |n: u32| ((n as f32 * scale).round() as u32).max(1);
        (scaled(w), scaled(h))
    }

    fn shrink(img: image::RgbaImage) -> image::RgbaImage {
        let (w, h) = Self::shrunk_size(img.dimensions());
        if (w, h) == img.dimensions() {
            return img;
        }
        image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
    }
}
