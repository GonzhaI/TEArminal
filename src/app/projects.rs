//! Proyectos del panel lateral: a qué proyecto pertenece cada sesión, cuál está a la vista y cómo se
//! agregan o se quitan. Los proyectos los elige el usuario; una sesión pertenece al de raíz más
//! profunda que contiene su carpeta y, si ninguno la contiene, va entre las sueltas.

use super::*;
use crate::renderer::{
    AiAgentStatus, SessionStatus, SidebarHeader, SidebarProject, SidebarRects, TopTabBarRects,
};
use crate::ui::LucideIcon;
use std::path::{Path, PathBuf};

/// Dónde se lista una sesión: en un proyecto agregado por el usuario o entre las sueltas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Membership {
    Project(usize),
    Loose,
}

/// Nombre con el que se lista una carpeta como proyecto (su último componente).
fn project_folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
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
        Some(
            Self::tab_dir(tab)
                .and_then(|d| crate::projects::project_for(&self.config.projects, &d))
                .map_or(Membership::Loose, Membership::Project),
        )
    }

    /// Carpeta local de la sesión (la del visor de archivos o la del shell); `None` por SSH.
    fn tab_dir(tab: &TerminalTab) -> Option<PathBuf> {
        if let Some(viewer) = &tab.file_viewer {
            return match viewer.remote_destination {
                Some(_) => None,
                None => viewer.path.parent().map(Path::to_path_buf),
            };
        }
        let terminal = &tab.active_pane()?.terminal;
        if terminal.ssh_session.as_ref().is_some_and(|s| s.is_active) {
            return None;
        }
        terminal.current_dir.clone()
    }

    /// Repositorio donde está la sesión a la vista cuando no pertenece a ningún proyecto: lo que ofrece
    /// la entrada «Agregar "repo"» del desplegable de proyectos (modo de pestañas superiores). No agrega
    /// nada por sí sola.
    pub(super) fn suggested_project(&self) -> Option<PathBuf> {
        if !self.is_horizontal_tabs() || self.selected_membership() != Membership::Loose {
            return None;
        }
        let index = match self.tab_membership(self.active_tab_index) {
            Some(_) => self.active_tab_index,
            None => self.last_terminal_tab_index,
        };
        let root = shell_context::git_root(&Self::tab_dir(self.tabs.get(index)?)?)?;
        root.is_dir().then_some(root)
    }

    /// Clic en la fila `i` del desplegable de proyectos: abre el proyecto o pide confirmar la
    /// sugerencia (`i` = tras los proyectos); cualquier otra abre el selector de carpetas.
    pub(super) fn click_panel_row(&mut self, i: usize) {
        let rail_len = self.panel_rail().len();
        if i < rail_len {
            self.click_rail(i);
        } else if let Some(root) = self.suggested_project().filter(|_| i == rail_len) {
            let name = project_folder_name(&root);
            let message = format!(
                "Se agregará {} a tus proyectos. Las sesiones que abras dentro de esa carpeta se agruparán bajo «{name}».",
                shell_context::shorten_home(&root)
            );
            self.open_dialog(Dialog::confirm(
                "Agregar proyecto",
                message,
                "Agregar",
                DialogAction::AddProject(root),
            ));
        } else {
            self.add_project_with_picker();
        }
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
                match m {
                    Membership::Project(i) => SidebarProject {
                        initial: crate::projects::initial(&self.config.projects[i].name),
                        color_slot: crate::projects::color_slot(i),
                        is_active: m == selected,
                        has_news,
                        is_loose: false,
                    },
                    Membership::Loose => SidebarProject {
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
            Membership::Loose => ("Sesiones".to_string(), Some("Sin proyecto".to_string())),
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
        }
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
                title: "Sesiones".to_string(),
                initial: '·',
                color_slot: 7,
                is_loose: true,
                has_news,
            },
        }
    }

    /// Entradas del desplegable del selector de la barra superior: los proyectos (el activo con una
    /// marca), la sugerencia si hay y «Agregar proyecto».
    pub(super) fn project_dropdown(&self) -> Vec<MenuItem> {
        let selected = self.selected_membership();
        let rail = self.panel_rail();
        let mut items: Vec<MenuItem> = rail
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                let (label, icon) = match m {
                    Membership::Project(p) => {
                        (self.config.projects[p].name.clone(), LucideIcon::Folder)
                    }
                    Membership::Loose => ("Sesiones".to_string(), LucideIcon::Folder),
                };
                let icon = if m == selected {
                    LucideIcon::Check
                } else {
                    icon
                };
                MenuItem::new(label, icon, "", MenuAction::PanelRow(i), true)
            })
            .collect();
        if !items.is_empty() {
            items.push(MenuItem::separator());
        }
        if let Some(root) = self.suggested_project() {
            items.push(MenuItem::new(
                format!("Agregar «{}»", project_folder_name(&root)),
                LucideIcon::Plus,
                "",
                MenuAction::PanelRow(rail.len()),
                true,
            ));
        }
        items.push(MenuItem::new(
            "Agregar proyecto…",
            LucideIcon::Plus,
            "",
            MenuAction::AddProject,
            true,
        ));
        items
    }

    /// Abre el desplegable de proyectos justo debajo del selector de la barra superior.
    pub(super) fn open_project_dropdown(&mut self, win: (usize, usize)) {
        let tabs = self.panel_tab_indices().len();
        let (x, y, _, h) = TopTabBarRects::compute(win.0, tabs).switcher_btn;
        self.show_menu(self.project_dropdown(), (x, y + h + 2), win);
    }

    /// Clic derecho sobre la fila `i` del desplegable: ofrece quitar el proyecto, si lo es.
    pub(super) fn open_project_row_menu(
        &mut self,
        i: usize,
        at: (usize, usize),
        win: (usize, usize),
    ) {
        if let Some(m @ Membership::Project(_)) = self.panel_rail().get(i).copied() {
            self.show_menu(self.project_menu(Some(m)), at, win);
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

    pub(super) fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
