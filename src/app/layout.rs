//! Disposición de pestañas y paneles: árbol de divisiones, rectángulos y geometría.

use super::*;

/// Identificador de un panel de terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct PaneId(pub(crate) usize);

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for PaneId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl From<PaneId> for usize {
    fn from(id: PaneId) -> Self {
        id.0
    }
}

impl std::ops::Add<usize> for PaneId {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }
}

impl std::ops::AddAssign<usize> for PaneId {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

/// Identificador de una pestaña.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct TabId(pub(crate) usize);

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for TabId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl From<TabId> for usize {
    fn from(id: TabId) -> Self {
        id.0
    }
}

impl std::ops::Add<usize> for TabId {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }
}

impl std::ops::AddAssign<usize> for TabId {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SplitDirection {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PaneDirection {
    #[default]
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl From<PaneId> for PaneNode {
    fn from(id: PaneId) -> Self {
        PaneNode::Leaf(id)
    }
}

/// Grosor del divisor entre dos paneles
const DIVIDER: usize = 2;
/// Tamaño mínimo (px) de un panel al arrastrar un divisor
const MIN_PANE: f32 = 48.0;

/// Divisor entre dos paneles
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Divider {
    /// `(x, y, ancho, alto)`
    pub(crate) rect: (usize, usize, usize, usize),
    /// Camino desde la raíz del árbol hasta el `Split` que lo origina (`false` = primer hijo)
    pub(crate) path: Vec<bool>,
    pub(crate) direction: SplitDirection,
    /// Inicio (y u x según `direction`) y largo disponible del `Split`, sin el divisor
    pub(crate) origin: usize,
    pub(crate) avail: usize,
}

impl Divider {
    /// Margen extra (px) a cada lado para poder agarrar un divisor de 2 px
    const GRAB: usize = 3;

    pub(crate) fn contains(&self, mx: usize, my: usize) -> bool {
        let (x, y, w, h) = self.rect;
        mx + Self::GRAB >= x
            && mx < x + w + Self::GRAB
            && my + Self::GRAB >= y
            && my < y + h + Self::GRAB
    }

    /// Proporción que deja el divisor con el borde del primer panel en `pos` (px de ventana)
    pub(crate) fn ratio_at(&self, pos: usize) -> f32 {
        if self.avail == 0 {
            return 0.5;
        }
        let min = (MIN_PANE / self.avail as f32).min(0.5);
        ((pos as f32 - self.origin as f32 - DIVIDER as f32 / 2.0) / self.avail as f32)
            .clamp(min, 1.0 - min)
    }

    pub(crate) fn cursor_icon(&self) -> winit::window::CursorIcon {
        match self.direction {
            SplitDirection::Horizontal => winit::window::CursorIcon::RowResize,
            SplitDirection::Vertical => winit::window::CursorIcon::ColResize,
        }
    }

