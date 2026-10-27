//! Tests del emulador de terminal: secuencias VT, scrollback, Unicode y rendimiento.

use super::*;
use std::time::Instant;

#[test]
fn test_default_theme_palette() {
    let theme = Theme::default();
    assert_eq!(theme.background, Color::from_rgb(0x30, 0x34, 0x46));
    assert_eq!(theme.cursor, Color::from_rgb(0xf2, 0xd5, 0xcf));
    assert_eq!(theme.ansi.len(), 16);
}

#[test]
fn test_decscusr_cursor_shapes() {
    let mut term = TerminalState::new(80, 24, 100, Theme::default());
    assert_eq!(term.cursor_shape, CursorShape::Block);

    // CSI 3 SP q -> Blinking Underline
    term.process_bytes(b"\x1b[3 q");
    assert_eq!(term.cursor_shape, CursorShape::Underline);

    // CSI 4 SP q -> Steady Underline
    term.process_bytes(b"\x1b[4 q");
    assert_eq!(term.cursor_shape, CursorShape::Underline);

    // CSI 5 SP q -> Blinking Bar
    term.process_bytes(b"\x1b[5 q");
    assert_eq!(term.cursor_shape, CursorShape::Beam);

    // CSI 6 SP q -> Steady Bar
    term.process_bytes(b"\x1b[6 q");
    assert_eq!(term.cursor_shape, CursorShape::Beam);

    // CSI 1 SP q -> Blinking Block
    term.process_bytes(b"\x1b[1 q");
    assert_eq!(term.cursor_shape, CursorShape::Block);

    // CSI 2 SP q -> Steady Block
    term.process_bytes(b"\x1b[2 q");
    assert_eq!(term.cursor_shape, CursorShape::Block);
}

#[test]
fn test_truecolor_parsing() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    // 1. Secuencia TrueColor estándar punto y coma: RGB(120, 200, 250)
    let seq = b"\x1b[38;2;120;200;250mHola TrueColor\x1b[0m";
    term.process_bytes(seq);

    let cell = &term.grid[0][0];
    assert_eq!(cell.c, 'H');
    assert_eq!(cell.fg, Color::from_rgb(120, 200, 250));

    // 2. TrueColor delimitado por dos puntos (38:2:R:G:B)
    term.process_bytes(b"\r\n\x1b[38:2:180:90:220mY\x1b[0m");
    let cell_y = &term.grid[1][0];
    assert_eq!(cell_y.c, 'Y');
    assert_eq!(cell_y.fg, Color::from_rgb(180, 90, 220));

    // 3. TrueColor formato ISO con espacio de color (38:2::70:140:210)
    term.process_bytes(b"\x1b[38:2::70:140:210mZ\x1b[0m");
    let cell_z = &term.grid[1][1];
    assert_eq!(cell_z.c, 'Z');
    assert_eq!(cell_z.fg, Color::from_rgb(70, 140, 210));

    // 4. Color de subrayado TrueColor (SGR 58;2;R;G;B y SGR 58:2:R:G:B)
    term.process_bytes(b"\r\n\x1b[4;58;2;255;100;50mA\x1b[58:2:200:50:80mB\x1b[59mC\x1b[0m");
    let cell_a = &term.grid[2][0];
    assert_eq!(cell_a.c, 'A');
    assert!(cell_a.underline());
    assert_eq!(
        term.underline_color_of(cell_a),
        Some(Color::from_rgb(255, 100, 50))
    );

    let cell_b = &term.grid[2][1];
    assert_eq!(cell_b.c, 'B');
    assert!(cell_b.underline());
    assert_eq!(
        term.underline_color_of(cell_b),
        Some(Color::from_rgb(200, 50, 80))
    );

    let cell_c = &term.grid[2][2];
    assert_eq!(cell_c.c, 'C');
    assert!(cell_c.underline());
    assert_eq!(term.underline_color_of(cell_c), None); // SGR 59 restauró al color por defecto (fg)
}

#[test]
fn test_cursor_and_clear_screen() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Escribir en posición (10, 5)
    term.process_bytes(b"\x1b[5;10HTest");
    assert_eq!(term.cursor_y, 4);
    assert_eq!(term.cursor_x, 13);

    // Limpiar pantalla
    term.process_bytes(b"\x1b[2J");
    assert_eq!(term.cursor_x, 0);
    assert_eq!(term.cursor_y, 0);
    assert_eq!(term.grid[0][0].c, ' ');
}

#[test]
fn test_scrollback_retention() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 10, 500, theme);

    // Insertar 50 líneas (más que el tamaño de pantalla)
    for i in 0..50 {
        let line = format!("Línea de log {}\r\n", i);
        term.process_bytes(line.as_bytes());
    }

    assert_eq!(term.scrollback.len(), 41);
    term.scroll_up(20);
    assert_eq!(term.scroll_offset, 20);
    term.reset_scroll();
    assert_eq!(term.scroll_offset, 0);
}

#[test]
fn bench_throughput_massive_stream() {
    let theme = Theme::default();
    let mut term = TerminalState::new(120, 40, 50_000, theme);

    // Generar un bloque representativo de logs de agentes/compilador con colores ANSI
    let sample = b"\x1b[32m[INFO]\x1b[0m \x1b[1mClaude/Agy Agent:\x1b[0m Analizando archivo \x1b[36msrc/main.rs\x1b[0m [OK]\r\n";
    let iterations = 20_000;
    let total_bytes = sample.len() * iterations;

    let start = Instant::now();
    for _ in 0..iterations {
        term.process_bytes(sample);
    }
    let elapsed = start.elapsed();

    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
    let lines_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "\n⚡ [TEArminal Benchmark] Procesados {} bytes ({} líneas) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
        total_bytes, iterations, elapsed, throughput_mb, lines_per_sec
    );

    let min_throughput = if cfg!(debug_assertions) { 1.5 } else { 20.0 };
    assert!(
        throughput_mb > min_throughput,
        "El throughput debe superar {min_throughput} MB/s (obtenido: {throughput_mb:.2} MB/s)"
    );
}

#[test]
fn test_iterm2_image_protocol() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // PNG 1x1 codificado en Base64
    let b64_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
    let osc_command = format!("\x1b]1337;File=inline=1:{}\x07", b64_png);

    term.process_bytes(osc_command.as_bytes());

    assert_eq!(
        term.images.len(),
        1,
        "Debe decodificar y registrar exactamente una imagen inline"
    );
    let img = &term.images[0];
    assert_eq!(img.width_px, 1);
    assert_eq!(img.height_px, 1);
    assert!(img.width_cols >= 5);
    assert!(img.height_rows >= 2);
}

#[test]
fn test_inline_images_are_downscaled_evicted_and_capped() {
    use base64::prelude::*;
    let mut png = std::io::Cursor::new(Vec::new());
    image::GrayImage::new(700, 600)
        .write_with_encoder(image::codecs::png::PngEncoder::new_with_quality(
            &mut png,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::NoFilter,
        ))
        .unwrap();
    let osc = format!(
        "\x1b]1337;File=inline=1:{}\x07",
        BASE64_STANDARD.encode(png.into_inner())
    );

    // Se guarda al tamaño de dibujo (640x540), no el RGBA completo (700x600)
    assert!(osc.len() < 1024, "{}", osc.len());
    let mut term = TerminalState::new(80, 24, 100, Theme::default());
    term.process_bytes(osc.as_bytes());
    let img = &term.images[0];
    assert!(
        img.rgba_data.len() <= 640 * 540 * 4,
        "{}",
        img.rgba_data.len()
    );
    assert_eq!(
        img.rgba_data.len(),
        (img.width_px * img.height_px * 4) as usize
    );

    // La línea sale del historial (max_scrollback = 100): la imagen se libera
    term.process_bytes(
        "
"
        .repeat(300)
        .as_bytes(),
    );
    assert!(term.images.is_empty());

    // ED 3 vacía la lista
    term.process_bytes(osc.as_bytes());
    term.process_bytes(b"\x1b[3J");
    assert!(term.images.is_empty());

    // Tope total: nunca más de 64 MB, se descartan las más antiguas
    let mut term = TerminalState::new(80, 24, 10_000, Theme::default());
    for _ in 0..50 {
        term.process_bytes(osc.as_bytes());
    }
    let total: usize = term.images.iter().map(|i| i.rgba_data.len()).sum();
    assert!(total <= 64 * 1024 * 1024 && term.images.len() < 50);
    assert_eq!(term.images.last().unwrap().id, 50);
}

#[test]
fn test_alt_screen_buffer_tmux() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // 1. Escribir en buffer principal
    term.process_bytes(b"Comando normal en PowerShell\r\n");
    assert_eq!(term.scrollback.len(), 0);
    assert!(!term.is_alt_screen);

    // 2. Entrar en modo Alternate Screen (CSI ? 1049 h usado por tmux/btop/vim)
    term.process_bytes(b"\x1b[?1049h");
    assert!(term.is_alt_screen);

    // 3. Escribir datos dentro de tmux / btop
    for i in 0..30 {
        let log = format!("Panel tmux fila {}\r\n", i);
        term.process_bytes(log.as_bytes());
    }

    // El scrollback principal NO debe ser contaminado por la pantalla alterna
    assert_eq!(
        term.scrollback.len(),
        0,
        "Alternate screen buffer no debe contaminar el scrollback principal"
    );

    // 4. Salir de tmux (CSI ? 1049 l)
    term.process_bytes(b"\x1b[?1049l");
    assert!(!term.is_alt_screen, "Debe restaurar el buffer principal");
}

