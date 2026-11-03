//! Movimiento del ratón: hover, arrastres, selección y reporte de ratón a TUIs.

use super::*;

impl TearminalApp {
    /// Movimiento del ratón: hover, arrastres, selección y reporte de ratón a TUIs.
    pub(super) fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.mouse_pos = position;
        let top_offset = self.top_offset();
        let mx = position.x as usize;
        let my = position.y as usize;

        let (win_w, win_h) = self.window_size();

        let mut needs_redraw = false;
        let mut is_pointer = false;
        let mut is_text = false;
        let mut new_url_span = None;
        let previous_pane_btn = self.hovered_pane_btn;
        self.hovered_pane_btn = None;
        let previous_pane_header = self.hovered_pane_header;
        self.hovered_pane_header = None;

        let is_settings_active = self.tabs[self.active_tab_index].is_settings;

        if self.cursor_handle_active_drags(mx, my, win_w, win_h) {
            return;
        }

        if self.cursor_handle_overlays(mx, my, win_w, win_h) {
            return;
        }

        let mut new_hovered_min = false;
        let mut new_hovered_max = false;
        let mut new_hovered_close_win = false;

        // Panel de Spaces
        let new_spaces_hover = self.spaces_hit(mx, my, win_h);
        if new_spaces_hover != crate::renderer::SpacesHover::None {
            is_pointer = true;
        }
        if self.spaces_hover != new_spaces_hover {
            self.spaces_hover = new_spaces_hover;
            needs_redraw = true;
        }

        {
            let visible = self.visible_tab_indices();
            let top_rects = TopTabBarRects::compute(win_w, self.spaces_w(), visible.len());
            let mut new_hovered_tab = None;
            let mut new_hovered_close = None;
            let mut new_hovered_new_tab = false;
            let mut new_hovered_files = false;
            let mut new_hovered_changes = false;

            if my < TopTabBarRects::HEIGHT && mx >= self.spaces_w() {
                if TopTabBarRects::inside(mx, my, top_rects.win_min_btn) {
                    new_hovered_min = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.win_max_btn) {
                    new_hovered_max = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.win_close_btn) {
                    new_hovered_close_win = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.plus_btn) {
                    new_hovered_new_tab = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.files_btn) {
                    new_hovered_files = true;
                    is_pointer = true;
                }
                if TopTabBarRects::inside(mx, my, top_rects.changes_btn) {
                    new_hovered_changes = true;
                    is_pointer = true;
                }
                for (pos, &idx) in visible.iter().enumerate() {
                    let tr = top_rects.tab_rect(pos);
                    if mx >= tr.0 && mx < tr.0 + tr.2 && my >= tr.1 && my < tr.1 + tr.3 {
                        new_hovered_tab = Some(idx);
                        is_pointer = true;
                        if TopTabBarRects::inside(mx, my, top_rects.close_btn_rect(pos)) {
                            new_hovered_close = Some(idx);
                        }
                        break;
                    }
                }
            }

            if self.hovered_tab != new_hovered_tab {
                self.hovered_tab = new_hovered_tab;
                needs_redraw = true;
            }
            if self.hovered_close != new_hovered_close {
                self.hovered_close = new_hovered_close;
                needs_redraw = true;
            }
            if self.hovered_new_tab != new_hovered_new_tab {
                self.hovered_new_tab = new_hovered_new_tab;
                needs_redraw = true;
            }
            if self.hovered_files_btn != new_hovered_files {
                self.hovered_files_btn = new_hovered_files;
                needs_redraw = true;
            }
            if self.hovered_changes_btn != new_hovered_changes {
                self.hovered_changes_btn = new_hovered_changes;
                needs_redraw = true;
            }
        }

