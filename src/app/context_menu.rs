//! Menú contextual (clic derecho): opciones según el elemento bajo el cursor (visor, terminal,
//! pestaña, explorador o ventana).

use super::*;
use crate::renderer::Rect;
use crate::ui::LucideIcon;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // Visor de archivos
    ViewerCopy,
    ViewerCut,
    ViewerPaste,
    ViewerSelectAll,
    ViewerUndo,
    ViewerRedo,
    ViewerSave,
    ViewerFoldAll,
    ViewerUnfoldAll,
    ToggleWordWrap,
    ViewerToggleReadOnly,
    ViewerTogglePreview,
    CopyFilePath,
    CopyFileName,
    // Terminal
    TermCopy,
    TermPaste,
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    ClearTerminal,
    // Pestañas y ventana
    PinTab(usize),
    MoveTabToNewWindow(usize),
    MoveTabToOtherWindow(usize),
    CloseTab(usize),
    CloseOtherTabs(usize),
    NewTab,
    AddProject,
    RemoveProject(usize),
    NewWindow,
    OpenSettings,
    ToggleExplorer,
    TriggerUpdate,
    OpenUrl(String),
    // Explorador
    ExplorerOpen(usize),
    ExplorerCopyPath(PathBuf),
    ExplorerRefresh,
    ExplorerCollapseAll,
    ExplorerExpandAll,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: &'static str,
    pub icon: Option<LucideIcon>,
    pub shortcut: &'static str,
    /// `None` = separador
    pub action: Option<MenuAction>,
    pub enabled: bool,
}

impl MenuItem {
    pub(super) fn new(
        label: &'static str,
        icon: LucideIcon,
        shortcut: &'static str,
        action: MenuAction,
        enabled: bool,
    ) -> Self {
        Self {
            label,
            icon: Some(icon),
            shortcut,
            action: Some(action),
            enabled,
        }
    }

