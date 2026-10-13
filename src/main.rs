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
        println!(
            "char_width: {}, row_height: {}, baseline: {}",
            renderer.char_width, renderer.row_height, renderer.baseline_y
        );
        assert!(renderer.char_width > 0);
        assert!(renderer.row_height > 0);
    }

    #[test]
    fn test_nerd_font_icons_and_color_rendering() {
        let config = AxonConfig::load_or_default();
        let mut renderer = renderer::TerminalRenderer::new(&config, 1.0);

        // 1. Probar que los iconos de Nerd Font existen y se rasterizan en JetBrains Mono Nerd Font
        let nerd_icons = [
            '\u{e0a0}', '\u{e0b0}', '\u{f179}', '\u{f489}', '\u{f07b}', '\u{e702}',
        ];
        for &icon in &nerd_icons {
            let glyph = renderer.get_glyph(icon, false);
            println!(
                "Nerd Font icon {:?} (U+{:04X}): width={}, height={}",
                icon, icon as u32, glyph.width, glyph.height
            );
            assert!(
                glyph.width > 0 && glyph.height > 0,
                "Icono U+{:04X} debe tener dimensiones válidas",
                icon as u32
            );
            assert!(
                !glyph.bitmap.is_empty(),
                "Icono U+{:04X} debe rasterizarse con bitmap no vacío",
                icon as u32
            );
        }

        // 2. Probar que una secuencia TrueColor se renderiza con los píxeles de color Ghostty
        let mut terminal = terminal::TerminalState::new(80, 24, 1000, theme::Theme::ghostty_dark());
        // Escribir texto con color verde Ghostty (#51e27a / RGB 81, 226, 122) y un icono Nerd Font
        let colored_seq = b"\x1b[38;2;81;226;122m\xee\x82\xa0 Ghostty\x1b[0m";
        terminal.process_bytes(colored_seq);

        let mut buffer = vec![0u32; 800 * 600];
        renderer.render(
            &mut buffer,
            800,
            600,
            0,
            &terminal,
            &terminal.theme,
            false,
            false,
            false,
        );

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

        assert!(
            background_matches,
            "El fondo del canvas debe coincidir exactamente con el fondo Ghostty (#161618)"
        );
        assert!(
            has_green_pixels,
            "El buffer debe contener píxeles con el color verde Ghostty renderizado"
        );
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
        assert!(
            path_str.is_some(),
            "Debe guardar la imagen exitosamente y devolver su ruta"
        );
        let path = std::path::PathBuf::from(path_str.unwrap());
        assert!(
            path.exists(),
            "El archivo PNG generado debe existir en disco"
        );

        // Formato para chat de IA: ruta entrecomillada con espacio al final
        let formatted = format!("\"{}\" ", path.to_string_lossy());
        assert!(formatted.starts_with('"') && formatted.ends_with("\" "));

        // Verificar que el archivo generado es un PNG válido decodificable
        let loaded =
            image::open(&path).expect("El PNG guardado debe ser decodificable por el crate image");
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

        assert_eq!(
            rows_no_sidebar, rows_with_sidebar,
            "La altura de filas debe ser idéntica"
        );
        assert!(
            cols_no_sidebar > cols_with_sidebar,
            "Sin barra lateral debe caber más columnas"
        );

        // 2. Probar renderizado de la barra lateral en el buffer
        let mut buffer = vec![0u32; 1000 * 600];
        let tabs = vec![
            SidebarTabItem {
                id: 1,
                title: "Terminal 1".to_string(),
                is_active: true,
                has_unread: false,
                pane_count: 1,
                is_settings: false,
            },
            SidebarTabItem {
                id: 2,
                title: "Terminal 2".to_string(),
                is_active: false,
                has_unread: true,
                pane_count: 2,
                is_settings: false,
            },
        ];

        let ghostty_theme = theme::Theme::ghostty_default_styledark();
        renderer.render_sidebar(
            &mut buffer,
            1000,
            600,
            190,
            &tabs,
            Some(1),
            None,
            false,
            false,
            false,
            &ghostty_theme,
        );

        // Validar que la región de la barra lateral (0..190) fue pintada con el fondo oscuro del sidebar
        let sidebar_bg_pixel = theme::Color::from_rgb(18, 18, 20).to_u32_xrgb();
        let border_pixel = theme::Color::from_rgb(38, 38, 43).to_u32_xrgb();

        assert_eq!(
            buffer[50 * 1000 + 2],
            sidebar_bg_pixel,
            "El fondo del sidebar debe ser #121214"
        );
        assert_eq!(
            buffer[50 * 1000 + 189],
            border_pixel,
            "El borde del sidebar debe ser #26262b"
        );

        // 3. Renderizar el área de terminal con offset
        let term = terminal::TerminalState::new(
            cols_with_sidebar,
            rows_with_sidebar,
            100,
            ghostty_theme.clone(),
        );
        renderer.render(
            &mut buffer,
            1000,
            600,
            190,
            &term,
            &term.theme,
            false,
            false,
            false,
        );

        let term_bg_pixel = term.theme.background.to_u32_xrgb();
        assert_eq!(
            buffer[50 * 1000 + 195],
            term_bg_pixel,
            "El área de terminal después del offset debe tener fondo Ghostty"
        );

        // 4. Probar renderizado del botón flotante expand cuando el sidebar está oculto
        renderer.render_floating_sidebar_toggle(
            &mut buffer,
            1000,
            600,
            true,
            false,
            &ghostty_theme,
        );
        let float_btn_bg = ghostty_theme.surface_hover().to_u32_xrgb();
        let float_y = 600 - 30 - 8 + 5;
        assert_eq!(
            buffer[float_y * 1000 + 10],
            float_btn_bg,
            "El botón flotante debe renderizarse correctamente"
        );

        // 5. Probar renderizado de la pestaña de Ajustes
        let settings_state = renderer::SettingsPageState::default();
        renderer.render_settings_page(
            &mut buffer,
            1000,
            600,
            190,
            &config,
            &settings_state,
            &ghostty_theme,
        );
        let rects = renderer::SettingsPageRects::compute(190, 0, 1000 - 190, 600);
        assert_eq!(rects.nav_appearance.0, 190 + 36);
    }

    #[test]
    fn test_settings_tab_actions_and_theme_switching() {
        let config = AxonConfig::load_or_default();
        let mut app = AxonApp::new(config, None);

        // 1. Alternar pestaña de Ajustes
        assert!(!app.tabs[app.active_tab_index].is_settings);
        app.toggle_settings();
        assert!(app.tabs[app.active_tab_index].is_settings);
        app.toggle_settings();
        assert!(!app.tabs[app.active_tab_index].is_settings);

        // 2. Cambio cíclico de temas
        let initial_theme = app.config.appearance.theme.clone();
        app.cycle_theme(true);
        assert_ne!(app.config.appearance.theme, initial_theme);
        assert!(!app.theme.ansi.is_empty());
        assert!(theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Frappé"));

        // 3. Ajuste de opacidad e interlineado
        let initial_op = app.config.appearance.window_opacity;
        app.adjust_opacity(-0.05);
        assert!(app.config.appearance.window_opacity <= initial_op);

        let initial_spacing = app.config.appearance.line_spacing;
        app.adjust_line_spacing(0.1);
        assert!(app.config.appearance.line_spacing >= initial_spacing);

        // 4. Toggles de terminal
        let init_blink = app.config.terminal.cursor_blink;
        app.toggle_cursor_blink();
        assert_eq!(app.config.terminal.cursor_blink, !init_blink);

        let init_prompt = app.config.terminal.ghostty_prompt;
        app.toggle_ghostty_prompt();
        assert_eq!(app.config.terminal.ghostty_prompt, !init_prompt);
    }

    #[test]
    fn test_app_tab_switching_and_toggle() {
        let config = AxonConfig::load_or_default();
        let mut app = AxonApp::new(config, None);

        assert!(
            !app.tabs.is_empty(),
            "Debe inicializar con al menos 1 pestaña"
        );

        // Forzar estado conocido para validar transición y cálculo de offset
        app.sidebar_visible = true;
        assert_eq!(app.sidebar_offset(), 190);

        // Conmutar visibilidad de la barra lateral (ocultar)
        app.toggle_sidebar();
        assert!(!app.sidebar_visible);
        assert_eq!(app.sidebar_offset(), 0);

        // Conmutar nuevamente (mostrar)
        app.toggle_sidebar();
        assert!(app.sidebar_visible);
        assert_eq!(app.sidebar_offset(), 190);
    }

    #[test]
    fn test_split_panes_layout_and_tree() {
        use crate::app::{SplitDirection, TerminalPane, TerminalTab};
        use crate::pty::PtyManager;
        use crate::terminal::TerminalState;
        use crate::theme::Theme;

        let theme = Theme::ghostty_default_styledark();
        let pty1 = PtyManager::new(1, "powershell.exe", false, 80, 24, None, None).expect("Pty 1");
        let term1 = TerminalState::new(80, 24, 1000, theme.clone());
        let pane1 = TerminalPane {
            id: 1,
            pty: pty1,
            terminal: term1,
            last_grid_size: (80, 24),
        };

        let mut tab = TerminalTab::new(0, "Test Tab".to_string(), pane1);
        assert_eq!(tab.panes.len(), 1);
        assert_eq!(tab.active_pane_id, 1);

        // 1. Rectángulo inicial sin splits
        let rects = tab.compute_pane_rects(0, 0, 1000, 600);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].pane_id, 1);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[0].width, 1000);
        assert_eq!(rects[0].height, 600);

        // 2. División horizontal (Ctrl + Alt + D)
        let pty2 = PtyManager::new(2, "powershell.exe", false, 80, 24, None, None).expect("Pty 2");
        let term2 = TerminalState::new(80, 24, 1000, theme.clone());
        let pane2 = TerminalPane {
            id: 2,
            pty: pty2,
            terminal: term2,
            last_grid_size: (80, 24),
        };

        tab.split_active_pane(2, pane2, SplitDirection::Horizontal);
        assert_eq!(tab.panes.len(), 2);
        assert_eq!(tab.active_pane_id, 2);

        let h_rects = tab.compute_pane_rects(0, 0, 1000, 600);
        assert_eq!(h_rects.len(), 2);
        // Ambos comparten el mismo ancho (1000)
        assert_eq!(h_rects[0].width, 1000);
        assert_eq!(h_rects[1].width, 1000);
        // La suma de alturas + divisor (2px) debe ser exactamente 600
        assert_eq!(h_rects[0].height + h_rects[1].height + 2, 600);
        assert_eq!(h_rects[0].y, 0);
        assert_eq!(h_rects[1].y, h_rects[0].height + 2);

        let dividers = tab.compute_dividers(0, 0, 1000, 600);
        assert_eq!(dividers.len(), 1);
        assert_eq!(dividers[0], (0, h_rects[0].height, 1000, 2));

        // 3. Cerrar un panel (Ctrl + Alt + W) y validar que el árbol colapsa limpiamente
        tab.close_pane(1);
        assert_eq!(tab.panes.len(), 1);
        assert_eq!(tab.active_pane_id, 2);

        let collapsed_rects = tab.compute_pane_rects(0, 0, 1000, 600);
        assert_eq!(collapsed_rects.len(), 1);
        assert_eq!(collapsed_rects[0].pane_id, 2);
        assert_eq!(collapsed_rects[0].width, 1000);
        assert_eq!(collapsed_rects[0].height, 600);
    }

    #[test]
    fn test_hotkey_physical_and_logical_matching() {
        use crate::app::{key_code_to_char, key_is_char};
        use winit::keyboard::{Key, KeyCode, NativeKey, PhysicalKey};

        // 1. Simulación de Windows WM_SYSKEYDOWN (Ctrl + Alt + D) donde logical_key es Unidentified
        let syskey_d_unidentified = Key::Unidentified(NativeKey::Windows(68));
        let phys_key_d = PhysicalKey::Code(KeyCode::KeyD);
        assert!(
            key_is_char(&syskey_d_unidentified, phys_key_d, 'd'),
            "Ctrl+Alt+D debe coincidir incluso cuando logical_key es Unidentified"
        );

        // 2. Coincidencia para V, E, W en splits y cierres
        assert!(key_is_char(
            &syskey_d_unidentified,
            PhysicalKey::Code(KeyCode::KeyV),
            'v'
        ));
        assert!(key_is_char(
            &syskey_d_unidentified,
            PhysicalKey::Code(KeyCode::KeyE),
            'e'
        ));
        assert!(key_is_char(
            &syskey_d_unidentified,
            PhysicalKey::Code(KeyCode::KeyW),
            'w'
        ));

        // 3. Simulación de logical_key estándar donde physical_key no coincide (ej. layout no QWERTY)
        let log_key_d = Key::Character("d".into());
        let log_key_upper_d = Key::Character("D".into());
        assert!(key_is_char(
            &log_key_d,
            PhysicalKey::Code(KeyCode::KeyA),
            'd'
        ));
        assert!(key_is_char(
            &log_key_upper_d,
            PhysicalKey::Code(KeyCode::KeyA),
            'd'
        ));

        // 4. Mapeo de key codes numéricos y símbolos
        assert_eq!(key_code_to_char(KeyCode::KeyD), Some('d'));
        assert_eq!(key_code_to_char(KeyCode::Digit1), Some('1'));
        assert_eq!(key_code_to_char(KeyCode::Numpad1), Some('1'));
        assert_eq!(key_code_to_char(KeyCode::Minus), Some('-'));
        assert_eq!(key_code_to_char(KeyCode::Equal), Some('='));
    }

    #[test]
    fn test_catppuccin_latte_and_light_mode() {
        let latte = theme::Theme::catppuccin_latte();
        assert!(
            latte.is_light(),
            "Catppuccin Latte debe reportar is_light() == true"
        );
        assert_eq!(latte.background, theme::Color::from_rgb(0xef, 0xf1, 0xf5));
        assert_eq!(latte.foreground, theme::Color::from_rgb(0x4c, 0x4f, 0x69));

        let frappe = theme::Theme::catppuccin_frappe();
        assert!(
            !frappe.is_light(),
            "Catppuccin Frappé debe reportar is_light() == false"
        );

        assert!(theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Latte"));
        assert!(theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Frappé"));

        let resolved_latte = theme::Theme::from_name("latte");
        assert!(resolved_latte.is_light());
    }

    #[test]
    fn test_ai_agent_detection_and_status_bar() {
        use crate::app::detect_ai_agent;
        use crate::renderer::AiAgentStatus;

        assert_eq!(
            detect_ai_agent(Some("Claude Code v0.2.29")),
            AiAgentStatus::Claude
        );
        assert_eq!(
            detect_ai_agent(Some("C:\\Users\\user\\.cargo\\bin\\agy.exe")),
            AiAgentStatus::Agy
        );
        assert_eq!(
            detect_ai_agent(Some("gemini-cli session")),
            AiAgentStatus::Gemini
        );
        assert_eq!(
            detect_ai_agent(Some("aider: editing src/app.rs")),
            AiAgentStatus::Aider
        );
        assert_eq!(detect_ai_agent(Some("copilot CLI")), AiAgentStatus::Copilot);
        assert_eq!(
            detect_ai_agent(Some("ollama run llama3")),
            AiAgentStatus::Ollama
        );
    }

    #[test]
    fn test_top_tab_bar_geometry_and_layout() {
        use crate::renderer::TopTabBarRects;

        let rects = TopTabBarRects::compute(1200, 3);
        assert_eq!(rects.bar_h, 38);
        assert_eq!(rects.tabs_start_x, 72);
        assert_eq!(rects.tab_w, 160);

        let tab0 = rects.tab_rect(0);
        assert_eq!(tab0, (72, 4, 160, 30));

        let close0 = rects.close_btn_rect(0);
        assert_eq!(close0.0, 72 + 160 - 22);
        assert_eq!(close0.1, 10);
        assert_eq!(close0.2, 18);
        assert_eq!(close0.3, 18);

        // Plus button debe situarse después de las 3 pestañas
        assert!(rects.plus_btn.0 > tab0.0 + (3 * 160));
        // Settings button debe ubicarse al extremo derecho
        assert_eq!(rects.settings_btn.0, 1200 - 36);
    }
}
