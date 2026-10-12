use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use winit::event_loop::EventLoopProxy;
use crate::app::AxonEvent;

pub struct PtyManager {
    #[allow(dead_code)]
    pub id: usize,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    to_pty_tx: Sender<Vec<u8>>,
    from_pty_rx: Receiver<Vec<u8>>,
}

impl PtyManager {
    pub fn new(
        id: usize,
        shell: &str,
        ghostty_prompt: bool,
        cols: u16,
        rows: u16,
        cwd: Option<&std::path::Path>,
        proxy: Option<EventLoopProxy<AxonEvent>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        if let Some(dir) = cwd {
            if dir.exists() {
                cmd.cwd(dir);
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // Inyectar el directorio de herramientas a nivel de aplicación al frente del PATH
        let tools_dir = crate::tools::get_bin_dir();
        let path_env = if let Ok(current_path) = std::env::var("PATH") {
            format!("{};{}", tools_dir.display(), current_path)
        } else {
            tools_dir.display().to_string()
        };
        cmd.env("PATH", &path_env);

        // Si se trata de PowerShell y ghostty_prompt está activo, inyectar el entorno Ghostty nativo
        if ghostty_prompt {
            let lower = shell.to_lowercase();
            if lower.contains("powershell") || lower.contains("pwsh") {
                cmd.arg("-NoLogo");
                cmd.arg("-NoExit");
                cmd.arg("-Command");
                let init_script = concat!(
                    "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; ",
                    "$OutputEncoding = [System.Text.Encoding]::UTF8; ",
                    "Import-Module PSReadLine -ErrorAction SilentlyContinue; ",
                    "Set-PSReadLineOption -Colors @{ ",
                    "Command = ([char]27 + '[38;2;81;226;122m'); ",
                    "Parameter = ([char]27 + '[38;2;78;146;249m'); ",
                    "Operator = ([char]27 + '[38;2;246;196;66m'); ",
                    "String = ([char]27 + '[38;2;120;247;156m'); ",
                    "Number = ([char]27 + '[38;2;207;112;255m'); ",
                    "Member = ([char]27 + '[38;2;54;214;214m'); ",
                    "Error = ([char]27 + '[38;2;244;56;65m') ",
                    "} -ErrorAction SilentlyContinue; ",
                    "Set-PSReadLineKeyHandler -Chord 'Alt+Backspace' -Function BackwardKillWord -ErrorAction SilentlyContinue; ",
                    "function prompt { ",
                    "$rawPath = (Get-Location).Path; ",
                    "$p = $rawPath.Replace($HOME, '~'); ",
                    "$esc = [char]27; ",
                    "$osc7 = '' + $esc + ']7;file://localhost/' + $rawPath.Replace('\\', '/') + [char]7; ",
                    "$git = ''; ",
                    "try { ",
                    "$b = (git rev-parse --abbrev-ref HEAD 2>$null); ",
                    "if ($b) { $git = ' ' + $esc + '[38;2;81;226;122m' + [char]0xe0a0 + ' ' + $b + $esc + '[0m' } ",
                    "} catch {}; ",
                    "return $osc7 + $esc + ']133;A`a' + $esc + '[38;2;78;146;249m' + [char]0xf07b + ' ' + $p + $esc + '[0m' + $git + [Environment]::NewLine + $esc + '[38;2;81;226;122m' + [char]0xf432 + ' ' + $esc + '[0m' + $esc + ']133;B`a' ",
                    "}; ",
                    "function ls { ",
                    "param([Parameter(Position=0, ValueFromPipeline=$true)][string]$Path = '.', [switch]$a, [switch]$Force); ",
                    "$eza = Get-Command -CommandType Application -Name eza.exe, eza -ErrorAction SilentlyContinue | Select-Object -First 1; ",
                    "if ($eza) { ",
                    "$ezaArgs = @('--icons', '--group-directories-first'); ",
                    "if ($a -or $Force) { $ezaArgs += '-a' }; ",
                    "if ($Path -and $Path -ne '.') { $ezaArgs += $Path }; ",
                    "& $eza @ezaArgs; return ",
                    "}; ",
                    "$lsd = Get-Command -CommandType Application -Name lsd.exe, lsd -ErrorAction SilentlyContinue | Select-Object -First 1; ",
                    "if ($lsd) { ",
                    "$lsdArgs = @('--group-dirs', 'first'); ",
                    "if ($a -or $Force) { $lsdArgs += '-a' }; ",
                    "if ($Path -and $Path -ne '.') { $lsdArgs += $Path }; ",
                    "& $lsd @lsdArgs; return ",
                    "}; ",
                    "$esc = [char]27; ",
                    "$show = $a -or $Force; ",
                    "$items = if ($show) { Get-ChildItem -Path $Path -Force -ErrorAction SilentlyContinue } else { Get-ChildItem -Path $Path -ErrorAction SilentlyContinue }; ",
                    "if (-not $items) { return }; ",
                    "$items = @($items | Sort-Object { -not $_.PSIsContainer }, Name); ",
                    "$maxLen = 0; foreach ($i in $items) { if ($i.Name.Length -gt $maxLen) { $maxLen = $i.Name.Length } }; ",
                    "$colWidth = [Math]::Max($maxLen + 2, 24); ",
                    "Write-Host ''; ",
                    "foreach ($item in $items) { ",
                    "$padded = $item.Name.PadRight($colWidth); ",
                    "if ($item.PSIsContainer) { ",
                    "$icon = $esc + '[38;2;78;146;249m' + [char]0xf07b + $esc + '[0m'; ",
                    "$name = $esc + '[1;38;2;78;146;249m' + $padded + $esc + '[0m'; ",
                    "$size = $esc + '[2m' + '<DIR>'.PadLeft(9) + $esc + '[0m' ",
                    "} else { ",
                    "$ext = $item.Extension.ToLower(); ",
                    "$icon = switch ($ext) { ",
                    "'.rs'   { $esc + '[38;2;244;56;65m'   + [char]0xe7a8 + $esc + '[0m' } ",
                    "'.toml' { $esc + '[38;2;246;196;66m'  + [char]0xe6b2 + $esc + '[0m' } ",
                    "'.yaml' { $esc + '[38;2;246;196;66m'  + [char]0xe6b2 + $esc + '[0m' } ",
                    "'.yml'  { $esc + '[38;2;246;196;66m'  + [char]0xe6b2 + $esc + '[0m' } ",
                    "'.json' { $esc + '[38;2;246;196;66m'  + [char]0xe60b + $esc + '[0m' } ",
                    "'.md'   { $esc + '[38;2;78;146;249m'   + [char]0xe73e + $esc + '[0m' } ",
                    "'.ps1'  { $esc + '[38;2;54;214;214m'   + [char]0xe70f + $esc + '[0m' } ",
                    "'.bat'  { $esc + '[38;2;54;214;214m'   + [char]0xe70f + $esc + '[0m' } ",
                    "'.cmd'  { $esc + '[38;2;54;214;214m'   + [char]0xe70f + $esc + '[0m' } ",
                    "'.exe'  { $esc + '[38;2;81;226;122m'   + [char]0xf489 + $esc + '[0m' } ",
                    "'.dll'  { $esc + '[38;2;207;112;255m'  + [char]0xf013 + $esc + '[0m' } ",
                    "'.png'  { $esc + '[38;2;207;112;255m'  + [char]0xf1c5 + $esc + '[0m' } ",
                    "'.jpg'  { $esc + '[38;2;207;112;255m'  + [char]0xf1c5 + $esc + '[0m' } ",
                    "'.svg'  { $esc + '[38;2;246;196;66m'  + [char]0xf1c5 + $esc + '[0m' } ",
                    "'.zip'  { $esc + '[38;2;255;220;107m' + [char]0xf1c6 + $esc + '[0m' } ",
                    "'.tar'  { $esc + '[38;2;255;220;107m' + [char]0xf1c6 + $esc + '[0m' } ",
                    "'.gz'   { $esc + '[38;2;255;220;107m' + [char]0xf1c6 + $esc + '[0m' } ",
                    "'.lock' { $esc + '[38;2;120;247;156m' + [char]0xf023 + $esc + '[0m' } ",
                    "'.py'   { $esc + '[38;2;246;196;66m'  + [char]0xe73c + $esc + '[0m' } ",
                    "'.js'   { $esc + '[38;2;246;196;66m'  + [char]0xe74e + $esc + '[0m' } ",
                    "'.ts'   { $esc + '[38;2;78;146;249m'   + [char]0xe628 + $esc + '[0m' } ",
                    "default { if ($item.Name.StartsWith('.')) { $esc + '[38;2;92;95;110m' + [char]0xf15b + $esc + '[0m' } else { $esc + '[38;2;230;230;235m' + [char]0xf15b + $esc + '[0m' } } ",
                    "}; ",
                    "$name = $esc + '[38;2;230;230;235m' + $padded + $esc + '[0m'; ",
                    "$len = $item.Length; ",
                    "$str = if ($len -gt 1GB) { '{0:N1} GB' -f ($len / 1GB) } elseif ($len -gt 1MB) { '{0:N1} MB' -f ($len / 1MB) } elseif ($len -gt 1KB) { '{0:N1} KB' -f ($len / 1KB) } else { '{0} B' -f $len }; ",
                    "$size = $esc + '[2m' + ($str.PadLeft(9)) + $esc + '[0m' ",
                    "}; ",
                    "$date = $esc + '[2m' + $item.LastWriteTime.ToString('yyyy-MM-dd HH:mm') + $esc + '[0m'; ",
                    "Write-Host ('  {0}  {1}  {2}  {3}' -f $icon, $name, $size, $date) ",
                    "}; ",
                    "Write-Host '' ",
                    "}; ",
                    "Remove-Item alias:ls -Force -ErrorAction SilentlyContinue; ",
                    "Remove-Item alias:dir -Force -ErrorAction SilentlyContinue; ",
                    "Set-Alias -Name dir -Value ls -Option AllScope -ErrorAction SilentlyContinue; ",
                    "if (Get-Command bat.exe, bat -ErrorAction SilentlyContinue) { ",
                    "Remove-Item alias:cat -Force -ErrorAction SilentlyContinue; ",
                    "Set-Alias -Name cat -Value bat -Option AllScope -ErrorAction SilentlyContinue; ",
                    "}; ",
                    "if (Get-Command rg.exe, rg -ErrorAction SilentlyContinue) { ",
                    "Set-Alias -Name grep -Value rg -Option AllScope -ErrorAction SilentlyContinue; ",
                    "}; ",
                    "if (Get-Command fd.exe, fd -ErrorAction SilentlyContinue) { ",
                    "Set-Alias -Name find -Value fd -Option AllScope -ErrorAction SilentlyContinue; ",
                    "}; ",
                    "if (Get-Command zoxide.exe, zoxide -ErrorAction SilentlyContinue) { ",
                    "Invoke-Expression (& zoxide init powershell --cmd cd | Out-String) -ErrorAction SilentlyContinue; ",
                    "}; ",
                    "function agy { ",
                    "$cmd = (Get-Command -CommandType Application -Name agy.exe, agy -ErrorAction SilentlyContinue | Select-Object -First 1); ",
                    "if ($cmd) { ",
                    "Clear-Host; [Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H'); ",
                    "$h = $Host.UI.RawUI.WindowSize.Height; for ($i = 0; $i -lt $h; $i++) { [Console]::WriteLine('') }; [Console]::Write([char]27 + '[' + $h + ';1H'); ",
                    "& $cmd @args ",
                    "} else { Write-Error \"El comando 'agy' no se encuentra en PATH.\" } ",
                    "}; ",
                    "function claude { ",
                    "$cmd = (Get-Command -CommandType Application -Name claude.exe, claude -ErrorAction SilentlyContinue | Select-Object -First 1); ",
                    "if ($cmd) { ",
                    "Clear-Host; [Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H'); ",
                    "$h = $Host.UI.RawUI.WindowSize.Height; for ($i = 0; $i -lt $h; $i++) { [Console]::WriteLine('') }; [Console]::Write([char]27 + '[' + $h + ';1H'); ",
                    "& $cmd @args ",
                    "} else { Write-Error \"El comando 'claude' no se encuentra en PATH.\" } ",
                    "}; ",
                    "Set-Alias -Name claude-code -Value claude -Option AllScope -ErrorAction SilentlyContinue; ",
                    "function gemini { ",
                    "$cmd = (Get-Command -CommandType Application -Name gemini.exe, gemini -ErrorAction SilentlyContinue | Select-Object -First 1); ",
                    "if ($cmd) { ",
                    "Clear-Host; [Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H'); ",
                    "$h = $Host.UI.RawUI.WindowSize.Height; for ($i = 0; $i -lt $h; $i++) { [Console]::WriteLine('') }; [Console]::Write([char]27 + '[' + $h + ';1H'); ",
                    "& $cmd @args ",
                    "} else { Write-Error \"El comando 'gemini' no se encuentra en PATH.\" } ",
                    "}; ",
                    "function opencode { ",
                    "$cmd = (Get-Command -CommandType Application -Name opencode.exe, opencode -ErrorAction SilentlyContinue | Select-Object -First 1); ",
                    "if ($cmd) { ",
                    "Clear-Host; [Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H'); ",
                    "& $cmd @args ",
                    "} else { Write-Error \"El comando 'opencode' no se encuentra en PATH.\" } ",
                    "}; ",
                    "Write-Host ([char]27 + '[2J' + [char]27 + '[H') -NoNewline"
                );
                cmd.arg(init_script);
            }
        }

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                if shell != "powershell.exe" {
                    let mut fallback_cmd = CommandBuilder::new("powershell.exe");
                    fallback_cmd.env("TERM", "xterm-256color");
                    fallback_cmd.env("COLORTERM", "truecolor");
                    fallback_cmd.env("PATH", &path_env);
                    pair.slave.spawn_command(fallback_cmd)?
                } else {
                    return Err(e.into());
                }
            }
        };

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        let (from_pty_tx, from_pty_rx) = channel::<Vec<u8>>();
        let (to_pty_tx, to_pty_rx) = channel::<Vec<u8>>();

        // Hilo lector del PTY (lee la salida del shell y la envía al terminal)
        let proxy_clone = proxy;
        thread::Builder::new()
            .name("axon-pty-reader".into())
            .spawn(move || {
                let mut buffer = [0u8; 8192];
                while let Ok(n) = reader.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    if from_pty_tx.send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                    if let Some(p) = &proxy_clone {
                        let _ = p.send_event(AxonEvent::PtyData(id));
                    }
                }
                if let Some(p) = &proxy_clone {
                    let _ = p.send_event(AxonEvent::PtyClosed(id));
                }
            })?;

