//! Soporte para el protocolo Kitty Graphics (secuencias APC `\x1b_G...`).
//!
//! Permite que herramientas CLI (`timg`, `chafa`, `icat`, etc.) transmitan y muestren
//! imágenes en la terminal en formatos PNG/comprimidos (`f=100`) o mapa de bits sin
//! procesar (RGB de 24 bits `f=24` y RGBA de 32 bits `f=32`), con soporte para fragmentación
//! (`m=1`/`m=0`), consultas de capacidades (`a=q`) y reutilización/borrado (`a=p`, `a=d`).

use super::{InlineImage, TerminalState};
use base64::prelude::*;
use image::imageops::FilterType;
use image::RgbaImage;

/// Parámetros de control decodificados de una secuencia Kitty Graphics
#[derive(Debug, Clone, Default)]
pub struct KittyControl {
    /// Acción (`a`):
    /// - 't': transmitir datos sin mostrar
    /// - 'T': transmitir y mostrar (predeterminado si hay datos y no se especifica)
    /// - 'p': mostrar / colocar imagen existente (especificada con `i`)
    /// - 'd': borrar imágenes
    /// - 'q': consultar soporte del protocolo
    pub action: char,
    /// Formato (`f`):
    /// - 100: PNG / compresión (autodetecta con `image`)
    /// - 32: RGBA raw de 32 bits
    /// - 24: RGB raw de 24 bits
    pub format: u32,
    /// Ancho en píxeles de los datos en bruto (`s`)
    pub src_width: u32,
    /// Alto en píxeles de los datos en bruto (`v`)
    pub src_height: u32,
    /// Columnas de terminal deseadas (`c`)
    pub cols: Option<usize>,
    /// Filas de terminal deseadas (`r`)
    pub rows: Option<usize>,
    /// Identificador numérico de la imagen (`i` o `I`)
    pub image_id: u64,
    /// Hay más fragmentos (`m=1`) o es el último/único (`m=0`)
    pub more_chunks: bool,
    /// Nivel de silencio (`q`): 0 = normal, 1 = silenciar OK, 2 = silenciar todo
    pub quiet: u8,
    /// Acción de borrado (`d`): 'a'/'A' = todas, 'i'/'I' = por ID, 'p'/'P' = en cursor
    pub delete_action: Option<char>,
    /// Movimiento del cursor (`C`): 0 = avanzar cursor (predeterminado), 1 = no mover
    pub cursor_movement: u8,
}

/// Estado de acumulación de fragmentos de imagen (`m=1`)
#[derive(Debug, Clone)]
pub struct KittyChunkState {
    pub control: KittyControl,
    pub data: Vec<u8>,
}

/// Decodifica una imagen que llega por la salida de la terminal (Kitty, iTerm2): entrada NO confiable, así que
/// con tope de dimensiones y de memoria (un PNG diminuto puede declarar miles de millones de píxeles)
pub(super) fn decode_untrusted(bytes: &[u8]) -> Option<RgbaImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().ok().map(|img| img.to_rgba8())
}

/// Reduce `img` a `tw` x `th` píxeles si es mayor (acota la RAM de las imágenes que viven en el historial)
pub(super) fn fit_image(img: RgbaImage, tw: u32, th: u32) -> RgbaImage {
    if img.width() > tw || img.height() > th {
        image::imageops::resize(&img, tw, th, FilterType::Triangle)
    } else {
        img
    }
}

/// Respuesta de error del protocolo (`ESC _ G i=<id>;<código>:<texto> ESC \`) para quien espera confirmación;
/// `q=2` la silencia y sin id no hay a quién responder
fn send_error(state: &mut TerminalState, control: &KittyControl, msg: &str) {
    if control.image_id > 0 && control.quiet < 2 {
        let resp = format!("\x1b_Gi={};{msg}\x1b\\", control.image_id);
        state.pty_write_queue.push(resp.into_bytes());
    }
}

