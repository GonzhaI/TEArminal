use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const FILE_EXPLORER_WIDTH: usize = 240;

#[derive(Debug, Clone)]
pub struct ExplorerItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub is_expanded: bool,
}

#[derive(Debug, Clone)]
pub struct FileExplorer {
    pub root_dir: PathBuf,
    pub items: Vec<ExplorerItem>,
    pub scroll_offset: usize,
    pub hovered_index: Option<usize>,
    pub hovered_close_btn: bool,
    pub hovered_up_btn: bool,
    pub hovered_refresh_btn: bool,
    pub selected_index: Option<usize>,
    pub selected_path: Option<PathBuf>,
    pub visible: bool,
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
            root_dir,
            items: Vec::new(),
            scroll_offset: 0,
            hovered_index: None,
            hovered_close_btn: false,
            hovered_up_btn: false,
            hovered_refresh_btn: false,
            selected_index: None,
            selected_path: None,
            visible,
        };

        explorer.refresh();
        explorer
    }

    /// Actualiza el directorio raíz y refresca la lista de elementos
    pub fn set_root(&mut self, new_root: PathBuf) {
        if new_root != self.root_dir && new_root.is_dir() {
            self.root_dir = new_root;
            self.scroll_offset = 0;
            self.selected_index = None;
            self.refresh();
        }
    }

    /// Refresca los contenidos del directorio raíz conservando las carpetas expandidas
    pub fn refresh(&mut self) {
        let expanded_paths: HashSet<PathBuf> = self
            .items
            .iter()
            .filter(|it| it.is_dir && it.is_expanded)
            .map(|it| it.path.clone())
            .collect();

        self.items.clear();
        Self::read_directory(&self.root_dir, 0, &expanded_paths, &mut self.items);
    }

    fn read_directory(
        dir: &Path,
        depth: usize,
        expanded_paths: &HashSet<PathBuf>,
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
            out.push(ExplorerItem {
                path: path.clone(),
                name,
                is_dir: true,
                depth,
                is_expanded,
            });

            if is_expanded {
                Self::read_directory(&path, depth + 1, expanded_paths, out);
            }
        }

        // Archivos después
        for (path, name) in files {
            out.push(ExplorerItem {
                path,
                name,
                is_dir: false,
                depth,
                is_expanded: false,
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

            let mut children = Vec::new();
            let expanded_set = HashSet::new();
            Self::read_directory(&dir_path, current_depth + 1, &expanded_set, &mut children);

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
}
