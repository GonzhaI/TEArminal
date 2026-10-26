# demo_tearminal.ps1 - Demostración de capacidades de TEArminal
# Prueba: Tipografía JetBrains Mono Nerd Font, Paleta ANSI, TrueColor e Iconos

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$esc = [char]27
$reset = "$esc[0m"
$bold = "$esc[1m"
$dim = "$esc[2m"
$italic = "$esc[3m"
$underline = "$esc[4m"
$invert = "$esc[7m"
$strike = "$esc[9m"

# Limpieza ANSI pura
Write-Host "$esc[2J$esc[H" -NoNewline

Write-Host ""
Write-Host "$bold$esc[32m  >_ TEArminal - Prueba de capacidades$reset"
Write-Host "$dim     Emulador nativo Windows 11 con renderizado propio$reset"
Write-Host ""

# 1. PRUEBA DE ICONOS NERD FONT (JetBrains Mono Nerd Font v3)
Write-Host "$bold$esc[34m[1] Iconos Nerd Font & Glifos PUA:$reset"
$branch = [char]0xe0a0
$github = [char]0xf113
$folder = [char]0xf07b
$file   = [char]0xf15b
$apple  = [char]0xf179
$win    = [char]0xe70f
$term   = [char]0xf489
Write-Host "    $esc[32m$branch main$reset   $esc[33m$github github$reset   $esc[34m$folder dev\tearminal$reset   $esc[31m$file Cargo.toml$reset   $esc[39m$apple macos$reset   $esc[94m$win windows$reset   $esc[32m$term tearminal$reset"
Write-Host ""

# 2. PRUEBA DE POWERLINE PROMPTS (Estilo Starship / Oh-My-Posh)
Write-Host "$bold$esc[34m[2] Segmentos Powerline & Prompts Modernos:$reset"
$arrow = [char]0xe0b0
$prompt_arrow = [char]0xf432
$p1 = "$esc[44;30m $folder tearminal $reset$esc[34;42m$arrow$reset"
$p2 = "$esc[42;30m $branch main $reset$esc[32;43m$arrow$reset"
$p3 = "$esc[43;30m $term v0.2.0 $reset$esc[33m$arrow$reset"
Write-Host "    $p1$p2$p3 $esc[32m$prompt_arrow$reset cargo build"
Write-Host ""

# 3. PALETA ANSI (16 colores)
Write-Host "$bold$esc[34m[3] Paleta de colores ANSI (16):$reset"
$ansi_names = @("Negro", "Rojo", "Verde", "Amarillo", "Azul", "Magenta", "Cian", "Blanco")
$row_normal = "    "
for ($i = 0; $i -lt 8; $i++) {
    $row_normal += "$esc[48;5;${i}m   $reset "
}
Write-Host "$row_normal  Normal (0-7)"

$row_bright = "    "
for ($i = 8; $i -lt 16; $i++) {
    $row_bright += "$esc[48;5;${i}m   $reset "
}
Write-Host "$row_bright  Bright (8-15)"
Write-Host ""

# 4. DEGRADADO 24-BIT TRUECOLOR (Full RGB Spectrum)
Write-Host "$bold$esc[34m[4] Gradiente 24-bit TrueColor:$reset"
$gradient = "    "
for ($i = 0; $i -lt 40; $i++) {
    $r = [int](244 * (1.0 - ($i / 40.0)) + 81 * ($i / 40.0))
    $g = [int](56 * (1.0 - ($i / 40.0)) + 226 * ($i / 40.0))
    $b = [int](65 * (1.0 - ($i / 40.0)) + 122 * ($i / 40.0))
    $gradient += "$esc[48;2;${r};${g};${b}m $reset"
}
for ($i = 0; $i -lt 40; $i++) {
    $r = [int](81 * (1.0 - ($i / 40.0)) + 78 * ($i / 40.0))
    $g = [int](226 * (1.0 - ($i / 40.0)) + 146 * ($i / 40.0))
    $b = [int](122 * (1.0 - ($i / 40.0)) + 249 * ($i / 40.0))
    $gradient += "$esc[48;2;${r};${g};${b}m $reset"
}
Write-Host $gradient
Write-Host ""

# 5. ESTILOS DE FORMATO SGR
Write-Host "$bold$esc[34m[5] Estilos y Atributos de Texto:$reset"
Write-Host "    $bold[Negrita]$reset   $dim[Atenuado]$reset   $italic[Cursiva]$reset   $underline[Subrayado]$reset   $strike[Tachado]$reset   $invert[Invertido]$reset"
Write-Host ""
