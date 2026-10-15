# Prueba de extremo a extremo del instalador: instala en silencio en una carpeta temporal, verifica
# archivos y herramientas, arranca Axon y desinstala.
#
# Uso:  pwsh packaging/test-installer.ps1 [-Setup dist\axon-setup-0.6.0.exe]
[CmdletBinding()]
param([string]$Setup)
$ErrorActionPreference = 'Stop'
$repo = (Get-Item $PSScriptRoot).Parent.FullName
if (-not $Setup) { $Setup = (Get-ChildItem (Join-Path $repo 'dist\axon-setup-*.exe') | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName }
if (-not (Test-Path $Setup)) { throw "No existe el instalador: $Setup" }

$dir = Join-Path $env:TEMP ("axon-install-test-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
$log = "$dir.log"
function Assert($cond, $msg) { if (-not $cond) { throw "FALLO: $msg" } else { Write-Host "  [ok] $msg" } }

try {
    Write-Host "Instalando $Setup en $dir"
    $p = Start-Process $Setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', "/DIR=$dir", "/LOG=$log" -Wait -PassThru
    Assert ($p.ExitCode -eq 0) "el instalador termina con código 0 (obtuvo $($p.ExitCode))"

    Assert (Test-Path "$dir\axon.exe") 'axon.exe instalado'
    Assert (-not (Test-Path "$dir\axon.toml")) 'no se instala axon.toml (no pisa la configuración del usuario)'
    Assert (Test-Path "$dir\licenses\THIRD-PARTY.txt") 'licencias de terceros incluidas'
    foreach ($t in 'eza', 'bat', 'rg', 'fd', 'zoxide', 'fzf') {
        $exe = "$dir\bin\$t.exe"
        Assert (Test-Path $exe) "$t.exe instalado"
        $out = @(& $exe --version 2>&1)   # salida completa: cortar la tubería con -First rompe $LASTEXITCODE
        Assert ($LASTEXITCODE -eq 0 -and $out.Count -gt 0) "$t.exe ejecuta ($($out[0]))"
    }
    Assert (Test-Path "$dir\bin\vcruntime140.dll") 'vcruntime140.dll junto a bat.exe (funciona sin Visual C++ Redistributable)'
    Assert (Test-Path "$dir\unins000.exe") 'desinstalador presente'

    Write-Host 'Arrancando Axon instalado...'
    $axon = Start-Process "$dir\axon.exe" -PassThru
    Start-Sleep -Seconds 5
    Assert (-not $axon.HasExited) 'Axon arranca y sigue en ejecución'
    Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $axon.Id } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $axon.Id -Force
    Start-Sleep -Seconds 1

    Write-Host 'Desinstalando...'
    $u = Start-Process "$dir\unins000.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
    Assert ($u.ExitCode -eq 0) 'el desinstalador termina con código 0'
    Start-Sleep -Seconds 2
    Assert (-not (Test-Path "$dir\axon.exe")) 'axon.exe eliminado tras desinstalar'
    Write-Host 'INSTALADOR OK'
}
finally {
    if (Test-Path "$dir\unins000.exe") { Start-Process "$dir\unins000.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES' -Wait }
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $log -Force -ErrorAction SilentlyContinue
}
