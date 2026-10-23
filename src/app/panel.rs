//! Colocación del panel lateral: lado de la ventana, coordenadas locales y el explorador alojado dentro.
//!
//! El panel mide siempre `SIDEBAR_WIDTH` y su geometría (`SidebarRects`) es local a él. A la izquierda
//! las coordenadas locales y las de ventana coinciden; a la derecha el panel se dibuja en un búfer propio
//! y los manejadores del ratón restan `panel_x`.

use super::*;
use crate::renderer::{
    AiAgentStatus, FilesPanelRects, Rect, SessionStatus, SidebarHeader, SidebarRects,
};
use std::path::{Path, PathBuf};

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
    pub(super) fn panel_on_right(&self) -> bool {
        self.config.appearance.panel_on_right()
    }

    /// ¿Se ve el panel lateral? Con pestañas verticales lista las sesiones; con las superiores, solo el
    /// explorador (las sesiones son las pestañas).
    pub(super) fn panel_shown(&self) -> bool {
        self.sidebar_visible && !self.settings_fullscreen()
    }

    /// Con pestañas superiores Ajustes es una pantalla aparte: ni el panel ni el explorador acoplado
    /// restan espacio mientras está a la vista (`sidebar_visible` y el explorador no cambian).
    pub(super) fn settings_fullscreen(&self) -> bool {
        self.is_horizontal_tabs()
            && self
                .tabs
                .get(self.active_tab_index)
                .is_some_and(|t| t.is_settings)
    }

    /// X de la ventana donde empieza el panel.
    pub(super) fn panel_x(&self, win_w: usize) -> usize {
        if self.panel_on_right() {
            win_w.saturating_sub(SIDEBAR_WIDTH)
        } else {
            0
        }
    }

    /// Coordenada x local al panel si `(mx, my)` cae en su parte propia. El explorador alojado dentro
    /// atiende su propio ratón, así que su rectángulo queda fuera.
    pub(super) fn panel_local_x(&self, mx: usize, my: usize, win_w: usize) -> Option<usize> {
        if !self.panel_shown() {
            return None;
        }
        let in_dock = self
            .file_explorer
            .dock
            .is_some_and(|(x, y, w, h)| mx >= x && mx < x + w && my >= y && my < y + h);
        if in_dock {
            return None;
        }
        let x0 = self.panel_x(win_w);
        (mx >= x0 && mx < x0 + SIDEBAR_WIDTH).then(|| mx - x0)
    }

    /// Ancho que el panel quita a la derecha cuando va a ese lado (Ajustes lo respeta al centrar su contenido).
    pub(super) fn panel_right_inset(&self) -> usize {
        if self.panel_shown() && self.panel_on_right() {
            SIDEBAR_WIDTH
        } else {
            0
        }
    }

    /// Botón flotante que devuelve la barra lateral cuando está oculta, en coordenadas de ventana.
    pub(super) fn floating_toggle_rect(&self, win_w: usize, usable_h: usize) -> Rect {
        let x = if self.panel_on_right() {
            win_w.saturating_sub(38)
        } else {
            8
        };
        (x, usable_h.saturating_sub(38), 30, 30)
    }

    /// Fin (y) de la lista de sesiones del panel vertical: con el explorador visible la lista cede
    /// espacio. Con pestañas superiores el panel no tiene lista.
    pub(super) fn panel_list_end(&self, usable_h: usize) -> usize {
        SidebarRects::list_end(usable_h, self.tabs.len(), self.file_explorer.visible)
    }

    /// Pestaña de la fila del panel vertical bajo `my`.
    pub(super) fn panel_tab_at(&self, my: usize, usable_h: usize) -> Option<usize> {
        let list_end = self.panel_list_end(usable_h);
        SidebarRects::tab_at_y(my, list_end, self.sidebar_scroll_offset, self.tabs.len())
    }

    /// Cabecera de la lista de sesiones: título fijo y, de la sesión a la vista, su rama y su carpeta.
    pub(super) fn sidebar_header(&mut self, usable_h: usize) -> SidebarHeader {
        let active = self.active_tab_index;
        let path = self
            .tabs
            .get(active)
            .and_then(Self::tab_dir)
            .map(|d| shell_context::shorten_home(&d).into_owned());
        SidebarHeader {
            title: "Sesiones".to_string(),
            branch: self.tabs.get_mut(active).and_then(|t| t.get_git_branch()),
            path,
            on_right: self.panel_on_right(),
            list_end: self.panel_list_end(usable_h),
        }
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

    /// Filas de la lista de sesiones y cuántas caben: base del desplazamiento con la rueda. Sin
    /// pestañas verticales no hay lista que desplazar.
    pub(super) fn panel_scroll_extent(&self, usable_h: usize) -> (usize, usize) {
        if self.is_horizontal_tabs() {
            (0, 0)
        } else {
            (
                self.tabs.len(),
                SidebarRects::visible_items(self.panel_list_end(usable_h)),
            )
        }
    }

    /// Coloca el explorador: dentro del panel (bajo la lista en vertical, ocupándolo entero con pestañas
    /// superiores) y acoplado al borde derecho con el panel oculto.
    pub(super) fn sync_explorer_dock(&mut self, win_w: usize, win_h: usize) {
        self.file_explorer.dock = if self.panel_shown() && self.file_explorer.visible {
            let usable_h = win_h.saturating_sub(self.status_bar_height());
            let (x, y, w, h) = if self.is_horizontal_tabs() {
                FilesPanelRects::explorer_rect(usable_h)
            } else {
                SidebarRects::explorer_rect_after(usable_h, self.panel_list_end(usable_h))
            };
            Some((self.panel_x(win_w) + x, y, w, h))
        } else {
            None
        };
    }
}
