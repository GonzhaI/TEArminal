//! Eventos de winit de UNA ventana: `AxonHost` (host.rs) los enruta hasta aquí y cada uno se despacha
//! al método que lo trata.

use super::*;

impl AxonApp {
    /// Evento de usuario de esta ventana (`PtyData`/`PtyClosed` de uno de sus paneles, o `SshDone`)
    pub(super) fn on_user_event(&mut self, event: AxonEvent) {
        match event {
            AxonEvent::SshDone => {
                self.process_ssh_results();
            }
            AxonEvent::FsChanged => {
                if self.file_explorer.visible && self.file_explorer.remote_session.is_none() {
                    self.file_explorer.refresh();
                    self.pump_explorer_ssh();
                    if !self.occluded {
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
            AxonEvent::UpdateCheckDone => {
                if !self.occluded {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            AxonEvent::PtyData(pane_id) => {
                let osc52 = self.config.terminal.osc52;
                let mut redraw = false;
                let mut should_sync_root = false;
                let mut should_update_title = false;
                let active_tab_index = self.active_tab_index;
                for (tab_idx, tab) in self.tabs.iter_mut().enumerate() {
                    if let Some(pane) = tab.find_pane_mut(pane_id) {
                        let mut got_bytes = false;
                        let mut chunks = 0;
                        while let Some(bytes) = pane.pty.try_recv_output() {
                            got_bytes = true;
                            pane.terminal.process_bytes(&bytes);
                            chunks += 1;
                            if chunks >= 32 {
                                break;
                            }
                        }
                        if chunks >= 32 {
                            if let Some(p) = &self.proxy {
                                let _ = p.send_event(AxonEvent::PtyData(pane_id));
                            }
                        }
                        for data in pane.terminal.pty_write_queue.drain(..) {
                            pane.pty.write_input(&data);
                        }
                        for text in pane.terminal.clipboard_queue.drain(..) {
                            // OSC 52 solo con `terminal.osc52`: si no, se descarta (nada de portapapeles silencioso)
                            if let (true, Some(cb)) = (osc52, &mut self.clipboard) {
                                cb.set_text(text).ok();
                            }
                        }
                        let in_sync = pane.terminal.is_in_sync_update();
                        if got_bytes {
                            if tab_idx != active_tab_index {
                                tab.has_unread = true;
                            } else {
                                should_sync_root = true;
                                should_update_title = true;
                            }
                            if !in_sync {
                                redraw = true;
                            }
                        }
                        break;
                    }
                }
                if should_sync_root {
                    self.sync_file_explorer_root();
                }
                if should_update_title {
                    self.update_window_title();
                }
                if redraw {
                    self.process_remote_results();
                }
                if redraw && !self.occluded {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            AxonEvent::PtyClosed(pane_id) => {
                crate::logger::log(&format!("PtyClosed recibido para pane_id {}", pane_id));
                // Drenar cualquier byte residual que haya quedado en el canal antes de cerrar
                for tab in &mut self.tabs {
                    if let Some(pane) = tab.find_pane_mut(pane_id) {
                        while let Some(bytes) = pane.pty.try_recv_output() {
                            pane.terminal.process_bytes(&bytes);
                        }
                    }
                }
                // El EOF del lector de un PTY ya sustituido (shell reabierto tras un agente) no debe cerrar
                // el panel nuevo: solo cuenta si el hijo del PTY actual ha terminado de verdad.
                let exited = self
                    .tabs
                    .iter_mut()
                    .find_map(|t| t.find_pane_mut(pane_id))
                    .is_some_and(|p| p.pty.is_child_exited());
                if exited {
                    self.on_shell_exit(pane_id);
                }
            }
        }
    }

    /// Crea la ventana nativa y su superficie (una vez que el bucle de eventos está activo)
    pub(super) fn init_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Crear icono nativo de Axon ('>' verde sin fondo)
        let icon = create_native_icon();

        let window_attributes = WindowAttributes::default()
            .with_title("Axon")
            .with_visible(false)
            .with_inner_size(PhysicalSize::new(1080, 680))
            .with_min_inner_size(PhysicalSize::new(540, 320))
            .with_decorations(false)
            .with_resizable(true)
            .with_window_icon(Some(icon));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Error al crear ventana nativa de Windows"),
        );

        // Configurar tema nativo inmersivo de Windows 11 (DWM Dark/Light Mode y Mica)
        update_windows_theme(&window, &self.theme);
        apply_window_opacity_native(&window, self.config.appearance.window_opacity);

        let init_win_w = 1080;
        let init_win_h = 680;

        // Intentar inicializar aceleración por GPU nativa en Windows (Direct3D 11 DirectFlip)
        let mut d3d11_presenter = None;
        if self.config.terminal.gpu {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                    let hwnd = windows::Win32::Foundation::HWND(win32.hwnd.get() as _);
                    if let Ok(presenter) = D3D11Presenter::new(hwnd, init_win_w, init_win_h) {
                        d3d11_presenter = Some(presenter);
                    }
                }
            }
        }

        let context = softbuffer::Context::new(window.clone())
            .expect("Error al inicializar contexto de aceleración gráfica de pantalla");
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .expect("Error al inicializar superficie de renderizado nativo");

        let scale_factor = window.scale_factor();
        let renderer = TerminalRenderer::new(&self.config, scale_factor);

        let _ = window.request_inner_size(PhysicalSize::new(init_win_w, init_win_h));
        let initial_size = window.inner_size();
        if let Some(d3d11) = &mut d3d11_presenter {
            let _ = d3d11.resize(initial_size.width, initial_size.height);
        } else if let (Some(w), Some(h)) = (
            NonZeroU32::new(initial_size.width),
            NonZeroU32::new(initial_size.height),
        ) {
            let _ = surface.resize(w, h);
        }

        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();

        let total_w = (initial_size.width as usize).saturating_sub(offset + right_offset);
        let total_h = (initial_size.height as usize).saturating_sub(top_offset + status_bar_h);
        for tab in &mut self.tabs {
            let pane_rects = tab.compute_pane_rects(offset, top_offset, total_w, total_h);
            for rect in pane_rects {
                if let Some(pane) = tab.find_pane_mut(rect.pane_id) {
                    let (cols, rows) = renderer.compute_pane_grid_size(rect.width, rect.height);
                    pane.last_grid_size = (cols, rows);
                    pane.terminal.resize(cols, rows);
                    pane.pty.resize(cols as u16, rows as u16);
                }
            }
        }

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.d3d11_presenter = d3d11_presenter;
        self.renderer = Some(renderer);

        // Pre-renderizar el primer fotograma completo antes de hacer visible la ventana
        // para evitar cualquier destello blanco y respetar la disposición activa (horizontal o vertical).
        self.on_redraw();
        if let Some(w) = &self.window {
            w.set_visible(true);
        }
    }

    pub(super) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    pub(super) fn on_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.request_close(),

            WindowEvent::Moved(pos) => {
                crate::logger::log(&format!("MOVED: {},{}", pos.x, pos.y));
                self.last_window_move = Instant::now();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                crate::logger::log(&format!("SCALE_FACTOR: {}", scale_factor));
                self.last_window_move = Instant::now();
                if let (Some(renderer), Some(_window)) = (&mut self.renderer, &self.window) {
                    renderer.update_scale(&self.config, scale_factor);
                }
                self.clamp_sidebar_scroll();
                self.schedule_pty_resize();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                crate::logger::log(&format!("RESIZED: {}x{}", size.width, size.height));
                self.last_window_move = Instant::now();
                if let Some(d3d11) = &mut self.d3d11_presenter {
                    let _ = d3d11.resize(size.width, size.height);
                } else if let Some(surface) = &mut self.surface {
                    if let (Some(w), Some(h)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    {
                        let _ = surface.resize(w, h);
                    }
                }
                self.clamp_sidebar_scroll();
                self.schedule_pty_resize();
                // Repintado síncrono: con `request_redraw` el WM_PAINT llega tarde (o se agrupa)
                // dentro del bucle modal de Windows y el fotograma queda a la zaga del borde.
                self.on_redraw();
            }

            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => self.on_redraw(),

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        text,
                        state,
                        physical_key,
                        repeat,
                        ..
                    },
                is_synthetic,
                ..
            } => {
                // winit re-emite como "pulsadas" las teclas que siguen abajo al recuperar el foco
                // (p. ej. Enter o flechas al mover la ventana de monitor); no son pulsaciones reales.
                if !is_synthetic {
                    let ev_type = match state {
                        ElementState::Pressed => {
                            self.cursor_on = true;
                            self.cursor_timer = Instant::now();
                            if repeat {
                                KeyEventType::Repeat
                            } else {
                                KeyEventType::Press
                            }
                        }
                        ElementState::Released => KeyEventType::Release,
                    };
                    self.on_key_event(logical_key, text, physical_key, ev_type);
                }
            }

            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),

            WindowEvent::MouseInput { state, button, .. } => self.on_mouse_input(state, button),

            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),

