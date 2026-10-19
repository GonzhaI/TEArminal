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
        if width <= panel_w + 16 || panel_h == 0 {
            return;
        }

        let border_color = if explorer.is_focused {
            theme.accent_color()
        } else {
            theme.border_color()
        };

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
            if explorer.changes_only {
                crate::ui::LucideIcon::GitBranch
            } else {
                crate::ui::LucideIcon::FolderTree
            },
            panel_x + 20,
            header_center_y,
            11.5,
            theme.accent_color(),
        );

        self.draw_ui_text(
            buffer,
            width,
            height,
            if explorer.changes_only {
                "CAMBIOS"
            } else {
                "ARCHIVOS"
            },
            panel_x + 32,
            header_base,
            panel_x + panel_w.saturating_sub(44),
            theme.foreground,
            11.5,
            true,
        );

        // Botón que alterna árbol de archivos / solo cambios (no hay git en sesiones remotas)
        if explorer.remote_session.is_none() {
            let (bx, by, bw, bh) =
                crate::explorer::FileExplorer::changes_btn_rect(panel_x, panel_y, panel_w);
            let (btn_bg, btn_fg) = if explorer.changes_only {
                (theme.surface_hover(), theme.accent_color())
            } else {
                (theme.surface_bg(), theme.ansi[7])
            };
            crate::ui::draw_rounded_rect(buffer, width, height, bx, by, bw, bh, 5, btn_bg);
            crate::ui::stroke_rounded_rect(
                buffer,
                width,
                height,
                bx,
                by,
                bw,
                bh,
                5,
                if explorer.changes_only {
                    theme.accent_color()
                } else {
                    border_color
                },
            );
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::GitBranch,
                bx + bw / 2,
                by + bh / 2,
                11.0,
                btn_fg,
            );
        }

        // Botón para fijar el explorador (evita que se mueva al cambiar de directorio o pestaña)
        let (px, py, pw, ph) =
            crate::explorer::FileExplorer::pin_btn_rect(panel_x, panel_y, panel_w);
        let (pin_bg, pin_fg) = if explorer.is_pinned {
            (theme.surface_hover(), theme.accent_color())
        } else if explorer.hovered_pin_btn {
            (theme.surface_hover(), theme.foreground)
        } else {
            (theme.surface_bg(), theme.ansi[7])
        };
        crate::ui::draw_rounded_rect(buffer, width, height, px, py, pw, ph, 5, pin_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            px,
            py,
            pw,
            ph,
            5,
            if explorer.is_pinned {
                theme.accent_color()
            } else {
                border_color
            },
        );
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Lock,
            px + pw / 2,
            py + ph / 2,
            11.0,
            pin_fg,
        );

        // 3. Sub-encabezado con botones de navegación [<] [>] [^] y ruta actual (28px)
        let sub_y = panel_y + header_h;
        let sub_h = crate::explorer::FileExplorer::SUBHEADER_HEIGHT;
        let sub_center_y = sub_y + (sub_h / 2);
        let up_base = sub_center_y + 4;

        // Botón [<] Atrás
        let back_x = panel_x + 8;
        let can_back = explorer.can_go_back();
        let back_bg = if explorer.hovered_back_btn && can_back {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, back_x, sub_y + 4, 22, 20, 5, back_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            back_x,
            sub_y + 4,
            22,
            20,
            5,
            border_color,
        );
        let back_fg = if can_back {
            if explorer.hovered_back_btn {
                theme.accent_color()
            } else {
                theme.ansi[7]
            }
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::ChevronLeft,
            back_x + 11,
            sub_y + 14,
            11.0,
            back_fg,
        );

        // Botón [>] Adelante
        let fwd_x = back_x + 24;
        let can_fwd = explorer.can_go_forward();
        let fwd_bg = if explorer.hovered_forward_btn && can_fwd {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, fwd_x, sub_y + 4, 22, 20, 5, fwd_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            fwd_x,
            sub_y + 4,
            22,
            20,
            5,
            border_color,
        );
        let fwd_fg = if can_fwd {
            if explorer.hovered_forward_btn {
                theme.accent_color()
            } else {
                theme.ansi[7]
            }
        } else {
            theme.ansi[8]
        };
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::ChevronRight,
            fwd_x + 11,
            sub_y + 14,
            11.0,
            fwd_fg,
        );

        // Botón [^] Subir directorio
        let up_x = fwd_x + 24;
        let up_bg = if explorer.hovered_up_btn {
            theme.surface_hover()
        } else {
            theme.surface_bg()
        };
        crate::ui::draw_rounded_rect(buffer, width, height, up_x, sub_y + 4, 22, 20, 5, up_bg);
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            up_x,
            sub_y + 4,
            22,
            20,
            5,
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
            up_x + 11,
            sub_y + 14,
            11.0,
            up_fg,
        );

        // Nombre de la carpeta actual con Inter
        let dir_label = if explorer.changes_only {
            format!("{} con cambios", explorer.items.len())
        } else if let Some(ref sess) = explorer.remote_session {
            let name = sess
                .remote_dir
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or("/");
            if name == "/" {
                "/".to_string()
            } else {
                format!("{}/", name)
            }
        } else {
            let dir_name = explorer
                .root_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| explorer.root_dir.to_string_lossy().to_string());
            format!("{}/", dir_name.trim_end_matches('/'))
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &dir_label,
            up_x + 28,
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
                10.5,
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
                panel_w.saturating_sub(18)
            } else {
                panel_w.saturating_sub(12)
            };

            if is_selected {
                let sel_bg = if explorer.is_focused {
                    theme.surface_hover()
                } else {
                    theme.surface_bg()
                };
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 6,
                    item_y,
                    row_w,
                    item_h,
                    6,
                    sel_bg,
                );
                let pill_color = if explorer.is_focused {
                    theme.accent_color()
                } else {
                    theme.ansi[8]
                };
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 8,
                    item_y + 3,
                    3,
                    item_h.saturating_sub(6),
                    2,
                    pill_color,
                );
            } else if is_hovered {
                let hover_row_bg = theme.surface_hover();
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    panel_x + 6,
                    item_y,
                    row_w,
                    item_h,
                    6,
                    hover_row_bg,
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
                    11.5,
                    theme.accent_color(),
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
                    max_name_x,
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
                    theme.ansi[7]
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
                    11.5,
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
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    badge_letter,
                    badge_x,
                    item_base,
                    panel_x + panel_w,
                    badge_color,
                    10.5,
                    true,
                );
            }
        }

        // 5. Barra de desplazamiento del explorador si la lista excede el área visible
        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        if explorer.items.len() > visible_items && visible_items > 0 {
            let track_w = 6;
            let track_x = panel_x + panel_w.saturating_sub(track_w + 3);
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
        let list_y = panel_y + header_h + sub_h + 4;
        let item_h = crate::explorer::FileExplorer::ITEM_HEIGHT;
        let list_max_y = panel_y + panel_h.saturating_sub(6);

        let visible_items = (list_max_y.saturating_sub(list_y)) / item_h;
        let total_items = explorer.items.len();
        if total_items <= visible_items || visible_items == 0 {
            return None;
        }

        let track_w = 6;
        let track_x = panel_x + panel_w.saturating_sub(track_w + 3);
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
