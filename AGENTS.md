# AGENTS.md — Guía de Desarrollo para Agentes de IA en Axon

Este documento define la arquitectura, principios de diseño, lecciones aprendidas y reglas críticas de implementación para cualquier agente de IA (*o desarrollador humano*) que trabaje en el código fuente de **Axon**.

---

## 🏛️ Arquitectura del Sistema

El codebase de Axon está estructurado de forma modular y con dependencias mínimas en Rust:

```text
src/
├── main.rs         # Punto de entrada: carga la configuración y arranca el EventLoop con proxy de eventos del PTY.
├── app/            # AxonApp (estado global) y bucle de eventos de winit, repartido por responsabilidad:
│   ├── mod.rs        #   struct AxonApp, new(), guardado de sesión, geometría de la ventana
│   ├── handler.rs    #   impl ApplicationHandler: user_event, resumed, window_event (despacho), about_to_wait
│   ├── redraw.rs     #   on_redraw: un fotograma completo
│   ├── keyboard.rs   #   on_key_pressed: atajos y traducción de teclas a bytes del PTY
│   ├── cursor.rs, click.rs, wheel.rs   #   ratón: movimiento/hover, clics, rueda
│   ├── tabs.rs       #   crear/cerrar/dividir/navegar pestañas y paneles
│   ├── settings.rs   #   acciones de configuración (tema, fuentes, opacidad, valores por defecto)
│   ├── clipboard.rs  #   pegado de texto, archivos e imágenes (incl. SCP en SSH)
│   ├── layout.rs     #   árbol de paneles, TerminalPane y TerminalTab
│   ├── platform.rs   #   integración Win32: icono, tema DWM, opacidad, modificadores físicos
│   ├── keys.rs       #   key_is_char / key_code_to_char (AltGr y WM_SYSKEYDOWN)
│   ├── shell_context.rs #  rama git (sin ejecutar git) y detección del agente de IA
│   └── tests.rs
├── pty.rs          # PtyManager: ConPTY vía portable-pty; hilos I/O (spawn_io_threads) con eventos coalescidos.
├── terminal/       # Emulador VT/ANSI
│   ├── mod.rs        #   TerminalState: rejilla, scrollback, cursor, OSC 133/7/52
│   ├── performer.rs  #   traduce los eventos de `vte` (CSI, OSC, SGR) en cambios del estado
│   ├── cell.rs       #   Cell (16 bytes), imágenes inline, bloques de comandos, selección
│   └── tests.rs
├── renderer/       # Render por software (CPU) sobre un framebuffer u32
│   ├── mod.rs        #   TerminalRenderer (dueño de fuentes y cachés), new(), tamaños de rejilla
│   ├── glyphs.rs     #   fuentes embebidas (perezosas), cachés de glifos (FastMap), rasterización
│   ├── draw.rs       #   primitivas: fill_rect, blit_glyph, texto de terminal/UI, iconos Lucide
│   ├── geometry.rs   #   rectángulos compartidos entre dibujo y detección de clics (TopTabBarRects, ...)
│   ├── state.rs      #   tipos de estado de UI (SidebarTabItem, SettingsPageState, AiAgentStatus, ...)
│   ├── pane.rs, sidebar.rs, top_bar.rs, status_bar.rs, explorer_panel.rs, settings_page.rs   #   una pantalla por archivo
│   └── tests.rs      #   benchmarks y pruebas de render
├── explorer.rs     # Explorador de archivos lateral.
├── theme.rs        # Colores y temas.
├── config.rs       # AxonConfig (axon.toml) y estado de sesión; en tests usa una carpeta temporal.
├── tools.rs        # bin\ de herramientas CLI opcionales junto al exe (se añade al final del PATH).
├── logger.rs       # Logger a disco (axon.log), solo con AXON_DEBUG/AXON_LOG en release.
└── ui/             # paint.rs (rect redondeado SDF) e icons.rs (enum LucideIcon -> codepoint).
assets/
├── shell/init.ps1  # Script de PowerShell que Axon inyecta (prompt OSC 7/133, ls, agentes, ssh).
├── fonts/          # Fuentes REDUCIDAS embebidas + src/ (completas, no embebidas).
└── icon.ico, icon-256.png, icon-64.png   # Generados con tools/make-icon.ps1.
packaging/          # Todo lo de distribución
├── package.ps1, fetch-tools.ps1, test-installer.ps1, winget-manifest.ps1
├── installer/axon.iss   # Inno Setup
├── tools.json           # herramientas CLI con versión y SHA-256 fijos
└── winget/              # plantillas de manifiesto
tools/              # Herramientas de desarrollo
├── subset-fonts/   # Crate Rust independiente (skera): regenera assets/fonts/*.ttf.
└── make-icon.ps1   # Genera el icono (GDI+).
examples/           # axon.toml de ejemplo y demos
.github/workflows/  # ci.yml (tests, matriz PowerShell 5.1/7, empaquetado) y release.yml
```

---

