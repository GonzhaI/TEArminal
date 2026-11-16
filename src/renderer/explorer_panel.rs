//! Panel flotante del explorador de archivos.

use super::*;

impl TerminalRenderer {
    /// Renderiza el panel lateral derecho del explorador de archivos con estilo overlay translúcido (Floating Island)
    pub(crate) fn render_file_explorer(
        &mut self,
        canvas: &mut Canvas,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 || !explorer.visible {
            return;
        }

        let (panel_x, panel_y, panel_w, panel_h) =
            explorer.panel_geometry(width, height, top_offset, status_bar_h);
        if panel_w == 0 || panel_h < 80 {
            return;
        }

        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        if explorer.dock.is_some() {
            // Fondo mantle del área de contenido del panel lateral contextual
            let content_y = panel_y + header_h;
            let content_h = panel_h.saturating_sub(header_h);
            Self::draw_rect(
                canvas,
                panel_x,
                content_y,
                panel_w,
                content_h,
                theme.bar_bg(),
            );
            Self::draw_rect(
                canvas,
                panel_x + panel_w - 1,
                content_y,
                1,
                content_h,
                theme.chrome_line(),
            );

            if explorer.changes_only {
                self.draw_git_panel(
                    canvas,
                    panel_x,
                    content_y,
                    panel_w,
                    content_h,
                    explorer,
                    cursor_visible,
                    theme,
                );
                return;
            }
        } else {
            // Superficie acoplada (standalone): fondo de chrome opaco y hairline izquierdo
            Self::draw_rect(
                canvas,
                panel_x,
                panel_y,
                panel_w,
                panel_h,
                theme.chrome_bg(),
            );
            let edge_color = theme.chrome_line();
            Self::draw_rect(canvas, panel_x, panel_y, 1, panel_h, edge_color);

            // Encabezado standalone
            Self::draw_rect(
                canvas,
                panel_x + 8,
                panel_y + header_h - 1,
                panel_w.saturating_sub(16),
                1,
                theme.chrome_line(),
            );
            let title = if explorer.changes_only {
                "Control de código"
            } else {
                "Archivos"
            };
            let baseline = self.centered_baseline(panel_y + (header_h - 1) / 2);
            let right = panel_x + panel_w - 16;
            let title_x = panel_x + 16;
            let cursor_x = self.draw_ui_text(
                canvas,
                title,
                title_x,
                baseline,
                right,
                theme.foreground,
                tokens::TITLE,
                true,
            );
            let change_count = explorer.change_count();
            if explorer.changes_only && change_count > 0 {
                let count = change_count.to_string();
                let cw = self.measure_mono_text(&count, tokens::MICRO, false);
                canvas.rounded_fill(
                    cursor_x + 8,
                    panel_y + (header_h - 1 - 18) / 2,
                    cw + 12,
                    18,
                    tokens::RADIUS_ROW,
                    theme.surface_bg(),
                );
                self.draw_mono_text(
                    canvas,
                    &count,
                    cursor_x + 14,
                    baseline,
                    right,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
            }

            if explorer.changes_only {
                self.draw_git_panel(
                    canvas,
                    panel_x,
                    panel_y + header_h,
                    panel_w,
                    panel_h.saturating_sub(header_h),
                    explorer,
                    cursor_visible,
                    theme,
                );
                return;
            }
        }

        // 2b. Barra de filtro rápido (`Ctrl + P` / `Ctrl + F`) si está activa
        self.draw_explorer_filter_bar(canvas, explorer, panel_x, panel_y, panel_w, theme);

        // 3. Lista de elementos (archivos y carpetas), justo bajo la cabecera y el filtro.
        let list_y = panel_y
            + header_h
            + explorer.subheader_height()
            + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let remote_footer_h = if explorer.remote_session.is_some() && panel_h > 140 {
            44
        } else {
            0
        };
        let list_max_y = panel_y + panel_h.saturating_sub(8 + remote_footer_h);

        if explorer.visible_item_count() == 0 {
            let empty_base = list_y + 14;
            let (msg, color) = if let Some(ref err) = explorer.error_message {
                (err.as_str(), theme.ansi[3])
            } else if explorer.is_filtering() {
                ("No se encontraron coincidencias", theme.text_muted())
            } else if explorer.changes_only {
                // Sin cambios: la pestaña queda vacía, sin mensaje.
                return;
            } else if explorer.remote_session.is_some() {
                ("Cargando archivos remotos...", theme.ansi[8])
            } else {
                ("(Directorio vacío)", theme.ansi[8])
            };
            self.draw_ui_text(
                canvas,
                msg,
                panel_x + 24,
                empty_base,
                panel_x + panel_w - 24,
                color,
                tokens::DETAIL,
                false,
            );
            self.draw_explorer_edit_bar(canvas, explorer, panel_x, panel_y, panel_w, theme);
            return;
        }

        let visible_count = explorer.visible_item_count();
        for rel_idx in 0..visible_count.saturating_sub(explorer.scroll_offset) {
            let actual_idx = explorer.scroll_offset + rel_idx;
            let Some(item) = explorer.visible_item(actual_idx) else {
                break;
            };
            let item_y = list_y + rel_idx * item_h;
            if item_y + item_h > list_max_y {
                break;
            }

            let is_hovered = explorer.hovered_index == Some(actual_idx);
            let is_selected = explorer.selected_index == Some(actual_idx)
                || explorer.selected_path.as_ref() == Some(&item.path);

            let row_w = panel_w;

            if is_selected || is_hovered {
                Self::draw_list_row_highlight(
                    canvas,
                    (panel_x, item_y, row_w, item_h),
                    is_selected,
                    explorer.is_focused,
                    theme,
                );
            }

            let indent = item.depth * 12;
            let item_center_y = item_y + (item_h / 2);
            let item_base = self.centered_baseline(item_center_y);
            let content_x = panel_x + 18 + indent;

            // Guías visuales de sangría estilo DataGrip / JetBrains para niveles anidados
            if item.depth > 0 {
                let guide_color = theme.mix(theme.chrome_bg(), theme.foreground, 0.22);
                let clip = (panel_x, panel_y, panel_x + panel_w, panel_y + panel_h);
                for d in 0..item.depth {
                    let guide_x = panel_x + 18 + d * 12 + 5;
                    canvas.fill_rect(clip, guide_x, item_y, 1, item_h, guide_color);
                }
            }

            // Escapado seguro de nombres remotos con secuencias de control o ANSI (Ssh.dc.html)
            let is_escaped = crate::db::sanitize::has_escapes(&item.name);
            let display_name = if is_escaped {
                crate::db::sanitize::display(&item.name)
            } else {
                std::borrow::Cow::Borrowed(item.name.as_str())
            };

            // Espacio reservado a la derecha para botones de acción o insignia de Git/escapado
            let right_reserve = if explorer.changes_only {
                68
            } else if item.git_status.is_some() {
                SidebarRects::PAD_R + 16 + 6
            } else if is_escaped {
                SidebarRects::PAD_R + 56
            } else {
                SidebarRects::PAD_R + 6
            };
            let max_name_x = panel_x + row_w.saturating_sub(right_reserve);

            if item.is_dir {
                // Directorio: Lucide Chevron + Folder/FolderOpen
                let chevron_icon = if item.is_expanded {
                    crate::ui::LucideIcon::ChevronDown
                } else {
                    crate::ui::LucideIcon::ChevronRight
                };
                self.draw_lucide_icon(
                    canvas,
                    chevron_icon,
                    content_x + 5,
                    item_center_y,
                    10.5,
                    theme.text_muted(),
                );

                let folder_color = if is_selected {
                    theme.accent_color()
                } else {
                    theme.text_muted()
                };
                self.draw_glyph_icon(
                    canvas,
                    crate::ui::file_icons::folder_glyph(item.is_expanded),
                    folder_color,
                    content_x + 22,
                    item_center_y,
                    tokens::BODY,
                );

                // Con cambios de git, el nombre y la letra llevan el color del estado.
                let dir_color = if is_escaped {
                    theme.ansi[3]
                } else if let Some(git) = item.git_status {
                    Self::git_status_color(git, theme)
                } else if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                self.draw_ui_text(
                    canvas,
                    &display_name,
                    content_x + 36,
                    item_base,
                    max_name_x,
                    dir_color,
                    tokens::BODY,
                    is_hovered || is_selected,
                );
            } else {
                // Archivo: Lucide file icons por extensión
                self.draw_file_icon(
                    canvas,
                    &item.path,
                    content_x + 10,
                    item_center_y,
                    tokens::BODY,
                    theme,
                );

                let text_color = if is_escaped {
                    theme.ansi[3]
                } else if let Some(git) = item.git_status {
                    Self::git_status_color(git, theme)
                } else if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                self.draw_ui_text(
                    canvas,
                    &display_name,
                    content_x + 26,
                    item_base,
                    max_name_x,
                    text_color,
                    tokens::BODY,
                    is_selected,
                );
            }

            let show_actions = explorer.changes_only && is_hovered;
            if show_actions {
                let buttons = crate::renderer::GitActionBtnRects::buttons_for_item(
                    panel_x,
                    row_w,
                    item_y,
                    item_h,
                    item.is_staged,
                    item.is_unstaged,
                );
                for (action, (bx, by, bw, bh)) in buttons {
                    let btn_hovered = explorer.hovered_git_action == Some((actual_idx, action));
                    if btn_hovered {
                        canvas.rounded_fill(
                            bx,
                            by,
                            bw,
                            bh,
                            tokens::RADIUS_ROW,
                            theme.surface_hover(),
                        );
                    }
                    let (icon, fg) = match action {
                        crate::renderer::GitActionKind::Stage => (
                            crate::ui::LucideIcon::Plus,
                            if btn_hovered {
                                theme.ansi[2]
                            } else {
                                theme.text_secondary()
                            },
                        ),
                        crate::renderer::GitActionKind::Unstage => (
                            crate::ui::LucideIcon::Minus,
                            if btn_hovered {
                                theme.ansi[3]
                            } else {
                                theme.text_secondary()
                            },
                        ),
                        crate::renderer::GitActionKind::Discard => (
                            crate::ui::LucideIcon::Undo2,
                            if btn_hovered {
                                theme.ansi[1]
                            } else {
                                theme.text_secondary()
                            },
                        ),
                    };
                    self.draw_lucide_icon(canvas, icon, bx + bw / 2, by + bh / 2, 11.0, fg);
                }
            } else if let Some(git) = item.git_status {
                let badge_letter = match git {
                    crate::explorer::GitFileStatus::Modified => "M",
                    crate::explorer::GitFileStatus::Untracked => "U",
                    crate::explorer::GitFileStatus::Added => "A",
                    crate::explorer::GitFileStatus::Deleted => "D",
                    crate::explorer::GitFileStatus::Renamed => "R",
                };
                let badge_color = Self::git_status_color(git, theme);
                let bw = self.measure_mono_text(badge_letter, tokens::MICRO, true);
                let badge_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + bw);
                self.draw_mono_text(
                    canvas,
                    badge_letter,
                    badge_x,
                    item_base,
                    panel_x + panel_w,
                    badge_color,
                    tokens::MICRO,
                    true,
                );
            } else if is_escaped {
                let badge_text = "escapado";
                let badge_color = theme.ansi[3];
                let bw = self.measure_ui_text(badge_text, tokens::MICRO, false);
                let badge_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + bw);
                self.draw_ui_text(
                    canvas,
                    badge_text,
                    badge_x,
                    item_base,
                    panel_x + panel_w,
                    badge_color,
                    tokens::MICRO,
                    false,
                );
            }
        }

