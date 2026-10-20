# AGENTS.md — Guía para agentes de IA y contribuidores de Axon

Terminal nativa para Windows en Rust (ConPTY, `winit`, render por software). Este archivo recoge solo lo que no se deduce del código: arquitectura, reglas y trampas de la plataforma.

## Arquitectura

```text
src/
├── main.rs        # carga config, fija AXON_SSH_INTEGRATION y arranca el EventLoop
├── app/           # AxonApp y bucle de winit, un archivo por responsabilidad:
│   host.rs (AxonHost: ventanas, único que toca el ActiveEventLoop) · mod.rs (estado, sesión) · handler.rs (despacho de eventos, about_to_wait) · redraw.rs (un fotograma)
│   keyboard.rs · cursor.rs · click.rs · wheel.rs (entrada) · tabs.rs · settings.rs · clipboard.rs
│   layout.rs (paneles, TerminalPane/TerminalTab) · viewer.rs (FileViewerState) · context_menu.rs
│   remote_files.rs (peticiones en banda por SSH) · shell_context.rs · platform.rs (Win32) · keys.rs · tests.rs
├── pty.rs         # PtyManager (ConPTY vía portable-pty), hilos I/O
├── terminal/      # emulador VT: mod.rs (TerminalState), performer.rs (vte → estado), cell.rs
├── renderer/      # render CPU sobre framebuffer u32; una pantalla por archivo (pane, sidebar, top_bar,
│                  # status_bar, explorer_panel, settings_page, file_viewer, md_preview, context_menu);
│                  # geometry.rs = rectángulos compartidos por dibujo y clics; state.rs; glyphs.rs; draw.rs; d3d11.rs (presentación GPU DirectFlip)
├── explorer.rs · theme.rs · config.rs (axon.toml + sesión) · tools.rs · logger.rs · updater.rs
└── ui/            # paint.rs (rect redondeado SDF) e icons.rs (enum LucideIcon)
assets/            # shell/init.ps1 (script que Axon inyecta), fuentes reducidas, iconos
packaging/         # Inno Setup, tools.json (herramientas CLI con SHA-256 fijos), winget, scripts
tools/             # subset-fonts (recorta fuentes), make-icon.ps1 y fetch-themes.ps1 (assets/themes.txt)
```

## Principios

1. **Nativo Windows:** sin Electron/WebView/GUI pesada; integración con el SO vía `windows-sys`.
2. **VT/ECMA-48 estricto** (xterm).
3. **Teclado sin suposiciones:** valida el estado físico de los modificadores.
4. **Pensado para agentes de CLI** (Claude Code, Aider…): imágenes pegadas, OSC 133.

## Trampas de la plataforma (leer antes de tocar teclado o VT)

