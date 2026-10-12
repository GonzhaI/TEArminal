use std::path::PathBuf;
use std::process::Command;
use std::thread;

pub const TOOL_BINARIES: &[&str] = &[
    "eza.exe",
    "bat.exe",
    "rg.exe",
    "fd.exe",
    "zoxide.exe",
    "fzf.exe",
];

/// Devuelve el directorio de binarios a nivel de la aplicación Axon.
/// 1. Prioridad: `bin/` junto al ejecutable (modo portable / bundle distribuido).
/// 2. Fallback: `%LOCALAPPDATA%\axon\bin` (instalación por usuario).
pub fn get_bin_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let portable_bin = parent.join("bin");
            if portable_bin.exists() {
                return portable_bin;
            }
        }
    }

    if let Some(dirs) = directories::ProjectDirs::from("com", "axon", "axon") {
        let local_bin = dirs.data_local_dir().join("bin");
        let _ = std::fs::create_dir_all(&local_bin);
        return local_bin;
    }

    let fallback = PathBuf::from(r"C:\Users\Default\AppData\Local\axon\bin");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// Comprueba si todos los binarios esenciales están instalados en `get_bin_dir()`.
pub fn is_all_tools_installed() -> bool {
    let bin_dir = get_bin_dir();
    TOOL_BINARIES.iter().all(|name| bin_dir.join(name).exists())
}

/// Asegura en segundo plano que los binarios modernos estén presentes en la terminal Axon.
/// Si faltan binarios, los aprovisiona descargando las versiones oficiales para Windows x64.
pub fn ensure_tools_installed_background() {
    if is_all_tools_installed() {
        return;
    }

    thread::Builder::new()
        .name("axon-tool-provisioner".into())
        .spawn(|| {
            let bin_dir = get_bin_dir();
            let missing: Vec<&&str> = TOOL_BINARIES
                .iter()
                .filter(|name| !bin_dir.join(name).exists())
                .collect();

            if missing.is_empty() {
                return;
            }

            crate::logger::log(&format!(
                "[TOOLS] Binarios faltantes detectados: {:?}. Iniciando aprovisionamiento automático...",
                missing
            ));

            let script = r#"
                $binDir = $args[0]
                if (-not (Test-Path $binDir)) { New-Item -ItemType Directory -Path $binDir -Force | Out-Null }
                $tools = @(
                    @{ Name = 'eza.exe'; Repo = 'eza-community/eza'; Match = '*x86_64*windows*zip*' },
                    @{ Name = 'bat.exe'; Repo = 'sharkdp/bat'; Match = '*x86_64*windows*zip*' },
                    @{ Name = 'rg.exe'; Repo = 'BurntSushi/ripgrep'; Match = '*x86_64*windows*zip*' },
                    @{ Name = 'fd.exe'; Repo = 'sharkdp/fd'; Match = '*x86_64*windows*zip*' },
                    @{ Name = 'zoxide.exe'; Repo = 'ajeetdsouza/zoxide'; Match = '*x86_64*windows*zip*' },
                    @{ Name = 'fzf.exe'; Repo = 'junegunn/fzf'; Match = '*windows_amd64.zip*' }
                )
                foreach ($t in $tools) {
                    $target = Join-Path $binDir $t.Name
                    if (Test-Path $target) { continue }
                    try {
                        $rel = Invoke-RestMethod -Uri ("https://api.github.com/repos/" + $t.Repo + "/releases/latest") -Headers @{ "User-Agent" = "Axon-Terminal" }
                        $asset = $rel.assets | Where-Object { $_.name -like $t.Match } | Select-Object -First 1
                        if (-not $asset) {
                            $asset = $rel.assets | Where-Object { $_.name -like '*windows*' -and $_.name -like '*zip*' } | Select-Object -First 1
                        }
                        if ($asset) {
                            $tmpZip = Join-Path $env:TEMP ("axon_" + $asset.name)
                            $tmpExtract = Join-Path $env:TEMP ("axon_extract_" + [System.IO.Path]::GetFileNameWithoutExtension($asset.name))
                            Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpZip -UseBasicParsing
                            Expand-Archive -Path $tmpZip -DestinationPath $tmpExtract -Force
                            $found = Get-ChildItem -Path $tmpExtract -Filter $t.Name -Recurse | Select-Object -First 1
                            if ($found) {
                                Copy-Item -Path $found.FullName -Destination $target -Force
                            }
                            Remove-Item -Path $tmpZip -Force -ErrorAction SilentlyContinue
                            Remove-Item -Path $tmpExtract -Recurse -Force -ErrorAction SilentlyContinue
                        }
                    } catch {}
                }
            "#;

            #[cfg(windows)]
            use std::os::windows::process::CommandExt;

            let mut cmd = Command::new("powershell.exe");
            #[cfg(windows)]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

            let _ = cmd
                .arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-Command")
                .arg(script)
                .arg(bin_dir.to_string_lossy().to_string())
                .output();

            crate::logger::log("[TOOLS] Aprovisionamiento de herramientas Axon completado.");
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_bin_dir_exists() {
        let dir = get_bin_dir();
        assert!(dir.is_absolute());
        assert!(dir.exists() || std::fs::create_dir_all(&dir).is_ok());
    }

    #[test]
    fn test_tools_binaries_list() {
        assert_eq!(TOOL_BINARIES.len(), 6);
        assert!(TOOL_BINARIES.contains(&"eza.exe"));
        assert!(TOOL_BINARIES.contains(&"bat.exe"));
        assert!(TOOL_BINARIES.contains(&"rg.exe"));
        assert!(TOOL_BINARIES.contains(&"fd.exe"));
        assert!(TOOL_BINARIES.contains(&"zoxide.exe"));
        assert!(TOOL_BINARIES.contains(&"fzf.exe"));
    }
}
