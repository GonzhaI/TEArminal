use super::*;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

#[test]
fn test_dir_git_status_uses_one_color_per_kind_and_yellow_for_mixes() {
    use GitFileStatus::*;
    let map = |entries: &[(&str, GitFileStatus)]| -> FxHashMap<PathBuf, GitStatusInfo> {
        entries
            .iter()
            .map(|(p, s)| {
                (
                    PathBuf::from(p),
                    GitStatusInfo {
                        status: *s,
                        is_staged: false,
                        is_unstaged: true,
                    },
                )
            })
            .collect()
    };
    let dir = Path::new("repo/src");
    assert_eq!(dir_git_status(&map(&[]), dir), None);
    assert_eq!(
        dir_git_status(&map(&[("repo/docs/a.md", Modified)]), dir),
        None
    );
    assert_eq!(
        dir_git_status(
            &map(&[("repo/src/a.rs", Untracked), ("repo/src/b.rs", Added)]),
            dir
        ),
        Some(Added)
    );
    assert_eq!(
        dir_git_status(&map(&[("repo/src/a.rs", Deleted)]), dir),
        Some(Deleted)
    );
    assert_eq!(
        dir_git_status(&map(&[("repo/src/a.rs", Renamed)]), dir),
        Some(Modified)
    );
    assert_eq!(
        dir_git_status(
            &map(&[("repo/src/a.rs", Deleted), ("repo/src/b.rs", Added)]),
            dir
        ),
        Some(Modified),
        "una mezcla es amarillo"
    );
}

/// `-z` entrega los nombres sin comillas ni escapes y el origen de un renombrado va en otra entrada
#[test]
fn test_parse_porcelain_z_handles_unicode_spaces_and_renames() {
    let root = Path::new("C:\\repo");
    let out =
        "?? niño.txt\0 M dir/mi archivo.rs\0R  nuevo.rs\0viejo.rs\0 D borrado.rs\0".as_bytes();
    let map = parse_porcelain_z(root, out);
    assert_eq!(map[&root.join("niño.txt")].status, GitFileStatus::Untracked);
    assert!(!map[&root.join("niño.txt")].is_staged);
    assert!(map[&root.join("niño.txt")].is_unstaged);
    assert_eq!(
        map[&root.join("dir\\mi archivo.rs")].status,
        GitFileStatus::Modified
    );
    assert!(!map[&root.join("dir\\mi archivo.rs")].is_staged);
    assert!(map[&root.join("dir\\mi archivo.rs")].is_unstaged);
    assert_eq!(map[&root.join("nuevo.rs")].status, GitFileStatus::Renamed);
    assert!(map[&root.join("nuevo.rs")].is_staged);
    assert!(!map[&root.join("nuevo.rs")].is_unstaged);
    assert!(!map.contains_key(&root.join("viejo.rs")));
    assert_eq!(map[&root.join("borrado.rs")].status, GitFileStatus::Deleted);
    assert!(!map[&root.join("borrado.rs")].is_staged);
    assert!(map[&root.join("borrado.rs")].is_unstaged);
}

/// Una sola consulta de `git status` en vuelo: la ráfaga siguiente queda como una petición pendiente
#[test]
fn test_git_status_requests_do_not_overlap() {
    let mut ex = FileExplorer::new(Some(std::env::temp_dir()), false);
    ex.git_status_req = Some((PathBuf::from("a"), false));
    assert!(ex.take_git_status_request().is_some());
    ex.git_status_req = Some((PathBuf::from("a"), false));
    assert!(ex.take_git_status_request().is_none());
    ex.apply_git_statuses(Path::new("a"), FxHashMap::default());
    assert!(ex.take_git_status_request().is_some());
}

