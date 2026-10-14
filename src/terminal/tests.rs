//! Tests del emulador de terminal: secuencias VT, scrollback, Unicode y rendimiento.

use super::*;
use std::time::Instant;

#[test]
fn test_ghostty_palette() {
    let theme = Theme::ghostty_dark();
    assert_eq!(theme.background, Color::from_rgb(22, 22, 24));
    assert_eq!(theme.cursor, Color::from_rgb(255, 255, 255));
    assert_eq!(theme.ansi.len(), 16);
}

#[test]
fn test_truecolor_parsing() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    // Secuencia TrueColor: RGB(120, 200, 250)
    let seq = b"\x1b[38;2;120;200;250mHola TrueColor\x1b[0m";
    term.process_bytes(seq);

    let cell = &term.grid[0][0];
    assert_eq!(cell.c, 'H');
    assert_eq!(cell.fg, Color::from_rgb(120, 200, 250));
}

#[test]
fn test_cursor_and_clear_screen() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
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
        "\n⚡ [Axon Benchmark] Procesados {} bytes ({} líneas) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
        total_bytes, iterations, elapsed, throughput_mb, lines_per_sec
    );

    assert!(
        throughput_mb > 3.0,
        "El throughput debe superar los 3 MB/s en debug y decenas de MB/s en release"
    );
}

#[test]
fn test_iterm2_image_protocol() {
    let theme = Theme::ghostty_dark();
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
fn test_alt_screen_buffer_tmux() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 20, 1000, theme);

    // Definir margen de desplazamiento de la línea 5 a la 10 (split pane de tmux)
    term.process_bytes(b"\x1b[5;10r");
    assert_eq!(term.scroll_region_top, 4);
    assert_eq!(term.scroll_region_bottom, 9);
}

#[test]
fn bench_massive_scrollback_100k_lines() {
    let theme = Theme::ghostty_dark();
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
        "\n🚀 [Axon Massive Stream Test] Procesadas {} líneas ({} MB) en {:.2?}: {:.2} MB/s | {:.0} líneas/segundo",
        count, (total_bytes / (1024 * 1024)), elapsed, throughput_mb, lines_per_sec
    );

    assert_eq!(
        term.scrollback.len() + term.rows - 1,
        count,
        "Debe retener las 100.000 líneas entre scrollback y pantalla activa"
    );
    assert!(
        lines_per_sec > 50_000.0,
        "Debe superar las 50.000 líneas por segundo en debug y >500.000 en release"
    );
}

#[test]
fn test_osc_52_clipboard() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Enviar secuencia OSC 52 con texto "Hello from SSH!" en base64 (SGVsbG8gZnJvbSBTU0gh)
    term.process_bytes(b"\x1b]52;c;SGVsbG8gZnJvbSBTU0gh\x07");

    assert_eq!(
        term.clipboard_queue.len(),
        1,
        "Debe agregar el texto decodificado a la cola de portapapeles"
    );
    assert_eq!(term.clipboard_queue[0], "Hello from SSH!");
}

#[test]
fn test_osc_133_semantic_prompts() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
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
fn test_powershell_full_startup_stream() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    let raw = b"\x1b[?9001h\x1b[?1004h\x1b[?25l\x1b[2J\x1b[m\x1b[HWindows PowerShell\r\nCopyright (C) Microsoft Corporation. All rights reserved.\x1b]0;C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\x07\x1b[?25h\x1b[?25l\x1b[4;1H\x1b[?25hPS C:\\Users\\gmora> ";
    term.process_bytes(raw);

    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            println!(
                "ROW {}: {:?} (cell[0].fg: {:?}, bg: {:?})",
                i, trimmed, row[0].fg, row[0].bg
            );
        }
    }
    println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);

    // Simular redimensionamiento de ventana al inicio (de 24 a 20 filas)
    println!("--- AFTER RESIZE to 20 rows ---");
    term.resize(80, 20);
    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            println!("GRID ROW {}: {:?}", i, trimmed);
        }
    }
    for (i, row) in term.scrollback.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            println!("SCROLLBACK ROW {}: {:?}", i, trimmed);
        }
    }
    println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);
    assert!(term.cursor_x > 0);
}

#[test]
fn test_unicode_double_width_emojis_and_cjk() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
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
fn test_truecolor_24bit_rgb_semicolon_and_colon() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // 1. TrueColor formato punto y coma estándar (38;2;R;G;B y 48;2;R;G;B)
    term.process_bytes(b"\x1b[38;2;250;120;40;48;2;15;25;35mX\x1b[0m");
    let cell_x = &term.grid[0][0];
    assert_eq!(cell_x.c, 'X');
    assert_eq!(cell_x.fg, Color::from_rgb(250, 120, 40));
    assert_eq!(cell_x.bg, Color::from_rgb(15, 25, 35));

    // 2. TrueColor formato delimitado por dos puntos (38:2:R:G:B)
    term.process_bytes(b"\x1b[38:2:180:90:220mY\x1b[0m");
    let cell_y = &term.grid[0][1];
    assert_eq!(cell_y.c, 'Y');
    assert_eq!(cell_y.fg, Color::from_rgb(180, 90, 220));

    // 3. TrueColor formato ISO con espacio de color (38:2:0:70:140:210)
    term.process_bytes(b"\x1b[38:2::70:140:210mZ\x1b[0m");
    let cell_z = &term.grid[0][2];
    assert_eq!(cell_z.c, 'Z');
    assert_eq!(cell_z.fg, Color::from_rgb(70, 140, 210));
}

