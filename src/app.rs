use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::renderer::TerminalRenderer;
use crate::terminal::{SelectionRange, TerminalState};
use crate::theme::Theme;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy)]
pub enum AxonEvent {
    PtyData,
    PtyClosed,
}

pub struct AxonApp {
    config: AxonConfig,
    theme: Theme,
    pty: PtyManager,
    terminal: TerminalState,
    renderer: Option<TerminalRenderer>,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    clipboard: Option<arboard::Clipboard>,
    modifiers: ModifiersState,
    drag_start: Option<(usize, usize)>,
    mouse_pos: PhysicalPosition<f64>,
    mouse_down: bool,
    cursor_timer: Instant,
    last_grid_size: (usize, usize),
    is_focused: bool,
}

impl AxonApp {
    pub fn new(config: AxonConfig, proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>) -> Self {
        let cols = 120;
        let rows = 30;
        let theme = Theme::ghostty_dark();
        let pty = PtyManager::new(
            &config.terminal.default_shell,
            config.terminal.ghostty_prompt,
            cols as u16,
            rows as u16,
            proxy,
        ).expect("Error al inicializar Windows ConPTY");

        let terminal = TerminalState::new(
            cols,
            rows,
            config.terminal.scrollback_lines,
            theme.clone(),
        );

        let clipboard = arboard::Clipboard::new().ok();

        Self {
            config,
            theme,
            pty,
            terminal,
            renderer: None,
            window: None,
            context: None,
            surface: None,
            clipboard,
            modifiers: ModifiersState::default(),
            drag_start: None,
            mouse_pos: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            cursor_timer: Instant::now(),
            last_grid_size: (cols, rows),
            is_focused: true,
        }
    }

    fn paste_text(&self, text: &str) {
        let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.terminal.bracketed_paste_mode {
            let mut payload = Vec::with_capacity(normalized.len() + 16);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            self.pty.write_input(&payload);
        } else {
            self.pty.write_input(normalized.as_bytes());
        }
    }

    /// Pegar desde el portapapeles con soporte inteligente para texto, imágenes de IA y archivos.
    /// Si el portapapeles contiene una imagen (captura Snipping Tool, Win+Shift+S, recorte web),
    /// la guarda automáticamente en un archivo PNG temporal y pega la ruta entrecomillada
    /// en el prompt de la terminal para consumo directo por agentes de IA (Claude Code, Aider, etc.).
    fn paste_from_clipboard(&mut self) {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = &mut self.clipboard else { return; };

        // 1. Si hay texto en el portapapeles (código, comandos, texto plano), pegar directamente
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                self.paste_text(&text);
                return;
            }
        }

        // 2. Si no hay texto, comprobar si hay una imagen (captura de pantalla, recorte, imagen web)
        if let Ok(img) = cb.get_image() {
            // Si Claude Code (u otro agente CLI compatible) está activo, enviamos la secuencia
            // nativa de pegado de imágenes (Alt + V: \x1bv). Esto hace que Claude Code
            // procese la imagen nativamente y muestre el chip interactivo [Image 1] en el prompt.
            if is_claude_running() {
                self.pty.write_input(b"\x1bv");
                return;
            }

            // Si Claude Code no está corriendo (ej: prompt PowerShell estándar u otras herramientas),
            // guardamos la captura en el directorio temporal y pegamos la ruta formateada.
            if let Some(path_str) = self.save_clipboard_image(&img) {
                let formatted = format!("\"{}\" ", path_str);
                self.paste_text(&formatted);
                return;
            }
        }

        // 3. Si no hay texto ni bitmap directo, comprobar si se copiaron archivos desde el Explorador de Windows (CF_HDROP)
        if let Some(files) = get_clipboard_files() {
            let formatted = files
                .into_iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(" ");
            if !formatted.is_empty() {
                let with_space = format!("{} ", formatted);
                self.paste_text(&with_space);
            }
        }
    }

    /// Guarda una imagen del portapapeles en la carpeta temporal de Axon como archivo PNG.
    /// Realiza limpieza pasiva de archivos temporales con más de 24 horas de antigüedad.
    pub(crate) fn save_clipboard_image(&self, img: &arboard::ImageData) -> Option<String> {
        let clip_dir = std::env::temp_dir().join("axon_clipboard");
        let _ = std::fs::create_dir_all(&clip_dir);

        // Limpieza pasiva de capturas antiguas (>24 horas)
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

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_path = clip_dir.join(format!("axon_clip_{}.png", timestamp));

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

    fn adjust_font_size(&mut self, delta: f32) {
        let new_size = (self.config.appearance.font_size + delta).clamp(8.0, 32.0);
        if (new_size - self.config.appearance.font_size).abs() > 0.01 {
            self.config.appearance.font_size = new_size;
            if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                let scale_factor = window.scale_factor();
                renderer.update_scale(&self.config, scale_factor);
                let size = window.inner_size();
                let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize);
                if (cols, rows) != self.last_grid_size {
                    self.last_grid_size = (cols, rows);
                    self.terminal.resize(cols, rows);
                    self.pty.resize(cols as u16, rows as u16);
                }
                window.request_redraw();
            }
        }
    }

    fn reset_font_size(&mut self) {
        let default_cfg = AxonConfig::load_or_default();
        let target_size = default_cfg.appearance.font_size;
        self.config.appearance.font_size = target_size;
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
            let size = window.inner_size();
            let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize);
            if (cols, rows) != self.last_grid_size {
                self.last_grid_size = (cols, rows);
                self.terminal.resize(cols, rows);
                self.pty.resize(cols as u16, rows as u16);
            }
            window.request_redraw();
        }
    }

    fn reload_config(&mut self) {
        self.config = AxonConfig::load_or_default();
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
            let size = window.inner_size();
            let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize);
            if (cols, rows) != self.last_grid_size {
                self.last_grid_size = (cols, rows);
                self.terminal.resize(cols, rows);
                self.pty.resize(cols as u16, rows as u16);
            }
            window.request_redraw();
        }
    }
}

