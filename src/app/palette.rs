//! Paleta de comandos y búsqueda central (Ctrl + K, WP5).
//!
//! Permite buscar y conmutar rápidamente entre sesiones (pestañas abiertas),
//! ejecutar comandos y acciones de TEArminal, y abrir archivos del proyecto.

use super::*;
use crate::ui::LucideIcon;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteCategory {
    Session,
    Command,
    File,
}

impl PaletteCategory {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Session => "Sesión",
            Self::Command => "Comando",
            Self::File => "Archivo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PaletteAction {
    SelectTab(usize),
    OpenFile(PathBuf),
    Execute(MenuAction),
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteItem {
    pub category: PaletteCategory,
    pub title: String,
    pub subtitle: String,
    pub icon: LucideIcon,
    pub action: PaletteAction,
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteState {
    pub input: crate::ui::TextInput,
    pub selected: usize,
    pub scroll_offset: usize,
    pub all_items: Vec<PaletteItem>,
    pub filtered: Vec<PaletteItem>,
}

impl PaletteState {
    pub(crate) fn new(items: Vec<PaletteItem>) -> Self {
        let mut s = Self {
            input: crate::ui::TextInput::default(),
            selected: 0,
            scroll_offset: 0,
            all_items: items,
            filtered: Vec::new(),
        };
        s.update_filter();
        s
    }

    #[allow(dead_code)]
    pub(crate) fn query(&self) -> &str {
        self.input.text()
    }

    pub(crate) fn update_filter(&mut self) {
        let q = self.input.text().trim().to_lowercase();
        if q.is_empty() {
            self.filtered = self.all_items.clone();
        } else {
            let mut matches: Vec<(i32, PaletteItem)> = self
                .all_items
                .iter()
                .filter_map(|item| {
                    let title_lower = item.title.to_lowercase();
                    let sub_lower = item.subtitle.to_lowercase();
                    let cat_lower = item.category.label().to_lowercase();

                    if title_lower == q {
                        Some((300, item.clone()))
                    } else if title_lower.starts_with(&q) {
                        Some((200 - (title_lower.len() as i32), item.clone()))
                    } else if let Some(pos) = title_lower.find(&q) {
                        let score = 120 - (pos as i32) * 2;
                        Some((score, item.clone()))
                    } else if sub_lower.starts_with(&q) {
                        Some((90, item.clone()))
                    } else if let Some(pos) = sub_lower.find(&q) {
                        let score = 60 - (pos as i32).min(40);
                        Some((score, item.clone()))
                    } else if cat_lower.starts_with(&q) {
                        Some((30, item.clone()))
                    } else {
                        let parts: Vec<&str> = q.split_whitespace().collect();
                        if parts.len() > 1
                            && parts
                                .iter()
                                .all(|p| title_lower.contains(p) || sub_lower.contains(p))
                        {
                            Some((50, item.clone()))
                        } else {
                            None
                        }
                    }
                })
                .collect();
            matches.sort_by_key(|a| std::cmp::Reverse(a.0));
            self.filtered = matches.into_iter().map(|(_, item)| item).collect();
        }
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
        self.ensure_visible();
    }

    pub(crate) fn ensure_visible(&mut self) {
        let max_vis = crate::renderer::PaletteRects::MAX_VISIBLE;
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + max_vis {
            self.scroll_offset = self.selected.saturating_sub(max_vis - 1);
        }
        if self.filtered.len() <= max_vis {
            self.scroll_offset = 0;
        }
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
            return;
        }
        let len = self.filtered.len() as isize;
        let next = (self.selected as isize + delta).rem_euclid(len);
        self.selected = next as usize;
        self.ensure_visible();
    }

    pub(crate) fn scroll_by(&mut self, steps: isize) {
        let max_vis = crate::renderer::PaletteRects::MAX_VISIBLE;
        let max_scroll = self.filtered.len().saturating_sub(max_vis);
        if max_scroll == 0 {
            self.scroll_offset = 0;
            return;
        }
        let current = self.scroll_offset as isize;
        let next = (current + steps).clamp(0, max_scroll as isize);
        self.scroll_offset = next as usize;
    }
}

impl TearminalApp {
    /// Desplaza la paleta de modo que el pulgar de su barra quede alineado con `thumb_top`.
    pub(super) fn palette_scroll_to_thumb(&mut self, thumb_top: usize) {
        let (win_w, win_h) = self.window_size();
        let Some(palette) = &mut self.palette else {
            return;
        };
        let rects = PaletteRects::compute_with_scroll(
            win_w,
            win_h,
            palette.filtered.len(),
            palette.scroll_offset,
        );
        let Some((track, thumb)) = rects.scrollbar else {
            return;
        };
        let max_scroll = palette
            .filtered
            .len()
            .saturating_sub(PaletteRects::MAX_VISIBLE);
        if let Some(off) =
            crate::renderer::scroll_offset_for_thumb_y(track, thumb, thumb_top as f32, max_scroll)
        {
            palette.scroll_offset = off;
        }
    }

