//! Disposición de pestañas y paneles: árbol de divisiones, rectángulos y geometría.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(usize),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct PaneRect {
    pub pane_id: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct TerminalPane {
    pub id: usize,
    pub pty: PtyManager,
    pub terminal: TerminalState,
    pub last_grid_size: (usize, usize),
    pub cached_git_branch: Option<String>,
    pub cached_git_dir: Option<std::path::PathBuf>,
}

impl TerminalPane {
    pub fn new(
        id: usize,
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

    pub fn get_git_branch(&mut self) -> Option<String> {
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
                if let Some(ref d) = latest_prompt_dir {
                    if s.remote_dir.as_deref() != Some(d) {
                        s.remote_dir = Some(d.clone());
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
}

pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    pub panes: Vec<TerminalPane>,
    pub active_pane_id: usize,
    pub layout: PaneNode,
    pub has_unread: bool,
    pub is_settings: bool,
    pub file_viewer: Option<FileViewerState>,
    /// Pestaña de vista previa (un clic en el explorador): la siguiente vista previa la reemplaza hasta que se fije
    pub is_preview: bool,
}

impl TerminalTab {
    pub fn new(id: usize, title: String, first_pane: TerminalPane) -> Self {
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
        }
    }

    pub fn new_settings(id: usize) -> Self {
        Self {
            id,
            title: "Ajustes".to_string(),
            panes: Vec::new(),
            active_pane_id: 0,
            layout: PaneNode::Leaf(0),
            has_unread: false,
            is_settings: true,
            file_viewer: None,
            is_preview: false,
        }
    }

    pub fn new_file_viewer(id: usize, path: std::path::PathBuf) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archivo".to_string());

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
                    lines.push(format!("Error al leer el archivo: {}", e));
                }
            }
        }

        let original_lines = lines.clone();
        let git_base = if is_binary {
            None
        } else {
            crate::explorer::git_head_lines(&path)
        };
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
            git_base,
            diff_stash: None,
            hovered_diff_btn: false,
            git_marks: Default::default(),
            selection: None,
            remote_destination: None,
            remote_branch: None,
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
            edit_gen: 0,
            fold_gen: 0,
            rows_cache: Default::default(),
        };

        Self {
            id,
            title: file_name,
            panes: Vec::new(),
            active_pane_id: 0,
            layout: PaneNode::Leaf(0),
            has_unread: false,
            is_settings: false,
            file_viewer: Some(viewer),
            is_preview: false,
        }
    }

    /// Pestaña de visor para un archivo remoto ya leído (por SSH o por el propio terminal)
    pub fn new_remote_file_viewer_from(
        id: usize,
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
                }
            }
            Err(err) if err.contains("Permission denied") => {
                lines.push(
                    "El servidor pide contraseña: para abrir archivos remotos configura"
                        .to_string(),
                );
                lines.push(
                    "una clave SSH (ssh-keygen y ssh-copy-id) y vuelve a abrirlo.".to_string(),
                );
            }
            Err(err) => lines.push(format!("Error remoto: {}", err)),
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
            // El guardado remoto siempre usa LF y salto final (`remote_content`)
            crlf: false,
            final_newline: true,
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
            edit_gen: 0,
            fold_gen: 0,
            rows_cache: Default::default(),
        };

        Self {
            id,
            title: file_name,
            panes: Vec::new(),
            active_pane_id: 0,
            layout: PaneNode::Leaf(0),
            has_unread: false,
            is_settings: false,
            file_viewer: Some(viewer),
            is_preview: false,
        }
    }

    pub fn get_git_branch(&mut self) -> Option<String> {
        if let Some(pane) = self.active_pane_mut() {
            pane.get_git_branch()
        } else if let Some(ref viewer) = self.file_viewer {
            if viewer.remote_destination.is_some() {
                viewer.remote_branch.clone()
            } else {
                crate::app::shell_context::get_git_branch(viewer.path.parent())
            }
        } else {
            None
        }
    }

    pub fn display_title(&self) -> String {
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

    pub fn clean_display_title(raw: &str, cwd: Option<&std::path::Path>) -> String {
        let raw_lower = raw.trim().to_lowercase();
        let is_shell_or_path = raw_lower.ends_with("pwsh.exe")
            || raw_lower.ends_with("powershell.exe")
            || raw_lower.ends_with("cmd.exe")
            || raw_lower == "powershell"
            || raw_lower == "pwsh"
            || raw_lower == "cmd"
            || raw_lower == "windows powershell"
            || raw_lower.contains("powershell\\7\\pwsh.exe")
            || raw_lower.contains("system32\\cmd.exe")
            || raw_lower.contains("windowspowershell");

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

    pub fn active_pane(&self) -> Option<&TerminalPane> {
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return None;
        }
        self.panes
            .iter()
            .find(|p| p.id == self.active_pane_id)
            .or_else(|| self.panes.first())
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
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

    pub fn find_pane(&self, pane_id: usize) -> Option<&TerminalPane> {
        self.panes.iter().find(|p| p.id == pane_id)
    }

    pub fn find_pane_mut(&mut self, pane_id: usize) -> Option<&mut TerminalPane> {
        self.panes.iter_mut().find(|p| p.id == pane_id)
    }

    pub fn compute_pane_rects(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<PaneRect> {
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return Vec::new();
        }
        let mut rects = Vec::new();
        self.collect_rects(&self.layout, total_x, total_y, total_w, total_h, &mut rects);
        rects
    }

    fn collect_rects(
        &self,
        node: &PaneNode,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        out: &mut Vec<PaneRect>,
    ) {
        const DIVIDER: usize = 2;
        match node {
            PaneNode::Leaf(pane_id) => {
                out.push(PaneRect {
                    pane_id: *pane_id,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => match direction {
                SplitDirection::Horizontal => {
                    let avail_h = h.saturating_sub(DIVIDER);
                    let h1 = ((avail_h as f32) * ratio).round() as usize;
                    let h1 = h1.min(avail_h);
                    let h2 = avail_h.saturating_sub(h1);
                    self.collect_rects(first, x, y, w, h1, out);
                    self.collect_rects(second, x, y + h1 + DIVIDER, w, h2, out);
                }
                SplitDirection::Vertical => {
                    let avail_w = w.saturating_sub(DIVIDER);
                    let w1 = ((avail_w as f32) * ratio).round() as usize;
                    let w1 = w1.min(avail_w);
                    let w2 = avail_w.saturating_sub(w1);
                    self.collect_rects(first, x, y, w1, h, out);
                    self.collect_rects(second, x + w1 + DIVIDER, y, w2, h, out);
                }
            },
        }
    }

    pub fn compute_dividers(
        &self,
        total_x: usize,
        total_y: usize,
        total_w: usize,
        total_h: usize,
    ) -> Vec<(usize, usize, usize, usize)> {
        if self.is_settings || self.file_viewer.is_some() || self.panes.is_empty() {
            return Vec::new();
        }
        let mut dividers = Vec::new();
        self.collect_dividers(
            &self.layout,
            total_x,
            total_y,
            total_w,
            total_h,
            &mut dividers,
        );
        dividers
    }

    fn collect_dividers(
        &self,
        node: &PaneNode,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        out: &mut Vec<(usize, usize, usize, usize)>,
    ) {
        const DIVIDER: usize = 2;
        match node {
            PaneNode::Leaf(_) => {}
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => match direction {
                SplitDirection::Horizontal => {
                    let avail_h = h.saturating_sub(DIVIDER);
                    let h1 = ((avail_h as f32) * ratio).round() as usize;
                    let h1 = h1.min(avail_h);
                    let h2 = avail_h.saturating_sub(h1);
                    out.push((x, y + h1, w, DIVIDER));
                    self.collect_dividers(first, x, y, w, h1, out);
                    self.collect_dividers(second, x, y + h1 + DIVIDER, w, h2, out);
                }
                SplitDirection::Vertical => {
                    let avail_w = w.saturating_sub(DIVIDER);
                    let w1 = ((avail_w as f32) * ratio).round() as usize;
                    let w1 = w1.min(avail_w);
                    let w2 = avail_w.saturating_sub(w1);
                    out.push((x + w1, y, DIVIDER, h));
                    self.collect_dividers(first, x, y, w1, h, out);
                    self.collect_dividers(second, x + w1 + DIVIDER, y, w2, h, out);
                }
            },
        }
    }

    pub fn split_active_pane(
        &mut self,
        new_pane_id: usize,
        new_pane: TerminalPane,
        direction: SplitDirection,
    ) {
        let active_id = self.active_pane_id;
        self.layout =
            Self::split_leaf_in_tree(self.layout.clone(), active_id, new_pane_id, direction);
        self.panes.push(new_pane);
        self.active_pane_id = new_pane_id;
    }

    fn split_leaf_in_tree(
        node: PaneNode,
        target_id: usize,
        new_id: usize,
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

    pub fn close_pane(&mut self, pane_id: usize) {
        self.panes.retain(|p| p.id != pane_id);
        if let Some(new_tree) = Self::remove_leaf_from_tree(self.layout.clone(), pane_id) {
            self.layout = new_tree;
        }
        if self.active_pane_id == pane_id {
            if let Some(p) = self.panes.last() {
                self.active_pane_id = p.id;
            }
        }
    }

    fn remove_leaf_from_tree(node: PaneNode, target_id: usize) -> Option<PaneNode> {
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
