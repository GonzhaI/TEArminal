use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod edit;
mod git;
pub(crate) use edit::{EditKind, ExplorerEdit};
pub(crate) use git::*;

pub(crate) const FILE_EXPLORER_WIDTH: usize = 256;
pub(crate) const FILE_EXPLORER_MIN_WIDTH: usize = 168;

#[inline]
fn cmp_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// Puntuación de coincidencia difusa entre `query` y `text`.
/// Devuelve `None` si no coincide. Puntuaciones mayores indican mayor relevancia.
pub(crate) fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q = query.to_lowercase();
    let target = text.to_lowercase();

    if target == q {
        return Some(1000);
    }
    if target.starts_with(&q) {
        return Some(800 - (target.len() as i32).min(100));
    }
    if let Some(pos) = target.find(&q) {
        return Some(600 - (pos as i32 * 10).min(200));
    }
    // Coincidencia de subsecuencia difusa: cada carácter de `q` aparece en orden en `target`
    let mut target_chars = target.chars();
    let mut score = 400;
    let mut last_idx = 0;
    for (qi, qc) in q.chars().enumerate() {
        let mut found = false;
        let mut idx = last_idx;
        for tc in target_chars.by_ref() {
            idx += 1;
            if tc == qc {
                found = true;
                if qi > 0 {
                    score -= (idx - last_idx).min(15);
                }
                last_idx = idx;
                break;
            }
        }
        if !found {
            return None;
        }
    }
    Some(score.max(50))
}

