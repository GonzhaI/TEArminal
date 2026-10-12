# demo_herdr_tui.ps1 - Simulación Interactiva de Herdr TUI en Axon Terminal
# Valida: Alternate Screen Buffer, SGR 1006 Mouse Tracking, TrueColor y Paneles Multi-Agente

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$esc = [char]27
$reset = "$esc[0m"
$bold = "$esc[1m"
$dim = "$esc[2m"

# Colores Ghostty
$bg_panel = "$esc[48;2;28;28;32m"
$fg_accent = "$esc[38;2;81;226;122m"      # Verde Ghostty
$fg_blue = "$esc[38;2;78;146;249m"        # Azul Ghostty
$fg_amber = "$esc[38;2;250;189;47m"       # Ámbar Ghostty
$fg_red = "$esc[38;2;244;56;65m"          # Rojo Ghostty
$fg_text = "$esc[38;2;230;230;235m"       # Blanco hueso Ghostty
$fg_dim = "$esc[38;2;120;125;140m"

function Draw-Screen {
    param([string]$selectedAgent = "Coder", [string]$lastEvent = "Listo. Haz clic en cualquier panel con el ratón.")

    # Mover al inicio sin parpadear
    Write-Host "$esc[H" -NoNewline

    # Encabezado
    Write-Host "$bold$fg_blue  ╭──────────────────────────────────────────────────────────────────────────────────╮$reset"
    Write-Host "$bold$fg_blue  │$reset$bold$fg_accent 󱚣 HERDR v0.4.0 — Multi-Agent AI Orchestrator $reset$fg_dim(ConPTY + SGR 1006 Mouse Tracking)$bold$fg_blue│$reset"
    Write-Host "$bold$fg_blue  ╰──────────────────────────────────────────────────────────────────────────────────╯$reset"
    Write-Host ""

    # Paneles de Agentes
    $agents = @(
        @{ Name = "Planner";  Role = "Arquitecto / Planificador"; State = "ACTIVO"; Color = $fg_blue;  Icon = "󰒋" },
        @{ Name = "Coder";    Role = "Desarrollador Rust (Axon)";  State = "RUNNING";Color = $fg_accent;Icon = "󰅩" },
        @{ Name = "Reviewer"; Role = "Auditor de Código & QA";     State = "IDLE";   Color = $fg_amber; Icon = "󰄲" }
    )

    foreach ($ag in $agents) {
        $isSelected = ($ag.Name -eq $selectedAgent)
        $borderCol = if ($isSelected) { $fg_accent } else { $fg_dim }
        $mark = if ($isSelected) { "$bold$fg_accent[SELECCIONADO]$reset" } else { "              " }

        Write-Host "  $borderCol┌─────────────────────────────────────────────────────────────┐$reset"
        Write-Host "  $borderCol│$reset $($ag.Color)$($ag.Icon) Agente: $($ag.Name.PadRight(12))$reset $mark  Estado: $($ag.Color)$($ag.State.PadRight(8))$reset $borderCol│$reset"
        Write-Host "  $borderCol│$reset   $fg_dim Rol:$reset $($ag.Role.PadRight(45)) $borderCol│$reset"
        Write-Host "  $borderCol└─────────────────────────────────────────────────────────────┘$reset"
    }

    Write-Host ""
    Write-Host "$bold$fg_text  Eventos del Ratón y Sincronía TUI:$reset"
    Write-Host "  $fg_dim╰─>$reset $fg_accent$lastEvent$reset"
    Write-Host ""
    Write-Host "  $bold$fg_red[ [Q] Salir del TUI / Haz clic aquí ]$reset    $fg_dim(Ratón interactivo activo: clics y scroll)$reset"
}

try {
    # 1. Conmutar a Alternate Screen Buffer, ocultar cursor y activar Mouse Tracking SGR 1006
    Write-Host "$esc[?1049h" -NoNewline
    Write-Host "$esc[?25l" -NoNewline
    Write-Host "$esc[?1000h$esc[?1002h$esc[?1006h" -NoNewline
    Write-Host "$esc[2J$esc[H" -NoNewline

    $currentAgent = "Coder"
    $lastEventInfo = "Haz clic en Planner, Coder o Reviewer con el ratón..."
    Draw-Screen -selectedAgent $currentAgent -lastEvent $lastEventInfo

    $running = $true
    $buf = ""

    while ($running) {
        if ([Console]::KeyAvailable) {
            $key = [Console]::ReadKey($true)

            if ($key.Key -eq [ConsoleKey]::Q -or $key.Key -eq [ConsoleKey]::Escape) {
                $running = $false
                break
            }

            # Procesar secuencias ANSI de ratón SGR: \x1b[<BTN;COL;ROWM o m
            if ($key.KeyChar -eq [char]27) {
                Start-Sleep -Milliseconds 25
                $seq = ""
                while ([Console]::KeyAvailable) {
                    $seq += [Console]::ReadKey($true).KeyChar
                }

                if ($seq -match '^\[<(\d+);(\d+);(\d+)([Mm])') {
                    $btn = [int]$Matches[1]
                    $col = [int]$Matches[2]
                    $row = [int]$Matches[3]
                    $action = if ($Matches[4] -eq 'M') { "Presionado" } else { "Soltado" }

                    $btnName = switch ($btn) {
                        0 { "Clic Izquierdo" }
                        1 { "Clic Central" }
                        2 { "Clic Derecho" }
                        64 { "Rueda Arriba" }
                        65 { "Rueda Abajo" }
                        default { "Botón $btn" }
                    }

                    # Detectar selección por coordenadas
                    if ($btn -eq 0 -and $Matches[4] -eq 'M') {
                        if ($row -ge 5 -and $row -le 7) {
                            $currentAgent = "Planner"
                        } elseif ($row -ge 8 -and $row -le 10) {
                            $currentAgent = "Coder"
                        } elseif ($row -ge 11 -and $row -le 13) {
                            $currentAgent = "Reviewer"
                        } elseif ($row -ge 17) {
                            $running = $false
                            break
                        }
                    }

                    $lastEventInfo = "$btnName en Col:$col, Fila:$row ($action)"
                    Draw-Screen -selectedAgent $currentAgent -lastEvent $lastEventInfo
                }
            }
        }
        Start-Sleep -Milliseconds 30
    }
}
finally {
    # 2. Restauración impecable: Desactivar mouse tracking, mostrar cursor y salir de Alt Screen
    Write-Host "$esc[?1000l$esc[?1002l$esc[?1006l" -NoNewline
    Write-Host "$esc[?25h" -NoNewline
    Write-Host "$esc[?1049l" -NoNewline
    Write-Host "$reset" -NoNewline
}
