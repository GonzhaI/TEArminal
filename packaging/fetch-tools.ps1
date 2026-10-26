# Descarga las herramientas CLI opcionales (packaging/tools.json) verificando su SHA-256 y las deja en
#   <Out>\bin\            ejecutables (más las DLL "app-local" que necesiten, p. ej. vcruntime140.dll de bat)
#   <Out>\licenses\       licencias de cada herramienta + THIRD-PARTY.txt (obligatorio al redistribuir)
#
# Uso:
#   pwsh packaging/fetch-tools.ps1 -Out dist\tearminal          # descarga y verifica
#   pwsh packaging/fetch-tools.ps1 -Update                 # tras cambiar versiones/URLs: recalcula los hashes
#
# Funciona en Windows PowerShell 5.1 y en PowerShell 7. Nada se ejecuta: solo se descargan y extraen archivos.
[CmdletBinding()]
param(
    [string]$Out,
    [string]$Cache = (Join-Path $env:TEMP 'tearminal-tools-cache'),
    [switch]$Update
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest es mucho más rápido sin barra de progreso
Add-Type -AssemblyName System.IO.Compression.FileSystem

$manifestPath = Join-Path $PSScriptRoot 'tools.json'
$manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $Update -and -not $Out) { throw 'Indica -Out <carpeta> (o -Update para recalcular hashes).' }
New-Item -ItemType Directory -Force $Cache | Out-Null

function Get-Sha256([string]$Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

# Descarga (con caché) y devuelve [ruta, sha256 real]. Con hash esperado, un desajuste aborta.
function Get-Download([string]$Url, [string]$Expected) {
    $key = [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($Url))).Replace('-', '').Substring(0, 12).ToLower()
    $file = Join-Path $Cache ("$key-" + [IO.Path]::GetFileName($Url))
    if ($Update -or -not (Test-Path $file) -or ($Expected -and (Get-Sha256 $file) -ne $Expected)) {
        for ($try = 1; $try -le 3; $try++) {
            try { Invoke-WebRequest -Uri $Url -OutFile $file -UseBasicParsing; break }
            catch { if ($try -eq 3) { throw "No se pudo descargar $Url : $($_.Exception.Message)" }; Start-Sleep -Seconds (2 * $try) }
        }
    }
    $actual = Get-Sha256 $file
    if (-not $Update -and $Expected -and $actual -ne $Expected) {
        Remove-Item $file -Force
        throw "SHA-256 distinto para $Url`n  esperado: $Expected`n  obtenido: $actual"
    }
    if (-not $Update -and -not $Expected) { throw "Falta el SHA-256 de $Url en packaging/tools.json (ejecuta -Update)" }
    return $file, $actual
}

# Copia una entrada del zip (búsqueda por nombre de archivo, sin importar la subcarpeta)
function Expand-ZipEntry([string]$Zip, [string]$Leaf, [string]$Dest) {
    $z = [IO.Compression.ZipFile]::OpenRead($Zip)
    try {
        $e = $z.Entries | Where-Object { $_.Name -ieq $Leaf } | Select-Object -First 1
        if (-not $e) { throw "$Leaf no está en $Zip" }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($e, $Dest, $true)
    } finally { $z.Dispose() }
}

# vcruntime140.dll del Redist de Visual Studio (distribuible junto a la aplicación según REDIST.TXT)
function Find-VcRuntimeDll([string]$Name) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        # Cualquier instalación puede ser la buena (p. ej. SSMS lleva el shell de VS pero no el Redist de C++)
        foreach ($vs in (& $vswhere -all -products * -property installationPath)) {
            $dll = Get-ChildItem (Join-Path $vs ('VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT\' + $Name)) -ErrorAction SilentlyContinue |
                Sort-Object FullName -Descending | Select-Object -First 1
            if ($dll) { return $dll.FullName }
        }
    }
    throw "No se encontró $Name en el Redist de Visual Studio. Instala 'Build Tools para Visual Studio' (carga de trabajo C++)."
}

if ($Out) {
    $bin = Join-Path $Out 'bin'; $lic = Join-Path $Out 'licenses'
    New-Item -ItemType Directory -Force $bin, $lic | Out-Null
}
$thirdParty = New-Object System.Collections.Generic.List[string]
$thirdParty.Add("Herramientas de terceros incluidas con TEArminal (cada una conserva su licencia, en esta carpeta):`r`n")

foreach ($t in $manifest.tools) {
    $zip, $sha = Get-Download $t.url $t.sha256
    if ($Update) {
        $t.sha256 = $sha
        if ($t.licenseUrl) { $t.licenseSha256 = (Get-Download $t.licenseUrl $null)[1] }
        Write-Host ("[hash] {0,-8} {1}" -f $t.name, $sha)
        continue
    }
    Expand-ZipEntry $zip $t.exe (Join-Path $bin $t.exe)
    $dir = Join-Path $lic $t.name; New-Item -ItemType Directory -Force $dir | Out-Null
    foreach ($l in $t.licenseFiles) { Expand-ZipEntry $zip $l (Join-Path $dir $l) }
    if ($t.licenseUrl) {
        $f, $null = Get-Download $t.licenseUrl $t.licenseSha256
        Copy-Item $f (Join-Path $dir 'LICENSE') -Force
    }
    foreach ($dll in @($t.appLocalDlls)) { if ($dll) { Copy-Item (Find-VcRuntimeDll $dll) (Join-Path $bin $dll) -Force } }
    $thirdParty.Add(("{0} {1} - {2} - {3}" -f $t.name, $t.version, $t.license, $t.homepage))
    Write-Host ("[ok]   {0,-8} {1}" -f $t.name, $t.version)
}

if ($Update) {
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding $false))
    Write-Host 'packaging/tools.json actualizado.'
} else {
    $thirdParty.Add("`r`nvcruntime140.dll (junto a bat.exe): Microsoft Visual C++ Redistributable, distribuible según REDIST.TXT de Visual Studio.")
    # UTF-8 sin BOM explícito: Set-Content -Encoding UTF8 escribe BOM en 5.1 y no en 7 (salida no determinista)
    [IO.File]::WriteAllText((Join-Path $lic 'THIRD-PARTY.txt'), ($thirdParty -join "`r`n"), (New-Object Text.UTF8Encoding $false))
}
