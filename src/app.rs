use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::renderer::{SidebarTabItem, TerminalRenderer};
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
    PtyData(usize),
    PtyClosed(usize),
}

pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    pub pty: PtyManager,
    pub terminal: TerminalState,
    pub last_grid_size: (usize, usize),
    pub has_unread: bool,
}

pub const SIDEBAR_WIDTH: usize = 190;

pub struct AxonApp {
    config: AxonConfig,
    theme: Theme,
    proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>,
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: usize,
    pub sidebar_visible: bool,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    hovered_new_tab: bool,
    hovered_toggle_btn: bool,
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
    is_focused: bool,
}

impl AxonApp {
    pub fn new(config: AxonConfig, proxy: Option<winit::event_loop::EventLoopProxy<AxonEvent>>) -> Self {
        let cols = 100;
        let rows = 30;
        let theme = Theme::ghostty_dark();
        let pty = PtyManager::new(
            0,
            &config.terminal.default_shell,
            config.terminal.ghostty_prompt,
            cols as u16,
            rows as u16,
            proxy.clone(),
        ).expect("Error al inicializar Windows ConPTY");

        let terminal = TerminalState::new(
            cols,
            rows,
            config.terminal.scrollback_lines,
            theme.clone(),
        );

        let initial_tab = TerminalTab {
            id: 0,
            title: "Terminal 1".to_string(),
            pty,
            terminal,
            last_grid_size: (cols, rows),
            has_unread: false,
        };

        let clipboard = arboard::Clipboard::new().ok();

        Self {
            config,
            theme,
            proxy,
            tabs: vec![initial_tab],
            active_tab_index: 0,
            next_tab_id: 1,
            sidebar_visible: true,
            hovered_tab: None,
            hovered_close: None,
            hovered_new_tab: false,
            hovered_toggle_btn: false,
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
            is_focused: true,
        }
    }

    pub fn sidebar_offset(&self) -> usize {
        if self.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    pub fn create_new_tab(&mut self) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let (cols, rows) = if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            renderer.compute_grid_size(size.width as usize, size.height as usize, self.sidebar_offset())
        } else {
            (100, 30)
        };