* **Borrado y salto de línea:** Backspace = `0x7f`; `Ctrl+Backspace` = `0x17` (no `0x08`: los harnesses raw lo leen como 1 carácter); `Alt+Backspace` = `ESC DEL`; `Shift+Enter` / `Alt+Enter` = `ESC CR` (`\x1b\r`, salto de línea en agentes CLI y `AddLine` en PSReadLine).
* **AltGr:** Windows inyecta un `VK_LCONTROL` sintético y a veces no emite su key-up. Consulta modificadores con `get_physical_modifiers()` (`GetAsyncKeyState`), no con `winit`. `Ctrl` activo solo si `VK_RCONTROL`, o `VK_LCONTROL` sin `VK_RMENU`. AltGr sin texto imprimible = `Ctrl+Alt`.
* **Atajos con Alt:** `winit` da `Key::Unidentified`; comprueba la tecla física con `key_is_char`.
* **Foco:** al cambiar de foco `modifiers` se resetea; `Alt+Tab` nunca emite `\t`. El reporte de foco (modo 1004) está desactivado: ConPTY lo convierte en Enter/Escape. `handler.rs` descarta `KeyboardInput { is_synthetic: true }` (winit reenvía teclas aún pulsadas al recuperar el foco).
* **ECH/ICH/DCH (`X`, `@`, `P`) deben implementarse** o queda texto fantasma. **BCE:** ED/EL/ECH rellenan con `current_bg`, no con transparente. **SGR 7:** intercambia fg y bg en `compute_cell_fg`/`effective_bg`. **ED 3** vacía scrollback, `scroll_offset` y `command_blocks`.
* **Logs:** en release `axon.log` solo con `AXON_DEBUG=1`/`AXON_LOG=1`. Nunca se registra el texto tecleado (`AXON_LOG_KEYS=1` solo guarda longitudes).
* **Directorio y sesión:** OSC 7 y OSC 9;9 actualizan el cwd; las pestañas se persisten en `%APPDATA%\axon\axon\config\session.toml`.
* **Entrada anclada al fondo** (`claude`, `gemini`, `agy`): lo hace `Invoke-AxonAgent` en `init.ps1`.
* **PTY:** `PtyManager` implementa `Drop` y mata el shell (si no, quedan `pwsh` huérfanos de ~100 MB): no lo quites. El resize al PTY va con antirrebote (`resize_deadline`, 120 ms): cada resize de ConPTY hace que PowerShell repinte el prompt, así que no llames `recompute_grid_for_all_tabs` por evento de tamaño.
* **Secuencias OSC:** el `Parser` de `vte` (feature `no_std` por defecto) limita cada OSC a 1 KB y descarta el resto en silencio; Axon lo crea con `Parser<OSC_BUF_SIZE>` (128 KB, en `Box`) para que quepan los listados `AxonLs`, los trozos de `AxonFile` (32 KB) y OSC 52. Un `head -N` remoto nuevo debe seguir cabiendo ahí (en hex: 2 000 entradas ≈ 80 KB); `ssh.rs` (sin OSC) sí admite `LISTING_ENTRIES` = 5 000.

## Rendimiento y recursos (medir antes de fusionar: `cargo test --release bench_ -- --nocapture`)

* **fontdue guarda todos los contornos en RAM.** Las fuentes embebidas están reducidas (`tools/subset-fonts`; completas en `assets/fonts/src/`, no se embeben). **Al añadir un `LucideIcon`: `cargo run --release --manifest-path tools/subset-fonts/Cargo.toml`** (si no falla `test_all_lucide_glyphs_exist_in_font`). Negrita, bloques Nerd y fallback son `OnceCell` perezosos: nunca en `new()`.
* **Un solo blitter:** glifos con `blit_glyph`, rellenos con `fill_rect` (recorte previo + `slice::fill`). Cachés con `FastMap` (no `HashMap`).
* **`Cell` = 16 bytes** (`flags: u8`, `Copy`). Rejillas `VecDeque`; la fila expulsada del scrollback se recicla. Defaults: `scrollback_lines` 10 000, `command_blocks` ≤ 512.
* **PTY:** el lector coalesce eventos (un `PtyData` por ráfaga); consume con `try_recv_output()` hasta `None`. Hilos solo vía `spawn_io_threads`.
* **Bucle de eventos:** `about_to_wait` calcula UN próximo despertar (`earliest(...)`) y usa `ControlFlow::Wait` si no hay (0 % CPU en reposo). Un temporizador nuevo se suma ahí; no llames `set_control_flow`. Sin foco u ocluida no se renderiza ni parpadea el cursor.
* **Render y presentación:** Render por software de la escena (~1,3 ms/fotograma a 1080p) con presentación nativa por GPU Direct3D 11 / DXGI Flip-Model (`terminal.gpu = true`, DirectFlip en Windows 11 sin tearing ni sobrecoste de GDI) y fallback automático a `softbuffer`. Pendiente a propósito: repintado parcial por filas sucias.
* **`init.ps1` es ruta caliente:** prohibido lanzar procesos en `prompt` (`git rev-parse` = 42 ms; leer `.git/HEAD` = 2 ms); no usar `Get-Command` para buscar programas (usa `Find-AxonApp`, memoizada). Colores solo de la paleta ANSI (`[31m`…`[37m`, `[90m`…`[97m`), nunca RGB fijo: así siguen el tema activo. Edita el `.ps1`, no lo incrustes en Rust; valida en `powershell.exe` 5.1 **y** `pwsh.exe` 7. `test_init_script_in_every_installed_powershell` exige prompt OSC 7/133, `ls` y `$Error.Count == 0` (usa `-ErrorAction Ignore`). El script va en la línea de comandos de cada `pwsh`: **nada ofuscado** (base64, `eval`, `-EncodedCommand`) porque los antivirus marcan `pwsh.exe`.

