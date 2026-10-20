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
        let n = if cfg!(debug_assertions) { 4 } else { 200 };
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

    #[test]
    fn bench_sdf_rounded_rect_and_stroke() {
        let (w, h) = (1920usize, 1080usize);
        let mut buf = vec![0xFF16_1618u32; w * h];
        let fill_color = Color::from_rgb(38, 42, 52);
        let stroke_color = Color::from_rgb(90, 100, 120);
        let n = if cfg!(debug_assertions) { 6 } else { 300 };
        let t = Instant::now();
        for _ in 0..n {
            for i in 0..12 {
                let y = 40 + i * 76;
                crate::ui::paint::draw_rounded_rect(&mut buf, w, h, 80, y, 820, 64, 10, fill_color);
                crate::ui::paint::stroke_rounded_rect(
                    &mut buf,
                    w,
                    h,
                    80,
                    y,
                    820,
                    64,
                    10,
                    stroke_color,
                );
            }
        }
        let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("bench_sdf_rounded_rect_and_stroke (12 cards 820x64): {per:.3} ms/iter");
    }

    #[test]
    fn bench_render_file_viewer_code_and_markdown() {
        let cfg = AxonConfig::default();
        let theme = Theme::from_name(&cfg.appearance.theme);
        let mut r = TerminalRenderer::new(&cfg, 1.0);
        let (w, h) = (1920usize, 1080usize);
        let mut buf = vec![0u32; w * h];

        let dir = std::env::temp_dir().join(format!("axon_bench_fv_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let rs_path = dir.join("sample.rs");
        let md_path = dir.join("README.md");

        let mut rs_code = String::new();
        for i in 0..120 {
            rs_code.push_str(&format!(
                "pub fn compute_block_{i}(input: &[u32], factor: usize) -> u64 {{ // comentario {i}\n    let mut acc: u64 = 0x1234_5678;\n    for &v in input {{ acc = acc.wrapping_add((v as u64) * (factor as u64)); }}\n    acc\n}}\n"
            ));
        }
        let _ = std::fs::write(&rs_path, &rs_code);

        let mut md_text = String::from("# Guía de Rendimiento de Axon\n\nTexto con **negrita**, *cursiva* y `código inline`.\n\n");
        for i in 0..40 {
            md_text.push_str(&format!(
                "## Sección {i}\n- Punto de verificación {i} con ruta `src/renderer/pane.rs`\n- [x] Tarea completada {i}\n\n```rust\nfn demo_{i}() {{ println!(\"hola {i}\"); }}\n```\n\n"
            ));
        }
        let _ = std::fs::write(&md_path, &md_text);

        let fv_rs = crate::app::TerminalTab::new_file_viewer(1, rs_path.clone())
            .file_viewer
            .unwrap();
        let fv_md = crate::app::TerminalTab::new_file_viewer(2, md_path.clone())
            .file_viewer
            .unwrap();

        r.render_file_viewer(
            &mut buf,
            w,
            h,
            190,
            0,
            w - 190,
            h - 26,
            0,
            true,
            &fv_rs,
            &theme,
        );
        let n = if cfg!(debug_assertions) { 3 } else { 100 };
        let t_rs = Instant::now();
        for _ in 0..n {
            r.render_file_viewer(
                &mut buf,
                w,
                h,
                190,
                0,
                w - 190,
                h - 26,
                0,
                true,
                &fv_rs,
                &theme,
            );
        }
        let rs_ms = t_rs.elapsed().as_secs_f64() * 1000.0 / n as f64;

        r.render_file_viewer(
            &mut buf,
            w,
            h,
            190,
            0,
            w - 190,
            h - 26,
            0,
            true,
            &fv_md,
            &theme,
        );
        let t_md = Instant::now();
        for _ in 0..n {
            r.render_file_viewer(
                &mut buf,
                w,
                h,
                190,
                0,
                w - 190,
                h - 26,
                0,
                true,
                &fv_md,
                &theme,
            );
        }
        let md_ms = t_md.elapsed().as_secs_f64() * 1000.0 / n as f64;

        let _ = std::fs::remove_dir_all(&dir);
        println!("bench_render_file_viewer (1920x1080): Rust code = {rs_ms:.2} ms/frame | Markdown preview = {md_ms:.2} ms/frame");
    }

    #[test]
    fn bench_render_ui_chrome() {
        let cfg = AxonConfig::default();
        let theme = Theme::from_name(&cfg.appearance.theme);
        let mut r = TerminalRenderer::new(&cfg, 1.0);
        let (w, h) = (1920usize, 1080usize);
        let mut buf = vec![0u32; w * h];
        let tabs: Vec<SidebarTabItem> = (0..8)
            .map(|i| SidebarTabItem {
                index: 0,
                status: None,
                title: format!("pwsh — axon/src #{i}"),
                detail: Some("C:\\Users\\dev\\axon".to_string()),
                is_active: i == 2,
                has_unread: i % 3 == 0,
                pane_count: 1 + (i % 2),
                is_settings: false,
                is_file_viewer: i == 5,
                is_preview: i == 5,
            })
            .collect();
        let settings_state = SettingsPageState::default();

        // Warm up
        r.render_sidebar(
            &mut buf,
            w,
            h,
            190,
            26,
            &[],
            &crate::renderer::SidebarHeader::default(),
            None,
            &tabs,
            0,
            None,
            None,
            false,
            false,
            false,
            &theme,
        );
        r.render_top_tab_bar(
            &mut buf,
            w,
            h,
            &tabs,
            &crate::renderer::TopBarProject::default(),
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            &theme,
        );
        r.render_status_bar(
            &mut buf,
            w,
            h,
            190,
            Some("~/dev/axon"),
            Some("main"),
            None,
            180,
            48,
            AiAgentStatus::Claude,
            true,
            &theme,
        );
        r.render_settings_page(&mut buf, w, h, 190, 0, 0, 26, &cfg, &settings_state, &theme);

        let n = if cfg!(debug_assertions) { 3 } else { 120 };
        let t_chrome = Instant::now();
        for _ in 0..n {
            r.render_sidebar(
                &mut buf,
                w,
                h,
                190,
                26,
                &[],
                &crate::renderer::SidebarHeader::default(),
                None,
                &tabs,
                0,
                None,
                None,
                false,
                false,
                false,
                &theme,
            );
            r.render_top_tab_bar(
                &mut buf,
                w,
                h,
                &tabs,
                &crate::renderer::TopBarProject::default(),
                false,
                None,
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                &theme,
            );
            r.render_status_bar(
                &mut buf,
                w,
                h,
                190,
                Some("~/dev/axon"),
                Some("main"),
                None,
                180,
                48,
                AiAgentStatus::Claude,
                true,
                &theme,
            );
        }
        let chrome_ms = t_chrome.elapsed().as_secs_f64() * 1000.0 / n as f64;

        let t_settings = Instant::now();
        for _ in 0..n {
            r.render_settings_page(&mut buf, w, h, 190, 0, 0, 26, &cfg, &settings_state, &theme);
        }
        let settings_ms = t_settings.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("bench_render_ui_chrome: sidebar+topbar+statusbar = {chrome_ms:.2} ms/frame | settings_page = {settings_ms:.2} ms/frame");
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

    // 2. Probar que una secuencia TrueColor se renderiza con los píxeles de ese color
    let mut terminal =
        crate::terminal::TerminalState::new(80, 24, 1000, crate::theme::Theme::default());
    // Escribir texto con color verde (#51e27a / RGB 81, 226, 122) y un icono Nerd Font
    let colored_seq = b"\x1b[38;2;81;226;122m\xee\x82\xa0 Axon\x1b[0m";
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
    let expected_bg = crate::theme::Theme::default().background.to_u32_xrgb();

    for (i, &pixel) in buffer.iter().enumerate() {
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;

        if g > 150 && r < 100 && b < 150 {
            has_green_pixels = true;
        }
        // Los primeros píxeles de margen deben ser el fondo del tema
        if i < 800 * 5 && pixel != expected_bg {
            background_matches = false;
        }
    }

    assert!(
        background_matches,
        "El fondo del canvas debe coincidir exactamente con el fondo del tema"
    );
    assert!(
        has_green_pixels,
        "El buffer debe contener píxeles con el color verde TrueColor renderizado"
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
            index: 1,
            status: None,
            title: "Terminal 1".to_string(),
            detail: Some("C:\\Users\\dev".to_string()),
            is_active: true,
            has_unread: false,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
        SidebarTabItem {
            index: 2,
            status: None,
            title: "Terminal 2".to_string(),
            detail: Some("2 paneles".to_string()),
            is_active: false,
            has_unread: true,
            pane_count: 2,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
    ];

    let default_theme = crate::theme::Theme::default();
    renderer.render_sidebar(
        &mut buffer,
        1000,
        600,
        190,
        26,
        &[],
        &crate::renderer::SidebarHeader::default(),
        None,
        &tabs,
        0,
        Some(1),
        None,
        false,
        false,
        false,
        &default_theme,
    );

    let sidebar_bg_pixel = default_theme.sidebar_bg().to_u32_xrgb();
    let border_pixel = default_theme.border_color().to_u32_xrgb();

    assert_eq!(
        buffer[50 * 1000 + SidebarRects::RAIL_W + 4],
        sidebar_bg_pixel,
        "El fondo de la columna de sesiones debe coincidir con sidebar_bg"
    );
    assert_eq!(
        buffer[50 * 1000 + SidebarRects::RAIL_W - 2],
        default_theme.background.to_u32_xrgb(),
        "El riel de proyectos usa el fondo del lienzo"
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
        default_theme.clone(),
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
        "El área de terminal después del offset debe tener fondo del tema"
    );

    // 4. Probar renderizado del botón flotante expand cuando el sidebar está oculto
    renderer.render_floating_sidebar_toggle(&mut buffer, 1000, 600, 0, true, false, &default_theme);
    let float_btn_bg = default_theme.surface_hover().to_u32_xrgb();
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
        0,
        26,
        &config,
        &settings_state,
        &default_theme,
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
    assert_eq!(detect_ai_agent(Some("pwsh")), AiAgentStatus::None);
    assert_eq!(detect_ai_agent(Some("axon")), AiAgentStatus::None);

    // Validar que con AiAgentStatus::None no se muestre el modelo de IA en el badge
    let theme = crate::theme::Theme::default();
    let (icon_none, label_none, fg_none) = crate::renderer::TerminalRenderer::status_bar_badge_info(
        AiAgentStatus::None,
        None,
        Some("Gemini 4 Argon (Medium)"),
        &theme,
    );
    assert_eq!(icon_none, crate::ui::LucideIcon::Dot);
    assert_eq!(label_none, "LOCAL");
    assert_eq!(fg_none, theme.text_muted());

    let (icon_agy, label_agy, fg_agy) = crate::renderer::TerminalRenderer::status_bar_badge_info(
        AiAgentStatus::Agy,
        None,
        Some("Gemini 3.8 Flash (Medium)"),
        &theme,
    );
    assert_eq!(icon_agy, crate::ui::LucideIcon::Sparkles);
    assert_eq!(label_agy, "AGY · Gemini 3.8 Flash (Medium)");
    assert_eq!(fg_agy, theme.ansi[6]);

    // Validar que render_status_bar cubre todo el ancho inferior (0..width)
    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let theme = crate::theme::Theme::default();
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
    assert_eq!(rects.tabs_start_x, TopTabBarRects::SWITCHER_W);
    assert_eq!(rects.tab_w, 190);

    let tab0 = rects.tab_rect(0);
    assert_eq!(tab0, (TopTabBarRects::SWITCHER_W, 0, 190, 38));

    let close0 = rects.close_btn_rect(0);
    assert_eq!(close0.0, TopTabBarRects::SWITCHER_W + 190 - 30);
    assert_eq!(close0.1, 7);
    assert_eq!(close0.2, 24);
    assert_eq!(close0.3, 24);

    // Plus button debe situarse después de las 3 pestañas
    assert!(rects.plus_btn.0 > tab0.0 + (3 * 190));
    // Settings button, files button y controles de ventana al extremo derecho
    assert_eq!(rects.settings_btn.0, 1200 - 212);
    assert_eq!(rects.files_btn.0, 1200 - 178);
    assert_eq!(rects.win_min_btn.0, 1200 - 138);
    assert_eq!(rects.win_max_btn.0, 1200 - 92);
    assert_eq!(rects.win_close_btn.0, 1200 - 46);

    // Validar que con 30 pestañas NO se desborde sobre los controles de la derecha
    let compact_rects = TopTabBarRects::compute(1200, 30);
    assert!(compact_rects.plus_btn.0 + compact_rects.plus_btn.2 <= compact_rects.settings_btn.0);
    let tab29 = compact_rects.tab_rect(29);
    assert!(tab29.0 + tab29.2 <= compact_rects.plus_btn.0);
}

#[test]
fn test_render_frame_latency_and_fps_benchmark() {
    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let mut term =
        crate::terminal::TerminalState::new(120, 40, 1000, crate::theme::Theme::default());

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

#[test]
fn test_settings_page_scrolls_only_the_content() {
    use crate::renderer::{SettingsPageRects, SettingsPageState, SettingsSection};
    let config = crate::config::AxonConfig::default();
    let theme = crate::theme::Theme::default();
    let mut renderer = TerminalRenderer::new(&config, 1.0);
    let (w, h, status) = (1000usize, 300usize, 26usize);
    let mut render = |scroll: usize| {
        let mut buf = vec![0u32; w * h];
        let state = SettingsPageState {
            scroll_y: scroll,
            ..SettingsPageState::default()
        };
        renderer.render_settings_page(&mut buf, w, h, 190, 0, 0, status, &config, &state, &theme);
        buf
    };
    let (top, scrolled) = (render(0), render(120));
    assert_ne!(top, scrolled, "el contenido debe moverse con el scroll");
    let nav = SettingsPageRects::CONTENT_TOP * w;
    assert_eq!(
        top[..nav],
        scrolled[..nav],
        "cabecera y navegación quedan fijas"
    );

    let max = SettingsPageRects::max_scroll(
        SettingsSection::Appearance,
        h - SettingsPageRects::CONTENT_TOP - status,
    );
    assert!(max > 0);
    assert_eq!(
        render(max),
        render(max + 500),
        "el scroll se acota al final del contenido"
    );
}

#[test]
fn test_file_viewer_layout_columns_and_horizontal_scrollbar() {
    use crate::renderer::FileViewerLayout;
    let lay = FileViewerLayout::compute(0, 40, 800, 400, 20, 9, 120);
    assert!(lay.text_x > lay.sep_x && lay.text_max_x > lay.text_x);

    // Clic antes del texto = columna 0; tabulador ocupa 4 celdas; sin scroll y con scroll
    assert_eq!(lay.col_at("abc", lay.text_x.saturating_sub(5), 0), 0);
    assert_eq!(lay.col_at("abc", lay.text_x + 9 * 2 + 1, 0), 2);
    assert_eq!(lay.col_at("\tx", lay.text_x + 9 * 4 + 2, 0), 1);
    assert_eq!(lay.col_at("abcdef", lay.text_x + 1, 3), 3);
    assert_eq!(
        lay.col_at("ab", lay.text_x + 900, 0),
        2,
        "más allá del final = fin de línea"
    );

    // Barra horizontal: oculta sin scroll; con scroll el pulgar recorre la pista de punta a punta
    assert!(lay.hscrollbar(0, 0).is_none());
    let (track, start) = lay.hscrollbar(50, 0).expect("barra");
    let (_, end) = lay.hscrollbar(50, 50).expect("barra");
    assert_eq!(start.0, track.0);
    assert_eq!(end.0 + end.2, track.0 + track.2);
    let (_, over) = lay.hscrollbar(50, 500).expect("barra");
    assert_eq!(over, end, "el scroll sobrante se acota");
}

#[test]
fn test_settings_scrollbar_geometry_spans_the_track() {
    use crate::renderer::{SettingsPageRects, SettingsSection};
    let sec = SettingsSection::Appearance;
    let max = SettingsPageRects::max_scroll(sec, 200);
    assert!(max > 0);
    let (track, top) = SettingsPageRects::scrollbar(sec, 40, 900, 200, 0).expect("barra");
    let (_, bottom) = SettingsPageRects::scrollbar(sec, 40, 900, 200, max).expect("barra");
    assert_eq!(top.1, track.1);
    assert_eq!(bottom.1 + bottom.3, track.1 + track.3);
    // sin desbordamiento no hay barra
    assert!(SettingsPageRects::scrollbar(sec, 40, 900, 5000, 0).is_none());
}

#[test]
fn test_font_ligature_shaping() {
    let cfg = AxonConfig::default();
    let mut r = TerminalRenderer::new(&cfg, 1.0);
    assert!(r.ligatures);

    // Probar secuencias de ligadura comunes
    for op in [
        "->", "=>", "!=", "==", "===", "<=", ">=", "::", ":=", "//", "/*", "*/",
    ] {
        let shaped = r.shape_punctuation_run(op.as_bytes(), false);
        // El primer glifo de la ligadura debe tener ancho 0 (filler)
        let first_glyph = r.get_ligature_glyph(shaped.gids[0], false);
        assert_eq!(
            first_glyph.width, 0,
            "Primer glifo de '{}' debe ser filler 0-width",
            op
        );

        // El último glifo debe tener x_offset negativo (se expande hacia atrás para cubrir las celdas previas)
        let last_glyph = r.get_ligature_glyph(shaped.gids[op.len() - 1], false);
        assert!(
            last_glyph.x_offset < 0,
            "Último glifo de '{}' debe tener x_offset < 0 (obtenido {})",
            op,
            last_glyph.x_offset
        );
        assert!(
            last_glyph.width > r.char_width,
            "Último glifo de '{}' debe abarcar más de una celda",
            op
        );
    }
}

#[test]
fn test_render_pane_with_and_without_ligatures() {
    let cfg = AxonConfig::default();
    let theme = Theme::from_name(&cfg.appearance.theme);
    let mut r = TerminalRenderer::new(&cfg, 1.0);
    let (w, h) = (400usize, 100usize);
    let (cols, rows) = r.compute_pane_grid_size(w, h);
    let mut term = TerminalState::new(cols, rows, 100, theme.clone());
    term.process_bytes(b"fn test() -> bool { a != b && c == d }");

    // Con ligaduras activas
    r.ligatures = true;
    let mut buf_with = vec![0u32; w * h];
    r.render_pane(
        &mut buf_with,
        w,
        h,
        0,
        0,
        w,
        h,
        &term,
        &theme,
        true,
        true,
        false,
    );

    // Sin ligaduras
    r.ligatures = false;
    let mut buf_without = vec![0u32; w * h];
    r.render_pane(
        &mut buf_without,
        w,
        h,
        0,
        0,
        w,
        h,
        &term,
        &theme,
        true,
        true,
        false,
    );

    // Los dos framebuffers deben diferir porque la flecha `->` y `!=` cambian de píxeles
    let diffs = buf_with
        .iter()
        .zip(buf_without.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        diffs > 0,
        "El renderizado con ligaduras debe diferir del estándar"
    );
}

#[test]
fn test_settings_ligatures_toggle_rect() {
    let r = SettingsPageRects::compute(0, 0, 800, 600);
    assert!(r.ligatures_btn.2 > 0 && r.ligatures_btn.3 > 0);
    assert_eq!(r.ligatures_btn.1, SettingsPageRects::CONTENT_TOP + 264);
}

#[test]
fn test_config_gpu_option_defaults_to_true() {
    let cfg = AxonConfig::default();
    assert!(cfg.terminal.gpu);
}

#[test]
fn test_d3d11_presenter_lifecycle() {
    use windows::Win32::Foundation::HWND;
    use windows_sys::Win32::Foundation::HWND as SysHWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, CW_USEDEFAULT, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };

    unsafe {
        let class_name: Vec<u16> = "AxonD3D11TestClass\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);

        let hwnd_sys = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            320,
            240,
            0 as SysHWND,
            0 as _,
            0 as _,
            std::ptr::null(),
        );

        if hwnd_sys != 0 as SysHWND {
            let hwnd = HWND(hwnd_sys as _);
            if let Ok(mut presenter) = D3D11Presenter::new(hwnd, 320, 240) {
                assert_eq!(presenter.size(), (320, 240));
                assert_eq!(presenter.buffer_mut().len(), 320 * 240);

                // Llenar buffer y presentar
                presenter.buffer_mut().fill(0x00FF0000);
                assert!(presenter.present().is_ok());

                // Redimensionar
                assert!(presenter.resize(640, 480).is_ok());
                assert_eq!(presenter.size(), (640, 480));
                assert_eq!(presenter.buffer_mut().len(), 640 * 480);
                assert!(presenter.present().is_ok());
            }
            DestroyWindow(hwnd_sys);
        }
    }
}

/// Previsualización manual de la interfaz (no forma parte de la suite):
/// `cargo test preview_ui_scenes -- --ignored --nocapture` deja PPM en `%TEMP%/axon-preview`.
#[test]
#[ignore]
fn preview_ui_scenes() {
    use crate::app::SIDEBAR_WIDTH;
    use crate::renderer::{SettingsPageState, SidebarTabItem};
    use crate::theme::Theme;

    let cfg = AxonConfig::default();
    let theme = Theme::default();
    let mut r = TerminalRenderer::new(&cfg, 1.0);
    let (w, h) = (1440usize, 900usize);
    let status_h = crate::renderer::StatusBarRects::BAR_H;
    let top_h = crate::renderer::TopTitleBarRects::compute(w).bar_h;
    let out = std::env::temp_dir().join("axon-preview");
    std::fs::create_dir_all(&out).unwrap();

    let write_ppm = |buf: &[u32], name: &str| {
        let mut data = format!("P6\n{} {}\n255\n", w, h).into_bytes();
        for px in buf {
            data.push(((px >> 16) & 0xFF) as u8);
            data.push(((px >> 8) & 0xFF) as u8);
            data.push((px & 0xFF) as u8);
        }
        std::fs::write(out.join(name), data).unwrap();
    };

    let tabs: Vec<SidebarTabItem> = vec![
        SidebarTabItem {
            index: 0,
            status: Some(crate::renderer::SessionStatus::Running),
            title: "Rediseño de UI".into(),
            detail: Some("claude, Sonnet 5.5, 4 min".into()),
            is_active: true,
            has_unread: false,
            pane_count: 2,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
        SidebarTabItem {
            index: 1,
            status: Some(crate::renderer::SessionStatus::Done),
            title: "Pruebas".into(),
            detail: Some("cargo test --release".into()),
            is_active: false,
            has_unread: true,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
        SidebarTabItem {
            index: 2,
            status: Some(crate::renderer::SessionStatus::Idle),
            title: "Terminal".into(),
            detail: Some("pwsh".into()),
            is_active: false,
            has_unread: false,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
    ];

    let projects = vec![
        crate::renderer::SidebarProject {
            name: "axon".into(),
            branch: Some("main".into()),
            status: Some(crate::renderer::SessionStatus::Running),
            initial: 'A',
            color_slot: 4,
            is_active: true,
            has_news: false,
            is_loose: false,
        },
        crate::renderer::SidebarProject {
            name: "api".into(),
            branch: Some("feature/auth".into()),
            status: Some(crate::renderer::SessionStatus::Done),
            initial: 'P',
            color_slot: 6,
            is_active: false,
            has_news: true,
            is_loose: false,
        },
        crate::renderer::SidebarProject {
            name: "docs".into(),
            branch: Some("main".into()),
            status: Some(crate::renderer::SessionStatus::Idle),
            initial: 'D',
            color_slot: 3,
            is_active: false,
            has_news: false,
            is_loose: false,
        },
    ];
    let changes: Vec<crate::renderer::SidebarChange> =
        ["theme.rs", "config.rs", "tokens.rs", "README.md"]
            .iter()
            .map(|name| crate::renderer::SidebarChange {
                name: (*name).to_string(),
                path: std::path::PathBuf::from("src/renderer").join(name),
                status: crate::explorer::GitFileStatus::Modified,
            })
            .collect();
    let header = crate::renderer::SidebarHeader {
        title: "axon".into(),
        branch: Some("main".into()),
        path: Some("C:\\Users\\dev\\axon".into()),
        on_right: false,
        list_end: crate::renderer::SidebarRects::list_end(h - status_h, 3, false),
        explorer_visible: false,
        changes: Vec::new(),
    };
    let header_exp = crate::renderer::SidebarHeader {
        list_end: crate::renderer::SidebarRects::list_end(h - status_h, 3, true),
        explorer_visible: true,
        changes: changes.clone(),
        ..header.clone()
    };

    let mut term = crate::terminal::TerminalState::new(110, 34, 2000, theme.clone());
    let script = concat!(
        "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07cargo test\x1b]133;C\x07\r\n",
        "\x1b[38;5;114m   Compiling\x1b[0m axon v0.9.0\r\n",
        "\x1b[38;5;114m    Finished\x1b[0m `test` profile [unoptimized] target(s) in 4.21s\r\n",
        "\x1b[38;5;114m     Running\x1b[0m unittests src\\main.rs\r\n",
        "\x1b[1mtest result: \x1b[38;5;114mok\x1b[0m\x1b[1m. 204 passed; 0 failed; finished in 22.9s\x1b[0m\r\n",
        "\x1b]133;D;0\x07",
        "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07git status --short\x1b]133;C\x07\r\n",
        "\x1b[38;5;221m M\x1b[0m src\\renderer\\pane.rs\r\n",
        "\x1b[38;5;221m M\x1b[0m src\\renderer\\status_bar.rs\r\n",
        "\x1b[38;5;114m??\x1b[0m PRODUCT.md\r\n",
        "\x1b]133;D;0\x07",
        "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07pnpm build\x1b]133;C\x07\r\n",
        "\x1b[38;5;203merror:\x1b[0m script \"build\" no encontrado\r\n",
        "\x1b]133;D;1\x07",
        "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m ",
    );
    term.process_bytes(script.as_bytes());

    // 1. Terminal con barra lateral y título de ventana
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    r.render_top_title_bar(
        &mut buf,
        w,
        h,
        "axon",
        Some("main"),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        &theme,
    );
    r.render_sidebar(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &header,
        None,
        &tabs,
        0,
        None,
        None,
        false,
        false,
        false,
        &theme,
    );
    let pane_w = w - SIDEBAR_WIDTH;
    let pane_h = h - top_h - status_h;
    r.render_pane(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        top_h,
        pane_w,
        pane_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("opus-4.6"),
        &[],
        false,
        false,
        &theme,
    );
    write_ppm(&buf, "terminal.ppm");

    // 2. Pestañas superiores: selector de proyecto, pestañas del proyecto y panel a la derecha
    let top_project = crate::renderer::TopBarProject {
        title: "axon".into(),
        initial: 'A',
        color_slot: 4,
        is_loose: false,
        has_news: true,
    };
    let usable_h = h - status_h;
    let rows_end = crate::renderer::ProjectsPanelRects::rows_end(usable_h, projects.len() + 1);
    let h_header = crate::renderer::SidebarHeader {
        on_right: true,
        list_end: rows_end,
        explorer_visible: true,
        changes: changes.clone(),
        ..Default::default()
    };
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    let top_tabs = crate::renderer::TopTabBarRects::compute(w, tabs.len());
    r.render_top_tab_bar(
        &mut buf,
        w,
        h,
        &tabs,
        &top_project,
        false,
        None,
        None,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        &theme,
    );
    let mut panel = vec![theme.background.to_u32_xrgb(); SIDEBAR_WIDTH * h];
    r.render_projects_panel(
        &mut panel,
        SIDEBAR_WIDTH,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &h_header,
        None,
        0,
        false,
        &theme,
    );
    let x0 = w - SIDEBAR_WIDTH;
    for y in crate::renderer::SidebarRects::TOP..usable_h {
        buf[y * w + x0..y * w + x0 + SIDEBAR_WIDTH]
            .copy_from_slice(&panel[y * SIDEBAR_WIDTH..(y + 1) * SIDEBAR_WIDTH]);
    }
    let mut h_explorer =
        crate::explorer::FileExplorer::new(Some(std::env::current_dir().unwrap()), true);
    let h_change_rows = crate::renderer::PanelChangesRects::shown_rows(
        crate::renderer::SidebarRects::content_bottom(usable_h),
        crate::renderer::PanelChangesRects::top(rows_end),
        h_header.changes.len(),
    );
    let er =
        crate::renderer::ProjectsPanelRects::explorer_rect_after(usable_h, rows_end, h_change_rows);
    h_explorer.dock = Some((x0 + er.0, er.1, er.2, er.3));
    r.render_pane(
        &mut buf,
        w,
        h,
        0,
        top_tabs.bar_h,
        w - SIDEBAR_WIDTH,
        h - top_tabs.bar_h - status_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_file_explorer(
        &mut buf,
        w,
        h,
        top_tabs.bar_h,
        status_h,
        &h_explorer,
        &theme,
    );
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("opus-4.6"),
        &[
            crate::renderer::StatusMeter {
                label: "5 h".into(),
                percent: 42.0,
            },
            crate::renderer::StatusMeter {
                label: "Semana".into(),
                percent: 18.0,
            },
        ],
        false,
        false,
        &theme,
    );
    write_ppm(&buf, "horizontal.ppm");

    // 3. Explorador acoplado sobre el terminal
    let mut explorer =
        crate::explorer::FileExplorer::new(Some(std::env::current_dir().unwrap()), true);
    explorer.is_focused = true;
    let change_rows = crate::renderer::PanelChangesRects::shown_rows(
        crate::renderer::SidebarRects::content_bottom(h - status_h),
        crate::renderer::PanelChangesRects::top(header_exp.list_end),
        header_exp.changes.len(),
    );
    explorer.dock = Some(crate::renderer::SidebarRects::explorer_rect_after(
        h - status_h,
        header_exp.list_end,
        change_rows,
    ));
    let explorer_w = 0;
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    r.render_top_title_bar(
        &mut buf,
        w,
        h,
        "axon",
        Some("main"),
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        &theme,
    );
    r.render_sidebar(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &header_exp,
        None,
        &tabs,
        0,
        None,
        None,
        false,
        false,
        false,
        &theme,
    );
    r.render_pane(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        top_h,
        w - SIDEBAR_WIDTH - explorer_w,
        h - top_h - status_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_file_explorer(&mut buf, w, h, top_h, status_h, &explorer, &theme);
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("opus-4.6"),
        &[],
        false,
        true,
        &theme,
    );
    write_ppm(&buf, "explorer.ppm");

    // 4. Ajustes
    let settings = SettingsPageState {
        section: crate::renderer::SettingsSection::Appearance,
        ..Default::default()
    };
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    r.render_top_title_bar(
        &mut buf, w, h, "Ajustes", None, false, false, false, false, false, false, false, &theme,
    );
    r.render_sidebar(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &header,
        None,
        &tabs,
        0,
        None,
        None,
        false,
        false,
        false,
        &theme,
    );
    r.render_settings_page(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        0,
        top_h,
        status_h,
        &cfg,
        &settings,
        &theme,
    );
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        0,
        Some("Ajustes"),
        None,
        None,
        110,
        34,
        AiAgentStatus::None,
        None,
        &[],
        false,
        false,
        &theme,
    );
    write_ppm(&buf, "settings.ppm");

    // 5. Visor de archivos
    let path = std::env::current_dir()
        .unwrap()
        .join("src/renderer/pane.rs");
    let tab = crate::app::TerminalTab::new_file_viewer(90, path);
    let viewer = tab.file_viewer.as_ref().unwrap();
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    r.render_top_title_bar(
        &mut buf,
        w,
        h,
        "pane.rs",
        Some("main"),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        &theme,
    );
    r.render_sidebar(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &header,
        None,
        &tabs,
        0,
        None,
        None,
        false,
        false,
        false,
        &theme,
    );
    r.render_file_viewer(
        &mut buf,
        w,
        h,
        SIDEBAR_WIDTH,
        top_h,
        w - SIDEBAR_WIDTH,
        h - top_h - status_h,
        0,
        true,
        viewer,
        &theme,
    );
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        0,
        Some("~/dev/axon/src/renderer"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::None,
        None,
        &[],
        false,
        false,
        &theme,
    );
    write_ppm(&buf, "viewer.ppm");

    // 6. Panel a la derecha: se dibuja en un búfer propio del ancho del panel y se copia a su sitio
    let header_right = crate::renderer::SidebarHeader {
        on_right: true,
        ..header_exp.clone()
    };
    let mut buf = vec![0u32; w * h];
    buf.fill(theme.background.to_u32_xrgb());
    r.render_top_title_bar(
        &mut buf,
        w,
        h,
        "axon",
        Some("main"),
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        &theme,
    );
    let mut panel = vec![theme.background.to_u32_xrgb(); SIDEBAR_WIDTH * h];
    r.render_sidebar(
        &mut panel,
        SIDEBAR_WIDTH,
        h,
        SIDEBAR_WIDTH,
        status_h,
        &projects,
        &header_right,
        None,
        &tabs,
        0,
        None,
        None,
        false,
        false,
        false,
        &theme,
    );
    let x0 = w - SIDEBAR_WIDTH;
    for y in crate::renderer::SidebarRects::TOP..h - status_h {
        buf[y * w + x0..y * w + x0 + SIDEBAR_WIDTH]
            .copy_from_slice(&panel[y * SIDEBAR_WIDTH..(y + 1) * SIDEBAR_WIDTH]);
    }
    explorer.dock = Some((
        x0 + crate::renderer::SidebarRects::RAIL_W,
        crate::renderer::SidebarRects::explorer_rect_after(
            h - status_h,
            header_exp.list_end,
            change_rows,
        )
        .1,
        SIDEBAR_WIDTH - crate::renderer::SidebarRects::RAIL_W,
        crate::renderer::SidebarRects::explorer_rect_after(
            h - status_h,
            header_exp.list_end,
            change_rows,
        )
        .3,
    ));
    r.render_pane(
        &mut buf,
        w,
        h,
        0,
        top_h,
        w - SIDEBAR_WIDTH,
        h - top_h - status_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_file_explorer(&mut buf, w, h, top_h, status_h, &explorer, &theme);
    r.render_status_bar_with_ai(
        &mut buf,
        w,
        h,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("opus-4.6"),
        &[],
        false,
        true,
        &theme,
    );
    write_ppm(&buf, "right.ppm");

    println!("previews in {}", out.display());
}

#[test]
fn test_status_bar_draws_limit_meters_only_when_given() {
    use crate::renderer::{AiAgentStatus, StatusMeter};

    let config = AxonConfig::load_or_default();
    let mut r = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let theme = crate::theme::Theme::default();
    let (w, h) = (1200usize, 300usize);
    let mut draw = |meters: &[StatusMeter]| {
        let mut buf = vec![theme.background.to_u32_xrgb(); w * h];
        r.render_status_bar_with_ai(
            &mut buf,
            w,
            h,
            0,
            Some("~/dev"),
            Some("main"),
            None,
            100,
            30,
            AiAgentStatus::Claude,
            Some("opus"),
            meters,
            false,
            false,
            &theme,
        );
        buf
    };
    let without = draw(&[]);
    let with = draw(&[StatusMeter {
        label: "5 h".into(),
        percent: 42.0,
    }]);
    assert_ne!(without, with, "los medidores deben dibujar algo");
    // Sin sitio (ventana estrecha) no se dibujan: no pisan la ruta ni la rama
    let mut narrow = vec![theme.background.to_u32_xrgb(); 400 * h];
    let mut narrow_with = narrow.clone();
    let meter = [StatusMeter {
        label: "5 h".into(),
        percent: 42.0,
    }];
    for (buf, meters) in [(&mut narrow, &[][..]), (&mut narrow_with, &meter[..])] {
        r.render_status_bar_with_ai(
            buf,
            400,
            h,
            0,
            Some("~/dev"),
            Some("main"),
            None,
            100,
            30,
            AiAgentStatus::Claude,
            Some("opus"),
            meters,
            false,
            false,
            &theme,
        );
    }
    assert_eq!(narrow, narrow_with);
}
