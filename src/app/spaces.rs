//! Spaces: grupos de pestañas que lista el panel izquierdo. El Space activo es el de la pestaña activa
//! (no se guarda aparte), la barra superior solo muestra las pestañas de ese Space y un Space sin
//! pestañas desaparece.

use super::*;
use crate::renderer::{SpaceItem, SpacesHover, SpacesPanelRects, SpacesSlot};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Space {
    pub(crate) id: SpaceId,
    pub(crate) name: String,
    /// Creado con «Añadir máquina»: cuenta como remoto aunque `ssh` aún no haya conectado. Un Space cuya
    /// terminal tiene una sesión SSH activa también es remoto, haya nacido como haya nacido.
    pub(crate) remote: bool,
}

/// Nombre de un Space nuevo: la carpeta de su primera terminal o, sin ella, «Space N».
pub(crate) fn space_name(cwd: Option<&Path>, n: usize) -> String {
    cwd.and_then(Path::file_name)
        .map(|f| f.to_string_lossy().into_owned())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| format!("Space {n}"))
}

/// Carpeta de la primera terminal de un reparto guardado (nombra el Space de una sesión antigua).
pub(crate) fn first_leaf_cwd(node: &crate::config::PaneSession) -> Option<PathBuf> {
    match node {
        crate::config::PaneSession::Leaf { cwd } => cwd.as_deref().map(PathBuf::from),
        crate::config::PaneSession::Split { first, .. } => first_leaf_cwd(first),
    }
}

impl TearminalApp {
    /// Ancho que el panel de Spaces quita al contenido por la izquierda.
    pub(crate) fn spaces_w(&self) -> usize {
        SpacesPanelRects::width_for(self.spaces_collapsed)
    }

    pub(super) fn spaces_rects(&self, win_h: usize) -> SpacesPanelRects {
        SpacesPanelRects::compute(self.spaces_collapsed, win_h)
    }

    /// ¿Es remoto el Space `space`?
    pub(crate) fn space_is_remote(&self, space: &Space) -> bool {
        space.remote
            || self.tabs.iter().any(|t| {
                t.space == space.id
                    && t.active_pane().is_some_and(|p| {
                        p.terminal.ssh_session.as_ref().is_some_and(|s| s.is_active)
                    })
            })
    }

    /// Posiciones (en `spaces`) en el orden del panel: primero los de este equipo y luego los remotos,
    /// cada grupo en el orden en que se crearon.
    pub(crate) fn space_order(&self) -> Vec<usize> {
        let (mut local, remote): (Vec<usize>, Vec<usize>) =
            (0..self.spaces.len()).partition(|&i| !self.space_is_remote(&self.spaces[i]));
        local.extend(remote);
        local
    }

    /// Filas del panel en su orden (locales y luego remotos), con el Space activo marcado.
    pub(super) fn space_items(&self) -> Vec<SpaceItem> {
        let active = self.active_space();
        self.space_order()
            .into_iter()
            .map(|i| {
                let s = &self.spaces[i];
                SpaceItem {
                    name: s.name.clone(),
                    is_active: s.id == active,
                    tab_count: self.tabs.iter().filter(|t| t.space == s.id).count(),
                    index: i,
                    remote: self.space_is_remote(s),
                }
            })
            .collect()
    }

    /// Qué Spaces de la lista (en el orden del panel) son remotos.
    pub(super) fn space_remote_flags(&self) -> Vec<bool> {
        self.space_order()
            .into_iter()
            .map(|i| self.space_is_remote(&self.spaces[i]))
            .collect()
    }

    /// Elemento del panel bajo `(mx, my)`.
    pub(super) fn spaces_hit(&self, mx: usize, my: usize, win_h: usize) -> SpacesHover {
        let r = self.spaces_rects(win_h);
        if !r.contains(mx, my, win_h) {
            return SpacesHover::None;
        }
        let inside = |rect| crate::renderer::SidebarRects::inside(mx, my, rect);
        if inside(r.toggle_btn) {
            return SpacesHover::Toggle;
        } else if inside(r.search_btn) {
            return SpacesHover::Search;
        } else if inside(r.plus_btn) {
            return SpacesHover::Plus;
        } else if inside(r.settings_btn) {
            return SpacesHover::Settings;
        } else if inside(r.machine_btn) {
            return SpacesHover::Machine;
        }
        if my < r.list_top || my >= r.list_bottom {
            return SpacesHover::None;
        }
        let order = self.space_order();
        for (slot, rect) in r.visible_slots(&self.space_remote_flags(), self.spaces_scroll) {
            match slot {
                SpacesSlot::Header { remote } if !r.collapsed => {
                    if inside(SpacesPanelRects::header_plus(rect)) {
                        return if remote {
                            SpacesHover::MachinePlus
                        } else {
                            SpacesHover::Plus
                        };
                    }
                }
                SpacesSlot::Space(pos) if inside(rect) => {
                    return order
                        .get(pos)
                        .map_or(SpacesHover::None, |&i| SpacesHover::Row(i));
                }
                _ => {}
            }
        }
        SpacesHover::None
    }

