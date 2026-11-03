# ROADMAP y Auditoría Arquitectónica de TEArminal

Este documento consolida la hoja de ruta (**Roadmap**) para la evolución de TEArminal, evaluando tanto las **nuevas características** a incorporar como las **funciones innecesarias, obsoletas o redundantes** que deben eliminarse o simplificarse para mantener el terminal ligero, rápido y centrado en su propósito.

---

## 1. Principios de Diseño y Filosofía

TEArminal se define por cuatro pilares irrenunciables:
1. **Nativo Windows y ConPTY:** Sin capas pesadas (sin Electron, WebView ni frameworks web). Integración directa con Win32 y DirectWrite vía `windows-sys`.
2. **Renderizado Direct3D 11 puro:** Toda la interfaz se graba vectorialmente en un `Canvas` y se envía a la GPU como quads instanciados (con respaldo en el rasterizador software WARP).
3. **Optimizado para Agentes de CLI:** Adaptado específicamente para flujos de trabajo con agentes como *Claude Code*, *Aider*, *AGY* y *Gemini CLI* (marcas OSC 133, pegado de imágenes, captura de salida).
4. **Presupuesto estricto de recursos:** < 1 % CPU en reposo, ~110 MB RAM, renderizado en < 0,3 ms/frame, y ejecución de subprocesos 100 % silenciosa y en segundo plano (`CREATE_NO_WINDOW`).

---

## 2. Nuevas Características a Implementar

### 2.1. Visor y Editor de Archivos (`src/renderer/file_viewer.rs`, `src/app/viewer.rs`)

