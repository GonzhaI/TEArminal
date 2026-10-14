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

pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    pub panes: Vec<TerminalPane>,
    pub active_pane_id: usize,
    pub layout: PaneNode,
    pub has_unread: bool,
    pub is_settings: bool,
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
        }
    }

    pub fn display_title(&self) -> String {
        if self.is_settings {
            return "Ajustes".to_string();
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
        self.panes
            .iter()
            .find(|p| p.id == self.active_pane_id)
            .or_else(|| self.panes.first())
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
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
        if self.is_settings || self.panes.is_empty() {
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
        if self.is_settings || self.panes.is_empty() {
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