    /// Coordenada del ratón que mueve este divisor
    pub(crate) fn axis(&self, mx: usize, my: usize) -> usize {
        match self.direction {
            SplitDirection::Horizontal => my,
            SplitDirection::Vertical => mx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaneRect {
    pub(crate) pane_id: PaneId,
    pub(crate) x: usize,
    /// Borde superior del área del terminal (ya descuenta la cabecera del panel)
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// Borde superior del hueco completo, cabecera incluida: de ahí cuelga `PaneHeaderRects`
    pub(crate) slot_y: usize,
    /// El panel se dibuja con cabecera (pestaña dividida)
    pub(crate) has_header: bool,
}

pub(crate) struct TerminalPane {
    pub(crate) id: PaneId,
    pub(crate) pty: PtyManager,
    pub(crate) terminal: TerminalState,
    pub(crate) last_grid_size: (usize, usize),
    pub(crate) cached_git_branch: Option<String>,
    pub(crate) cached_git_dir: Option<std::path::PathBuf>,
}

impl TerminalPane {
    pub(crate) fn new(
        id: PaneId,
        pty: PtyManager,
        mut terminal: TerminalState,
        last_grid_size: (usize, usize),
    ) -> Self {
        terminal.pane_token = pty.token.clone();
        Self {
            id,
            pty,
            terminal,
            last_grid_size,
            cached_git_branch: None,
            cached_git_dir: None,
        }
    }

    pub(crate) fn get_git_branch(&mut self) -> Option<String> {
        let is_ssh = self
            .terminal
            .ssh_session
            .as_ref()
            .map(|s| s.is_active)
            .unwrap_or(false);

        if is_ssh {
            // Con la integración del shell remoto la rama llega en cada prompt (vacía = no es un repositorio)
            if self.terminal.remote_integration {
                return self
                    .terminal
                    .ssh_session
                    .as_ref()
                    .and_then(|s| s.remote_git_branch.clone());
            }
            let rows = self.terminal.grid.len();
            let mut detected_branch = None;
            let mut latest_prompt_dir = None;
            let mut found_prompt = false;

            for r in (0..rows).rev().take(35) {
                if let Some(row) = self.terminal.grid.get(r) {
                    let line: String = row.iter().map(|c| c.c).collect();
                    let trimmed = line.trim();

                    // Detectar CWD y rama del prompt: ej. "user@host:/path$" o "user@host:~/project (qa)$"
                    if let Some((user_host, rest)) = trimmed.split_once(':') {
                        if user_host.contains('@') {
                            if let Some((path_part, _)) =
                                rest.split_once('$').or_else(|| rest.split_once('#'))
                            {
                                let dir_raw = path_part.trim();
                                if !found_prompt {
                                    found_prompt = true;
                                    // Si contiene rama en paréntesis, ej. "/home/django (qa)"
                                    if let Some((dir_clean, branch_inside)) =
                                        dir_raw.split_once('(')
                                    {
                                        let dir = dir_clean.trim();
                                        latest_prompt_dir = Some(dir.to_string());
                                        if detected_branch.is_none() {
                                            let b = branch_inside.trim_end_matches(')').trim();
                                            let words: Vec<&str> = b.split_whitespace().collect();
                                            if words.len() == 1
                                                && words[0].chars().all(|c| {
                                                    c.is_alphanumeric()
                                                        || matches!(c, '/' | '-' | '_' | '.')
                                                })
                                            {
                                                detected_branch = Some(words[0].to_string());
                                            }
                                        }
                                    } else {
                                        let dir =
                                            dir_raw.split_whitespace().next().unwrap_or(dir_raw);
                                        latest_prompt_dir = Some(dir.to_string());
                                    }
                                }
                            }
                        }
                    }

                    // Rama mostrada por la salida de git (git status / git branch), en cualquier idioma
                    if detected_branch.is_none() {
                        detected_branch = shell_context::branch_from_output_line(trimmed);
                    }
                }
            }

            if let Some(ref mut s) = self.terminal.ssh_session {
                if let Some(d) = latest_prompt_dir
                    .as_deref()
                    .and_then(crate::pty::safe_remote_dir)
                {
                    if s.remote_dir.as_deref() != Some(d) {
                        s.remote_dir = Some(d.to_string());
                        s.remote_git_branch = None; // Reset cached branch on directory change!
                        if let Some(dir) = crate::terminal::performer::accept_cwd(d) {
                            self.terminal.current_dir = Some(dir);
                        }
                    }
                }
                if detected_branch.is_some() {
                    s.remote_git_branch = detected_branch.clone();
                }
                return s.remote_git_branch.clone();
            }

            return detected_branch;
        }

        if self.terminal.current_dir != self.cached_git_dir {
            self.cached_git_branch = get_git_branch(self.terminal.current_dir.as_deref());
            self.cached_git_dir = self.terminal.current_dir.clone();
        }
        self.cached_git_branch.clone()
    }

    /// El panel corre un agente de IA (✧): lo avisa el wrapper del shell o el título de la ventana.
    pub(crate) fn is_agent_pane(&self) -> bool {
        self.terminal.agent_running || self.terminal.active_ai_agent() != AiAgentStatus::None
    }

    /// Título de la cabecera del panel: el título que el shell puso a su ventana (el del agente) y,
    /// si no hay, el de la pestaña.
    pub(crate) fn header_title(&self, tab_title: &str) -> String {
        let raw = self
            .terminal
            .window_title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(tab_title);
        TerminalTab::clean_display_title(raw, self.terminal.current_dir.as_deref())
    }

    /// Subtítulo atenuado de la cabecera: destino SSH, carpeta del proyecto o el comando en marcha.
    pub(crate) fn header_subtitle(&self) -> Option<String> {
        if let Some(ssh) = self.terminal.ssh_session.as_ref().filter(|s| s.is_active) {
            return Some(ssh.destination.clone());
        }
        let cwd_name = self
            .terminal
            .current_dir
            .as_deref()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .map(str::to_string);
        // Un agente se identifica por su proyecto, no por el comando que lo lanzó
        if self.is_agent_pane() {
            return cwd_name;
        }
        self.running_command().or(cwd_name)
    }

    /// Comando en curso según OSC 133 (entre «salida empieza» y «comando terminado»).
    fn running_command(&self) -> Option<String> {
        let term = &self.terminal;
        if term.is_alt_screen {
            return None;
        }
        let block = term.active_block.as_ref()?;
        // Mientras solo se teclea (sin OSC 133;C) el subtítulo sigue siendo la carpeta
        block.output_start?;
        let line = block.command_line?;
        command_hint(&term.row_text_to_string(term.get_buffer_line(line)?)).map(|c| c.into_owned())
    }
}

/// Texto del comando sin el prefijo del prompt (`PS C:\x> `, `$ `, `❯ `) y acotado para la
/// cabecera. OSC 133 solo da la línea donde se escribió, no el comando aislado.
pub(super) fn command_hint<'a>(line: &'a str) -> Option<std::borrow::Cow<'a, str>> {
    let text = line.trim();
    if text.is_empty() {
        return None;
    }
    let cut = text
        .char_indices()
        .filter(|(i, c)| {
            matches!(c, '>' | '$' | '#' | '❯') && text[i + c.len_utf8()..].starts_with(' ')
        })
        .map(|(i, _)| i + 1)
        .next_back();
    let hint = cut.map_or(text, |i| text[i..].trim_start()).trim();
    if hint.is_empty() {
        return None;
    }
    if hint.chars().count() <= 48 {
        Some(std::borrow::Cow::Borrowed(hint))
    } else {
        Some(std::borrow::Cow::Owned(hint.chars().take(48).collect()))
    }
}

pub(crate) struct TerminalTab {
    pub(crate) id: TabId,
    pub(crate) title: String,
    pub(crate) panes: Vec<TerminalPane>,
    pub(crate) active_pane_id: PaneId,
    pub(crate) layout: PaneNode,
    pub(crate) has_unread: bool,
    pub(crate) is_settings: bool,
    pub(crate) file_viewer: Option<FileViewerState>,
    /// Pestaña de vista previa (un clic en el explorador): la siguiente vista previa la reemplaza hasta que se fije
    pub(crate) is_preview: bool,
    /// Cuándo se abrió la pestaña (subtítulo «N min» de la lista de sesiones).
    pub(crate) started_at: Instant,
    /// Panel maximizado: ocupa todo el área y el resto del reparto queda oculto (solo mientras es el activo)
    pub(crate) zoomed_pane: Option<PaneId>,
}

impl TerminalTab {
    pub(crate) fn new(id: TabId, title: String, first_pane: TerminalPane) -> Self {
        let active_pane_id = first_pane.id;
        let layout = PaneNode::Leaf(active_pane_id);
        Self {
            id,
            title,
            panes: vec![first_pane],
            active_pane_id,
            layout,
            has_unread: false,
            is_settings: false,
            file_viewer: None,
            is_preview: false,
            started_at: Instant::now(),
            zoomed_pane: None,
        }
    }

