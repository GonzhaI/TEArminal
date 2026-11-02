//! Gestión de archivos del explorador local: nuevo archivo, nueva carpeta, renombrar y eliminar (a la
//! Papelera de reciclaje). La edición del nombre vive en `FileExplorer::edit`; la barra se dibuja en
//! `renderer/explorer_panel.rs` y el teclado/clic la gobiernan `app/keyboard.rs` y `app/click.rs`.

use super::FileExplorer;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    NewFile,
    NewDir,
    Rename,
}

/// Nombre en edición: `target` es la carpeta donde se crea (`NewFile`/`NewDir`) o el elemento que se renombra.
#[derive(Debug, Clone)]
pub(crate) struct ExplorerEdit {
    pub kind: EditKind,
    pub target: PathBuf,
    pub input: crate::ui::TextInput,
    pub error: Option<String>,
}

/// Rechaza lo que Windows no admite en un nombre (o que saldría de la carpeta). Solo valida el nombre: que ya
/// exista lo comprueba el que llama.
fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name == "." || name == ".." {
        return Err("Nombre no válido");
    }
    if name
        .chars()
        .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err("Caracteres no permitidos: < > : \" / \\ | ? *");
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("No puede terminar en punto ni espacio");
    }
    Ok(())
}

/// Envía `path` a la Papelera de reciclaje (`SHFileOperationW` con `FOF_ALLOWUNDO`), sin diálogos ni confirmación:
/// se puede deshacer desde la Papelera. Con una ruta de `MAX_PATH` o más Windows borraría de forma permanente, así
/// que se rechaza.
pub(crate) fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
        SHFILEOPSTRUCTW,
    };
    let mut from: Vec<u16> =
        std::os::windows::ffi::OsStrExt::encode_wide(path.as_os_str()).collect();
    if from.len() >= 260 {
        return Err("Ruta demasiado larga para la Papelera".into());
    }
    // `pFrom` es una lista de rutas terminada en dos nulos
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: 0,
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as _,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `op` es una estructura válida y `from` (doble nulo) vive durante la llamada
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 || op.fAnyOperationsAborted != 0 {
        return Err(format!("No se pudo enviar a la Papelera (código {code})"));
    }
    Ok(())
}

impl FileExplorer {
    /// Desplazamiento del texto respecto al borde izquierdo de la barra de edición (icono + aire)
    pub(crate) const EDIT_TEXT_DX: usize = 32;

    /// Solo el explorador local en su vista de árbol: el remoto no escribe en disco y «Cambios» es una lista plana.
    pub(crate) fn can_manage_files(&self) -> bool {
        self.remote_session.is_none() && !self.changes_only
    }

    fn selected_item_path(&self) -> Option<PathBuf> {
        let sel = self.selected_path.as_ref()?;
        self.items
            .iter()
            .any(|i| &i.path == sel)
            .then(|| sel.clone())
    }

    /// Abre la barra de edición. Nuevo archivo/carpeta se crea en la carpeta seleccionada (o en la del archivo
    /// seleccionado, o en la raíz); renombrar exige una selección. Devuelve `false` si no procede.
    pub(crate) fn begin_edit(&mut self, kind: EditKind) -> bool {
        if !self.can_manage_files() {
            return false;
        }
        let selected = self.selected_item_path();
        let (target, initial) = match kind {
            EditKind::Rename => {
                let Some(path) = selected else {
                    return false;
                };
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (path, name)
            }
            EditKind::NewFile | EditKind::NewDir => {
                let dir = match selected {
                    Some(p) if p.is_dir() => p,
                    Some(p) => p
                        .parent()
                        .map_or_else(|| self.root_dir.clone(), Path::to_path_buf),
                    None => self.root_dir.clone(),
                };
                (dir, String::new())
            }
        };
        let mut input = crate::ui::TextInput::new(initial);
        input.select_all();
        self.is_focused = true;
        self.edit = Some(ExplorerEdit {
            kind,
            target,
            input,
            error: None,
        });
        true
    }

    pub(crate) fn cancel_edit(&mut self) {
        self.edit = None;
    }

    /// Aplica la edición. Con éxito cierra la barra, refresca, selecciona el resultado y lo devuelve; con un
    /// nombre inválido o un fallo del sistema deja la barra abierta con el motivo en `error`.
    pub(crate) fn commit_edit(&mut self, max_visible: usize) -> Option<PathBuf> {
        let edit = self.edit.as_mut()?;
        let name = edit.input.text().trim().to_string();
        if let Err(msg) = validate_name(&name) {
            edit.error = Some(msg.to_string());
            return None;
        }
        let (dir, source) = match edit.kind {
            EditKind::Rename => (
                edit.target
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default(),
                Some(edit.target.clone()),
            ),
            _ => (edit.target.clone(), None),
        };
        let dest = dir.join(&name);
        if source.as_ref() == Some(&dest) {
            self.edit = None;
            return Some(dest);
        }
        // Un cambio solo de mayúsculas es el mismo archivo para Windows: se permite al renombrar
        let same_file_other_case = source.as_ref().is_some_and(|s| {
            s.to_string_lossy()
                .eq_ignore_ascii_case(&dest.to_string_lossy())
        });
        if dest.exists() && !same_file_other_case {
            edit.error = Some(format!("Ya existe «{name}»"));
            return None;
        }
        let done = match (&source, edit.kind) {
            (Some(from), _) => fs::rename(from, &dest),
            (None, EditKind::NewDir) => fs::create_dir(&dest),
            (None, _) => fs::File::create_new(&dest).map(|_| ()),
        };
        if let Err(e) = done {
            edit.error = Some(e.to_string());
            return None;
        }
        self.edit = None;
        if source.is_none() && dir != self.root_dir {
            self.set_expanded(&dir, true);
        }
        self.refresh();
        self.select_path(&dest, max_visible);
        Some(dest)
    }