#[test]
fn test_file_explorer_listing_and_navigation() {
    let current = std::env::current_dir().unwrap();
    let mut explorer = FileExplorer::new(Some(current.clone()), true);
    assert!(explorer.visible);
    assert!(!explorer.items.is_empty());

    // Debe listar directorios y archivos ordenados
    let has_src = explorer
        .items
        .iter()
        .any(|it| it.name == "src" && it.is_dir);
    let has_cargo = explorer
        .items
        .iter()
        .any(|it| it.name == "Cargo.toml" && !it.is_dir);
    assert!(has_src);
    assert!(has_cargo);

    // Subir de nivel
    let initial_root = explorer.root_dir.clone();
    explorer.navigate_up();
    assert_ne!(explorer.root_dir, initial_root);
    assert_eq!(explorer.root_dir, initial_root.parent().unwrap());

    // Regresar a initial
    explorer.set_root(initial_root);
    assert_eq!(explorer.root_dir, current);

    // Expandir carpeta src
    if let Some(pos) = explorer
        .items
        .iter()
        .position(|it| it.name == "src" && it.is_dir)
    {
        assert!(!explorer.items[pos].is_expanded);
        explorer.toggle_expand(pos);
        assert!(explorer.items[pos].is_expanded);
        // Debe haber insertado elementos hijos dentro de src
        let has_main = explorer
            .items
            .iter()
            .any(|it| it.name == "main.rs" && it.depth > 0);
        assert!(has_main);

        // Colapsar
        explorer.toggle_expand(pos);
        assert!(!explorer.items[pos].is_expanded);
        let has_main_after = explorer
            .items
            .iter()
            .any(|it| it.name == "main.rs" && it.depth > 0);
        assert!(!has_main_after);
    }

    // item_at_y
    let first_item_y =
        FileExplorer::HEADER_HEIGHT + FileExplorer::SUBHEADER_HEIGHT + FileExplorer::LIST_TOP_PAD;
    assert_eq!(explorer.item_at_y(first_item_y, 0), Some(0));

    // toggle visibility
    explorer.toggle();
    assert!(!explorer.visible);
    explorer.toggle();
    assert!(explorer.visible);
}

#[test]
fn test_file_explorer_keyboard_navigation() {
    let current = std::env::current_dir().unwrap();
    let mut explorer = FileExplorer::new(Some(current), true);
    assert!(!explorer.items.is_empty());

    // Selección inicial
    explorer.selected_index = Some(0);
    explorer.navigate_selection(1, 10);
    assert_eq!(explorer.selected_index, Some(1));

    explorer.navigate_selection(-1, 10);
    assert_eq!(explorer.selected_index, Some(0));

    // ensure_visible
    explorer.ensure_visible(15, 10);
    assert_eq!(explorer.scroll_offset, 6);
    explorer.ensure_visible(2, 10);
    assert_eq!(explorer.scroll_offset, 2);

    // Expansión con flecha derecha y retorno con flecha izquierda
    if let Some(src_pos) = explorer
        .items
        .iter()
        .position(|it| it.name == "src" && it.is_dir)
    {
        explorer.selected_index = Some(src_pos);
        assert!(!explorer.items[src_pos].is_expanded);

        // Flecha derecha: expandir
        let res = explorer.handle_arrow_right(10);
        assert!(res.is_none());
        assert!(explorer.items[src_pos].is_expanded);

        // Flecha derecha de nuevo: entra al hijo
        let _ = explorer.handle_arrow_right(10);
        assert_eq!(explorer.selected_index, Some(src_pos + 1));

        // Flecha izquierda desde el hijo: selecciona el directorio padre (src)
        explorer.handle_arrow_left(10);
        assert_eq!(explorer.selected_index, Some(src_pos));

        // Flecha izquierda sobre src expandido: colapsa
        explorer.handle_arrow_left(10);
        assert!(!explorer.items[src_pos].is_expanded);
    }

    // Enter sobre un archivo devuelve la ruta para abrir en visor
    if let Some(cargo_pos) = explorer
        .items
        .iter()
        .position(|it| it.name == "Cargo.toml" && !it.is_dir)
    {
        explorer.selected_index = Some(cargo_pos);
        let path = explorer.handle_enter(10);
        assert!(path.is_some());
        assert!(path.unwrap().ends_with("Cargo.toml"));
    }
}

