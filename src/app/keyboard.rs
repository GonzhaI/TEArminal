//! Entrada de teclado.

use super::*;

impl TearminalApp {
    /// Teclado: atajos de la aplicación y traducción de teclas a bytes para el PTY con soporte de eventos Kitty.
    pub(super) fn on_key_event(
        &mut self,
        logical_key: Key,
        text: Option<SmolStr>,
        physical_key: PhysicalKey,
        event_type: KeyEventType,
    ) {
        if !self.is_focused {
            return;
        }

        if self.handle_key_early_release(&logical_key, physical_key, event_type) {
            return;
        }

        if self.handle_key_overlays(&logical_key, physical_key, &text) {
            return;
        }

        let is_modifier = matches!(
            logical_key,
            Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
        );

        let (ctrl, shift, alt, altgr, win) = self.physical_modifiers();

        // Debounce de foco: suprimir solo Tab residual si Alt sigue presionado al alternar ventanas con Alt+Tab
        if self.last_focus_gained.elapsed().as_millis() < 30
            && logical_key == Key::Named(NamedKey::Tab)
            && (alt || self.modifiers.alt_key())
        {
            crate::logger::log("-> Ignorado Tab residual con Alt de foco");
            return;
        }

        // Si la tecla Windows/Super está presionada (o atajos de Windows como Win+Shift+Flechas):
        if win || self.modifiers.super_key() {
            crate::logger::log("-> Ignorada tecla con Windows/Super activo");
            return;
        }

        // Teclas modificadoras por sí solas (Shift, Ctrl, Alt, Super) nunca deben desplazar el scroll ni escribir
        if is_modifier {
            if let Some(active) = self.tabs[self.active_tab_index].active_pane() {
                let flags = active.terminal.kitty_keyboard_flags;
                if (flags & 1) != 0 || (flags & 2) != 0 {
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

        // Solo con TEARMINAL_LOG_KEYS=1; nunca se registra el texto producido, solo su longitud
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

        if self.handle_key_space_edit(&logical_key, physical_key, &text) {
            return;
        }

        if self.handle_key_explorer_inputs(&logical_key, physical_key, &text) {
            return;
        }

        if self.handle_key_settings(&logical_key, physical_key, &text, ctrl, shift) {
            return;
        }

        if self.handle_key_app_shortcuts(&logical_key, physical_key, &text, ctrl, shift, alt) {
            return;
        }

        if self.handle_key_file_viewer(&logical_key, physical_key, &text, ctrl, shift, alt) {
            return;
        }

        self.handle_key_terminal(
            &logical_key,
            physical_key,
            text,
            ctrl,
            shift,
            alt,
            altgr,
            win,
            event_type,
        );
    }

    fn handle_key_early_release(
        &self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        event_type: KeyEventType,
    ) -> bool {
        if event_type == KeyEventType::Release {
            if let Some(active) = self.tabs[self.active_tab_index].active_pane() {
                let flags = active.terminal.kitty_keyboard_flags;
                if (flags & 2) != 0 {
                    let (ctrl, shift, alt, _altgr, win) = self.physical_modifiers();
                    if let Some(seq) = encode_kitty_key(
                        logical_key,
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
            return true;
        }
        false
    }

    fn handle_key_overlays(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
    ) -> bool {
        // Con el menú contextual abierto, Escape solo lo cierra; cualquier otra tecla lo cierra y sigue su curso
        let is_modifier = matches!(
            logical_key,
            Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
        );
        if !is_modifier && self.context_menu.take().is_some() {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            if *logical_key == Key::Named(NamedKey::Escape) {
                return true;
            }
        }

        // Selector de pantalla encendida: Escape lo cierra
        if self.keep_awake_popup_open && *logical_key == Key::Named(NamedKey::Escape) {
            self.keep_awake_popup_open = false;
            self.hovered_keep_awake_option = None;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // Con el modal «Añadir máquina» abierto, interceptar teclado por completo
        if self.machine_modal.is_some() {
            self.handle_machine_key(logical_key, physical_key, text);
            return true;
        }

        // Con la paleta de comandos abierta, interceptar teclado por completo
        if self.palette.is_some() && self.handle_palette_key(logical_key, physical_key, text) {
            return true;
        }

        false
    }

    pub(crate) fn handle_key_space_edit(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
    ) -> bool {
        if self.space_edit.is_none() {
            return false;
        }
        match &logical_key {
            Key::Named(NamedKey::Escape) => self.cancel_space_rename(),
            Key::Named(NamedKey::Enter) => self.commit_space_rename(),
            _ => {
                if let Some(mut edit) = self.space_edit.take() {
                    self.handle_text_input_key(
                        &mut edit.input,
                        logical_key,
                        physical_key,
                        text.as_deref(),
                    );
                    self.space_edit = Some(edit);
                }
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        true
    }

    fn handle_key_explorer_inputs(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
    ) -> bool {
        // Nombre en edición en el explorador (nuevo archivo/carpeta o renombrar): la barra absorbe todo el teclado
        if self.file_explorer.visible && self.file_explorer.edit.is_some() {
            match &logical_key {
                Key::Named(NamedKey::Escape) => self.file_explorer.cancel_edit(),
                Key::Named(NamedKey::Enter) => {
                    let max_visible = self.explorer_max_visible();
                    if self.file_explorer.commit_edit(max_visible).is_some() {
                        self.sync_fs_watcher();
                    }
                }
                _ => {
                    if let Some(mut edit) = self.file_explorer.edit.take() {
                        let (_, changed) = self.handle_text_input_key(
                            &mut edit.input,
                            logical_key,
                            physical_key,
                            text.as_deref(),
                        );
                        if changed {
                            edit.error = None;
                        }
                        self.file_explorer.edit = Some(edit);
                    }
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // Filtro rápido de archivos en el explorador: la barra absorbe el teclado mientras esté abierta
        if self.file_explorer.visible
            && self.file_explorer.filter.is_some()
            && self.file_explorer.is_focused
        {
            let max_visible = self.explorer_max_visible();

            match &logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.file_explorer.close_filter();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.file_explorer.navigate_selection(-1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.file_explorer.navigate_selection(1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(path) = self.file_explorer.handle_enter(max_visible) {
                        self.file_explorer.close_filter();
                        self.open_file_in_viewer(&path, true);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                _ => {
                    if let Some(mut filter_input) = self.file_explorer.filter.take() {
                        let (_, changed) = self.handle_text_input_key(
                            &mut filter_input,
                            logical_key,
                            physical_key,
                            text.as_deref(),
                        );
                        self.file_explorer.filter = Some(filter_input);
                        if changed {
                            self.file_explorer.update_filter();
                        }
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
            }
        }

        false
    }

    fn handle_key_settings(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
        ctrl: bool,
        shift: bool,
    ) -> bool {
        // Si estamos en la pestaña de Ajustes, capturar Escape, Ctrl+, o Ctrl+W para cerrarla/alternarla
        let active_is_settings = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| t.is_settings)
            .unwrap_or(false);
        if active_is_settings {
            if self.settings_page_state.editing_setting == Some(EditingSettingField::ThemeSearch) {
                self.edit_theme_search(logical_key, physical_key, text.as_deref());
                return true;
            }
            if self.settings_page_state.editing_setting == Some(EditingSettingField::GeneralSearch)
            {
                self.edit_general_settings_search(logical_key, physical_key, text.as_deref());
                return true;
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
                            EditingSettingField::ScrollbackLines => {
                                if let Ok(val) = input.parse::<usize>() {
                                    self.set_scrollback_lines(val);
                                }
                            }
                            EditingSettingField::ThemeSearch
                            | EditingSettingField::GeneralSearch => {}
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
                        return true;
                    }
                    Key::Named(NamedKey::Escape) => {
                        self.settings_page_state.editing_setting = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.settings_page_state.setting_input_buffer.pop();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
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
                            return true;
                        }
                    }
                }
            }

            // Arriba / abajo: sección anterior / siguiente de la navegación lateral
            if let Key::Named(arrow @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) = &logical_key {
                use crate::renderer::SettingsSection::{About, Appearance, Shortcuts, Terminal};
                let order = [Appearance, Terminal, Shortcuts, About];
                let pos = order
                    .iter()
                    .position(|s| *s == self.settings_page_state.section)
                    .unwrap_or(0);
                let next = if matches!(arrow, NamedKey::ArrowDown) {
                    (pos + 1) % order.len()
                } else {
                    (pos + order.len() - 1) % order.len()
                };
                self.settings_page_state.section = order[next];
                self.settings_page_state.scroll_y = 0;
                self.request_redraw();
                return true;
            }

            if matches!(&logical_key, Key::Named(NamedKey::Escape))
                || (ctrl && !shift && key_is_char(logical_key, physical_key, ','))
            {
                crate::logger::log("-> Cerrando Ajustes");
                self.close_settings();
                return true;
            }
            if ctrl && !shift && key_is_char(logical_key, physical_key, 'f') {
                self.settings_page_state.editing_setting = Some(EditingSettingField::GeneralSearch);
                self.cursor_on = true;
                self.cursor_timer = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return true;
            }
            if ctrl && !shift && key_is_char(logical_key, physical_key, 'w') {
                crate::logger::log("-> Cerrando pestaña de Ajustes con Ctrl+W");
                self.close_tab(self.active_tab_index);
                return true;
            }
        }

        false
    }

    pub(crate) fn handle_key_app_shortcuts(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        let active_is_settings = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| t.is_settings)
            .unwrap_or(false);

        // Pantalla completa (F11 o Alt + Enter, estándar Windows)
        let is_enter = matches!(
            physical_key,
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
        ) || matches!(&logical_key, Key::Named(NamedKey::Enter));
        if matches!(&logical_key, Key::Named(NamedKey::F11)) || (alt && !ctrl && !shift && is_enter)
        {
            crate::logger::log("-> Hotkey: Alternar pantalla completa (F11 / Alt+Enter)");
            self.toggle_fullscreen();
            return true;
        }

        // Refrescar explorador y estado git (F5, estándar Windows)
        if !ctrl && !alt && !shift && matches!(&logical_key, Key::Named(NamedKey::F5)) {
            crate::logger::log("-> Hotkey: Refrescar (F5)");
            self.file_explorer.refresh();
            self.request_redraw();
            return true;
        }

        // Abrir / alternar pestaña de Ajustes (Ctrl + ,)
        if ctrl && !shift && key_is_char(logical_key, physical_key, ',') {
            crate::logger::log("-> Hotkey: Abrir / alternar Ajustes (Ctrl+,)");
            self.open_or_switch_settings_tab();
            return true;
        }

        // Paleta de comandos (Ctrl + Shift + P, F1, Ctrl + K)
        let is_k = key_is_char(logical_key, physical_key, 'k');
        let is_p = key_is_char(logical_key, physical_key, 'p');
        let is_f1 = matches!(&logical_key, Key::Named(NamedKey::F1));
        if (ctrl && shift && !alt && is_p)
            || (ctrl && !shift && !alt && is_k)
            || (!ctrl && !shift && !alt && is_f1)
        {
            crate::logger::log("-> Hotkey: Paleta de comandos (Ctrl+Shift+P / F1 / Ctrl+K)");
            self.toggle_palette();
            return true;
        }

        // Filtro rápido de archivos en el árbol del explorador (Ctrl + P)
        if ctrl && !shift && !alt && is_p {
            crate::logger::log("-> Hotkey: Filtro rápido de archivos (Ctrl+P)");
            self.open_explorer_filter();
            return true;
        }

        // Búsqueda / filtro en el explorador con Ctrl+F si tiene el foco
        if ctrl
            && !shift
            && !alt
            && key_is_char(logical_key, physical_key, 'f')
            && self.file_explorer.visible
            && self.file_explorer.is_focused
        {
            crate::logger::log("-> Hotkey: Filtro en explorador (Ctrl+F)");
            self.open_explorer_filter();
            return true;
        }

        // Búsqueda en el historial de la terminal activa (Ctrl + F o Ctrl + Shift + F) y su barra abierta
        let term_tab = self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(|t| t.file_viewer.is_none() && !t.is_settings);
        if term_tab && ctrl && !alt && key_is_char(logical_key, physical_key, 'f') {
            if let Some(pane) = self.tabs[self.active_tab_index].active_pane_mut() {
                pane.terminal.open_search();
            }
            self.cursor_on = true;
            self.cursor_timer = Instant::now();
            self.request_redraw();
            return true;
        }
        if term_tab
            && self.tabs[self.active_tab_index]
                .active_pane()
                .is_some_and(|p| p.terminal.search.is_some())
        {
            let step = match &logical_key {
                Key::Named(NamedKey::Enter | NamedKey::F3) => Some(!shift),
                Key::Named(NamedKey::ArrowDown) => Some(true),
                Key::Named(NamedKey::ArrowUp) => Some(false),
                _ => None,
            };
            if matches!(&logical_key, Key::Named(NamedKey::Escape)) {
                if let Some(pane) = self.tabs[self.active_tab_index].active_pane_mut() {
                    pane.terminal.close_search();
                }
                self.request_redraw();
                return true;
            }
            if let Some(forward) = step {
                if let Some(pane) = self.tabs[self.active_tab_index].active_pane_mut() {
                    pane.terminal.search_step(forward);
                }
                self.request_redraw();
                return true;
            }
            let mut input = self.tabs[self.active_tab_index]
                .active_pane_mut()
                .and_then(|p| p.terminal.search.as_mut())
                .map(|s| std::mem::take(&mut s.input))
                .unwrap_or_default();
            let (consumed, changed) =
                self.handle_text_input_key(&mut input, logical_key, physical_key, text.as_deref());
            if let Some(pane) = self.tabs[self.active_tab_index].active_pane_mut() {
                if let Some(s) = pane.terminal.search.as_mut() {
                    s.input = input;
                }
                if changed {
                    pane.terminal.update_search_matches();
                }
            }
            if consumed {
                self.request_redraw();
                return true;
            }
        }

        // --- ATAJOS DE PANELES DIVIDIDOS (SPLIT PANES) CANÓNICOS (Alt+Shift, Alt o Ctrl+Shift) ---
        let is_d = key_is_char(logical_key, physical_key, 'd');
        let is_v = key_is_char(logical_key, physical_key, 'v');
        let is_w = key_is_char(logical_key, physical_key, 'w');
        let is_plus = key_is_char(logical_key, physical_key, '+')
            || key_is_char(logical_key, physical_key, '=')
            || matches!(physical_key, PhysicalKey::Code(KeyCode::NumpadAdd));
        let is_minus = key_is_char(logical_key, physical_key, '-')
            || key_is_char(logical_key, physical_key, '_')
            || matches!(physical_key, PhysicalKey::Code(KeyCode::NumpadSubtract));
        let is_split_mod = (alt && shift) || (ctrl && alt);

        // 1. División Horizontal: Alt + Shift + D o Alt + Shift + - (estándar Windows Terminal)
        if (is_split_mod && is_d) || (alt && shift && is_minus) {
            crate::logger::log("-> Hotkey: Split Horizontal (Alt+Shift+D / Alt+Shift+-)");
            self.split_active_pane(SplitDirection::Horizontal);
            return true;
        }

        // 2. División Vertical: Alt + Shift + V o Alt + Shift + + (estándar Windows Terminal)
        if (is_split_mod && is_v) || (alt && shift && is_plus) {
            crate::logger::log("-> Hotkey: Split Vertical (Alt+Shift+V / Alt+Shift++)");
            self.split_active_pane(SplitDirection::Vertical);
            return true;
        }

        // 3. Cerrar panel activo en split: Ctrl + Shift + W o Alt + Shift + W
        if (ctrl && shift && !alt && is_w) || (is_split_mod && is_w) {
            let has_splits = self
                .tabs
                .get(self.active_tab_index)
                .is_some_and(|t| t.panes.len() > 1);
            if has_splits {
                crate::logger::log("-> Hotkey: Cerrar panel activo (Ctrl+Shift+W / Alt+Shift+W)");
                self.close_active_pane();
                return true;
            } else if ctrl && shift && is_w {
                crate::logger::log("-> Hotkey: Cerrar pestaña activa (Ctrl+Shift+W)");
                self.close_tab(self.active_tab_index);
                return true;
            }
        }

        // 4. Navegación direccional entre paneles: Alt + Flechas o Alt + Shift + Flechas (Windows Terminal)
        let has_splits = self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(|t| t.panes.len() > 1);
        if is_split_mod || (alt && !ctrl && !shift && has_splits) {
            let is_up = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowUp))
                || *logical_key == Key::Named(NamedKey::ArrowUp);
            let is_down = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowDown))
                || *logical_key == Key::Named(NamedKey::ArrowDown);
            let is_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                || *logical_key == Key::Named(NamedKey::ArrowLeft);
            let is_right = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                || *logical_key == Key::Named(NamedKey::ArrowRight);

            if is_up {
                self.navigate_pane(PaneDirection::Up);
                return true;
            }
            if is_down {
                self.navigate_pane(PaneDirection::Down);
                return true;
            }
            if is_left {
                self.navigate_pane(PaneDirection::Left);
                return true;
            }
            if is_right {
                self.navigate_pane(PaneDirection::Right);
                return true;
            }
        }

        // --- ATAJOS DE GESTIÓN DE PESTAÑAS (TABS & SIDEBAR) CANÓNICOS DE WINDOWS ---
        // 1. Nueva pestaña / terminal: Ctrl + T
        if ctrl && !alt && !shift && key_is_char(logical_key, physical_key, 't') {
            crate::logger::log("-> Hotkey: Nueva terminal (Ctrl+T)");
            self.create_new_tab();
            return true;
        }

        // Nueva ventana: Ctrl + Shift + N o Ctrl + N (si la pestaña activa no es terminal o no escribe)
        if ctrl && shift && !alt && key_is_char(logical_key, physical_key, 'n') {
            crate::logger::log("-> Hotkey: Nueva ventana (Ctrl+Shift+N)");
            self.new_window_request = true;
            return true;
        }

        // 2. Cerrar pestaña activa: Ctrl + W
        if ctrl && !alt && !shift && key_is_char(logical_key, physical_key, 'w') {
            crate::logger::log("-> Hotkey: Cerrar pestaña activa (Ctrl+W)");
            self.close_tab(self.active_tab_index);
            return true;
        }

        // 3. Conmutar pestaña siguiente / anterior: Ctrl + Tab / Ctrl + Shift + Tab / Ctrl + PageDown / Ctrl + PageUp
        if ctrl && !alt {
            let is_next = (*logical_key == Key::Named(NamedKey::Tab) && !shift)
                || (!shift && matches!(&logical_key, Key::Named(NamedKey::PageDown)));
            let is_prev = (*logical_key == Key::Named(NamedKey::Tab) && shift)
                || (!shift && matches!(&logical_key, Key::Named(NamedKey::PageUp)));
            if is_next {
                crate::logger::log("-> Hotkey: Pestaña siguiente (Ctrl+Tab / Ctrl+PageDown)");
                self.next_tab();
                return true;
            }
            if is_prev {
                crate::logger::log("-> Hotkey: Pestaña anterior (Ctrl+Shift+Tab / Ctrl+PageUp)");
                self.prev_tab();
                return true;
            }
        }

        // 4. Cambiar directamente a pestaña 1-8 o última con Ctrl + 9 (estándar Windows)
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
                let visible = self.visible_tab_indices();
                if idx == 8 {
                    // Ctrl+9 en Windows salta a la última pestaña visible
                    if let Some(&last_real) = visible.last() {
                        crate::logger::log("-> Hotkey: Ir a última pestaña (Ctrl+9)");
                        self.switch_to_tab(last_real);
                        return true;
                    }
                } else if let Some(&real) = visible.get(idx) {
                    crate::logger::log(&format!("-> Hotkey: Ir a pestaña {}", idx + 1));
                    self.switch_to_tab(real);
                    return true;
                }
            }
        }

        // Spaces: Ctrl+Shift+T nuevo, Ctrl+Alt+↑/↓ anterior / siguiente, Ctrl+Shift+B colapsa el panel
        if ctrl && shift && !alt && key_is_char(logical_key, physical_key, 't') {
            self.create_space(None);
            return true;
        }
        if ctrl && shift && !alt && key_is_char(logical_key, physical_key, 'b') {
            self.toggle_spaces_panel();
            return true;
        }
        if ctrl && alt && !shift {
            match &logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.step_space(true);
                    return true;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.step_space(false);
                    return true;
                }
                _ => {}
            }
        }

        // 5. Alternar barra lateral (Sidebar toggle): Ctrl + B
        if ctrl && !alt && !shift && key_is_char(logical_key, physical_key, 'b') {
            crate::logger::log("-> Hotkey: Alternar barra lateral (Ctrl+B)");
            self.toggle_sidebar();
            return true;
        }

        // Alternar / enfocar explorador lateral de archivos: Ctrl + Shift + E o Alt + E
        let is_e = key_is_char(logical_key, physical_key, 'e');
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
            return true;
        }

        // 6. Pegar desde el portapapeles: Ctrl + V, Ctrl + Shift + V o Shift + Insert (estándar Windows)
        let is_v = key_is_char(logical_key, physical_key, 'v');
        let is_insert = matches!(physical_key, PhysicalKey::Code(KeyCode::Insert))
            || matches!(&logical_key, Key::Named(NamedKey::Insert));
        if (ctrl && !alt && is_v) || (shift && !ctrl && !alt && is_insert) {
            crate::logger::log("-> Hotkey: Pegar desde portapapeles (Ctrl+V / Shift+Insert)");
            self.paste_from_clipboard();
            return true;
        }

        // 7. Zoom dinámico de tipografía (Ctrl + +, Ctrl + -, Ctrl + 0 y Teclado Numérico)
        let is_numpad_plus = matches!(physical_key, PhysicalKey::Code(KeyCode::NumpadAdd));
        let is_numpad_minus = matches!(physical_key, PhysicalKey::Code(KeyCode::NumpadSubtract));
        let is_numpad_zero = matches!(physical_key, PhysicalKey::Code(KeyCode::Numpad0));

        if ctrl && !shift {
            if key_is_char(logical_key, physical_key, '+')
                || key_is_char(logical_key, physical_key, '=')
                || is_numpad_plus
            {
                self.adjust_font_size(1.0);
                return true;
            }
            if key_is_char(logical_key, physical_key, '-')
                || key_is_char(logical_key, physical_key, '_')
                || is_numpad_minus
            {
                self.adjust_font_size(-1.0);
                return true;
            }
            if key_is_char(logical_key, physical_key, '0') || is_numpad_zero {
                self.reset_font_size();
                return true;
            }
        } else if ctrl && shift {
            if key_is_char(logical_key, physical_key, '+')
                || key_is_char(logical_key, physical_key, '=')
                || is_numpad_plus
            {
                self.adjust_font_size(1.0);
                return true;
            }
            if key_is_char(logical_key, physical_key, '_')
                || key_is_char(logical_key, physical_key, '-')
                || is_numpad_minus
            {
                self.adjust_font_size(-1.0);
                return true;
            }
        }

        // 8. Recarga dinámica de configuración (Ctrl + Shift + ,)
        if ctrl && shift && key_is_char(logical_key, physical_key, ',') {
            self.reload_config();
            return true;
        }

        // Si estamos en la pestaña de Ajustes, absorber el resto de teclas
        if active_is_settings {
            return true;
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
                    + self.file_explorer.subheader_height(),
            );
            let max_visible = (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1);

            match logical_key {
                Key::Named(NamedKey::ArrowUp) => {
                    self.file_explorer.navigate_selection(-1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.file_explorer.navigate_selection(1, max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    self.file_explorer.handle_arrow_left(max_visible);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if let Some(path) = self.file_explorer.handle_arrow_right(max_visible) {
                        self.open_file_in_viewer(&path, true);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(path) = self.file_explorer.handle_enter(max_visible) {
                        self.open_file_in_viewer(&path, true);
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    let remote_parent = if self.file_explorer.remote_session.is_some() {
                        self.file_explorer.remote_parent()
                    } else {
                        None
                    };
                    match remote_parent {
                        Some(parent) => {
                            self.remote_cd(&parent);
                        }
                        None => self.file_explorer.navigate_up(),
                    }
                    self.file_explorer.selected_index = if self.file_explorer.items.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    self.file_explorer.selected_path =
                        self.file_explorer.items.first().map(|i| i.path.clone());
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::F2) => {
                    self.file_explorer
                        .begin_edit(crate::explorer::EditKind::Rename);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::Delete) => {
                    self.delete_explorer_selection();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                Key::Named(NamedKey::Escape) => {
                    self.file_explorer.is_focused = false;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
                _ => {}
            }
        }

        // F2 sobre un espacio del panel lateral: renombrar el espacio
        if !ctrl && !alt && !shift && matches!(&logical_key, Key::Named(NamedKey::F2)) {
            if let crate::renderer::SpacesHover::Row(idx) = self.spaces_hover {
                if let Some(id) = self.spaces.get(idx).map(|s| s.id) {
                    self.start_renaming_space(id);
                    return true;
                }
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_key_terminal(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: Option<SmolStr>,
        ctrl: bool,
        shift: bool,
        alt: bool,
        _altgr: bool,
        win: bool,
        event_type: KeyEventType,
    ) {
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
            active.terminal.reset_scroll();
            active.pty.write_input(b"\x1b\x7f");
            return;
        }

        // 2. Ctrl + Backspace: Byte estándar werase (Ctrl+W: 0x17) utilizado por Windows Terminal.
        if ctrl && is_backspace {
            crate::logger::log("-> Enviando 0x17 (Ctrl+Backspace / Ctrl+W)");
            active.terminal.reset_scroll();
            active.pty.write_input(&[0x17]);
            return;
        }

        // 3. Backspace normal (o Shift+Backspace):
        if is_backspace {
            crate::logger::log("-> Enviando 0x7f (borrado de caracter)");
            active.terminal.reset_scroll();
            active.pty.write_input(&[0x7f]);
            return;
        }

        // Copiar selección (Ctrl + Shift + C, Ctrl + C con selección activa, o Ctrl + Insert estándar Windows)
        let is_insert = matches!(physical_key, PhysicalKey::Code(KeyCode::Insert))
            || matches!(&logical_key, Key::Named(NamedKey::Insert));
        if (ctrl && shift && key_is_char(logical_key, physical_key, 'c'))
            || (ctrl && is_insert && active.terminal.has_selection())
            || (ctrl
                && !shift
                && key_is_char(logical_key, physical_key, 'c')
                && active.terminal.has_selection())
        {
            if let Some(sel_text) = active.terminal.copy_selection_text() {
                clipboard::copy_text(&mut self.clipboard, &sel_text);
                active.terminal.selection = None;
                self.request_redraw();
            }
            return;
        }

        // Soporte de Kitty Keyboard Protocol (CSI u) cuando el cliente lo ha activado
        let kitty_flags = active.terminal.kitty_keyboard_flags;
        if kitty_flags != 0 {
            if let Some(seq) = encode_kitty_key(
                logical_key,
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
            if key_is_char(logical_key, physical_key, 'v') {
                active.pty.write_input(b"\x1bv");
                return;
            }

            let is_alt_left = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowLeft))
                || *logical_key == Key::Named(NamedKey::ArrowLeft);
            let is_alt_right = matches!(physical_key, PhysicalKey::Code(KeyCode::ArrowRight))
                || *logical_key == Key::Named(NamedKey::ArrowRight);
            let is_alt_delete = matches!(physical_key, PhysicalKey::Code(KeyCode::Delete))
                || *logical_key == Key::Named(NamedKey::Delete);

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
                Key::Named(NamedKey::Space) => {
                    let win = self.window.as_ref().map_or((1080, 680), |w| {
                        let s = w.inner_size();
                        (s.width as usize, s.height as usize)
                    });
                    let items = self.tab_or_window_menu(None);
                    self.show_menu(items, (10, 32), win);
                    return;
                }
                Key::Character(ref s) => {
                    if s == " " || matches!(physical_key, PhysicalKey::Code(KeyCode::Space)) {
                        let win = self.window.as_ref().map_or((1080, 680), |w| {
                            let s = w.inner_size();
                            (s.width as usize, s.height as usize)
                        });
                        let items = self.tab_or_window_menu(None);
                        self.show_menu(items, (10, 32), win);
                        return;
                    }
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
        if ctrl && shift && key_is_char(logical_key, physical_key, 'o') {
            if let Some(output) = active.terminal.get_last_output_text() {
                clipboard::copy_text(&mut self.clipboard, &output);
            }
            return;
        }

        // Limpiar pantalla y scrollback buffer (Ctrl + Shift + K)
        if ctrl && shift && key_is_char(logical_key, physical_key, 'k') {
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

        // Salto de palabras y selección en línea de comandos (Ctrl [+ Shift] + Flechas/Home/End/Supr).
        // Ctrl = modificador 5 y Ctrl+Shift = 6 (PSReadLine y readline: SelectBackwardWord…).
        // Ctrl+Shift+Arriba/Abajo ya se atendieron arriba (navegación entre comandos).
        if ctrl {
            let m = if shift { '6' } else { '5' };
            let seq = match logical_key {
                Key::Named(NamedKey::ArrowLeft) => Some(format!("\x1b[1;{m}D")),
                Key::Named(NamedKey::ArrowRight) => Some(format!("\x1b[1;{m}C")),
                Key::Named(NamedKey::ArrowUp) => Some(format!("\x1b[1;{m}A")),
                Key::Named(NamedKey::ArrowDown) => Some(format!("\x1b[1;{m}B")),
                Key::Named(NamedKey::Delete) => Some(format!("\x1b[3;{m}~")),
                Key::Named(NamedKey::Home) => Some(format!("\x1b[1;{m}H")),
                Key::Named(NamedKey::End) => Some(format!("\x1b[1;{m}F")),
                _ => None,
            };
            if let Some(seq) = seq {
                active.pty.write_input(seq.as_bytes());
                return;
            }
        }

        // Combinaciones de control clásicas de terminal (Ctrl+C, Ctrl+D, Ctrl+L, Ctrl+Z, Ctrl+R, etc.)
        if ctrl && !shift {
            if let Key::Character(ref s) = logical_key {
                let c = s.chars().next().unwrap_or(' ').to_ascii_lowercase();
                if c.is_ascii_lowercase() {
                    active.terminal.reset_scroll();
                    let code = (c as u8) - b'a' + 1;
                    active.pty.write_input(&[code]);
                    return;
                }
            }
        }

        // Scroll por página directo sin Shift
        if *logical_key == Key::Named(NamedKey::PageUp) {
            let half = (active.terminal.rows / 2).max(1);
            active.terminal.scroll_up(half);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }
        if *logical_key == Key::Named(NamedKey::PageDown) {
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
            active.terminal.reset_scroll();
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
            Key::Named(NamedKey::Home) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOH");
                } else {
                    active.pty.write_input(b"\x1b[H");
                }
            }
            Key::Named(NamedKey::End) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOF");
                } else {
                    active.pty.write_input(b"\x1b[F");
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOA");
                } else {
                    active.pty.write_input(b"\x1b[A");
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOB");
                } else {
                    active.pty.write_input(b"\x1b[B");
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOC");
                } else {
                    active.pty.write_input(b"\x1b[C");
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if active.terminal.cursor_key_mode {
                    active.pty.write_input(b"\x1bOD");
                } else {
                    active.pty.write_input(b"\x1b[D");
                }
            }
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
                        active.terminal.reset_scroll();
                        active.pty.write_input(t.as_bytes());
                    }
                }
            }
        }
    }
}
