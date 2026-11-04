//! Gestión de eventos de clic dentro del explorador de archivos y barra lateral.

use super::*;
use crate::renderer::TopTabBarRects;

impl TearminalApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn click_explorer_area(
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
}
