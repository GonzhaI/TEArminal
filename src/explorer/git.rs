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

/// Añade un archivo al índice de Git (`git add`)
pub(crate) fn git_stage_file(repo: &Path, file: &Path) -> std::io::Result<()> {
    let rel = file.strip_prefix(repo).unwrap_or(file);
    let status = git_command(repo).args(["add", "--"]).arg(rel).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git add falló"))
    }
}

/// Quita un archivo del índice de Git (`git restore --staged`, `git reset HEAD` o `git rm --cached`)
pub(crate) fn git_unstage_file(repo: &Path, file: &Path) -> std::io::Result<()> {
    let rel = file.strip_prefix(repo).unwrap_or(file);
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
    let rel = file.strip_prefix(repo).unwrap_or(file);
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
#[allow(dead_code)]
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
        let file_path = repo_root.join(rel_path.replace('/', "\\"));

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
