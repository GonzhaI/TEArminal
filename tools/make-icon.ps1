# Genera el icono de Axon: assets/icon.ico (multi-resolución, PNG embebidos), assets/icon-256.png
# (icono de ventana y logo del README) y assets/icon-64.png. Dibuja con GDI+ nativo a cada tamaño
# (vectorial, sin reescalar) para que 16/24/32 px queden nítidos.
#
# Uso, desde la raíz del repo:  pwsh tools/make-icon.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

function New-AxonIcon([int]$S) {
    $bmp = New-Object System.Drawing.Bitmap $S, $S, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.Clear([System.Drawing.Color]::Transparent)

    # Base: cuadrado redondeado con degradado vertical (azul noche, tono Catppuccin Frappé)
    $m = $S * 0.04; $w = $S - 2 * $m; $r = $S * 0.23
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($m, $m, 2 * $r, 2 * $r, 180, 90); $path.AddArc($m + $w - 2 * $r, $m, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($m + $w - 2 * $r, $m + $w - 2 * $r, 2 * $r, 2 * $r, 0, 90); $path.AddArc($m, $m + $w - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()
    $rect = New-Object System.Drawing.RectangleF $m, $m, $w, $w
    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, 52, 58, 82)), ([System.Drawing.Color]::FromArgb(255, 28, 31, 46)), 90
    $g.FillPath($bg, $path)
    if ($S -ge 32) {   # filo de luz sutil
        $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(60, 255, 255, 255)), ([Math]::Max(1, $S / 128))
        $g.DrawPath($pen, $path)
    }

    # Prompt ">_": chevron verde->azul + cursor azul. Trazos más gruesos en tamaños pequeños.
    $sw = $S * $(if ($S -le 24) { 0.115 } elseif ($S -le 48) { 0.10 } else { 0.092 })
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.PointF ($S * 0.20), ($S * 0.18)), (New-Object System.Drawing.PointF ($S * 0.60), ($S * 0.82)), ([System.Drawing.Color]::FromArgb(255, 81, 226, 122)), ([System.Drawing.Color]::FromArgb(255, 78, 146, 249))
    $chev = New-Object System.Drawing.Pen $grad, $sw
    $chev.StartCap = 'Round'; $chev.EndCap = 'Round'; $chev.LineJoin = 'Round'
    $g.DrawLines($chev, [System.Drawing.PointF[]]@(
        (New-Object System.Drawing.PointF ($S * 0.27), ($S * 0.27)),
        (New-Object System.Drawing.PointF ($S * 0.51), ($S * 0.50)),
        (New-Object System.Drawing.PointF ($S * 0.27), ($S * 0.73))))
    $cur = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 78, 146, 249)), $sw
    $cur.StartCap = 'Round'; $cur.EndCap = 'Round'
    $g.DrawLine($cur, ($S * 0.58), ($S * 0.73), ($S * 0.76), ($S * 0.73))
    $g.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) { $ms = New-Object System.IO.MemoryStream; $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); return , $ms.ToArray() }

$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$frames = foreach ($s in $sizes) { , @($s, (Get-PngBytes (New-AxonIcon $s))) }

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
    if ($f[0] -in 64, 256) { [IO.File]::WriteAllBytes("$root\assets\icon-$($f[0]).png", [byte[]]$f[1]) }
}
"icon.ico: {0} KB ({1} tamaños)" -f [int]((Get-Item "$root\assets\icon.ico").Length / 1KB), $frames.Count
