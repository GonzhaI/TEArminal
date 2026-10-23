//! Anfitrión de ventanas: posee todas las `AxonApp` (una por ventana) y enruta hasta ellas los eventos de
//! winit. Es el único que toca el `ActiveEventLoop`: abre y retira ventanas, y fija el próximo despertar.

use super::*;
use crate::config::SessionState;
use winit::event_loop::{ControlFlow, EventLoopProxy};

pub(crate) struct AxonHost {
    proxy: Option<EventLoopProxy<AxonEvent>>,
    windows: Vec<AxonApp>,
}

impl AxonHost {
    /// Abre las ventanas de la sesión guardada (con `terminal.restore_session`) o una ventana nueva
    pub(crate) fn new(config: AxonConfig, proxy: Option<EventLoopProxy<AxonEvent>>) -> Self {
        let saved = if config.terminal.restore_session {
            SessionState::load().map(|s| s.windows)
        } else {
            None
        };
        let mut windows: Vec<AxonApp> = saved
            .into_iter()
            .flatten()
            .filter(|w| !w.tabs.is_empty())
            .map(|w| AxonApp::from_session(config.clone(), proxy.clone(), Some(w)))
            .collect();
        if windows.is_empty() {
            windows.push(AxonApp::new(config, proxy.clone()));
        }
        // `session.toml` guarda todas las ventanas a la vez: que ninguna falte aunque otra guarde antes
        for w in &windows {
            w.save_session();
        }
        Self { proxy, windows }
    }

    /// Tras cada evento: abre las ventanas pedidas, retira las cerradas y sale si no queda ninguna
    fn settle(&mut self, event_loop: &ActiveEventLoop) {
        let requested: Vec<_> = self
            .windows
            .iter_mut()
            .filter_map(|w| {
                std::mem::take(&mut w.new_window_request)
                    .then(|| (w.config.clone(), w.new_window_session()))
            })
            .collect();
        for (config, session) in requested {
            let mut app = AxonApp::from_session(config, self.proxy.clone(), Some(session));
            app.init_window(event_loop);
            app.save_session();
            self.windows.push(app);
        }

        let requested_tabs: Vec<_> = self
            .windows
            .iter_mut()
            .filter_map(|w| {
                w.new_window_with_tab
                    .take()
                    .map(|tab| (w.config.clone(), tab))
            })
            .collect();
        for (config, tab) in requested_tabs {
            let mut app = AxonApp::from_tab(config, self.proxy.clone(), tab);
            app.init_window(event_loop);
            app.save_session();
            self.windows.push(app);
        }

        // Transferencias explícitas de pestañas solicitadas desde el menú contextual
        let move_requests: Vec<(usize, usize)> = self
            .windows
            .iter_mut()
            .enumerate()
            .filter_map(|(from_w_idx, w)| {
                w.move_tab_to_other_window
                    .take()
                    .map(|tab_idx| (from_w_idx, tab_idx))
            })
            .collect();
        for (from_w_idx, tab_idx) in move_requests {
            if self.windows.len() > 1 && tab_idx < self.windows[from_w_idx].tabs.len() {
                let target_idx = self
                    .windows
                    .iter()
                    .enumerate()
                    .position(|(i, w)| i != from_w_idx && !w.closing);
                if let Some(target_idx) = target_idx {
                    let tab = self.windows[from_w_idx].tabs.remove(tab_idx);
                    if self.windows[from_w_idx].active_tab_index
                        >= self.windows[from_w_idx].tabs.len()
                    {
                        self.windows[from_w_idx].active_tab_index =
                            self.windows[from_w_idx].tabs.len().saturating_sub(1);
                    }
                    if self.windows[from_w_idx].tabs.is_empty() {
                        self.windows[from_w_idx].closing = true;
                    } else {
                        self.windows[from_w_idx].update_window_title();
                        self.windows[from_w_idx].save_session();
                    }
                    self.windows[target_idx].attach_tab(tab);
                }
            }
        }

        // Pestañas soltadas con el mouse para acoplar a otra ventana o crear nueva ventana
        let drop_actions: Vec<(usize, TabDropAction)> = self
            .windows
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, w)| w.tab_drop_action.take().map(|action| (idx, action)))
            .collect();

        for (from_w_idx, drop) in drop_actions {
            let (sx, sy) = drop.screen_pos;
            let target_idx = self.windows.iter().enumerate().position(|(i, w)| {
                if i == from_w_idx || w.closing {
                    return false;
                }
                if let Some(win) = &w.window {
                    if let (Ok(pos), size) = (win.outer_position(), win.outer_size()) {
                        let wx = pos.x;
                        let wy = pos.y;
                        let ww = size.width as i32;
                        let wh = size.height as i32;
                        return sx >= wx && sx < wx + ww && sy >= wy && sy < wy + wh;
                    }
                }
                false
            });

            if let Some(target_idx) = target_idx {
                crate::logger::log(&format!(
                    "Acoplando pestaña '{}' a ventana existente",
                    drop.tab.display_title()
                ));
                self.windows[target_idx].attach_tab(drop.tab);
                if let Some(win) = &self.windows[target_idx].window {
                    win.focus_window();
                    win.request_redraw();
                }
            } else {
                crate::logger::log(&format!(
                    "Desacoplando pestaña '{}' a nueva ventana en ({}, {})",
                    drop.tab.display_title(),
                    sx,
                    sy
                ));
                let config = self
                    .windows
                    .get(from_w_idx)
                    .map(|w| w.config.clone())
                    .unwrap_or_else(|| self.windows[0].config.clone());
                let mut app = AxonApp::from_tab(config, self.proxy.clone(), drop.tab);
                app.init_window(event_loop);
                if let Some(win) = &app.window {
                    win.set_outer_position(winit::dpi::PhysicalPosition::new(sx.max(0), sy.max(0)));
                    win.focus_window();
                }
                app.save_session();
                self.windows.push(app);
            }
        }

        let has_other = self.windows.len() > 1;
        for w in &mut self.windows {
            w.has_other_windows = has_other;
        }

        if self.windows.iter().any(|w| w.closing) {
            // Cerrar una ventana con otras abiertas la quita de la sesión; la última se conserva para restaurarla
            let others_remain = self.windows.iter().any(|w| !w.closing);
            self.windows.retain(|w| {
                if w.closing && others_remain {
                    SessionState::forget_window(w.session_id);
                }
                !w.closing
            });
            if self.windows.is_empty() {
                event_loop.exit();
            }
        }
    }
}

impl ApplicationHandler<AxonEvent> for AxonHost {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AxonEvent) {
        match event {
            AxonEvent::SshDone | AxonEvent::FsChanged | AxonEvent::UpdateCheckDone => {
                for w in &mut self.windows {
                    w.on_user_event(event);
                }
            }
            AxonEvent::PtyData(pane_id) | AxonEvent::PtyClosed(pane_id) => {
                if let Some(w) = self.windows.iter_mut().find(|w| w.owns_pane(pane_id)) {
                    w.on_user_event(event);
                }
            }
        }
        self.settle(event_loop);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        for w in &mut self.windows {
            w.init_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(w) = self
            .windows
            .iter_mut()
            .find(|w| w.window_id() == Some(window_id))
        {
            w.on_window_event(event);
        }
        self.settle(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Próximo despertar = el más cercano de todas las ventanas; sin temporizadores, 0 % CPU en reposo
        let wake = self.windows.iter_mut().filter_map(AxonApp::tick).min();
        self.settle(event_loop);
        event_loop.set_control_flow(match wake {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}
