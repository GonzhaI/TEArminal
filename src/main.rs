#![windows_subsystem = "windows"]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app;
mod config;
mod explorer;
mod json;
mod logger;
mod pty;
mod renderer;
mod ssh;
mod terminal;
mod theme;
mod tools;
mod ui;
mod updater;

use anyhow::{Context, Result};
use app::{TearminalEvent, TearminalHost};
use config::TearminalConfig;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
    // 1. Cargar configuración de TEArminal
    let config = TearminalConfig::load_or_default();

    // Limpieza única en arranque de directorios temporales obsoletos
    for name in &["tearminal_clipboard", "axon_clipboard"] {
        let legacy = std::env::temp_dir().join(name);
        if legacy.exists() {
            let _ = std::fs::remove_dir_all(&legacy);
        }
    }

    // 2. Crear EventLoop nativo de Windows con soporte para eventos asíncronos de ConPTY
    let event_loop = EventLoop::<TearminalEvent>::with_user_event()
        .build()
        .context("Fallo al inicializar el bucle de eventos nativo de Windows (EventLoop)")?;
    let proxy = event_loop.create_proxy();

    // 3. Comprobación de actualizaciones en segundo plano si está activado
    if config.terminal.auto_check_updates {
        updater::check_for_updates_background(Some(proxy.clone()));
    }

    // 4. Instanciar el anfitrión de ventanas de TEArminal
    // Antes de crearlo: la primera pestaña lanza el shell en `new` y debe heredar la variable
    std::env::set_var(
        "TEARMINAL_SSH_INTEGRATION",
        if config.terminal.ssh_integration {
            "1"
        } else {
            "0"
        },
    );
    let mut host = TearminalHost::new(config, Some(proxy));

    // 5. Iniciar ciclo de eventos del sistema operativo
    event_loop
        .run_app(&mut host)
        .context("Fallo durante la ejecución del bucle de eventos de la aplicación")?;

    Ok(())
}
