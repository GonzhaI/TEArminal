//! Entrada de teclado.

use super::*;

impl AxonApp {
    /// Teclado: atajos de la aplicación y traducción de teclas a bytes para el PTY con soporte de eventos Kitty.
    pub(super) fn on_key_event(
        &mut self,
        logical_key: Key,
        text: Option<SmolStr>,
        physical_key: PhysicalKey,
        event_type: KeyEventType,
    ) {
        // Si la terminal no tiene el foco activo, ignorar la entrada de teclado
        if !self.is_focused {
            return;
        }

        // En eventos de liberación (Release), solo procesar si Kitty Keyboard Protocol tiene activo el flag 2 (report event types)
        if event_type == KeyEventType::Release {
            if let Some(active) = self.tabs[self.active_tab_index].active_pane() {
                let flags = active.terminal.kitty_keyboard_flags;
                if (flags & 2) != 0 {
                    #[cfg(target_os = "windows")]
                    let (ctrl, shift, alt, _altgr, win) = get_physical_modifiers();
                    #[cfg(not(target_os = "windows"))]
                    let (ctrl, shift, alt, _altgr, win) = (
                        self.modifiers.control_key(),
                        self.modifiers.shift_key(),
                        self.modifiers.alt_key(),
                        false,
                        self.modifiers.super_key(),
                    );
                    if let Some(seq) = encode_kitty_key(
                        &logical_key,
                        physical_key,
                        ctrl,
                        shift,
                        alt,
                        win,
                        event_type,
                        flags,
                    ) {
                        active.pty.write_input(&seq);
                    }
                }
            }
            return;
        }

        // Con el menú contextual abierto, Escape solo lo cierra; cualquier otra tecla lo cierra y sigue su curso
        let is_modifier = matches!(
            logical_key,
            Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
        );
        if !is_modifier && self.context_menu.take().is_some() {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            if logical_key == Key::Named(NamedKey::Escape) {
                return;
            }
        }

        // Debounce de foco: suprimir Enter/Tab residuales generados al alternar ventanas con Alt+Tab / Task View
        if self.last_focus_gained.elapsed().as_millis() < 150
            && (logical_key == Key::Named(NamedKey::Enter)
                || logical_key == Key::Named(NamedKey::Tab))
        {
            crate::logger::log("-> Ignorado Enter/Tab sintético residual de foco");
            return;
        }

        #[cfg(target_os = "windows")]
        let (ctrl, shift, alt, altgr, win) = get_physical_modifiers();

        #[cfg(not(target_os = "windows"))]
        let (ctrl, shift, alt, altgr, win) = (
            self.modifiers.control_key(),
            self.modifiers.shift_key(),
            self.modifiers.alt_key(),
            false,
            self.modifiers.super_key(),
        );

        // Si la tecla Windows/Super está presionada (o atajos de Windows como Win+Shift+Flechas):
        if win || self.modifiers.super_key() {
            crate::logger::log("-> Ignorada tecla con Windows/Super activo");
            return;
        }

        // Debounce de movimiento de ventana / cambio de monitor (Win+Shift+Flechas o arrastre)
        if self.last_window_move.elapsed().as_millis() < 250
            && matches!(
                logical_key,
                Key::Named(NamedKey::Enter)
                    | Key::Named(NamedKey::ArrowUp)
                    | Key::Named(NamedKey::ArrowDown)
                    | Key::Named(NamedKey::ArrowLeft)
                    | Key::Named(NamedKey::ArrowRight)
            )
        {
            crate::logger::log(
                "-> Ignorada tecla de navegación/enter residual tras mover de monitor",
            );
            return;
        }

        // Solo con AXON_LOG_KEYS=1; nunca se registra el texto producido, solo su longitud
        if crate::logger::keys_enabled() {
            crate::logger::log(&format!(
                "KEY: phys={:?}, log={:?}, text_len={}, ctrl={}, shift={}, alt={}, altgr={}",
                physical_key,
                logical_key,
                text.as_ref().map_or(0, |t| t.len()),
                ctrl,
                shift,
                alt,
                altgr
            ));
        }

        // Si AltGr está presionado (teclados en español para @, #, \, ~, etc.),
        // procesar exclusivamente como entrada de texto si genera caracteres imprimibles
        let mut alt = alt;
        let mut ctrl = ctrl;
        if altgr {
            if let Some(ref t) = text {
                if !t.is_empty() && t != "\r" && t != "\n" && t != "\t" {
                    if crate::logger::keys_enabled() {
                        crate::logger::log(&format!("-> AltGr text ({} bytes)", t.len()));
                    }
                    if let Some(active_pane) = self.tabs[self.active_tab_index].active_pane() {
                        active_pane.pty.write_input(t.as_bytes());
                        return;
                    }
                    let visible = self.viewer_visible_lines();
                    if let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_mut() {
                        if !viewer.is_read_only {
                            for ch in t.chars() {
                                if !ch.is_control() {
                                    viewer.insert_char(ch);
                                }
                            }
                            if self.config.terminal.auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }
                    return;
                }
            }
            // Si AltGr no generó texto imprimible (teclas como D, V, W, etc.),
            // en Windows AltGr emula por hardware/driver la combinación Ctrl + Alt.
            // Permitir que atajos como Ctrl + Alt + D se activen limpiamente con Right Alt / AltGr.
            alt = true;
            ctrl = true;
        }

        // Alt + Tab es el atajo de cambio de tarea de Windows; NUNCA debe enviar \t al shell
        if alt && logical_key == Key::Named(NamedKey::Tab) {
            crate::logger::log("-> Ignorado Alt+Tab");
            return;
        }

        // Si estamos en la pestaña de Ajustes, capturar Escape, Ctrl+, o Ctrl+W para cerrarla/alternarla
        let active_is_settings = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| t.is_settings)
            .unwrap_or(false);
        if active_is_settings {
            if self.settings_page_state.editing_setting == Some(EditingSettingField::ThemeSearch) {
                self.edit_theme_search(&logical_key, text.as_deref());
                return;
            }
            // Edición inline de campo numérico
            if let Some(editing_field) = self.settings_page_state.editing_setting {
                match &logical_key {
                    Key::Named(NamedKey::Enter) => {
                        let input = self
                            .settings_page_state
                            .setting_input_buffer
                            .trim()
                            .replace('%', "");
                        match editing_field {
                            EditingSettingField::FontSize => {
                                if let Ok(val) = input.parse::<f32>() {
                                    let clamped = val.clamp(8.0, 32.0);
                                    let delta = clamped - self.config.appearance.font_size;
                                    self.adjust_font_size(delta);
                                }
                            }
                            EditingSettingField::UiFontSize => {
                                if let Ok(val) = input.parse::<f32>() {
                                    let clamped = val.clamp(8.0, 32.0);
                                    let delta = clamped - self.config.appearance.ui_font_size;
                                    self.adjust_ui_font_size(delta);
                                }
                            }
                            EditingSettingField::LineSpacing => {
                                if let Ok(val) = input.parse::<f32>() {
                                    let clamped = (val.clamp(0.5, 2.5) * 100.0).round() / 100.0;
                                    self.config.appearance.line_spacing = clamped;
                                    self.recompute_grid_for_all_tabs();
                                    let _ = self.config.save();
                                }
                            }
                            EditingSettingField::ThemeSearch => {}
                            EditingSettingField::WindowOpacity => {
                                if let Ok(mut val) = input.parse::<f32>() {
                                    if val > 1.0 {
                                        val /= 100.0;
                                    }
                                    let clamped = (val.clamp(0.20, 1.0) * 100.0).round() / 100.0;
                                    self.config.appearance.window_opacity = clamped;
                                    let _ = self.config.save();
                                    if let Some(w) = &self.window {
                                        apply_window_opacity_native(w, clamped);
                                    }
                                }
                            }
                        }
                        self.settings_page_state.editing_setting = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                    Key::Named(NamedKey::Escape) => {
                        self.settings_page_state.editing_setting = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.settings_page_state.setting_input_buffer.pop();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                    _ => {
                        if let Some(ref t) = text {
                            for ch in t.chars() {
                                if (ch.is_ascii_digit() || ch == '.')
                                    && self.settings_page_state.setting_input_buffer.len() < 8
                                {
                                    self.settings_page_state.setting_input_buffer.push(ch);
                                }
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                    }
                }
            }

            if matches!(&logical_key, Key::Named(NamedKey::Escape))
                || (ctrl && !shift && key_is_char(&logical_key, physical_key, ','))
            {
                crate::logger::log("-> Cerrando / alternando pestaña de Ajustes");
                self.open_or_switch_settings_tab();
                return;
            }
            if ctrl && !shift && key_is_char(&logical_key, physical_key, 'w') {
                crate::logger::log("-> Cerrando pestaña de Ajustes con Ctrl+W");
                self.close_tab(self.active_tab_index);
                return;
            }
        }

        // Abrir / alternar pestaña de Ajustes (Ctrl + ,)
        if ctrl && !shift && key_is_char(&logical_key, physical_key, ',') {
            crate::logger::log("-> Hotkey: Abrir / alternar Ajustes (Ctrl+,)");
            self.open_or_switch_settings_tab();
            return;
        }

        // --- ATAJOS DE PANELES DIVIDIDOS (SPLIT PANES) CANÓNICOS (Alt+Shift o Ctrl+Alt) ---
        let is_d = key_is_char(&logical_key, physical_key, 'd');
        let is_v = key_is_char(&logical_key, physical_key, 'v');
        let is_w = key_is_char(&logical_key, physical_key, 'w');
        let is_split_mod = (alt && shift) || (ctrl && alt);

        // 1. División Horizontal: Alt + Shift + D (o Ctrl + Alt + D) -> Lanzar terminal debajo
        if is_split_mod && is_d {
            crate::logger::log("-> Hotkey: Split Horizontal (Alt+Shift+D)");
            self.split_active_pane(SplitDirection::Horizontal);
            return;
        }

        // 2. División Vertical: Alt + Shift + V (o Ctrl + Alt + V) -> Lanzar terminal al lado
        if is_split_mod && is_v {
            crate::logger::log("-> Hotkey: Split Vertical (Alt+Shift+V)");
            self.split_active_pane(SplitDirection::Vertical);
            return;
        }

        // 3. Cerrar panel activo en split: Alt + Shift + W (o Ctrl + Alt + W)
        if is_split_mod && is_w {
            crate::logger::log("-> Hotkey: Cerrar panel activo (Alt+Shift+W)");
            self.close_active_pane();
            return;
        }

        // 4. Navegación direccional entre paneles: Alt + Shift + Flechas (o Ctrl + Alt + Flechas)
        if is_split_mod {
            let is_up = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowUp))
                || logical_key == Key::Named(NamedKey::ArrowUp);
            let is_down = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowDown))
                || logical_key == Key::Named(NamedKey::ArrowDown);
            let is_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                || logical_key == Key::Named(NamedKey::ArrowLeft);
            let is_right = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                || logical_key == Key::Named(NamedKey::ArrowRight);

            if is_up {
                self.navigate_pane(PaneDirection::Up);
                return;
            }
            if is_down {
                self.navigate_pane(PaneDirection::Down);
                return;
            }
            if is_left {
                self.navigate_pane(PaneDirection::Left);
                return;
            }
            if is_right {
                self.navigate_pane(PaneDirection::Right);
                return;
            }
        }

        // --- ATAJOS DE GESTIÓN DE PESTAÑAS (TABS & SIDEBAR) CANÓNICOS ---
        // 1. Nueva pestaña / terminal: Ctrl + T
        if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 't') {
            crate::logger::log("-> Hotkey: Nueva terminal (Ctrl+T)");
            self.create_new_tab();
            return;
        }