## UI y visor

* **Geometría única** para dibujo y clics: `TopTabBarRects`, `SettingsPageRects`, `FileViewerLayout` en `renderer/geometry.rs`. No la recalcules en `click`/`cursor`.
* **Barras de scroll** (`draw_scrollbar`/`draw_scrollbar_rects`): auto-ocultas; visibles con el ratón encima o mientras `scrollbars_awake` (1,25 s desde `last_scroll_time`). Toda rueda/arrastre debe refrescarlo.
* **Ajustes:** el contenido bajo la navegación se dibuja fuera de pantalla y se copia la ventana visible; `SettingsPageRects::{CONTENT_TOP, content_height, max_scroll, scrollbar}` son la fuente única y clic/hover traducen `my` con `settings_scroll()`. Si cambias el alto de una tarjeta, actualiza `content_height`.
* **Cursor:** parpadeo con estado explícito `cursor_on` (nunca derivado de `cursor_timer.elapsed()`).
* **Visor:**
  * **Guardado local:** `disk_mtime` guarda la fecha del archivo al abrirlo/guardarlo; si en disco es otra (un agente lo cambió) el autoguardado se detiene y Ctrl+S avisa y exige repetirlo para sobrescribir. `text_for_disk()` (fin de línea y salto final originales) lo usan el guardado local y el remoto.
  * `git status` del explorador: siempre `--no-optional-locks` (si no, bloquea el `git add`/`commit` del agente) y `-z`; solo una consulta en vuelo (`git_status_running`).
  * `FileViewerState::is_editable()` es la única barrera de mutación en modo lectura (dentro de cada mutador, no en los atajos).
  * `rows()` (caché por `edit_gen`/`fold_gen`/ancho/modo) une ajuste de línea (`terminal.word_wrap`, Alt+Z) y plegado (sangría; títulos en Markdown). `scroll_offset` es índice de FILA: usa `rows()`, `row_of`, `row_text`, `hit_test`. Todo mutador sube `edit_gen`. El cursor nunca queda en una línea oculta (`reveal_line`).
  * La vista previa de Markdown (`md_preview.rs`) usa iconos Lucide; la selección no tiene esquinas redondeadas y marca espacios/tabuladores.
  * **Un clic** en un archivo del explorador abre una pestaña de **vista previa** (`is_preview`, título subrayado) que la siguiente vista previa reemplaza (`install_viewer_tab`); **doble clic** (en el archivo o en la pestaña), Enter, «Abrir» o editar la **fija**. Una vista previa con cambios nunca se reemplaza.
