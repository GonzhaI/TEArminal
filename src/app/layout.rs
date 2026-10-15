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
        terminal: TerminalState,
        last_grid_size: (usize, usize),
    ) -> Self {
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
        if self.terminal.current_dir != self.cached_git_dir {
            self.cached_git_branch = get_git_branch(self.terminal.current_dir.as_deref());
            self.cached_git_dir = self.terminal.current_dir.clone();
        }
        self.cached_git_branch.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ViewerSnapshot {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

#[derive(Debug, Clone)]
pub struct FileViewerState {
    pub path: std::path::PathBuf,
    pub file_name: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
    pub scroll_offset_x: usize,
    pub is_binary: bool,
    pub file_size: u64,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub is_dirty: bool,
    pub undo_stack: Vec<ViewerSnapshot>,
    pub redo_stack: Vec<ViewerSnapshot>,
    pub original_lines: Vec<String>,
}

impl FileViewerState {
    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(ViewerSnapshot {
            lines: self.lines.clone(),
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
        });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(ViewerSnapshot {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
            });
            self.lines = prev.lines;
            self.cursor_line = prev.cursor_line.min(self.lines.len().saturating_sub(1));
            self.cursor_col = prev.cursor_col;
            self.is_dirty = self.lines != self.original_lines;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(ViewerSnapshot {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
            });
            self.lines = next.lines;
            self.cursor_line = next.cursor_line.min(self.lines.len().saturating_sub(1));
            self.cursor_col = next.cursor_col;
            self.is_dirty = self.lines != self.original_lines;
        }
    }

    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor_line.saturating_sub(visible_lines.saturating_sub(1));
        }

        // Horizontal scroll ensure visible (columna)
        if self.cursor_col < self.scroll_offset_x {
            self.scroll_offset_x = self.cursor_col;
        } else if self.cursor_col >= self.scroll_offset_x + 80 {
            self.scroll_offset_x = self.cursor_col.saturating_sub(79);
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.is_binary {
            return;
        }
        self.push_undo();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &mut self.lines[self.cursor_line];
        let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
        char_indices.push(line.len());

        let byte_pos = if self.cursor_col < char_indices.len() {
            char_indices[self.cursor_col]
        } else {
            line.len()
        };

        line.insert(byte_pos, ch);
        self.cursor_col += 1;
        self.is_dirty = true;
    }

    pub fn insert_text(&mut self, text: &str) {
        if self.is_binary || text.is_empty() {
            return;
        }
        self.push_undo();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let text_clean = text.replace("\r\n", "\n").replace('\r', "\n");
        let parts: Vec<&str> = text_clean.split('\n').collect();

        if parts.len() == 1 {
            let line = &mut self.lines[self.cursor_line];
            let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
            char_indices.push(line.len());

            let byte_pos = if self.cursor_col < char_indices.len() {
                char_indices[self.cursor_col]
            } else {
                line.len()
            };

            line.insert_str(byte_pos, parts[0]);
            self.cursor_col += parts[0].chars().count();
        } else {
            let line = &self.lines[self.cursor_line];
            let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
            char_indices.push(line.len());

            let byte_pos = if self.cursor_col < char_indices.len() {
                char_indices[self.cursor_col]
            } else {
                line.len()
            };

            let prefix = self.lines[self.cursor_line][..byte_pos].to_string();
            let suffix = self.lines[self.cursor_line][byte_pos..].to_string();

            self.lines[self.cursor_line] = format!("{}{}", prefix, parts[0]);

            for (idx, &part) in parts.iter().enumerate().skip(1) {
                if idx == parts.len() - 1 {
                    self.lines.insert(self.cursor_line + idx, format!("{}{}", part, suffix));
                    self.cursor_col = part.chars().count();
                } else {
                    self.lines.insert(self.cursor_line + idx, part.to_string());
                }
            }
            self.cursor_line += parts.len() - 1;
        }
        self.is_dirty = true;
    }

    pub fn insert_newline(&mut self) {
        if self.is_binary {
            return;
        }
        self.push_undo();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &self.lines[self.cursor_line];
        let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
        char_indices.push(line.len());

        let byte_pos = if self.cursor_col < char_indices.len() {
            char_indices[self.cursor_col]
        } else {
            line.len()
        };

        let remainder = self.lines[self.cursor_line][byte_pos..].to_string();
        self.lines[self.cursor_line].truncate(byte_pos);

        self.cursor_line += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_line, remainder);
        self.is_dirty = true;
    }

    pub fn insert_tab(&mut self) {
        self.insert_text("    ");
    }

    pub fn backspace(&mut self) {
        if self.is_binary {
            return;
        }
        if self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
            char_indices.push(line.len());

            if self.cursor_col < char_indices.len() {
                let start_byte = char_indices[self.cursor_col - 1];
                let end_byte = char_indices[self.cursor_col];
                line.drain(start_byte..end_byte);
                self.cursor_col -= 1;
                self.is_dirty = true;
            }
        } else if self.cursor_line > 0 {
            // Unir la línea actual con la anterior
            let cur_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.lines[self.cursor_line].chars().count();
            self.lines[self.cursor_line].push_str(&cur_line);
            self.cursor_col = prev_len;
            self.is_dirty = true;
        }
    }

    pub fn delete(&mut self) {
        if self.is_binary {
            return;
        }
        if self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }

        let line = &self.lines[self.cursor_line];
        let char_count = line.chars().count();

        if self.cursor_col < char_count {
            let line = &mut self.lines[self.cursor_line];
            let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
            char_indices.push(line.len());

            let start_byte = char_indices[self.cursor_col];
            let end_byte = char_indices[self.cursor_col + 1];
            line.drain(start_byte..end_byte);
            self.is_dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            // Unir la línea siguiente a la actual
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.is_dirty = true;
        }
    }

    /// Borrado de palabra hacia la izquierda (Ctrl + Backspace)
    pub fn delete_word_left(&mut self) {
        if self.is_binary || self.lines.is_empty() {
            return;
        }
        if self.cursor_col == 0 {
            self.backspace();
            return;
        }
        self.push_undo();
        let target_col = self.find_word_boundary_left();
        let line = &mut self.lines[self.cursor_line];
        let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
        char_indices.push(line.len());

        let start_byte = char_indices[target_col];
        let end_byte = char_indices[self.cursor_col];
        line.drain(start_byte..end_byte);
        self.cursor_col = target_col;
        self.is_dirty = true;
    }

    /// Borrado de palabra hacia la derecha (Ctrl + Delete)
    pub fn delete_word_right(&mut self) {
        if self.is_binary || self.lines.is_empty() {
            return;
        }
        let line_len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col >= line_len {
            self.delete();
            return;
        }
        self.push_undo();
        let target_col = self.find_word_boundary_right();
        let line = &mut self.lines[self.cursor_line];
        let mut char_indices: Vec<usize> = line.char_indices().map(|(idx, _)| idx).collect();
        char_indices.push(line.len());

        let start_byte = char_indices[self.cursor_col];
        let end_byte = char_indices[target_col];
        line.drain(start_byte..end_byte);
        self.is_dirty = true;
    }

    pub fn find_word_boundary_left(&self) -> usize {
        if self.cursor_col == 0 || self.cursor_line >= self.lines.len() {
            return 0;
        }
        let chars: Vec<char> = self.lines[self.cursor_line].chars().collect();
        let mut idx = self.cursor_col;

        // Saltar espacios a la izquierda
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        if idx == 0 {
            return 0;
        }

        // Si es caracter alfanumérico o guion bajo, buscar el inicio de la palabra
        let is_alphanumeric = chars[idx - 1].is_alphanumeric() || chars[idx - 1] == '_';
        if is_alphanumeric {
            while idx > 0 && (chars[idx - 1].is_alphanumeric() || chars[idx - 1] == '_') {
                idx -= 1;
            }
        } else {
            // Símbolos puntuación
            while idx > 0 && !chars[idx - 1].is_alphanumeric() && !chars[idx - 1].is_whitespace() {
                idx -= 1;
            }
        }
        idx
    }

    pub fn find_word_boundary_right(&self) -> usize {
        if self.cursor_line >= self.lines.len() {
            return 0;
        }
        let chars: Vec<char> = self.lines[self.cursor_line].chars().collect();
        let len = chars.len();
        if self.cursor_col >= len {
            return len;
        }
        let mut idx = self.cursor_col;

        let is_alphanumeric = chars[idx].is_alphanumeric() || chars[idx] == '_';
        if is_alphanumeric {
            while idx < len && (chars[idx].is_alphanumeric() || chars[idx] == '_') {
                idx += 1;
            }
        } else if !chars[idx].is_whitespace() {
            while idx < len && !chars[idx].is_alphanumeric() && !chars[idx].is_whitespace() {
                idx += 1;
            }
        }
        // Saltar espacios posteriores
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }
        idx
    }

    pub fn get_line_git_diff_status(&self, line_idx: usize) -> Option<GitLineStatus> {
        if line_idx >= self.lines.len() {
            return None;
        }
        if self.original_lines.is_empty() {
            // Todo el archivo es nuevo / no trackeado
            return Some(GitLineStatus::Added);
        }
        if line_idx >= self.original_lines.len() {
            return Some(GitLineStatus::Added);
        }
        if self.lines[line_idx] != self.original_lines[line_idx] {
            return Some(GitLineStatus::Modified);
        }
        None
    }

    pub fn save_to_disk(&mut self) -> Result<(), std::io::Error> {
        if self.is_binary {
            return Ok(());
        }
        let content = self.lines.join("\r\n");
        std::fs::write(&self.path, content)?;
        self.is_dirty = false;
        self.file_size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        self.original_lines = self.lines.clone();
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitLineStatus {
    Added,
    Modified,
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
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            lines.push(line.to_string());
                        }
                        if lines.is_empty() {
                            lines.push(String::new());
                        }
                    }
                }
                Err(e) => {
                    lines.push(format!("Error al leer el archivo: {}", e));
                }
            }
        }

        let original_lines = lines.clone();
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