#[test]
fn test_decstbm_tmux_split_margins() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 20, 1000, theme);

    // Definir margen de desplazamiento de la línea 5 a la 10 (split pane de tmux)
    term.process_bytes(b"\x1b[5;10r");
    assert_eq!(term.scroll_region_top, 4);
    assert_eq!(term.scroll_region_bottom, 9);
}

#[test]
fn bench_massive_scrollback_100k_lines() {
    let theme = Theme::default();
    // 100.000 líneas de scrollback en memoria
    let mut term = TerminalState::new(120, 40, 100_000, theme);

    let line = b"[PyTorch/vLLM INFO] Step 12450/50000 | Loss: 0.0412 | GPU-0: 98% VRAM: 22.4/24.0 GB | 185 tokens/sec\r\n";
    let count = 100_000;
    let total_bytes = line.len() * count;

    let start = Instant::now();
    for _ in 0..count {
        term.process_bytes(line);
    }
    let elapsed = start.elapsed();

    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
    let lines_per_sec = count as f64 / elapsed.as_secs_f64();

    println!(
        "\n🚀 [TEArminal Massive Stream Test] Procesadas {} líneas ({} MB) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
        count, (total_bytes / (1024 * 1024)), elapsed, throughput_mb, lines_per_sec
    );

    assert_eq!(
        term.scrollback.len() + term.rows - 1,
        count,
        "Debe retener las 100.000 líneas entre scrollback y pantalla activa"
    );
    let min_lines_per_sec = if cfg!(debug_assertions) {
        25_000.0
    } else {
        300_000.0
    };
    assert!(
        lines_per_sec > min_lines_per_sec,
        "Debe superar {min_lines_per_sec} líneas/s (obtenido: {lines_per_sec:.0} líneas/s)"
    );
}

#[test]
fn test_osc_52_clipboard() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Enviar secuencia OSC 52 con texto "Hello from SSH!" en base64 (SGVsbG8gZnJvbSBTU0gh)
    term.process_bytes(b"\x1b]52;c;SGVsbG8gZnJvbSBTU0gh\x07");

    assert_eq!(
        term.clipboard_queue.len(),
        1,
        "Debe agregar el texto decodificado a la cola de portapapeles"
    );
    assert_eq!(term.clipboard_queue[0], "Hello from SSH!");

    // Capped at 100 KB
    use base64::prelude::*;
    let enc = |n: usize| BASE64_STANDARD.encode(vec![b'a'; n]);
    assert_eq!(
        super::performer::decode_osc52(enc(100 * 1024).as_bytes()).map(|t| t.len()),
        Some(100 * 1024)
    );
    assert!(super::performer::decode_osc52(enc(100 * 1024 + 1).as_bytes()).is_none());
    assert!(super::performer::decode_osc52(enc(10 * 1024 * 1024).as_bytes()).is_none());
}

#[test]
fn test_osc_133_semantic_prompts() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Simulación de secuencia Shell Integration OSC 133
    // 1. Inicio de prompt (A)
    term.process_bytes(b"\x1b]133;A\x07");
    // 2. Inicio de comando (B)
    term.process_bytes(b"\x1b]133;B\x07git status\r\n");
    // 3. Inicio de salida (C)
    term.process_bytes(b"\x1b]133;C\x07On branch main\r\nnothing to commit\r\n");
    // 4. Fin de comando con exit code 0 (D;0)
    term.process_bytes(b"\x1b]133;D;0\x07");

    assert_eq!(
        term.command_blocks.len(),
        1,
        "Debe registrar un bloque de comando semántico"
    );
    let block = &term.command_blocks[0];
    assert_eq!(block.exit_code, Some(0), "El código de salida debe ser 0");

    let last_output = term.get_last_output_text();
    assert!(
        last_output.is_some(),
        "Debe poder extraer la salida del comando sin mouse"
    );
    assert!(last_output.unwrap().contains("On branch main"));
}

#[test]
fn test_bracketed_paste_mode() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    assert!(!term.bracketed_paste_mode);
    // Activar modo bracketed paste (?2004h)
    term.process_bytes(b"\x1b[?2004h");
    assert!(term.bracketed_paste_mode);
    // Desactivar modo bracketed paste (?2004l)
    term.process_bytes(b"\x1b[?2004l");
    assert!(!term.bracketed_paste_mode);
}

#[test]
fn test_decckm_cursor_key_mode() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    assert!(!term.cursor_key_mode);
    // Activar modo de teclas de cursor de aplicación (?1h)
    term.process_bytes(b"\x1b[?1h");
    assert!(term.cursor_key_mode);
    // Desactivar modo de teclas de cursor (?1l)
    term.process_bytes(b"\x1b[?1l");
    assert!(!term.cursor_key_mode);
}

#[test]
fn test_powershell_full_startup_stream() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    let raw = b"\x1b[?9001h\x1b[?1004h\x1b[?25l\x1b[2J\x1b[m\x1b[HWindows PowerShell\r\nCopyright (C) Microsoft Corporation. All rights reserved.\x1b]0;C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\x07\x1b[?25h\x1b[?25l\x1b[4;1H\x1b[?25hPS C:\\Users\\gmora> ";
    term.process_bytes(raw);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    use std::io::Write;

    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            let _ = writeln!(
                out,
                "ROW {}: {:?} (cell[0].fg: {:?}, bg: {:?})",
                i, trimmed, row[0].fg, row[0].bg
            );
        }
    }
    let _ = writeln!(
        out,
        "cursor_x: {}, cursor_y: {}",
        term.cursor_x, term.cursor_y
    );

    // Simular redimensionamiento de ventana al inicio (de 24 a 20 filas)
    let _ = writeln!(out, "--- AFTER RESIZE to 20 rows ---");
    term.resize(80, 20);
    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            let _ = writeln!(out, "GRID ROW {}: {:?}", i, trimmed);
        }
    }
    for (i, row) in term.scrollback.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            let _ = writeln!(out, "SCROLLBACK ROW {}: {:?}", i, trimmed);
        }
    }
    let _ = writeln!(
        out,
        "cursor_x: {}, cursor_y: {}",
        term.cursor_x, term.cursor_y
    );
    assert!(term.cursor_x > 0);
}

#[test]
fn test_unicode_double_width_emojis_and_cjk() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Imprimir emoji robot (🤖) y texto "AI"
    let text = "🤖AI";
    term.process_bytes(text.as_bytes());

    // El emoji '🤖' ocupa 2 columnas:
    // Columna 0: carácter '🤖' con width = 2
    // Columna 1: celda de continuación espaciadora con width = 0
    assert_eq!(term.grid[0][0].c, '🤖');
    assert_eq!(term.grid[0][0].width, 2);
    assert_eq!(term.grid[0][1].width, 0);

    // Columna 2: 'A' con width = 1
    assert_eq!(term.grid[0][2].c, 'A');
    assert_eq!(term.grid[0][2].width, 1);

    // Columna 3: 'I' con width = 1
    assert_eq!(term.grid[0][3].c, 'I');
    assert_eq!(term.grid[0][3].width, 1);

    // El cursor debe estar en la columna 4
    assert_eq!(term.cursor_x, 4);
}

#[test]
fn test_unicode_box_drawing_and_braille_patterns() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Caracteres de diagramación (Box Drawing) y patrones Braille (utilizados por btop / spinners)
    let tui_chars = "┌─┐⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
    term.process_bytes(tui_chars.as_bytes());

    assert_eq!(term.grid[0][0].c, '┌');
    assert_eq!(term.grid[0][1].c, '─');
    assert_eq!(term.grid[0][2].c, '┐');
    assert_eq!(term.grid[0][3].c, '⠋');
    assert_eq!(term.grid[0][4].c, '⠙');
    assert_eq!(term.cursor_x, 13);
}

