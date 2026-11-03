//! Clics: pestañas, paneles, explorador, ajustes, selección y TUIs.

use super::*;

impl TearminalApp {
    /// Botones del ratón: pestañas, paneles, explorador, ajustes, selección y TUIs.
    pub(super) fn on_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        let mx = self.mouse_pos.x as usize;
        let my = self.mouse_pos.y as usize;
        let (win_w, win_h) = self.window_size();
        let is_settings_active = self.tabs[self.active_tab_index].is_settings;

        if self.click_overlays(state, button, mx, my, win_w, win_h, is_settings_active) {
            return;
        }

        if self.click_drags_and_releases(state, button, mx, my, win_w, win_h) {
            return;
        }

        if self.click_window_nav_and_borders(
            state,
            button,
            mx,
            my,
            win_w,
            win_h,
            is_settings_active,
        ) {
            return;
        }

        if self.click_spaces_panel_area(state, button, mx, my, win_h) {
            return;
        }

        if self.click_top_bar_area(state, button, mx, my, win_w, win_h) {
            return;
        }

        if self.click_settings_area(state, button, mx, my, is_settings_active) {
            return;
        }

        if self.click_explorer_area(state, button, mx, my, win_w, win_h, is_settings_active) {
            return;
        }

        if self.click_file_viewer_area(state, button, mx, my, win_w, win_h) {
            return;
        }

