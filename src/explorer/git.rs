use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GitFileStatus {
    #[default]
    Modified,
    Untracked,
    Added,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct GitStatusInfo {
    pub(crate) status: GitFileStatus,
    pub(crate) is_staged: bool,
    pub(crate) is_unstaged: bool,
}

/// Valida que `file` pertenezca a `repo` y no intente escapar con `..`
fn validate_repo_path<'a>(repo: &Path, file: &'a Path) -> std::io::Result<&'a Path> {
    let rel = if file.is_relative() {
        file
    } else {
        file.strip_prefix(repo).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ruta fuera del repositorio",
            )
        })?
    };
    if rel
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "la ruta contiene componentes '..'",
        ));
    }
    Ok(rel)
}

/// Añade un archivo al índice de Git (`git add`)
pub(crate) fn git_stage_file(repo: &Path, file: &Path) -> std::io::Result<()> {
    let rel = validate_repo_path(repo, file)?;
    let status = git_command(repo).args(["add", "--"]).arg(rel).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git add falló"))
    }
}

/// Añade todos los cambios al índice de Git (`git add -A`)
pub(crate) fn git_stage_all(repo: &Path) -> std::io::Result<()> {
    let status = git_command(repo).args(["add", "-A"]).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git add -A falló"))
    }
}

/// Quita un archivo del índice de Git (`git restore --staged`, `git reset HEAD` o `git rm --cached`)
pub(crate) fn git_unstage_file(repo: &Path, file: &Path) -> std::io::Result<()> {
    let rel = validate_repo_path(repo, file)?;
    let mut cmd = git_command(repo);
    cmd.args(["restore", "--staged", "--"]).arg(rel);
    cmd.stderr(std::process::Stdio::null());
    if let Ok(st) = cmd.status() {
        if st.success() {
            return Ok(());
        }
    }
    let mut reset_cmd = git_command(repo);
    reset_cmd.args(["reset", "HEAD", "--"]).arg(rel);
    reset_cmd.stderr(std::process::Stdio::null());
    if let Ok(status) = reset_cmd.status() {
        if status.success() {
            return Ok(());
        }
    }
    let mut rm_cmd = git_command(repo);
    rm_cmd.args(["rm", "--cached", "--"]).arg(rel);
    rm_cmd.stderr(std::process::Stdio::null());
    let status = rm_cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git unstage falló"))
    }
}

/// Quita todos los archivos del índice de Git (`git restore --staged .` con fallback a `git reset HEAD`)
pub(crate) fn git_unstage_all(repo: &Path) -> std::io::Result<()> {
    let mut cmd = git_command(repo);
    cmd.args(["restore", "--staged", "."]);
    cmd.stderr(std::process::Stdio::null());
    if let Ok(st) = cmd.status() {
        if st.success() {
            return Ok(());
        }
    }
    let mut reset_cmd = git_command(repo);
    reset_cmd.args(["reset", "HEAD"]);
    reset_cmd.stderr(std::process::Stdio::null());
    let status = reset_cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git restore --staged falló"))
    }
}

/// Descarta los cambios de un archivo en el árbol de trabajo (`git restore` o borrado de untracked)
pub(crate) fn git_discard_file(
    repo: &Path,
    file: &Path,
    is_untracked: bool,
) -> std::io::Result<()> {
    if is_untracked {
        if file.is_dir() {
            std::fs::remove_dir_all(file)?;
        } else if file.exists() {
            std::fs::remove_file(file)?;
        }
        return Ok(());
    }
    let rel = validate_repo_path(repo, file)?;
    let mut cmd = git_command(repo);
    cmd.args(["restore", "--"]).arg(rel);
    if let Ok(st) = cmd.status() {
        if st.success() {
            return Ok(());
        }
    }
    let mut co_cmd = git_command(repo);
    co_cmd.args(["checkout", "HEAD", "--"]).arg(rel);
    let status = co_cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git discard falló"))
    }
}

/// Descarta todos los cambios rastreados en el árbol de trabajo (`git restore .`)
pub(crate) fn git_discard_all(repo: &Path) -> std::io::Result<()> {
    let mut cmd = git_command(repo);
    cmd.args(["restore", "."]);
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git restore . falló"))
    }
}

/// Crea un commit con el mensaje dado, opcionalmente enmendando el último commit (`--amend`)
pub(crate) fn git_commit(repo: &Path, message: &str, amend: bool) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("El mensaje de commit no puede estar vacío".to_string());
    }
    let mut cmd = git_command(repo);
    cmd.arg("commit");
    if amend {
        cmd.arg("--amend");
    }
    cmd.arg("-m").arg(message);
    let output = cmd
        .output()
        .map_err(|e| format!("No se pudo ejecutar git commit: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let err_msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "Fallo al ejecutar git commit".to_string()
        };
        Err(err_msg)
    }
}

