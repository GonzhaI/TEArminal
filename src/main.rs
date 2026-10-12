#![windows_subsystem = "windows"]

pub mod logger;
mod app;
mod config;
mod pty;
mod renderer;
mod terminal;
mod theme;

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
        renderer.render(&mut buffer, 800, 600, 0, &terminal, &terminal.theme, false);

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

    #[test]
    fn test_clipboard_image_saving_and_paste_formatting() {
        let config = AxonConfig::load_or_default();
        let app = AxonApp::new(config, None);

        // Crear una imagen RGBA de prueba (4x4 pixels en verde Ghostty)
        let width = 4;
        let height = 4;
        let mut bytes = Vec::with_capacity(width * height * 4);
        for _ in 0..(width * height) {
            bytes.extend_from_slice(&[0x51, 0xe2, 0x7a, 0xff]);
        }

        let img_data = arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Owned(bytes),
        };

        let path_str = app.save_clipboard_image(&img_data);
        assert!(path_str.is_some(), "Debe guardar la imagen exitosamente y devolver su ruta");
        let path = std::path::PathBuf::from(path_str.unwrap());
        assert!(path.exists(), "El archivo PNG generado debe existir en disco");

        // Formato para chat de IA: ruta entrecomillada con espacio al final
        let formatted = format!("\"{}\" ", path.to_string_lossy());
        assert!(formatted.starts_with('"') && formatted.ends_with("\" "));

        // Verificar que el archivo generado es un PNG válido decodificable
        let loaded = image::open(&path).expect("El PNG guardado debe ser decodificable por el crate image");
        assert_eq!(loaded.width(), 4);
        assert_eq!(loaded.height(), 4);

        // Limpiar archivo de prueba
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_sidebar_rendering_and_layout() {
        use crate::renderer::SidebarTabItem;

        let config = AxonConfig::load_or_default();
        let mut renderer = renderer::TerminalRenderer::new(&config, 1.0);

        // 1. Validar cómputo de columnas con y sin barra lateral
        let (cols_no_sidebar, rows_no_sidebar) = renderer.compute_grid_size(1000, 600, 0);
        let (cols_with_sidebar, rows_with_sidebar) = renderer.compute_grid_size(1000, 600, 190);

        assert_eq!(rows_no_sidebar, rows_with_sidebar, "La altura de filas debe ser idéntica");
        assert!(cols_no_sidebar > cols_with_sidebar, "Sin barra lateral debe caber más columnas");

        // 2. Probar renderizado de la barra lateral en el buffer
        let mut buffer = vec![0u32; 1000 * 600];
        let tabs = vec![
            SidebarTabItem {
                id: 1,
                title: "Terminal 1".to_string(),
                is_active: true,
                has_unread: false,
            },
            SidebarTabItem {
                id: 2,
                title: "Terminal 2".to_string(),
                is_active: false,
                has_unread: true,
            },
        ];

        renderer.render_sidebar(
            &mut buffer,
            1000,
            600,
            190,
            &tabs,
            Some(1),
            None,
            false,
        );

        // Validar que la región de la barra lateral (0..190) fue pintada con el fondo oscuro del sidebar
        let sidebar_bg_pixel = theme::Color::from_rgb(18, 18, 20).to_u32_xrgb();
        let border_pixel = theme::Color::from_rgb(38, 38, 43).to_u32_xrgb();

        assert_eq!(buffer[50 * 1000 + 2], sidebar_bg_pixel, "El fondo del sidebar debe ser #121214");
        assert_eq!(buffer[50 * 1000 + 189], border_pixel, "El borde del sidebar debe ser #26262b");

        // 3. Renderizar el área de terminal con offset
        let term = terminal::TerminalState::new(cols_with_sidebar, rows_with_sidebar, 100, theme::Theme::ghostty_dark());
        renderer.render(&mut buffer, 1000, 600, 190, &term, &term.theme, false);

        let term_bg_pixel = term.theme.background.to_u32_xrgb();
        assert_eq!(buffer[50 * 1000 + 195], term_bg_pixel, "El área de terminal después del offset debe tener fondo Ghostty");
    }

    #[test]
    fn test_app_tab_switching_and_toggle() {
        let config = AxonConfig::load_or_default();
        let mut app = AxonApp::new(config, None);

        assert_eq!(app.tabs.len(), 1, "Debe inicializar con 1 pestaña por defecto");
        assert_eq!(app.active_tab_index, 0);
        assert!(app.sidebar_visible, "La barra lateral debe estar visible inicialmente");
        assert_eq!(app.sidebar_offset(), 190);

        // Conmutar visibilidad de la barra lateral
        app.toggle_sidebar();
        assert!(!app.sidebar_visible);
        assert_eq!(app.sidebar_offset(), 0);

        app.toggle_sidebar();
        assert!(app.sidebar_visible);
        assert_eq!(app.sidebar_offset(), 190);
    }
}

