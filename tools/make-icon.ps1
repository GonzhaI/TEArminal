# Genera el icono de TEArminal: assets/icon.ico (multi-resolución, PNG embebidos), assets/icon-256.png
# (icono de ventana) y assets/logo.png (logo del README). Dibuja con GDI+ nativo a cada tamaño
# (vectorial, sin reescalar) para que 16/24/32 px queden nítidos.
#
# Logo: anillo con un prompt ">" y tres púas barridas hacia atrás, en blanco sobre negro.
#
# Uso, desde la raíz del repo:  pwsh tools/make-icon.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

function Get-Polar([double]$cx, [double]$cy, [double]$r, [double]$deg) {
    $a = $deg * [Math]::PI / 180
    [System.Drawing.PointF]::new([single]($cx + $r * [Math]::Cos($a)), [single]($cy + $r * [Math]::Sin($a)))
}

# Curva cuadrática (GDI+ solo tiene cúbicas): control c1 = p0 + 2/3 (q - p0), c2 = p2 + 2/3 (q - p2)
function Add-Quad($path, $p0, $q, $p2) {
    $c1 = [System.Drawing.PointF]::new([single]($p0.X + 2 / 3 * ($q.X - $p0.X)), [single]($p0.Y + 2 / 3 * ($q.Y - $p0.Y)))
    $c2 = [System.Drawing.PointF]::new([single]($p2.X + 2 / 3 * ($q.X - $p2.X)), [single]($p2.Y + 2 / 3 * ($q.Y - $p2.Y)))
    $path.AddBezier($p0, $c1, $c2, $p2)
}