        self.click_terminal_panes_area(state, button, mx, my, win_w, win_h);
    }

    #[allow(clippy::too_many_arguments)]
    fn click_overlays(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
        is_settings_active: bool,
    ) -> bool {
        // Modal «Añadir máquina»: consume todos los clics
        if self.machine_modal.is_some() {
            if state == ElementState::Pressed && button == MouseButton::Left {
                self.click_machine_modal(mx, my);
            } else if state == ElementState::Released {
                self.handle_all_text_inputs_mouse_up();
            }
            return true;
        }

        // Menú contextual abierto: un clic sobre una opción la ejecuta; cualquier otro clic lo cierra
        if let Some(menu) = &self.context_menu {
            if state != ElementState::Pressed {
                return true;
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
                return true;
            }
            if button != MouseButton::Right {
                return true;
            }
        }

        // Paleta de comandos abierta: un clic en input posiciona o selecciona; clic en scrollbar desplaza; clic en un elemento lo ejecuta; cualquier clic fuera la cierra
        if let Some(palette) = &mut self.palette {
            let rects = PaletteRects::compute_with_scroll(
                win_w,
                win_h,
                palette.filtered.len(),
                palette.scroll_offset,
            );
            if button == MouseButton::Left {
                if state == ElementState::Pressed {
                    // Clic en la barra de scroll de la paleta: arrastrar el pulgar o saltar al punto pulsado
                    if let Some((track, thumb)) = rects.scrollbar {
                        let right = rects.card.0 + rects.card.2;
                        if crate::renderer::ScrollbarRects::hit(track, right, mx, my) {
                            if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                self.scrollbar_drag = Some(ScrollbarDragState {
                                    target: ScrollbarTarget::Palette,
                                    grab_offset: (my - thumb.1) as f32,
                                });
                            } else {
                                self.palette_scroll_to_thumb(my.saturating_sub(thumb.3 / 2));
                            }
                            self.last_scroll_time = Instant::now();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                    }

                    if PaletteRects::inside(mx, my, rects.input) {
                        let text_start_x = rects.input.0 + 40;
                        let mut input = std::mem::take(&mut palette.input);
                        self.handle_text_input_click(&mut input, mx, text_start_x);
                        if let Some(p) = &mut self.palette {
                            p.input = input;
                        }
                        return true;
                    }
                    if PaletteRects::inside(mx, my, rects.card) {
                        if let Some(idx) = rects.item_at_with_offset(mx, my, palette.scroll_offset)
                        {
                            if let Some(item) = palette.filtered.get(idx) {
                                let action = item.action.clone();
                                self.execute_palette_action(action);
                                return true;
                            }
                        }
                        return true;
                    }
                    self.palette = None;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                } else if state == ElementState::Released {
                    palette.input.mouse_up();
                    if self.scrollbar_drag.take().is_some() {
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    return true;
                }
            }
            return true;
        }
        // Selector de pantalla encendida abierto: un clic sobre una opción la aplica; cualquier otro clic lo cierra
        // (el clic sobre su propio botón lo alterna, así que ese no lo reabre)
        if self.keep_awake_popup_open && !is_settings_active && state == ElementState::Pressed {
            let option = StatusBarRects::keep_awake_option_at(win_w, win_h, mx, my);
            self.keep_awake_popup_open = false;
            self.hovered_keep_awake_option = None;
            self.request_redraw();
            if button == MouseButton::Left {
                if let Some(i) = option {
                    let mode = [
                        KeepAwakeMode::Off,
                        KeepAwakeMode::Agent,
                        KeepAwakeMode::Active,
                    ][i];
                    self.set_keep_awake_mode(mode);
                }
                return true;
            }
        }

        false
    }

    fn click_drags_and_releases(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        if button == MouseButton::Right
            && state == ElementState::Pressed
            && self.open_context_menu(mx, my)
        {
            return true;
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
            return true;
        }
        if button == MouseButton::Left && state == ElementState::Released {
            self.handle_all_text_inputs_mouse_up();
            if let Some(drag) = self.explorer_drag.take() {
                let panel_x = win_w.saturating_sub(self.panel_width());
                if drag.is_dragging {
                    self.set_cursor_icon(winit::window::CursorIcon::Default);
                }
                if drag.is_dragging && mx < panel_x {
                    // Se soltó sobre el área de contenido / terminal
                    self.on_file_dropped(drag.path);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                } else if !drag.is_dragging || mx >= panel_x {
                    let dist_sq = ((mx as isize - drag.start_pos.0 as isize).pow(2)
                        + (my as isize - drag.start_pos.1 as isize).pow(2))
                        as usize;
                    if dist_sq < 400 && drag.is_dir {
                        if let Some(real_idx) =
                            self.file_explorer.actual_index_for_visible(drag.idx)
                        {
                            self.file_explorer.toggle_expand(real_idx);
                            if self.file_explorer.is_filtering() {
                                self.file_explorer.update_filter();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                    }
                }
            }
            if let Some(drag_idx) = self.tab_drag.take() {
                let torn_off = my >= TopTabBarRects::HEIGHT + 15 || my >= win_h || mx >= win_w;
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

                        if self.active_tab_index >= self.tabs.len() {
                            self.active_tab_index = self.tabs.len().saturating_sub(1);
                        }
                        if self.last_terminal_tab_index >= self.tabs.len() {
                            self.last_terminal_tab_index = self.tabs.len().saturating_sub(1);
                        }

                        if !self.tabs.is_empty() {
                            self.tabs[self.active_tab_index].has_unread = false;
                            self.sync_file_explorer_root();
                            self.clamp_spaces_scroll();
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
            self.handle_all_text_inputs_mouse_up();
            self.viewer_drag_selecting = false;
            self.viewer_drag_start = None;
            let was_mouse_down = std::mem::take(&mut self.mouse_down);
            let had_drag = self.drag_start.take().is_some() | self.drag_anchor.take().is_some();
            if had_drag || was_mouse_down {
                let active_tab = &mut self.tabs[self.active_tab_index];
                if let Some(pane) = active_tab.find_pane_mut(active_tab.active_pane_id) {
                    if pane.terminal.selection.is_some_and(|s| s.start == s.end) {
                        pane.terminal.selection = None;
                    } else if pane.terminal.selection.is_some() {
                        super::clipboard::copy_pane_selection(
                            &mut self.clipboard,
                            self.config.terminal.copy_on_select,
                            pane,
                        );
                    }
                }
            }
            if self.scrollbar_drag.take().is_some() {
                self.last_scroll_time = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return true;
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn click_window_nav_and_borders(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
        is_settings_active: bool,
    ) -> bool {
        // Botones laterales del ratón (Atrás / Adelante en explorador o navegación de pestañas)
        if state == ElementState::Pressed {
            match button {
                MouseButton::Back => {
                    if self.file_explorer.visible && self.file_explorer.can_go_back() {
                        self.file_explorer.go_back();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if self.tabs.len() > 1 {
                        if self.active_tab_index == 0 {
                            self.switch_to_tab(self.tabs.len() - 1);
                        } else {
                            self.switch_to_tab(self.active_tab_index - 1);
                        }
                        return true;
                    }
                }
                MouseButton::Forward => {
                    if self.file_explorer.visible && self.file_explorer.can_go_forward() {
                        self.file_explorer.go_forward();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if self.tabs.len() > 1 {
                        self.switch_to_tab((self.active_tab_index + 1) % self.tabs.len());
                        return true;
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
                    return true;
                }
            }
        }

        // Con Ajustes abierto el velo cubre toda la ventana: un clic fuera del modal lo cierra
        if is_settings_active {
            let (rx, ry, rw, rh) = self.settings_modal();
            let inside = mx >= rx && mx < rx + rw && my >= ry && my < ry + rh;
            if !inside {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    self.close_settings();
                }
                return true;
            }
        }

        false
    }

    fn click_spaces_panel_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_h: usize,
    ) -> bool {
        // Panel de Spaces: ocupa todo el alto a la izquierda de la barra superior
        if self.spaces_rects(win_h).contains(mx, my, win_h) {
            if state == ElementState::Pressed {
                match button {
                    MouseButton::Left => {
                        self.click_spaces_panel(mx, my, win_h);
                    }
                    MouseButton::Middle => self.middle_click_spaces_panel(mx, my, win_h),
                    _ => {}
                }
            }
            return true;
        }

        false
    }

    fn click_top_bar_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        // Barra superior: pestañas del Space activo, «+», Files · Changes y controles de ventana
        if my < TopTabBarRects::HEIGHT {
            if state == ElementState::Pressed
                && (button == MouseButton::Left || button == MouseButton::Middle)
            {
                let visible = self.visible_tab_indices();
                let top_rects = TopTabBarRects::compute(win_w, self.spaces_w(), visible.len());

                // Clic en [-] Minimizar
                if TopTabBarRects::inside(mx, my, top_rects.win_min_btn) {
                    if let Some(w) = &self.window {
                        w.set_minimized(true);
                    }
                    return true;
                }

                // Clic en [□] Maximizar / Restaurar
                if TopTabBarRects::inside(mx, my, top_rects.win_max_btn) {
                    if let Some(w) = &self.window {
                        w.set_maximized(!w.is_maximized());
                    }
                    return true;
                }

                // Clic en [✕] Cerrar ventana
                if TopTabBarRects::inside(mx, my, top_rects.win_close_btn) {
                    self.request_close();
                    return true;
                }

                // Clic en [+] nueva pestaña (en el Space activo)
                if TopTabBarRects::inside(mx, my, top_rects.plus_btn) {
                    if self.modifiers.shift_key() || self.modifiers.alt_key() {
                        crate::logger::log("-> Shift/Alt+Clic en [+] top bar: selector de shell");
                        let items = self.shell_profiles_menu();
                        self.show_menu(
                            items,
                            (
                                top_rects.plus_btn.0,
                                top_rects.plus_btn.1 + top_rects.plus_btn.3 + 4,
                            ),
                            (win_w, win_h),
                        );
                        return true;
                    }
                    crate::logger::log("-> Clic en [+] top bar: nueva terminal");
                    self.create_new_tab();
                    return true;
                }

                // Píldoras Files / Changes
                if TopTabBarRects::inside(mx, my, top_rects.files_btn) {
                    crate::logger::log("-> Clic en Files (top bar)");
                    self.show_explorer_view(false);
                    return true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.changes_btn) {
                    crate::logger::log("-> Clic en Changes (top bar)");
                    self.show_explorer_view(true);
                    return true;
                }

                // Clic en pestañas o en botón cerrar 'x'
                for (pos, &idx) in visible.iter().enumerate() {
                    let tr = top_rects.tab_rect(pos);
                    if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                        if button == MouseButton::Middle {
                            crate::logger::log(&format!(
                                "-> Clic botón central (rueda) en pestaña {idx}"
                            ));
                            self.close_tab(idx);
                            return true;
                        }
                        if button == MouseButton::Left {
                            let cr = top_rects.close_btn_rect(pos);
                            if mx >= cr.0 && mx < cr.0 + cr.2 && my >= cr.1 && my < cr.1 + cr.3 {
                                crate::logger::log(&format!("-> Clic en [x] pestaña {idx}"));
                                self.close_tab(idx);
                                return true;
                            }
                            crate::logger::log(&format!("-> Clic en pestaña {idx}"));
                            self.click_tab(idx);
                            self.tab_drag = Some(idx);
                            return true;
                        }
                    }
                }

                // Arrastrar ventana o doble clic para maximizar
                self.drag_or_maximize_window();
            }
            return true;
        }

        false
    }

    fn click_settings_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        is_settings_active: bool,
    ) -> bool {
        // 2. Clics en la pestaña de Ajustes (si está activa)
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

    #[allow(clippy::too_many_arguments)]
    fn click_explorer_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
        is_settings_active: bool,
    ) -> bool {
        // Redimensionamiento del panel lateral / explorador de archivos (prioridad sobre el margen del panel y el sidebar)
        if state == ElementState::Pressed
            && button == MouseButton::Left
            && self.on_panel_resize_edge(mx, my, win_w, win_h)
        {
            self.explorer_resizing = true;
            return true;
        }

        // Modalidad horizontal: lo que el panel tiene fuera del explorador (su margen) no hace nada;
        // el explorador alojado dentro atiende su propio ratón
        if self.panel_local_x(mx, my, win_w).is_some() {
            return true;
        }

        // Clics dentro del panel del explorador lateral de archivos / barra lateral
        if (self.file_explorer.visible || self.panel_shown()) && !is_settings_active {
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let (panel_x, panel_y, panel_w, panel_h) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            let expanded_hover = self.renderer.as_mut().and_then(|r| {
                r.explorer_hovered_item_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                    &self.file_explorer,
                )
            });
            let in_expanded_item = expanded_hover.is_some_and(|(idx, item_y, item_h, row_w)| {
                self.file_explorer.hovered_index == Some(idx)
                    && my >= item_y
                    && my < item_y + item_h
                    && mx >= panel_x
                    && mx < panel_x + row_w
            });
            let in_panel =
                mx >= panel_x && mx < panel_x + panel_w && my >= panel_y && my < panel_y + panel_h;

            if in_panel || in_expanded_item {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    // Barra de edición de nombre (nuevo/renombrar): un clic dentro coloca el cursor; fuera la cancela
                    if self.file_explorer.edit.is_some() {
                        let bar =
                            crate::explorer::FileExplorer::edit_bar_rect(panel_x, panel_y, panel_w);
                        if TopTabBarRects::inside(mx, my, bar) {
                            if let Some(mut edit) = self.file_explorer.edit.take() {
                                let text_x = bar.0 + crate::explorer::FileExplorer::EDIT_TEXT_DX;
                                self.handle_text_input_click(&mut edit.input, mx, text_x);
                                self.file_explorer.edit = Some(edit);
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        self.file_explorer.cancel_edit();
                    }

                    // Clic en la barra de desplazamiento del explorador de archivos (si no estamos sobre elemento expandido)
                    if !in_expanded_item {
                        if let Some((track, thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                            win_w,
                            win_h,
                            top_offset,
                            status_bar_h,
                            &self.file_explorer,
                        ) {
                            if crate::renderer::ScrollbarRects::hit(
                                track,
                                panel_x + panel_w,
                                mx,
                                my,
                            ) {
                                if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                    self.scrollbar_drag = Some(ScrollbarDragState {
                                        target: ScrollbarTarget::Explorer,
                                        grab_offset: (my - thumb.1) as f32,
                                    });
                                    self.last_scroll_time = Instant::now();
                                } else {
                                    let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                                    let list_start_y = panel_y
                                        + crate::explorer::FileExplorer::HEADER_HEIGHT
                                        + self.file_explorer.subheader_height()
                                        + crate::explorer::FileExplorer::LIST_TOP_PAD;
                                    let list_h = (panel_y + panel_h).saturating_sub(list_start_y);
                                    let visible_count = list_h.checked_div(item_h).unwrap_or(1);
                                    let max_offset = self
                                        .file_explorer
                                        .visible_item_count()
                                        .saturating_sub(visible_count);
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
                                return true;
                            }
                        }
                    }

                    // Al interactuar con el explorador gana el foco de teclado
                    self.file_explorer.is_focused = true;

                    // Clic en la barra de filtro rápido (cerrar o enfocar input)
                    if self.file_explorer.is_filtering() {
                        let (bx, by, bw, bh) = crate::explorer::FileExplorer::filter_bar_rect(
                            panel_x, panel_y, panel_w,
                        );
                        let (cx, cy, cw, ch) = crate::explorer::FileExplorer::filter_close_btn_rect(
                            panel_x, panel_y, panel_w,
                        );
                        if mx >= cx && mx < cx + cw && my >= cy && my < cy + ch {
                            self.file_explorer.close_filter();
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        if mx >= bx && mx < bx + bw && my >= by && my < by + bh {
                            if let Some(mut input) = self.file_explorer.filter.take() {
                                self.handle_text_input_click(&mut input, mx, bx + 26);
                                self.file_explorer.filter = Some(input);
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                    }

                    // Solo procesar clics en elementos si estamos a la izquierda de la barra de desplazamiento
                    // o sobre un elemento expandido bajo el cursor
                    if in_expanded_item || mx < panel_x + panel_w.saturating_sub(16) {
                        let clicked_idx = if in_expanded_item {
                            self.file_explorer.hovered_index
                        } else {
                            self.file_explorer.item_at_y(my, panel_y)
                        };
                        if let Some(idx) = clicked_idx {
                            if idx < self.file_explorer.visible_item_count() {
                                if let Some(item) = self.file_explorer.visible_item(idx).cloned() {
                                    let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                                    if self.file_explorer.changes_only {
                                        if let Some(item_y) =
                                            self.file_explorer.item_y(idx, panel_y)
                                        {
                                            if let Some(action) =
                                                crate::renderer::GitActionBtnRects::hit_test(
                                                    panel_x,
                                                    panel_w,
                                                    item_y,
                                                    item_h,
                                                    item.is_staged,
                                                    item.is_unstaged,
                                                    mx,
                                                    my,
                                                )
                                            {
                                                let is_untracked = item.git_status
                                                    == Some(
                                                        crate::explorer::GitFileStatus::Untracked,
                                                    );
                                                self.execute_git_action(
                                                    &item.path,
                                                    action,
                                                    is_untracked,
                                                );
                                                if let Some(w) = &self.window {
                                                    w.request_redraw();
                                                }
                                                return true;
                                            }
                                        }
                                    }

                                    self.file_explorer.selected_index = Some(idx);
                                    self.file_explorer.selected_path = Some(item.path.clone());
                                    self.explorer_drag = Some(crate::app::ExplorerDragState {
                                        path: item.path.clone(),
                                        start_pos: (mx, my),
                                        is_dir: item.is_dir,
                                        idx,
                                        is_dragging: false,
                                    });

                                    if item.is_dir {
                                        crate::logger::log(&format!(
                                            "-> Clic en carpeta: {}",
                                            item.name
                                        ));
                                        self.last_explorer_click_idx = None;
                                        if let Some(real_idx) =
                                            self.file_explorer.actual_index_for_visible(idx)
                                        {
                                            self.file_explorer.toggle_expand(real_idx);
                                            if self.file_explorer.is_filtering() {
                                                self.file_explorer.update_filter();
                                            }
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
                                            && now.duration_since(self.last_explorer_click_time)
                                                <= platform::double_click_time();
                                        self.last_explorer_click_idx = Some(idx);
                                        self.last_explorer_click_time = now;
                                        self.open_file_in_viewer(&item.path, double);
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                }
                return true;
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
            return true;
        }

        // Clic sobre el botón de modo pantalla encendida (Activo, Agente, Apagado) en la esquina inferior derecha
        if self.hovered_keep_awake
            && self.config.appearance.show_status_bar
            && !self.tabs[self.active_tab_index].is_settings
            && state == ElementState::Pressed
            && button == MouseButton::Left
        {
            self.keep_awake_popup_open = !self.keep_awake_popup_open;
            self.hovered_keep_awake_option = None;
            self.request_redraw();
            return true;
        }

        false
    }

    fn click_file_viewer_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
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
                        return true;
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
                                viewer.is_diff(),
                            );

                            if let Some(rect) = header_rects.diff_mode_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, rect) {
                                    viewer.toggle_diff_mode();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

                            if let Some(rect) = header_rects.diff_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, rect) {
                                    viewer.toggle_diff();
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
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
                                return true;
                            }

                            if let Some(md_rect) = header_rects.md_btn {
                                if crate::renderer::FileViewerHeaderRects::inside(mx, my, md_rect) {
                                    viewer.render_markdown = !viewer.render_markdown;
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
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

                            // Interacción con la barra flotante de búsqueda (`Ctrl + F`)
                            if viewer.search_open {
                                let s_rects = crate::renderer::FileViewerSearchRects::compute(&lay);
                                if crate::renderer::FileViewerSearchRects::inside(
                                    mx,
                                    my,
                                    s_rects.bar,
                                ) {
                                    let visible_lines = (lay.content_h / lay.row_h.max(16)).max(1);
                                    if crate::renderer::FileViewerSearchRects::inside(
                                        mx,
                                        my,
                                        s_rects.close_btn,
                                    ) {
                                        viewer.close_search();
                                    } else if !viewer.goto_mode
                                        && crate::renderer::FileViewerSearchRects::inside(
                                            mx,
                                            my,
                                            s_rects.prev_btn,
                                        )
                                    {
                                        viewer.search_prev(visible_lines);
                                    } else if !viewer.goto_mode
                                        && crate::renderer::FileViewerSearchRects::inside(
                                            mx,
                                            my,
                                            s_rects.next_btn,
                                        )
                                    {
                                        viewer.search_next(visible_lines);
                                    } else if crate::renderer::FileViewerSearchRects::inside(
                                        mx,
                                        my,
                                        s_rects.input,
                                    ) {
                                        let mut input = std::mem::take(&mut viewer.search_input);
                                        self.handle_text_input_click(
                                            &mut input,
                                            mx,
                                            s_rects.text_start_x,
                                        );
                                        if let Some(ref mut v) =
                                            self.tabs[self.active_tab_index].file_viewer
                                        {
                                            v.search_input = input;
                                        }
                                    }
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }

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
                                        self.scrollbar_drag = Some(ScrollbarDragState {
                                            target: ScrollbarTarget::ViewerHorizontal,
                                            grab_offset: (mx - thumb.0) as f32,
                                        });
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
                                    return true;
                                }
                            }

                            // Comprobar clic en la barra de desplazamiento del visor
                            if let Some((track, thumb)) =
                                TerminalRenderer::file_viewer_scrollbar_geometry(
                                    offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                                )
                            {
                                if crate::renderer::ScrollbarRects::hit(
                                    track,
                                    offset + viewer_w,
                                    mx,
                                    my,
                                ) {
                                    if my >= thumb.1 && my < thumb.1 + thumb.3 {
                                        self.scrollbar_drag = Some(ScrollbarDragState {
                                            target: ScrollbarTarget::ViewerVertical,
                                            grab_offset: (my - thumb.1) as f32,
                                        });
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
                                    return true;
                                }
                            }

                            // Clic dentro del contenido de texto para posicionar el cursor y seleccionar
                            let content_y =
                                top_offset + crate::renderer::FileViewerLayout::HEADER_H;
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
                                            return true;
                                        }
                                    }
                                }
                                let Some((target_line, target_col)) =
                                    viewer.hit_test(clicked_row, mx, &lay)
                                else {
                                    return true;
                                };

                                // Si es enlace externo (en modo Markdown preview o con Ctrl pulsado): abrir en navegador
                                if (viewer.render_markdown || self.modifiers.control_key())
                                    && !self.modifiers.shift_key()
                                {
                                    if let Some(url) = viewer.link_at(target_line, target_col) {
                                        crate::app::platform::shell_open_url(&url);
                                        return true;
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
                                    let is_quick_click = now
                                        .duration_since(self.last_viewer_click_time)
                                        <= platform::double_click_time()
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
                return true;
            }
        }

        // 2. Divisor entre paneles: agarrarlo para redimensionar
        if button == MouseButton::Left && state == ElementState::Pressed {
            if let Some(divider) = self.divider_at(mx, my) {
                self.divider_drag = Some(divider);
                return true;
            }
        }

        // 2b. Cabecera de un panel dividido: foco y botones dividir · maximizar · cerrar
        if button == MouseButton::Left && state == ElementState::Pressed {
            let tab = &self.tabs[self.active_tab_index];
            let hit = if tab.is_settings || tab.file_viewer.is_some() {
                None
            } else {
                self.active_tab_pane_rects(win_w, win_h)
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
                        Some((rect.pane_id, focused, header.button_at(mx, my)))
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
                    Some(PaneHeaderButton::SplitHorizontal) => {
                        self.split_active_pane(SplitDirection::Horizontal);
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
                self.request_redraw();
                return true;
            }
        }

        false
    }

    fn click_terminal_panes_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) {
        // 3. Clics en paneles de terminal activa
        let bypass = self.modifiers.shift_key();
        let pane_rects = self.active_tab_pane_rects(win_w, win_h);
        let active_tab = &mut self.tabs[self.active_tab_index];

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
                                self.scrollbar_drag = Some(ScrollbarDragState {
                                    target: ScrollbarTarget::Terminal(pane.id),
                                    grab_offset: (my - thumb_y) as f32,
                                });
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
                            .map(super::clipboard::clean_block_command)
                            .map(|c| c.into_owned());
                        let exit_code = block.as_ref().and_then(|b| b.exit_code);
                        match action {
                            crate::renderer::BlockCardButton::Copy => {
                                if let Some(command) = command {
                                    self.set_clipboard_text(&command);
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
                                if let Some(command) = command {
                                    let code = exit_code.unwrap_or(1);
                                    self.set_clipboard_text(&format!(
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
                                        && now.duration_since(self.last_terminal_click_time)
                                            <= platform::double_click_time()
                                        && self.last_terminal_click_pos.is_some_and(|(c, l)| {
                                            l == global_line
                                                && (c as isize - col as isize).abs() <= 1
                                        });

                                    if is_quick_click && self.terminal_click_count == 1 {
                                        pane.terminal.select_word_at(col, global_line);
                                        self.terminal_click_count = 2;
                                        self.drag_start = None;
                                        self.drag_anchor = pane.terminal.selection;
                                        super::clipboard::copy_pane_selection(
                                            &mut self.clipboard,
                                            self.config.terminal.copy_on_select,
                                            pane,
                                        );
                                    } else if is_quick_click && self.terminal_click_count == 2 {
                                        pane.terminal.select_line_at(global_line);
                                        self.terminal_click_count = 3;
                                        self.drag_start = None;
                                        self.drag_anchor = pane.terminal.selection;
                                        super::clipboard::copy_pane_selection(
                                            &mut self.clipboard,
                                            self.config.terminal.copy_on_select,
                                            pane,
                                        );
                                    } else {
                                        pane.terminal.selection = None;
                                        self.terminal_click_count = 1;
                                        self.drag_anchor = None;
                                        self.drag_start = Some((col, global_line));
                                    }
                                    self.last_terminal_click_time = now;
                                    self.last_terminal_click_pos = Some((col, global_line));
                                    self.last_terminal_click_pane_id = Some(pane.id);

                                    self.request_redraw();
                                }
                            }
                            ElementState::Released => {
                                self.mouse_down = false;
                                self.drag_start = None;
                                self.drag_anchor = None;
                                if pane.terminal.selection.is_some_and(|s| s.start == s.end) {
                                    pane.terminal.selection = None;
                                } else {
                                    super::clipboard::copy_pane_selection(
                                        &mut self.clipboard,
                                        self.config.terminal.copy_on_select,
                                        pane,
                                    );
                                }
                                self.request_redraw();
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