#[test]
fn test_terminal_grapheme_clustering() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Letra 'e' seguida de acento agudo combinatorio (U+0301) y luego 'X'
    let text = "e\u{0301}X";
    term.process_bytes(text.as_bytes());

    // 'e' + '\u{0301}' deben agruparse en la celda 0 con ancho 1
    let cell_e = &term.grid[0][0];
    assert!(
        cell_e.is_grapheme(),
        "La celda debe marcarse como clúster de grafema"
    );
    assert_eq!(cell_e.width, 1);
    assert_eq!(term.grapheme_of(cell_e), Some("e\u{0301}"));

    // 'X' debe estar en la columna 1
    assert_eq!(term.grid[0][1].c, 'X');
    assert_eq!(term.grid[0][1].width, 1);
    assert_eq!(term.cursor_x, 2);

    // Múltiples diacríticos combinatorios: 'o' + horn (U+031B) + acute (U+0301)
    let mut term2 = TerminalState::new(80, 24, 1000, Theme::default());
    term2.process_bytes("o\u{031B}\u{0301}".as_bytes());
    let cell_o = &term2.grid[0][0];
    assert!(cell_o.is_grapheme());
    assert_eq!(cell_o.width, 1);
    assert_eq!(term2.grapheme_of(cell_o), Some("o\u{031B}\u{0301}"));
    assert_eq!(term2.cursor_x, 1);

    // Pulgar arriba con tono de piel medio (U+1F44D U+1F3FD)
    let mut term3 = TerminalState::new(80, 24, 1000, Theme::default());
    term3.process_bytes("👍🏽!".as_bytes());
    let cell_thumb = &term3.grid[0][0];
    assert!(cell_thumb.is_grapheme());
    assert_eq!(cell_thumb.width, 2);
    assert_eq!(term3.grapheme_of(cell_thumb), Some("👍🏽"));
    assert_eq!(term3.grid[0][1].width, 0); // espaciador
    assert_eq!(term3.grid[0][2].c, '!');
    assert_eq!(term3.cursor_x, 3);

    // Secuencia ZWJ (U+1F468 U+200D U+1F469: 👨‍👩)
    let mut term_zwj = TerminalState::new(80, 24, 1000, Theme::default());
    term_zwj.process_bytes("👨\u{200D}👩".as_bytes());
    let cell_zwj = &term_zwj.grid[0][0];
    assert!(cell_zwj.is_grapheme());
    assert_eq!(cell_zwj.width, 2);
    assert_eq!(term_zwj.grapheme_of(cell_zwj), Some("👨\u{200D}👩"));
    assert_eq!(term_zwj.cursor_x, 2);
}

#[test]
fn test_terminal_grapheme_clustering_copy_selection_text() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    term.process_bytes("AI: e\u{0301} y 👍🏽\r\n".as_bytes());

    term.selection = Some(SelectionRange {
        start: (0, 0),
        end: (12, 0),
    });

    let copied = term
        .copy_selection_text()
        .expect("Debe haber texto seleccionado");
    assert!(
        copied.contains("e\u{0301}"),
        "El texto copiado debe conservar el grafema"
    );
    assert!(
        copied.contains("👍🏽"),
        "El texto copiado debe conservar el emoji"
    );
}

#[test]
fn test_terminal_selection_tracks_text_when_scrolled() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    for i in 0..40 {
        term.process_bytes(format!("echo line {i}\r\n").as_bytes());
    }

    assert!(term.scrollback.len() >= 16);

    // Seleccionar "echo line 5" que ahora está en el scrollback
    term.selection = Some(SelectionRange {
        start: (0, 5),
        end: (11, 5),
    });

    let text_at_rest = term.copy_selection_text().expect("texto seleccionado");
    assert_eq!(text_at_rest, "echo line 5");

    // Desplazar la terminal hacia arriba (scroll_offset > 0)
    term.scroll_offset = 10;
    let text_scrolled = term
        .copy_selection_text()
        .expect("texto seleccionado al scrollear");
    assert_eq!(
        text_scrolled, "echo line 5",
        "El texto seleccionado debe mantenerse idéntico sin importar el scroll_offset"
    );

    // Desplazar de nuevo hacia abajo al fondo
    term.scroll_offset = 0;
    assert_eq!(
        term.copy_selection_text().as_deref(),
        Some("echo line 5"),
        "El texto seleccionado debe persistir al volver al prompt activo"
    );
}

#[test]
fn test_terminal_selection_trims_trailing_empty_space() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    term.process_bytes("cargo build\r\n".as_bytes());

    // Seleccionar desde el inicio hasta la columna 70 (que es espacio vacío en la pantalla)
    term.selection = Some(SelectionRange {
        start: (0, 0),
        end: (70, 0),
    });

    let copied = term.copy_selection_text().expect("texto copiado");
    assert_eq!(
        copied, "cargo build",
        "La copia no debe incluir los 59 espacios vacíos de la rejilla"
    );
}

#[test]
fn test_terminal_selection_shifts_on_scrollback_eviction() {
    let mut term = TerminalState::new(80, 5, 5, Theme::default());
    for i in 0..5 {
        term.process_bytes(format!("line {i}\r\n").as_bytes());
    }
    // Llenar scrollback hasta su límite (5 líneas)
    for i in 5..10 {
        term.process_bytes(format!("line {i}\r\n").as_bytes());
    }
    assert_eq!(term.scrollback.len(), 5);

    // Seleccionar línea 2
    term.selection = Some(SelectionRange {
        start: (0, 2),
        end: (6, 2),
    });

    // Añadir una línea nueva que provoca la expulsión de la línea 0 de scrollback
    term.process_bytes(b"line new\r\n");
    let sel = term.selection.expect("debe existir selección");
    assert_eq!(sel.start.1, 1, "La selección debió decrementarse de 2 a 1");
    assert_eq!(sel.end.1, 1);
}

#[test]
fn test_sgr_text_styles_dim_bold_underline_strikethrough() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Estilos combinados: 1 (bold), 2 (dim), 4 (underline), 9 (strikethrough)
    term.process_bytes(b"\x1b[1;2;4;9mA\x1b[0mB");

    let cell_a = &term.grid[0][0];
    assert_eq!(cell_a.c, 'A');
    assert!(cell_a.bold());
    assert!(cell_a.dim());
    assert!(cell_a.underline());
    assert!(cell_a.strikethrough());

    // Tras el reset (SGR 0), 'B' debe tener estilos limpios
    let cell_b = &term.grid[0][1];
    assert_eq!(cell_b.c, 'B');
    assert!(!cell_b.bold());
    assert!(!cell_b.dim());
    assert!(!cell_b.underline());
    assert!(!cell_b.strikethrough());
}

#[test]
fn test_tui_mouse_and_sync_protocols() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    assert!(!term.is_mouse_reporting_active());
    assert!(!term.mouse_reporting_sgr);
    assert!(!term.focus_event_reporting);
    assert!(!term.sync_update_mode);

    // Herdr / Ratatui activa mouse tracking (1000, 1002, 1006) y focus (1004)
    term.process_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[?2026h");
    assert!(term.is_mouse_reporting_active());
    assert!(term.mouse_reporting_normal);
    assert!(term.mouse_reporting_button_event);
    assert!(term.mouse_reporting_sgr);
    assert!(term.focus_event_reporting);
    assert!(term.sync_update_mode);

    // Desactivación al salir del TUI
    term.process_bytes(b"\x1b[?1000l\x1b[?1002l\x1b[?1006l\x1b[?1004l\x1b[?2026l");
    assert!(!term.is_mouse_reporting_active());
    assert!(!term.mouse_reporting_sgr);
    assert!(!term.focus_event_reporting);
    assert!(!term.sync_update_mode);
}

#[test]
fn test_pwsh_startup_prompt() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    let raw = b"\x1b[?9001h\x1b[?1004h\x1b[2J\x1b[?25l\x1b[2J\x1b[m\x1b[H\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[H\x1b]0;pwsh.exe\x07\x1b[?25h\x1b[38;2;78;146;249m\xef\x81\xbb ~\x1b[38;2;81;226;122m\r\n\xef\x90\xb2 ";
    term.process_bytes(raw);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    use std::io::Write;
    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            let _ = writeln!(out, "ROW {}: {:?}", i, trimmed);
        }
    }
    let _ = writeln!(
        out,
        "cursor_x: {}, cursor_y: {}",
        term.cursor_x, term.cursor_y
    );
    assert!(term.cursor_x > 0);
}

#[test]
fn test_ech_erase_characters() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Escribir "hello world"
    term.process_bytes(b"hello world");
    assert_eq!(term.grid[0][0].c, 'h');
    assert_eq!(term.grid[0][6].c, 'w');

    // Mover cursor a columna 6 (índice 5: ' ') y borrar 6 caracteres con ECH (\x1b[6X)
    term.process_bytes(b"\x1b[1;6H\x1b[6X");
    assert_eq!(term.cursor_x, 5); // El cursor no debe moverse
    assert_eq!(term.grid[0][0].c, 'h');
    assert_eq!(term.grid[0][4].c, 'o');
    assert_eq!(term.grid[0][5].c, ' ');
    assert_eq!(term.grid[0][6].c, ' ');
    assert_eq!(term.grid[0][10].c, ' ');
}

#[test]
fn test_bce_erase_in_line() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Establecer color de fondo azul (44) y borrar línea con \x1b[2K
    term.process_bytes(b"\x1b[44m\x1b[2K");
    assert_eq!(term.grid[0][0].bg, term.theme.ansi[4]);
    assert_eq!(term.grid[0][79].bg, term.theme.ansi[4]);
}

#[test]
fn test_osc_cwd_reporting() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Test OSC 7
    term.process_bytes(b"\x1b]7;file://localhost/C:/Users/gmora/dev/tearminal\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:/Users/gmora/dev/tearminal"))
    );

    // Test OSC 9;9
    term.process_bytes(b"\x1b]9;9;\"C:\\Users\\gmora\\dev\"\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:\\Users\\gmora\\dev"))
    );
}