impl ApplicationHandler<AxonEvent> for AxonApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AxonEvent) {
        match event {
            AxonEvent::PtyData => {
                let mut got_bytes = false;
                while let Some(bytes) = self.pty.try_recv_output() {
                    got_bytes = true;
                    if bytes.contains(&0x1b) {
                        crate::logger::log(&format!("PTY RECV ESC: {:?}", String::from_utf8_lossy(&bytes)));
                    }
                    self.terminal.process_bytes(&bytes);
                }
                for text in self.terminal.clipboard_queue.drain(..) {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set_text(text).ok();
                    }
                }
                if got_bytes {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            AxonEvent::PtyClosed => {
                crate::logger::log("PtyClosed recibido: cerrando Axon");
                event_loop.exit();
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Crear icono nativo de Axon (Ghostty Green '>' en fondo oscuro)
        let icon = create_native_icon();

        let window_attributes = WindowAttributes::default()
            .with_title("Axon")
            .with_visible(false)
            .with_inner_size(PhysicalSize::new(1020, 680))
            .with_min_inner_size(PhysicalSize::new(480, 280))
            .with_window_icon(Some(icon));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Error al crear ventana nativa de Windows"),
        );

        // Habilitar barra de título oscura inmersiva de Windows 11 (DWM Dark Mode)
        // y fijar el pincel de fondo de clase Win32 al tema oscuro para evitar flash blanco
        enable_windows_dark_mode(&window);

        let context = softbuffer::Context::new(window.clone())
            .expect("Error al inicializar contexto de aceleración gráfica de pantalla");
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .expect("Error al inicializar superficie de renderizado nativo");

        let scale_factor = window.scale_factor();
        let mut renderer = TerminalRenderer::new(&self.config, scale_factor);

        let initial_size = window.inner_size();
        if let (Some(w), Some(h)) = (NonZeroU32::new(initial_size.width), NonZeroU32::new(initial_size.height)) {
            let _ = surface.resize(w, h);
        }

        let (cols, rows) = renderer.compute_grid_size(initial_size.width as usize, initial_size.height as usize);
        self.last_grid_size = (cols, rows);
        self.terminal.resize(cols, rows);
        self.pty.resize(cols as u16, rows as u16);

        // Pre-renderizar el primer frame oscuro en la superficie antes de hacer visible la ventana.
        // Esto garantiza que el usuario vea la terminal oscura de inmediato sin ningún destello blanco.
        if let Ok(mut buffer) = surface.buffer_mut() {
            renderer.render(
                &mut buffer,
                initial_size.width as usize,
                initial_size.height as usize,
                &self.terminal,
                &self.theme,
                true,
            );
            buffer.present().ok();
        }

        // Mostrar la ventana una vez que el primer frame oscuro ya está listo y dibujado
        window.set_visible(true);

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    renderer.update_scale(&self.config, scale_factor);
                    let size = window.inner_size();
                    let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize);
                    if (cols, rows) != self.last_grid_size {
                        self.last_grid_size = (cols, rows);
                        self.terminal.resize(cols, rows);
                        self.pty.resize(cols as u16, rows as u16);
                    }
                    window.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(renderer)) = (&mut self.surface, &self.renderer) {
                    if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        surface.resize(w, h).ok();
                        let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize);
                        if (cols, rows) != self.last_grid_size {
                            self.last_grid_size = (cols, rows);
                            self.terminal.resize(cols, rows);
                            self.pty.resize(cols as u16, rows as u16);
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(surface), Some(renderer), Some(window)) =
                    (&mut self.surface, &mut self.renderer, &self.window)
                {
                    let size = window.inner_size();
                    if size.width == 0 || size.height == 0 {
                        return;
                    }

                    let cursor_visible = !self.config.terminal.cursor_blink
                        || (self.cursor_timer.elapsed().as_millis() % 1000) < 600;

                    if let Ok(mut buffer) = surface.buffer_mut() {
                        renderer.render(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            &self.terminal,
                            &self.theme,
                            cursor_visible,
                        );
                        buffer.present().ok();
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        text,
                        state: ElementState::Pressed,
                        physical_key,
                        ..
                    },
                ..
            } => {
                // Si la terminal no tiene el foco activo, ignorar la entrada de teclado
                if !self.is_focused {
                    return;
                }

                #[cfg(target_os = "windows")]
                let (ctrl, shift, alt, altgr) = get_physical_modifiers();

                #[cfg(not(target_os = "windows"))]
                let (ctrl, shift, alt, altgr) = (
                    self.modifiers.control_key(),
                    self.modifiers.shift_key(),
                    self.modifiers.alt_key(),
                    false,
                );

                crate::logger::log(&format!(
                    "KEY: phys={:?}, log={:?}, text={:?}, ctrl={}, shift={}, alt={}, altgr={}",
                    physical_key, logical_key, text, ctrl, shift, alt, altgr
                ));

                // Si AltGr está presionado (teclados en español para @, #, \, ~, etc.),
                // procesar exclusivamente como entrada de texto y nunca como atajo de control
                if altgr {
                    if let Some(t) = text {
                        if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                            crate::logger::log(&format!("-> AltGr text: {:?}", t));
                            self.pty.write_input(t.as_bytes());
                        }
                    }
                    return;
                }

