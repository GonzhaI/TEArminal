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

        if self.space_edit.is_some() && !self.spaces_rects(win_h).contains(mx, my, win_h) {
            self.commit_space_rename();
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

        if self.click_status_bar_area(state, button, mx, my, win_w, win_h, is_settings_active) {
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
                        let real_idx = self
                            .file_explorer
                            .items
                            .iter()
                            .position(|it| it.path == drag.path)
                            .or_else(|| self.file_explorer.actual_index_for_visible(drag.idx));
                        if let Some(real_idx) = real_idx {
                            self.file_explorer.toggle_expand(real_idx);
                            self.pump_explorer_ssh();
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
            if let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_mut() {
                if viewer.selection.is_some_and(|(p1, p2)| p1 == p2) {
                    viewer.selection = None;
                }
            }
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

    #[allow(clippy::too_many_arguments)]
    fn click_status_bar_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
        is_settings_active: bool,
    ) -> bool {
        if !self.config.appearance.show_status_bar || is_settings_active {
            return false;
        }
        let status_bar_h = self.status_bar_height();
        if my < win_h.saturating_sub(status_bar_h) {
            return false;
        }

        if state != ElementState::Pressed || button != MouseButton::Left {
            return false;
        }

        // Clic sobre el badge de actualización disponible en la barra de estado
        if self.hovered_update_badge {
            crate::updater::trigger_update();
            return true;
        }

        // Clic sobre el botón de modo pantalla encendida (Activo, Agente, Apagado) en la esquina inferior derecha
        let keep_awake_w = self.renderer.as_ref().map_or(65, |r| {
            r.keep_awake_button_width(self.config.terminal.keep_awake)
        });
        let rects = StatusBarRects::compute_full(win_w, win_h, 0, None, 0, 0, keep_awake_w);
        if self.hovered_keep_awake || StatusBarRects::inside(mx, my, rects.keep_awake_btn) {
            self.keep_awake_popup_open = !self.keep_awake_popup_open;
            self.hovered_keep_awake_option = None;
            self.request_redraw();
            return true;
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
        let (ctrl_pressed, shift_pressed, _, _, _) = self.physical_modifiers();
        let bypass = shift_pressed;
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

                                    // 1. Ctrl+Click en enlace externo (navegador) o ruta de archivo (visor interno)
                                    if ctrl_pressed {
                                        if let Some(url) = pane.terminal.url_at(col, global_line) {
                                            crate::app::platform::shell_open_url(&url);
                                            self.drag_start = None;
                                            return;
                                        }
                                        if pane.terminal.ssh_session.is_none() {
                                            if let Some(candidate) = pane
                                                .terminal
                                                .file_path_candidate_at(col, global_line)
                                            {
                                                let root = &self.file_explorer.root_dir;
                                                let cwd = pane.terminal.current_dir.as_deref();
                                                if let Some(resolved) =
                                                    Self::resolve_terminal_file_path(
                                                        root, cwd, &candidate,
                                                    )
                                                {
                                                    self.open_file_in_viewer_at(
                                                        &resolved,
                                                        true,
                                                        candidate.line,
                                                        candidate.col,
                                                    );
                                                    self.drag_start = None;
                                                    return;
                                                }
                                            }
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

    /// Comprueba si una ruta apunta a un archivo local seguro (descarta rutas UNC/SMB como `\\host\share`
    /// o `//host/share` que en Windows bloquean el hilo y filtran hashes NTLMv2 por SMB).
    fn is_safe_local_file(path: &std::path::Path) -> bool {
        let s = path.as_os_str().to_string_lossy();
        if s.starts_with(r"\\") || s.starts_with("//") {
            return false;
        }
        if path.components().any(|c| {
            matches!(
                c,
                std::path::Component::Prefix(pr)
                    if matches!(
                        pr.kind(),
                        std::path::Prefix::UNC(..)
                            | std::path::Prefix::VerbatimUNC(..)
                            | std::path::Prefix::DeviceNS(_)
                    )
            )
        }) {
            return false;
        }
        path.is_file()
    }

    /// Resuelve un candidato de ruta de archivo a una ruta existente en el disco local
    pub(crate) fn resolve_terminal_file_path(
        root_dir: &std::path::Path,
        cwd: Option<&std::path::Path>,
        candidate: &crate::terminal::FilePathCandidate,
    ) -> Option<std::path::PathBuf> {
        let raw = candidate.raw_path.trim().trim_matches(['"', '\'', '`']);
        if raw.is_empty() || raw.starts_with(r"\\") || raw.starts_with("//") {
            return None;
        }
        // Bloquear `file://host/share` (UNC remoto); solo se admite `file:///ruta/local`
        if raw.starts_with("file://") && !raw.starts_with("file:///") {
            return None;
        }

        let cleaned = raw.strip_prefix("file:///").unwrap_or(raw);
        if cleaned.is_empty() || cleaned.starts_with(r"\\") || cleaned.starts_with("//") {
            return None;
        }

        let path = std::path::Path::new(cleaned);

        // 1. Ruta absoluta existente
        if path.is_absolute() && Self::is_safe_local_file(path) {
            return Some(path.to_path_buf());
        }

        // 2. Relativa a pane.terminal.current_dir
        if let Some(dir) = cwd {
            let candidate_path = dir.join(path);
            if Self::is_safe_local_file(&candidate_path) {
                return Some(candidate_path);
            }
        }

        // 3. Relativa a la raíz del explorador de proyectos
        let candidate_path = root_dir.join(path);
        if Self::is_safe_local_file(&candidate_path) {
            return Some(candidate_path);
        }

        // 4. Si path comienza con '/' o '\' en Windows, probar dentro de la raíz del explorador
        let trimmed_leading = cleaned.trim_start_matches(['/', '\\']);
        if trimmed_leading != cleaned && !trimmed_leading.is_empty() {
            let candidate_path = root_dir.join(trimmed_leading);
            if Self::is_safe_local_file(&candidate_path) {
                return Some(candidate_path);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::FilePathCandidate;

    fn dummy_candidate(raw: &str) -> FilePathCandidate {
        FilePathCandidate {
            raw_path: raw.to_string(),
            line: None,
            col: None,
            start_col: 0,
            end_col: raw.len(),
        }
    }

    #[test]
    fn resolve_terminal_file_path_rejects_unc_and_remote_file_uris() {
        let root = std::env::temp_dir();
        for unsafe_path in [
            r"\\attacker.example.com\share\secret.txt",
            "//attacker.example.com/share/secret.txt",
            "file://attacker.example.com/share/secret.txt",
            r"file:///\\attacker.example.com\share\secret.txt",
            "file://///attacker.example.com/share/secret.txt",
        ] {
            let cand = dummy_candidate(unsafe_path);
            assert!(
                TearminalApp::resolve_terminal_file_path(&root, None, &cand).is_none(),
                "debe rechazar ruta UNC/remota insegura: {unsafe_path}"
            );
        }
    }

    #[test]
    fn resolve_terminal_file_path_resolves_existing_local_file() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cand = dummy_candidate("Cargo.toml");
        let resolved = TearminalApp::resolve_terminal_file_path(&root, None, &cand);
        assert_eq!(resolved, Some(root.join("Cargo.toml")));
    }
}