#[test]
fn test_osc_cwd_rejects_network_paths() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Ninguna ruta de red (UNC) puede llegar a `current_dir`: Windows abriría SMB y filtraría el hash NTLM
    for seq in [
        &b"\x1b]9;9;\\\\evil.example\\x\x07"[..],
        b"\x1b]9;9;//evil.example/x\x07",
        b"\x1b]9;9;\\\\?\\UNC\\evil.example\\x\x07",
        b"\x1b]7;file://localhost///evil.example/x\x07",
        b"\x1b]7;file://localhost/\\\\evil.example\\x\x07",
        b"\x1b]0;dev@host://evil.example/x\x07",
    ] {
        term.process_bytes(seq);
        assert_eq!(term.current_dir, None, "{:?}", String::from_utf8_lossy(seq));
    }
}

#[test]
fn test_osc_7_decodes_every_percent_escape() {
    use super::performer::percent_decode;
    assert_eq!(percent_decode("/a%20b%2Fc%C3%B1%25"), "/a b/cñ%");
    // Escapes incompletos o no hexadecimales se dejan tal cual
    assert_eq!(percent_decode("100%"), "100%");
    assert_eq!(percent_decode("%4"), "%4");
    assert_eq!(percent_decode("%zz%+1"), "%zz%+1");

    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    term.process_bytes(b"\x1b]7;file://localhost/C:/Users/Jos%C3%A9/mi%20carpeta%23\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:/Users/José/mi carpeta#"))
    );
    // Un `%0A` decodificado es un carácter de control: no es un directorio aceptable
    term.process_bytes(b"\x1b]7;file://localhost/C:/x%0Ay\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:/Users/José/mi carpeta#"))
    );
}

/// El gancho de bash escribe `%` como `%25` (y los espacios como `%20`): una carpeta llamada `a%41` no se
/// puede confundir con `aA` al decodificar
#[test]
fn test_hook_osc_7_round_trips_special_directory_names() {
    let Ok(out) = std::process::Command::new("bash")
        .args(["-c", "command -v od tr"])
        .output()
    else {
        return; // sin bash en esta máquina
    };
    if !out.status.success() {
        return;
    }
    let init = include_str!("../../assets/shell/init.ps1").replace("\r\n", "\n");
    let start = init.find("$hook = @'\n").expect("gancho") + "$hook = @'\n".len();
    let hook = &init[start..start + init[start..].find("\n'@").expect("fin del gancho")];
    let dir = format!("/tmp/tearminal_osc7_{}/a b%41", std::process::id());
    let script_content = format!(
        "mkdir -p '{dir}' && cd '{dir}'\nHOSTNAME=srv\n{hook}\nprintf '\\n@@@%s' \"$PWD\"\nrm -rf '/tmp/tearminal_osc7_{}'\n",
        std::process::id()
    );
    use std::io::Write;
    let mut child = std::process::Command::new("bash")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script_content.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).replace('\r', "");
    let (osc, pwd) = text.split_once("\n@@@").expect("salida del gancho");
    let uri = osc
        .split("]7;file://srv")
        .nth(1)
        .and_then(|r| r.split('\x07').next())
        .expect("OSC 7");
    assert!(uri.ends_with("a%20b%2541"), "{uri:?}");
    assert_eq!(super::performer::percent_decode(uri), pwd);
}

#[test]
fn test_title_with_path_only_sets_cwd_inside_an_authenticated_ssh_session() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    term.pane_token = "t0k3n".into();
    // Título de WSL (`user@host: /ruta`): la ruta no existe en Windows, no es un directorio local
    term.process_bytes(b"\x1b]0;user@host: /home/user/proyecto\x07");
    assert_eq!(term.current_dir, None);

    // Dentro de una sesión SSH autenticada sí actualiza el directorio remoto que se muestra
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=user@host;token=t0k3n\x07");
    term.process_bytes(b"\x1b]0;user@host: /home/user/proyecto\x07");
    assert_eq!(
        term.ssh_session.as_ref().unwrap().remote_dir.as_deref(),
        Some("/home/user/proyecto")
    );
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("/home/user/proyecto"))
    );
}

#[test]
fn test_resize_respects_max_scrollback() {
    let mut term = TerminalState::new(80, 24, 3, Theme::default());
    for i in 0..24 {
        term.process_bytes(format!("fila {i}\r\n").as_bytes());
    }
    // El cursor desborda la nueva altura: 20+ filas pasarían al historial, que admite 3
    term.resize(80, 4);
    assert!(term.scrollback.len() <= 3, "{}", term.scrollback.len());
}

#[test]
fn test_osc_window_title_does_not_infer_ssh() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Envío de título de ventana tradicional OSC 0
    term.process_bytes(b"\x1b]0;My Custom Window\x07");
    assert_eq!(term.window_title.as_deref(), Some("My Custom Window"));
    assert!(term.ssh_session.is_none());

    // El título no autentica nada: "ssh: user@host" no crea sesión SSH
    term.process_bytes(b"\x1b]0;ssh: dev@server.example.com\x07");
    assert_eq!(
        term.window_title.as_deref(),
        Some("ssh: dev@server.example.com")
    );
    assert!(term.ssh_session.is_none());
}

#[test]
fn test_osc_tearminal_ssh_signaling() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    term.pane_token = "t0k3n".into();

    // Sin token o con uno ajeno (p. ej. un `cat` de un archivo con la secuencia) se ignora
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=evil.example\x07");
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=evil.example;token=otro\x07");
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=evil.example;token=\x07");
    assert!(term.ssh_session.is_none());

    // Inicio de sesión SSH mediante protocolo OSC 1337 TearminalSSH=start con el token del panel
    term.process_bytes(
        b"\x1b]1337;TearminalSSH=start;dest=ubuntu@production-node-1;token=t0k3n\x07",
    );
    let sess = term
        .ssh_session
        .as_ref()
        .expect("Debe registrar la sesión SSH activa");
    assert_eq!(sess.destination, "ubuntu@production-node-1");
    assert!(sess.is_active);

    // Sin token o con uno ajeno no se puede cerrar la sesión de otro panel
    term.process_bytes(b"\x1b]1337;TearminalSSH=end\x07");
    term.process_bytes(b"\x1b]1337;TearminalSSH=end;token=otro\x07");
    assert!(term.ssh_session.is_some());

    // Finalización de sesión SSH mediante OSC 1337 TearminalSSH=end con el token del panel
    term.process_bytes(b"\x1b]1337;TearminalSSH=end;token=t0k3n\x07");
    assert!(
        term.ssh_session.is_none(),
        "Debe limpiar la sesión SSH al desconectarse"
    );
}

#[test]
fn test_osc_7_remote_host_ssh() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    term.pane_token = "t0k3n".into();

    // OSC 7 de un host remoto no puede crear la sesión (cualquier texto en pantalla lo emitiría)
    term.process_bytes(b"\x1b]7;file://bastion.corp.internal/home/admin/projects\x07");
    assert!(term.ssh_session.is_none());

    // Con la sesión ya autenticada, solo actualiza el directorio remoto
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=bastion.corp.internal;token=t0k3n\x07");
    term.process_bytes(b"\x1b]7;file://bastion.corp.internal/home/admin/projects\x07");
    let sess = term.ssh_session.as_ref().expect("sesión autenticada");
    assert_eq!(sess.destination, "bastion.corp.internal");
    assert_eq!(sess.remote_dir.as_deref(), Some("/home/admin/projects"));
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("/home/admin/projects"))
    );
    assert!(sess.is_active);
}

#[test]
fn test_cell_memory_footprint_16_bytes() {
    assert_eq!(
        std::mem::size_of::<crate::terminal::Cell>(),
        16,
        "Cell debe estar optimizada a exactamente 16 bytes de memoria"
    );
}

