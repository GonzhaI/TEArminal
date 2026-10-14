<div align="center">
  <img src="assets/icon-256.png" width="120" height="120" alt="Axon" />
  <h1>Axon</h1>
  <p><em>Terminal nativa para Windows, pequeña y rápida, hecha para PowerShell y agentes de IA en la CLI</em></p>
</div>

**Axon** es un emulador de terminal escrito en Rust, solo para Windows (ConPTY) y optimizado para **PowerShell**. Sin Electron, sin WebView, sin framework de GUI: ventana nativa (`winit`), render por software (`softbuffer` + `fontdue`) y emulación VT/ECMA-48 propia sobre `vte`. Está pensada para trabajar con agentes de CLI (*Claude Code, Gemini CLI, Aider, OpenCode…*) y sesiones SSH.

| | Axon 0.5 |
| :--- | :--- |
| Binario `axon.exe` (release) | **~4,2 MB** |
| RAM en reposo (working set) | **~47 MB** (privada ~34 MB) |
| CPU en reposo | **< 1 %** (el bucle de eventos duerme; solo el parpadeo del cursor despierta 2 veces por segundo) |
| Fotograma completo, panel 1920×1080 con texto denso | **~1,3 ms** de CPU |
| Presentar el fotograma (`present`) | **~0,3 ms** |
| Parseo VT (un hilo, texto continuo) | **~68 MB/s** |
| Coste del prompt de PowerShell | **~2,4 ms** (era ~42 ms con `git` por prompt) |

> Medido en release en Windows 11. El benchmark de render y de throughput se reproduce con `cargo test --release bench_ -- --nocapture`.

---

## Por qué render por software (y no GPU)

Un fotograma completo cuesta ~1,3 ms en CPU a 1080p (≈ 750 fps) y presentarlo ~0,3 ms: muy por debajo del presupuesto de 16,6 ms de 60 Hz. El cuello de botella real al volcar mucha salida es ConPTY/conhost (~2 MB/s), no el dibujo, así que una ruta GPU no haría la terminal más rápida y sí costaría memoria (los runtimes de D3D/Vulkan suman decenas de MB de RAM), arranque y binario. Axon prioriza **latencia baja, poca RAM y 0 % de CPU en reposo**. Si en el futuro aparece un caso donde el render domine (4K a alta frecuencia con scroll continuo), el diseño apunta a una capa GPU solo para la rejilla de celdas; hasta que se mida, no se añade.

---

## Características

* **Nativo y ligero:** `winit` + `softbuffer`, ventana sin bordes con controles integrados, Mica/Dark Mode de Windows 11 (DWM) y tema claro/oscuro sincronizable con el sistema.
* **Tipografía:** JetBrains Mono Nerd Font (iconos Nerd cargados bajo demanda) en la rejilla e Inter + iconos Lucide en la interfaz.
* **Diseñada para agentes de IA:**
  * **Pegado inteligente de imágenes** (`Ctrl+V`): guarda el PNG del portapapeles y pega su ruta; `Alt+V` para los chips de imagen de Claude Code.
  * **Marcas semánticas OSC 133:** salta entre prompts con `Ctrl+Shift+↑/↓` y copia la salida del último comando con `Ctrl+Shift+O`.
  * **Entrada anclada al fondo** para `claude`, `gemini` y `agy`, e indicador del agente activo en la barra de estado.
* **SSH integrado:** al dividir un panel con SSH activo, el nuevo panel reconecta al mismo host y ruta; las imágenes pegadas se transfieren con `scp.exe` nativo; el portapapeles remoto funciona vía OSC 52 (Neovim, tmux, vim).
* **Motor VT completo:** TrueColor (`;` y `:`), ED/EL/ECH/DCH/ICH con BCE, pantalla alterna (tmux, vim, btop), regiones de scroll, ratón SGR 1006/1002, *bracketed paste*, renderizado sincronizado DEC 2026, imágenes inline (iTerm2 / OSC 1337) y doble ancho (CJK, emoji).
* **Pestañas y paneles:** pestañas en isla flotante horizontal o barra lateral vertical, paneles divididos, restauración de sesión y detección de directorio (OSC 7 / OSC 9;9).
* **Explorador de archivos** lateral (`Ctrl+Shift+E`): clic en un archivo inserta su ruta en el prompt activo.
* **Ajustes en pestaña propia** (`Ctrl+,`): temas (Catppuccin Frappé/Latte, Ghostty, One Dark, Tokyo Night, Monokai Pro, Nord, Solarized Dark), fuentes, opacidad, interlineado, atajos.
* **Teclado correcto en Windows:** `Backspace` (`0x7f`), `Ctrl+Backspace` (`0x17`), `Alt+Backspace` (`ESC DEL`), `AltGr` sin Ctrl atascado y `Alt+Tab` sin tabuladores fantasma.

---

## Atajos de teclado

| Atajo | Acción |
| :--- | :--- |
| `Ctrl + ,` | Abrir ajustes (`Esc` vuelve a la terminal) |
| `Ctrl + T` / `Ctrl + W` | Nueva pestaña / cerrar pestaña |
| `Ctrl + Tab`, `Ctrl + 1…9` | Siguiente pestaña / ir a la pestaña N |
| `Ctrl + B` | Mostrar/ocultar la barra lateral |
| `Ctrl + Shift + E` | Mostrar/ocultar el explorador de archivos |
| `Ctrl + Alt + D` / `Ctrl + Alt + V` | Dividir panel horizontal / vertical |
| `Ctrl + Alt + W` | Cerrar panel activo |
| `Ctrl + Alt + ←↑↓→` | Moverse entre paneles |
| `Ctrl + Shift + C` / `Ctrl + V` | Copiar / pegar (texto o imagen) |
| `Alt + V` | Enviar `Alt+V` (imágenes en Claude Code) |
| `Ctrl + Shift + K` | Limpiar pantalla y scrollback |
| `Ctrl + Shift + O` | Copiar la salida del último comando |
| `Ctrl + Shift + ↑ / ↓` | Saltar entre prompts (OSC 133) |
| `Ctrl + +` / `Ctrl + -` / `Ctrl + 0` | Aumentar / reducir / restablecer fuente |
| `Ctrl + Shift + ,` | Recargar `axon.toml` |
| `Shift + PageUp / PageDown` | Desplazar el historial |