/// Empuja los commits de la rama actual al repositorio remoto (`git push`)
pub(crate) fn git_push(repo: &Path) -> Result<(), String> {
    let mut cmd = git_command(repo);
    cmd.arg("push");
    let output = cmd
        .output()
        .map_err(|e| format!("No se pudo ejecutar git push: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let err_msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "Fallo al ejecutar git push".to_string()
        };
        Err(err_msg)
    }
}

/// Consulta cuántos commits estamos por delante y por detrás del upstream (`HEAD...@{upstream}`)
pub(crate) fn git_ahead_behind(repo: &Path) -> Option<(usize, usize)> {
    let output = git_command(repo)
        .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let ahead = parts.next()?.parse().ok()?;
    let behind = parts.next()?.parse().ok()?;
    Some((ahead, behind))
}

/// Obtiene el mensaje del último commit (`git log -1 --pretty=%B`) para la opción de enmendar
pub(crate) fn git_last_commit_message(repo: &Path) -> Option<String> {
    let output = git_command(repo)
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let msg = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    (!msg.is_empty()).then_some(msg)
}

/// Consulta la lista de ramas locales del repositorio y marca cuál es la actual
pub(crate) fn git_local_branches(repo: &Path) -> Vec<(String, bool)> {
    let Ok(output) = git_command(repo)
        .args([
            "for-each-ref",
            "--format=%(HEAD)%09%(refname:short)",
            "refs/heads",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();
    for line in text.lines() {
        let Some((head_mark, name)) = line.split_once('\t') else {
            continue;
        };
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            branches.push((trimmed.to_string(), head_mark.trim() == "*"));
        }
    }
    branches
}

/// Cambia de rama en el repositorio validando el nombre para evitar flags inyectados
pub(crate) fn git_checkout_branch(repo: &Path, branch: &str) -> Result<(), String> {
    let trimmed = branch.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('-')
        || trimmed.chars().any(char::is_whitespace)
        || trimmed.contains("..")
    {
        return Err("Nombre de rama inválido".to_string());
    }
    let output = git_command(repo)
        .args(["checkout", trimmed, "--"])
        .output()
        .map_err(|e| format!("Error al ejecutar git checkout: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if err.is_empty() {
            Err("git checkout falló".to_string())
        } else {
            Err(err)
        }
    }
}

/// Raíz del repositorio git que contiene `dir` (busca `.git` hacia arriba)
pub(crate) fn git_repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|p| p.join(".git").exists())
        .map(Path::to_path_buf)
}