* **Consumo de IA** (popup de la barra de estado, `app/ai_usage.rs`): un bloque por agente distinto de la pestaña (`active_agents`, el panel enfocado primero), cada uno con su modelo, plan y ventanas; los tokens de AGY y Claude nunca se suman. El modelo sale de la transcripción **más reciente**. **Nunca se inventa un porcentaje de cuota.** Solo Claude tiene límites reales: `fetch_claude_limits` lee la sesión de Claude Code (`~/.claude/.credentials.json`) y pide `api.anthropic.com/api/oauth/usage` con `curl.exe` (cabeceras por stdin con `-H @-`, para que el token no salga en la lista de procesos), como mucho una vez por minuto y en un hilo. **El token solo se lee: nunca se renueva** (rotaría el refresh token y cerraría la sesión de Claude Code) y no se registra. Es la única petición de red en ejecución además del actualizador. AGY no tiene fuente legítima (su cuota no se guarda en disco y su servidor local exige un token CSRF interno; `~/.gemini/oauth_creds.json` es el login de Gemini CLI, no el de AGY): el bloque lo dice con `ai_limits_note`. No intentes leer secretos de otro proceso.
* **Proyectos** (barra lateral vertical): el usuario agrega carpetas (`[[projects]]` en `axon.toml`, `projects.rs`) y una sesión pertenece al proyecto de raíz más profunda que contiene su carpeta (`current_dir`, OSC 7); sin proyecto va a «Sueltas». La pertenencia **se calcula, no se guarda** (`app/projects.rs`: `panel_tab_indices`, `panel_rail`). El panel mide `SIDEBAR_WIDTH` y su geometría (`SidebarRects`) es **local a él**: a la izquierda coincide con la de ventana; a la derecha (`panel_side`) `redraw::draw_panel` lo dibuja en un búfer propio y los manejadores restan `panel_x` con `panel_local_x` (nunca comparan `mx < SIDEBAR_WIDTH`). El espacio que quita al contenido sale de `sidebar_offset()` y `file_explorer_offset()` (inset derecho), y Ajustes recibe `panel_right_inset()`. **El explorador va alojado dentro del panel** en vertical (`FileExplorer::dock`, que fija `sync_explorer_dock` en cada fotograma; su vista «Cambios» ya existía) y solo se acopla al borde derecho con el panel oculto. **Con pestañas superiores** el panel es `render_projects_panel` (geometría `ProjectsPanelRects`: lista de proyectos con rueda y el explorador debajo); el explorador se muestra y oculta con el panel (el botón de archivos y Ctrl+B hacen lo mismo, `toggle_file_explorer` → `toggle_sidebar`), la barra superior lleva el selector de proyecto (`TopTabBarRects::switcher_btn`, un clic pasa al siguiente) y solo las pestañas del proyecto a la vista (`panel_tab_indices`; `SidebarTabItem::index` es la real). `panel_local_x(mx, my, win_w)` excluye el rectángulo del explorador: dentro de él manda el ratón del explorador. «Agregar proyecto» abre el selector de carpetas de Windows (`platform::pick_folder`, crate `windows`). La barra de estado dibuja medidores de límites reales (`StatusMeter`, solo Claude) y los omite si no hay sitio. La lista de sesiones cede espacio al explorador: usa `SidebarRects::list_end`, no `usable_h`. `SidebarTabItem::index` es la posición real en `tabs`; la lista del panel está filtrada, así que clic, hover, rueda y arrastre usan `panel_tab_at`, nunca `tabs.len()`. El estado de una sesión sale solo de OSC 133 (`SessionStatus`): no se inventa «trabajando» ni «pide confirmación».
* **Menú contextual** (clic derecho): `ContextMenu` según el elemento; las TUI que piden el ratón reciben el clic derecho salvo con Shift. Opción nueva = variante de `MenuAction` + entrada en el constructor + brazo en `run_menu_action`. La rueda sobre el menú no actúa; fuera lo cierra.
* **Pestañas:** vertical (sidebar) u horizontal (`tab_layout`); los Ajustes tienen un único botón (barra superior).
* **Tema:** los temas son los `.itermcolors` originales de iterm2-color-schemes (carpeta `schemes/`, no las carpetas generadas por formato: su generador retoca colores por contraste WCAG): `assets/themes.txt` (una línea por tema, generada con `pwsh tools/fetch-themes.ps1`, commit fijado) se embebe con `include_str!` y `Theme::from_name` la interpreta; no hay temas escritos en Rust. Los colores de interfaz se derivan del tema (`theme.surface_bg()`, `accent_color()`…), sin casos por tema. Hay dos ranuras, `theme` (oscuro) y `theme_light` (claro); `theme_mode` (`dark`/`light`/`system`) decide cuál se usa y `AppearanceConfig::resolve_theme` es el único sitio que lo resuelve (con `system`, `AxonApp::system_light` detecta el cambio de Windows en `about_to_wait`; una config antigua con un solo `theme` se migra al cargar). Los nombres se comparan sin distinguir mayúsculas ni separadores; los nombres antiguos (`frappe`, `latte`, `one dark`, `Axon Night`, `Axon Day`…) siguen valiendo y uno desconocido cae en `Theme::DEFAULT_DARK`. Con `theme_mode = "system"` se lee el registro de Windows. El selector de Ajustes (buscador + lista de 6 filas con rueda) usa `SettingsPageRects::{theme_search_box, theme_list, theme_row_at}`.