/// Parsea los pares clave=valor del encabezado de control de Kitty
pub fn parse_kitty_control(bytes: &[u8]) -> KittyControl {
    let mut ctrl = KittyControl::default();
    let text = String::from_utf8_lossy(bytes);
    for item in text.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (key, val) = match item.find('=') {
            Some(pos) => (item[..pos].trim(), item[pos + 1..].trim()),
            None => (item, ""),
        };
        match key {
            "a" | "A" => {
                if let Some(ch) = val.chars().next() {
                    ctrl.action = ch;
                }
            }
            "f" | "F" => {
                if val.eq_ignore_ascii_case("png") || val == "100" {
                    ctrl.format = 100;
                } else if val == "32" {
                    ctrl.format = 32;
                } else if val == "24" {
                    ctrl.format = 24;
                } else if let Ok(n) = val.parse::<u32>() {
                    ctrl.format = n;
                }
            }
            // `S` es el tamaño de los datos (medios por archivo, no soportados), no el ancho
            "s" => {
                if let Ok(n) = val.parse::<u32>() {
                    ctrl.src_width = n;
                }
            }
            "v" => {
                if let Ok(n) = val.parse::<u32>() {
                    ctrl.src_height = n;
                }
            }
            "c" => {
                if let Ok(n) = val.parse::<usize>() {
                    ctrl.cols = Some(n);
                }
            }
            "r" | "R" => {
                if let Ok(n) = val.parse::<usize>() {
                    ctrl.rows = Some(n);
                }
            }
            "i" | "I" => {
                if let Ok(n) = val.parse::<u64>() {
                    ctrl.image_id = n;
                }
            }
            "m" | "M" => {
                ctrl.more_chunks = val == "1";
            }
            "q" | "Q" => {
                if let Ok(n) = val.parse::<u8>() {
                    ctrl.quiet = n;
                }
            }
            "d" | "D" => {
                if let Some(ch) = val.chars().next() {
                    ctrl.delete_action = Some(ch);
                }
            }
            "C" => {
                if let Ok(n) = val.parse::<u8>() {
                    ctrl.cursor_movement = n;
                }
            }
            _ => {}
        }
    }

    if ctrl.action == '\0' {
        if ctrl.delete_action.is_some() {
            ctrl.action = 'd';
        } else {
            ctrl.action = 'T';
        }
    }

    ctrl
}

