//! Acciones de configuración: tema, fuentes, opacidad, diseño y valores por defecto.

use super::*;

impl TearminalApp {
    /// Alto visible del contenido desplazable de Ajustes
    fn settings_viewport_h(&self) -> usize {
        let win_h = self.window_height();
        win_h.saturating_sub(
            self.top_offset() + SettingsPageRects::CONTENT_TOP + self.status_bar_height(),
        )
    }

    /// Pista y pulgar de la barra de scroll de Ajustes con la geometría actual
    pub(super) fn settings_scrollbar(
        &self,
    ) -> Option<(crate::renderer::Rect, crate::renderer::Rect)> {
        let win_w = self.window_width();
        SettingsPageRects::scrollbar(
            self.settings_page_state.section,
            self.top_offset(),
            win_w.saturating_sub(self.panel_right_inset()),
            self.settings_viewport_h(),
            self.settings_scroll(),
        )
    }

    /// Mueve el scroll de Ajustes para que el pulgar quede con su borde superior en `thumb_top`
    pub(super) fn settings_scroll_to_thumb(&mut self, thumb_top: usize) {
        let Some((track, thumb)) = self.settings_scrollbar() else {
            return;
        };
        let max = SettingsPageRects::max_scroll(
            self.settings_page_state.section,
            self.settings_viewport_h(),
        );
        if let Some(scroll) =
            crate::renderer::scroll_offset_for_thumb_y(track, thumb, thumb_top as f32, max)
        {
            self.settings_page_state.scroll_y = scroll;
            self.last_scroll_time = Instant::now();
        }
    }

    /// Scroll vertical efectivo de Ajustes, acotado al viewport actual (mismo cálculo que el render).
    pub(super) fn settings_scroll(&self) -> usize {
        self.settings_page_state
            .scroll_y
            .min(SettingsPageRects::max_scroll(
                self.settings_page_state.section,
                self.settings_viewport_h(),
            ))
    }

