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

        // 1. Superficie acoplada: fondo de chrome opaco y hairline izquierdo. Sin radios ni sombra.
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

        // 2. Encabezado: título de la vista (Archivos o Cambios, con su contador) a la izquierda y el
        //    nombre de la carpeta o del host remoto a la derecha. La vista se elige en la barra superior.
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        Self::draw_rect(
            canvas,
            panel_x + 8,
            panel_y + header_h - 1,
            panel_w.saturating_sub(16),
            1,
            theme.chrome_line(),
        );
        let title = if explorer.changes_only {
            "Cambios"
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

        // 2b. Barra de filtro rápido (`Ctrl + P` / `Ctrl + F`) si está activa
        self.draw_explorer_filter_bar(canvas, explorer, panel_x, panel_y, panel_w, theme);

        // 3. Lista de elementos (archivos y carpetas), justo bajo la cabecera y el filtro.
        let list_y = panel_y
            + header_h
            + explorer.subheader_height()
            + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);

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

            // Espacio reservado a la derecha para botones de acción o insignia de Git
            let right_reserve = if explorer.changes_only {
                68
            } else if item.git_status.is_some() {
                SidebarRects::PAD_R + 16 + 6
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
                let dir_color = if let Some(git) = item.git_status {
                    Self::git_status_color(git, theme)
                } else if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                let label = item.name.clone();
                self.draw_ui_text(
                    canvas,
                    &label,
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

                let text_color = if let Some(git) = item.git_status {
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
                    &item.name,
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
        if !explorer.visible {
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
        if !explorer.visible {
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
        let badge_w = if item.git_status.is_some() { 16 } else { 0 };
        let default_max_x = panel_x + panel_w.saturating_sub(SidebarRects::PAD_R + badge_w + 6);
        let name_w = self.measure_ui_text(&item.name, tokens::BODY, is_selected);

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

            let dir_color = if let Some(git) = item.git_status {
                Self::git_status_color(git, theme)
            } else if is_selected {
                theme.accent_color()
            } else {
                theme.foreground
            };
            self.draw_ui_text(
                canvas,
                &item.name,
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

            let text_color = if let Some(git) = item.git_status {
                Self::git_status_color(git, theme)
            } else if is_selected {
                theme.accent_color()
            } else {
                theme.foreground
            };
            self.draw_ui_text(
                canvas,
                &item.name,
                content_x + 26,
                item_base,
                max_name_x,
                text_color,
                tokens::BODY,
                is_selected,
            );
        }

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
        }
    }

    /// Dibuja el fondo del panel en un búfer propio de ancho `panel_w`.
    /// `on_right`: el panel está a la derecha y el borde que lo separa del contenido es el izquierdo.
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
}
