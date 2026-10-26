# Genera assets/themes.txt a partir de iterm2-color-schemes (carpeta schemes/, los .itermcolors originales de iTerm2) en un commit fijo.
# NO usa las carpetas generadas del repositorio (alacritty/, kitty/…): su generador aplica un ajuste automático de contraste WCAG que cambia colores (sobre todo el negro brillante).
# Una línea por tema, campos separados por tabulador y sin '#':
#   nombre  fondo  texto  cursor  selección-fondo  selección-texto  p0 .. p15
# Los campos opcionales que el tema no define van vacíos. Los temas en espacio P3 se pasan a sRGB igual que gen.py del repositorio.
#
# Uso, desde la raíz del repo:  pwsh tools/fetch-themes.ps1 [-Commit <sha>]
param([string]$Commit = '86a776d947b6f0c95d35005cb67f3cea435362fc')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tmp = Join-Path ([IO.Path]::GetTempPath()) "tearminal-themes-$([guid]::NewGuid().ToString('N'))"

function ToLinear([double]$c) { if ($c -lt 0.04045) { $c / 12.92 } else { [math]::Pow(($c + 0.055) / 1.055, 2.4) } }
function FromLinear([double]$c) { if ($c -lt 0.0031308) { 12.92 * $c } else { 1.055 * [math]::Pow($c, 1 / 2.4) - 0.055 } }
function Clamp01([double]$c) { [math]::Max(0.0, [math]::Min(1.0, $c)) }

# Componentes 0..1 de un <dict> de color de iTerm -> "rrggbb"
function ColorHex($dict) {
    $v = @{}
    $n = @($dict.SelectNodes('*'))
    for ($i = 0; $i -lt $n.Count; $i += 2) { $v[$n[$i].InnerText] = $n[$i + 1].InnerText }
    $c = 'Red Component', 'Green Component', 'Blue Component' | ForEach-Object { [double]::Parse($v[$_], [Globalization.CultureInfo]::InvariantCulture) }
    if ($v['Color Space'] -and $v['Color Space'].Trim() -eq 'P3') {
        $l = $c | ForEach-Object { ToLinear $_ }
        $m = @(@(1.22494018, -0.22469880, -0.00012973), @(-0.04205631, 1.04203282, -0.00000601), @(-0.01963755, -0.07862814, 1.09826365))
        $c = foreach ($row in $m) { FromLinear (Clamp01 ($row[0] * $l[0] + $row[1] * $l[1] + $row[2] * $l[2])) }
    }
    -join ($c | ForEach-Object { '{0:x2}' -f [int][math]::Round($_ * 255, [MidpointRounding]::ToEven) })
}

try {
    git clone --quiet --filter=blob:none --no-checkout --sparse https://github.com/mbadolato/iTerm2-Color-Schemes.git $tmp
    git -C $tmp sparse-checkout set schemes
    git -C $tmp checkout --quiet $Commit

    $lines = foreach ($f in Get-ChildItem (Join-Path $tmp 'schemes') -Filter *.itermcolors -File | Sort-Object Name | Where-Object { $_.BaseName -notlike 'Ghostty*' }) {
        $xml = New-Object Xml.XmlDocument
        $xml.XmlResolver = $null
        $xml.Load($f.FullName)
        $d = $xml.SelectSingleNode('/plist/dict')
        $c = @{}
        $n = @($d.SelectNodes('*'))
        for ($i = 0; $i -lt $n.Count; $i += 2) { $c[$n[$i].InnerText] = ColorHex $n[$i + 1] }
        $pal = 0..15 | ForEach-Object { $c["Ansi $_ Color"] }
        if ($pal -contains $null -or -not $c['Background Color'] -or -not $c['Foreground Color']) { throw "Tema incompleto: $($f.Name)" }
        (@($f.BaseName, $c['Background Color'], $c['Foreground Color'], $c['Cursor Color'], $c['Selection Color'], $c['Selected Text Color']) + $pal) -join "`t"
    }
    $out = Join-Path $root 'assets/themes.txt'
    [IO.File]::WriteAllText($out, (($lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding $false))
    Write-Host "$($lines.Count) temas de iterm2-color-schemes@$Commit -> assets/themes.txt"
} finally {
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
}
