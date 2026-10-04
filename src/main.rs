mod config;
mod pty;
mod terminal;
mod theme;
mod ui;

use config::AxonConfig;
use eframe::egui::Vec2;
use ui::AxonApp;

fn main() -> eframe::Result<()> {
    // 1. Cargar configuración de Axon
    let config = AxonConfig::load_or_default();

    // 2. Opciones de ventana nativa de Windows (DirectX / GPU)
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Axon")
            .with_inner_size(Vec2::new(1020.0, 680.0))
            .with_min_inner_size(Vec2::new(500.0, 320.0))
            .with_transparent(false),
        ..Default::default()
    };

    // 3. Ejecutar la aplicación
    eframe::run_native(
        "Axon Terminal",
        native_options,
        Box::new(|_cc| Ok(Box::new(AxonApp::new(config)))),
    )
}
