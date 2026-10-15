//! Tests y benchmarks del renderer.

use super::*;
use crate::config::AxonConfig;

#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    /// Coste CPU de un fotograma completo de terminal. `cargo test --release bench_render -- --nocapture`
    #[test]
    fn bench_render_pane_full_frame() {
        let cfg = AxonConfig::default();
        let theme = Theme::from_name(&cfg.appearance.theme);
        let mut r = TerminalRenderer::new(&cfg, 1.0);
        let (w, h) = (1920usize, 1080usize);
        let (cols, rows) = r.compute_pane_grid_size(w, h);
        let mut term = TerminalState::new(cols, rows, 1000, theme.clone());
        for i in 0..rows {
            let line = format!(
                "\x1b[3{}m{:>4}\x1b[0m  fn render_frame(&mut self, buf: &mut [u32]) -> Result<(), Error> {{ let x = {}; }}\r\n",
                1 + i % 7, i, i * 37
            );
            term.process_bytes(line.clone().as_bytes());
        }
        let mut buf = vec![0u32; w * h];
        r.render_pane(&mut buf, w, h, 0, 0, w, h, &term, &theme, true, true, false); // calienta cachés
        let n = 200;
        let t = Instant::now();
        for _ in 0..n {
            r.render_pane(&mut buf, w, h, 0, 0, w, h, &term, &theme, true, true, false);
        }
        let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t = Instant::now();
        for _ in 0..n {
            buf.fill(theme.background.to_u32_xrgb());
        }
        let fill = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("render_pane {w}x{h} ({cols}x{rows} celdas): {per:.2} ms/frame; buffer.fill: {fill:.2} ms");
    }
}

#[test]
fn test_native_renderer_metrics() {
    let config = AxonConfig::load_or_default();
    let renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
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
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);

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
    let mut terminal =
        crate::terminal::TerminalState::new(80, 24, 1000, crate::theme::Theme::ghostty_dark());
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
    let expected_bg = crate::theme::Color::from_rgb(22, 22, 24).to_u32_xrgb();

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
fn test_sidebar_rendering_and_layout() {
    use crate::renderer::SidebarTabItem;

    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);

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
            is_file_viewer: false,
        },
        SidebarTabItem {
            id: 2,
            title: "Terminal 2".to_string(),
            is_active: false,
            has_unread: true,
            pane_count: 2,
            is_settings: false,
            is_file_viewer: false,
        },
    ];

    let ghostty_theme = crate::theme::Theme::ghostty_default_styledark();
    renderer.render_sidebar(
        &mut buffer,
        1000,
        600,
        190,
        26,
        &tabs,
        0,
        Some(1),
        None,
        false,
        false,
        false,
        &ghostty_theme,
    );

    let sidebar_bg_pixel = ghostty_theme.sidebar_bg().to_u32_xrgb();
    let border_pixel = ghostty_theme.border_color().to_u32_xrgb();

    assert_eq!(
        buffer[50 * 1000 + 2],
        sidebar_bg_pixel,
        "El fondo del sidebar debe coincidir con sidebar_bg"
    );
    assert_eq!(
        buffer[50 * 1000 + 189],
        border_pixel,
        "El borde del sidebar debe coincidir con border_color"
    );

    // 3. Renderizar el área de terminal con offset
    let term = crate::terminal::TerminalState::new(
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
    renderer.render_floating_sidebar_toggle(&mut buffer, 1000, 600, 0, true, false, &ghostty_theme);
    let float_btn_bg = ghostty_theme.surface_hover().to_u32_xrgb();
    let float_y = 600 - 30 - 8 + 5;
    assert_eq!(
        buffer[float_y * 1000 + 10],
        float_btn_bg,
        "El botón flotante debe renderizarse correctamente"
    );

    // 5. Probar renderizado de la pestaña de Ajustes
    let settings_state = crate::renderer::SettingsPageState::default();
    renderer.render_settings_page(
        &mut buffer,
        1000,
        600,
        190,
        0,
        &config,
        &settings_state,
        &ghostty_theme,
    );
    let rects = crate::renderer::SettingsPageRects::compute(190, 0, 1000 - 190, 600);
    assert_eq!(rects.nav_appearance.0, rects.content_x);
    assert!(rects.card_w >= 460 && rects.card_w <= (1000 - 190));
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

    // Validar que render_status_bar cubre todo el ancho inferior (0..width)
    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let theme = crate::theme::Theme::ghostty_default_styledark();
    let mut buffer = vec![0xCAFEBABE; 1000 * 600];

    let sidebar_offset = 180;
    renderer.render_status_bar(
        &mut buffer,
        1000,
        600,
        sidebar_offset,
        Some("C:\\Users\\test"),
        Some("main"),
        None,
        80,
        24,
        AiAgentStatus::Claude,
        false,
        &theme,
    );

    // El status bar cubre todo el ancho inferior (0..width), por encima del cual reposa el sidebar
    let status_bg = theme.sidebar_bg().to_u32_xrgb();
    let border_c = theme.border_color().to_u32_xrgb();
    assert_eq!(
        buffer[(600 - 26) * 1000 + 5],
        border_c,
        "El borde superior del status bar debe extenderse por todo el ancho"
    );
    assert_eq!(
        buffer[(600 - 20) * 1000 + 5],
        status_bg,
        "El fondo del status bar debe dibujarse a todo lo ancho"
    );
    assert_eq!(
        buffer[(600 - 20) * 1000 + sidebar_offset + 5],
        status_bg,
        "El fondo del status bar debe dibujarse en el área de trabajo"
    );
}