    /// Selecciona `path` si está en la lista y la deja a la vista.
    fn select_path(&mut self, path: &Path, max_visible: usize) {
        if let Some(idx) = self.items.iter().position(|i| i.path == path) {
            self.selected_index = Some(idx);
            self.selected_path = Some(path.to_path_buf());
            self.ensure_visible(idx, max_visible);
        }
    }

    /// Envía a la Papelera el elemento seleccionado y selecciona su vecino. Devuelve la ruta eliminada.
    pub(crate) fn delete_selected(&mut self, max_visible: usize) -> Result<PathBuf, String> {
        if !self.can_manage_files() {
            return Err("No disponible en esta vista".into());
        }
        let path = self.selected_item_path().ok_or("Nada seleccionado")?;
        let idx = self.items.iter().position(|i| i.path == path).unwrap_or(0);
        move_to_recycle_bin(&path)?;
        self.refresh();
        match self.items.get(idx.min(self.items.len().saturating_sub(1))) {
            Some(next) => {
                let next = next.path.clone();
                self.select_path(&next, max_visible);
            }
            None => {
                self.selected_index = None;
                self.selected_path = None;
            }
        }
        Ok(path)
    }

    /// Rectángulo `(x, y, ancho, alto)` de la barra de edición, compartido por dibujo y clic: sobre las primeras
    /// filas de la lista del panel `(panel_x, panel_y, panel_w)`.
    pub(crate) fn edit_bar_rect(
        panel_x: usize,
        panel_y: usize,
        panel_w: usize,
    ) -> (usize, usize, usize, usize) {
        let y = panel_y + Self::HEADER_HEIGHT + Self::SUBHEADER_HEIGHT + Self::LIST_TOP_PAD + 2;
        (
            panel_x + 10,
            y,
            panel_w.saturating_sub(20 + crate::renderer::SidebarRects::PAD_R),
            28,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_explorer(tag: &str) -> (FileExplorer, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tearminal_edit_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        (FileExplorer::new(Some(dir.clone()), true), dir)
    }

    fn set_name(ex: &mut FileExplorer, name: &str) {
        ex.edit.as_mut().unwrap().input = crate::ui::TextInput::new(name);
    }

    #[test]
    fn test_validate_name_rejects_invalid() {
        for bad in [
            "", ".", "..", "a/b", "a\\b", "a:b", "a?", "fin.", "fin ", "a\tb",
        ] {
            assert!(validate_name(bad).is_err(), "{bad:?} debería rechazarse");
        }
        assert!(validate_name("main.rs").is_ok());
        assert!(validate_name(".gitignore").is_ok());
    }

    #[test]
    fn test_create_rename_and_collisions() {
        let (mut ex, dir) = temp_explorer("crud");

        assert!(ex.begin_edit(EditKind::NewFile));
        set_name(&mut ex, "a.txt");
        assert_eq!(ex.commit_edit(10), Some(dir.join("a.txt")));
        assert!(dir.join("a.txt").is_file());
        assert_eq!(ex.selected_path, Some(dir.join("a.txt")));
        assert!(ex.edit.is_none());

        // Nombre repetido: no se pisa y la barra sigue abierta con el motivo
        assert!(ex.begin_edit(EditKind::NewDir));
        set_name(&mut ex, "a.txt");
        assert_eq!(ex.commit_edit(10), None);
        assert!(ex.edit.as_ref().unwrap().error.is_some());
        set_name(&mut ex, "sub");
        assert_eq!(ex.commit_edit(10), Some(dir.join("sub")));
        assert!(dir.join("sub").is_dir());

        // Una carpeta seleccionada recibe el nuevo archivo dentro y se expande
        assert!(ex.begin_edit(EditKind::NewFile));
        assert_eq!(ex.edit.as_ref().unwrap().target, dir.join("sub"));
        set_name(&mut ex, "b.txt");
        assert_eq!(ex.commit_edit(10), Some(dir.join("sub").join("b.txt")));
        assert!(ex
            .items
            .iter()
            .any(|i| i.path == dir.join("sub").join("b.txt")));

        // Renombrar (también solo mayúsculas)
        ex.select_path(&dir.join("a.txt"), 10);
        assert!(ex.begin_edit(EditKind::Rename));
        assert_eq!(ex.edit.as_ref().unwrap().input.text(), "a.txt");
        set_name(&mut ex, "A.txt");
        assert!(ex.commit_edit(10).is_some());
        assert!(fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name() == "A.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rename_requires_selection_and_local_tree() {
        let (mut ex, dir) = temp_explorer("guards");
        ex.selected_path = None;
        assert!(!ex.begin_edit(EditKind::Rename));
        ex.changes_only = true;
        assert!(!ex.begin_edit(EditKind::NewFile));
        assert!(ex.delete_selected(10).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete_goes_to_recycle_bin() {
        let (mut ex, dir) = temp_explorer("del");
        let file = dir.join("borrame.txt");
        fs::write(&file, "x").unwrap();
        ex.refresh();
        ex.select_path(&file, 10);
        assert_eq!(ex.delete_selected(10), Ok(file.clone()));
        assert!(!file.exists());
        assert!(ex.items.iter().all(|i| i.path != file));
        let _ = fs::remove_dir_all(&dir);
    }
}
