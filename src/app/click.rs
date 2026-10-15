//! Clics: pestañas, paneles, explorador, ajustes, selección y TUIs.

use super::*;

impl AxonApp {
    /// Botones del ratón: pestañas, paneles, explorador, ajustes, selección y TUIs.
    pub(super) fn on_mouse_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: ElementState,
        button: MouseButton,
    ) {
        let mx = self.mouse_pos.x as usize;
        let my = self.mouse_pos.y as usize;

        let (win_w, win_h) = if let Some(w) = &self.window {
            let s = w.inner_size();
            (s.width as usize, s.height as usize)
        } else {
            (800, 600)
        };

        let offset = self.sidebar_offset();
        let is_settings_active = self.tabs[self.active_tab_index].is_settings;

        // Si soltamos el botón izquierdo, cancelar arrastre de la barra de desplazamiento
        if button == MouseButton::Left && state == ElementState::Released {
            let was_dragging = self.scrollbar_dragging
                || self.sidebar_scrollbar_dragging
                || self.explorer_scrollbar_dragging
                || self.viewer_scrollbar_dragging;
            if was_dragging {
                self.scrollbar_dragging = false;
                self.scrollbar_drag_pane_id = None;
                self.sidebar_scrollbar_dragging = false;
                self.explorer_scrollbar_dragging = false;
                self.viewer_scrollbar_dragging = false;
                self.last_scroll_time = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        }

        // Redimensionamiento de ventana al arrastrar bordes (ventana frameless)
        if button == MouseButton::Left && state == ElementState::Pressed {
            let border_margin = 6;
            let at_left = mx < border_margin;
            let at_right = mx >= win_w.saturating_sub(border_margin);
            let at_top = my < border_margin;
            let at_bottom = my >= win_h.saturating_sub(border_margin);

            let resize_direction = if at_top && at_left {
                Some(winit::window::ResizeDirection::NorthWest)
            } else if at_top && at_right {
                Some(winit::window::ResizeDirection::NorthEast)
            } else if at_bottom && at_left {
                Some(winit::window::ResizeDirection::SouthWest)
            } else if at_bottom && at_right {
                Some(winit::window::ResizeDirection::SouthEast)
            } else if at_left {
                Some(winit::window::ResizeDirection::West)
            } else if at_right {
                Some(winit::window::ResizeDirection::East)
            } else if at_top {
                Some(winit::window::ResizeDirection::North)
            } else if at_bottom {
                Some(winit::window::ResizeDirection::South)
            } else {
                None
            };

            if let Some(dir) = resize_direction {
                if let Some(w) = &self.window {
                    let _ = w.drag_resize_window(dir);
                    return;
                }
            }
        }

        // Si estamos en modo de pestañas horizontales superiores
        if self.is_horizontal_tabs() && my < TopTabBarRects::HEIGHT {
            if state == ElementState::Pressed && button == MouseButton::Left {
                let top_rects = TopTabBarRects::compute(win_w, self.tabs.len());

                // Clic en [-] Minimizar
                if TopTabBarRects::inside(mx, my, top_rects.win_min_btn) {
                    if let Some(w) = &self.window {
                        w.set_minimized(true);
                    }
                    return;
                }

                // Clic en [□] Maximizar / Restaurar
                if TopTabBarRects::inside(mx, my, top_rects.win_max_btn) {
                    if let Some(w) = &self.window {
                        w.set_maximized(!w.is_maximized());
                    }
                    return;
                }

                // Clic en [✕] Cerrar ventana
                if TopTabBarRects::inside(mx, my, top_rects.win_close_btn) {
                    self.save_session();
                    event_loop.exit();
                    return;
                }

                // Clic en [+] nueva pestaña
                if TopTabBarRects::inside(mx, my, top_rects.plus_btn) {
                    crate::logger::log("-> Clic en [+] top bar: nueva terminal");
                    self.create_new_tab();
                    return;
                }

                // Clic en [◈] Explorador de archivos
                if TopTabBarRects::inside(mx, my, top_rects.files_btn) {
                    crate::logger::log("-> Clic en [◈] top bar: Explorador de archivos");
                    self.toggle_file_explorer();
                    return;
                }

                // Clic en [⚙] Ajustes
                if TopTabBarRects::inside(mx, my, top_rects.settings_btn) {
                    crate::logger::log("-> Clic en [⚙] top bar: Ajustes");
                    self.open_or_switch_settings_tab();
                    return;
                }

                // Clic en pestañas o en botón cerrar 'x'
                for idx in 0..self.tabs.len() {
                    let tr = top_rects.tab_rect(idx);
                    if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                        if self.tabs.len() > 1 {
                            let cr = top_rects.close_btn_rect(idx);
                            if mx >= cr.0 && mx < cr.0 + cr.2 && my >= cr.1 && my < cr.1 + cr.3 {
                                crate::logger::log(&format!("-> Clic en [x] pestaña {}", idx));
                                self.close_tab(idx, event_loop);
                                return;
                            }
                        }
                        crate::logger::log(&format!("-> Clic en pestaña {}", idx));
                        self.switch_to_tab(idx);
                        return;
                    }
                }

                // Arrastrar ventana o doble clic para maximizar
                let now = Instant::now();
                if now.duration_since(self.last_titlebar_click).as_millis() < 300 {
                    if let Some(w) = &self.window {
                        w.set_maximized(!w.is_maximized());
                    }
                } else {
                    self.last_titlebar_click = now;
                    if let Some(w) = &self.window {
                        let _ = w.drag_window();
                    }
                }
            }
            return;
        }

        // Modalidad vertical: comprobación de barra de título superior
        if !self.is_horizontal_tabs() && my < TopTitleBarRects::HEIGHT {
            if state == ElementState::Pressed && button == MouseButton::Left {
                let title_rects = TopTitleBarRects::compute(win_w);

                if TopTabBarRects::inside(mx, my, title_rects.win_min_btn) {
                    if let Some(w) = &self.window {
                        w.set_minimized(true);
                    }
                    return;
                }

                if TopTabBarRects::inside(mx, my, title_rects.win_max_btn) {
                    if let Some(w) = &self.window {
                        w.set_maximized(!w.is_maximized());
                    }
                    return;
                }

                if TopTabBarRects::inside(mx, my, title_rects.win_close_btn) {
                    self.save_session();
                    event_loop.exit();
                    return;
                }

                if TopTabBarRects::inside(mx, my, title_rects.files_btn) {
                    self.toggle_file_explorer();
                    return;
                }

                if TopTabBarRects::inside(mx, my, title_rects.settings_btn) {
                    self.open_or_switch_settings_tab();
                    return;
                }

                // Arrastrar ventana o doble clic para maximizar
                let now = Instant::now();
                if now.duration_since(self.last_titlebar_click).as_millis() < 300 {
                    if let Some(w) = &self.window {
                        w.set_maximized(!w.is_maximized());
                    }
                } else {
                    self.last_titlebar_click = now;
                    if let Some(w) = &self.window {
                        let _ = w.drag_window();
                    }
                }
            }
            return;
        }

        // Modalidad vertical: comprobación de botones de alternar barra lateral y clics en el sidebar
        if !self.is_horizontal_tabs() {
            let status_bar_h = self.status_bar_height();
            let toggle_btn_w = if self.sidebar_visible {
                SIDEBAR_WIDTH.saturating_sub(16)
            } else {
                32
            };
            let toggle_btn_h = 30;
            let toggle_btn_x = 8;
            let toggle_btn_y = win_h.saturating_sub(toggle_btn_h + 8 + status_bar_h);

            let is_click_toggle = mx >= toggle_btn_x
                && mx < toggle_btn_x + toggle_btn_w
                && my >= toggle_btn_y
                && my < toggle_btn_y + toggle_btn_h;

            if is_click_toggle && state == ElementState::Pressed && button == MouseButton::Left {
                crate::logger::log("-> Clic en botón inferior de alternar barra lateral");
                self.toggle_sidebar();
                return;
            }

            // Si la barra lateral está colapsada, clic en el botón flotante de Ajustes
            if !self.sidebar_visible {
                let float_set_x = 44;
                let float_set_w = 32;
                let is_click_float_set = mx >= float_set_x
                    && mx < float_set_x + float_set_w
                    && my >= toggle_btn_y
                    && my < toggle_btn_y + toggle_btn_h;
                if is_click_float_set
                    && state == ElementState::Pressed
                    && button == MouseButton::Left
                {
                    crate::logger::log("-> Clic en botón flotante de Ajustes");
                    self.open_or_switch_settings_tab();
                    return;
                }
            }

            // Clics en la barra lateral abierta
            if self.sidebar_visible && mx < SIDEBAR_WIDTH {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    // Clic en botón "+"
                    let plus_btn_x = SIDEBAR_WIDTH.saturating_sub(34);
                    let plus_btn_y = 8;
                    let plus_btn_w = 24;
                    let plus_btn_h = 24;
                    if mx >= plus_btn_x
                        && mx < plus_btn_x + plus_btn_w
                        && my >= plus_btn_y
                        && my < plus_btn_y + plus_btn_h
                    {
                        crate::logger::log("-> Clic en [+] sidebar: nueva terminal");
                        self.create_new_tab();
                        return;
                    }

                    // Clic en la barra de desplazamiento de la barra lateral vertical
                    let usable_sidebar_h = win_h.saturating_sub(status_bar_h);
                    if let Some((track, thumb)) = TerminalRenderer::sidebar_scrollbar_geometry(
                        SIDEBAR_WIDTH,
                        usable_sidebar_h,
                        self.tabs.len(),
                        self.sidebar_scroll_offset,
                    ) {
                        let hit_x = track.0.saturating_sub(4);
                        if mx >= hit_x
                            && mx < SIDEBAR_WIDTH
                            && my >= track.1
                            && my < track.1 + track.3
                        {
                            if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                self.sidebar_scrollbar_dragging = true;
                                self.sidebar_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                            } else {
                                let available_travel = track.3.saturating_sub(thumb.3);
                                let tab_item_h = 36;
                                let tab_spacing = 4;
                                let list_start_y = 42;
                                let toggle_h = 30;
                                let toggle_y = usable_sidebar_h.saturating_sub(toggle_h + 8);
                                let list_max_y = toggle_y.saturating_sub(8);
                                let visible_items = if list_max_y > list_start_y {
                                    (list_max_y - list_start_y) / (tab_item_h + tab_spacing)
                                } else {
                                    1
                                };
                                let max_offset = self.tabs.len().saturating_sub(visible_items);
                                if available_travel > 0 && max_offset > 0 {
                                    let click_rel =
                                        (my.saturating_sub(track.1 + thumb.3 / 2)) as f32;
                                    let ratio =
                                        (click_rel / available_travel as f32).clamp(0.0, 1.0);
                                    self.sidebar_scroll_offset =
                                        ((ratio * max_offset as f32).round() as usize)
                                            .min(max_offset);
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                            return;
                        }
                    }

                    // Clic en pestañas o botón cerrar "x"
                    let tab_item_h = 36;
                    let tab_spacing = 4;
                    let list_start_y = 42;
                    let tab_x = 8;
                    let tab_w = SIDEBAR_WIDTH.saturating_sub(16);
                    let list_max_y = toggle_btn_y.saturating_sub(8);

                    if my >= list_start_y && my < list_max_y {
                        let rel_y = my - list_start_y;
                        let item_total = tab_item_h + tab_spacing;
                        let vis_idx = rel_y / item_total;
                        let tab_idx = vis_idx + self.sidebar_scroll_offset;
                        let in_item = (rel_y % item_total) < tab_item_h;

                        if in_item && tab_idx < self.tabs.len() && mx >= tab_x && mx < tab_x + tab_w
                        {
                            let tab_y = list_start_y + vis_idx * item_total;
                            let close_x = tab_x + tab_w.saturating_sub(22);
                            let close_y = tab_y + 6;
                            let close_size = 20;

                            if self.tabs.len() > 1
                                && mx >= close_x
                                && mx < close_x + close_size
                                && my >= close_y
                                && my < close_y + close_size
                            {
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
        }

        // 2. Clics en la pestaña de Ajustes (si está activa)
        if is_settings_active {
            let top_offset = self.top_offset();
            if mx >= offset
                && my >= top_offset
                && state == ElementState::Pressed
                && button == MouseButton::Left
            {
                let page_w = win_w.saturating_sub(offset);
                let page_h = win_h.saturating_sub(top_offset);
                let rects = SettingsPageRects::compute(offset, top_offset, page_w, page_h);

                // Botón Restablecer valores por defecto (Header)
                if SettingsPageRects::inside(mx, my, rects.reset_defaults_btn) {
                    crate::logger::log("-> Clic en restablecer valores por defecto (Header)");
                    self.reset_to_defaults();
                    return;
                }

                // Pestañas de navegación
                if SettingsPageRects::inside(mx, my, rects.nav_appearance) {
                    self.settings_page_state.section = SettingsSection::Appearance;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_terminal) {
                    self.settings_page_state.section = SettingsSection::Terminal;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_shortcuts) {
                    self.settings_page_state.section = SettingsSection::Shortcuts;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_about) {
                    self.settings_page_state.section = SettingsSection::About;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Contenido de la sección activa
                match self.settings_page_state.section {
                    SettingsSection::Appearance => {
                        for (idx, r) in rects.theme_buttons.iter().enumerate() {
                            if SettingsPageRects::inside(mx, my, *r) {
                                if let Some(th_name) = Theme::AVAILABLE_THEMES.get(idx) {
                                    crate::logger::log(&format!("-> Clic en tema: {}", th_name));
                                    self.config.appearance.theme_mode =
                                        if th_name.eq_ignore_ascii_case("Catppuccin Latte") {
                                            "light".to_string()
                                        } else {
                                            "dark".to_string()
                                        };
                                    self.apply_theme(th_name);
                                    return;
                                }
                            }
                        }
                        if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                            crate::logger::log("-> Clic en modo de tema: Sistema");
                            self.set_theme_mode("system");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.theme_mode_dark_btn) {
                            crate::logger::log("-> Clic en modo de tema: Oscuro");
                            self.set_theme_mode("dark");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.theme_mode_light_btn) {
                            crate::logger::log("-> Clic en modo de tema: Claro");
                            self.set_theme_mode("light");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.tab_layout_vertical_btn) {
                            crate::logger::log("-> Clic en disposición: Vertical");
                            self.set_tab_layout("vertical");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.tab_layout_horizontal_btn)
                        {
                            crate::logger::log("-> Clic en disposición: Horizontal");
                            self.set_tab_layout("horizontal");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.font_value_box) {
                            crate::logger::log("-> Clic en caja de valor: tamaño de fuente");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::FontSize);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.1}", self.config.appearance.font_size);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.font_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente -");
                            self.adjust_font_size(-1.0);
                            let _ = self.config.save();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.font_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente +");
                            self.adjust_font_size(1.0);
                            let _ = self.config.save();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_value_box) {
                            crate::logger::log("-> Clic en caja de valor: tamaño de fuente UI");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::UiFontSize);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.1}", self.config.appearance.ui_font_size);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente UI -");
                            self.adjust_ui_font_size(-1.0);
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ui_font_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en fuente UI +");
                            self.adjust_ui_font_size(1.0);
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_value_box) {
                            crate::logger::log("-> Clic en caja de valor: espaciado de línea");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::LineSpacing);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.2}", self.config.appearance.line_spacing);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en espaciado -");
                            self.adjust_line_spacing(-0.1);
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.spacing_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en espaciado +");
                            self.adjust_line_spacing(0.1);
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_value_box) {
                            crate::logger::log("-> Clic en caja de valor: opacidad");
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::WindowOpacity);
                            self.settings_page_state.setting_input_buffer =
                                format!("{:.0}", self.config.appearance.window_opacity * 100.0);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_minus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en opacidad -");
                            self.adjust_opacity(-0.05);
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.opacity_plus_btn) {
                            self.settings_page_state.editing_setting = None;
                            crate::logger::log("-> Clic en opacidad +");
                            self.adjust_opacity(0.05);
                            return;
                        } else {
                            self.settings_page_state.editing_setting = None;
                        }
                    }
                    SettingsSection::Terminal => {
                        if SettingsPageRects::inside(mx, my, rects.cursor_blink_btn) {
                            crate::logger::log("-> Clic en parpadeo del cursor");
                            self.toggle_cursor_blink();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ghostty_prompt_btn) {
                            crate::logger::log("-> Clic en prompt Ghostty");
                            self.toggle_ghostty_prompt();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.copy_on_select_btn) {
                            crate::logger::log("-> Clic en copiar al seleccionar");
                            self.toggle_copy_on_select();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn) {
                            crate::logger::log("-> Clic en alternar barra de estado");
                            self.toggle_status_bar();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn)
                        {
                            crate::logger::log(
                                "-> Clic en alternar explorador lateral de archivos (Settings)",
                            );
                            self.toggle_file_explorer();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.restore_session_btn) {
                            crate::logger::log("-> Clic en alternar restaurar pestañas al iniciar");
                            self.toggle_restore_session();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.open_file_btn) {
                            crate::logger::log("-> Clic en abrir archivo axon.toml");
                            self.open_config_file();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.term_reset_btn) {
                            crate::logger::log(
                                "-> Clic en restablecer valores por defecto (Terminal)",
                            );
                            self.reset_to_defaults();
                            return;
                        }
                    }
                    SettingsSection::About => {
                        if SettingsPageRects::inside(mx, my, rects.open_file_btn) {
                            crate::logger::log("-> Clic en abrir archivo axon.toml (About)");
                            self.open_config_file();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.term_reset_btn) {
                            crate::logger::log(
                                "-> Clic en restablecer valores por defecto (About)",
                            );
                            self.reset_to_defaults();
                            return;
                        }
                    }
                    SettingsSection::Shortcuts => {}
                }
            }
            return;
        }

        // Clics dentro del panel del explorador lateral de archivos (a la derecha)
        if self.file_explorer.visible && !is_settings_active {
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let (panel_x, panel_y, panel_w, panel_h) =
                crate::explorer::FileExplorer::panel_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                );
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    // Clic en la barra de desplazamiento del explorador de archivos
                    if let Some((track, thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                        win_w,
                        win_h,
                        top_offset,
                        status_bar_h,
                        &self.file_explorer,
                    ) {
                        let hit_x = track.0.saturating_sub(4);
                        if mx >= hit_x
                            && mx < panel_x + panel_w
                            && my >= track.1
                            && my < track.1 + track.3
                        {
                            if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                self.explorer_scrollbar_dragging = true;
                                self.explorer_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                            } else {
                                let available_travel = track.3.saturating_sub(thumb.3);
                                let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                                let list_start_y = panel_y
                                    + crate::explorer::FileExplorer::HEADER_HEIGHT
                                    + crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
                                let list_h = (panel_y + panel_h).saturating_sub(list_start_y);
                                let visible_count = list_h.checked_div(item_h).unwrap_or(1);
                                let max_offset =
                                    self.file_explorer.items.len().saturating_sub(visible_count);
                                if available_travel > 0 && max_offset > 0 {
                                    let click_rel =
                                        (my.saturating_sub(track.1 + thumb.3 / 2)) as f32;
                                    let ratio =
                                        (click_rel / available_travel as f32).clamp(0.0, 1.0);
                                    self.file_explorer.scroll_offset =
                                        ((ratio * max_offset as f32).round() as usize)
                                            .min(max_offset);
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                            return;
                        }
                    }

                    let close_x = panel_x + panel_w.saturating_sub(28);
                    if mx >= close_x && mx < close_x + 22 && my >= panel_y + 7 && my < panel_y + 29
                    {
                        crate::logger::log("-> Clic en [x] cerrar explorador de archivos");
                        self.toggle_file_explorer();
                        return;
                    }

                    let refresh_x = panel_x + panel_w.saturating_sub(56);
                    if mx >= refresh_x
                        && mx < refresh_x + 22
                        && my >= panel_y + 7
                        && my < panel_y + 29
                    {
                        crate::logger::log("-> Clic en [↻] refrescar explorador de archivos");
                        self.file_explorer.refresh();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }

                    let sub_y = panel_y + crate::explorer::FileExplorer::HEADER_HEIGHT;
                    let back_x = panel_x + 8;
                    if mx >= back_x && mx < back_x + 22 && my >= sub_y + 4 && my < sub_y + 24 {
                        if self.file_explorer.can_go_back() {
                            crate::logger::log("-> Clic en [<] atrás en explorador");
                            self.file_explorer.go_back();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }

                    let fwd_x = back_x + 24;
                    if mx >= fwd_x && mx < fwd_x + 22 && my >= sub_y + 4 && my < sub_y + 24 {
                        if self.file_explorer.can_go_forward() {
                            crate::logger::log("-> Clic en [>] adelante en explorador");
                            self.file_explorer.go_forward();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }

                    let up_x = fwd_x + 24;
                    if mx >= up_x && mx < up_x + 22 && my >= sub_y + 4 && my < sub_y + 24 {
                        crate::logger::log("-> Clic en [^] subir nivel en explorador");
                        self.file_explorer.navigate_up();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }

                    // Al interactuar con el explorador gana el foco de teclado
                    self.file_explorer.is_focused = true;

                    // Solo procesar clics en elementos si estamos a la izquierda de la barra de desplazamiento
                    if mx < panel_x + panel_w.saturating_sub(16) {
                        if let Some(idx) = self.file_explorer.item_at_y(my, panel_y) {
                            if idx < self.file_explorer.items.len() {
                                self.file_explorer.selected_index = Some(idx);
                                let item = self.file_explorer.items[idx].clone();
                                self.file_explorer.selected_path = Some(item.path.clone());

                                if item.is_dir {
                                    crate::logger::log(&format!(
                                        "-> Clic en carpeta: {}",
                                        item.name
                                    ));
                                    self.file_explorer.toggle_expand(idx);
                                } else {
                                    // Con solo un clic se abre y visualiza el archivo inmediatamente
                                    crate::logger::log(&format!(
                                        "-> Abrir visor de archivos: {}",
                                        item.name
                                    ));
                                    self.open_file_in_viewer(&item.path);
                                }
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }
                        }
                    }
                }
                return;
            }
        }

        // Clics dentro del visor de archivos si la pestaña activa es un file viewer
        let is_viewer_active = self.tabs[self.active_tab_index].file_viewer.is_some();
        if is_viewer_active {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let right_offset = self.file_explorer_offset();
            let viewer_w = win_w.saturating_sub(offset + right_offset);
            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let char_w = self.renderer.as_ref().map(|r| r.char_width).unwrap_or(8);

            if mx >= offset && mx < offset + viewer_w && my >= top_offset && my < top_offset + viewer_h {
                self.file_explorer.is_focused = false;
                if state == ElementState::Pressed && button == MouseButton::Left {
                    if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                        // Comprobar clic en la barra de desplazamiento del visor
                        if let Some((track, thumb)) = TerminalRenderer::file_viewer_scrollbar_geometry(
                            offset,
                            top_offset,
                            viewer_w,
                            viewer_h,
                            viewer,
                            row_h,
                        ) {
                            let hit_x = track.0.saturating_sub(4);
                            if mx >= hit_x && mx < offset + viewer_w && my >= track.1 && my < track.1 + track.3 {
                                if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                    self.viewer_scrollbar_dragging = true;
                                    self.viewer_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                                } else {
                                    let available_travel = track.3.saturating_sub(thumb.3);
                                    let content_h = viewer_h.saturating_sub(38);
                                    let visible_lines = (content_h / row_h.max(16)).max(1);
                                    let max_offset = viewer.lines.len().saturating_sub(visible_lines);
                                    if available_travel > 0 && max_offset > 0 {
                                        let click_rel = (my.saturating_sub(track.1 + thumb.3 / 2)) as f32;
                                        let ratio = (click_rel / available_travel as f32).clamp(0.0, 1.0);
                                        viewer.scroll_offset = ((ratio * max_offset as f32).round() as usize).min(max_offset);
                                        if let Some(w) = &self.window {
                                            w.request_redraw();
                                        }
                                    }
                                }
                                return;
                            }
                        }

                        // Clic dentro del contenido de texto para posicionar el cursor
                        let content_y = top_offset + 38;
                        if my >= content_y && !viewer.is_binary {
                            let clicked_row = (my - content_y) / row_h.max(16);
                            let target_line = viewer.scroll_offset + clicked_row;
                            if target_line < viewer.lines.len() {
                                viewer.cursor_line = target_line;
                                let total_lines = viewer.lines.len();
                                let digits = format!("{}", total_lines).len().max(3);
                                let gutter_w = digits * char_w.max(7) + 24;
                                let text_start_x = offset + gutter_w + 14;

                                if mx >= text_start_x {
                                    let rel_x = (mx - text_start_x) + viewer.scroll_offset_x * char_w.max(7);
                                    let line_str = &viewer.lines[target_line];
                                    let mut cur_x_acc = 0;
                                    let mut target_col = line_str.chars().count();
                                    for (ci, ch) in line_str.chars().enumerate() {
                                        let ch_adv = if ch == '\t' { char_w.max(7) * 4 } else { char_w.max(7) };
                                        if cur_x_acc + ch_adv / 2 >= rel_x {
                                            target_col = ci;
                                            break;
                                        }
                                        cur_x_acc += ch_adv;
                                    }
                                    viewer.cursor_col = target_col;
                                } else {
                                    viewer.cursor_col = 0;
                                }
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        }
                    }
                }
                return;
            }
        }

        // 2. Clics en paneles de terminal activa
        let bypass = self.modifiers.shift_key();
        let offset = self.sidebar_offset();
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let right_offset = self.file_explorer_offset();
        let term_area_w = win_w.saturating_sub(offset + right_offset);
        let term_area_h = win_h.saturating_sub(top_offset + status_bar_h);
        let active_tab = &mut self.tabs[self.active_tab_index];
        let pane_rects =
            active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);

        let clicked_pane_rect = pane_rects
            .iter()
            .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
            .copied();

        if let Some(rect) = clicked_pane_rect {
            self.file_explorer.is_focused = false;
            // Cambiar foco al panel clickeado si no está activo
            if active_tab.active_pane_id != rect.pane_id {
                active_tab.active_pane_id = rect.pane_id;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                // Clic en scrollbar de este panel
                if let Some(renderer) = &self.renderer {
                    if let Some((_thumb_x, thumb_y, _thumb_w, thumb_h)) = renderer
                        .pane_scrollbar_geometry(
                            rect.x,
                            rect.y,
                            rect.width,
                            rect.height,
                            &pane.terminal,
                        )
                    {
                        let scroll_margin = 16;
                        if mx >= (rect.x + rect.width).saturating_sub(scroll_margin)
                            && button == MouseButton::Left
                            && state == ElementState::Pressed
                        {
                            if my >= thumb_y && my < thumb_y + thumb_h {
                                self.scrollbar_dragging = true;
                                self.scrollbar_drag_pane_id = Some(pane.id);
                                self.scrollbar_drag_offset_y = (my - thumb_y) as f32;
                                self.last_scroll_time = Instant::now();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            } else if my < thumb_y {
                                pane.terminal.scroll_up(pane.terminal.rows);
                                self.last_scroll_time = Instant::now();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            } else {
                                pane.terminal.scroll_down(pane.terminal.rows);
                                self.last_scroll_time = Instant::now();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }
                        }
                    }
                }

                if pane.terminal.is_mouse_reporting_active() && !bypass {
                    if let Some(renderer) = &self.renderer {
                        let term_x = mx.saturating_sub(rect.x);
                        let term_y = my.saturating_sub(rect.y);
                        let col = if term_x >= renderer.margin_x {
                            ((term_x - renderer.margin_x) / renderer.char_width) + 1
                        } else {
                            1
                        }
                        .min(pane.terminal.cols);
                        let row = if term_y >= renderer.margin_y {
                            ((term_y - renderer.margin_y) / renderer.row_height) + 1
                        } else {
                            1
                        }
                        .min(pane.terminal.rows);

                        let base_code = match button {
                            MouseButton::Left => 0,
                            MouseButton::Middle => 1,
                            MouseButton::Right => 2,
                            _ => 0,
                        };

                        let mut code = base_code;
                        if self.modifiers.shift_key() {
                            code += 4;
                        }
                        if self.modifiers.alt_key() {
                            code += 8;
                        }
                        if self.modifiers.control_key() {
                            code += 16;
                        }

                        let is_pressed = state == ElementState::Pressed;
                        self.mouse_down = is_pressed && button == MouseButton::Left;

                        if pane.terminal.mouse_reporting_sgr {
                            let suffix = if is_pressed { 'M' } else { 'm' };
                            let seq = format!("\x1b[<{};{};{}{}", code, col, row, suffix);
                            pane.pty.write_input(seq.as_bytes());
                        } else {
                            let btn_byte = if is_pressed { 32 + code as u8 } else { 32 + 3 };
                            let seq = [
                                0x1b,
                                b'[',
                                b'M',
                                btn_byte,
                                32 + col.min(223) as u8,
                                32 + row.min(223) as u8,
                            ];
                            pane.pty.write_input(&seq);
                        }
                    }
                } else {
                    match button {
                        MouseButton::Left => match state {
                            ElementState::Pressed => {
                                self.mouse_down = true;
                                if let Some(renderer) = &self.renderer {
                                    let term_x = mx.saturating_sub(rect.x);
                                    let term_y = my.saturating_sub(rect.y);
                                    if term_x >= renderer.margin_x && term_y >= renderer.margin_y {
                                        let col =
                                            (term_x - renderer.margin_x) / renderer.char_width;
                                        let row =
                                            (term_y - renderer.margin_y) / renderer.row_height;
                                        self.drag_start = Some((col, row));
                                        pane.terminal.selection = None;
                                        if let Some(w) = &self.window {
                                            w.request_redraw();
                                        }
                                    }
                                }
                            }
                            ElementState::Released => {
                                self.mouse_down = false;
                                self.drag_start = None;
                            }
                        },
                        MouseButton::Right | MouseButton::Middle
                            if state == ElementState::Pressed =>
                        {
                            self.paste_from_clipboard();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
