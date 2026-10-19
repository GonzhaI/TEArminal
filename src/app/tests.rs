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

    // 2. Cambio de temas
    app.apply_theme("Nord");
    assert_eq!(app.config.appearance.theme, "Nord");
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
    assert_eq!(dividers[0].rect, (0, h_rects[0].height, 1000, 2));

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
    viewer.is_read_only = false;
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
    viewer.is_read_only = false;

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
    viewer.is_read_only = false;

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
    viewer.is_read_only = false;

    assert_eq!(viewer.get_line_git_diff_status(0), None);
    assert_eq!(viewer.get_line_git_diff_status(1), None);

    viewer.lines[1] = "line2 modified".to_string();
    viewer.edit_gen += 1;
    assert_eq!(
        viewer.get_line_git_diff_status(1),
        Some(crate::app::viewer::GitLineStatus::Modified)
    );

    viewer.lines.push("line4 added".to_string());
    viewer.edit_gen += 1;
    assert_eq!(
        viewer.get_line_git_diff_status(3),
        Some(crate::app::viewer::GitLineStatus::Added)
    );

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_file_viewer_selection_and_deletion() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("axon_test_sel_{}.txt", std::process::id()));
    let _ = std::fs::write(&temp_file, "hello beautiful world\nsecond line here");

    let mut tab = TerminalTab::new_file_viewer(103, temp_file.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();
    viewer.is_read_only = false;

    // Sin selección inicial
    assert!(!viewer.has_selection());
    assert_eq!(viewer.selected_text(), None);

    // Seleccionar palabra "beautiful"
    viewer.cursor_line = 0;
    viewer.cursor_col = 8; // dentro de "beautiful"
    viewer.select_word_at_cursor();
    assert!(viewer.has_selection());
    assert_eq!(viewer.selected_text().as_deref(), Some("beautiful"));

    // Borrar la selección
    assert!(viewer.delete_selection());
    assert_eq!(viewer.lines[0], "hello  world");
    assert!(!viewer.has_selection());

    // Seleccionar línea completa
    viewer.cursor_line = 1;
    viewer.cursor_col = 3;
    viewer.select_line_at_cursor();
    assert_eq!(viewer.selected_text().as_deref(), Some("second line here"));

    // Seleccionar todo
    viewer.select_all();
    assert_eq!(
        viewer.selected_text().as_deref(),
        Some("hello  world\nsecond line here")
    );

    // Borrar todo
    assert!(viewer.delete_selection());
    assert_eq!(viewer.lines.len(), 1);
    assert_eq!(viewer.lines[0], "");

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_terminal_pane_git_branch_ssh_isolation() {
    let theme = crate::theme::Theme::ghostty_default_styledark();
    let pty =
        crate::pty::PtyManager::new(1, "powershell.exe", false, 80, 24, None, None).expect("pty");
    let term = crate::terminal::TerminalState::new(80, 24, 1000, theme);
    let mut pane = TerminalPane::new(1, pty, term, (80, 24));
    pane.terminal.current_dir = Some(std::path::PathBuf::from("C:\\Users\\Mock\\Repo"));

    // Sin SSH activo: se consulta el repositorio local (o None si no existe el path mock)
    pane.terminal.ssh_session = None;
    let _ = pane.get_git_branch();

    // Con SSH activo: NUNCA debe devolver la rama local, siempre debe aislar
    pane.terminal.ssh_session = Some(crate::terminal::SshSession {
        destination: "user@remotehost".to_string(),
        remote_dir: Some("/home/user/app".to_string()),
        remote_git_branch: None,
        is_active: true,
    });
    assert_eq!(pane.get_git_branch(), None);

    // Cuando se detecta una rama remota: debe devolver la rama remota
    if let Some(ref mut s) = pane.terminal.ssh_session {
        s.remote_git_branch = Some("feature-remote".to_string());
    }
    assert_eq!(pane.get_git_branch().as_deref(), Some("feature-remote"));
}

#[test]
fn test_file_explorer_remote_session_state() {
    let mut explorer = crate::explorer::FileExplorer::new(None, true);
    assert!(explorer.remote_session.is_none());

    let remote = crate::explorer::RemoteSshSession {
        destination: "admin@server".to_string(),
        remote_dir: "/var/www".to_string(),
    };

    // Configurar sesión remota: no se conecta aquí, solo deja el listado pedido para un hilo
    explorer.set_remote_session(Some(remote.clone()));
    assert_eq!(explorer.remote_session.as_ref(), Some(&remote));
    assert_eq!(explorer.root_dir, std::path::PathBuf::from("/var/www"));

    // Desactivar sesión remota y volver a local
    explorer.set_remote_session(None);
    assert!(explorer.remote_session.is_none());
}

fn ssh_output(stdout: &str) -> Result<crate::ssh::Output, String> {
    Ok(crate::ssh::Output {
        ok: true,
        truncated: false,
        stdout: stdout.as_bytes().to_vec(),
        stderr: String::new(),
    })
}

/// El explorador remoto por SSH lento nunca ejecuta `ssh` él mismo: pide el listado, no lo repite mientras la
/// sesión sea la misma (aunque el servidor devuelva otra raíz canónica) e ignora respuestas de una vista anterior
#[test]
fn test_explorer_remote_listing_is_requested_not_run_and_stale_replies_ignored() {
    let mut explorer = crate::explorer::FileExplorer::new(None, true);
    let remote = crate::explorer::RemoteSshSession {
        destination: "admin@server".to_string(),
        remote_dir: ".".to_string(),
    };

    explorer.set_remote_session(Some(remote.clone()));
    let mut reqs = explorer.take_ssh_requests();
    assert_eq!(reqs.len(), 1);
    let first = reqs.remove(0);
    assert_eq!(
        (first.dest.as_str(), first.dir.as_str()),
        ("admin@server", ".")
    );
    assert!(first.expand.is_none());

    // Respuesta válida: raíz canónica, rama y entradas
    let branch = explorer.apply_remote_listing(
        &first,
        ssh_output("/home/admin\nmain\n---\n./\n../\nsrc/\nREADME.md\n"),
    );
    assert_eq!(branch.as_deref(), Some("main"));
    let names: Vec<_> = explorer.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["src", "README.md"]);

    // Misma sesión pedida otra vez (la raíz ahora es canónica): no se vuelve a pedir
    explorer.set_remote_session(Some(remote));
    assert!(explorer.take_ssh_requests().is_empty());

    // Una respuesta de una generación anterior no pisa la vista actual
    explorer.refresh();
    let second = explorer.take_ssh_requests().remove(0);
    assert!(second.gen > first.gen);
    assert!(explorer
        .apply_remote_listing(&first, ssh_output("/tmp\n\n---\nintruso\n"))
        .is_none());
    assert_eq!(explorer.items.len(), 2);
}

