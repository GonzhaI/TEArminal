//! Contexto del shell activo: rama git (sin ejecutar git) y agente de IA en ejecución.

use super::*;

pub(super) fn get_git_branch(start_dir: Option<&std::path::Path>) -> Option<String> {
    use std::io::BufRead;
    let mut cur = start_dir?.to_path_buf();
    let mut line_buf = String::with_capacity(128);
    for _ in 0..8 {
        let git_dir = cur.join(".git");
        let head_path = if git_dir.is_dir() {
            Some(git_dir.join("HEAD"))
        } else if git_dir.is_file() {
            line_buf.clear();
            let file = std::fs::File::open(&git_dir).ok()?;
            let mut reader = std::io::BufReader::new(file);
            if reader.read_line(&mut line_buf).ok()? > 0 {
                line_buf
                    .trim()
                    .strip_prefix("gitdir: ")
                    .map(|p| cur.join(p.trim()).join("HEAD"))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(head_path) = head_path {
            line_buf.clear();
            let file = std::fs::File::open(head_path).ok()?;
            let mut reader = std::io::BufReader::new(file);
            if reader.read_line(&mut line_buf).ok()? > 0 {
                let trimmed = line_buf.trim();
                if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                } else if trimmed.len() >= 7 {
                    return Some(trimmed[..7].to_string());
                }
            }
            return None;
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

pub(super) fn shorten_home<'a>(path: &'a std::path::Path) -> std::borrow::Cow<'a, str> {
    static HOME: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    let home = HOME.get_or_init(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from));
    if let Some(home) = home.as_deref() {
        if let Ok(rel) = path.strip_prefix(home) {
            let rel_str = rel.to_string_lossy();
            return if rel_str.is_empty() {
                std::borrow::Cow::Borrowed("~")
            } else {
                std::borrow::Cow::Owned(format!("~\\{}", rel_str.replace('/', "\\")))
            };
        }
        let s = path.to_string_lossy();
        let h = home.to_string_lossy();
        if s.len() >= h.len() && s[..h.len()].eq_ignore_ascii_case(&h) {
            let rest = &s[h.len()..];
            return if rest.is_empty() {
                std::borrow::Cow::Borrowed("~")
            } else {
                // OSC 7 entrega el cwd con `/`: la barra de estado usa `\` en todo el trayecto.
                std::borrow::Cow::Owned(format!(
                    "~\\{}",
                    rest.trim_start_matches(['\\', '/']).replace('/', "\\")
                ))
            };
        }
    }
    let s = path.to_string_lossy();
    // Solo rutas de unidad de Windows; las remotas (SSH) son POSIX y conservan `/`.
    if s.as_bytes().get(1) == Some(&b':') && s.contains('/') {
        std::borrow::Cow::Owned(s.replace('/', "\\"))
    } else {
        s
    }
}

fn contains_word_ignore_ascii_case(haystack: &str, word: &str) -> bool {
    let h_bytes = haystack.as_bytes();
    let w_bytes = word.as_bytes();
    let w_len = w_bytes.len();
    if h_bytes.len() < w_len {
        return false;
    }
    for i in 0..=(h_bytes.len() - w_len) {
        if h_bytes[i..i + w_len].eq_ignore_ascii_case(w_bytes) {
            let left_ok = if i == 0 {
                true
            } else {
                !h_bytes[i - 1].is_ascii_alphanumeric()
            };
            let right_ok = if i + w_len == h_bytes.len() {
                true
            } else {
                !h_bytes[i + w_len].is_ascii_alphanumeric()
            };
            if left_ok && right_ok {
                return true;
            }
        }
    }
    false
}

pub(crate) fn detect_ai_agent(title: Option<&str>) -> AiAgentStatus {
    use crate::renderer::contains_ignore_ascii_case;
    if let Some(t) = title {
        if contains_ignore_ascii_case(t, "claude") {
            return AiAgentStatus::Claude;
        }
        if contains_word_ignore_ascii_case(t, "agy") || contains_ignore_ascii_case(t, "antigravity")
        {
            return AiAgentStatus::Agy;
        }
        if contains_ignore_ascii_case(t, "gemini") {
            return AiAgentStatus::Gemini;
        }
        if contains_ignore_ascii_case(t, "aider") {
            return AiAgentStatus::Aider;
        }
        if contains_ignore_ascii_case(t, "copilot") {
            return AiAgentStatus::Copilot;
        }
        if contains_ignore_ascii_case(t, "ollama") {
            return AiAgentStatus::Ollama;
        }
    }
    AiAgentStatus::None
}

/// Rama git que muestra la salida de un comando en una sesión SSH (sin ejecutar nada en remoto):
/// `git status` ("On branch X" / "En la rama X"), `git branch` ("* X") o `git status -sb` ("## X...origin/X").
pub(super) fn branch_from_output_line(line: &str) -> Option<String> {
    let line = line.trim();
    let candidate = ["On branch ", "En la rama ", "En rama ", "* ", "## "]
        .iter()
        .find_map(|p| line.strip_prefix(p))?;
    let candidate = candidate.split("...").next()?.trim();
    let valid = !candidate.is_empty()
        && candidate
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
    valid.then(|| candidate.to_string())
}
