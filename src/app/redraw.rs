//! Repintado de un fotograma completo.

use super::*;

impl AxonApp {
    /// Dibuja un fotograma completo (`WindowEvent::RedrawRequested`).
    pub(super) fn on_redraw(&mut self) {
        if self.occluded {
            return;
        }
        let is_horizontal = self.is_horizontal_tabs();
        let offset = self.sidebar_offset();
        let right_offset = self.file_explorer_offset();
        let explorer_visible = self.file_explorer.visible;
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let sidebar_visible = self.sidebar_visible && !is_horizontal;
        let active_idx = self.active_tab_index;
        let hovered_tab = self.hovered_tab;
        let hovered_close = self.hovered_close;
        let hovered_new_tab = self.hovered_new_tab;
        let hovered_files_btn = self.hovered_files_btn;
        let hovered_settings_btn = self.hovered_settings_btn;
        let hovered_toggle_btn = self.hovered_toggle_btn;
        let cursor_visible = !self.config.terminal.cursor_blink || self.cursor_on;

        // Editar una vista previa la fija (si no, la siguiente vista previa borraría el trabajo)
        for t in &mut self.tabs {
            if t.is_preview
                && t.file_viewer
                    .as_ref()
                    .is_some_and(|v| v.is_dirty || !v.is_read_only)
            {
                t.is_preview = false;
            }
        }

        let tab_items: Vec<SidebarTabItem> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(idx, t)| SidebarTabItem {
                id: t.id,
                title: t.display_title(),
                is_active: idx == active_idx,
                has_unread: t.has_unread,
                pane_count: t.panes.len(),
                is_settings: t.is_settings,
                is_file_viewer: t.file_viewer.is_some(),
                is_preview: t.is_preview,
            })
            .collect();

        let active_tab = &mut self.tabs[active_idx];