/// Procesa una secuencia Kitty Graphics completa recibida en APC (`\x1b_G...`)
pub fn process_kitty_apc(state: &mut TerminalState, payload: &[u8]) {
    // 1. Separar control y carga base64 (separador ';')
    let (control_bytes, payload_bytes) = if let Some(pos) = payload.iter().position(|&b| b == b';')
    {
        (&payload[..pos], &payload[pos + 1..])
    } else {
        (payload, &[][..])
    };

    let mut control = parse_kitty_control(control_bytes);

    // 2. Manejo de consultas de capacidades (a=q)
    if control.action == 'q' || control.action == 'Q' {
        if control.quiet < 2 {
            let resp = format!("\x1b_Gi={};OK\x1b\\", control.image_id);
            state.pty_write_queue.push(resp.into_bytes());
        }
        return;
    }

    // 3. Manejo de borrado de imágenes (a=d)
    if control.action == 'd' || control.action == 'D' {
        match control.delete_action {
            Some('a') | Some('A') => {
                state.images.clear();
                state.kitty_stored_images.clear();
            }
            Some('i') | Some('I') => {
                if control.image_id > 0 {
                    state.images.retain(|img| img.id != control.image_id);
                    state.kitty_stored_images.remove(&control.image_id);
                }
            }
            _ => {
                if control.image_id > 0 {
                    state.images.retain(|img| img.id != control.image_id);
                    state.kitty_stored_images.remove(&control.image_id);
                }
            }
        }
        if control.image_id > 0 && control.quiet == 0 {
            let resp = format!("\x1b_Gi={};OK\x1b\\", control.image_id);
            state.pty_write_queue.push(resp.into_bytes());
        }
        return;
    }

    // 4. Manejo de colocación de imagen pretransmitida (a=p)
    if control.action == 'p' || control.action == 'P' {
        if let Some(stored) = state.kitty_stored_images.get(&control.image_id).cloned() {
            let mut img = stored;
            img.line_index = state.current_global_line();
            img.col = state.cursor_x;
            if let Some(c) = control.cols {
                img.width_cols = c.clamp(1, state.cols);
            }
            if let Some(r) = control.rows {
                img.height_rows = r.clamp(1, 100);
            }
            let rows = img.height_rows;
            state.push_image(img);
            if control.cursor_movement != 1 {
                for _ in 0..rows {
                    state.newline();
                }
            }
            if control.image_id > 0 && control.quiet == 0 {
                let resp = format!("\x1b_Gi={};OK\x1b\\", control.image_id);
                state.pty_write_queue.push(resp.into_bytes());
            }
        } else if control.image_id > 0 && control.quiet == 0 {
            let resp = format!("\x1b_Gi={};ENOENT\x1b\\", control.image_id);
            state.pty_write_queue.push(resp.into_bytes());
        }
        return;
    }

    // 5. Ensamblaje de fragmentos (m=1 / m=0) para transmisión de datos (a=T / a=t)
    const MAX_KITTY_PAYLOAD: usize = 64 * 1024 * 1024; // Límite de 64 MB
    let full_payload: Vec<u8> = if let Some(mut chunk) = state.kitty_chunk_data.take() {
        if chunk.data.len() + payload_bytes.len() > MAX_KITTY_PAYLOAD {
            // Descartar si supera la cuota de seguridad
            return;
        }
        chunk.data.extend_from_slice(payload_bytes);
        if let Some(c) = control.cols {
            chunk.control.cols = Some(c);
        }
        if let Some(r) = control.rows {
            chunk.control.rows = Some(r);
        }
        if control.quiet != 0 {
            chunk.control.quiet = control.quiet;
        }
        if control.more_chunks {
            state.kitty_chunk_data = Some(chunk);
            return;
        }
        control = chunk.control;
        chunk.data
    } else if control.more_chunks {
        if payload_bytes.len() > MAX_KITTY_PAYLOAD {
            return;
        }
        let mut data = Vec::with_capacity(payload_bytes.len());
        data.extend_from_slice(payload_bytes);
        state.kitty_chunk_data = Some(KittyChunkState { control, data });
        return;
    } else {
        payload_bytes.to_vec()
    };

    // 6. Decodificación de datos base64
    let clean_b64: Vec<u8> = full_payload
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if clean_b64.is_empty() {
        return;
    }

    let Ok(decoded) = BASE64_STANDARD.decode(&clean_b64) else {
        send_error(state, &control, "EINVAL:invalid base64");
        return;
    };

    // 7. Decodificación de imagen según formato. Las dimensiones se comparan en `u64`: con `u32`,
    // `s * v * 4` se desborda con valores que el emisor controla.
    let (s, v) = (control.src_width, control.src_height);
    let raw_len = |bytes_per_pixel: u64| u64::from(s) * u64::from(v) * bytes_per_pixel;
    let rgba_img: Option<RgbaImage> = match control.format {
        32 if s > 0 && v > 0 && decoded.len() as u64 == raw_len(4) => {
            RgbaImage::from_raw(s, v, decoded)
        }
        24 if s > 0 && v > 0 && decoded.len() as u64 == raw_len(3) => {
            let mut rgba = Vec::with_capacity(decoded.len() / 3 * 4);
            for rgb in decoded.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            RgbaImage::from_raw(s, v, rgba)
        }
        // PNG/JPEG (formato 100) o datos que no encajan con las dimensiones declaradas
        _ => decode_untrusted(&decoded).or_else(|| {
            (s > 0 && v > 0 && decoded.len() as u64 == raw_len(4))
                .then(|| RgbaImage::from_raw(s, v, decoded))
                .flatten()
        }),
    };

    let Some(rgba_img) = rgba_img else {
        send_error(state, &control, "EBADPNG:cannot decode image");
        return;
    };

    let (w, h) = rgba_img.dimensions();
    if w == 0 || h == 0 {
        return;
    }

    // 8. Cálculo de dimensiones de rejilla (celdas de terminal)
    let cols = if let Some(c) = control.cols {
        c.clamp(1, state.cols)
    } else if let Some(r) = control.rows {
        let aspect = w as f32 / h as f32;
        ((r as f32 * (18.0 / 8.0) * aspect).round() as usize).clamp(1, state.cols)
    } else {
        // `clamp(10, cols)` entraba en pánico (y con `panic = "abort"` cerraba Axon) en paneles de menos de 10 columnas
        let max_cols = (state.cols.saturating_sub(state.cursor_x + 2))
            .max(10)
            .min(state.cols)
            .max(1);
        ((w as f32 / 8.0).round() as usize).clamp(1, max_cols)
    };

    let rows = if let Some(r) = control.rows {
        r.clamp(1, 100)
    } else {
        let aspect = h as f32 / w as f32;
        ((cols as f32 * (8.0 / 18.0) * aspect).round() as usize).clamp(1, 100)
    };

    // 9. Reescalar a resolución objetivo para acotar el uso de RAM (~80x30 celdas = 640x540 px)
    let (tw, th) = ((cols * 8) as u32, (rows * 18) as u32);
    let scaled_rgba = fit_image(rgba_img, tw, th);

    let img_id = if control.image_id > 0 {
        control.image_id
    } else {
        let id = state.next_image_id;
        state.next_image_id += 1;
        id
    };

    let img_entry = InlineImage::new(
        img_id,
        (state.cursor_x, state.current_global_line()),
        (cols, rows),
        scaled_rgba,
    );

    if control.image_id > 0 {
        state.store_kitty_image(control.image_id, img_entry.clone());
    }

    // Si la acción es transmitir y mostrar (a=T)
    if control.action == 'T' {
        state.push_image(img_entry);
        if control.cursor_movement != 1 {
            for _ in 0..rows {
                state.newline();
            }
        }
    }

    // Respuesta OK si se solicitó ID y no se silenciaron los reportes
    if control.image_id > 0 && control.quiet == 0 {
        let resp = format!("\x1b_Gi={};OK\x1b\\", control.image_id);
        state.pty_write_queue.push(resp.into_bytes());
    }
}