## SSH

* El wrapper `ssh` de `init.ps1` emite `OSC 1337 AxonSSH=start/end`; solo se acepta con el token del panel (`AXON_PANE_TOKEN`, aleatorio por `PtyManager::new`, adjunto como `;token=`): ni el título ni OSC 7 crean sesiones SSH, solo actualizan el directorio remoto de una ya autenticada. Las conexiones SSH en segundo plano usan `BatchMode` sin `accept-new` (un host sin entrada en `known_hosts` falla en vez de aceptarse a ciegas). Al dividir un panel con SSH se reconecta al mismo host (`PtyManager::new_ssh`); las imágenes pegadas se suben por `ssh.exe` (stdin) a `<TMPDIR|/tmp>/axon-<uid>` (modo 700), nunca a `/tmp` a secas; OSC 52 copia al portapapeles de Windows solo con `terminal.osc52 = true` (apagado por defecto; tope de 100 KB; la lectura `?` siempre se ignora).
* **Integración** (`terminal.ssh_integration`, activa por defecto; env `AXON_SSH_INTEGRATION`, que debe fijarse **antes** de crear la primera pestaña): en sesiones interactivas sin comando remoto el wrapper lanza `ssh -t host "sh -c '...'"` y un bash remoto emite en cada prompt `OSC 1337 AxonGit=<rama>` (`-`/vacía = sin repo), `OSC 7` y `OSC 1337 AxonLs=<hex de ls -1Ap>`. El gancho va en **texto plano**, sin comillas dobles en PS < 7.3 salvo escapadas (`\"`), con las simples escapadas `'\''`, y sin la palabra `base64`.
* **Peticiones en banda** (archivos y carpetas con contraseña, sin segunda conexión): el gancho enlaza `ESC[1337~` con `bind -x`; `TerminalState::remote_request_bytes` escribe `Ctrl+E` + `#AXR#<id> <get|put|ls> <carga en percent-encoding>` + esa secuencia, solo con integración y `remote_at_prompt` (Enter o pegado con salto de línea lo apagan; cada `AxonGit` lo enciende). Responde `OSC 1337 AxonFile=<id>;ok|err;<n>;<total>;<hex>` en trozos de 32 KB. `app/remote_files.rs` lleva las pendientes (6 s; cada una guarda su `pane_id` y solo la resuelve una respuesta de ESE panel; los ids arrancan en un valor aleatorio) y, si vencen, cae a una segunda conexión SSH (solo con clave). El `put` responde con el `cksum` (CRC y tamaño) de lo que escribió y Axon lo compara con lo enviado (`save_matches`): `ok` de `printf` no basta, porque lo que se teclee o llegue durante la petición se mezcla con el contenido; si no coincide, el visor sigue sin guardar y muestra el error. Límites: lectura 512 KB, guardado 96 KB, 64 trozos, 8 respuestas en curso.
* **Vía lenta (segunda conexión SSH, solo con clave):** nunca bloquea el bucle de eventos. `ssh.rs` ejecuta `ssh.exe`/`scp.exe` con `BatchMode`, sin `accept-new`, con salida acotada (`head -c 5242881 -- ruta`, `ls -1ap | head -300`) y desde un hilo (`AxonApp::spawn_ssh` en `app/ssh_jobs.rs`); el resultado vuelve por canal + `AxonEvent::SshDone`. El explorador solo *pide* listados (`take_ssh_requests`, con generación para descartar respuestas viejas); `about_to_wait` los lanza (`pump_explorer_ssh`). Abrir/guardar archivos y subir capturas usan el mismo canal; `FileViewerState::save_to_disk` rechaza los remotos. Una orden SSH nueva va en `ssh.rs` y su resultado en `SshDone`, nunca `cmd.output()` en el hilo de la interfaz.
* Sin integración: scraping de la salida visible (`branch_from_output_line`) y segunda conexión SSH (exige clave). En sesión SSH `get_git_branch()` nunca devuelve la rama de Windows.

