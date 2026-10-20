//! Clics: pestañas, paneles, explorador, ajustes, selección y TUIs.

use super::*;

impl AxonApp {
    /// Botones del ratón: pestañas, paneles, explorador, ajustes, selección y TUIs.
    pub(super) fn on_mouse_input(&mut self, state: ElementState, button: MouseButton) {
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

        // Diálogo modal abierto: es lo único que atiende el ratón
        if self.dialog_click(mx, my, (win_w, win_h), state, button) {
            return;
        }

        // Menú contextual abierto: un clic sobre una opción la ejecuta; cualquier otro clic lo cierra
        if let Some(menu) = &self.context_menu {
            if state != ElementState::Pressed {
                return;
            }
            let action = menu
                .item_at(mx, my)
                .and_then(|i| menu.items.get(i))
                .filter(|item| item.enabled)
                .and_then(|item| item.action.clone());
            self.context_menu = None;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            if button == MouseButton::Left {
                if let Some(action) = action {
                    self.run_menu_action(action);
                }
                return;
            }
            if button != MouseButton::Right {
                return;
            }
        }
        if button == MouseButton::Right
            && state == ElementState::Pressed
            && self.open_context_menu(mx, my)
        {
            return;
        }

        // Soltar un divisor o el borde del explorador: tamaño final de los PTY y disposición a la sesión
        // (`|` y no `||`: hay que soltar ambos estados)
        if button == MouseButton::Left
            && state == ElementState::Released
            && (self.divider_drag.take().is_some() | std::mem::take(&mut self.explorer_resizing))
        {
            self.resize_deadline = None;
            self.recompute_grid_for_all_tabs();
            self.save_session();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return;
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_idx) = self.tab_drag.take() {
                let torn_off = if self.is_horizontal_tabs() {
                    my >= TopTabBarRects::HEIGHT + 15 || my >= win_h || mx >= win_w
                } else {
                    let x0 = self.panel_x(win_w);
                    mx + 15 < x0 || mx >= x0 + SIDEBAR_WIDTH + 15 || my >= win_h || mx >= win_w
                };
                if torn_off && drag_idx < self.tabs.len() && !self.tabs[drag_idx].is_settings {
                    // Si hay más de una pestaña, o hay otras ventanas abiertas a donde acoplar
                    if self.tabs.len() > 1 || self.has_other_windows {
                        let screen_pos = crate::app::platform::get_screen_cursor_pos()
                            .or_else(|| {
                                self.window.as_ref().and_then(|w| {
                                    w.outer_position()
                                        .ok()
                                        .map(|p| (p.x + mx as i32, p.y + my as i32))
                                })
                            })
                            .unwrap_or((mx as i32, my as i32));

                        if let Some(viewer) = self.tabs[drag_idx].file_viewer.as_mut() {
                            if viewer.needs_autosave {
                                viewer.autosave_local();
                            }
                        }

                        let tab = self.tabs.remove(drag_idx);
                        self.prune_pending_remote();

                        if self.active_tab_index >= self.tabs.len() {
                            self.active_tab_index = self.tabs.len().saturating_sub(1);
                        }
                        if self.last_terminal_tab_index >= self.tabs.len() {
                            self.last_terminal_tab_index = self.tabs.len().saturating_sub(1);
                        }

                        if !self.tabs.is_empty() {
                            self.tabs[self.active_tab_index].has_unread = false;
                            self.sync_file_explorer_root();
                            self.clamp_sidebar_scroll();
                            self.update_window_title();
                            self.save_session();
                        } else {
                            // Se transfirió la última pestaña: la ventana se cierra
                            self.closing = true;
                        }

                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }

                        self.tab_drop_action = Some(TabDropAction { tab, screen_pos });
                    }
                }
            }
        }

        // Si soltamos el botón izquierdo, cancelar arrastre de la barra de desplazamiento y selección
        if button == MouseButton::Left && state == ElementState::Released {
            self.viewer_drag_selecting = false;
            let was_dragging = self.scrollbar_dragging
                || self.sidebar_scrollbar_dragging
                || self.explorer_scrollbar_dragging
                || self.viewer_scrollbar_dragging
                || self.viewer_hscrollbar_dragging
                || self.settings_scrollbar_dragging;
            if was_dragging {
                self.scrollbar_dragging = false;
                self.scrollbar_drag_pane_id = None;
                self.sidebar_scrollbar_dragging = false;
                self.explorer_scrollbar_dragging = false;
                self.viewer_scrollbar_dragging = false;
                self.viewer_hscrollbar_dragging = false;
                self.settings_scrollbar_dragging = false;
                self.last_scroll_time = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        }

        // Botones laterales del ratón (Atrás / Adelante en explorador o navegación de pestañas)
        if state == ElementState::Pressed {
            match button {
                MouseButton::Back => {
                    if self.file_explorer.visible && self.file_explorer.can_go_back() {
                        self.file_explorer.go_back();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                    if self.tabs.len() > 1 {
                        if self.active_tab_index == 0 {
                            self.switch_to_tab(self.tabs.len() - 1);
                        } else {
                            self.switch_to_tab(self.active_tab_index - 1);
                        }
                        return;
                    }
                }
                MouseButton::Forward => {
                    if self.file_explorer.visible && self.file_explorer.can_go_forward() {
                        self.file_explorer.go_forward();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                    if self.tabs.len() > 1 {
                        self.switch_to_tab((self.active_tab_index + 1) % self.tabs.len());
                        return;
                    }
                }
                _ => {}
            }
        }

        // Redimensionamiento de ventana al arrastrar bordes (solo si no está maximizada ni en pantalla completa).
        let can_resize = self
            .window
            .as_ref()
            .is_some_and(|w| !w.is_maximized() && w.fullscreen().is_none());
        if can_resize && button == MouseButton::Left && state == ElementState::Pressed {
            if let Some(dir) = self.compute_border_resize_direction(mx, my, win_w, win_h) {
                if let Some(w) = &self.window {
                    let _ = w.drag_resize_window(dir);
                    return;
                }
            }
        }

        // Si estamos en modo de pestañas horizontales superiores
        if self.is_horizontal_tabs() && my < TopTabBarRects::HEIGHT {
            if state == ElementState::Pressed
                && (button == MouseButton::Left || button == MouseButton::Middle)
            {
                let list = self.panel_tab_indices();
                let top_rects = TopTabBarRects::compute(win_w, list.len());

                // Clic en el selector de proyecto: pasa al siguiente
                if TopTabBarRects::inside(mx, my, top_rects.switcher_btn) {
                    self.cycle_project();
                    return;
                }

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
                    self.request_close();
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

                // Clic en [🔍] Buscar: la paleta (WP5) aún no existe; no arrastra la ventana.
                if TopTabBarRects::inside(mx, my, top_rects.search_btn) {
                    return;
                }

                // Clic en pestañas o en botón cerrar 'x'
                for (pos, &idx) in list.iter().enumerate() {
                    let tr = top_rects.tab_rect(pos);
                    if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                        if button == MouseButton::Middle {
                            crate::logger::log(&format!(
                                "-> Clic botón central (rueda) en pestaña {}",
                                idx
                            ));
                            self.close_tab(idx);
                            return;
                        }
                        if button == MouseButton::Left {
                            let cr = top_rects.close_btn_rect(pos);
                            if mx >= cr.0 && mx < cr.0 + cr.2 && my >= cr.1 && my < cr.1 + cr.3 {
                                crate::logger::log(&format!("-> Clic en [x] pestaña {}", idx));
                                self.close_tab(idx);
                                return;
                            }
                            crate::logger::log(&format!("-> Clic en pestaña {}", idx));
                            self.click_tab(idx);
                            self.tab_drag = Some(idx);
                            return;
                        }
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
                    self.request_close();
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

                // Píldora de búsqueda: la paleta (WP5) aún no existe; el clic no arrastra la ventana.
                if TopTabBarRects::inside(mx, my, title_rects.search_box) {
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

        // Modalidad horizontal: panel de proyectos (el explorador alojado dentro atiende su propio ratón)
        if self.is_horizontal_tabs() && self.panel_local_x(mx, my, win_w).is_some() {
            if state == ElementState::Pressed && button == MouseButton::Left {
                let usable_h = win_h.saturating_sub(self.status_bar_height());
                let entries = self.panel_row_count();
                if let Some(i) = crate::renderer::ProjectsPanelRects::row_at(
                    my,
                    usable_h,
                    entries,
                    self.sidebar_scroll_offset,
                ) {
                    self.click_panel_row(i);
                }
            }
            return;
        }

        // Modalidad vertical: comprobación de botones de alternar barra lateral y clics en el sidebar
        if !self.is_horizontal_tabs() {
            let status_bar_h = self.status_bar_height();
            let usable_h = win_h.saturating_sub(status_bar_h);
            let local = self.panel_local_x(mx, my, win_w);
            let list_end = self.panel_list_end(usable_h);
            let toggle_hit = if self.sidebar_visible {
                local.is_some_and(|lx| {
                    TopTabBarRects::inside(
                        lx,
                        my,
                        crate::renderer::SidebarRects::settings_btn_rect(usable_h),
                    )
                })
            } else {
                TopTabBarRects::inside(mx, my, self.floating_toggle_rect(win_w, usable_h))
            };

            if toggle_hit && state == ElementState::Pressed && button == MouseButton::Left {
                crate::logger::log("-> Clic en ajustes del riel / alternar barra lateral");
                if self.sidebar_visible {
                    self.open_or_switch_settings_tab();
                } else {
                    self.toggle_sidebar();
                }
                return;
            }

            // Clics en la barra lateral abierta
            if let (true, Some(lx)) = (self.sidebar_visible, local) {
                // Dentro del panel todas las coordenadas son locales a él
                let mx = lx;
                if state == ElementState::Pressed
                    && (button == MouseButton::Left || button == MouseButton::Middle)
                {
                    // Riel de proyectos: abrir uno o agregar el último cuadrado
                    if mx < crate::renderer::SidebarRects::RAIL_W {
                        let count = self.panel_rail().len() + 1;
                        if let Some(i) = crate::renderer::SidebarRects::rail_at(mx, my, count) {
                            if button == MouseButton::Left {
                                self.click_rail(i);
                            }
                        }
                        return;
                    }

                    // Clic en botón "+"
                    let plus = crate::renderer::SidebarRects::plus_btn();
                    if TopTabBarRects::inside(mx, my, plus) {
                        crate::logger::log("-> Clic en [+] sidebar: nueva terminal");
                        self.create_new_tab();
                        return;
                    }

                    // Clic en la barra de desplazamiento de la barra lateral vertical
                    if let Some((track, thumb)) = TerminalRenderer::sidebar_scrollbar_geometry(
                        SIDEBAR_WIDTH,
                        list_end,
                        self.panel_tab_indices().len(),
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
                                self.last_scroll_time = Instant::now();
                            } else {
                                let visible_items =
                                    crate::renderer::SidebarRects::visible_items(list_end);
                                let max_offset =
                                    self.panel_tab_indices().len().saturating_sub(visible_items);
                                if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                                    track,
                                    thumb,
                                    my.saturating_sub(thumb.3 / 2) as f32,
                                    max_offset,
                                ) {
                                    self.sidebar_scroll_offset = off;
                                    self.last_scroll_time = Instant::now();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                            return;
                        }
                    }

                    // Clic en pestañas o botón cerrar "x" (misma retícula que el dibujo y el hover)
                    if let Some((pos, tab_idx)) = self.panel_tab_at(my, usable_h) {
                        let row = crate::renderer::SidebarRects::tab_rect(
                            pos,
                            self.sidebar_scroll_offset,
                        );
                        if crate::renderer::SidebarRects::inside(mx, my, row) {
                            if button == MouseButton::Middle {
                                crate::logger::log(&format!(
                                    "-> Clic botón central (rueda) en pestaña sidebar {}",
                                    tab_idx
                                ));
                                self.close_tab(tab_idx);
                                return;
                            }
                            if button == MouseButton::Left {
                                let close = crate::renderer::SidebarRects::close_btn_rect(row);
                                if self.tabs.len() > 1
                                    && crate::renderer::SidebarRects::inside(mx, my, close)
                                {
                                    crate::logger::log(&format!(
                                        "-> Clic en [x] pestaña {}",
                                        tab_idx
                                    ));
                                    self.close_tab(tab_idx);
                                } else {
                                    crate::logger::log(&format!("-> Clic en pestaña {}", tab_idx));
                                    self.click_tab(tab_idx);
                                    self.tab_drag = Some(tab_idx);
                                }
                                return;
                            }
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
                let page_w = win_w.saturating_sub(offset + self.panel_right_inset());
                let page_h = win_h.saturating_sub(top_offset);
                let rects = SettingsPageRects::compute(offset, top_offset, page_w, page_h);
                // Barra de scroll de Ajustes: arrastrar el pulgar o saltar al punto pulsado
                if let Some((track, thumb)) = self.settings_scrollbar() {
                    if mx + 6 >= track.0 && my >= track.1 && my < track.1 + track.3 {
                        if my >= thumb.1 && my < thumb.1 + thumb.3 {
                            self.settings_scrollbar_dragging = true;
                            self.settings_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                        } else {
                            self.settings_scroll_to_thumb(my.saturating_sub(thumb.3 / 2));
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
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
                    return;
                }

                // Pestañas de navegación
                if SettingsPageRects::inside(mx, my, rects.nav_appearance) {
                    self.settings_page_state.section = SettingsSection::Appearance;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_terminal) {
                    self.settings_page_state.section = SettingsSection::Terminal;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_shortcuts) {
                    self.settings_page_state.section = SettingsSection::Shortcuts;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                } else if SettingsPageRects::inside(mx, my, rects.nav_about) {
                    self.settings_page_state.section = SettingsSection::About;
                    self.settings_page_state.scroll_y = 0;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
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
                                return;
                            }
                        }
                        let searching = SettingsPageRects::inside(mx, my, rects.theme_search_box);
                        if searching {
                            self.settings_page_state.editing_setting =
                                Some(crate::renderer::EditingSettingField::ThemeSearch);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return;
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
                                return;
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
                        } else if SettingsPageRects::inside(mx, my, rects.panel_side_left_btn) {
                            crate::logger::log("-> Clic en lado del panel: Izquierda");
                            self.set_panel_side("left");
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.panel_side_right_btn) {
                            crate::logger::log("-> Clic en lado del panel: Derecha");
                            self.set_panel_side("right");
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
                        } else if SettingsPageRects::inside(mx, my, rects.shell_integration_btn) {
                            crate::logger::log("-> Clic en integración de shell");
                            self.toggle_shell_integration();
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
                        } else if SettingsPageRects::inside(mx, my, rects.auto_save_btn) {
                            crate::logger::log("-> Clic en alternar auto-guardar archivos");
                            self.toggle_auto_save();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.word_wrap_btn) {
                            self.toggle_word_wrap();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ssh_integration_btn) {
                            self.toggle_ssh_integration();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.ligatures_btn) {
                            self.toggle_ligatures();
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
                        if SettingsPageRects::inside(mx, my, rects.about_open_file_btn) {
                            crate::logger::log("-> Clic en abrir archivo axon.toml (About)");
                            self.open_config_file();
                            return;
                        } else if SettingsPageRects::inside(mx, my, rects.about_reset_btn) {
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
            if state == ElementState::Pressed
                && button == MouseButton::Left
                && self
                    .file_explorer
                    .on_resize_edge(mx, my, win_w, win_h, top_offset, status_bar_h)
            {
                self.explorer_resizing = true;
                return;
            }
            let (panel_x, panel_y, panel_w, panel_h) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            if mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    // Pestañas «Archivos» · «Cambios»
                    let tabs = crate::renderer::ExplorerTabsRects::compute(
                        panel_x,
                        panel_y,
                        panel_w,
                        crate::explorer::FileExplorer::HEADER_HEIGHT,
                    );
                    if let Some(changes) = tabs.tab_at(mx, my) {
                        self.file_explorer.is_focused = true;
                        if changes != self.file_explorer.changes_only {
                            self.file_explorer.toggle_changes_only();
                        }
                        self.last_explorer_click_idx = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }

                    // Clic en la barra de desplazamiento del explorador de archivos
                    if let Some((track, thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                        win_w,
                        win_h,
                        top_offset,
                        status_bar_h,
                        &self.file_explorer,
                    ) {
                        if TerminalRenderer::explorer_scrollbar_hit(
                            track,
                            panel_x + panel_w,
                            mx,
                            my,
                        ) {
                            if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                self.explorer_scrollbar_dragging = true;
                                self.explorer_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                                self.last_scroll_time = Instant::now();
                            } else {
                                let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                                let list_start_y = panel_y
                                    + crate::explorer::FileExplorer::HEADER_HEIGHT
                                    + crate::explorer::FileExplorer::SUBHEADER_HEIGHT
                                    + crate::explorer::FileExplorer::LIST_TOP_PAD;
                                let list_h = (panel_y + panel_h).saturating_sub(list_start_y);
                                let visible_count = list_h.checked_div(item_h).unwrap_or(1);
                                let max_offset =
                                    self.file_explorer.items.len().saturating_sub(visible_count);
                                if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                                    track,
                                    thumb,
                                    my.saturating_sub(thumb.3 / 2) as f32,
                                    max_offset,
                                ) {
                                    self.file_explorer.scroll_offset = off;
                                    self.last_scroll_time = Instant::now();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                            return;
                        }
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
                                    self.last_explorer_click_idx = None;
                                    if self.file_explorer.inband_remote {
                                        self.remote_toggle_dir(idx);
                                    } else {
                                        self.file_explorer.toggle_expand(idx);
                                    }
                                } else {
                                    // Con solo un clic se abre y visualiza el archivo inmediatamente
                                    crate::logger::log(&format!(
                                        "-> Abrir visor de archivos: {}",
                                        item.name
                                    ));
                                    // Un clic = vista previa; doble clic sobre el mismo archivo = pestaña fija
                                    let now = Instant::now();
                                    let double = self.last_explorer_click_idx == Some(idx)
                                        && now
                                            .duration_since(self.last_explorer_click_time)
                                            .as_millis()
                                            < 400;
                                    self.last_explorer_click_idx = Some(idx);
                                    self.last_explorer_click_time = now;
                                    self.open_file_in_viewer(&item.path, double);
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

        // Clics sobre el menú flotante de consumo de IA en la barra de estado: no deben filtrarse al visor ni terminal
        if self.hovered_ai_badge
            && self.config.appearance.show_status_bar
            && !self.tabs[self.active_tab_index].is_settings
        {
            let popup_h = TerminalRenderer::ai_usage_popup_height(&self.ai_usage.snapshot);
            let popup_rect = StatusBarRects::popup_rect(win_w, win_h, popup_h);
            if StatusBarRects::inside(mx, my, popup_rect) {
                return;
            }
        }

        // Clic sobre el badge de actualización disponible en la barra de estado
        if self.hovered_update_badge
            && self.config.appearance.show_status_bar
            && !self.tabs[self.active_tab_index].is_settings
            && state == ElementState::Pressed
            && button == MouseButton::Left
        {
            crate::updater::trigger_update();
            return;
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

            if mx >= offset
                && mx < offset + viewer_w
                && my >= top_offset
                && my < top_offset + viewer_h
            {
                self.file_explorer.is_focused = false;
                if state == ElementState::Pressed {
                    if button == MouseButton::Middle {
                        // Clic central: pegar contenido del portapapeles
                        self.paste_from_clipboard();
                        return;
                    }
                    if button == MouseButton::Left {
                        if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                            let ext = viewer
                                .path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let is_markdown = ext == "md" || ext == "markdown";
                            let header_rects = crate::renderer::FileViewerHeaderRects::compute(
                                offset,
                                top_offset,
                                viewer_w,
                                viewer_h,
                                is_markdown,
                                viewer.git_base.is_some(),
                            );

                            if let Some(rect) = header_rects.diff_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, rect) {
                                    viewer.toggle_diff();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }

                            if crate::renderer::FileViewerHeaderRects::inside(
                                mx,
                                my,
                                header_rects.mode_btn,
                            ) {
                                viewer.toggle_read_only();
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }

                            if let Some(md_rect) = header_rects.md_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, md_rect) {
                                    viewer.render_markdown = !viewer.render_markdown;
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }

                            let lay = crate::renderer::FileViewerLayout::compute(
                                offset,
                                top_offset,
                                viewer_w,
                                viewer_h,
                                row_h,
                                char_w,
                                viewer.lines.len(),
                            );

                            // Barra de desplazamiento horizontal
                            let max_x = viewer.max_scroll_x();
                            if let Some((track, thumb)) =
                                lay.hscrollbar(max_x, viewer.scroll_offset_x)
                            {
                                if mx >= track.0
                                    && mx < track.0 + track.2
                                    && my + 4 >= track.1
                                    && my < track.1 + track.3 + 4
                                {
                                    if mx >= thumb.0 && mx < thumb.0 + thumb.2 {
                                        self.viewer_hscrollbar_dragging = true;
                                        self.viewer_hscrollbar_drag_offset_x =
                                            (mx - thumb.0) as f32;
                                    } else {
                                        let rel = mx.saturating_sub(track.0 + thumb.2 / 2);
                                        if let Some(x) = (rel * max_x)
                                            .checked_div(track.2.saturating_sub(thumb.2))
                                        {
                                            viewer.scroll_offset_x = x.min(max_x);
                                        }
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }

                            // Comprobar clic en la barra de desplazamiento del visor
                            if let Some((track, thumb)) =
                                TerminalRenderer::file_viewer_scrollbar_geometry(
                                    offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                                )
                            {
                                let hit_x = track.0.saturating_sub(4);
                                if mx >= hit_x
                                    && mx < offset + viewer_w
                                    && my >= track.1
                                    && my < track.1 + track.3
                                {
                                    if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                        self.viewer_scrollbar_dragging = true;
                                        self.viewer_scrollbar_drag_offset_y = (my - thumb.1) as f32;
                                        self.last_scroll_time = Instant::now();
                                    } else {
                                        let visible_lines = lay.content_h / lay.row_h;
                                        let max_offset =
                                            viewer.rows().len().saturating_sub(visible_lines);
                                        if let Some(off) =
                                            crate::renderer::scroll_offset_for_thumb_y(
                                                track,
                                                thumb,
                                                my.saturating_sub(thumb.3 / 2) as f32,
                                                max_offset,
                                            )
                                        {
                                            viewer.scroll_offset = off;
                                            self.last_scroll_time = Instant::now();
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                        }
                                    }
                                    return;
                                }
                            }

                            // Clic dentro del contenido de texto para posicionar el cursor y seleccionar
                            let content_y = top_offset + 38;
                            if my >= content_y && !viewer.is_binary && !viewer.lines.is_empty() {
                                let clicked_row =
                                    viewer.scroll_offset + (my - content_y) / row_h.max(16);

                                // Clic en el margen: pliega o despliega la región de esa línea
                                if mx < lay.sep_x {
                                    if let Some(row) = viewer.rows().get(clicked_row).copied() {
                                        if row.start == 0
                                            && (viewer.folded.contains(&row.line)
                                                || viewer.fold_end(row.line).is_some())
                                        {
                                            viewer.toggle_fold(row.line);
                                            if let Some(w) = &self.window {
                                                w.request_redraw();
                                            }
                                            return;
                                        }
                                    }
                                }
                                let Some((target_line, target_col)) =
                                    viewer.hit_test(clicked_row, mx, &lay)
                                else {
                                    return;
                                };

                                // Si es enlace externo (en modo Markdown preview o con Ctrl pulsado): abrir en navegador
                                if (viewer.render_markdown || self.modifiers.control_key())
                                    && !self.modifiers.shift_key()
                                {
                                    if let Some(url) = viewer.link_at(target_line, target_col) {
                                        crate::app::platform::shell_open_url(&url);
                                        return;
                                    }
                                }

                                if self.modifiers.shift_key() {
                                    let start = viewer
                                        .selection
                                        .map(|s| s.0)
                                        .unwrap_or((viewer.cursor_line, viewer.cursor_col));
                                    viewer.cursor_line = target_line;
                                    viewer.cursor_col = target_col;
                                    viewer.selection = Some((start, (target_line, target_col)));
                                } else {
                                    let now = Instant::now();
                                    let is_quick_click =
                                        now.duration_since(self.last_viewer_click_time).as_millis()
                                            < 400
                                            && self.last_viewer_click_pos.is_some_and(|(l, c)| {
                                                l == target_line
                                                    && (c as isize - target_col as isize).abs() <= 1
                                            });

                                    viewer.cursor_line = target_line;
                                    viewer.cursor_col = target_col;

                                    if is_quick_click && self.viewer_click_count == 1 {
                                        viewer.select_word_at_cursor();
                                        self.viewer_click_count = 2;
                                    } else if is_quick_click && self.viewer_click_count == 2 {
                                        viewer.select_line_at_cursor();
                                        self.viewer_click_count = 3;
                                    } else {
                                        viewer.clear_selection();
                                        self.viewer_click_count = 1;
                                        self.viewer_drag_selecting = true;
                                        self.viewer_drag_start = Some((target_line, target_col));
                                    }
                                    self.last_viewer_click_time = now;
                                    self.last_viewer_click_pos = Some((target_line, target_col));
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

        // 2. Divisor entre paneles: agarrarlo para redimensionar
        if button == MouseButton::Left && state == ElementState::Pressed {
            if let Some(divider) = self.divider_at(mx, my) {
                self.divider_drag = Some(divider);
                return;
            }
        }

        // 2b. Cabecera de un panel dividido: foco y botones dividir · maximizar · cerrar
        if button == MouseButton::Left && state == ElementState::Pressed {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let right_offset = self.file_explorer_offset();
            let term_area_w = win_w.saturating_sub(offset + right_offset);
            let term_area_h = win_h.saturating_sub(top_offset + self.status_bar_height());
            let tab = &self.tabs[self.active_tab_index];
            let hit = if tab.is_settings || tab.file_viewer.is_some() {
                None
            } else {
                tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h)
                    .iter()
                    .find_map(|rect| {
                        if !rect.has_header {
                            return None;
                        }
                        let header = PaneHeaderRects::compute(rect.x, rect.slot_y, rect.width);
                        if !PaneHeaderRects::inside(mx, my, header.header) {
                            return None;
                        }
                        let focused = tab.active_pane_id == rect.pane_id;
                        Some((rect.pane_id, focused, header.button_at(mx, my, focused)))
                    })
            };
            if let Some((pane_id, focused, hit)) = hit {
                if !focused {
                    self.tabs[self.active_tab_index].active_pane_id = pane_id;
                }
                match hit {
                    Some(PaneHeaderButton::Split) => {
                        self.split_active_pane(SplitDirection::Vertical);
                    }
                    Some(PaneHeaderButton::Zoom) => {
                        if self.tabs[self.active_tab_index].toggle_pane_zoom(pane_id) {
                            self.recompute_grid_for_all_tabs();
                            self.save_session();
                        }
                    }
                    Some(PaneHeaderButton::Close) => self.close_pane_by_id(pane_id),
                    None => {}
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        }

        // 3. Clics en paneles de terminal activa
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
            // El compositor solo se dibuja en el panel enfocado: el primer clic solo enfoca.
            let was_active = active_tab.active_pane_id == rect.pane_id;
            // Cambiar foco al panel clickeado si no está activo
            if active_tab.active_pane_id != rect.pane_id {
                active_tab.active_pane_id = rect.pane_id;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                // Clic en botón flotante "Ir hasta abajo"
                if pane.terminal.scroll_offset > 0 && !pane.terminal.is_alt_screen {
                    let (bx, by, bw, bh) = crate::renderer::pane_scroll_to_bottom_btn_rect(
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                    );
                    if mx >= bx
                        && mx < bx + bw
                        && my >= by
                        && my < by + bh
                        && button == MouseButton::Left
                        && state == ElementState::Pressed
                    {
                        pane.terminal.reset_scroll();
                        self.last_scroll_time = Instant::now();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }

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

                // Botón «enviar» del compositor: equivale a pulsar Enter en el prompt.
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && was_active
                    && self.renderer.as_ref().is_some_and(|renderer| {
                        renderer
                            .composer_layout(
                                rect.x,
                                rect.y,
                                rect.width,
                                rect.height,
                                &pane.terminal,
                            )
                            .is_some_and(|c| c.send_at(mx, my))
                    })
                {
                    pane.pty.write_input(b"\r");
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Tarjetas de bloque (OSC 133): copiar el comando, repetirlo o atender la tarjeta de error.
                if button == MouseButton::Left && state == ElementState::Pressed {
                    let hit = self.renderer.as_ref().and_then(|renderer| {
                        let cards = renderer.block_cards(
                            rect.x,
                            rect.y,
                            rect.width,
                            rect.height,
                            &pane.terminal,
                        );
                        cards.iter().find_map(|card| {
                            card.button_at(mx, my)
                                .map(|button| (card.block_index, button))
                        })
                    });
                    if let Some((block_index, action)) = hit {
                        let terminal = &mut pane.terminal;
                        // El índice de la tarjeta en curso cae justo después de los bloques cerrados.
                        let block = if block_index < terminal.command_blocks.len() {
                            terminal.command_blocks.get_mut(block_index)
                        } else {
                            terminal.active_block.as_mut()
                        };
                        // El texto del comando sale de la pantalla (OSC 133), que controla el programa en ejecución:
                        // no es confiable. Una sola línea, sin caracteres de control ni de formato.
                        let command = block
                            .as_ref()
                            .and_then(|b| b.command.as_deref())
                            .map(super::clipboard::clean_block_command);
                        let exit_code = block.as_ref().and_then(|b| b.exit_code);
                        match action {
                            crate::renderer::BlockCardButton::Copy => {
                                if let (Some(command), Some(cb)) = (command, &mut self.clipboard) {
                                    let _ = cb.set_text(command);
                                }
                            }
                            crate::renderer::BlockCardButton::Repeat => {
                                // Solo con el panel en el prompt (si corre otro comando, el texto iría a su entrada) y
                                // sin `\r`: el comando queda escrito en el prompt y lo ejecuta el usuario con Enter.
                                let running = terminal
                                    .active_block
                                    .as_ref()
                                    .is_some_and(|b| b.output_start.is_some());
                                if let (Some(command), false) = (command, running) {
                                    pane.pty.write_input(command.as_bytes());
                                }
                            }
                            // Sin agente integrado, «Explicar» deja en el portapapeles la petición lista para pegar.
                            crate::renderer::BlockCardButton::Explain => {
                                if let (Some(command), Some(cb)) = (command, &mut self.clipboard) {
                                    let code = exit_code.unwrap_or(1);
                                    let _ = cb.set_text(format!(
                                        "El comando `{command}` terminó con código {code}. Explica el error y propón la solución."
                                    ));
                                }
                            }
                            crate::renderer::BlockCardButton::Ignore => {
                                if let Some(block) = block {
                                    block.error_dismissed = true;
                                }
                            }
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }

                // La cabecera de una tarjeta no inicia una selección de texto.
                if button == MouseButton::Left && state == ElementState::Pressed {
                    let on_header = self.renderer.as_ref().is_some_and(|renderer| {
                        renderer
                            .block_cards(rect.x, rect.y, rect.width, rect.height, &pane.terminal)
                            .iter()
                            .any(|card| card.header_at(mx, my))
                    });
                    if on_header {
                        return;
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
                                    let col = term_x.saturating_sub(renderer.margin_x)
                                        / renderer.char_width;
                                    let col = col.min(pane.terminal.cols.saturating_sub(1));
                                    let row_in_view = term_y.saturating_sub(renderer.margin_y)
                                        / renderer.row_height;
                                    let row_in_view =
                                        row_in_view.min(pane.terminal.rows.saturating_sub(1));
                                    let global_line =
                                        pane.terminal.visible_line_start() + row_in_view;

                                    // 1. Ctrl+Click en enlace externo: abrir en el navegador
                                    if self.modifiers.control_key() {
                                        if let Some(url) = pane.terminal.url_at(col, global_line) {
                                            crate::app::platform::shell_open_url(&url);
                                            self.drag_start = None;
                                            return;
                                        }
                                    }

                                    // 2. Doble y triple clic para palabra y línea
                                    let now = Instant::now();
                                    let is_quick_click = self.last_terminal_click_pane_id
                                        == Some(pane.id)
                                        && now
                                            .duration_since(self.last_terminal_click_time)
                                            .as_millis()
                                            < 400
                                        && self.last_terminal_click_pos.is_some_and(|(c, l)| {
                                            l == global_line
                                                && (c as isize - col as isize).abs() <= 1
                                        });

                                    if is_quick_click && self.terminal_click_count == 1 {
                                        pane.terminal.select_word_at(col, global_line);
                                        self.terminal_click_count = 2;
                                        self.drag_start = None;
                                        self.drag_anchor = pane.terminal.selection;
                                        if self.config.terminal.copy_on_select {
                                            if let Some(text) = pane.terminal.copy_selection_text()
                                            {
                                                if let Some(cb) = &mut self.clipboard {
                                                    let _ = cb.set_text(text);
                                                }
                                            }
                                        }
                                    } else if is_quick_click && self.terminal_click_count == 2 {
                                        pane.terminal.select_line_at(global_line);
                                        self.terminal_click_count = 3;
                                        self.drag_start = None;
                                        self.drag_anchor = pane.terminal.selection;
                                        if self.config.terminal.copy_on_select {
                                            if let Some(text) = pane.terminal.copy_selection_text()
                                            {
                                                if let Some(cb) = &mut self.clipboard {
                                                    let _ = cb.set_text(text);
                                                }
                                            }
                                        }
                                    } else {
                                        pane.terminal.selection = None;
                                        self.terminal_click_count = 1;
                                        self.drag_anchor = None;
                                        self.drag_start = Some((col, global_line));
                                    }
                                    self.last_terminal_click_time = now;
                                    self.last_terminal_click_pos = Some((col, global_line));
                                    self.last_terminal_click_pane_id = Some(pane.id);

                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                            ElementState::Released => {
                                self.mouse_down = false;
                                self.drag_start = None;
                                self.drag_anchor = None;
                                if pane.terminal.selection.is_some_and(|s| s.start == s.end) {
                                    pane.terminal.selection = None;
                                } else if self.config.terminal.copy_on_select
                                    && pane.terminal.selection.is_some()
                                {
                                    if let Some(text) = pane.terminal.copy_selection_text() {
                                        if let Some(cb) = &mut self.clipboard {
                                            let _ = cb.set_text(text);
                                        }
                                    }
                                }
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        },
                        MouseButton::Middle if state == ElementState::Pressed => {
                            self.paste_from_clipboard();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Determina la dirección de redimensionamiento de ventana al interactuar con los bordes.
    /// El borde norte es más fino que los demás para no restarle superficie a la barra superior
    /// (arrastre de ventana, pestañas y botones de control).
    pub(crate) fn compute_border_resize_direction(
        &self,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> Option<winit::window::ResizeDirection> {
        use winit::window::ResizeDirection::*;
        let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
        let margin = (6.0 * scale).round() as usize;
        let top_margin = (4.0 * scale).round() as usize;
        let at_left = mx < margin;
        let at_right = mx >= win_w.saturating_sub(margin);
        let at_top = my < top_margin;
        let at_bottom = my >= win_h.saturating_sub(margin);

        match (at_top, at_bottom, at_left, at_right) {
            (true, _, true, _) => Some(NorthWest),
            (true, _, _, true) => Some(NorthEast),
            (_, true, true, _) => Some(SouthWest),
            (_, true, _, true) => Some(SouthEast),
            (_, _, true, _) => Some(West),
            (_, _, _, true) => Some(East),
            (true, ..) => Some(North),
            (_, true, ..) => Some(South),
            _ => None,
        }
    }
}