/// La salida del servidor es entrada no confiable: el listado raíz pasa por `clean_listing` (sin `/`, control,
/// `.` ni `..`) y la carpeta canónica debe ser absoluta y sin caracteres de control
#[test]
fn test_explorer_remote_root_listing_filters_hostile_output() {
    let mut explorer = crate::explorer::FileExplorer::new(None, true);
    explorer.set_remote_session(Some(crate::explorer::RemoteSshSession {
        destination: "admin@server".to_string(),
        remote_dir: "/srv".to_string(),
    }));
    let req = explorer.take_ssh_requests().remove(0);
    explorer.apply_remote_listing(
        &req,
        ssh_output("/srv\nmain\n---\n../\nok.txt\n../../etc/passwd\nevil\x1b[2J\nlibs/\n.git/\n"),
    );
    let names: Vec<_> = explorer.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["libs", "ok.txt"]);

    // Carpeta canónica con caracteres de control o relativa: se descarta el listado entero
    for pwd in ["rel/path", "/srv/\x1b]0;x\x07"] {
        explorer.refresh();
        let req = explorer.take_ssh_requests().remove(0);
        explorer.apply_remote_listing(&req, ssh_output(&format!("{pwd}\n\n---\na.txt\n")));
        assert!(explorer.items.is_empty(), "{pwd:?}");
    }
}

/// Expandir una carpeta remota pide su listado y los hijos se insertan al llegar; un fallo la contrae
#[test]
fn test_explorer_remote_expand_is_async() {
    let mut explorer = crate::explorer::FileExplorer::new(None, true);
    explorer.set_remote_session(Some(crate::explorer::RemoteSshSession {
        destination: "admin@server".to_string(),
        remote_dir: "/srv".to_string(),
    }));
    let root = explorer.take_ssh_requests().remove(0);
    explorer.apply_remote_listing(&root, ssh_output("/srv\n\n---\napp/\nlogs/\n"));

    explorer.toggle_expand(0);
    assert_eq!(
        explorer.items.len(),
        2,
        "sin hijos hasta que llegue la respuesta"
    );
    let req = explorer.take_ssh_requests().remove(0);
    assert_eq!(
        req.expand.as_deref(),
        Some(std::path::Path::new("/srv/app"))
    );
    assert_eq!(req.dir, "/srv/app");

    explorer.apply_remote_listing(&req, ssh_output("./\n../\nmain.rs\nlib/\n"));
    let names: Vec<_> = explorer.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["app", "lib", "main.rs", "logs"]);

    // Fallo del servidor: la carpeta queda contraída
    explorer.toggle_expand(3);
    let req = explorer.take_ssh_requests().remove(0);
    explorer.apply_remote_listing(
        &req,
        Ok(crate::ssh::Output {
            ok: false,
            truncated: false,
            stdout: Vec::new(),
            stderr: "permission denied".into(),
        }),
    );
    assert!(!explorer.items[3].is_expanded);
}

