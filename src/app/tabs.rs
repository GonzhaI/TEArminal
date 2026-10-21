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
        self.settings_page_state.theme_slot_light =
            self.config.appearance.uses_light_theme(self.system_light);
        self.reveal_current_theme();
    }

    /// Sincroniza dinámicamente el directorio raíz del explorador con el CWD o la sesión SSH de la terminal activa
    pub fn sync_file_explorer_root(&mut self) {
        if !self.file_explorer.visible || self.file_explorer.is_pinned {
            self.sync_fs_watcher();
            return;
        }
        if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
            if let Some(pane) = active.active_pane_mut() {
                if let Some(ref ssh) = pane.terminal.ssh_session {
                    if ssh.is_active && pane.terminal.remote_integration {
                        // El shell remoto envía carpeta y listado: sin segunda conexión SSH (que exigiría clave)
                        if let (Some(rdir), Some(names)) = (
                            ssh.remote_dir.clone(),
                            pane.terminal.remote_listing.as_ref(),
                        ) {
                            let sess = crate::explorer::RemoteSshSession {
                                destination: ssh.destination.clone(),
                                remote_dir: rdir,
                            };
                            self.file_explorer.set_remote_listing(sess, names);
                        }
                        self.sync_fs_watcher();
                        return;
                    }
                    if ssh.is_active {
                        let dest = ssh.destination.clone();
                        let rdir = ssh.remote_dir.clone().unwrap_or_else(|| ".".to_string());
                        let remote_sess = crate::explorer::RemoteSshSession {
                            destination: dest,
                            remote_dir: rdir,
                        };
                        self.file_explorer.set_remote_session(Some(remote_sess));
                        self.sync_fs_watcher();
                        return;
                    }
                }
                self.file_explorer.set_remote_session(None);
                if let Some(cwd) = &pane.terminal.current_dir {
                    self.file_explorer.set_root(cwd.clone());
                }
            }
        }
        self.sync_fs_watcher();
    }

    /// Abre un archivo en una pestaña de visor. Sin `pin` es una vista previa que la siguiente vista previa
    /// reemplaza (un clic en el explorador); con `pin` (doble clic, Enter) queda fija.
    pub fn open_file_in_viewer(&mut self, path: &std::path::Path, pin: bool) {
        if let Some(rem) = self.file_explorer.remote_session.clone() {
            // Modo remoto: verificar si ya existe una pestaña con este archivo remoto
            if let Some(pos) = self.tabs.iter().position(|t| {
                t.file_viewer
                    .as_ref()
                    .map(|v| {
                        v.path == path && v.remote_destination.as_deref() == Some(&rem.destination)
                    })
                    .unwrap_or(false)
            }) {
                self.tabs[pos].is_preview &= !pin;
                self.switch_to_tab(pos);
                return;
            }

            // Ya pedido por la sesión (primer clic de un doble clic): solo se fija al llegar
            if self.pin_pending_remote_open(&rem.destination, path, pin) {
                return;
            }
            // Primero por la propia sesión (sirve con contraseña); la pestaña se abre al llegar el contenido
            if self.remote_open(&rem.destination, path, pin) {
                return;
            }
            // Sin integración (o si falla): segunda conexión SSH en un hilo; la pestaña se abre al llegar
            self.ssh_open_file(&rem.destination, path, pin);
            return;
        }

        if !path.exists() {
            return;
        }

        // Si ya existe una pestaña con este archivo local, alternar a ella
        if let Some(pos) = self.tabs.iter().position(|t| {
            t.file_viewer
                .as_ref()
                .map(|v| v.path == path && v.remote_destination.is_none())
                .unwrap_or(false)
        }) {
            self.tabs[pos].is_preview &= !pin;
            self.switch_to_tab(pos);
            return;
        }

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = TerminalTab::new_file_viewer(tab_id, path.to_path_buf());
        let fetch_git_base = !cfg!(test)
            && tab
                .file_viewer
                .as_ref()
                .is_some_and(|v| !v.is_binary && v.git_base.is_none());
        self.install_viewer_tab(tab, pin);
        if fetch_git_base {
            self.spawn_git_head_lines(path.to_path_buf());
        }
    }

    /// Clic izquierdo en una pestaña: la activa; un segundo clic rápido sobre la misma fija una vista previa
    pub(super) fn click_tab(&mut self, idx: usize) {
        let now = Instant::now();
        let double = self.last_tab_click_idx == Some(idx)
            && now.duration_since(self.last_tab_click_time).as_millis() < 400;
        self.last_tab_click_idx = Some(idx);
        self.last_tab_click_time = now;
        if double {
            if let Some(tab) = self.tabs.get_mut(idx) {
                tab.is_preview = false;
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        self.switch_to_tab(idx);
    }

    /// Coloca una pestaña de visor nueva: una vista previa reemplaza a la vista previa anterior (si no
    /// tiene cambios) en su misma posición; una fija se añade al final.
    pub(super) fn install_viewer_tab(&mut self, mut tab: TerminalTab, pin: bool) {
        tab.is_preview = !pin;
        let old_preview = if pin {
            None
        } else {
            self.tabs.iter().position(|t| {
                t.is_preview
                    && t.file_viewer
                        .as_ref()
                        .is_some_and(|v| !v.is_dirty && v.is_read_only)
            })
        };
        match old_preview {
            Some(pos) => {
                self.tabs[pos] = tab;
                if pos == self.active_tab_index {
                    self.update_window_title();
                } else {
                    self.switch_to_tab(pos);
                }
            }
            None => {
                self.tabs.push(tab);
                self.switch_to_tab(self.tabs.len() - 1);
                self.update_window_title();
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Con pestañas superiores el explorador vive en el panel y se muestra u oculta con él.
    pub fn toggle_file_explorer(&mut self) {
        if self.is_horizontal_tabs() {
            self.toggle_sidebar();
        } else {
            self.flip_file_explorer();
        }
    }

    pub(super) fn flip_file_explorer(&mut self) {
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
            self.sync_fs_watcher();
        }
        self.recompute_grid_for_all_tabs();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Sesión nueva en la carpeta por defecto del shell.
    pub fn create_new_tab(&mut self) {
        self.create_tab_in(None);
    }

    /// Sesión nueva con un shell en `cwd` (o en la carpeta por defecto del shell).
    pub(super) fn create_tab_in(&mut self, cwd: Option<std::path::PathBuf>) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = alloc_pane_id();

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
            self.config.terminal.shell_integration,
            cols as u16,
            rows as u16,
            cwd.as_deref(),
            self.proxy.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                crate::pty::report_spawn_error(&self.config.terminal.default_shell, &e);
                return;
            }
        };

        let title = format!("Terminal {}", self.tabs.len() + 1);
        let mut terminal = TerminalState::new(
            cols,
            rows,
            self.config.terminal.scrollback_lines,
            self.theme.clone(),
        );

        terminal.current_dir = cwd;
        let pane = TerminalPane::new(pane_id, pty, terminal, (cols, rows));

        self.tabs.push(TerminalTab::new(tab_id, title, pane));

        self.active_tab_index = self.tabs.len() - 1;
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        if let Some(viewer) = self.tabs[index].file_viewer.as_mut() {
            if viewer.needs_autosave {
                viewer.autosave_local();
            }
        }

        self.tabs.remove(index);
        self.prune_pending_remote();
        if self.tabs.is_empty() {
            crate::logger::log("No quedan pestañas activas: cerrando la ventana");
            self.request_close();
            return;
        }

        if index < self.active_tab_index {
            self.active_tab_index -= 1;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        if index < self.last_terminal_tab_index {
            self.last_terminal_tab_index -= 1;
        } else if self.last_terminal_tab_index >= self.tabs.len() {
            self.last_terminal_tab_index = self.tabs.len() - 1;
        }
        self.tabs[self.active_tab_index].has_unread = false;
        self.sync_file_explorer_root();
        self.clamp_sidebar_scroll();
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Desprende la pestaña `index` y solicita al anfitrión transferirla a una nueva ventana
    pub fn detach_tab_to_new_window(&mut self, index: usize) {
        if index >= self.tabs.len() || self.tabs.len() <= 1 || self.tabs[index].is_settings {
            return;
        }

        if let Some(viewer) = self.tabs[index].file_viewer.as_mut() {
            if viewer.needs_autosave {
                viewer.autosave_local();
            }
        }

        let tab = self.tabs.remove(index);
        self.prune_pending_remote();

        if index < self.active_tab_index {
            self.active_tab_index -= 1;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        if index < self.last_terminal_tab_index {
            self.last_terminal_tab_index -= 1;
        } else if self.last_terminal_tab_index >= self.tabs.len() {
            self.last_terminal_tab_index = self.tabs.len() - 1;
        }
        self.tabs[self.active_tab_index].has_unread = false;
        self.sync_file_explorer_root();
        self.clamp_sidebar_scroll();
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }

        crate::logger::log(&format!(
            "Desprendiendo pestaña '{}' a nueva ventana",
            tab.display_title()
        ));
        self.new_window_with_tab = Some(tab);
    }

    /// Acopla una pestaña existente (transferida desde otra ventana) en esta ventana
    pub fn attach_tab(&mut self, mut tab: TerminalTab) {
        tab.id = self.next_tab_id;
        self.next_tab_id += 1;
        let new_idx = self.tabs.len();
        self.tabs.push(tab);
        self.active_tab_index = new_idx;
        if !self.tabs[new_idx].is_settings {
            self.last_terminal_tab_index = new_idx;
        }
        self.recompute_grid_for_all_tabs();
        self.tabs[new_idx].has_unread = false;
        self.sync_file_explorer_root();
        self.clamp_sidebar_scroll();
        self.update_window_title();
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_other_tabs(&mut self, keep: usize) {
        let Some(id) = self.tabs.get(keep).map(|t| t.id) else {
            return;
        };
        for tab in &mut self.tabs {
            if tab.id != id {
                if let Some(viewer) = tab.file_viewer.as_mut() {
                    if viewer.needs_autosave {
                        viewer.autosave_local();
                    }
                }
            }
        }
        self.tabs.retain(|t| t.id == id);
        self.prune_pending_remote();
        self.active_tab_index = 0;
        self.last_terminal_tab_index = 0;
        self.tabs[0].has_unread = false;
        self.sync_file_explorer_root();
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
        let new_pane_id = alloc_pane_id();

        let active_pane = self
            .tabs
            .get(self.active_tab_index)
            .and_then(|t| t.active_pane());
        let ssh_session = active_pane.and_then(|p| p.terminal.ssh_session.clone());

        // Heredar el directorio de trabajo del pane activo
        let cwd_path = active_pane.and_then(|p| p.terminal.current_dir.clone());
        let cwd_ref = cwd_path.as_deref();

        let (cols, rows) = (80, 24);

        // Con sesión SSH activa se reconecta al mismo host; si falla (o no hay sesión) se abre un shell local
        let ssh_pty = ssh_session
            .as_ref()
            .filter(|s| s.is_active)
            .and_then(|ssh| {
                crate::logger::log(&format!(
                    "Split pane: reconectando automáticamente vía SSH a {}",
                    ssh.destination
                ));
                PtyManager::new_ssh(
                    new_pane_id,
                    &ssh.destination,
                    ssh.remote_dir.as_deref(),
                    cols as u16,
                    rows as u16,
                    self.proxy.clone(),
                )
                .map_err(|e| {
                    crate::logger::log(&format!(
                        "Fallo al conectar SSH para split: {:?}, usando local",
                        e
                    ))
                })
                .ok()
            });
        let (pty, initial_ssh) = match ssh_pty {
            Some(p) => (p, ssh_session),
            None => match PtyManager::new(
                new_pane_id,
                &self.config.terminal.default_shell,
                self.config.terminal.shell_integration,
                cols as u16,
                rows as u16,
                cwd_ref,
                self.proxy.clone(),
            ) {
                Ok(p) => (p, None),
                Err(e) => {
                    crate::pty::report_spawn_error(&self.config.terminal.default_shell, &e);
                    return;
                }
            },
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
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn close_active_pane(&mut self) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            if active_tab.is_settings || active_tab.file_viewer.is_some() {
                self.close_tab(self.active_tab_index);
                return;
            }
            let active_pane_id = active_tab.active_pane_id;
            self.close_pane_by_id(active_pane_id);
        }
    }

    /// El shell de un panel terminó. Si había un agente en marcha (opencode se lleva la consola por delante
    /// al salir) se abre un shell nuevo en el mismo panel y carpeta; si no (un `exit` normal) se cierra el panel.
    pub(super) fn on_shell_exit(&mut self, pane_id: usize) {
        if !self.respawn_shell_after_agent(pane_id) {
            self.close_pane_by_id(pane_id);
        }
    }

    fn respawn_shell_after_agent(&mut self, pane_id: usize) -> bool {
        let Some(pane) = self
            .tabs
            .iter_mut()
            .find_map(|t| t.find_pane_mut(pane_id))
            .filter(|p| p.terminal.agent_running)
        else {
            return false;
        };
        // Una sola reapertura: si el nuevo shell también muere, el panel se cierra
        pane.terminal.agent_running = false;
        let (cols, rows) = pane.last_grid_size;
        let cwd = pane.terminal.current_dir.clone().filter(|d| d.is_dir());
        match PtyManager::new(
            pane_id,
            &self.config.terminal.default_shell,
            self.config.terminal.shell_integration,
            cols as u16,
            rows as u16,
            cwd.as_deref(),
            self.proxy.clone(),
        ) {
            Ok(pty) => {
                crate::logger::log(&format!(
                    "Pane {pane_id}: el shell terminó con un agente en marcha, se reabre"
                ));
                pane.terminal.pane_token = pty.token.clone();
                pane.pty = pty;
                // Pantalla alternativa, modos de ratón/pegado y atributos como los dejaría un agente que sale bien
                pane.terminal.process_bytes(
                    b"\x1b[?1049l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[0m\x1b[?25h\x1b[r\x1b[2J\x1b[3J\x1b[H",
                );
                true
            }
            Err(e) => {
                crate::logger::log(&format!("No se pudo reabrir el shell: {e}"));
                false
            }
        }
    }

    pub fn close_pane_by_id(&mut self, pane_id: usize) {
        let mut tab_to_close = None;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if tab.panes.iter().any(|p| p.id == pane_id) {
                if tab.panes.len() <= 1 {
                    tab_to_close = Some(idx);
                } else {
                    tab.close_pane(pane_id);
                    self.prune_pending_remote();
                    self.recompute_grid_for_all_tabs();
                    self.update_window_title();
                    self.save_session();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                break;
            }
        }

        if let Some(idx) = tab_to_close {
            self.close_tab(idx);
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

    /// Área de paneles de terminal `(x, y, ancho, alto)` entre barras, barra lateral y explorador
    fn pane_area(&self) -> Option<(usize, usize, usize, usize)> {
        let size = self.window.as_ref()?.inner_size();
        let (x, y) = (self.sidebar_offset(), self.top_offset());
        let w = (size.width as usize).saturating_sub(x + self.file_explorer_offset());
        let h = (size.height as usize).saturating_sub(y + self.status_bar_height());
        Some((x, y, w, h))
    }

    /// Divisor entre paneles bajo `(mx, my)` en la pestaña activa
    pub(super) fn divider_at(&self, mx: usize, my: usize) -> Option<Divider> {
        let (x, y, w, h) = self.pane_area()?;
        let tab = self.tabs.get(self.active_tab_index)?;
        let dividers = tab.compute_dividers(x, y, w, h);
        dividers.into_iter().find(|d| d.contains(mx, my))
    }

    /// Mueve el divisor agarrado hasta el ratón; el PTY se redimensiona al soltarlo o al detenerse
    pub(super) fn drag_divider(&mut self, mx: usize, my: usize) {
        let Some(divider) = &self.divider_drag else {
            return;
        };
        let ratio = divider.ratio_at(divider.axis(mx, my));
        let path = divider.path.clone();
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.set_split_ratio(&path, ratio);
        }
        self.schedule_pty_resize();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Posición de pestaña bajo el ratón mientras se arrastra una (solo cuenta la coordenada a lo largo de la lista)
    pub(super) fn tab_slot_at(&self, mx: usize, my: usize) -> Option<usize> {
        let slot = if self.is_horizontal_tabs() {
            let width = self.window.as_ref()?.inner_size().width as usize;
            let rects = TopTabBarRects::compute(width, self.tabs.len());
            (0..self.tabs.len()).find(|&idx| {
                let r = rects.tab_rect(idx);
                mx >= r.0 && mx < r.0 + r.2
            })?
        } else {
            let height = self.window.as_ref()?.inner_size().height as usize;
            let usable_h = height.saturating_sub(self.status_bar_height());
            self.panel_tab_at(my, usable_h)?
        };
        (slot < self.tabs.len()).then_some(slot)
    }

    /// Mueve la pestaña `from` a la posición `to`; la activa sigue siendo la misma pestaña
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        let moved = |i: usize| moved_index(i, from, to);
        self.active_tab_index = moved(self.active_tab_index);
        self.last_terminal_tab_index = moved(self.last_terminal_tab_index);
        self.last_tab_click_idx = self.last_tab_click_idx.map(moved);
        self.hovered_tab = self.hovered_tab.map(moved);
        self.tab_drag = self.tab_drag.map(moved);
        self.save_session();
        if let Some(w) = &self.window {
            w.request_redraw();
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
        if self.is_horizontal_tabs() && self.file_explorer.visible != self.sidebar_visible {
            self.flip_file_explorer();
        }
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
        let usable_h = win_h.saturating_sub(self.status_bar_height());
        let (total, visible) = self.panel_scroll_extent(usable_h);
        let max_offset = total.saturating_sub(visible);
        self.sidebar_scroll_offset = self.sidebar_scroll_offset.min(max_offset);
    }

    /// Navega el explorador remoto escribiendo `cd` en el shell remoto de la terminal activa
    /// (la nueva carpeta llega de vuelta por OSC 7 y AxonLs). `false` si no hay integración SSH.
    pub fn remote_cd(&mut self, dir: &str) -> bool {
        let Some(pane) = self
            .tabs
            .get_mut(self.active_tab_index)
            .and_then(|t| t.active_pane_mut())
        else {
            return false;
        };
        if !pane.terminal.remote_integration
            || !pane.terminal.remote_at_prompt
            || dir.chars().any(char::is_control)
        {
            return false;
        }
        pane.terminal.note_enter_sent();
        // Ctrl+U limpia lo que hubiera escrito en la línea antes de lanzar el cd
        let cmd = format!("\x15cd -- {}\r", crate::ssh::quote(dir));
        pane.pty.write_input(cmd.as_bytes());
        true
    }
}

/// Dónde queda la pestaña `i` tras mover la de `from` a `to`
pub(super) fn moved_index(i: usize, from: usize, to: usize) -> usize {
    if i == from {
        to
    } else if from < to && i > from && i <= to {
        i - 1
    } else if to < from && i >= to && i < from {
        i + 1
    } else {
        i
    }
}