                // Alt + Tab es el atajo de cambio de tarea de Windows; NUNCA debe enviar \t al shell
                if alt && logical_key == Key::Named(NamedKey::Tab) {
                    crate::logger::log("-> Ignorado Alt+Tab");
                    return;
                }

                // Siempre devolver scroll al prompt activo al pulsar teclas normales (excepto PageUp/Down y Shift+Home/End)
                if logical_key != Key::Named(NamedKey::PageUp)
                    && logical_key != Key::Named(NamedKey::PageDown)
                    && !(shift && (logical_key == Key::Named(NamedKey::Home) || logical_key == Key::Named(NamedKey::End)))
                {
                    self.terminal.reset_scroll();
                }

                // Detección robusta de tecla Backspace (por physical_key independiente de layout/Alt y por logical_key)
                let is_backspace = matches!(physical_key, PhysicalKey::Code(KeyCode::Backspace))
                    || matches!(&logical_key, Key::Named(NamedKey::Backspace))
                    || matches!(&logical_key, Key::Character(s) if s == "\u{8}" || s == "\u{7f}" || s == "\u{17}");

                // 1. Si Ctrl o Alt están presionados (Ctrl+Backspace o Alt+Backspace):
                //    En Windows ConPTY, el byte nativo que emite Windows Terminal para borrar la palabra
                //    anterior (BackwardKillWord en PSReadLine) es 0x08 (ASCII Backspace). ConPTY lo mapea
                //    a VK_BACK con Ctrl activado, borrando la palabra completa limpiamente.
                if (ctrl || alt) && is_backspace {
                    crate::logger::log("-> Enviando 0x08 (borrado de palabra)");
                    self.pty.write_input(&[0x08]);
                    return;
                }

                // 2. Backspace normal (o Shift+Backspace):
                //    Enviar 0x7f para borrar carácter por carácter de forma limpia.
                if is_backspace {
                    crate::logger::log("-> Enviando 0x7f (borrado de caracter)");
                    self.pty.write_input(&[0x7f]);
                    return;
                }