#[test]
fn test_file_viewer_read_only_and_modes() {
    let temp_md = std::env::temp_dir().join(format!(
        "test_doc_{}.md",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&temp_md, "# Title\nLine 1\nLine 2").expect("write temp md");

    let mut tab = TerminalTab::new_file_viewer(99, temp_md.clone());
    let viewer = tab.file_viewer.as_mut().expect("viewer");

    // Todo archivo arranca en modo lectura y los .md con render_markdown activo
    assert!(viewer.is_read_only);
    assert!(viewer.render_markdown);

    // Auto-guardar global en configuración
    let mut config = crate::config::AxonConfig::default();
    assert!(!config.terminal.auto_save);
    config.terminal.auto_save = true;
    assert!(config.terminal.auto_save);

    // Alternar modo lectura a edición
    viewer.is_read_only = false;
    assert!(!viewer.is_read_only);

    // Alternar vista previa de markdown a raw
    viewer.render_markdown = false;
    assert!(!viewer.render_markdown);

    let _ = std::fs::remove_file(temp_md);
}

#[test]
fn test_status_bar_ssh_with_cwd_and_branch() {
    let config = crate::config::AxonConfig::default();
    let mut renderer = crate::renderer::TerminalRenderer::new(&config, 1.0);
    let mut buffer = vec![0u32; 1000 * 600];
    let theme = crate::theme::Theme::ghostty_default_styledark();

    // Renderizar barra de estado con SSH Host, CWD y rama Git simultáneamente
    renderer.render_status_bar(
        &mut buffer,
        1000,
        600,
        0,
        Some("/home/django"),
        Some("qa"),
        Some("developer@192.168.31.240"),
        120,
        35,
        crate::renderer::AiAgentStatus::None,
        false,
        &theme,
    );

    // Debe renderizar sin panics y pintar la barra
    let bar_pixel = buffer[599 * 1000 + 50];
    assert_ne!(bar_pixel, 0);
}

#[test]
fn test_branch_from_output_line_multilanguage() {
    use super::shell_context::branch_from_output_line as b;
    assert_eq!(b("En la rama qa").as_deref(), Some("qa"));
    assert_eq!(b("On branch feature/x-1").as_deref(), Some("feature/x-1"));
    assert_eq!(b("* main").as_deref(), Some("main"));
    assert_eq!(b("## qa...origin/qa").as_deref(), Some("qa"));
    assert_eq!(b("Tu rama está actualizada con 'origin/qa'."), None);
    assert_eq!(b("* two words"), None);
}

#[test]
fn test_file_viewer_read_only_blocks_every_mutation() {
    let path = std::env::temp_dir().join(format!("axon_test_ro_{}.txt", std::process::id()));
    std::fs::write(
        &path,
        "hola mundo
segunda",
    )
    .expect("write");
    let mut tab = TerminalTab::new_file_viewer(99, path.clone());
    let v = tab.file_viewer.as_mut().expect("viewer");
    assert!(v.is_read_only);

    v.select_all();
    assert!(
        !v.delete_selection(),
        "cortar/borrar no debe actuar en lectura"
    );
    v.insert_char('x');
    v.insert_text("pegado");
    v.insert_newline();
    v.backspace();
    v.delete();
    v.delete_word_left();
    v.delete_word_right();
    v.undo();
    v.redo();
    assert_eq!(
        v.lines,
        vec!["hola mundo".to_string(), "segunda".to_string()]
    );
    assert!(!v.is_dirty);
    // copiar sigue funcionando
    assert_eq!(
        v.selected_text().as_deref(),
        Some(
            "hola mundo
segunda"
        )
    );

    v.is_read_only = false;
    v.insert_char('x');
    assert!(v.is_dirty);
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_file_viewer_selection_stats_and_horizontal_scroll_limits() {
    let path = std::env::temp_dir().join(format!("axon_test_stats_{}.txt", std::process::id()));
    std::fs::write(&path, "    indentado\n\ttab\nlínea larguísima de prueba").expect("write");
    let mut tab = TerminalTab::new_file_viewer(99, path.clone());
    let v = tab.file_viewer.as_mut().expect("viewer");

    assert_eq!(v.selection_stats(), None);
    v.selection = Some(((0, 0), (1, 3)));
    let s = v.selection_stats().expect("stats");
    assert_eq!((s.chars, s.lines), (17, 2));

    v.visible_cols = 10;
    let longest = "línea larguísima de prueba".chars().count();
    assert_eq!(v.max_cols(), longest);
    assert_eq!(v.max_scroll_x(), longest + 2 - 10);
    v.visible_cols = 500;
    assert_eq!(v.max_scroll_x(), 0, "si todo cabe no hay scroll horizontal");
    let _ = std::fs::remove_file(path);
}

fn viewer_with(name: &str, content: &str) -> (TerminalTab, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("axon_test_{}_{}", std::process::id(), name));
    std::fs::write(&path, content).expect("write");
    (TerminalTab::new_file_viewer(99, path.clone()), path)
}

#[test]
fn test_file_viewer_folding_hides_lines_and_reveals_cursor() {
    let src = "fn a() {\n    x;\n    y;\n}\nfn b() {\n    z;\n}\n";
    let (mut tab, path) = viewer_with("fold.rs", src);
    let v = tab.file_viewer.as_mut().expect("viewer");

    assert_eq!(
        v.fold_end(0),
        Some(2),
        "la región termina antes de la llave de cierre"
    );
    assert_eq!(v.fold_end(1), None);
    let total = v.rows().len();

    v.toggle_fold(0);
    let rows = v.rows();
    assert_eq!(rows.len(), total - 2);
    assert!(rows.iter().all(|r| r.line != 1 && r.line != 2));
    assert_eq!(v.row_of(1, 0), None, "una línea plegada no tiene fila");

    // el cursor nunca queda oculto: al moverlo dentro, se despliega
    v.cursor_line = 2;
    v.ensure_cursor_visible(10);
    assert!(v.folded.is_empty());
    assert_eq!(v.rows().len(), total);

    v.fold_all();
    assert_eq!(v.folded.len(), 2);
    v.unfold_all();
    assert!(v.folded.is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_file_viewer_markdown_headings_fold_their_section() {
    let (mut tab, path) = viewer_with("fold.md", "# A\ntexto\n## B\nmás\n# C\nfin\n");
    let v = tab.file_viewer.as_mut().expect("viewer");
    assert_eq!(
        v.fold_end(0),
        Some(3),
        "un título pliega hasta el siguiente de su nivel o superior"
    );
    assert_eq!(v.fold_end(2), Some(3));
    assert_eq!(v.fold_end(4), Some(5));
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_file_viewer_word_wrap_splits_long_lines_into_rows() {
    let (mut tab, path) = viewer_with(
        "wrap.txt",
        "uno dos tres cuatro cinco seis siete ocho\ncorta",
    );
    let v = tab.file_viewer.as_mut().expect("viewer");
    v.visible_cols = 16;
    assert_eq!(v.rows().len(), 2, "sin ajuste cada línea es una fila");

    v.word_wrap = true;
    let rows = v.rows();
    assert!(rows.len() > 3);
    // cada tramo cabe en el ancho y juntos reconstruyen la línea original
    let joined: String = rows
        .iter()
        .filter(|r| r.line == 0)
        .map(|r| v.row_text(r).to_string())
        .collect();
    assert_eq!(joined, v.lines[0]);
    assert!(rows
        .iter()
        .filter(|r| r.line == 0)
        .all(|r| v.row_text(r).chars().count() <= 16));
    // el cursor al final de la línea larga cae en su última fila
    let last = rows.iter().rposition(|r| r.line == 0).expect("fila");
    assert_eq!(v.row_of(0, v.lines[0].chars().count()), Some(last));
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_explorer_shows_in_band_remote_listing_without_ssh() {
    use crate::explorer::{FileExplorer, RemoteSshSession};
    let mut ex = FileExplorer::new(None, false);
    let sess = RemoteSshSession {
        destination: "user@host".into(),
        remote_dir: "/home/django".into(),
    };
    let names: Vec<String> = ["z.txt", "app/", ".git/", "Bin/", "a.py"]
        .into_iter()
        .map(String::from)
        .collect();
    ex.set_remote_listing(sess.clone(), &names);

    assert!(ex.inband_remote && ex.error_message.is_none());
    let shown: Vec<_> = ex
        .items
        .iter()
        .map(|i| (i.name.as_str(), i.is_dir))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("app", true),
            ("Bin", true),
            ("a.py", false),
            ("z.txt", false)
        ],
        "carpetas primero, sin .git, orden sin distinguir mayúsculas"
    );
    assert_eq!(ex.items[0].path.to_string_lossy(), "/home/django/app");
    assert_eq!(ex.remote_parent().as_deref(), Some("/home"));
    assert!(!ex.can_go_back() && !ex.can_go_forward());

    // Reaplicar el mismo listado no reconstruye nada; otro distinto sí
    ex.selected_index = Some(1);
    ex.set_remote_listing(sess.clone(), &names);
    assert_eq!(ex.selected_index, Some(1));
    ex.set_remote_listing(sess, &["solo.txt".to_string()]);
    assert_eq!(ex.items.len(), 1);
}

#[test]
fn test_explorer_collapse_and_expand_all() {
    use crate::explorer::FileExplorer;
    let root = std::env::temp_dir().join(format!("axon_test_tree_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("a/b/c")).expect("mkdir");
    std::fs::create_dir_all(root.join("target/x")).expect("mkdir");
    std::fs::write(root.join("a/b/c/f.txt"), "x").expect("write");

    let mut ex = FileExplorer::new(Some(root.clone()), true);
    ex.expand_all();
    let names: Vec<_> = ex
        .items
        .iter()
        .map(|i| i.name.trim_end_matches('/').to_string())
        .collect();
    assert!(
        names.contains(&"f.txt".to_string()),
        "debe abrir niveles anidados: {names:?}"
    );
    assert!(!names.contains(&"x".to_string()), "no abre target");

    ex.collapse_all();
    assert!(ex.items.iter().all(|i| i.depth == 0 && !i.is_expanded));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_explorer_remote_tree_expands_in_band_and_survives_root_refresh() {
    use crate::explorer::{FileExplorer, RemoteSshSession};
    use std::path::Path;
    let mut ex = FileExplorer::new(None, false);
    let sess = RemoteSshSession {
        destination: "user@host".into(),
        remote_dir: "/srv".into(),
    };
    let root: Vec<String> = ["app/", "docs/", "run.sh"]
        .into_iter()
        .map(String::from)
        .collect();
    ex.set_remote_listing(sess.clone(), &root);

    // Expandir app/ con el listado recibido por el terminal
    let kids: Vec<String> = ["models.py", "static/", ".git/"]
        .into_iter()
        .map(String::from)
        .collect();
    ex.expand_remote_dir(Path::new("/srv/app"), &kids);
    let names: Vec<_> = ex
        .items
        .iter()
        .map(|i| (i.name.as_str(), i.depth))
        .collect();
    assert_eq!(
        names,
        vec![
            ("app", 0),
            ("static", 1),
            ("models.py", 1),
            ("docs", 0),
            ("run.sh", 0)
        ]
    );
    assert!(ex.items[0].is_expanded);

    // Reaplicar el mismo listado raíz no destruye lo expandido
    ex.set_remote_listing(sess, &root);
    assert_eq!(ex.items.len(), 5);

    // «Expandir todo» pide la siguiente carpeta sin abrir, en orden de aparición
    let next = ex.next_remote_dir_to_expand().expect("pendiente");
    assert_eq!(ex.items[next].name, "static");
    ex.set_expanded(Path::new("/srv/app/static"), true);
    let next = ex.next_remote_dir_to_expand().expect("pendiente");
    assert_eq!(ex.items[next].name, "docs");

    // Contraer todo deja solo el primer nivel
    ex.collapse_all();
    assert!(ex.items.iter().all(|i| i.depth == 0 && !i.is_expanded));
}

#[test]
fn test_single_click_opens_replaceable_preview_and_double_click_pins() {
    let config = crate::config::AxonConfig::default();
    let mut app = AxonApp::new(config, None);
    let dir = std::env::temp_dir().join(format!("axon_test_preview_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let files: Vec<_> = ["a.txt", "b.txt", "c.txt"]
        .iter()
        .map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, "x").expect("write");
            p
        })
        .collect();
    let viewers = |app: &AxonApp| {
        app.tabs
            .iter()
            .filter(|t| t.file_viewer.is_some())
            .map(|t| (t.title.clone(), t.is_preview))
            .collect::<Vec<_>>()
    };

    // Un clic: vista previa; otro clic en otro archivo la reemplaza en su sitio
    app.open_file_in_viewer(&files[0], false);
    assert_eq!(viewers(&app), vec![("a.txt".to_string(), true)]);
    let tabs_before = app.tabs.len();
    app.open_file_in_viewer(&files[1], false);
    assert_eq!(viewers(&app), vec![("b.txt".to_string(), true)]);
    assert_eq!(app.tabs.len(), tabs_before, "reemplaza, no añade");

    // Doble clic (mismo archivo con pin): queda fija; la siguiente vista previa ya no la pisa
    app.open_file_in_viewer(&files[1], true);
    assert_eq!(viewers(&app), vec![("b.txt".to_string(), false)]);
    app.open_file_in_viewer(&files[2], false);
    assert_eq!(
        viewers(&app),
        vec![("b.txt".to_string(), false), ("c.txt".to_string(), true)]
    );

    // Una vista previa con cambios sin guardar no se reemplaza
    let pos = app
        .tabs
        .iter()
        .position(|t| t.title == "c.txt")
        .expect("c.txt");
    app.tabs[pos].file_viewer.as_mut().unwrap().is_dirty = true;
    app.open_file_in_viewer(&files[0], false);
    assert_eq!(
        app.tabs.iter().filter(|t| t.file_viewer.is_some()).count(),
        3
    );

    // Volver a abrir un archivo ya fijo no lo convierte en vista previa
    app.open_file_in_viewer(&files[1], false);
    assert!(
        !app.tabs
            .iter()
            .find(|t| t.title == "b.txt")
            .unwrap()
            .is_preview
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_double_click_on_a_preview_tab_pins_it() {
    let mut app = AxonApp::new(crate::config::AxonConfig::default(), None);
    let path = std::env::temp_dir().join(format!("axon_test_tabclick_{}.txt", std::process::id()));
    std::fs::write(&path, "x").expect("write");
    app.open_file_in_viewer(&path, false);
    let idx = app.tabs.len() - 1;
    assert!(app.tabs[idx].is_preview);

    // Dos clics rápidos en la misma pestaña la fijan; un clic aislado no
    app.last_tab_click_idx = None;
    app.click_tab(idx);
    assert!(app.tabs[idx].is_preview);
    app.click_tab(idx);
    assert!(!app.tabs[idx].is_preview);

    // Dos clics en pestañas distintas no cuentan como doble clic
    app.tabs[idx].is_preview = true;
    app.click_tab(0);
    app.click_tab(idx);
    assert!(app.tabs[idx].is_preview);
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_diff_marks_separate_hunks_and_deletion() {
    use crate::app::viewer::{diff_marks, GitLineStatus::*};
    let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    // dos cambios separados: lo que hay entre ambos no se marca
    let marks = diff_marks(&v("a b c d e"), &v("a B c d E"));
    assert_eq!(
        marks,
        vec![None, Some(Modified), None, None, Some(Modified)]
    );
    // línea nueva en medio y borrado marcado en la línea siguiente
    assert_eq!(
        diff_marks(&v("a b"), &v("a x b")),
        vec![None, Some(Added), None]
    );
    assert_eq!(
        diff_marks(&v("a b c"), &v("a c")),
        vec![None, Some(Deleted)]
    );
    // sin base: todo añadido
    assert_eq!(diff_marks(&[], &v("a b")), vec![Some(Added), Some(Added)]);
}

#[test]
fn test_explorer_changes_view_lists_only_git_changes() {
    let dir = std::env::temp_dir().join(format!("axon_test_changes_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    std::fs::write(
        dir.join("keep.txt"),
        "uno
dos
",
    )
    .unwrap();
    std::fs::write(
        dir.join("sub").join("mod.txt"),
        "a
",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    std::fs::write(
        dir.join("sub").join("mod.txt"),
        "a
b
",
    )
    .unwrap();
    std::fs::write(
        dir.join("nuevo.txt"),
        "x
",
    )
    .unwrap();

    let mut explorer = crate::explorer::FileExplorer::new(Some(dir.clone()), true);
    explorer.toggle_changes_only();
    let names: Vec<_> = explorer.items.iter().map(|i| i.name.clone()).collect();
    assert_eq!(names, vec!["nuevo.txt", "mod.txt  sub"]);

    // el visor compara contra HEAD aunque el archivo ya esté guardado en disco
    let tab = TerminalTab::new_file_viewer(103, dir.join("sub").join("mod.txt"));
    let viewer = tab.file_viewer.as_ref().unwrap();
    assert_eq!(viewer.get_line_git_diff_status(0), None);
    assert_eq!(
        viewer.get_line_git_diff_status(1),
        Some(crate::app::viewer::GitLineStatus::Added)
    );

    explorer.toggle_changes_only();
    assert!(explorer.items.iter().any(|i| i.name == "keep.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_unified_diff_and_toggle() {
    let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    let diff =
        crate::app::viewer::unified_diff(&v("a b c d e f g h i j"), &v("a B c d e f g h i J"), 1);
    assert_eq!(
        diff,
        [
            "@@ -1,3 +1,3 @@",
            " a",
            "-b",
            "+B",
            " c",
            "@@ -9,2 +9,2 @@",
            " i",
            "-j",
            "+J"
        ]
    );
    assert!(crate::app::viewer::unified_diff(&v("a"), &v("a"), 3).is_empty());

    // Alternar: el diff sustituye al texto y volver restaura archivo, cursor y scroll
    let path = std::env::temp_dir().join(format!("axon_test_difftab_{}.txt", std::process::id()));
    std::fs::write(
        &path, "a
c
",
    )
    .unwrap();
    let mut tab = TerminalTab::new_file_viewer(104, path.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();
    viewer.git_base = Some(v("a b"));
    viewer.is_read_only = false;
    viewer.cursor_line = 1;
    viewer.scroll_offset = 1;
    viewer.toggle_diff();
    assert!(viewer.is_diff() && !viewer.is_editable() && !viewer.is_dirty);
    assert_eq!(viewer.lines, ["@@ -1,2 +1,2 @@", " a", "-b", "+c"]);
    assert_eq!(viewer.get_line_git_diff_status(2), None);
    assert!(viewer.has_git_changes());
    viewer.save_to_disk().unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a
c
"
    );
    viewer.toggle_diff();
    assert!(!viewer.is_diff());
    assert_eq!(viewer.lines, ["a", "c"]);
    assert_eq!((viewer.cursor_line, viewer.scroll_offset), (1, 1));
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_file_viewer_save_preserves_format_and_never_corrupts() {
    let dir = std::env::temp_dir().join(format!("axon_test_save_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let open = |name: &str, bytes: &[u8]| {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        let mut tab = TerminalTab::new_file_viewer(110, p.clone());
        tab.file_viewer.as_mut().unwrap().is_read_only = false;
        (tab, p)
    };

    // LF sigue en LF y CRLF en CRLF; sin salto final no se añade uno
    for (name, before, after) in [
        ("lf.sh", "a\nb\n", "xa\nb\n"),
        ("crlf.txt", "a\r\nb\r\n", "xa\r\nb\r\n"),
        ("nofinal.txt", "a\nb", "xa\nb"),
    ] {
        let (mut tab, p) = open(name, before.as_bytes());
        let v = tab.file_viewer.as_mut().unwrap();
        v.insert_char('x');
        v.save_to_disk().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), after, "{name}");
        assert!(!p.with_file_name(format!("{name}.axon-tmp")).exists());
    }

    // Un archivo que no es UTF-8 (Latin-1) no se puede editar ni se reescribe
    let latin1 = b"caf\xe9\n";
    let (mut tab, p) = open("latin1.txt", latin1);
    let v = tab.file_viewer.as_mut().unwrap();
    v.is_read_only = true;
    v.toggle_read_only();
    assert!(v.non_utf8 && v.is_read_only && !v.is_editable());
    v.is_read_only = false;
    assert!(!v.is_editable());
    v.insert_char('x');
    v.save_to_disk().unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), latin1);

    // Si no se puede guardar, el original queda intacto y el error se expone
    let (mut tab, p) = open("keep.txt", b"original\n");
    let v = tab.file_viewer.as_mut().unwrap();
    std::fs::create_dir(dir.join("keep.txt.axon-tmp")).unwrap();
    v.insert_char('x');
    v.autosave_local();
    assert!(v.save_error.is_some() && v.is_dirty);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "original\n");
    std::fs::remove_dir(dir.join("keep.txt.axon-tmp")).unwrap();
    v.save_to_disk().unwrap();
    assert!(v.save_error.is_none());

    let _ = std::fs::remove_dir_all(dir);
}

/// Pestaña de terminal con `n` paneles en cadena vertical (lado a lado)
fn tab_with_panes(app: &mut AxonApp, n: usize) {
    for _ in 1..n {
        app.split_active_pane(SplitDirection::Vertical);
    }
}

#[test]
fn test_divider_drag_moves_split_and_respects_minimum_size() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    tab_with_panes(&mut app, 2);
    let tab = &mut app.tabs[0];

    let div = tab.compute_dividers(0, 0, 1000, 600).remove(0);
    assert_eq!(div.direction, SplitDirection::Vertical);
    // Agarrable con 3 px de margen, no más lejos
    let (x, y, _, _) = div.rect;
    assert!(div.contains(x - 3, y + 10) && div.contains(x + 4, y + 10));
    assert!(!div.contains(x + 6, y + 10) && !div.contains(x.saturating_sub(5), y + 10));

    // Arrastrar a 300 px: el primer panel mide ~300 y los paneles siguen sumando el total
    tab.set_split_ratio(&div.path, div.ratio_at(div.axis(300, 100)));
    let rects = tab.compute_pane_rects(0, 0, 1000, 600);
    assert!(
        (rects[0].width as i32 - 299).abs() <= 2,
        "{}",
        rects[0].width
    );
    assert_eq!(rects[0].width + rects[1].width + 2, 1000);

    // Ni el borde ni más allá aplastan un panel por debajo del mínimo
    for pos in [0, 5, 999, 5000] {
        let r = div.ratio_at(pos);
        tab.set_split_ratio(&div.path, r);
        let rects = tab.compute_pane_rects(0, 0, 1000, 600);
        assert!(rects.iter().all(|r| r.width >= 47), "pos {pos}: {rects:?}");
    }
}

#[test]
fn test_nested_divider_path_targets_the_right_split() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    tab_with_panes(&mut app, 2);
    app.split_active_pane(SplitDirection::Horizontal); // divide el panel derecho
    let tab = &mut app.tabs[0];
    let dividers = tab.compute_dividers(0, 0, 1000, 600);
    assert_eq!(dividers.len(), 2);
    let inner = dividers
        .iter()
        .find(|d| d.direction == SplitDirection::Horizontal)
        .unwrap();
    assert_eq!(inner.path, vec![true]);

    let before = tab.compute_pane_rects(0, 0, 1000, 600);
    tab.set_split_ratio(&inner.path, 0.2);
    let after = tab.compute_pane_rects(0, 0, 1000, 600);
    // El panel izquierdo no se mueve; solo cambia el reparto vertical del derecho
    assert_eq!(before[0].width, after[0].width);
    assert!(after[1].height < before[1].height);
}

#[test]
fn test_moved_index_follows_the_dragged_tab() {
    // Mover 1 -> 3 en [a b c d e]: b pasa al final de ese tramo
    let order: Vec<usize> = (0..5).map(|i| super::tabs::moved_index(i, 1, 3)).collect();
    assert_eq!(order, vec![0, 3, 1, 2, 4]);
    let order: Vec<usize> = (0..5).map(|i| super::tabs::moved_index(i, 3, 1)).collect();
    assert_eq!(order, vec![0, 2, 3, 1, 4]);
}

#[test]
fn test_move_tab_reorders_and_keeps_active_tab() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    app.create_new_tab();
    app.create_new_tab();
    let ids: Vec<usize> = app.tabs.iter().map(|t| t.id).collect();
    assert_eq!(app.active_tab_index, 2);

    app.move_tab(2, 0);
    let moved: Vec<usize> = app.tabs.iter().map(|t| t.id).collect();
    assert_eq!(moved, vec![ids[2], ids[0], ids[1]]);
    assert_eq!(app.active_tab_index, 0, "la activa viaja con su pestaña");

    app.move_tab(1, 2); // otra pestaña: la activa no cambia de pestaña
    assert_eq!(app.tabs[app.active_tab_index].id, ids[2]);
    app.move_tab(0, 9); // fuera de rango: no hace nada
    assert_eq!(app.tabs[0].id, ids[2]);
}

#[test]
fn test_session_restores_tabs_and_split_layout() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    tab_with_panes(&mut app, 2);
    app.split_active_pane(SplitDirection::Horizontal);
    app.tabs[0].set_split_ratio(&[], 0.3);
    app.create_new_tab();
    app.open_or_switch_settings_tab(); // Ajustes no se guarda
    app.last_terminal_tab_index = 0;

    let snap = app.session_snapshot();
    assert_eq!(snap.tabs.len(), 2, "solo pestañas de terminal");
    assert_eq!(snap.active_tab_index, 0);

    let restored = AxonApp::from_session(AxonConfig::default(), None, Some(snap.clone()));
    assert_eq!(restored.tabs.len(), 2);
    assert_eq!(restored.tabs[0].panes.len(), 3);
    assert_eq!(restored.tabs[1].panes.len(), 1);
    // Misma forma, proporciones y panel activo (los ids de panel son nuevos)
    assert_eq!(restored.session_snapshot(), snap);
    assert!(restored.tabs[0].panes.iter().all(|p| !app.owns_pane(p.id)));
}

#[test]
fn test_restore_ignores_missing_folders_and_empty_sessions() {
    use crate::config::{PaneSession, TabSession, WindowSession};
    let gone = PaneSession::Leaf {
        cwd: Some("Z:/no/existe/axon".to_string()),
    };
    let session = WindowSession {
        active_tab_index: 7, // fuera de rango
        sidebar_visible: true,
        file_explorer_visible: false,
        tabs: vec![TabSession {
            title: "T".into(),
            layout: gone,
            active_pane: 9,
        }],
    };
    let app = AxonApp::from_session(AxonConfig::default(), None, Some(session));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_index, 0);
    assert!(app.tabs[0].active_pane().is_some());

    let empty = WindowSession {
        active_tab_index: 0,
        sidebar_visible: true,
        file_explorer_visible: false,
        tabs: Vec::new(),
    };
    let app = AxonApp::from_session(AxonConfig::default(), None, Some(empty));
    assert_eq!(
        app.tabs.len(),
        1,
        "sin pestañas guardadas arranca una nueva"
    );
}

#[test]
fn test_pane_ids_are_unique_across_windows() {
    let a = AxonApp::new(AxonConfig::default(), None);
    let b = AxonApp::new(AxonConfig::default(), None);
    let (pa, pb) = (a.tabs[0].panes[0].id, b.tabs[0].panes[0].id);
    assert_ne!(pa, pb);
    assert!(a.owns_pane(pa) && !a.owns_pane(pb));
    assert!(b.owns_pane(pb) && !b.owns_pane(pa));
}

#[test]
fn test_closing_last_tab_closes_the_window_not_the_process() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    app.create_new_tab();
    app.close_tab(0);
    assert!(!app.closing, "queda una pestaña");
    app.close_tab(0);
    assert!(app.closing && app.tabs.is_empty());
}

#[test]
fn test_close_tab_preserves_active_tab_and_close_other_tabs_keeps_target() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    let term = &mut app.tabs[0].panes[0].terminal;
    term.remote_integration = true;
    term.remote_at_prompt = false;
    assert!(
        !app.remote_cd("/srv"),
        "no debe escribir si hay un programa corriendo"
    );
    app.tabs[0].panes[0].terminal.remote_at_prompt = true;
    assert!(app.remote_cd("/srv"));
    assert!(!app.tabs[0].panes[0].terminal.remote_at_prompt);

    app.tabs.push(TerminalTab::new_settings(10));
    app.tabs.push(TerminalTab::new_settings(11));
    app.switch_to_tab(1);
    app.close_tab(0);
    assert_eq!(app.tabs[app.active_tab_index].id, 10);

    app.tabs.push(TerminalTab::new_settings(12));
    app.close_other_tabs(2);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs[0].id, 12);
    assert_eq!(app.active_tab_index, 0);
}

