# AGENTS.md — Guía de Desarrollo para Agentes de IA en Axon

Este documento define la arquitectura, principios de diseño, lecciones aprendidas y reglas críticas de implementación para cualquier agente de IA (*o desarrollador humano*) que trabaje en el código fuente de **Axon**.

---

## 🏛️ Arquitectura del Sistema

El codebase de Axon está estructurado de forma modular y con dependencias mínimas en Rust:

```text
src/
├── main.rs       # Punto de entrada; inicializa el EventLoop con proxy de eventos asíncronos.
├── app.rs        # AxonApp: Implementa ApplicationHandler de winit, captura teclas, foco y portapapeles.
├── pty.rs        # PtyManager: Wrapper de Windows ConPTY mediante portable-pty e hilo asíncrono de I/O.
├── terminal.rs   # TerminalState: Máquina de estados VT/ANSI (VTE parser), grilla activa, scrollback y OSC 133.
├── renderer.rs   # TerminalRenderer: Rasterizador tipográfico fontdue, caché de glifos y softbuffer.
├── theme.rs      # Definiciones de color y paleta de Ghostty.
├── config.rs     # AxonConfig: Carga y serialización en TOML.
├── tools.rs      # Gestor y aprovisionador de herramientas CLI a nivel de aplicación (eza, bat, rg, fd, zoxide, fzf).
└── logger.rs     # Logger ligero en disco (axon.log) para diagnóstico de I/O y secuencias de escape.
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
* **Regla obligatoria:** En [`src/app.rs`](file:///C:/Users/gmora/dev/axon/src/app.rs), la consulta de teclas modificadoras se realiza mediante [`get_physical_modifiers()`](file:///C:/Users/gmora/dev/axon/src/app.rs) con `GetAsyncKeyState`:
  * `VK_LMENU` = Alt izquierdo real.
  * `VK_RMENU` = AltGr.
  * `Ctrl` solo se considera activo si `VK_RCONTROL` está presionado, o si `VK_LCONTROL` está activo **sin** que `VK_RMENU` (AltGr) esté presente.
  * Si `AltGr` no genera texto imprimible (teclas como D, V, W), en Windows equivale por driver a `Ctrl + Alt`, permitiendo activar los atajos de split correspondientes.
* **Atajos con `Alt` y `WM_SYSKEYDOWN`:** En Windows, cuando `Alt` está activo, `ToUnicodeEx` no genera caracteres y `winit` reporta `logical_key` como `Key::Unidentified`. Por ello, la detección de atajos (ej. `Ctrl+Alt+D`, `Alt+V`) **debe** verificar la tecla de hardware física [`PhysicalKey::Code(KeyCode)`](file:///C:/Users/gmora/dev/axon/src/app.rs) mediante [`key_is_char`](file:///C:/Users/gmora/dev/axon/src/app.rs).

### 3. Secuencia ECH (`\x1b[PnX`) y Texto Fantasma
* Al borrar palabras o caracteres con `Ctrl+Backspace`, PowerShell mueve el cursor hacia atrás y envía **`\x1b[PnX` (ECH - Erase Character)** para limpiar los `Pn` caracteres a la derecha del cursor.
* Si el emulador no procesa la acción `'X'`, los caracteres borrados permanecen dibujados en pantalla como "texto fantasma". `'X'`, `'P'` (DCH) y `'@'` (ICH) están implementados en [`TerminalState::csi_dispatch`](file:///C:/Users/gmora/dev/axon/src/terminal.rs).

### 4. Limpieza de Scrollback en `clear` (`ED 3`)
* En PowerShell, `Clear-Host` empuja las líneas visibles hacia el búfer de scrollback y luego envía `\x1b[3J` (*Erase in Display modo 3*).
* Modo `2` solo borra la grilla visible activa.
* Modo `3` **debe** vaciar obligatoriamente `self.scrollback.clear()`, reiniciar `self.scroll_offset = 0` y limpiar `self.command_blocks.clear()`.

### 5. Seguridad de Foco y `Alt+Tab`
* Al alternar entre ventanas con `Alt+Tab`, Windows absorbe la liberación de `Alt`.
* En `WindowEvent::Focused(false)` y `WindowEvent::Focused(true)`, `self.modifiers` **debe** resetearse a `ModifiersState::empty()` para evitar modificadores zombis.
* `NamedKey::Tab` con `Alt` activo debe ignorarse inmediatamente para no emitir `\t` accidental a la shell.

### 6. Video Inverso (SGR 7) y Background Color Erase (BCE) en TUIs
* **Video Inverso (SGR 7):** Usado por TUIs como OpenCode, lazygit y htop para barras de pestañas, advertencias y selecciones. En [`src/renderer.rs`](file:///C:/Users/gmora/dev/axon/src/renderer.rs), `compute_cell_fg` y `effective_bg` **deben** intercambiar `fg` y `bg`. Si `effective_bg` usa `cell.bg` en lugar de `cell.fg`, el texto y el fondo se vuelven del mismo color, volviendo el texto invisible ("texto gris fantasma").
* **Background Color Erase (BCE):** Al procesar `ED` (`'J'`), `EL` (`'K'`) y `ECH` (`'X'`) en [`src/terminal.rs`](file:///C:/Users/gmora/dev/axon/src/terminal.rs), las celdas borradas deben rellenarse con `self.state.current_bg` si un fondo SGR está activo, en lugar de resetear indiscriminadamente a `Color::TRANSPARENT`. De lo contrario, se producen cortes visuales o líneas partidas en ventanas modales y banners.

### 7. Entorno de Producción vs Desarrollo y Telemetría de Diagnóstico
* En compilaciones de Release (`cargo build --release`), [`src/logger.rs`](file:///C:/Users/gmora/dev/axon/src/logger.rs) desactiva completamente la creación y escritura en `axon.log` para maximizar el rendimiento y evitar escrituras en disco en equipos de usuarios finales.
* Solo se activa el log en Release si el usuario o agente define explícitamente las variables de entorno `AXON_DEBUG=1` o `AXON_LOG=1`.
* El perfil `[profile.release]` en `Cargo.toml` incluye optimizaciones agresivas (`opt-level = 3`, `lto = true`, `strip = true`, `panic = "abort"`).

### 8. Persistencia de Sesiones y Detección de Directorio (OSC 7 / OSC 9;9)
* Las pestañas activas, sus títulos, visibilidad del panel lateral y sus directorios de trabajo actuales se persisten automáticamente en `%LOCALAPPDATA%\axon\session.toml` (`SessionState`).
* El directorio de trabajo se sincroniza dinámicamente mediante el protocolo estándar OSC 7 (`\x1b]7;file://localhost/...`) emitido en cada comando del prompt de PowerShell, y mediante OSC 9;9 (`\x1b]9;9;"..."`).

### 9. Anclaje de Entrada Inferior para Agentes de IA (Claude Code, Gemini, Agy)
* En terminales tradicionales, los agentes CLI de entrada continua inician su prompt en la parte superior y se desplazan hacia abajo con cada consulta.
* En [`src/pty.rs`](file:///C:/Users/gmora/dev/axon/src/pty.rs), los wrappers de arranque para `claude`, `gemini` y `agy` limpian la pantalla y posicionan el cursor en la última línea visible (`$Host.UI.RawUI.WindowSize.Height`), garantizando que la caja de entrada permanezca permanentemente anclada al fondo de la terminal.

### 10. Soporte Extendido para Conexiones SSH (Clonación de Paneles, SCP de Imágenes y OSC 1337)
* **Detección transparente de sesión:** El wrapper `function ssh` en el script de arranque de PowerShell emite `OSC 1337 AxonSSH=start;dest=<host>` al conectar y `AxonSSH=end` dentro de un bloque `try ... finally`. Adicionalmente, títulos emitidos vía `OSC 0` / `OSC 2` (`ssh: user@host`) y secuencias de directorio de trabajo `OSC 7` (`file://remotehost/path`) identifican y rastrean automáticamente el destino y directorio remoto.
* **Clonación de paneles divididos (`Ctrl+Alt+D` / `Ctrl+Alt+V`):** Al realizar un split sobre una terminal con sesión SSH activa, `split_active_pane` propaga la información mediante [`PtyManager::new_ssh()`](file:///C:/Users/gmora/dev/axon/src/pty.rs), conectando instantáneamente el nuevo panel al mismo host y ruta remota, con retorno limpio a la shell local de Windows cuando el usuario finaliza la sesión SSH (`exit`).
* **Pegado inteligente de imágenes en SSH (`Ctrl+V`):** Al detectar una imagen en el portapapeles durante una sesión SSH activa, Axon evalúa si corre Claude Code remoto (emitiendo `\x1bv` / `Alt+V`). Para shells remotas convencionales, Axon guarda el PNG en la caché local y lo transfiere al servidor remoto usando `scp.exe` nativo de Windows (OpenSSH) hacia `/tmp/axon_img_<timestamp>.png`, inyectando la ruta remota en la consola remota de forma totalmente transparente para el usuario o agentes CLI remotos.
* **Portapapeles remoto bidireccional (OSC 52):** Soporte para que herramientas remotas (Neovim, tmux, vim) copien texto directamente al portapapeles del sistema operativo anfitrión en Windows mediante el protocolo OSC 52 en base64.

### 11. Pestaña Dedicada de Ajustes, Tema Catppuccin Frappé y Atajos Canónicos (`Ctrl + ,`)
* **Pestaña nativa de página completa (`TerminalTab::new_settings`):** En lugar de un popup modal superpuesto con riesgo de desbordamiento, los Ajustes residen en una pestaña dedicada accesible vía `Ctrl + ,` o con el botón `⚙` del encabezado del sidebar o barra superior. Cuenta con 4 secciones conmutables: `◈ Apariencia`, `◈ Terminal`, `◈ Atajos de Teclado` e `◈ Acerca de`.
* **Tema Catppuccin Frappé y elevaciones de UI:** Se integra el tema `Catppuccin Frappé` junto a Ghostty Dark, One Dark, Tokyo Night, Monokai Pro, Nord, Solarized Dark y Catppuccin Latte. La barra lateral y componentes de UI usan colores dinámicos (`theme.sidebar_bg()`, `theme.surface_bg()`, `theme.surface_hover()`, `theme.border_color()`, `theme.accent_color()`), proporcionando contraste y elevación visual limpia en cualquier tema.
* **Geometría compartida (`SettingsPageRects`):** La estructura [`SettingsPageRects::compute()`](file:///C:/Users/gmora/dev/axon/src/renderer.rs) centraliza la posición de las píldoras de navegación de pestañas y botones interactivos tanto para el renderizado como para `CursorMoved` y `MouseInput`, garantizando detección precisa de clics y estados hover.
* **Aislamiento de entrada y PTY:** La pestaña de ajustes se inicializa sin procesos ConPTY (`panes: vec![]`). La pulsación de teclas y clics son absorbidos limpiamente por la interfaz de ajustes sin emitir bytes accidentales a terminales en segundo plano.
* **Atajos Canónicos Unificados:** Se eliminaron las combinaciones duplicadas para una misma acción; cada comando tiene un único atajo canónico estándar (`Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Ctrl+Alt+D`, `Ctrl+Alt+V`, `Ctrl+Alt+W`, `Ctrl+B`, `Ctrl+,`, `Ctrl+V`, `Ctrl+Shift+C`, `Ctrl+Shift+K`, `Ctrl+Shift+O`).

### 12. Identidad Dev-First, Pestañas Horizontales/Verticales, Status Bar y Tema de Sistema
* **Cero Emojis y Motivo Unificado `◈`:** Regla estética estricta: ninguna parte de la interfaz de Axon debe utilizar emojis visuales de mensajería. Se utiliza el rombo de desarrollador `◈` como motivo de diseño insignia que se extiende de manera uniforme en pestañas, botones de navegación, rutas y status bar, junto a los glifos de ingeniería `⎇` (Git branch), `⚙` (Ajustes), `●` / `○` (estado de agentes).
* **Disposición de Pestañas Intercambiable (`tab_layout`):** Soporte tanto para barra lateral vertical (default, espaciado generoso de 36px por pestaña) como para barra horizontal superior tradicional estilo Chrome/Zed (38px de altura, línea de acento superior de 2px, renderizado en [`render_top_tab_bar`](file:///C:/Users/gmora/dev/axon/src/renderer.rs)), conmutable dinámicamente desde Ajustes o `axon.toml`.
* **Barra de Estado Inferior Informativa (Status Bar - 26px):** Proporciona contexto continuo para desarrolladores:
  - *Izquierda:* Ruta de trabajo actual (`◈ C:\...`) o sesión remota (`◈ ssh: user@host`) y rama Git activa (`⎇ main`, consultada de forma no bloqueante a nivel de archivo `<0.02ms`).
  - *Derecha:* Dimensiones de grilla activa (`cols×rows`) y badge en tiempo real del agente de IA activo (`● CLAUDE`, `● AGY`, `● GEMINI`, `● AIDER`, `● COPILOT`, `● OLLAMA` o `○ LOCAL`).
* **Soporte de Modo Claro (Catppuccin Latte) y Sincronización con Windows 11:**
  - Integración del tema claro oficial Catppuccin Latte (`#eff1f5`, `#4c4f69`).
  - Con `theme_mode = "system"`, Axon consulta dinámicamente el Registro de Windows (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`) actualizando la interfaz y la barra de título DWM en tiempo real cuando el sistema operativo cambia entre tema claro y oscuro.

### 13. Explorador Lateral Derecho de Archivos (Dev-First, Click-to-Prompt y Ctrl+Shift+E)
* **Panel derecho integrado (`FILE_EXPLORER_WIDTH = 240`):** Implementado en [`src/explorer.rs`](file:///C:/Users/gmora/dev/axon/src/explorer.rs) y renderizado en [`render_file_explorer`](file:///C:/Users/gmora/dev/axon/src/renderer.rs). Muestra el árbol de archivos y carpetas del proyecto activo sin emojis, utilizando glifos `◈`, `▸`, `▾`, `↻`, `x`, `..`.
* **Click-to-Prompt:** Al hacer clic en cualquier archivo, su ruta relativa (o entre comillas si contiene espacios) se inyecta directamente en el prompt del PTY activo, permitiendo interacción instantánea con shells o agentes CLI (Claude Code, agy, gemini).
* **Navegación completa y scroll:** Clic en directorios expande/colapsa carpetas conservando el árbol, `..` sube al directorio padre, `↻` recarga el disco y la rueda del ratón permite scroll suave en repositorios grandes.
* **Control y persistencia unificada:** Conmutable mediante el atajo canónico `Ctrl + Shift + E`, el botón `ARCHIVOS` de la barra de estado inferior, el botón de archivos de la barra superior horizontal, o la opción en Ajustes (`Ctrl + ,`). Su visibilidad se persiste en `%LOCALAPPDATA%\axon\session.toml`.

### 14. Diseño Minimalista Floating Island, Iconografía Lucide y Tipografía Inter
* **Sub-sistema UI Nativo (`src/ui/`):**
  - [`src/ui/paint.rs`](file:///C:/Users/gmora/dev/axon/src/ui/paint.rs): Primitivas de dibujo 2D aceleradas por CPU para `softbuffer`: `draw_rounded_rect` (con antialiasing suave y mezcla alpha 0-255) y `stroke_rounded_rect`.
  - [`src/ui/icons.rs`](file:///C:/Users/gmora/dev/axon/src/ui/icons.rs): Catálogo y mapeo estricto del enum `LucideIcon` a los puntos de código Unicode de la tipografía vectorial embebida [`assets/fonts/lucide.ttf`](file:///C:/Users/gmora/dev/axon/assets/fonts/lucide.ttf).
* **Tipografía Dual de Alta Fidelidad:**
  - *Área de terminal:* **JetBrains Mono Nerd Font** (monoespaciada con métricas precisas para la grilla VT y programación).
  - *Interfaz gráfica:* **Inter** ([`Inter-500.ttf`](file:///C:/Users/gmora/dev/axon/assets/fonts/Inter-500.ttf) y [`Inter-600.ttf`](file:///C:/Users/gmora/dev/axon/assets/fonts/Inter-600.ttf)), proporcionando una legibilidad moderna, limpia y proporcionada en pestañas, explorador, barra de estado y página de ajustes.
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

---

## 🧪 Pruebas y Validación

Antes de dar por completado cualquier cambio:

1. **Ejecutar suite de tests:**
   ```powershell
   cargo test
   ```
2. **Validar integración PTY:**
   `src/pty.rs` contiene pruebas con procesos reales de `powershell.exe` dentro de ConPTY. Si modificas entrada de teclado o secuencias VT, añade o valida los tests correspondientes allí.
3. **Compilar en Release:**
   ```powershell
   cargo build --release
   ```
4. **Verificar diagnósticos:**
   Revisa [`axon.log`](file:///C:/Users/gmora/dev/axon/axon.log) en modo de desarrollo (`cargo run`) si necesitas depurar secuencias de escape recibidas o eventos de hardware emitidos.
