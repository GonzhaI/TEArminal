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
        let renderer = renderer::TerminalRenderer::new(&config, 1.0);
        println!("char_width: {}, row_height: {}, baseline: {}", renderer.char_width, renderer.row_height, renderer.baseline_y);
        assert!(renderer.char_width > 0);
        assert!(renderer.row_height > 0);
    }

    #[test]
    fn test_nerd_font_icons_and_color_rendering() {
        let config = AxonConfig::load_or_default();
        let mut renderer = renderer::TerminalRenderer::new(&config, 1.0);

        // 1. Probar que los iconos de Nerd Font existen y se rasterizan en JetBrains Mono Nerd Font
        let nerd_icons = ['\u{e0a0}', '\u{e0b0}', '\u{f179}', '\u{f489}', '\u{f07b}', '\u{e702}'];
        for &icon in &nerd_icons {
            let glyph = renderer.get_glyph(icon, false);
            println!("Nerd Font icon {:?} (U+{:04X}): width={}, height={}", icon, icon as u32, glyph.width, glyph.height);
            assert!(glyph.width > 0 && glyph.height > 0, "Icono U+{:04X} debe tener dimensiones válidas", icon as u32);
            assert!(!glyph.bitmap.is_empty(), "Icono U+{:04X} debe rasterizarse con bitmap no vacío", icon as u32);
        }

        // 2. Probar que una secuencia TrueColor se renderiza con los píxeles de color Ghostty
        let mut terminal = terminal::TerminalState::new(80, 24, 1000, theme::Theme::ghostty_dark());
        // Escribir texto con color verde Ghostty (#51e27a / RGB 81, 226, 122) y un icono Nerd Font
        let colored_seq = b"\x1b[38;2;81;226;122m\xee\x82\xa0 Ghostty\x1b[0m";
        terminal.process_bytes(colored_seq);

        let mut buffer = vec![0u32; 800 * 600];
        renderer.render(&mut buffer, 800, 600, &terminal, &terminal.theme, false);

        // Comprobar que en el buffer hay píxeles coloreados con verde (canal G alto, canal R y B bajos)
        let mut has_green_pixels = false;
        let mut background_matches = true;
        let expected_bg = theme::Color::from_rgb(22, 22, 24).to_u32_xrgb();

        for (i, &pixel) in buffer.iter().enumerate() {
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;

            if g > 150 && r < 100 && b < 150 {
                has_green_pixels = true;
            }
            // Los primeros píxeles de margen deben ser el fondo Ghostty #161618
            if i < 800 * 5 && pixel != expected_bg {
                background_matches = false;
            }
        }

        assert!(background_matches, "El fondo del canvas debe coincidir exactamente con el fondo Ghostty (#161618)");
        assert!(has_green_pixels, "El buffer debe contener píxeles con el color verde Ghostty renderizado");
    }
}

