#![windows_subsystem = "windows"]

mod app;
mod config;
mod pty;
mod renderer;
mod terminal;
mod theme;

use app::AxonApp;
use config::AxonConfig;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Cargar configuración de Axon
    let config = AxonConfig::load_or_default();

    // 2. Crear EventLoop nativo de Windows
    let event_loop = EventLoop::new()?;

    // 3. Instanciar aplicación nativa de Axon
    let mut app = AxonApp::new(config);

    // 4. Iniciar ciclo de eventos del sistema operativo
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_renderer_metrics() {
        let config = AxonConfig::load_or_default();
        let renderer = renderer::TerminalRenderer::new(&config);
        println!("char_width: {}, row_height: {}, baseline: {}", renderer.char_width, renderer.row_height, renderer.baseline_y);
        assert!(renderer.char_width > 0);
        assert!(renderer.row_height > 0);
    }
}