        // Nueva ventana: Ctrl + Shift + N
        if ctrl && shift && !alt && key_is_char(&logical_key, physical_key, 'n') {
            crate::logger::log("-> Hotkey: Nueva ventana (Ctrl+Shift+N)");
            self.new_window_request = true;
            return;
        }

        // 2. Cerrar pestaña activa: Ctrl + W
        if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 'w') {
            crate::logger::log("-> Hotkey: Cerrar pestaña activa (Ctrl+W)");
            self.close_tab(self.active_tab_index);
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
                Key::Character(s) if matches!(s.as_bytes(), [b'1'..=b'9']) => {
                    Some((s.as_bytes()[0] - b'1') as usize)
                }
                _ => match physical_key {
                    PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => Some(0),
                    PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => Some(1),
                    PhysicalKey::Code(KeyCode::Digit3 | KeyCode::Numpad3) => Some(2),
                    PhysicalKey::Code(KeyCode::Digit4 | KeyCode::Numpad4) => Some(3),
                    PhysicalKey::Code(KeyCode::Digit5 | KeyCode::Numpad5) => Some(4),
                    PhysicalKey::Code(KeyCode::Digit6 | KeyCode::Numpad6) => Some(5),
                    PhysicalKey::Code(KeyCode::Digit7 | KeyCode::Numpad7) => Some(6),
                    PhysicalKey::Code(KeyCode::Digit8 | KeyCode::Numpad8) => Some(7),
                    PhysicalKey::Code(KeyCode::Digit9 | KeyCode::Numpad9) => Some(8),
                    _ => None,
                },
            };
            if let Some(idx) = tab_num {
                if idx < self.tabs.len() {
                    crate::logger::log(&format!("-> Hotkey: Ir a pestaña {}", idx + 1));
                    self.switch_to_tab(idx);
                    return;
                }
            }
        }

        // 5. Alternar barra lateral (Sidebar toggle): Ctrl + B
        if ctrl && !alt && !shift && key_is_char(&logical_key, physical_key, 'b') {
            crate::logger::log("-> Hotkey: Alternar barra lateral (Ctrl+B)");
            self.toggle_sidebar();
            return;
        }

        // Alternar / enfocar explorador lateral de archivos: Ctrl + Shift + E o Alt + E
        let is_e = key_is_char(&logical_key, physical_key, 'e');
        if (ctrl && shift && !alt && is_e) || (alt && !ctrl && !shift && is_e) {
            crate::logger::log("-> Hotkey: Alternar / enfocar explorador de archivos");
            if !self.file_explorer.visible {
                self.toggle_file_explorer();
                self.file_explorer.is_focused = true;
                if self.file_explorer.selected_index.is_none()
                    && !self.file_explorer.items.is_empty()
                {
                    self.file_explorer.selected_index = Some(0);
                    self.file_explorer.selected_path =
                        Some(self.file_explorer.items[0].path.clone());
                }
            } else if !self.file_explorer.is_focused {
                self.file_explorer.is_focused = true;
                if self.file_explorer.selected_index.is_none()
                    && !self.file_explorer.items.is_empty()
                {
                    self.file_explorer.selected_index = Some(0);
                    self.file_explorer.selected_path =
                        Some(self.file_explorer.items[0].path.clone());
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            } else {
                self.toggle_file_explorer();
            }
            return;
        }

        // 6. Pegar desde el portapapeles: Ctrl + V
        if ctrl && !shift && key_is_char(&logical_key, physical_key, 'v') {
            self.paste_from_clipboard();
            return;
        }

        // 7. Zoom dinámico de tipografía (Ctrl + +, Ctrl + -, Ctrl + 0)
        if ctrl && !shift {
            if key_is_char(&logical_key, physical_key, '+')
                || key_is_char(&logical_key, physical_key, '=')
            {
                self.adjust_font_size(1.0);
                return;
            }
            if key_is_char(&logical_key, physical_key, '-')
                || key_is_char(&logical_key, physical_key, '_')
            {
                self.adjust_font_size(-1.0);
                return;
            }
            if key_is_char(&logical_key, physical_key, '0') {
                self.reset_font_size();
                return;
            }
        } else if ctrl && shift {
            if key_is_char(&logical_key, physical_key, '+')
                || key_is_char(&logical_key, physical_key, '=')
            {
                self.adjust_font_size(1.0);
                return;
            }
            if key_is_char(&logical_key, physical_key, '_')
                || key_is_char(&logical_key, physical_key, '-')
            {
                self.adjust_font_size(-1.0);
                return;
            }
        }

        // 8. Recarga dinámica de configuración (Ctrl + Shift + ,)
        if ctrl && shift && key_is_char(&logical_key, physical_key, ',') {
            self.reload_config();
            return;
        }

        // Si estamos en la pestaña de Ajustes, absorber el resto de teclas
        if active_is_settings {
            return;
        }

        // Navegación con teclado en el explorador de archivos cuando tiene foco
        if self.file_explorer.visible && self.file_explorer.is_focused && !ctrl && !alt {
            let win_h = self
                .window
                .as_ref()
                .map(|w| w.inner_size().height as usize)
                .unwrap_or(600);
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let (_, _, _, panel_h) =
                self.file_explorer
                    .panel_geometry(800, win_h, top_offset, status_bar_h);
            let list_h = panel_h.saturating_sub(
                crate::explorer::FileExplorer::HEADER_HEIGHT
                    + crate::explorer::FileExplorer::SUBHEADER_HEIGHT,
            );
            let max_visible = (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);

            match logical_key {
                Key::Named(NamedKey::ArrowUp) => {
                    self.file_explorer.navigate_selection(-1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.file_explorer.navigate_selection(1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    self.file_explorer.handle_arrow_left(max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if let Some(path) = self.file_explorer.handle_arrow_right(max_visible) {
                        self.open_file_in_viewer(&path, true);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(path) = self.file_explorer.handle_enter(max_visible) {
                        self.open_file_in_viewer(&path, true);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Backspace) => {
                    self.file_explorer.navigate_up();
                    self.file_explorer.selected_index = if self.file_explorer.items.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    self.file_explorer.selected_path = self
                        .file_explorer
                        .selected_index
                        .map(|i| self.file_explorer.items[i].path.clone());
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    self.file_explorer.is_focused = false;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Si la pestaña activa es un visor de archivos, gestionar edición, navegación y atajos
        let is_viewer_active = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| t.file_viewer.is_some())
            .unwrap_or(false);
        if is_viewer_active {
            let win_h = self
                .window
                .as_ref()
                .map(|w| w.inner_size().height as usize)
                .unwrap_or(600);
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let visible_lines =
                ((win_h.saturating_sub(top_offset + status_bar_h + 38)) / row_h.max(16)).max(1);

            // Ajuste de línea global: Alt + Z
            if alt && !ctrl && !shift && key_is_char(&logical_key, physical_key, 'z') {
                self.toggle_word_wrap();
                return;
            }

            // 1. Guardar archivo: Ctrl + S
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 's') {
                self.save_active_viewer();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }

            // 2. Cerrar visor o cancelar selección: Escape o Ctrl + W
            if matches!(logical_key, Key::Named(NamedKey::Escape)) {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    if viewer.has_selection() {
                        viewer.clear_selection();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }
                self.close_tab(self.active_tab_index);
                return;
            }
            if ctrl && !shift && key_is_char(&logical_key, physical_key, 'w') {
                self.close_tab(self.active_tab_index);
                return;
            }

            // Seleccionar todo: Ctrl + A
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'a') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.select_all();
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return;
            }

            // Deshacer: Ctrl + Z
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'z') {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.undo();
                    if auto_save {
                        viewer.schedule_autosave();
                    }
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return;
            }

            // Rehacer: Ctrl + Y o Ctrl + Shift + Z
            if (ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'y'))
                || (ctrl && shift && !alt && key_is_char(&logical_key, physical_key, 'z'))
            {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.redo();
                    if auto_save {
                        viewer.schedule_autosave();
                    }
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return;
            }

            // Cortar selección: Ctrl + X
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'x') {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    if !viewer.is_editable() {
                        return;
                    }
                    if let Some(sel_text) = viewer.selected_text() {
                        super::clipboard::copy_text(&mut self.clipboard, sel_text);
                        viewer.delete_selection();
                        if auto_save {
                            viewer.schedule_autosave();
                        }
                        viewer.ensure_cursor_visible(visible_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
                return;
            }

            // Copiar selección o línea actual: Ctrl + C
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'c') {
                if let Some(ref viewer) = self.tabs[self.active_tab_index].file_viewer {
                    let text = if let Some(sel_text) = viewer.selected_text() {
                        Some(sel_text)
                    } else {
                        viewer
                            .lines
                            .get(viewer.cursor_line)
                            .map(|line| format!("{}\n", line))
                    };
                    if let Some(txt) = text {
                        super::clipboard::copy_text(&mut self.clipboard, txt);
                    }
                }
                return;
            }

            // Alternar modo solo lectura / edición: Ctrl + E
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'e') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.toggle_read_only();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
            }

            // Pegar texto del portapapeles: Ctrl + V (el modo lectura lo bloquea el propio visor)
            if ctrl && !shift && !alt && key_is_char(&logical_key, physical_key, 'v') {
                self.paste_from_clipboard();
                return;
            }

            // 3. Edición de texto y navegación de cursor
            let global_auto_save = self.config.terminal.auto_save;
            if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                if !viewer.is_binary {
                    let prev_pos = (viewer.cursor_line, viewer.cursor_col);
                    let is_shift_nav = shift && !alt;

                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            if ctrl {
                                // Ctrl + Arriba: al inicio del archivo
                                viewer.cursor_line = 0;
                                viewer.cursor_col = 0;
                            } else if viewer.cursor_line > 0 {
                                viewer.cursor_line -= 1;
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if viewer.cursor_col > line_len {
                                viewer.cursor_col = line_len;
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            if ctrl {
                                // Ctrl + Abajo: al final del archivo
                                viewer.cursor_line = viewer.lines.len().saturating_sub(1);
                                viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                            } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                viewer.cursor_line += 1;
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if viewer.cursor_col > line_len {
                                viewer.cursor_col = line_len;
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            if ctrl {
                                if viewer.cursor_col > 0 {
                                    viewer.cursor_col = viewer.find_word_boundary_left();
                                } else if viewer.cursor_line > 0 {
                                    viewer.cursor_line -= 1;
                                    viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                                }
                            } else if viewer.cursor_col > 0 {
                                viewer.cursor_col -= 1;
                            } else if viewer.cursor_line > 0 {
                                viewer.cursor_line -= 1;
                                viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if ctrl {
                                if viewer.cursor_col < line_len {
                                    viewer.cursor_col = viewer.find_word_boundary_right();
                                } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                    viewer.cursor_line += 1;
                                    viewer.cursor_col = 0;
                                }
                            } else {
                                if viewer.cursor_col < line_len {
                                    viewer.cursor_col += 1;
                                } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                    viewer.cursor_line += 1;
                                    viewer.cursor_col = 0;
                                }
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            if ctrl {
                                viewer.cursor_line = 0;
                            }
                            viewer.cursor_col = 0;
                            viewer.ensure_cursor_visible(visible_lines);
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            if ctrl {
                                viewer.cursor_line = viewer.lines.len().saturating_sub(1);
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            viewer.cursor_col = line_len;
                            viewer.ensure_cursor_visible(visible_lines);
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::PageUp) => {
                            let page = visible_lines.saturating_sub(2).max(1);
                            viewer.cursor_line = viewer.cursor_line.saturating_sub(page);
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let page = visible_lines.saturating_sub(2).max(1);
                            viewer.cursor_line = (viewer.cursor_line + page)
                                .min(viewer.lines.len().saturating_sub(1));
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Enter) => {
                            if viewer.is_read_only {
                                return;
                            }
                            viewer.insert_newline();
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            if viewer.is_read_only {
                                return;
                            }
                            if ctrl {
                                viewer.delete_word_left();
                            } else {
                                viewer.backspace();
                            }
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Delete) => {
                            if viewer.is_read_only {
                                return;
                            }
                            if ctrl {
                                viewer.delete_word_right();
                            } else {
                                viewer.delete();
                            }
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Tab) => {
                            if viewer.is_read_only {
                                return;
                            }
                            viewer.insert_tab();
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        _ => {
                            // Insertar caracteres de texto imprimible
                            if !ctrl && !alt {
                                if viewer.is_read_only {
                                    return;
                                }
                                if let Some(ref t) = text {
                                    for ch in t.chars() {
                                        if !ch.is_control() {
                                            viewer.insert_char(ch);
                                        }
                                    }
                                    if global_auto_save {
                                        viewer.schedule_autosave();
                                    }
                                    viewer.ensure_cursor_visible(visible_lines);
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }
                    }
                } else {
                    // Modo de solo lectura binario
                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            viewer.scroll_offset = viewer.scroll_offset.saturating_sub(1);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            viewer.scroll_offset = (viewer.scroll_offset + 1)
                                .min(viewer.rows().len().saturating_sub(1));
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        }
                        _ => {}
                    }
                }
            }
            return;
        }

        // Obtener panel activo para entrada de teclado
        let active = match self.tabs[self.active_tab_index].active_pane_mut() {
            Some(p) => p,
            None => return,
        };

        // Detección robusta de tecla Backspace (por physical_key independiente de layout/Alt y por logical_key)
        let is_backspace = matches!(physical_key, PhysicalKey::Code(KeyCode::Backspace))
            || matches!(&logical_key, Key::Named(NamedKey::Backspace));

        // 1. Alt + Backspace: Secuencia estándar VT (ESC + DEL: \x1b\x7f).
        if alt && is_backspace {
            crate::logger::log("-> Enviando \\x1b\\x7f (Alt+Backspace)");
            active.pty.write_input(b"\x1b\x7f");
            return;
        }

        // 2. Ctrl + Backspace: Byte estándar werase (Ctrl+W: 0x17) utilizado por Windows Terminal.
        if ctrl && is_backspace {
            crate::logger::log("-> Enviando 0x17 (Ctrl+Backspace / Ctrl+W)");
            active.pty.write_input(&[0x17]);
            return;
        }

        // 3. Backspace normal (o Shift+Backspace):
        if is_backspace {
            crate::logger::log("-> Enviando 0x7f (borrado de caracter)");
            active.pty.write_input(&[0x7f]);
            return;
        }

        // Siempre devolver scroll al prompt activo al pulsar teclas normales (excepto PageUp/Down y Shift+Home/End)
        if logical_key != Key::Named(NamedKey::PageUp)
            && logical_key != Key::Named(NamedKey::PageDown)
            && !(shift
                && (logical_key == Key::Named(NamedKey::Home)
                    || logical_key == Key::Named(NamedKey::End)))
        {
            active.terminal.reset_scroll();
        }

        // Copiar selección (Ctrl + Shift + C o Ctrl + C con selección activa)
        if (ctrl && shift && key_is_char(&logical_key, physical_key, 'c'))
            || (ctrl
                && !shift
                && key_is_char(&logical_key, physical_key, 'c')
                && active.terminal.has_selection())
        {
            if let Some(sel_text) = active.terminal.copy_selection_text() {
                if let Some(cb) = &mut self.clipboard {
                    cb.set_text(sel_text).ok();
                }
                active.terminal.selection = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            return;
        }

        // Soporte de Kitty Keyboard Protocol (CSI u) cuando el cliente lo ha activado
        let kitty_flags = active.terminal.kitty_keyboard_flags;
        if kitty_flags != 0 {
            if let Some(seq) = encode_kitty_key(
                &logical_key,
                physical_key,
                ctrl,
                shift,
                alt,
                win,
                event_type,
                kitty_flags,
            ) {
                let is_enter = matches!(
                    physical_key,
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
                ) || matches!(&logical_key, Key::Named(NamedKey::Enter));
                if is_enter {
                    active.terminal.note_enter_sent();
                }
                active.pty.write_input(&seq);
                return;
            }
        }

        // Combinaciones con Alt (Alt + V para imágenes en Claude Code, Alt + Flechas)
        if alt && !ctrl {
            if key_is_char(&logical_key, physical_key, 'v') {
                active.pty.write_input(b"\x1bv");
                return;
            }

            let is_alt_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                || logical_key == Key::Named(NamedKey::ArrowLeft);
            let is_alt_right = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                || logical_key == Key::Named(NamedKey::ArrowRight);
            let is_alt_delete = matches!(physical_key, PhysicalKey::Code(KeyCode::Delete))
                || logical_key == Key::Named(NamedKey::Delete);

            if is_alt_left {
                active.pty.write_input(b"\x1bb"); // Salto palabra atrás
                return;
            }
            if is_alt_right {
                active.pty.write_input(b"\x1bf"); // Salto palabra adelante
                return;
            }
            if is_alt_delete {
                active.pty.write_input(b"\x1bd"); // Borrado de palabra adelante
                return;
            }

            match logical_key {
                Key::Named(NamedKey::Tab) => {
                    return; // Prevenir cualquier propagación de Alt+Tab
                }
                Key::Named(NamedKey::F4) => {
                    return; // Alt+F4 manejado por el sistema
                }
                Key::Character(ref s) => {
                    if let Some(c) = s.chars().next() {
                        if c.is_ascii_alphabetic() {
                            let mut buf = [0u8; 5];
                            buf[0] = 0x1b;
                            let len = c.encode_utf8(&mut buf[1..]).len();
                            active.pty.write_input(&buf[..1 + len]);
                            return;
                        }
                    }
                }
                _ => {
                    if let PhysicalKey::Code(code) = physical_key {
                        if let Some(c) = key_code_to_char(code) {
                            if c.is_ascii_alphabetic() {
                                let mut buf = [0u8; 5];
                                buf[0] = 0x1b;
                                let len = c.encode_utf8(&mut buf[1..]).len();
                                active.pty.write_input(&buf[..1 + len]);
                                return;
                            }
                        }
                    }
                }
            }
        }

        // Copiar salida del último comando del agente (Ctrl + Shift + O)
        if ctrl && shift && key_is_char(&logical_key, physical_key, 'o') {
            if let Some(output) = active.terminal.get_last_output_text() {
                if let Some(cb) = &mut self.clipboard {
                    cb.set_text(output).ok();
                }
            }
            return;
        }

        // Limpiar pantalla y scrollback buffer (Ctrl + Shift + K)
        if ctrl && shift && key_is_char(&logical_key, physical_key, 'k') {
            active.terminal.clear_scrollback();
            active.pty.write_input(b"\x0c");
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }

        // Navegación semántica entre comandos de agentes OSC 133 (Ctrl + Shift + Arriba/Abajo)
        if ctrl && shift {
            if let Key::Named(NamedKey::ArrowUp) = logical_key {
                if let Some(block) = active.terminal.command_blocks.last() {
                    active.terminal.scroll_offset = active
                        .terminal
                        .scrollback
                        .len()
                        .saturating_sub(block.prompt_line);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return;
            }
            if let Key::Named(NamedKey::ArrowDown) = logical_key {
                active.terminal.reset_scroll();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        }

        // Navegación de historial y scrollback (Shift + PageUp / PageDown / Home / End)
        if shift && !ctrl {
            match logical_key {
                Key::Named(NamedKey::PageUp) => {
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_up(half);
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::PageDown) => {
                    let half = (active.terminal.rows / 2).max(1);
                    active.terminal.scroll_down(half);
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Home) => {
                    active.terminal.scroll_offset = active.terminal.scrollback.len();
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::End) => {
                    active.terminal.reset_scroll();
                    self.last_scroll_time = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Salto de palabras y edición en línea de comandos (Ctrl + Flechas, Ctrl + Delete)
        if ctrl && !shift {
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

        // Combinaciones de control clásicas de terminal (Ctrl+C, Ctrl+D, Ctrl+L, Ctrl+Z, Ctrl+R, etc.)
        if ctrl && !shift {
            if let Key::Character(ref s) = logical_key {
                let c = s.chars().next().unwrap_or(' ').to_ascii_lowercase();
                if c.is_ascii_lowercase() {
                    let code = (c as u8) - b'a' + 1;
                    active.pty.write_input(&[code]);
                    return;
                }
            }
        }

        // Scroll por página directo sin Shift
        if logical_key == Key::Named(NamedKey::PageUp) {
            let half = (active.terminal.rows / 2).max(1);
            active.terminal.scroll_up(half);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }
        if logical_key == Key::Named(NamedKey::PageDown) {
            let half = (active.terminal.rows / 2).max(1);
            active.terminal.scroll_down(half);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }

        // Enter con Shift o Alt (incluso bajo WM_SYSKEYDOWN donde logical_key es Unidentified):
        // envía ESC + CR (\x1b\r), secuencia que Claude Code, Gemini CLI, agy, OpenCode, Aider y PSReadLine
        // interpretan como salto de línea sin enviar el mensaje.
        let is_enter = matches!(
            physical_key,
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
        ) || matches!(&logical_key, Key::Named(NamedKey::Enter));
        if is_enter && !ctrl {
            active.terminal.note_enter_sent();
            if shift || alt {
                active.pty.write_input(b"\x1b\r");
            } else {
                active.pty.write_input(b"\r");
            }
            return;
        }

        // Teclas especiales y flechas VT estándar
        match logical_key {
            Key::Named(NamedKey::Enter) => {
                active.terminal.note_enter_sent();
                active.pty.write_input(b"\r")
            }
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
}
