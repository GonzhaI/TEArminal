<div align="center">
  <img src="assets/logo.png" width="120" height="120" alt="Axon" />
  <h1>Axon</h1>
  <p><em>Terminal nativa para Windows, pequeña y rápida, hecha para PowerShell y agentes de IA en la CLI</em></p>
</div>

**Axon** es un emulador de terminal en Rust solo para Windows (ConPTY) y optimizado para **PowerShell**. Sin Electron ni WebView: ventana nativa (`winit`), dibujo con Direct3D 11 (la GPU, o el rasterizador por software WARP si no hay), texto con `rustybuzz` + `fontdue` y emulación VT propia sobre `vte`. Pensada para agentes de CLI (*Claude Code, Gemini CLI, Aider, OpenCode…*) y sesiones SSH.

| | |
| :--- | :--- |
| `axon.exe` (release) | **~5,9 MB** |
| RAM en reposo | **~111 MB** (privada ~126 MB) |
| CPU en reposo | **< 1 %** |
| Fotograma 1920×1080 con texto denso | **~0,3 ms** de CPU |

> Medido en release en Windows 11 (`cargo test --release bench_ -- --nocapture`). Son ejecuciones únicas en una sola máquina: la RAM y la CPU en reposo, con la sesión restaurada (2 pestañas, un panel dividido y el explorador), tras 8 s de arranque y 30 s sin tocar la ventana (CPU en tres ejecuciones: 0 a 0,7 % de un núcleo); el fotograma es la CPU que cuesta grabar la escena y enviarla a la GPU (`bench_gpu_encode_and_submit_full_frame`), no el tiempo de la GPU. La escena se graba en la CPU y la dibuja Direct3D 11 (hardware, o el rasterizador por software WARP si no hay GPU o falla el controlador); el texto se compone y rasteriza en la CPU (`rustybuzz`, `fontdue`). `AXON_RENDERER=warp` fuerza WARP. El cuello de botella al volcar salida es ConPTY, no el dibujo.

## Características