function New-TearminalIcon([int]$S) {
    $bmp = New-Object System.Drawing.Bitmap $S, $S, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.Clear([System.Drawing.Color]::Transparent)

    # Base: cuadrado redondeado con degradado vertical (negro)
    $m = $S * 0.04; $w = $S - 2 * $m; $r = $S * 0.23
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($m, $m, 2 * $r, 2 * $r, 180, 90); $path.AddArc($m + $w - 2 * $r, $m, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($m + $w - 2 * $r, $m + $w - 2 * $r, 2 * $r, 2 * $r, 0, 90); $path.AddArc($m, $m + $w - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()
    $rect = New-Object System.Drawing.RectangleF $m, $m, $w, $w
    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, 34, 34, 34)), ([System.Drawing.Color]::FromArgb(255, 10, 10, 10)), 90
    $g.FillPath($bg, $path)
    if ($S -ge 32) {   # filo de luz sutil
        $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(50, 255, 255, 255)), ([Math]::Max(1, $S / 128))
        $g.DrawPath($pen, $path)
    }

    # Emblema en un espacio de diseño de ~256 unidades; trazos más gruesos en tamaños pequeños.
    $cx = 150.0; $cy = 128.0; $ringR = 66.0
    $small = $S -le 32
    $sw = if ($small) { 30.0 } else { 22.0 }       # grosor del anillo
    $halfW = if ($small) { 22.0 } else { 17.0 }    # semianchura de la base de cada púa
    $cw = if ($small) { 22.0 } else { 16.0 }       # grosor del ">"
    $white = [System.Drawing.Brushes]::White

    # Tres púas hacia atrás (izquierda), curvadas hacia abajo: ángulo de salida, largo
    $blades = New-Object System.Drawing.Drawing2D.GraphicsPath
    $tips = New-Object System.Collections.Generic.List[System.Drawing.PointF]
    # (PowerShell no distingue mayúsculas en variables: nada de $b/$B ni $r/$R.)
    foreach ($spec in @(@(-34, 84), @(0, 100), @(34, 84))) {
        $ang = 180 + $spec[0]
        $base = Get-Polar $cx $cy ($ringR + 4) $ang
        $tip = Get-Polar $cx $cy ($ringR + $spec[1]) ($ang + 14)
        $nx = [Math]::Cos(($ang + 90) * [Math]::PI / 180); $ny = [Math]::Sin(($ang + 90) * [Math]::PI / 180)
        $side1 = [System.Drawing.PointF]::new([single]($base.X + $nx * $halfW), [single]($base.Y + $ny * $halfW))
        $side2 = [System.Drawing.PointF]::new([single]($base.X - $nx * $halfW), [single]($base.Y - $ny * $halfW))
        $bend = 12
        $mid1 = [System.Drawing.PointF]::new([single](($side1.X + $tip.X) / 2 + $nx * $bend), [single](($side1.Y + $tip.Y) / 2 + $ny * $bend))
        $mid2 = [System.Drawing.PointF]::new([single](($side2.X + $tip.X) / 2 + $nx * $bend), [single](($side2.Y + $tip.Y) / 2 + $ny * $bend))
        $blades.StartFigure()
        Add-Quad $blades $side1 $mid1 $tip
        Add-Quad $blades $tip $mid2 $side2
        $blades.CloseFigure()
        $tips.Add($tip)
    }

    # Caja del emblema (anillo + puntas de las púas) para centrarlo y escalarlo dentro del cuadrado
    $x0 = ($tips | ForEach-Object { $_.X } | Measure-Object -Minimum).Minimum
    $y0 = [Math]::Min(($tips | ForEach-Object { $_.Y } | Measure-Object -Minimum).Minimum, $cy - $ringR - $sw / 2)
    $y1 = [Math]::Max(($tips | ForEach-Object { $_.Y } | Measure-Object -Maximum).Maximum, $cy + $ringR + $sw / 2)
    $x1 = $cx + $ringR + $sw / 2
    $fit = $S * $(if ($small) { 0.78 } else { 0.68 }) / [Math]::Max($x1 - $x0, $y1 - $y0)
    $g.TranslateTransform([single]($S / 2), [single]($S / 2))
    $g.ScaleTransform([single]$fit, [single]$fit)
    $g.TranslateTransform([single](-($x0 + $x1) / 2), [single](-($y0 + $y1) / 2))

    $g.FillPath($white, $blades)
    $ring = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), $sw
    $g.DrawEllipse($ring, [single]($cx - $ringR), [single]($cy - $ringR), [single](2 * $ringR), [single](2 * $ringR))
    $chev = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), $cw
    $chev.StartCap = 'Round'; $chev.EndCap = 'Round'; $chev.LineJoin = 'Round'
    $g.DrawLines($chev, [System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new([single]($cx - 18), [single]($cy - 24)),
        [System.Drawing.PointF]::new([single]($cx + 10), [single]$cy),
        [System.Drawing.PointF]::new([single]($cx - 18), [single]($cy + 24))))
    $g.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) { $ms = New-Object System.IO.MemoryStream; $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); return , $ms.ToArray() }

$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$frames = foreach ($s in $sizes) { , @($s, (Get-PngBytes (New-TearminalIcon $s))) }

# ICO con entradas PNG (Vista+)
$ms = New-Object System.IO.MemoryStream; $bw = New-Object System.IO.BinaryWriter $ms
$bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$frames.Count)
$offset = 6 + 16 * $frames.Count
foreach ($f in $frames) {
    $dim = if ($f[0] -ge 256) { 0 } else { $f[0] }
    $bw.Write([byte]$dim); $bw.Write([byte]$dim); $bw.Write([byte]0); $bw.Write([byte]0)
    $bw.Write([uint16]1); $bw.Write([uint16]32); $bw.Write([uint32]$f[1].Length); $bw.Write([uint32]$offset)
    $offset += $f[1].Length
}
foreach ($f in $frames) { $bw.Write([byte[]]$f[1]) }
[IO.File]::WriteAllBytes("$root\assets\icon.ico", $ms.ToArray())
foreach ($f in $frames) {
    if ($f[0] -eq 256) { [IO.File]::WriteAllBytes("$root\assets\icon-256.png", [byte[]]$f[1]) }
}
Copy-Item "$root\assets\icon-256.png" "$root\assets\logo.png" -Force
"icon.ico: {0} KB ({1} tamaños)" -f [int]((Get-Item "$root\assets\icon.ico").Length / 1KB), $frames.Count
