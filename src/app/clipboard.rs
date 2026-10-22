//! Portapapeles: pegado de texto, archivos e imágenes (también por SCP en sesiones SSH).

use super::*;

/// Abre el portapapeles la primera vez que se necesita (puede fallar si otro proceso lo tiene)
pub(super) fn lazy_clipboard(
    slot: &mut Option<arboard::Clipboard>,
) -> Option<&mut arboard::Clipboard> {
    if slot.is_none() {
        *slot = arboard::Clipboard::new().ok();
    }
    slot.as_mut()
}

/// Copia `text` al portapapeles; recibe el campo y no `&mut self` para poder usarse con el visor prestado
pub(super) fn copy_text(slot: &mut Option<arboard::Clipboard>, text: String) {
    if let Some(cb) = lazy_clipboard(slot) {
        let _ = cb.set_text(text);
    }
}

impl AxonApp {
    pub(super) fn set_clipboard_text(&mut self, text: String) {
        copy_text(&mut self.clipboard, text);
    }

    pub(super) fn paste_text(&mut self, text: &str) {
        let active_tab = &mut self.tabs[self.active_tab_index];
        if let Some(active_pane) = active_tab.active_pane_mut() {
            Self::paste_into_pane(active_pane, text);
        }
    }

    /// Pega en un panel concreto (no necesariamente el activo: p. ej. al terminar una subida por `ssh`)
    pub(super) fn paste_into_pane(pane: &mut TerminalPane, text: &str) {
        let normalized = sanitize_paste(text);
        if normalized.contains(['\n', '\r']) {
            pane.terminal.note_enter_sent();
        }
        if pane.terminal.bracketed_paste_mode {
            let mut payload = Vec::with_capacity(normalized.len() + 16);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            pane.pty.write_input(&payload);
        } else {
            pane.pty.write_input(normalized.as_bytes());
        }
    }

