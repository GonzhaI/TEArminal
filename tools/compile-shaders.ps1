# Compila src/renderer/gpu/shaders/quad.hlsl (Shader Model 4.0) con fxc.exe del Windows SDK.
# Los .cso y el SHA-256 del .hlsl se versionan: compilar el proyecto no exige el SDK.
# Uso: pwsh tools/compile-shaders.ps1
$ErrorActionPreference = 'Stop'
$dir = Join-Path $PSScriptRoot '..\src\renderer\gpu\shaders' | Resolve-Path
$hlsl = Join-Path $dir 'quad.hlsl'

$fxc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\fxc.exe" -ErrorAction Ignore |
    Sort-Object { [version](Split-Path (Split-Path (Split-Path $_.FullName)) -Leaf) } -Descending |
    Select-Object -First 1
if (-not $fxc) { throw 'No se encontró fxc.exe: instala el Windows SDK.' }

foreach ($s in @(@('vs_4_0', 'vs_main', 'quad_vs.cso'), @('ps_4_0', 'ps_main', 'quad_ps.cso'))) {
    & $fxc.FullName /nologo /O3 /Qstrip_reflect /Qstrip_debug /T $s[0] /E $s[1] /Fo (Join-Path $dir $s[2]) $hlsl
    if ($LASTEXITCODE -ne 0) { throw "fxc falló ($($s[0]))" }
}

# Hash con finales de línea normalizados a LF: el mismo cálculo que hace el test de Rust
$text = (Get-Content $hlsl -Raw) -replace "`r`n", "`n"
$sha = [System.Security.Cryptography.SHA256]::Create()
$hash = ($sha.ComputeHash([System.Text.UTF8Encoding]::new($false).GetBytes($text)) | ForEach-Object { $_.ToString('x2') }) -join ''
[System.IO.File]::WriteAllText((Join-Path $dir 'quad.hlsl.sha256'), "$hash`n", [System.Text.UTF8Encoding]::new($false))
Write-Host "OK: $($fxc.FullName)`n$hash"
