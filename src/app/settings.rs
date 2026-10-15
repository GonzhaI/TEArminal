//! Acciones de configuración: tema, fuentes, opacidad, diseño y valores por defecto.

use super::*;

impl AxonApp {
    /// Alto visible del contenido desplazable de Ajustes
    fn settings_viewport_h(&self) -> usize {
        let win_h = self
            .window
            .as_ref()
            .map_or(600, |w| w.inner_size().height as usize);
        win_h.saturating_sub(
            self.top_offset() + SettingsPageRects::CONTENT_TOP + self.status_bar_height(),
        )
    }

    /// Pista y pulgar de la barra de scroll de Ajustes con la geometría actual
    pub(super) fn settings_scrollbar(
        &self,
    ) -> Option<(crate::renderer::Rect, crate::renderer::Rect)> {
        let win_w = self
            .window
            .as_ref()
            .map_or(800, |w| w.inner_size().width as usize);
        SettingsPageRects::scrollbar(
            self.settings_page_state.section,
            self.top_offset(),
            win_w,
            self.settings_viewport_h(),
            self.settings_scroll(),
        )
    }

    /// Mueve el scroll de Ajustes para que el pulgar quede con su borde superior en `thumb_top`
    pub(super) fn settings_scroll_to_thumb(&mut self, thumb_top: usize) {
        let Some((track, thumb)) = self.settings_scrollbar() else {
            return;
        };
        let travel = track.3.saturating_sub(thumb.3);
        let max = SettingsPageRects::max_scroll(
            self.settings_page_state.section,
            self.settings_viewport_h(),
        );
        if let Some(scroll) =
            (thumb_top.saturating_sub(track.1).min(travel) * max).checked_div(travel)
        {
            self.settings_page_state.scroll_y = scroll.min(max);
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

    pub fn toggle_restore_session(&mut self) {
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
        let default_cfg = AxonConfig::load_or_default();
        let target_size = default_cfg.appearance.font_size;
        self.config.appearance.font_size = target_size;
        self.config.appearance.ui_font_size = default_cfg.appearance.ui_font_size;
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(super) fn reload_config(&mut self) {
        self.config = AxonConfig::load_or_default();
        self.theme = Theme::from_name(&self.config.appearance.theme);
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
            renderer.update_scale(&self.config, scale_factor);
        }
        self.recompute_grid_for_all_tabs();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    #[allow(dead_code)]
    pub fn cycle_theme(&mut self, next: bool) {
        let themes = crate::theme::Theme::AVAILABLE_THEMES;
        let current = &self.config.appearance.theme;
        let current_idx = themes
            .iter()
            .position(|&t| t.eq_ignore_ascii_case(current))
            .unwrap_or(0);
        let new_idx = if next {
            (current_idx + 1) % themes.len()
        } else {
            (current_idx + themes.len() - 1) % themes.len()
        };
        let new_theme = themes[new_idx];
        self.apply_theme(new_theme);
    }

    pub fn apply_theme(&mut self, theme_name: &str) {
        self.config.appearance.theme = theme_name.to_string();
        self.theme = Theme::from_name(theme_name);
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        let _ = self.config.save();
        if let Some(w) = &self.window {
            update_windows_theme(w, &self.theme);
            w.request_redraw();
        }
    }

    pub fn set_theme_mode(&mut self, mode: &str) {
        self.config.appearance.theme_mode = mode.to_string();
        if mode.eq_ignore_ascii_case("system") {
            let is_light = crate::theme::is_windows_system_light_mode();
            self.apply_theme(if is_light {
                "Catppuccin Latte"
            } else {
                "Catppuccin Frappé"
            });
        } else if mode.eq_ignore_ascii_case("light") {
            self.apply_theme("Catppuccin Latte");
        } else {
            if self.theme.is_light() {
                self.apply_theme("Catppuccin Frappé");
            }
        }
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn set_tab_layout(&mut self, layout: &str) {
        self.config.appearance.tab_layout = layout.to_string();
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_status_bar(&mut self) {
        self.config.appearance.show_status_bar = !self.config.appearance.show_status_bar;
        self.recompute_grid_for_all_tabs();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn adjust_opacity(&mut self, delta: f32) {
        let new_op = (self.config.appearance.window_opacity + delta).clamp(0.50, 1.0);
        self.config.appearance.window_opacity = (new_op * 100.0).round() / 100.0;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            apply_window_opacity_native(w, self.config.appearance.window_opacity);
            w.request_redraw();
        }
    }

    pub fn adjust_line_spacing(&mut self, delta: f32) {
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

    pub fn toggle_cursor_blink(&mut self) {
        self.config.terminal.cursor_blink = !self.config.terminal.cursor_blink;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_ghostty_prompt(&mut self) {
        self.config.terminal.ghostty_prompt = !self.config.terminal.ghostty_prompt;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_copy_on_select(&mut self) {
        self.config.terminal.copy_on_select = !self.config.terminal.copy_on_select;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Ajuste de línea global del visor (se aplica a todas las pestañas del visor en el siguiente fotograma)
    pub fn toggle_word_wrap(&mut self) {
        self.config.terminal.word_wrap = !self.config.terminal.word_wrap;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Los shells nuevos heredan `AXON_SSH_INTEGRATION`, que lee el wrapper `ssh` de `init.ps1`
    pub fn apply_ssh_integration_env(&self) {
        let on = self.config.terminal.ssh_integration;
        std::env::set_var("AXON_SSH_INTEGRATION", if on { "1" } else { "0" });
    }

    pub fn toggle_ssh_integration(&mut self) {
        self.config.terminal.ssh_integration = !self.config.terminal.ssh_integration;
        self.apply_ssh_integration_env();
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_auto_save(&mut self) {
        self.config.terminal.auto_save = !self.config.terminal.auto_save;
        let _ = self.config.save();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn open_config_file(&self) {
        let path = AxonConfig::config_path();
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

    pub fn reset_to_defaults(&mut self) {
        crate::logger::log("-> Restableciendo configuración a valores por defecto");
        self.config = AxonConfig::default();
        let _ = self.config.save();
        self.theme = if self
            .config
            .appearance
            .theme_mode
            .eq_ignore_ascii_case("system")
        {
            if crate::theme::is_windows_system_light_mode() {
                Theme::catppuccin_latte()
            } else {
                Theme::from_name(&self.config.appearance.theme)
            }
        } else if self
            .config
            .appearance
            .theme_mode
            .eq_ignore_ascii_case("light")
        {
            Theme::catppuccin_latte()
        } else {
            Theme::from_name(&self.config.appearance.theme)
        };
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.terminal.theme = self.theme.clone();
            }
        }
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let scale_factor = window.scale_factor();
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