fn score_item(query: &str, item: &ExplorerItem, root: &Path) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    // 1. Coincidencia en el nombre del archivo o carpeta
    if let Some(name_score) = fuzzy_score(query, &item.name) {
        return Some(name_score + 200);
    }
    // 2. Coincidencia en la ruta relativa respecto a la raíz
    if let Ok(rel) = item.path.strip_prefix(root) {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Some(path_score) = fuzzy_score(query, &rel_str) {
            return Some(path_score);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ExplorerItem {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) depth: usize,
    pub(crate) is_expanded: bool,
    pub(crate) git_status: Option<GitFileStatus>,
    pub(crate) is_staged: bool,
    pub(crate) is_unstaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RemoteSshSession {
    pub(crate) destination: String,
    pub(crate) remote_dir: String,
    pub(crate) identity_file: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) is_inband: bool,
}

impl From<(&str, &str)> for RemoteSshSession {
    fn from((destination, remote_dir): (&str, &str)) -> Self {
        Self {
            destination: destination.to_string(),
            remote_dir: remote_dir.to_string(),
            identity_file: None,
            port: None,
            is_inband: false,
        }
    }
}

impl From<(String, String)> for RemoteSshSession {
    fn from((destination, remote_dir): (String, String)) -> Self {
        Self {
            destination,
            remote_dir,
            identity_file: None,
            port: None,
            is_inband: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FileExplorer {
    pub root_dir: PathBuf,
    pub items: Vec<ExplorerItem>,
    pub scroll_offset: usize,
    pub hovered_index: Option<usize>,
    pub selected_index: Option<usize>,
    pub selected_path: Option<PathBuf>,
    pub visible: bool,
    /// Ancho del panel en píxeles; se ajusta arrastrando su borde izquierdo (`set_width`)
    pub width: usize,
    pub hovered_scrollbar: bool,
    /// Sesión SSH con integración: el listado llega por el propio terminal (sin segunda conexión)
    pub inband_remote: bool,
    pub cached_remote_names: Vec<String>,
    /// «Expandir todo» remoto en curso: cada listado recibido dispara la petición de la siguiente carpeta
    pub remote_expand_all: bool,
    pub is_focused: bool,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub git_statuses: FxHashMap<PathBuf, GitStatusInfo>,
    /// Botón de acción rápida de Git (`(row_idx, action)`) sobre el que se encuentra el cursor
    pub hovered_git_action: Option<(usize, crate::renderer::GitActionKind)>,
    /// Rectángulo `(x, y, ancho, alto)` cuando vive dentro del panel lateral; con `None` se acopla al borde derecho
    pub dock: Option<(usize, usize, usize, usize)>,
    /// Raíz del repositorio del directorio actual (None si no hay repo)
    pub git_root: Option<PathBuf>,
    /// Vista «Cambios»: lista plana solo con los archivos modificados según git
    pub changes_only: bool,
    /// Fija el explorador en el directorio actual (evita sincronizar con el cwd de la terminal)
    pub is_pinned: bool,
    pub remote_session: Option<RemoteSshSession>,
    pub error_message: Option<String>,
    /// Nombre en edición (nuevo archivo/carpeta o renombrar): ver `explorer/edit.rs`
    pub edit: Option<ExplorerEdit>,
    /// Entrada de texto para filtrado rápido en el árbol (`Ctrl + P` / `Ctrl + F`)
    pub filter: Option<crate::ui::TextInput>,
    /// Índices de `items` que coinciden con el filtro activo (en orden de relevancia o jerárquico)
    pub filtered_indices: Option<Vec<usize>>,
    /// Botón de cerrar/limpiar el filtro bajo el ratón
    pub hovered_filter_close: bool,
    /// Carpetas que estaban expandidas antes de activar el filtro (para restaurar al salir con Esc)
    filter_saved_expansion: Option<FxHashSet<PathBuf>>,
    /// Última sesión remota pedida con `set_remote_session` (la raíz canónica de `remote_session` puede diferir)
    remote_requested: Option<RemoteSshSession>,
    /// Generación de la vista remota por SSH lento: las respuestas de una anterior se ignoran
    remote_gen: u64,
    /// Listados pedidos y aún sin lanzar (la app los ejecuta en hilos: `TearminalApp::pump_explorer_ssh`)
    ssh_requests: Vec<RemoteLsReq>,
    /// Consulta de `git status` pendiente de lanzar en segundo plano (`(repo_root, changes_only)`)
    git_status_req: Option<(PathBuf, bool)>,
    /// Hay un `git status` en vuelo: no se lanza otro hasta que llegue su resultado (una ráfaga de cambios
    /// deja solo una petición pendiente y las respuestas nunca se cruzan)
    git_status_running: bool,
}

/// Listado remoto por SSH lento: la raíz (`expand = None`) o el contenido de una carpeta del árbol
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RemoteLsReq {
    pub(crate) gen: u64,
    pub(crate) dest: String,
    pub(crate) dir: String,
    pub(crate) identity_file: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) expand: Option<PathBuf>,
}

impl FileExplorer {
    pub(crate) const HEADER_HEIGHT: usize = 38;
    /// Aire entre la cabecera de sección y la lista (sin barra de herramientas).
    pub(crate) const SUBHEADER_HEIGHT: usize = 0;
    /// Alto de la barra de filtro rápido de archivos (`Ctrl + P` / `Ctrl + F`)
    pub(crate) const FILTER_BAR_HEIGHT: usize = 32;
    pub(crate) const ITEM_HEIGHT: usize = 26;
    /// Aire entre el sub-encabezado y la primera fila de la lista
    pub(crate) const LIST_TOP_PAD: usize = 4;

    /// Alto dinámico del subencabezado según si el filtro está abierto o no
    #[inline]
    pub(crate) fn subheader_height(&self) -> usize {
        if self.filter.is_some() {
            Self::FILTER_BAR_HEIGHT
        } else {
            0
        }
    }

    /// Rectángulo del cajón de búsqueda del filtro rápido
    #[inline]
    pub(crate) fn filter_bar_rect(
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
    ) -> (usize, usize, usize, usize) {
        let bx = panel_x + 8;
        let by = panel_y + Self::HEADER_HEIGHT + 2;
        let bw = panel_w.saturating_sub(16);
        let bh = Self::FILTER_BAR_HEIGHT.saturating_sub(4);
        (bx, by, bw, bh)
    }

    /// Rectángulo del botón de cerrar/limpiar (X) del filtro
    #[inline]
    pub(crate) fn filter_close_btn_rect(
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
    ) -> (usize, usize, usize, usize) {
        let (bx, by, bw, bh) = Self::filter_bar_rect(panel_x, panel_y, panel_w);
        let btn_w = 20;
        let btn_h = bh;
        let btn_x = bx + bw.saturating_sub(btn_w + 2);
        (btn_x, by, btn_w, btn_h)
    }

    /// Indica si el filtro rápido del árbol está activo
    #[inline]
    pub(crate) fn is_filtering(&self) -> bool {
        self.filter.is_some()
    }

    /// Total de elementos visibles teniendo en cuenta el filtro
    #[inline]
    pub(crate) fn visible_item_count(&self) -> usize {
        if let Some(indices) = &self.filtered_indices {
            indices.len()
        } else {
            self.items.len()
        }
    }

    /// Referencia al elemento en la posición visible `visible_idx`
    #[inline]
    pub(crate) fn visible_item(&self, visible_idx: usize) -> Option<&ExplorerItem> {
        if let Some(indices) = &self.filtered_indices {
            indices.get(visible_idx).and_then(|&i| self.items.get(i))
        } else {
            self.items.get(visible_idx)
        }
    }

    /// Índice real en `self.items` correspondiente a la fila visible `visible_idx`
    #[inline]
    pub(crate) fn actual_index_for_visible(&self, visible_idx: usize) -> Option<usize> {
        if let Some(indices) = &self.filtered_indices {
            indices.get(visible_idx).copied()
        } else {
            Some(visible_idx)
        }
    }

    /// Calcula la geometría del panel acoplado del explorador de archivos.
    #[inline]
    pub(crate) fn panel_geometry(
        &self,
        win_w: usize,
        win_h: usize,
        top_offset: usize,
        status_bar_h: usize,
    ) -> (usize, usize, usize, usize) {
        if let Some(dock) = self.dock {
            return dock;
        }
        let panel_x = win_w.saturating_sub(self.width);
        let panel_y = top_offset;
        let panel_h = win_h.saturating_sub(top_offset + status_bar_h);
        (panel_x, panel_y, self.width, panel_h)
    }

    /// Acota `width` entre el mínimo y la mitad de la ventana (siempre queda sitio para el terminal)
    pub(crate) fn clamp_width(width: usize, win_w: usize) -> usize {
        width.clamp(
            FILE_EXPLORER_MIN_WIDTH,
            (win_w / 2).max(FILE_EXPLORER_MIN_WIDTH),
        )
    }

    /// Ancho que deja el borde izquierdo del panel bajo el ratón en `mx`
    #[cfg(test)]
    pub(crate) fn set_width_from_edge(&mut self, mx: usize, win_w: usize) {
        self.width = Self::clamp_width(win_w.saturating_sub(mx), win_w);
    }

    /// ¿Está `(mx, my)` sobre el borde izquierdo del panel (franja de agarre para redimensionar)?
    pub(crate) fn on_resize_edge(
        &self,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
        top_offset: usize,
        status_bar_h: usize,
    ) -> bool {
        let (panel_x, panel_y, _, panel_h) =
            self.panel_geometry(win_w, win_h, top_offset, status_bar_h);
        self.visible
            && self.dock.is_none()
            && mx + 6 >= panel_x
            && mx <= panel_x + 5
            && my >= panel_y
            && my < panel_y + panel_h
    }

    pub(crate) fn new(initial_dir: Option<PathBuf>, visible: bool) -> Self {
        let root_dir = initial_dir
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut explorer = Self {
            root_dir: root_dir.clone(),
            items: Vec::new(),
            scroll_offset: 0,
            hovered_index: None,
            selected_index: None,
            selected_path: None,
            visible,
            width: FILE_EXPLORER_WIDTH,
            hovered_scrollbar: false,
            inband_remote: false,
            cached_remote_names: Vec::new(),
            remote_expand_all: false,
            remote_requested: None,
            remote_gen: 0,
            ssh_requests: Vec::new(),
            git_status_req: None,
            git_status_running: false,
            is_focused: false,
            history: vec![root_dir],
            history_index: 0,
            git_statuses: FxHashMap::default(),
            hovered_git_action: None,
            dock: None,
            git_root: None,
            changes_only: false,
            is_pinned: false,
            remote_session: None,
            error_message: None,
            edit: None,
            filter: None,
            filtered_indices: None,
            hovered_filter_close: false,
            filter_saved_expansion: None,
        };

        if visible {
            explorer.refresh();
        }
        explorer
    }

    /// Alterna el estado de fijado del explorador
    #[cfg(test)]
    pub(crate) fn toggle_pinned(&mut self) {
        self.is_pinned = !self.is_pinned;
    }

    pub(crate) fn set_expanded(&mut self, path: &Path, expanded: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.path == path && i.is_dir) {
            item.is_expanded = expanded;
        }
    }

    /// Siguiente carpeta remota sin abrir para «Expandir todo» (3 niveles, 1 000 entradas como máximo)
    pub(crate) fn next_remote_dir_to_expand(&self) -> Option<usize> {
        const MAX_ITEMS: usize = 1000;
        const MAX_DEPTH: usize = 3;
        const SKIP: [&str; 4] = [".git", "node_modules", "target", "__pycache__"];
        if self.items.len() >= MAX_ITEMS {
            return None;
        }
        self.items.iter().position(|i| {
            i.is_dir
                && !i.is_expanded
                && i.depth < MAX_DEPTH
                && !SKIP.contains(&i.name.trim_end_matches('/'))
        })
    }

    /// Elementos de un listado remoto ya depurado (`clean_listing`): carpetas primero, sin `.git`, por nombre
    fn remote_items(base: &str, depth: usize, names: &[String]) -> Vec<ExplorerItem> {
        let (mut dirs, mut files): (Vec<_>, Vec<_>) = names
            .iter()
            .map(|n| match n.strip_suffix('/') {
                Some(d) => (d, true),
                None => (n.as_str(), false),
            })
            .filter(|(name, _)| *name != ".git")
            .map(|(name, is_dir)| ExplorerItem {
                path: PathBuf::from(format!("{}/{}", base.trim_end_matches('/'), name)),
                name: name.to_string(),
                is_dir,
                depth,
                is_expanded: false,
                git_status: None,
                is_staged: false,
                is_unstaged: false,
            })
            .partition(|i| i.is_dir);
        dirs.sort_by(|a, b| cmp_ignore_case(&a.name, &b.name));
        files.sort_by(|a, b| cmp_ignore_case(&a.name, &b.name));
        dirs.extend(files);
        dirs
    }

    /// Inserta bajo la carpeta remota `dir` los nombres de su listado (recibido por el terminal)
    pub(crate) fn expand_remote_dir(&mut self, dir: &Path, names: &[String]) {
        let Some(idx) = self.items.iter().position(|i| i.path == dir && i.is_dir) else {
            return;
        };
        let depth = self.items[idx].depth;
        let count = self.items[idx + 1..]
            .iter()
            .take_while(|item| item.depth > depth)
            .count();
        self.items.drain(idx + 1..idx + 1 + count);
        self.items[idx].is_expanded = true;
        let base = dir.to_string_lossy().trim_end_matches('/').to_string();
        let children = Self::remote_items(&base, depth + 1, names);
        self.items.splice(idx + 1..idx + 1, children);
        self.apply_statuses_to_items();
        if self.is_filtering() {
            self.update_filter();
        }
    }

    /// Cierra todas las carpetas abiertas y deja solo el primer nivel
    pub(crate) fn collapse_all(&mut self) {
        self.remote_expand_all = false;
        self.items.retain(|i| i.depth == 0);
        for item in &mut self.items {
            item.is_expanded = false;
        }
        self.scroll_offset = 0;
        self.selected_index = None;
        if self.is_filtering() {
            self.update_filter();
        }
    }

    /// Abre todas las carpetas locales (hasta 4 niveles y 2 000 entradas; sin `.git`, `target`, etc.)
    pub(crate) fn expand_all(&mut self) {
        const MAX_ITEMS: usize = 2000;
        const MAX_DEPTH: usize = 4;
        const SKIP: [&str; 5] = [".git", "target", "node_modules", ".cargo", "dist"];
        if self.remote_session.is_some() {
            self.remote_expand_all = true;
            if let Some(idx) = self.next_remote_dir_to_expand() {
                self.toggle_expand(idx);
            }
            return;
        }
        while self.items.len() < MAX_ITEMS {
            let next = self.items.iter().position(|i| {
                i.is_dir
                    && !i.is_expanded
                    && i.depth < MAX_DEPTH
                    && !SKIP.contains(&i.name.trim_end_matches('/'))
            });
            let Some(idx) = next else {
                break;
            };
            self.toggle_expand(idx);
        }
    }

    /// Abre el filtro rápido del árbol expandiendo carpetas para búsqueda profunda
    pub(crate) fn open_filter(&mut self) {
        if self.filter.is_some() {
            if let Some(ref mut input) = self.filter {
                input.select_all();
            }
            return;
        }
        let expanded: FxHashSet<PathBuf> = self
            .items
            .iter()
            .filter(|it| it.is_dir && it.is_expanded)
            .map(|it| it.path.clone())
            .collect();
        self.filter_saved_expansion = Some(expanded);
        if self.remote_session.is_none() && !self.changes_only {
            self.expand_all();
        }
        self.filter = Some(crate::ui::TextInput::new(""));
        self.update_filter();
    }

    /// Cierra el filtro rápido y restaura el estado previo del árbol
    pub(crate) fn close_filter(&mut self) {
        self.filter = None;
        self.filtered_indices = None;
        self.hovered_filter_close = false;
        if let Some(saved) = self.filter_saved_expansion.take() {
            if self.remote_session.is_none() && !self.changes_only {
                self.populate_directory_with_expanded(&saved);
            }
        }
        self.scroll_offset = 0;
        self.selected_index = if self.items.is_empty() { None } else { Some(0) };
        self.selected_path = self.items.first().map(|i| i.path.clone());
    }

    /// Actualiza los índices filtrados en memoria según el texto de búsqueda
    pub(crate) fn update_filter(&mut self) {
        let Some(input) = &self.filter else {
            self.filtered_indices = None;
            return;
        };
        let query = input.text.trim();
        if query.is_empty() {
            self.filtered_indices = Some((0..self.items.len()).collect());
            self.scroll_offset = 0;
            self.selected_index = if self.items.is_empty() { None } else { Some(0) };
            self.selected_path = self.items.first().map(|i| i.path.clone());
            return;
        }

        let mut keep = vec![false; self.items.len()];
        let mut best_score = i32::MIN;
        let mut best_file_idx = None;

        for (idx, item) in self.items.iter().enumerate() {
            if let Some(score) = score_item(query, item, &self.root_dir) {
                keep[idx] = true;
                // Un archivo gana a cualquier carpeta; una carpeta solo si aún no hay archivo
                let better = if item.is_dir {
                    best_file_idx.is_none()
                } else {
                    true
                };
                if better && score > best_score {
                    best_score = score;
                    best_file_idx = Some(idx);
                }
            }
        }

        // Mantener visibles las carpetas ancestras de cualquier elemento coincidente
        for idx in 0..self.items.len() {
            if keep[idx] {
                let mut cur_depth = self.items[idx].depth;
                if cur_depth > 0 {
                    for p in (0..idx).rev() {
                        if self.items[p].is_dir && self.items[p].depth < cur_depth {
                            keep[p] = true;
                            self.items[p].is_expanded = true;
                            cur_depth = self.items[p].depth;
                            if cur_depth == 0 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let indices: Vec<usize> = (0..self.items.len()).filter(|&i| keep[i]).collect();

        let selected_pos = if let Some(best_real) = best_file_idx {
            indices.iter().position(|&i| i == best_real).unwrap_or(0)
        } else {
            0
        };

        let count = indices.len();
        self.filtered_indices = Some(indices);
        self.scroll_offset = 0;
        if count > 0 {
            self.selected_index = Some(selected_pos.min(count - 1));
            self.selected_path = self
                .visible_item(selected_pos.min(count - 1))
                .map(|i| i.path.clone());
        } else {
            self.selected_index = None;
            self.selected_path = None;
        }
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    /// Carpeta padre de la carpeta remota actual
    pub(crate) fn remote_parent(&self) -> Option<String> {
        let cur = self
            .remote_session
            .as_ref()?
            .remote_dir
            .trim_end_matches('/');
        let pos = cur.rfind('/')?;
        Some(if pos == 0 {
            "/".to_string()
        } else {
            cur[..pos].to_string()
        })
    }

    /// Muestra el listado de la carpeta remota recibido por el terminal (OSC 1337 TearminalLs).
    /// No hace nada si ya está aplicado, así que puede llamarse tras cada ráfaga de salida.
    pub(crate) fn set_remote_listing(&mut self, session: RemoteSshSession, names: &[String]) {
        let same = self.inband_remote && self.remote_session.as_ref() == Some(&session);
        self.cached_remote_names = names.to_vec();
        if same && !self.changes_only && self.listing_names_match(names) {
            return;
        }
        if !same {
            self.remote_expand_all = false;
            self.scroll_offset = 0;
            self.selected_index = None;
            self.selected_path = None;
        }
        self.inband_remote = true;
        self.error_message = None;
        self.root_dir = PathBuf::from(&session.remote_dir);
        let base = session.remote_dir.trim_end_matches('/').to_string();
        self.remote_session = Some(session);
        if self.changes_only {
            self.items = self.change_items();
        } else {
            self.items = Self::remote_items(&base, 0, names);
            self.apply_statuses_to_items();
        }
    }

    fn listing_names_match(&self, names: &[String]) -> bool {
        // Solo cuenta la raíz: las carpetas expandidas (profundidad > 0) no deben invalidar el listado
        let roots: Vec<&ExplorerItem> = self.items.iter().filter(|i| i.depth == 0).collect();
        let shown = names.iter().filter(|n| n.as_str() != ".git/").count();
        shown == roots.len()
            && names
                .iter()
                .filter(|n| n.as_str() != ".git/")
                .all(|n| roots.iter().any(|i| i.name == n.trim_end_matches('/')))
    }

    pub(crate) fn go_back(&mut self) {
        if self.can_go_back() {
            self.step_history(self.history_index - 1);
        }
    }

    pub(crate) fn go_forward(&mut self) {
        if self.can_go_forward() {
            self.step_history(self.history_index + 1);
        }
    }

    fn step_history(&mut self, next_index: usize) {
        self.history_index = next_index;
        let target = self.history[self.history_index].clone();
        if let Some(ref mut sess) = self.remote_session {
            sess.remote_dir = target.to_string_lossy().replace('\\', "/");
            self.root_dir = target;
            self.scroll_offset = 0;
            self.selected_index = None;
            self.refresh();
        } else if target.is_dir() {
            self.root_dir = target;
            self.scroll_offset = 0;
            self.selected_index = None;
            self.refresh();
        }
    }

    /// Actualiza el directorio raíz y refresca la lista de elementos
    pub(crate) fn set_root(&mut self, new_root: PathBuf) {
        if let Some(ref mut sess) = self.remote_session {
            let r_str = new_root.to_string_lossy().replace('\\', "/");
            if r_str != sess.remote_dir {
                if self.history_index + 1 < self.history.len() {
                    self.history.truncate(self.history_index + 1);
                }
                self.history.push(new_root.clone());
                self.history_index = self.history.len() - 1;

                sess.remote_dir = r_str;
                self.root_dir = new_root;
                self.scroll_offset = 0;
                self.selected_index = None;
                self.refresh();
            }
            return;
        }

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

    /// Configura o desactiva la sesión SSH remota para el explorador. El listado llega después, por la vía
    /// lenta (`take_ssh_requests` + `apply_remote_listing`).
    pub(crate) fn set_remote_session(&mut self, session: Option<RemoteSshSession>) {
        if self.remote_session == session {
            return;
        }
        // La raíz canónica que devuelve el servidor cambia `remote_session`: sin esta comparación cada ráfaga
        // de salida volvería a pedir el mismo listado
        if session.is_some() && self.remote_requested == session {
            return;
        }
        self.remote_requested = session.clone();
        self.remote_session = session;
        self.scroll_offset = 0;
        self.selected_index = None;
        self.selected_path = None;
        if let Some(ref rem) = self.remote_session {
            self.root_dir = PathBuf::from(&rem.remote_dir);
            self.history = vec![self.root_dir.clone()];
            self.history_index = 0;
            self.items.clear();
            self.cached_remote_names.clear();
            if rem.is_inband {
                self.inband_remote = true;
            } else {
                self.refresh_remote();
            }
        } else {
            self.inband_remote = false;
            self.cached_remote_names.clear();
            self.history = vec![self.root_dir.clone()];
            self.history_index = 0;
            self.refresh();
        }
    }

    /// Pide el listado de la raíz remota por SSH (sin bloquear: lo lanza la app en un hilo)
    pub(crate) fn refresh_remote(&mut self) {
        if self.inband_remote {
            return;
        }
        let Some(sess) = &self.remote_session else {
            return;
        };
        let req = RemoteLsReq {
            gen: self.remote_gen + 1,
            dest: sess.destination.clone(),
            dir: sess.remote_dir.clone(),
            identity_file: sess.identity_file.clone(),
            port: sess.port,
            expand: None,
        };
        self.remote_gen = req.gen;
        self.ssh_requests = vec![req]; // las peticiones de generaciones anteriores ya no valen
        self.error_message = None;
    }

    /// Listados pedidos hasta ahora (la app los ejecuta en hilos)
    pub(crate) fn take_ssh_requests(&mut self) -> Vec<RemoteLsReq> {
        std::mem::take(&mut self.ssh_requests)
    }

    /// Aplica el resultado de un listado remoto; devuelve la rama git si era el de la raíz.
    /// Se ignora si la vista cambió mientras tanto (otra sesión, otra generación o listado en banda).
    pub(crate) fn apply_remote_listing(
        &mut self,
        req: &RemoteLsReq,
        result: Result<crate::ssh::Output, String>,
    ) -> Option<String> {
        let current = self.remote_session.as_ref().map(|s| s.destination.as_str());
        if req.gen != self.remote_gen || current != Some(req.dest.as_str()) {
            return None;
        }
        match &req.expand {
            None => self.apply_remote_root(result),
            Some(dir) => {
                match result {
                    Ok(out) if out.ok => {
                        let names =
                            crate::terminal::clean_listing(&String::from_utf8_lossy(&out.stdout));
                        self.expand_remote_dir(dir, &names);
                    }
                    _ => self.set_expanded(dir, false),
                }
                None
            }
        }
    }

    fn apply_remote_root(&mut self, result: Result<crate::ssh::Output, String>) -> Option<String> {
        if self.inband_remote {
            return None;
        }
        let output = match result {
            Ok(out) => out,
            Err(e) => {
                self.items.clear();
                self.error_message = Some(e);
                return None;
            }
        };

        if !output.ok {
            self.items.clear();
            if output.stderr.contains("Permission denied") {
                self.error_message = Some(
                    "Requiere clave SSH en servidor remoto (o ejecuta 'ls' en consola)".to_string(),
                );
            } else {
                let trimmed = output.stderr.trim();
                self.error_message = Some(if trimmed.is_empty() {
                    "No se pudo listar directorio remoto".to_string()
                } else {
                    trimmed.to_string()
                });
            }
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();

        // La salida la controla el servidor: carpeta absoluta sin caracteres de control y rama de ≤ 200 bytes
        // (al reconectar, `safe_remote_dir` vuelve a filtrar la carpeta antes de interpolarla)
        let canonical_pwd =
            match lines.next().map(str::trim).filter(|d| {
                d.starts_with('/') && d.len() <= 1024 && !d.chars().any(char::is_control)
            }) {
                Some(dir) => dir.to_string(),
                None => {
                    self.items.clear();
                    return None;
                }
            };
        if let Some(ref mut s) = self.remote_session {
            s.remote_dir = canonical_pwd.clone();
        }
        self.root_dir = PathBuf::from(&canonical_pwd);

        // Entre la carpeta y `---` va la rama git (si hay repositorio)
        let mut git_branch = None;
        for l in lines.by_ref() {
            let l = l.trim();
            if l == "---" {
                break;
            }
            if !l.is_empty() && l.len() <= 200 && !l.chars().any(char::is_control) {
                git_branch = Some(l.to_string());
            }
        }

        let rest: Vec<&str> = lines.collect();
        let names = crate::terminal::clean_listing(&rest.join("\n"));
        self.items = Self::remote_items(&canonical_pwd, 0, &names);

        git_branch
    }

    /// Petición de `git status` pendiente de ejecutar en un hilo
    pub(crate) fn take_git_status_request(&mut self) -> Option<(PathBuf, bool)> {
        if self.git_status_running {
            return None;
        }
        let req = self.git_status_req.take();
        self.git_status_running = req.is_some();
        req
    }

    /// El hilo de `git status` no pudo lanzarse: libera la guarda para el siguiente intento
    pub(crate) fn cancel_git_status(&mut self) {
        self.git_status_running = false;
    }

    /// Aplica el mapa de estados de `git status` a los elementos actuales del árbol
    pub(crate) fn apply_statuses_to_items(&mut self) {
        for item in &mut self.items {
            if item.is_dir {
                item.git_status = dir_git_status(&self.git_statuses, &item.path);
            } else if let Some(info) = self.git_statuses.get(&item.path) {
                item.git_status = Some(info.status);
                item.is_staged = info.is_staged;
                item.is_unstaged = info.is_unstaged;
            } else {
                item.git_status = None;
                item.is_staged = false;
                item.is_unstaged = false;
            }
        }
    }

    /// Aplica el mapa de estados de `git status` obtenido en segundo plano (sesión local)
    pub(crate) fn apply_git_statuses(
        &mut self,
        repo_root: &Path,
        statuses: FxHashMap<PathBuf, GitStatusInfo>,
    ) {
        self.git_status_running = false;
        if self.remote_session.is_some() || self.git_root.as_deref() != Some(repo_root) {
            return;
        }
        self.git_statuses = statuses;
        if self.changes_only {
            self.items = self.change_items();
            self.scroll_offset = self.scroll_offset.min(self.items.len().saturating_sub(1));
        } else {
            self.apply_statuses_to_items();
        }
    }

    /// Aplica el mapa de estados Git recibido del servidor remoto vía integración en banda
    pub(crate) fn apply_remote_git_statuses(
        &mut self,
        repo_root: &Path,
        statuses: FxHashMap<PathBuf, GitStatusInfo>,
    ) {
        if self.remote_session.is_none() {
            return;
        }
        if self.git_root.as_deref() == Some(repo_root) && self.git_statuses == statuses {
            return;
        }
        self.git_root = Some(repo_root.to_path_buf());
        self.git_statuses = statuses;
        if self.changes_only {
            self.items = self.change_items();
            self.scroll_offset = self.scroll_offset.min(self.items.len().saturating_sub(1));
        } else {
            self.apply_statuses_to_items();
        }
    }

    /// Consulta el estado Git del repositorio activo (en un hilo en la GUI; síncrono en tests)
    pub(crate) fn refresh_git_status(&mut self) {
        if self.remote_session.is_some() {
            return;
        }
        let new_root = git_repo_root(&self.root_dir);
        if new_root != self.git_root {
            self.git_statuses.clear();
            self.git_root = new_root;
        }
        let Some(repo_root) = self.git_root.clone() else {
            self.git_status_req = None;
            return;
        };

        if cfg!(test) {
            let statuses = collect_git_statuses(&repo_root, self.changes_only);
            self.apply_git_statuses(&repo_root, statuses);
        } else {
            self.git_status_req = Some((repo_root, self.changes_only));
        }
    }

    /// Archivos con cambios de git (sin los borrados): el contador de la pestaña «Cambios».
    pub(crate) fn change_count(&self) -> usize {
        if self.changes_only {
            return self.items.len();
        }
        self.git_statuses
            .values()
            .filter(|s| s.status != GitFileStatus::Deleted)
            .count()
    }

    /// Alterna entre el árbol de archivos y la lista de archivos con cambios
    pub(crate) fn toggle_changes_only(&mut self) {
        self.changes_only = !self.changes_only;
        self.scroll_offset = 0;
        self.selected_index = None;
        if self.changes_only {
            self.items = self.change_items();
        } else if self.inband_remote {
            if let Some(sess) = &self.remote_session {
                let base = sess.remote_dir.trim_end_matches('/').to_string();
                self.items = Self::remote_items(&base, 0, &self.cached_remote_names);
                self.apply_statuses_to_items();
            }
        } else {
            self.refresh();
        }
    }

    /// Archivos con cambios (sin los borrados, que ya no se pueden abrir), ordenados por ruta relativa;
    /// la carpeta solo se muestra cuando el nombre de archivo se repite.
    fn change_items(&self) -> Vec<ExplorerItem> {
        let is_remote = self.remote_session.is_some();
        let mut items: Vec<ExplorerItem> = self
            .git_statuses
            .iter()
            .filter(|(path, info)| {
                info.status != GitFileStatus::Deleted && (is_remote || path.is_file())
            })
            .map(|(path, info)| ExplorerItem {
                path: path.clone(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                is_dir: false,
                depth: 0,
                is_expanded: false,
                git_status: Some(info.status),
                is_staged: info.is_staged,
                is_unstaged: info.is_unstaged,
            })
            .collect();
        items.sort_by(|a, b| a.path.cmp(&b.path));
        let mut counts: FxHashMap<String, usize> = FxHashMap::default();
        for item in &items {
            *counts.entry(item.name.clone()).or_insert(0) += 1;
        }
        if let Some(root) = self.git_root.as_deref() {
            for item in &mut items {
                if counts.get(&item.name).copied().unwrap_or(0) > 1 {
                    if let Some(dir) = item
                        .path
                        .parent()
                        .and_then(|p| p.strip_prefix(root).ok())
                        .map(|d| d.to_string_lossy().replace('\\', "/"))
                        .filter(|d| !d.is_empty())
                    {
                        item.name = format!("{}  {dir}", item.name);
                    }
                }
            }
        }
        items
    }

    /// Refresca los contenidos del directorio raíz conservando las carpetas expandidas
    pub(crate) fn refresh(&mut self) {
        if self.remote_session.is_some() {
            self.refresh_remote();
            return;
        }

        self.refresh_git_status();

        if self.changes_only {
            self.items = self.change_items();
            self.scroll_offset = self.scroll_offset.min(self.items.len().saturating_sub(1));
            return;
        }

        let expanded_paths: FxHashSet<PathBuf> = self
            .items
            .iter()
            .filter(|it| it.is_dir && it.is_expanded)
            .map(|it| it.path.clone())
            .collect();

        self.populate_directory_with_expanded(&expanded_paths);
        if self.is_filtering() {
            self.update_filter();
        }
    }

    /// Rellena `self.items` leyendo el directorio con las carpetas de `expanded` abiertas
    pub(crate) fn populate_directory_with_expanded(&mut self, expanded: &FxHashSet<PathBuf>) {
        if self.remote_session.is_some() {
            return;
        }
        self.items.clear();
        Self::read_directory(
            &self.root_dir,
            0,
            expanded,
            &self.git_statuses,
            &mut self.items,
        );
    }

    fn read_directory(
        dir: &Path,
        depth: usize,
        expanded_paths: &FxHashSet<PathBuf>,
        git_map: &FxHashMap<PathBuf, GitStatusInfo>,
        out: &mut Vec<ExplorerItem>,
    ) {
        let Ok(read_dir) = fs::read_dir(dir) else {
            return;
        };

        let mut dirs = Vec::with_capacity(32);
        let mut files = Vec::with_capacity(64);

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
        dirs.sort_by(|a, b| cmp_ignore_case(&a.1, &b.1));
        files.sort_by(|a, b| cmp_ignore_case(&a.1, &b.1));

        // Directorios primero
        for (path, name) in dirs {
            let is_expanded = expanded_paths.contains(&path);

            let git_status = dir_git_status(git_map, &path);

            out.push(ExplorerItem {
                path: path.clone(),
                name,
                is_dir: true,
                depth,
                is_expanded,
                git_status,
                is_staged: false,
                is_unstaged: false,
            });

            if is_expanded {
                Self::read_directory(&path, depth + 1, expanded_paths, git_map, out);
            }
        }

        // Archivos después
        for (path, name) in files {
            let (git_status, is_staged, is_unstaged) = if let Some(info) = git_map.get(&path) {
                (Some(info.status), info.is_staged, info.is_unstaged)
            } else {
                (None, false, false)
            };
            out.push(ExplorerItem {
                path,
                name,
                is_dir: false,
                depth,
                is_expanded: false,
                git_status,
                is_staged,
                is_unstaged,
            });
        }
    }

    /// Alterna el estado de expansión de un directorio
    pub(crate) fn toggle_expand(&mut self, index: usize) {
        if index >= self.items.len() || !self.items[index].is_dir {
            return;
        }

        if self.items[index].is_expanded {
            // Colapsar: remover todos los elementos hijos que tengan mayor profundidad
            self.items[index].is_expanded = false;
            let current_depth = self.items[index].depth;
            let remove_count = self.items[index + 1..]
                .iter()
                .take_while(|item| item.depth > current_depth)
                .count();
            if remove_count > 0 {
                self.items.drain((index + 1)..(index + 1 + remove_count));
            }
        } else {
            // Expandir: leer el directorio e insertar sus hijos directamente después
            self.items[index].is_expanded = true;
            let current_depth = self.items[index].depth;
            let dir_path = self.items[index].path.clone();

            if let Some(ref sess) = self.remote_session {
                // El listado llega por SSH sin bloquear (`apply_remote_listing` inserta los hijos)
                self.ssh_requests.push(RemoteLsReq {
                    gen: self.remote_gen,
                    dest: sess.destination.clone(),
                    dir: dir_path.to_string_lossy().replace('\\', "/"),
                    identity_file: sess.identity_file.clone(),
                    port: sess.port,
                    expand: Some(dir_path),
                });
            } else {
                let expanded_set: FxHashSet<PathBuf> = self
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
    }

    /// Sube al directorio padre
    pub(crate) fn navigate_up(&mut self) {
        if let Some(ref mut sess) = self.remote_session {
            let cur = sess.remote_dir.trim_end_matches('/');
            if let Some(pos) = cur.rfind('/') {
                let parent = if pos == 0 { "/" } else { &cur[..pos] };
                if parent != sess.remote_dir {
                    let parent_str = parent.to_string();
                    sess.remote_dir = parent_str.clone();
                    self.root_dir = PathBuf::from(parent_str);
                    if self.history_index + 1 < self.history.len() {
                        self.history.truncate(self.history_index + 1);
                    }
                    self.history.push(self.root_dir.clone());
                    self.history_index = self.history.len() - 1;
                    self.scroll_offset = 0;
                    self.selected_index = None;
                    self.refresh();
                }
            }
            return;
        }

        if let Some(parent) = self.root_dir.parent() {
            let p_buf = parent.to_path_buf();
            if p_buf.is_dir() {
                self.set_root(p_buf);
            }
        }
    }

    /// Desplaza la lista verticalmente con la rueda del ratón
    pub(crate) fn scroll(&mut self, delta: isize, max_visible_lines: usize) {
        let max_offset = self.visible_item_count().saturating_sub(max_visible_lines);
        if delta < 0 {
            self.scroll_offset = (self.scroll_offset + delta.unsigned_abs()).min(max_offset);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(delta as usize);
        }
    }

    /// Alterna la visibilidad del explorador de archivos lateral
    pub(crate) fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.items.is_empty() && self.remote_session.is_none() {
            self.refresh();
        }
    }

    /// Obtiene la posición Y de la fila `idx` considerando el desplazamiento superior
    pub(crate) fn item_y(&self, idx: usize, top_offset: usize) -> Option<usize> {
        if idx < self.scroll_offset || idx >= self.visible_item_count() {
            return None;
        }
        let list_top =
            top_offset + Self::HEADER_HEIGHT + self.subheader_height() + Self::LIST_TOP_PAD;
        Some(list_top + (idx - self.scroll_offset) * Self::ITEM_HEIGHT)
    }

    /// Obtiene el índice del elemento en la posición Y considerando el desplazamiento superior
    pub(crate) fn item_at_y(&self, my: usize, top_offset: usize) -> Option<usize> {
        let list_top =
            top_offset + Self::HEADER_HEIGHT + self.subheader_height() + Self::LIST_TOP_PAD;
        if my < list_top {
            return None;
        }
        let rel_y = my - list_top;
        let item_idx = self.scroll_offset + (rel_y / Self::ITEM_HEIGHT);
        if item_idx < self.visible_item_count() {
            Some(item_idx)
        } else {
            None
        }
    }

    /// Mueve la selección hacia arriba o hacia abajo y ajusta el scroll si es necesario
    pub(crate) fn navigate_selection(&mut self, delta: isize, max_visible: usize) {
        let count = self.visible_item_count();
        if count == 0 {
            return;
        }
        let current = self.selected_index.unwrap_or(0);
        let next = if delta > 0 {
            (current + delta as usize).min(count - 1)
        } else {
            current.saturating_sub(delta.unsigned_abs())
        };
        self.selected_index = Some(next);
        self.selected_path = self.visible_item(next).map(|i| i.path.clone());
        self.ensure_visible(next, max_visible);
    }

    /// Garantiza que un elemento sea visible dentro de la ventana de scroll
    pub(crate) fn ensure_visible(&mut self, index: usize, max_visible: usize) {
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
    pub(crate) fn handle_arrow_left(&mut self, max_visible: usize) {
        let count = self.visible_item_count();
        if count == 0 {
            return;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = self.visible_item(0).map(|i| i.path.clone());
            return;
        };
        if idx >= count {
            return;
        }

        let Some(item) = self.visible_item(idx) else {
            return;
        };

        let is_dir = item.is_dir;
        let is_expanded = item.is_expanded;
        let depth = item.depth;

        if is_dir && is_expanded {
            // Colapsar carpeta
            if let Some(real_idx) = self.actual_index_for_visible(idx) {
                self.toggle_expand(real_idx);
                if self.is_filtering() {
                    self.update_filter();
                }
            }
        } else if depth > 0 {
            // Seleccionar directorio padre de este elemento
            let target_depth = depth - 1;
            let mut found_parent = None;
            for p_idx in (0..idx).rev() {
                if let Some(p_item) = self.visible_item(p_idx) {
                    if p_item.is_dir && p_item.depth == target_depth {
                        found_parent = Some((p_idx, p_item.path.clone()));
                        break;
                    }
                }
            }
            if let Some((p_idx, path)) = found_parent {
                self.selected_index = Some(p_idx);
                self.selected_path = Some(path);
                self.ensure_visible(p_idx, max_visible);
            }
        } else if !self.is_filtering() {
            // En nivel raíz: subir de directorio (..)
            self.navigate_up();
            self.selected_index = if self.items.is_empty() { None } else { Some(0) };
            self.selected_path = self.items.first().map(|i| i.path.clone());
        }
    }

    /// Maneja la pulsación de Flecha Derecha: expande carpeta, entra al primer hijo, o devuelve archivo para abrir
    pub(crate) fn handle_arrow_right(&mut self, max_visible: usize) -> Option<PathBuf> {
        let count = self.visible_item_count();
        if count == 0 {
            return None;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = self.visible_item(0).map(|i| i.path.clone());
            return None;
        };
        if idx >= count {
            return None;
        }

        let (is_dir, is_expanded, depth, item_path) = {
            let item = self.visible_item(idx)?;
            (item.is_dir, item.is_expanded, item.depth, item.path.clone())
        };

        if is_dir {
            if !is_expanded {
                if let Some(real_idx) = self.actual_index_for_visible(idx) {
                    self.toggle_expand(real_idx);
                    if self.is_filtering() {
                        self.update_filter();
                    }
                }
            } else if idx + 1 < count {
                let next_child = self.visible_item(idx + 1).and_then(|next_item| {
                    if next_item.depth > depth {
                        Some((idx + 1, next_item.path.clone()))
                    } else {
                        None
                    }
                });
                if let Some((next_idx, path)) = next_child {
                    self.selected_index = Some(next_idx);
                    self.selected_path = Some(path);
                    self.ensure_visible(next_idx, max_visible);
                }
            }
            None
        } else {
            Some(item_path)
        }
    }

    /// Maneja la pulsación de Enter: alterna carpeta o devuelve archivo para abrir en visor
    pub(crate) fn handle_enter(&mut self, _max_visible: usize) -> Option<PathBuf> {
        let count = self.visible_item_count();
        if count == 0 {
            return None;
        }
        let Some(idx) = self.selected_index else {
            self.selected_index = Some(0);
            self.selected_path = self.visible_item(0).map(|i| i.path.clone());
            return None;
        };
        if idx >= count {
            return None;
        }

        let item = self.visible_item(idx)?;

        if item.is_dir {
            if let Some(real_idx) = self.actual_index_for_visible(idx) {
                self.toggle_expand(real_idx);
                if self.is_filtering() {
                    self.update_filter();
                }
            }
            None
        } else {
            Some(item.path.clone())
        }
    }
}

#[cfg(test)]
mod tests;