        // 5. Barra de desplazamiento del explorador si la lista excede el área visible
        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        if let Some((track, thumb)) = ScrollbarRects::vertical(
            panel_x + panel_w,
            list_y,
            list_max_y.saturating_sub(list_y),
            explorer.items.len(),
            visible_items,
            explorer.scroll_offset,
        ) {
            Self::draw_scrollbar_rects(
                canvas,
                track,
                thumb,
                explorer.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }

        // 6. Pie explicativo para sesiones remotas SSH (Ssh.dc.html)
        if explorer.remote_session.is_some() && panel_h > 140 {
            let foot_y = panel_y + panel_h.saturating_sub(44);
            let clip = (panel_x, foot_y, panel_x + panel_w, panel_y + panel_h);
            canvas.fill_rect(clip, panel_x, foot_y, panel_w, 1, theme.chrome_line());
            let line1 = "Archivos y ramas por canal SSH de la sesión.";
            let line2 = "Nombres con caracteres de control escapados.";
            let base1 = self.centered_baseline(foot_y + 13);
            let base2 = self.centered_baseline(foot_y + 27);
            self.draw_ui_text(
                canvas,
                line1,
                panel_x + 14,
                base1,
                panel_x + panel_w.saturating_sub(14),
                theme.text_muted(),
                tokens::MICRO,
                false,
            );
            self.draw_ui_text(
                canvas,
                line2,
                panel_x + 14,
                base2,
                panel_x + panel_w.saturating_sub(14),
                theme.text_muted(),
                tokens::MICRO,
                false,
            );
        }

        self.draw_explorer_edit_bar(canvas, explorer, panel_x, panel_y, panel_w, theme);
    }

    /// Barra de filtro rápido (`Ctrl + P` / `Ctrl + F`), con icono de lupa, input y botón de cerrar.
    fn draw_explorer_filter_bar(
        &mut self,
        canvas: &mut Canvas,
        explorer: &crate::explorer::FileExplorer,
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
        theme: &Theme,
    ) {
        let Some(filter_input) = explorer.filter.as_ref() else {
            return;
        };
        let (bx, by, bw, bh) =
            crate::explorer::FileExplorer::filter_bar_rect(panel_x, panel_y, panel_w);
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_CONTROL, theme.surface_bg());
        let outline = if explorer.is_focused {
            theme.accent_color()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_CONTROL, outline);