#[test]
fn test_osc_8_explicit_hyperlinks() {
    let mut term = TerminalState::new(80, 24, 100, Theme::default());

    // 1. Enlace explícito terminado con ST (\x1b\)
    let seq =
        b"\x1b]8;id=xyz;https://tearminal.terminal/docs\x1b\\Documentacion\x1b]8;;\x1b\\ texto normal";
    term.process_bytes(seq);

    // Las celdas de "Documentacion" (cols 0..13) tienen el hipervínculo
    for col in 0..13 {
        assert_eq!(
            term.url_at(col, 0),
            Some("https://tearminal.terminal/docs".to_string()),
            "col {col} debe tener el URL explicito"
        );
        assert!(term.grid[0][col].hyperlink_id > 0);
    }

    // " texto normal" (col 13 en adelante) no tiene enlace
    for col in 13..26 {
        assert_eq!(term.url_at(col, 0), None);
        assert_eq!(term.grid[0][col].hyperlink_id, 0);
    }

    // 2. Enlace terminado con BEL (\x07) y URL con punto y coma
    term.process_bytes(b"\r\n\x1b]8;;https://example.com/api?a=1;b=2\x07API\x1b]8;;\x07");
    for col in 0..3 {
        assert_eq!(
            term.url_at(col, 1),
            Some("https://example.com/api?a=1;b=2".to_string())
        );
    }

    // 3. Enlace que envuelve de línea: mantiene el mismo ID y URL en ambas líneas
    term.cursor_x = 75;
    term.cursor_y = 2;
    term.process_bytes(b"\x1b]8;;https://wrapped.link\x1b\\1234567890\x1b]8;;\x1b\\");
    // Los primeros 5 caracteres están en cols 75..80 de la línea 2
    for col in 75..80 {
        assert_eq!(
            term.url_at(col, 2),
            Some("https://wrapped.link".to_string())
        );
    }
    // Los siguientes 5 caracteres envolvieron a la línea 3 (cols 0..5)
    for col in 0..5 {
        assert_eq!(
            term.url_at(col, 3),
            Some("https://wrapped.link".to_string())
        );
    }

    // 4. Alt screen no filtra el enlace activo
    term.process_bytes(b"\x1b]8;;https://leak.test\x1b\\Texto");
    assert!(term.current_hyperlink > 0);
    term.enter_alt_screen();
    assert_eq!(term.current_hyperlink, 0);
    term.exit_alt_screen();
    assert_eq!(term.current_hyperlink, 0);

    // 5. Rechazo de URLs inválidas o con caracteres de control
    let bad_ctrl = b"\x1b]8;;https://evil.com/\x00malicious\x1b\\malo\x1b]8;;\x1b\\";
    term.process_bytes(bad_ctrl);
    assert_eq!(term.current_hyperlink, 0);

    // 6. Sobrescribir celdas con texto plano limpia el hipervínculo en esas celdas
    let mut term2 = TerminalState::new(80, 24, 100, Theme::default());
    term2.process_bytes(b"\x1b]8;;https://example.com\x1b\\HOLA\x1b]8;;\x1b\\");
    assert_eq!(term2.url_at(0, 0), Some("https://example.com".to_string()));
    // Volver al inicio y sobrescribir "HO" con "XX"
    term2.process_bytes(b"\rXX");
    assert_eq!(term2.url_at(0, 0), None);
    assert_eq!(term2.url_at(1, 0), None);
    assert_eq!(term2.url_at(2, 0), Some("https://example.com".to_string()));
    assert_eq!(term2.url_at(3, 0), Some("https://example.com".to_string()));

    // 5. El texto plano sin secuencias OSC 8 no genera hipervínculos
    term2.cursor_x = 0;
    term2.cursor_y = 1;
    term2.process_bytes(b"Visita https://github.com/GonzhaI/TEArminal para mas info.");
    assert_eq!(term2.url_at(0, 1), None);
    assert_eq!(term2.url_at(7, 1), None);
    assert_eq!(term2.url_at(20, 1), None);
    assert_eq!(term2.url_at(36, 1), None);
}

/// Las secuencias de escape son entrada no confiable: no pueden fijar un destino SSH con metacaracteres
/// (se interpola en una orden de PowerShell al dividir un panel).
#[test]
fn test_untrusted_escape_sequences_cannot_set_unsafe_ssh_session() {
    let new = || TerminalState::new(80, 24, 100, Theme::default());

    let mut t = new();
    t.process_bytes(b"\x1b]0;ssh: user@host\x07");
    assert!(t.ssh_session.is_none());

    // OSC usa `;` como separador de parámetros, así que los vectores reales son otros metacaracteres
    for evil in [
        &b"\x1b]0;ssh: x && calc\x07"[..],
        b"\x1b]0;ssh: $(calc)\x07",
        b"\x1b]2;ssh: x|calc\x07",
        b"\x1b]2;ssh: -oProxyCommand=calc\x07",
        b"\x1b]1337;TearminalSSH=start;dest=$(calc);token=t0k3n\x07",
        b"\x1b]1337;TearminalSSH=start;dest=-oProxyCommand=calc;token=t0k3n\x07",
        b"\x1b]7;file://evil$(calc)/home/x\x07",
    ] {
        let mut t = new();
        t.pane_token = "t0k3n".into();
        t.process_bytes(evil);
        assert!(
            t.ssh_session.is_none(),
            "no debe crear sesión SSH con {:?}",
            String::from_utf8_lossy(evil)
        );
    }

    // host válido por OSC 7 pero directorio con comillas: la sesión se crea sin directorio remoto
    let mut t = new();
    t.pane_token = "t0k3n".into();
    t.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=server;token=t0k3n\x07");
    t.process_bytes(b"\x1b]7;file://server/tmp/'$(calc)'\x07");
    let s = t.ssh_session.as_ref().expect("host válido");
    assert_eq!(s.destination, "server");
    assert_eq!(s.remote_dir, None);
}

#[test]
fn test_remote_integration_reports_branch_and_listing_and_validates_input() {
    let hex = |s: &str| s.bytes().map(|b| format!("{b:02x}")).collect::<String>();
    let theme = crate::theme::Theme::default();
    let mut term = TerminalState::new(80, 24, 100, theme);
    term.pane_token = "t0k3n".into();
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=user@host;token=t0k3n\x07");
    assert!(!term.remote_integration);

    // La rama llega en cada prompt y se limpia al salir de un repositorio
    term.process_bytes(b"\x1b]1337;TearminalGit=qa\x07");
    assert!(term.remote_integration);
    assert_eq!(
        term.ssh_session
            .as_ref()
            .unwrap()
            .remote_git_branch
            .as_deref(),
        Some("qa")
    );
    term.process_bytes(b"\x1b]1337;TearminalGit=\x07");
    assert_eq!(term.ssh_session.as_ref().unwrap().remote_git_branch, None);

    term.process_bytes(b"\x1b]1337;TearminalGit=feature/x\x07");
    assert_eq!(
        term.ssh_session
            .as_ref()
            .unwrap()
            .remote_git_branch
            .as_deref(),
        Some("feature/x")
    );
    term.process_bytes(b"\x1b]1337;TearminalGit=-\x07");
    assert_eq!(
        term.ssh_session.as_ref().unwrap().remote_git_branch,
        None,
        "'-' = sin rama"
    );

    // Listado: se descartan nombres peligrosos (rutas, control, . y ..)
    let raw = "src/\nREADME.md\n../\n./\na/b\nmalo\u{1b}[31m\n.env\n";
    let osc = format!("\x1b]1337;TearminalLs={}\x07", hex(raw));
    term.process_bytes(osc.as_bytes());
    assert_eq!(
        term.remote_listing.as_deref(),
        Some(
            &[
                "src/".to_string(),
                "README.md".to_string(),
                ".env".to_string()
            ][..]
        )
    );

    // Un directorio grande conserva los archivos ocultos que quedan al final (p. ej. `.env`)
    let many: String = (0..1000)
        .map(|i| {
            format!(
                "f{i}
"
            )
        })
        .chain([".env
"
        .into()])
        .collect();
    let osc = format!("]1337;TearminalLs={}", hex(&many));
    term.process_bytes(osc.as_bytes());
    let listed = term.remote_listing.as_deref().unwrap();
    assert_eq!(listed.len(), 1001);
    assert_eq!(listed.last().map(String::as_str), Some(".env"));

    // Un listado desmesurado se ignora y al terminar la sesión todo se reinicia
    let huge = format!("\x1b]1337;TearminalLs={}\x07", "ab".repeat(300 * 1024));
    term.remote_listing = None;
    term.process_bytes(huge.as_bytes());
    // El búfer OSC acota la secuencia: lo que queda no es un nombre válido
    assert!(term
        .remote_listing
        .as_deref()
        .is_none_or(<[String]>::is_empty));
    term.process_bytes(b"\x1b]1337;TearminalSSH=end;token=t0k3n\x07");
    assert!(!term.remote_integration && term.remote_listing.is_none());

    // Sin sesión SSH nadie puede inyectar listado ni rama
    term.process_bytes(osc.as_bytes());
    assert_eq!(term.remote_listing, None);
}