---

## Instalación

Descarga la última versión desde [Releases](https://github.com/GonzhaI/axon/releases):

* **`axon-setup-X.Y.Z.exe`** (recomendado): instalador por usuario, **sin permisos de administrador**, en `%LOCALAPPDATA%\Programs\Axon`. Crea el acceso en el menú Inicio, se actualiza instalando encima y se desinstala desde *Aplicaciones instaladas*. Disponible en español e inglés.
* **`axon-X.Y.Z-windows-x64.zip`**: versión portable; descomprime y ejecuta `axon.exe`.
* `SHA256SUMS.txt`: sumas de verificación de ambos archivos.

Todo lo necesario va incluido; no hay nada más que instalar:

| Incluido | Detalle |
| :--- | :--- |
| `axon.exe` autónomo | CRT estático (sin Visual C++ Redistributable) y fuentes embebidas |
| Herramientas CLI en `bin\` | `eza` 0.23.5, `bat` 0.26.1, `ripgrep` 15.2.0, `fd` 10.5.0, `zoxide` 0.10.0 y `fzf` 0.74.5, con sus licencias en `licenses\`. Se añaden al **final** del PATH de las terminales de Axon: las versiones que ya tengas instaladas tienen prioridad. Son opcionales; sin ellas Axon usa su `ls` propio y los comandos de PowerShell |
| PowerShell | Axon usa el que ya tiene Windows (5.1) o PowerShell 7 si está instalado |

Requisitos: Windows 10 versión 1809 (build 17763) o superior, x64 (ConPTY viene en el sistema). Si ConPTY no está disponible, Axon muestra un mensaje de error en vez de cerrarse sin avisar.

> El instalador aún no está firmado digitalmente, así que SmartScreen puede mostrar un aviso la primera vez (*Más información → Ejecutar de todas formas*). Las herramientas incluidas se descargan en la build con versión y SHA-256 fijos (`packaging/tools.json`) y nunca en tiempo de ejecución.

## Compilar desde el código

Requiere [Rust](https://rustup.rs/) estable.

```powershell
git clone https://github.com/GonzhaI/axon.git
cd axon
cargo build --release
.\target\release\axon.exe
```

Para generar el instalador y el ZIP (requiere [Inno Setup 6](https://jrsoftware.org/isdl.php) y Visual Studio Build Tools con C++, de donde sale `vcruntime140.dll` para `bat`):

```powershell
pwsh packaging/package.ps1          # dist\axon-setup-X.Y.Z.exe, dist\axon-X.Y.Z-windows-x64.zip y SHA256SUMS.txt
pwsh packaging/test-installer.ps1   # instala en silencio, ejecuta las herramientas y Axon, y desinstala
```

### Compatibilidad con PowerShell

Axon usa `pwsh.exe` 7 si está instalado en `C:\Program Files\PowerShell\7\` y, si no, `powershell.exe` 5.1 (incluido en Windows). El script de inicio (`assets/shell/init.ps1`) se prueba en CI contra **ambas versiones** en `windows-2022` y `windows-latest`: prompt con OSC 7/133, rama git, `ls`, búsqueda de programas y que no registre ningún error en `$Error`. Otra shell (`cmd`, WSL…) funciona como terminal normal, sin las funciones de Axon. Si ConPTY no está disponible, Axon muestra un mensaje de error en vez de cerrarse sin avisar.

---

## Configuración (`axon.toml`)

Axon busca `axon.toml` en el directorio actual, junto al ejecutable o en `%APPDATA%\axon\axon\config`; si no existe lo genera con los valores por defecto. Se puede editar a mano o desde la pestaña de ajustes (`Ctrl + ,`), y se recarga con `Ctrl + Shift + ,`.

[`examples/axon.toml`](examples/axon.toml) documenta todas las claves: tema y modo (`dark` / `light` / `system`), fuentes, opacidad, distribución de pestañas, barra de estado, shell por defecto, prompt de Axon, scrollback, etc. Si `default_shell` no existe, Axon usa `powershell.exe`.

---

## Estructura del proyecto

```text
src/          app/ (estado y eventos de winit), terminal/ (emulador VT), renderer/ (render por software),
              pty.rs (ConPTY), explorer.rs, theme.rs, config.rs, tools.rs, ui/
assets/       fuentes reducidas embebidas, icono y assets/shell/init.ps1 (el script de PowerShell que Axon inyecta)
packaging/    instalador (Inno Setup), herramientas CLI con hashes fijos, winget y scripts de empaquetado
tools/        herramientas de desarrollo: subset-fonts (recorta fuentes) y make-icon.ps1
examples/     axon.toml de ejemplo y demos
.github/      CI (tests con PowerShell 5.1 y 7, empaquetado) y publicación de releases
```

Guía técnica y reglas de rendimiento para contribuir: [`AGENTS.md`](AGENTS.md).

---

## Pruebas

```powershell
cargo test                                   # unitarias + integración con ConPTY real
cargo test --release bench_ -- --nocapture   # benchmarks de render y de throughput VT
```

---

## Licencia

MIT.