        // Icono de lupa a la izquierda
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Search,
            bx + 14,
            by + bh / 2,
            11.0,
            theme.text_muted(),
        );

        // Botón de cerrar/limpiar (X) a la derecha
        let (cx, cy, cw, ch) =
            crate::explorer::FileExplorer::filter_close_btn_rect(panel_x, panel_y, panel_w);
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::X,
            cx + cw / 2,
            cy + ch / 2,
            11.0,
            if explorer.hovered_filter_close {
                theme.foreground
            } else {
                theme.text_muted()
            },
        );

        // Input de texto
        let base = self.centered_baseline(by + bh / 2);
        self.render_text_input_content(
            canvas,
            filter_input,
            bx + 26,
            base,
            cx.saturating_sub(4),
            by,
            bh,
            explorer.is_focused,
            "Filtrar... (Esc)",
            true,
            theme,
        );
    }

    /// Barra de edición de nombre (nuevo archivo/carpeta o renombrar), sobre las primeras filas de la lista.
    /// El motivo del último rechazo (nombre repetido o no válido) va en una tira bajo ella.
    fn draw_explorer_edit_bar(
        &mut self,
        canvas: &mut Canvas,
        explorer: &crate::explorer::FileExplorer,
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
        theme: &Theme,
    ) {
        use crate::explorer::{EditKind, FileExplorer};
        let Some(edit) = explorer.edit.as_ref() else {
            return;
        };
        let (bx, by, bw, bh) = FileExplorer::edit_bar_rect(panel_x, panel_y, panel_w);
        canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_CONTROL, theme.surface_bg());
        let outline = if edit.error.is_some() {
            theme.ansi[1]
        } else {
            theme.accent_color()
        };
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_CONTROL, outline);

        let (icon, placeholder) = match edit.kind {
            EditKind::NewFile => (crate::ui::LucideIcon::File, "Nombre del archivo"),
            EditKind::NewDir => (crate::ui::LucideIcon::Folder, "Nombre de la carpeta"),
            EditKind::Rename => (crate::ui::LucideIcon::Pencil, "Nuevo nombre"),
        };
        self.draw_lucide_icon(canvas, icon, bx + 16, by + bh / 2, 12.0, theme.text_muted());
        let base = self.centered_baseline(by + bh / 2);
        self.render_text_input_content(
            canvas,
            &edit.input,
            bx + FileExplorer::EDIT_TEXT_DX,
            base,
            bx + bw - 8,
            by,
            bh,
            true,
            placeholder,
            true,
            theme,
        );

        if let Some(err) = &edit.error {
            let (ey, eh) = (by + bh + 2, 22);
            canvas.rounded_fill(bx, ey, bw, eh, tokens::RADIUS_ROW, theme.chrome_bg());
            let err_base = self.centered_baseline(ey + eh / 2);
            self.draw_ui_text(
                canvas,
                err,
                bx + 8,
                err_base,
                bx + bw - 8,
                theme.ansi[1],
                tokens::DETAIL,
                false,
            );
        }
    }

    /// Color de un estado de git: amarillo modificado, verde añadido o sin seguir, rojo borrado y cian
    /// renombrado. Lo usan el nombre y la letra del explorador (árbol y vista «Cambios»).
    pub(crate) fn git_status_color(status: crate::explorer::GitFileStatus, theme: &Theme) -> Color {
        use crate::explorer::GitFileStatus::*;
        match status {
            Modified => theme.ansi[3],
            Untracked | Added => theme.ansi[2],
            Deleted => theme.ansi[1],
            Renamed => theme.ansi[6],
        }
    }

    pub(crate) fn explorer_scrollbar_geometry(
        width: usize,
        height: usize,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
    ) -> Option<(Rect, Rect)> {
        if !explorer.visible || explorer.changes_only {
            return None;
        }
        let (panel_x, panel_y, panel_w, panel_h) =
            explorer.panel_geometry(width, height, top_offset, status_bar_h);
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let sub_h = explorer.subheader_height();
        let list_y = panel_y + header_h + sub_h + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);

        ScrollbarRects::vertical(
            panel_x + panel_w,
            list_y,
            list_max_y.saturating_sub(list_y),
            explorer.visible_item_count(),
            (list_max_y.saturating_sub(list_y)) / item_h,
            explorer.scroll_offset,
        )
    }

    /// Si el elemento bajo el cursor sobrepasa el ancho permitido del explorador, calcula su geometría
    /// expandida: `(hovered_idx, item_y, item_h, row_w)`.
    pub(crate) fn explorer_hovered_item_geometry(
        &mut self,
        width: usize,
        height: usize,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
    ) -> Option<(usize, usize, usize, usize)> {
        if !explorer.visible || explorer.changes_only {
            return None;
        }
        let hovered_idx = explorer.hovered_index?;
        if hovered_idx < explorer.scroll_offset || hovered_idx >= explorer.visible_item_count() {
            return None;
        }
        let (panel_x, panel_y, panel_w, panel_h) =
            explorer.panel_geometry(width, height, top_offset, status_bar_h);
        if panel_w == 0 || panel_h < 80 {
            return None;
        }

        let rel_idx = hovered_idx - explorer.scroll_offset;
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let list_y = panel_y
            + header_h
            + explorer.subheader_height()
            + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);
        let item_y = list_y + rel_idx * item_h;
        if item_y + item_h > list_max_y {
            return None;
        }

        let item = explorer.visible_item(hovered_idx)?;
        let is_selected = explorer.selected_index == Some(hovered_idx)
            || explorer.selected_path.as_ref() == Some(&item.path);

        let indent = item.depth * 12;
        let content_x = panel_x + 18 + indent;
        let icon_text_offset = if item.is_dir { 36 } else { 26 };
        let text_x = content_x + icon_text_offset;
        let is_escaped = crate::db::sanitize::has_escapes(&item.name);
        let display_name = if is_escaped {
            crate::db::sanitize::display(&item.name)
        } else {
            std::borrow::Cow::Borrowed(item.name.as_str())
        };
        let badge_w = if item.git_status.is_some() {
            16
        } else if is_escaped {
            50
        } else {
            0
        };
        let default_max_x = panel_x + panel_w.saturating_sub(SidebarRects::PAD_R + badge_w + 6);
        let name_w = self.measure_ui_text(&display_name, tokens::BODY, is_selected);

        if text_x + name_w <= default_max_x {
            return None;
        }

        let needed_w =
            (text_x + name_w + SidebarRects::PAD_R + badge_w + 12).saturating_sub(panel_x);
        let max_allowed_w = width.saturating_sub(panel_x + 8);
        let row_w = panel_w.max(needed_w).min(max_allowed_w);
        Some((hovered_idx, item_y, item_h, row_w))
    }

    /// Dibuja la fila expandida flotante por encima del panel, contorno y barras de desplazamiento
    /// para permitir leer el nombre completo del archivo o carpeta cómodamente.
    pub(crate) fn render_file_explorer_hover_overlay(
        &mut self,
        canvas: &mut Canvas,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }
        let (hovered_idx, item_y, item_h, row_w) = match self.explorer_hovered_item_geometry(
            width,
            height,
            top_offset,
            status_bar_h,
            explorer,
        ) {
            Some(geo) => geo,
            None => return,
        };

        let (panel_x, panel_y, _panel_w, panel_h) =
            explorer.panel_geometry(width, height, top_offset, status_bar_h);
        let item = &explorer.items[hovered_idx];
        let is_selected = explorer.selected_index == Some(hovered_idx)
            || explorer.selected_path.as_ref() == Some(&item.path);

        let pill_x = panel_x + 8;
        let pill_w = row_w.saturating_sub(16);

        let fill = if is_selected && !explorer.is_focused {
            theme.surface_bg()
        } else {
            theme.surface_hover()
        };
        canvas.rounded_fill(pill_x, item_y, pill_w, item_h, tokens::RADIUS_ROW, fill);
        canvas.rounded_stroke(
            pill_x,
            item_y,
            pill_w,
            item_h,
            tokens::RADIUS_ROW,
            theme.card_outline(),
        );

        if is_selected {
            let bar = if explorer.is_focused {
                theme.accent_color()
            } else {
                theme.chrome_line_strong()
            };
            canvas.rounded_fill(
                panel_x + 12,
                item_y + 6,
                2,
                item_h.saturating_sub(12),
                tokens::RADIUS_ROW,
                bar,
            );
        }

        let item_center_y = item_y + (item_h / 2);
        let item_base = self.centered_baseline(item_center_y);
        let indent = item.depth * 12;
        let content_x = panel_x + 18 + indent;

        if item.depth > 0 {
            let guide_color = theme.mix(theme.chrome_bg(), theme.foreground, 0.22);
            let clip = (panel_x, panel_y, panel_x + row_w, panel_y + panel_h);
            for d in 0..item.depth {
                let guide_x = panel_x + 18 + d * 12 + 5;
                canvas.fill_rect(clip, guide_x, item_y, 1, item_h, guide_color);
            }
        }

        let badge_w = if item.git_status.is_some() { 16 } else { 0 };
        let max_name_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + badge_w + 6);

        if item.is_dir {
            let chevron_icon = if item.is_expanded {
                crate::ui::LucideIcon::ChevronDown
            } else {
                crate::ui::LucideIcon::ChevronRight
            };
            self.draw_lucide_icon(
                canvas,
                chevron_icon,
                content_x + 5,
                item_center_y,
                10.5,
                theme.text_muted(),
            );

            let folder_color = if is_selected {
                theme.accent_color()
            } else {
                theme.text_muted()
            };
            self.draw_glyph_icon(
                canvas,
                crate::ui::file_icons::folder_glyph(item.is_expanded),
                folder_color,
                content_x + 22,
                item_center_y,
                tokens::BODY,
            );

            let is_escaped = crate::db::sanitize::has_escapes(&item.name);
            let display_name = if is_escaped {
                crate::db::sanitize::display(&item.name)
            } else {
                std::borrow::Cow::Borrowed(item.name.as_str())
            };

            let dir_color = if is_escaped {
                theme.ansi[3]
            } else if let Some(git) = item.git_status {
                Self::git_status_color(git, theme)
            } else if is_selected {
                theme.accent_color()
            } else {
                theme.foreground
            };
            self.draw_ui_text(
                canvas,
                &display_name,
                content_x + 36,
                item_base,
                max_name_x,
                dir_color,
                tokens::BODY,
                true,
            );
        } else {
            self.draw_file_icon(
                canvas,
                &item.path,
                content_x + 10,
                item_center_y,
                tokens::BODY,
                theme,
            );

            let is_escaped = crate::db::sanitize::has_escapes(&item.name);
            let display_name = if is_escaped {
                crate::db::sanitize::display(&item.name)
            } else {
                std::borrow::Cow::Borrowed(item.name.as_str())
            };

            let text_color = if is_escaped {
                theme.ansi[3]
            } else if let Some(git) = item.git_status {
                Self::git_status_color(git, theme)
            } else if is_selected {
                theme.accent_color()
            } else {
                theme.foreground
            };
            self.draw_ui_text(
                canvas,
                &display_name,
                content_x + 26,
                item_base,
                max_name_x,
                text_color,
                tokens::BODY,
                is_selected,
            );
        }

        let is_escaped = crate::db::sanitize::has_escapes(&item.name);
        if let Some(git) = item.git_status {
            let badge_letter = match git {
                crate::explorer::GitFileStatus::Modified => "M",
                crate::explorer::GitFileStatus::Untracked => "U",
                crate::explorer::GitFileStatus::Added => "A",
                crate::explorer::GitFileStatus::Deleted => "D",
                crate::explorer::GitFileStatus::Renamed => "R",
            };
            let badge_color = Self::git_status_color(git, theme);
            let bw = self.measure_mono_text(badge_letter, tokens::MICRO, true);
            let badge_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + bw);
            self.draw_mono_text(
                canvas,
                badge_letter,
                badge_x,
                item_base,
                panel_x + row_w,
                badge_color,
                tokens::MICRO,
                true,
            );
        } else if is_escaped {
            let badge_text = "escapado";
            let badge_color = theme.ansi[3];
            let bw = self.measure_ui_text(badge_text, tokens::MICRO, false);
            let badge_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + bw);
            self.draw_ui_text(
                canvas,
                badge_text,
                badge_x,
                item_base,
                panel_x + row_w,
                badge_color,
                tokens::MICRO,
                false,
            );
        }
    }

    /// Dibuja el fondo del panel en un búfer propio de ancho `panel_w`.
    /// `on_right`: el panel está a la derecha y el borde que lo separa del contenido es el izquierdo.
    #[allow(dead_code)]
    pub(crate) fn render_files_panel(
        &mut self,
        canvas: &mut Canvas,
        panel_w: usize,
        status_bar_h: usize,
        on_right: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if panel_w == 0 || width == 0 || height == 0 {
            return;
        }
        let top = SidebarRects::TOP;
        let body_h = height.saturating_sub(status_bar_h).saturating_sub(top);
        Self::draw_rect(canvas, 0, top, panel_w, body_h, theme.chrome_bg());
        let edge_x = if on_right { 0 } else { panel_w - 1 };
        Self::draw_rect(canvas, edge_x, top, 1, body_h, theme.chrome_line());

        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        Self::draw_rect(
            canvas,
            8,
            top + header_h - 1,
            panel_w.saturating_sub(16),
            1,
            theme.chrome_line(),
        );
        let baseline = self.centered_baseline(top + (header_h - 1) / 2);
        self.draw_ui_text_tracked(
            canvas,
            "ARCHIVOS",
            16,
            baseline,
            panel_w.saturating_sub(16),
            theme.text_muted(),
            tokens::TITLE,
            true,
            tokens::TRACKING,
        );
    }

    /// Resalte de una fila de lista del panel (Archivos y Cambios): inset de 8 px, esquinas redondeadas
    /// y, si está seleccionada, una marca de acento (gris cuando la lista no tiene el foco).
    pub(crate) fn draw_list_row_highlight(
        canvas: &mut Canvas,
        row: Rect,
        selected: bool,
        focused: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = row;
        let fill = if selected && !focused {
            theme.surface_bg()
        } else {
            theme.surface_hover()
        };
        let row_w = w.saturating_sub(16);
        canvas.rounded_fill(x + 8, y, row_w, h, tokens::RADIUS_ROW, fill);
        if selected && focused {
            canvas.rounded_stroke(x + 8, y, row_w, h, tokens::RADIUS_ROW, theme.card_outline());
        }
        if selected {
            let bar = if focused {
                theme.accent_color()
            } else {
                theme.chrome_line_strong()
            };
            canvas.rounded_fill(
                x + 12,
                y + 6,
                2,
                h.saturating_sub(12),
                tokens::RADIUS_ROW,
                bar,
            );
        }
    }

    /// Renderiza el panel contextual de Git (Fase F2)
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_git_panel(
        &mut self,
        canvas: &mut Canvas,
        panel_x: usize,
        content_y: usize,
        panel_w: usize,
        content_h: usize,
        explorer: &crate::explorer::FileExplorer,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        let clip: Clip = (
            panel_x,
            content_y,
            (panel_x + panel_w).min(width),
            (content_y + content_h).min(height),
        );

        let staged_items = explorer.staged_change_items();
        let unstaged_items = explorer.unstaged_change_items();
        let staged_count = staged_items.len();
        let unstaged_count = unstaged_items.len();
        let staged_collapsed = explorer.staged_collapsed;
        let unstaged_collapsed = explorer.unstaged_collapsed;
        let has_discard_confirm = explorer.discard_confirm.is_some();

        let rects = crate::renderer::GitPanelRects::compute(
            panel_x,
            content_y,
            panel_w,
            content_h,
            staged_count,
            staged_collapsed,
            unstaged_count,
            unstaged_collapsed,
            has_discard_confirm,
        );

        let hov = explorer.hovered_git_target;

        // 1. Fila de rama (height: 36 px)
        let br = rects.branch_row;
        if hov == Some(crate::renderer::GitClickTarget::BranchRow) {
            canvas.rounded_fill(
                br.0 + 6,
                br.1 + 4,
                br.2.saturating_sub(12),
                br.3.saturating_sub(8),
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }
        Self::draw_rect(canvas, br.0, br.1 + br.3 - 1, br.2, 1, theme.chrome_line());
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::GitBranch,
            br.0 + 16,
            br.1 + br.3 / 2,
            12.0,
            theme.text_secondary(),
        );

        let branch_name = explorer.git_branch.as_deref().unwrap_or("main");
        let br_base = self.centered_baseline(br.1 + br.3 / 2);
        let branch_x = br.0 + 34;
        let ab_text = explorer
            .git_commit
            .ahead_behind
            .map(|(a, b)| format!("↑{a} ↓{b}"))
            .unwrap_or_else(|| "↑0 ↓0".into());
        let ab_w = self.measure_mono_text(&ab_text, tokens::MICRO, false);
        let ab_x = br.0 + br.2.saturating_sub(14 + ab_w);
        self.draw_mono_text(
            canvas,
            branch_name,
            branch_x,
            br_base,
            ab_x.saturating_sub(8),
            theme.foreground,
            tokens::DETAIL,
            true,
        );
        self.draw_mono_text(
            canvas,
            &ab_text,
            ab_x,
            br_base,
            br.0 + br.2 - 10,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );

        // 2. Formulario de commit
        // 2a. Input de mensaje de commit
        let ci = rects.commit_input;
        canvas.fill_rect(clip, ci.0, ci.1, ci.2, ci.3, theme.background);
        let border_color = if explorer.git_commit.is_focused {
            theme.accent_color()
        } else {
            theme.chrome_line()
        };
        canvas.rounded_stroke(ci.0, ci.1, ci.2, ci.3, 0, border_color);

        let input_text_y = ci.1 + 18;
        let base_input_y = self.centered_baseline(input_text_y);
        self.render_text_input_content(
            canvas,
            &explorer.git_commit.input,
            ci.0 + 8,
            base_input_y,
            ci.0 + ci.2 - 8,
            ci.1 + 4,
            20,
            cursor_visible,
            "Mensaje de commit...",
            explorer.git_commit.is_focused,
            theme,
        );

        // 2b. Botones de acción (Confirmar N y Subir)
        let cb = rects.commit_btn;
        let can_commit = !explorer.git_commit.input.text().trim().is_empty()
            && (staged_count > 0 || explorer.git_commit.amend);
        let cb_base_bg = if can_commit {
            theme.accent_color()
        } else {
            theme.mix(theme.accent_color(), theme.background, 0.45)
        };
        let cb_bg = if hov == Some(crate::renderer::GitClickTarget::CommitBtn) {
            theme.mix(cb_base_bg, theme.foreground, 0.14)
        } else {
            cb_base_bg
        };
        canvas.rounded_fill(cb.0, cb.1, cb.2, cb.3, tokens::RADIUS_CONTROL, cb_bg);
        let cb_label = if explorer.git_commit.amend {
            "Confirmar enmienda".to_string()
        } else if staged_count > 0 {
            format!("Confirmar {staged_count}")
        } else {
            "Confirmar".to_string()
        };
        let cb_lw = self.measure_ui_text(&cb_label, tokens::DETAIL, true);
        let cb_tx = cb.0 + (cb.2.saturating_sub(cb_lw)) / 2;
        let cb_base = self.centered_baseline(cb.1 + cb.3 / 2);
        self.draw_ui_text(
            canvas,
            &cb_label,
            cb_tx,
            cb_base,
            cb.0 + cb.2,
            theme.background,
            tokens::DETAIL,
            true,
        );

        let pb = rects.push_btn;
        let push_hov = hov == Some(crate::renderer::GitClickTarget::PushBtn);
        if push_hov {
            canvas.rounded_fill(
                pb.0,
                pb.1,
                pb.2,
                pb.3,
                tokens::RADIUS_CONTROL,
                theme.surface_hover(),
            );
        }
        canvas.rounded_stroke(
            pb.0,
            pb.1,
            pb.2,
            pb.3,
            tokens::RADIUS_CONTROL,
            if push_hov {
                theme.chrome_line_strong()
            } else {
                theme.chrome_line()
            },
        );
        let pb_label = if explorer.git_commit.is_pushing {
            "Subiendo..."
        } else {
            "Subir"
        };
        let pb_lw = self.measure_ui_text(pb_label, tokens::DETAIL, true);
        let pb_tx = pb.0 + (pb.2.saturating_sub(pb_lw)) / 2;
        let pb_base = self.centered_baseline(pb.1 + pb.3 / 2);
        self.draw_ui_text(
            canvas,
            pb_label,
            pb_tx,
            pb_base,
            pb.0 + pb.2,
            theme.foreground,
            tokens::DETAIL,
            true,
        );

        // 2c. Fila meta (contador de caracteres y opción enmendar)
        let cc = rects.char_count;
        let char_len = explorer.git_commit.input.text().chars().count();
        let cc_text = format!("{char_len} / 72 caracteres");
        let cc_color = if char_len > 72 {
            theme.ansi[3]
        } else {
            theme.text_muted()
        };
        let cc_base = self.centered_baseline(cc.1 + cc.3 / 2);
        self.draw_ui_text(
            canvas,
            &cc_text,
            cc.0,
            cc_base,
            cc.0 + cc.2,
            cc_color,
            tokens::MICRO,
            false,
        );

        let at = rects.amend_toggle;
        let amend_hov = hov == Some(crate::renderer::GitClickTarget::AmendToggle);
        let chk_size = 12;
        let chk_y = at.1 + (at.3.saturating_sub(chk_size)) / 2;
        if explorer.git_commit.amend {
            canvas.fill_rect(clip, at.0, chk_y, chk_size, chk_size, theme.accent_color());
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Check,
                at.0 + chk_size / 2,
                chk_y + chk_size / 2,
                9.0,
                theme.background,
            );
        } else {
            canvas.rounded_stroke(
                at.0,
                chk_y,
                chk_size,
                chk_size,
                0,
                if amend_hov {
                    theme.chrome_line_strong()
                } else {
                    theme.chrome_line()
                },
            );
        }
        let at_base = self.centered_baseline(at.1 + at.3 / 2);
        self.draw_ui_text(
            canvas,
            "Enmendar último commit",
            at.0 + chk_size + 6,
            at_base,
            at.0 + at.2,
            if amend_hov {
                theme.foreground
            } else {
                theme.text_muted()
            },
            tokens::MICRO,
            false,
        );

        // Mensaje de estado (éxito o error de commit/push)
        if let Some((msg, is_err)) = &explorer.git_commit.status_message {
            let msg_color = if *is_err {
                theme.ansi[1]
            } else {
                theme.ansi[2]
            };
            let msg_y = self.centered_baseline(rects.list_rect.1.saturating_sub(8));
            self.draw_ui_text(
                canvas,
                msg,
                panel_x + 12,
                msg_y,
                panel_x + panel_w - 12,
                msg_color,
                tokens::MICRO,
                false,
            );
        }

        // Línea divisoria previa a la lista
        Self::draw_rect(
            canvas,
            panel_x,
            rects.list_rect.1 - 1,
            panel_w,
            1,
            theme.chrome_line(),
        );

        // 3. Lista de archivos (Preparados y Cambios)
        let list_clip: Clip = (
            panel_x,
            rects.list_rect.1,
            (panel_x + panel_w).min(width),
            (rects.list_rect.1 + rects.list_rect.3).min(height),
        );
        let list_max_y = rects.list_rect.1 + rects.list_rect.3;

        // 3a. Cabecera «PREPARADOS»
        if let Some((sh, uab)) = rects.staged_header_rect(explorer.scroll_offset) {
            let sh_hov = matches!(
                hov,
                Some(
                    crate::renderer::GitClickTarget::StagedHeaderToggle
                        | crate::renderer::GitClickTarget::UnstageAll
                )
            );
            let sh_bg = if sh_hov {
                theme.surface_hover()
            } else {
                theme.background
            };
            canvas.fill_rect(list_clip, sh.0, sh.1, sh.2, sh.3, sh_bg);
            let chev = if staged_collapsed {
                crate::ui::LucideIcon::ChevronRight
            } else {
                crate::ui::LucideIcon::ChevronDown
            };
            self.draw_lucide_icon(
                canvas,
                chev,
                sh.0 + 12,
                sh.1 + sh.3 / 2,
                11.0,
                if sh_hov {
                    theme.foreground
                } else {
                    theme.text_muted()
                },
            );
            let sh_base = self.centered_baseline(sh.1 + sh.3 / 2);
            let sh_title = "PREPARADOS";
            let next_x = self.draw_ui_text(
                canvas,
                sh_title,
                sh.0 + 24,
                sh_base,
                sh.0 + sh.2 - 40,
                if sh_hov {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
                tokens::MICRO,
                true,
            );
            let count_str = format!("{staged_count}");
            self.draw_mono_text(
                canvas,
                &count_str,
                next_x + 8,
                sh_base,
                sh.0 + sh.2 - 32,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );

            // Botón quitar todo (−)
            if staged_count > 0 {
                let uab_hov = hov == Some(crate::renderer::GitClickTarget::UnstageAll);
                if uab_hov {
                    canvas.rounded_fill(
                        uab.0,
                        uab.1,
                        uab.2,
                        uab.3,
                        tokens::RADIUS_ROW,
                        theme.surface_bg(),
                    );
                }
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Minus,
                    uab.0 + uab.2 / 2,
                    uab.1 + uab.3 / 2,
                    11.0,
                    if uab_hov {
                        theme.ansi[3]
                    } else {
                        theme.text_secondary()
                    },
                );
            }
        }

        // 3b. Filas de preparados
        if !staged_collapsed {
            for (i, item) in staged_items.iter().enumerate() {
                let Some(iy) = rects.staged_item_y(i, explorer.scroll_offset) else {
                    continue;
                };
                if iy + crate::renderer::GitPanelRects::ITEM_ROW_H > list_max_y {
                    continue;
                }
                let row_hov = matches!(
                    hov,
                    Some(
                        crate::renderer::GitClickTarget::StagedItem(idx)
                            | crate::renderer::GitClickTarget::UnstageItem(idx)
                    ) if idx == i
                );
                let action_hov = matches!(
                    hov,
                    Some(crate::renderer::GitClickTarget::UnstageItem(idx)) if idx == i
                )
                .then_some(crate::renderer::GitActionKind::Unstage);
                self.draw_git_item_row(
                    canvas,
                    list_clip,
                    item,
                    panel_x,
                    iy,
                    panel_w,
                    true,
                    explorer.selected_path.as_ref() == Some(&item.path),
                    row_hov,
                    explorer.is_focused,
                    action_hov,
                    theme,
                );
            }
        }

        // 3c. Cabecera «CAMBIOS»
        if let Some((uh, sab, dab)) =
            rects.unstaged_header_rect(staged_count, staged_collapsed, explorer.scroll_offset)
        {
            let uh_hov = matches!(
                hov,
                Some(
                    crate::renderer::GitClickTarget::UnstagedHeaderToggle
                        | crate::renderer::GitClickTarget::StageAll
                        | crate::renderer::GitClickTarget::DiscardAllPrompt
                )
            );
            let uh_bg = if uh_hov {
                theme.surface_hover()
            } else {
                theme.background
            };
            canvas.fill_rect(list_clip, uh.0, uh.1, uh.2, uh.3, uh_bg);
            let chev = if unstaged_collapsed {
                crate::ui::LucideIcon::ChevronRight
            } else {
                crate::ui::LucideIcon::ChevronDown
            };
            self.draw_lucide_icon(
                canvas,
                chev,
                uh.0 + 12,
                uh.1 + uh.3 / 2,
                11.0,
                if uh_hov {
                    theme.foreground
                } else {
                    theme.text_muted()
                },
            );
            let uh_base = self.centered_baseline(uh.1 + uh.3 / 2);
            let uh_title = "CAMBIOS";
            let next_x = self.draw_ui_text(
                canvas,
                uh_title,
                uh.0 + 24,
                uh_base,
                uh.0 + uh.2 - 60,
                if uh_hov {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
                tokens::MICRO,
                true,
            );
            let count_str = format!("{unstaged_count}");
            self.draw_mono_text(
                canvas,
                &count_str,
                next_x + 8,
                uh_base,
                uh.0 + uh.2 - 50,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );

            // Botones preparar todo (+) y descartar todo (↺)
            if unstaged_count > 0 {
                let sab_hov = hov == Some(crate::renderer::GitClickTarget::StageAll);
                if sab_hov {
                    canvas.rounded_fill(
                        sab.0,
                        sab.1,
                        sab.2,
                        sab.3,
                        tokens::RADIUS_ROW,
                        theme.surface_bg(),
                    );
                }
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Plus,
                    sab.0 + sab.2 / 2,
                    sab.1 + sab.3 / 2,
                    11.0,
                    if sab_hov {
                        theme.ansi[2]
                    } else {
                        theme.text_secondary()
                    },
                );
                let dab_hov = hov == Some(crate::renderer::GitClickTarget::DiscardAllPrompt);
                if dab_hov {
                    canvas.rounded_fill(
                        dab.0,
                        dab.1,
                        dab.2,
                        dab.3,
                        tokens::RADIUS_ROW,
                        theme.surface_bg(),
                    );
                }
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Undo2,
                    dab.0 + dab.2 / 2,
                    dab.1 + dab.3 / 2,
                    11.0,
                    if dab_hov {
                        theme.ansi[1]
                    } else {
                        theme.text_secondary()
                    },
                );
            }
        }

        // 3d. Filas de cambios sin preparar
        if !unstaged_collapsed {
            for (i, item) in unstaged_items.iter().enumerate() {
                let Some(iy) = rects.unstaged_item_y(
                    i,
                    staged_count,
                    staged_collapsed,
                    explorer.scroll_offset,
                ) else {
                    continue;
                };
                if iy + crate::renderer::GitPanelRects::ITEM_ROW_H > list_max_y {
                    continue;
                }
                let row_hov = matches!(
                    hov,
                    Some(
                        crate::renderer::GitClickTarget::UnstagedItem(idx)
                            | crate::renderer::GitClickTarget::StageItem(idx)
                            | crate::renderer::GitClickTarget::DiscardItemPrompt(idx)
                    ) if idx == i
                );
                let action_hov = match hov {
                    Some(crate::renderer::GitClickTarget::StageItem(idx)) if idx == i => {
                        Some(crate::renderer::GitActionKind::Stage)
                    }
                    Some(crate::renderer::GitClickTarget::DiscardItemPrompt(idx)) if idx == i => {
                        Some(crate::renderer::GitActionKind::Discard)
                    }
                    _ => None,
                };
                self.draw_git_item_row(
                    canvas,
                    list_clip,
                    item,
                    panel_x,
                    iy,
                    panel_w,
                    false,
                    explorer.selected_path.as_ref() == Some(&item.path),
                    row_hov,
                    explorer.is_focused,
                    action_hov,
                    theme,
                );
            }
        }

        // Estado vacío si no hay cambios
        if staged_count == 0 && unstaged_count == 0 {
            let empty_y = self.centered_baseline(rects.list_rect.1 + 40);
            self.draw_ui_text(
                canvas,
                "No hay cambios pendientes",
                panel_x + 20,
                empty_y,
                panel_x + panel_w - 20,
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );
        }

        // 4. Diálogo de confirmación de descarte si está activo
        if let (Some(b_rect), Some(cancel_btn), Some(exec_btn)) = (
            rects.discard_banner,
            rects.discard_cancel_btn,
            rects.discard_execute_btn,
        ) {
            canvas.fill_rect(
                clip,
                b_rect.0,
                b_rect.1,
                b_rect.2,
                b_rect.3,
                theme.surface_bg(),
            );
            canvas.rounded_stroke(b_rect.0, b_rect.1, b_rect.2, b_rect.3, 0, theme.ansi[1]);
            // Franja superior roja
            canvas.fill_rect(clip, b_rect.0, b_rect.1, b_rect.2, 3, theme.ansi[1]);

            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Info,
                b_rect.0 + 16,
                b_rect.1 + 18,
                13.0,
                theme.ansi[1],
            );
            let title_y = self.centered_baseline(b_rect.1 + 18);
            self.draw_ui_text(
                canvas,
                "¿Descartar cambios?",
                b_rect.0 + 30,
                title_y,
                b_rect.0 + b_rect.2 - 10,
                theme.ansi[1],
                tokens::DETAIL,
                true,
            );

            let sub_y = self.centered_baseline(b_rect.1 + 34);
            let sub_text = match &explorer.discard_confirm {
                Some(crate::explorer::DiscardTarget::All) => {
                    "Se descartarán todos los cambios sin preparar."
                }
                Some(crate::explorer::DiscardTarget::File(p, _)) => {
                    p.file_name().and_then(|n| n.to_str()).unwrap_or("archivo")
                }
                None => "",
            };
            self.draw_ui_text(
                canvas,
                sub_text,
                b_rect.0 + 14,
                sub_y,
                b_rect.0 + b_rect.2 - 14,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );

            // Botón Cancelar
            let cancel_hov = hov == Some(crate::renderer::GitClickTarget::DiscardConfirmCancel);
            if cancel_hov {
                canvas.rounded_fill(
                    cancel_btn.0,
                    cancel_btn.1,
                    cancel_btn.2,
                    cancel_btn.3,
                    tokens::RADIUS_ROW,
                    theme.surface_hover(),
                );
            }
            canvas.rounded_stroke(
                cancel_btn.0,
                cancel_btn.1,
                cancel_btn.2,
                cancel_btn.3,
                tokens::RADIUS_ROW,
                if cancel_hov {
                    theme.chrome_line_strong()
                } else {
                    theme.chrome_line()
                },
            );
            let c_w = self.measure_ui_text("Cancelar", tokens::MICRO, true);
            let c_base = self.centered_baseline(cancel_btn.1 + cancel_btn.3 / 2);
            self.draw_ui_text(
                canvas,
                "Cancelar",
                cancel_btn.0 + (cancel_btn.2.saturating_sub(c_w)) / 2,
                c_base,
                cancel_btn.0 + cancel_btn.2,
                theme.foreground,
                tokens::MICRO,
                true,
            );

            // Botón Descartar (rojo)
            let exec_hov = hov == Some(crate::renderer::GitClickTarget::DiscardConfirmExecute);
            let exec_bg = if exec_hov {
                theme.mix(theme.ansi[1], theme.foreground, 0.14)
            } else {
                theme.ansi[1]
            };
            canvas.rounded_fill(
                exec_btn.0,
                exec_btn.1,
                exec_btn.2,
                exec_btn.3,
                tokens::RADIUS_ROW,
                exec_bg,
            );
            let d_w = self.measure_ui_text("Descartar", tokens::MICRO, true);
            let d_base = self.centered_baseline(exec_btn.1 + exec_btn.3 / 2);
            self.draw_ui_text(
                canvas,
                "Descartar",
                exec_btn.0 + (exec_btn.2.saturating_sub(d_w)) / 2,
                d_base,
                exec_btn.0 + exec_btn.2,
                theme.background,
                tokens::MICRO,
                true,
            );
        }
    }

    /// Dibuja una fila individual de archivo dentro de la lista de Git
    #[allow(clippy::too_many_arguments)]
    fn draw_git_item_row(
        &mut self,
        canvas: &mut Canvas,
        _clip: Clip,
        item: &crate::explorer::ExplorerItem,
        panel_x: usize,
        iy: usize,
        panel_w: usize,
        is_staged: bool,
        is_selected: bool,
        is_hovered: bool,
        is_focused: bool,
        hovered_action: Option<crate::renderer::GitActionKind>,
        theme: &Theme,
    ) {
        let row_h = crate::renderer::GitPanelRects::ITEM_ROW_H;
        let row_rect = (panel_x, iy, panel_w, row_h);

        if is_selected || is_hovered {
            Self::draw_list_row_highlight(canvas, row_rect, is_selected, is_focused, theme);
        }

        // Insignia con fondo translúcido redondeado y letra (fill_rect ignora alfa en el shader)
        let (badge_letter, badge_color) = match item.git_status {
            Some(crate::explorer::GitFileStatus::Modified) => ("M", theme.ansi[3]),
            Some(crate::explorer::GitFileStatus::Added) => ("A", theme.ansi[2]),
            Some(crate::explorer::GitFileStatus::Untracked) => ("U", theme.ansi[2]),
            Some(crate::explorer::GitFileStatus::Deleted) => ("D", theme.ansi[1]),
            Some(crate::explorer::GitFileStatus::Renamed) => ("R", theme.ansi[5]),
            None => ("M", theme.ansi[3]),
        };
        let badge_bg = Color::from_rgba(badge_color.r(), badge_color.g(), badge_color.b(), 45);

        let badge_size = 18;
        let badge_x = panel_x + 14;
        let badge_y = iy + (row_h.saturating_sub(badge_size)) / 2;
        canvas.rounded_fill(
            badge_x,
            badge_y,
            badge_size,
            badge_size,
            tokens::RADIUS_ROW,
            badge_bg,
        );

        let bw = self.measure_mono_text(badge_letter, tokens::MICRO, true);
        let bx = badge_x + (badge_size.saturating_sub(bw)) / 2;
        let by = self.centered_baseline(badge_y + badge_size / 2);
        self.draw_mono_text(
            canvas,
            badge_letter,
            bx,
            by,
            badge_x + badge_size,
            badge_color,
            tokens::MICRO,
            true,
        );

        // Icono de tipo de archivo y nombre + directorio atenuado
        let file_icon_cx = badge_x + badge_size + 12;
        self.draw_file_icon(
            canvas,
            &item.path,
            file_icon_cx,
            iy + row_h / 2,
            tokens::DETAIL,
            theme,
        );
        let text_x = file_icon_cx + 12;
        let max_text_x = panel_x + panel_w.saturating_sub(62);
        let base_y = self.centered_baseline(iy + row_h / 2);

        let name_w = self.measure_ui_text(&item.name, tokens::DETAIL, true);
        self.draw_ui_text(
            canvas,
            &item.name,
            text_x,
            base_y,
            max_text_x,
            theme.foreground,
            tokens::DETAIL,
            true,
        );

        let rel_dir = item
            .path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        if let Some(dir) = rel_dir {
            let dir_x = text_x + name_w + 6;
            if dir_x < max_text_x {
                self.draw_ui_text(
                    canvas,
                    dir,
                    dir_x,
                    base_y,
                    max_text_x,
                    theme.text_muted(),
                    tokens::MICRO,
                    false,
                );
            }
        }

        // Botones de acción contextuales a la derecha
        let action_btn_size = crate::renderer::GitPanelRects::ICON_BTN_SIZE;
        let btn_y = iy + (row_h.saturating_sub(action_btn_size)) / 2;
        if is_staged {
            let unstage_x = panel_x + panel_w.saturating_sub(action_btn_size + 10);
            if hovered_action == Some(crate::renderer::GitActionKind::Unstage) {
                canvas.rounded_fill(
                    unstage_x,
                    btn_y,
                    action_btn_size,
                    action_btn_size,
                    tokens::RADIUS_ROW,
                    theme.surface_bg(),
                );
            }
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Minus,
                unstage_x + action_btn_size / 2,
                btn_y + action_btn_size / 2,
                11.0,
                theme.ansi[3],
            );
        } else {
            let discard_x = panel_x + panel_w.saturating_sub(action_btn_size + 10);
            let stage_x = discard_x.saturating_sub(action_btn_size + 4);
            if hovered_action == Some(crate::renderer::GitActionKind::Stage) {
                canvas.rounded_fill(
                    stage_x,
                    btn_y,
                    action_btn_size,
                    action_btn_size,
                    tokens::RADIUS_ROW,
                    theme.surface_bg(),
                );
            }
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Plus,
                stage_x + action_btn_size / 2,
                btn_y + action_btn_size / 2,
                11.0,
                theme.ansi[2],
            );
            if hovered_action == Some(crate::renderer::GitActionKind::Discard) {
                canvas.rounded_fill(
                    discard_x,
                    btn_y,
                    action_btn_size,
                    action_btn_size,
                    tokens::RADIUS_ROW,
                    theme.surface_bg(),
                );
            }
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Undo2,
                discard_x + action_btn_size / 2,
                btn_y + action_btn_size / 2,
                11.0,
                theme.ansi[1],
            );
        }
    }
}