    pub(crate) fn toggle_spaces_panel(&mut self) {
        self.spaces_collapsed = !self.spaces_collapsed;
        self.spaces_scroll = 0;
        self.recompute_grid_for_all_tabs();
        self.save_session();
        self.request_redraw();
    }

    /// Clic central en una fila del panel: cierra ese Space.
    pub(super) fn middle_click_spaces_panel(&mut self, mx: usize, my: usize, win_h: usize) {
        if let SpacesHover::Row(i) = self.spaces_hit(mx, my, win_h) {
            if let Some(id) = self.spaces.get(i).map(|s| s.id) {
                self.close_space(id);
            }
        }
    }

    /// Clic izquierdo en el panel de Spaces. Devuelve `true` si el clic cayó dentro del panel.
    pub(super) fn click_spaces_panel(&mut self, mx: usize, my: usize, win_h: usize) -> bool {
        if !self.spaces_rects(win_h).contains(mx, my, win_h) {
            return false;
        }
        match self.spaces_hit(mx, my, win_h) {
            SpacesHover::Toggle => self.toggle_spaces_panel(),
            SpacesHover::Search => self.open_palette(),
            SpacesHover::MachinePlus => self.open_machine_modal(),
            SpacesHover::Plus => {
                if self.modifiers.shift_key() || self.modifiers.alt_key() {
                    let r = self.spaces_rects(win_h);
                    let (win_w, win_h_curr) = self.window_size();
                    let items = self.shell_profiles_menu();
                    self.show_menu(items, (r.plus_btn.0, r.plus_btn.1 + r.plus_btn.3 + 4), (win_w, win_h_curr));
                    return true;
                }
                let cwd = self.active_cwd();
                self.create_space(cwd);
            }
            SpacesHover::Row(i) => {
                if let Some(id) = self.spaces.get(i).map(|s| s.id) {
                    self.select_space(id);
                }
            }
            SpacesHover::Settings => self.open_or_switch_settings_tab(),
            SpacesHover::Machine => self.open_machine_modal(),
            SpacesHover::None => {
                // Cabecera vacía: arrastra la ventana (doble clic = maximizar), como la barra superior.
                if my < SpacesPanelRects::HEADER_H {
                    self.drag_or_maximize_window();
                }
            }
        }
        true
    }

    /// Arrastra la ventana desde una zona vacía del chrome; un segundo clic rápido la maximiza.
    pub(super) fn drag_or_maximize_window(&mut self) {
        let now = Instant::now();
        let Some(w) = &self.window else {
            return;
        };
        if now.duration_since(self.last_titlebar_click) <= platform::double_click_time() {
            w.set_maximized(!w.is_maximized());
        } else {
            self.last_titlebar_click = now;
            let _ = w.drag_window();
        }
    }

    /// Space de la pestaña activa.
    pub(crate) fn active_space(&self) -> SpaceId {
        self.tabs
            .get(self.active_tab_index)
            .map_or(SpaceId(0), |t| t.space)
    }

    /// Posiciones (en `tabs`) de las pestañas de un Space, en orden.
    pub(crate) fn space_tab_indices(&self, space: SpaceId) -> Vec<usize> {
        (0..self.tabs.len())
            .filter(|&i| self.tabs[i].space == space)
            .collect()
    }

    /// Pestañas que muestra la barra superior: las del Space activo.
    pub(crate) fn visible_tab_indices(&self) -> Vec<usize> {
        // La pestaña de Ajustes es un modal, no un chip
        let mut v = self.space_tab_indices(self.active_space());
        v.retain(|&i| !self.tabs[i].is_settings);
        v
    }