#[test]
fn test_remote_file_responses_are_reassembled_and_bounded() {
    let hex = |s: &str| s.bytes().map(|b| format!("{b:02x}")).collect::<String>();
    let theme = crate::theme::Theme::default();
    let mut term = TerminalState::new(80, 24, 100, theme);

    // Sin integración no se puede pedir nada (los bytes irían a otro programa)
    assert!(term.remote_request_bytes("get", 1, "abc").is_none());
    term.pane_token = "t0k3n".into();
    term.process_bytes(b"\x1b]1337;TearminalSSH=start;dest=user@host;token=t0k3n\x07");
    term.process_bytes(b"\x1b]1337;TearminalGit=\x07");
    assert!(term.remote_at_prompt);
    let req = term
        .remote_request_bytes("get", 7, "%2Fetc%2Fhosts")
        .expect("petición");
    assert_eq!(req, b"\x05#AXR#7 get %2Fetc%2Fhosts\x1b[1337~".to_vec());
    assert!(
        term.remote_request_bytes("rm", 7, "x").is_none(),
        "solo operaciones conocidas"
    );
    assert!(
        term.remote_request_bytes("get", 7, "a;b").is_none(),
        "carga no segura"
    );
    term.note_enter_sent();
    assert!(
        term.remote_request_bytes("get", 7, "abc").is_none(),
        "tras Enter ya no hay prompt libre"
    );

    // Respuesta en dos trozos, llegando desordenados
    let data = hex("hola mundo");
    let (a, b) = data.split_at(8);
    term.process_bytes(format!("\x1b]1337;TearminalFile=7;ok;1;2;{b}\x07").as_bytes());
    assert!(term.remote_file_results.is_empty());
    term.process_bytes(format!("\x1b]1337;TearminalFile=7;ok;0;2;{a}\x07").as_bytes());
    assert_eq!(
        term.remote_file_results,
        vec![RemoteFileResult {
            id: 7,
            ok: true,
            data: b"hola mundo".to_vec()
        }]
    );

    // Error del servidor y datos corruptos
    term.remote_file_results.clear();
    term.process_bytes(b"\x1b]1337;TearminalFile=8;err;0;1;\x07");
    term.process_bytes(b"\x1b]1337;TearminalFile=9;ok;0;1;zz\x07");
    assert_eq!(term.remote_file_results.len(), 2);
    assert!(term
        .remote_file_results
        .iter()
        .all(|r| !r.ok && r.data.is_empty()));

    // Límites: demasiados trozos, índice fuera de rango, sin sesión
    term.remote_file_results.clear();
    term.process_bytes(b"\x1b]1337;TearminalFile=10;ok;0;9999;ab\x07");
    term.process_bytes(b"\x1b]1337;TearminalFile=10;ok;5;2;ab\x07");
    assert!(term.remote_file_results.is_empty());
    term.process_bytes(b"\x1b]1337;TearminalSSH=end;token=t0k3n\x07");
    term.process_bytes(b"\x1b]1337;TearminalFile=11;ok;0;1;6162\x07");
    assert!(term.remote_file_results.is_empty());
}

/// agy y Claude Code envían `CSI > 4 ; 2 m` (modifyOtherKeys) al arrancar: no es SGR y no debe
/// dejar subrayado/tenue activos (subrayaba la primera fila del banner, también los espacios).
#[test]
fn test_private_marker_csi_m_is_not_sgr() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    term.process_bytes(b"\x1b[>4;2m\x1b[>4m\x1b[>1u\x1b[0 qA");
    let cell = &term.grid[0][0];
    assert_eq!(cell.c, 'A');
    assert!(!cell.underline() && !cell.dim() && !cell.strikethrough());
    // el SGR de verdad sigue funcionando
    term.process_bytes(b"\x1b[4mB");
    assert!(term.grid[0][1].underline());
}

/// El wrapper de agentes avisa con OSC 1337 TearminalAgent; TEArminal lo usa para reabrir el shell si muere con el agente.
#[test]
fn test_tearminal_agent_osc_tracks_running_agent() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    assert!(!term.agent_running);
    term.process_bytes(b"\x1b]1337;TearminalAgent=start;name=claude\x07");
    assert!(term.agent_running);
    assert_eq!(
        term.active_ai_agent(),
        crate::renderer::AiAgentStatus::Claude
    );

    // Enviar un mensaje o cambiar el título (p. ej. herramientas o "Thinking...") no pierde el agente
    term.process_bytes(b"\x1b]0;Thinking...\x07");
    assert_eq!(
        term.active_ai_agent(),
        crate::renderer::AiAgentStatus::Claude
    );

    term.process_bytes(b"\x1b]1337;TearminalAgent=end\x07");
    assert!(!term.agent_running);
    assert_eq!(term.active_ai_agent(), crate::renderer::AiAgentStatus::None);

    // Al terminar un agente, TEArminal restaura la región de scroll y modos TUI sin mover el cursor a (0,0)
    term.process_bytes(
        b"\x1b]1337;TearminalAgent=start\x07\x1b[2;20r\x1b[?1000h\x1b[?2004h\x1b[10;5HUpdated to 1.2.3",
    );
    assert_eq!(term.scroll_region_top, 1);
    assert_eq!(term.scroll_region_bottom, 19);
    assert!(term.mouse_reporting_normal);
    assert!(term.bracketed_paste_mode);
    let cy_before = term.cursor_y;
    term.process_bytes(b"\x1b]1337;TearminalAgent=end\x07");
    assert_eq!(term.scroll_region_top, 0);
    assert_eq!(term.scroll_region_bottom, 23);
    assert!(!term.mouse_reporting_normal);
    assert!(!term.bracketed_paste_mode);
    assert_eq!(term.cursor_y, cy_before);
    assert_eq!(term.grid[9][4].c, 'U');
}

#[test]
fn test_osc_133_blocks_shift_on_scrollback_eviction() {
    let mut term = TerminalState::new(80, 4, 5, Theme::default());
    term.process_bytes(
        b"\x1b]133;A\x07\x1b]133;B\x07old\r\n\x1b]133;C\x07viejo\r\n\x1b]133;D;0\x07",
    );
    for i in 0..8 {
        term.process_bytes(format!("relleno {i}\r\n").as_bytes());
    }
    term.process_bytes(
        b"\x1b]133;A\x07\x1b]133;B\x07cmd\r\n\x1b]133;C\x07salida viva\r\n\x1b]133;D;0\x07",
    );
    assert_eq!(
        term.command_blocks.len(),
        1,
        "el primer bloque salió del historial"
    );
    assert_eq!(term.get_last_output_text().as_deref(), Some("salida viva"));
}

#[test]
fn test_clear_scrollback_and_ed_3() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    for i in 0..50 {
        term.process_bytes(format!("Línea {i}\r\n").as_bytes());
    }
    assert!(!term.scrollback.is_empty());
    term.scroll_up(10);
    assert!(term.scroll_offset > 0);

    // CSI 3 J (ED 3: limpiar scrollback y resetear scroll)
    term.process_bytes(b"\x1b[3J");
    assert_eq!(term.scrollback.len(), 0);
    assert_eq!(term.scroll_offset, 0);
    assert_eq!(term.total_lines_count, term.rows);
    assert!(term.command_blocks.is_empty());
    assert!(term.selection.is_none());
    assert!(term.images.is_empty());
}

#[test]
fn bench_csi_cursor_and_erase_churn() {
    let mut term = TerminalState::new(160, 48, 2000, Theme::default());
    let mut payload = Vec::with_capacity(64 * 1024);
    for row in 1..=48 {
        let chunk = format!(
            "\x1b[{row};1H\x1b[48;2;24;26;31;38;2;190;210;255mStatus row {row:02}: compiling crate tearminal::terminal::performer...\x1b[K\x1b[{row};20H\x1b[8X\x1b[4@\x1b[4P\x1b[0m"
        );
        payload.extend_from_slice(chunk.as_bytes());
    }
    let iterations = if cfg!(debug_assertions) { 10 } else { 250 };
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        term.process_bytes(&payload);
    }
    let elapsed = start.elapsed();
    let total_mb = (payload.len() * iterations) as f64 / (1024.0 * 1024.0);
    let mb_per_sec = total_mb / elapsed.as_secs_f64();
    println!(
        "bench_csi_cursor_and_erase_churn: {iterations} frames TUI ({total_mb:.2} MB) en {elapsed:.2?}: {mb_per_sec:.2} MB/s"
    );
}

#[test]
fn test_kitty_graphics_query_support() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // Consulta con ID específico: \x1b_Ga=q,i=42\x1b\
    term.process_bytes(b"\x1b_Ga=q,i=42\x1b\\");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b_Gi=42;OK\x1b\\");

    // Consulta sin ID y terminada con BEL (\x07)
    term.pty_write_queue.clear();
    term.process_bytes(b"\x1b_Ga=q;somepayload\x07");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b_Gi=0;OK\x1b\\");

    // Consulta con modo silencioso (q=2): no debe encolar respuesta
    term.pty_write_queue.clear();
    term.process_bytes(b"\x1b_Ga=q,i=100,q=2\x1b\\");
    assert!(term.pty_write_queue.is_empty());
}

#[test]
fn test_kitty_graphics_transmit_and_display_png() {
    use base64::prelude::*;
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // Crear un PNG mínimo de 2x2 píxeles
    let img = image::RgbaImage::new(2, 2);
    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
    let b64 = BASE64_STANDARD.encode(&png_bytes);

    let seq = format!("\x1b_Ga=T,f=100,i=15;{b64}\x1b\\");
    term.process_bytes(seq.as_bytes());

    assert_eq!(term.images.len(), 1);
    assert_eq!(term.images[0].id, 15);
    assert_eq!(term.images[0].width_cols, 1);
    assert!(term.kitty_stored_images.contains_key(&15));

    // Confirmación OK enviada al PTY porque i > 0 y q == 0
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b_Gi=15;OK\x1b\\");

    // Transmisión fragmentada (m=1 y m=0)
    let raw_rgba = vec![200u8; 16];
    let b64_chunked = BASE64_STANDARD.encode(&raw_rgba);
    let (chunk1, chunk2) = b64_chunked.split_at(b64_chunked.len() / 2);
    let seq1 = format!("\x1b_Ga=T,f=32,s=2,v=2,i=25,m=1;{chunk1}\x1b\\");
    let seq2 = format!("\x1b_Gm=0;{chunk2}\x1b\\");
    term.process_bytes(seq1.as_bytes());
    assert_eq!(term.images.len(), 1);
    assert!(term.kitty_chunk_data.is_some());
    term.process_bytes(seq2.as_bytes());
    assert_eq!(term.images.len(), 2);
    assert_eq!(term.images[1].id, 25);
    assert!(term.kitty_chunk_data.is_none());

    // Secuencia dividida en varias llamadas
    term.pty_write_queue.clear();
    term.process_bytes(b"\x1b_Ga=q,i=");
    term.process_bytes(b"77\x1b\\");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b_Gi=77;OK\x1b\\");
}

