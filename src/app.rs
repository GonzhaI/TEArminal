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
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

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
}

impl AxonApp {
    pub fn new(config: AxonConfig) -> Self {
        let cols = 120;
        let rows = 30;
        let theme = Theme::ghostty_dark();

        let pty = PtyManager::new(&config.terminal.default_shell, cols as u16, rows as u16)
            .expect("Error al inicializar Windows ConPTY");

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
        }
    }

    fn paste_text(&self, text: &str) {
        let use_bracketed = self.terminal.bracketed_paste_mode || self.config.terminal.bracketed_paste;
        if use_bracketed {
            let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
            let mut payload = Vec::with_capacity(normalized.len() + 16);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            self.pty.write_input(&payload);
        } else {
            self.pty.write_input(text.as_bytes());
        }
    }
}

impl ApplicationHandler for AxonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Crear icono nativo de Axon (Ghostty Green '>' en fondo oscuro)
        let icon = create_native_icon();

        let window_attributes = WindowAttributes::default()
            .with_title("Axon")
            .with_inner_size(PhysicalSize::new(1020, 680))
            .with_min_inner_size(PhysicalSize::new(480, 280))
            .with_window_icon(Some(icon));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Error al crear ventana nativa de Windows"),
        );

        // Habilitar barra de título oscura inmersiva de Windows 11 (DWM Dark Mode)
        enable_windows_dark_mode(&window);

        let context = softbuffer::Context::new(window.clone())
            .expect("Error al inicializar contexto de aceleración gráfica de pantalla");
        let surface = softbuffer::Surface::new(&context, window.clone())
            .expect("Error al inicializar superficie de renderizado nativo");

        let renderer = TerminalRenderer::new(&self.config);

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
                        ..
                    },
                ..
            } => {
                // Siempre devolver scroll al prompt activo al pulsar tecla
                if logical_key != Key::Named(NamedKey::PageUp) && logical_key != Key::Named(NamedKey::PageDown) {
                    self.terminal.reset_scroll();
                }

                let ctrl = self.modifiers.control_key();
                let shift = self.modifiers.shift_key();

                // Copiar salida del último comando del agente (Ctrl + Shift + O)
                if ctrl && shift && logical_key == Key::Character("O".into()) {
                    if let Some(output) = self.terminal.get_last_output_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(output).ok();
                        }
                    }
                    return;
                }

                // Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
                if (ctrl && shift && logical_key == Key::Character("C".into()))
                    || (ctrl && logical_key == Key::Character("c".into()) && self.terminal.selection.is_some())
                {
                    if let Some(sel_text) = self.terminal.copy_selection_text() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set_text(sel_text).ok();
                        }
                    }
                    return;
                }

                // Pegar (Ctrl + Shift + V o Ctrl + V)
                if (ctrl && shift && logical_key == Key::Character("V".into()))
                    || (ctrl && logical_key == Key::Character("v".into()))
                {
                    if let Some(cb) = &mut self.clipboard {
                        if let Ok(text) = cb.get_text() {
                            self.paste_text(&text);
                        }
                    }
                    return;
                }

                // Navegación semántica entre comandos de agentes (Ctrl + Shift + Arriba/Abajo)
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

                // Combinaciones de control clásicas de terminal (Ctrl+C, Ctrl+D, Ctrl+L, etc.)
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

                // Historial de scroll (PageUp / PageDown)
                if let Key::Named(NamedKey::PageUp) = logical_key {
                    let half = (self.terminal.rows / 2).max(1);
                    self.terminal.scroll_up(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }
                if let Key::Named(NamedKey::PageDown) = logical_key {
                    let half = (self.terminal.rows / 2).max(1);
                    self.terminal.scroll_down(half);
                    if let Some(w) = &self.window { w.request_redraw(); }
                    return;
                }

                // Teclas especiales y flechas VT
                match logical_key {
                    Key::Named(NamedKey::Enter) => self.pty.write_input(b"\r"),
                    Key::Named(NamedKey::Backspace) => self.pty.write_input(&[0x08]),
                    Key::Named(NamedKey::Tab) => self.pty.write_input(b"\t"),
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
                if self.mouse_down {
                    if let Some(renderer) = &self.renderer {
                        let mx = position.x as usize;
                        let my = position.y as usize;
                        if mx >= 12 && my >= 10 {
                            let col = (mx - 12) / renderer.char_width;
                            let row = (my - 10) / renderer.row_height;
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

            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.mouse_down = true;
                        if let Some(renderer) = &self.renderer {
                            let mx = self.mouse_pos.x as usize;
                            let my = self.mouse_pos.y as usize;
                            if mx >= 12 && my >= 10 {
                                let col = (mx - 12) / renderer.char_width;
                                let row = (my - 10) / renderer.row_height;
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

            WindowEvent::MouseWheel { delta, .. } => {
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

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // 1. Drenar salida de ConPTY
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

        // 3. Solicitar repintado si llegaron nuevos bytes del PTY o para parpadeo de cursor
        if got_bytes {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        } else if self.config.terminal.cursor_blink {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
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
                let dark_mode: i32 = 1;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    20, // DWMWA_USE_IMMERSIVE_DARK_MODE
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_windows_dark_mode(_window: &Window) {}