    /// Añade una pestaña al final, en el Space activo.
    pub(super) fn push_tab(&mut self, tab: TerminalTab) {
        let space = self.active_space();
        self.push_tab_in(tab, space);
    }

    pub(super) fn push_tab_in(&mut self, mut tab: TerminalTab, space: SpaceId) {
        tab.space = space;
        self.tabs.push(tab);
    }

    /// Space nuevo con una terminal en `cwd`.
    pub(crate) fn create_space(&mut self, cwd: Option<PathBuf>) {
        let shell = self.config.terminal.default_shell.clone();
        self.create_space_with_shell(cwd, &shell);
    }

    /// Space nuevo con un shell específico en `cwd`.
    pub(crate) fn create_space_with_shell(&mut self, cwd: Option<PathBuf>, shell: &str) {
        let id = SpaceId(self.spaces.iter().map(|s| s.id.0 + 1).max().unwrap_or(0));
        let name = space_name(cwd.as_deref(), self.spaces.len() + 1);
        self.spaces.push(Space {
            id,
            name,
            remote: false,
        });
        let before = self.tabs.len();
        self.create_tab_in_space_with_shell(cwd, id, shell);
        if self.tabs.len() == before {
            // El shell no arrancó: no deja un Space vacío.
            self.prune_spaces();
        }
    }

    /// Lleva el foco al Space `space`: su pestaña usada más recientemente, o la primera.
    pub(crate) fn select_space(&mut self, space: SpaceId) {
        let in_space = self.space_tab_indices(space);
        let recent = self
            .tab_history
            .iter()
            .rev()
            .find_map(|&id| in_space.iter().copied().find(|&i| self.tabs[i].id == id));
        if let Some(idx) = recent.or_else(|| in_space.first().copied()) {
            self.switch_to_tab(idx);
        }
    }

    /// Pestaña que pasa a ser la activa al cerrar `index` (ya retirada de `tabs`): la más reciente del
    /// mismo Space; si era la última de su Space, la más reciente de todas.
    pub(super) fn tab_to_activate_after_close(&self, closed_space: SpaceId, index: usize) -> usize {
        let recent = |same_space: bool| {
            self.tab_history.iter().rev().find_map(|&id| {
                self.tabs
                    .iter()
                    .position(|t| t.id == id && (!same_space || t.space == closed_space))
            })
        };
        recent(true)
            .or_else(|| recent(false))
            .unwrap_or_else(|| index.min(self.tabs.len() - 1))
    }

    /// Space siguiente o anterior en el orden del panel (circular).
    pub(crate) fn step_space(&mut self, forward: bool) {
        let order = self.space_order();
        let n = order.len();
        let active = self.active_space();
        let Some(pos) = order.iter().position(|&i| self.spaces[i].id == active) else {
            return;
        };
        if n > 1 {
            let next = if forward {
                (pos + 1) % n
            } else {
                (pos + n - 1) % n
            };
            self.select_space(self.spaces[order[next]].id);
        }
    }

    /// Cierra todas las pestañas de un Space (y con ellas el Space).
    pub(crate) fn close_space(&mut self, space: SpaceId) {
        while let Some(i) = self.tabs.iter().position(|t| t.space == space) {
            let before = self.tabs.len();
            self.close_tab(i);
            if self.tabs.len() >= before {
                break;
            }
        }
    }

    /// Menú contextual de la fila `row` del panel.
    pub(super) fn space_menu(&self, row: usize) -> Option<Vec<MenuItem>> {
        let id = self.spaces.get(row)?.id;
        Some(vec![
            MenuItem::new(
                "Nueva pestaña aquí",
                crate::ui::LucideIcon::Plus,
                "",
                MenuAction::NewTabInSpace(id),
                true,
            ),
            MenuItem::new(
                "Cerrar espacio",
                crate::ui::LucideIcon::X,
                "",
                MenuAction::CloseSpace(id),
                true,
            ),
        ])
    }

    /// Quita los Spaces que se quedaron sin pestañas.
    pub(super) fn prune_spaces(&mut self) {
        let tabs = &self.tabs;
        self.spaces.retain(|s| tabs.iter().any(|t| t.space == s.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_name_uses_folder_or_numbered_fallback() {
        assert_eq!(
            space_name(Some(Path::new("C:\\Users\\a\\TEArminal")), 3),
            "TEArminal"
        );
        assert_eq!(space_name(None, 3), "Space 3");
        assert_eq!(space_name(Some(Path::new("")), 2), "Space 2");
    }
}