#[test]
fn test_top_tab_bar_geometry_and_layout() {
    use crate::renderer::TopTabBarRects;

    let rects = TopTabBarRects::compute(1200, 3);
    assert_eq!(rects.bar_h, 38);
    assert_eq!(rects.tabs_start_x, 273);
    assert_eq!(rects.tab_w, 160);

    let tab0 = rects.tab_rect(0);
    assert_eq!(tab0, (273, 4, 160, 30));

    let close0 = rects.close_btn_rect(0);
    assert_eq!(close0.0, 273 + 160 - 20);
    assert_eq!(close0.1, 10);
    assert_eq!(close0.2, 16);
    assert_eq!(close0.3, 16);

    // Plus button debe situarse después de las 3 pestañas
    assert!(rects.plus_btn.0 > tab0.0 + (3 * 160));
    // Settings button y controles de ventana al extremo derecho
    assert_eq!(rects.settings_btn.0, 1200 - 160);
    assert_eq!(rects.win_min_btn.0, 1200 - 126);
    assert_eq!(rects.win_max_btn.0, 1200 - 84);
    assert_eq!(rects.win_close_btn.0, 1200 - 42);

    // Validar que con 30 pestañas NO se desborde sobre los controles de la derecha
    let compact_rects = TopTabBarRects::compute(1200, 30);
    assert!(compact_rects.plus_btn.0 + compact_rects.plus_btn.2 <= compact_rects.files_btn.0);
    let tab29 = compact_rects.tab_rect(29);
    assert!(tab29.0 + tab29.2 <= compact_rects.plus_btn.0);
}

#[test]
fn test_render_frame_latency_and_fps_benchmark() {
    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let mut term =
        crate::terminal::TerminalState::new(120, 40, 1000, crate::theme::Theme::ghostty_dark());

    // Llenar grilla con texto y colores realistas
    for i in 0..40 {
        let line = format!(
                "\x1b[38;2;81;226;122m[Line {:02}]\x1b[0m fn main() {{ println!(\"Axon fast renderer #{} \u{e0a0}\"); }} // OK\r\n",
                i, i
            );
        term.process_bytes(line.as_bytes());
    }

    let width = 1920;
    let height = 1080;
    let mut buffer = vec![0u32; width * height];

    // Warm up
    renderer.render_pane(
        &mut buffer,
        width,
        height,
        0,
        0,
        width,
        height,
        &term,
        &term.theme,
        true,
        true,
        false,
    );

    let frames = 60;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        renderer.render_pane(
            &mut buffer,
            width,
            height,
            0,
            0,
            width,
            height,
            &term,
            &term.theme,
            true,
            true,
            false,
        );
    }
    let total_elapsed = start.elapsed();
    let avg_frame_ms = total_elapsed.as_secs_f64() * 1000.0 / frames as f64;
    let potential_fps = frames as f64 / total_elapsed.as_secs_f64();

    println!(
            "\n⚡ [Axon Render Benchmark 1080p Fullscreen] {} fotogramas en {:.2?}: {:.3} ms/frame | {:.0} FPS teóricos",
            frames, total_elapsed, avg_frame_ms, potential_fps
        );

    // El renderizado por software en CPU de una grilla 1080p completa debe ser ultra fluido (<3ms en release y <35ms en debug)
    let max_allowed_ms = if cfg!(debug_assertions) { 40.0 } else { 16.6 };
    assert!(
        avg_frame_ms < max_allowed_ms,
        "El frametime ({:.2}ms) debe ser menor al límite ({:.2}ms)",
        avg_frame_ms,
        max_allowed_ms
    );
}

#[test]
fn test_ui_font_size_scaling() {
    let mut config = AxonConfig::load_or_default();
    config.appearance.ui_font_size = 13.0;
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);

    let text = "Configuración y Ajustes de Axon";
    let w_13 = renderer.measure_ui_text(text, 11.5, false);

    // Ajustar a 16.0 pt
    config.appearance.ui_font_size = 16.0;
    renderer.update_scale(&config, 1.0);
    let w_16 = renderer.measure_ui_text(text, 11.5, false);

    // Ajustar a 10.0 pt
    config.appearance.ui_font_size = 10.0;
    renderer.update_scale(&config, 1.0);
    let w_10 = renderer.measure_ui_text(text, 11.5, false);

    assert!(
        w_16 > w_13,
        "El ancho a 16pt ({w_16}px) debe ser mayor que a 13pt ({w_13}px)"
    );
    assert!(
        w_10 < w_13,
        "El ancho a 10pt ({w_10}px) debe ser menor que a 13pt ({w_13}px)"
    );
}
