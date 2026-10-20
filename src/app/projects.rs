//! Proyectos del panel lateral: a qué proyecto pertenece cada sesión, cuál está a la vista y cómo se
//! agregan o se quitan. Los proyectos los elige el usuario; una sesión pertenece al de raíz más
//! profunda que contiene su carpeta y, si ninguno la contiene, va entre las sueltas.

use super::*;
use crate::renderer::{
    AiAgentStatus, ProjectsPanelRects, SessionStatus, SidebarChange, SidebarHeader, SidebarProject,
    SidebarRects,
};
use crate::ui::LucideIcon;
use std::path::{Path, PathBuf};

/// Dónde se lista una sesión: en un proyecto agregado por el usuario o entre las sueltas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Membership {
    Project(usize),
    Loose,
}

/// Estado de la sesión según OSC 133. `None` en Ajustes y visores de archivo.
pub(super) fn session_status(tab: &TerminalTab) -> Option<SessionStatus> {
    let terminal = &tab.active_pane()?.terminal;
    if terminal
        .active_block
        .as_ref()
        .is_some_and(|b| b.output_start.is_some())
    {
        return Some(SessionStatus::Running);
    }
    Some(
        match terminal
            .command_blocks
            .iter()
            .rev()
            .find_map(|b| b.exit_code)
        {
            Some(0) => SessionStatus::Done,
            Some(_) => SessionStatus::Failed,
            None => SessionStatus::Idle,
        },
    )
}

impl AxonApp {
    /// Proyecto de la pestaña `index`. `None` en Ajustes, que acompaña a la lista que está a la vista.
    fn tab_membership(&self, index: usize) -> Option<Membership> {
        let tab = self.tabs.get(index)?;
        if tab.is_settings {
            return None;
        }
        let dir: Option<PathBuf> = if let Some(viewer) = &tab.file_viewer {
            if viewer.remote_destination.is_some() {
                None
            } else {
                viewer.path.parent().map(Path::to_path_buf)
            }
        } else {
            let terminal = &tab.active_pane()?.terminal;
            let remote = terminal.ssh_session.as_ref().is_some_and(|s| s.is_active);
            if remote {
                None
            } else {
                terminal.current_dir.clone()
            }
        };
        Some(
            dir.and_then(|d| crate::projects::project_for(&self.config.projects, &d))
                .map_or(Membership::Loose, Membership::Project),
        )
    }

    /// Lista que muestra el panel: la de la pestaña activa o, en Ajustes, la de la última sesión vista.
    pub(super) fn selected_membership(&self) -> Membership {
        self.tab_membership(self.active_tab_index)
            .or_else(|| self.tab_membership(self.last_terminal_tab_index))
            .unwrap_or(Membership::Loose)
    }

    /// Entradas del riel: los proyectos del usuario y, si hay, las sesiones sueltas.
    pub(super) fn panel_rail(&self) -> Vec<Membership> {
        let mut rail: Vec<Membership> = (0..self.config.projects.len())
            .map(Membership::Project)
            .collect();
        let has_loose = self.selected_membership() == Membership::Loose
            || (0..self.tabs.len()).any(|i| self.tab_membership(i) == Some(Membership::Loose));
        if has_loose {
            rail.push(Membership::Loose);
        }
        rail
    }

    /// Pestañas del proyecto a la vista, en el orden de `tabs`. Ajustes aparece siempre.
    pub(super) fn panel_tab_indices(&self) -> Vec<usize> {
        let selected = self.selected_membership();
        (0..self.tabs.len())
            .filter(|&i| self.tab_membership(i).is_none_or(|m| m == selected))
            .collect()
    }

    /// Fila bajo `my` en la lista del panel: `(posición en la lista, índice real de la pestaña)`.
    pub(super) fn panel_tab_at(&self, my: usize, usable_h: usize) -> Option<(usize, usize)> {
        let list = self.panel_tab_indices();
        let list_end = self.panel_list_end(usable_h);
        let pos = SidebarRects::tab_at_y(my, list_end, self.sidebar_scroll_offset, list.len())?;
        Some((pos, list[pos]))
    }

    pub(super) fn sidebar_projects(&self) -> Vec<SidebarProject> {
        let selected = self.selected_membership();
        self.panel_rail()
            .into_iter()
            .map(|m| {
                let has_news = m != selected
                    && (0..self.tabs.len())
                        .any(|i| self.tabs[i].has_unread && self.tab_membership(i) == Some(m));
                let status = self.membership_status(m);
                match m {
                    Membership::Project(i) => {
                        let name = self.config.projects[i].name.clone();
                        let branch =
                            crate::projects::read_branch(Path::new(&self.config.projects[i].path));
                        SidebarProject {
                            initial: crate::projects::initial(&name),
                            name,
                            branch,
                            status,
                            color_slot: crate::projects::color_slot(i),
                            is_active: m == selected,
                            has_news,
                            is_loose: false,
                        }
                    }
                    Membership::Loose => SidebarProject {
                        name: "Sueltas".to_string(),
                        branch: None,
                        status,
                        initial: '·',
                        color_slot: 7,
                        is_active: m == selected,
                        has_news,
                        is_loose: true,
                    },
                }
            })
            .collect()
    }