#[test]
fn test_viewer_replace_selection_takes_single_undo() {
    let (mut tab, path) = viewer_with("undo_sel.txt", "hola mundo");
    let v = tab.file_viewer.as_mut().unwrap();
    v.is_read_only = false;
    v.select_all();
    v.insert_char('X');
    assert_eq!(v.lines[0], "X");
    v.undo();
    assert_eq!(v.lines[0], "hola mundo");
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_get_git_branch_worktree_detached_head() {
    let dir = std::env::temp_dir().join(format!("axon_test_wt_{}", std::process::id()));
    let real_git = dir.join("real_git");
    std::fs::create_dir_all(&real_git).unwrap();
    std::fs::write(dir.join(".git"), "gitdir: real_git\n").unwrap();
    std::fs::write(real_git.join("HEAD"), "abcdef1234567890\n").unwrap();
    assert_eq!(
        super::shell_context::get_git_branch(Some(&dir)).as_deref(),
        Some("abcdef1")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bench_viewer_wrap_edit_and_selection() {
    let mut text = String::new();
    for i in 0..1000 {
        text.push_str(&format!(
            "    let resultado_{i} = calcular_transformacion_linea({i}, \"cadena de prueba con longitud suficiente para envolver en 80 columnas\");\n"
        ));
    }
    let (mut tab, path) = viewer_with("bench_viewer.rs", &text);
    let v = tab.file_viewer.as_mut().unwrap();
    v.is_read_only = false;
    v.word_wrap = true;
    v.visible_cols = 80;
    v.cursor_line = 500;
    v.cursor_col = 20;

    let n = if cfg!(debug_assertions) { 10 } else { 200 };
    let start = std::time::Instant::now();
    for _ in 0..n {
        v.insert_char('x');
        let _ = v.rows().len();
        let _ = v.max_cols();
        let _ = v.has_git_changes();
        v.selection = Some(((100, 4), (v.cursor_line, v.cursor_col)));
        let _ = v.selection_stats();
        v.selection = None;
        v.backspace();
    }
    let elapsed = start.elapsed();
    let _ = std::fs::remove_file(path);
    println!(
        "bench_viewer_wrap_edit_and_selection: {n} ciclos (insert+wrap+git+sel+backspace en 1000 líneas) en {elapsed:.2?} ({:.2} µs/ciclo)",
        elapsed.as_secs_f64() * 1_000_000.0 / n as f64
    );
}

#[test]
fn test_viewer_save_preserves_undo_redo_and_autosave_blank_lines() {
    let (mut tab, path) = viewer_with("save_undo_test.txt", "linea 1\nlinea 2\n");
    let v = tab.file_viewer.as_mut().unwrap();
    v.is_read_only = false;
    v.cursor_line = 1;
    v.cursor_col = 7;
    v.insert_char('!');
    assert!(v.is_dirty);
    assert!(!v.undo_stack.is_empty());

    // Guardar en disco no debe vaciar undo_stack ni redo_stack
    assert!(v.save_to_disk().is_ok());
    assert!(!v.is_dirty);
    assert!(!v.undo_stack.is_empty());

    // Se puede deshacer después de guardar
    v.undo();
    assert_eq!(v.lines[1], "linea 2");
    assert!(!v.redo_stack.is_empty());

    // Se puede rehacer
    v.redo();
    assert_eq!(v.lines[1], "linea 2!");

    // Autosave no debe recortar líneas en blanco finales
    v.lines.push(String::new());
    v.lines.push(String::new());
    v.cursor_line = 3;
    v.cursor_col = 0;
    v.autosave_local();
    assert_eq!(v.lines.len(), 4);

    // save_to_disk explícito sí recorta líneas en blanco (dejando como máximo una) y ajusta el cursor
    v.cursor_line = 3;
    assert!(v.save_to_disk().is_ok());
    assert_eq!(v.lines.len(), 3);
    assert!(v.cursor_line < v.lines.len());

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_reset_font_size_default() {
    let config = AxonConfig::load_or_default();
    let mut app = AxonApp::new(config, None);
    app.config.appearance.font_size = 22.0;
    app.config.appearance.ui_font_size = 18.0;

    app.reset_font_size();
    assert_eq!(
        app.config.appearance.font_size,
        AxonConfig::default().appearance.font_size
    );
    assert_eq!(app.config.appearance.ui_font_size, 18.0);
}

#[test]
fn test_scroll_offset_for_thumb_y_helper() {
    let track = (100, 50, 10, 200);
    let thumb = (100, 50, 10, 40);
    let max_offset = 100;

    // Arriba del todo
    assert_eq!(
        crate::renderer::scroll_offset_for_thumb_y(track, thumb, 50.0, max_offset),
        Some(0)
    );
    // Abajo del todo (disponible = 200 - 40 = 160)
    assert_eq!(
        crate::renderer::scroll_offset_for_thumb_y(track, thumb, 210.0, max_offset),
        Some(100)
    );
    // Mitad (50 + 80 = 130)
    assert_eq!(
        crate::renderer::scroll_offset_for_thumb_y(track, thumb, 130.0, max_offset),
        Some(50)
    );
}

#[test]
fn test_close_other_tabs_flushes_pending_autosave() {
    let mut app = AxonApp::new(AxonConfig::default(), None);
    let temp_dir =
        std::env::temp_dir().join(format!("axon-test-close-others-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("test.txt");
    std::fs::write(&file_path, "initial").unwrap();

    let tab_id = app.next_tab_id;
    app.next_tab_id += 1;
    let mut tab = TerminalTab::new_file_viewer(tab_id, file_path.clone());
    let viewer = tab.file_viewer.as_mut().unwrap();
    viewer.is_read_only = false;
    viewer.insert_text(" modified");
    viewer.schedule_autosave();
    assert!(viewer.is_dirty);
    assert!(viewer.needs_autosave);
    app.tabs.push(tab);

    assert_eq!(app.tabs.len(), 2);
    // Keep tab 0 (the terminal), closing tab 1 (the viewer with pending edits)
    app.close_other_tabs(0);

    assert_eq!(app.tabs.len(), 1);
    let saved = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(saved, " modifiedinitial");

    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(&temp_dir);
}

#[test]
fn test_fs_changed_event_refreshes_explorer_and_git_status() {
    let temp_dir =
        std::env::temp_dir().join(format!("axon-test-fs-changed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    // Inicializar repo git con un commit inicial
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output();
    let file_path = temp_dir.join("file.txt");
    std::fs::write(&file_path, "v1").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "file.txt"])
        .current_dir(&temp_dir)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init", "--no-gpg-sign"])
        .current_dir(&temp_dir)
        .output();

    let mut app = AxonApp::new(AxonConfig::default(), None);
    app.file_explorer.visible = true;
    app.file_explorer.set_root(temp_dir.clone());
    app.file_explorer.refresh();

    // En tests cfg!(test), refresh_git_status ejecuta síncronamente
    assert!(app.file_explorer.git_statuses.is_empty());

    // Modificar archivo simulando cambio externo o comando
    std::fs::write(&file_path, "v2 modified").unwrap();

    // Disparar FsChanged
    app.on_user_event(AxonEvent::FsChanged);

    // Debe haber detectado el archivo modificado
    assert_eq!(
        app.file_explorer.git_statuses.get(&file_path),
        Some(&crate::explorer::GitFileStatus::Modified)
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_sync_fs_watcher_behavior() {
    let temp_dir =
        std::env::temp_dir().join(format!("axon-test-watcher-sync-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut app = AxonApp::new(AxonConfig::default(), None);
    app.file_explorer.visible = false;
    app.sync_fs_watcher();
    assert!(app.fs_watcher.is_none());

    // Con explorador visible pero sin proxy de ventana (modo headless/test), se maneja limpiamente sin pánico
    app.file_explorer.visible = true;
    app.file_explorer.set_root(temp_dir.clone());
    app.sync_fs_watcher();
    assert!(app.fs_watcher.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