## Seguridad: la salida de la terminal es entrada NO confiable

* Destino SSH, directorio remoto, listados y respuestas `AxonFile` los controla el servidor. Valida en el origen (`terminal/performer.rs`) **y** en cada sumidero: `pty::is_safe_ssh_destination` (solo `host`/`usuario@host`, sin `-` inicial), `pty::safe_remote_dir`, `clean_listing` (descarta `/`, control, `.`, `..`) y límites de tamaño; usa `--` antes del destino. Tests: `test_ssh_destination_rejects_injection`, `test_remote_dir_rejects_shell_metacharacters`, `test_untrusted_escape_sequences_cannot_set_unsafe_ssh_session`, `test_remote_*`.
* **Pegado:** `clipboard::sanitize_paste` elimina caracteres de control (ESC incluido) para que el portapapeles no cierre el pegado entre corchetes.

## Distribución

* `packaging/package.ps1` = build + `fetch-tools.ps1` + ZIP + instalador (Inno Setup) + `SHA256SUMS.txt`; `test-installer.ps1` instala en silencio, prueba y desinstala. Ambos corren en CI.
* Las herramientas CLI (eza, bat, rg, fd, zoxide, fzf) están en `packaging/tools.json` con SHA-256 fijos (`fetch-tools.ps1 -Update` los recalcula); licencias en `licenses/`; `bat` necesita `vcruntime140.dll`. Axon no descarga herramientas en ejecución; `bin\` va **al final** del PATH. La única descarga es la del instalador de actualización (`updater.rs`): solo de `github.com/GonzhaI/axon/releases/download/`, con `curl.exe -f --proto =https`, y no se ejecuta si su SHA-256 no coincide con el de `SHA256SUMS.txt` de la misma release.
* **Builds reproducibles:** Rust (`toolchain: 1.99.0`) e Inno Setup (`INNO_VERSION`, `choco --version` + comprobación) van con versión fija en `ci.yml` y `release.yml`; al subir cualquiera, cámbiala en los dos archivos y pasa `packaging/test-installer.ps1`.
* Instalador por usuario; **no cambies el `AppId` de `axon.iss`**. `.cargo/config.toml` enlaza el CRT estático (no lo quites). Un fallo de ConPTY se muestra con `pty::report_spawn_error` (nunca `.expect()` al crear PTYs). Los `.ps1`/`.iss` se guardan en UTF-8 con BOM. Pendiente: firma de código.
* Icono: `pwsh tools/make-icon.ps1`. Winget: `pwsh packaging/winget-manifest.ps1 -Version X.Y.Z`.

## Convenciones

* Un método nuevo de `AxonApp` va en su submódulo temático, no en `mod.rs`; los brazos de `window_event` solo llaman a un método. Un archivo de ~1 000 líneas se divide por responsabilidad.
* Tests junto al código (`src/<módulo>/tests.rs`); no toques la configuración real (`config::dirs_or_fallback()` usa una carpeta temporal en tests).
* En documentación usa rutas relativas. `axon.toml` de la raíz y `*.log` están en `.gitignore` (ejemplo en `examples/axon.toml`).
* Al mover código entre archivos, compara hashes de fotogramas antes y después: `cargo test` no detecta cambios de píxeles.

## Validación antes de dar algo por terminado

```powershell
cargo test
cargo fmt --check; cargo clippy --all-targets
cargo build --release
```

Si tocaste teclado, el script de inicio o secuencias VT, añade o valida tests en `src/pty.rs` (procesos reales en ConPTY, 5.1 y 7) o `src/terminal/tests.rs`. Si tocaste empaquetado: `pwsh packaging/package.ps1` y `pwsh packaging/test-installer.ps1`. Para depurar eventos: `AXON_DEBUG=1` y `axon.log`.