        // Hilo escritor del PTY (toma inputs del teclado y los manda al shell)
        thread::Builder::new()
            .name("axon-pty-writer".into())
            .spawn(move || {
                while let Ok(data) = to_pty_rx.recv() {
                    if writer.write_all(&data).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
            })?;

        Ok(Self {
            id,
            master: Arc::new(Mutex::new(pair.master)),
            child,
            to_pty_tx,
            from_pty_rx,
        })
    }

    pub fn is_child_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    pub fn write_input(&self, data: &[u8]) {
        let _ = self.to_pty_tx.send(data.to_vec());
    }

    pub fn try_recv_output(&self) -> Option<Vec<u8>> {
        self.from_pty_rx.try_recv().ok()
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_powershell_spawn_and_read() {
        let pty = PtyManager::new(0, "powershell.exe", false, 80, 24, None, None).unwrap();
        let mut total_output = String::new();
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                total_output.push_str(&s);
            }
        }
        println!("TOTAL PTY OUTPUT (len {}):\n{:?}", total_output.len(), total_output);

        // Enviar un comando "echo hello\r\n" a ver si responde
        pty.write_input(b"echo hello\r\n");
        let mut echo_output = String::new();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                echo_output.push_str(&s);
            }
            if echo_output.contains("hello") {
                break;
            }
        }
        println!("ECHO OUTPUT (len {}):\n{:?}", echo_output.len(), echo_output);
        assert!(echo_output.contains("hello"), "El shell debe responder al input del usuario");

        // Probar Backspace con 0x7f (debe borrar solo 1 carácter)
        pty.write_input(b"wordX\x7f\r\n");
        let mut bs_output = String::new();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            while let Some(bytes) = pty.try_recv_output() {
                let s = String::from_utf8_lossy(&bytes);
                bs_output.push_str(&s);
            }
            if bs_output.contains("word") {
                break;
            }
        }
        // Probar exactamente el texto de la captura del usuario
        pty.write_input(b"asldkflsdkflsdkflsadkf lasdkflsdkflasfksadlfk alsdkflsakdflasdkf laskdflksdlf");
        std::thread::sleep(std::time::Duration::from_millis(200));
        while let Some(_) = pty.try_recv_output() {}

        // Enviar 0x08 para borrar la última palabra
        pty.write_input(&[0x08]);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del1 = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del1.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("DEL 1 PTY OUTPUT: {:?}", del1);

        // Enviar 0x08 para borrar otra palabra
        pty.write_input(&[0x08]);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del2 = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del2.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("DEL 2 PTY OUTPUT: {:?}", del2);
    }

    #[test]
    fn test_powershell_and_harness_kill_word() {
        let pty = PtyManager::new(0, "powershell.exe", true, 120, 30, None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        while let Some(_) = pty.try_recv_output() {}

        // 1. Probar 0x17 (Ctrl+W)
        pty.write_input(b"echo first second");
        std::thread::sleep(std::time::Duration::from_millis(200));
        while let Some(_) = pty.try_recv_output() {}

        pty.write_input(&[0x17]); // Ctrl+W / BackwardKillWord
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del_ctrl_w = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del_ctrl_w.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("CTRL+W (0x17) PTY OUTPUT: {:?}", del_ctrl_w);

        // 2. Probar \x1b\x7f (Alt+Backspace)
        pty.write_input(b" third");
        std::thread::sleep(std::time::Duration::from_millis(200));
        while let Some(_) = pty.try_recv_output() {}

        pty.write_input(b"\x1b\x7f"); // Alt+Backspace
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut del_alt_bs = String::new();
        while let Some(bytes) = pty.try_recv_output() {
            del_alt_bs.push_str(&String::from_utf8_lossy(&bytes));
        }
        println!("ALT+BACKSPACE (\\x1b\\x7f) PTY OUTPUT: {:?}", del_alt_bs);
    }

    #[test]
    fn test_pty_powershell_exit_detection() {
        let mut pty = PtyManager::new(0, "powershell.exe", false, 80, 24, None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(!pty.is_child_exited(), "El proceso debería estar corriendo inicialmente");
        pty.write_input(b"exit\r\n");
        let mut exited = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if pty.is_child_exited() {
                exited = true;
                break;
            }
        }
        assert!(exited, "El proceso hijo debe detectarse como finalizado al recibir exit");
    }
}