            WindowEvent::Focused(focused) => {
                crate::logger::log(&format!("FOCUS: {}", focused));
                self.is_focused = focused;
                if !focused {
                    self.context_menu = None;
                }
                if !focused {
                    self.modifiers = ModifiersState::empty();
                    self.mouse_down = false;
                    self.drag_start = None;
                    self.drag_anchor = None;
                    self.divider_drag = None;
                    self.tab_drag = None;
                } else {
                    self.modifiers = ModifiersState::empty();
                    self.last_focus_gained = Instant::now();
                    self.cursor_on = true;
                    self.cursor_timer = Instant::now();
                    if self.file_explorer.visible && self.file_explorer.remote_session.is_none() {
                        self.file_explorer.refresh();
                        self.pump_explorer_ssh();
                        self.sync_fs_watcher();
                    }
                }

                // En Windows ConPTY, las secuencias de foco VT (\x1b[I / \x1b[O) no son reconocidas
                // en la entrada de la consola y ConPTY las traduce erróneamente a eventos de teclado (Enter/Escape),
                // provocando prompts vacíos adicionales en PowerShell cada vez que se cambia de ventana.
                #[cfg(not(target_os = "windows"))]
                {
                    if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                        if let Some(pane) = active.active_pane_mut() {
                            if pane.terminal.focus_event_reporting {
                                if focused {
                                    pane.pty.write_input(b"\x1b[I");
                                } else {
                                    pane.pty.write_input(b"\x1b[O");
                                }
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    /// Trabajo periódico de la ventana tras cada tanda de eventos. Devuelve el instante en que debe volver a
    /// despertar (`None` = dormir hasta el próximo evento).
    pub(super) fn tick(&mut self) -> Option<Instant> {
        // 1. Drenar salida de ConPTY de todos los paneles antes de comprobar salidas
        let mut got_bytes = false;
        let mut has_more_pty_data = false;
        let active_idx = self.active_tab_index;
        let osc52 = self.config.terminal.osc52;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let mut tab_got_bytes = false;
            for pane in &mut tab.panes {
                let mut pane_got_bytes = false;
                let mut chunks = 0;
                while let Some(bytes) = pane.pty.try_recv_output() {
                    pane_got_bytes = true;
                    pane.terminal.process_bytes(&bytes);
                    chunks += 1;
                    if chunks >= 32 {
                        has_more_pty_data = true;
                        break;
                    }
                }
                for data in pane.terminal.pty_write_queue.drain(..) {
                    pane.pty.write_input(&data);
                }
                for text in pane.terminal.clipboard_queue.drain(..) {
                    if let (true, Some(cb)) = (osc52, &mut self.clipboard) {
                        cb.set_text(text).ok();
                    }
                }
                if pane_got_bytes {
                    tab_got_bytes = true;
                }
            }
            if tab_got_bytes {
                got_bytes = true;
                if idx != active_idx {
                    tab.has_unread = true;
                }
            }
        }

        if got_bytes {
            self.process_remote_results();
        }

        // 2. Revisar si algún panel tiene su proceso terminado (tras haber consumido toda su salida)
        let mut closed_panes = Vec::new();
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                if pane.pty.is_child_exited() {
                    closed_panes.push(pane.id);
                }
            }
        }
        for pane_id in closed_panes {
            crate::logger::log(&format!("Proceso hijo de pane {} terminado", pane_id));
            self.on_shell_exit(pane_id);
        }
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando la ventana");
            self.request_close();
            return None;
        }

        // Listados SSH que el explorador pidió desde cualquier clic: se ejecutan en hilos
        self.pump_explorer_ssh();

        // 3. Solicitar repintado si llegaron nuevos bytes del PTY
        if got_bytes && !self.occluded {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        // 4. Próximo despertar = el más cercano entre parpadeo del cursor, auto-ocultado de la barra de scroll y sondeo del tema del sistema. Sin temporizadores
        //    activos el bucle duerme indefinidamente (ControlFlow::Wait): 0 % CPU en reposo.
        fn earliest(slot: &mut Option<Instant>, t: Instant) {
            *slot = Some(slot.map_or(t, |w| w.min(t)));
        }
        let mut wake: Option<Instant> = None;
        let now = Instant::now();

        // Peticiones en banda al servidor SSH sin respuesta: vía lenta al vencer el plazo
        if let Some(t) = self.expire_remote_requests() {
            earliest(&mut wake, t);
        }

        // Tamaño final tras un redimensionado: una sola orden de resize al PTY cuando la ventana se estabiliza
        if let Some(deadline) = self.resize_deadline {
            if now >= deadline {
                self.resize_deadline = None;
                self.recompute_grid_for_all_tabs();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            } else {
                earliest(&mut wake, deadline);
            }
        }

        // Parpadeo del cursor: solo con foco y ventana visible
        if self.config.terminal.cursor_blink && self.is_focused && !self.occluded {
            const BLINK: std::time::Duration = std::time::Duration::from_millis(500);
            if self.cursor_timer.elapsed() >= BLINK {
                self.cursor_timer = Instant::now();
                self.cursor_on = !self.cursor_on;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            earliest(&mut wake, self.cursor_timer + BLINK);
        }

        // Spinner de la pestaña que ejecuta: solo con foco y a la vista. 110 ms por paso, el mismo
        // compás que dibuja la barra superior; sin pestañas en curso no arma ningún despertar.
        if self.is_focused && !self.occluded {
            const SPINNER: std::time::Duration = std::time::Duration::from_millis(110);
            let running = self
                .tabs
                .iter()
                .any(|t| super::projects::session_status(t) == Some(SessionStatus::Running));
            if running {
                if self.spinner_timer.elapsed() >= SPINNER {
                    self.spinner_timer = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                earliest(&mut wake, self.spinner_timer + SPINNER);
            }
        }

        // Auto-ocultamiento de la barra de desplazamiento: un repintado exacto al expirar
        let hide_at = self.last_scroll_time + std::time::Duration::from_millis(1250);
        if now < hide_at {
            self.scroll_hide_armed = true;
            earliest(&mut wake, hide_at);
        } else if self.scroll_hide_armed {
            self.scroll_hide_armed = false;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        // Autoguardado diferido tras dejar de escribir (400 ms de inactividad)
        if self.config.terminal.auto_save {
            const AUTOSAVE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(400);
            let mut autosaved_local = false;
            for tab in &mut self.tabs {
                if let Some(viewer) = tab.file_viewer.as_mut() {
                    if viewer.needs_autosave && viewer.remote_destination.is_none() {
                        let deadline = viewer.last_edit_time + AUTOSAVE_DEBOUNCE;
                        if now >= deadline {
                            viewer.autosave_local();
                            autosaved_local = true;
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        } else {
                            earliest(&mut wake, deadline);
                        }
                    }
                }
            }
            if autosaved_local
                && self.file_explorer.visible
                && self.file_explorer.remote_session.is_none()
            {
                self.file_explorer.refresh_git_status();
                self.pump_explorer_ssh();
            }
        }

        // Tema del sistema (cada 2 s) si theme_mode es "system"
        if self
            .config
            .appearance
            .theme_mode
            .eq_ignore_ascii_case("system")
        {
            if self.last_system_theme_check.elapsed().as_secs() >= 2 {
                self.last_system_theme_check = Instant::now();
                let is_sys_light = crate::theme::is_windows_system_light_mode();
                if is_sys_light != self.system_light {
                    crate::logger::log(&format!("-> Cambio de tema del sistema Windows detectado (light={}): actualizando interfaz", is_sys_light));
                    self.apply_active_theme();
                }
            }
            earliest(
                &mut wake,
                self.last_system_theme_check + std::time::Duration::from_secs(2),
            );
        }

        if has_more_pty_data {
            earliest(&mut wake, now);
        }

        // Renderizado sincronizado (DEC 2026): timeout de seguridad para repintar si la app retuvo el fotograma más de 200 ms
        for tab in &self.tabs {
            for pane in &tab.panes {
                if pane.terminal.sync_update_mode {
                    if let Some(start) = pane.terminal.sync_update_start {
                        let deadline = start + std::time::Duration::from_millis(200);
                        if now >= deadline {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        } else {
                            earliest(&mut wake, deadline);
                        }
                    }
                }
            }
        }

        wake
    }
}
