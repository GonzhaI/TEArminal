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

### 2. El Bug del Control Sintético con `AltGr` en Win32
* En distribuciones de teclado en español/latinoamericanas, presionar `AltGr` (`VK_RMENU`) inyecta a nivel de driver un `VK_LCONTROL` sintético.
* Al soltar `AltGr`, Windows frecuentemente pierde o no emite el evento *Key-Up* de ese Control sintético, dejando el modificador `Ctrl` atascado permanentemente en los frameworks de ventanas como `winit`.
* **Regla obligatoria:** En [`src/app.rs`](file:///C:/Users/gmora/dev/axon/src/app.rs), la consulta de teclas modificadoras se realiza mediante [`get_physical_modifiers()`](file:///C:/Users/gmora/dev/axon/src/app.rs) con `GetAsyncKeyState`:
  * `VK_LMENU` = Alt izquierdo real.
  * `VK_RMENU` = AltGr.
  * `Ctrl` solo se considera activo si `VK_RCONTROL` está presionado, o si `VK_LCONTROL` está activo **sin** que `VK_RMENU` (AltGr) esté presente.

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