    /// Pestaña con varios paneles ya creados (restauración de sesión)
    pub(crate) fn from_layout(
        id: TabId,
        title: String,
        layout: PaneNode,
        panes: Vec<TerminalPane>,
        active_pane_id: PaneId,
    ) -> Self {
        Self {
            id,
            title,
            panes,
            active_pane_id,
            layout,
            has_unread: false,
            is_settings: false,
            file_viewer: None,
            is_preview: false,
            started_at: Instant::now(),
            zoomed_pane: None,
        }
    }

    pub(crate) fn new_settings(id: TabId) -> Self {
        Self {
            id,
            title: "Ajustes".to_string(),
            panes: Vec::new(),
            active_pane_id: PaneId(0),
            layout: PaneNode::Leaf(PaneId(0)),
            has_unread: false,
            is_settings: true,
            file_viewer: None,
            is_preview: false,
            started_at: Instant::now(),
            zoomed_pane: None,
        }
    }

    pub(crate) fn new_file_viewer(id: TabId, path: std::path::PathBuf) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archivo".to_string());

        // La fecha se toma antes de leer: una escritura posterior cuenta como cambio externo
        let disk_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let mut is_binary = false;
        let mut lines = Vec::new();
        let mut text_meta = (false, true, false);

        if file_size > 5 * 1024 * 1024 {
            is_binary = true;
            lines.push(format!(
                "El archivo es demasiado grande ({:.1} MB) para previsualizar.",
                file_size as f64 / (1024.0 * 1024.0)
            ));
        } else {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let check_len = bytes.len().min(4096);
                    if bytes[..check_len].contains(&0) {
                        is_binary = true;
                        lines.push(
                            "Archivo binario detectado. No se puede previsualizar en modo texto."
                                .to_string(),
                        );
                    } else {
                        let d = crate::app::viewer::DecodedText::decode(&bytes);
                        lines = d.lines;
                        text_meta = (d.crlf, d.final_newline, d.non_utf8);
                    }
                }
                Err(e) => {
                    is_binary = true;
                    lines.push(format!("Error al leer el archivo: {}", e));
                }
            }
        }

