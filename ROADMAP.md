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
* **Salto rápido a línea (`Ctrl + G`)**:
  * Input modal para saltar directamente a `línea[:columna]`.
  * Reutiliza [`FileViewerState::reveal_line()`](file:///src/app/viewer.rs) para desplegar bloques plegados si la línea está oculta.
* **Visor nativo de imágenes (`.png`, `.jpg`, `.webp`, `.ico`)**:
  * En lugar de mostrar código binario o texto corrupto, decodifica imágenes y las proyecta centradas en el lienzo mediante el atlas de texturas Direct3D 11.
* **Diff lado a lado (*Side-by-side*)**:
  * Conmutador en cabecera para comparar en dos columnas sincronizadas (versión en `HEAD` vs versión en disco), facilitando la revisión de cambios masivos producidos por agentes.

### 2.2. Panel Lateral y Explorador (`src/app/panel.rs`, `src/explorer.rs`)

* **Filtro rápido en el árbol (Fuzzy Filter / `Ctrl + P`)**:
  * Filtrado difuso en memoria sobre [`FileExplorer::flat_indices`](file:///src/explorer.rs) sin reconsultar el disco ni bloquear el renderizado.
* **Gestión contextual de archivos (Menú contextual)**:
  * Opciones: *Nuevo archivo*, *Nueva carpeta*, *Renombrar* y *Eliminar*.
  * La eliminación utiliza la API Win32 (`IFileOperation` con `FOF_ALLOWUNDO`) para enviar a la **Papelera de Reciclaje**, protegiendo el trabajo del usuario.
* **Arrastrar y soltar (Drag & Drop) hacia la terminal**:
  * Soporte de `WM_DROPFILES` para soltar archivos desde el explorador o desde Windows hacia el panel de terminal activo, inyectando su ruta entrecomillada.
* **Acciones rápidas de Git en pestaña "Cambios"**:
  * Botones discretos en cada fila para *Stage* (`git add`), *Unstage* y *Discard* (`git restore`), ejecutados de forma totalmente oculta (`CREATE_NO_WINDOW`).

### 2.3. Emulador de Terminal y Paneles (`src/terminal.rs`, `src/pty.rs`, `src/renderer/pane.rs`)

* **Búsqueda en el Scrollback (`Ctrl + Shift + F`)**:
  * Búsqueda sobre la `VecDeque` del buffer histórico con navegación y cajas de resaltado sobre las filas de la rejilla.
* **Hipervínculos interactivos y protocolo OSC 8**:
  * Soporte de enlaces explícitos OSC 8 y detección regex de URLs (`https?://`, `localhost:\d+`).
  * Subrayado visual con `Ctrl + hover` y apertura en el navegador predeterminado mediante `ShellExecuteW`.
* **Perfiles rápidos de Shell en el selector `+`**:
  * Menú desplegable para abrir pestañas o dividir paneles eligiendo entre PowerShell 7, Windows PowerShell 5.1, WSL (Ubuntu/Debian), Git Bash o CMD.
* **Diseños predefinidos de división (*Grid Layouts*)**:
  * Presets rápidos: cuadrícula 2×2, 1 panel principal a la izquierda y 2 divididos a la derecha.

### 2.4. Ajustes (`src/app/settings.rs`, `src/renderer/settings_page.rs`)

* **Editor interactivo de atajos de teclado**:
  * Reemplazo de la tabla puramente estática por una interfaz donde hacer clic en una acción y pulsar la nueva combinación para guardarla en `tearminal.toml`.
* **Selector de familias tipográficas instaladas**:
  * Enumeración de fuentes monoespaciadas del sistema vía `DirectWrite` (`IDWriteFontCollection`), manteniendo las fuentes embebidas como fallback seguro.
* **Importación de temas `.itermcolors` externos**:
  * Soporte para cargar archivos `.itermcolors` colocados en `%APPDATA%\tearminal\themes\` o arrastrados a Ajustes, sin depender de la lista fija de `themes.txt`.
* **Perfiles de agentes CLI configurables**:
  * Registro de nombres de ejecutables de agentes adicionales para monitorización de consumo y anclaje de entrada en `init.ps1`.

---

## 3. Auditoría de Funciones Innecesarias, Redundantes o a Eliminar

Para preservar la ligereza del binario y evitar sobrecarga cognitiva y técnica, se identifican las siguientes áreas de **limpieza y simplificación**:

### 3.1. Configuración Zombie / Obsoleta: `terminal.gpu`
* **Situación:** En `src/config.rs` se mantiene la opción booleana `terminal.gpu`. Como indica expresamente `AGENTS.md`: *«terminal.gpu se ignora (se conserva en la configuración por compatibilidad: ya no hay otro camino)»*.
* **Por qué sobra:** El renderer Direct3D 11 (o su fallback WARP por software) es la única vía de dibujo existente en el código. Mantener una opción de configuración que no hace nada genera confusión en el usuario y líneas de parsing innecesarias.
* **Acción recomendada:** Eliminar `terminal.gpu` de la interfaz de Ajustes y del struct de configuración (o marcarla como deprecada e ignorada en silencio durante la carga de configs antiguas).

### 3.2. Código legado de limpieza de temporales obsoletos
* **Situación:** En `src/main.rs` y `src/app/clipboard.rs` existen bloques dedicados a buscar y eliminar carpetas temporales antiguas como `tearminal_clipboard` en `%TEMP%` creadas por versiones preliminares (pre-0.18).
* **Por qué sobra:** Esas rutas ya no se generan hace múltiples versiones. Ejecutar comprobaciones de disco en cada arranque o uso del portapapeles para migrar versiones antiguas añade I/O inútil en la ruta de inicio.
* **Acción recomendada:** Eliminar la lógica de limpieza de carpetas temporales obsoletas.

### 3.3. Heurísticas frágiles de cuota para agentes sin API pública (AGY)
* **Situación:** En `src/app/ai_usage.rs` se mantiene código e infraestructura para lidiar con límites de AGY. Sin embargo, como documenta `AGENTS.md`: *«AGY no tiene fuente legítima (su cuota no se guarda en disco y su servidor local exige un token CSRF interno... no intentes leer secretos de otro proceso)»*.
* **Por qué sobra:** Mantener ramas especiales, mensajes de advertencia de límites no disponibles y sondeos para cuotas que no se pueden consultar añade ruido al popup y código muerto.
* **Acción recomendada:** Simplificar la telemetría de AGY para limitarse estrictamente a lo que sí existe de forma legítima: modelo activo extraído de la transcripción y conteo de tokens calculados, eliminando toda la maquinaria superflua de intento de límites de cuota.

### 3.4. Redundancia en transferencia de archivos remotos (OSC en banda vs `SshConnection`)
* **Situación:** Coexisten dos mecanismos de comunicación remota:
  1. `ssh.rs`: Conexión de fondo limpia mediante `ssh.exe` en `BatchMode` ejecutando comandos con `CREATE_NO_WINDOW`.
  2. `remote_files.rs`: Protocolo en banda mediante secuencias de escape OSC custom (`TEArminalLs`, `TEArminalFile`) emitidas por scripts del servidor y capturadas por el parser VT.
* **Por qué sobra / conflicto:** El canal en banda OSC tiene un límite estricto de buffer (128 KB en `Parser<OSC_BUF_SIZE>`), fragmenta respuestas, requiere lógica compleja de ensamblado de chunks y puede corromperse ante ruidos en el PTY. `SshConnection` fuera de banda es mucho más robusto, admite listados de hasta 5.000 entradas y no contamina el emulador VT.
* **Acción recomendada:** Consolidar progresivamente la navegación y transferencia de archivos remotos a través del canal `SshConnection`, reduciendo la complejidad del parser OSC y evitando fragmentaciones en memoria.

### 3.5. Banderas VT no operativas o anuladas por ConPTY
* **Situación:** En `src/terminal.rs` se mantienen banderas como el modo 1004 (Focus reporting), que luego debe ser neutralizado porque Windows ConPTY lo convierte erróneamente en caracteres `Enter` o `Escape`.
* **Acción recomendada:** Auditar y eliminar estados del emulador VT que nunca pueden ser procesados de forma fiable a través de ConPTY en Windows, documentando las secuencias no soportadas en lugar de emularlas a medias.

### 3.6. Modo "Solo lectura" artificial en el Visor de Archivos
* **Situación:** En `src/app/viewer.rs` existe una alternancia manual entre modo lectura y edición (`Ctrl + E`). Para archivos locales regulares en los que el usuario tiene permisos de escritura, este bloqueo suele provocar pulsaciones perdidas y confusión en el usuario.
* **Acción recomendada:** Hacer que los archivos con permisos de escritura sean editables por defecto (como en editores modernos), reservando el indicador de solo lectura estrictamente para archivos del sistema que carecen de permisos de escritura o buffers protegidos.

---

## 4. Matriz de Priorización y Plan de Acción

| Área | Acción | Tipo | Prioridad | Estado | Impacto |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Visor** | Búsqueda interna en archivo (`Ctrl + F`) | Característica | **P1** | ✅ Completado | Alto |
| **Visor** | Lectura Markdown responsiva y tablas adaptadas | Usabilidad | **P1** | ✅ Completado | Alto |
| **UI** | Cursor nítido, selección y Undo/Redo en inputs | Usabilidad | **P1** | ✅ Completado | Alto |
| **Ventanas** | Navegación MRU al cerrar pestañas y ventanas | UX / Flujo | **P1** | ✅ Completado | Alto |
| **Visor** | Salto a línea (`Ctrl + G`) | Característica | **P1** | Pendiente | Medio |
| **Explorador** | Gestión de archivos (Nuevo, Renombrar, Papelera Win32) | Característica | **P1** | Pendiente | Alto |
| **Config** | Eliminar opción fantasma `terminal.gpu` | Limpieza | **P1** | Pendiente | Medio (Claridad) |
| **Cleanup** | Eliminar comprobaciones de temporales pre-0.18 | Limpieza | **P1** | Pendiente | Bajo (Arranque) |
| **Explorador** | Filtro rápido en árbol (Fuzzy / `Ctrl + P`) | Característica | **P2** | Pendiente | Alto |
| **Terminal** | Hipervínculos interactivos (OSC 8 / URLs regex) | Característica | **P2** | Pendiente | Alto |
| **Terminal** | Perfiles rápidos de Shell en botón `+` | Característica | **P2** | Pendiente | Alto |
| **Visor** | Visor nativo de imágenes (`.png`, `.jpg`) | Característica | **P2** | Pendiente | Medio-Alto |
| **IA / Telemetría** | Simplificar AGY y eliminar sondeos superfluos de cuota | Limpieza | **P2** | Pendiente | Medio |
| **Terminal** | Búsqueda en el scrollback (`Ctrl + Shift + F`) | Característica | **P3** | Pendiente | Alto |
| **Ajustes** | Editor interactivo de atajos de teclado | Característica | **P3** | Pendiente | Medio-Alto |
| **Ajustes** | Selector de fuentes instaladas del sistema (`DirectWrite`) | Característica | **P3** | Pendiente | Medio |
| **SSH** | Consolidar listados remotos en `SshConnection` vs OSC en banda | Limpieza / Refactor | **P3** | Pendiente | Alto (Estabilidad) |
| **Visor** | Diff lado a lado (*Side-by-side*) | Característica | **P4** | Pendiente | Medio-Alto |