#[test]
fn test_sgr_text_styles_dim_bold_underline_strikethrough() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);
    let raw = b"\x1b[?9001h\x1b[?1004h\x1b[2J\x1b[?25l\x1b[2J\x1b[m\x1b[H\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[H\x1b]0;pwsh.exe\x07\x1b[?25h\x1b[38;2;78;146;249m\xef\x81\xbb ~\x1b[38;2;81;226;122m\r\n\xef\x90\xb2 ";
    term.process_bytes(raw);
    for (i, row) in term.grid.iter().enumerate() {
        let s: String = row.iter().map(|c| c.c).collect();
        let trimmed = s.trim_end();
        if !trimmed.is_empty() {
            println!("ROW {}: {:?}", i, trimmed);
        }
    }
    println!("cursor_x: {}, cursor_y: {}", term.cursor_x, term.cursor_y);
    assert!(term.cursor_x > 0);
}

#[test]
fn test_ech_erase_characters() {
    let theme = Theme::ghostty_dark();
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
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Establecer color de fondo azul (44) y borrar línea con \x1b[2K
    term.process_bytes(b"\x1b[44m\x1b[2K");
    assert_eq!(term.grid[0][0].bg, term.theme.ansi[4]);
    assert_eq!(term.grid[0][79].bg, term.theme.ansi[4]);
}

#[test]
fn test_osc_cwd_reporting() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Test OSC 7
    term.process_bytes(b"\x1b]7;file://localhost/C:/Users/gmora/dev/axon\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:/Users/gmora/dev/axon"))
    );

    // Test OSC 9;9
    term.process_bytes(b"\x1b]9;9;\"C:\\Users\\gmora\\dev\"\x07");
    assert_eq!(
        term.current_dir.as_deref(),
        Some(std::path::Path::new("C:\\Users\\gmora\\dev"))
    );
}

#[test]
fn test_osc_window_title_and_ssh_inference() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Envío de título de ventana tradicional OSC 0
    term.process_bytes(b"\x1b]0;My Custom Window\x07");
    assert_eq!(term.window_title.as_deref(), Some("My Custom Window"));
    assert!(term.ssh_session.is_none());

    // Inferencia de sesión SSH mediante título "ssh: user@host"
    term.process_bytes(b"\x1b]0;ssh: dev@server.example.com\x07");
    assert_eq!(
        term.window_title.as_deref(),
        Some("ssh: dev@server.example.com")
    );
    let sess = term
        .ssh_session
        .as_ref()
        .expect("Debe inferir la sesión SSH desde el título");
    assert_eq!(sess.destination, "dev@server.example.com");
    assert!(sess.is_active);
}

#[test]
fn test_osc_axon_ssh_signaling() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // Inicio de sesión SSH mediante protocolo OSC 1337 AxonSSH=start
    term.process_bytes(b"\x1b]1337;AxonSSH=start;dest=ubuntu@production-node-1\x07");
    let sess = term
        .ssh_session
        .as_ref()
        .expect("Debe registrar la sesión SSH activa");
    assert_eq!(sess.destination, "ubuntu@production-node-1");
    assert!(sess.is_active);

    // Finalización de sesión SSH mediante OSC 1337 AxonSSH=end
    term.process_bytes(b"\x1b]1337;AxonSSH=end\x07");
    assert!(
        term.ssh_session.is_none(),
        "Debe limpiar la sesión SSH al desconectarse"
    );
}

#[test]
fn test_osc_7_remote_host_ssh() {
    let theme = Theme::ghostty_dark();
    let mut term = TerminalState::new(80, 24, 1000, theme);

    // OSC 7 emitido por una shell remota con hostname no local
    term.process_bytes(b"\x1b]7;file://bastion.corp.internal/home/admin/projects\x07");
    let sess = term
        .ssh_session
        .as_ref()
        .expect("Debe detectar host remoto vía OSC 7");
    assert_eq!(sess.destination, "bastion.corp.internal");
    assert_eq!(sess.remote_dir.as_deref(), Some("/home/admin/projects"));
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

/// Las secuencias de escape son entrada no confiable: no pueden fijar un destino SSH con metacaracteres
/// (se interpola en una orden de PowerShell al dividir un panel).
#[test]
fn test_untrusted_escape_sequences_cannot_set_unsafe_ssh_session() {
    let new = || TerminalState::new(80, 24, 100, Theme::ghostty_dark());

    let mut t = new();
    t.process_bytes(b"\x1b]0;ssh: user@host\x07");
    assert_eq!(
        t.ssh_session.as_ref().map(|s| s.destination.as_str()),
        Some("user@host")
    );

    // OSC usa `;` como separador de parámetros, así que los vectores reales son otros metacaracteres
    for evil in [
        &b"\x1b]0;ssh: x && calc\x07"[..],
        b"\x1b]0;ssh: $(calc)\x07",
        b"\x1b]2;ssh: x|calc\x07",
        b"\x1b]2;ssh: -oProxyCommand=calc\x07",
        b"\x1b]1337;AxonSSH=start;dest=$(calc)\x07",
        b"\x1b]1337;AxonSSH=start;dest=-oProxyCommand=calc\x07",
        b"\x1b]7;file://evil$(calc)/home/x\x07",
    ] {
        let mut t = new();
        t.process_bytes(evil);
        assert!(
            t.ssh_session.is_none(),
            "no debe crear sesión SSH con {:?}",
            String::from_utf8_lossy(evil)
        );
    }

    // host válido por OSC 7 pero directorio con comillas: la sesión se crea sin directorio remoto
    let mut t = new();
    t.process_bytes(b"\x1b]7;file://server/tmp/'$(calc)'\x07");
    let s = t.ssh_session.as_ref().expect("host válido");
    assert_eq!(s.destination, "server");
    assert_eq!(s.remote_dir, None);
}
