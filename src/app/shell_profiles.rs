//! Perfiles de shell disponibles en Windows: PowerShell 7, Windows PowerShell 5.1, WSL, Git Bash y CMD.

use crate::ui::LucideIcon;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellProfile {
    pub id: &'static str,
    pub title: &'static str,
    pub command: String,
    pub icon: LucideIcon,
}

/// Detecta los shells instalados en el sistema y los devuelve en orden de preferencia.
pub(crate) fn available_shell_profiles() -> Vec<ShellProfile> {
    let mut profiles = Vec::with_capacity(5);

    // 1. PowerShell 7 (si está instalado)
    if let Some(pwsh) = crate::config::find_pwsh_path() {
        profiles.push(ShellProfile {
            id: "pwsh",
            title: "PowerShell 7",
            command: pwsh,
            icon: LucideIcon::Terminal,
        });
    }

    // 2. Windows PowerShell 5.1 (siempre presente en Windows)
    profiles.push(ShellProfile {
        id: "powershell",
        title: "Windows PowerShell",
        command: "powershell.exe".to_string(),
        icon: LucideIcon::Terminal,
    });

    // 3. WSL (si wsl.exe existe en System32)
    if let Some(wsl) = detect_wsl() {
        profiles.push(ShellProfile {
            id: "wsl",
            title: "WSL (Linux)",
            command: wsl,
            icon: LucideIcon::Terminal,
        });
    }

    // 4. Git Bash (si está instalado)
    if let Some(bash) = detect_git_bash() {
        profiles.push(ShellProfile {
            id: "git-bash",
            title: "Git Bash",
            command: bash,
            icon: LucideIcon::GitBranch,
        });
    }

    // 5. Símbolo del sistema (CMD)
    profiles.push(ShellProfile {
        id: "cmd",
        title: "Símbolo del sistema (CMD)",
        command: "cmd.exe".to_string(),
        icon: LucideIcon::Terminal,
    });

    profiles
}

fn detect_git_bash() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let base = PathBuf::from(pf).join("Git");
        candidates.push(base.join("bin").join("bash.exe"));
        candidates.push(base.join("usr").join("bin").join("bash.exe"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let base = PathBuf::from(pf86).join("Git");
        candidates.push(base.join("bin").join("bash.exe"));
        candidates.push(base.join("usr").join("bin").join("bash.exe"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let base = PathBuf::from(local_app_data).join("Programs").join("Git");
        candidates.push(base.join("bin").join("bash.exe"));
        candidates.push(base.join("usr").join("bin").join("bash.exe"));
    }
    candidates.push(PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"));

    for path in candidates {
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(path) = crate::config::search_path_native("bash.exe") {
        let path_str = path.to_string_lossy();
        if !path_str.to_ascii_lowercase().contains("windowsapps") && path.is_file() {
            return Some(path_str.to_string());
        }
    }
    None
}

fn detect_wsl() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(system_root) = std::env::var("SystemRoot") {
        candidates.push(PathBuf::from(system_root).join("System32").join("wsl.exe"));
    }
    candidates.push(PathBuf::from(r"C:\Windows\System32\wsl.exe"));

    for path in candidates {
        if path.is_file() {
            return Some("wsl.exe".to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::context_menu::MenuAction;

    #[test]
    fn test_available_shell_profiles_contains_builtin_shells() {
        let profiles = available_shell_profiles();
        assert!(
            !profiles.is_empty(),
            "debe haber al menos un shell en Windows"
        );
        let has_ps = profiles
            .iter()
            .any(|p| p.id == "powershell" || p.id == "pwsh");
        assert!(has_ps, "debe incluir PowerShell");
        let has_cmd = profiles.iter().any(|p| p.id == "cmd");
        assert!(has_cmd, "debe incluir CMD");
        for p in profiles {
            assert!(!p.command.is_empty());
            assert!(!p.title.is_empty());
        }
    }

    #[test]
    fn test_shell_menu_action_variants() {
        let act = MenuAction::NewTabWithShell("pwsh.exe".into());
        assert_eq!(act, MenuAction::NewTabWithShell("pwsh.exe".into()));
        assert_ne!(act, MenuAction::NewTab);

        let split_v = MenuAction::SplitVerticalWithShell("cmd.exe".into());
        assert_ne!(split_v, MenuAction::SplitVertical);

        let split_h = MenuAction::SplitHorizontalWithShell("bash.exe".into());
        assert_ne!(split_h, MenuAction::SplitHorizontal);
    }
}
