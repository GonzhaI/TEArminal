use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const FILE_EXPLORER_WIDTH: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Modified,
    Untracked,
    Added,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone)]
pub struct ExplorerItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub is_expanded: bool,
    pub git_status: Option<GitFileStatus>,
}

#[derive(Debug, Clone)]
pub struct FileExplorer {
    pub root_dir: PathBuf,
    pub items: Vec<ExplorerItem>,
    pub scroll_offset: usize,
    pub hovered_index: Option<usize>,
    pub hovered_close_btn: bool,
    pub hovered_back_btn: bool,
    pub hovered_forward_btn: bool,
    pub hovered_up_btn: bool,
    pub hovered_refresh_btn: bool,
    pub selected_index: Option<usize>,
    pub selected_path: Option<PathBuf>,
    pub visible: bool,
    pub hovered_scrollbar: bool,
    pub is_focused: bool,
    pub last_click_item: Option<usize>,
    pub last_click_time: std::time::Instant,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub git_statuses: HashMap<PathBuf, GitFileStatus>,
}

impl FileExplorer {
    pub const HEADER_HEIGHT: usize = 36;
    pub const SUBHEADER_HEIGHT: usize = 28;
    pub const ITEM_HEIGHT: usize = 24;

    /// Calcula la geometría exacta del panel flotante del explorador de archivos
    #[inline]
    pub fn panel_geometry(
        win_w: usize,
        win_h: usize,
        top_offset: usize,
        status_bar_h: usize,
    ) -> (usize, usize, usize, usize) {
        let margin_top = top_offset + 8;
        let margin_bottom = status_bar_h + 8;
        let panel_x = win_w.saturating_sub(FILE_EXPLORER_WIDTH + 8);
        let panel_y = margin_top;
        let panel_w = FILE_EXPLORER_WIDTH;
        let panel_h = win_h.saturating_sub(margin_top + margin_bottom);
        (panel_x, panel_y, panel_w, panel_h)
    }