## 📐 Principios de Diseño Fundamentales

1. **Native Windows First:** Axon no utiliza capas web (Electron, CEF, Webview) ni frameworks de GUI pesados. Toda integración con el sistema operativo (DWM Dark Mode, Mica, portapapeles HDROP, teclas de hardware) se realiza directamente vía `windows-sys`.
2. **Cumplimiento Estricto de Estándares VT/ECMA-48:** Cada secuencia de escape debe ajustarse a las especificaciones estándar de terminales (xterm, Ghostty, ECMA-48).
3. **Cero Suposiciones en Entrada de Teclado:** Nunca asumas el comportamiento de modificadores en Windows sin validar el estado físico del hardware.
4. **Optimizado para Agentes:** Cualquier nueva característica debe facilitar la interacción entre modelos de lenguaje (Claude Code, Aider, etc.) y la terminal (como la captura de imágenes o comandos OSC 133).

---

## ⚠️ Peculiaridades Críticas de la Plataforma (Lectura Obligatoria)

Al trabajar con eventos de teclado o emulación VT en Windows, **debes tener en cuenta los siguientes descubrimientos**:

### 1. Manejo de Backspace y Borrado de Palabras en Windows ConPTY y Harnesses (Claude Code, Agy)
* **Borrado de 1 carácter:** El emulador emite el byte **`0x7f`** (ASCII DEL). ConPTY lo traduce a un evento `VK_BACK` limpio (sin modificadores). Esto activa `BackwardDeleteChar` en PowerShell (`PSReadLine`) y borrado de 1 carácter en harnesses.
* **Borrado de palabra con `Ctrl+Backspace`:** El emulador emite el byte **`0x17`** (ASCII 23 / `Ctrl+W` / `werase`), que es el estándar adoptado por Windows Terminal. En PowerShell, `PSReadLine` tiene `Ctrl+w` enlazado nativamente a `BackwardKillWord`; y en harnesses de agentes de IA (Node.js/Ink en Claude Code, Go en agy), `0x17` activa directamente el borrado de palabra atrás (`kDeleteWordLeft`). *(Evitar enviar `0x08` para borrado de palabra, ya que los entornos en modo raw lo interpretan como un simple Backspace de 1 carácter).*
* **Borrado de palabra con `Alt+Backspace`:** El emulador emite la secuencia VT estándar **`\x1b\x7f`** (ESC + DEL). En Claude Code activa `key.meta` y ejecuta `kDeleteWordLeft`, en agy activa borrado de palabra, y en PowerShell se soporta limpiamente vinculando `Set-PSReadLineKeyHandler -Chord 'Alt+Backspace' -Function BackwardKillWord` en el script de arranque.

### 2. El Bug del Control Sintético con `AltGr` en Win32 y Atajos con `Alt` (WM_SYSKEYDOWN)
* En distribuciones de teclado en español/latinoamericanas, presionar `AltGr` (`VK_RMENU`) inyecta a nivel de driver un `VK_LCONTROL` sintético.
* Al soltar `AltGr`, Windows frecuentemente pierde o no emite el evento *Key-Up* de ese Control sintético, dejando el modificador `Ctrl` atascado permanentemente en los frameworks de ventanas como `winit`.
* **Regla obligatoria:** En [`src/app/`](src/app/), la consulta de teclas modificadoras se realiza mediante [`get_physical_modifiers()`](src/app/platform.rs) con `GetAsyncKeyState`:
  * `VK_LMENU` = Alt izquierdo real.
  * `VK_RMENU` = AltGr.
  * `Ctrl` solo se considera activo si `VK_RCONTROL` está presionado, o si `VK_LCONTROL` está activo **sin** que `VK_RMENU` (AltGr) esté presente.
  * Si `AltGr` no genera texto imprimible (teclas como D, V, W), en Windows equivale por driver a `Ctrl + Alt`, permitiendo activar los atajos de split correspondientes.
* **Atajos con `Alt` y `WM_SYSKEYDOWN`:** En Windows, cuando `Alt` está activo, `ToUnicodeEx` no genera caracteres y `winit` reporta `logical_key` como `Key::Unidentified`. Por ello, la detección de atajos (ej. `Ctrl+Alt+D`, `Alt+V`) **debe** verificar la tecla de hardware física [`PhysicalKey::Code(KeyCode)`](src/app/keys.rs) mediante [`key_is_char`](src/app/keys.rs).

### 3. Secuencia ECH (`\x1b[PnX`) y Texto Fantasma
* Al borrar palabras o caracteres con `Ctrl+Backspace`, PowerShell mueve el cursor hacia atrás y envía **`\x1b[PnX` (ECH - Erase Character)** para limpiar los `Pn` caracteres a la derecha del cursor.
* Si el emulador no procesa la acción `'X'`, los caracteres borrados permanecen dibujados en pantalla como "texto fantasma". `'X'`, `'P'` (DCH) y `'@'` (ICH) están implementados en [`TerminalState::csi_dispatch`](src/terminal.rs).

