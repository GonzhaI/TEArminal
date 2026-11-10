# Inicialización de PowerShell para TEArminal. Se embebe con include_str! en src/pty.rs y se ejecuta con
# `powershell -NoExit -Command <este script>`. Reglas de rendimiento (se ejecuta en CADA prompt / comando):
#   - Nada de procesos externos en `prompt` (un `git rev-parse` cuesta ~40 ms; leer .git/HEAD, ~2 ms).
#   - Nada de Get-Command para buscar programas (ver Find-TearminalApp) y los resultados se memoizan.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
if ($env:TEARMINAL_THEME_FG -and $env:TEARMINAL_THEME_BG) {
    [Console]::Write([char]27 + ']10;' + $env:TEARMINAL_THEME_FG + [char]7)
    [Console]::Write([char]27 + ']11;' + $env:TEARMINAL_THEME_BG + [char]7)
}
Import-Module PSReadLine -ErrorAction SilentlyContinue
Set-PSReadLineOption -Colors @{
    Command   = ([char]27 + '[32m')
    Parameter = ([char]27 + '[34m')
    Operator  = ([char]27 + '[33m')
    String    = ([char]27 + '[92m')
    Number    = ([char]27 + '[35m')
    Member    = ([char]27 + '[36m')
    Error     = ([char]27 + '[31m')
} -ErrorAction SilentlyContinue
Set-PSReadLineKeyHandler -Chord 'Alt+Backspace' -Function BackwardKillWord -ErrorAction SilentlyContinue
Set-PSReadLineKeyHandler -Chord 'Alt+Enter' -Function AddLine -ErrorAction SilentlyContinue