    fn separator() -> Self {
        Self {
            label: "",
            icon: None,
            shortcut: "",
            action: None,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub x: usize,
    pub y: usize,
    /// Ancho calculado con el texto real de etiquetas y atajos
    pub w: usize,
    pub items: Vec<MenuItem>,
    pub hovered: Option<usize>,
}

impl ContextMenu {
    pub const ITEM_H: usize = 28;
    pub const SEP_H: usize = 9;
    pub const PAD: usize = 6;

    /// Crea el menú y lo coloca dentro de la ventana (se abre hacia el lado con espacio).
    fn new(
        items: Vec<MenuItem>,
        w: usize,
        (mx, my): (usize, usize),
        (win_w, win_h): (usize, usize),
    ) -> Self {
        let mut menu = Self {
            x: 0,
            y: 0,
            w,
            items,
            hovered: None,
        };
        menu.x = if mx + w + 4 > win_w {
            mx.saturating_sub(w)
        } else {
            mx
        };
        let h = menu.height();
        menu.y = if my + h + 4 > win_h {
            win_h.saturating_sub(h + 4)
        } else {
            my
        };
        menu
    }

    pub fn height(&self) -> usize {
        Self::PAD * 2
            + self
                .items
                .iter()
                .map(|i| {
                    if i.action.is_some() {
                        Self::ITEM_H
                    } else {
                        Self::SEP_H
                    }
                })
                .sum::<usize>()
    }

    pub fn bounds(&self) -> Rect {
        (self.x, self.y, self.w, self.height())
    }

    /// Rectángulo del elemento `idx` (los separadores también tienen rectángulo)
    pub fn item_rect(&self, idx: usize) -> Rect {
        let mut y = self.y + Self::PAD;
        for (i, item) in self.items.iter().enumerate() {
            let h = if item.action.is_some() {
                Self::ITEM_H
            } else {
                Self::SEP_H
            };
            if i == idx {
                return (self.x + Self::PAD, y, self.w - Self::PAD * 2, h);
            }
            y += h;
        }
        (self.x, y, self.w, 0)
    }

    /// Índice del elemento accionable bajo el ratón
    pub fn item_at(&self, mx: usize, my: usize) -> Option<usize> {
        (0..self.items.len()).find(|&i| {
            self.items[i].action.is_some() && {
                let r = self.item_rect(i);
                mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
            }
        })
    }
}

impl AxonApp {
    /// Abre el menú contextual según lo que haya bajo el ratón. `false` = no hay menú para ese
    /// elemento (o la aplicación del terminal captura el ratón) y el clic sigue su curso normal.
    pub(super) fn open_context_menu(&mut self, mx: usize, my: usize) -> bool {
        let (win_w, win_h) = self
            .window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                (s.width as usize, s.height as usize)
            })
            .unwrap_or((800, 600));
        let offset = self.sidebar_offset();
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let right_offset = self.file_explorer_offset();

        let items = if self.is_horizontal_tabs() && my < TopTabBarRects::HEIGHT {
            let list = self.panel_tab_indices();
            let rects = TopTabBarRects::compute(win_w, list.len());
            let hit = (0..list.len())
                .find(|&pos| {
                    let r = rects.tab_rect(pos);
                    mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
                })
                .map(|pos| list[pos]);
            self.tab_or_window_menu(hit)
        } else if let Some(lx) = self.panel_local_x(mx, my, win_w) {
            // Misma retícula que el dibujo y el clic izquierdo de la barra lateral
            if self.is_horizontal_tabs() {
                let usable_h = win_h.saturating_sub(status_bar_h);
                self.row_menu(my, usable_h)
            } else if lx < crate::renderer::SidebarRects::RAIL_W {
                self.rail_menu(lx, my)
            } else {
                let usable_h = win_h.saturating_sub(status_bar_h);
                let hit = self.panel_tab_at(my, usable_h).map(|(_, tab)| tab);
                self.tab_or_window_menu(hit)
            }
        } else if self.tabs[self.active_tab_index].is_settings {
            return false;
        } else if self.file_explorer.visible && {
            let (px, py, pw, ph) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            mx >= px && mx < px + pw && my >= py && my < py + ph
        } {
            let (_, panel_y, _, _) =
                self.file_explorer
                    .panel_geometry(win_w, win_h, top_offset, status_bar_h);
            let idx = self.file_explorer.item_at_y(my, panel_y);
            self.file_explorer.is_focused = true;
            let mut items = Vec::new();
            if let Some(idx) = idx {
                let item = &self.file_explorer.items[idx];
                self.file_explorer.selected_index = Some(idx);
                self.file_explorer.selected_path = Some(item.path.clone());
                let (label, icon) = if item.is_dir {
                    ("Expandir / contraer", LucideIcon::FolderOpen)
                } else {
                    ("Abrir en el visor", LucideIcon::FileText)
                };
                items.push(MenuItem::new(
                    label,
                    icon,
                    "",
                    MenuAction::ExplorerOpen(idx),
                    true,
                ));
                items.push(MenuItem::new(
                    "Copiar ruta",
                    LucideIcon::Copy,
                    "",
                    MenuAction::ExplorerCopyPath(item.path.clone()),
                    true,
                ));
                items.push(MenuItem::separator());
            }
            let any_open = self.file_explorer.items.iter().any(|i| i.is_expanded);
            items.push(MenuItem::new(
                "Contraer todo",
                LucideIcon::Minimize2,
                "",
                MenuAction::ExplorerCollapseAll,
                any_open,
            ));
            items.push(MenuItem::new(
                "Expandir todo",
                LucideIcon::Maximize2,
                "",
                MenuAction::ExplorerExpandAll,
                self.file_explorer.remote_session.is_none() || self.file_explorer.inband_remote,
            ));
            items.push(MenuItem::separator());
            items.push(MenuItem::new(
                "Actualizar",
                LucideIcon::RefreshCw,
                "",
                MenuAction::ExplorerRefresh,
                true,
            ));
            items
        } else if self.tabs[self.active_tab_index].file_viewer.is_some() {
            let in_viewer = mx >= offset
                && mx < win_w.saturating_sub(right_offset)
                && my >= top_offset
                && my < win_h.saturating_sub(status_bar_h);
            if !in_viewer {
                return false;
            }
            let clicked_url =
                if let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_ref() {
                    let row_h = self.renderer.as_ref().map_or(20, |r| r.row_height);
                    let content_y = top_offset + 38;
                    if my >= content_y {
                        let clicked_row = viewer.scroll_offset + (my - content_y) / row_h.max(16);
                        let lay = self.renderer.as_ref().map(|r| {
                            let viewer_w = win_w.saturating_sub(offset + right_offset);
                            let viewer_h = win_h.saturating_sub(top_offset + status_bar_h);
                            crate::renderer::FileViewerLayout::compute(
                                offset,
                                top_offset,
                                viewer_w,
                                viewer_h,
                                r.row_height,
                                r.char_width,
                                viewer.lines.len(),
                            )
                        });
                        if let Some(lay) = lay {
                            if let Some((l, c)) = viewer.hit_test(clicked_row, mx, &lay) {
                                viewer.link_at(l, c)
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
            self.viewer_menu(clicked_url)
        } else {
            let area_w = win_w.saturating_sub(offset + right_offset);
            let area_h = win_h.saturating_sub(top_offset + status_bar_h);
            let shift = self.modifiers.shift_key();
            let tab = &mut self.tabs[self.active_tab_index];
            let hit = tab
                .compute_pane_rects(offset, top_offset, area_w, area_h)
                .into_iter()
                .find(|r| mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height);
            let Some(rect) = hit else {
                return false;
            };
            let Some(pane) = tab.find_pane(rect.pane_id) else {
                return false;
            };
            // Las aplicaciones TUI que piden el ratón reciben el clic derecho (Shift lo evita)
            if pane.terminal.is_mouse_reporting_active() && !shift {
                return false;
            }
            let clicked_url = if let Some(renderer) = &self.renderer {
                let term_x = mx.saturating_sub(rect.x);
                let term_y = my.saturating_sub(rect.y);
                let col = term_x.saturating_sub(renderer.margin_x) / renderer.char_width;
                let col = col.min(pane.terminal.cols.saturating_sub(1));
                let row_in_view = term_y.saturating_sub(renderer.margin_y) / renderer.row_height;
                let row_in_view = row_in_view.min(pane.terminal.rows.saturating_sub(1));
                let global_line = pane.terminal.visible_line_start() + row_in_view;
                pane.terminal.url_at(col, global_line)
            } else {
                None
            };
            let has_selection = pane.terminal.has_selection();
            tab.active_pane_id = rect.pane_id;
            self.terminal_menu(has_selection, clicked_url)
        };

        let w = self
            .renderer
            .as_ref()
            .map_or(244, |r| r.context_menu_width(&items));
        self.context_menu = Some(ContextMenu::new(items, w, (mx, my), (win_w, win_h)));
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        true
    }

    pub(super) fn tab_or_window_menu(&self, tab: Option<usize>) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if let Some(idx) = tab {
            if self.tabs.get(idx).is_some_and(|t| t.is_preview) {
                items.push(MenuItem::new(
                    "Fijar pestaña",
                    LucideIcon::Check,
                    "",
                    MenuAction::PinTab(idx),
                    true,
                ));
            }
            if self.tabs.len() > 1 && !self.tabs[idx].is_settings {
                items.push(MenuItem::new(
                    "Mover a nueva ventana",
                    LucideIcon::ExternalLink,
                    "",
                    MenuAction::MoveTabToNewWindow(idx),
                    true,
                ));
            }
            if self.has_other_windows && !self.tabs[idx].is_settings {
                items.push(MenuItem::new(
                    "Acoplar a otra ventana",
                    LucideIcon::ChevronRight,
                    "",
                    MenuAction::MoveTabToOtherWindow(idx),
                    true,
                ));
            }
            items.push(MenuItem::new(
                "Cerrar pestaña",
                LucideIcon::X,
                "Ctrl+W",
                MenuAction::CloseTab(idx),
                true,
            ));
            items.push(MenuItem::new(
                "Cerrar las demás",
                LucideIcon::Minus,
                "",
                MenuAction::CloseOtherTabs(idx),
                self.tabs.len() > 1,
            ));
            items.push(MenuItem::separator());
        }
        if crate::updater::get_available_update().is_some() {
            items.push(MenuItem::new(
                "Actualizar Axon",
                LucideIcon::ArrowUp,
                "",
                MenuAction::TriggerUpdate,
                true,
            ));
            items.push(MenuItem::separator());
        }
        items.push(MenuItem::new(
            "Nueva pestaña",
            LucideIcon::Plus,
            "Ctrl+T",
            MenuAction::NewTab,
            true,
        ));
        items.push(MenuItem::new(
            "Nueva ventana",
            LucideIcon::ExternalLink,
            "Ctrl+Shift+N",
            MenuAction::NewWindow,
            true,
        ));
        items.push(MenuItem::new(
            "Explorador de archivos",
            LucideIcon::FolderTree,
            "Ctrl+Shift+E",
            MenuAction::ToggleExplorer,
            true,
        ));
        items.push(MenuItem::new(
            "Ajustes",
            LucideIcon::Settings,
            "Ctrl+,",
            MenuAction::OpenSettings,
            true,
        ));
        items
    }

    fn terminal_menu(&self, has_selection: bool, clicked_url: Option<String>) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if let Some(url) = clicked_url {
            items.push(MenuItem::new(
                "Abrir enlace",
                LucideIcon::ExternalLink,
                "",
                MenuAction::OpenUrl(url),
                true,
            ));
            items.push(MenuItem::separator());
        }
        items.extend([
            MenuItem::new(
                "Copiar",
                LucideIcon::Copy,
                "Ctrl+Shift+C",
                MenuAction::TermCopy,
                has_selection,
            ),
            MenuItem::new(
                "Pegar",
                LucideIcon::ClipboardPaste,
                "Ctrl+V",
                MenuAction::TermPaste,
                true,
            ),
            MenuItem::separator(),
            MenuItem::new(
                "Dividir horizontalmente",
                LucideIcon::Rows2,
                "Ctrl+Alt+D",
                MenuAction::SplitHorizontal,
                true,
            ),
            MenuItem::new(
                "Dividir verticalmente",
                LucideIcon::Columns2,
                "Ctrl+Alt+V",
                MenuAction::SplitVertical,
                true,
            ),
            MenuItem::new(
                "Cerrar panel",
                LucideIcon::X,
                "Ctrl+Alt+W",
                MenuAction::ClosePane,
                true,
            ),
            MenuItem::separator(),
            MenuItem::new(
                "Limpiar pantalla",
                LucideIcon::RefreshCw,
                "Ctrl+Shift+K",
                MenuAction::ClearTerminal,
                true,
            ),
            MenuItem::new(
                "Nueva pestaña",
                LucideIcon::Plus,
                "Ctrl+T",
                MenuAction::NewTab,
                true,
            ),
        ]);
        items
    }

    fn viewer_menu(&self, clicked_url: Option<String>) -> Vec<MenuItem> {
        let Some(v) = self.tabs[self.active_tab_index].file_viewer.as_ref() else {
            return Vec::new();
        };
        let editable = v.is_editable();
        let is_md = v
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"));
        let mut items = Vec::new();
        if let Some(url) = clicked_url {
            items.push(MenuItem::new(
                "Abrir enlace",
                LucideIcon::ExternalLink,
                "",
                MenuAction::OpenUrl(url),
                true,
            ));
            items.push(MenuItem::separator());
        }
        items.extend([
            MenuItem::new(
                "Copiar",
                LucideIcon::Copy,
                "Ctrl+C",
                MenuAction::ViewerCopy,
                !v.is_binary,
            ),
            MenuItem::new(
                "Cortar",
                LucideIcon::Scissors,
                "Ctrl+X",
                MenuAction::ViewerCut,
                editable && v.has_selection(),
            ),
            MenuItem::new(
                "Pegar",
                LucideIcon::ClipboardPaste,
                "Ctrl+V",
                MenuAction::ViewerPaste,
                editable,
            ),
            MenuItem::new(
                "Seleccionar todo",
                LucideIcon::ScanText,
                "Ctrl+A",
                MenuAction::ViewerSelectAll,
                !v.is_binary,
            ),
            MenuItem::separator(),
            MenuItem::new(
                "Deshacer",
                LucideIcon::Undo2,
                "Ctrl+Z",
                MenuAction::ViewerUndo,
                editable && !v.undo_stack.is_empty(),
            ),
            MenuItem::new(
                "Rehacer",
                LucideIcon::Redo2,
                "Ctrl+Y",
                MenuAction::ViewerRedo,
                editable && !v.redo_stack.is_empty(),
            ),
            MenuItem::separator(),
            MenuItem::new(
                "Guardar",
                LucideIcon::Save,
                "Ctrl+S",
                MenuAction::ViewerSave,
                v.is_dirty,
            ),
        ]);
        let foldable = (0..v.lines.len()).any(|i| v.fold_end(i).is_some());
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Contraer todo",
            LucideIcon::Minimize2,
            "",
            MenuAction::ViewerFoldAll,
            foldable,
        ));
        items.push(MenuItem::new(
            "Expandir todo",
            LucideIcon::Maximize2,
            "",
            MenuAction::ViewerUnfoldAll,
            !v.folded.is_empty(),
        ));
        items.push(MenuItem::new(
            if self.config.terminal.word_wrap {
                "Desactivar ajuste de línea"
            } else {
                "Activar ajuste de línea"
            },
            LucideIcon::WrapText,
            "Alt+Z",
            MenuAction::ToggleWordWrap,
            true,
        ));
        items.push(MenuItem::separator());
        items.push(if v.is_read_only {
            MenuItem::new(
                "Pasar a modo edición",
                LucideIcon::Pencil,
                "Ctrl+E",
                MenuAction::ViewerToggleReadOnly,
                !v.is_binary,
            )
        } else {
            MenuItem::new(
                "Pasar a modo lectura",
                LucideIcon::Lock,
                "Ctrl+E",
                MenuAction::ViewerToggleReadOnly,
                true,
            )
        });
        if is_md {
            items.push(if v.render_markdown {
                MenuItem::new(
                    "Ver código fuente",
                    LucideIcon::Code,
                    "",
                    MenuAction::ViewerTogglePreview,
                    true,
                )
            } else {
                MenuItem::new(
                    "Ver vista previa",
                    LucideIcon::Eye,
                    "",
                    MenuAction::ViewerTogglePreview,
                    true,
                )
            });
        }
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Copiar ruta del archivo",
            LucideIcon::Link,
            "",
            MenuAction::CopyFilePath,
            true,
        ));
        items.push(MenuItem::new(
            "Copiar nombre del archivo",
            LucideIcon::FileText,
            "",
            MenuAction::CopyFileName,
            true,
        ));
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Cerrar pestaña",
            LucideIcon::X,
            "Ctrl+W",
            MenuAction::CloseTab(self.active_tab_index),
            true,
        ));
        items
    }

    /// Filas de texto visibles en el visor de la pestaña activa
    pub(super) fn viewer_visible_lines(&self) -> usize {
        let win_h = self
            .window
            .as_ref()
            .map_or(600, |w| w.inner_size().height as usize);
        let row_h = self.renderer.as_ref().map_or(20, |r| r.row_height);
        (win_h.saturating_sub(self.top_offset() + self.status_bar_height() + 38) / row_h.max(16))
            .max(1)
    }

    /// Ctrl+S / «Guardar»: escribe el archivo del visor activo y refresca el estado git del explorador
    pub(super) fn save_active_viewer(&mut self) {
        // Archivo remoto: por la propia sesión SSH si es posible (la confirmación llega después)
        let remote = self.tabs[self.active_tab_index]
            .file_viewer
            .as_ref()
            .and_then(|v| Some((v.remote_destination.clone()?, v.path.clone(), v.is_dirty)));
        if let Some((dest, path, dirty)) = remote {
            if dirty && !self.remote_save_active_viewer() {
                // Sin integración o archivo grande: segunda conexión SSH en un hilo
                self.ssh_save_viewer(&dest, &path);
            }
            return;
        }
        let Some(viewer) = self.tabs[self.active_tab_index].file_viewer.as_mut() else {
            return;
        };
        match viewer.save_to_disk() {
            Ok(()) => {
                crate::logger::log(&format!("-> Archivo guardado: {}", viewer.file_name));
                if self.file_explorer.visible {
                    self.file_explorer.refresh();
                    self.pump_explorer_ssh();
                }
            }
            Err(e) => crate::logger::log(&format!("-> Error al guardar archivo: {}", e)),
        }
    }

    pub(super) fn run_menu_action(&mut self, action: MenuAction) {
        let visible_lines = self.viewer_visible_lines();
        let idx = self.active_tab_index;
        match action {
            MenuAction::ViewerCopy => {
                let text = self.tabs[idx].file_viewer.as_ref().and_then(|v| {
                    v.selected_text()
                        .or_else(|| v.lines.get(v.cursor_line).map(|l| format!("{l}\n")))
                });
                if let Some(text) = text {
                    self.set_clipboard_text(text);
                }
            }
            MenuAction::ViewerCut => {
                let auto_save = self.config.terminal.auto_save;
                let text = self.tabs[idx]
                    .file_viewer
                    .as_ref()
                    .filter(|v| v.is_editable())
                    .and_then(|v| v.selected_text());
                if let Some(text) = text {
                    self.set_clipboard_text(text);
                    if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                        v.delete_selection();
                        if auto_save {
                            v.autosave_local();
                        }
                        v.ensure_cursor_visible(visible_lines);
                    }
                }
            }
            MenuAction::ViewerPaste | MenuAction::TermPaste => self.paste_from_clipboard(),
            MenuAction::ViewerSelectAll => {
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    v.select_all();
                    v.ensure_cursor_visible(visible_lines);
                }
            }
            MenuAction::ViewerUndo | MenuAction::ViewerRedo => {
                let auto_save = self.config.terminal.auto_save;
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    if action == MenuAction::ViewerUndo {
                        v.undo();
                    } else {
                        v.redo();
                    }
                    if auto_save {
                        v.autosave_local();
                    }
                    v.ensure_cursor_visible(visible_lines);
                }
            }
            MenuAction::ViewerSave => self.save_active_viewer(),
            MenuAction::ViewerFoldAll | MenuAction::ViewerUnfoldAll => {
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    if action == MenuAction::ViewerFoldAll {
                        v.fold_all();
                    } else {
                        v.unfold_all();
                    }
                    v.ensure_cursor_visible(visible_lines);
                }
            }
            MenuAction::ToggleWordWrap => self.toggle_word_wrap(),
            MenuAction::ViewerToggleReadOnly => {
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    v.toggle_read_only();
                }
            }
            MenuAction::ViewerTogglePreview => {
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    v.render_markdown = !v.render_markdown;
                }
            }
            MenuAction::CopyFilePath | MenuAction::CopyFileName => {
                let text = self.tabs[idx].file_viewer.as_ref().map(|v| {
                    if action == MenuAction::CopyFileName {
                        v.file_name.clone()
                    } else if let Some(dest) = &v.remote_destination {
                        format!("{}:{}", dest, v.path.display())
                    } else {
                        v.path.display().to_string()
                    }
                });
                if let Some(text) = text {
                    self.set_clipboard_text(text);
                }
            }
            MenuAction::TermCopy => {
                let text = self.tabs[idx]
                    .active_pane_mut()
                    .and_then(|p| p.terminal.copy_selection_text());
                if let Some(text) = text {
                    self.set_clipboard_text(text);
                    if let Some(p) = self.tabs[idx].active_pane_mut() {
                        p.terminal.selection = None;
                    }
                }
            }
            MenuAction::SplitHorizontal => self.split_active_pane(SplitDirection::Horizontal),
            MenuAction::SplitVertical => self.split_active_pane(SplitDirection::Vertical),
            MenuAction::ClosePane => self.close_active_pane(),
            MenuAction::ClearTerminal => {
                if let Some(p) = self.tabs[idx].active_pane_mut() {
                    p.terminal.clear_scrollback();
                    p.pty.write_input(b"\x0c");
                }
            }
            MenuAction::PinTab(i) => {
                if let Some(t) = self.tabs.get_mut(i) {
                    t.is_preview = false;
                }
            }
            MenuAction::MoveTabToNewWindow(i) => self.detach_tab_to_new_window(i),
            MenuAction::MoveTabToOtherWindow(i) => self.move_tab_to_other_window = Some(i),
            MenuAction::CloseTab(i) => self.close_tab(i),
            MenuAction::CloseOtherTabs(keep) => self.close_other_tabs(keep),
            MenuAction::NewTab => self.create_new_tab(),
            MenuAction::AddProject => self.add_project_with_picker(),
            MenuAction::RemoveProject(i) => self.remove_project(i),
            MenuAction::NewWindow => self.new_window_request = true,
            MenuAction::OpenSettings => self.open_or_switch_settings_tab(),
            MenuAction::ToggleExplorer => self.toggle_file_explorer(),
            MenuAction::TriggerUpdate => crate::updater::trigger_update(),
            MenuAction::OpenUrl(url) => {
                crate::app::platform::shell_open_url(&url);
            }
            MenuAction::ExplorerOpen(i) => {
                if let Some(item) = self.file_explorer.items.get(i).cloned() {
                    if item.is_dir {
                        self.file_explorer.toggle_expand(i);
                    } else {
                        self.open_file_in_viewer(&item.path, true);
                    }
                }
            }
            MenuAction::ExplorerCopyPath(path) => {
                self.set_clipboard_text(path.display().to_string());
            }
            MenuAction::ExplorerCollapseAll => self.file_explorer.collapse_all(),
            MenuAction::ExplorerExpandAll => {
                if self.file_explorer.inband_remote {
                    self.file_explorer.remote_expand_all = true;
                    self.remote_expand_next();
                } else {
                    self.file_explorer.expand_all();
                }
            }
            MenuAction::ExplorerRefresh => {
                self.file_explorer.refresh();
                self.pump_explorer_ssh();
                self.sync_fs_watcher();
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(enabled: bool) -> MenuItem {
        MenuItem::new("x", LucideIcon::Copy, "", MenuAction::NewTab, enabled)
    }

    #[test]
    fn test_menu_is_clamped_inside_the_window_and_hit_testing_skips_separators() {
        let items = vec![item(true), MenuItem::separator(), item(true)];
        let menu = ContextMenu::new(items, 244, (790, 590), (800, 600));
        let (x, y, w, h) = menu.bounds();
        assert!(
            x + w <= 800 && y + h <= 600,
            "el menú no debe salirse de la ventana"
        );

        let first = menu.item_rect(0);
        let sep = menu.item_rect(1);
        let last = menu.item_rect(2);
        assert_eq!(menu.item_at(first.0 + 4, first.1 + 4), Some(0));
        assert_eq!(menu.item_at(sep.0 + 4, sep.1 + 2), None);
        assert_eq!(menu.item_at(last.0 + 4, last.1 + 4), Some(2));
        assert_eq!(menu.item_at(0, 0), None);
    }
}