        let original_lines = lines.clone();
        let git_base = if is_binary || !cfg!(test) {
            None
        } else {
            crate::explorer::git_head_lines(&path)
        };
        let is_markdown = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false);
        let local_dir = if path.is_relative() {
            std::env::current_dir()
                .ok()
                .map(|cd| cd.join(&path))
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        } else {
            path.parent().map(|p| p.to_path_buf())
        };
        let local_branch = crate::app::shell_context::get_git_branch(local_dir.as_deref());
        let viewer = FileViewerState {
            path: path.clone(),
            file_name: file_name.clone(),
            lines,
            scroll_offset: 0,
            scroll_offset_x: 0,
            is_binary,
            file_size,
            cursor_line: 0,
            cursor_col: 0,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            original_lines,
            git_base,
            diff_stash: None,
            hovered_diff_btn: false,
            git_marks: Default::default(),
            selection: None,
            remote_destination: None,
            remote_branch: local_branch,
            is_read_only: true,
            crlf: text_meta.0,
            final_newline: text_meta.1,
            non_utf8: text_meta.2,
            save_error: None,
            render_markdown: is_markdown,
            hovered_mode_btn: false,
            hovered_md_btn: false,
            hovered_scrollbar: false,
            hovered_hscrollbar: false,
            hovered_gutter: false,
            visible_cols: 80,
            word_wrap: false,
            folded: Default::default(),
            disk_mtime,
            needs_autosave: false,
            last_edit_time: Instant::now(),
            edit_gen: 0,
            fold_gen: 0,
            rows_cache: Default::default(),
        };

        Self {
            id,
            title: file_name,
            panes: Vec::new(),
            active_pane_id: PaneId(0),
            layout: PaneNode::Leaf(PaneId(0)),
            has_unread: false,
            is_settings: false,
            file_viewer: Some(viewer),
            is_preview: false,
            started_at: Instant::now(),
            zoomed_pane: None,
        }
    }

    /// Pestaña de visor para un archivo remoto ya leído (por SSH o por el propio terminal)
    pub(crate) fn new_remote_file_viewer_from(
        id: TabId,
        dest: String,
        path: std::path::PathBuf,
        remote_branch: Option<String>,
        data: Result<Vec<u8>, String>,
    ) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archivo".to_string());

