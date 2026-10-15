//! Tests de la aplicación: pestañas, paneles, ajustes, atajos y portapapeles.

use super::*;
use crate::config::AxonConfig;

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
fn test_settings_tab_actions_and_theme_switching() {
    struct ConfigRestorer(Option<String>);
    impl Drop for ConfigRestorer {
        fn drop(&mut self) {
            if let Some(content) = &self.0 {
                let _ = std::fs::write(AxonConfig::config_path(), content);
            }
        }
    }
    let _restorer = ConfigRestorer(std::fs::read_to_string(AxonConfig::config_path()).ok());

    let config = AxonConfig::load_or_default();
    let mut app = AxonApp::new(config, None);

    // 1. Abrir o cambiar a pestaña de Ajustes (siempre navega a Ajustes sin intercambiar)
    assert!(!app.tabs[app.active_tab_index].is_settings);
    app.open_or_switch_settings_tab();
    assert!(app.tabs[app.active_tab_index].is_settings);
    app.open_or_switch_settings_tab();
    assert!(app.tabs[app.active_tab_index].is_settings);
    app.switch_to_tab(0);
    assert!(!app.tabs[app.active_tab_index].is_settings);

    // 2. Cambio cíclico de temas
    let initial_theme = app.config.appearance.theme.clone();
    app.cycle_theme(true);
    assert_ne!(app.config.appearance.theme, initial_theme);
    assert!(!app.theme.ansi.is_empty());
    assert!(crate::theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Frappé"));

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
fn test_settings_reset_to_defaults() {
    struct ConfigRestorer(Option<String>);
    impl Drop for ConfigRestorer {
        fn drop(&mut self) {
            if let Some(content) = &self.0 {
                let _ = std::fs::write(AxonConfig::config_path(), content);
            }
        }
    }
    let _restorer = ConfigRestorer(std::fs::read_to_string(AxonConfig::config_path()).ok());

    let config = AxonConfig::load_or_default();
    let mut app = AxonApp::new(config, None);

    // Modificar varios parámetros a valores no predeterminados
    app.config.appearance.font_size = 22.0;
    app.config.appearance.window_opacity = 0.65;
    app.config.appearance.theme = "Nord".to_string();
    app.config.appearance.tab_layout = "vertical".to_string();
    app.config.terminal.cursor_blink = false;

    // Restablecer valores por defecto
    app.reset_to_defaults();

    // Validar que se restauraron los valores por defecto
    assert_eq!(app.config.appearance.font_size, 15.0);
    assert_eq!(app.config.appearance.ui_font_size, 15.0);
    assert_eq!(app.config.appearance.window_opacity, 1.0);
    assert_eq!(app.config.appearance.theme, "frappe");
    assert_eq!(app.config.appearance.tab_layout, "horizontal");
    assert!(app.config.terminal.cursor_blink);
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
    app.config.appearance.tab_layout = "vertical".to_string();
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
    let pane1 = TerminalPane::new(1, pty1, term1, (80, 24));

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
    let pane2 = TerminalPane::new(2, pty2, term2, (80, 24));

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

/// Un portapapeles malicioso no debe poder cerrar el pegado entre corchetes ni emitir secuencias de terminal.
#[test]
fn test_paste_is_sanitized_against_escape_injection() {
    use super::clipboard::sanitize_paste;
    let out = sanitize_paste("ls\x1b[201~\x1b]0;x\x07rm -rf /\r\nfin\tñ");
    assert!(!out.contains('\x1b'), "sin ESC: {out:?}");
    assert!(!out.contains('\x07'), "sin BEL: {out:?}");
    assert!(
        !out.contains('\n'),
        "saltos de línea normalizados a CR: {out:?}"
    );
    assert_eq!(out, "ls[201~]0;xrm -rf /\rfin\tñ");
}

#[test]
fn test_file_viewer_tab_creation_and_title() {
    let cargo_toml = std::path::PathBuf::from("Cargo.toml");
    assert!(cargo_toml.exists());

    let tab = TerminalTab::new_file_viewer(42, cargo_toml.clone());
    assert_eq!(tab.id, 42);
    assert!(!tab.is_settings);
    assert!(tab.file_viewer.is_some());
    assert_eq!(tab.display_title(), "Cargo.toml");

    let viewer = tab.file_viewer.as_ref().unwrap();
    assert_eq!(viewer.file_name, "Cargo.toml");
    assert!(!viewer.is_binary);
    assert!(!viewer.lines.is_empty());
    assert!(viewer.file_size > 0);

    // Panes debe ser vacío y active_pane debe devolver None
    assert!(tab.panes.is_empty());
    assert!(tab.active_pane().is_none());
    assert!(tab.compute_pane_rects(0, 0, 800, 600).is_empty());
    assert!(tab.compute_dividers(0, 0, 800, 600).is_empty());
}

#[test]
fn test_file_viewer_editing_and_dirty_state() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("axon_test_edit_{}.rs", std::process::id()));
    let _ = std::fs::write(&temp_file, "");

    let mut tab = TerminalTab::new_file_viewer(99, temp_file.clone());

    assert_eq!(
        tab.display_title(),
        temp_file.file_name().unwrap().to_str().unwrap()
    );

    let viewer = tab.file_viewer.as_mut().unwrap();
    assert!(!viewer.is_dirty);
    viewer.insert_char('f');
    viewer.insert_char('n');
    assert!(viewer.is_dirty);
    assert_eq!(viewer.cursor_col, 2);

    viewer.backspace();
    assert_eq!(viewer.cursor_col, 1);
    assert_eq!(viewer.lines[0], "f");

    viewer.insert_newline();
    assert_eq!(viewer.cursor_line, 1);
    assert_eq!(viewer.cursor_col, 0);
    assert_eq!(viewer.lines.len(), 2);

    assert_eq!(
        tab.display_title(),
        format!("● {}", temp_file.file_name().unwrap().to_str().unwrap())
    );

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_file_viewer_undo_redo() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("axon_test_undo_{}.txt", std::process::id()));
    let _ = std::fs::write(&temp_file, "hello world");

    let mut tab = TerminalTab::new_file_viewer(100, temp_file.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();

    viewer.cursor_col = 5;
    viewer.insert_text(" beautiful");
    assert_eq!(viewer.lines[0], "hello beautiful world");

    viewer.undo();
    assert_eq!(viewer.lines[0], "hello world");
    assert_eq!(viewer.cursor_col, 5);

    viewer.redo();
    assert_eq!(viewer.lines[0], "hello beautiful world");

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_file_viewer_word_navigation_and_deletion() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("axon_test_words_{}.txt", std::process::id()));
    let _ = std::fs::write(&temp_file, "foo bar_baz  qux");

    let mut tab = TerminalTab::new_file_viewer(101, temp_file.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();

    // cursor at end
    viewer.cursor_col = viewer.lines[0].chars().count();
    let left1 = viewer.find_word_boundary_left();
    assert_eq!(left1, 13); // inicio de "qux"
    viewer.cursor_col = left1;

    let left2 = viewer.find_word_boundary_left();
    assert_eq!(left2, 4); // inicio de "bar_baz"
    viewer.cursor_col = left2;

    let right1 = viewer.find_word_boundary_right();
    assert_eq!(right1, 13); // después de "bar_baz  "

    // Borrado de palabra hacia la izquierda
    viewer.cursor_col = 7; // después de "foo bar"
    viewer.delete_word_left();
    assert_eq!(viewer.lines[0], "foo _baz  qux");

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_file_viewer_git_diff_status() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("axon_test_diff_{}.txt", std::process::id()));
    let _ = std::fs::write(&temp_file, "line1\nline2\nline3");

    let mut tab = TerminalTab::new_file_viewer(102, temp_file.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();

    assert_eq!(viewer.get_line_git_diff_status(0), None);
    assert_eq!(viewer.get_line_git_diff_status(1), None);

    viewer.lines[1] = "line2 modified".to_string();
    assert_eq!(
        viewer.get_line_git_diff_status(1),
        Some(crate::app::layout::GitLineStatus::Modified)
    );

    viewer.lines.push("line4 added".to_string());
    assert_eq!(
        viewer.get_line_git_diff_status(3),
        Some(crate::app::layout::GitLineStatus::Added)
    );

    let _ = std::fs::remove_file(temp_file);
}
