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
        let mut canvas = Canvas::new();
        canvas.begin(w as u32, h as u32);
        r.render_pane(&mut canvas, 0, 0, w, h, &term, &theme, true, true, false); // calienta cachés
        let n = if cfg!(debug_assertions) { 4 } else { 200 };
        let t = Instant::now();
        for _ in 0..n {
            canvas.begin(w as u32, h as u32);
            r.render_pane(&mut canvas, 0, 0, w, h, &term, &theme, true, true, false);
            rasterize(canvas.scene(), &mut buf);
        }
        let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t = Instant::now();
        for _ in 0..n {
            canvas.begin(w as u32, h as u32);
            r.render_pane(&mut canvas, 0, 0, w, h, &term, &theme, true, true, false);
        }
        let record = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t = Instant::now();
        for _ in 0..n {
            buf.fill(theme.background.to_u32_xrgb());
        }
        let fill = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("render_pane {w}x{h} ({cols}x{rows} celdas): {per:.2} ms/frame (grabar+rasterizar; solo grabar: {record:.2} ms); buffer.fill: {fill:.2} ms");
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

        let mut canvas = Canvas::new();
        canvas.begin(w as u32, h as u32);
        r.render_file_viewer(
            &mut canvas,
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
            canvas.begin(w as u32, h as u32);
            r.render_file_viewer(
                &mut canvas,
                190,
                0,
                w - 190,
                h - 26,
                0,
                true,
                &fv_rs,
                &theme,
            );
            rasterize(canvas.scene(), &mut buf);
        }
        let rs_ms = t_rs.elapsed().as_secs_f64() * 1000.0 / n as f64;

        r.render_file_viewer(
            &mut canvas,
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
            canvas.begin(w as u32, h as u32);
            r.render_file_viewer(
                &mut canvas,
                190,
                0,
                w - 190,
                h - 26,
                0,
                true,
                &fv_md,
                &theme,
            );
            rasterize(canvas.scene(), &mut buf);
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
        let mut canvas = Canvas::new();
        canvas.begin(w as u32, h as u32);
        r.render_sidebar(
            &mut canvas,
            190,
            26,
            &crate::renderer::SidebarHeader::default(),
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
            &mut canvas,
            &tabs,
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
            &mut canvas,
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
        r.render_settings_page(&mut canvas, 190, 0, 0, 26, &cfg, &settings_state, &theme);

        let n = if cfg!(debug_assertions) { 3 } else { 120 };
        let t_chrome = Instant::now();
        for _ in 0..n {
            canvas.begin(w as u32, h as u32);
            r.render_sidebar(
                &mut canvas,
                190,
                26,
                &crate::renderer::SidebarHeader::default(),
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
                &mut canvas,
                &tabs,
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
                &mut canvas,
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
            rasterize(canvas.scene(), &mut buf);
        }
        let chrome_ms = t_chrome.elapsed().as_secs_f64() * 1000.0 / n as f64;

        let t_settings = Instant::now();
        for _ in 0..n {
            canvas.begin(w as u32, h as u32);
            r.render_settings_page(&mut canvas, 190, 0, 0, 26, &cfg, &settings_state, &theme);
            rasterize(canvas.scene(), &mut buf);
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

    let mut canvas = Canvas::new();
    canvas.begin(800, 600);
    renderer.render(
        &mut canvas,
        0,
        &terminal,
        &terminal.theme,
        false,
        false,
        false,
    );
    let buffer = canvas.snapshot();

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
    let mut canvas = Canvas::new();
    canvas.begin(1000, 600);
    renderer.render_sidebar(
        &mut canvas,
        190,
        26,
        &crate::renderer::SidebarHeader::default(),
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
        canvas.snapshot()[50 * 1000 + SidebarRects::RAIL_W + 4],
        sidebar_bg_pixel,
        "El fondo de la columna de sesiones debe coincidir con sidebar_bg"
    );
    assert_eq!(
        canvas.snapshot()[50 * 1000 + SidebarRects::RAIL_W - 2],
        default_theme.background.to_u32_xrgb(),
        "El riel usa el fondo del lienzo"
    );
    assert_eq!(
        canvas.snapshot()[50 * 1000 + 189],
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
    renderer.render(&mut canvas, 190, &term, &term.theme, false, false, false);

    let term_bg_pixel = term.theme.background.to_u32_xrgb();
    assert_eq!(
        canvas.snapshot()[50 * 1000 + 195],
        term_bg_pixel,
        "El área de terminal después del offset debe tener fondo del tema"
    );

    // 4. Probar renderizado del botón flotante expand cuando el sidebar está oculto
    renderer.render_floating_sidebar_toggle(&mut canvas, 0, true, false, &default_theme);
    let float_btn_bg = default_theme.surface_hover().to_u32_xrgb();
    let float_y = 600 - 30 - 8 + 5;
    assert_eq!(
        canvas.snapshot()[float_y * 1000 + 10],
        float_btn_bg,
        "El botón flotante debe renderizarse correctamente"
    );

    // 5. Probar renderizado de la pestaña de Ajustes
    let settings_state = crate::renderer::SettingsPageState::default();
    renderer.render_settings_page(
        &mut canvas,
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
    assert_eq!(label_none, "Local");
    assert_eq!(fg_none, theme.text_muted());

    let (icon_agy, label_agy, fg_agy) = crate::renderer::TerminalRenderer::status_bar_badge_info(
        AiAgentStatus::Agy,
        None,
        Some("Gemini 3.8 Flash (Medium)"),
        &theme,
    );
    assert_eq!(icon_agy, crate::ui::LucideIcon::Sparkles);
    assert_eq!(label_agy, "AGY Gemini 3.8 Flash (Medium)");
    assert_eq!(fg_agy, theme.ansi[6]);

    // El badge muestra el nombre legible del modelo, nunca el identificador crudo.
    for (raw, expected) in [
        ("opus-4.6", "Claude Opus 4.6"),
        ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
        ("claude-opus-4-6-20251101", "Claude Opus 4.6"),
        ("Sonnet 4.5", "Claude Sonnet 4.5"),
    ] {
        let (_, label, _) = crate::renderer::TerminalRenderer::status_bar_badge_info(
            AiAgentStatus::Claude,
            None,
            Some(raw),
            &theme,
        );
        assert_eq!(label, expected, "modelo `{raw}`");
    }

    // Validar que render_status_bar cubre todo el ancho inferior (0..width)
    let config = AxonConfig::load_or_default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let theme = crate::theme::Theme::default();
    let sidebar_offset = 180;
    let mut canvas = Canvas::new();
    canvas.begin(1000, 600);
    canvas.clear(Color::from_u32_xrgb(0xCAFEBABE)); // valor centinela: lo no dibujado se nota
    renderer.render_status_bar(
        &mut canvas,
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
    let buffer = canvas.snapshot();
    let status_bg = theme.bar_bg().to_u32_xrgb();
    assert_eq!(
        buffer[(600 - 28) * 1000 + 5],
        status_bg,
        "El status bar debe empezar en su primera fila, sin hairline sobre el lienzo"
    );
    assert_eq!(
        buffer[(600 - 22) * 1000 + 5],
        status_bg,
        "El fondo del status bar debe dibujarse a todo lo ancho"
    );
    assert_eq!(
        buffer[(600 - 22) * 1000 + sidebar_offset + 5],
        status_bg,
        "El fondo del status bar debe dibujarse en el área de trabajo"
    );
}

#[test]
fn test_top_tab_bar_geometry_and_layout() {
    use crate::renderer::TopTabBarRects;

    let rects = TopTabBarRects::compute(1200, 3);
    assert_eq!(rects.bar_h, 38);
    // Las pestañas empiezan tras la marca
    let left = TopTabBarRects::BRAND_W;
    assert_eq!(rects.tabs_start_x, left);
    assert_eq!(rects.tab_w, 177);

    let tab0 = rects.tab_rect(0);
    assert_eq!(tab0, (left, 0, 177, 38));

    let close0 = rects.close_btn_rect(0);
    assert_eq!(close0.0, left + 177 - 30);
    assert_eq!(close0.1, 7);
    assert_eq!(close0.2, 24);
    assert_eq!(close0.3, 24);

    // Plus button debe situarse después de las 3 pestañas
    assert!(rects.plus_btn.0 > tab0.0 + (3 * 177));
    // Acciones (buscar · panel · ajustes) y controles de ventana al extremo derecho
    assert_eq!(rects.settings_btn.0, 1200 - 178);
    assert_eq!(rects.files_btn.0, 1200 - 212);
    assert_eq!(rects.search_btn.0, 1200 - 246);
    assert_eq!(rects.win_min_btn.0, 1200 - 138);
    assert_eq!(rects.settings_btn.3, TopTabBarRects::ACTION);
    assert!(rects.search_btn.0 + rects.search_btn.2 <= rects.files_btn.0);
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
    let mut canvas = Canvas::new();
    canvas.begin(width as u32, height as u32);
    renderer.render_pane(
        &mut canvas,
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
        canvas.begin(width as u32, height as u32);
        renderer.render_pane(
            &mut canvas,
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
        rasterize(canvas.scene(), &mut buffer);
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
        let mut canvas = Canvas::new();
        canvas.begin(w as u32, h as u32);
        let state = SettingsPageState {
            scroll_y: scroll,
            ..SettingsPageState::default()
        };
        renderer.render_settings_page(&mut canvas, 190, 0, 0, status, &config, &state, &theme);
        canvas.snapshot()
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
    let mut canvas_0 = test_support::new_canvas(w, h);
    r.render_pane(&mut canvas_0, 0, 0, w, h, &term, &theme, true, true, false);
    let buf_with = canvas_0.snapshot();

    // Sin ligaduras
    r.ligatures = false;
    let mut canvas_1 = test_support::new_canvas(w, h);
    r.render_pane(&mut canvas_1, 0, 0, w, h, &term, &theme, true, true, false);
    let buf_without = canvas_1.snapshot();

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

/// Escenas de referencia de la interfaz, en orden fijo: `(nombre, framebuffer XRGB de w x h)`.
/// Las usa `preview_ui_scenes` (PPM y PNG) y el test de hashes `test_golden_scene_hashes`.
pub(super) fn build_scenes(w: usize, h: usize) -> Vec<(&'static str, Vec<u32>)> {
    use crate::app::SIDEBAR_WIDTH;
    use crate::renderer::{SettingsPageState, SidebarTabItem};
    use crate::theme::Theme;

    let cfg = AxonConfig::default();
    let theme = Theme::default();
    let mut r = TerminalRenderer::new(&cfg, 1.0);
    let status_h = crate::renderer::StatusBarRects::BAR_H;
    let top_h = crate::renderer::TopTitleBarRects::compute(w).bar_h;

    // Escenas deterministas: fase fija del spinner y árbol de archivos fijo en vez del directorio de trabajo.
    super::test_support::set_spinner_phase(Some(0));
    let fixture = super::test_support::fixture_root();

    let mut scenes: Vec<(&'static str, Vec<u32>)> = Vec::new();
    let mut push_scene = |buf: &[u32], name: &'static str| scenes.push((name, buf.to_vec()));

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

    let header = crate::renderer::SidebarHeader {
        title: "Sesiones".into(),
        branch: Some("main".into()),
        path: Some("C:\\Users\\dev\\axon".into()),
        on_right: false,
        list_end: crate::renderer::SidebarRects::list_end(h - status_h, 3, false),
    };
    let header_exp = crate::renderer::SidebarHeader {
        list_end: crate::renderer::SidebarRects::list_end(h - status_h, 3, true),
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
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header,
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
        &mut canvas,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
        &[],
        false,
        false,
        &theme,
    );
    push_scene(&canvas.snapshot(), "terminal-vertical");

    // 2. Pestañas superiores: marca, pestañas y panel a la derecha
    let usable_h = h - status_h;
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    let top_tabs = crate::renderer::TopTabBarRects::compute(w, tabs.len());
    r.render_top_tab_bar(
        &mut canvas,
        &tabs,
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
    let x0 = w - SIDEBAR_WIDTH;
    let sb_top = crate::renderer::SidebarRects::TOP;
    canvas.fill_rect(
        (0, 0, w, h),
        x0,
        sb_top,
        SIDEBAR_WIDTH,
        usable_h - sb_top,
        theme.background,
    );
    canvas.with_offset(
        x0 as i32,
        0,
        (SIDEBAR_WIDTH, h),
        (x0, sb_top, w, usable_h),
        |c| r.render_files_panel(c, SIDEBAR_WIDTH, status_h, true, &theme),
    );
    let mut h_explorer = crate::explorer::FileExplorer::new(Some(fixture.clone()), true);
    let er = crate::renderer::FilesPanelRects::explorer_rect(usable_h);
    h_explorer.dock = Some((x0 + er.0, er.1, er.2, er.3));
    r.render_pane(
        &mut canvas,
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
    r.render_file_explorer(&mut canvas, top_tabs.bar_h, status_h, &h_explorer, &theme);
    r.render_status_bar_with_ai(
        &mut canvas,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
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
    push_scene(&canvas.snapshot(), "tabs-superiores");

    // 3. Explorador acoplado sobre el terminal
    let mut explorer = crate::explorer::FileExplorer::new(Some(fixture.clone()), true);
    explorer.is_focused = true;
    explorer.dock = Some(crate::renderer::SidebarRects::explorer_rect_after(
        h - status_h,
        header_exp.list_end,
    ));
    let explorer_w = 0;
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header_exp,
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
        &mut canvas,
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
    r.render_file_explorer(&mut canvas, top_h, status_h, &explorer, &theme);
    r.render_status_bar_with_ai(
        &mut canvas,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
        &[],
        false,
        true,
        &theme,
    );
    push_scene(&canvas.snapshot(), "explorador-acoplado");

    // 4. Ajustes
    let settings = SettingsPageState {
        section: crate::renderer::SettingsSection::Appearance,
        ..Default::default()
    };
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
        "Ajustes",
        None,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        0,
        top_h,
        status_h,
        &cfg,
        &settings,
        &theme,
    );
    r.render_status_bar_with_ai(
        &mut canvas,
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
    push_scene(&canvas.snapshot(), "ajustes");

    // 5. Visor de archivos
    let path = fixture.join("src/renderer/pane.rs");
    let tab = crate::app::TerminalTab::new_file_viewer(90, path);
    let viewer = tab.file_viewer.as_ref().unwrap();
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header,
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
        &mut canvas,
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
        &mut canvas,
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
    push_scene(&canvas.snapshot(), "visor");

    // 6. Panel a la derecha: se dibuja en un búfer propio del ancho del panel y se copia a su sitio
    let header_right = crate::renderer::SidebarHeader {
        on_right: true,
        ..header_exp.clone()
    };
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
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
    let x0 = w - SIDEBAR_WIDTH;
    let sb_top = crate::renderer::SidebarRects::TOP;
    canvas.fill_rect(
        (0, 0, w, h),
        x0,
        sb_top,
        SIDEBAR_WIDTH,
        h - status_h - sb_top,
        theme.background,
    );
    canvas.with_offset(
        x0 as i32,
        0,
        (SIDEBAR_WIDTH, h),
        (x0, sb_top, w, h - status_h),
        |c| {
            r.render_sidebar(
                c,
                SIDEBAR_WIDTH,
                status_h,
                &header_right,
                &tabs,
                0,
                None,
                None,
                false,
                false,
                false,
                &theme,
            )
        },
    );
    explorer.dock = Some((
        x0 + crate::renderer::SidebarRects::RAIL_W,
        crate::renderer::SidebarRects::explorer_rect_after(h - status_h, header_exp.list_end).1,
        SIDEBAR_WIDTH - crate::renderer::SidebarRects::RAIL_W,
        crate::renderer::SidebarRects::explorer_rect_after(h - status_h, header_exp.list_end).3,
    ));
    r.render_pane(
        &mut canvas,
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
    r.render_file_explorer(&mut canvas, top_h, status_h, &explorer, &theme);
    r.render_status_bar_with_ai(
        &mut canvas,
        0,
        Some("~/dev/axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
        &[],
        false,
        true,
        &theme,
    );
    push_scene(&canvas.snapshot(), "panel-derecha");

    // 7. WP1 · modo vertical: marca, píldora centrada y barra de estado del bosquejo
    let wp1_meters = [
        crate::renderer::StatusMeter {
            label: "5 h".into(),
            percent: 42.0,
        },
        crate::renderer::StatusMeter {
            label: "Semana".into(),
            percent: 18.0,
        },
    ];
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    r.render_top_title_bar(
        &mut canvas,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        top_h,
        w - SIDEBAR_WIDTH,
        h - top_h - status_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_status_bar_with_ai(
        &mut canvas,
        SIDEBAR_WIDTH,
        Some("C:\\Users\\dev\\axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
        &wp1_meters,
        false,
        false,
        &theme,
    );
    push_scene(&canvas.snapshot(), "wp1-vertical");

    // 8. WP1 · modo superior: pestañas con icono de tipo, spinner en la que corre y acciones a la derecha
    let wp1_tabs = vec![
        SidebarTabItem {
            index: 0,
            status: Some(crate::renderer::SessionStatus::Idle),
            title: "axon".into(),
            detail: Some("pwsh".into()),
            is_active: true,
            has_unread: false,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
        SidebarTabItem {
            index: 1,
            status: Some(crate::renderer::SessionStatus::Running),
            title: "Claude, rediseño".into(),
            detail: Some("claude".into()),
            is_active: false,
            has_unread: false,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
        SidebarTabItem {
            index: 2,
            status: Some(crate::renderer::SessionStatus::Idle),
            title: "ssh: dev@staging-01".into(),
            detail: Some("~/api".into()),
            is_active: false,
            has_unread: false,
            pane_count: 1,
            is_settings: false,
            is_file_viewer: false,
            is_preview: false,
        },
    ];
    let mut canvas = test_support::new_canvas(w, h);
    canvas.clear(theme.background);
    let wp1_rects = crate::renderer::TopTabBarRects::compute(w, wp1_tabs.len());
    r.render_top_tab_bar(
        &mut canvas,
        &wp1_tabs,
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
    r.render_sidebar(
        &mut canvas,
        SIDEBAR_WIDTH,
        status_h,
        &header,
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
        &mut canvas,
        SIDEBAR_WIDTH,
        wp1_rects.bar_h,
        w - SIDEBAR_WIDTH,
        h - wp1_rects.bar_h - status_h,
        &term,
        &theme,
        true,
        true,
        false,
    );
    r.render_status_bar_with_ai(
        &mut canvas,
        SIDEBAR_WIDTH,
        Some("C:\\Users\\dev\\axon"),
        Some("main"),
        None,
        110,
        34,
        AiAgentStatus::Claude,
        Some("claude-sonnet-5-5"),
        &wp1_meters,
        false,
        false,
        &theme,
    );
    push_scene(&canvas.snapshot(), "wp1-superior");
    // 7. Paneles divididos con cabecera (WP3): agente a la izquierda, shell y SSH a la derecha
    {
        use crate::ui::LucideIcon;
        let content_h = h - top_h - status_h;
        let left_w = 673;
        let right_x = SIDEBAR_WIDTH + left_w + 2;
        let right_w = w - right_x;
        let top_pane_h = 472;
        let header_offset = crate::renderer::PaneHeaderRects::CONTENT_OFFSET;

        let mut agent = crate::terminal::TerminalState::new(62, 30, 2000, theme.clone());
        agent.process_bytes(
            concat!(
                "\x1b[38;5;114m●\x1b[0m Reviso cómo se dibuja la lista de sesiones antes de cambiarla.\r\n",
                "  \x1b[38;5;245mRead(src/renderer/sidebar.rs)\x1b[0m\r\n",
                "  \x1b[38;5;245m474 líneas\x1b[0m\r\n",
                "\r\n",
                "\x1b[38;5;114m●\x1b[0m Cada fila mostrará el estado del agente que corre en ella.\r\n",
                "  \x1b[38;5;245mUpdate(src/renderer/sidebar.rs)\x1b[0m\r\n",
                "  \x1b[38;5;114m+38\x1b[0m \x1b[38;5;203m−12\x1b[0m  fila de sesión con indicador de estado\r\n",
                "\r\n",
                "\x1b[38;5;114m●\x1b[0m Compilo y pruebo antes de seguir.\r\n",
                "  \x1b[38;5;245mBash(cargo test --release sidebar)\x1b[0m\r\n",
                "  test result: ok. 12 passed; 0 failed\r\n",
                "\r\n",
                "\x1b[38;5;176m✳\x1b[0m Ajustando el selector de proyectos… \x1b[38;5;245m(2m 14s, esc para interrumpir)\x1b[0m"
            )
            .as_bytes(),
        );

        let mut shell = crate::terminal::TerminalState::new(40, 18, 2000, theme.clone());
        shell.process_bytes(
            concat!(
                "\x1b[38;5;114m   Compiling\x1b[0m axon v0.9.0\r\n",
                "\x1b[38;5;114m    Finished\x1b[0m `release` profile in 1m 06s\r\n",
                "\x1b[38;5;114m     Running\x1b[0m unittests src\\main.rs\r\n",
                "\r\n",
                "running 206 tests\r\n",
                "test theme::tests::test_default_themes_are_captured ok\r\n",
                "test config::tests::test_theme_migration ok\r\n",
                "test terminal::tests::test_default_theme_palette ok\r\n",
                "\r\n",
                "\x1b[1mtest result: \x1b[38;5;114mok\x1b[0m\x1b[1m. 205 passed; 0 failed\x1b[0m"
            )
            .as_bytes(),
        );

        let mut ssh = crate::terminal::TerminalState::new(40, 12, 2000, theme.clone());
        ssh.process_bytes(
            concat!(
                "\x1b]133;A\x07\x1b[38;5;114mdev@staging-01\x1b[0m:\x1b[38;5;39m~/api\x1b[0m$ systemctl status api\r\n",
                "\x1b[38;5;114m●\x1b[0m api.service - API\r\n",
                "     Active: active (running) since 09:12\r\n"
            )
            .as_bytes(),
        );

        let mut canvas = test_support::new_canvas(w, h);
        canvas.clear(theme.background);
        r.render_top_title_bar(
            &mut canvas,
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
            &mut canvas,
            SIDEBAR_WIDTH,
            status_h,
            &header,
            &tabs,
            0,
            None,
            None,
            false,
            false,
            false,
            &theme,
        );
        type PaneSpec<'a> = (
            usize,
            usize,
            usize,
            usize,
            &'a crate::terminal::TerminalState,
            &'a str,
            &'a str,
            LucideIcon,
            bool,
            bool,
        );
        let panes: [PaneSpec; 3] = [
            (
                SIDEBAR_WIDTH,
                top_h,
                left_w,
                content_h,
                &agent,
                "Rediseño de UI",
                "axon",
                LucideIcon::Sparkles,
                true,
                true,
            ),
            (
                right_x,
                top_h,
                right_w,
                top_pane_h,
                &shell,
                "Pruebas",
                "cargo test",
                LucideIcon::Terminal,
                false,
                false,
            ),
            (
                right_x,
                top_h + top_pane_h,
                right_w,
                content_h - top_pane_h,
                &ssh,
                "Servidor de pruebas",
                "dev@staging-01",
                LucideIcon::Server,
                false,
                false,
            ),
        ];
        for (x, y, pw, ph, term, title, subtitle, icon, focused, agent_pane) in panes {
            let icon_color = if agent_pane {
                theme.accent_color()
            } else {
                theme.text_secondary()
            };
            r.render_pane_header(
                &mut canvas,
                (x, y, pw),
                title,
                Some(subtitle),
                icon,
                icon_color,
                focused,
                false,
                None,
                &theme,
            );
            r.render_pane(
                &mut canvas,
                x,
                y + header_offset,
                pw,
                ph - header_offset,
                term,
                &theme,
                true,
                focused,
                true,
            );
        }
        // Divisores: 2 px entre columnas y entre las dos filas de la derecha
        let divider_color = theme.border_color();
        for (dx, dy, dw, dh) in [
            (right_x - 2, top_h, 2, content_h),
            (right_x, top_h + top_pane_h - 2, right_w, 2),
        ] {
            TerminalRenderer::render_pane_divider(&mut canvas, dx, dy, dw, dh, divider_color);
        }

        r.render_status_bar_with_ai(
            &mut canvas,
            0,
            Some("C:\\Users\\dev\\axon"),
            Some("main"),
            None,
            110,
            34,
            AiAgentStatus::Claude,
            Some("Sonnet 5.5"),
            &[],
            false,
            false,
            &theme,
        );
        push_scene(&canvas.snapshot(), "paneles-divididos");
    }

    // 9. WP4 · bloques OSC 133 como tarjetas con franja de contexto (mock v2-superior-izquierda)
    {
        use crate::ui::LucideIcon;
        let mut cards_term = crate::terminal::TerminalState::new(110, 30, 2000, theme.clone());
        cards_term.current_dir = Some(std::path::PathBuf::from("C:\\Users\\dev\\axon"));
        cards_term.process_bytes(
            concat!(
                "\r\n\r\n",
                "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07cargo test --release\x1b]133;C\x07\r\n",
                "\x1b[38;5;114m   Compiling\x1b[0m axon v0.9.0\r\n",
                "\x1b[38;5;114m    Finished\x1b[0m `release` profile [optimized] in 1m 06s\r\n",
                "     Running unittests src\\main.rs\r\n",
                "\x1b[1mtest result: \x1b[38;5;114mok\x1b[0m\x1b[1m. 205 passed; 0 failed\x1b[0m\r\n",
                "\x1b]133;D;0\x07",
                "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07git status --short\x1b]133;C\x07\r\n",
                "\x1b[38;5;221m M\x1b[0m src\\theme.rs\r\n",
                "\x1b[38;5;221m M\x1b[0m src\\config.rs\r\n",
                "\x1b[38;5;221m M\x1b[0m src\\renderer\\tokens.rs\r\n",
                "\x1b[38;5;114m??\x1b[0m mock\\a.html\r\n",
                "\x1b]133;D;0\x07",
                "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07pnpm build\x1b]133;C\x07\r\n",
                "\x1b[38;5;203merror:\x1b[0m script \"build\" no encontrado en package.json\r\n",
                "\x1b[38;5;245mDid you mean one of these?\x1b[0m\r\n",
                "    build:release  build:debug\r\n",
                "\x1b]133;D;1\x07",
                "\x1b]133;A\x07\x1b[38;5;39mPS C:\\Users\\dev\\axon>\x1b[0m \x1b]133;B\x07claude\x1b]133;C\x07\r\n",
                " \x1b[38;5;114m●\x1b[0m Actualizo la lista de sesiones con el estado del agente.\r\n",
                "   \x1b[38;5;245mUpdate(src/renderer/sidebar.rs)\x1b[0m \x1b[38;5;114m+38\x1b[0m \x1b[38;5;203m-12\x1b[0m\r\n",
            )
            .as_bytes(),
        );
        let durations = [
            std::time::Duration::from_millis(22_900),
            std::time::Duration::from_millis(100),
            std::time::Duration::from_millis(400),
        ];
        for (block, duration) in cards_term.command_blocks.iter_mut().zip(durations) {
            block.finished = block.started.map(|s| s + duration);
        }
        if let Some(running) = cards_term.active_block.as_mut() {
            running.started =
                Some(std::time::Instant::now() - std::time::Duration::from_millis(254_500));
        }

        let mut canvas = test_support::new_canvas(w, h);
        canvas.clear(theme.background);
        let wp4_rects = crate::renderer::TopTabBarRects::compute(w, wp1_tabs.len());
        r.render_top_tab_bar(
            &mut canvas,
            &wp1_tabs,
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
        r.render_sidebar(
            &mut canvas,
            SIDEBAR_WIDTH,
            status_h,
            &header,
            &tabs,
            0,
            None,
            None,
            false,
            false,
            false,
            &theme,
        );
        let pane_x = SIDEBAR_WIDTH;
        let pane_y = wp4_rects.bar_h;
        let pane_w = w - SIDEBAR_WIDTH;
        let pane_h = h - wp4_rects.bar_h - status_h;
        r.render_pane_header(
            &mut canvas,
            (pane_x, pane_y, pane_w),
            "axon",
            Some("cargo test"),
            LucideIcon::Terminal,
            theme.text_secondary(),
            true,
            false,
            None,
            &theme,
        );
        let header_offset = crate::renderer::PaneHeaderRects::CONTENT_OFFSET;
        r.render_pane(
            &mut canvas,
            pane_x,
            pane_y + header_offset,
            pane_w,
            pane_h - header_offset,
            &cards_term,
            &theme,
            true,
            true,
            false,
        );
        r.render_status_bar_with_ai(
            &mut canvas,
            SIDEBAR_WIDTH,
            Some("C:\\Users\\dev\\axon"),
            Some("main"),
            None,
            110,
            30,
            AiAgentStatus::Claude,
            Some("claude-sonnet-5-5"),
            &wp1_meters,
            false,
            false,
            &theme,
        );
        push_scene(&canvas.snapshot(), "bloques-osc133");
    }

    scenes
}

/// Previsualización manual de la interfaz (no forma parte de la suite):
/// `cargo test preview_ui_scenes -- --ignored --nocapture` deja PPM y PNG por escena en `%TEMP%/axon-preview`.
#[test]
#[ignore]
fn preview_ui_scenes() {
    let out = std::env::var_os("AXON_PREVIEW_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("axon-preview"));
    std::fs::create_dir_all(&out).unwrap();
    let (w, h) = (1440usize, 900usize);

    for (name, buf) in build_scenes(w, h) {
        let mut data = format!("P6\n{} {}\n255\n", w, h).into_bytes();
        for px in &buf {
            data.push(((px >> 16) & 0xFF) as u8);
            data.push(((px >> 8) & 0xFF) as u8);
            data.push((px & 0xFF) as u8);
        }
        std::fs::write(out.join(format!("{name}.ppm")), data).unwrap();
        let rgb: Vec<u8> = buf
            .iter()
            .flat_map(|px| {
                [
                    ((px >> 16) & 0xFF) as u8,
                    ((px >> 8) & 0xFF) as u8,
                    (px & 0xFF) as u8,
                ]
            })
            .collect();
        image::RgbImage::from_raw(w as u32, h as u32, rgb)
            .unwrap()
            .save(out.join(format!("{name}.png")))
            .unwrap();
    }
    println!("previews in {}", out.display());
}

/// Regresión visual: el hash de cada escena de referencia debe coincidir con `golden/hashes.txt`.
/// Con `AXON_BLESS=1` reescribe ese archivo en lugar de comparar (ver `test_support`).
#[test]
fn test_golden_scene_hashes() {
    use super::test_support::{bless_requested, frame_hash};
    const GOLDEN: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/renderer/golden/hashes.txt"
    );

    let mut actual = String::new();
    for (name, buf) in build_scenes(1440, 900) {
        actual.push_str(&format!("{name}={:016x}\n", frame_hash(&buf)));
    }

    if bless_requested() {
        std::fs::write(GOLDEN, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(GOLDEN)
        .expect("falta golden/hashes.txt: generarlo con AXON_BLESS=1");
    assert_eq!(
        actual, expected,
        "hashes de escenas distintos (si el cambio es intencional, AXON_BLESS=1)"
    );
}

#[test]
fn test_signature_distance_is_zero_for_identical_and_grows_with_difference() {
    use super::test_support::{signature, signature_distance};
    let (w, h) = (96usize, 96usize);
    let base = vec![0u32; w * h];
    let mut changed = base.clone();
    for y in 0..48 {
        changed[y * w..y * w + 48].fill(0x00FF_FFFF);
    }
    let (a, b) = (signature(&base, w, h), signature(&changed, w, h));
    assert_eq!(a.len(), 4 * 3);
    assert_eq!(signature_distance(&a, &a), 0.0);
    // Un bloque de 4 difiere en 255 por canal: 3 * 255 / 12 valores.
    assert_eq!(signature_distance(&a, &b), 63.75);
}

/// Las firmas de bloques de cada escena deben coincidir con `golden/signatures.bin` (la GPU de la etapa 2
/// se compara contra ellas con tolerancia). Con `AXON_BLESS=1` reescribe el archivo.
/// Formato: por escena, `u8` largo del nombre, nombre UTF-8, `u32` LE largo de la firma, firma.
#[test]
fn test_golden_scene_signatures_match() {
    use super::test_support::{bless_requested, signature, signature_distance};
    const GOLDEN: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/renderer/golden/signatures.bin"
    );

    let actual: Vec<(&str, Vec<u8>)> = build_scenes(1440, 900)
        .into_iter()
        .map(|(name, buf)| (name, signature(&buf, 1440, 900)))
        .collect();

    if bless_requested() {
        let mut out = Vec::new();
        for (name, sig) in &actual {
            out.push(name.len() as u8);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(sig.len() as u32).to_le_bytes());
            out.extend_from_slice(sig);
        }
        std::fs::write(GOLDEN, out).unwrap();
        return;
    }
    let data =
        std::fs::read(GOLDEN).expect("falta golden/signatures.bin: generarlo con AXON_BLESS=1");
    let mut pos = 0;
    let mut take = |n: usize| {
        let s = &data[pos..pos + n];
        pos += n;
        s
    };
    for (name, sig) in &actual {
        let name_len = take(1)[0] as usize;
        assert_eq!(take(name_len), name.as_bytes(), "orden de escenas distinto");
        let len = u32::from_le_bytes(take(4).try_into().unwrap()) as usize;
        let expected = take(len);
        assert_eq!(
            signature_distance(sig, expected),
            0.0,
            "firma de la escena {name} distinta (si es intencional, AXON_BLESS=1)"
        );
    }
    assert_eq!(pos, data.len(), "signatures.bin tiene escenas de más");
}

#[test]
fn test_status_bar_draws_limit_meters_only_when_given() {
    use crate::renderer::{AiAgentStatus, StatusMeter};

    let config = AxonConfig::load_or_default();
    let mut r = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let theme = crate::theme::Theme::default();
    let (w, h) = (1200usize, 300usize);
    let mut draw = |meters: &[StatusMeter]| {
        let mut canvas = test_support::new_canvas(w, h);
        canvas.clear(theme.background);
        r.render_status_bar_with_ai(
            &mut canvas,
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
        canvas.snapshot()
    };
    let without = draw(&[]);
    let with = draw(&[StatusMeter {
        label: "5 h".into(),
        percent: 42.0,
    }]);
    assert_ne!(without, with, "los medidores deben dibujar algo");
    // Sin sitio (ventana estrecha) no se dibujan: no pisan la ruta ni la rama
    let mut narrow = test_support::new_canvas(400, h);
    narrow.clear(theme.background);
    let mut narrow_with = test_support::new_canvas(400, h);
    narrow_with.clear(theme.background);
    let meter = [StatusMeter {
        label: "5 h".into(),
        percent: 42.0,
    }];
    for (canvas, meters) in [(&mut narrow, &[][..]), (&mut narrow_with, &meter[..])] {
        r.render_status_bar_with_ai(
            canvas,
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
    assert_eq!(narrow.snapshot(), narrow_with.snapshot());
}

/// Previsualización manual del panel superior (marco, selección en «Cambios») y del visor en diff:
/// `cargo test preview_panel_card -- --ignored --nocapture` deja PNG en `%TEMP%/axon-preview`.
#[test]
#[ignore]
fn preview_panel_card() {
    use crate::app::SIDEBAR_WIDTH;
    use crate::renderer::SidebarRects;
    use crate::theme::Theme;

    let cfg = AxonConfig::default();
    let theme = Theme::default();
    let mut r = TerminalRenderer::new(&cfg, 1.0);
    let (w, h) = (900usize, 560usize);
    let status_h = crate::renderer::StatusBarRects::BAR_H;
    let out = std::env::var_os("AXON_PREVIEW_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("axon-preview"));
    std::fs::create_dir_all(&out).unwrap();
    let save = |buf: &[u32], name: &str| {
        let rgb: Vec<u8> = buf
            .iter()
            .flat_map(|px| [(px >> 16) as u8, (px >> 8) as u8, *px as u8])
            .collect();
        image::RgbImage::from_raw(w as u32, h as u32, rgb)
            .unwrap()
            .save(out.join(name))
            .unwrap();
    };

    let mut canvas_0 = test_support::new_canvas(w, h);
    canvas_0.clear(theme.background);
    r.render_files_panel(&mut canvas_0, SIDEBAR_WIDTH, status_h, false, &theme);
    let mut explorer =
        crate::explorer::FileExplorer::new(Some(std::env::current_dir().unwrap()), true);
    explorer.is_focused = true;
    explorer.dock = Some(crate::renderer::FilesPanelRects::explorer_rect(
        h - status_h,
    ));
    r.scrollbars_awake = true;
    r.render_file_explorer(
        &mut canvas_0,
        SidebarRects::TOP,
        status_h,
        &explorer,
        &theme,
    );
    explorer.toggle_changes_only();
    explorer.hovered_tab = Some(true);
    let mut canvas_1 = test_support::new_canvas(w, h);
    canvas_1.clear(theme.background);
    r.render_files_panel(&mut canvas_1, SIDEBAR_WIDTH, status_h, false, &theme);
    r.render_file_explorer(
        &mut canvas_1,
        SidebarRects::TOP,
        status_h,
        &explorer,
        &theme,
    );
    let region = (
        0,
        SidebarRects::TOP,
        SIDEBAR_WIDTH,
        h - status_h - SidebarRects::TOP,
    );
    for canvas in [&mut canvas_0, &mut canvas_1] {
        canvas.card(
            region,
            SidebarRects::CARD_INSET,
            SidebarRects::CARD_RADIUS,
            theme.bar_bg(),
        );
    }
    save(&canvas_1.snapshot(), "panel-changes.png");
    save(&canvas_0.snapshot(), "panel-card.png");

    // Visor en modo «Cambios» (diff) de un archivo modificado del repositorio
    {
        let path = std::env::current_dir().unwrap().join("src/app/click.rs");
        let mut viewer = crate::app::TerminalTab::new_file_viewer(1, path.clone())
            .file_viewer
            .unwrap();
        viewer.git_base = crate::explorer::git_head_lines(&path);
        viewer.toggle_diff();
        let mut canvas = test_support::new_canvas(w, h);
        canvas.clear(theme.background);
        r.render_file_viewer(&mut canvas, 0, 0, w, h - status_h, 0, true, &viewer, &theme);
        save(&canvas.snapshot(), "viewer-diff.png");
    }
    println!("PNG en {}", out.display());
}

#[test]
fn test_scrollbar_geometry_is_shared_by_every_list() {
    use crate::renderer::{ScrollbarRects, SettingsPageRects, SidebarRects};

    // Todo cabe: no hay barra
    assert!(ScrollbarRects::vertical(500, 40, 300, 10, 10, 0).is_none());

    // Mismo carril (ancho y X) para cualquier contenido que termine en el mismo borde
    let (track, thumb) = ScrollbarRects::vertical(500, 40, 300, 100, 10, 0).unwrap();
    assert_eq!(track.2, ScrollbarRects::WIDTH);
    assert_eq!(track.0 + track.2 + 2 + SidebarRects::CARD_INSET, 500);
    assert_eq!((thumb.0, thumb.2), (track.0, track.2));

    // El pulgar recorre la pista de arriba abajo y nunca la rebasa
    assert_eq!(thumb.1, track.1);
    let (_, end) = ScrollbarRects::vertical(500, 40, 300, 100, 10, 90).unwrap();
    assert_eq!(end.1 + end.3, track.1 + track.3);
    let (_, past) = ScrollbarRects::vertical(500, 40, 300, 100, 10, 1_000).unwrap();
    assert_eq!(past, end, "un desplazamiento de más se recorta");

    // Con listas enormes el pulgar sigue siendo agarrable
    let (_, tiny) = ScrollbarRects::vertical(500, 40, 300, 1_000_000, 10, 0).unwrap();
    assert!(tiny.3 >= 18);

    // La zona de clic cubre el carril y el margen hasta el borde, pero no la fila de al lado
    assert!(ScrollbarRects::hit(track, 500, track.0 + 2, 100));
    assert!(ScrollbarRects::hit(track, 500, 499, 100));
    assert!(!ScrollbarRects::hit(
        track,
        500,
        track.0.saturating_sub(20),
        100
    ));
    assert!(!ScrollbarRects::hit(track, 500, track.0, track.1 + track.3));

    // Ajustes usa la misma barra
    let settings = SettingsPageRects::scrollbar(
        crate::renderer::SettingsSection::Appearance,
        38,
        900,
        100,
        0,
    );
    if let Some((t, _)) = settings {
        assert_eq!(
            (t.0, t.2),
            (ScrollbarRects::track_x(900), ScrollbarRects::WIDTH)
        );
    }
}

/// Copia del bucle de imagen que había inline en `pane.rs` (sustituido por `Canvas::image`), como referencia.
fn legacy_image(
    buf: &mut [u32],
    w: usize,
    clip: Clip,
    rgba: &[u8],
    (iw, ih): (usize, usize),
    (ix, iy, tw, th): (i32, i32, usize, usize),
) {
    for dy in 0..th {
        let py = iy + dy as i32;
        if py < clip.1 as i32 || py >= clip.3 as i32 {
            continue;
        }
        let sy = dy * ih / th;
        for dx in 0..tw {
            let px = ix + dx as i32;
            if px < clip.0 as i32 || px >= clip.2 as i32 {
                continue;
            }
            let i = (sy * iw + dx * iw / tw) * 4;
            if i + 3 < rgba.len() {
                let (r, g, b, a) = (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]);
                let idx = py as usize * w + px as usize;
                if a == 255 {
                    buf[idx] = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                } else if a > 0 {
                    let d = buf[idx];
                    let bg = Color::from_rgb((d >> 16) as u8, (d >> 8) as u8, d as u8);
                    buf[idx] = bg.blend_over(Color::from_rgb(r, g, b), a);
                }
            }
        }
    }
}

#[test]
fn test_canvas_matches_legacy_primitives() {
    const W: usize = 40;
    const H: usize = 30;
    // Fondo con degradado para que las mezclas con alfa se noten
    let base: Vec<u32> = (0..W * H)
        .map(|i| (i as u32).wrapping_mul(0x010203) & 0xFFFFFF)
        .collect();
    let pair = || (base.clone(), base.clone());
    // Graba con `Canvas` y rasteriza sobre `buf` tal cual está (el degradado, no un búfer a cero)
    let draw = |buf: &mut Vec<u32>, f: &dyn Fn(&mut Canvas)| {
        let mut cv = test_support::new_canvas(W, H);
        f(&mut cv);
        rasterize(cv.scene(), buf);
    };
    let clip: Clip = (5, 4, 30, 22);
    let col = Color::from_rgb(200, 90, 30);

    // Relleno con recorte por dos lados
    let (mut a, mut b) = pair();
    draw(&mut a, &|c| c.fill_rect(clip, 0, 0, 20, 15, col));
    fill_rect(&mut b, W, clip, 0, 0, 20, 15, col.to_u32_xrgb());
    assert_eq!(a, b, "fill_rect");

    // clear
    let (mut a, mut b) = pair();
    draw(&mut a, &|c| c.clear(col));
    b.fill(col.to_u32_xrgb());
    assert_eq!(a, b, "clear");

    // Glifo con alfa parcial pegado al borde del recorte
    let glyph = Rc::new(CachedGlyph {
        width: 4,
        height: 3,
        x_offset: 0,
        y_offset: 0,
        bitmap: vec![0, 64, 128, 255, 255, 200, 100, 1, 10, 0, 255, 77],
    });
    let (mut a, mut b) = pair();
    draw(&mut a, &|c| c.glyph(clip, &glyph, 3, 20, col));
    blit_glyph(&mut b, W, clip, &glyph, 3, 20, col);
    assert_eq!(a, b, "glyph");
    assert_ne!(a, base, "el glifo debe dibujar algo");

    // Redondeado y trazo, incluido un rectángulo que sale del búfer
    for (x, y, w, h) in [(4, 3, 20, 14), (30, 22, 20, 14)] {
        let (mut a, mut b) = pair();
        draw(&mut a, &|cv| {
            cv.rounded_fill(x, y, w, h, 5, col);
            cv.rounded_stroke(x, y, w, h, 5, Color::from_rgb(10, 220, 240));
        });
        crate::ui::draw_rounded_rect(&mut b, W, H, x, y, w, h, 5, col);
        crate::ui::stroke_rounded_rect(&mut b, W, H, x, y, w, h, 5, Color::from_rgb(10, 220, 240));
        assert_eq!(a, b, "rounded {x},{y}");
    }

    // Imagen 2x2 con alfa 0/128/255 + otro píxel semitransparente, escalada y recortada
    let rgba: Vec<u8> = vec![
        255, 0, 0, 255, //
        0, 255, 0, 128, //
        0, 0, 255, 0, //
        255, 255, 255, 40,
    ];
    let (mut a, mut b) = pair();
    let dst = (-3, 2, 16, 12);
    draw(&mut a, &|c| {
        c.image(
            clip,
            ImageRef {
                rgba: &rgba,
                width: 2,
                height: 2,
            },
            dst,
        )
    });
    legacy_image(&mut b, W, clip, &rgba, (2, 2), dst);
    assert_eq!(a, b, "image");
    assert_ne!(a, base, "la imagen debe dibujar algo");

    // rgba truncada: no entra en pánico y no dibuja nada (el bucle antiguo leería a medias)
    let (mut a, _) = pair();
    draw(&mut a, &|c| {
        c.image(
            clip,
            ImageRef {
                rgba: &rgba[..15],
                width: 2,
                height: 2,
            },
            dst,
        )
    });
    assert_eq!(a, base, "image truncada");

    // Tarjeta
    let (mut a, mut b) = pair();
    draw(&mut a, &|c| c.card((2, 2, 30, 24), 3, 4, col));
    crate::ui::frame_card(&mut b, W, H, (2, 2, 30, 24), 3, 4, col);
    assert_eq!(a, b, "card");

    assert_eq!(test_support::new_canvas(W, H).size(), (W, H));
}

#[test]
fn test_draw_helpers_write_through_canvas() {
    const W: usize = 40;
    const H: usize = 30;
    let col = Color::from_rgb(10, 200, 30);
    let px = test_support::with_canvas(W, H, |c| TerminalRenderer::draw_rect(c, 2, 3, 4, 5, col));
    for y in 0..H {
        for x in 0..W {
            let inside = (2..6).contains(&x) && (3..8).contains(&y);
            let want = if inside { col.to_u32_xrgb() } else { 0 };
            assert_eq!(px[y * W + x], want, "({x},{y})");
        }
    }

    // La barra solo se dibuja con el ratón encima o con scroll activo
    let theme = Theme::default();
    let bar = |hovered, awake| {
        test_support::with_canvas(W, H, |c| {
            TerminalRenderer::draw_scrollbar_rects(
                c,
                (30, 2, 6, 26),
                (30, 4, 6, 8),
                hovered,
                awake,
                &theme,
            )
        })
    };
    assert!(bar(false, false).iter().all(|&p| p == 0));
    assert!(bar(true, false).iter().any(|&p| p != 0));
}

#[test]
fn test_with_offset_equals_offscreen_copy() {
    // Real: 60x50. Local (lo que ven las pantallas): 56x100, con el origen en (-4, -20) reales,
    // así que parte del contenido empieza en coordenadas reales negativas.
    const W: usize = 60;
    const H: usize = 50;
    const LW: usize = 56;
    const LH: usize = 100;
    let (dx, dy) = (-4i32, -20i32);
    let clip: Clip = (10, 8, 50, 40);
    let bg = Color::from_rgb(30, 31, 40);
    let col = Color::from_rgb(200, 90, 30);
    let glyph = Rc::new(CachedGlyph {
        width: 4,
        height: 3,
        x_offset: 0,
        y_offset: 0,
        bitmap: vec![0, 64, 128, 255, 255, 200, 100, 1, 10, 0, 255, 77],
    });
    let rgba: Vec<u8> = vec![
        255, 0, 0, 255, //
        0, 255, 0, 128, //
        0, 0, 255, 0, //
        255, 255, 255, 40,
    ];
    // Local y reales: 28 = borde superior del recorte, 60 = borde inferior; x: 14 = izquierdo, 54 = derecho
    let ops = |c: &mut Canvas| {
        assert_eq!(c.size(), (LW, LH));
        c.fill_rect((0, 0, LW, LH), 0, 0, LW, 12, col); // empieza fuera por arriba y por la izquierda
        c.fill_rect((0, 26, 30, 40), 6, 24, 40, 8, Color::from_rgb(10, 200, 30));
        c.rounded_fill(2, 20, 30, 22, 6, Color::from_rgb(80, 120, 220)); // cruza arriba y a la izquierda
        c.rounded_stroke(2, 20, 30, 22, 6, Color::from_rgb(10, 220, 240));
        c.rounded_fill(30, 50, 40, 30, 8, Color::from_rgb(220, 60, 120)); // cruza derecha y abajo
        c.rounded_stroke(30, 50, 40, 30, 8, Color::from_rgb(250, 250, 90));
        c.rounded_fill(10, 80, 20, 6, 0, Color::from_rgb(1, 2, 3)); // radio 0
        c.glyph((0, 0, LW, LH), &glyph, 12, 26, col); // cruza el borde superior del recorte
        c.glyph((0, 0, LW, LH), &glyph, 51, 60, col);
        c.image(
            (0, 0, LW, LH),
            ImageRef {
                rgba: &rgba,
                width: 2,
                height: 2,
            },
            (-2, 30, 24, 18),
        );
    };
    let base: Vec<u32> = (0..W * H)
        .map(|i| (i as u32).wrapping_mul(0x010203) & 0xFFFFFF)
        .collect();

    // Método antiguo: búfer local prerrellenado, mismo dibujo y copia de la ventana
    let mut old = base.clone();
    let mut scratch = vec![bg.to_u32_xrgb(); LW * LH];
    let mut local = test_support::new_canvas(LW, LH);
    ops(&mut local);
    rasterize(local.scene(), &mut scratch);
    for ry in clip.1..clip.3 {
        for rx in clip.0..clip.2 {
            let (lx, ly) = (rx as i32 - dx, ry as i32 - dy);
            if (0..LW as i32).contains(&lx) && (0..LH as i32).contains(&ly) {
                old[ry * W + rx] = scratch[ly as usize * LW + lx as usize];
            }
        }
    }

    // Método nuevo: fondo explícito en la ventana y dibujo desplazado y recortado
    let mut new = base.clone();
    {
        let mut c = test_support::new_canvas(W, H);
        c.fill_rect(
            (0, 0, W, H),
            clip.0,
            clip.1,
            clip.2 - clip.0,
            clip.3 - clip.1,
            bg,
        );
        c.with_offset(dx, dy, (LW, LH), clip, ops);
        assert_eq!(c.size(), (W, H), "with_offset restaura el estado");
        rasterize(c.scene(), &mut new);
    }
    assert_ne!(new, base, "el dibujo debe cambiar algo");
    assert_eq!(old, new);
}

fn test_glyph() -> Rc<CachedGlyph> {
    Rc::new(CachedGlyph {
        width: 4,
        height: 3,
        x_offset: 0,
        y_offset: 0,
        bitmap: vec![0, 64, 128, 255, 255, 200, 100, 1, 10, 0, 255, 77],
    })
}

#[test]
fn test_scene_records_ops_in_order() {
    use scene::Op;
    let col = Color::from_rgb(200, 90, 30);
    let glyph = test_glyph();
    let mut c = test_support::new_canvas(60, 50);
    c.fill_rect((5, 4, 30, 22), 2, 3, 10, 6, col);
    // Dentro de with_offset: coordenadas trasladadas y recorte intersecado con la ventana local
    c.with_offset(10, 20, (30, 20), (0, 0, 60, 50), |c| {
        c.rounded_fill(1, 2, 8, 6, 3, col);
        c.glyph((0, 0, 30, 20), &glyph, 4, 5, col);
    });

    let s = c.scene();
    assert_eq!((s.w, s.h), (60, 50));
    assert_eq!(s.ops.len(), 3, "una operación grabada por llamada");
    assert_eq!(
        s.ops[0],
        Op::Fill {
            clip: (5, 4, 30, 22),
            x: 2,
            y: 3,
            w: 10,
            h: 6,
            color: col
        }
    );
    assert!(
        matches!(
            s.ops[1],
            Op::Rounded {
                clip: (10, 20, 40, 40),
                x: 11,
                y: 22,
                w: 8,
                h: 6,
                radius: 3,
                stroke: false,
                ..
            }
        ),
        "{:?}",
        s.ops[1]
    );
    let Op::Glyph {
        clip,
        glyph: id,
        x,
        y,
        ..
    } = s.ops[2]
    else {
        panic!("la tercera operación debe ser un glifo: {:?}", s.ops[2]);
    };
    assert_eq!((clip, x, y), ((10, 20, 40, 40), 14, 25));
    assert_eq!(s.glyph(id).bitmap, glyph.bitmap);
    assert_eq!(c.size(), (60, 50), "with_offset restaura el estado");
}

#[test]
fn test_scene_dedups_glyphs_by_identity() {
    use scene::Op;
    let col = Color::from_rgb(10, 200, 30);
    let (a, b) = (test_glyph(), test_glyph()); // mismo contenido, identidad distinta
    let mut c = test_support::new_canvas(60, 50);
    let clip = (0, 0, 60, 50);
    c.glyph(clip, &a, 1, 1, col);
    c.glyph(clip, &b, 10, 1, col);
    c.glyph(clip, &a, 20, 1, col);
    c.glyph(clip, &a, 30, 1, col);
    let s = c.scene();
    assert_eq!(s.glyph_count(), 2, "una entrada por Rc distinto");
    let ids: Vec<_> = s
        .ops
        .iter()
        .map(|op| match op {
            Op::Glyph { glyph, .. } => *glyph,
            other => panic!("solo glifos: {other:?}"),
        })
        .collect();
    assert_eq!(ids[0], ids[2]);
    assert_eq!(ids[0], ids[3]);
    assert_ne!(ids[0], ids[1]);
    assert!(Rc::ptr_eq(s.glyph_rc(ids[0]), &a));
    assert!(Rc::ptr_eq(s.glyph_rc(ids[1]), &b));
    assert!(
        Rc::ptr_eq(s.glyph_rc(ids[0]), s.glyph_rc(ids[3])),
        "mismo glifo, mismo Rc"
    );
    assert_eq!(
        Rc::strong_count(&a),
        2,
        "la escena guarda una sola referencia"
    );
}

#[test]
fn test_scene_image_table_copies_rgba_and_skips_invalid() {
    use scene::Op;
    let rgba: Vec<u8> = (0..16).collect(); // 2x2
    let mut c = test_support::new_canvas(40, 30);
    let clip = (0, 0, 40, 30);
    fn img(rgba: &[u8], width: u32, height: u32) -> ImageRef<'_> {
        ImageRef {
            rgba,
            width,
            height,
        }
    }
    c.image(clip, img(&rgba[..15], 2, 2), (0, 0, 8, 8)); // truncada
    c.image(clip, img(&rgba, 0, 2), (0, 0, 8, 8)); // sin ancho
    c.image(clip, img(&rgba, 2, 2), (3, 4, 0, 8)); // destino vacío
    assert!(c.scene().ops.is_empty(), "nada que dibujar, nada grabado");
    c.image(clip, img(&rgba, 2, 2), (3, 4, 8, 6));
    let s = c.scene();
    let Op::Image {
        image, x, y, w, h, ..
    } = s.ops[0]
    else {
        panic!("se esperaba una imagen: {:?}", s.ops[0]);
    };
    assert_eq!((x, y, w, h), (3, 4, 8, 6));
    let got = s.image(image);
    assert_eq!((got.width, got.height), (2, 2));
    assert_eq!(got.rgba, &rgba[..]);
}

#[test]
fn test_canvas_begin_reuses_capacity_and_clears_state() {
    use scene::Op;
    let col = Color::from_rgb(1, 2, 3);
    let glyph = test_glyph();
    let mut c = test_support::new_canvas(100, 80);
    for i in 0..200 {
        c.fill_rect((0, 0, 100, 80), i % 50, 0, 5, 5, col);
    }
    c.glyph((0, 0, 100, 80), &glyph, 1, 1, col);
    c.with_offset(5, 5, (50, 50), (0, 0, 100, 80), |c| {
        c.fill_rect((0, 0, 50, 50), 0, 0, 3, 3, col)
    });
    assert_eq!(c.scene().ops.len(), 202);
    assert_eq!(Rc::strong_count(&glyph), 2);
    let cap = c.scene().ops.capacity();

    c.begin(64, 48);
    let s = c.scene();
    assert!(s.ops.is_empty());
    assert_eq!((s.w, s.h), (64, 48));
    assert_eq!(s.glyph_count(), 0);
    assert_eq!(
        s.ops.capacity(),
        cap,
        "begin conserva la memoria de la escena"
    );
    assert_eq!(
        Rc::strong_count(&glyph),
        1,
        "begin suelta los glifos de la escena"
    );
    assert_eq!(c.size(), (64, 48));
    // El recorte vuelve a ser la ventana completa: `clear` es un Clear y no un relleno recortado
    c.clear(col);
    assert_eq!(c.scene().ops, [Op::Clear(col)]);
}

mod gpu_device {
    use crate::renderer::gpu::{max_texture_dim_for, requested_driver_from, DriverKind, GpuDevice};
    use windows::Win32::Graphics::Direct3D::*;

    #[test]
    fn test_gpu_device_warp_is_always_available() {
        let gpu = GpuDevice::new_with(Some(DriverKind::Warp), D3D_FEATURE_LEVEL_10_0)
            .expect("WARP debe existir en Windows 10/11");
        assert_eq!(gpu.kind, DriverKind::Warp);
    }

    #[test]
    fn test_gpu_device_feature_level_10_0_is_accepted() {
        let gpu = GpuDevice::new_with(Some(DriverKind::Warp), D3D_FEATURE_LEVEL_10_0).unwrap();
        assert!(gpu.feature_level.0 >= D3D_FEATURE_LEVEL_10_0.0);
        assert!(gpu.max_texture_dim() >= 8192);
        // Solo informativo (`--nocapture`)
        println!("WARP: feature level {:#x}", gpu.feature_level.0);
        match GpuDevice::new(None) {
            Ok(g) => println!(
                "new(None): {:?}, feature level {:#x}",
                g.kind, g.feature_level.0
            ),
            Err(e) => println!("new(None): {e}"),
        }
    }

    #[test]
    fn test_requested_driver_reads_env() {
        assert_eq!(requested_driver_from(Some("warp")), Some(DriverKind::Warp));
        assert_eq!(requested_driver_from(Some("WARP")), Some(DriverKind::Warp));
        assert_eq!(requested_driver_from(Some("hardware")), None);
        assert_eq!(requested_driver_from(None), None);
    }

    #[test]
    fn test_max_texture_dim_by_feature_level() {
        assert_eq!(max_texture_dim_for(D3D_FEATURE_LEVEL_10_0), 8192);
        assert_eq!(max_texture_dim_for(D3D_FEATURE_LEVEL_10_1), 8192);
        assert_eq!(max_texture_dim_for(D3D_FEATURE_LEVEL_11_0), 16384);
        assert_eq!(max_texture_dim_for(D3D_FEATURE_LEVEL_11_1), 16384);
    }

    #[test]
    fn test_gpu_device_can_be_capped_to_feature_level_10_0() {
        let gpu = GpuDevice::new_at_most(DriverKind::Warp, D3D_FEATURE_LEVEL_10_0)
            .expect("WARP debe aceptar 10_0");
        assert_eq!(gpu.feature_level, D3D_FEATURE_LEVEL_10_0);
        assert_eq!(gpu.max_texture_dim(), 8192);
    }
}

mod gpu_atlas {
    use crate::renderer::gpu::{
        AtlasError, AtlasRect, DriverKind, GlyphAtlas, GpuDevice, ImageTexture, ShelfPacker,
    };
    use crate::renderer::CachedGlyph;
    use std::rc::Rc;
    use windows::core::ComInterface;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;

    fn warp() -> GpuDevice {
        GpuDevice::new_with(Some(DriverKind::Warp), D3D_FEATURE_LEVEL_10_0).unwrap()
    }

    fn glyph(w: usize, h: usize, fill: impl Fn(usize) -> u8) -> Rc<CachedGlyph> {
        Rc::new(CachedGlyph {
            width: w,
            height: h,
            x_offset: 0,
            y_offset: 0,
            bitmap: (0..w * h).map(fill).collect(),
        })
    }

    /// Copia la textura de `srv` a una staging y devuelve `(bytes sin relleno de fila, ancho, alto)`.
    fn readback(
        gpu: &GpuDevice,
        srv: &ID3D11ShaderResourceView,
        bpp: usize,
    ) -> (Vec<u8>, usize, usize) {
        let res = unsafe { srv.GetResource().unwrap() };
        let tex: ID3D11Texture2D = res.cast().unwrap();
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { tex.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging = None;
        unsafe {
            gpu.device
                .CreateTexture2D(&desc, None, Some(&mut staging))
                .unwrap();
        }
        let staging = staging.unwrap();
        unsafe { gpu.context.CopyResource(&staging, &tex) };
        let mut map = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            gpu.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map))
                .unwrap();
        }
        let (w, h) = (desc.Width as usize, desc.Height as usize);
        let mut out = Vec::with_capacity(w * h * bpp);
        for row in 0..h {
            let src = unsafe {
                std::slice::from_raw_parts(
                    (map.pData as *const u8).add(row * map.RowPitch as usize),
                    w * bpp,
                )
            };
            out.extend_from_slice(src);
        }
        unsafe { gpu.context.Unmap(&staging, 0) };
        (out, w, h)
    }

    /// Bytes R8 de la región `r` del atlas.
    fn region(gpu: &GpuDevice, atlas: &GlyphAtlas, r: AtlasRect) -> Vec<u8> {
        let (all, w, _) = readback(gpu, atlas.srv(), 1);
        let mut out = Vec::new();
        for y in r.y as usize..(r.y + r.h) as usize {
            out.extend_from_slice(&all[y * w + r.x as usize..y * w + (r.x + r.w) as usize]);
        }
        out
    }

    #[test]
    fn test_shelf_packer_fills_row_then_opens_new_shelf() {
        let mut p = ShelfPacker::new(32, 32);
        let a = p.alloc(10, 10).unwrap();
        let b = p.alloc(10, 10).unwrap();
        let c = p.alloc(10, 10).unwrap();
        assert_eq!(a.1, b.1, "los dos primeros comparten fila");
        assert!(b.0 > a.0 + 10, "con 1 px de separación");
        assert!(c.1 >= a.1 + 10, "el tercero abre un estante debajo");
    }

    #[test]
    fn test_shelf_packer_returns_none_when_full() {
        let mut p = ShelfPacker::new(32, 32);
        for _ in 0..4 {
            assert!(p.alloc(15, 15).is_some()); // 2x2 de 16
        }
        assert!(p.alloc(15, 15).is_none());
        assert!(p.alloc(64, 1).is_none(), "más ancho que el atlas");
    }

    #[test]
    fn test_shelf_packer_reset_allows_reuse() {
        let mut p = ShelfPacker::new(16, 16);
        assert_eq!(p.alloc(15, 15), Some((0, 0)));
        assert!(p.alloc(15, 15).is_none());
        p.reset();
        assert_eq!(p.alloc(15, 15), Some((0, 0)));
    }

    #[test]
    fn test_shelf_packer_never_overlaps() {
        let mut p = ShelfPacker::new(128, 128);
        let mut rects = Vec::new();
        for i in 0..400u32 {
            let (w, h) = (3 + i * 7 % 13, 4 + i * 5 % 17);
            if let Some((x, y)) = p.alloc(w, h) {
                assert!(x + w <= 128 && y + h <= 128);
                rects.push((x, y, w, h));
            }
        }
        assert!(rects.len() > 20);
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let apart =
                    a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0 || a.1 + a.3 <= b.1 || b.1 + b.3 <= a.1;
                assert!(apart, "{a:?} solapa con {b:?}");
            }
        }
    }

    #[test]
    fn test_atlas_uploads_glyph_and_reads_back_coverage() {
        let gpu = warp();
        let mut atlas = GlyphAtlas::new(&gpu).unwrap();
        let g = glyph(4, 4, |i| (i * 16) as u8);
        let r = atlas.rect_for(&gpu, &g).unwrap();
        assert_eq!((r.w, r.h), (4, 4));
        assert_eq!(region(&gpu, &atlas, r), g.bitmap);
    }

    #[test]
    fn test_atlas_empty_glyph_reserves_nothing() {
        let gpu = warp();
        let mut atlas = GlyphAtlas::new(&gpu).unwrap();
        let mut truncated = glyph(3, 3, |_| 1);
        Rc::get_mut(&mut truncated).unwrap().bitmap.truncate(4);
        for g in [glyph(0, 5, |_| 1), glyph(5, 0, |_| 1), truncated] {
            let r = atlas.rect_for(&gpu, &g).unwrap();
            assert_eq!(r, AtlasRect::default());
            assert_eq!(Rc::strong_count(&g), 1, "no se retiene");
        }
        let first = atlas.rect_for(&gpu, &glyph(4, 4, |_| 9)).unwrap();
        assert_eq!((first.x, first.y), (0, 0), "no se reservó espacio");
    }

    #[test]
    fn test_atlas_retains_glyph_while_entry_exists() {
        let gpu = warp();
        let mut atlas = GlyphAtlas::new(&gpu).unwrap();
        let g = glyph(4, 4, |_| 7);
        atlas.rect_for(&gpu, &g).unwrap();
        assert_eq!(Rc::strong_count(&g), 2);
        atlas.reset();
        assert_eq!(Rc::strong_count(&g), 1);
    }

    #[test]
    fn test_atlas_same_glyph_returns_same_rect_without_reupload() {
        let gpu = warp();
        let mut atlas = GlyphAtlas::new(&gpu).unwrap();
        let g = glyph(4, 4, |i| i as u8 + 1);
        let a = atlas.rect_for(&gpu, &g).unwrap();
        let other = atlas.rect_for(&gpu, &glyph(4, 4, |_| 3)).unwrap();
        assert_ne!(a, other);
        assert_eq!(atlas.rect_for(&gpu, &g).unwrap(), a);
        assert_eq!(Rc::strong_count(&g), 2, "una sola entrada");
    }

    /// Emula al renderer: ante `Full` vacía el atlas y reintenta una vez. Devuelve los vaciados.
    fn request_all(gpu: &GpuDevice, atlas: &mut GlyphAtlas, glyphs: &[Rc<CachedGlyph>]) -> u32 {
        let mut resets = 0;
        for g in glyphs {
            let r = match atlas.rect_for(gpu, g) {
                Ok(r) => r,
                Err(AtlasError::Full) => {
                    atlas.reset();
                    resets += 1;
                    atlas.rect_for(gpu, g).expect("tras vaciar siempre cabe")
                }
                Err(e) => panic!("{e:?}"),
            };
            assert!(r.w == 16 && r.h == 16, "rect válido y no vacío");
        }
        resets
    }

    #[test]
    fn test_atlas_overflow_never_drops_glyphs() {
        let gpu = warp();
        let glyphs: Vec<_> = (0..4000).map(|i| glyph(16, 16, |_| i as u8)).collect();
        // Tope pequeño: fuerza vaciados
        let mut small = GlyphAtlas::with_limits(&gpu, 64, 128).unwrap();
        assert!(request_all(&gpu, &mut small, &glyphs) > 0);
        let last = small.rect_for(&gpu, glyphs.last().unwrap()).unwrap();
        assert_eq!(region(&gpu, &small, last), glyphs.last().unwrap().bitmap);
        // Tope real: 4 000 glifos de 16x16 caben creciendo, sin vaciar
        let mut big = GlyphAtlas::new(&gpu).unwrap();
        assert_eq!(request_all(&gpu, &mut big, &glyphs), 0);
        // y tras vaciar sigue aceptando
        big.reset();
        assert!(big.rect_for(&gpu, &glyphs[0]).is_ok());
    }

    #[test]
    fn test_atlas_grows_and_keeps_old_rects_valid() {
        let gpu = warp();
        let mut atlas = GlyphAtlas::with_limits(&gpu, 64, 256).unwrap();
        let first = glyph(8, 8, |i| i as u8 + 1);
        let r0 = atlas.rect_for(&gpu, &first).unwrap();
        assert_eq!(atlas.size(), 64);
        // 64x64 admite ~49 glifos de 8x8: se piden más para forzar el crecimiento
        for i in 0..80 {
            atlas.rect_for(&gpu, &glyph(8, 8, |_| i as u8)).unwrap();
        }
        assert!(atlas.size() > 64, "el atlas creció");
        assert_eq!(atlas.rect_for(&gpu, &first).unwrap(), r0);
        assert_eq!(region(&gpu, &atlas, r0), first.bitmap);
    }

    #[test]
    fn test_image_texture_rejects_truncated_zero_and_oversized() {
        let gpu = warp();
        let max = gpu.max_texture_dim();
        assert!(ImageTexture::new(&gpu, &[0; 8], 2, 2).unwrap().is_none());
        assert!(ImageTexture::new(&gpu, &[], 0, 5).unwrap().is_none());
        assert!(ImageTexture::new(&gpu, &[0; 4], max + 1, 1)
            .unwrap()
            .is_none());
        assert!(ImageTexture::new(&gpu, &[0; 4], 1, 1).unwrap().is_some());
    }

    #[test]
    fn test_image_texture_uploads_bgra() {
        let gpu = warp();
        let rgba = [255, 0, 0, 255, 0, 255, 0, 128];
        let img = ImageTexture::new(&gpu, &rgba, 2, 1).unwrap().unwrap();
        assert_eq!(img.size(), (2, 1));
        let (bytes, w, h) = readback(&gpu, img.srv(), 4);
        assert_eq!((w, h), (2, 1));
        assert_eq!(bytes, [0, 0, 255, 255, 0, 255, 0, 128]);
    }
}

mod gpu_pipeline {
    use crate::renderer::gpu::{DriverKind, GpuDevice, Instance, Kind, Pipeline};
    use windows::Win32::Graphics::Direct3D::*;

    #[test]
    fn test_instance_is_32_bytes() {
        assert_eq!(std::mem::size_of::<Instance>(), 32);
    }

    #[test]
    fn test_instance_packs_kind_radius_and_extra() {
        let i = Instance::new(
            Kind::CardCorner,
            [-3, 4, 5, 6],
            [0, 0, 10, 10],
            0xFF11_2233,
            [7, 8],
            9,
            2,
        );
        assert_eq!(i.kind_radius, 5 | (9 << 8) | (2 << 16));
        assert_eq!(i.rect, [-3, 4, 5, 6]);
        assert_eq!(i.color, 0xFF11_2233);
        assert_eq!(i.uv, [7, 8]);
        let g = Instance::new(Kind::Glyph, [0; 4], [0; 4], 0, [0; 2], 0, 0);
        assert_eq!(g.kind_radius, 3);
    }

    #[test]
    fn test_embedded_shaders_match_hlsl_hash() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/renderer/gpu/shaders");
        let hlsl = std::fs::read_to_string(format!("{dir}/quad.hlsl")).unwrap();
        // Mismo criterio que tools/compile-shaders.ps1: finales de línea normalizados a LF
        let tmp = std::env::temp_dir().join(format!("axon-hlsl-{}.txt", std::process::id()));
        std::fs::write(&tmp, hlsl.replace("\r\n", "\n")).unwrap();
        let actual = crate::updater::sha256_hex(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let expected = std::fs::read_to_string(format!("{dir}/quad.hlsl.sha256")).unwrap();
        assert_eq!(
            actual,
            expected.trim(),
            "quad.hlsl cambió: ejecuta `pwsh tools/compile-shaders.ps1`"
        );
    }

    #[test]
    fn test_pipeline_creates_on_warp_at_feature_level_10_0() {
        let gpu = GpuDevice::new_at_most(DriverKind::Warp, D3D_FEATURE_LEVEL_10_0).unwrap();
        assert_eq!(gpu.feature_level, D3D_FEATURE_LEVEL_10_0);
        Pipeline::new(&gpu).expect("los shaders SM 4.0 deben valer en FL 10_0");
    }
}

mod gpu_renderer {
    use crate::renderer::gpu::{
        encode, AtlasError, AtlasRect, Batch, DriverKind, EncodeStop, GpuDevice, GpuRenderer,
        Instance, Kind,
    };
    use crate::renderer::scene::{Op, Scene};
    use crate::renderer::test_support::{self, PARITY_TOLERANCE};
    use crate::renderer::{rasterize, CachedGlyph, ImageRef};
    use crate::theme::Color;
    use std::rc::Rc;
    use windows::Win32::Graphics::Direct3D::*;

    /// (tamaño del búfer, región, inset, radio) de una tarjeta
    type CardCase = ((usize, usize), (usize, usize, usize, usize), usize, usize);

    fn warp() -> GpuDevice {
        GpuDevice::new_with(Some(DriverKind::Warp), D3D_FEATURE_LEVEL_10_0).unwrap()
    }

    fn renderer() -> GpuRenderer {
        GpuRenderer::new(warp()).unwrap()
    }

    fn glyph(w: usize, h: usize, fill: impl Fn(usize) -> u8) -> Rc<CachedGlyph> {
        Rc::new(CachedGlyph {
            width: w,
            height: h,
            x_offset: 0,
            y_offset: 0,
            bitmap: (0..w * h).map(fill).collect(),
        })
    }

    /// Rampa de cobertura 0..=255 con los extremos exactos (0 y 255 cubren las dos ramas de `blit_glyph`).
    fn ramp_glyph(w: usize, h: usize) -> Rc<CachedGlyph> {
        let n = (w * h).max(2) - 1;
        glyph(w, h, move |i| (i * 255 / n) as u8)
    }

    /// Fondo no trivial `0x00RRGGBB`: cambia en las tres componentes y en ambos ejes.
    fn gradient(w: usize, h: usize) -> Vec<u32> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let r = (x * 255 / w.max(1)) as u32;
                let g = (y * 255 / h.max(1)) as u32;
                let b = ((x * 5 + y * 3) % 256) as u32;
                r << 16 | g << 8 | b
            })
            .collect()
    }

    fn rgba_pattern(w: usize, h: usize, alphas: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for i in 0..w * h {
            let a = alphas[i % alphas.len()];
            v.extend_from_slice(&[
                (i * 37 % 256) as u8,
                (255 - i * 11 % 256) as u8,
                (i * 91 % 256) as u8,
                a,
            ]);
        }
        v
    }

    /// Dibuja `scene` con la CPU y con la GPU sobre la misma base y exige diferencia por canal
    /// <= `PARITY_TOLERANCE`. Devuelve la diferencia máxima medida.
    fn assert_parity(name: &str, r: &mut GpuRenderer, scene: &Scene, base: Option<&[u32]>) -> u8 {
        let (w, h) = (scene.w as usize, scene.h as usize);
        let mut cpu = base.map_or_else(|| vec![0u32; w * h], |b| b.to_vec());
        rasterize(scene, &mut cpu);
        let (gpu, gw, gh) = match base {
            Some(b) => r.render_to_pixels_over(scene, b),
            None => r.render_to_pixels(scene),
        }
        .unwrap();
        assert_eq!((gw as usize, gh as usize), (w, h), "{name}: tamaño");
        let mut max = 0u8;
        let mut first_bad = None;
        let mut differing = 0usize;
        for (i, (&c, &g)) in cpu.iter().zip(&gpu).enumerate() {
            let d = (0..3)
                .map(|s| ((c >> (8 * s)) & 255).abs_diff((g >> (8 * s)) & 255) as u8)
                .max()
                .unwrap();
            differing += (d > 0) as usize;
            max = max.max(d);
            if d > PARITY_TOLERANCE && first_bad.is_none() {
                first_bad = Some((i % w, i / w, c, g));
            }
        }
        println!(
            "paridad {name}: diferencia máxima {max}, {differing} píxeles distintos de {}",
            w * h
        );
        if let Some((x, y, c, g)) = first_bad {
            panic!("{name}: primer píxel fuera de tolerancia en ({x}, {y}): CPU {c:#08x} GPU {g:#08x} (máx {max})");
        }
        max
    }

    fn kind_of(i: &Instance) -> u32 {
        i.kind_radius & 15
    }

    /// `rect_for` falso: el glifo ocupa un rect de su tamaño en (5, 6).
    fn fake_rect(sc: &Scene, id: crate::renderer::scene::GlyphId) -> Result<AtlasRect, AtlasError> {
        let g = sc.glyph(id);
        Ok(AtlasRect {
            x: 5,
            y: 6,
            w: g.width as u16,
            h: g.height as u16,
        })
    }

    fn encode_all(scene: &Scene, target: (u32, u32)) -> (Vec<Instance>, Vec<Batch>) {
        let (mut out, mut batches) = (Vec::new(), Vec::new());
        let stop = encode(scene, 0, target, &mut out, &mut batches, &mut fake_rect).unwrap();
        assert_eq!(stop, EncodeStop::Done);
        (out, batches)
    }

    // ---- codificación pura -------------------------------------------------------------------

    #[test]
    fn test_encode_preserves_op_order() {
        let (w, h) = (60usize, 40usize);
        let mut c = test_support::new_canvas(w, h);
        let clip = (0, 0, w, h);
        let col = Color::from_rgba(200, 100, 50, 180);
        let g = ramp_glyph(4, 5);
        let rgba = rgba_pattern(2, 2, &[255]);
        c.clear(Color::from_rgb(1, 2, 3));
        c.fill_rect(clip, 1, 2, 10, 10, col);
        c.rounded_fill(5, 5, 20, 12, 4, col);
        c.rounded_stroke(6, 6, 20, 12, 4, col);
        c.glyph(clip, &g, 3, 3, col);
        let img = ImageRef {
            rgba: &rgba,
            width: 2,
            height: 2,
        };
        c.image(clip, img, (10, 10, 8, 8));
        c.card((0, 0, w, h), 2, 4, Color::from_rgb(9, 9, 9));
        let (out, batches) = encode_all(c.scene(), (w as u32, h as u32));
        let expect = [
            Kind::Solid,
            Kind::Solid,
            Kind::RoundedFill,
            Kind::RoundedStroke,
            Kind::Glyph,
            Kind::Image,
            Kind::Solid,
            Kind::Solid,
            Kind::Solid,
            Kind::Solid,
            Kind::CardCorner,
            Kind::CardCorner,
            Kind::CardCorner,
            Kind::CardCorner,
        ];
        let got: Vec<u32> = out.iter().map(kind_of).collect();
        assert_eq!(got, expect.map(|k| k as u32));
        assert_eq!(
            out[0].rect,
            [0, 0, w as i16, h as i16],
            "Clear = destino entero"
        );
        assert_eq!(
            out[4].rect,
            [3, 3, 4, 5],
            "glifo en su sitio, tamaño del bitmap"
        );
        assert_eq!(out[4].uv, [5, 6]);
        assert_eq!(out[5].rect, [10, 10, 8, 8]);
        // La imagen rompe el lote: [sólidos y glifos] [imagen] [tarjeta]
        assert_eq!(batches.len(), 3);
        assert_eq!(batches.iter().map(|b| b.count).sum::<u32>(), 14);
        assert!(batches[1].image.is_some() && batches[0].image.is_none());
    }

    #[test]
    fn test_encode_card_emits_corners_and_margins() {
        let outer = Color::from_rgb(10, 20, 30);
        const SENTINEL: u32 = 0x00FF_FFFF;
        // (tamaño del búfer, región, inset, radio)
        let cases: [CardCase; 7] = [
            ((60, 50), (0, 0, 60, 50), 6, 10),
            ((100, 80), (60, 40, 100, 80), 6, 10), // media tarjeta fuera del búfer
            ((100, 80), (95, 70, 50, 50), 3, 8),   // casi toda fuera
            ((60, 50), (0, 0, 60, 50), 0, 8),      // sin margen: solo esquinas
            ((60, 50), (0, 0, 60, 50), 6, 0),      // sin esquinas
            ((60, 50), (10, 10, 12, 12), 6, 4),    // w <= 2·inset: no hace nada
            ((60, 50), (80, 80, 20, 20), 2, 4),    // fuera por completo
        ];
        for (n, ((bw, bh), region, inset, radius)) in cases.into_iter().enumerate() {
            let mut c = test_support::new_canvas(bw, bh);
            c.card(region, inset, radius, outer);
            let mut cpu = vec![SENTINEL; bw * bh];
            rasterize(c.scene(), &mut cpu);
            let (out, _) = encode_all(c.scene(), (bw as u32, bh as u32));

            let mut solid = vec![0u8; bw * bh];
            let mut corner = vec![false; bw * bh];
            let mut corners = 0;
            for i in &out {
                let [x, y, w, h] = i.rect;
                let (x, y, w, h) = (x as i32, y as i32, w as i32, h as i32);
                let is_solid = kind_of(i) == Kind::Solid as u32;
                if !is_solid {
                    assert_eq!(kind_of(i), Kind::CardCorner as u32, "caso {n}");
                    corners += 1;
                }
                for py in y..y + h {
                    for px in x..x + w {
                        if px < 0 || py < 0 || px >= bw as i32 || py >= bh as i32 {
                            assert!(!is_solid, "caso {n}: sólido fuera del búfer ({px}, {py})");
                            continue;
                        }
                        let p = py as usize * bw + px as usize;
                        if is_solid {
                            solid[p] += 1;
                        } else {
                            corner[p] = true;
                        }
                    }
                }
            }
            for p in 0..bw * bh {
                if corner[p] {
                    assert_eq!(solid[p], 0, "caso {n}: esquina y margen se pisan en {p}");
                    continue;
                }
                let changed = cpu[p] != SENTINEL;
                assert_eq!(
                    solid[p],
                    changed as u8,
                    "caso {n}: píxel {p} ({}, {})",
                    p % bw,
                    p / bw
                );
            }
            let tiny = region.2 <= inset * 2 || region.3 <= inset * 2;
            let outside = region.0 >= bw || region.1 >= bh;
            if tiny || outside {
                assert!(out.is_empty(), "caso {n}: tarjeta que no se pinta");
            }
            if n == 0 {
                assert_eq!(corners, 4);
                let corner_rects: Vec<_> = out
                    .iter()
                    .filter(|i| kind_of(i) == Kind::CardCorner as u32)
                    .map(|i| {
                        (
                            i.rect,
                            (i.kind_radius >> 16) & 3,
                            (i.kind_radius >> 8) & 255,
                            i.color,
                        )
                    })
                    .collect();
                let col = 0xFF00_0000 | (10 << 16) | (20 << 8) | 30;
                assert_eq!(
                    corner_rects,
                    [
                        ([6, 6, 10, 10], 0, 10, col),
                        ([44, 6, 10, 10], 1, 10, col),
                        ([6, 34, 10, 10], 2, 10, col),
                        ([44, 34, 10, 10], 3, 10, col),
                    ]
                );
            }
            if radius == 0 {
                assert_eq!(corners, 0, "caso {n}");
            }
        }
    }

    #[test]
    fn test_encode_skips_zero_sized_ops() {
        let mut s = Scene::default();
        s.reset(50, 50);
        let col = Color::from_rgba(1, 2, 3, 200);
        let clip = (0, 0, 50, 50);
        let empty = glyph(0, 0, |_| 0);
        let off = ramp_glyph(3, 3);
        let eid = s.intern_glyph(&empty);
        let oid = s.intern_glyph(&off);
        let img = rgba_pattern(1, 1, &[255]);
        let iid = s.push_image(&ImageRef {
            rgba: &img,
            width: 1,
            height: 1,
        });
        s.ops.extend([
            Op::Fill {
                clip,
                x: 1,
                y: 1,
                w: 0,
                h: 5,
                color: col,
            },
            Op::Fill {
                clip,
                x: 1,
                y: 1,
                w: 5,
                h: 0,
                color: col,
            },
            Op::Fill {
                clip: (10, 10, 10, 20),
                x: 0,
                y: 0,
                w: 40,
                h: 40,
                color: col,
            }, // recorte vacío
            Op::Fill {
                clip,
                x: -20,
                y: 0,
                w: 10,
                h: 10,
                color: col,
            }, // fuera
            Op::Rounded {
                clip,
                x: 1,
                y: 1,
                w: 0,
                h: 9,
                radius: 2,
                color: col,
                stroke: false,
            },
            Op::Rounded {
                clip,
                x: 1,
                y: 1,
                w: 9,
                h: 9,
                radius: 2,
                color: Color::from_rgba(1, 2, 3, 0),
                stroke: true,
            },
            Op::Glyph {
                clip,
                glyph: eid,
                x: 5,
                y: 5,
                color: col,
            },
            Op::Glyph {
                clip,
                glyph: oid,
                x: 60,
                y: 5,
                color: col,
            }, // fuera del recorte: ni se sube
            Op::Image {
                clip,
                image: iid,
                x: 0,
                y: 0,
                w: 0,
                h: 4,
            },
            Op::Image {
                clip,
                image: iid,
                x: 0,
                y: 0,
                w: 4,
                h: 0,
            },
        ]);
        let mut calls = 0;
        let (mut out, mut batches) = (Vec::new(), Vec::new());
        let stop = encode(&s, 0, (50, 50), &mut out, &mut batches, &mut |sc, id| {
            calls += 1;
            fake_rect(sc, id)
        })
        .unwrap();
        assert_eq!(stop, EncodeStop::Done);
        assert!(out.is_empty(), "{out:?}");
        assert!(batches.is_empty());
        assert_eq!(
            calls, 0,
            "ni siquiera se pide hueco para glifos invisibles o vacíos"
        );
        // Un glifo cuyo rect del atlas es vacío tampoco dibuja
        s.ops.clear();
        s.ops.push(Op::Glyph {
            clip,
            glyph: oid,
            x: 5,
            y: 5,
            color: col,
        });
        let mut none = |_: &Scene, _| Ok(AtlasRect::default());
        let stop = encode(&s, 0, (50, 50), &mut out, &mut batches, &mut none).unwrap();
        assert_eq!(stop, EncodeStop::Done);
        assert!(out.is_empty());
    }

    #[test]
    fn test_encode_groups_batches_by_texture() {
        let mut s = Scene::default();
        s.reset(80, 80);
        let col = Color::from_rgb(9, 9, 9);
        let clip = (0, 0, 80, 80);
        let g = ramp_glyph(3, 3);
        let gid = s.intern_glyph(&g);
        let px = rgba_pattern(2, 2, &[255]);
        let img = ImageRef {
            rgba: &px,
            width: 2,
            height: 2,
        };
        let (a, b) = (s.push_image(&img), s.push_image(&img));
        let fill = Op::Fill {
            clip,
            x: 0,
            y: 0,
            w: 5,
            h: 5,
            color: col,
        };
        let glyph_op = Op::Glyph {
            clip,
            glyph: gid,
            x: 1,
            y: 1,
            color: col,
        };
        let im = |image| Op::Image {
            clip,
            image,
            x: 0,
            y: 0,
            w: 4,
            h: 4,
        };
        s.ops.extend([
            fill,
            glyph_op,
            fill, // 3 sin imagen (el atlas no rompe el lote)
            im(a),
            im(a),    // 2 de la misma imagen
            fill,     // 1
            im(b),    // 1 de otra imagen
            glyph_op, // 1
        ]);
        let (out, batches) = encode_all(&s, (80, 80));
        assert_eq!(out.len(), 8);
        let summary: Vec<_> = batches
            .iter()
            .map(|b| (b.first, b.count, b.image))
            .collect();
        assert_eq!(
            summary,
            [
                (0, 3, None),
                (3, 2, Some(a)),
                (5, 1, None),
                (6, 1, Some(b)),
                (7, 1, None)
            ]
        );
    }

    #[test]
    fn test_encode_stops_on_atlas_full_and_resumes() {
        let (w, h) = (80usize, 20usize);
        let mut c = test_support::new_canvas(w, h);
        let clip = (0, 0, w, h);
        let col = Color::from_rgb(1, 2, 3);
        let (ga, gb, gc) = (ramp_glyph(3, 3), ramp_glyph(4, 3), ramp_glyph(5, 3));
        c.fill_rect(clip, 0, 0, 5, 5, col);
        c.glyph(clip, &ga, 10, 1, col);
        c.fill_rect(clip, 20, 0, 5, 5, col);
        c.glyph(clip, &gb, 30, 1, col); // operación 3: el atlas «se llena» aquí
        c.glyph(clip, &gc, 40, 1, col);
        let s = c.scene();

        let (whole, _) = encode_all(s, (w as u32, h as u32));
        assert_eq!(whole.len(), 5);

        let mut full_once = true;
        let mut rect = |sc: &Scene, id| {
            if full_once && Rc::ptr_eq(sc.glyph_rc(id), &gb) {
                full_once = false;
                return Err(AtlasError::Full);
            }
            fake_rect(sc, id)
        };
        let (mut out, mut batches) = (Vec::new(), Vec::new());
        let stop = encode(
            s,
            0,
            (w as u32, h as u32),
            &mut out,
            &mut batches,
            &mut rect,
        )
        .unwrap();
        assert_eq!(stop, EncodeStop::AtlasFull { at_op: 3 });
        assert_eq!(
            out.len(),
            3,
            "lo anterior a la operación 3 ya está codificado"
        );
        // El llamador vuelca `out`, vacía el atlas y sigue por la misma operación
        let (mut out2, mut batches2) = (Vec::new(), Vec::new());
        let stop = encode(
            s,
            3,
            (w as u32, h as u32),
            &mut out2,
            &mut batches2,
            &mut rect,
        )
        .unwrap();
        assert_eq!(stop, EncodeStop::Done);
        assert_eq!(out2.len(), 2);
        out.extend(out2);
        assert_eq!(
            format!("{out:?}"),
            format!("{whole:?}"),
            "sin perder ni repetir nada"
        );
    }

    // ---- paridad GPU / CPU (WARP) -----------------------------------------------------------------

    #[test]
    fn test_gpu_parity_fill_with_clip() {
        let (w, h) = (96usize, 64usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        c.fill_rect(full, 3, 4, 30, 20, Color::from_rgb(200, 10, 90));
        // El alfa del color no importa: fill_rect escribe XRGB opaco
        c.fill_rect(full, 20, 10, 40, 30, Color::from_rgba(10, 220, 40, 90));
        c.fill_rect(
            (10, 10, 50, 30),
            0,
            0,
            90,
            60,
            Color::from_rgba(5, 5, 250, 0),
        );
        c.with_offset(-7, -5, (60, 40), (0, 0, 60, 40), |c| {
            c.fill_rect((0, 0, 60, 40), 0, 0, 40, 30, Color::from_rgb(250, 250, 10));
            c.fill_rect((5, 5, 20, 20), 0, 0, 60, 40, Color::from_rgb(1, 2, 3));
        });
        c.fill_rect(full, 90, 60, 20, 20, Color::from_rgb(77, 88, 99)); // sale por la esquina
        c.fill_rect(full, 5, 5, 0, 7, Color::from_rgb(255, 0, 0)); // sin ancho
        assert_parity("fill+clip", &mut r, c.scene(), Some(&base));

        // Clear primero y sin base
        let mut c = test_support::new_canvas(w, h);
        c.clear(Color::from_rgb(12, 34, 56));
        c.fill_rect(full, 1, 1, 10, 10, Color::from_rgb(200, 100, 50));
        assert_parity("clear+fill", &mut r, c.scene(), None);
    }

    #[test]
    fn test_gpu_parity_glyph_coverage() {
        let (w, h) = (96usize, 64usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        let (a, b, d) = (ramp_glyph(9, 7), ramp_glyph(1, 1), ramp_glyph(16, 20));
        let solid = glyph(5, 5, |_| 255);
        let faint = glyph(6, 4, |i| (i % 4) as u8);
        for (i, &(alpha, x, y)) in [(255u8, 2, 3), (128, 15, 3), (40, 28, 3), (0, 41, 3)]
            .iter()
            .enumerate()
        {
            let col = Color::from_rgba(240 - i as u8 * 50, 30 + i as u8 * 60, 200, alpha);
            c.glyph(full, &a, x, y, col);
            c.glyph(full, &b, x, y + 12, col);
            c.glyph(full, &solid, x, y + 20, col);
            c.glyph(full, &faint, x, y + 28, col);
        }
        // Recorte parcial y origen negativo / fuera por la derecha
        c.glyph(
            (60, 30, 75, 45),
            &d,
            58,
            28,
            Color::from_rgba(250, 250, 20, 200),
        );
        c.glyph(full, &d, -6, -8, Color::from_rgba(20, 250, 250, 255));
        c.glyph(full, &d, 88, 50, Color::from_rgba(250, 20, 250, 77));
        c.with_offset(30, 40, (30, 20), (0, 0, 30, 20), |c| {
            c.glyph((0, 0, 30, 20), &a, 25, 15, Color::from_rgba(1, 1, 1, 255));
            c.glyph(
                (2, 2, 12, 12),
                &a,
                -2,
                0,
                Color::from_rgba(255, 255, 255, 128),
            );
        });
        // Mismo glifo apilado: la mezcla en orden importa
        c.glyph(full, &a, 70, 3, Color::from_rgba(255, 0, 0, 100));
        c.glyph(full, &a, 72, 4, Color::from_rgba(0, 255, 0, 100));
        assert_parity("glyph", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_parity_rounded_fill_and_stroke() {
        let (w, h) = (120usize, 80usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let shapes: &[(usize, usize, usize, usize, usize)] = &[
            (3, 3, 30, 20, 6),
            (40, 3, 31, 21, 0), // radio 0
            (80, 3, 7, 5, 2),   // impares
            (95, 3, 1, 1, 3),
            (100, 3, 2, 2, 5),
            (3, 30, 40, 40, 255), // el radio se acota a la mitad
            (50, 30, 25, 9, 4),
            (80, 30, 33, 33, 16),
            (3, 72, 17, 3, 1),
        ];
        let cols = [
            Color::from_rgba(250, 20, 20, 255),
            Color::from_rgba(20, 250, 20, 128),
            Color::from_rgba(20, 20, 250, 60),
            Color::from_rgba(250, 250, 20, 5),
        ];
        for (i, &(x, y, sw, sh, rad)) in shapes.iter().enumerate() {
            c.rounded_fill(x, y, sw, sh, rad, cols[i % cols.len()]);
            c.rounded_stroke(x, y, sw, sh, rad, cols[(i + 1) % cols.len()]);
        }
        c.rounded_fill(10, 10, 10, 10, 3, Color::from_rgba(1, 2, 3, 0)); // invisible
        c.rounded_stroke(10, 10, 10, 10, 3, Color::from_rgba(1, 2, 3, 0));
        // Parcialmente fuera del destino y con recorte
        c.with_offset(-9, -7, (80, 50), (0, 0, 80, 50), |c| {
            c.rounded_fill(0, 0, 30, 20, 8, Color::from_rgba(200, 100, 50, 255));
            c.rounded_stroke(0, 0, 30, 20, 8, Color::from_rgba(10, 100, 250, 255));
        });
        c.with_offset(100, 60, (60, 40), (0, 0, 60, 40), |c| {
            c.rounded_fill(0, 0, 30, 30, 10, Color::from_rgba(200, 100, 250, 200));
            c.rounded_stroke(0, 0, 30, 30, 10, Color::from_rgba(250, 250, 250, 255));
        });
        c.with_offset(60, 50, (30, 20), (0, 0, 20, 12), |c| {
            c.rounded_fill(0, 0, 40, 30, 9, Color::from_rgba(30, 130, 30, 220));
        });
        assert_parity("rounded", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_parity_image_nearest_with_alpha() {
        let (w, h) = (100usize, 70usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        let small = rgba_pattern(4, 3, &[0, 128, 255, 255, 200, 1]);
        let big = rgba_pattern(40, 30, &[255, 128, 0, 255, 255, 77]);
        let opaque = rgba_pattern(5, 5, &[255]);
        fn im(rgba: &[u8], width: u32, height: u32) -> ImageRef<'_> {
            ImageRef {
                rgba,
                width,
                height,
            }
        }
        c.image(full, im(&small, 4, 3), (2, 2, 17, 11)); // ampliada
        c.image(full, im(&big, 40, 30), (30, 2, 7, 5)); // reducida
        c.image(full, im(&big, 40, 30), (40, 2, 40, 30)); // 1:1
        c.image((5, 20, 30, 40), im(&small, 4, 3), (-6, 15, 33, 25)); // recortada y con origen negativo
        c.image(full, im(&opaque, 5, 5), (90, 60, 30, 30)); // sale por la esquina
        c.image(full, im(&opaque, 5, 5), (10, 40, 1, 1));
        c.image(full, im(&small, 4, 3), (50, 40, 3, 20)); // anisótropa
        c.with_offset(60, 45, (40, 25), (0, 0, 40, 25), |c| {
            c.image((0, 0, 40, 25), im(&small, 4, 3), (-3, -2, 20, 14));
        });
        assert_parity("image", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_parity_card() {
        let mut r = renderer();
        let outer = Color::from_rgb(10, 20, 30);
        let cases: [CardCase; 6] = [
            ((60, 50), (0, 0, 60, 50), 6, 10),
            ((100, 80), (60, 40, 100, 80), 6, 10),
            ((100, 80), (5, 5, 90, 70), 4, 14),
            ((60, 50), (0, 0, 60, 50), 0, 8),
            ((60, 50), (0, 0, 60, 50), 6, 0),
            ((60, 50), (10, 10, 12, 12), 6, 4),
        ];
        for (n, ((bw, bh), region, inset, radius)) in cases.into_iter().enumerate() {
            let base = gradient(bw, bh);
            let mut c = test_support::new_canvas(bw, bh);
            c.fill_rect(
                (0, 0, bw, bh),
                0,
                0,
                bw / 2,
                bh,
                Color::from_rgb(200, 180, 20),
            );
            c.card(region, inset, radius, outer);
            assert_parity(&format!("card {n}"), &mut r, c.scene(), Some(&base));
        }
        // Salida translúcida: la CPU ignora el alfa de `outer`
        let base = gradient(60, 50);
        let mut c = test_support::new_canvas(60, 50);
        c.card((0, 0, 60, 50), 6, 10, Color::from_rgba(200, 40, 40, 90));
        assert_parity("card translúcida", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_parity_ops_over_a_gradient_base() {
        let (w, h) = (128usize, 96usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        let g = ramp_glyph(12, 14);
        let small = ramp_glyph(7, 9);
        let px = rgba_pattern(6, 6, &[255, 160, 90, 0, 255, 30]);
        fn img(rgba: &[u8]) -> ImageRef<'_> {
            ImageRef {
                rgba,
                width: 6,
                height: 6,
            }
        }
        // Formas translúcidas solapadas de dos en dos (cada píxel recibe como mucho 2 capas parciales):
        // el resultado depende del orden. La CPU trunca en cada capa y la GPU redondea, así que con
        // 3 o más capas la diferencia pasa de `PARITY_TOLERANCE` aunque no haya ningún error (ver el
        // informe de la tarea 16). Este test busca un orden o una mezcla equivocados, que dan
        // diferencias de decenas.
        for i in 0..4usize {
            let o = i * 31;
            // fila A: relleno translúcido y glifo encima
            c.rounded_fill(
                3 + o,
                5,
                28,
                36,
                9,
                Color::from_rgba(250 - i as u8 * 40, 40 * i as u8, 120, 110),
            );
            c.glyph(
                full,
                &g,
                11 + o as i32,
                14,
                Color::from_rgba(255, 255, 255, 255 - i as u8 * 30),
            );
            // fila B: imagen translúcida y contorno encima
            c.image(full, img(&px), (3 + o as i32, 46, 26, 26));
            c.rounded_stroke(5 + o, 48, 22, 22, 6, Color::from_rgba(255, 220, 0, 200));
            // fila C: dos rellenos que se pisan en 9 px, con glifos fuera del solape
            c.rounded_fill(
                3 + o,
                78,
                40,
                12,
                4,
                Color::from_rgba(30 * i as u8, 60, 90, 120),
            );
            c.glyph(
                full,
                &small,
                5 + o as i32,
                79,
                Color::from_rgba(0, 0, 0, 140),
            );
        }
        c.fill_rect(full, 60, 42, 8, 3, Color::from_rgb(30, 60, 90));
        c.card((0, 0, w, h), 3, 6, Color::from_rgb(8, 8, 12));
        assert_parity("orden sobre degradado", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_atlas_full_splits_batch_without_dropping_glyphs() {
        let (w, h) = (240usize, 120usize);
        // Atlas de 64x64: cabe un puñado de glifos de 9x9 y la escena usa 200 distintos
        let mut r = GpuRenderer::with_atlas_limits(warp(), 32, 64).unwrap();
        let base = gradient(w, h);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        let glyphs: Vec<_> = (0..200usize)
            .map(|n| glyph(9, 9, move |i| ((i * 29 + n * 53) % 256) as u8))
            .collect();
        for (n, g) in glyphs.iter().enumerate() {
            let (x, y) = ((n % 20) as i32 * 11 + 3, (n / 20) as i32 * 11 + 3);
            c.glyph(
                full,
                g,
                x,
                y,
                Color::from_rgba(255 - n as u8, n as u8, 128, 255),
            );
        }
        // Reutiliza los primeros al final: tras varios vaciados vuelven a subirse
        for (n, g) in glyphs.iter().take(20).enumerate() {
            c.glyph(
                full,
                g,
                n as i32 * 11 + 3,
                115 - 9,
                Color::from_rgba(255, 255, 255, 200),
            );
        }
        assert!(c.scene().glyph_count() >= 200);
        assert_parity("atlas lleno", &mut r, c.scene(), Some(&base));
    }

    #[test]
    fn test_gpu_instance_buffer_grows_past_its_initial_capacity() {
        let (w, h) = (200usize, 100usize);
        let mut r = renderer();
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        // Más de 4096 instancias (la capacidad inicial del buffer)
        for n in 0..9000usize {
            let (x, y) = (n % w, (n / w) % h);
            let col = Color::from_rgb((n * 7) as u8, (n * 13) as u8, (n * 29) as u8);
            c.fill_rect(full, x, y, 1, 1, col);
        }
        assert!(c.scene().ops.len() > 4096);
        assert_parity("buffer de instancias", &mut r, c.scene(), None);
    }

    #[test]
    fn test_gpu_glyph_larger_than_atlas_is_skipped_not_looped() {
        let (w, h) = (60usize, 40usize);
        let mut r = GpuRenderer::with_atlas_limits(warp(), 16, 16).unwrap();
        let base = gradient(w, h);
        let full = (0, 0, w, h);
        let (small_a, small_b) = (ramp_glyph(5, 5), ramp_glyph(6, 4));
        let huge = ramp_glyph(20, 20); // >= tope del atlas: no cabe nunca
        let col = Color::from_rgb(250, 250, 10);
        let mut c = test_support::new_canvas(w, h);
        c.glyph(full, &small_a, 2, 2, col);
        c.glyph(full, &huge, 10, 5, col);
        c.glyph(full, &small_b, 40, 2, col);
        c.glyph(full, &huge, 20, 15, col);
        c.glyph(full, &small_a, 2, 30, col);
        let (gpu, ..) = r.render_to_pixels_over(c.scene(), &base).unwrap(); // si bucla, el test no termina
                                                                            // Lo mismo sin los glifos enormes
        let mut c2 = test_support::new_canvas(w, h);
        c2.glyph(full, &small_a, 2, 2, col);
        c2.glyph(full, &small_b, 40, 2, col);
        c2.glyph(full, &small_a, 2, 30, col);
        let mut cpu = base.clone();
        rasterize(c2.scene(), &mut cpu);
        let max = cpu
            .iter()
            .zip(&gpu)
            .map(|(a, b)| {
                (0..3)
                    .map(|s| ((a >> (8 * s)) & 255).abs_diff((b >> (8 * s)) & 255))
                    .max()
                    .unwrap()
            })
            .max()
            .unwrap();
        assert!(
            max <= PARITY_TOLERANCE as u32,
            "los glifos pequeños deben dibujarse (máx {max})"
        );
    }

    #[test]
    fn test_gpu_oversized_scene_is_clamped_not_panicking() {
        let dev = GpuDevice::new_at_most(DriverKind::Warp, D3D_FEATURE_LEVEL_10_0).unwrap();
        assert_eq!(dev.max_texture_dim(), 8192);
        let mut r = GpuRenderer::new(dev).unwrap();
        let (w, h) = (9000usize, 4usize);
        let mut c = test_support::new_canvas(w, h);
        let full = (0, 0, w, h);
        c.clear(Color::from_rgb(10, 20, 30));
        c.fill_rect(full, 8000, 0, 500, 4, Color::from_rgb(200, 100, 50)); // cruza el límite
        c.fill_rect(full, 8300, 0, 300, 4, Color::from_rgb(1, 2, 3)); // fuera del límite
        c.rounded_fill(8100, 0, 200, 4, 2, Color::from_rgba(250, 250, 250, 128));
        let (px, gw, gh) = r.render_to_pixels(c.scene()).unwrap();
        assert_eq!((gw, gh), (8192, 4));
        assert_eq!(px.len(), 8192 * 4);
        let mut cpu = vec![0u32; w * h];
        rasterize(c.scene(), &mut cpu);
        for y in 0..h {
            for x in 0..8192 {
                let (a, b) = (cpu[y * w + x], px[y * 8192 + x]);
                let d = (0..3)
                    .map(|s| ((a >> (8 * s)) & 255).abs_diff((b >> (8 * s)) & 255))
                    .max()
                    .unwrap();
                assert!(
                    d <= PARITY_TOLERANCE as u32,
                    "({x}, {y}): CPU {a:#08x} GPU {b:#08x}"
                );
            }
        }
    }

    #[test]
    fn test_gpu_truncated_image_is_skipped() {
        let (w, h) = (40usize, 30usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let full = (0, 0, w, h);
        let mut c = test_support::new_canvas(w, h);
        let rgba = rgba_pattern(4, 4, &[255]);
        // Truncada y vacía: el lienzo ni las graba
        c.image(
            full,
            ImageRef {
                rgba: &rgba[..20],
                width: 4,
                height: 4,
            },
            (2, 2, 10, 10),
        );
        c.image(
            full,
            ImageRef {
                rgba: &rgba,
                width: 0,
                height: 4,
            },
            (2, 2, 10, 10),
        );
        assert!(c.scene().ops.is_empty());
        let (px, ..) = r.render_to_pixels_over(c.scene(), &base).unwrap();
        assert_eq!(px, base, "no dibuja nada");

        // Más ancha que la textura máxima (8192 con FL 10_0): la GPU la descarta sin fallar
        let dev = GpuDevice::new_at_most(DriverKind::Warp, D3D_FEATURE_LEVEL_10_0).unwrap();
        let mut r = GpuRenderer::new(dev).unwrap();
        let wide = vec![255u8; 8193 * 4];
        let mut c = test_support::new_canvas(w, h);
        c.image(
            full,
            ImageRef {
                rgba: &wide,
                width: 8193,
                height: 1,
            },
            (2, 2, 20, 4),
        );
        c.fill_rect(full, 0, 20, 5, 5, Color::from_rgb(9, 9, 9));
        assert_eq!(c.scene().ops.len(), 2);
        let (px, ..) = r.render_to_pixels_over(c.scene(), &base).unwrap();
        let mut expect = base.clone();
        for y in 20..25 {
            expect[y * w..y * w + 5].fill(0x0009_0909);
        }
        assert_eq!(px, expect, "la imagen descartada no pinta y lo demás sí");
    }

    #[test]
    fn test_gpu_empty_and_truncated_glyphs_draw_nothing() {
        let (w, h) = (30usize, 20usize);
        let mut r = renderer();
        let base = gradient(w, h);
        let full = (0, 0, w, h);
        let empty = glyph(0, 0, |_| 0);
        let wide_empty = glyph(5, 0, |_| 0);
        let truncated = Rc::new(CachedGlyph {
            width: 6,
            height: 6,
            x_offset: 0,
            y_offset: 0,
            bitmap: vec![255; 10],
        });
        let mut c = test_support::new_canvas(w, h);
        for g in [&empty, &wide_empty, &truncated] {
            c.glyph(full, g, 3, 3, Color::from_rgb(255, 255, 255));
        }
        let (px, ..) = r.render_to_pixels_over(c.scene(), &base).unwrap();
        assert_eq!(px, base);
    }
}

mod gpu_surface {
    use crate::renderer::gpu::{DriverKind, GpuSurface, Presented};
    use crate::renderer::scene::Scene;
    use crate::renderer::Canvas;
    use crate::theme::Color;
    use windows::Win32::Foundation::HWND;
    use windows_sys::Win32::Foundation::HWND as SysHWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, CW_USEDEFAULT, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };

    /// Ventana real (oculta) que se destruye al salir del ámbito, también si el test entra en pánico.
    /// Si no se puede crear, el test falla: nunca se salta en silencio.
    struct TestWindow(SysHWND);

    impl TestWindow {
        fn new() -> Self {
            let class_name: Vec<u16> = "AxonGpuSurfaceTestClass\0".encode_utf16().collect();
            unsafe {
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(DefWindowProcW),
                    lpszClassName: class_name.as_ptr(),
                    ..std::mem::zeroed()
                };
                RegisterClassW(&wc); // varios tests la registran: «la clase ya existe» no importa
                let hwnd = CreateWindowExW(
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
                assert!(
                    hwnd != 0 as SysHWND,
                    "no se pudo crear la ventana de prueba"
                );
                TestWindow(hwnd)
            }
        }

        fn hwnd(&self) -> HWND {
            HWND(self.0 as _)
        }

        fn surface(&self, w: u32, h: u32) -> GpuSurface {
            GpuSurface::new(self.hwnd(), w, h).expect("crear la superficie")
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) };
        }
    }

    fn scene(canvas: &mut Canvas, w: u32, h: u32, color: u32) -> &Scene {
        canvas.begin(w, h);
        canvas.clear(Color::from_u32_xrgb(color));
        canvas.scene()
    }

    #[test]
    fn test_surface_resize_to_zero_clamps_to_one() {
        let win = TestWindow::new();
        let mut s = win.surface(64, 48);
        s.resize(0, 0).unwrap(); // ventana minimizada
        assert_eq!(s.size(), (1, 1));
        let mut canvas = Canvas::new();
        assert!(matches!(
            s.present(scene(&mut canvas, 1, 1, 0x336699)),
            Ok(Presented::Ok)
        ));
    }

    #[test]
    fn test_surface_resize_beyond_limit_is_clamped() {
        let win = TestWindow::new();
        let mut s = win.surface(64, 48);
        s.set_max_dim(512);
        s.resize(1000, 300).unwrap();
        assert_eq!(s.size(), (512, 300));
        s.resize(1000, 700).unwrap();
        assert_eq!(s.size(), (512, 512));
    }

    #[test]
    fn test_surface_present_renders_and_resizes() {
        let win = TestWindow::new();
        let mut s = win.surface(64, 48);
        assert_eq!(s.size(), (64, 48));
        let mut canvas = Canvas::new();
        assert!(matches!(
            s.present(scene(&mut canvas, 64, 48, 0xFF8000)),
            Ok(Presented::Ok)
        ));
        s.resize(128, 96).unwrap();
        assert_eq!(s.size(), (128, 96));
        assert!(matches!(
            s.present(scene(&mut canvas, 128, 96, 0x0080FF)),
            Ok(Presented::Ok)
        ));
    }

    #[test]
    fn test_surface_recovers_after_device_loss() {
        let win = TestWindow::new();
        let mut s = win.surface(64, 48);
        let mut canvas = Canvas::new();
        canvas.begin(64, 48);
        canvas.clear(Color::from_u32_xrgb(0x203040));
        canvas.fill_rect((0, 0, 64, 48), 8, 6, 20, 10, Color::from_u32_xrgb(0xE0A010));
        let scene = canvas.scene();
        assert!(matches!(s.present(scene), Ok(Presented::Ok)));
        let before = s.renderer_mut().render_to_pixels(scene).unwrap();

        s.simulate_device_loss();
        assert!(matches!(s.present(scene), Ok(Presented::Recovered)));
        let after = s.renderer_mut().render_to_pixels(scene).unwrap();
        assert_eq!(before, after, "el renderer recreado dibuja igual");
        assert_eq!(s.size(), (64, 48));
        assert!(matches!(s.present(scene), Ok(Presented::Ok)));
    }

    #[test]
    fn test_surface_driver_kind_is_reported() {
        let win = TestWindow::new();
        let s = win.surface(64, 48);
        assert!(matches!(
            s.driver_kind(),
            DriverKind::Hardware | DriverKind::Warp
        ));
    }
}
