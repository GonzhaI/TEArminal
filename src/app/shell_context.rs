//! Contexto del shell activo: rama git (sin ejecutar git) y agente de IA en ejecución.

use super::*;

pub(super) fn get_git_branch(start_dir: Option<&std::path::Path>) -> Option<String> {
    let mut cur = start_dir?.to_path_buf();
    for _ in 0..8 {
        let git_dir = cur.join(".git");
        let head_path = if git_dir.is_dir() {
            Some(git_dir.join("HEAD"))
        } else if git_dir.is_file() {
            std::fs::read_to_string(&git_dir).ok().and_then(|c| {
                c.lines()
                    .next()?
                    .strip_prefix("gitdir: ")
                    .map(|p| cur.join(p.trim()).join("HEAD"))
            })
        } else {
            None
        };
        if let Some(head_path) = head_path {
            let content = std::fs::read_to_string(head_path).ok()?;
            let line = content.lines().next()?.trim();
            if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                return Some(branch.to_string());
            } else if line.len() >= 7 {
                return Some(line[..7].to_string());
            }
            return None;
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

pub(super) fn shorten_home(path: &std::path::Path) -> String {
    static HOME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let s = path.to_string_lossy();
    match HOME.get_or_init(|| std::env::var("USERPROFILE").ok()) {
        Some(home) if s.starts_with(home.as_str()) => format!("~{}", &s[home.len()..]),
        _ => s.into_owned(),
    }
}

pub(crate) fn detect_ai_agent(title: Option<&str>) -> AiAgentStatus {
    use crate::renderer::contains_ignore_ascii_case;
    if let Some(t) = title {
        if contains_ignore_ascii_case(t, "claude") {
            return AiAgentStatus::Claude;
        }
        if contains_ignore_ascii_case(t, "agy") || contains_ignore_ascii_case(t, "antigravity") {
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