        let pty = match PtyManager::new(
            id,
            &self.config.terminal.default_shell,
            self.config.terminal.ghostty_prompt,
            cols as u16,
            rows as u16,
            self.proxy.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                crate::logger::log(&format!("Error al spawnear nuevo PTY: {:?}", e));
                return;
            }
        };

        let title = format!("Terminal {}", self.tabs.len() + 1);
        let terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );

        self.tabs.push(TerminalTab {
            id,
            title,
            pty,
            terminal,
            last_grid_size: (cols, rows),
            has_unread: false,
        });

        self.active_tab_index = self.tabs.len() - 1;
        self.update_window_title();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_tab(&mut self, index: usize, event_loop: &ActiveEventLoop) {
        if index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando Axon");
            event_loop.exit();
            return;
        }

        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        self.tabs[self.active_tab_index].has_unread = false;
        self.update_window_title();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab_index {
            self.active_tab_index = index;
            self.tabs[index].has_unread = false;
            self.update_window_title();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            self.switch_to_tab(next);
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            self.switch_to_tab(prev);
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn update_window_title(&self) {
        if let (Some(w), Some(tab)) = (&self.window, self.tabs.get(self.active_tab_index)) {
            w.set_title(&format!("Axon - {}", tab.title));
        }
    }

    fn recompute_grid_for_all_tabs(&mut self) {
        if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            let offset = self.sidebar_offset();
            let (cols, rows) = renderer.compute_grid_size(size.width as usize, size.height as usize, offset);
            for tab in &mut self.tabs {
                if (cols, rows) != tab.last_grid_size {
                    tab.last_grid_size = (cols, rows);
                    tab.terminal.resize(cols, rows);
                    tab.pty.resize(cols as u16, rows as u16);
                }
            }
        }
    }

    fn paste_text(&self, text: &str) {
        let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
        let active = &self.tabs[self.active_tab_index];
        if active.terminal.bracketed_paste_mode {
            let mut payload = Vec::with_capacity(normalized.len() + 16);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            active.pty.write_input(&payload);
        } else {
            active.pty.write_input(normalized.as_bytes());
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
                self.tabs[self.active_tab_index].pty.write_input(b"\x1bv");
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
            }
            self.recompute_grid_for_all_tabs();
            if let Some(w) = &self.window {
                w.request_redraw();
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
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn reload_config(&mut self) {
        self.config = AxonConfig::load_or_default();
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler<AxonEvent> for AxonApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AxonEvent) {
        match event {
            AxonEvent::PtyData(tab_id) => {
                if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
                    let mut got_bytes = false;
                    while let Some(bytes) = self.tabs[pos].pty.try_recv_output() {
                        got_bytes = true;
                        self.tabs[pos].terminal.process_bytes(&bytes);
                    }
                    for text in self.tabs[pos].terminal.clipboard_queue.drain(..) {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(text).ok();
                        }
                    }
                    if got_bytes {
                        if pos != self.active_tab_index {
                            self.tabs[pos].has_unread = true;
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
            AxonEvent::PtyClosed(tab_id) => {
                crate::logger::log(&format!("PtyClosed recibido para tab_id {}: cerrando tab", tab_id));
                if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
                    self.close_tab(pos, event_loop);
                }
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
            .with_inner_size(PhysicalSize::new(1080, 680))
            .with_min_inner_size(PhysicalSize::new(540, 320))
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

        let offset = self.sidebar_offset();
        let (cols, rows) = renderer.compute_grid_size(initial_size.width as usize, initial_size.height as usize, offset);
        for tab in &mut self.tabs {
            tab.last_grid_size = (cols, rows);
            tab.terminal.resize(cols, rows);
            tab.pty.resize(cols as u16, rows as u16);
        }

        // Pre-renderizar el primer frame oscuro en la superficie antes de hacer visible la ventana.
        // Esto garantiza que el usuario vea la terminal oscura de inmediato sin ningún destello blanco.
        if let Ok(mut buffer) = surface.buffer_mut() {
            if self.sidebar_visible {
                let sidebar_tabs: Vec<SidebarTabItem> = self.tabs.iter().enumerate().map(|(idx, t)| SidebarTabItem {
                    id: t.id,
                    title: t.title.clone(),
                    is_active: idx == self.active_tab_index,
                    has_unread: t.has_unread,
                }).collect();

                renderer.render_sidebar(
                    &mut buffer,
                    initial_size.width as usize,
                    initial_size.height as usize,
                    SIDEBAR_WIDTH,
                    &sidebar_tabs,
                    self.hovered_tab,
                    self.hovered_close,
                    self.hovered_new_tab,
                    self.hovered_toggle_btn,
                );
            }

            let active_tab = &self.tabs[self.active_tab_index];
            renderer.render(
                &mut buffer,
                initial_size.width as usize,
                initial_size.height as usize,
                offset,
                &active_tab.terminal,
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
                if let (Some(renderer), Some(_window)) = (&mut self.renderer, &self.window) {
                    renderer.update_scale(&self.config, scale_factor);
                }
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        let _ = surface.resize(w, h);
                    }
                }
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let offset = self.sidebar_offset();
                let sidebar_visible = self.sidebar_visible;
                let active_idx = self.active_tab_index;
                let hovered_tab = self.hovered_tab;
                let hovered_close = self.hovered_close;
                let hovered_new_tab = self.hovered_new_tab;
                let hovered_toggle_btn = self.hovered_toggle_btn;
                let cursor_visible = !self.config.terminal.cursor_blink
                    || (self.cursor_timer.elapsed().as_millis() % 1000) < 600;

                let sidebar_tabs: Vec<SidebarTabItem> = if sidebar_visible {
                    self.tabs.iter().enumerate().map(|(idx, t)| SidebarTabItem {
                        id: t.id,
                        title: t.title.clone(),
                        is_active: idx == active_idx,
                        has_unread: t.has_unread,
                    }).collect()
                } else {
                    Vec::new()
                };

                let active_tab = &self.tabs[active_idx];

                if let (Some(surface), Some(renderer), Some(window)) =
                    (&mut self.surface, &mut self.renderer, &self.window)
                {
                    let size = window.inner_size();
                    if size.width == 0 || size.height == 0 {
                        return;
                    }

                    if let Ok(mut buffer) = surface.buffer_mut() {
                        if sidebar_visible {
                            renderer.render_sidebar(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                SIDEBAR_WIDTH,
                                &sidebar_tabs,
                                hovered_tab,
                                hovered_close,
                                hovered_new_tab,
                                hovered_toggle_btn,
                            );
                        }

                        renderer.render(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            offset,
                            &active_tab.terminal,
                            &self.theme,
                            cursor_visible,
                        );

                        if !sidebar_visible {
                            renderer.render_floating_sidebar_toggle(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                hovered_toggle_btn,
                            );
                        }

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
                            self.tabs[self.active_tab_index].pty.write_input(t.as_bytes());
                        }
                    }
                    return;
                }

                // Alt + Tab es el atajo de cambio de tarea de Windows; NUNCA debe enviar \t al shell
                if alt && logical_key == Key::Named(NamedKey::Tab) {
                    crate::logger::log("-> Ignorado Alt+Tab");
                    return;
                }

                // Detección robusta de tecla Backspace (por physical_key independiente de layout/Alt y por logical_key)
                let is_backspace = matches!(physical_key, PhysicalKey::Code(KeyCode::Backspace))
                    || matches!(&logical_key, Key::Named(NamedKey::Backspace));

                // 1. Alt + Backspace: Secuencia estándar VT (ESC + DEL: \x1b\x7f).
                //    Activa borrado de palabra (deleteWordLeft / backward-kill-word) en Claude Code (key.meta),
                //    harnesses como agy, shells POSIX y PowerShell (mediante PSReadLineKeyHandler).
                if alt && is_backspace {
                    crate::logger::log("-> Enviando \\x1b\\x7f (Alt+Backspace)");
                    self.tabs[self.active_tab_index].pty.write_input(b"\x1b\x7f");
                    return;
                }

                // 2. Ctrl + Backspace: Byte estándar werase (Ctrl+W: 0x17) utilizado por Windows Terminal.
                //    Activa BackwardKillWord en PowerShell de forma nativa y deleteWordLeft en Claude Code y agy.
                if ctrl && is_backspace {
                    crate::logger::log("-> Enviando 0x17 (Ctrl+Backspace / Ctrl+W)");
                    self.tabs[self.active_tab_index].pty.write_input(&[0x17]);
                    return;
                }

                // 3. Backspace normal (o Shift+Backspace):
                //    Enviar 0x7f para borrar carácter por carácter de forma limpia.
                if is_backspace {
                    crate::logger::log("-> Enviando 0x7f (borrado de caracter)");
                    self.tabs[self.active_tab_index].pty.write_input(&[0x7f]);
                    return;
                }

                // --- ATAJOS DE GESTIÓN DE PESTAÑAS (TABS & SIDEBAR) ---
                // 1. Nueva pestaña / terminal: Ctrl + T (o Ctrl + Shift + T)
                if ctrl && !alt && key_is_char(&logical_key, 't') {
                    crate::logger::log("-> Hotkey: Nueva terminal (Ctrl+T)");
                    self.create_new_tab();
                    return;
                }

                // 2. Cerrar pestaña activa: Ctrl + W (o Ctrl + Shift + W)
                if ctrl && !alt && key_is_char(&logical_key, 'w') {
                    crate::logger::log("-> Hotkey: Cerrar pestaña activa (Ctrl+W)");
                    self.close_tab(self.active_tab_index, event_loop);
                    return;
                }

                // 3. Conmutar pestaña siguiente / anterior: Ctrl + Tab / Ctrl + Shift + Tab
                if ctrl && logical_key == Key::Named(NamedKey::Tab) {
                    if shift {
                        crate::logger::log("-> Hotkey: Pestaña anterior");
                        self.prev_tab();
                    } else {
                        crate::logger::log("-> Hotkey: Pestaña siguiente");
                        self.next_tab();
                    }
                    return;
                }

                // 4. Cambiar directamente a pestaña 1-9: Ctrl + 1..9
                if ctrl && !shift && !alt {
                    let tab_num = match &logical_key {
                        Key::Character(s) if s.len() == 1 => {
                            let ch = s.chars().next().unwrap();
                            if ch >= '1' && ch <= '9' {
                                Some((ch as usize) - ('1' as usize))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(idx) = tab_num {
                        if idx < self.tabs.len() {
                            crate::logger::log(&format!("-> Hotkey: Ir a pestaña {}", idx + 1));
                            self.switch_to_tab(idx);
                            return;
                        }
                    }
                }

                // 5. Alternar barra lateral (Sidebar toggle): Ctrl + B o Ctrl + Shift + B
                if ctrl && key_is_char(&logical_key, 'b') {
                    crate::logger::log("-> Hotkey: Alternar barra lateral (Ctrl+B)");
                    self.toggle_sidebar();
                    return;
                }

                // Siempre devolver scroll al prompt activo al pulsar teclas normales (excepto PageUp/Down y Shift+Home/End)
                if logical_key != Key::Named(NamedKey::PageUp)
                    && logical_key != Key::Named(NamedKey::PageDown)
                    && !(shift && (logical_key == Key::Named(NamedKey::Home) || logical_key == Key::Named(NamedKey::End)))
                {
                    self.tabs[self.active_tab_index].terminal.reset_scroll();
                }

                // 1. Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
                if (ctrl && shift && key_is_char(&logical_key, 'c'))
                    || (ctrl && !shift && key_is_char(&logical_key, 'c') && self.tabs[self.active_tab_index].terminal.selection.is_some())
                {
                    if let Some(sel_text) = self.tabs[self.active_tab_index].terminal.copy_selection_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(sel_text).ok();
                        }
                        // Deseleccionar y repintar tras copiar exitosamente
                        self.tabs[self.active_tab_index].terminal.selection = None;
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
                        self.tabs[self.active_tab_index].pty.write_input(b"\x1bv");
                        return;
                    }

                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.tabs[self.active_tab_index].pty.write_input(b"\x1bb"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.tabs[self.active_tab_index].pty.write_input(b"\x1bf"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            self.tabs[self.active_tab_index].pty.write_input(b"\x1bd"); // Borrado de palabra adelante
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
                                    self.tabs[self.active_tab_index].pty.write_input(&buf[..1 + len]);
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
                    if let Some(output) = self.tabs[self.active_tab_index].terminal.get_last_output_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(output).ok();
                        }
                    }
                    return;
                }

                // 5b. Limpiar pantalla y scrollback buffer al estilo Ghostty (Ctrl + Shift + K)
                if ctrl && shift && (key_is_char(&logical_key, 'k') || key_is_char(&logical_key, 'l')) {
                    self.tabs[self.active_tab_index].terminal.scrollback.clear();
                    self.tabs[self.active_tab_index].terminal.reset_scroll();
                    self.tabs[self.active_tab_index].terminal.command_blocks.clear();
                    self.tabs[self.active_tab_index].pty.write_input(b"\x0c");
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // 6. Navegación semántica entre comandos de agentes OSC 133 (Ctrl + Shift + Arriba/Abajo)
                if ctrl && shift {
                    if let Key::Named(NamedKey::ArrowUp) = logical_key {
                        let active = &mut self.tabs[self.active_tab_index];
                        if let Some(block) = active.terminal.command_blocks.last() {
                            let target = active.terminal.total_lines_count.saturating_sub(block.prompt_line);
                            active.terminal.scroll_offset = target.min(active.terminal.scrollback.len());
                            if let Some(w) = &self.window { w.request_redraw(); }
                        }
                        return;
                    }
                    if let Key::Named(NamedKey::ArrowDown) = logical_key {
                        self.tabs[self.active_tab_index].terminal.reset_scroll();
                        if let Some(w) = &self.window { w.request_redraw(); }
                        return;
                    }
                }

                // 7. Navegación de historial y scrollback (Shift + PageUp / PageDown / Home / End)
                if shift && !ctrl {
                    let active = &mut self.tabs[self.active_tab_index];
                    match logical_key {
                        Key::Named(NamedKey::PageUp) => {
                            let half = (active.terminal.rows / 2).max(1);
                            active.terminal.scroll_up(half);
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let half = (active.terminal.rows / 2).max(1);
                            active.terminal.scroll_down(half);
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            active.terminal.scroll_offset = active.terminal.scrollback.len();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            active.terminal.reset_scroll();
                            if let Some(w) = &self.window { w.request_redraw(); }
                            return;
                        }
                        _ => {}
                    }
                }

                // 8. Salto de palabras y edición en línea de comandos (Ctrl + Flechas, Ctrl + Delete)
                if ctrl && !shift {
                    let active = &mut self.tabs[self.active_tab_index];
                    match logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            active.pty.write_input(b"\x1b[1;5D"); // Salto palabra atrás
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            active.pty.write_input(b"\x1b[1;5C"); // Salto palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            active.pty.write_input(b"\x1b[1;5A");
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            active.pty.write_input(b"\x1b[1;5B");
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            active.pty.write_input(b"\x1b[3;5~"); // Borrar palabra adelante
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            active.pty.write_input(b"\x1b[1;5H");
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            active.pty.write_input(b"\x1b[1;5F");
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
                                self.tabs[self.active_tab_index].pty.write_input(&[code]);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // 10. Scroll por página directo sin Shift
                if logical_key == Key::Named(NamedKey::PageUp) {
                    let active = &mut self.tabs[self.active_tab_index];
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_up(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }
                if logical_key == Key::Named(NamedKey::PageDown) {
                    let active = &mut self.tabs[self.active_tab_index];
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_down(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // 11. Teclas especiales y flechas VT estándar
                let active = &mut self.tabs[self.active_tab_index];
                match logical_key {
                    Key::Named(NamedKey::Enter) => active.pty.write_input(b"\r"),
                    Key::Named(NamedKey::Backspace) => active.pty.write_input(&[0x7f]),
                    Key::Named(NamedKey::Tab) => {
                        if alt {
                            return;
                        }
                        if shift {
                            active.pty.write_input(b"\x1b[Z");
                        } else {
                            active.pty.write_input(b"\t");
                        }
                    }
                    Key::Named(NamedKey::Escape) => active.pty.write_input(&[0x1b]),
                    Key::Named(NamedKey::Delete) => active.pty.write_input(b"\x1b[3~"),
                    Key::Named(NamedKey::Home) => active.pty.write_input(b"\x1b[H"),
                    Key::Named(NamedKey::End) => active.pty.write_input(b"\x1b[F"),
                    Key::Named(NamedKey::ArrowUp) => active.pty.write_input(b"\x1b[A"),
                    Key::Named(NamedKey::ArrowDown) => active.pty.write_input(b"\x1b[B"),
                    Key::Named(NamedKey::ArrowRight) => active.pty.write_input(b"\x1b[C"),
                    Key::Named(NamedKey::ArrowLeft) => active.pty.write_input(b"\x1b[D"),
                    Key::Named(NamedKey::F1) => active.pty.write_input(b"\x1bOP"),
                    Key::Named(NamedKey::F2) => active.pty.write_input(b"\x1bOQ"),
                    Key::Named(NamedKey::F3) => active.pty.write_input(b"\x1bOR"),
                    Key::Named(NamedKey::F4) => active.pty.write_input(b"\x1bOS"),
                    Key::Named(NamedKey::F5) => active.pty.write_input(b"\x1b[15~"),
                    Key::Named(NamedKey::F6) => active.pty.write_input(b"\x1b[17~"),
                    Key::Named(NamedKey::F7) => active.pty.write_input(b"\x1b[18~"),
                    Key::Named(NamedKey::F8) => active.pty.write_input(b"\x1b[19~"),
                    Key::Named(NamedKey::F9) => active.pty.write_input(b"\x1b[20~"),
                    Key::Named(NamedKey::F10) => active.pty.write_input(b"\x1b[21~"),
                    Key::Named(NamedKey::F11) => active.pty.write_input(b"\x1b[23~"),
                    Key::Named(NamedKey::F12) => active.pty.write_input(b"\x1b[24~"),
                    _ => {
                        // Entrada de texto regular (letras, números, símbolos, emojis)
                        if let Some(t) = text {
                            if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                                active.pty.write_input(t.as_bytes());
                            }
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = position;
                let offset = self.sidebar_offset();
                let mx = position.x as usize;
                let my = position.y as usize;

                let win_h = if let Some(w) = &self.window {
                    w.inner_size().height as usize
                } else {
                    600
                };

                // Comprobar hover en botón inferior de alternar barra lateral
                let toggle_btn_w = if self.sidebar_visible { SIDEBAR_WIDTH.saturating_sub(16) } else { 32 };
                let toggle_btn_h = 30;
                let toggle_btn_x = 8;
                let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8);

                let is_hover_toggle = mx >= toggle_btn_x
                    && mx < toggle_btn_x + toggle_btn_w
                    && my >= toggle_btn_y
                    && my < toggle_btn_y + toggle_btn_h;

                let mut needs_redraw = false;
                if self.hovered_toggle_btn != is_hover_toggle {
                    self.hovered_toggle_btn = is_hover_toggle;
                    needs_redraw = true;
                }

                // 1. Detección de hover en la barra lateral si está visible
                if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                    // Hover en botón "+"
                    let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                    let plus_btn_y = 8;
                    let plus_btn_w = 24;
                    let plus_btn_h = 24;
                    let is_new_tab = mx >= plus_btn_x && mx < plus_btn_x + plus_btn_w && my >= plus_btn_y && my < plus_btn_y + plus_btn_h;
                    if self.hovered_new_tab != is_new_tab {
                        self.hovered_new_tab = is_new_tab;
                        needs_redraw = true;
                    }

                    // Hover en pestañas y botón de cerrar "x"
                    let tab_item_h = 34;
                    let tab_spacing = 4;
                    let list_start_y = 42;
                    let tab_x = 8;
                    let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                    let list_max_y = win_h.saturating_sub(44);

                    let mut new_hovered_tab = None;
                    let mut new_hovered_close = None;

                    if my >= list_start_y && my < list_max_y {
                        let rel_y = my - list_start_y;
                        let item_total = tab_item_h + tab_spacing;
                        let tab_idx = rel_y / item_total;
                        let in_item = (rel_y % item_total) < tab_item_h;

                        if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w {
                            new_hovered_tab = Some(tab_idx);

                            if self.tabs.len() > 1 {
                                let tab_y = list_start_y + tab_idx * item_total;
                                let close_x = tab_x + tab_w.saturating_sub(22);
                                let close_y = tab_y + 6;
                                let close_size = 20;

                                if mx >= close_x && mx < close_x + close_size && my >= close_y && my < close_y + close_size {
                                    new_hovered_close = Some(tab_idx);
                                }
                            }
                        }
                    }

                    if self.hovered_tab != new_hovered_tab {
                        self.hovered_tab = new_hovered_tab;
                        needs_redraw = true;
                    }
                    if self.hovered_close != new_hovered_close {
                        self.hovered_close = new_hovered_close;
                        needs_redraw = true;
                    }

                    if needs_redraw {
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                    return;
                } else if self.sidebar_visible {
                    // Si el ratón salió de la barra lateral, resetear estados hover
                    if self.hovered_tab.is_some() { self.hovered_tab = None; needs_redraw = true; }
                    if self.hovered_close.is_some() { self.hovered_close = None; needs_redraw = true; }
                    if self.hovered_new_tab { self.hovered_new_tab = false; needs_redraw = true; }
                    if needs_redraw {
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                } else if needs_redraw {
                    if let Some(w) = &self.window { w.request_redraw(); }
                }

                // 2. Coordenadas relativas a la cuadrícula de la terminal activa
                let bypass = self.modifiers.shift_key();
                let active = &mut self.tabs[self.active_tab_index];

                if active.terminal.is_mouse_reporting_active() && !bypass {
                    // Reportar movimiento/arrastre a Herdr / Ratatui (SGR 1006 / 1002 / 1003)
                    if (active.terminal.mouse_reporting_button_event && self.mouse_down) || active.terminal.mouse_reporting_any_event {
                        if let Some(renderer) = &self.renderer {
                            let term_x = mx.saturating_sub(offset);
                            let col = if term_x >= renderer.margin_x {
                                ((term_x - renderer.margin_x) / renderer.char_width) + 1
                            } else { 1 }.min(active.terminal.cols);
                            let row = if my >= renderer.margin_y {
                                ((my - renderer.margin_y) / renderer.row_height) + 1
                            } else { 1 }.min(active.terminal.rows);

                            let mut code = 32; // Flag de arrastre/movimiento
                            if self.modifiers.shift_key() { code += 4; }
                            if self.modifiers.alt_key() { code += 8; }
                            if self.modifiers.control_key() { code += 16; }

                            if active.terminal.mouse_reporting_sgr {
                                let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                active.pty.write_input(seq.as_bytes());
                            } else {
                                let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                                active.pty.write_input(&seq);
                            }
                        }
                    }
                } else if self.mouse_down {
                    if let Some(renderer) = &self.renderer {
                        let term_x = mx.saturating_sub(offset);
                        if term_x >= renderer.margin_x && my >= renderer.margin_y {
                            let col = (term_x - renderer.margin_x) / renderer.char_width;
                            let row = (my - renderer.margin_y) / renderer.row_height;
                            if let Some(start) = self.drag_start {
                                active.terminal.selection = Some(SelectionRange {
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
                let mx = self.mouse_pos.x as usize;
                let my = self.mouse_pos.y as usize;

                let win_h = if let Some(w) = &self.window {
                    w.inner_size().height as usize
                } else {
                    600
                };

                // Comprobar clic en botón inferior de alternar barra lateral (en ambas modalidades)
                let toggle_btn_w = if self.sidebar_visible { SIDEBAR_WIDTH.saturating_sub(16) } else { 32 };
                let toggle_btn_h = 30;
                let toggle_btn_x = 8;
                let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8);

                let is_click_toggle = mx >= toggle_btn_x
                    && mx < toggle_btn_x + toggle_btn_w
                    && my >= toggle_btn_y
                    && my < toggle_btn_y + toggle_btn_h;

                if is_click_toggle && state == ElementState::Pressed && button == MouseButton::Left {
                    crate::logger::log("-> Clic en botón inferior de alternar barra lateral");
                    self.toggle_sidebar();
                    return;
                }

                // 1. Clics en la barra lateral abierta
                if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                    if state == ElementState::Pressed && button == MouseButton::Left {
                        // Clic en botón "+"
                        let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                        let plus_btn_y = 8;
                        let plus_btn_w = 24;
                        let plus_btn_h = 24;
                        if mx >= plus_btn_x && mx < plus_btn_x + plus_btn_w && my >= plus_btn_y && my < plus_btn_y + plus_btn_h {
                            crate::logger::log("-> Clic en [+] sidebar: nueva terminal");
                            self.create_new_tab();
                            return;
                        }

                        // Clic en pestañas o botón cerrar "x"
                        let tab_item_h = 34;
                        let tab_spacing = 4;
                        let list_start_y = 42;
                        let tab_x = 8;
                        let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                        let list_max_y = win_h.saturating_sub(44);

                        if my >= list_start_y && my < list_max_y {
                            let rel_y = my - list_start_y;
                            let item_total = tab_item_h + tab_spacing;
                            let tab_idx = rel_y / item_total;
                            let in_item = (rel_y % item_total) < tab_item_h;

                            if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w {
                                let tab_y = list_start_y + tab_idx * item_total;
                                let close_x = tab_x + tab_w.saturating_sub(22);
                                let close_y = tab_y + 6;
                                let close_size = 20;

                                if self.tabs.len() > 1 && mx >= close_x && mx < close_x + close_size && my >= close_y && my < close_y + close_size {
                                    crate::logger::log(&format!("-> Clic en [x] pestaña {}", tab_idx));
                                    self.close_tab(tab_idx, event_loop);
                                } else {
                                    crate::logger::log(&format!("-> Clic en pestaña {}", tab_idx));
                                    self.switch_to_tab(tab_idx);
                                }
                                return;
                            }
                        }
                    }
                    return;
                }

                // 2. Clics en la cuadrícula de la terminal activa
                let bypass = self.modifiers.shift_key();
                let offset = self.sidebar_offset();
                let active = &mut self.tabs[self.active_tab_index];

                if active.terminal.is_mouse_reporting_active() && !bypass {
                    // Transmitir evento de ratón a Herdr / Ratatui
                    if let Some(renderer) = &self.renderer {
                        let term_x = mx.saturating_sub(offset);
                        let col = if term_x >= renderer.margin_x {
                            ((term_x - renderer.margin_x) / renderer.char_width) + 1
                        } else { 1 }.min(active.terminal.cols);
                        let row = if my >= renderer.margin_y {
                            ((my - renderer.margin_y) / renderer.row_height) + 1
                        } else { 1 }.min(active.terminal.rows);

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

                        if active.terminal.mouse_reporting_sgr {
                            let suffix = if is_pressed { 'M' } else { 'm' };
                            let seq = format!("\x1b[<{};{};{}{}", code, col, row, suffix);
                            active.pty.write_input(seq.as_bytes());
                        } else {
                            let btn_byte = if is_pressed { 32 + code as u8 } else { 32 + 3 };
                            let seq = [0x1b, b'[', b'M', btn_byte, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                            active.pty.write_input(&seq);
                        }
                    }
                } else {
                    match button {
                        MouseButton::Left => {
                            match state {
                                ElementState::Pressed => {
                                    self.mouse_down = true;
                                    if let Some(renderer) = &self.renderer {
                                        let term_x = mx.saturating_sub(offset);
                                        if term_x >= renderer.margin_x && my >= renderer.margin_y {
                                            let col = (term_x - renderer.margin_x) / renderer.char_width;
                                            let row = (my - renderer.margin_y) / renderer.row_height;
                                            self.drag_start = Some((col, row));
                                            active.terminal.selection = None;
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
                let offset = self.sidebar_offset();
                let mx = self.mouse_pos.x as usize;
                let my = self.mouse_pos.y as usize;

                let active = &mut self.tabs[self.active_tab_index];

                if active.terminal.is_mouse_reporting_active() && !bypass {
                    // Transmitir scroll del ratón al TUI activo (Herdr / Ratatui)
                    if let Some(renderer) = &self.renderer {
                        let term_x = mx.saturating_sub(offset);
                        let col = if term_x >= renderer.margin_x {
                            ((term_x - renderer.margin_x) / renderer.char_width) + 1
                        } else { 1 }.min(active.terminal.cols);
                        let row = if my >= renderer.margin_y {
                            ((my - renderer.margin_y) / renderer.row_height) + 1
                        } else { 1 }.min(active.terminal.rows);

                        let is_up = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                            MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                        };

                        let mut code = if is_up { 64 } else { 65 };
                        if self.modifiers.shift_key() { code += 4; }
                        if self.modifiers.alt_key() { code += 8; }
                        if self.modifiers.control_key() { code += 16; }

                        if active.terminal.mouse_reporting_sgr {
                            let seq = format!("\x1b[<{};{};{}M", code, col, row);
                            active.pty.write_input(seq.as_bytes());
                        } else {
                            let seq = [0x1b, b'[', b'M', 32 + code as u8, 32 + col.min(223) as u8, 32 + row.min(223) as u8];
                            active.pty.write_input(&seq);
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
                        active.terminal.scroll_up(lines);
                    } else {
                        active.terminal.scroll_down(lines);
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

                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    if active.terminal.focus_event_reporting {
                        if focused {
                            active.pty.write_input(b"\x1b[I");
                        } else {
                            active.pty.write_input(b"\x1b[O");
                        }
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 1. Revisar si alguna pestaña tiene su proceso terminado
        let mut exited_indices = Vec::new();
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if tab.pty.is_child_exited() {
                exited_indices.push(idx);
            }
        }
        for idx in exited_indices.into_iter().rev() {
            crate::logger::log(&format!("Proceso hijo de tab {} terminado: cerrando pestaña", idx));
            self.close_tab(idx, event_loop);
        }
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando Axon");
            event_loop.exit();
            return;
        }

        // 2. Drenar salida de ConPTY de todas las pestañas por si quedó algún byte residual
        let mut got_bytes = false;
        let active_idx = self.active_tab_index;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let mut tab_got_bytes = false;
            while let Some(bytes) = tab.pty.try_recv_output() {
                tab_got_bytes = true;
                tab.terminal.process_bytes(&bytes);
            }
            for text in tab.terminal.clipboard_queue.drain(..) {
                if let Some(cb) = &mut self.clipboard {
                    cb.set_text(text).ok();
                }
            }
            if tab_got_bytes {
                got_bytes = true;
                if idx != active_idx {
                    tab.has_unread = true;
                }
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



