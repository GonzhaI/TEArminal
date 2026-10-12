# scripts/package.ps1 — Empaqueta Axon en un archivo portable con todas sus herramientas CLI incluidas

$ErrorActionPreference = 'Stop'
$repoRoot = (Get-Item $PSScriptRoot).Parent.FullName

Write-Host "Compilando Axon en Release..."
Set-Location $repoRoot
cargo build --release

$distDir = Join-Path $repoRoot "dist\axon"
$binDir = Join-Path $distDir "bin"

if (Test-Path $distDir) {
    Remove-Item -Path $distDir -Recurse -Force
}
New-Item -ItemType Directory -Path $binDir -Force | Out-Null

Write-Host "Copiando binario y configuración de Axon..."
Copy-Item (Join-Path $repoRoot "target\release\axon.exe") -Destination $distDir
Copy-Item (Join-Path $repoRoot "axon.toml") -Destination $distDir

Write-Host "Aprovisionando herramientas a nivel de aplicación en dist\axon\bin..."
$localBin = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) "axon\bin"

$toolNames = @('eza.exe', 'bat.exe', 'rg.exe', 'fd.exe', 'zoxide.exe', 'fzf.exe')
foreach ($tool in $toolNames) {
    $src = Join-Path $localBin $tool
    if (Test-Path $src) {
        Copy-Item $src -Destination $binDir -Force
        Write-Host "  [OK] Empaquetado $tool"
    } else {
        Write-Warning "  [AVISO] No se encontró $tool en $localBin"
    }
}

$zipPath = Join-Path $repoRoot "dist\axon-portable-windows-x64.zip"
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }

Write-Host "Generando archivo ZIP portable: $zipPath..."
Compress-Archive -Path "$distDir\*" -DestinationPath $zipPath -Force

Write-Host "¡Paquete completado con éxito en $zipPath!"