        let mut lines = Vec::new();
        let mut is_binary = false;
        let mut file_size = 0;
        let mut non_utf8 = false;
        // Fin de línea y salto final del archivo remoto: `remote_content` los conserva al guardar
        let (mut crlf, mut final_newline) = (false, true);

        match data {
            Ok(bytes) => {
                file_size = bytes.len() as u64;
                if file_size > 5 * 1024 * 1024 {
                    is_binary = true;
                    lines.push(format!(
                        "El archivo es demasiado grande ({:.1} MB) para previsualizar.",
                        file_size as f64 / (1024.0 * 1024.0)
                    ));
                } else if bytes[..bytes.len().min(4096)].contains(&0) {
                    is_binary = true;
                    lines.push(
                        "Archivo binario detectado. No se puede previsualizar en modo texto."
                            .to_string(),
                    );
                } else {
                    let d = crate::app::viewer::DecodedText::decode(&bytes);
                    lines = d.lines;
                    non_utf8 = d.non_utf8;
                    crlf = d.crlf;
                    final_newline = d.final_newline;
                }
            }
            Err(err) if err.contains("Permission denied") => {
                is_binary = true;
                lines.push(
                    "El servidor pide contraseña: para abrir archivos remotos configura"
                        .to_string(),
                );
                lines.push(
                    "una clave SSH (ssh-keygen y ssh-copy-id) y vuelve a abrirlo.".to_string(),
                );
            }
            Err(err) => {
                is_binary = true;
                lines.push(format!("Error remoto: {}", err));
            }
        }

