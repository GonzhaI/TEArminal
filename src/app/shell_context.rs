//! Contexto del shell activo: rama git (sin ejecutar git) y agente de IA en ejecución.

use super::*;

pub(super) fn get_git_branch(start_dir: Option<&std::path::Path>) -> Option<String> {
    let mut cur = start_dir?.to_path_buf();
    for _ in 0..8 {
        let git_dir = cur.join(".git");
        if git_dir.is_dir() {
            let head_path = git_dir.join("HEAD");
            if let Ok(content) = std::fs::read_to_string(head_path) {
                let line = content.lines().next()?.trim();
                if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                } else if line.len() >= 7 {
                    return Some(line[..7].to_string());
                }
            }
            return None;
        } else if git_dir.is_file() {
            if let Ok(content) = std::fs::read_to_string(&git_dir) {
                if let Some(path_str) = content
                    .lines()
                    .next()
                    .and_then(|l| l.strip_prefix("gitdir: "))
                {
                    let actual_git_dir = cur.join(path_str.trim());
                    let head_path = actual_git_dir.join("HEAD");
                    if let Ok(head_content) = std::fs::read_to_string(head_path) {
                        let line = head_content.lines().next()?.trim();
                        if let Some(branch) = line.strip_prefix("ref: refs/heads/") {
                            return Some(branch.to_string());
                        }
                    }
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

pub(crate) fn detect_ai_agent(title: Option<&str>) -> AiAgentStatus {
    if let Some(t) = title {
        let lower = t.to_lowercase();
        if lower.contains("claude") {
            return AiAgentStatus::Claude;
        }
        if lower.contains("agy") || lower.contains("antigravity") {
            return AiAgentStatus::Agy;
        }
        if lower.contains("gemini") {
            return AiAgentStatus::Gemini;
        }
        if lower.contains("aider") {
            return AiAgentStatus::Aider;
        }
        if lower.contains("copilot") {
            return AiAgentStatus::Copilot;
        }
        if lower.contains("ollama") {
            return AiAgentStatus::Ollama;
        }
    }
    AiAgentStatus::None
}