                // 1. Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
                if (ctrl && shift && key_is_char(&logical_key, 'c'))
                    || (ctrl && !shift && key_is_char(&logical_key, 'c') && self.terminal.selection.is_some())
                {
                    if let Some(sel_text) = self.terminal.copy_selection_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(sel_text).ok();
                        }
                        // Deseleccionar y repintar tras copiar exitosamente
                        self.terminal.selection = None;
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                    return;
                }

                // 2. Pegar desde el portapapeles (Ctrl + Shift + V, Ctrl + V, Shift + Insert)
                if (ctrl && shift && key_is_char(&logical_key, 'v'))
                    || (ctrl && !shift && key_is_char(&logical_key, 'v'))
                    || (shift && logical_key == Key::Named(NamedKey::Insert))
                {
                    self.paste_from_clipboard();
                    return;
                }

                // 3. Combinaciones con Alt (Alt + V para imágenes en Claude Code, Alt + Flechas)
                if alt && !ctrl {
                    if key_is_char(&logical_key, 'v') {
                        // Enviar Alt + V (\x1bv) para pegado nativo de imágenes en Claude Code
                        self.pty.write_input(b"\x1bv");
                        return;
                    }

                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.pty.write_input(b"\x1bb"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.pty.write_input(b"\x1bf"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Tab) => {
                            return; // Prevenir cualquier propagación de Alt+Tab
                        }
                        Key::Named(NamedKey::F4) => {
                            return; // Alt+F4 manejado por el sistema
                        }
                        Key::Character(ref s) => {
                            // Solo enviar secuencias escape Alt+letra para letras ASCII minúsculas/mayúsculas
                            // para no interferir con combinaciones AltGr de teclados en español (@, #, \, ~, etc.)
                            if let Some(c) = s.chars().next() {
                                if c.is_ascii_alphabetic() {
                                    let mut buf = [0u8; 5];
                                    buf[0] = 0x1b;
                                    let len = c.encode_utf8(&mut buf[1..]).len();
                                    self.pty.write_input(&buf[..1 + len]);
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // 3. Zoom dinámico de tipografía (Ctrl + +, Ctrl + -, Ctrl + 0)
                if ctrl && !shift {
                    if key_is_char(&logical_key, '+') || key_is_char(&logical_key, '=') {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '-') || key_is_char(&logical_key, '_') {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '0') {
                        self.reset_font_size();
                        return;
                    }
                } else if ctrl && shift {
                    if key_is_char(&logical_key, '+') || key_is_char(&logical_key, '=') {
                        self.adjust_font_size(1.0);
                        return;
                    }
                    if key_is_char(&logical_key, '_') || key_is_char(&logical_key, '-') {
                        self.adjust_font_size(-1.0);
                        return;
                    }
                }

                // 4. Recarga dinámica de configuración Ghostty (Ctrl + Shift + ,)
                if ctrl && shift && key_is_char(&logical_key, ',') {
                    self.reload_config();
                    return;
                }

                // 5. Copiar salida del último comando del agente (Ctrl + Shift + O)
                if ctrl && shift && key_is_char(&logical_key, 'o') {
                    if let Some(output) = self.terminal.get_last_output_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(output).ok();
                        }
                    }
                    return;
                }

                // 5b. Limpiar pantalla y scrollback buffer al estilo Ghostty (Ctrl + Shift + K)
                if ctrl && shift && (key_is_char(&logical_key, 'k') || key_is_char(&logical_key, 'l')) {
                    self.terminal.scrollback.clear();
                    self.terminal.reset_scroll();
                    self.terminal.command_blocks.clear();
                    self.pty.write_input(b"\x0c");
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // 6. Navegación semántica entre comandos de agentes OSC 133 (Ctrl + Shift + Arriba/Abajo)
                if ctrl && shift {
                    if let Key::Named(NamedKey::ArrowUp) = logical_key {
                        if let Some(block) = self.terminal.command_blocks.last() {
                            let target = self.terminal.total_lines_count.saturating_sub(block.prompt_line);
                            self.terminal.scroll_offset = target.min(self.terminal.scrollback.len());
                            if let Some(w) = &self.window { w.request_redraw(); }
                        }
                        return;
                    }
                    if let Key::Named(NamedKey::ArrowDown) = logical_key {
                        self.terminal.reset_scroll();
                        if let Some(w) = &self.window { w.request_redraw(); }
                        return;
                    }
                }

                // 7. Navegación de historial y scrollback (Shift + PageUp / PageDown / Home / End)
                if shift && !ctrl {
                    match logical_key {
                        Key::Named(NamedKey::PageUp) => {
                            let half = (self.terminal.rows / 2).max(1);
                            self.terminal.scroll_up(half);
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let half = (self.terminal.rows / 2).max(1);
                            self.terminal.scroll_down(half);
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            self.terminal.scroll_offset = self.terminal.scrollback.len();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            self.terminal.reset_scroll();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        _ => {}
                    }
                }

                // 8. Salto de palabras y edición en línea de comandos (Ctrl + Flechas, Ctrl + Backspace, Ctrl + Delete)
                if ctrl && !shift {
                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.pty.write_input(b"\x1b[1;5D"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.pty.write_input(b"\x1b[1;5C"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            self.pty.write_input(b"\x1b[1;5A");
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            self.pty.write_input(b"\x1b[1;5B");
                            return;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            self.pty.write_input(&[0x08]); // 0x08: Borrar palabra atrás en ConPTY
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            self.pty.write_input(b"\x1b[3;5~"); // Borrar palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            self.pty.write_input(b"\x1b[1;5H");
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            self.pty.write_input(b"\x1b[1;5F");
                            return;
                        }
                        _ => {}
                    }
                }

                // 9. Combinaciones de control clásicas de terminal (Ctrl+C, Ctrl+D, Ctrl+L, Ctrl+Z, Ctrl+R, etc.)
                if ctrl && !shift {
                    match logical_key {
                        Key::Character(ref s) => {
                            let c = s.chars().next().unwrap_or(' ').to_ascii_lowercase();
                            if c >= 'a' && c <= 'z' {
                                let code = (c as u8) - b'a' + 1;
                                self.pty.write_input(&[code]);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // 10. Scroll por página directo sin Shift
                if logical_key == Key::Named(NamedKey::PageUp) {
                    let half = (self.terminal.rows / 2).max(1);
                    self.terminal.scroll_up(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }
                if logical_key == Key::Named(NamedKey::PageDown) {
                    let half = (self.terminal.rows / 2).max(1);
                    self.terminal.scroll_down(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // 11. Teclas especiales y flechas VT estándar
                match logical_key {
                    Key::Named(NamedKey::Enter) => self.pty.write_input(b"\r"),
                    Key::Named(NamedKey::Backspace) => self.pty.write_input(&[0x7f]),
                    Key::Named(NamedKey::Tab) => {
                        if alt {
                            return;
                        }
                        if shift {
                            self.pty.write_input(b"\x1b[Z");
                        } else {
                            self.pty.write_input(b"\t");
                        }
                    }
                    Key::Named(NamedKey::Escape) => self.pty.write_input(&[0x1b]),
                    Key::Named(NamedKey::Delete) => self.pty.write_input(b"\x1b[3~"),
                    Key::Named(NamedKey::Home) => self.pty.write_input(b"\x1b[H"),
                    Key::Named(NamedKey::End) => self.pty.write_input(b"\x1b[F"),
                    Key::Named(NamedKey::ArrowUp) => self.pty.write_input(b"\x1b[A"),
                    Key::Named(NamedKey::ArrowDown) => self.pty.write_input(b"\x1b[B"),
                    Key::Named(NamedKey::ArrowRight) => self.pty.write_input(b"\x1b[C"),
                    Key::Named(NamedKey::ArrowLeft) => self.pty.write_input(b"\x1b[D"),
                    Key::Named(NamedKey::F1) => self.pty.write_input(b"\x1bOP"),
                    Key::Named(NamedKey::F2) => self.pty.write_input(b"\x1bOQ"),
                    Key::Named(NamedKey::F3) => self.pty.write_input(b"\x1bOR"),
                    Key::Named(NamedKey::F4) => self.pty.write_input(b"\x1bOS"),
                    Key::Named(NamedKey::F5) => self.pty.write_input(b"\x1b[15~"),
                    Key::Named(NamedKey::F6) => self.pty.write_input(b"\x1b[17~"),
                    Key::Named(NamedKey::F7) => self.pty.write_input(b"\x1b[18~"),
                    Key::Named(NamedKey::F8) => self.pty.write_input(b"\x1b[19~"),
                    Key::Named(NamedKey::F9) => self.pty.write_input(b"\x1b[20~"),
                    Key::Named(NamedKey::F10) => self.pty.write_input(b"\x1b[21~"),
                    Key::Named(NamedKey::F11) => self.pty.write_input(b"\x1b[23~"),
                    Key::Named(NamedKey::F12) => self.pty.write_input(b"\x1b[24~"),
                    _ => {
                        // Entrada de texto regular (letras, números, símbolos, emojis)
                        if let Some(t) = text {
                            if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                                self.pty.write_input(t.as_bytes());
                            }
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = position;
                let bypass = self.modifiers.shift_key();

                if self.terminal.is_mouse_reporting_active() && !bypass {
                    // Reportar movimiento/arrastre a Herdr / Ratatui (SGR 1006 / 1002 / 1003)
                    if (self.terminal.mouse_reporting_button_event && self.mouse_down) || self.terminal.mouse_reporting_any_event {
                        if let Some(renderer) = &self.renderer {
                            let mx = position.x as usize;
                            let my = position.y as usize;
                            let col = if mx >= renderer.margin_x {
                                ((mx - renderer.margin_x) / renderer.char_width) + 1
                            } else { 1 }.min(self.terminal.cols);
                            let row = if my >= renderer.margin_y {
                                ((my - renderer.margin_y) / renderer.row_height) + 1
                            } else { 1 }.min(self.terminal.rows);

                            let mut code = 32; // Flag de arrastre/movimiento
                            if self.modifiers.shift_key() { code += 4; }
                            if self.modifiers.alt_key() { code += 8; }
                            if self.modifiers.control_key() { code += 16; }

                            if self.terminal.mouse_reporting_sgr {
                                let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                self.pty.write_input(seq.as_bytes());
                            } else {
                                let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                                self.pty.write_input(&seq);
                            }
                        }
                    }
                } else if self.mouse_down {
                    if let Some(renderer) = &self.renderer {
                        let mx = position.x as usize;
                        let my = position.y as usize;
                        if mx >= renderer.margin_x && my >= renderer.margin_y {
                            let col = (mx - renderer.margin_x) / renderer.char_width;
                            let row = (my - renderer.margin_y) / renderer.row_height;
                            if let Some(start) = self.drag_start {
                                self.terminal.selection = Some(SelectionRange {
                                    start,
                                    end: (col, row),
                                });
                                if let Some(w) = &self.window { w.request_redraw(); }
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let bypass = self.modifiers.shift_key();

                if self.terminal.is_mouse_reporting_active() && !bypass {
                    // Transmitir evento de ratón a Herdr / Ratatui
                    if let Some(renderer) = &self.renderer {
                        let mx = self.mouse_pos.x as usize;
                        let my = self.mouse_pos.y as usize;
                        let col = if mx >= renderer.margin_x {
                            ((mx - renderer.margin_x) / renderer.char_width) + 1
                        } else { 1 }.min(self.terminal.cols);
                        let row = if my >= renderer.margin_y {
                            ((my - renderer.margin_y) / renderer.row_height) + 1
                        } else { 1 }.min(self.terminal.rows);

                        let base_code = match button {
                            MouseButton::Left => 0,
                            MouseButton::Middle => 1,
                            MouseButton::Right => 2,
                            _ => 0,
                        };

                        let mut code = base_code;
                        if self.modifiers.shift_key() { code += 4; }
                        if self.modifiers.alt_key() { code += 8; }
                        if self.modifiers.control_key() { code += 16; }

                        let is_pressed = state == ElementState::Pressed;
                        self.mouse_down = is_pressed && button == MouseButton::Left;

                        if self.terminal.mouse_reporting_sgr {
                            let suffix = if is_pressed { 'M' } else { 'm' };
                            let seq = format!("\x1b[<{};{};{}{}", code, col, row, suffix);
                            self.pty.write_input(seq.as_bytes());
                        } else {
                            let btn_byte = if is_pressed { 32 + code as u8 } else { 32 + 3 };
                            let seq = [0x1b, b'[', b'M', btn_byte, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                            self.pty.write_input(&seq);
                        }
                    }
                } else {
                    match button {
                        MouseButton::Left => {
                            match state {
                                ElementState::Pressed => {
                                    self.mouse_down = true;
                                    if let Some(renderer) = &self.renderer {
                                        let mx = self.mouse_pos.x as usize;
                                        let my = self.mouse_pos.y as usize;
                                        if mx >= renderer.margin_x && my >= renderer.margin_y {
                                            let col = (mx - renderer.margin_x) / renderer.char_width;
                                            let row = (my - renderer.margin_y) / renderer.row_height;
                                            self.drag_start = Some((col, row));
                                            self.terminal.selection = None;
                                            if let Some(w) = &self.window { w.request_redraw(); }
                                        }
                                    }
                                }
                                ElementState::Released => {
                                    self.mouse_down = false;
                                    self.drag_start = None;
                                }
                            }
                        }
                        MouseButton::Right | MouseButton::Middle => {
                            if state == ElementState::Pressed {
                                self.paste_from_clipboard();
                            }
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let bypass = self.modifiers.shift_key();

                if self.terminal.is_mouse_reporting_active() && !bypass {
                    // Transmitir scroll del ratón al TUI activo (Herdr / Ratatui)
                    if let Some(renderer) = &self.renderer {
                        let mx = self.mouse_pos.x as usize;
                        let my = self.mouse_pos.y as usize;
                        let col = if mx >= renderer.margin_x {
                            ((mx - renderer.margin_x) / renderer.char_width) + 1
                        } else { 1 }.min(self.terminal.cols);
                        let row = if my >= renderer.margin_y {
                            ((my - renderer.margin_y) / renderer.row_height) + 1
                        } else { 1 }.min(self.terminal.rows);

                        let is_up = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                            MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                        };

                        let mut code = if is_up { 64 } else { 65 };
                        if self.modifiers.shift_key() { code += 4; }
                        if self.modifiers.alt_key() { code += 8; }
                        if self.modifiers.control_key() { code += 16; }

                        if self.terminal.mouse_reporting_sgr {
                            let seq = format!("\x1b[<{};{};{}M", code, col, row);
                            self.pty.write_input(seq.as_bytes());
                        } else {
                            let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                            self.pty.write_input(&seq);
                        }
                    }
                } else {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1) * 3,
                        MouseScrollDelta::PixelDelta(pos) => ((pos.y.abs() / 20.0).ceil() as usize).max(1) * 3,
                    };

                    let is_up = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                        MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                    };

                    if is_up {
                        self.terminal.scroll_up(lines);
                    } else {
                        self.terminal.scroll_down(lines);
                    }

                    if let Some(w) = &self.window { w.request_redraw(); }
                }
            }

            WindowEvent::Focused(focused) => {
                crate::logger::log(&format!("FOCUS: {}", focused));
                self.is_focused = focused;
                if !focused {
                    // Al perder el foco (p. ej. al hacer Alt+Tab hacia otra ventana),
                    // resetear los modificadores para evitar que Alt o Ctrl queden atascados.
                    self.modifiers = ModifiersState::empty();
                    self.mouse_down = false;
                    self.drag_start = None;
                } else {
                    // Al retomar foco, también limpiar para partir de un estado limpio
                    self.modifiers = ModifiersState::empty();
                }

                if self.terminal.focus_event_reporting {
                    if focused {
                        self.pty.write_input(b"\x1b[I");
                    } else {
                        self.pty.write_input(b"\x1b[O");
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.pty.is_child_exited() {
            crate::logger::log("Proceso hijo terminado: cerrando Axon");
            event_loop.exit();
            return;
        }

        // 1. Drenar salida de ConPTY por si quedó algún byte residual
        let mut got_bytes = false;
        while let Some(bytes) = self.pty.try_recv_output() {
            got_bytes = true;
            self.terminal.process_bytes(&bytes);
        }

        // 2. Drenar comandos OSC 52 hacia el portapapeles de Windows
        for text in self.terminal.clipboard_queue.drain(..) {
            if let Some(cb) = &mut self.clipboard {
                cb.set_text(text).ok();
            }
        }

        // 3. Solicitar repintado si llegaron nuevos bytes del PTY
        if got_bytes {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        // 4. Temporizador de parpadeo de cursor cuando la terminal está inactiva
        if self.config.terminal.cursor_blink {
            let elapsed = self.cursor_timer.elapsed().as_millis();
            if elapsed >= 500 {
                self.cursor_timer = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                    std::time::Duration::from_millis(500),
                ));
            } else {
                let remain = (500 - elapsed).max(16) as u64;
                event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                    std::time::Duration::from_millis(remain),
                ));
            }
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}

fn create_native_icon() -> Icon {
    let size = 32;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let mut r = 0x16;
            let mut g = 0x16;
            let mut b = 0x18;
            let a = 255;

            let in_arrow_top = x >= 9 && x <= 16 && (y as i32 - x as i32).abs() <= 1 && y >= 9 && y <= 16;
            let in_arrow_bot = x >= 9 && x <= 16 && (y as i32 + x as i32 - 32).abs() <= 1 && y >= 16 && y <= 23;
            let in_cursor = x >= 18 && x <= 24 && (y == 21 || y == 22);

            if in_arrow_top || in_arrow_bot || in_cursor {
                r = 0x51; g = 0xe2; b = 0x7a; // Verde Ghostty #51e27a
            }

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = a;
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32).expect("Error al crear icono de ventana")
}

#[cfg(target_os = "windows")]
fn enable_windows_dark_mode(window: &Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                // 1. Dark Mode inmersivo
                let dark_mode: i32 = 1;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    20, // DWMWA_USE_IMMERSIVE_DARK_MODE
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                // 2. Color de fondo del titlebar idéntico a Ghostty (#161618 -> BGR: 0x00181616)
                let caption_color: u32 = 0x00181616;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    35, // DWMWA_CAPTION_COLOR
                    &caption_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 3. Color del texto del titlebar Ghostty (#e6e6eb -> BGR: 0x00ebe6e6)
                let text_color: u32 = 0x00ebe6e6;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    36, // DWMWA_TEXT_COLOR
                    &text_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 4. Color del borde sutil de la ventana (#28282c -> BGR: 0x002c2828)
                let border_color: u32 = 0x002c2828;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    34, // DWMWA_BORDER_COLOR
                    &border_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 5. Esquinas redondeadas nativas de Windows 11
                let corner_pref: u32 = 2; // DWMWCP_ROUND
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    33, // DWMWA_WINDOW_CORNER_PREFERENCE
                    &corner_pref as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 6. Efecto de fondo Mica nativo de Windows 11 (estilo Ghostty)
                let backdrop_type: u32 = 2; // 2 = DWMSBT_MAINWINDOW (Mica)
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    38, // DWMWA_SYSTEMBACKDROP_TYPE
                    &backdrop_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 7. Establecer el pincel de fondo de la clase Win32 al color oscuro de Ghostty (#161618)
                // Esto previene que Windows pinte el fondo blanco por defecto (COLOR_WINDOW / WHITE_BRUSH)
                // al crear la ventana o al procesar eventos WM_ERASEBKGND durante redimensionado.
                let dark_brush = windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(0x00181616);
                if dark_brush != 0 {
                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::GCLP_HBRBACKGROUND,
                        dark_brush as isize,
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_physical_modifiers() -> (bool, bool, bool, bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_RCONTROL, VK_RMENU, VK_SHIFT,
    };

    unsafe {
        let is_down = |vk: u16| -> bool {
            (GetAsyncKeyState(vk as i32) as i16) < 0
        };

        let shift = is_down(VK_SHIFT);
        let rmenu = is_down(VK_RMENU); // AltGr (Right Alt)
        let lmenu = is_down(VK_LMENU); // Left Alt
        let lctrl = is_down(VK_LCONTROL);
        let rctrl = is_down(VK_RCONTROL);

        // En Windows, presionar AltGr (RMENU) inyecta automáticamente un LCONTROL sintético.
        // Por tanto, Ctrl solo es genuino si RCONTROL está presionado, o si LCONTROL
        // está presionado SIN que RMENU (AltGr) esté activo.
        let ctrl = rctrl || (lctrl && !rmenu);
        let alt = lmenu;
        let altgr = rmenu;

        (ctrl, shift, alt, altgr)
    }
}

#[cfg(not(target_os = "windows"))]
fn get_physical_modifiers() -> (bool, bool, bool, bool) {
    (false, false, false, false)
}

fn key_is_char(key: &Key, target: char) -> bool {
    match key {
        Key::Character(s) => {
            if let Some(c) = s.chars().next() {
                if c.to_ascii_lowercase() == target.to_ascii_lowercase() {
                    return true;
                }
                let target_lower = target.to_ascii_lowercase();
                if target_lower >= 'a' && target_lower <= 'z' {
                    let code = (target_lower as u8) - b'a' + 1;
                    if c as u32 == code as u32 {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}


/// Obtiene rutas de archivos copiados al portapapeles desde el Explorador de Windows (CF_HDROP)
#[cfg(target_os = "windows")]
fn get_clipboard_files() -> Option<Vec<String>> {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows_sys::Win32::UI::Shell::DragQueryFileW;
    const CF_HDROP: u32 = 15;

    unsafe {
        if OpenClipboard(0) == 0 {
            return None;
        }

        let handle = GetClipboardData(CF_HDROP);
        if handle == 0 {
            CloseClipboard();
            return None;
        }

        let count = DragQueryFileW(handle as _, 0xFFFFFFFF, std::ptr::null_mut(), 0);
        let mut paths = Vec::with_capacity(count as usize);

        for i in 0..count {
            let len = DragQueryFileW(handle as _, i, std::ptr::null_mut(), 0);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                DragQueryFileW(handle as _, i, buf.as_mut_ptr(), len + 1);
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                paths.push(path);
            }
        }

        CloseClipboard();

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_clipboard_files() -> Option<Vec<String>> {
    None
}

/// Comprueba si Claude Code (o un agente CLI compatible con imágenes) se encuentra activo en el sistema
#[cfg(target_os = "windows")]
pub(crate) fn is_claude_running() -> bool {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == 0 || snapshot == -1isize as _ {
            return false;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                if name.contains("claude") {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        windows_sys::Win32::Foundation::CloseHandle(snapshot);
        found
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_claude_running() -> bool {
    false
}