    /// Estado más urgente de las sesiones de `m` (en curso, falló, terminó, en espera).
    fn membership_status(&self, m: Membership) -> Option<SessionStatus> {
        let rank = |s: SessionStatus| match s {
            SessionStatus::Running => 3,
            SessionStatus::Failed => 2,
            SessionStatus::Done => 1,
            SessionStatus::Idle => 0,
        };
        (0..self.tabs.len())
            .filter(|&i| self.tab_membership(i) == Some(m))
            .filter_map(|i| session_status(&self.tabs[i]))
            .max_by_key(|&s| rank(s))
    }

    pub(super) fn sidebar_header(&mut self, usable_h: usize) -> SidebarHeader {
        let selected = self.selected_membership();
        let (title, path) = match selected {
            Membership::Project(i) => {
                let p = &self.config.projects[i];
                (
                    p.name.clone(),
                    Some(shell_context::shorten_home(Path::new(&p.path))),
                )
            }
            Membership::Loose => ("Sueltas".to_string(), Some("Sin proyecto".to_string())),
        };
        let active = self.active_tab_index;
        let branch = if self.tab_membership(active) == Some(selected) {
            self.tabs.get_mut(active).and_then(|t| t.get_git_branch())
        } else {
            None
        };
        SidebarHeader {
            title,
            branch,
            path,
            on_right: self.panel_on_right(),
            list_end: self.panel_list_end(usable_h),
            explorer_visible: self.file_explorer.visible,
            changes: self.panel_changes(),
        }
    }

    /// Archivos con cambios de git para la sección «Cambios» del panel, ordenados por ruta.
    pub(super) fn panel_changes(&self) -> Vec<SidebarChange> {
        let Some(root) = self.file_explorer.git_root.as_deref() else {
            return Vec::new();
        };
        let mut changes: Vec<SidebarChange> = self
            .file_explorer
            .git_statuses
            .iter()
            .filter(|(path, status)| {
                **status != crate::explorer::GitFileStatus::Deleted && path.starts_with(root)
            })
            .map(|(path, status)| SidebarChange {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                path: path.clone(),
                status: *status,
            })
            .collect();
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes
    }

    /// Segunda línea de la fila de sesión: agente, modelo y minutos si corre un agente de IA; si no,
    /// el contexto real de la pestaña (comando, carpeta o SSH).
    pub(super) fn session_detail(&self, tab: &TerminalTab) -> Option<String> {
        if let Some(kind) = super::ai_usage::active_agents(tab).first().copied() {
            let agent = match kind {
                AiAgentStatus::Claude => "claude",
                AiAgentStatus::Agy => "agy",
                AiAgentStatus::Gemini => "gemini",
                AiAgentStatus::Aider => "aider",
                AiAgentStatus::Copilot => "copilot",
                AiAgentStatus::Ollama => "ollama",
                AiAgentStatus::None => "",
            };
            let mut detail = agent.to_string();
            if let Some(model) = self.ai_usage.snapshot.model_for(kind) {
                detail.push_str(", ");
                detail.push_str(model);
            }
            let mins = (tab.started_at.elapsed().as_secs() / 60).max(1);
            detail.push_str(&format!(", {mins} min"));
            return Some(detail);
        }
        tab.sidebar_detail()
    }

    /// Archivo de la sección «Cambios» bajo `(mx, my)` (coordenadas locales al panel).
    pub(super) fn panel_change_at(&self, mx: usize, my: usize, usable_h: usize) -> Option<PathBuf> {
        if !self.file_explorer.visible {
            return None;
        }
        let list_end = self.panel_list_end(usable_h);
        let (x, width) = if self.is_horizontal_tabs() {
            (8, SidebarRects::WIDTH - 16)
        } else {
            (
                SidebarRects::PAD_X,
                SidebarRects::WIDTH - SidebarRects::PAD_X - 8,
            )
        };
        let top = crate::renderer::PanelChangesRects::top(list_end);
        let bottom = SidebarRects::content_bottom(usable_h);
        let changes = self.panel_changes();
        let rows = crate::renderer::PanelChangesRects::shown_rows(bottom, top, changes.len());
        let i = crate::renderer::PanelChangesRects::row_at(mx, my, x, top, width, rows)?;
        changes.get(i).map(|c| c.path.clone())
    }

