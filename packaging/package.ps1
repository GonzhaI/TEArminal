# Genera los entregables de TEArminal en dist\:
#   tearminal-<version>-windows-x64.zip   portable (tearminal.exe + bin\ con herramientas + licenses\)
#   tearminal-setup-<version>.exe         instalador por usuario (Inno Setup), si ISCC está disponible
#   SHA256SUMS.txt
#
# Uso:  pwsh packaging/package.ps1 [-Version 0.7.0] [-SkipBuild] [-SkipInstaller]
[CmdletBinding()]
param(
    [string]$Version,
    [switch]$SkipBuild,
    [switch]$SkipInstaller
)
$ErrorActionPreference = 'Stop'
$repo = (Get-Item $PSScriptRoot).Parent.FullName
Set-Location $repo

if (-not $Version) { $Version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value }
Write-Host "TEArminal $Version"

if (-not $SkipBuild) {
    Write-Host 'Compilando en Release...'
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo build falló' }
}

$dist = Join-Path $repo 'dist'
$stage = Join-Path $dist 'tearminal'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item (Join-Path $repo 'target\release\tearminal.exe') $stage
Copy-Item (Join-Path $repo 'README.md') $stage

# Herramientas opcionales (descarga verificada con SHA-256): bin\ y licenses\
& (Join-Path $PSScriptRoot 'fetch-tools.ps1') -Out $stage

# ZIP portable
$zip = Join-Path $dist "tearminal-$Version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
Write-Host "ZIP portable: $zip"

# Instalador (Inno Setup)
$setup = Join-Path $dist "tearminal-setup-$Version.exe"
if (-not $SkipInstaller) {
    $iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
    if (-not $iscc) {
        $iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") |
            Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if ($iscc) {
        if (Test-Path $setup) { Remove-Item $setup -Force }
        & $iscc "/DAppVersion=$Version" "/DStageDir=$stage" '/Qp' (Join-Path $repo 'packaging\installer\tearminal.iss')
        if ($LASTEXITCODE -ne 0) { throw 'ISCC falló' }
        Write-Host "Instalador: $setup"
    } else {
        Write-Warning 'Inno Setup (ISCC.exe) no encontrado: se omite el instalador. Instálalo desde https://jrsoftware.org/isdl.php'
    }
}

# Sumas de verificación
$sums = Get-ChildItem $dist -File | Where-Object { $_.Name -match '^tearminal-(setup-|.*-windows-x64\.zip)' -and $_.Name -match [regex]::Escape($Version) } |
    ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name }
[IO.File]::WriteAllText((Join-Path $dist 'SHA256SUMS.txt'), (($sums -join "`n") + "`n"), (New-Object Text.UTF8Encoding $false))
Write-Host "Listo:`n$($sums -join "`n")"
