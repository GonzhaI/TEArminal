//! Anfitrión de ventanas: posee todas las `AxonApp` (una por ventana) y enruta hasta ellas los eventos de
//! winit. Es el único que toca el `ActiveEventLoop`: abre y retira ventanas, y fija el próximo despertar.

use super::*;
use crate::config::SessionState;
use winit::event_loop::{ControlFlow, EventLoopProxy};

pub struct AxonHost {
    proxy: Option<EventLoopProxy<AxonEvent>>,
    windows: Vec<AxonApp>,
}

impl AxonHost {
    /// Abre las ventanas de la sesión guardada (con `terminal.restore_session`) o una ventana nueva
    pub fn new(config: AxonConfig, proxy: Option<EventLoopProxy<AxonEvent>>) -> Self {
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
            AxonEvent::SshDone | AxonEvent::FsChanged => {
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
