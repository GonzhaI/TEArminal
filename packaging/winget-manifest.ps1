# Genera los manifiestos de winget para una versión publicada, a partir de packaging\winget\ y de
# dist\SHA256SUMS.txt. Resultado: dist\winget\ (listo para un PR a microsoft/winget-pkgs).
#
# Uso:  pwsh packaging/winget-manifest.ps1 -Version 0.6.0
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Version)
$ErrorActionPreference = 'Stop'
$repo = (Get-Item $PSScriptRoot).Parent.FullName
$line = Get-Content (Join-Path $repo 'dist\SHA256SUMS.txt') | Where-Object { $_ -match "axon-setup-$([regex]::Escape($Version))\.exe$" } | Select-Object -First 1
if (-not $line) { throw "No hay axon-setup-$Version.exe en dist\SHA256SUMS.txt (ejecuta packaging\package.ps1)" }
$sha = ($line -split '\s+')[0].ToUpper()
$out = Join-Path $repo 'dist\winget'
New-Item -ItemType Directory -Force $out | Out-Null
foreach ($f in Get-ChildItem (Join-Path $repo 'packaging\winget') -Filter *.yaml) {
    $text = (Get-Content $f.FullName -Raw -Encoding UTF8).Replace('<VERSION>', $Version).Replace('<SHA256>', $sha)
    [IO.File]::WriteAllText((Join-Path $out $f.Name), $text, (New-Object Text.UTF8Encoding $false))
}
Write-Host "Manifiestos en $out (SHA-256 $sha)"
