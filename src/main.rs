#![windows_subsystem = "windows"]

mod app;
mod config;
pub mod explorer;
mod json;
pub mod logger;
mod projects;
mod pty;
mod renderer;
mod ssh;
mod terminal;
mod theme;
pub mod tools;
pub mod ui;
pub mod updater;

use app::{AxonEvent, AxonHost};
use config::AxonConfig;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Cargar configuración de Axon
    let config = AxonConfig::load_or_default();

    // 2. Crear EventLoop nativo de Windows con soporte para eventos asíncronos de ConPTY
    let event_loop = EventLoop::<AxonEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    // 3. Comprobación de actualizaciones en segundo plano si está activado
    if config.terminal.auto_check_updates {
        updater::check_for_updates_background(Some(proxy.clone()));
    }

    // 4. Instanciar el anfitrión de ventanas de Axon
    // Antes de crearlo: la primera pestaña lanza el shell en `new` y debe heredar la variable
    std::env::set_var(
        "AXON_SSH_INTEGRATION",
        if config.terminal.ssh_integration {
            "1"
        } else {
            "0"
        },
    );
    let mut host = AxonHost::new(config, Some(proxy));

    // 5. Iniciar ciclo de eventos del sistema operativo
    event_loop.run_app(&mut host)?;

    Ok(())
}