        // Detección de hover en la pestaña de Ajustes (si está activa)
        let (modal_x, modal_y, modal_w, modal_h) = self.settings_modal();
        if is_settings_active
            && mx >= modal_x
            && mx < modal_x + modal_w
            && my >= modal_y
            && my < modal_y + modal_h
        {
            // Coordenadas locales al modal: su esquina es el origen de la página
            let (mx, my) = (mx - modal_x, my - modal_y);
            let (offset, top_offset) = (0, 0);
            // La navegación lateral no se desplaza con el contenido: usa la `y` sin scroll
            let nav_my = my;
            let rects = SettingsPageRects::compute_modal(offset, top_offset, modal_w, modal_h);
            let right = modal_w;
            let over_sb = self
                .settings_scrollbar()
                .is_some_and(|(t, _)| crate::renderer::ScrollbarRects::hit(t, right, mx, my));
            if over_sb {
                is_pointer = true;
            }
            if self.settings_page_state.hovered_scrollbar != over_sb {
                self.settings_page_state.hovered_scrollbar = over_sb;
                needs_redraw = true;
            }
            let my = if my >= top_offset + SettingsPageRects::CONTENT_TOP {
                my + self.settings_scroll()
            } else {
                my
            };

            let h_sec = if SettingsPageRects::inside(mx, nav_my, rects.nav_appearance) {
                Some(SettingsSection::Appearance)
            } else if SettingsPageRects::inside(mx, nav_my, rects.nav_terminal) {
                Some(SettingsSection::Terminal)
            } else if SettingsPageRects::inside(mx, nav_my, rects.nav_shortcuts) {
                Some(SettingsSection::Shortcuts)
            } else if SettingsPageRects::inside(mx, nav_my, rects.nav_about) {
                Some(SettingsSection::About)
            } else {
                None
            };

            if h_sec.is_some() {
                is_pointer = true;
            }

            let mut h_theme = None;
            let mut h_ts = None;
            let mut h_tm = None;
            let mut h_ff_prev = false;
            let mut h_ff_next = false;
            let mut h_fm = false;
            let mut h_fp = false;
            let mut h_fb = false;
            let mut h_uif_m = false;
            let mut h_uif_p = false;
            let mut h_uif_b = false;
            let mut h_sm = false;
            let mut h_sp = false;
            let mut h_sb_val = false;
            let mut h_om = false;
            let mut h_op = false;
            let mut h_ob = false;
            let mut h_cb = false;
            let mut h_cshape = None;
            let mut h_dshell = None;
            let mut h_sbm = false;
            let mut h_sbp = false;
            let mut h_sbb = false;
            let mut h_gp = false;
            let mut h_cs = false;
            let mut h_sb = false;
            let mut h_sdim = false;
            let mut h_fe = false;
            let mut h_rs = false;
            let mut h_as = false;
            let mut h_ww = false;
            let mut h_si = false;
            let mut h_osc52 = false;
            let mut h_lig = false;
            let mut h_ail = false;
            let mut h_acu = false;
            let mut h_ka = None;
            let mut h_of = false;
            let mut h_reset = SettingsPageRects::inside(mx, my, rects.reset_defaults_btn);
            let h_search = SettingsPageRects::inside(mx, my, rects.search_box);
            if h_search {
                is_text = true;
            }
            if self.settings_page_state.search_input.is_dragging {
                let text_start_x = rects.search_box.0 + 28;
                let mut input = std::mem::take(&mut self.settings_page_state.search_input);
                if self.handle_text_input_drag(&mut input, mx, text_start_x) {
                    needs_redraw = true;
                }
                self.settings_page_state.search_input = input;
            }

            match self.settings_page_state.section {
                SettingsSection::Appearance => {
                    if SettingsPageRects::inside(mx, my, rects.theme_search_box) {
                        is_text = true;
                    }
                    if self.settings_page_state.theme_search_input.is_dragging {
                        let text_start_x = rects.theme_search_box.0 + 30;
                        let mut input =
                            std::mem::take(&mut self.settings_page_state.theme_search_input);
                        if self.handle_text_input_drag(&mut input, mx, text_start_x) {
                            needs_redraw = true;
                        }
                        self.settings_page_state.theme_search_input = input;
                    }
                    h_theme = rects.theme_row_at(mx, my);
                    h_ts = if SettingsPageRects::inside(mx, my, rects.theme_slot_dark_btn) {
                        Some(0)
                    } else if SettingsPageRects::inside(mx, my, rects.theme_slot_light_btn) {
                        Some(1)
                    } else {
                        None
                    };
                    if SettingsPageRects::inside(mx, my, rects.theme_mode_system_btn) {
                        h_tm = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.theme_mode_dark_btn) {
                        h_tm = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.theme_mode_light_btn) {
                        h_tm = Some(2);
                    }

                    h_ff_prev = SettingsPageRects::inside(mx, my, rects.font_family_prev_btn);
                    h_ff_next = SettingsPageRects::inside(mx, my, rects.font_family_next_btn);
                    h_fm = SettingsPageRects::inside(mx, my, rects.font_minus_btn);
                    h_fp = SettingsPageRects::inside(mx, my, rects.font_plus_btn);
                    h_fb = SettingsPageRects::inside(mx, my, rects.font_value_box);
                    h_uif_m = SettingsPageRects::inside(mx, my, rects.ui_font_minus_btn);
                    h_uif_p = SettingsPageRects::inside(mx, my, rects.ui_font_plus_btn);
                    h_uif_b = SettingsPageRects::inside(mx, my, rects.ui_font_value_box);
                    h_sm = SettingsPageRects::inside(mx, my, rects.spacing_minus_btn);
                    h_sp = SettingsPageRects::inside(mx, my, rects.spacing_plus_btn);
                    h_sb_val = SettingsPageRects::inside(mx, my, rects.spacing_value_box);
                    h_om = SettingsPageRects::inside(mx, my, rects.opacity_minus_btn);
                    h_op = SettingsPageRects::inside(mx, my, rects.opacity_plus_btn);
                    h_ob = SettingsPageRects::inside(mx, my, rects.opacity_value_box);
                }
                SettingsSection::Terminal => {
                    h_cb = SettingsPageRects::inside(mx, my, rects.cursor_blink_btn);
                    if SettingsPageRects::inside(mx, my, rects.cursor_shape_block_btn) {
                        h_cshape = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.cursor_shape_beam_btn) {
                        h_cshape = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.cursor_shape_underline_btn) {
                        h_cshape = Some(2);
                    }
                    if SettingsPageRects::inside(mx, my, rects.shell_pwsh_btn) {
                        h_dshell = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.shell_powershell_btn) {
                        h_dshell = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.shell_cmd_btn) {
                        h_dshell = Some(2);
                    }
                    h_sbm = SettingsPageRects::inside(mx, my, rects.scrollback_minus_btn);
                    h_sbp = SettingsPageRects::inside(mx, my, rects.scrollback_plus_btn);
                    h_sbb = SettingsPageRects::inside(mx, my, rects.scrollback_value_box);
                    h_gp = SettingsPageRects::inside(mx, my, rects.shell_integration_btn);
                    h_cs = SettingsPageRects::inside(mx, my, rects.copy_on_select_btn);
                    h_sb = SettingsPageRects::inside(mx, my, rects.status_bar_toggle_btn);
                    h_sdim = SettingsPageRects::inside(mx, my, rects.show_dimensions_btn);
                    h_fe = SettingsPageRects::inside(mx, my, rects.file_explorer_toggle_btn);
                    h_rs = SettingsPageRects::inside(mx, my, rects.restore_session_btn);
                    h_as = SettingsPageRects::inside(mx, my, rects.auto_save_btn);
                    h_ww = SettingsPageRects::inside(mx, my, rects.word_wrap_btn);
                    h_si = SettingsPageRects::inside(mx, my, rects.ssh_integration_btn);
                    h_osc52 = SettingsPageRects::inside(mx, my, rects.osc52_btn);
                    h_lig = SettingsPageRects::inside(mx, my, rects.ligatures_btn);
                    h_ail = SettingsPageRects::inside(mx, my, rects.ai_limits_btn);
                    h_acu = SettingsPageRects::inside(mx, my, rects.auto_check_updates_btn);
                    if SettingsPageRects::inside(mx, my, rects.keep_awake_off_btn) {
                        h_ka = Some(0);
                    } else if SettingsPageRects::inside(mx, my, rects.keep_awake_agent_btn) {
                        h_ka = Some(1);
                    } else if SettingsPageRects::inside(mx, my, rects.keep_awake_active_btn) {
                        h_ka = Some(2);
                    }
                    h_of = SettingsPageRects::inside(mx, my, rects.open_file_btn);
                    h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.term_reset_btn);
                }
                SettingsSection::About => {
                    h_of = SettingsPageRects::inside(mx, my, rects.about_open_file_btn);
                    h_reset = h_reset || SettingsPageRects::inside(mx, my, rects.about_reset_btn);
                }
                SettingsSection::Shortcuts => {}
            }

            if h_theme.is_some()
                || h_ts.is_some()
                || h_tm.is_some()
                || h_ff_prev
                || h_ff_next
                || h_fm
                || h_fp
                || h_fb
                || h_uif_m
                || h_uif_p
                || h_uif_b
                || h_sm
                || h_sp
                || h_sb_val
                || h_om
                || h_op
                || h_ob
                || h_cb
                || h_cshape.is_some()
                || h_dshell.is_some()
                || h_sbm
                || h_sbp
                || h_sbb
                || h_gp
                || h_cs
                || h_sb
                || h_sdim
                || h_fe
                || h_rs
                || h_as
                || h_ww
                || h_si
                || h_osc52
                || h_lig
                || h_ail
                || h_acu
                || h_ka.is_some()
                || h_of
                || h_reset
            {
                is_pointer = true;
            }