### 4. Limpieza de Scrollback en `clear` (`ED 3`)
* En PowerShell, `Clear-Host` empuja las líneas visibles hacia el búfer de scrollback y luego envía `\x1b[3J` (*Erase in Display modo 3*).
* Modo `2` solo borra la grilla visible activa.
* Modo `3` **debe** vaciar obligatoriamente `self.scrollback.clear()`, reiniciar `self.scroll_offset = 0` y limpiar `self.command_blocks.clear()`.

### 5. Seguridad de Foco, `Alt+Tab` y ConPTY Focus Reporting
* Al alternar entre ventanas con `Alt+Tab`, Windows absorbe la liberación de `Alt`.
* En `WindowEvent::Focused(false)` y `WindowEvent::Focused(true)`, `self.modifiers` **debe** resetearse a `ModifiersState::empty()` para evitar modificadores zombis.
* `NamedKey::Tab` con `Alt` activo debe ignorarse inmediatamente para no emitir `\t` accidental a la shell.
* **Focus Reporting y ConPTY:** Windows ConPTY no soporta secuencias VT de modo 1004 (`\x1b[I` / `\x1b[O`) en el canal de entrada; ConPTY las traduce a nivel de consola a pulsaciones de tecla no deseadas (`Enter`/`Escape`), provocando múltiples líneas de prompt vacías al cambiar de ventana. Por esta razón, el reporte de foco hacia el PTY está estrictamente desactivado en Windows.

### 6. Video Inverso (SGR 7) y Background Color Erase (BCE) en TUIs
* **Video Inverso (SGR 7):** Usado por TUIs como OpenCode, lazygit y htop para barras de pestañas, advertencias y selecciones. En [`src/renderer/`](src/renderer.rs), `compute_cell_fg` y `effective_bg` **deben** intercambiar `fg` y `bg`. Si `effective_bg` usa `cell.bg` en lugar de `cell.fg`, el texto y el fondo se vuelven del mismo color, volviendo el texto invisible ("texto gris fantasma").
* **Background Color Erase (BCE):** Al procesar `ED` (`'J'`), `EL` (`'K'`) y `ECH` (`'X'`) en [`src/terminal/`](src/terminal/), las celdas borradas deben rellenarse con `self.state.current_bg` si un fondo SGR está activo, en lugar de resetear indiscriminadamente a `Color::TRANSPARENT`. De lo contrario, se producen cortes visuales o líneas partidas en ventanas modales y banners.

### 7. Entorno de Producción vs Desarrollo y Telemetría de Diagnóstico
* En compilaciones de Release (`cargo build --release`), [`src/logger.rs`](src/logger.rs) desactiva completamente la creación y escritura en `axon.log` para maximizar el rendimiento y evitar escrituras en disco en equipos de usuarios finales.
* Solo se activa el log en Release si el usuario o agente define explícitamente las variables de entorno `AXON_DEBUG=1` o `AXON_LOG=1`.
* El perfil `[profile.release]` en `Cargo.toml` incluye optimizaciones agresivas (`opt-level = 3`, `lto = true`, `strip = true`, `panic = "abort"`).

### 8. Persistencia de Sesiones y Detección de Directorio (OSC 7 / OSC 9;9)
* Las pestañas activas, sus títulos, visibilidad del panel lateral y sus directorios de trabajo actuales se persisten automáticamente en `%APPDATA%\axon\axon\config\session.toml` (`SessionState`).
* El directorio de trabajo se sincroniza dinámicamente mediante el protocolo estándar OSC 7 (`\x1b]7;file://localhost/...`) emitido en cada comando del prompt de PowerShell, y mediante OSC 9;9 (`\x1b]9;9;"..."`).

### 9. Anclaje de Entrada Inferior para Agentes de IA (Claude Code, Gemini, Agy)
* En terminales tradicionales, los agentes CLI de entrada continua inician su prompt en la parte superior y se desplazan hacia abajo con cada consulta.
* En [`assets/shell/init.ps1`](assets/shell/init.ps1) (función `Invoke-AxonAgent`; `src/pty.rs` solo lo embebe con `include_str!`), los wrappers de `claude`, `gemini` y `agy` (modo `Anchor`) limpian la pantalla y posicionan el cursor en la última línea visible (`$Host.UI.RawUI.WindowSize.Height`), garantizando que la caja de entrada permanezca permanentemente anclada al fondo de la terminal.