* **Búsqueda interna (`Ctrl + F`)** ✅ *[Completado]*:
  * Barra flotante compacta en la esquina superior derecha (`RADIUS_CONTROL`, `Theme::card_outline()`, `Theme::chrome_bg()`).
  * Conteo de coincidencias en tiempo real (`X / N`), navegación con `Enter` / `Shift + Enter` / `F3` y cierre con `Esc`.
  * Resaltado visual con tinte translúcido mezclado sobre el fondo y marcas de posición sobre el carril de la barra de desplazamiento ([`ScrollbarRects`](file:///src/renderer/geometry.rs)).
  * Despliegue automático de bloques plegados ([`FileViewerState::reveal_line()`](file:///src/app/viewer.rs)) al saltar a coincidencias.
* **Modo lectura Markdown adaptable y fluido** ✅ *[Completado]*:
  * Ajuste de línea (`word_wrap`) automático para párrafos, listas, citas y bloques en modo lectura.
  * Formato adaptativo de tablas y contenido con eliminación de barras de desplazamiento horizontal innecesarias.
* **Unificación de inputs de texto y cursor de alta visibilidad** ✅ *[Completado]*:
  * Cursor de 16 px centrado con `theme.cursor` y selección nítida (`theme.selection`, `theme.selection_fg`).
  * Pila de Undo/Redo (50 pasos), soporte completo de teclado (Ctrl+Z, Ctrl+Y, Shift+Flechas, etc.), selección con ratón y Shift+Click en todos los inputs de la interfaz.
* **Historial MRU en cierre de pestañas y ventanas** ✅ *[Completado]*:
  * Cierre de pestañas y ventanas (`Ctrl + W`, `Esc`, `×`) devuelve siempre a la pestaña o ventana más recientemente activa en lugar de un índice adyacente ciego.
* **Salto rápido a línea (`Ctrl + G`)** ✅ *[Completado]*:
  * Input modal para saltar directamente a `línea[:columna]`.
  * Reutiliza [`FileViewerState::reveal_line()`](file:///src/app/viewer.rs) para desplegar bloques plegados si la línea está oculta.
* **Visor nativo de imágenes (`.png`, `.jpg`, `.gif`, `.webp`, `.ico`)** ✅ *[Completado]*:
  * En lugar de mostrar código binario o texto corrupto, decodifica imágenes y las proyecta centradas en el lienzo mediante el atlas de texturas Direct3D 11 (`Canvas::image`), ajustadas al área y ampliadas como máximo 4×. Las transparencias se ven sobre el fondo del visor.
  * Los **GIF animados** se reproducen completos en bucle (`ViewerImage::frames`, fotograma elegido con el reloj del sistema en `current_frame`); el bucle de eventos solo despierta en el próximo fotograma y repinta únicamente cuando cambia.
  * Funciona igual con archivos **remotos** por SSH (`new_remote_file_viewer_from`).
  * Decodificación con los mismos topes que las imágenes del protocolo kitty (`untrusted_limits`); el lado mayor se reduce a 2048 px y un GIF conserva fotogramas hasta 96 MB. Archivos de hasta 20 MB (texto: 5 MB); una imagen dañada cae al aviso de binario.
* **Diff lado a lado (*Side-by-side*)** ✅ *[Completado]*:
  * Conmutador en cabecera para comparar en dos columnas sincronizadas (versión en `HEAD` vs versión en disco), facilitando la revisión de cambios masivos producidos por agentes.
  * Se activa de forma automática al abrir archivos desde la pestaña de **Cambios** del explorador y permite alternar fluidamente entre la vista de dos columnas y el diff unificado mediante un selector dedicado.

### 2.2. Panel Lateral y Explorador (`src/app/panel.rs`, `src/explorer.rs`)

* **Filtro rápido en el árbol (Fuzzy Filter / `Ctrl + P`)** ✅ *[Completado]*:
  * Filtrado difuso en memoria sobre [`FileExplorer::flat_indices`](file:///src/explorer.rs) sin reconsultar el disco ni bloquear el renderizado.
* **Gestión contextual de archivos (Menú contextual)** ✅ *[Completado]*:
  * Opciones: *Nuevo archivo*, *Nueva carpeta*, *Renombrar* y *Eliminar*.
  * La eliminación utiliza la API Win32 (`SHFileOperationW` con `FOF_ALLOWUNDO`, sin dependencias nuevas) para enviar a la **Papelera de Reciclaje**, protegiendo el trabajo del usuario.
* **Arrastrar y soltar (Drag & Drop) hacia la terminal** ✅ *[Completado]*:
  * Soporte nativo de `WindowEvent::DroppedFile` / `WM_DROPFILES` para soltar archivos desde el explorador del sistema de Windows o desde el explorador lateral de TEArminal hacia el panel de terminal bajo el cursor o activo, inyectando su ruta entre comillas (con espacio final) y con compatibilidad total con Bracketed Paste Mode.
  * Si se suelta sobre una pestaña de visor de archivos, abre automáticamente el archivo arrastrado en el visor.
* **Acciones rápidas de Git en pestaña "Cambios"** ✅ *[Completado]*:
  * Botones discretos en cada fila al pasar el cursor para *Stage* (`+` / `git add`), *Unstage* (`-` / `git restore --staged`) y *Discard* (`↩` / `git restore`), ejecutados de forma totalmente oculta (`CREATE_NO_WINDOW`) con recálculo y sincronización inmediata de visores abiertos.

### 2.3. Emulador de Terminal y Paneles (`src/terminal.rs`, `src/pty.rs`, `src/renderer/pane.rs`)

* **Búsqueda en el Scrollback (`Ctrl + Shift + F`)** ✅ *[Completado]*:
  * Búsqueda sin distinguir mayúsculas sobre historial y pantalla (`terminal/search.rs`), navegación con `Enter` / `Shift + Enter` / `F3` / flechas y cierre con `Esc`. Resaltado de coincidencias en la rejilla (la activa con el color de acento) y contador `X / N` en una barra flotante del panel.
* **Hipervínculos interactivos y protocolo OSC 8** (✅ Completado):
  * Soporte de enlaces explícitos OSC 8 y detección regex de URLs (`https?://`, `localhost:\d+`, `127.0.0.1:\d+`).
  * Subrayado visual con `Ctrl + hover`, cursor pointer (`hand`), menú contextual («Abrir enlace», «Copiar enlace») y apertura en el navegador predeterminado mediante `ShellExecuteW`.
* **Perfiles rápidos de Shell en el selector `+`** ✅ *[Completado]*:
  * Menú desplegable para abrir pestañas o dividir paneles eligiendo entre PowerShell 7, Windows PowerShell 5.1, WSL (Ubuntu/Debian), Git Bash o CMD.
  * Accesible mediante clic derecho o `Shift + Clic` / `Alt + Clic` en el botón `+`, en el menú contextual de la barra superior y en la paleta de comandos (`Ctrl + Shift + P`).
* **Diseños predefinidos de división (*Grid Layouts*)**:
  * Presets rápidos: cuadrícula 2×2, 1 panel principal a la izquierda y 2 divididos a la derecha.

### 2.4. Ajustes (`src/app/settings.rs`, `src/renderer/settings_page.rs`)

* **Editor interactivo de atajos de teclado**:
  * Reemplazo de la tabla puramente estática por una interfaz donde hacer clic en una acción y pulsar la nueva combinación para guardarla en `tearminal.toml`.
* **Selector de familias tipográficas instaladas** ✅ *[Completado]*:
  * Enumeración asíncrona en segundo plano de fuentes monoespaciadas del sistema (`src/fonts.rs`) mediante inspección de tablas OpenType (`ttf_parser` integrado en `rustybuzz`), selector interactivo (`‹` / `›`) en la tarjeta de Tipografía de Ajustes, persistencia en `tearminal.toml` y recálculo dinámico de métricas/rejilla conservando JetBrains Mono como fallback seguro.
* **Importación de temas `.itermcolors` externos**:
  * Soporte para cargar archivos `.itermcolors` colocados en `%APPDATA%\tearminal\themes\` o arrastrados a Ajustes, sin depender de la lista fija de `themes.txt`.
* **Perfiles de agentes CLI configurables**:
  * Registro de nombres de ejecutables de agentes adicionales para monitorización de consumo en `init.ps1`.

---

## 3. Auditoría de Funciones Innecesarias, Redundantes o a Eliminar

Para preservar la ligereza del binario y evitar sobrecarga cognitiva y técnica, se identifican las siguientes áreas de **limpieza y simplificación**:

### 3.1. Configuración Zombie / Obsoleta: `terminal.gpu` ✅ *[Completado]*
* **Situación:** En `src/config.rs` se mantiene la opción booleana `terminal.gpu`. Como indica expresamente `AGENTS.md`: *«terminal.gpu se ignora (se conserva en la configuración por compatibilidad: ya no hay otro camino)»*.
* **Por qué sobra:** El renderer Direct3D 11 (o su fallback WARP por software) es la única vía de dibujo existente en el código. Mantener una opción de configuración que no hace nada genera confusión en el usuario y líneas de parsing innecesarias.
* **Acción recomendada:** Eliminar `terminal.gpu` de la interfaz de Ajustes y del struct de configuración (o marcarla como deprecada e ignorada en silencio durante la carga de configs antiguas).

### 3.2. Código legado de limpieza de temporales obsoletos ✅ *[Completado]*
* **Situación:** En `src/main.rs` y `src/app/clipboard.rs` existen bloques dedicados a buscar y eliminar carpetas temporales antiguas como `tearminal_clipboard` en `%TEMP%` creadas por versiones preliminares (pre-0.18).
* **Por qué sobra:** Esas rutas ya no se generan hace múltiples versiones. Ejecutar comprobaciones de disco en cada arranque o uso del portapapeles para migrar versiones antiguas añade I/O inútil en la ruta de inicio.
* **Acción recomendada:** Eliminar la lógica de limpieza de carpetas temporales obsoletas.

### 3.3. Heurísticas frágiles de cuota para agentes sin API pública (AGY) ✅ *[Completado]*
* **Situación:** En `src/app/ai_usage.rs` se mantiene código e infraestructura para lidiar con límites de AGY. Sin embargo, como documenta `AGENTS.md`: *«AGY no tiene fuente legítima (su cuota no se guarda en disco y su servidor local exige un token CSRF interno... no intentes leer secretos de otro proceso)»*.
* **Por qué sobra:** Mantener ramas especiales, mensajes de advertencia de límites no disponibles y sondeos para cuotas que no se pueden consultar añade ruido al popup y código muerto.
* **Acción recomendada:** Simplificar la telemetría de AGY para limitarse estrictamente a lo que sí existe de forma legítima: modelo activo extraído de la transcripción y conteo de tokens calculados, eliminando toda la maquinaria superflua de intento de límites de cuota.

### 3.4. Redundancia en transferencia de archivos remotos (OSC en banda vs `SshConnection`) ✅ *[Completado]*
* **Situación:** Coexisten dos mecanismos de comunicación remota:
  1. `ssh.rs`: Conexión de fondo limpia mediante `ssh.exe` en `BatchMode` ejecutando comandos con `CREATE_NO_WINDOW`.
  2. `remote_files.rs`: Protocolo en banda mediante secuencias de escape OSC custom (`TEArminalLs`, `TEArminalFile`) emitidas por scripts del servidor y capturadas por el parser VT.
* **Por qué sobra / conflicto:** El canal en banda OSC tiene un límite estricto de buffer (128 KB en `Parser<OSC_BUF_SIZE>`), fragmenta respuestas, requiere lógica compleja de ensamblado de chunks y puede corromperse ante ruidos en el PTY. `SshConnection` fuera de banda es mucho más robusto, admite listados de hasta 5.000 entradas y no contamina el emulador VT.
* **Acción recomendada:** Consolidar progresivamente la navegación y transferencia de archivos remotos a través del canal `SshConnection`, reduciendo la complejidad del parser OSC y evitando fragmentaciones en memoria.

### 3.5. Banderas VT no operativas o anuladas por ConPTY
* **Situación:** En `src/terminal.rs` se mantienen banderas como el modo 1004 (Focus reporting), que luego debe ser neutralizado porque Windows ConPTY lo convierte erróneamente en caracteres `Enter` o `Escape`.
* **Acción recomendada:** Auditar y eliminar estados del emulador VT que nunca pueden ser procesados de forma fiable a través de ConPTY en Windows, documentando las secuencias no soportadas en lugar de emularlas a medias.

### 3.6. Modo "Solo lectura" artificial en el Visor de Archivos ✅ *[Completado]*
* **Situación:** En `src/app/viewer.rs` existía una alternancia manual entre modo lectura y edición (`Ctrl + E`). Para archivos locales regulares en los que el usuario tiene permisos de escritura, este bloqueo provocaba pulsaciones perdidas y confusión en el usuario.
* **Acción realizada:** Los archivos locales y remotos con permisos de escritura se abren directamente en modo editable, reservando el indicador y bloqueo de solo lectura estrictamente para archivos marcados como de solo lectura en el sistema (`permissions().readonly()`), archivos binarios, no-UTF8 o en modo diff. Las pestañas de vista previa se reemplazan automáticamente mientras no existan modificaciones (`!is_dirty`) y se fijan de inmediato tan pronto como el usuario realiza una edición.

---

## 4. Matriz de Priorización y Plan de Acción

| Área | Acción | Tipo | Prioridad | Estado | Impacto |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Visor** | Búsqueda interna en archivo (`Ctrl + F`) | Característica | **P1** | ✅ Completado | Alto |
| **Visor** | Lectura Markdown responsiva y tablas adaptadas | Usabilidad | **P1** | ✅ Completado | Alto |
| **UI** | Cursor nítido, selección y Undo/Redo en inputs | Usabilidad | **P1** | ✅ Completado | Alto |
| **Ventanas** | Navegación MRU al cerrar pestañas y ventanas | UX / Flujo | **P1** | ✅ Completado | Alto |
| **Visor** | Salto a línea (`Ctrl + G`) | Característica | **P1** | ✅ Completado | Medio |
| **Explorador** | Gestión de archivos (Nuevo, Renombrar, Papelera Win32) | Característica | **P1** | ✅ Completado | Alto |
| **Config** | Eliminar opción fantasma `terminal.gpu` | Limpieza | **P1** | ✅ Completado | Medio (Claridad) |
| **Cleanup** | Eliminar comprobaciones de temporales pre-0.18 | Limpieza | **P1** | ✅ Completado | Bajo (Arranque) |
| **Explorador** | Filtro rápido en árbol (Fuzzy / `Ctrl + P`) | Característica | **P2** | ✅ Completado | Alto |
| **Explorador** | Acciones rápidas de Git en pestaña «Cambios» (Stage / Unstage / Discard) | Característica | **P2** | ✅ Completado | Alto |
| **Terminal** | Hipervínculos interactivos (OSC 8 / URLs regex) | Característica | **P2** | ✅ Completado | Alto |
| **Terminal** | Perfiles rápidos de Shell en botón `+` | Característica | **P2** | ✅ Completado | Alto |
| **Visor** | Visor nativo de imágenes (`.png`, `.jpg`, `.gif` animado, `.webp`, `.ico`; local y remoto) | Característica | **P2** | ✅ Completado | Medio-Alto |
| **IA / Telemetría** | Simplificar AGY y eliminar sondeos superfluos de cuota | Limpieza | **P2** | ✅ Completado | Medio |
| **Terminal** | Búsqueda en el scrollback (`Ctrl + Shift + F`) | Característica | **P3** | ✅ Completado | Alto |
| **Ajustes** | Editor interactivo de atajos de teclado | Característica | **P3** | Pendiente | Medio-Alto |
| **Ajustes** | Selector de fuentes instaladas del sistema (`DirectWrite`) | Característica | **P3** | ✅ Completado | Medio |
| **SSH** | Consolidar listados remotos en `SshConnection` vs OSC en banda | Limpieza / Refactor | **P3** | ✅ Completado | Alto (Estabilidad) |
| **Visor** | Abrir archivos con permisos de escritura en modo editable por defecto | Usabilidad / Limpieza | **P3** | ✅ Completado | Alto |
| **Visor** | Diff lado a lado (*Side-by-side*) | Característica | **P4** | ✅ Completado | Medio-Alto |
