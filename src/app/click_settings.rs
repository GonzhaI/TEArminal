//! Gestión de eventos de clic dentro de la página y modal de Ajustes.

use super::*;
use crate::config::KeepAwakeMode;
use crate::renderer::SettingsPageRects;

impl TearminalApp {
    pub(super) fn click_settings_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        is_settings_active: bool,
    ) -> bool {
        // Clics en la pestaña de Ajustes (si está activa)
        if is_settings_active {
            let (modal_x, modal_y, modal_w, modal_h) = self.settings_modal();
            let in_modal =
                mx >= modal_x && mx < modal_x + modal_w && my >= modal_y && my < modal_y + modal_h;
            if !in_modal {
                // Un clic en el velo cierra el modal
                if state == ElementState::Pressed && button == MouseButton::Left {
                    self.close_settings();
                }
                return true;
            }
            // Coordenadas locales al modal: su esquina es el origen de la página
            let (mx, my) = (mx - modal_x, my - modal_y);
            let (offset, top_offset) = (0, 0);
            // La navegación lateral no se desplaza con el contenido: usa la `y` sin scroll
            let nav_my = my;
            if state == ElementState::Pressed && button == MouseButton::Left {
                let rects = SettingsPageRects::compute_modal(offset, top_offset, modal_w, modal_h);
                // Barra de scroll de Ajustes: arrastrar el pulgar o saltar al punto pulsado
                if let Some((track, thumb)) = self.settings_scrollbar() {
                    let right = modal_w;
                    if crate::renderer::ScrollbarRects::hit(track, right, mx, my) {
                        if my >= thumb.1 && my < thumb.1 + thumb.3 {
                            self.scrollbar_drag = Some(ScrollbarDragState {
                                target: ScrollbarTarget::Settings,
                                grab_offset: (my - thumb.1) as f32,
                            });
                        } else {
                            self.settings_scroll_to_thumb(my.saturating_sub(thumb.3 / 2));
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                }

                // Bajo la navegación el contenido está desplazado: se traduce el ratón a coordenadas sin scroll
                let my = if my >= top_offset + SettingsPageRects::CONTENT_TOP {
                    my + self.settings_scroll()
                } else {
                    my
                };

                // Botón Restablecer valores por defecto (Header)
                if SettingsPageRects::inside(mx, my, rects.reset_defaults_btn) {
                    crate::logger::log("-> Clic en restablecer valores por defecto (Header)");
                    self.reset_to_defaults();
                    return true;
                }

                // Buscador rápido de ajustes estilo JetBrains
                if SettingsPageRects::inside(mx, my, rects.search_box) {
                    self.settings_page_state.editing_setting =
                        Some(crate::renderer::EditingSettingField::GeneralSearch);
                    let text_start_x = rects.search_box.0 + 28;
                    let mut input = std::mem::take(&mut self.settings_page_state.search_input);
                    self.handle_text_input_click(&mut input, mx, text_start_x);
                    self.settings_page_state.search_input = input;
                    return true;
                } else if self.settings_page_state.editing_setting
                    == Some(crate::renderer::EditingSettingField::GeneralSearch)
                {
                    self.settings_page_state.editing_setting = None;
                }

                // Pestañas de navegación
                if SettingsPageRects::inside(mx, nav_my, rects.nav_appearance) {
                    self.settings_page_state.section = SettingsSection::Appearance;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                } else if SettingsPageRects::inside(mx, nav_my, rects.nav_terminal) {
                    self.settings_page_state.section = SettingsSection::Terminal;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                } else if SettingsPageRects::inside(mx, nav_my, rects.nav_shortcuts) {
                    self.settings_page_state.section = SettingsSection::Shortcuts;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                } else if SettingsPageRects::inside(mx, nav_my, rects.nav_about) {
                    self.settings_page_state.section = SettingsSection::About;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }

                // Contenido de la sección activa
                match self.settings_page_state.section {
                    SettingsSection::Appearance => {
                        for (i, r) in [rects.theme_slot_dark_btn, rects.theme_slot_light_btn]
                            .into_iter()
                            .enumerate()
                        {
                            if SettingsPageRects::inside(mx, my, r) {
                                self.settings_page_state.theme_slot_light = i == 1;
                                self.reveal_current_theme();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return true;
                            }
                        }
                        let searching = SettingsPageRects::inside(mx, my, rects.theme_search_box);
                        if searching {
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::ThemeSearch);
                            let text_start_x = rects.theme_search_box.0 + 30;
                            let mut input =
                                std::mem::take(&mut self.settings_page_state.theme_search_input);
                            self.handle_text_input_click(&mut input, mx, text_start_x);
                            self.settings_page_state.theme_search_input = input;
                            return true;
                        } else if self.settings_page_state.editing_setting
                            == Some(crate::renderer::EditingSettingField::ThemeSearch)
                        {
                            self.settings_page_state.editing_setting = None;
                        }
                        if let Some(row) = rects.theme_row_at(mx, my) {
                            let matches = &self.settings_page_state.filtered_themes;
                            let first = self
                                .settings_page_state
                                .theme_scroll
                                .min(matches.len().saturating_sub(SettingsPageRects::THEME_ROWS));
                            if let Some(&th_name) = matches.get(first + row) {
                                crate::logger::log(&format!("-> Clic en tema: {}", th_name));
                                self.pick_theme(th_name);
                                return true;
                            }
                        }
                        if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                            crate::logger::log("-> Clic en modo de tema: Sistema");
                            self.set_theme_mode("system");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.theme_mode_dark_btn) {
                            crate::logger::log("-> Clic en modo de tema: Oscuro");
                            self.set_theme_mode("dark");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.theme_mode_light_btn) {
                            crate::logger::log("-> Clic en modo de tema: Claro");
                            self.set_theme_mode("light");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.font_value_box) {
                            crate::logger::log("-> Clic en caja de valor: tamaño de fuente");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::FontSize);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.1}", self.config.appearance.font_size);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.font_family_prev_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.step_font_family(-1);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.font_family_next_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.step_font_family(1);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.font_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente -");
                            self.adjust_font_size(-1.0);
                            let _ = self.config.save();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.font_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente +");
                            self.adjust_font_size(1.0);
                            let _ = self.config.save();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_value_box) {
                            crate::logger::log("-> Clic en caja de valor: tamaño de fuente UI");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::UiFontSize);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.1}", self.config.appearance.ui_font_size);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente UI -");
                            self.adjust_ui_font_size(-1.0);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente UI +");
                            self.adjust_ui_font_size(1.0);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_value_box) {
                            crate::logger::log("-> Clic en caja de valor: espaciado de línea");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::LineSpacing);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.2}", self.config.appearance.line_spacing);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en espaciado -");
                            self.adjust_line_spacing(-0.1);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en espaciado +");
                            self.adjust_line_spacing(0.1);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_value_box) {
                            crate::logger::log("-> Clic en caja de valor: opacidad");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::WindowOpacity);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.0}", self.config.appearance.window_opacity * 100.0);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en opacidad -");
                            self.adjust_opacity(-0.05);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en opacidad +");
                            self.adjust_opacity(0.05);
                            return true;
                        } else {
                            self.settings_page_state.editing_setting = None;
                        }
                    }
                    SettingsSection::Terminal => {
                        if SettingsPageRects::inside(mx, my, rects.cursor_blink_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en parpadeo del cursor");
                            self.toggle_cursor_blink();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.cursor_shape_block_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_cursor_shape("block");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.cursor_shape_beam_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_cursor_shape("beam");
                            return true;
                        } else if SettingsPageRects::inside(
                            mx,
                            my,
                            rects.cursor_shape_underline_btn,
                        ) {
                            self.settings_page_state.editing_setting = None;
                            self.set_cursor_shape("underline");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.shell_pwsh_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_default_shell("pwsh");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.shell_powershell_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_default_shell("powershell");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.shell_cmd_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_default_shell("cmd");
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.scrollback_value_box) {
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::ScrollbackLines);
                            self.settings_page_state.setting_input_buffer =
                                format!("{}", self.config.terminal.scrollback_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.scrollback_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.adjust_scrollback_lines(-1000);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.scrollback_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.adjust_scrollback_lines(1000);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.shell_integration_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en integración de shell");
                            self.toggle_shell_integration();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.copy_on_select_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en copiar al seleccionar");
                            self.toggle_copy_on_select();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en alternar barra de estado");
                            self.toggle_status_bar();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.show_dimensions_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_show_dimensions();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn)
                        {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log(
                                "-> Clic en alternar explorador lateral de archivos (Settings)",
                            );
                            self.toggle_file_explorer();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.restore_session_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en alternar restaurar pestañas al iniciar");
                            self.toggle_restore_session();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.auto_save_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en alternar auto-guardar archivos");
                            self.toggle_auto_save();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.word_wrap_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_word_wrap();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ssh_integration_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_ssh_integration();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.osc52_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_osc52();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ligatures_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_ligatures();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.ai_limits_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_ai_limits();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.auto_check_updates_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.toggle_auto_check_updates();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.keep_awake_off_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_keep_awake_mode(KeepAwakeMode::Off);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.keep_awake_agent_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_keep_awake_mode(KeepAwakeMode::Agent);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.keep_awake_active_btn) {
                            self.settings_page_state.editing_setting = None;
                            self.set_keep_awake_mode(KeepAwakeMode::Active);
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.open_file_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en abrir archivo tearminal.toml");
                            self.open_config_file();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.term_reset_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log(
                                "-> Clic en restablecer valores por defecto (Terminal)",
                            );
                            self.reset_to_defaults();
                            return true;
                        } else {
                            self.settings_page_state.editing_setting = None;
                        }
                    }
                    SettingsSection::About => {
                        if SettingsPageRects::inside(mx, my, rects.about_open_file_btn) {
                            crate::logger::log("-> Clic en abrir archivo tearminal.toml (About)");
                            self.open_config_file();
                            return true;
                        } else if SettingsPageRects::inside(mx, my, rects.about_reset_btn) {
                            crate::logger::log(
                                "-> Clic en restablecer valores por defecto (About)",
                            );
                            self.reset_to_defaults();
                            return true;
                        }
                    }
                    SettingsSection::Shortcuts => {}
                }
            }
            return true;
        }

        false
    }
}
