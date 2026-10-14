#![windows_subsystem = "windows"]

mod app;
mod config;
pub mod explorer;
pub mod logger;
mod pty;
mod renderer;
mod terminal;
mod theme;
pub mod tools;
pub mod ui;

use app::{AxonApp, AxonEvent};
use config::AxonConfig;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Cargar configuración de Axon
    let config = AxonConfig::load_or_default();

    // 2. Crear EventLoop nativo de Windows con soporte para eventos asíncronos de ConPTY
    let event_loop = EventLoop::<AxonEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    // 3. Instanciar aplicación nativa de Axon
    let mut app = AxonApp::new(config, Some(proxy));

    // 4. Iniciar ciclo de eventos del sistema operativo
    event_loop.run_app(&mut app)?;

    Ok(())
}