        let original_lines = lines.clone();
        let is_markdown = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false);
        let viewer = FileViewerState {
            path: path.clone(),
            file_name: file_name.clone(),
            lines,
            scroll_offset: 0,
            scroll_offset_x: 0,
            is_binary,
            file_size,
            cursor_line: 0,
            cursor_col: 0,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            original_lines,
            git_base: None,
            diff_stash: None,
            hovered_diff_btn: false,
            git_marks: Default::default(),
            selection: None,
            remote_destination: Some(dest),
            remote_branch,
            is_read_only: true,
            crlf,
            final_newline,
            non_utf8,
            save_error: None,
            render_markdown: is_markdown,
            hovered_mode_btn: false,
            hovered_md_btn: false,
            hovered_scrollbar: false,
            hovered_hscrollbar: false,
            hovered_gutter: false,
            visible_cols: 80,
            word_wrap: false,
            folded: Default::default(),
            disk_mtime: None,
            needs_autosave: false,
            last_edit_time: Instant::now(),
            edit_gen: 0,
            fold_gen: 0,
            rows_cache: Default::default(),
        };

        Self {
            id,
            title: file_name,
            panes: Vec::new(),
            active_pane_id: PaneId(0),
            layout: PaneNode::Leaf(PaneId(0)),
            has_unread: false,
            is_settings: false,
            file_viewer: Some(viewer),
            is_preview: false,
            started_at: Instant::now(),
            zoomed_pane: None,
        }
    }

    pub(crate) fn get_git_branch(&mut self) -> Option<String> {
        if let Some(pane) = self.active_pane_mut() {
            pane.get_git_branch()
        } else if let Some(ref viewer) = self.file_viewer {
            viewer.remote_branch.clone()
        } else {
            None
        }
    }

    /// Contexto real de la pestaña para la segunda línea de la barra lateral:
    /// carpeta contenedora, directorio remoto o número de paneles. Nada decorativo.
    pub(crate) fn sidebar_detail(&self) -> Option<String> {
        if self.is_settings {
            return Some("Preferencias".to_string());
        }
        if let Some(ref viewer) = self.file_viewer {
            if let Some(ref dest) = viewer.remote_destination {
                return Some(dest.clone());
            }
            return viewer
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .or_else(|| Some("Archivo".to_string()));
        }
        let active = self.active_pane()?;
        let panes = if self.panes.len() > 1 {
            Some(format!("{} paneles", self.panes.len()))
        } else {
            None
        };
        if let Some(ssh) = active.terminal.ssh_session.as_ref().filter(|s| s.is_active) {
            let remote = ssh
                .remote_dir
                .clone()
                .unwrap_or_else(|| "Sesión remota".to_string());
            return Some(match panes {
                Some(panes) => format!("{remote} · {panes}"),
                None => remote,
            });
        }
        if panes.is_some() {
            return panes;
        }
        let dir = active.terminal.current_dir.as_deref()?;
        let text = dir.parent().unwrap_or(dir).display().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub(crate) fn display_title(&self) -> String {
        if self.is_settings {
            return "Ajustes".to_string();
        }
        if let Some(ref viewer) = self.file_viewer {
            if viewer.is_dirty {
                return format!("● {}", viewer.file_name);
            } else {
                return viewer.file_name.clone();
            }
        }
        if let Some(active) = self.active_pane() {
            if let Some(ref ssh) = active.terminal.ssh_session {
                if ssh.is_active {
                    return format!("ssh: {}", ssh.destination);
                }
            }
            let raw = active
                .terminal
                .window_title
                .as_deref()
                .unwrap_or(&self.title);
            return Self::clean_display_title(raw, active.terminal.current_dir.as_deref());
        }
        Self::clean_display_title(&self.title, None)
    }

    pub(crate) fn clean_display_title(raw: &str, cwd: Option<&std::path::Path>) -> String {
        use crate::renderer::contains_ignore_ascii_case;
        let trimmed = raw.trim();
        let ends_ci = |suf: &str| {
            trimmed.len() >= suf.len()
                && trimmed.as_bytes()[trimmed.len() - suf.len()..]
                    .eq_ignore_ascii_case(suf.as_bytes())
        };
        let is_shell_or_path = ends_ci("pwsh.exe")
            || ends_ci("powershell.exe")
            || ends_ci("cmd.exe")
            || trimmed.eq_ignore_ascii_case("powershell")
            || trimmed.eq_ignore_ascii_case("pwsh")
            || trimmed.eq_ignore_ascii_case("cmd")
            || trimmed.eq_ignore_ascii_case("windows powershell")
            || contains_ignore_ascii_case(trimmed, "powershell\\7\\pwsh.exe")
            || contains_ignore_ascii_case(trimmed, "system32\\cmd.exe")
            || contains_ignore_ascii_case(trimmed, "windowspowershell");

        if is_shell_or_path {
            if let Some(path) = cwd {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
                return path.display().to_string();
            }
            return "PowerShell".to_string();
        }

        if raw.contains('\\') || raw.contains('/') {
            let p = std::path::Path::new(raw);
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if !stem.is_empty() {
                    return stem.to_string();
                }
            }
        }

        raw.trim().to_string()
    }

    pub(crate) fn active_pane(&self) -> Option<&TerminalPane> {
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return None;
        }
        self.panes
            .iter()
            .find(|p| p.id == self.active_pane_id)
            .or_else(|| self.panes.first())
    }

    pub(crate) fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return None;
        }
        let active_id = self.active_pane_id;
        if let Some(pos) = self.panes.iter().position(|p| p.id == active_id) {
            Some(&mut self.panes[pos])
        } else {
            self.panes.first_mut()
        }
    }

    pub(crate) fn find_pane(&self, pane_id: PaneId) -> Option<&TerminalPane> {
        self.panes.iter().find(|p| p.id == pane_id)
    }

    pub(crate) fn find_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut TerminalPane> {
        self.panes.iter_mut().find(|p| p.id == pane_id)
    }

    pub(crate) fn compute_pane_rects(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<PaneRect> {
        self.geometry(total_x, total_y, total_w, total_h).0
    }

    /// Divisores entre paneles: su rectángulo y lo necesario para arrastrarlos
    pub(crate) fn compute_dividers(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<Divider> {
        self.geometry(total_x, total_y, total_w, total_h).1
    }

    /// Única fuente de la geometría de paneles y divisores (dibujo, clics y arrastre)
    fn geometry(&self, x: usize, y: usize, w: usize, h: usize) -> (Vec<PaneRect>, Vec<Divider>) {
        let mut out = (Vec::new(), Vec::new());
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return out;
        }
        let has_header = self.panes.len() > 1;
        // El zoom solo vale mientras su panel es el activo: al navegar a otro se recupera el reparto
        if let Some(pane_id) = self
            .zoomed_pane
            .filter(|id| *id == self.active_pane_id && self.find_pane(*id).is_some())
        {
            let offset = if has_header {
                crate::renderer::PaneHeaderRects::CONTENT_OFFSET
            } else {
                0
            };
            out.0.push(PaneRect {
                pane_id,
                x,
                y: y + offset,
                width: w,
                height: h.saturating_sub(offset),
                slot_y: y,
                has_header,
            });
            return out;
        }
        Self::walk(
            &self.layout,
            (x, y, w, h),
            has_header,
            &mut Vec::new(),
            &mut out,
        );
        out
    }

    fn walk(
        node: &PaneNode,
        (x, y, w, h): (usize, usize, usize, usize),
        has_header: bool,
        path: &mut Vec<bool>,
        out: &mut (Vec<PaneRect>, Vec<Divider>),
    ) {
        let PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } = node
        else {
            if let PaneNode::Leaf(pane_id) = node {
                let offset = if has_header {
                    crate::renderer::PaneHeaderRects::CONTENT_OFFSET
                } else {
                    0
                };
                out.0.push(PaneRect {
                    pane_id: *pane_id,
                    x,
                    y: y + offset,
                    width: w,
                    height: h.saturating_sub(offset),
                    slot_y: y,
                    has_header,
                });
            }
            return;
        };
        let (origin, len) = match direction {
            SplitDirection::Horizontal => (y, h),
            SplitDirection::Vertical => (x, w),
        };
        let avail = len.saturating_sub(DIVIDER);
        let len1 = (((avail as f32) * ratio).round() as usize).min(avail);
        let len2 = avail - len1;
        let (rect1, rect2, divider) = match direction {
            SplitDirection::Horizontal => (
                (x, y, w, len1),
                (x, y + len1 + DIVIDER, w, len2),
                (x, y + len1, w, DIVIDER),
            ),
            SplitDirection::Vertical => (
                (x, y, len1, h),
                (x + len1 + DIVIDER, y, len2, h),
                (x + len1, y, DIVIDER, h),
            ),
        };
        out.1.push(Divider {
            rect: divider,
            path: path.clone(),
            direction: *direction,
            origin,
            avail,
        });
        path.push(false);
        Self::walk(first, rect1, has_header, path, out);
        path.pop();
        path.push(true);
        Self::walk(second, rect2, has_header, path, out);
        path.pop();
    }

    /// Mueve el divisor `path` (camino desde la raíz: `false` = primer hijo) a `ratio`
    pub(crate) fn set_split_ratio(&mut self, path: &[bool], new_ratio: f32) {
        let mut node = &mut self.layout;
        for &second in path {
            let PaneNode::Split {
                first, second: s, ..
            } = node
            else {
                return;
            };
            node = if second { s } else { first };
        }
        if let PaneNode::Split { ratio, .. } = node {
            *ratio = new_ratio;
        }
    }

    /// Árbol de paneles en forma persistible; `cwd_of` da la carpeta local de cada panel
    pub(crate) fn session_layout(
        &self,
        cwd_of: &dyn Fn(&TerminalPane) -> Option<String>,
    ) -> PaneSession {
        fn conv(
            tab: &TerminalTab,
            node: &PaneNode,
            cwd_of: &dyn Fn(&TerminalPane) -> Option<String>,
        ) -> PaneSession {
            match node {
                PaneNode::Leaf(id) => PaneSession::Leaf {
                    cwd: tab.find_pane(*id).and_then(cwd_of),
                },
                PaneNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                } => PaneSession::Split {
                    direction: match direction {
                        SplitDirection::Horizontal => "horizontal",
                        SplitDirection::Vertical => "vertical",
                    }
                    .to_string(),
                    ratio: *ratio,
                    first: Box::new(conv(tab, first, cwd_of)),
                    second: Box::new(conv(tab, second, cwd_of)),
                },
            }
        }
        conv(self, &self.layout, cwd_of)
    }

    /// Ids de los paneles de `layout` de izquierda a derecha
    pub(crate) fn leaf_ids(&self) -> smallvec::SmallVec<[PaneId; 4]> {
        fn walk(node: &PaneNode, out: &mut smallvec::SmallVec<[PaneId; 4]>) {
            match node {
                PaneNode::Leaf(id) => out.push(*id),
                PaneNode::Split { first, second, .. } => {
                    walk(first, out);
                    walk(second, out);
                }
            }
        }
        let mut ids = smallvec::SmallVec::new();
        walk(&self.layout, &mut ids);
        ids
    }

    pub(crate) fn split_active_pane(
        &mut self,
        new_pane_id: PaneId,
        new_pane: TerminalPane,
        direction: SplitDirection,
    ) {
        let active_id = self.active_pane_id;
        self.layout =
            Self::split_leaf_in_tree(self.layout.clone(), active_id, new_pane_id, direction);
        self.panes.push(new_pane);
        self.active_pane_id = new_pane_id;
        // El panel nuevo debe verse: dividir con uno maximizado restauraría un reparto oculto
        self.zoomed_pane = None;
    }

    /// Maximiza `pane_id` (ocupa todo el área) o restaura el reparto si ya estaba maximizado.
    /// Devuelve `true` si el estado cambió.
    pub(crate) fn toggle_pane_zoom(&mut self, pane_id: PaneId) -> bool {
        let next = if self.zoomed_pane == Some(pane_id) || self.panes.len() < 2 {
            None
        } else {
            Some(pane_id)
        };
        if self.zoomed_pane == next {
            return false;
        }
        self.zoomed_pane = next;
        true
    }

    fn split_leaf_in_tree(
        node: PaneNode,
        target_id: PaneId,
        new_id: PaneId,
        direction: SplitDirection,
    ) -> PaneNode {
        match node {
            PaneNode::Leaf(id) => {
                if id == target_id {
                    PaneNode::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(PaneNode::Leaf(target_id)),
                        second: Box::new(PaneNode::Leaf(new_id)),
                    }
                } else {
                    PaneNode::Leaf(id)
                }
            }
            PaneNode::Split {
                direction: d,
                ratio,
                first,
                second,
            } => PaneNode::Split {
                direction: d,
                ratio,
                first: Box::new(Self::split_leaf_in_tree(
                    *first, target_id, new_id, direction,
                )),
                second: Box::new(Self::split_leaf_in_tree(
                    *second, target_id, new_id, direction,
                )),
            },
        }
    }

    pub(crate) fn close_pane(&mut self, pane_id: PaneId) {
        self.panes.retain(|p| p.id != pane_id);
        if let Some(new_tree) = Self::remove_leaf_from_tree(self.layout.clone(), pane_id) {
            self.layout = new_tree;
        }
        if self.active_pane_id == pane_id {
            if let Some(p) = self.panes.last() {
                self.active_pane_id = p.id;
            }
        }
        if self.zoomed_pane == Some(pane_id) {
            self.zoomed_pane = None;
        }
    }

    fn remove_leaf_from_tree(node: PaneNode, target_id: PaneId) -> Option<PaneNode> {
        match node {
            PaneNode::Leaf(id) => {
                if id == target_id {
                    None
                } else {
                    Some(PaneNode::Leaf(id))
                }
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let f = Self::remove_leaf_from_tree(*first, target_id);
                let s = Self::remove_leaf_from_tree(*second, target_id);
                match (f, s) {
                    (Some(f_node), Some(s_node)) => Some(PaneNode::Split {
                        direction,
                        ratio,
                        first: Box::new(f_node),
                        second: Box::new(s_node),
                    }),
                    (Some(f_node), None) => Some(f_node),
                    (None, Some(s_node)) => Some(s_node),
                    (None, None) => None,
                }
            }
        }
    }
}