pub(crate) fn git_command(repo: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(repo);
    crate::app::platform::hide_console(&mut cmd);
    cmd
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitHeadResult {
    /// Fuera de un repositorio Git
    None,
    /// Archivo nuevo dentro del repo (untracked o recién añadido, no existe en HEAD)
    NewFile,
    /// Contenido base en HEAD
    Lines(Vec<String>),
}

/// Consulta HEAD para `path`: `Lines` si existe en HEAD, `NewFile` si está en el repo pero no en HEAD,
/// o `None` si está fuera de un repositorio.
pub(crate) fn git_head_result(path: &Path) -> GitHeadResult {
    let Some(repo) = git_repo_root(path.parent().unwrap_or(path)) else {
        return GitHeadResult::None;
    };
    let Ok(rel) = path.strip_prefix(&repo) else {
        return GitHeadResult::None;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    let Ok(out) = git_command(&repo)
        .args(["show", &format!("HEAD:{rel}")])
        .output()
    else {
        return GitHeadResult::None;
    };
    if !out.status.success() {
        return GitHeadResult::NewFile;
    }
    GitHeadResult::Lines(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(String::from)
            .collect(),
    )
}

/// Contenido de `path` en `HEAD`, base para marcar en el visor lo que cambió.
/// `None` si el archivo no está versionado en el repo (untracked) o fuera de un repo.
#[cfg(test)]
pub(crate) fn git_head_lines(path: &Path) -> Option<Vec<String>> {
    match git_head_result(path) {
        GitHeadResult::Lines(lines) => Some(lines),
        _ => None,
    }
}

/// Estado de una carpeta según lo que cambió dentro: si todo es del mismo tipo, ese (verde añadido,
/// rojo borrado); con una mezcla, o con modificados, amarillo. `None` si no cambió nada.
pub(crate) fn dir_git_status(
    git_map: &FxHashMap<PathBuf, GitStatusInfo>,
    dir: &Path,
) -> Option<GitFileStatus> {
    let mut found: Option<GitFileStatus> = None;
    for (path, info) in git_map {
        if !path.starts_with(dir) {
            continue;
        }
        let kind = match info.status {
            GitFileStatus::Untracked | GitFileStatus::Added => GitFileStatus::Added,
            GitFileStatus::Deleted => GitFileStatus::Deleted,
            GitFileStatus::Modified | GitFileStatus::Renamed => GitFileStatus::Modified,
        };
        match found {
            None => found = Some(kind),
            Some(prev) if prev == kind => {}
            Some(_) => return Some(GitFileStatus::Modified),
        }
    }
    found
}

/// Ejecuta `git status --porcelain -z` sobre `repo_root` y devuelve el estado por ruta absoluta.
/// `--no-optional-locks`: sin él `git status` toma `index.lock` para refrescar el índice y hace fallar un
/// `git add`/`commit` que el usuario o un agente lance a la vez.
pub(crate) fn collect_git_statuses(
    repo_root: &Path,
    changes_only: bool,
) -> FxHashMap<PathBuf, GitStatusInfo> {
    let untracked_mode = if changes_only { "-uall" } else { "-unormal" };
    let Ok(output) = git_command(repo_root)
        .args(["--no-optional-locks", "status", "--porcelain", "-z"])
        .arg(untracked_mode)
        .output()
    else {
        return FxHashMap::default();
    };
    if !output.status.success() {
        return FxHashMap::default();
    }
    parse_porcelain_z(repo_root, &output.stdout)
}

/// Salida de `git status --porcelain -z`: entradas `XY ruta` separadas por NUL, sin comillas ni escapes (los
/// nombres con tildes o espacios llegan tal cual); tras un renombrado/copia va otra entrada con el origen
pub(crate) fn parse_porcelain_z(
    repo_root: &Path,
    stdout: &[u8],
) -> FxHashMap<PathBuf, GitStatusInfo> {
    let mut map = FxHashMap::default();
    let text = String::from_utf8_lossy(stdout);
    let mut entries = text.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(status_code), Some(rel_path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        if rel_path.is_empty() {
            continue;
        }
        if status_code.contains(['R', 'C']) {
            entries.next();
        }
        let repo_str = repo_root.to_string_lossy();
        let file_path = if repo_str.starts_with('/') {
            let clean_root = repo_str.replace('\\', "/");
            let clean_root = clean_root.trim_end_matches('/');
            let clean_rel = rel_path.trim_start_matches('/');
            PathBuf::from(format!("{clean_root}/{clean_rel}"))
        } else {
            repo_root.join(rel_path.replace('/', "\\"))
        };

        let bytes = status_code.as_bytes();
        let x = bytes.first().copied().unwrap_or(b' ');
        let y = bytes.get(1).copied().unwrap_or(b' ');

        let (status, is_staged, is_unstaged) = if x == b'?' && y == b'?' {
            (GitFileStatus::Untracked, false, true)
        } else {
            let staged = x != b' ' && x != b'?';
            let unstaged = y != b' ' && y != b'?';
            let st = if x == b'A' || y == b'A' {
                GitFileStatus::Added
            } else if x == b'D' || y == b'D' {
                GitFileStatus::Deleted
            } else if x == b'R' || y == b'R' {
                GitFileStatus::Renamed
            } else {
                GitFileStatus::Modified
            };
            (st, staged, unstaged)
        };

        map.insert(
            file_path,
            GitStatusInfo {
                status,
                is_staged,
                is_unstaged,
            },
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_porcelain_z_unix_and_windows_paths() {
        let unix_root = Path::new("/home/django/repo");
        let stdout = b" M app/models.py\0?? config.ini\0A  new_file.txt\0";
        let map = parse_porcelain_z(unix_root, stdout);
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get(&PathBuf::from("/home/django/repo/app/models.py"))
                .map(|i| (i.status, i.is_staged, i.is_unstaged)),
            Some((GitFileStatus::Modified, false, true))
        );
        assert_eq!(
            map.get(&PathBuf::from("/home/django/repo/config.ini"))
                .map(|i| (i.status, i.is_staged, i.is_unstaged)),
            Some((GitFileStatus::Untracked, false, true))
        );
        assert_eq!(
            map.get(&PathBuf::from("/home/django/repo/new_file.txt"))
                .map(|i| (i.status, i.is_staged, i.is_unstaged)),
            Some((GitFileStatus::Added, true, false))
        );
    }

    #[test]
    fn test_validate_repo_path_traversal() {
        let repo = Path::new("C:\\Users\\Gonzalo\\repo");
        let safe = Path::new("C:\\Users\\Gonzalo\\repo\\src\\main.rs");
        assert!(validate_repo_path(repo, safe).is_ok());

        let escaping = Path::new("C:\\Users\\Gonzalo\\repo\\..\\secret.txt");
        assert!(validate_repo_path(repo, escaping).is_err());

        let outside = Path::new("C:\\Users\\Gonzalo\\other\\file.txt");
        assert!(validate_repo_path(repo, outside).is_err());

        let rel_safe = Path::new("src/main.rs");
        assert!(validate_repo_path(repo, rel_safe).is_ok());

        let rel_escape = Path::new("../secret.txt");
        assert!(validate_repo_path(repo, rel_escape).is_err());
    }

    fn setup_temp_repo() -> Option<PathBuf> {
        static CNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("tearminal_test_git_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).ok()?;
        let init = git_command(&path).args(["init"]).status().ok()?;
        if !init.success() {
            let _ = std::fs::remove_dir_all(&path);
            return None;
        }
        let _ = git_command(&path)
            .args(["config", "user.name", "TEArminal Test"])
            .status();
        let _ = git_command(&path)
            .args(["config", "user.email", "test@tearminal.local"])
            .status();
        let _ = git_command(&path)
            .args(["config", "commit.gpgsign", "false"])
            .status();
        Some(path)
    }

    #[test]
    fn test_git_operations_lifecycle_in_temp_repo() {
        let Some(repo) = setup_temp_repo() else {
            return;
        };

        // 1. Crear archivo y comprobar stage
        let file_a = repo.join("archivo_a.txt");
        std::fs::write(&file_a, "Línea 1\n").unwrap();

        let st1 = collect_git_statuses(&repo, true);
        assert!(st1
            .get(&file_a)
            .is_some_and(|i| i.status == GitFileStatus::Untracked));

        git_stage_file(&repo, &file_a).expect("stage file_a");
        let st2 = collect_git_statuses(&repo, true);
        assert!(st2
            .get(&file_a)
            .is_some_and(|i| i.is_staged && !i.is_unstaged));

        // 2. Unstage
        git_unstage_file(&repo, &file_a).expect("unstage file_a");
        let st3 = collect_git_statuses(&repo, true);
        assert!(st3.get(&file_a).is_some_and(|i| !i.is_staged));

        // 3. Stage All y Commit con comillas y caracteres especiales
        git_stage_all(&repo).expect("stage all");
        let commit_msg = "feat: primer commit con 'comillas' y caracteres especiales: áéíóú #1";
        git_commit(&repo, commit_msg, false).expect("commit");

        let last_msg = git_last_commit_message(&repo);
        assert_eq!(last_msg.as_deref(), Some(commit_msg));

        // 4. Modificar archivo, enmendar commit
        std::fs::write(&file_a, "Línea 1\nLínea 2\n").unwrap();
        git_stage_file(&repo, &file_a).expect("stage modified");

        let amend_msg = "feat: commit enmendado\n\nCon cuerpo multilínea.";
        git_commit(&repo, amend_msg, true).expect("amend commit");

        let last_msg_amended = git_last_commit_message(&repo);
        assert_eq!(last_msg_amended.as_deref(), Some(amend_msg));

        // 5. Descartar cambios
        std::fs::write(&file_a, "Cambio no deseado\n").unwrap();
        git_discard_file(&repo, &file_a, false).expect("discard file_a");
        let content = std::fs::read_to_string(&file_a).unwrap();
        assert_eq!(content, "Línea 1\nLínea 2\n");

        // 6. Descartar todo (git_discard_all)
        std::fs::write(&file_a, "Otro cambio que se descartará en bloque\n").unwrap();
        git_discard_all(&repo).expect("discard all");
        let content_after_discard_all = std::fs::read_to_string(&file_a).unwrap();
        assert_eq!(content_after_discard_all, "Línea 1\nLínea 2\n");

        // 7. Listar ramas locales y validar rechazo de nombres inválidos en checkout
        let branches = git_local_branches(&repo);
        assert!(!branches.is_empty());
        assert!(branches.iter().any(|(_, is_cur)| *is_cur));
        assert!(git_checkout_branch(&repo, "--orphan").is_err());
        assert!(git_checkout_branch(&repo, "a b").is_err());
        let _ = std::fs::remove_dir_all(&repo);
    }
}
