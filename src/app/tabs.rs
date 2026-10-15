//! Pestañas y paneles: crear, cerrar, dividir, navegar y cambiar de pestaña.

use super::*;

impl AxonApp {
    pub fn open_or_switch_settings_tab(&mut self) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            if active_tab.is_settings {
                // Si la pestaña de ajustes ya está abierta y activa, permanecer en ella
                return;
            }
        }

        self.last_terminal_tab_index = self.active_tab_index;

        if !self.is_horizontal_tabs() && !self.sidebar_visible {
            self.sidebar_visible = true;
        }

        if let Some(pos) = self.tabs.iter().position(|t| t.is_settings) {
            self.switch_to_tab(pos);
        } else {
            let tab_id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs.push(TerminalTab::new_settings(tab_id));
            self.switch_to_tab(self.tabs.len() - 1);
        }
    }

    /// Sincroniza dinámicamente el directorio raíz del explorador con el CWD de la terminal activa
    pub fn sync_file_explorer_root(&mut self) {
        if let Some(active) = self.tabs.get(self.active_tab_index) {
            if let Some(pane) = active.active_pane() {
                if let Some(cwd) = &pane.terminal.current_dir {
                    self.file_explorer.set_root(cwd.clone());
                }
            }
        }
    }

    pub fn open_file_in_viewer(&mut self, path: &std::path::Path) {
        if !path.exists() {
            return;
        }

        // Si ya existe una pestaña con este archivo, alternar a ella
        if let Some(pos) = self.tabs.iter().position(|t| {
            t.file_viewer
                .as_ref()
                .map(|v| v.path == path)
                .unwrap_or(false)
        }) {
            self.switch_to_tab(pos);
            return;
        }

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs
            .push(TerminalTab::new_file_viewer(tab_id, path.to_path_buf()));
        self.switch_to_tab(self.tabs.len() - 1);
        self.update_window_title();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn toggle_file_explorer(&mut self) {
        self.file_explorer.toggle();
        if self.file_explorer.visible {
            self.sync_file_explorer_root();
            self.file_explorer.is_focused = true;
            if self.file_explorer.selected_index.is_none() && !self.file_explorer.items.is_empty() {
                self.file_explorer.selected_index = Some(0);
                self.file_explorer.selected_path = Some(self.file_explorer.items[0].path.clone());
            }
        } else {
            self.file_explorer.is_focused = false;
        }
        self.recompute_grid_for_all_tabs();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn create_new_tab(&mut self) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let (cols, rows) = if let (Some(renderer), Some(window)) = (&self.renderer, &self.window) {
            let size = window.inner_size();
            let avail_w = (size.width as usize)
                .saturating_sub(self.sidebar_offset() + self.file_explorer_offset());
            let avail_h =
                (size.height as usize).saturating_sub(self.top_offset() + self.status_bar_height());
            renderer.compute_pane_grid_size(avail_w, avail_h)
        } else {
            (100, 30)
        };

        let pty = match PtyManager::new(
            pane_id,
            &self.config.terminal.default_shell,
            self.config.terminal.ghostty_prompt,
            cols as u16,
            rows as u16,
            None,
            self.proxy.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                crate::pty::report_spawn_error(&self.config.terminal.default_shell, &e);
                return;
            }
        };

        let title = format!("Terminal {}", self.tabs.len() + 1);
        let terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );

        let pane = TerminalPane::new(pane_id, pty, terminal, (cols, rows));

        self.tabs.push(TerminalTab::new(tab_id, title, pane));

        self.active_tab_index = self.tabs.len() - 1;
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_tab(&mut self, index: usize, event_loop: &ActiveEventLoop) {
        if index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando Axon");
            self.save_session();
            event_loop.exit();
            return;
        }

        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        self.tabs[self.active_tab_index].has_unread = false;
        self.clamp_sidebar_scroll();
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn split_active_pane(&mut self, direction: SplitDirection) {
        if self.tabs[self.active_tab_index].is_settings
            || self.tabs[self.active_tab_index].file_viewer.is_some()
        {
            return;
        }
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let active_pane = self
            .tabs
            .get(self.active_tab_index)
            .and_then(|t| t.active_pane());
        let ssh_session = active_pane.and_then(|p| p.terminal.ssh_session.clone());

        // Heredar el directorio de trabajo del pane activo
        let cwd_path = active_pane.and_then(|p| p.terminal.current_dir.clone());
        let cwd_ref = cwd_path.as_deref();

        let (cols, rows) = (80, 24);

        let (pty, initial_ssh) = if let Some(ref ssh) = ssh_session {
            if ssh.is_active {
                crate::logger::log(&format!(
                    "Split pane: reconectando automáticamente vía SSH a {}",
                    ssh.destination
                ));
                match PtyManager::new_ssh(
                    new_pane_id,
                    &ssh.destination,
                    ssh.remote_dir.as_deref(),
                    cols as u16,
                    rows as u16,
                    self.proxy.clone(),
                ) {
                    Ok(p) => (p, Some(ssh.clone())),
                    Err(e) => {
                        crate::logger::log(&format!(
                            "Fallo al conectar SSH para split: {:?}, usando local",
                            e
                        ));
                        let local_p = match PtyManager::new(
                            new_pane_id,
                            &self.config.terminal.default_shell,
                            self.config.terminal.ghostty_prompt,
                            cols as u16,
                            rows as u16,
                            cwd_ref,
                            self.proxy.clone(),
                        ) {
                            Ok(p) => p,
                            Err(e2) => {
                                crate::pty::report_spawn_error(
                                    &self.config.terminal.default_shell,
                                    &e2,
                                );
                                return;
                            }
                        };
                        (local_p, None)
                    }
                }
            } else {
                let local_p = match PtyManager::new(
                    new_pane_id,
                    &self.config.terminal.default_shell,
                    self.config.terminal.ghostty_prompt,
                    cols as u16,
                    rows as u16,
                    cwd_ref,
                    self.proxy.clone(),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        crate::pty::report_spawn_error(&self.config.terminal.default_shell, &e);
                        return;
                    }
                };
                (local_p, None)
            }
        } else {
            let local_p = match PtyManager::new(
                new_pane_id,
                &self.config.terminal.default_shell,
                self.config.terminal.ghostty_prompt,
                cols as u16,
                rows as u16,
                cwd_ref,
                self.proxy.clone(),
            ) {
                Ok(p) => p,
                Err(e) => {
                    crate::pty::report_spawn_error(&self.config.terminal.default_shell, &e);
                    return;
                }
            };
            (local_p, None)
        };

        let mut terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );
        terminal.current_dir = cwd_path;
        terminal.ssh_session = initial_ssh;

        let new_pane = TerminalPane::new(new_pane_id, pty, terminal, (cols, rows));

        let active_tab = &mut self.tabs[self.active_tab_index];
        active_tab.split_active_pane(new_pane_id, new_pane, direction);

        self.recompute_grid_for_all_tabs();
        self.update_window_title();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_active_pane(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            if active_tab.is_settings || active_tab.file_viewer.is_some() {
                self.close_tab(self.active_tab_index, event_loop);
                return;
            }
            let active_pane_id = active_tab.active_pane_id;
            self.close_pane_by_id(active_pane_id, event_loop);
        }
    }

    pub fn close_pane_by_id(&mut self, pane_id: usize, event_loop: &ActiveEventLoop) {
        let mut tab_to_close = None;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if tab.panes.iter().any(|p| p.id == pane_id) {
                if tab.panes.len() <= 1 {
                    tab_to_close = Some(idx);
                } else {
                    tab.close_pane(pane_id);
                    self.recompute_grid_for_all_tabs();
                    self.update_window_title();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                break;
            }
        }

        if let Some(idx) = tab_to_close {
            self.close_tab(idx, event_loop);
        }
    }

    pub fn navigate_pane(&mut self, dir: PaneDirection) {
        if self.tabs[self.active_tab_index].is_settings
            || self.tabs[self.active_tab_index].file_viewer.is_some()
        {
            return;
        }
        if let (Some(window), Some(_renderer)) = (&self.window, &self.renderer) {
            let size = window.inner_size();
            let offset = self.sidebar_offset();
            let right_offset = self.file_explorer_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let total_w = (size.width as usize).saturating_sub(offset + right_offset);
            let total_h = (size.height as usize).saturating_sub(top_offset + status_bar_h);
            if total_w == 0 || total_h == 0 {
                return;
            }

            let active_tab = &mut self.tabs[self.active_tab_index];
            let rects = active_tab.compute_pane_rects(offset, top_offset, total_w, total_h);
            if rects.len() <= 1 {
                return;
            }

            let cur_rect = match rects
                .iter()
                .find(|r| r.pane_id == active_tab.active_pane_id)
            {
                Some(r) => *r,
                None => return,
            };

            let cx = cur_rect.x as isize + (cur_rect.width as isize / 2);
            let cy = cur_rect.y as isize + (cur_rect.height as isize / 2);

            let mut best_candidate: Option<(usize, isize)> = None;

            for r in &rects {
                if r.pane_id == active_tab.active_pane_id {
                    continue;
                }
                let rcx = r.x as isize + (r.width as isize / 2);
                let rcy = r.y as isize + (r.height as isize / 2);

                let is_valid = match dir {
                    PaneDirection::Up => rcy < cy,
                    PaneDirection::Down => rcy > cy,
                    PaneDirection::Left => rcx < cx,
                    PaneDirection::Right => rcx > cx,
                };

                if is_valid {
                    let dist = match dir {
                        PaneDirection::Up => (cy - rcy) * 2 + (cx - rcx).abs(),
                        PaneDirection::Down => (rcy - cy) * 2 + (cx - rcx).abs(),
                        PaneDirection::Left => (cx - rcx) * 2 + (cy - rcy).abs(),
                        PaneDirection::Right => (rcx - cx) * 2 + (cy - rcy).abs(),
                    };
                    if best_candidate.is_none_or(|(_, best)| dist < best) {
                        best_candidate = Some((r.pane_id, dist));
                    }
                }
            }

            if let Some((next_id, _)) = best_candidate {
                active_tab.active_pane_id = next_id;
                window.request_redraw();
            }
        }
    }

    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab_index {
            if !self.tabs[self.active_tab_index].is_settings {
                self.last_terminal_tab_index = self.active_tab_index;
            }
            self.active_tab_index = index;
            self.tabs[index].has_unread = false;
            self.sync_file_explorer_root();
            self.update_window_title();
            self.save_session();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            self.switch_to_tab(next);
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            self.switch_to_tab(prev);
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.recompute_grid_for_all_tabs();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn clamp_sidebar_scroll(&mut self) {
        let win_h = if let Some(w) = &self.window {
            w.inner_size().height as usize
        } else {
            800
        };
        let status_bar_h = self.status_bar_height();
        let usable_h = win_h.saturating_sub(status_bar_h);
        let tab_item_h = 36;
        let tab_spacing = 4;
        let list_start_y = 42;
        let toggle_h = 30;
        let toggle_y = usable_h.saturating_sub(toggle_h + 8);
        let list_max_y = toggle_y.saturating_sub(8);
        let visible_items = if list_max_y > list_start_y {
            (list_max_y - list_start_y) / (tab_item_h + tab_spacing)
        } else {
            1
        };
        let max_offset = self.tabs.len().saturating_sub(visible_items);
        self.sidebar_scroll_offset = self.sidebar_scroll_offset.min(max_offset);
    }
}