    pub(crate) fn toggle_palette(&mut self) {
        if self.palette.is_some() {
            self.palette = None;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        } else {
            self.open_palette();
        }
    }

    pub(crate) fn handle_palette_key(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
    ) -> bool {
        let (ctrl, shift, alt, _, _) = self.physical_modifiers();

        // Ctrl+K vuelve a alternar/cerrar la paleta
        if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'k') {
            self.palette = None;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        match logical_key {
            Key::Named(NamedKey::Escape) => {
                self.palette = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return true;
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(p) = &mut self.palette {
                    p.move_selection(-1);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(p) = &mut self.palette {
                    p.move_selection(1);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }
            Key::Named(NamedKey::Enter) => {
                let action = self
                    .palette
                    .as_ref()
                    .and_then(|p| p.filtered.get(p.selected))
                    .map(|item| item.action.clone());
                if let Some(action) = action {
                    self.execute_palette_action(action);
                } else {
                    self.palette = None;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }
            _ => {
                if let Some(p) = &mut self.palette {
                    let mut input = std::mem::take(&mut p.input);
                    let (consumed, changed) = self.handle_text_input_key(
                        &mut input,
                        logical_key,
                        physical_key,
                        text.as_deref(),
                    );
                    if let Some(p) = &mut self.palette {
                        p.input = input;
                        if changed {
                            p.update_filter();
                        }
                    }
                    if consumed {
                        return true;
                    }
                }
            }
        }
        true
    }

    pub(crate) fn open_palette(&mut self) {
        let mut items = Vec::new();

        // 1. Sesiones (pestañas abiertas)
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let is_active = idx == self.active_tab_index;
            let (icon, subtitle) = if let Some(viewer) = &tab.file_viewer {
                (LucideIcon::File, viewer.path.display().to_string())
            } else if let Some(pane) = tab.active_pane_mut() {
                let ssh_host = pane
                    .terminal
                    .ssh_session
                    .as_ref()
                    .filter(|s| s.is_active)
                    .map(|s| s.destination.clone());
                let git_branch = pane.get_git_branch();
                let cwd = pane
                    .terminal
                    .current_dir
                    .as_deref()
                    .map(|d| shell_context::shorten_home(d).into_owned())
                    .unwrap_or_default();

                let sub = if let Some(host) = ssh_host {
                    format!("SSH: {host}")
                } else if let Some(branch) = git_branch {
                    format!("git: {branch} ({cwd})")
                } else {
                    cwd
                };

                let ai_status = pane.terminal.active_ai_agent();
                let ic = match ai_status {
                    AiAgentStatus::Agy | AiAgentStatus::Claude => LucideIcon::Sparkles,
                    AiAgentStatus::Gemini
                    | AiAgentStatus::Aider
                    | AiAgentStatus::Copilot
                    | AiAgentStatus::Ollama => LucideIcon::Bot,
                    AiAgentStatus::None => {
                        if pane.terminal.ssh_session.is_some() {
                            LucideIcon::Server
                        } else {
                            LucideIcon::Terminal
                        }
                    }
                };
                (ic, sub)
            } else {
                (LucideIcon::Terminal, String::new())
            };

            let title = if is_active {
                format!("{} (actual)", tab.display_title())
            } else {
                tab.display_title()
            };

            items.push(PaletteItem {
                category: PaletteCategory::Session,
                title,
                subtitle,
                icon,
                action: PaletteAction::SelectTab(idx),
            });
        }

        // 2. Comandos de TEArminal
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Nueva pestaña".to_string(),
            subtitle: "Ctrl+T".to_string(),
            icon: LucideIcon::Plus,
            action: PaletteAction::Execute(MenuAction::NewTab),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Dividir panel horizontalmente".to_string(),
            subtitle: "Ctrl+Shift+D".to_string(),
            icon: LucideIcon::Rows2,
            action: PaletteAction::Execute(MenuAction::SplitHorizontal),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Dividir panel verticalmente".to_string(),
            subtitle: "Alt+Shift+D".to_string(),
            icon: LucideIcon::Columns2,
            action: PaletteAction::Execute(MenuAction::SplitVertical),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Cerrar panel activo".to_string(),
            subtitle: "Ctrl+Shift+W".to_string(),
            icon: LucideIcon::X,
            action: PaletteAction::Execute(MenuAction::ClosePane),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Cerrar pestaña actual".to_string(),
            subtitle: "Ctrl+W".to_string(),
            icon: LucideIcon::X,
            action: PaletteAction::Execute(MenuAction::CloseTab(self.active_tab_index)),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Abrir Ajustes".to_string(),
            subtitle: "Ctrl+,".to_string(),
            icon: LucideIcon::Settings,
            action: PaletteAction::Execute(MenuAction::OpenSettings),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Mostrar u ocultar explorador de archivos".to_string(),
            subtitle: "Ctrl+B".to_string(),
            icon: LucideIcon::Folder,
            action: PaletteAction::Execute(MenuAction::ToggleExplorer),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Limpiar pantalla y scrollback".to_string(),
            subtitle: "Ctrl+Shift+K".to_string(),
            icon: LucideIcon::Trash2,
            action: PaletteAction::Execute(MenuAction::ClearTerminal),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Alternar ajuste de línea (Word Wrap)".to_string(),
            subtitle: "Alt+Z".to_string(),
            icon: LucideIcon::Code,
            action: PaletteAction::Execute(MenuAction::ToggleWordWrap),
        });
        items.push(PaletteItem {
            category: PaletteCategory::Command,
            title: "Alternar vista previa Markdown".to_string(),
            subtitle: "Ctrl+Shift+M".to_string(),
            icon: LucideIcon::Eye,
            action: PaletteAction::Execute(MenuAction::ViewerTogglePreview),
        });

        // 3. Archivos del proyecto y directorio actual
        let current_dir = self
            .tabs
            .get(self.active_tab_index)
            .and_then(|tab| {
                if let Some(viewer) = &tab.file_viewer {
                    viewer.path.parent().map(|p| p.to_path_buf())
                } else if let Some(pane) = tab.active_pane() {
                    pane.terminal.current_dir.as_ref().map(PathBuf::from)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.file_explorer.root_dir.clone());

        let project_files = collect_project_files(&current_dir, 2000);
        for (path, name, rel_path) in project_files {
            items.push(PaletteItem {
                category: PaletteCategory::File,
                title: name,
                subtitle: rel_path,
                icon: crate::ui::icons::file_icon(&path),
                action: PaletteAction::OpenFile(path),
            });
        }

        self.palette_scrollbar_dragging = false;
        self.palette = Some(PaletteState::new(items));
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn execute_palette_action(&mut self, action: PaletteAction) {
        self.palette = None;
        match action {
            PaletteAction::SelectTab(idx) => {
                if idx < self.tabs.len() {
                    self.switch_to_tab(idx);
                }
            }
            PaletteAction::OpenFile(path) => {
                self.open_file_in_viewer(&path, true);
            }
            PaletteAction::Execute(menu_action) => {
                self.run_menu_action(menu_action);
            }
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

/// Recorre el directorio raíz indexando archivos para búsqueda rápida, ignorando carpetas de dependencias y temporales.
fn collect_project_files(
    root: &std::path::Path,
    max_files: usize,
) -> Vec<(PathBuf, String, String)> {
    let mut results = Vec::new();
    if !root.is_dir() {
        return results;
    }

    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let ignore_names = [
        ".git",
        "target",
        "node_modules",
        ".next",
        "dist",
        "build",
        "vendor",
        ".venv",
        "venv",
        "__pycache__",
        ".idea",
        ".vscode",
        "coverage",
        ".turbo",
        "bin",
        "obj",
        ".cargo",
    ];

    while let Some((dir, depth)) = stack.pop() {
        if depth > 7 || results.len() >= max_files {
            continue;
        }
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if ignore_names
                .iter()
                .any(|&ign| ign.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                stack.push((path, depth + 1));
            } else {
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| name.clone());
                results.push((path, name, rel));
                if results.len() >= max_files {
                    break;
                }
            }
        }
    }
    results.sort_by_key(|a| a.1.to_lowercase());
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_filter_matches_title_and_subtitle() {
        let items = vec![
            PaletteItem {
                category: PaletteCategory::Session,
                title: "PowerShell".to_string(),
                subtitle: "C:\\Users\\dev".to_string(),
                icon: LucideIcon::Terminal,
                action: PaletteAction::SelectTab(0),
            },
            PaletteItem {
                category: PaletteCategory::Command,
                title: "Nueva pestaña".to_string(),
                subtitle: "Ctrl+T".to_string(),
                icon: LucideIcon::Plus,
                action: PaletteAction::Execute(MenuAction::NewTab),
            },
            PaletteItem {
                category: PaletteCategory::File,
                title: "main.rs".to_string(),
                subtitle: "src\\main.rs".to_string(),
                icon: LucideIcon::File,
                action: PaletteAction::OpenFile(PathBuf::from("src/main.rs")),
            },
        ];

        let mut state = PaletteState::new(items);
        assert_eq!(state.filtered.len(), 3);
        assert_eq!(state.selected, 0);

        // Buscar "nueva"
        state.input.set_text("nueva");
        state.update_filter();
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0].title, "Nueva pestaña");

        // Buscar por subtítulo "main.rs"
        state.input.set_text("main");
        state.update_filter();
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0].title, "main.rs");

        // Buscar sin coincidencia
        state.input.set_text("xyz123nonexistent");
        state.update_filter();
        assert_eq!(state.filtered.len(), 0);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_palette_navigation_clamps_and_wraps() {
        let items = vec![
            PaletteItem {
                category: PaletteCategory::Command,
                title: "1".to_string(),
                subtitle: "".to_string(),
                icon: LucideIcon::Plus,
                action: PaletteAction::Execute(MenuAction::NewTab),
            },
            PaletteItem {
                category: PaletteCategory::Command,
                title: "2".to_string(),
                subtitle: "".to_string(),
                icon: LucideIcon::Plus,
                action: PaletteAction::Execute(MenuAction::NewTab),
            },
            PaletteItem {
                category: PaletteCategory::Command,
                title: "3".to_string(),
                subtitle: "".to_string(),
                icon: LucideIcon::Plus,
                action: PaletteAction::Execute(MenuAction::NewTab),
            },
        ];

        let mut state = PaletteState::new(items);
        assert_eq!(state.selected, 0);
        state.move_selection(1);
        assert_eq!(state.selected, 1);
        state.move_selection(1);
        assert_eq!(state.selected, 2);
        state.move_selection(1);
        assert_eq!(state.selected, 0); // Wraps around

        state.move_selection(-1);
        assert_eq!(state.selected, 2); // Wraps backwards
    }

    #[test]
    fn test_palette_geometry_bounds() {
        let rects = PaletteRects::compute(1000, 700, 5);
        assert_eq!(rects.card.2, PaletteRects::WIDTH);
        assert_eq!(rects.card.0, (1000 - PaletteRects::WIDTH) / 2);
        assert_eq!(rects.item_rect(0).3, PaletteRects::ITEM_H);
        assert!(PaletteRects::inside(
            rects.card.0 + 10,
            rects.card.1 + 10,
            rects.card
        ));
        assert!(!PaletteRects::inside(0, 0, rects.card));
    }

    #[test]
    fn test_palette_scroll_offset_and_ensure_visible() {
        let items: Vec<PaletteItem> = (0..20)
            .map(|i| PaletteItem {
                category: PaletteCategory::Command,
                title: format!("Comando {i}"),
                subtitle: String::new(),
                icon: LucideIcon::Command,
                action: PaletteAction::Execute(MenuAction::NewTab),
            })
            .collect();

        let mut state = PaletteState::new(items);
        assert_eq!(state.scroll_offset, 0);
        assert_eq!(state.selected, 0);

        // Moverse hacia abajo más allá de MAX_VISIBLE (8)
        for _ in 0..10 {
            state.move_selection(1);
        }
        assert_eq!(state.selected, 10);
        assert!(state.scroll_offset >= 3);
        assert!(state.selected >= state.scroll_offset);
        assert!(state.selected < state.scroll_offset + PaletteRects::MAX_VISIBLE);

        // Volver al inicio envolviendo hacia atrás
        state.move_selection(-11);
        assert_eq!(state.selected, 19);
        assert!(state.scroll_offset >= 12);
    }

    #[test]
    fn test_collect_project_files_finds_nested_files() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let files = collect_project_files(&manifest_dir, 500);
        assert!(!files.is_empty());
        // Debe encontrar archivos dentro de subcarpetas como src/
        let has_src_file = files
            .iter()
            .any(|(_, name, rel)| name == "main.rs" && rel.contains("src"));
        assert!(has_src_file, "debe encontrar src/main.rs");
    }

    #[test]
    fn test_palette_scrollbar_hit_does_not_activate_item() {
        let (win_w, win_h) = (800, 600);
        let items_count = 20;
        let rects = PaletteRects::compute_with_scroll(win_w, win_h, items_count, 0);
        let (track, _) = rects.scrollbar.expect("debe haber scrollbar para 20 items");

        let right = rects.card.0 + rects.card.2;
        let scrollbar_x = track.0 + 2;
        let scrollbar_y = track.1 + 10;
        assert!(crate::renderer::ScrollbarRects::hit(
            track,
            right,
            scrollbar_x,
            scrollbar_y
        ));

        // Un clic en el área del scrollbar no debe activar el item que queda debajo
        assert_eq!(rects.item_at_with_offset(scrollbar_x, scrollbar_y, 0), None);

        // Un clic en el cuerpo del item sí debe activarlo
        let item_0_x = rects.card.0 + 50;
        let item_0_y = rects.list.1 + 10;
        assert_eq!(rects.item_at_with_offset(item_0_x, item_0_y, 0), Some(0));
    }
}
