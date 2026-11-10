//! Menú contextual (clic derecho): opciones según el elemento bajo el cursor (visor, terminal,
//! pestaña, explorador o ventana).

use super::*;
use crate::renderer::Rect;
use crate::ui::LucideIcon;
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MenuAction {
    // Visor de archivos
    ViewerCopy,
    ViewerCut,
    ViewerPaste,
    ViewerSelectAll,
    ViewerSearch,
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
    NewTabInSpace(SpaceId),
    CloseSpace(SpaceId),
    RenameSpace(SpaceId),
    NewSpace,
    NewSpaceWithShell(String),
    PinTab(usize),
    MoveTabToNewWindow(usize),
    MoveTabToOtherWindow(usize),
    CloseTab(usize),
    CloseOtherTabs(usize),
    NewTab,
    NewTabWithShell(String),
    SplitHorizontalWithShell(String),
    SplitVerticalWithShell(String),
    OpenMachineModal,
    NewWindow,
    OpenSettings,
    ToggleExplorer,
    TriggerUpdate,
    OpenUrl(String),
    CopyUrl(String),
    ToggleMaximize,
    MinimizeWindow,
    CloseWindow,
    // Explorador
    ExplorerOpen(usize),
    ExplorerCopyPath(PathBuf),
    ExplorerNewFile,
    ExplorerNewDir,
    ExplorerRename,
    ExplorerDelete,
    ExplorerRefresh,
    ExplorerCollapseAll,
    ExplorerExpandAll,
    FilterExplorer,
    // Modo pantalla encendida (keep-awake)
    SetKeepAwake(KeepAwakeMode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuItem {
    pub(crate) label: Cow<'static, str>,
    pub(crate) icon: Option<LucideIcon>,
    pub(crate) shortcut: &'static str,
    /// `None` = separador
    pub(crate) action: Option<MenuAction>,
    pub(crate) enabled: bool,
}

impl MenuItem {
    pub(super) fn new(
        label: impl Into<Cow<'static, str>>,
        icon: LucideIcon,
        shortcut: &'static str,
        action: MenuAction,
        enabled: bool,
    ) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon),
            shortcut,
            action: Some(action),
            enabled,
        }
    }

    pub(super) fn separator() -> Self {
        Self {
            label: Cow::Borrowed(""),
            icon: None,
            shortcut: "",
            action: None,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ContextMenu {
    pub(crate) x: usize,
    pub(crate) y: usize,
    /// Ancho calculado con el texto real de etiquetas y atajos
    pub(crate) w: usize,
    pub(crate) items: Vec<MenuItem>,
    pub(crate) hovered: Option<usize>,
}

impl ContextMenu {
    pub(crate) const ITEM_H: usize = 28;
    pub(crate) const SEP_H: usize = 9;
    pub(crate) const PAD: usize = 6;

    /// Crea el menú y lo coloca dentro de la ventana (se abre hacia el lado con espacio).
    pub(super) fn new(
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

    pub(crate) fn height(&self) -> usize {
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

    pub(crate) fn bounds(&self) -> Rect {
        (self.x, self.y, self.w, self.height())
    }

    /// Rectángulo del elemento `idx` (los separadores también tienen rectángulo)
    pub(crate) fn item_rect(&self, idx: usize) -> Rect {
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
    pub(crate) fn item_at(&self, mx: usize, my: usize) -> Option<usize> {
        self.items.iter().enumerate().position(|(i, item)| {
            item.action.is_some() && {
                let r = self.item_rect(i);
                mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
            }
        })
    }
}

impl TearminalApp {
    /// Abre el menú contextual según lo que haya bajo el ratón. `false` = no hay menú para ese
    /// elemento (o la aplicación del terminal captura el ratón) y el clic sigue su curso normal.
    pub(super) fn open_context_menu(&mut self, mx: usize, my: usize) -> bool {
        let (win_w, win_h) = self.window_size();
        let offset = self.sidebar_offset();
        let top_offset = self.top_offset();
        let status_bar_h = self.status_bar_height();
        let right_offset = self.file_explorer_offset();

        let items = if self.spaces_rects(win_h).contains(mx, my, win_h) {
            match self.spaces_hit(mx, my, win_h) {
                crate::renderer::SpacesHover::Plus => {
                    let r = self.spaces_rects(win_h);
                    let items = self.space_profiles_menu();
                    self.show_menu(
                        items,
                        (r.plus_btn.0, r.plus_btn.1 + r.plus_btn.3 + 4),
                        (win_w, win_h),
                    );
                    return true;
                }
                crate::renderer::SpacesHover::Row(i) => match self.space_menu(i) {
                    Some(items) => items,
                    None => return false,
                },
                _ => return false,
            }
        } else if my < TopTabBarRects::HEIGHT {
            let visible = self.visible_tab_indices();
            let rects = TopTabBarRects::compute(win_w, self.spaces_w(), visible.len());
            if TopTabBarRects::inside(mx, my, rects.plus_btn) {
                let items = self.shell_profiles_menu();
                self.show_menu(
                    items,
                    (rects.plus_btn.0, rects.plus_btn.1 + rects.plus_btn.3 + 4),
                    (win_w, win_h),
                );
                return true;
            }
            let hit = (0..visible.len())
                .find(|&pos| {
                    let r = rects.tab_rect(pos);
                    mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
                })
                .map(|pos| visible[pos]);
            self.tab_or_window_menu(hit)
        } else if self.panel_local_x(mx, my, win_w, win_h).is_some() {
            // El panel derecho es solo el explorador, que tiene su propio menú
            return false;
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
            let mut items = Vec::with_capacity(8);
            if let Some(idx) = idx {
                if let Some(item) = self.file_explorer.visible_item(idx).cloned() {
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
            }
            let can_manage = self.file_explorer.can_manage_files();
            items.push(MenuItem::new(
                "Nuevo archivo",
                LucideIcon::File,
                "",
                MenuAction::ExplorerNewFile,
                can_manage,
            ));
            items.push(MenuItem::new(
                "Nueva carpeta",
                LucideIcon::Folder,
                "",
                MenuAction::ExplorerNewDir,
                can_manage,
            ));
            if idx.is_some() {
                items.push(MenuItem::new(
                    "Renombrar",
                    LucideIcon::Pencil,
                    "F2",
                    MenuAction::ExplorerRename,
                    can_manage,
                ));
                items.push(MenuItem::new(
                    "Eliminar",
                    LucideIcon::Trash2,
                    "Supr",
                    MenuAction::ExplorerDelete,
                    can_manage,
                ));
            }
            items.push(MenuItem::separator());
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
                self.file_explorer.remote_session.is_none(),
            ));
            items.push(MenuItem::separator());
            items.push(MenuItem::new(
                "Filtrar archivos...",
                LucideIcon::Search,
                "Ctrl+P",
                MenuAction::FilterExplorer,
                true,
            ));
            items.push(MenuItem::new(
                "Actualizar",
                LucideIcon::RefreshCw,
                "",
                MenuAction::ExplorerRefresh,
                true,
            ));
            items
        } else if self.config.appearance.show_status_bar && my >= win_h.saturating_sub(status_bar_h)
        {
            let keep_awake_w = self.renderer.as_ref().map_or(65, |r| {
                r.keep_awake_button_width(self.config.terminal.keep_awake)
            });
            let rects = StatusBarRects::compute_full(win_w, win_h, 0, None, 0, 0, keep_awake_w);
            if StatusBarRects::inside(mx, my, rects.keep_awake_btn) {
                self.keep_awake_menu()
            } else {
                return false;
            }
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
                            let (cx, cy, cw, ch) = self.content_area_rect(win_w, win_h);
                            let show_gutter = !(viewer.render_markdown && viewer.is_markdown());
                            crate::renderer::FileViewerLayout::compute(
                                cx,
                                cy,
                                cw,
                                ch,
                                r.row_height,
                                r.char_width,
                                viewer.lines.len(),
                                show_gutter,
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
            let (cx, cy, cw, ch) = self.content_area_rect(win_w, win_h);
            let shift = self.modifiers.shift_key();
            let tab = &mut self.tabs[self.active_tab_index];
            let hit = tab
                .compute_pane_rects(cx, cy, cw, ch)
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

        self.show_menu(items, (mx, my), (win_w, win_h));
        true
    }

    /// Abre un menú con `items` anclado en `at`, dentro de la ventana `win`.
    pub(super) fn show_menu(
        &mut self,
        items: Vec<MenuItem>,
        at: (usize, usize),
        win: (usize, usize),
    ) {
        let w = self
            .renderer
            .as_ref()
            .map_or(244, |r| r.context_menu_width(&items));
        self.context_menu = Some(ContextMenu::new(items, w, at, win));
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(super) fn tab_or_window_menu(&self, tab: Option<usize>) -> Vec<MenuItem> {
        let mut items = Vec::with_capacity(10);
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
        } else {
            let is_maximized = self.window.as_ref().is_some_and(|w| w.is_maximized());
            if is_maximized {
                items.push(MenuItem::new(
                    "Restaurar",
                    LucideIcon::Copy,
                    "",
                    MenuAction::ToggleMaximize,
                    true,
                ));
            } else {
                items.push(MenuItem::new(
                    "Maximizar",
                    LucideIcon::Square,
                    "",
                    MenuAction::ToggleMaximize,
                    true,
                ));
            }
            items.push(MenuItem::new(
                "Minimizar",
                LucideIcon::Minus,
                "",
                MenuAction::MinimizeWindow,
                true,
            ));
            items.push(MenuItem::separator());
        }
        if crate::updater::get_available_update().is_some() {
            items.push(MenuItem::new(
                "Actualizar TEArminal",
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
        if tab.is_none() {
            let profiles = crate::app::shell_profiles::available_shell_profiles();
            for p in &profiles {
                items.push(MenuItem::new(
                    format!("Nueva pestaña: {}", p.title),
                    p.icon,
                    "",
                    MenuAction::NewTabWithShell(p.command.clone()),
                    true,
                ));
            }
        }
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
        if tab.is_none() {
            items.push(MenuItem::separator());
            items.push(MenuItem::new(
                "Cerrar",
                LucideIcon::X,
                "Alt+F4",
                MenuAction::CloseWindow,
                true,
            ));
        }
        items
    }

    /// Menú desplegable de perfiles de shell y división de paneles (asociado al botón «+»).
    pub(super) fn shell_profiles_menu(&self) -> Vec<MenuItem> {
        let profiles = crate::app::shell_profiles::available_shell_profiles();
        let mut items = Vec::with_capacity(profiles.len() + 6);
        for p in &profiles {
            items.push(MenuItem::new(
                p.title,
                p.icon,
                "",
                MenuAction::NewTabWithShell(p.command.clone()),
                true,
            ));
        }
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Dividir panel en columnas",
            LucideIcon::Columns2,
            "Ctrl+Shift+D",
            MenuAction::SplitVertical,
            true,
        ));
        items.push(MenuItem::new(
            "Dividir panel en filas",
            LucideIcon::Rows2,
            "Alt+Shift+D",
            MenuAction::SplitHorizontal,
            true,
        ));
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Añadir máquina...",
            LucideIcon::Server,
            "",
            MenuAction::OpenMachineModal,
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

    /// Menú desplegable de perfiles de shell para crear un nuevo espacio (botón «+» de Spaces).
    pub(super) fn space_profiles_menu(&self) -> Vec<MenuItem> {
        let profiles = crate::app::shell_profiles::available_shell_profiles();
        let mut items = Vec::with_capacity(profiles.len() + 3);
        for p in &profiles {
            items.push(MenuItem::new(
                p.title,
                p.icon,
                "",
                MenuAction::NewSpaceWithShell(p.command.clone()),
                true,
            ));
        }
        items.push(MenuItem::separator());
        items.push(MenuItem::new(
            "Añadir máquina...",
            LucideIcon::Server,
            "",
            MenuAction::OpenMachineModal,
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
        let mut items = Vec::with_capacity(6);
        if let Some(url) = clicked_url {
            items.push(MenuItem::new(
                "Abrir enlace",
                LucideIcon::ExternalLink,
                "",
                MenuAction::OpenUrl(url.clone()),
                true,
            ));
            items.push(MenuItem::new(
                "Copiar enlace",
                LucideIcon::Copy,
                "",
                MenuAction::CopyUrl(url),
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
        let mut items = Vec::with_capacity(12);
        if let Some(url) = clicked_url {
            items.push(MenuItem::new(
                "Abrir enlace",
                LucideIcon::ExternalLink,
                "",
                MenuAction::OpenUrl(url.clone()),
                true,
            ));
            items.push(MenuItem::new(
                "Copiar enlace",
                LucideIcon::Copy,
                "",
                MenuAction::CopyUrl(url),
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
            MenuItem::new(
                "Buscar",
                LucideIcon::Search,
                "Ctrl+F",
                MenuAction::ViewerSearch,
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
        let foldable = v
            .lines
            .iter()
            .enumerate()
            .any(|(i, _)| v.fold_end(i).is_some());
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

    /// Menú contextual para la opción de modo pantalla encendida (keep-awake)
    pub(super) fn keep_awake_menu(&self) -> Vec<MenuItem> {
        let current = self.config.terminal.keep_awake;
        vec![
            MenuItem::new(
                if current == KeepAwakeMode::Off {
                    "✓ Apagado (ajustes del sistema)"
                } else {
                    "    Apagado (ajustes del sistema)"
                },
                LucideIcon::Eye,
                "",
                MenuAction::SetKeepAwake(KeepAwakeMode::Off),
                true,
            ),
            MenuItem::new(
                if current == KeepAwakeMode::Agent {
                    "✓ Agente (solo trabajando)"
                } else {
                    "    Agente (solo trabajando)"
                },
                LucideIcon::Sparkles,
                "",
                MenuAction::SetKeepAwake(KeepAwakeMode::Agent),
                true,
            ),
            MenuItem::new(
                if current == KeepAwakeMode::Active {
                    "✓ Activo (pantalla siempre encendida)"
                } else {
                    "    Activo (pantalla siempre encendida)"
                },
                LucideIcon::Eye,
                "",
                MenuAction::SetKeepAwake(KeepAwakeMode::Active),
                true,
            ),
        ]
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
                    self.set_clipboard_text(&text);
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
                    self.set_clipboard_text(&text);
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
            MenuAction::ViewerSearch => {
                if let Some(v) = self.tabs[idx].file_viewer.as_mut() {
                    v.open_search(visible_lines);
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
                    self.set_clipboard_text(&text);
                }
            }
            MenuAction::TermCopy => {
                let text = self.tabs[idx]
                    .active_pane_mut()
                    .and_then(|p| p.terminal.copy_selection_text());
                if let Some(text) = text {
                    self.set_clipboard_text(&text);
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
            MenuAction::NewTabInSpace(id) => {
                self.select_space(id);
                self.create_new_tab();
            }
            MenuAction::CloseSpace(id) => self.close_space(id),
            MenuAction::RenameSpace(id) => self.start_renaming_space(id),
            MenuAction::NewSpace => self.create_space(None),
            MenuAction::NewSpaceWithShell(shell) => self.create_space_with_shell(None, &shell),
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
            MenuAction::NewTabWithShell(shell) => self.create_new_tab_with_shell(&shell),
            MenuAction::SplitHorizontalWithShell(shell) => {
                self.split_active_pane_with_shell(SplitDirection::Horizontal, &shell);
            }
            MenuAction::SplitVerticalWithShell(shell) => {
                self.split_active_pane_with_shell(SplitDirection::Vertical, &shell);
            }
            MenuAction::OpenMachineModal => self.open_machine_modal(),
            MenuAction::NewWindow => self.new_window_request = true,
            MenuAction::OpenSettings => self.open_or_switch_settings_tab(),
            MenuAction::ToggleExplorer => self.toggle_file_explorer(),
            MenuAction::TriggerUpdate => crate::updater::trigger_update(),
            MenuAction::OpenUrl(url) => {
                crate::app::platform::shell_open_url(&url);
            }
            MenuAction::CopyUrl(url) => {
                self.set_clipboard_text(&url);
            }
            MenuAction::ToggleMaximize => {
                if let Some(w) = &self.window {
                    w.set_maximized(!w.is_maximized());
                }
            }
            MenuAction::MinimizeWindow => {
                if let Some(w) = &self.window {
                    w.set_minimized(true);
                }
            }
            MenuAction::CloseWindow => self.request_close(),
            MenuAction::ExplorerOpen(i) => {
                if let Some(item) = self.file_explorer.visible_item(i).cloned() {
                    if item.is_dir {
                        if let Some(real_idx) = self.file_explorer.actual_index_for_visible(i) {
                            self.file_explorer.toggle_expand(real_idx);
                            self.pump_explorer_ssh();
                            if self.file_explorer.is_filtering() {
                                self.file_explorer.update_filter();
                            }
                        }
                    } else {
                        if self.file_explorer.is_filtering() {
                            self.file_explorer.close_filter();
                        }
                        self.open_file_in_viewer(&item.path, true);
                    }
                }
            }
            MenuAction::ExplorerCopyPath(path) => {
                self.set_clipboard_text(&path.to_string_lossy());
            }
            MenuAction::ExplorerNewFile => {
                self.file_explorer
                    .begin_edit(crate::explorer::EditKind::NewFile);
            }
            MenuAction::ExplorerNewDir => {
                self.file_explorer
                    .begin_edit(crate::explorer::EditKind::NewDir);
            }
            MenuAction::ExplorerRename => {
                self.file_explorer
                    .begin_edit(crate::explorer::EditKind::Rename);
            }
            MenuAction::ExplorerDelete => self.delete_explorer_selection(),
            MenuAction::ExplorerCollapseAll => self.file_explorer.collapse_all(),
            MenuAction::ExplorerExpandAll => {
                self.file_explorer.expand_all();
                self.pump_explorer_ssh();
            }
            MenuAction::ExplorerRefresh => {
                self.file_explorer.refresh();
                self.pump_explorer_ssh();
                self.sync_fs_watcher();
            }
            MenuAction::FilterExplorer => {
                self.open_explorer_filter();
            }
            MenuAction::SetKeepAwake(mode) => {
                self.set_keep_awake_mode(mode);
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl TearminalApp {
    /// Filas de la lista del explorador que caben a la vista (para dejar visible lo recién creado o seleccionado).
    pub(crate) fn explorer_max_visible(&self) -> usize {
        let win_h = self
            .window
            .as_ref()
            .map_or(600, |w| w.inner_size().height as usize);
        let (_, _, _, panel_h) = self.file_explorer.panel_geometry(
            800,
            win_h,
            self.top_offset(),
            self.status_bar_height(),
        );
        let list_h = panel_h.saturating_sub(
            crate::explorer::FileExplorer::HEADER_HEIGHT + self.file_explorer.subheader_height(),
        );
        (list_h / crate::explorer::FileExplorer::ITEM_HEIGHT).max(1)
    }

    /// Envía a la Papelera el elemento seleccionado del explorador (Supr y «Eliminar» del menú).
    pub(crate) fn delete_explorer_selection(&mut self) {
        let max_visible = self.explorer_max_visible();
        match self.file_explorer.delete_selected(max_visible) {
            Ok(path) => {
                crate::logger::log(&format!("Explorador: a la Papelera {}", path.display()))
            }
            Err(e) => crate::logger::log(&format!("Explorador: no se pudo eliminar: {e}")),
        }
        self.sync_fs_watcher();
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