#[test]
fn test_file_explorer_pin_toggle() {
    let mut explorer = FileExplorer::new(None, false);
    assert!(!explorer.is_pinned);
    explorer.toggle_pinned();
    assert!(explorer.is_pinned);
    explorer.toggle_pinned();
    assert!(!explorer.is_pinned);

    let (px, py, pw, ph) = FileExplorer::pin_btn_rect(100, 50, 260);
    assert_eq!(pw, 24);
    assert_eq!(ph, 24);
    assert_eq!(py, 57);
    assert_eq!(px, 100 + 260 - 66);
}

#[test]
fn test_fuzzy_score() {
    assert_eq!(fuzzy_score("", "anything"), Some(0));
    assert_eq!(fuzzy_score("main", "main"), Some(1000));
    assert!(fuzzy_score("main", "main.rs").unwrap() > fuzzy_score("ain", "main.rs").unwrap());
    assert!(fuzzy_score("geo", "geometry.rs").is_some());
    assert!(fuzzy_score("crg", "Cargo.toml").is_some());
    assert_eq!(fuzzy_score("xyz123", "main.rs"), None);
}

#[test]
fn test_file_explorer_filter_lifecycle() {
    let tmp = std::env::temp_dir().join(format!("tearminal_filter_test_{}", std::process::id()));
    let sub = tmp.join("sub");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(tmp.join("lib.rs"), "pub fn lib() {}").unwrap();
    std::fs::write(sub.join("inner.rs"), "pub fn inner() {}").unwrap();

    let mut explorer = FileExplorer::new(Some(tmp.clone()), false);
    explorer.refresh();
    assert!(!explorer.is_filtering());
    assert_eq!(explorer.subheader_height(), 0);

    // Abrir filtro
    explorer.open_filter();
    assert!(explorer.is_filtering());
    assert_eq!(explorer.subheader_height(), FileExplorer::FILTER_BAR_HEIGHT);

    // Filtrar por 'inner'
    explorer.filter.as_mut().unwrap().set_text("inner");
    explorer.update_filter();

    // Debe mantener visible la carpeta 'sub' y el archivo 'inner.rs'
    assert!(explorer.visible_item_count() >= 2);
    let matching_names: Vec<String> = (0..explorer.visible_item_count())
        .filter_map(|i| explorer.visible_item(i).map(|it| it.name.clone()))
        .collect();
    assert!(matching_names.contains(&"sub".to_string()));
    assert!(matching_names.contains(&"inner.rs".to_string()));

    // El archivo seleccionado debe ser 'inner.rs'
    let sel_idx = explorer
        .selected_index
        .expect("debe haber elemento seleccionado");
    let sel_item = explorer
        .visible_item(sel_idx)
        .expect("item seleccionado debe existir");
    assert_eq!(sel_item.name, "inner.rs");

    // Enter devuelve la ruta del archivo
    let opened = explorer.handle_enter(10);
    assert!(opened.is_some());
    assert!(opened.unwrap().ends_with("inner.rs"));

    // Cerrar filtro restaura el estado
    explorer.close_filter();
    assert!(!explorer.is_filtering());
    assert_eq!(explorer.subheader_height(), 0);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_action_buttons_geometry_and_hit_test() {
    use crate::renderer::{GitActionBtnRects, GitActionKind};
    let panel_x = 100;
    let panel_w = 200;
    let item_y = 50;
    let item_h = 24;

    // 1. Solo staged: unstage
    let btns = GitActionBtnRects::buttons_for_item(panel_x, panel_w, item_y, item_h, true, false);
    assert_eq!(btns.len(), 1);
    assert_eq!(btns[0].0, GitActionKind::Unstage);
    let (_action, rect) = btns[0];
    assert_eq!(
        GitActionBtnRects::hit_test(
            panel_x,
            panel_w,
            item_y,
            item_h,
            true,
            false,
            rect.0 + 2,
            rect.1 + 2
        ),
        Some(GitActionKind::Unstage)
    );

    // 2. Solo unstaged: stage y discard
    let btns = GitActionBtnRects::buttons_for_item(panel_x, panel_w, item_y, item_h, false, true);
    assert_eq!(btns.len(), 2);
    assert_eq!(btns[0].0, GitActionKind::Stage);
    assert_eq!(btns[1].0, GitActionKind::Discard);
    assert_eq!(
        GitActionBtnRects::hit_test(
            panel_x,
            panel_w,
            item_y,
            item_h,
            false,
            true,
            btns[0].1 .0 + 2,
            btns[0].1 .1 + 2
        ),
        Some(GitActionKind::Stage)
    );
    assert_eq!(
        GitActionBtnRects::hit_test(
            panel_x,
            panel_w,
            item_y,
            item_h,
            false,
            true,
            btns[1].1 .0 + 2,
            btns[1].1 .1 + 2
        ),
        Some(GitActionKind::Discard)
    );

    // 3. Staged y unstaged a la vez: unstage, stage, discard
    let btns = GitActionBtnRects::buttons_for_item(panel_x, panel_w, item_y, item_h, true, true);
    assert_eq!(btns.len(), 3);
    assert_eq!(btns[0].0, GitActionKind::Unstage);
    assert_eq!(btns[1].0, GitActionKind::Stage);
    assert_eq!(btns[2].0, GitActionKind::Discard);

    // 4. Fuera de los botones: None
    assert_eq!(
        GitActionBtnRects::hit_test(
            panel_x,
            panel_w,
            item_y,
            item_h,
            true,
            true,
            panel_x + 10,
            item_y + 2
        ),
        None
    );
}

#[test]
fn test_git_stage_unstage_and_discard_actions() {
    let tmp = std::env::temp_dir().join(format!(
        "tearminal-test-git-actions-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&tmp);

    // Inicializar repositorio git
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&tmp);
    crate::app::platform::hide_console(&mut cmd);
    let _ = cmd.args(["init"]).status();
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&tmp);
    crate::app::platform::hide_console(&mut cmd);
    let _ = cmd.args(["config", "core.autocrlf", "false"]).status();

    let file_a = tmp.join("file_a.txt");
    std::fs::write(&file_a, "initial line 1\n").unwrap();

    // 1. Stage de file_a
    let res = git_stage_file(&tmp, &file_a);
    assert!(res.is_ok());
    let statuses = collect_git_statuses(&tmp, true);
    assert!(statuses
        .get(&file_a)
        .is_some_and(|s| s.is_staged && !s.is_unstaged));

    // 2. Unstage de file_a
    let res = git_unstage_file(&tmp, &file_a);
    assert!(res.is_ok());
    let statuses = collect_git_statuses(&tmp, true);
    assert!(statuses
        .get(&file_a)
        .is_some_and(|s| !s.is_staged && s.is_unstaged));

    // Commit inicial para poder probar restore de modificaciones
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&tmp);
    crate::app::platform::hide_console(&mut cmd);
    let _ = cmd.args(["config", "user.name", "TestUser"]).status();
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&tmp);
    crate::app::platform::hide_console(&mut cmd);
    let _ = cmd
        .args(["config", "user.email", "test@example.com"])
        .status();
    let _ = git_stage_file(&tmp, &file_a);
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&tmp);
    crate::app::platform::hide_console(&mut cmd);
    let _ = cmd.args(["commit", "-m", "init"]).status();

    // 3. Modificar file_a y descartar cambios
    std::fs::write(&file_a, "modified line\n").unwrap();
    let statuses = collect_git_statuses(&tmp, true);
    assert!(statuses.get(&file_a).is_some_and(|s| s.is_unstaged));
    let res = git_discard_file(&tmp, &file_a, false);
    assert!(res.is_ok());
    assert_eq!(
        std::fs::read_to_string(&file_a)
            .unwrap()
            .replace("\r\n", "\n"),
        "initial line 1\n"
    );

    // 4. Crear archivo untracked y descartarlo (debe ser eliminado)
    let file_b = tmp.join("file_b.txt");
    std::fs::write(&file_b, "untracked\n").unwrap();
    assert!(file_b.exists());
    let res = git_discard_file(&tmp, &file_b, true);
    assert!(res.is_ok());
    assert!(!file_b.exists());

    let _ = std::fs::remove_dir_all(&tmp);
}
