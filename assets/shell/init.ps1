# Inicialización de PowerShell para Axon. Se embebe con include_str! en src/pty.rs y se ejecuta con
# `powershell -NoExit -Command <este script>`. Reglas de rendimiento (se ejecuta en CADA prompt / comando):
#   - Nada de procesos externos en `prompt` (un `git rev-parse` cuesta ~40 ms; leer .git/HEAD, ~2 ms).
#   - Nada de Get-Command para buscar programas (ver Find-AxonApp) y los resultados se memoizan.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
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
$script:AxonApps = @{}
$script:AxonExts = @($env:PATHEXT.Split(';') | Where-Object { $_ })
function Find-AxonApp([string]$Name) {
    if ($script:AxonApps.ContainsKey($Name)) { return $script:AxonApps[$Name] }
    foreach ($dir in $env:PATH.Split(';')) {
        if (-not $dir) { continue }
        foreach ($e in $script:AxonExts) {
            $f = $dir.TrimEnd('\') + '\' + $Name + $e
            if ([IO.File]::Exists($f)) { return ($script:AxonApps[$Name] = $f) }
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
$script:AxonLister = $null
$script:AxonIcons = @{
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
    if ($null -eq $script:AxonLister) {
        $eza = Find-AxonApp 'eza'
        $script:AxonLister = @{ Eza = $eza; Lsd = $(if ($eza) { $null } else { Find-AxonApp 'lsd' }) }
    }
    $show = $a -or $Force
    if ($script:AxonLister.Eza) {
        $ezaArgs = @('--icons', '--group-directories-first')
        if ($show) { $ezaArgs += '-a' }
        if ($Path -and $Path -ne '.') { $ezaArgs += $Path }
        & $script:AxonLister.Eza @ezaArgs; return
    }
    if ($script:AxonLister.Lsd) {
        $lsdArgs = @('--group-dirs', 'first')
        if ($show) { $lsdArgs += '-a' }
        if ($Path -and $Path -ne '.') { $lsdArgs += $Path }
        & $script:AxonLister.Lsd @lsdArgs; return
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
            $known = $script:AxonIcons[$item.Extension]
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
if (Find-AxonApp 'bat') { Remove-Item alias:cat -Force -ErrorAction Ignore; Set-Alias -Name cat -Value bat -Option AllScope -ErrorAction SilentlyContinue }
if (Find-AxonApp 'rg') { Set-Alias -Name grep -Value rg -Option AllScope -ErrorAction SilentlyContinue }
if (Find-AxonApp 'fd') { Set-Alias -Name find -Value fd -Option AllScope -ErrorAction SilentlyContinue }
if (Find-AxonApp 'zoxide') { Invoke-Expression (& zoxide init powershell --cmd cd | Out-String) -ErrorAction SilentlyContinue }

# Agentes de IA en CLI. Mode: Anchor = limpia y ancla el prompt a la última línea (entrada inferior);
# Clear = solo limpia; None = solo título. Subcomandos y flags no interactivos (update, --help, -v, -p…)
# pasan a None para conservar la pantalla; al salir no se llama a Clear-Host ni a ESC[r (lleva el cursor a 1;1).
function Invoke-AxonAgent([string]$Name, [string]$Mode, [object[]]$ArgumentList) {
    $cmd = Find-AxonApp $Name
    if (-not $cmd) { Write-Error "El comando '$Name' no se encuentra en PATH."; return }
    if ($Mode -ne 'None' -and $ArgumentList -and $ArgumentList.Count -gt 0) {
        $first = [string]$ArgumentList[0]
        $subcommands = @(
            'update', 'upgrade', 'uninstall', 'install', 'help', 'version', 'doctor', 'mcp',
            'plugin', 'plugins', 'auth', 'login', 'logout', 'config', 'models', 'agent', 'agents',
            'changelog', 'logs', 'stop', 'kill', 'rm', 'purge', 'respawn', 'setup-token',
            'auto-mode', 'gateway', 'import', 'ultrareview', 'remote-control', 'mic-serve',
            'extensions', 'extension', 'skills', 'skill', 'hooks', 'hook', 'gemma', 'acp', 'api',
            'debug', 'stats', 'run', 'session', 'service', 'reload', 'pair', 'serve'
        )
        if ($first -in $subcommands -or ($Name -eq 'agy' -and -not $first.StartsWith('-'))) {
            $Mode = 'None'
        } else {
            foreach ($a in $ArgumentList) {
                $s = [string]$a
                if ($s -in @('-h', '--help', '-?', '/?', '-v', '--version', '-V', '-p', '--print', '--prompt', '-l', '--list-extensions', '--list-sessions', '--delete-session', '--bg', '--background', '--desktop', '--completions') -or
                    $s.StartsWith('--print=') -or $s.StartsWith('--prompt=') -or $s.StartsWith('--delete-session=') -or $s.StartsWith('--completions=')) {
                    $Mode = 'None'
                    break
                }
            }
        }
    }
    $esc = [char]27; $bel = [char]7
    [Console]::Write($esc + ']0;' + $Name + $bel)
    if ($Mode -ne 'None') { Clear-Host; [Console]::Write($esc + '[2J' + $esc + '[3J' + $esc + '[H') }
    if ($Mode -eq 'Anchor') {
        $h = $Host.UI.RawUI.WindowSize.Height
        for ($i = 0; $i -lt $h; $i++) { [Console]::WriteLine('') }
        [Console]::Write($esc + '[' + $h + ';1H')
    }
    # OSC 1337 AxonAgent: si la consola muere con el agente (opencode lo hace al salir), Axon reabre el shell
    [Console]::Write($esc + ']1337;AxonAgent=start' + $bel)
    try { & $cmd @ArgumentList }
    finally {
        [Console]::Write($esc + ']1337;AxonAgent=end' + $bel + $esc + ']0;powershell' + $bel + $esc + '[0m' + $esc + '[?25h' + $esc + '[J')
    }
}
function agy      { Invoke-AxonAgent 'agy' 'Anchor' $args }
function claude   { Invoke-AxonAgent 'claude' 'Anchor' $args }
function gemini   { Invoke-AxonAgent 'gemini' 'Anchor' $args }
function opencode { Invoke-AxonAgent 'opencode' 'Clear' $args }
function aider    { Invoke-AxonAgent 'aider' 'None' $args }
Set-Alias -Name claude-code -Value claude -Option AllScope -ErrorAction SilentlyContinue

# ssh: señaliza inicio/fin de sesión a Axon (OSC 1337) para clonar paneles y detectar el host remoto
function ssh {
    $rawArgs = $args
    $dest = ''
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
            if ($arg -in $optWithArg -and ($i + 1 -lt $rawArgs.Count)) { $i++ }
            continue
        }
        if (-not $dest) { $dest = $arg }
        $posCount++
    }
    if (-not $dest) { $dest = 'remote' }
    # Integración (config terminal.ssh_integration): un bash remoto informa en cada prompt de la rama git,
    # la carpeta (OSC 7) y su listado (OSC 1337), y atiende peticiones de Axon (leer/guardar archivo, listar
    # carpeta) por la misma sesión, sin segunda conexión: funciona también con contraseña. Solo en sesiones
    # interactivas sin comando remoto. Texto plano, sin ofuscar (los antivirus desconfían de lo contrario).
    $integrate = ($env:AXON_SSH_INTEGRATION -eq '1') -and ($dest -ne 'remote') -and ($posCount -eq 1)
    foreach ($a in $rawArgs) { if ($a -in @('-N', '-T', '-W', '-O', '-f', '-V', '-G', '-Q', '-t', '-tt')) { $integrate = $false } }
    if ($integrate) {
        $hook = @'
__axq=${PWD//\%/%25};__axp=$(git symbolic-ref --short -q HEAD 2>/dev/null||git rev-parse --short HEAD 2>/dev/null);printf '\033]1337;AxonGit=%s\007\033]7;file://%s%s\007' ${__axp:--} ${HOSTNAME:-host} ${__axq// /%20};if [[ $__axd != $PWD ]];then __axd=$PWD;printf '\033]1337;AxonLs=%s\007' $(ls -1Ap 2>/dev/null|head -2000|od -An -v -tx1|tr -d \\040\\012);fi;if [[ -z $__axb ]];then __axb=1;__axon_req(){ local l=$READLINE_LINE p r id op a f c d st n t;p=${l%%#AXR#*};r=${l#*#AXR#};READLINE_LINE=$p;READLINE_POINT=${#p};id=${r%% *};r=${r#* };op=${r%% *};a=${r#* };st=err;d=;case $op in get)f=$(printf %b "${a//\%/\\x}");if [[ -f $f && -r $f && $(wc -c <"$f") -le 10485760 ]];then d=$(od -An -v -tx1 <"$f"|tr -d \\040\\012);st=ok;fi;;ls)f=$(printf %b "${a//\%/\\x}");if [[ -d $f ]];then d=$(ls -1Ap -- "$f" 2>/dev/null|head -2000|od -An -v -tx1|tr -d \\040\\012);st=ok;fi;;put)f=${a%% *};f=$(printf %b "${f//\%/\\x}");c=${a#* };if [[ -n $f ]]&&printf %b "${c//\%/\\x}" >"$f" 2>/dev/null;then d=$(cksum <"$f" 2>/dev/null|od -An -v -tx1|tr -d \\040\\012);st=ok;fi;;esac;t=$(( (${#d}+32767)/32768 ));[[ $t -lt 1 ]]&&t=1;for((n=0;n<t;n++));do printf '\033]1337;AxonFile=%s;%s;%s;%s;%s\007' $id $st $n $t ${d:$((n*32768)):32768};done;};bind -x '"\e[1337~":__axon_req';fi
'@
        $q = { param($t) "'" + $t.Replace("'", "'\''") + "'" }
        $script = 'case $SHELL in */bash) export PROMPT_COMMAND=' + (& $q $hook.Trim()) + ';; esac;cat /run/motd.dynamic /etc/motd 2>/dev/null;exec ${SHELL:-/bin/sh} -l'
        $remoteCmd = 'sh -c ' + (& $q $script)
        # Windows PowerShell 5.1 (y 7.0-7.2) no escapa las comillas dobles al llamar a un .exe
        if ($PSVersionTable.PSVersion -lt [version]'7.3') { $remoteCmd = $remoteCmd.Replace('"', '\"') }
        $rawArgs = @('-t') + $rawArgs + @($remoteCmd)
    }
    $esc = [char]27; $bel = [char]7
    if ($dest -ne 'remote') {
        [Console]::Write($esc + ']1337;AxonSSH=start;dest=' + $dest + ';token=' + $env:AXON_PANE_TOKEN + $bel)
        [Console]::Write($esc + ']0;ssh: ' + $dest + $bel)
    }
    $prevTerm = $env:TERM; $prevColor = $env:COLORTERM
    $env:TERM = 'xterm-256color'; $env:COLORTERM = 'truecolor'
    $sshCmd = Find-AxonApp 'ssh'
    if ($sshCmd) {
        try { & $sshCmd @rawArgs }
        finally {
            if ($dest -ne 'remote') {
                [Console]::Write($esc + ']1337;AxonSSH=end;token=' + $env:AXON_PANE_TOKEN + $bel)
                [Console]::Write($esc + ']0;powershell' + $bel)
            }
            if ($prevTerm) { $env:TERM = $prevTerm } else { Remove-Item env:TERM -ErrorAction SilentlyContinue }
            if ($prevColor) { $env:COLORTERM = $prevColor } else { Remove-Item env:COLORTERM -ErrorAction SilentlyContinue }
        }
    } else {
        Write-Error "El comando 'ssh' no se encuentra en PATH."
        if ($dest -ne 'remote') {
            [Console]::Write($esc + ']1337;AxonSSH=end;token=' + $env:AXON_PANE_TOKEN + $bel)
        }
    }
}
Write-Host ([char]27 + '[2J' + [char]27 + '[H') -NoNewline