# Busca un ejecutable en PATH respetando PATHEXT (claude.cmd de npm incluido). Get-Command tarda 250-500 ms
# cuando el programa NO está instalado (recorre cada entrada de PATH); File.Exists, ~5 ms. Solo memoiza aciertos.
$script:TearminalApps = @{}
$script:TearminalExts = @($env:PATHEXT.Split(';') | Where-Object { $_ })
function Find-TearminalApp([string]$Name) {
    if ($script:TearminalApps.ContainsKey($Name)) { return $script:TearminalApps[$Name] }
    foreach ($dir in $env:PATH.Split(';')) {
        if (-not $dir) { continue }
        foreach ($e in $script:TearminalExts) {
            $f = $dir.TrimEnd('\') + '\' + $Name + $e
            if ([IO.File]::Exists($f)) { return ($script:TearminalApps[$Name] = $f) }
        }
    }
}

# Prompt: OSC 7 (cwd) + OSC 133 A/B (marcas semánticas) + carpeta + rama git
function prompt {
    $esc = [char]27; $bel = [char]7
    $rawPath = (Get-Location).Path
    $p = $rawPath.Replace($HOME, '~')
    $osc7 = $esc + ']7;file://localhost/' + $rawPath.Replace('%', '%25').Replace('\', '/') + $bel
    $git = ''
    for ($d = $rawPath; $d; $d = [IO.Path]::GetDirectoryName($d)) {
        $dotGit = [IO.Path]::Combine($d, '.git')
        $head = $null
        if ([IO.File]::Exists($dotGit)) {            # worktree / submódulo: ".git" es un archivo "gitdir: ..."
            $t = [IO.File]::ReadAllText($dotGit).Trim()
            if ($t.StartsWith('gitdir: ')) { $head = [IO.Path]::Combine($d, $t.Substring(8), 'HEAD') }
        } else {
            $head = [IO.Path]::Combine($dotGit, 'HEAD')
        }
        if ($head -and [IO.File]::Exists($head)) {
            $l = [IO.File]::ReadAllText($head).Trim()
            $b = if ($l.StartsWith('ref: refs/heads/')) { $l.Substring(16) } else { $l.Substring(0, [Math]::Min(7, $l.Length)) }
            if ($b) { $git = ' ' + $esc + '[32m' + [char]0xe0a0 + ' ' + $b + $esc + '[0m' }
            break
        }
    }
    return $osc7 + $esc + ']133;A' + $bel + $esc + '[34m' + [char]0xf07b + ' ' + $p + $esc + '[0m' + $git +
        [Environment]::NewLine + $esc + '[32m' + [char]0xf432 + ' ' + $esc + '[0m' + $esc + ']133;B' + $bel
}

# ls: eza > lsd > listado propio con iconos Nerd Font (la búsqueda se hace una sola vez por sesión)
$script:TearminalLister = $null
$script:TearminalIcons = @{
    '.rs'   = @('31', 0xe7a8);   '.toml' = @('33', 0xe6b2);  '.yaml' = @('33', 0xe6b2)
    '.yml'  = @('33', 0xe6b2);  '.json' = @('33', 0xe60b);  '.md'   = @('34', 0xe73e)
    '.ps1'  = @('36', 0xe70f);  '.bat'  = @('36', 0xe70f);  '.cmd'  = @('36', 0xe70f)
    '.exe'  = @('32', 0xf489);  '.dll'  = @('35', 0xf013); '.png'  = @('35', 0xf1c5)
    '.jpg'  = @('35', 0xf1c5); '.svg'  = @('33', 0xf1c5);  '.zip'  = @('93', 0xf1c6)
    '.tar'  = @('93', 0xf1c6); '.gz'   = @('93', 0xf1c6); '.lock' = @('92', 0xf023)
    '.py'   = @('33', 0xe73c);  '.js'   = @('33', 0xe74e);  '.ts'   = @('34', 0xe628)
}
function ls {
    param([Parameter(Position = 0, ValueFromPipeline = $true)][string]$Path = '.', [switch]$a, [switch]$Force)
    if ($null -eq $script:TearminalLister) {
        $eza = Find-TearminalApp 'eza'
        $script:TearminalLister = @{ Eza = $eza; Lsd = $(if ($eza) { $null } else { Find-TearminalApp 'lsd' }) }
    }
    $show = $a -or $Force
    if ($script:TearminalLister.Eza) {
        $ezaArgs = @('--icons', '--group-directories-first')
        if ($show) { $ezaArgs += '-a' }
        if ($Path -and $Path -ne '.') { $ezaArgs += $Path }
        & $script:TearminalLister.Eza @ezaArgs; return
    }
    if ($script:TearminalLister.Lsd) {
        $lsdArgs = @('--group-dirs', 'first')
        if ($show) { $lsdArgs += '-a' }
        if ($Path -and $Path -ne '.') { $lsdArgs += $Path }
        & $script:TearminalLister.Lsd @lsdArgs; return
    }
    $esc = [char]27
    $items = if ($show) { Get-ChildItem -Path $Path -Force -ErrorAction SilentlyContinue } else { Get-ChildItem -Path $Path -ErrorAction SilentlyContinue }
    if (-not $items) { return }
    $items = @($items | Sort-Object { -not $_.PSIsContainer }, Name)
    $maxLen = 0; foreach ($i in $items) { if ($i.Name.Length -gt $maxLen) { $maxLen = $i.Name.Length } }
    $colWidth = [Math]::Max($maxLen + 2, 24)
    Write-Host ''
    foreach ($item in $items) {
        $padded = $item.Name.PadRight($colWidth)
        if ($item.PSIsContainer) {
            $icon = $esc + '[34m' + [char]0xf07b + $esc + '[0m'
            $name = $esc + '[1;34m' + $padded + $esc + '[0m'
            $size = $esc + '[2m' + '<DIR>'.PadLeft(9) + $esc + '[0m'
        } else {
            $known = $script:TearminalIcons[$item.Extension]
            if ($known) { $color = $known[0]; $glyph = $known[1] }
            else { $glyph = 0xf15b; $color = if ($item.Name.StartsWith('.')) { '90' } else { '39' } }
            $icon = $esc + '[' + $color + 'm' + [char]$glyph + $esc + '[0m'
            $name = $esc + '[39m' + $padded + $esc + '[0m'
            $len = $item.Length
            $str = if ($len -gt 1GB) { '{0:N1} GB' -f ($len / 1GB) } elseif ($len -gt 1MB) { '{0:N1} MB' -f ($len / 1MB) } elseif ($len -gt 1KB) { '{0:N1} KB' -f ($len / 1KB) } else { '{0} B' -f $len }
            $size = $esc + '[2m' + ($str.PadLeft(9)) + $esc + '[0m'
        }
        $date = $esc + '[2m' + $item.LastWriteTime.ToString('yyyy-MM-dd HH:mm') + $esc + '[0m'
        Write-Host ('  {0}  {1}  {2}  {3}' -f $icon, $name, $size, $date)
    }
    Write-Host ''
}
Remove-Item alias:ls -Force -ErrorAction Ignore
Remove-Item alias:dir -Force -ErrorAction Ignore
Set-Alias -Name dir -Value ls -Option AllScope -ErrorAction SilentlyContinue

# Herramientas modernas opcionales (una sola búsqueda por herramienta al arrancar)
if (Find-TearminalApp 'bat') { Remove-Item alias:cat -Force -ErrorAction Ignore; Set-Alias -Name cat -Value bat -Option AllScope -ErrorAction SilentlyContinue }
if (Find-TearminalApp 'rg') { Set-Alias -Name grep -Value rg -Option AllScope -ErrorAction SilentlyContinue }
if (Find-TearminalApp 'fd') { Set-Alias -Name find -Value fd -Option AllScope -ErrorAction SilentlyContinue }
if (Find-TearminalApp 'zoxide') { Invoke-Expression (& zoxide init powershell --cmd cd | Out-String) -ErrorAction SilentlyContinue }

# Agentes de IA en CLI. Señaliza inicio/fin (OSC 1337 TearminalAgent) para TEArminal
# y restaura el estado del terminal al salir, permitiendo que el agente siga su flujo normal
# sin limpiar la pantalla ni alterar la posición del cursor.
function Invoke-TearminalAgent([string]$Name, [object[]]$ArgumentList) {
    $cmd = Find-TearminalApp $Name
    if (-not $cmd) { Write-Error "El comando '$Name' no se encuentra en PATH."; return }
    $esc = [char]27; $bel = [char]7
    [Console]::Write($esc + ']0;' + $Name + $bel)
    # OSC 1337 TearminalAgent: si la consola muere con el agente (opencode lo hace al salir), TEArminal reabre el shell
    [Console]::Write($esc + ']1337;TearminalAgent=start;name=' + $Name + $bel)
    try { & $cmd @ArgumentList }
    finally {
        [Console]::Write($esc + ']1337;TearminalAgent=end' + $bel + $esc + ']0;powershell' + $bel + $esc + '[0m' + $esc + '[?25h')
    }
}
function agy      { Invoke-TearminalAgent 'agy' $args }
function claude   { Invoke-TearminalAgent 'claude' $args }
function gemini   { Invoke-TearminalAgent 'gemini' $args }
function opencode { Invoke-TearminalAgent 'opencode' $args }
function aider    { Invoke-TearminalAgent 'aider' $args }
Set-Alias -Name claude-code -Value claude -Option AllScope -ErrorAction SilentlyContinue

# ssh: señaliza inicio/fin de sesión a TEArminal (OSC 1337) para clonar paneles y detectar el host remoto
function ssh {
    $rawArgs = $args
    $dest = ''
    $loginUser = ''
    $identity = ''
    $port = ''
    $posCount = 0
    $optWithArg = @('-p', '-i', '-l', '-o', '-c', '-b', '-B', '-J', '-F', '-D', '-E', '-I', '-R', '-L', '-m', '-O', '-Q', '-S', '-w', '-W')
    for ($i = 0; $i -lt $rawArgs.Count; $i++) {
        $arg = [string]$rawArgs[$i]
        if ($arg -eq '--') {
            $rem = $rawArgs.Count - ($i + 1)
            if ($rem -ge 1 -and -not $dest) { $dest = [string]$rawArgs[$i + 1] }
            $posCount += [Math]::Max(0, $rem)
            break
        }
        if ($arg.StartsWith('-')) {
            if ($arg -eq '-i' -and ($i + 1 -lt $rawArgs.Count)) {
                $i++
                $identity = [string]$rawArgs[$i]
                continue
            }
            if ($arg -eq '-p' -and ($i + 1 -lt $rawArgs.Count)) {
                $i++
                $port = [string]$rawArgs[$i]
                continue
            }
            if ($arg -eq '-l' -and ($i + 1 -lt $rawArgs.Count)) {
                $i++
                $loginUser = [string]$rawArgs[$i]
                continue
            }
            if ($arg -in $optWithArg -and ($i + 1 -lt $rawArgs.Count)) { $i++ }
            continue
        }
        if (-not $dest) { $dest = $arg }
        $posCount++
    }
    if (-not $dest) { $dest = 'remote' } elseif ($loginUser -and -not $dest.Contains('@')) { $dest = "$loginUser@$dest" }
    if ($identity) {
        try {
            if (Test-Path -Path $identity) {
                $identity = (Resolve-Path -Path $identity).ProviderPath
            } elseif (Test-Path -LiteralPath $identity) {
                $identity = (Resolve-Path -LiteralPath $identity).ProviderPath
            }
        } catch { }
        # Con llaves .pem explícitas, forzar IdentitiesOnly=yes evita que OpenSSH pruebe primero
        # otras llaves de ssh-agent o ~/.ssh/ que agotan MaxAuthTries y provocan baneo (fail2ban)
        $hasIdentities = $false
        foreach ($a in $rawArgs) {
            if ([string]$a -like '*IdentitiesOnly*') { $hasIdentities = $true; break }
        }
        if (-not $hasIdentities) {
            $rawArgs = @('-o', 'IdentitiesOnly=yes') + $rawArgs
        }
    }
    # Mantener viva la sesión SSH: si el usuario no especificó ServerAliveInterval, enviar
    # paquetes keep-alive cada 30 s para evitar desconexiones por inactividad de routers/firewalls (NAT drop)
    $hasAlive = $false
    foreach ($a in $rawArgs) {
        if ([string]$a -like '*ServerAliveInterval*') { $hasAlive = $true; break }
    }
    if (-not $hasAlive -and $dest -ne 'remote') {
        $rawArgs = @('-o', 'ServerAliveInterval=30', '-o', 'ServerAliveCountMax=3') + $rawArgs
    }
    # Integración (config terminal.ssh_integration): un bash remoto informa en cada prompt de la rama git,
    # la carpeta (OSC 7) y su listado (OSC 1337), y atiende peticiones de TEArminal (leer/guardar archivo, listar
    # carpeta) por la misma sesión, sin segunda conexión: funciona también con contraseña. Solo en sesiones
    # interactivas sin comando remoto. Texto plano, sin ofuscar (los antivirus desconfían de lo contrario).
    $integrate = ($env:TEARMINAL_SSH_INTEGRATION -eq '1') -and ($dest -ne 'remote') -and ($posCount -eq 1)
    foreach ($a in $rawArgs) { if ($a -cin @('-N', '-T', '-W', '-O', '-f', '-V', '-G', '-Q', '-t', '-tt')) { $integrate = $false } }
    if ($integrate) {
        $hook = @'
__tearminal_req(){ local o i a f c d;IFS=" " read -r o i a;f=$(shopt -u patsub_replacement 2>/dev/null;printf %b "${a//\%/\\x}");case $f in "~")f=$HOME;;"~/"*)f="$HOME/${f#*/}";;esac;case $o in get)if [ ! -e "$f" ];then printf '\033]1337;TearminalFile=%s;status=enoent\007' "$i";elif [ -d "$f" ];then printf '\033]1337;TearminalFile=%s;status=eisdir\007' "$i";elif [ ! -r "$f" ];then printf '\033]1337;TearminalFile=%s;status=eacces\007' "$i";else while IFS= read -r -n 32768 d||[ -n "$d" ];do [ -n "$d" ]&&printf '\033]1337;TearminalFile=%s;part=%s\007' "$i" "$d";done < <(head -c 10485761 -- "$f" 2>/dev/null|base64|tr -d '\r\n');printf '\033]1337;TearminalFile=%s;status=ok\007' "$i";fi;;ls)if [ -d "$f" ]&&[ -r "$f" ];then while IFS= read -r -n 32768 d||[ -n "$d" ];do [ -n "$d" ]&&printf '\033]1337;TearminalFile=%s;part=%s\007' "$i" "$d";done < <(ls -1Ap -- "$f" 2>/dev/null|head -n 5000|base64 2>/dev/null|tr -d '\r\n');printf '\033]1337;TearminalFile=%s;status=ok\007' "$i";else printf '\033]1337;TearminalFile=%s;status=err\007' "$i";fi;;put)IFS=" " read -r f a <<<"$a";f=$(shopt -u patsub_replacement 2>/dev/null;printf %b "${f//\%/\\x}");case $f in "~")f=$HOME;;"~/"*)f="$HOME/${f#*/}";;esac;c="$f.tearminal.$i.tmp";while IFS= read -r d;do [ "$d" = . ]&&break;printf %s "$d";done 2>/dev/null|base64 -d >"$c" 2>/dev/null;if [ -f "$c" ]&&[ "$(wc -c <"$c" 2>/dev/null)" -eq "$a" ] 2>/dev/null&&mv -f -- "$c" "$f" 2>/dev/null;then printf '\033]1337;TearminalFile=%s;status=ok\007' "$i";else rm -f -- "$c" 2>/dev/null;printf '\033]1337;TearminalFile=%s;status=err\007' "$i";fi;;esac;};bind -x '"\e[1337~":__tearminal_req' 2>/dev/null;__axq=${PWD//\%/%25};__axp=$(git symbolic-ref --short -q HEAD 2>/dev/null||git rev-parse --short HEAD 2>/dev/null);if [ -r . ];then __axl=$(ls -1Ap 2>/dev/null|head -n 500|base64 2>/dev/null|tr -d '\r\n');__axl=${__axl:-EMPTY};else __axl="-";fi;if git rev-parse --is-inside-work-tree >/dev/null 2>&1;then __axs=$(git --no-optional-locks status --porcelain -z 2>/dev/null|head -c 65536|base64 2>/dev/null|tr -d '\r\n');__axs=${__axs:-EMPTY};else __axs="-";fi;printf '\033]1337;TearminalGit=%s\007\033]7;file://%s%s\007\033]1337;TearminalLs=%s\007\033]1337;TearminalGitStatus=%s\007' "${__axp:--}" "${HOSTNAME:-host}" "${__axq// /%20}" "$__axl" "$__axs";
'@
        $q = { param($t) "'" + $t.Replace("'", "'\''") + "'" }
        $script = 'case $SHELL in */bash) export PROMPT_COMMAND=' + (& $q $hook.Trim()) + ';; */zsh) export PROMPT_COMMAND=' + (& $q $hook.Trim()) + '; precmd() { eval "$PROMPT_COMMAND"; };; esac;cat /run/motd.dynamic /etc/motd 2>/dev/null;case $SHELL in */bash|*/zsh) exec "$SHELL" -l;; *) exec ${SHELL:-/bin/sh};; esac'
        $remoteCmd = 'sh -c ' + (& $q $script)
        # Windows PowerShell 5.1 (y 7.0-7.2) no escapa las comillas dobles al llamar a un .exe
        if ($PSVersionTable.PSVersion -lt [version]'7.3') { $remoteCmd = $remoteCmd.Replace('"', '\"') }
        $rawArgs = @('-t') + $rawArgs + @($remoteCmd)
    }
    $esc = [char]27; $bel = [char]7
    if ($dest -ne 'remote') {
        $startMsg = $esc + ']1337;TearminalSSH=start;dest=' + $dest
        if ($identity) { $startMsg += ';ident=' + $identity }
        if ($port) { $startMsg += ';port=' + $port }
        if ($integrate) { $startMsg += ';inband=1' }
        $startMsg += ';token=' + $env:TEARMINAL_PANE_TOKEN + $bel
        [Console]::Write($startMsg)
        [Console]::Write($esc + ']0;ssh: ' + $dest + $bel)
    }
    $prevTerm = $env:TERM; $prevColor = $env:COLORTERM
    $env:TERM = 'xterm-256color'; $env:COLORTERM = 'truecolor'
    $sshCmd = Find-TearminalApp 'ssh'
    if ($sshCmd) {
        try { & $sshCmd @rawArgs }
        finally {
            if ($dest -ne 'remote') {
                [Console]::Write($esc + ']1337;TearminalSSH=end;token=' + $env:TEARMINAL_PANE_TOKEN + $bel)
                [Console]::Write($esc + ']0;powershell' + $bel)
            }
            if ($prevTerm) { $env:TERM = $prevTerm } else { Remove-Item env:TERM -ErrorAction SilentlyContinue }
            if ($prevColor) { $env:COLORTERM = $prevColor } else { Remove-Item env:COLORTERM -ErrorAction SilentlyContinue }
        }
    } else {
        Write-Error "El comando 'ssh' no se encuentra en PATH."
        if ($dest -ne 'remote') {
            [Console]::Write($esc + ']1337;TearminalSSH=end;token=' + $env:TEARMINAL_PANE_TOKEN + $bel)
        }
    }
}
function Clear-Host {
    [Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H')
}

[Console]::Write([char]27 + '[2J' + [char]27 + '[3J' + [char]27 + '[H')
