use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const FILE_EXPLORER_WIDTH: usize = 256;
pub(crate) const FILE_EXPLORER_MIN_WIDTH: usize = 168;

#[inline]
fn cmp_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitFileStatus {
    Modified,
    Untracked,
    Added,
    Deleted,
    Renamed,
}

/// Raíz del repositorio git que contiene `dir` (busca `.git` hacia arriba)
pub(crate) fn git_repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|p| p.join(".git").exists())
        .map(Path::to_path_buf)
}

fn git_command(repo: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(repo);
    crate::app::platform::hide_console(&mut cmd);
    cmd
}

/// Contenido de `path` en `HEAD`, base para marcar en el visor lo que cambió.
/// `Some(vacío)` si el archivo es nuevo en un repo (todo es añadido); `None` fuera de un repo.
pub(crate) fn git_head_lines(path: &Path) -> Option<Vec<String>> {
    let repo = git_repo_root(path.parent()?)?;
    let rel = path
        .strip_prefix(&repo)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let out = git_command(&repo)
        .args(["show", &format!("HEAD:{rel}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return Some(Vec::new());
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(String::from)
            .collect(),
    )
}

/// Estado de una carpeta según lo que cambió dentro: si todo es del mismo tipo, ese (verde añadido,
/// rojo borrado); con una mezcla, o con modificados, amarillo. `None` si no cambió nada.
pub(crate) fn dir_git_status(
    git_map: &FxHashMap<PathBuf, GitFileStatus>,
    dir: &Path,
) -> Option<GitFileStatus> {
    let mut found: Option<GitFileStatus> = None;
    for (path, status) in git_map {
        if !path.starts_with(dir) {
            continue;
        }
        let kind = match status {
            GitFileStatus::Untracked | GitFileStatus::Added => GitFileStatus::Added,
            GitFileStatus::Deleted => GitFileStatus::Deleted,
            GitFileStatus::Modified | GitFileStatus::Renamed => GitFileStatus::Modified,
        };
        match found {
            None => found = Some(kind),
            Some(prev) if prev == kind => {}
            Some(_) => return Some(GitFileStatus::Modified),
        }
    }
    found
}

/// Ejecuta `git status --porcelain -z` sobre `repo_root` y devuelve el estado por ruta absoluta.
/// `--no-optional-locks`: sin él `git status` toma `index.lock` para refrescar el índice y hace fallar un
/// `git add`/`commit` que el usuario o un agente lance a la vez.
pub(crate) fn collect_git_statuses(
    repo_root: &Path,
    changes_only: bool,
) -> FxHashMap<PathBuf, GitFileStatus> {
    let untracked_mode = if changes_only { "-uall" } else { "-unormal" };
    let Ok(output) = git_command(repo_root)
        .args(["--no-optional-locks", "status", "--porcelain", "-z"])
        .arg(untracked_mode)
        .output()
    else {
        return FxHashMap::default();
    };
    if !output.status.success() {
        return FxHashMap::default();
    }
    parse_porcelain_z(repo_root, &output.stdout)
}

/// Salida de `git status --porcelain -z`: entradas `XY ruta` separadas por NUL, sin comillas ni escapes (los
/// nombres con tildes o espacios llegan tal cual); tras un renombrado/copia va otra entrada con el origen
fn parse_porcelain_z(repo_root: &Path, stdout: &[u8]) -> FxHashMap<PathBuf, GitFileStatus> {
    let mut map = FxHashMap::default();
    let text = String::from_utf8_lossy(stdout);
    let mut entries = text.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(status_code), Some(rel_path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        if rel_path.is_empty() {
            continue;
        }
        if status_code.contains(['R', 'C']) {
            entries.next();
        }
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

        map.insert(file_path, status);
    }
    map
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ExplorerItem {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) depth: usize,
    pub(crate) is_expanded: bool,
    pub(crate) git_status: Option<GitFileStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RemoteSshSession {
    pub(crate) destination: String,
    pub(crate) remote_dir: String,
}

impl From<(&str, &str)> for RemoteSshSession {
    fn from((destination, remote_dir): (&str, &str)) -> Self {
        Self {
            destination: destination.to_string(),
            remote_dir: remote_dir.to_string(),
        }
    }
}

impl From<(String, String)> for RemoteSshSession {
    fn from((destination, remote_dir): (String, String)) -> Self {
        Self {
            destination,
            remote_dir,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct FileExplorer {
    pub root_dir: PathBuf,
    pub items: Vec<ExplorerItem>,
    pub scroll_offset: usize,
    pub hovered_index: Option<usize>,
    pub hovered_back_btn: bool,
    pub hovered_forward_btn: bool,
    pub hovered_up_btn: bool,
    pub selected_index: Option<usize>,
    pub selected_path: Option<PathBuf>,
    pub visible: bool,
    /// Ancho del panel en píxeles; se ajusta arrastrando su borde izquierdo (`set_width`)
    pub width: usize,
    pub hovered_scrollbar: bool,
    /// Sesión SSH con integración: el listado llega por el propio terminal (sin segunda conexión)
    pub inband_remote: bool,
    /// «Expandir todo» remoto en curso: cada listado recibido dispara la petición de la siguiente carpeta
    pub remote_expand_all: bool,
    pub is_focused: bool,
    pub last_click_item: Option<usize>,
    pub last_click_time: std::time::Instant,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub git_statuses: FxHashMap<PathBuf, GitFileStatus>,
    /// Rectángulo `(x, y, ancho, alto)` cuando vive dentro del panel lateral; con `None` se acopla al borde derecho
    pub dock: Option<(usize, usize, usize, usize)>,
    /// Raíz del repositorio del directorio actual (None si no hay repo)
    pub git_root: Option<PathBuf>,
    /// Vista «Cambios»: lista plana solo con los archivos modificados según git
    pub changes_only: bool,
    /// Pestaña bajo el ratón: `Some(true)` «Cambios», `Some(false)` «Archivos».
    pub hovered_tab: Option<bool>,
    pub hovered_refresh_btn: bool,
    pub hovered_collapse_btn: bool,
    pub hovered_new_btn: bool,
    /// Fija el explorador en el directorio actual (evita sincronizar con el cwd de la terminal)
    pub is_pinned: bool,
    pub hovered_pin_btn: bool,
    pub remote_session: Option<RemoteSshSession>,
    pub error_message: Option<String>,
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
    pub(crate) expand: Option<PathBuf>,
}

impl FileExplorer {
    pub(crate) const HEADER_HEIGHT: usize = 38;
    /// Aire entre la cabecera de sección y la lista (sin barra de herramientas).
    pub(crate) const SUBHEADER_HEIGHT: usize = 0;
    pub(crate) const ITEM_HEIGHT: usize = 26;
    /// Aire entre el sub-encabezado y la primera fila de la lista
    pub(crate) const LIST_TOP_PAD: usize = 4;

    /// Botón «Cambios» del encabezado (compartido por dibujo y clics): x, y, ancho, alto
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn changes_btn_rect(
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
    ) -> (usize, usize, usize, usize) {
        (panel_x + panel_w.saturating_sub(36), panel_y + 7, 24, 24)
    }

    /// Botón «Fijar» (Pin) del encabezado (compartido por dibujo y clics): x, y, ancho, alto
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn pin_btn_rect(
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
    ) -> (usize, usize, usize, usize) {
        (panel_x + panel_w.saturating_sub(66), panel_y + 7, 24, 24)
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
    #[allow(dead_code)]
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
            && mx < panel_x + 3
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
            hovered_back_btn: false,
            hovered_forward_btn: false,
            hovered_up_btn: false,
            selected_index: None,
            selected_path: None,
            visible,
            width: FILE_EXPLORER_WIDTH,
            hovered_scrollbar: false,
            inband_remote: false,
            remote_expand_all: false,
            remote_requested: None,
            remote_gen: 0,
            ssh_requests: Vec::new(),
            git_status_req: None,
            git_status_running: false,
            is_focused: false,
            last_click_item: None,
            last_click_time: std::time::Instant::now(),
            history: vec![root_dir],
            history_index: 0,
            git_statuses: FxHashMap::default(),
            dock: None,
            git_root: None,
            changes_only: false,
            hovered_tab: None,
            hovered_refresh_btn: false,
            hovered_collapse_btn: false,
            hovered_new_btn: false,
            is_pinned: false,
            hovered_pin_btn: false,
            remote_session: None,
            error_message: None,
        };

        if visible {
            explorer.refresh();
        }
        explorer
    }

    /// Alterna el estado de fijado del explorador
    #[allow(dead_code)]
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
    }

    /// Cierra todas las carpetas abiertas y deja solo el primer nivel
    pub(crate) fn collapse_all(&mut self) {
        self.items.retain(|i| i.depth == 0);
        for item in &mut self.items {
            item.is_expanded = false;
        }
        self.scroll_offset = 0;
        self.selected_index = None;
    }

    /// Abre todas las carpetas locales (hasta 4 niveles y 2 000 entradas; sin `.git`, `target`, etc.)
    pub(crate) fn expand_all(&mut self) {
        const MAX_ITEMS: usize = 2000;
        const MAX_DEPTH: usize = 4;
        const SKIP: [&str; 5] = [".git", "target", "node_modules", ".cargo", "dist"];
        if self.remote_session.is_some() {
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

    pub(crate) fn can_go_back(&self) -> bool {
        !self.inband_remote && self.history_index > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.inband_remote && self.history_index + 1 < self.history.len()
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
        if same && self.listing_names_match(names) {
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
        self.items = Self::remote_items(&base, 0, names);
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
        if session.is_some() && !self.inband_remote && self.remote_requested == session {
            return;
        }
        self.remote_requested = session.clone();
        self.remote_session = session;
        self.inband_remote = false;
        self.scroll_offset = 0;
        self.selected_index = None;
        self.selected_path = None;
        if let Some(ref rem) = self.remote_session {
            self.root_dir = PathBuf::from(&rem.remote_dir);
            self.history = vec![self.root_dir.clone()];
            self.history_index = 0;
            self.items.clear();
            self.refresh_remote();
        } else {
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
        if req.gen != self.remote_gen || self.inband_remote || current != Some(req.dest.as_str()) {
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

    /// Aplica el mapa de estados de `git status` obtenido en segundo plano
    pub(crate) fn apply_git_statuses(
        &mut self,
        repo_root: &Path,
        statuses: FxHashMap<PathBuf, GitFileStatus>,
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
            for item in &mut self.items {
                if item.is_dir {
                    item.git_status = dir_git_status(&self.git_statuses, &item.path);
                } else {
                    item.git_status = self.git_statuses.get(&item.path).copied();
                }
            }
        }
    }

    /// Consulta el estado Git del repositorio activo (en un hilo en la GUI; síncrono en tests)
    pub(crate) fn refresh_git_status(&mut self) {
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
            self.git_statuses = collect_git_statuses(&repo_root, self.changes_only);
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
            .filter(|s| **s != GitFileStatus::Deleted)
            .count()
    }

    /// Alterna entre el árbol de archivos y la lista de archivos con cambios (solo sesiones locales)
    pub(crate) fn toggle_changes_only(&mut self) {
        if self.remote_session.is_some() {
            return;
        }
        self.changes_only = !self.changes_only;
        self.scroll_offset = 0;
        self.selected_index = None;
        self.refresh();
    }

    /// Archivos con cambios (sin los borrados, que ya no se pueden abrir), ordenados por ruta relativa
    fn change_items(&self) -> Vec<ExplorerItem> {
        let mut items: Vec<ExplorerItem> = self
            .git_statuses
            .iter()
            .filter(|(path, status)| **status != GitFileStatus::Deleted && path.is_file())
            .map(|(path, status)| {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dir = self
                    .git_root
                    .as_deref()
                    .and_then(|root| path.parent()?.strip_prefix(root).ok())
                    .map(|d| d.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                ExplorerItem {
                    path: path.clone(),
                    name: if dir.is_empty() {
                        file
                    } else {
                        format!("{file}  {dir}")
                    },
                    is_dir: false,
                    depth: 0,
                    is_expanded: false,
                    git_status: Some(*status),
                }
            })
            .collect();
        items.sort_by(|a, b| a.path.cmp(&b.path));
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
        expanded_paths: &FxHashSet<PathBuf>,
        git_map: &FxHashMap<PathBuf, GitFileStatus>,
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
        let max_offset = self.items.len().saturating_sub(max_visible_lines);
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

    /// Obtiene el índice del elemento en la posición Y considerando el desplazamiento superior
    pub(crate) fn item_at_y(&self, my: usize, top_offset: usize) -> Option<usize> {
        let list_top =
            top_offset + Self::HEADER_HEIGHT + Self::SUBHEADER_HEIGHT + Self::LIST_TOP_PAD;
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
    pub(crate) fn navigate_selection(&mut self, delta: isize, max_visible: usize) {
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
            self.selected_index = if self.items.is_empty() { None } else { Some(0) };
            self.selected_path = self.items.first().map(|i| i.path.clone());
        }
    }

    /// Maneja la pulsación de Flecha Derecha: expande carpeta, entra al primer hijo, o devuelve archivo para abrir
    pub(crate) fn handle_arrow_right(&mut self, max_visible: usize) -> Option<PathBuf> {
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
            } else if idx + 1 < self.items.len()
                && self.items[idx + 1].depth > self.items[idx].depth
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
    pub(crate) fn handle_enter(&mut self, _max_visible: usize) -> Option<PathBuf> {
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

    #[test]
    fn test_dir_git_status_uses_one_color_per_kind_and_yellow_for_mixes() {
        use GitFileStatus::*;
        let map = |entries: &[(&str, GitFileStatus)]| -> FxHashMap<PathBuf, GitFileStatus> {
            entries
                .iter()
                .map(|(p, s)| (PathBuf::from(p), *s))
                .collect()
        };
        let dir = Path::new("repo/src");
        assert_eq!(dir_git_status(&map(&[]), dir), None);
        assert_eq!(
            dir_git_status(&map(&[("repo/docs/a.md", Modified)]), dir),
            None
        );
        assert_eq!(
            dir_git_status(
                &map(&[("repo/src/a.rs", Untracked), ("repo/src/b.rs", Added)]),
                dir
            ),
            Some(Added)
        );
        assert_eq!(
            dir_git_status(&map(&[("repo/src/a.rs", Deleted)]), dir),
            Some(Deleted)
        );
        assert_eq!(
            dir_git_status(&map(&[("repo/src/a.rs", Renamed)]), dir),
            Some(Modified)
        );
        assert_eq!(
            dir_git_status(
                &map(&[("repo/src/a.rs", Deleted), ("repo/src/b.rs", Added)]),
                dir
            ),
            Some(Modified),
            "una mezcla es amarillo"
        );
    }
    use super::*;

    /// `-z` entrega los nombres sin comillas ni escapes y el origen de un renombrado va en otra entrada
    #[test]
    fn test_parse_porcelain_z_handles_unicode_spaces_and_renames() {
        let root = Path::new("C:\\repo");
        let out =
            "?? niño.txt\0 M dir/mi archivo.rs\0R  nuevo.rs\0viejo.rs\0 D borrado.rs\0".as_bytes();
        let map = parse_porcelain_z(root, out);
        assert_eq!(map[&root.join("niño.txt")], GitFileStatus::Untracked);
        assert_eq!(
            map[&root.join("dir\\mi archivo.rs")],
            GitFileStatus::Modified
        );
        assert_eq!(map[&root.join("nuevo.rs")], GitFileStatus::Renamed);
        assert!(!map.contains_key(&root.join("viejo.rs")));
        assert_eq!(map[&root.join("borrado.rs")], GitFileStatus::Deleted);
    }

    /// Una sola consulta de `git status` en vuelo: la ráfaga siguiente queda como una petición pendiente
    #[test]
    fn test_git_status_requests_do_not_overlap() {
        let mut ex = FileExplorer::new(Some(std::env::temp_dir()), false);
        ex.git_status_req = Some((PathBuf::from("a"), false));
        assert!(ex.take_git_status_request().is_some());
        ex.git_status_req = Some((PathBuf::from("a"), false));
        assert!(ex.take_git_status_request().is_none());
        ex.apply_git_statuses(Path::new("a"), FxHashMap::default());
        assert!(ex.take_git_status_request().is_some());
    }

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
        let first_item_y = FileExplorer::HEADER_HEIGHT
            + FileExplorer::SUBHEADER_HEIGHT
            + FileExplorer::LIST_TOP_PAD;
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

    #[test]
    fn test_file_explorer_pin_toggle() {
        let mut explorer = FileExplorer::new(None, false);
        assert!(!explorer.is_pinned);
        explorer.toggle_pinned();
        assert!(explorer.is_pinned);
        explorer.toggle_pinned();
        assert!(!explorer.is_pinned);

        let (px, py, pw, ph) = FileExplorer::pin_btn_rect(100, 50, 260);
        assert_eq!(pw, 24);
        assert_eq!(ph, 24);
        assert_eq!(py, 57);
        assert_eq!(px, 100 + 260 - 66);
    }
}