    /// Pegar desde el portapapeles con soporte inteligente para texto, imágenes de IA y archivos.
    /// Si el portapapeles contiene una imagen (captura Snipping Tool, Win+Shift+S, recorte web),
    /// la guarda automáticamente en un archivo PNG temporal y pega la ruta entrecomillada
    /// en el prompt de la terminal para consumo directo por agentes de IA (Claude Code, Aider, etc.).
    pub(super) fn paste_from_clipboard(&mut self) {
        if self.tabs[self.active_tab_index].is_settings {
            return;
        }
        lazy_clipboard(&mut self.clipboard);
        if self.tabs[self.active_tab_index].file_viewer.is_some() {
            let text = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok());
            let (auto_save, visible) =
                (self.config.terminal.auto_save, self.viewer_visible_lines());
            if let (Some(text), Some(viewer)) =
                (text, self.tabs[self.active_tab_index].file_viewer.as_mut())
            {
                viewer.insert_text(&text);
                if auto_save && viewer.is_dirty {
                    viewer.autosave_local();
                }
                viewer.ensure_cursor_visible(visible);
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }
        let Some(cb) = &mut self.clipboard else {
            return;
        };

        // 1. Si hay texto en el portapapeles (código, comandos, texto plano), pegar directamente
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                self.paste_text(&text);
                return;
            }
        }

        // 2. Si no hay texto, comprobar si hay una imagen (captura de pantalla, recorte, imagen web)
        if let Ok(img) = cb.get_image() {
            let active_pane = self
                .tabs
                .get(self.active_tab_index)
                .and_then(|t| t.active_pane());
            let ssh_session = active_pane.and_then(|p| p.terminal.ssh_session.clone());
            let active_pane_id = active_pane.map(|p| p.id);
            let is_ssh = ssh_session.as_ref().map(|s| s.is_active).unwrap_or(false);

            // Solo Claude Code soporta pegado nativo de imágenes con Alt+V (\x1bv).
            // Para otros agentes (gemini, agy, aider...) o la shell, se guarda el PNG y se pega su ruta.
            let is_claude =
                active_pane.is_some_and(|p| p.terminal.active_ai_agent() == AiAgentStatus::Claude);

            // Si Claude Code (local o remoto vía SSH) está activo, enviamos la secuencia
            // nativa de pegado de imágenes (Alt + V: \x1bv).
            if is_claude {
                if let Some(pane) = active_pane {
                    pane.pty.write_input(b"\x1bv");
                }
                return;
            }

            // Si estamos en una sesión remota SSH, transferimos la imagen por ssh
            // y pegamos la ruta remota /tmp/axon_img_... en lugar de una ruta local de Windows
            if is_ssh {
                if let Some(session) = ssh_session {
                    if let (Some(local_path), Some(pane_id)) =
                        (self.save_clipboard_image(&img), active_pane_id)
                    {
                        // `ssh` en un hilo: al terminar se pega la ruta remota (o la local si falló)
                        self.ssh_upload_image(pane_id, local_path, &session.destination);
                        return;
                    }
                }
            }

            // Si Claude Code no está corriendo y es una sesión local,
            // guardamos la captura en el directorio temporal y pegamos la ruta formateada.
            if let Some(path_str) = self.save_clipboard_image(&img) {
                let formatted = format!("\"{}\" ", path_str);
                self.paste_text(&formatted);
                return;
            }
        }

        // 3. Si no hay texto ni bitmap directo, comprobar si se copiaron archivos desde el Explorador de Windows (CF_HDROP)
        let files = self
            .clipboard
            .as_mut()
            .and_then(|cb| cb.get().file_list().ok());
        if let Some(files) = files {
            let formatted = files
                .into_iter()
                .map(|p| format!("\"{}\"", p.display()))
                .collect::<Vec<_>>()
                .join(" ");
            if !formatted.is_empty() {
                let with_space = format!("{} ", formatted);
                self.paste_text(&with_space);
            }
        }
    }

    /// Guarda una imagen del portapapeles en la carpeta de caché local de Axon (`%LOCALAPPDATA%\axon\axon\cache\clipboard`) como PNG.
    /// Realiza limpieza pasiva periódica de capturas antiguas (>24 horas) con aceleración (máximo una vez por hora)
    /// y elimina cualquier carpeta temporal obsoleta en `%TEMP%\axon_clipboard` para evitar alertas heurísticas de antivirus.
    pub(crate) fn save_clipboard_image(&self, img: &arboard::ImageData) -> Option<String> {
        let clip_dir = crate::config::cache_dir_or_fallback().join("clipboard");
        let _ = std::fs::create_dir_all(&clip_dir);

        // Limpieza pasiva con aceleración (máx. 1 vez cada hora) para evitar escaneos constantes de I/O y falsos positivos de antivirus
        static LAST_CLEANUP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = LAST_CLEANUP.load(std::sync::atomic::Ordering::Relaxed);
        if now_secs.saturating_sub(last) > 3600 {
            LAST_CLEANUP.store(now_secs, std::sync::atomic::Ordering::Relaxed);

            // 1. Limpieza de capturas con más de 24 horas en la caché de la aplicación
            if let Ok(entries) = std::fs::read_dir(&clip_dir) {
                let now = std::time::SystemTime::now();
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age.as_secs() > 86400 {
                                    let _ = std::fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
            }

            // 2. Limpieza proactiva del directorio temporal obsoleto en %TEMP%\axon_clipboard
            let legacy_temp = std::env::temp_dir().join("axon_clipboard");
            if legacy_temp.exists() {
                let _ = std::fs::remove_dir_all(legacy_temp);
            }
        }

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_path = clip_dir.join(format!("axon_clip_{timestamp}_{seq}.png"));

        let width = img.width as u32;
        let height = img.height as u32;
        let raw_bytes = img.bytes.to_vec();

        if let Some(rgba_img) = image::RgbaImage::from_raw(width, height, raw_bytes) {
            if rgba_img.save(&file_path).is_ok() {
                return Some(file_path.to_string_lossy().to_string());
            }
        }

        None
    }
}

/// Prepara texto pegado para el PTY: saltos de línea a `\r` y se eliminan los caracteres de control.
///
/// Sin esto, un portapapeles con `ESC [ 2 0 1 ~` cerraría el pegado entre corchetes antes de tiempo y el
/// resto del texto se ejecutaría como si lo hubiera tecleado el usuario; `ESC` también permite emitir
/// secuencias de terminal arbitrarias. Se conservan el tabulador y los saltos de línea.
pub(super) fn sanitize_paste(text: &str) -> String {
    text.replace("\r\n", "\r")
        .replace('\n', "\r")
        .chars()
        .filter(|&c| c == '\r' || c == '\t' || !(c.is_control()))
        .collect()
}

/// Limpia el comando de una tarjeta de bloque antes de copiarlo o escribirlo en el prompt.
///
/// El texto sale de la región OSC 133 de la pantalla, que controla el programa en ejecución: se descartan
/// los caracteres de control y los de formato Unicode (bidi, ancho cero, marca de orden de bytes), que
/// permitirían ocultar o reordenar lo que el usuario cree que va a escribir.
pub(super) fn clean_block_command(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            !c.is_control()
                && !matches!(
                    c,
                    '\u{00AD}'
                        | '\u{061C}'
                        | '\u{200B}'..='\u{200F}'
                        | '\u{202A}'..='\u{202E}'
                        | '\u{2060}'..='\u{2064}'
                        | '\u{2066}'..='\u{206F}'
                        | '\u{FEFF}'
                )
        })
        .collect()
}
