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
        let edge_color = if explorer.is_focused {
            theme.accent_color()
        } else {
            theme.chrome_line()
        };
        Self::draw_rect(canvas, panel_x, panel_y, 1, panel_h, edge_color);

        // 2. Pestañas «Archivos» · «Cambios»: la activa lleva el acento y una línea de 2 px bajo ella,
        //    sobre un filete que cruza las dos mitades.
        let header_h = crate::explorer::FileExplorer::HEADER_HEIGHT;
        let tabs = ExplorerTabsRects::compute(panel_x, panel_y, panel_w, header_h);
        Self::draw_rect(
            canvas,
            panel_x + 8,
            panel_y + header_h - 1,
            panel_w.saturating_sub(16),
            1,
            theme.chrome_line(),
        );
        let change_count = explorer.change_count();
        for (rect, label, active, hovered) in [
            (
                tabs.files,
                "Archivos",
                !explorer.changes_only,
                explorer.hovered_tab == Some(false),
            ),
            (
                tabs.changes,
                "Cambios",
                explorer.changes_only,
                explorer.hovered_tab == Some(true),
            ),
        ] {
            let color = if active {
                theme.accent_color()
            } else if hovered {
                theme.foreground
            } else {
                theme.text_secondary()
            };
            let count = (label == "Cambios" && change_count > 0).then(|| change_count.to_string());
            let label_w = self.measure_ui_text(label, tokens::TITLE, active);
            let count_w = count
                .as_ref()
                .map_or(0, |c| 6 + self.measure_mono_text(c, tokens::MICRO, false));
            let x = rect.0 + rect.2.saturating_sub(label_w + count_w) / 2;
            let baseline = rect.1 + (rect.3 - 1) / 2 + 4;
            self.draw_ui_text(
                canvas,
                label,
                x,
                baseline,
                rect.0 + rect.2,
                color,
                tokens::TITLE,
                active,
            );
            if let Some(count) = count {
                self.draw_mono_text(
                    canvas,
                    &count,
                    x + label_w + 6,
                    baseline,
                    rect.0 + rect.2,
                    if active { color } else { theme.text_muted() },
                    tokens::MICRO,
                    false,
                );
            }
            if active {
                Self::draw_rect(
                    canvas,
                    rect.0,
                    rect.1 + rect.3 - 2,
                    rect.2,
                    2,
                    theme.accent_color(),
                );
            }
        }

        // 2b. Micro-herramientas de la cabecera estilo DataGrip (+, colapsar todo, refrescar)
        let tool_btns = [
            (tabs.new_btn, crate::ui::LucideIcon::Plus, explorer.hovered_new_btn),
            (tabs.collapse_btn, crate::ui::LucideIcon::Minus, explorer.hovered_collapse_btn),
            (tabs.refresh_btn, crate::ui::LucideIcon::RefreshCw, explorer.hovered_refresh_btn),
        ];
        for (btn_rect, icon, hovered) in tool_btns {
            if let Some(r) = btn_rect {
                if hovered {
                    canvas.rounded_fill(r.0, r.1, r.2, r.3, tokens::RADIUS_ROW, theme.surface_hover());
                }
                let fg = if hovered {
                    theme.foreground
                } else {
                    theme.text_muted()
                };
                self.draw_lucide_icon(canvas, icon, r.0 + r.2 / 2, r.1 + r.3 / 2, 11.0, fg);
            }
        }

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
                canvas,
                msg,
                panel_x + 24,
                empty_base,
                panel_x + panel_w - 24,
                color,
                tokens::DETAIL,
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
            let item_base = item_center_y + 4;
            let content_x = panel_x + 16 + indent;

            // Guías visuales de sangría estilo DataGrip / JetBrains para niveles anidados
            if item.depth > 0 {
                let guide_color = theme.mix(theme.background, theme.foreground, 0.08);
                let clip = (panel_x, panel_y, panel_w, panel_h);
                for d in 0..item.depth {
                    let guide_x = panel_x + 16 + d * 12 + 4;
                    canvas.fill_rect(clip, guide_x, item_y, 1, item_h, guide_color);
                }
            }

            // Insignia de Git si el archivo/carpeta tiene estado
            let badge_w = if item.git_status.is_some() { 16 } else { 0 };
            let max_name_x = panel_x + row_w.saturating_sub(SidebarRects::PAD_R + badge_w + 6);

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
                    canvas,
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
                    canvas,
                    file_icon,
                    content_x + 10,
                    item_center_y,
                    11.5,
                    file_fg,
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
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let list_y = panel_y + header_h + sub_h + crate::explorer::FileExplorer::LIST_TOP_PAD;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(8);

        ScrollbarRects::vertical(
            panel_x + panel_w,
            list_y,
            list_max_y.saturating_sub(list_y),
            explorer.items.len(),
            (list_max_y.saturating_sub(list_y)) / item_h,
            explorer.scroll_offset,
        )
    }
}