    pub(crate) fn toggle_restore_session(&mut self) {
        self.config.terminal.restore_session = !self.config.terminal.restore_session;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(super) fn adjust_font_size(&mut self, delta: f32) {
        let new_size = (self.config.appearance.font_size + delta).clamp(8.0, 32.0);
        if (new_size - self.config.appearance.font_size).abs() > 0.01 {
            self.config.appearance.font_size = new_size;
            let _ = self.config.save();
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

    pub(super) fn adjust_ui_font_size(&mut self, delta: f32) {
        let new_size = (self.config.appearance.ui_font_size + delta).clamp(8.0, 32.0);
        let rounded = (new_size * 10.0).round() / 10.0;
        if (rounded - self.config.appearance.ui_font_size).abs() > 0.01 {
            self.config.appearance.ui_font_size = rounded;
            if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                let scale_factor = window.scale_factor();
                renderer.update_scale(&self.config, scale_factor);
            }
            let _ = self.config.save();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    pub(super) fn reset_font_size(&mut self) {
        let target_size = TearminalConfig::default().appearance.font_size;
        if (self.config.appearance.font_size - target_size).abs() > 0.01 {
            self.config.appearance.font_size = target_size;
            let _ = self.config.save();
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

    pub(super) fn reload_config(&mut self) {
        self.config = TearminalConfig::load_or_default();
        self.apply_loaded_config();
    }

    /// Asigna un tema de la lista a la ranura que se está editando (oscuro o claro).
    pub(super) fn pick_theme(&mut self, name: &str) {
        if self.settings_page_state.theme_slot_light {
            self.config.appearance.theme_light = name.to_string();
        } else {
            self.config.appearance.theme = name.to_string();
        }
        self.refresh_theme();
    }

    /// Desplaza la lista de temas `rows` filas (positivo = hacia abajo).
    pub(super) fn scroll_theme_list(&mut self, rows: isize) {
        let n = self.settings_page_state.filtered_themes.len();
        let max = n.saturating_sub(SettingsPageRects::THEME_ROWS);
        let cur = self.settings_page_state.theme_scroll.min(max) as isize;
        self.settings_page_state.theme_scroll = (cur + rows).clamp(0, max as isize) as usize;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Deja visible el tema activo en la lista (al abrir Ajustes).
    pub(super) fn reveal_current_theme(&mut self) {
        let a = &self.config.appearance;
        let current = Theme::canonical_name(if self.settings_page_state.theme_slot_light {
            &a.theme_light
        } else {
            &a.theme
        });
        let matches = &self.settings_page_state.filtered_themes;
        let idx = matches.iter().position(|&n| n == current).unwrap_or(0);
        self.settings_page_state.theme_scroll = idx.saturating_sub(2);
    }

    /// Teclado del buscador de temas: escribe, borra palabras/caracteres, navega, selecciona y busca.
    pub(super) fn edit_theme_search(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: Option<&str>,
    ) {
        match logical_key {
            Key::Named(NamedKey::Enter) => {
                let st = &mut self.settings_page_state;
                st.editing_setting = None;
                if let Some(&name) = st.filtered_themes.first() {
                    self.pick_theme(name);
                }
            }
            Key::Named(NamedKey::Escape) => {
                let st = &mut self.settings_page_state;
                st.editing_setting = None;
                st.theme_search_input.clear();
                st.theme_filter.clear();
                st.filtered_themes = Theme::search("");
                st.theme_scroll = 0;
                self.reveal_current_theme();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {
                let mut input = std::mem::take(&mut self.settings_page_state.theme_search_input);
                let (_consumed, changed) =
                    self.handle_text_input_key(&mut input, logical_key, physical_key, text);
                self.settings_page_state.theme_search_input = input;
                if changed {
                    self.sync_theme_search_filter();
                }
            }
        }
    }

    pub(super) fn sync_theme_search_filter(&mut self) {
        let st = &mut self.settings_page_state;
        st.theme_filter = st.theme_search_input.text().to_string();
        st.filtered_themes = Theme::search(&st.theme_filter);
        st.theme_scroll = 0;
    }

    /// Teclado del buscador general de ajustes estilo JetBrains / DataGrip.
    pub(super) fn edit_general_settings_search(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: Option<&str>,
    ) {
        match logical_key {
            Key::Named(NamedKey::Enter) => {
                self.settings_page_state.editing_setting = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.settings_page_state.editing_setting = None;
                self.settings_page_state.search_input.clear();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {
                let mut input = std::mem::take(&mut self.settings_page_state.search_input);
                let (_consumed, changed) =
                    self.handle_text_input_key(&mut input, logical_key, physical_key, text);
                self.settings_page_state.search_input = input;
                if changed {
                    let q = self.settings_page_state.search_input.text().to_lowercase();
                    if !q.is_empty() {
                        if q.contains("tema")
                            || q.contains("color")
                            || q.contains("oscuro")
                            || q.contains("claro")
                            || q.contains("sistema")
                            || q.contains("pestaña")
                            || q.contains("panel")
                            || q.contains("opacid")
                            || q.contains("fuente")
                            || q.contains("espaci")
                        {
                            self.settings_page_state.section =
                                crate::renderer::SettingsSection::Appearance;
                        } else if q.contains("term")
                            || q.contains("shell")
                            || q.contains("cursor")
                            || q.contains("copi")
                            || q.contains("barra")
                            || q.contains("explor")
                            || q.contains("sesi")
                            || q.contains("guard")
                            || q.contains("ajust")
                            || q.contains("ssh")
                            || q.contains("ligat")
                            || q.contains("archiv")
                        {
                            self.settings_page_state.section =
                                crate::renderer::SettingsSection::Terminal;
                        } else if q.contains("ataj")
                            || q.contains("ctrl")
                            || q.contains("alt")
                            || q.contains("shift")
                            || q.contains("tecl")
                            || q.contains("divid")
                            || q.contains("zoom")
                            || q.contains("palet")
                        {
                            self.settings_page_state.section =
                                crate::renderer::SettingsSection::Shortcuts;
                        } else if q.contains("acerc")
                            || q.contains("version")
                            || q.contains("versión")
                            || q.contains("updat")
                            || q.contains("actualiz")
                        {
                            self.settings_page_state.section =
                                crate::renderer::SettingsSection::About;
                        }
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
        }
    }

    /// Recalcula el tema activo (modo + tema de Windows) y lo aplica a ventana y paneles.
    pub(super) fn apply_active_theme(&mut self) {
        self.system_light = crate::theme::is_windows_system_light_mode();
        let new_theme = self.config.appearance.resolve_theme(self.system_light);
        self.theme = new_theme.clone();
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                let was_dark = pane.terminal.theme.is_dark();
                pane.terminal.update_theme(new_theme.clone());
                let now_dark = pane.terminal.theme.is_dark();
                if pane.terminal.report_color_scheme_mode && was_dark != now_dark {
                    let report = pane.terminal.color_scheme_report_bytes();
                    pane.pty.write_input(&report);
                }
            }
        }
        if let Some(w) = &self.window {
            update_windows_theme(w, &self.theme);
            w.request_redraw();
        }
    }

    /// Igual que `apply_active_theme`, tras un cambio de configuración que se guarda.
    fn refresh_theme(&mut self) {
        self.apply_active_theme();
        let _ = self.config.save();
    }

    pub(crate) fn set_theme_mode(&mut self, mode: &str) {
        self.config.appearance.theme_mode = mode.to_string();
        // La lista pasa a editar el tema que ahora está en uso
        self.settings_page_state.theme_slot_light = self
            .config
            .appearance
            .uses_light_theme(crate::theme::is_windows_system_light_mode());
        self.reveal_current_theme();
        self.refresh_theme();
    }

    pub(crate) fn set_tab_layout(&mut self, layout: &str) {
        self.config.appearance.tab_layout = layout.to_string();
        // Con pestañas superiores el explorador va con el panel
        if self.is_horizontal_tabs() && self.file_explorer.visible != self.sidebar_visible {
            self.flip_file_explorer();
        }
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Lado de la ventana donde va el panel de navegación: `"left"` o `"right"`.
    pub(crate) fn set_panel_side(&mut self, side: &str) {
        self.config.appearance.panel_side = side.to_string();
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_status_bar(&mut self) {
        self.config.appearance.show_status_bar = !self.config.appearance.show_status_bar;
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn adjust_opacity(&mut self, delta: f32) {
        let new_op = (self.config.appearance.window_opacity + delta).clamp(0.50, 1.0);
        self.config.appearance.window_opacity = (new_op * 100.0).round() / 100.0;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            apply_window_opacity_native(w, self.config.appearance.window_opacity);
            w.request_redraw();
        }
    }

    pub(crate) fn adjust_line_spacing(&mut self, delta: f32) {
        let new_sp = (self.config.appearance.line_spacing + delta).clamp(0.8, 2.0);
        self.config.appearance.line_spacing = (new_sp * 10.0).round() / 10.0;
        let _ = self.config.save();
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_cursor_blink(&mut self) {
        self.config.terminal.cursor_blink = !self.config.terminal.cursor_blink;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_shell_integration(&mut self) {
        self.config.terminal.shell_integration = !self.config.terminal.shell_integration;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_copy_on_select(&mut self) {
        self.config.terminal.copy_on_select = !self.config.terminal.copy_on_select;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Ajuste de línea global del visor (se aplica a todas las pestañas del visor en el siguiente fotograma)
    pub(crate) fn toggle_word_wrap(&mut self) {
        self.config.terminal.word_wrap = !self.config.terminal.word_wrap;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Los shells nuevos heredan `TEARMINAL_SSH_INTEGRATION`, que lee el wrapper `ssh` de `init.ps1`
    pub(crate) fn apply_ssh_integration_env(&self) {
        let on = self.config.terminal.ssh_integration;
        std::env::set_var("TEARMINAL_SSH_INTEGRATION", if on { "1" } else { "0" });
    }

    pub(crate) fn toggle_ssh_integration(&mut self) {
        self.config.terminal.ssh_integration = !self.config.terminal.ssh_integration;
        self.apply_ssh_integration_env();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_ligatures(&mut self) {
        self.config.terminal.ligatures = !self.config.terminal.ligatures;
        if let Some(r) = &mut self.renderer {
            r.ligatures = self.config.terminal.ligatures;
        }
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn toggle_auto_save(&mut self) {
        self.config.terminal.auto_save = !self.config.terminal.auto_save;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn open_config_file(&self) {
        let path = TearminalConfig::config_path();
        if !path.exists() {
            let _ = self.config.save();
        }
        let path_str = path.to_string_lossy().to_string();
        crate::logger::log(&format!(
            "-> Abriendo archivo de configuración: {}",
            path_str
        ));
        if !platform::shell_open(&path) {
            let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
        }
    }

    pub(crate) fn reset_to_defaults(&mut self) {
        crate::logger::log("-> Restableciendo configuración a valores por defecto");
        self.config = TearminalConfig::default();
        let _ = self.config.save();
        self.apply_loaded_config();
    }

    fn apply_loaded_config(&mut self) {
        self.apply_active_theme();
        self.apply_ssh_integration_env();
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.ligatures = self.config.terminal.ligatures;
            renderer.update_scale(&self.config, scale_factor);
            update_windows_theme(window, &self.theme);
            apply_window_opacity_native(window, self.config.appearance.window_opacity);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
