//! Proyectos del panel de navegación: carpetas que el usuario agrega y a las que se asignan
//! las sesiones según su directorio de trabajo.

use crate::config::ProjectConfig;
use std::path::Path;

/// Índices ANSI de los colores de proyecto (azul, cian, magenta, amarillo, verde, rojo), en orden.
const COLOR_SLOTS: [usize; 6] = [4, 6, 5, 3, 2, 1];

/// Clave comparable de una ruta: Windows ignora mayúsculas y el tipo de separador, y `\\?\` no cuenta.
fn key(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn contains(root: &str, dir: &str) -> bool {
    dir == root || dir.strip_prefix(root).is_some_and(|r| r.starts_with('\\'))
}

/// Proyecto al que pertenece `cwd`: el de raíz más profunda que lo contiene.
pub fn project_for(projects: &[ProjectConfig], cwd: &Path) -> Option<usize> {
    let cwd = key(&cwd.to_string_lossy());
    projects
        .iter()
        .enumerate()
        .map(|(i, p)| (i, key(&p.path)))
        .filter(|(_, root)| contains(root, &cwd))
        .max_by_key(|(_, root)| root.len())
        .map(|(i, _)| i)
}

/// Agrega `path` como proyecto y devuelve su índice. Si ya estaba, no lo duplica.
pub fn add(projects: &mut Vec<ProjectConfig>, path: &Path) -> usize {
    let wanted = key(&path.to_string_lossy());
    if let Some(i) = projects.iter().position(|p| key(&p.path) == wanted) {
        return i;
    }
    let name = path
        .file_name()
        .map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy())
        .into_owned();
    projects.push(ProjectConfig {
        name,
        path: path.to_string_lossy().into_owned(),
    });
    projects.len() - 1
}

/// Rama del repositorio en `root` leyendo `.git/HEAD` (el commit abreviado si `HEAD` está suelto).
/// `None` si no es un repositorio o `.git` es un archivo (worktree).
pub fn read_branch(root: &Path) -> Option<String> {
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(name) => Some(name.to_string()),
        None => (head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| head[..7].to_string()),
    }
}

/// Inicial en mayúscula que representa al proyecto en el riel.
pub fn initial(name: &str) -> char {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map_or('?', |c| c.to_uppercase().next().unwrap_or(c))
}

/// Índice ANSI del color del proyecto: estable según su posición en la lista.
pub fn color_slot(index: usize) -> usize {
    COLOR_SLOTS[index % COLOR_SLOTS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project(path: &str) -> ProjectConfig {
        ProjectConfig {
            name: String::new(),
            path: path.to_string(),
        }
    }

    #[test]
    fn test_session_belongs_to_deepest_matching_project() {
        let projects = vec![project(r"C:\dev"), project(r"C:\dev\axon")];
        assert_eq!(
            project_for(&projects, Path::new(r"C:\dev\axon\src")),
            Some(1)
        );
        assert_eq!(project_for(&projects, Path::new(r"C:\dev\api")), Some(0));
    }

    #[test]
    fn test_project_match_ignores_case_separators_and_verbatim_prefix() {
        let projects = vec![project(r"C:\Dev\Axon\")];
        assert_eq!(
            project_for(&projects, Path::new(r"\\?\c:\dev\axon")),
            Some(0)
        );
        assert_eq!(
            project_for(&projects, Path::new("c:/dev/axon/src")),
            Some(0)
        );
    }

    #[test]
    fn test_sibling_folder_with_same_prefix_is_not_inside_project() {
        let projects = vec![project(r"C:\dev\axon")];
        assert_eq!(project_for(&projects, Path::new(r"C:\dev\axon2")), None);
        assert_eq!(project_for(&projects, Path::new(r"D:\dev\axon")), None);
    }

    #[test]
    fn test_adding_a_project_twice_keeps_one_entry() {
        let mut projects = Vec::new();
        let a = add(&mut projects, &PathBuf::from(r"C:\dev\axon"));
        let b = add(&mut projects, &PathBuf::from(r"c:\DEV\axon\"));
        assert_eq!((a, b, projects.len()), (0, 0, 1));
        assert_eq!(projects[0].name, "axon");
    }

    #[test]
    fn test_branch_is_read_from_git_head() {
        let dir = std::env::temp_dir().join(format!("axon-branch-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let head = dir.join(".git").join("HEAD");
        std::fs::write(&head, "ref: refs/heads/feature/auth\n").unwrap();
        assert_eq!(read_branch(&dir).as_deref(), Some("feature/auth"));
        std::fs::write(&head, "0123456789abcdef0123456789abcdef01234567\n").unwrap();
        assert_eq!(read_branch(&dir).as_deref(), Some("0123456"));
        std::fs::write(&head, "basura").unwrap();
        assert_eq!(read_branch(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read_branch(&dir), None);
    }

    #[test]
    fn test_project_initial_and_color_slot() {
        assert_eq!(initial("axon"), 'A');
        assert_eq!(initial(".github"), 'G');
        assert_eq!(initial("..."), '?');
        assert_eq!(color_slot(0), color_slot(COLOR_SLOTS.len()));
    }
}