    pub fn new(initial_dir: Option<PathBuf>, visible: bool) -> Self {
        let root_dir = initial_dir
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut explorer = Self {
            root_dir: root_dir.clone(),
            items: Vec::new(),
            scroll_offset: 0,
            hovered_index: None,
            hovered_close_btn: false,
            hovered_back_btn: false,
            hovered_forward_btn: false,
            hovered_up_btn: false,
            hovered_refresh_btn: false,
            selected_index: None,
            selected_path: None,
            visible,
            hovered_scrollbar: false,
            is_focused: false,
            last_click_item: None,
            last_click_time: std::time::Instant::now(),
            history: vec![root_dir],
            history_index: 0,
            git_statuses: HashMap::new(),
        };

        explorer.refresh();
        explorer
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    pub fn go_back(&mut self) {
        if self.can_go_back() {
            self.history_index -= 1;
            let target = self.history[self.history_index].clone();
            if target.is_dir() {
                self.root_dir = target;
                self.scroll_offset = 0;
                self.selected_index = None;
                self.refresh();
            }
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            self.history_index += 1;
            let target = self.history[self.history_index].clone();
            if target.is_dir() {
                self.root_dir = target;
                self.scroll_offset = 0;
                self.selected_index = None;
                self.refresh();
            }
        }
    }

    /// Actualiza el directorio raíz y refresca la lista de elementos
    pub fn set_root(&mut self, new_root: PathBuf) {
        if new_root != self.root_dir && new_root.is_dir() {
            if self.history_index + 1 < self.history.len() {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(new_root.clone());
            self.history_index = self.history.len() - 1;

            self.root_dir = new_root;
            self.scroll_offset = 0;
            self.selected_index = None;
            self.refresh();
        }
    }

    /// Consulta de manera no bloqueante el estado Git del repositorio activo
    pub fn refresh_git_status(&mut self) {
        self.git_statuses.clear();

        let mut cur = self.root_dir.as_path();
        let mut git_repo_root = None;
        loop {
            if cur.join(".git").exists() {
                git_repo_root = Some(cur.to_path_buf());
                break;
            }
            match cur.parent() {
                Some(p) => cur = p,
                None => break,
            }
        }

        let Some(repo_root) = git_repo_root else {
            return;
        };

        let mut cmd = std::process::Command::new("git");
        cmd.args(["status", "--porcelain", "-uall"])
            .current_dir(&repo_root);

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.len() < 4 {
                        continue;
                    }
                    let status_code = &line[0..2];
                    let rel_path = line[3..].trim().trim_matches('"');
                    let file_path = repo_root.join(rel_path.replace('/', "\\"));

                    let status = if status_code.contains('M') {
                        GitFileStatus::Modified
                    } else if status_code.contains('?') {
                        GitFileStatus::Untracked
                    } else if status_code.contains('A') {
                        GitFileStatus::Added
                    } else if status_code.contains('D') {
                        GitFileStatus::Deleted
                    } else if status_code.contains('R') {
                        GitFileStatus::Renamed
                    } else {
                        GitFileStatus::Modified
                    };

                    self.git_statuses.insert(file_path, status);
                }
            }
        }
    }

    /// Refresca los contenidos del directorio raíz conservando las carpetas expandidas
    pub fn refresh(&mut self) {
        self.refresh_git_status();

        let expanded_paths: HashSet<PathBuf> = self
            .items
            .iter()
            .filter(|it| it.is_dir && it.is_expanded)
            .map(|it| it.path.clone())
            .collect();

        self.items.clear();
        Self::read_directory(
            &self.root_dir,
            0,
            &expanded_paths,
            &self.git_statuses,
            &mut self.items,
        );
    }

    fn read_directory(
        dir: &Path,
        depth: usize,
        expanded_paths: &HashSet<PathBuf>,
        git_map: &HashMap<PathBuf, GitFileStatus>,
        out: &mut Vec<ExplorerItem>,
    ) {
        let Ok(read_dir) = fs::read_dir(dir) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Ocultar ruido excesivo del repositorio como .git
            if name == ".git" {
                continue;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push((path, name));
            } else {
                files.push((path, name));
            }
        }

        // Ordenar alfabéticamente sin distinguir mayúsculas/minúsculas
        dirs.sort_by_key(|a| a.1.to_lowercase());
        files.sort_by_key(|a| a.1.to_lowercase());

        // Directorios primero
        for (path, name) in dirs {
            let is_expanded = expanded_paths.contains(&path);

            let has_git_changes = git_map.keys().any(|p| p.starts_with(&path));
            let git_status = if has_git_changes {
                Some(GitFileStatus::Modified)
            } else {
                None
            };

            out.push(ExplorerItem {
                path: path.clone(),
                name,
                is_dir: true,
                depth,
                is_expanded,
                git_status,
            });

            if is_expanded {
                Self::read_directory(&path, depth + 1, expanded_paths, git_map, out);
            }
        }

        // Archivos después
        for (path, name) in files {
            let git_status = git_map.get(&path).copied();
            out.push(ExplorerItem {
                path,
                name,
                is_dir: false,
                depth,
                is_expanded: false,
                git_status,
            });
        }
    }

    /// Alterna el estado de expansión de un directorio
    pub fn toggle_expand(&mut self, index: usize) {
        if index >= self.items.len() || !self.items[index].is_dir {
            return;
        }

        if self.items[index].is_expanded {
            // Colapsar: remover todos los elementos hijos que tengan mayor profundidad
            self.items[index].is_expanded = false;
            let current_depth = self.items[index].depth;
            let mut remove_count = 0;
            for i in (index + 1)..self.items.len() {
                if self.items[i].depth > current_depth {
                    remove_count += 1;
                } else {
                    break;
                }
            }
            if remove_count > 0 {
                self.items.drain((index + 1)..(index + 1 + remove_count));
            }
        } else {
            // Expandir: leer el directorio e insertar sus hijos directamente después
            self.items[index].is_expanded = true;
            let current_depth = self.items[index].depth;
            let dir_path = self.items[index].path.clone();

            let expanded_set: HashSet<PathBuf> = self
                .items
                .iter()
                .filter(|it| it.is_dir && it.is_expanded)
                .map(|it| it.path.clone())
                .collect();
            let mut children = Vec::new();

            Self::read_directory(
                &dir_path,
                current_depth + 1,
                &expanded_set,
                &self.git_statuses,
                &mut children,
            );

            let insert_pos = index + 1;
            for (offset, item) in children.into_iter().enumerate() {
                self.items.insert(insert_pos + offset, item);
            }
        }
    }

    /// Sube al directorio padre
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.root_dir.parent() {
            let p_buf = parent.to_path_buf();
            if p_buf.is_dir() {
                self.set_root(p_buf);
            }
        }
    }

    /// Desplaza la lista verticalmente con la rueda del ratón
    pub fn scroll(&mut self, delta: isize, max_visible_lines: usize) {
        let max_offset = self.items.len().saturating_sub(max_visible_lines);
        if delta < 0 {
            self.scroll_offset = (self.scroll_offset + delta.unsigned_abs()).min(max_offset);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(delta as usize);
        }
    }

    /// Alterna la visibilidad del explorador de archivos lateral
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Obtiene el índice del elemento en la posición Y considerando el desplazamiento superior
    pub fn item_at_y(&self, my: usize, top_offset: usize) -> Option<usize> {
        let list_top = top_offset + Self::HEADER_HEIGHT + Self::SUBHEADER_HEIGHT;
        if my < list_top {
            return None;
        }
        let rel_y = my - list_top;
        let item_idx = self.scroll_offset + (rel_y / Self::ITEM_HEIGHT);
        if item_idx < self.items.len() {
            Some(item_idx)
        } else {
            None
        }
    }

    /// Mueve la selección hacia arriba o hacia abajo y ajusta el scroll si es necesario
    pub fn navigate_selection(&mut self, delta: isize, max_visible: usize) {
        if self.items.is_empty() {
            return;
        }
        let current = self.selected_index.unwrap_or(0);
        let next = if delta > 0 {
            (current + delta as usize).min(self.items.len() - 1)
        } else {
            current.saturating_sub(delta.unsigned_abs())
        };
        self.selected_index = Some(next);
        self.selected_path = Some(self.items[next].path.clone());
        self.ensure_visible(next, max_visible);
    }

    /// Garantiza que un elemento sea visible dentro de la ventana de scroll
    pub fn ensure_visible(&mut self, index: usize, max_visible: usize) {
        if max_visible == 0 {
            return;
        }
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + max_visible {
            self.scroll_offset = index + 1 - max_visible;
        }
    }

    /// Maneja la pulsación de Flecha Izquierda: colapsa carpeta, sube al directorio padre o a nivel raíz
    pub fn handle_arrow_left(&mut self, max_visible: usize) {
        if self.items.is_empty() {
            return;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = Some(self.items[0].path.clone());
            return;
        };
        if idx >= self.items.len() {
            return;
        }

        if self.items[idx].is_dir && self.items[idx].is_expanded {
            // Colapsar carpeta
            self.toggle_expand(idx);
        } else if self.items[idx].depth > 0 {
            // Seleccionar directorio padre de este elemento
            let target_depth = self.items[idx].depth - 1;
            for p_idx in (0..idx).rev() {
                if self.items[p_idx].is_dir && self.items[p_idx].depth == target_depth {
                    self.selected_index = Some(p_idx);
                    self.selected_path = Some(self.items[p_idx].path.clone());
                    self.ensure_visible(p_idx, max_visible);
                    break;
                }
            }
        } else {
            // En nivel raíz: subir de directorio (..)
            self.navigate_up();
            self.selected_index = if self.items.is_empty() {
                None
            } else {
                Some(0)
            };
            self.selected_path = self.selected_index.map(|i| self.items[i].path.clone());
        }
    }

    /// Maneja la pulsación de Flecha Derecha: expande carpeta, entra al primer hijo, o devuelve archivo para abrir
    pub fn handle_arrow_right(&mut self, max_visible: usize) -> Option<PathBuf> {
        if self.items.is_empty() {
            return None;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = Some(self.items[0].path.clone());
            return None;
        };
        if idx >= self.items.len() {
            return None;
        }

        if self.items[idx].is_dir {
            if !self.items[idx].is_expanded {
                self.toggle_expand(idx);
            } else if idx + 1 < self.items.len() && self.items[idx + 1].depth > self.items[idx].depth
            {
                // Ya expandido: entrar al primer hijo
                self.selected_index = Some(idx + 1);
                self.selected_path = Some(self.items[idx + 1].path.clone());
                self.ensure_visible(idx + 1, max_visible);
            }
            None
        } else {
            Some(self.items[idx].path.clone())
        }
    }

    /// Maneja la pulsación de Enter: alterna carpeta o devuelve archivo para abrir en visor
    pub fn handle_enter(&mut self, _max_visible: usize) -> Option<PathBuf> {
        if self.items.is_empty() {
            return None;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = Some(self.items[0].path.clone());
            return None;
        };
        if idx >= self.items.len() {
            return None;
        }

        if self.items[idx].is_dir {
            self.toggle_expand(idx);
            None
        } else {
            Some(self.items[idx].path.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_explorer_listing_and_navigation() {
        let current = std::env::current_dir().unwrap();
        let mut explorer = FileExplorer::new(Some(current.clone()), true);
        assert!(explorer.visible);
        assert!(!explorer.items.is_empty());

        // Debe listar directorios y archivos ordenados
        let has_src = explorer
            .items
            .iter()
            .any(|it| it.name == "src" && it.is_dir);
        let has_cargo = explorer
            .items
            .iter()
            .any(|it| it.name == "Cargo.toml" && !it.is_dir);
        assert!(has_src);
        assert!(has_cargo);

        // Subir de nivel
        let initial_root = explorer.root_dir.clone();
        explorer.navigate_up();
        assert_ne!(explorer.root_dir, initial_root);
        assert_eq!(explorer.root_dir, initial_root.parent().unwrap());

        // Regresar a initial
        explorer.set_root(initial_root);
        assert_eq!(explorer.root_dir, current);

        // Expandir carpeta src
        if let Some(pos) = explorer
            .items
            .iter()
            .position(|it| it.name == "src" && it.is_dir)
        {
            assert!(!explorer.items[pos].is_expanded);
            explorer.toggle_expand(pos);
            assert!(explorer.items[pos].is_expanded);
            // Debe haber insertado elementos hijos dentro de src
            let has_main = explorer
                .items
                .iter()
                .any(|it| it.name == "main.rs" && it.depth > 0);
            assert!(has_main);

            // Colapsar
            explorer.toggle_expand(pos);
            assert!(!explorer.items[pos].is_expanded);
            let has_main_after = explorer
                .items
                .iter()
                .any(|it| it.name == "main.rs" && it.depth > 0);
            assert!(!has_main_after);
        }

        // item_at_y
        let first_item_y = FileExplorer::HEADER_HEIGHT + FileExplorer::SUBHEADER_HEIGHT + 4;
        assert_eq!(explorer.item_at_y(first_item_y, 0), Some(0));

        // toggle visibility
        explorer.toggle();
        assert!(!explorer.visible);
        explorer.toggle();
        assert!(explorer.visible);
    }

    #[test]
    fn test_file_explorer_keyboard_navigation() {
        let current = std::env::current_dir().unwrap();
        let mut explorer = FileExplorer::new(Some(current), true);
        assert!(!explorer.items.is_empty());

        // Selección inicial
        explorer.selected_index = Some(0);
        explorer.navigate_selection(1, 10);
        assert_eq!(explorer.selected_index, Some(1));

        explorer.navigate_selection(-1, 10);
        assert_eq!(explorer.selected_index, Some(0));

        // ensure_visible
        explorer.ensure_visible(15, 10);
        assert_eq!(explorer.scroll_offset, 6);
        explorer.ensure_visible(2, 10);
        assert_eq!(explorer.scroll_offset, 2);

        // Expansión con flecha derecha y retorno con flecha izquierda
        if let Some(src_pos) = explorer
            .items
            .iter()
            .position(|it| it.name == "src" && it.is_dir)
        {
            explorer.selected_index = Some(src_pos);
            assert!(!explorer.items[src_pos].is_expanded);

            // Flecha derecha: expandir
            let res = explorer.handle_arrow_right(10);
            assert!(res.is_none());
            assert!(explorer.items[src_pos].is_expanded);

            // Flecha derecha de nuevo: entra al hijo
            let _ = explorer.handle_arrow_right(10);
            assert_eq!(explorer.selected_index, Some(src_pos + 1));

            // Flecha izquierda desde el hijo: selecciona el directorio padre (src)
            explorer.handle_arrow_left(10);
            assert_eq!(explorer.selected_index, Some(src_pos));

            // Flecha izquierda sobre src expandido: colapsa
            explorer.handle_arrow_left(10);
            assert!(!explorer.items[src_pos].is_expanded);
        }

        // Enter sobre un archivo devuelve la ruta para abrir en visor
        if let Some(cargo_pos) = explorer
            .items
            .iter()
            .position(|it| it.name == "Cargo.toml" && !it.is_dir)
        {
            explorer.selected_index = Some(cargo_pos);
            let path = explorer.handle_enter(10);
            assert!(path.is_some());
            assert!(path.unwrap().ends_with("Cargo.toml"));
        }
    }
}