        if let (Some(surface), Some(renderer), Some(window)) =
            (&mut self.surface, &mut self.renderer, &self.window)
        {
            let size = window.inner_size();
            if size.width == 0 || size.height == 0 {
                return;
            }

            renderer.scrollbars_awake =
                self.last_scroll_time.elapsed() < std::time::Duration::from_millis(1250);
            let is_maximized = window.is_maximized();
            if let Ok(mut buffer) = surface.buffer_mut() {
                buffer.fill(self.theme.background.to_u32_xrgb());

                if is_horizontal {
                    renderer.render_top_tab_bar(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        &tab_items,
                        hovered_tab,
                        hovered_close,
                        hovered_new_tab,
                        hovered_files_btn,
                        hovered_settings_btn,
                        self.hovered_win_min,
                        self.hovered_win_max,
                        self.hovered_win_close,
                        is_maximized,
                        explorer_visible,
                        &self.theme,
                    );
                } else {
                    let active_title = active_tab.display_title();
                    let git_branch = active_tab.get_git_branch();
                    renderer.render_top_title_bar(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        &active_title,
                        git_branch.as_deref(),
                        hovered_files_btn,
                        hovered_settings_btn,
                        self.hovered_win_min,
                        self.hovered_win_max,
                        self.hovered_win_close,
                        is_maximized,
                        explorer_visible,
                        &self.theme,
                    );

                    if sidebar_visible {
                        renderer.render_sidebar(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            SIDEBAR_WIDTH,
                            status_bar_h,
                            &tab_items,
                            self.sidebar_scroll_offset,
                            hovered_tab,
                            hovered_close,
                            hovered_new_tab,
                            hovered_settings_btn,
                            hovered_toggle_btn,
                            self.sidebar_scrollbar_hovered,
                            &self.theme,
                        );
                    }
                }

                if active_tab.is_settings {
                    renderer.render_settings_page(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        offset,
                        top_offset,
                        status_bar_h,
                        &self.config,
                        &self.settings_page_state,
                        &self.theme,
                    );
                } else if let Some(ref mut viewer) = active_tab.file_viewer {
                    let term_area_w = (size.width as usize).saturating_sub(offset + right_offset);
                    let term_area_h =
                        (size.height as usize).saturating_sub(top_offset + status_bar_h);
                    viewer.word_wrap = self.config.terminal.word_wrap;
                    viewer.visible_cols = crate::renderer::FileViewerLayout::compute(
                        offset,
                        top_offset,
                        term_area_w,
                        term_area_h,
                        renderer.row_height,
                        renderer.char_width,
                        viewer.lines.len(),
                    )
                    .visible_cols;

                    renderer.render_file_viewer(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        offset,
                        top_offset,
                        term_area_w + right_offset,
                        term_area_h,
                        right_offset,
                        cursor_visible,
                        viewer,
                        &self.theme,
                    );

                    if self.file_explorer.visible {
                        renderer.render_file_explorer(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            top_offset,
                            status_bar_h,
                            &self.file_explorer,
                            &self.theme,
                        );
                    }
                } else {
                    let term_area_w = (size.width as usize).saturating_sub(offset + right_offset);
                    let term_area_h =
                        (size.height as usize).saturating_sub(top_offset + status_bar_h);
                    let pane_rects =
                        active_tab.compute_pane_rects(offset, top_offset, term_area_w, term_area_h);
                    let dividers =
                        active_tab.compute_dividers(offset, top_offset, term_area_w, term_area_h);
                    let has_multiple = pane_rects.len() > 1;

                    for rect in &pane_rects {
                        if let Some(pane) = active_tab.find_pane(rect.pane_id) {
                            let is_active = rect.pane_id == active_tab.active_pane_id;
                            renderer.render_pane(
                                &mut buffer,
                                size.width as usize,
                                size.height as usize,
                                rect.x,
                                rect.y,
                                rect.width,
                                rect.height,
                                &pane.terminal,
                                &self.theme,
                                cursor_visible,
                                is_active,
                                has_multiple,
                            );
                        }
                    }

                    let divider_color = self.theme.border_color();
                    for (dx, dy, dw, dh) in dividers {
                        TerminalRenderer::render_pane_divider(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            dx,
                            dy,
                            dw,
                            dh,
                            divider_color,
                        );
                    }

                    if self.file_explorer.visible {
                        renderer.render_file_explorer(
                            &mut buffer,
                            size.width as usize,
                            size.height as usize,
                            top_offset,
                            status_bar_h,
                            &self.file_explorer,
                            &self.theme,
                        );
                    }
                }

                if !is_horizontal && !sidebar_visible && hovered_toggle_btn {
                    renderer.render_floating_sidebar_toggle(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        status_bar_h,
                        hovered_toggle_btn,
                        &self.theme,
                    );
                }

                if self.config.appearance.show_status_bar && !active_tab.is_settings {
                    let (cwd, git_branch, ssh_host, cols, rows, ai_status) =
                        if let Some(ref viewer) = active_tab.file_viewer {
                            let path_str = {
                                static HOME: std::sync::OnceLock<Option<String>> =
                                    std::sync::OnceLock::new();
                                let s = viewer.path.to_string_lossy().to_string();
                                match HOME.get_or_init(|| std::env::var("USERPROFILE").ok()) {
                                    Some(home) if s.starts_with(home.as_str()) => {
                                        format!("~{}", &s[home.len()..])
                                    }
                                    _ => s,
                                }
                            };
                            let branch = if viewer.remote_destination.is_some() {
                                viewer.remote_branch.clone()
                            } else {
                                let dir = if viewer.path.is_relative() {
                                    std::env::current_dir()
                                        .ok()
                                        .map(|cd| cd.join(&viewer.path))
                                        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                                } else {
                                    viewer.path.parent().map(|p| p.to_path_buf())
                                };
                                crate::app::shell_context::get_git_branch(dir.as_deref())
                            };
                            let ssh = viewer.remote_destination.clone();
                            let lines_count = viewer.lines.len();
                            (
                                Some(path_str),
                                branch,
                                ssh,
                                0,
                                lines_count,
                                AiAgentStatus::None,
                            )
                        } else if let Some(pane) = active_tab.active_pane_mut() {
                            let c_dir = pane.terminal.current_dir.as_ref().map(|p| {
                                static HOME: std::sync::OnceLock<Option<String>> =
                                    std::sync::OnceLock::new();
                                let s = p.to_string_lossy().to_string();
                                match HOME.get_or_init(|| std::env::var("USERPROFILE").ok()) {
                                    Some(home) if s.starts_with(home.as_str()) => {
                                        format!("~{}", &s[home.len()..])
                                    }
                                    _ => s,
                                }
                            });
                            let g_branch = pane.get_git_branch();
                            let s_host = pane
                                .terminal
                                .ssh_session
                                .as_ref()
                                .filter(|s| s.is_active)
                                .map(|s| s.destination.clone());
                            let ai = detect_ai_agent(pane.terminal.window_title.as_deref());
                            (
                                c_dir,
                                g_branch,
                                s_host,
                                pane.terminal.cols,
                                pane.terminal.rows,
                                ai,
                            )
                        } else {
                            (None, None, None, 100, 30, AiAgentStatus::None)
                        };

                    renderer.render_status_bar(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        offset,
                        cwd.as_deref(),
                        git_branch.as_deref(),
                        ssh_host.as_deref(),
                        cols,
                        rows,
                        ai_status,
                        explorer_visible,
                        &self.theme,
                    );
                }

                if let Some(menu) = &self.context_menu {
                    renderer.render_context_menu(
                        &mut buffer,
                        size.width as usize,
                        size.height as usize,
                        menu,
                        &self.theme,
                    );
                }

                buffer.present().ok();
            }
        }
    }
}
