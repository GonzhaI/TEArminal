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
            crate::explorer::FileExplorer::panel_geometry(width, height, top_offset, status_bar_h);
        if width <= panel_w + 16 || panel_h == 0 {
            return;
        }

        let border_color = theme.border_color();

        // 1. Fondo del panel con esquinas redondeadas y translucidez suave
        let bg_color = Color::from_rgba(
            theme.surface_bg().r(),
            theme.surface_bg().g(),
            theme.surface_bg().b(),
            230,
        );
        crate::ui::draw_rounded_rect(
            buffer, width, height, panel_x, panel_y, panel_w, panel_h, 12, bg_color,
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            12,
            border_color,
        );

        // 2. Encabezado del explorador (36px)
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let header_center_y = panel_y + (header_h / 2);
        let header_base = header_center_y + 4;

        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::FolderTree,
            panel_x + 20,
            header_center_y,
            13.5,
            theme.accent_color(),
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            "ARCHIVOS",
            panel_x + 32,
            header_base,
            panel_x + panel_w.saturating_sub(60),
            theme.foreground,
            11.5,
            true,
        );

        // Botón '↻' refrescar
        let ref_x = panel_x + panel_w.saturating_sub(56);
        let ref_bg = if explorer.hovered_refresh_btn {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        crate::ui::draw_rounded_rect(buffer, width, height, ref_x, panel_y + 7, 22, 22, 6, ref_bg);
        let ref_fg = if explorer.hovered_refresh_btn {
            theme.foreground
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::RefreshCw,
            ref_x + 11,
            panel_y + 18,
            12.0,
            ref_fg,
        );

        // Botón 'x' cerrar
        let close_x = panel_x + panel_w.saturating_sub(28);
        let close_bg = if explorer.hovered_close_btn {
            theme.surface_hover()
        } else {
            Color::TRANSPARENT
        };
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            close_x,
            panel_y + 7,
            22,
            22,
            6,
            close_bg,
        );
        let close_fg = if explorer.hovered_close_btn {
            theme.ansi[1]
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::X,
            close_x + 11,
            panel_y + 18,
            12.0,
            close_fg,
        );

        // 3. Sub-encabezado con ruta actual y botón [..] (28px)
        let sub_y = panel_y + header_h;
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let sub_center_y = sub_y + (sub_h / 2);
        let up_base = sub_center_y + 4;

        let up_x = panel_x + 10;
        let up_bg = if explorer.hovered_up_btn {
            theme.surface_hover()
        } else {
            Color::from_rgba(255, 255, 255, 8)
        };
        crate::ui::draw_rounded_rect(buffer, width, height, up_x, sub_y + 4, 26, 20, 6, up_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            up_x,
            sub_y + 4,
            26,
            20,
            6,
            border_color,
        );

        let up_fg = if explorer.hovered_up_btn {
            theme.accent_color()
        } else {
            theme.ansi[7]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::ArrowUp,
            up_x + 13,
            sub_y + 14,
            11.5,
            up_fg,
        );

        // Nombre de la carpeta actual con Inter
        let dir_name = explorer
            .root_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| explorer.root_dir.to_string_lossy().to_string());
        let dir_label = format!("{}/", dir_name);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &dir_label,
            up_x + 34,
            up_base,
            panel_x + panel_w - 8,
            theme.ansi[8],
            11.0,
            false,
        );

        // 4. Lista de elementos (archivos y carpetas)
        let list_y = sub_y + sub_h + 4;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(6);

        if explorer.items.is_empty() {
            let empty_base = list_y + 14;
            self.draw_ui_text(
                buffer,
                width,
                height,
                "(Directorio vacío)",
                panel_x + 16,
                empty_base,
                panel_x + panel_w - 16,
                theme.ansi[8],
                11.0,
                false,
            );
            return;
        }

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

            if is_selected {
                let sel_bg = theme.surface_hover();
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 6,
                    item_y,
                    panel_w.saturating_sub(12),
                    item_h,
                    6,
                    sel_bg,
                );
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 8,
                    item_y + 3,
                    3,
                    item_h.saturating_sub(6),
                    2,
                    theme.accent_color(),
                );
            } else if is_hovered {
                let hover_row_bg = Color::from_rgba(255, 255, 255, 12);
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 6,
                    item_y,
                    panel_w.saturating_sub(12),
                    item_h,
                    6,
                    hover_row_bg,
                );
            }

            let indent = item.depth * 12;
            let item_center_y = item_y + (item_h / 2);
            let item_base = item_center_y + 4;
            let content_x = panel_x + 12 + indent;

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
                    theme.ansi[8],
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
                    12.5,
                    theme.accent_color(),
                );

                let dir_color = if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.ansi[7]
                };
                let label = format!("{}/", item.name);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &label,
                    content_x + 28,
                    item_base,
                    panel_x + panel_w - 8,
                    dir_color,
                    11.5,
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
                    12.0,
                    file_fg,
                );

                let text_color = if is_selected {
                    theme.accent_color()
                } else if is_hovered {
                    theme.foreground
                } else {
                    theme.ansi[7]
                };
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &item.name,
                    content_x + 22,
                    item_base,
                    panel_x + panel_w.saturating_sub(14),
                    text_color,
                    11.5,
                    is_selected,
                );
            }
        }

        // 5. Barra de desplazamiento del explorador si la lista excede el área visible
        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        if explorer.items.len() > visible_items && visible_items > 0 {
            let track_w = 4;
            let track_x = panel_x + panel_w.saturating_sub(track_w + 5);
            let track_y = list_y;
            let track_h = list_max_y.saturating_sub(list_y);

            // Pista sutil
            let track_bg = Color::from_rgba(
                theme.surface_hover().r(),
                theme.surface_hover().g(),
                theme.surface_hover().b(),
                120,
            );
            crate::ui::draw_rounded_rect(
                buffer, width, height, track_x, track_y, track_w, track_h, 2, track_bg,
            );

            // Pulgar claramente visible
            let total_items = explorer.items.len();
            let thumb_h = (((visible_items as f32 / total_items as f32) * track_h as f32).max(20.0)
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

            let thumb_color = theme.ansi[8];
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                track_x,
                thumb_y,
                track_w,
                thumb_h,
                2,
                thumb_color,
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
            crate::explorer::FileExplorer::panel_geometry(width, height, top_offset, status_bar_h);
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let list_y = panel_y + header_h + sub_h + 4;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(6);

        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        let total_items = explorer.items.len();
        if total_items <= visible_items || visible_items == 0 {
            return None;
        }

        let track_w = 6;
        let track_x = panel_x + panel_w.saturating_sub(track_w + 5);
        let track_y = list_y;
        let track_h = list_max_y.saturating_sub(list_y);

        let thumb_h = (((visible_items as f32 / total_items as f32) * track_h as f32).max(20.0)
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