#[test]
fn test_kitty_graphics_raw_rgba_and_rgb() {
    use base64::prelude::*;
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // Raw RGBA 32 bits (f=32), 2x2 = 16 bytes
    let raw_rgba = vec![255u8; 16];
    let b64_rgba = BASE64_STANDARD.encode(&raw_rgba);
    let seq_rgba = format!("\x1b_Ga=T,f=32,s=2,v=2,i=16;{b64_rgba}\x1b\\");
    term.process_bytes(seq_rgba.as_bytes());

    assert_eq!(term.images.len(), 1);
    assert_eq!(term.images[0].id, 16);

    // Raw RGB 24 bits (f=24), 2x2 = 12 bytes
    let raw_rgb = vec![128u8; 12];
    let b64_rgb = BASE64_STANDARD.encode(&raw_rgb);
    let seq_rgb = format!("\x1b_Ga=T,f=24,s=2,v=2,i=17;{b64_rgb}\x1b\\");
    term.process_bytes(seq_rgb.as_bytes());

    assert_eq!(term.images.len(), 2);
    assert_eq!(term.images[1].id, 17);
}

#[test]
fn test_kitty_graphics_transmit_put_and_delete() {
    use base64::prelude::*;
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    let raw_rgba = vec![255u8; 16];
    let b64 = BASE64_STANDARD.encode(&raw_rgba);

    // 1. Transmitir solo (a=t): se guarda pero no se añade a images
    let seq_t = format!("\x1b_Ga=t,f=32,s=2,v=2,i=30;{b64}\x1b\\");
    term.process_bytes(seq_t.as_bytes());
    assert_eq!(term.images.len(), 0);
    assert!(term.kitty_stored_images.contains_key(&30));

    // 2. Colocar / mostrar (a=p)
    term.process_bytes(b"\x1b_Ga=p,i=30\x1b\\");
    assert_eq!(term.images.len(), 1);
    assert_eq!(term.images[0].id, 30);

    // 3. Borrar por ID (a=d, d=i, i=30)
    term.process_bytes(b"\x1b_Ga=d,d=i,i=30\x1b\\");
    assert_eq!(term.images.len(), 0);
    assert!(!term.kitty_stored_images.contains_key(&30));

    // 4. Colocar otra y borrar todas (a=d, d=a)
    let seq_t2 = format!("\x1b_Ga=T,f=32,s=2,v=2,i=31;{b64}\x1b\\");
    term.process_bytes(seq_t2.as_bytes());
    assert_eq!(term.images.len(), 1);
    term.process_bytes(b"\x1b_Ga=d,d=a\x1b\\");
    assert_eq!(term.images.len(), 0);
    assert!(term.kitty_stored_images.is_empty());

    // 5. Expulsión LRU al superar el límite de 32 imágenes guardadas
    for id in 1..=40u64 {
        let seq = format!("\x1b_Ga=t,f=32,s=2,v=2,i={id},q=2;{b64}\x1b\\");
        term.process_bytes(seq.as_bytes());
    }
    assert_eq!(term.kitty_stored_images.len(), 32);
    assert!(term.kitty_stored_images.contains_key(&40));
    assert!(term.kitty_stored_images.contains_key(&9));
    assert!(!term.kitty_stored_images.contains_key(&8));
}

/// Entrada hostil: dimensiones que desbordan `u32`, un PNG con ancho fuera de tope y paneles estrechos no
/// producen pánico ni imagen, y quien espera confirmación recibe un error
#[test]
fn test_kitty_rejects_hostile_images_without_panicking() {
    use base64::prelude::*;
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // s * v * 4 = 2^34: con aritmética de 32 bits coincidiría con un tamaño pequeño
    let seq = format!(
        "\x1b_Ga=T,f=32,s=65536,v=65536,i=60;{}\x1b\\",
        BASE64_STANDARD.encode([1u8, 2, 3, 4])
    );
    term.process_bytes(seq.as_bytes());
    assert!(term.images.is_empty());
    assert!(String::from_utf8_lossy(&term.pty_write_queue[0]).contains("EBADPNG"));

    // PNG diminuto (comprime muy bien) cuyo ancho supera el tope de 16384
    let mut png = Vec::new();
    image::RgbaImage::new(20_000, 1)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let seq = format!(
        "\x1b_Ga=T,f=100,i=61,q=2;{}\x1b\\",
        BASE64_STANDARD.encode(&png)
    );
    term.process_bytes(seq.as_bytes());
    assert!(term.images.is_empty());

    // Panel de 5 columnas: antes `clamp(10, cols)` entraba en pánico
    let mut narrow = TerminalState::new(5, 24, 1000, Theme::default());
    let seq = format!(
        "\x1b_Ga=T,f=32,s=2,v=2;{}\x1b\\",
        BASE64_STANDARD.encode(vec![9u8; 16])
    );
    narrow.process_bytes(seq.as_bytes());
    assert_eq!(narrow.images.len(), 1);
}

#[test]
fn test_kitty_graphics_file_medium_and_animation_frames() {
    use base64::prelude::*;
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // 1. Crear imagen PNG de prueba en disco (%TEMP%)
    let temp_file =
        std::env::temp_dir().join(format!("tearminal_kitty_test_{}.png", std::process::id()));
    let img = image::RgbaImage::new(4, 4);
    img.save(&temp_file).unwrap();
    assert!(temp_file.exists());

    // Transmitir vía archivo regular (t=f)
    let path_b64 = BASE64_STANDARD.encode(temp_file.to_string_lossy().as_bytes());
    let seq_f = format!("\x1b_Ga=T,f=100,t=f,i=90;{path_b64}\x1b\\");
    term.process_bytes(seq_f.as_bytes());
    assert_eq!(term.images.len(), 1);
    assert_eq!(term.images[0].id, 90);
    assert!(
        temp_file.exists(),
        "t=f no debe borrar el archivo de origen"
    );

    // 2. Transmitir vía archivo temporal (t=t): debe leerlo y borrarlo
    term.images.clear();
    let seq_t = format!("\x1b_Ga=T,f=100,t=t,i=91;{path_b64}\x1b\\");
    term.process_bytes(seq_t.as_bytes());
    assert_eq!(term.images.len(), 1);
    assert_eq!(term.images[0].id, 91);
    assert!(
        !temp_file.exists(),
        "t=t debe eliminar el archivo temporal tras cargarlo"
    );

    // 3. Rechazo de rutas de red UNC por seguridad
    term.pty_write_queue.clear();
    let unc_b64 = BASE64_STANDARD.encode(br"\\attacker-server\share\exploit.png");
    let seq_unc = format!("\x1b_Ga=T,f=100,t=f,i=92;{unc_b64}\x1b\\");
    term.process_bytes(seq_unc.as_bytes());
    assert_eq!(term.images.len(), 1); // No añadió imagen
    assert_eq!(term.pty_write_queue.len(), 1);
    assert!(String::from_utf8_lossy(&term.pty_write_queue[0]).contains("EINVAL"));

    // 4. Fotogramas de animación Kitty (a=f, z=80)
    let raw_rgba = vec![180u8; 16];
    let frame_b64 = BASE64_STANDARD.encode(&raw_rgba);
    let seq_frame = format!("\x1b_Ga=f,f=32,s=2,v=2,i=91,z=80;{frame_b64}\x1b\\");
    term.process_bytes(seq_frame.as_bytes());
    assert_eq!(term.images[0].frames.len(), 1);
    assert_eq!(term.images[0].frames[0].delay_ms, 80);
    assert!(term.images[0].next_frame_delay_ms().is_none()); // solo 1 frame extra

    // Añadir segundo fotograma para activar ciclo animado
    let seq_frame2 = format!("\x1b_Ga=f,f=32,s=2,v=2,i=91,z=120;{frame_b64}\x1b\\");
    term.process_bytes(seq_frame2.as_bytes());
    assert_eq!(term.images[0].frames.len(), 2);
    assert!(term.images[0].next_frame_delay_ms().is_some());
    assert!(!term.images[0].active_rgba().is_empty());
}