### 10. Soporte Extendido para Conexiones SSH (Clonación de Paneles, SCP de Imágenes y OSC 1337)
* **Detección transparente de sesión:** El wrapper `function ssh` en el script de arranque de PowerShell emite `OSC 1337 AxonSSH=start;dest=<host>` al conectar y `AxonSSH=end` dentro de un bloque `try ... finally`. Adicionalmente, títulos emitidos vía `OSC 0` / `OSC 2` (`ssh: user@host`) y secuencias de directorio de trabajo `OSC 7` (`file://remotehost/path`) identifican y rastrean automáticamente el destino y directorio remoto.
* **Clonación de paneles divididos (`Ctrl+Alt+D` / `Ctrl+Alt+V`):** Al realizar un split sobre una terminal con sesión SSH activa, `split_active_pane` propaga la información mediante [`PtyManager::new_ssh()`](src/pty.rs), conectando instantáneamente el nuevo panel al mismo host y ruta remota, con retorno limpio a la shell local de Windows cuando el usuario finaliza la sesión SSH (`exit`).
* **Pegado inteligente de imágenes en SSH (`Ctrl+V`):** Al detectar una imagen en el portapapeles durante una sesión SSH activa, Axon evalúa si corre Claude Code remoto (emitiendo `\x1bv` / `Alt+V`). Para shells remotas convencionales, Axon guarda el PNG en la caché local y lo transfiere al servidor remoto usando `scp.exe` nativo de Windows (OpenSSH) hacia `/tmp/axon_img_<timestamp>.png`, inyectando la ruta remota en la consola remota de forma totalmente transparente para el usuario o agentes CLI remotos.
* **Portapapeles remoto bidireccional (OSC 52):** Soporte para que herramientas remotas (Neovim, tmux, vim) copien texto directamente al portapapeles del sistema operativo anfitrión en Windows mediante el protocolo OSC 52 en base64.

### 11. Pestaña Dedicada de Ajustes, Tema Catppuccin Frappé y Atajos Canónicos (`Ctrl + ,`)
* **Pestaña nativa de página completa (`TerminalTab::new_settings`):** En lugar de un popup modal superpuesto con riesgo de desbordamiento, los Ajustes residen en una pestaña dedicada accesible vía `Ctrl + ,` o con el botón `⚙` del encabezado del sidebar o barra superior. Cuenta con 4 secciones conmutables: `◈ Apariencia`, `◈ Terminal`, `◈ Atajos de Teclado` e `◈ Acerca de`.
* **Tema Catppuccin Frappé y elevaciones de UI:** Se integra el tema `Catppuccin Frappé` junto a Ghostty Dark, One Dark, Tokyo Night, Monokai Pro, Nord, Solarized Dark y Catppuccin Latte. La barra lateral y componentes de UI usan colores dinámicos (`theme.sidebar_bg()`, `theme.surface_bg()`, `theme.surface_hover()`, `theme.border_color()`, `theme.accent_color()`), proporcionando contraste y elevación visual limpia en cualquier tema.
* **Geometría compartida (`SettingsPageRects`):** La estructura [`SettingsPageRects::compute()`](src/renderer/geometry.rs) centraliza la posición de las píldoras de navegación de pestañas y botones interactivos tanto para el renderizado como para `CursorMoved` y `MouseInput`, garantizando detección precisa de clics y estados hover.
* **Aislamiento de entrada y PTY:** La pestaña de ajustes se inicializa sin procesos ConPTY (`panes: vec![]`). La pulsación de teclas y clics son absorbidos limpiamente por la interfaz de ajustes sin emitir bytes accidentales a terminales en segundo plano.
* **Atajos Canónicos Unificados:** Se eliminaron las combinaciones duplicadas para una misma acción; cada comando tiene un único atajo canónico estándar (`Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Ctrl+Alt+D`, `Ctrl+Alt+V`, `Ctrl+Alt+W`, `Ctrl+B`, `Ctrl+,`, `Ctrl+V`, `Ctrl+Shift+C`, `Ctrl+Shift+K`, `Ctrl+Shift+O`).

### 12. Identidad Dev-First, Pestañas Horizontales/Verticales, Status Bar y Tema de Sistema
* **Cero Emojis y Motivo Unificado `◈`:** Regla estética estricta: ninguna parte de la interfaz de Axon debe utilizar emojis visuales de mensajería. Se utiliza el rombo de desarrollador `◈` como motivo de diseño insignia que se extiende de manera uniforme en pestañas, botones de navegación, rutas y status bar, junto a los glifos de ingeniería `⎇` (Git branch), `⚙` (Ajustes), `●` / `○` (estado de agentes).
* **Disposición de Pestañas Intercambiable (`tab_layout`):** Soporte tanto para barra lateral vertical (default, espaciado generoso de 36px por pestaña) como para barra horizontal superior tradicional estilo Chrome/Zed (38px de altura, línea de acento superior de 2px, renderizado en [`render_top_tab_bar`](src/renderer/top_bar.rs)), conmutable dinámicamente desde Ajustes o `axon.toml`.
* **Barra de Estado Inferior Informativa (Status Bar - 26px):** Proporciona contexto continuo para desarrolladores:
  - *Izquierda:* Ruta de trabajo actual (`◈ C:\...`) o sesión remota (`◈ ssh: user@host`) y rama Git activa (`⎇ main`, consultada de forma no bloqueante a nivel de archivo `<0.02ms`).
  - *Derecha:* Dimensiones de grilla activa (`cols×rows`) y badge en tiempo real del agente de IA activo (`● CLAUDE`, `● AGY`, `● GEMINI`, `● AIDER`, `● COPILOT`, `● OLLAMA` o `○ LOCAL`).