* **Para agentes de IA:** pegado de imágenes (`Ctrl+V` guarda el PNG y pega su ruta; `Alt+V` para Claude Code), marcas OSC 133 (`Ctrl+Shift+↑/↓` entre prompts, `Ctrl+Shift+O` copia la última salida), entrada anclada al fondo para `claude`/`gemini`/`agy` e indicador del agente activo.
* **SSH:** los paneles divididos reconectan al mismo host; con bash en el servidor, la rama git, la carpeta y los archivos aparecen solos en el explorador, también con contraseña (se desactiva en Ajustes); portapapeles remoto por OSC 52 (Neovim, tmux).
* **Visor y editor de archivos** en pestaña: un clic abre una vista previa, doble clic la fija; resaltado de sintaxis, Markdown renderizado, plegado de bloques, ajuste de línea (`Alt+Z`), diff git en el margen y edición local o remota.
* **Menú contextual** (clic derecho) según el elemento: visor, terminal, pestaña, explorador.
* **Motor VT completo:** TrueColor, pantalla alterna, ratón SGR, *bracketed paste*, imágenes inline (Kitty Graphics Protocol e iTerm2), doble ancho (CJK, emoji) y *grapheme clustering* (UAX #29).
* **Multi-ventana y desprendimiento de pestañas:** abre nuevas ventanas con `Ctrl+Shift+N` o mueve pestañas a una nueva ventana mediante clic derecho (*«Mover a nueva ventana»*) o arrastrándolas fuera de la barra (*tab tear-off*).
* **Auto-actualización automática:** comprueba en segundo plano nuevas versiones desde GitHub Releases, avisa en la barra de estado e instala la actualización con 1 clic (solo si el SHA-256 del instalador coincide con el de `SHA256SUMS.txt` de la misma release; si no, abre la página de la versión).
* **Interfaz y renderizado:** pestañas horizontales o laterales, paneles divididos, explorador de archivos, 817 temas de iTerm2 (por defecto `Catppuccin Frappe` y `Catppuccin Latte`) (los `.itermcolors` originales de iterm2-color-schemes, sin retoques), toda la interfaz y el prompt coloreados con el tema elegido, dibujo por GPU con Direct3D 11 (DirectFlip; WARP de respaldo), ligaduras tipográficas y restauración de sesión.
* **Teclado correcto en Windows:** `AltGr` sin Ctrl atascado, `Ctrl+Backspace` borra palabra y `Alt+Tab` sin tabuladores fantasma.

## Atajos

| Atajo | Acción |
| :--- | :--- |
| `Ctrl+,` | Ajustes |
| `Ctrl+T` / `Ctrl+W` | Nueva pestaña / cerrar |
| `Ctrl+Shift+N` | Nueva ventana |
| `Ctrl+Tab`, `Ctrl+1…9` | Siguiente pestaña / ir a la N |
| `Ctrl+B` / `Ctrl+Shift+E` | Barra lateral / explorador |
| `Ctrl+Alt+D` / `Ctrl+Alt+V` / `Ctrl+Alt+W` | Dividir horizontal / vertical / cerrar panel |
| `Ctrl+Alt+←↑↓→` | Moverse entre paneles |
| `Ctrl+Shift+C` / `Ctrl+V` | Copiar / pegar (texto o imagen) |
| `Ctrl+Shift+K` | Limpiar pantalla y scrollback |
| `Ctrl+Shift+O` / `Ctrl+Shift+↑↓` | Copiar última salida / saltar entre prompts |
| `Ctrl++` `Ctrl+-` `Ctrl+0` | Tamaño de fuente |
| `Ctrl+Shift+,` | Recargar `axon.toml` |
| `Shift+PageUp/PageDown` | Historial |

En el visor: `Ctrl+S` guardar, `Ctrl+E` lectura/edición, `Ctrl+Z/Y` deshacer/rehacer, `Alt+Z` ajuste de línea, `Esc` cerrar.

## Instalación

Descarga desde [Releases](https://github.com/GonzhaI/axon/releases):

* **`axon-setup-X.Y.Z.exe`** (recomendado): instalador por usuario, sin administrador, en `%LOCALAPPDATA%\Programs\Axon`; se actualiza instalando encima.
* **`axon-X.Y.Z-windows-x64.zip`**: portable. `SHA256SUMS.txt` verifica ambos.

Incluye `axon.exe` autónomo (CRT estático, fuentes embebidas) y las herramientas opcionales `eza`, `bat`, `ripgrep`, `fd`, `zoxide` y `fzf` en `bin\` (al final del PATH: tus versiones tienen prioridad; versiones y licencias en `packaging/tools.json` y `licenses\`). Usa PowerShell 7 si está instalado y, si no, el 5.1 de Windows.

Requisitos: Windows 10 1809 o superior, x64. El instalador aún no está firmado, así que SmartScreen puede avisar la primera vez (*Más información → Ejecutar de todas formas*).

## Compilar

Requiere [Rust](https://rustup.rs/) estable.

```powershell
git clone https://github.com/GonzhaI/axon.git
cd axon
cargo build --release
.\target\release\axon.exe
```

Instalador y ZIP (requieren [Inno Setup 6](https://jrsoftware.org/isdl.php) y Visual Studio Build Tools con C++):

```powershell
pwsh packaging/package.ps1          # dist\axon-setup-X.Y.Z.exe, el ZIP y SHA256SUMS.txt
pwsh packaging/test-installer.ps1   # instala en silencio, prueba y desinstala
```

Pruebas: `cargo test` (incluye PowerShell 5.1 y 7 reales sobre ConPTY) y `cargo test --release bench_ -- --nocapture`.

## Configuración

`axon.toml` se busca junto al ejecutable o en `%APPDATA%\axon\axon\config` (se genera si falta). Solo en compilaciones de desarrollo (debug) se lee también el del directorio actual: en release se ignora para que un repo ajeno no pueda elegir qué programa se ejecuta. Se edita a mano o desde Ajustes (`Ctrl+,`) y se recarga con `Ctrl+Shift+,`. [`examples/axon.toml`](examples/axon.toml) documenta todas las claves.

**Red y credenciales:** Axon hace dos llamadas por su cuenta. La comprobación de versiones consulta la API de GitHub Releases (se apaga con `terminal.auto_check_updates = false`). Las cuotas de Claude que muestra la barra de estado se piden a `api.anthropic.com/api/oauth/usage` con el token de sesión de Claude Code (`~/.claude/.credentials.json`, solo lectura: nunca se renueva, no se registra ni se muestra); `terminal.ai_limits = false` evita leer esa credencial y hacer la llamada.

Con `terminal.shell_integration = true` (por defecto) Axon inyecta su script en PowerShell: marcas OSC 7/133, rama git, `ls` con iconos y resaltado de sintaxis, todo con colores de la paleta ANSI del tema elegido. Con `false` el shell arranca limpio, sin nada inyectado.

## Contribuir

Arquitectura, reglas de rendimiento y trampas de Windows: [`AGENTS.md`](AGENTS.md). Antes de enviar cambios: `cargo test`, `cargo fmt --check` y `cargo clippy --all-targets` sin avisos.

## Licencia

Sin licencia definida por ahora: el código no se distribuye bajo una licencia de código abierto (todos los derechos reservados por defecto). Las herramientas CLI incluidas conservan sus propias licencias en `licenses\`.