            let st = &mut self.settings_page_state;
            if st.hovered_section != h_sec
                || st.hovered_theme != h_theme
                || st.hovered_theme_slot != h_ts
                || st.hovered_theme_mode != h_tm
                || st.hovered_font_family_prev != h_ff_prev
                || st.hovered_font_family_next != h_ff_next
                || st.hovered_font_minus != h_fm
                || st.hovered_font_plus != h_fp
                || st.hovered_font_box != h_fb
                || st.hovered_ui_font_minus != h_uif_m
                || st.hovered_ui_font_plus != h_uif_p
                || st.hovered_ui_font_box != h_uif_b
                || st.hovered_spacing_minus != h_sm
                || st.hovered_spacing_plus != h_sp
                || st.hovered_spacing_box != h_sb_val
                || st.hovered_opacity_minus != h_om
                || st.hovered_opacity_plus != h_op
                || st.hovered_opacity_box != h_ob
                || st.hovered_cursor_blink != h_cb
                || st.hovered_cursor_shape != h_cshape
                || st.hovered_default_shell != h_dshell
                || st.hovered_scrollback_minus != h_sbm
                || st.hovered_scrollback_plus != h_sbp
                || st.hovered_scrollback_box != h_sbb
                || st.hovered_shell_integration != h_gp
                || st.hovered_copy_on_select != h_cs
                || st.hovered_status_bar != h_sb
                || st.hovered_show_dimensions != h_sdim
                || st.hovered_file_explorer != h_fe
                || st.hovered_restore_session != h_rs
                || st.hovered_auto_save != h_as
                || st.hovered_word_wrap != h_ww
                || st.hovered_ssh_integration != h_si
                || st.hovered_osc52 != h_osc52
                || st.hovered_ligatures != h_lig
                || st.hovered_ai_limits != h_ail
                || st.hovered_auto_check_updates != h_acu
                || st.hovered_keep_awake != h_ka
                || st.hovered_open_file != h_of
                || st.hovered_reset_defaults != h_reset
                || st.hovered_search_box != h_search
            {
                st.hovered_section = h_sec;
                st.hovered_theme = h_theme;
                st.hovered_theme_slot = h_ts;
                st.hovered_theme_mode = h_tm;
                st.hovered_font_family_prev = h_ff_prev;
                st.hovered_font_family_next = h_ff_next;
                st.hovered_font_minus = h_fm;
                st.hovered_font_plus = h_fp;
                st.hovered_font_box = h_fb;
                st.hovered_ui_font_minus = h_uif_m;
                st.hovered_ui_font_plus = h_uif_p;
                st.hovered_ui_font_box = h_uif_b;
                st.hovered_spacing_minus = h_sm;
                st.hovered_spacing_plus = h_sp;
                st.hovered_spacing_box = h_sb_val;
                st.hovered_opacity_minus = h_om;
                st.hovered_opacity_plus = h_op;
                st.hovered_opacity_box = h_ob;
                st.hovered_cursor_blink = h_cb;
                st.hovered_cursor_shape = h_cshape;
                st.hovered_default_shell = h_dshell;
                st.hovered_scrollback_minus = h_sbm;
                st.hovered_scrollback_plus = h_sbp;
                st.hovered_scrollback_box = h_sbb;
                st.hovered_shell_integration = h_gp;
                st.hovered_copy_on_select = h_cs;
                st.hovered_status_bar = h_sb;
                st.hovered_show_dimensions = h_sdim;
                st.hovered_file_explorer = h_fe;
                st.hovered_restore_session = h_rs;
                st.hovered_auto_save = h_as;
                st.hovered_word_wrap = h_ww;
                st.hovered_ssh_integration = h_si;
                st.hovered_osc52 = h_osc52;
                st.hovered_ligatures = h_lig;
                st.hovered_ai_limits = h_ail;
                st.hovered_auto_check_updates = h_acu;
                st.hovered_keep_awake = h_ka;
                st.hovered_open_file = h_of;
                st.hovered_reset_defaults = h_reset;
                st.hovered_search_box = h_search;
                needs_redraw = true;
            }
        } else if is_settings_active && self.settings_page_state.clear_hover() {
            needs_redraw = true;
        }

        // Detección de hover dentro del explorador lateral de archivos (si no es settings)
        if !is_settings_active && self.file_explorer.visible {
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
                let is_over_scrollbar = if in_expanded_item {
                    false
                } else if let Some((track, _thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                    &self.file_explorer,
                ) {
                    crate::renderer::ScrollbarRects::hit(track, panel_x + panel_w, mx, my)
                } else {
                    false
                };
                let close_hover = if self.file_explorer.is_filtering() {
                    let (cx, cy, cw, ch) = crate::explorer::FileExplorer::filter_close_btn_rect(
                        panel_x, panel_y, panel_w,
                    );
                    mx >= cx && mx < cx + cw && my >= cy && my < cy + ch
                } else {
                    false
                };
                if self.file_explorer.hovered_filter_close != close_hover {
                    self.file_explorer.hovered_filter_close = close_hover;
                    needs_redraw = true;
                }

                is_pointer |= is_over_scrollbar || in_expanded_item || close_hover;

                let item_hover = if in_expanded_item {
                    self.file_explorer.hovered_index
                } else if is_over_scrollbar {
                    None
                } else {
                    self.file_explorer.item_at_y(my, panel_y)
                };

                let git_action_hover = if self.file_explorer.changes_only {
                    if let Some(idx) = item_hover {
                        if let Some(item) = self.file_explorer.visible_item(idx) {
                            if let Some(item_y) = self.file_explorer.item_y(idx, panel_y) {
                                let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
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
                                .map(|act| (idx, act))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if item_hover.is_some() || git_action_hover.is_some() {
                    is_pointer = true;
                }

                if self.file_explorer.hovered_index != item_hover
                    || self.file_explorer.hovered_scrollbar != is_over_scrollbar
                    || self.file_explorer.hovered_git_action != git_action_hover
                {
                    self.file_explorer.hovered_index = item_hover;
                    self.file_explorer.hovered_scrollbar = is_over_scrollbar;
                    self.file_explorer.hovered_git_action = git_action_hover;
                    needs_redraw = true;
                }
            } else if self.file_explorer.hovered_index.is_some()
                || self.file_explorer.hovered_scrollbar
                || self.file_explorer.hovered_filter_close
                || self.file_explorer.hovered_git_action.is_some()
            {
                self.file_explorer.hovered_index = None;
                self.file_explorer.hovered_scrollbar = false;
                self.file_explorer.hovered_filter_close = false;
                self.file_explorer.hovered_git_action = None;
                needs_redraw = true;
            }
        }

        // Coordenadas relativas a paneles en la terminal activa
        if !is_settings_active && self.tabs[self.active_tab_index].file_viewer.is_none() {
            let bypass = self.modifiers.shift_key();
            let pane_rects = self.active_tab_pane_rects(win_w, win_h);
            let active_tab = &mut self.tabs[self.active_tab_index];
            let active_pane_id = active_tab.active_pane_id;

            // Botones de la cabecera de cada panel dividido (dividir · maximizar · cerrar)
            let pane_btn = pane_rects.iter().find_map(|rect| {
                if !rect.has_header {
                    return None;
                }
                let header = PaneHeaderRects::compute(rect.x, rect.slot_y, rect.width);
                header
                    .button_at(mx, my)
                    .map(|button| (rect.pane_id, button))
            });
            self.hovered_pane_header = pane_rects.iter().find_map(|rect| {
                let header = PaneHeaderRects::compute(rect.x, rect.slot_y, rect.width);
                (rect.has_header && PaneHeaderRects::inside(mx, my, header.header))
                    .then_some(rect.pane_id)
            });
            if pane_btn.is_some() {
                is_pointer = true;
            }

            let hovered_pane_rect = pane_rects
                .iter()
                .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height)
                .copied();

            let hovered_pid = hovered_pane_rect.map(|r| r.pane_id);
            for p in &mut active_tab.panes {
                if Some(p.id) != hovered_pid {
                    if p.terminal.scrollbar_hovered {
                        p.terminal.scrollbar_hovered = false;
                        needs_redraw = true;
                    }
                    if p.terminal.scroll_to_bottom_hovered {
                        p.terminal.scroll_to_bottom_hovered = false;
                        needs_redraw = true;
                    }
                }
            }

            if let Some(rect) = hovered_pane_rect {
                if let Some(pane) = active_tab.find_pane_mut(rect.pane_id) {
                    let is_over_scrollbar = if let Some(renderer) = &self.renderer {
                        if let Some((_tx, ty, _tw, th)) = renderer.pane_scrollbar_geometry(
                            rect.x,
                            rect.y,
                            rect.width,
                            rect.height,
                            &pane.terminal,
                        ) {
                            let scroll_margin = 16;
                            mx >= (rect.x + rect.width).saturating_sub(scroll_margin)
                                && my >= ty
                                && my < ty + th
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    let is_over_scroll_to_bottom =
                        if pane.terminal.scroll_offset > 0 && !pane.terminal.is_alt_screen {
                            let (bx, by, bw, bh) = crate::renderer::pane_scroll_to_bottom_btn_rect(
                                rect.x,
                                rect.y,
                                rect.width,
                                rect.height,
                            );
                            mx >= bx && mx < bx + bw && my >= by && my < by + bh
                        } else {
                            false
                        };

                    if is_over_scroll_to_bottom {
                        is_pointer = true;
                        if !pane.terminal.scroll_to_bottom_hovered {
                            pane.terminal.scroll_to_bottom_hovered = true;
                            needs_redraw = true;
                        }
                    } else if pane.terminal.scroll_to_bottom_hovered {
                        pane.terminal.scroll_to_bottom_hovered = false;
                        needs_redraw = true;
                    }

                    if is_over_scrollbar {
                        is_pointer = true;
                        if !pane.terminal.scrollbar_hovered {
                            pane.terminal.scrollbar_hovered = true;
                            needs_redraw = true;
                        }
                    } else {
                        if pane.terminal.scrollbar_hovered {
                            pane.terminal.scrollbar_hovered = false;
                            needs_redraw = true;
                        }
                        if !pane.terminal.is_mouse_reporting_active() || bypass {
                            let mut is_link = false;
                            if self.modifiers.control_key() {
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
                                    if let Some((_, sc, ec)) =
                                        pane.terminal.url_and_span_at(col, global_line)
                                    {
                                        new_url_span = Some((pane.id, global_line, sc, ec));
                                        is_link = true;
                                    }
                                }
                            }
                            if is_link {
                                is_pointer = true;
                            } else {
                                is_text = true;
                            }
                        }
                    }

                    if pane.terminal.is_mouse_reporting_active() && !bypass {
                        if (pane.terminal.mouse_reporting_button_event && self.mouse_down)
                            || pane.terminal.mouse_reporting_any_event
                        {
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

                                let mut code = 32;
                                if self.modifiers.shift_key() {
                                    code += 4;
                                }
                                if self.modifiers.alt_key() {
                                    code += 8;
                                }
                                if self.modifiers.control_key() {
                                    code += 16;
                                }

                                if pane.terminal.mouse_reporting_sgr {
                                    let seq = format!("\x1b[<{};{};{}M", code, col, row);
                                    pane.pty.write_input(seq.as_bytes());
                                } else {
                                    let seq = [
                                        0x1b,
                                        b'[',
                                        b'M',
                                        32 + code as u8,
                                        32 + col.min(223) as u8,
                                        32 + row.min(223) as u8,
                                    ];
                                    pane.pty.write_input(&seq);
                                }
                            }
                        }
                    } else if self.mouse_down && rect.pane_id == active_pane_id {
                        if let Some(renderer) = &self.renderer {
                            let term_x = mx.saturating_sub(rect.x);
                            let term_y = my.saturating_sub(rect.y);
                            let col =
                                term_x.saturating_sub(renderer.margin_x) / renderer.char_width;
                            let col = col.min(pane.terminal.cols.saturating_sub(1));

                            // Auto-scroll si el cursor se arrastra por encima o por debajo del panel
                            let mut scrolled = false;
                            if my < rect.y + renderer.margin_y
                                && pane.terminal.scroll_offset < pane.terminal.scrollback.len()
                            {
                                pane.terminal.scroll_up(1);
                                scrolled = true;
                            } else if my >= rect.y + rect.height && pane.terminal.scroll_offset > 0
                            {
                                pane.terminal.scroll_down(1);
                                scrolled = true;
                            }
                            if scrolled {
                                self.last_scroll_time = Instant::now();
                            }

                            let row_in_view =
                                term_y.saturating_sub(renderer.margin_y) / renderer.row_height;
                            let row_in_view = row_in_view.min(pane.terminal.rows.saturating_sub(1));
                            let global_line = pane.terminal.visible_line_start() + row_in_view;

                            let new_sel = if let Some(anchor) = self.drag_anchor {
                                // Tras doble/triple clic: extiende por palabra/línea.
                                pane.terminal.selection = Some(anchor);
                                if self.terminal_click_count >= 3 {
                                    pane.terminal.select_line_at(global_line);
                                } else {
                                    pane.terminal.select_word_at(col, global_line);
                                }
                                pane.terminal
                                    .selection
                                    .map(|unit| {
                                        let key = |p: (usize, usize)| (p.1, p.0);
                                        let (a, b) = (anchor.start, anchor.end);
                                        let (u0, u1) = (unit.start, unit.end);
                                        if key(u0) < key(a) {
                                            SelectionRange { start: u0, end: b }
                                        } else {
                                            let end = if key(u1) > key(b) { u1 } else { b };
                                            SelectionRange { start: a, end }
                                        }
                                    })
                                    .or_else(|| {
                                        pane.terminal.selection = Some(anchor);
                                        None
                                    })
                            } else {
                                self.drag_start.map(|start| SelectionRange {
                                    start,
                                    end: (col, global_line),
                                })
                            };
                            if let Some(new_sel) = new_sel {
                                let new_sel = Some(new_sel);
                                if pane.terminal.selection != new_sel {
                                    pane.terminal.selection = new_sel;
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            self.hovered_pane_btn = pane_btn;
        }

        // Si la pestaña activa es un visor de archivos, gestionar arrastre de selección y cursor de texto (I-Beam)
        if !is_settings_active && self.tabs[self.active_tab_index].file_viewer.is_some() {
            let offset = self.sidebar_offset();
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let right_offset = self.file_explorer_offset();
            let viewer_w = win_w.saturating_sub(offset + right_offset);
            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
            let content_y = top_offset + crate::renderer::FileViewerLayout::HEADER_H;
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let char_w = self.renderer.as_ref().map(|r| r.char_width).unwrap_or(8);

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

                let is_hover_diff_mode = header_rects
                    .diff_mode_btn
                    .map(|r| crate::renderer::FileViewerHeaderRects::inside(mx, my, r))
                    .unwrap_or(false);
                if viewer.hovered_diff_mode_btn != is_hover_diff_mode {
                    viewer.hovered_diff_mode_btn = is_hover_diff_mode;
                    needs_redraw = true;
                }
                let is_hover_diff = header_rects
                    .diff_btn
                    .map(|r| crate::renderer::FileViewerHeaderRects::inside(mx, my, r))
                    .unwrap_or(false);
                if viewer.hovered_diff_btn != is_hover_diff {
                    viewer.hovered_diff_btn = is_hover_diff;
                    needs_redraw = true;
                }
                let is_hover_mode =
                    crate::renderer::FileViewerHeaderRects::inside(mx, my, header_rects.mode_btn);
                let is_hover_md = header_rects
                    .md_btn
                    .map(|r| crate::renderer::FileViewerHeaderRects::inside(mx, my, r))
                    .unwrap_or(false);

                if is_hover_mode || is_hover_md || is_hover_diff || is_hover_diff_mode {
                    is_pointer = true;
                }
                if viewer.hovered_mode_btn != is_hover_mode {
                    viewer.hovered_mode_btn = is_hover_mode;
                    needs_redraw = true;
                }
                if viewer.hovered_md_btn != is_hover_md {
                    viewer.hovered_md_btn = is_hover_md;
                    needs_redraw = true;
                }

                let is_over_viewer_scrollbar = if let Some((track, _)) =
                    TerminalRenderer::file_viewer_scrollbar_geometry(
                        offset, top_offset, viewer_w, viewer_h, viewer, row_h,
                    ) {
                    crate::renderer::ScrollbarRects::hit(track, offset + viewer_w, mx, my)
                } else {
                    false
                };
                if is_over_viewer_scrollbar {
                    is_pointer = true;
                }
                if viewer.hovered_scrollbar != is_over_viewer_scrollbar {
                    viewer.hovered_scrollbar = is_over_viewer_scrollbar;
                    needs_redraw = true;
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

                let is_over_search_bar = if viewer.search_open {
                    let s_rects = crate::renderer::FileViewerSearchRects::compute(&lay);
                    let hov_prev =
                        crate::renderer::FileViewerSearchRects::inside(mx, my, s_rects.prev_btn);
                    let hov_next =
                        crate::renderer::FileViewerSearchRects::inside(mx, my, s_rects.next_btn);
                    let hov_close =
                        crate::renderer::FileViewerSearchRects::inside(mx, my, s_rects.close_btn);
                    let is_over_input =
                        crate::renderer::FileViewerSearchRects::inside(mx, my, s_rects.input);

                    if viewer.hovered_search_prev != hov_prev {
                        viewer.hovered_search_prev = hov_prev;
                        needs_redraw = true;
                    }
                    if viewer.hovered_search_next != hov_next {
                        viewer.hovered_search_next = hov_next;
                        needs_redraw = true;
                    }
                    if viewer.hovered_search_close != hov_close {
                        viewer.hovered_search_close = hov_close;
                        needs_redraw = true;
                    }

                    if hov_prev || hov_next || hov_close {
                        is_pointer = true;
                    } else if is_over_input {
                        is_text = true;
                    }

                    crate::renderer::FileViewerSearchRects::inside(mx, my, s_rects.bar)
                } else {
                    false
                };

                if viewer.search_open && viewer.search_input.is_dragging {
                    let s_rects = crate::renderer::FileViewerSearchRects::compute(&lay);
                    let char_idx = if let Some(r) = &self.renderer {
                        viewer.search_input.hit_test(mx, s_rects.text_start_x, |s| {
                            r.measure_ui_text(s, crate::renderer::tokens::BODY, true)
                        })
                    } else {
                        viewer.search_input.len()
                    };
                    viewer.search_input.mouse_drag(char_idx);
                    self.cursor_on = true;
                    self.cursor_timer = std::time::Instant::now();
                    needs_redraw = true;
                }
                let gutter_w = lay.gutter_w;
                let over_gutter =
                    mx >= offset && mx < lay.sep_x && my >= content_y && my < top_offset + viewer_h;
                if viewer.hovered_gutter != over_gutter {
                    viewer.hovered_gutter = over_gutter;
                    needs_redraw = true;
                }

                let over_hscroll = lay
                    .hscrollbar(viewer.max_scroll_x(), viewer.scroll_offset_x)
                    .is_some_and(|(track, _)| {
                        mx >= track.0
                            && mx < track.0 + track.2
                            && my + 4 >= track.1
                            && my < track.1 + track.3 + 4
                    });
                if over_hscroll {
                    is_pointer = true;
                }
                if viewer.hovered_hscrollbar != over_hscroll {
                    viewer.hovered_hscrollbar = over_hscroll;
                    needs_redraw = true;
                }

                if self.viewer_drag_selecting && !viewer.is_binary && !viewer.lines.is_empty() {
                    let visible_lines = lay.content_h / lay.row_h;
                    let total_rows = viewer.rows().len();

                    if my < content_y && viewer.scroll_offset > 0 {
                        viewer.scroll_offset = viewer.scroll_offset.saturating_sub(1);
                    } else if my >= top_offset + viewer_h {
                        let max_scroll = total_rows.saturating_sub(visible_lines);
                        if viewer.scroll_offset < max_scroll {
                            viewer.scroll_offset += 1;
                        }
                    }

                    let clicked_row = viewer.scroll_offset
                        + if my < content_y {
                            0
                        } else {
                            (my - content_y) / row_h.max(16)
                        };
                    let (target_line, target_col) = viewer
                        .hit_test(clicked_row, mx, &lay)
                        .unwrap_or((viewer.cursor_line, viewer.cursor_col));

                    viewer.cursor_line = target_line;
                    viewer.cursor_col = target_col;

                    if let Some(start) = self.viewer_drag_start {
                        viewer.selection = Some((start, (target_line, target_col)));
                    }
                    let content_h =
                        viewer_h.saturating_sub(crate::renderer::FileViewerLayout::HEADER_H);
                    let visible_lines = (content_h / row_h.max(16)).max(1);
                    viewer.ensure_cursor_visible(visible_lines);
                    needs_redraw = true;
                }

                if !is_over_search_bar
                    && !viewer.is_binary
                    && mx >= offset
                    && mx < offset + viewer_w
                    && my >= content_y
                    && my < top_offset + viewer_h
                {
                    // Si no está sobre la barra de desplazamiento lateral derecha
                    let scrollbar_margin = 16;
                    let text_max_x = (offset + viewer_w).saturating_sub(scrollbar_margin);
                    if mx >= (offset + gutter_w) && mx < text_max_x {
                        let clicked_row = viewer.scroll_offset + (my - content_y) / row_h.max(16);
                        let is_link = if viewer.render_markdown || self.modifiers.control_key() {
                            viewer
                                .hit_test(clicked_row, mx, &lay)
                                .and_then(|(l, c)| viewer.link_at(l, c))
                                .is_some()
                        } else {
                            false
                        };
                        if is_link {
                            is_pointer = true;
                        } else {
                            is_text = true;
                        }
                    }
                }
            }
        }

        // Hover en el indicador de modelo/agente de IA, actualización y modo keep-awake en la barra de estado
        let (new_hovered_ai, new_hovered_update, new_hovered_keep_awake) = if self
            .config
            .appearance
            .show_status_bar
            && !is_settings_active
        {
            let active_tab = &self.tabs[self.active_tab_index];
            let (ssh_host, ai_status, dims) = if let Some(ref viewer) = active_tab.file_viewer {
                (
                    viewer.remote_destination.as_deref(),
                    AiAgentStatus::None,
                    (0, viewer.lines.len()),
                )
            } else if let Some(pane) = active_tab
                .panes
                .iter()
                .find(|p| p.id == active_tab.active_pane_id)
            {
                let s_host = pane
                    .terminal
                    .ssh_session
                    .as_ref()
                    .filter(|s| s.is_active)
                    .map(|s| s.destination.as_str());
                let ai = pane.terminal.active_ai_agent();
                (s_host, ai, (pane.terminal.cols, pane.terminal.rows))
            } else {
                (None, AiAgentStatus::None, (100, 30))
            };
            let active_model = self.ai_usage.snapshot.model_for(ai_status);
            let badge_info =
                TerminalRenderer::status_bar_badge(ai_status, ssh_host, active_model, &self.theme);
            let (_, label, _) = &badge_info;
            let label_w = self
                .renderer
                .as_ref()
                .map(|r| r.measure_ui_text(label, 10.5, true))
                .unwrap_or(label.len() * 7);

            let update_info = crate::updater::get_available_update();
            let update_text = update_info
                .as_ref()
                .map(|u| format!("v{} disponible", u.latest_version));
            let update_w = update_text.as_ref().map(|t| {
                self.renderer
                    .as_ref()
                    .map(|r| r.measure_mono_text(t, 10.5, false))
                    .unwrap_or(t.len() * 7)
            });

            // Las dimensiones y los medidores van a la derecha del badge: misma geometría que el dibujo.
            let dims_w = if self.config.appearance.show_dimensions {
                let dims_text = format!("{} × {}", dims.0, dims.1);
                self.renderer.as_ref().map_or(dims_text.len() * 6, |r| {
                    r.measure_mono_text(&dims_text, 10.0, false)
                })
            } else {
                0
            };
            let meters = TerminalRenderer::status_meters_for(&self.ai_usage.snapshot, ai_status);
            let meters_w = self
                .renderer
                .as_ref()
                .map_or(meters.len() * 150, |r| r.status_meters_width(&meters));
            let keep_awake_w = self.renderer.as_ref().map_or(65, |r| {
                r.keep_awake_button_width(self.config.terminal.keep_awake)
            });

            let sb_rects = StatusBarRects::compute_full(
                win_w,
                win_h,
                label_w,
                update_w,
                dims_w,
                meters_w,
                keep_awake_w,
            );
            let update_hov = sb_rects
                .update_badge
                .is_some_and(|r| StatusBarRects::inside(mx, my, r));
            let keep_awake_hov = StatusBarRects::inside(mx, my, sb_rects.keep_awake_btn);
            (false, update_hov, keep_awake_hov)
        } else {
            (false, false, false)
        };

        if new_hovered_ai {
            is_pointer = true;
            let just_entered = !self.hovered_ai_badge;
            let active_tab = &self.tabs[self.active_tab_index];
            let (term_ref, agents) = if active_tab.file_viewer.is_some() {
                (None, Vec::new())
            } else if let Some(pane) = active_tab
                .panes
                .iter()
                .find(|p| p.id == active_tab.active_pane_id)
            {
                (Some(&pane.terminal), active_agents(active_tab))
            } else {
                (None, Vec::new())
            };
            if self.ai_usage.refresh_if_needed(term_ref, just_entered) {
                self.spawn_ai_local_scan(agents);
            }
        }
        if new_hovered_update || new_hovered_keep_awake {
            is_pointer = true;
        }
        let new_ka_option = if self.keep_awake_popup_open
            && self.config.appearance.show_status_bar
            && !is_settings_active
        {
            StatusBarRects::keep_awake_option_at(win_w, win_h, mx, my)
        } else {
            None
        };
        if new_ka_option.is_some() {
            is_pointer = true;
        }
        if self.hovered_keep_awake_option != new_ka_option {
            self.hovered_keep_awake_option = new_ka_option;
            needs_redraw = true;
        }
        if self.hovered_ai_badge != new_hovered_ai {
            self.hovered_ai_badge = new_hovered_ai;
            needs_redraw = true;
        }
        if self.hovered_update_badge != new_hovered_update {
            self.hovered_update_badge = new_hovered_update;
            needs_redraw = true;
        }
        if self.hovered_keep_awake != new_hovered_keep_awake {
            self.hovered_keep_awake = new_hovered_keep_awake;
            needs_redraw = true;
        }

        if self.hovered_win_min != new_hovered_min {
            self.hovered_win_min = new_hovered_min;
            needs_redraw = true;
        }
        if self.hovered_win_max != new_hovered_max {
            self.hovered_win_max = new_hovered_max;
            needs_redraw = true;
        }
        if self.hovered_win_close != new_hovered_close_win {
            self.hovered_win_close = new_hovered_close_win;
            needs_redraw = true;
        }
        if self.hovered_pane_btn != previous_pane_btn
            || self.hovered_pane_header != previous_pane_header
        {
            needs_redraw = true;
        }

        // Cursor de redimensionamiento en los bordes (solo en ventana flotante, no maximizada ni pantalla completa).
        let can_resize = self
            .window
            .as_ref()
            .is_some_and(|w| !w.is_maximized() && w.fullscreen().is_none());
        let border_resize_cursor = if can_resize {
            use winit::window::ResizeDirection;
            self.compute_border_resize_direction(mx, my, win_w, win_h)
                .map(|dir| match dir {
                    ResizeDirection::NorthEast | ResizeDirection::SouthWest => {
                        winit::window::CursorIcon::NeswResize
                    }
                    ResizeDirection::NorthWest | ResizeDirection::SouthEast => {
                        winit::window::CursorIcon::NwseResize
                    }
                    ResizeDirection::West | ResizeDirection::East => {
                        winit::window::CursorIcon::EwResize
                    }
                    ResizeDirection::North | ResizeDirection::South => {
                        winit::window::CursorIcon::NsResize
                    }
                })
        } else {
            None
        };

        let divider_cursor = self.divider_at(mx, my).map(|d| d.cursor_icon());
        let explorer_edge_cursor = (!is_settings_active
            && self.on_panel_resize_edge(mx, my, win_w, win_h))
        .then_some(winit::window::CursorIcon::ColResize);
        let cursor_icon = if let Some(resize_cur) = border_resize_cursor
            .or(explorer_edge_cursor)
            .or(divider_cursor)
        {
            resize_cur
        } else if is_pointer {
            winit::window::CursorIcon::Pointer
        } else if is_text {
            winit::window::CursorIcon::Text
        } else {
            winit::window::CursorIcon::Default
        };
        if self.hovered_url_span != new_url_span {
            self.hovered_url_span = new_url_span;
            needs_redraw = true;
        }
        self.set_cursor_icon(cursor_icon);
        if needs_redraw {
            self.request_redraw();
        }
    }

    fn handle_scrollbar_drag(
        &mut self,
        drag: ScrollbarDragState,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) {
        match drag.target {
            ScrollbarTarget::Terminal(pid) => {
                let (cx, cy, cw, ch) = self.content_area_rect(win_w, win_h);
                let active_tab = &mut self.tabs[self.active_tab_index];
                let pane_rects = active_tab.compute_pane_rects(cx, cy, cw, ch);
                if let Some(target_rect) = pane_rects.iter().find(|r| r.pane_id == pid) {
                    if let Some(pane) = active_tab.find_pane_mut(pid) {
                        if let Some(renderer) = &self.renderer {
                            if !pane.terminal.scrollback.is_empty() {
                                let track_y = target_rect.y + renderer.margin_y;
                                let track_h =
                                    target_rect.height.saturating_sub(renderer.margin_y * 2);
                                let total_history =
                                    pane.terminal.scrollback.len() + pane.terminal.rows;
                                let thumb_h = ((pane.terminal.rows as f32 / total_history as f32)
                                    * track_h as f32)
                                    .max(24.0)
                                    .min(track_h as f32)
                                    as usize;
                                let available_travel = track_h.saturating_sub(thumb_h);
                                if available_travel > 0 {
                                    let target_thumb_top =
                                        (my as f32 - drag.grab_offset - track_y as f32)
                                            .clamp(0.0, available_travel as f32);
                                    let ratio = target_thumb_top / available_travel as f32;
                                    let new_offset = ((1.0 - ratio)
                                        * pane.terminal.scrollback.len() as f32)
                                        .round()
                                        as usize;
                                    pane.terminal.scroll_offset =
                                        new_offset.min(pane.terminal.scrollback.len());
                                    self.last_scroll_time = Instant::now();
                                    self.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            ScrollbarTarget::Explorer => {
                let top_offset = self.top_offset();
                let status_bar_h = self.status_bar_height();
                if let Some((track, thumb)) = TerminalRenderer::explorer_scrollbar_geometry(
                    win_w,
                    win_h,
                    top_offset,
                    status_bar_h,
                    &self.file_explorer,
                ) {
                    let (_panel_x, panel_y, _panel_w, panel_h) =
                        self.file_explorer
                            .panel_geometry(win_w, win_h, top_offset, status_bar_h);
                    let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
                    let sub_h = self.file_explorer.subheader_height();
                    let list_y = panel_y + header_h + sub_h + 4;
                    let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
                    let list_max_y = panel_y + panel_h.saturating_sub(6);
                    let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
                    let max_offset = self
                        .file_explorer
                        .visible_item_count()
                        .saturating_sub(visible_items);
                    if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                        track,
                        thumb,
                        my as f32 - drag.grab_offset,
                        max_offset,
                    ) {
                        self.file_explorer.scroll_offset = off;
                        self.last_scroll_time = Instant::now();
                        self.request_redraw();
                    }
                }
            }
            ScrollbarTarget::ViewerVertical => {
                let (cx, cy, cw, ch) = self.content_area_rect(win_w, win_h);
                let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);

                if let Some(ref mut tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut viewer) = tab.file_viewer {
                        if let Some((track, thumb)) =
                            TerminalRenderer::file_viewer_scrollbar_geometry(
                                cx, cy, cw, ch, viewer, row_h,
                            )
                        {
                            let content_h = ch.saturating_sub(38);
                            let visible_lines = (content_h / row_h.max(16)).max(1);
                            let max_offset =
                                viewer.total_display_rows().saturating_sub(visible_lines);

                            if let Some(off) = crate::renderer::scroll_offset_for_thumb_y(
                                track,
                                thumb,
                                my as f32 - drag.grab_offset,
                                max_offset,
                            ) {
                                viewer.scroll_offset = off;
                                self.last_scroll_time = Instant::now();
                                self.request_redraw();
                            }
                        }
                    }
                }
            }
            ScrollbarTarget::ViewerHorizontal => {
                let offset = self.sidebar_offset();
                let top_offset = self.top_offset();
                let viewer_w = win_w.saturating_sub(offset + self.file_explorer_offset());
                let viewer_h = win_h.saturating_sub(top_offset + self.status_bar_height());
                let (row_h, char_w) = self
                    .renderer
                    .as_ref()
                    .map(|r| (r.row_height, r.char_width))
                    .unwrap_or((20, 8));
                let grab = drag.grab_offset;
                if let Some(viewer) = self
                    .tabs
                    .get_mut(self.active_tab_index)
                    .and_then(|t| t.file_viewer.as_mut())
                {
                    let lay = crate::renderer::FileViewerLayout::compute(
                        offset,
                        top_offset,
                        viewer_w,
                        viewer_h,
                        row_h,
                        char_w,
                        viewer.lines.len(),
                    );
                    let max_x = viewer.max_scroll_x();
                    if let Some((track, thumb)) = lay.hscrollbar(max_x, viewer.scroll_offset_x) {
                        let travel = track.2.saturating_sub(thumb.2);
                        if travel > 0 {
                            let thumb_left =
                                (mx as f32 - grab - track.0 as f32).clamp(0.0, travel as f32);
                            viewer.scroll_offset_x =
                                ((thumb_left / travel as f32 * max_x as f32).round() as usize)
                                    .min(max_x);
                            self.request_redraw();
                        }
                    }
                }
            }
            ScrollbarTarget::Settings => {
                let modal_y = self.settings_modal().1 as f32;
                let top = (my as f32 - modal_y - drag.grab_offset).max(0.0) as usize;
                self.settings_scroll_to_thumb(top);
                self.request_redraw();
            }
            ScrollbarTarget::Palette => {
                let top = (my as f32 - drag.grab_offset).max(0.0) as usize;
                self.palette_scroll_to_thumb(top);
                self.last_scroll_time = Instant::now();
                self.request_redraw();
            }
        }
    }

    fn cursor_handle_active_drags(
        &mut self,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        // Manejo de arrastre activo de la barra de desplazamiento
        if let Some(drag) = self.scrollbar_drag {
            self.handle_scrollbar_drag(drag, mx, my, win_w, win_h);
            self.set_cursor_icon(winit::window::CursorIcon::Pointer);
            return true;
        }

        // Arrastre del borde del explorador / panel: el PTY se redimensiona al soltar o al detenerse
        if self.explorer_resizing {
            self.resize_panel_from_mouse(mx, win_w);
            self.schedule_pty_resize();
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::ColResize);
                w.request_redraw();
            }
            return true;
        }

        // Arrastre de un divisor entre paneles
        if let Some(icon) = self.divider_drag.as_ref().map(Divider::cursor_icon) {
            self.drag_divider(mx, my);
            if let Some(w) = &self.window {
                w.set_cursor(icon);
            }
            return true;
        }

        // Reordenar pestañas: la arrastrada salta a la posición bajo el ratón solo dentro de la barra
        if let Some(from) = self.tab_drag {
            let in_tab_bar = my < TopTabBarRects::HEIGHT + 10 && mx < win_w;
            if in_tab_bar {
                if let Some(to) = self.tab_slot_at(mx, my).filter(|&to| to != from) {
                    self.move_tab(from, to);
                }
            }
        }

        // Arrastrar archivo o carpeta desde el explorador hacia la terminal
        if let Some(drag) = &mut self.explorer_drag {
            let dx = (mx as isize - drag.start_pos.0 as isize).abs();
            let dy = (my as isize - drag.start_pos.1 as isize).abs();
            if dx > 4 || dy > 4 {
                drag.is_dragging = true;
            }
            if drag.is_dragging {
                let panel_x = win_w.saturating_sub(self.panel_width());
                if mx < panel_x {
                    self.set_cursor_icon(winit::window::CursorIcon::Copy);
                } else {
                    self.set_cursor_icon(winit::window::CursorIcon::Default);
                }
                return true;
            }
        }

        false
    }

    fn cursor_handle_overlays(&mut self, mx: usize, my: usize, win_w: usize, win_h: usize) -> bool {
        // Con el menú contextual abierto solo se resalta su opción bajo el ratón
        if let Some(menu) = &mut self.context_menu {
            let hovered = menu.item_at(mx, my);
            if menu.hovered != hovered {
                menu.hovered = hovered;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            if let Some(w) = &self.window {
                w.set_cursor(winit::window::CursorIcon::Default);
            }
            return true;
        }

        // Con el modal «Añadir máquina» abierto solo se resalta su elemento bajo el ratón
        if let Some(rects) = self.machine_rects() {
            let hit = rects.hit(mx, my);
            let mut needs_redraw = false;
            if let Some(m) = &mut self.machine_modal {
                if m.hovered != hit {
                    m.hovered = hit;
                    needs_redraw = true;
                }
            }
            if needs_redraw {
                self.request_redraw();
            }
            self.set_cursor_icon(match hit {
                Some(crate::renderer::MachineHover::Input) => winit::window::CursorIcon::Text,
                Some(_) => winit::window::CursorIcon::Pointer,
                None => winit::window::CursorIcon::Default,
            });
            return true;
        }

        // Con la paleta de comandos abierta solo se resalta su opción bajo el ratón
        if self.palette.is_some() {
            let (is_dragging, filtered_len, scroll_offset, selected) = {
                let p = self.palette.as_ref().unwrap();
                (
                    p.input.is_dragging,
                    p.filtered.len(),
                    p.scroll_offset,
                    p.selected,
                )
            };
            let rects =
                PaletteRects::compute_with_scroll(win_w, win_h, filtered_len, scroll_offset);
            if is_dragging {
                let text_start_x = rects.input.0 + 40;
                if let Some(p) = &mut self.palette {
                    let mut input = std::mem::take(&mut p.input);
                    self.handle_text_input_drag(&mut input, mx, text_start_x);
                    if let Some(p) = &mut self.palette {
                        p.input = input;
                    }
                }
            }
            let is_scrollbar_hovered = rects.scrollbar.is_some_and(|(track, _)| {
                crate::renderer::ScrollbarRects::hit(track, rects.card.0 + rects.card.2, mx, my)
            });
            let hovered_item = rects.item_at_with_offset(mx, my, scroll_offset);
            if let Some(idx) = hovered_item {
                if selected != idx {
                    if let Some(p) = &mut self.palette {
                        p.selected = idx;
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            if let Some(w) = &self.window {
                let cursor = if hovered_item.is_some() || is_scrollbar_hovered {
                    winit::window::CursorIcon::Pointer
                } else if PaletteRects::inside(mx, my, rects.input) {
                    winit::window::CursorIcon::Text
                } else {
                    winit::window::CursorIcon::Default
                };
                w.set_cursor(cursor);
            }
            return true;
        }

        false
    }
}