* **Soporte de Modo Claro (Catppuccin Latte) y Sincronización con Windows 11:**
  - Integración del tema claro oficial Catppuccin Latte (`#eff1f5`, `#4c4f69`).
  - Con `theme_mode = "system"`, Axon consulta dinámicamente el Registro de Windows (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`) actualizando la interfaz y la barra de título DWM en tiempo real cuando el sistema operativo cambia entre tema claro y oscuro.

### 13. Explorador Lateral Derecho de Archivos (Dev-First, Click-to-Prompt y Ctrl+Shift+E)
* **Panel derecho integrado (`FILE_EXPLORER_WIDTH = 240`):** Implementado en [`src/explorer.rs`](src/explorer.rs) y renderizado en [`render_file_explorer`](src/renderer/explorer_panel.rs). Muestra el árbol de archivos y carpetas del proyecto activo sin emojis, utilizando glifos `◈`, `▸`, `▾`, `↻`, `x`, `..`.
* **Click-to-Prompt:** Al hacer clic en cualquier archivo, su ruta relativa (o entre comillas si contiene espacios) se inyecta directamente en el prompt del PTY activo, permitiendo interacción instantánea con shells o agentes CLI (Claude Code, agy, gemini).
* **Navegación completa y scroll:** Clic en directorios expande/colapsa carpetas conservando el árbol, `..` sube al directorio padre, `↻` recarga el disco y la rueda del ratón permite scroll suave en repositorios grandes.
* **Control y persistencia unificada:** Conmutable mediante el atajo canónico `Ctrl + Shift + E`, el botón `ARCHIVOS` de la barra de estado inferior, el botón de archivos de la barra superior horizontal, o la opción en Ajustes (`Ctrl + ,`). Su visibilidad se persiste en `%APPDATA%\axon\axon\config\session.toml`.

### 14. Diseño Minimalista Floating Island, Iconografía Lucide y Tipografía Inter
* **Sub-sistema UI Nativo (`src/ui/`):**
  - [`src/ui/paint.rs`](src/ui/paint.rs): Primitivas de dibujo 2D aceleradas por CPU para `softbuffer`: `draw_rounded_rect` (con antialiasing suave y mezcla alpha 0-255) y `stroke_rounded_rect`.
  - [`src/ui/icons.rs`](src/ui/icons.rs): Catálogo y mapeo estricto del enum `LucideIcon` a los puntos de código Unicode de la tipografía vectorial embebida [`assets/fonts/lucide.ttf`](assets/fonts/lucide.ttf).
* **Tipografía Dual de Alta Fidelidad:**
  - *Área de terminal:* **JetBrains Mono Nerd Font** (monoespaciada con métricas precisas para la grilla VT y programación).
  - *Interfaz gráfica:* **Inter** ([`Inter-500.ttf`](assets/fonts/Inter-500.ttf) y [`Inter-600.ttf`](assets/fonts/Inter-600.ttf)), proporcionando una legibilidad moderna, limpia y proporcionada en pestañas, explorador, barra de estado y página de ajustes.
* **Barra Flotante de Pestañas (Floating Island Capsule):**
  - La barra superior centra una cápsula flotante con esquinas redondeadas (radio 14px), fondo translúcido y borde sutil.
  - Pestañas con icono Lucide semántico (`Sparkles` para agentes IA, `Server` para SSH, `Terminal` para PowerShell/shells, `Settings` para ajustes), título en Inter, botón de cierre `X` y botón `+` integrado.
* **Explorador y Paneles Overlay:**
  - El explorador de archivos se muestra como un panel flotante overlay translúcido con esquinas redondeadas, sin fragmentar abruptamente la grilla del terminal.
  - Iconos Lucide adaptados a la extensión del archivo (`FileCode` para `.rs`, `FileJson` para `.toml`/`.json`, `FileText` para `.md`, etc.) y jerarquía con chevrons vectoriales.

### 15. Optimización de Memoria RAM, Throughput VT y Renderizado CPU
* **Empaquetado de Cell (16 bytes):** La estructura `Cell` empaqueta los estilos booleanos en un bitfield de 1 byte (`flags: u8`), reduciendo su tamaño de 24 a 16 bytes y derivando `Copy`.
* **Trimming de Scrollback:** Al transferir filas de `grid` a `scrollback`, se recortan celdas vacías por la derecha, reduciendo la huella de memoria del historial en un 70% - 85%.
* **Desplazamiento $O(1)$ y Reciclaje de Búfers:** `grid` y `alt_grid` utilizan `VecDeque<Vec<Cell>>`. Al alcanzar `max_scrollback`, la fila expulsada se recicla como nueva fila inferior, eliminando asignaciones en el heap por salto de línea.
* **Fast-Path ASCII y Tabla Plana de Glifos:** `put_char()` resuelve caracteres ASCII imprimibles (`0x20..=0x7e`) con ancho fijo 1 sin consultar las tablas de `unicode-width`. `TerminalRenderer` indexa glifos ASCII directamente en `[Option<CachedGlyph>; 256]` $O(1)$ sin colisiones de hash.
* **Alpha Blending In-Situ y Fast-Path Opaque:** En `render_pane()`, píxeles con `alpha == 255` se escriben directamente en el buffer de softbuffer sin lecturas de RAM previas.
* **Caché de Rama Git en el Render Loop:** `get_git_branch()` se cachea en `TerminalPane` y solo accede a disco cuando `current_dir` cambia, eliminando lecturas síncronas de `.git/HEAD` en cada fotograma.
* **Pausa de Cursor Blinking en Segundo Plano:** Al perder foco (`!self.is_focused`), el temporizador de cursor se apaga y el event loop entra en reposo completo (`ControlFlow::Wait`), reduciendo el consumo de CPU y batería a cero.

### 16. Reglas de Recursos (RAM / CPU / Binario) — Puntos Únicos a Respetar
* **fontdue guarda TODOS los contornos de una fuente en RAM** (JetBrains Nerd completa = ~50 MB, Lucide completa = ~15 MB). Por eso las fuentes embebidas están reducidas con [`tools/subset-fonts`](tools/subset-fonts/src/main.rs) (Rust, crate `skera`) (fuentes completas en `assets/fonts/src/`, no se embeben): `JetBrainsMono-{Regular,Bold}-Core.ttf` (sin PUA), iconos Nerd en dos bloques cargados bajo demanda (`Regular-Nerd` U+E000–F8FF, `Regular-NerdMd` U+F0000+), y `lucide.ttf` solo con los iconos de `src/ui/icons.rs`.
  * **Al añadir un `LucideIcon` nuevo: ejecutar `cargo run --release --manifest-path tools/subset-fonts/Cargo.toml`** (si no, el test `test_all_lucide_glyphs_exist_in_font` falla). No volver a embeber las fuentes completas.
  * Negrita, bloques Nerd y fallback `seguisym.ttf` son `OnceCell` perezosos en `TerminalRenderer`; nunca parsearlos en `new()`.
* **Un solo blitter de texto:** todo glifo (terminal, Inter, Lucide) se compone con `blit_glyph(...)` y todo relleno de rectángulo con `fill_rect(...)` (recorte previo + `slice::fill`) en `src/renderer/`. No escribir bucles píxel a píxel con `if x < max_x` nuevos.
* **Cachés de glifos con `FastMap`** (hasher FxHash propio), no `HashMap` por defecto. `measure_ui_text` usa `ui_advance_cache`, no recalcula métricas por fotograma.
* **PTY:** el hilo lector coalesce eventos (`output_pending` en `PtyManager`): un `PtyData` por ráfaga. Cualquier consumidor debe vaciar con `try_recv_output()` hasta `None` (ese método rearma el flag). Los hilos I/O se crean solo vía `spawn_io_threads` (pila de 256 KB).
* **Procesos huérfanos:** `PtyManager` implementa `Drop` y termina el shell (`child.kill()`). Soltar el ConPTY por sí solo cierra su consola, pero no alcanza a un shell que aún arranca: sobrevivía (~100 MB cada `pwsh`) al cerrar una pestaña, la ventana o al terminar los tests. No quites el `Drop`. Tests: `test_dropping_pty_kills_shell_and_its_children` y `test_dropping_pty_during_startup_leaves_no_orphans` (este último solo falla bajo carga, con la suite completa en paralelo). Para comprobar fugas a mano: contar procesos `pwsh`/`powershell` con `-NoExit` y el script de inicio cuyo padre ya no existe.
* **Límites:** `scrollback_lines` por defecto 10 000; `command_blocks` acotado a 512 (`push_command_block`); `SetWindowText` solo si el título cambia.
* **Script de PowerShell (`assets/shell/init.ps1`) = ruta caliente del usuario.** Se ejecuta en cada prompt y comando; medido en Windows 11:
  * **Prohibido lanzar procesos en `prompt`.** `git rev-parse` costaba ~42 ms por prompt; leer `.git/HEAD` cuesta ~2 ms.
  * **No usar `Get-Command` para buscar programas:** si el programa no existe recorre todo PATH (250–560 ms). Usar `Find-AxonApp` (`File.Exists` + `PATHEXT`, memoizada).
  * Editar el `.ps1` (con resaltado), nunca volver a incrustar el script como cadenas en Rust. Validar con `cargo test pty` y probando en `powershell.exe` 5.1 **y** `pwsh.exe` 7 (el shell por defecto es `pwsh.exe` si está en Program Files).
* **Bucle de eventos (`about_to_wait`):** calcula UN único próximo despertar (`earliest(...)`: cursor, ocultado de scrollbar, tema del sistema) y usa `ControlFlow::Wait` si no hay ninguno (0 % CPU en reposo). Al añadir un temporizador, súmalo a ese cálculo; no llames a `set_control_flow` por tu cuenta. Con la ventana oculta (`WindowEvent::Occluded`) no se renderiza ni parpadea el cursor, pero el PTY se sigue procesando.
* **Scrollback:** `newline()` reutiliza el buffer de la fila que sale como nueva fila inferior y guarda una copia recortada de tamaño exacto (una asignación por línea).
* **Decisión: render por software, sin GPU.** Medido en release: fotograma completo 1920×1080 con texto denso ≈ 1,3 ms de CPU; `present` ≈ 0,3 ms; el volcado masivo está limitado por ConPTY/conhost (~2 MB/s), no por el dibujo. Un backend D3D11/wgpu no mejora esas cifras y añade decenas de MB de RAM, arranque y binario. Se probaron y descartaron: límite de fotogramas bajo ráfagas (sin diferencia medible de CPU) y reescritura GPU completa. **Antes de proponer una ruta GPU, mide primero** (`cargo test --release bench_ -- --nocapture`); solo se justifica si el render pasa a dominar (p. ej. 4K de alta frecuencia con scroll continuo), y entonces solo para la rejilla de celdas con la UI cacheada como textura.
* **Cómo medir (reproducible):** `cargo test --release bench_ -- --nocapture` (render de fotograma y throughput VT). RAM: arrancar `target\release\axon.exe` y leer `WorkingSet64`/`PrivateMemorySize64` (referencia: ~47 MB / ~34 MB en reposo). No fusionar un cambio de rendimiento sin una cifra antes/después.
* **Pendiente conocido (no hecho a propósito):** repintado parcial por filas sucias. Ahorraría ~1 ms por fotograma pero exige que cualquier estado de UI nuevo invalide el fotograma completo; solo con medición que lo justifique.
* **Distribución autónoma:** `.cargo/config.toml` enlaza el CRT estáticamente (`+crt-static`): `axon.exe` no importa `vcruntime140.dll` y arranca en un Windows limpio. No lo quites; CI falla si reaparece la dependencia. Un fallo de ConPTY se muestra con `pty::report_spawn_error`/`fatal_spawn_error` (MessageBox): el subsistema "windows" no tiene consola y `panic = "abort"` cerraría la app sin avisar; no uses `.expect()` al crear PTYs.
* **Compatibilidad con PowerShell = test obligatorio.** `test_init_script_in_every_installed_powershell` (src/pty.rs) arranca el script real en `powershell.exe` 5.1 y en `pwsh.exe` 7 y exige prompt OSC 7/133, rama git, `Find-AxonApp`, `ls` y `$Error.Count == 0` (en 7 los errores concisos no traen `CategoryInfo`, por eso se cuenta `$Error`). Usa `-ErrorAction Ignore` (no `SilentlyContinue`) en comandos cuyo fallo es esperado. `.github/workflows/ci.yml` lo ejecuta en `windows-2022` y `windows-latest` y falla si falta alguna de las dos versiones.
* **Distribución (Inno Setup + herramientas empaquetadas):**
  * `packaging/package.ps1` = build + `fetch-tools.ps1` + ZIP portable + instalador (`packaging/installer/axon.iss`) + `SHA256SUMS.txt`; `packaging/test-installer.ps1` instala en silencio, ejecuta cada herramienta y Axon, y desinstala. CI (`ci.yml` job `package`, y `release.yml`) ejecuta ambos.
  * Las herramientas (eza, bat, rg, fd, zoxide, fzf) están en `packaging/tools.json` con URL y **SHA-256 fijos**; `fetch-tools.ps1` aborta si un hash no coincide. Para actualizar una: cambia `version`/`url`, ejecuta `pwsh packaging/fetch-tools.ps1 -Update` (recalcula hashes; verifica el diff) y prueba con `package.ps1` + `test-installer.ps1`. Las licencias van en `licenses/` (obligatorio al redistribuir). `bat` necesita `vcruntime140.dll` (se copia del Redist de Visual Studio junto a `bat.exe`); al añadir una herramienta, comprueba si depende del runtime de VC++ (`appLocalDlls`).
  * Axon NO descarga nada en ejecución. `tools.rs` añade `bin\` junto al exe **al final** del PATH de los procesos hijos (las versiones del usuario tienen prioridad).
  * El instalador es por usuario (`PrivilegesRequired=lowest`), no instala `axon.toml` (Axon crea la configuración en `%APPDATA%\axon\axon\config` y no se pisa la del usuario) y el `AppId` de `axon.iss` es fijo: **no lo cambies** o las actualizaciones dejarán de reconocer la instalación previa. Los scripts `.ps1` se guardan en UTF-8 **con BOM** (Windows PowerShell 5.1 los lee como ANSI sin él). Winget: `pwsh packaging/winget-manifest.ps1 -Version X.Y.Z` genera `dist\winget\` a partir de `packaging/winget/` (PR a microsoft/winget-pkgs).
  * Pendiente: firma de código del `.exe` y del instalador (SmartScreen avisa mientras no esté firmado).
* **Icono:** `pwsh tools/make-icon.ps1` regenera `assets/icon.ico` (9 tamaños), `icon-256.png` (ventana y README) e `icon-64.png`. `build.rs` incrusta `icon.ico` en el `.exe`.

---

## 🧩 Convenciones de Código y Repositorio

* **Dónde va cada cosa** (ver el árbol de arriba): una pantalla/componente de UI por archivo en `src/renderer/`; un método nuevo de `AxonApp` va en el submódulo temático (`tabs`, `settings`, `clipboard`, ...), no en `mod.rs`; el manejo de eventos entra por `app/handler.rs` y se **despacha** a `redraw`, `keyboard`, `cursor`, `click` o `wheel` (los brazos de `window_event` deben seguir siendo una línea que llama a un método). Un archivo que pase de ~1 000 líneas es señal de que hay que dividirlo por responsabilidad.
* **Tests junto al código:** `src/<módulo>/tests.rs` (`#[cfg(test)] mod tests;`). Los tests no deben tocar la configuración real: `config::dirs_or_fallback()` usa una carpeta temporal bajo `cfg!(test)`; no la saltes.
* **Formato y lints:** `cargo fmt` y `cargo clippy --all-targets` sin avisos antes de dar algo por terminado. Los pocos `allow` están justificados con un comentario (primitivas de dibujo con coordenadas explícitas, p. ej.).
* **Enlaces y rutas:** en la documentación usa rutas relativas al repositorio (nunca rutas absolutas de tu máquina).
* **Archivos locales:** `axon.toml` de la raíz y `*.log` están en `.gitignore` (la configuración de ejemplo está en `examples/axon.toml`). Los `.ps1`/`.iss` se guardan en UTF-8 con BOM (`.editorconfig`).
* **Cambios mecánicos (mover código entre archivos):** verifica que el render no cambia comparando hashes de fotogramas antes y después (se usó un test temporal que dibuja todas las pantallas con datos fijos); `cargo test` por sí solo no detecta un cambio de píxeles.

---

### Seguridad: la salida de la terminal es entrada NO confiable
* **Nada que venga de secuencias de escape se interpola en órdenes sin validar.** El destino SSH (título OSC 0/2, `AxonSSH` OSC 1337, host de OSC 7) y el directorio remoto (OSC 7) los puede fijar cualquier programa o servidor al que te conectes. `pty::is_safe_ssh_destination` (solo `host` / `usuario@host`, sin empezar por `-`) y `pty::safe_remote_dir` se aplican en el origen (`terminal/performer.rs`) **y** en los sumideros (`PtyManager::new_ssh`, subida por `scp` en `app/clipboard.rs`), y se usa `--` antes del destino. Si añades otro sumidero que use `ssh_session`, valida igual. Tests: `test_ssh_destination_rejects_injection`, `test_remote_dir_rejects_shell_metacharacters`, `test_untrusted_escape_sequences_cannot_set_unsafe_ssh_session`.
* **Pegado:** `clipboard::sanitize_paste` elimina los caracteres de control (ESC incluido) antes de escribir en el PTY, para que un portapapeles no pueda cerrar el pegado entre corchetes (`ESC[201~`) ni emitir secuencias. Test: `test_paste_is_sanitized_against_escape_injection`.
* **Logs:** `axon.log` nunca registra el texto tecleado. La traza de teclas es opt-in (`AXON_LOG_KEYS=1`, independiente de `AXON_DEBUG`/`AXON_LOG`) y solo guarda la longitud del texto; una contraseña tecleada en un prompt no debe acabar en disco.
* **Decisión pendiente (no resuelta a propósito):** OSC 52 permite que cualquier programa escriba en el portapapeles de Windows sin confirmar (es una función deseada para Neovim/tmux por SSH). Si se quiere endurecer: opción de configuración o confirmación; hoy se acepta sin avisar.

---

## 🧪 Pruebas y Validación

Antes de dar por completado cualquier cambio:

1. **Ejecutar suite de tests:**
   ```powershell
   cargo test
   ```
2. **Validar integración PTY:**
   `src/pty.rs` contiene pruebas con procesos reales de PowerShell dentro de ConPTY (5.1 y 7). Si modificas entrada de teclado, el script de inicio o secuencias VT, añade o valida los tests correspondientes allí o en `src/terminal/tests.rs`.
3. **Formato y lints:**
   ```powershell
   cargo fmt --check; cargo clippy --all-targets
   ```
4. **Compilar en Release:**
   ```powershell
   cargo build --release
   ```
5. **Si tocaste empaquetado:** `pwsh packaging/package.ps1` y `pwsh packaging/test-installer.ps1`.
6. **Verificar diagnósticos:**
   Revisa `axon.log` en modo de desarrollo (`cargo run`) si necesitas depurar secuencias de escape recibidas o eventos de hardware emitidos.