#[test]
fn test_synchronized_rendering_dec_2026_and_timeout() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());
    assert!(!term.is_in_sync_update());

    // Activar modo 2026
    term.process_bytes(b"\x1b[?2026h");
    assert!(term.sync_update_mode);
    assert!(term.sync_update_start.is_some());
    assert!(term.is_in_sync_update());

    // Desactivar modo 2026
    term.process_bytes(b"\x1b[?2026l");
    assert!(!term.sync_update_mode);
    assert!(term.sync_update_start.is_none());
    assert!(!term.is_in_sync_update());

    // Timeout de seguridad si pasan más de 200 ms
    term.process_bytes(b"\x1b[?2026h");
    assert!(term.is_in_sync_update());

    // Consulta DECRQM (\x1b[?2026$p) en estado activo
    term.pty_write_queue.clear();
    term.process_bytes(b"\x1b[?2026$p");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?2026;1$y");

    term.process_bytes(b"\x1b[?2026l");
    term.pty_write_queue.clear();
    term.process_bytes(b"\x1b[?2026$p");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?2026;2$y");

    term.process_bytes(b"\x1b[?2026h");
    term.sync_update_start =
        Some(std::time::Instant::now() - std::time::Duration::from_millis(250));
    assert!(
        !term.is_in_sync_update(),
        "El timeout de seguridad de 200ms debe desbloquear el render"
    );
}

#[test]
fn test_osc_10_and_11_color_queries() {
    let theme = Theme::default();
    let mut term = TerminalState::new(80, 24, 1000, theme.clone());

    // Consulta de color de primer plano (OSC 10;?)
    term.process_bytes(b"\x1b]10;?\x07");
    assert_eq!(term.pty_write_queue.len(), 1);
    let fg = theme.foreground;
    let expected_fg = format!(
        "\x1b]10;rgb:{:04x}/{:04x}/{:04x}\x1b\\",
        fg.r as u16 * 0x0101,
        fg.g as u16 * 0x0101,
        fg.b as u16 * 0x0101
    );
    assert_eq!(term.pty_write_queue[0], expected_fg.as_bytes());

    term.pty_write_queue.clear();

    // Consulta de color de fondo (OSC 11;?)
    term.process_bytes(b"\x1b]11;?\x1b\\");
    assert_eq!(term.pty_write_queue.len(), 1);
    let bg = theme.background;
    let expected_bg = format!(
        "\x1b]11;rgb:{:04x}/{:04x}/{:04x}\x1b\\",
        bg.r as u16 * 0x0101,
        bg.g as u16 * 0x0101,
        bg.b as u16 * 0x0101
    );
    assert_eq!(term.pty_write_queue[0], expected_bg.as_bytes());
}

#[test]
fn test_dec_2031_color_scheme_mode() {
    let dark_theme = Theme::from_name(Theme::DEFAULT_DARK);
    let mut term = TerminalState::new(80, 24, 1000, dark_theme);

    // Activar DEC 2031 (modo oscuro reporta 1)
    term.process_bytes(b"\x1b[?2031h");
    assert!(term.report_color_scheme_mode);
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?2031;1$y");

    term.pty_write_queue.clear();

    // Consulta DECRQM \x1b[?2031$p
    term.process_bytes(b"\x1b[?2031$p");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?2031;1$y");

    term.pty_write_queue.clear();

    // Desactivar DEC 2031
    term.process_bytes(b"\x1b[?2031l");
    assert!(!term.report_color_scheme_mode);

    // Tema claro reporta 2
    let light_theme = Theme::from_name(Theme::DEFAULT_LIGHT);
    let mut light_term = TerminalState::new(80, 24, 1000, light_theme);
    light_term.process_bytes(b"\x1b[?2031h");
    assert_eq!(light_term.pty_write_queue[0], b"\x1b[?2031;2$y");
}

#[test]
fn test_kitty_keyboard_protocol_csi_u_push_pop_and_query() {
    let mut term = TerminalState::new(80, 24, 1000, Theme::default());

    // Query flags inicial (debe ser 0)
    term.process_bytes(b"\x1b[?u");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?0u");
    term.pty_write_queue.clear();

    // Push flag 1 (disambiguate)
    term.process_bytes(b"\x1b[>1u");
    assert_eq!(term.kitty_keyboard_flags, 1);
    assert_eq!(term.kitty_keyboard_stack, vec![0]);

    // Push OR flag 2 (report event types): mode 2
    term.process_bytes(b"\x1b[>2;2u");
    assert_eq!(term.kitty_keyboard_flags, 3);
    assert_eq!(term.kitty_keyboard_stack, vec![0, 1]);

    // Query flags activas (debe ser 3)
    term.process_bytes(b"\x1b[?u");
    assert_eq!(term.pty_write_queue.len(), 1);
    assert_eq!(term.pty_write_queue[0], b"\x1b[?3u");
    term.pty_write_queue.clear();

    // Pop 1
    term.process_bytes(b"\x1b[<u");
    assert_eq!(term.kitty_keyboard_flags, 1);
    assert_eq!(term.kitty_keyboard_stack, vec![0]);

    // Pop final
    term.process_bytes(b"\x1b[<1u");
    assert_eq!(term.kitty_keyboard_flags, 0);
    assert!(term.kitty_keyboard_stack.is_empty());
}

#[test]
fn test_scrollback_preserves_reading_position_when_new_output_arrives() {
    let theme = Theme::from_name(Theme::DEFAULT_DARK);
    let mut term = TerminalState::new(80, 5, 100, theme);

    // Escribir 10 líneas de contenido
    for i in 1..=10 {
        term.process_bytes(format!("Línea {i}\r\n").as_bytes());
    }

    // El scrollback tiene 6 líneas (filas 1..=6) y la pantalla tiene 5 líneas (filas 7..=10 + línea actual)
    assert!(term.scrollback.len() >= 5);
    assert_eq!(term.scroll_offset, 0);

    // El usuario se desplaza 3 líneas hacia arriba para leer
    term.scroll_up(3);
    assert_eq!(term.scroll_offset, 3);
    let visible_before: Vec<String> = term
        .visible_rows()
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| c.c)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();

    // Llega nueva salida de fondo (2 líneas más)
    term.process_bytes(b"Nueva linea 11\r\nNueva linea 12\r\n");

    // scroll_offset se ajustó automáticamente para mantener la misma vista
    assert_eq!(term.scroll_offset, 5);
    let visible_after: Vec<String> = term
        .visible_rows()
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| c.c)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();

    // Las filas visibles para el usuario son EXACTAMENTE las mismas (no se movieron)
    assert_eq!(visible_before, visible_after);

    // Cuando el usuario regresa al fondo, scroll_offset vuelve a 0 y ve la nueva salida
    term.reset_scroll();
    assert_eq!(term.scroll_offset, 0);
    let visible_bottom: Vec<String> = term
        .visible_rows()
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| c.c)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    assert!(visible_bottom
        .iter()
        .any(|line| line.contains("Nueva linea 12")));
}

#[test]
fn test_terminal_update_theme_migrates_colors_and_preserves_contrast() {
    let dark_theme = Theme::from_name(Theme::DEFAULT_DARK);
    let light_theme = Theme::from_name(Theme::DEFAULT_LIGHT);
    let mut term = TerminalState::new(80, 5, 100, dark_theme.clone());

    // Escribir texto por defecto y con secuencias ANSI
    // "Hola": texto default fg (en dark_theme = dark_theme.foreground)
    // "\x1b[31mRojo": color ANSI 1
    // "\x1b[90mGris": color ANSI 8
    term.process_bytes(b"Hola \x1b[31mRojo\x1b[0m \x1b[90mGris\x1b[0m\r\n");

    let row = &term.grid[0];
    let cell_hola = row[0]; // 'H'
    let cell_rojo = row[5]; // 'R'
    let cell_gris = row[10]; // 'G'

    assert_eq!(cell_hola.fg, dark_theme.foreground);
    assert_eq!(cell_rojo.fg, dark_theme.ansi[1]);
    assert_eq!(cell_gris.fg, dark_theme.ansi[8]);
    assert_eq!(term.current_fg, dark_theme.foreground);

    // Cambiar a tema claro
    term.update_theme(light_theme.clone());

    let row_after = &term.grid[0];
    let cell_hola_after = row_after[0];
    let cell_rojo_after = row_after[5];
    let cell_gris_after = row_after[10];

    // Ahora los colores se han migrado coherentemente para el nuevo tema claro
    assert_eq!(cell_hola_after.fg, light_theme.foreground);
    assert_eq!(cell_rojo_after.fg, light_theme.ansi[1]);
    assert_eq!(term.current_fg, light_theme.foreground);

    // El contraste de todos ellos sobre el fondo claro es perfectamente legible (>= 2.0)
    // El contraste de todos ellos sobre el fondo claro es perfectamente legible (>= 2.0)
    assert!(crate::theme::contrast_ratio(cell_hola_after.fg, light_theme.background) >= 3.0);
    assert!(crate::theme::contrast_ratio(cell_rojo_after.fg, light_theme.background) >= 2.0);
    assert!(crate::theme::contrast_ratio(cell_gris_after.fg, light_theme.background) >= 2.0);
}
