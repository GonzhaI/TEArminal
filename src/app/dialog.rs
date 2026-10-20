//! Diálogo modal reutilizable: un título, un mensaje y los botones Cancelar/Aceptar. Mientras está
//! abierto captura el ratón y el teclado (Intro acepta, Esc cancela); lo que acepta lo decide `DialogAction`.

use super::*;
use crate::renderer::{DialogButton, DialogRects};
use std::path::PathBuf;

/// Qué hace el diálogo al aceptarse. Una acción nueva = variante + brazo en `run_dialog_action`.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogAction {
    /// Agrega la carpeta como proyecto del panel.
    AddProject(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub accept: String,
    pub cancel: String,
    pub action: DialogAction,
    pub hovered: Option<DialogButton>,
}

impl Dialog {
    pub fn confirm(title: &str, message: String, accept: &str, action: DialogAction) -> Self {
        Self {
            title: title.to_string(),
            message,
            accept: accept.to_string(),
            cancel: "Cancelar".to_string(),
            action,
            hovered: None,
        }
    }
}

impl AxonApp {
    pub(super) fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.context_menu = None;
        self.request_redraw();
    }

    fn close_dialog(&mut self) -> Option<Dialog> {
        let dialog = self.dialog.take();
        self.request_redraw();
        dialog
    }

    fn accept_dialog(&mut self) {
        if let Some(dialog) = self.close_dialog() {
            match dialog.action {
                DialogAction::AddProject(path) => self.add_project(&path),
            }
        }
    }

    /// Clic con un diálogo abierto: actúa sobre sus botones, un clic fuera lo cancela y el resto del
    /// panel no hace nada. Devuelve `false` si no hay diálogo (el evento sigue su curso).
    pub(super) fn dialog_click(
        &mut self,
        mx: usize,
        my: usize,
        win: (usize, usize),
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        if self.dialog.is_none() {
            return false;
        }
        if state == ElementState::Pressed && button == MouseButton::Left {
            let rects = DialogRects::compute(win.0, win.1);
            match rects.button_at(mx, my) {
                Some(DialogButton::Accept) => self.accept_dialog(),
                Some(DialogButton::Cancel) => {
                    self.close_dialog();
                }
                None if !rects.contains(mx, my) => {
                    self.close_dialog();
                }
                None => {}
            }
        }
        true
    }

    /// Resalta el botón bajo el ratón. Devuelve `false` si no hay diálogo.
    pub(super) fn dialog_hover(&mut self, mx: usize, my: usize, win: (usize, usize)) -> bool {
        let Some(dialog) = &mut self.dialog else {
            return false;
        };
        let hovered = DialogRects::compute(win.0, win.1).button_at(mx, my);
        if dialog.hovered != hovered {
            dialog.hovered = hovered;
            self.request_redraw();
        }
        if let Some(w) = &self.window {
            w.set_cursor(if hovered.is_some() {
                winit::window::CursorIcon::Pointer
            } else {
                winit::window::CursorIcon::Default
            });
        }
        true
    }

    /// Tecla con un diálogo abierto: Intro acepta, Esc cancela y las demás se descartan.
    /// Devuelve `false` si no hay diálogo.
    pub(super) fn dialog_key(&mut self, key: &Key, event_type: KeyEventType) -> bool {
        if self.dialog.is_none() {
            return false;
        }
        if event_type != KeyEventType::Release {
            match key {
                Key::Named(NamedKey::Enter) => self.accept_dialog(),
                Key::Named(NamedKey::Escape) => {
                    self.close_dialog();
                }
                _ => {}
            }
        }
        true
    }
}