    /// Carpeta donde abre una sesión nueva: la raíz del proyecto a la vista, si es una carpeta.
    pub(super) fn new_tab_cwd(&self) -> Option<PathBuf> {
        match self.selected_membership() {
            Membership::Project(i) => {
                Some(PathBuf::from(&self.config.projects.get(i)?.path)).filter(|p| p.is_dir())
            }
            Membership::Loose => None,
        }
    }

    /// Proyecto a la vista para el selector de la barra de pestañas superiores.
    pub(super) fn top_bar_project(&self) -> TopBarProject {
        let selected = self.selected_membership();
        let has_news = (0..self.tabs.len())
            .any(|i| self.tabs[i].has_unread && self.tab_membership(i) != Some(selected));
        match selected {
            Membership::Project(i) => {
                let name = self.config.projects[i].name.clone();
                TopBarProject {
                    initial: crate::projects::initial(&name),
                    title: name,
                    color_slot: crate::projects::color_slot(i),
                    is_loose: false,
                    has_news,
                }
            }
            Membership::Loose => TopBarProject {
                title: "Sueltas".to_string(),
                initial: '·',
                color_slot: 7,
                is_loose: true,
                has_news,
            },
        }
    }

    /// Pasa al proyecto siguiente del riel (el selector de la barra superior).
    pub(super) fn cycle_project(&mut self) {
        let rail = self.panel_rail();
        let selected = self.selected_membership();
        if let Some(pos) = rail.iter().position(|&m| m == selected) {
            if rail.len() > 1 {
                self.select_membership(rail[(pos + 1) % rail.len()]);
            }
        }
    }

    /// Clic en el cuadrado `i` del riel: abre el proyecto o, si es el último, lo agrega.
    pub(super) fn click_rail(&mut self, i: usize) {
        let rail = self.panel_rail();
        match rail.get(i).copied() {
            Some(target) => self.select_membership(target),
            None => self.add_project_with_picker(),
        }
    }

    fn select_membership(&mut self, target: Membership) {
        if target == self.selected_membership() {
            return;
        }
        self.sidebar_scroll_offset = 0;
        if let Some(idx) = (0..self.tabs.len()).find(|&t| self.tab_membership(t) == Some(target)) {
            self.switch_to_tab(idx);
        } else if let Membership::Project(i) = target {
            let cwd = PathBuf::from(&self.config.projects[i].path);
            self.create_tab_in(cwd.is_dir().then_some(cwd));
        }
    }

    /// Abre el selector de carpetas de Windows y agrega la elegida como proyecto.
    pub(super) fn add_project_with_picker(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        if let Some(dir) = super::platform::pick_folder(window, "Elige la carpeta del proyecto") {
            if dir.is_dir() {
                self.add_project(&dir);
            }
        }
    }

    pub(super) fn add_project(&mut self, path: &Path) {
        let before = self.config.projects.len();
        let i = crate::projects::add(&mut self.config.projects, path);
        if self.config.projects.len() != before {
            self.save_config();
        }
        self.select_membership(Membership::Project(i));
        self.request_redraw();
    }

    /// Quita el proyecto `i` del panel. Sus sesiones siguen abiertas y pasan a las sueltas.
    pub(super) fn remove_project(&mut self, i: usize) {
        if i < self.config.projects.len() {
            self.config.projects.remove(i);
            self.save_config();
            self.sidebar_scroll_offset = 0;
            self.request_redraw();
        }
    }

    /// Menú del riel: quitar el proyecto bajo el cursor o agregar uno.
    pub(super) fn rail_menu(&self, mx: usize, my: usize) -> Vec<MenuItem> {
        let rail = self.panel_rail();
        let hit = SidebarRects::rail_at(mx, my, rail.len() + 1).and_then(|i| rail.get(i).copied());
        self.project_menu(hit)
    }

    /// Menú de las filas del panel de proyectos (modo de pestañas superiores).
    pub(super) fn row_menu(&self, my: usize, usable_h: usize) -> Vec<MenuItem> {
        let rail = self.panel_rail();
        let hit =
            ProjectsPanelRects::row_at(my, usable_h, rail.len() + 1, self.sidebar_scroll_offset)
                .and_then(|i| rail.get(i).copied());
        self.project_menu(hit)
    }

    fn project_menu(&self, hit: Option<Membership>) -> Vec<MenuItem> {
        vec![match hit {
            Some(Membership::Project(i)) => MenuItem::new(
                "Quitar proyecto",
                LucideIcon::X,
                "",
                MenuAction::RemoveProject(i),
                true,
            ),
            _ => MenuItem::new(
                "Agregar proyecto",
                LucideIcon::Plus,
                "",
                MenuAction::AddProject,
                true,
            ),
        }]
    }

    fn save_config(&self) {
        if let Err(e) = self.config.save() {
            crate::logger::log(&format!("-> No se pudo guardar la configuración: {e}"));
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
