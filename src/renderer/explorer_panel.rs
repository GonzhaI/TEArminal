//! Panel flotante del explorador de archivos.

use super::*;

impl TerminalRenderer {
    /// Renderiza el panel lateral derecho del explorador de archivos con estilo overlay translúcido (Floating Island)
    pub fn render_file_explorer(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        top_offset: usize,
        status_bar_h: usize,
        explorer: &crate::explorer::FileExplorer,
        theme: &Theme,
    ) {
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
            buffer,
            width,
            height,
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            theme.chrome_bg(),
        );
        let edge_color = if explorer.is_focused {
            theme.accent_color()
        } else {
            theme.chrome_line()
        };
        Self::draw_rect(
            buffer, width, height, panel_x, panel_y, 1, panel_h, edge_color,
        );

        // 2. Cabecera de sección: icono Lucide y título en caja normal (sin barra de herramientas).
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let header_center_y = panel_y + (header_h / 2);

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            if explorer.changes_only {
                crate::ui::LucideIcon::GitBranch
            } else {
                crate::ui::LucideIcon::Folder
            },
            panel_x + 16,
            header_center_y,
            14.0,
            if explorer.changes_only {
                theme.accent_color()
            } else {
                theme.text_secondary()
            },
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            if explorer.changes_only {
                "Cambios"
            } else {
                "Archivos"
            },
            panel_x + 30,
            header_center_y + 4,
            panel_x + panel_w.saturating_sub(12),
            theme.foreground,
            tokens::SECTION,
            true,
        );

        // 3. Lista de elementos (archivos y carpetas), justo bajo la cabecera.
        let list_y = panel_y
            + header_h
            + crate::explorer::FileExplorer::SUBHEADER_HEIGHT
            + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);

        if explorer.items.is_empty() {
            let empty_base = list_y + 14;
            let (msg, color) = if let Some(ref err) = explorer.error_message {
                (err.as_str(), theme.ansi[3])
            } else if explorer.remote_session.is_some() {
                ("Cargando archivos remotos...", theme.ansi[8])
            } else if explorer.changes_only {
                ("(Sin cambios en git)", theme.ansi[8])
            } else {
                ("(Directorio vacío)", theme.ansi[8])
            };
            self.draw_ui_text(
                buffer,
                width,
                height,
                msg,
                panel_x + 16,
                empty_base,
                panel_x + panel_w - 16,
                color,
                tokens::DETAIL,
                false,
            );
            return;
        }

        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;

        for (rel_idx, item) in explorer
            .items
            .iter()
            .skip(explorer.scroll_offset)
            .enumerate()
        {
            let actual_idx = explorer.scroll_offset + rel_idx;
            let item_y = list_y + rel_idx * item_h;
            if item_y + item_h > list_max_y {
                break;
            }

            let is_hovered = explorer.hovered_index == Some(actual_idx);
            let is_selected = explorer.selected_index == Some(actual_idx)
                || explorer.selected_path.as_ref() == Some(&item.path);

            let has_scrollbar = explorer.items.len() > visible_items;
            let row_w = if has_scrollbar {
                panel_w.saturating_sub(14)
            } else {
                panel_w
            };

            if is_selected {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    panel_x,
                    item_y,
                    row_w,
                    item_h,
                    if explorer.is_focused {
                        theme.surface_hover()
                    } else {
                        theme.surface_bg()
                    },
                );
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    panel_x,
                    item_y,
                    2,
                    item_h,
                    if explorer.is_focused {
                        theme.accent_color()
                    } else {
                        theme.chrome_line_strong()
                    },
                );
            } else if is_hovered {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    panel_x,
                    item_y,
                    row_w,
                    item_h,
                    theme.surface_hover(),
                );
            }

            let indent = item.depth * 12;
            let item_center_y = item_y + (item_h / 2);
            let item_base = item_center_y + 4;
            let content_x = panel_x + 12 + indent;

            // Insignia de Git si el archivo/carpeta tiene estado
            let badge_w = if item.git_status.is_some() { 16 } else { 0 };
            let max_name_x = panel_x + row_w.saturating_sub(badge_w + 6);

            if item.is_dir {
                // Directorio: Lucide Chevron + Folder/FolderOpen
                let chevron_icon = if item.is_expanded {
                    crate::ui::LucideIcon::ChevronDown
                } else {
                    crate::ui::LucideIcon::ChevronRight
                };
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    chevron_icon,
                    content_x + 4,
                    item_center_y,
                    10.5,
                    theme.text_muted(),
                );

                let folder_icon = if item.is_expanded {
                    crate::ui::LucideIcon::FolderOpen
                } else {
                    crate::ui::LucideIcon::Folder
                };
                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    folder_icon,
                    content_x + 18,
                    item_center_y,
                    11.5,
                    if is_selected {
                        theme.accent_color()
                    } else {
                        theme.text_muted()
                    },
                );

                let dir_color = if let Some(git) = item.git_status {
                    match git {
                        crate::explorer::GitFileStatus::Modified => theme.ansi[3], // Amarillo
                        crate::explorer::GitFileStatus::Untracked
                        | crate::explorer::GitFileStatus::Added => theme.ansi[2], // Verde
                        crate::explorer::GitFileStatus::Deleted => theme.ansi[1],  // Rojo
                        crate::explorer::GitFileStatus::Renamed => theme.ansi[6],  // Cian
                    }
                } else if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                let label = item.name.clone();
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &label,
                    content_x + 28,
                    item_base,
                    max_name_x,
                    dir_color,
                    tokens::BODY,
                    is_hovered || is_selected,
                );
            } else {
                // Archivo: Lucide file icons por extensión
                let ext = item.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let (file_icon, file_fg) = match ext {
                    "rs" => (crate::ui::LucideIcon::FileCode, theme.ansi[1]),
                    "toml" | "json" | "yaml" | "yml" => {
                        (crate::ui::LucideIcon::FileJson, theme.ansi[3])
                    }
                    "md" | "txt" => (crate::ui::LucideIcon::FileText, theme.ansi[6]),
                    "png" | "jpg" | "svg" => (crate::ui::LucideIcon::File, theme.ansi[5]),
                    "exe" | "bat" | "ps1" => (crate::ui::LucideIcon::FileCog, theme.ansi[2]),
                    _ => (crate::ui::LucideIcon::File, theme.ansi[8]),
                };

                self.draw_lucide_icon(
                    buffer,
                    width,
                    height,
                    file_icon,
                    content_x + 10,
                    item_center_y,
                    11.5,
                    file_fg,
                );

                let text_color = if let Some(git) = item.git_status {
                    match git {
                        crate::explorer::GitFileStatus::Modified => theme.ansi[3], // Amarillo
                        crate::explorer::GitFileStatus::Untracked
                        | crate::explorer::GitFileStatus::Added => theme.ansi[2], // Verde
                        crate::explorer::GitFileStatus::Deleted => theme.ansi[1],  // Rojo
                        crate::explorer::GitFileStatus::Renamed => theme.ansi[6],  // Cian
                    }
                } else if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.text_secondary()
                };
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &item.name,
                    content_x + 22,
                    item_base,
                    max_name_x,
                    text_color,
                    tokens::BODY,
                    is_selected,
                );
            }

            // Renderizar insignia de Git a la derecha (M, U, A, D)
            if let Some(git) = item.git_status {
                let (badge_letter, badge_color) = match git {
                    crate::explorer::GitFileStatus::Modified => ("M", theme.ansi[3]),
                    crate::explorer::GitFileStatus::Untracked => ("U", theme.ansi[2]),
                    crate::explorer::GitFileStatus::Added => ("A", theme.ansi[2]),
                    crate::explorer::GitFileStatus::Deleted => ("D", theme.ansi[1]),
                    crate::explorer::GitFileStatus::Renamed => ("R", theme.ansi[6]),
                };
                let badge_x = panel_x + row_w.saturating_sub(14);
                self.draw_mono_text(
                    buffer,
                    width,
                    height,
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
        if explorer.items.len() > visible_items && visible_items > 0 {
            let track_w = 4;
            let track_x = panel_x + panel_w.saturating_sub(track_w + 4);
            let track_y = list_y;
            let track_h = list_max_y.saturating_sub(list_y);

            let total_items = explorer.items.len();
            let thumb_h = (((visible_items as f32 / total_items as f32) * track_h as f32).max(18.0)
                as usize)
                .min(track_h);
            let max_offset = total_items.saturating_sub(visible_items);
            let travel = track_h.saturating_sub(thumb_h);
            let thumb_y = if max_offset > 0 {
                track_y
                    + ((explorer.scroll_offset as f32 / max_offset as f32) * travel as f32) as usize
            } else {
                track_y
            };

            Self::draw_scrollbar(
                buffer,
                width,
                height,
                track_x,
                track_y,
                track_w,
                track_h,
                thumb_y,
                thumb_h,
                explorer.hovered_scrollbar,
                self.scrollbars_awake,
                theme,
            );
        }
    }

    pub fn explorer_scrollbar_geometry(
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
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let list_y = panel_y + header_h + sub_h + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);

        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        let total_items = explorer.items.len();
        if total_items <= visible_items || visible_items == 0 {
            return None;
        }

        let track_w = 4;
        let track_x = panel_x + panel_w.saturating_sub(track_w + 4);
        let track_y = list_y;
        let track_h = list_max_y.saturating_sub(list_y);

        let thumb_h = (((visible_items as f32 / total_items as f32) * track_h as f32).max(18.0)
            as usize)
            .min(track_h);
        let max_offset = total_items.saturating_sub(visible_items);
        let travel = track_h.saturating_sub(thumb_h);
        let thumb_y = if max_offset > 0 {
            track_y + ((explorer.scroll_offset as f32 / max_offset as f32) * travel as f32) as usize
        } else {
            track_y
        };

        Some((
            (track_x, track_y, track_w, track_h),
            (track_x, thumb_y, track_w, thumb_h),
        ))
    }
}
