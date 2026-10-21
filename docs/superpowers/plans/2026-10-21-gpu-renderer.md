# Migración del renderer a GPU (Direct3D 11) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que todo el fotograma de Axon se componga en la GPU con Direct3D 11 (WARP como respaldo) y desaparezcan `softbuffer` y el rasterizado en CPU.

**Architecture:** El renderer deja de escribir en un `&mut [u32]` y graba una `Scene` a través de un `Canvas`. Primero se porta todo el dibujo a `Canvas` sin cambiar píxeles (hashes idénticos); después `Canvas` graba y un rasterizador CPU de referencia reproduce la escena; después un backend D3D11 ejecuta la misma escena y se compara con la referencia; al final se borra el camino CPU.

**Tech Stack:** Rust 1.99, crate `windows` 0.52 (D3D11/DXGI, ya presente), HLSL SM 4.0 precompilado con `fxc`, `fontdue` y `rustybuzz` (sin cambios), `winit`.

**Spec:** `docs/superpowers/specs/2026-10-21-gpu-renderer-design.md`

## Global Constraints

* Windows 10 1809+ x64; *feature level* mínimo D3D 10_0, shaders en Shader Model 4.0; atlas y texturas ≤ 8192 px.
* Destino UNORM (no sRGB): la mezcla alfa sigue siendo en espacio gamma, como hoy. Coordenadas y glifos en píxeles enteros.
* Shaders precompilados y embebidos: nada de `D3DCompile` ni `d3dcompiler_47.dll` en ejecución.
* `AXON_RENDERER=warp` fuerza WARP. La clave `terminal.gpu` se acepta en configuraciones antiguas y se ignora.
* Etapas 0 y 1: los fotogramas deben ser **idénticos bit a bit** a los de la base (`golden/hashes.txt`). Etapa 2 en adelante: comparación por firma con la tolerancia fijada en la tarea 18.
* Reglas de `AGENTS.md` que siguen vigentes: un solo blitter (`blit_glyph`/`fill_rect` pasan a ser operaciones de `Canvas`), cachés con `FastMap`, `Cell` = 16 bytes, un método nuevo de `AxonApp` va en su submódulo, sin `.expect()` al crear dispositivos gráficos, `.ps1` en UTF-8 con BOM, rutas relativas en documentación.
* Validación al terminar cada tarea: `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`. Al final de cada etapa: `cargo build --release`.
* Mensajes de commit en español con el estilo del repositorio (`feat(ui): …`, `test(renderer): …`) y terminados con la línea `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
* No se toca la configuración real del usuario en tests (`config::dirs_or_fallback()`).

## Review Focus

Entradas o fallos que la spec implica y que ninguna tarea de «camino feliz» cubriría. Cada línea tiene su test en la tarea indicada.

1. **Ventana minimizada o de 0×0** (`resize(0, 0)`): no debe fallar ni crear texturas de tamaño cero → tarea 17.
2. **Dispositivo perdido a mitad de fotograma** (`DXGI_ERROR_DEVICE_REMOVED/RESET`): el siguiente fotograma debe ser idéntico al anterior → tarea 17.
3. **Más glifos distintos que el atlas en un solo fotograma** (CJK + iconos Nerd): no debe faltar ningún glifo ni fallar → tarea 14.
4. **Ventana o imagen mayor que el límite de textura (8192)**: se acota sin pánico → tareas 16 y 17.
5. **Glifos de bitmap vacío (espacio) e imágenes con datos incompletos** (`rgba_data` más corto que `width×height×4`): no se reserva atlas ni se lee fuera de rango → tareas 14 y 16.

---

## Etapa 0 — Base

### Task 1: Escenas de referencia, hashes y medidas base

**Files:**
- Modify: `src/renderer/tests.rs` (extraer la construcción de escenas de `preview_ui_scenes`, líneas ~1004–2060)
- Create: `src/renderer/test_support.rs` (`#[cfg(test)]`)
- Create: `src/renderer/golden/hashes.txt`
- Create: `docs/superpowers/specs/2026-10-21-gpu-renderer-baseline.md`

**Interfaces:**
- Produces:
  - `pub(super) fn build_scenes(w: usize, h: usize) -> Vec<(&'static str, Vec<u32>)>` (en `tests.rs`, 10 escenas con los nombres de los comentarios actuales: `terminal-vertical`, `tabs-superiores`, `explorador-acoplado`, `ajustes`, `visor`, `panel-derecha`, `wp1-vertical`, `wp1-superior`, `paneles-divididos`, `bloques-osc133`).
  - `pub(super) fn frame_hash(buf: &[u32]) -> u64` (FNV-1a de 64 bits sobre los `u32`) en `test_support.rs`.
  - `pub(super) fn bless_requested() -> bool` (`AXON_BLESS=1`).

- [ ] **Step 1: Extraer `build_scenes`.** `preview_ui_scenes` pasa a llamar a `build_scenes(1440, 900)` y solo escribe los PPM. La lógica de cada escena no cambia.
- [ ] **Step 2: Escribir el test que falla** `test_golden_scene_hashes`: para cada `(name, buf)` de `build_scenes(1440, 900)` compara `frame_hash(&buf)` con la línea `name=<hex>` de `golden/hashes.txt`; con `AXON_BLESS=1` reescribe el archivo en lugar de comparar. Falla porque el archivo no existe.
- [ ] **Step 3: Generar la base:** `$env:AXON_BLESS='1'; cargo test test_golden_scene_hashes; Remove-Item Env:AXON_BLESS`. Ejecutar de nuevo sin la variable: pasa. Ejecutar dos veces seguidas para comprobar que los hashes son deterministas (mismas escenas, mismo hash).
- [ ] **Step 4: Medir la base** y anotarla en `docs/superpowers/specs/2026-10-21-gpu-renderer-baseline.md`: salida de `cargo test --release bench_ -- --nocapture`; RAM privada y CPU de `axon.exe` (release) tras 30 s en reposo con una pestaña (Administrador de tareas → Detalles, o `Get-Process axon | Select WorkingSet64,PrivateMemorySize64`); tamaño de `target\release\axon.exe`; modelo de GPU/driver y versión de Windows de esta máquina.
- [ ] **Step 5: Commit** `test(renderer): escenas de referencia con hashes y medidas base`.

---

## Etapa 1a — Portar el dibujo a `Canvas` (sin cambiar píxeles)

### Task 2: `Canvas` inmediato

**Files:**
- Create: `src/renderer/canvas.rs`
- Modify: `src/renderer/mod.rs` (declarar `mod canvas;`, `pub use canvas::Canvas;`)
- Test: `src/renderer/tests.rs`

**Interfaces:**
- Produces (`Clip` es el actual `(x0, y0, x1, y1)`, extremo superior exclusivo):
  ```rust
  pub struct Canvas<'a> { /* buf, w, h */ }
  impl<'a> Canvas<'a> {
      pub fn over(buf: &'a mut [u32], w: usize, h: usize) -> Self;   // envoltorio inmediato; desaparece en la tarea 10
      pub fn size(&self) -> (usize, usize);
      pub fn clear(&mut self, color: Color);
      pub fn fill_rect(&mut self, clip: Clip, x: usize, y: usize, w: usize, h: usize, color: Color);
      pub fn glyph(&mut self, clip: Clip, glyph: &CachedGlyph, gx: i32, gy: i32, fg: Color);
      pub fn rounded_fill(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, color: Color);
      pub fn rounded_stroke(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, color: Color);
      pub fn image(&mut self, clip: Clip, img: ImageRef<'_>, dst: (i32, i32, usize, usize));
      pub fn card(&mut self, region: (usize, usize, usize, usize), inset: usize, radius: usize, outer: Color);
  }
  pub struct ImageRef<'a> { pub rgba: &'a [u8], pub width: u32, pub height: u32 }
  ```
- Consumes: `fill_rect`/`blit_glyph` de `draw.rs`, `draw_rounded_rect`/`stroke_rounded_rect`/`frame_card` de `ui/paint.rs`, y el bucle de imagen de `pane.rs:~680` (se mueve a `Canvas::image` con el mismo muestreo del vecino más próximo).

- [ ] **Step 1: Test que falla** `test_canvas_matches_legacy_primitives`: para cada operación (relleno con recorte parcial, glifo con alfa, redondeado y trazo, imagen con alfa 0/128/255, `card`) pinta con `Canvas` y con la función antigua sobre dos búferes iguales y exige `assert_eq!` de ambos.
- [ ] **Step 2: Ejecutarlo y verificar que falla** (`cargo test test_canvas_matches_legacy_primitives`): no compila, `Canvas` no existe.
- [ ] **Step 3: Implementar `Canvas`** delegando en las funciones existentes (cada método es una llamada). `image` copia el bucle de `pane.rs` con las comprobaciones de rango intactas, y se detiene si `rgba.len() < width*height*4` en lugar de leer fuera (Review Focus 5).
- [ ] **Step 4: Ejecutarlo: pasa.**
- [ ] **Step 5: Commit** `refactor(renderer): Canvas inmediato sobre las primitivas existentes`.

### Task 3: Portar `draw.rs` y `ui/paint.rs`

**Files:**
- Modify: `src/renderer/draw.rs`, `src/ui/paint.rs`, `src/ui/mod.rs`

**Regla de portado (vale para las tareas 3–9):** cada función que recibe `buffer: &mut [u32]` + `buf_width` (+ `buf_height`) pasa a recibir `canvas: &mut Canvas` (el ancho y alto salen de `canvas.size()`); toda escritura directa a `buffer[...]` se sustituye por un método de `Canvas`. La lógica, el orden de las operaciones y las coordenadas no cambian. Los métodos públicos de `TerminalRenderer` conservan su nombre.

**Interfaces:**
- Consumes: `Canvas` (tarea 2).
- Produces: `draw_ui_text`, `draw_ui_text_tracked`, `draw_mono_text`, `draw_keycap`, `draw_lucide_icon`, `draw_scrollbar`, `draw_scrollbar_rects`, `draw_rect` con `canvas: &mut Canvas` en el lugar de `(buffer, buf_width, buf_height)`. `ui::draw_rounded_rect`, `ui::stroke_rounded_rect` y `ui::frame_card` se vuelven pasos internos de `Canvas`; los llamantes usan `canvas.rounded_fill/rounded_stroke/card`.

- [ ] **Step 1: Portar `draw.rs` y `paint.rs`** según la regla (el portado es de abajo arriba: primero las primitivas, luego las pantallas).
- [ ] **Step 2: Adaptadores temporales.** Cada llamada a estas funciones desde una pantalla aún sin portar se envuelve en el sitio: `&mut Canvas::over(buffer, buf_width, buf_height)`. Es una edición mecánica de una línea por llamada, guiada por el compilador; las tareas 4–8 las eliminan al portar cada archivo.
- [ ] **Step 3: Los tests de `renderer/tests.rs`** que llamen a estas funciones usan un ayudante `with_canvas(w, h, |c: &mut Canvas| …) -> Vec<u32>` añadido a `test_support.rs`.
- [ ] **Step 4: Gate:** `cargo test` completo en verde y `test_golden_scene_hashes` sin `AXON_BLESS`: hashes idénticos.
- [ ] **Step 5: Commit** `refactor(renderer): draw.rs y paint.rs usan Canvas`.

### Task 4: Portar terminal y bloques

**Files:** Modify `src/renderer/pane.rs`, `src/renderer/blocks.rs`

- [ ] **Step 1:** aplicar la regla de portado y eliminar los adaptadores `Canvas::over` de estos archivos. El bucle de imagen de `pane.rs:~680–715` se sustituye por `canvas.image(...)`.
- [ ] **Step 2: Gate:** `cargo test` en verde y `test_golden_scene_hashes` sin `AXON_BLESS`: hashes idénticos.
- [ ] **Step 3: Commit** `refactor(renderer): pane y bloques usan Canvas`.

### Task 5: Portar paneles laterales y menú

**Files:** Modify `src/renderer/sidebar.rs`, `src/renderer/files_panel.rs`, `src/renderer/explorer_panel.rs`, `src/renderer/context_menu.rs`

- [ ] **Step 1:** aplicar la regla de portado y eliminar los adaptadores `Canvas::over` de estos archivos.
- [ ] **Step 2: Gate:** `cargo test` en verde y hashes idénticos a la base.
- [ ] **Step 3: Commit** `refactor(renderer): sidebar, explorador y menú usan Canvas`.

### Task 6: Portar barras

**Files:** Modify `src/renderer/top_bar.rs`, `src/renderer/status_bar.rs`

- [ ] **Step 1:** aplicar la regla de portado y eliminar los adaptadores `Canvas::over` de estos archivos.
- [ ] **Step 2: Gate:** `cargo test` en verde y hashes idénticos a la base.
- [ ] **Step 3: Commit** `refactor(renderer): barras usan Canvas`.

### Task 7: Portar visor y vista previa de Markdown

**Files:** Modify `src/renderer/file_viewer.rs`, `src/renderer/md_preview.rs`

- [ ] **Step 1:** aplicar la regla de portado y eliminar los adaptadores `Canvas::over` de estos archivos.
- [ ] **Step 2: Gate:** `cargo test` en verde y hashes idénticos a la base.
- [ ] **Step 3: Commit** `refactor(renderer): visor y md_preview usan Canvas`.

### Task 8: Portar Ajustes (sin tocar la copia fuera de pantalla)

**Files:** Modify `src/renderer/settings_page.rs`

- [ ] **Step 1:** aplicar la regla de portado y eliminar los adaptadores `Canvas::over` de este archivo. El búfer `tmp` de Ajustes se envuelve en su propio `Canvas::over(&mut tmp, width, tmp_h)`; la copia de la ventana visible se mantiene tal cual hasta la tarea 11.
- [ ] **Step 2: Gate:** `cargo test` en verde y hashes idénticos a la base.
- [ ] **Step 3: Commit** `refactor(renderer): Ajustes usa Canvas`.

### Task 9: Portar `app/redraw.rs` y cerrar la etapa 1a

**Files:** Modify `src/app/redraw.rs`, `src/renderer/tests.rs`, `src/renderer/test_support.rs`

- [ ] **Step 1:** `draw_full_scene` y `draw_panel` crean `Canvas::over(buffer, width, height)` una sola vez y lo pasan; `frame_card` pasa a `canvas.card(...)`. El panel derecho conserva su búfer propio y la copia (tarea 11).
- [ ] **Step 2: Gate de la etapa:** `rg "Canvas::over\(" src` solo debe aparecer en `redraw.rs`, `settings_page.rs` (búfer `tmp`), `test_support.rs` y el propio `canvas.rs`. `cargo test` completo y `test_golden_scene_hashes` **sin `AXON_BLESS`**: hashes idénticos a la base. Cualquier diferencia es un error del portado, no un motivo para regenerar.
- [ ] **Step 3:** `cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo build --release`.
- [ ] **Step 4: Commit** `refactor(renderer): todo el dibujo pasa por Canvas`.

---

## Etapa 1b — Escena grabada y rasterizador CPU de referencia

### Task 10: `Scene` y `raster_cpu`

**Files:**
- Create: `src/renderer/scene.rs`, `src/renderer/raster_cpu.rs`
- Modify: `src/renderer/canvas.rs`, `src/renderer/mod.rs`, `src/renderer/test_support.rs`
- Test: `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  // scene.rs
  pub enum Op {
      Clear(Color),
      Fill   { clip: Clip, x: i32, y: i32, w: u32, h: u32, color: Color },
      Glyph  { clip: Clip, glyph: GlyphId, x: i32, y: i32, color: Color },
      Rounded{ x: i32, y: i32, w: u32, h: u32, radius: u32, color: Color, stroke: bool },
      Image  { clip: Clip, image: ImageId, x: i32, y: i32, w: u32, h: u32 },
      Card   { region: (u32, u32, u32, u32), inset: u32, radius: u32, outer: Color },
  }
  pub struct Scene { pub w: u32, pub h: u32, pub ops: Vec<Op>, /* glifos e imágenes usados */ }
  pub struct GlyphId(u32);  pub struct ImageId(u32);
  // raster_cpu.rs
  pub fn rasterize(scene: &Scene, buf: &mut [u32]);
  ```
  `Canvas` deja de envolver un búfer: `Canvas::over` se elimina y pasa a `Canvas::new() -> Canvas` (sin argumentos, sin lifetime), `begin(&mut self, w: u32, h: u32)` (vacía la escena conservando su memoria) y `scene(&self) -> &Scene`. `AxonApp` posee un `Canvas`. En `on_redraw` se llama a `begin`, se graba el fotograma con `draw_full_scene(&mut canvas, …)` y, **hasta la tarea 19**, se rasteriza con `rasterize(canvas.scene(), buffer)` sobre el búfer del presentador actual (`d3d11`/`softbuffer`), de modo que la presentación no cambia. El rasterizador reproduce las operaciones **en el mismo orden** con las funciones de dibujo actuales, generalizadas a origen `i32` (necesario para la tarea 11).
- `GlyphId`/`ImageId` indexan una tabla dentro de la escena que guarda una copia del bitmap (se vacía al reiniciar la escena); sirven para que el backend GPU pueda subirlos al atlas.

- [ ] **Step 1: Tests que fallan:** `test_scene_records_ops_in_order` (tres operaciones → `ops.len() == 3` y orden conservado), `test_rasterize_matches_immediate_canvas` (la misma secuencia de operaciones rasterizada y pintada de forma inmediata produce el mismo búfer).
- [ ] **Step 2: Ejecutarlos: fallan.**
- [ ] **Step 3: Implementar** `Scene`, el cambio interno de `Canvas` (graba en lugar de escribir) y `rasterize`. `test_support::with_canvas` devuelve el búfer ya rasterizado.
- [ ] **Step 4: Gate:** `test_golden_scene_hashes` sin `AXON_BLESS`: hashes idénticos a la base. `cargo test --release bench_ -- --nocapture`: anotar la variación en el documento base (la grabación añade un paso, no debe duplicar el coste).
- [ ] **Step 5: Commit** `feat(renderer): Scene grabada y rasterizador CPU de referencia`.

### Task 11: Recorte y desplazamiento en lugar de copias de búfer

**Files:** Modify `src/renderer/canvas.rs`, `src/renderer/settings_page.rs`, `src/app/redraw.rs`

**Interfaces:**
- Produces: `Canvas::with_offset(&mut self, dx: i32, dy: i32, clip: Clip, f: impl FnOnce(&mut Canvas))`: dentro de `f`, todas las coordenadas se trasladan por `(dx, dy)` al grabar y el recorte efectivo es `clip` (coordenadas absolutas) intersecado con el explícito de cada operación.

- [ ] **Step 1: Test que falla** `test_with_offset_equals_offscreen_copy`: dibuja un conjunto de operaciones (relleno, texto, redondeado que cruza el borde del recorte) con `with_offset` y con el método antiguo (búfer temporal + `copy_from_slice`) y exige búferes iguales.
- [ ] **Step 2: Implementar `with_offset`.**
- [ ] **Step 3: Ajustes:** quitar `settings_scratch` y la copia; dibujar `draw_settings_*` con `with_offset(0, page_y + top - scroll_abs, clip_ventana_visible, …)`. **Panel derecho:** quitar `panel_buf` y la copia de `draw_panel`; `render_sidebar`/`render_files_panel` se dibujan con `with_offset(x0, 0, clip_panel, …)` y el fondo del panel se rellena antes con `canvas.fill_rect`.
- [ ] **Step 4: Gate:** `test_golden_scene_hashes` sin `AXON_BLESS` (escenas `ajustes` y `panel-derecha` incluidas). Si una difiere, la causa probable es un fondo que antes se pintaba en el búfer temporal: pintarlo explícitamente, no regenerar hashes.
- [ ] **Step 5: Commit** `refactor(renderer): Ajustes y panel derecho con recorte y desplazamiento`.

### Task 12: Firmas de referencia para la etapa 2

**Files:** Modify `src/renderer/test_support.rs`, `src/renderer/tests.rs`; Create `src/renderer/golden/signatures.bin`

**Interfaces:**
- Produces: `pub(super) fn signature(buf: &[u32], w: usize, h: usize) -> Vec<u8>` (media RGB por bloque de 48×48 px, 3 bytes por bloque) y `pub(super) fn signature_distance(a: &[u8], b: &[u8]) -> f32` (diferencia media absoluta por canal, 0–255).

- [ ] **Step 1: Test que falla** `test_signature_distance_is_zero_for_identical_and_grows_with_difference` (misma imagen → 0.0; una imagen con un bloque 48×48 cambiado en 255 → > 0.0).
- [ ] **Step 2: Implementar** ambas funciones y generar `signatures.bin` con `AXON_BLESS=1` desde la escena CPU actual.
- [ ] **Step 3: Test** `test_golden_scene_signatures_match` compara la firma de cada escena con `signatures.bin` (distancia 0.0 por ahora).
- [ ] **Step 4: Commit** `test(renderer): firmas de bloques como referencia para la GPU`. **Fin de la etapa 1.** `cargo build --release`.

---

## Etapa 2 — Backend GPU

### Task 13: Dispositivo D3D11 con respaldo WARP

**Files:**
- Create: `src/renderer/gpu/mod.rs`, `src/renderer/gpu/device.rs`
- Modify: `src/renderer/mod.rs` (`pub mod gpu;`), `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum DriverKind { Hardware, Warp }
  pub struct GpuDevice { pub device: ID3D11Device, pub context: ID3D11DeviceContext, pub kind: DriverKind, pub feature_level: D3D_FEATURE_LEVEL }
  impl GpuDevice {
      pub fn new(hwnd: Option<HWND>) -> Result<GpuDevice>;        // Hardware (adaptador del monitor) → Warp
      pub fn new_with(kind: Option<DriverKind>, min_level: D3D_FEATURE_LEVEL) -> Result<GpuDevice>; // forzado para tests
      pub fn max_texture_dim(&self) -> u32;                         // 8192 con FL 10_0, 16384 con FL 11_0+
  }
  pub fn requested_driver() -> Option<DriverKind>;                  // lee AXON_RENDERER=warp
  ```
- Consumes: la lógica de `create_device` y `adapter_for_window` de `renderer/d3d11.rs` (se copia; el archivo viejo se borra en la tarea 20).

- [ ] **Step 1: Tests que fallan:** `test_gpu_device_warp_is_always_available` (`new_with(Some(Warp), D3D_FEATURE_LEVEL_10_0)` devuelve `Ok` y `kind == Warp`), `test_gpu_device_feature_level_10_0_is_accepted` (mismo dispositivo, `feature_level >= 10_0` y `max_texture_dim() >= 8192`), `test_requested_driver_reads_env` (con `AXON_RENDERER=warp` devuelve `Some(Warp)`; sin la variable, `None`).
- [ ] **Step 2: Ejecutarlos: fallan** (módulo inexistente).
- [ ] **Step 3: Implementar** `GpuDevice`. `new(hwnd)`: si `requested_driver()` es `Warp` va directo a WARP; si no, intenta hardware con el adaptador del monitor (`D3D_DRIVER_TYPE_UNKNOWN` con adaptador explícito) y, si falla, WARP. Banderas `D3D11_CREATE_DEVICE_BGRA_SUPPORT`; en `debug_assertions` además `D3D11_CREATE_DEVICE_DEBUG` solo si la capa está instalada (reintentar sin ella si falla).
- [ ] **Step 4: Ejecutarlos: pasan.**
- [ ] **Step 5: Commit** `feat(gpu): dispositivo D3D11 con respaldo WARP`.

### Task 14: Atlas de glifos e imágenes

**Files:** Create `src/renderer/gpu/atlas.rs`; Modify `src/renderer/gpu/mod.rs`, `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct ShelfPacker { /* ancho, alto, estantes */ }
  impl ShelfPacker {
      pub fn new(w: u32, h: u32) -> Self;
      pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)>; // None si no cabe
      pub fn reset(&mut self);
  }
  pub struct GlyphAtlas { /* textura R8 + packer + mapa GlyphId → rect */ }
  impl GlyphAtlas {
      pub fn new(dev: &GpuDevice) -> Result<Self>;                   // 1024×1024 R8
      pub fn rect_for(&mut self, dev: &GpuDevice, id: GlyphId, glyph: &CachedGlyph) -> Result<AtlasRect>; // sube si falta
      pub fn begin_frame(&mut self);                                  // si el fotograma anterior se llenó, reset()
      pub fn srv(&self) -> &ID3D11ShaderResourceView;
  }
  pub struct AtlasRect { pub x: u16, pub y: u16, pub w: u16, pub h: u16 }
  ```
- Un glifo con `width == 0 || height == 0` devuelve un `AtlasRect` vacío sin reservar espacio (Review Focus 5).
- El atlas crece por duplicación hasta `min(4096, max_texture_dim)`; si aun así no cabe, marca «lleno», **vacía el atlas al inicio del fotograma siguiente** y el glifo que no cupo se dibuja con un fallback (se vuelve a subir tras el vaciado). Ningún glifo se omite (Review Focus 3).

- [ ] **Step 1: Tests que fallan (puros, sin GPU):** `test_shelf_packer_fills_row_then_opens_new_shelf` (dos glifos de 10×10 en un 32×32 caben en la misma fila; el tercero abre estante), `test_shelf_packer_returns_none_when_full`, `test_shelf_packer_reset_allows_reuse`. **Con WARP:** `test_atlas_uploads_glyph_and_reads_back_coverage` (sube un `CachedGlyph` 4×4 con valores 0..16 y lee la textura con una *staging texture*), `test_atlas_empty_glyph_reserves_nothing`, `test_atlas_overflow_never_drops_glyphs` (sube 4 000 glifos distintos de 16×16 en un fotograma: todos reciben rect válido, ninguno error, y tras `begin_frame()` el atlas vuelve a aceptar).
- [ ] **Step 2: Ejecutarlos: fallan.**
- [ ] **Step 3: Implementar** `ShelfPacker` (estantes de altura = glifo más alto de la fila, con 1 px de separación para evitar sangrado del muestreo) y `GlyphAtlas`. Subida con `UpdateSubresource` sobre la región. Las imágenes RGBA usan una textura propia por imagen (`ImageTexture`, `DXGI_FORMAT_B8G8R8A8_UNORM`, tamaño acotado a `max_texture_dim`: si la imagen es mayor se descarta con `log::warn` y no se dibuja; Review Focus 4).
- [ ] **Step 4: Ejecutarlos: pasan.**
- [ ] **Step 5: Commit** `feat(gpu): atlas de glifos con empaquetado por estantes`.

### Task 15: Shaders HLSL y bytecode embebido

**Files:**
- Create: `src/renderer/gpu/shaders/quad.hlsl`, `tools/compile-shaders.ps1`, `src/renderer/gpu/shaders/quad_vs.cso`, `src/renderer/gpu/shaders/quad_ps.cso`, `src/renderer/gpu/shaders/quad.hlsl.sha256`
- Create: `src/renderer/gpu/pipeline.rs`
- Modify: `src/renderer/gpu/mod.rs`, `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  #[repr(C)] #[derive(Clone, Copy)]
  pub struct Instance {            // 32 bytes
      pub rect: [i16; 4],          // x, y, w, h (en píxeles; x e y pueden ser negativos)
      pub clip: [u16; 4],          // x0, y0, x1, y1
      pub color: u32,              // 0xAARRGGBB
      pub uv: [u16; 2],            // origen en el atlas (Glyph) o 0
      pub kind_radius: u32,        // bits 0..3 = Kind, 8..15 = radio, 16..23 = borde (px)
  }
  pub enum Kind { Solid = 0, RoundedFill = 1, RoundedStroke = 2, Glyph = 3, Image = 4, CardCorner = 5 }
  pub struct Pipeline { /* VS, PS, input layout, blend, sampler(punto), constantes: tamaño del destino */ }
  impl Pipeline { pub fn new(dev: &GpuDevice) -> Result<Self>; }
  ```
- `tools/compile-shaders.ps1` localiza `fxc.exe` en `${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\`, compila `vs_4_0` y `ps_4_0` (`/O3 /Qstrip_reflect /Qstrip_debug`), escribe los `.cso` y el SHA-256 del `.hlsl`. UTF-8 con BOM. Los `.cso` se versionan.
- El shader de píxel: `Glyph` multiplica la cobertura del atlas por el alfa del color y mezcla `SrcAlpha, InvSrcAlpha`; `RoundedFill/Stroke` usan la misma SDF que `ui/paint.rs` (distancia al rectángulo redondeado con cobertura lineal de 1 px); `Image` usa muestreo de **punto** (igual que el vecino más próximo actual); `CardCorner` pinta `outer` con la cobertura exterior del cuarto de círculo. `discard` si el píxel cae fuera de `clip`. Los píxeles se evalúan en el centro (`+0.5`) como el rasterizador CPU.

- [ ] **Step 1: Tests que fallan:** `test_instance_is_32_bytes` (`size_of::<Instance>() == 32`), `test_embedded_shaders_match_hlsl_hash` (SHA-256 de `quad.hlsl` == contenido de `quad.hlsl.sha256`: falla si alguien edita el HLSL sin recompilar), `test_pipeline_creates_on_warp_at_feature_level_10_0`.
- [ ] **Step 2: Ejecutarlos: fallan.**
- [ ] **Step 3: Escribir el HLSL y el script; ejecutar** `pwsh tools/compile-shaders.ps1`; implementar `Pipeline::new` con `include_bytes!`.
- [ ] **Step 4: Ejecutarlos: pasan.**
- [ ] **Step 5: Commit** `feat(gpu): pipeline de quads con shaders SM 4.0 embebidos`.

### Task 16: Ejecutar una `Scene` en la GPU (sin ventana) y paridad por primitiva

**Files:** Create `src/renderer/gpu/renderer.rs`; Modify `src/renderer/gpu/mod.rs`, `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct GpuRenderer { /* GpuDevice, Pipeline, GlyphAtlas, buffer de instancias dinámico, texturas de imagen */ }
  impl GpuRenderer {
      pub fn new(dev: GpuDevice) -> Result<Self>;
      pub fn encode(scene: &Scene, atlas_rect: impl FnMut(GlyphId) -> AtlasRect) -> Vec<Instance>; // pura, testeable
      pub fn render(&mut self, scene: &Scene, target: &ID3D11RenderTargetView) -> Result<()>;
      pub fn render_to_pixels(&mut self, scene: &Scene) -> Result<Vec<u32>>; // textura sin ventana + staging; solo tests y capturas
  }
  ```
- `encode` convierte cada `Op` en una o varias `Instance` conservando el orden (`Card` → 4 `CardCorner` + 4 `Solid` de margen). Todas las instancias se dibujan con una llamada instanciada por grupo consecutivo de misma textura (atlas, imagen, ninguna).
- Una escena mayor que `max_texture_dim()` se acota a ese tamaño (Review Focus 4). Una imagen con `rgba.len() < width*height*4` no se sube ni se dibuja (Review Focus 5).

- [ ] **Step 1: Tests que fallan (puros):** `test_encode_preserves_op_order`, `test_encode_card_emits_corners_and_margins`, `test_encode_skips_zero_sized_ops`. **Paridad con WARP**, uno por primitiva, comparando `render_to_pixels` con `raster_cpu::rasterize` y exigiendo diferencia máxima por canal ≤ `PARITY_TOLERANCE` (constante provisional `2`, se fija en la tarea 18): `test_gpu_parity_fill_with_clip`, `test_gpu_parity_glyph_coverage`, `test_gpu_parity_rounded_fill_and_stroke`, `test_gpu_parity_image_nearest_with_alpha`, `test_gpu_parity_card`, `test_gpu_oversized_scene_is_clamped_not_panicking`, `test_gpu_truncated_image_is_skipped`.
- [ ] **Step 2: Ejecutarlos: fallan.**
- [ ] **Step 3: Implementar** `encode`, el buffer de instancias (`D3D11_USAGE_DYNAMIC`, `MAP_WRITE_DISCARD`, crece por duplicación), `render` y `render_to_pixels`. Cada diferencia de paridad que pase del umbral se corrige en el shader, no subiendo el umbral.
- [ ] **Step 4: Ejecutarlos: pasan.**
- [ ] **Step 5: Commit** `feat(gpu): ejecución de Scene en D3D11 con paridad por primitiva`.

### Task 17: Superficie con ventana, redimensionado y recuperación de dispositivo

**Files:** Create `src/renderer/gpu/surface.rs`; Modify `src/renderer/gpu/mod.rs`, `src/renderer/tests.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct GpuSurface { /* GpuRenderer, IDXGISwapChain1 (Flip, 2 buffers), RTV, tamaño */ }
  impl GpuSurface {
      pub fn new(hwnd: HWND, w: u32, h: u32) -> Result<Self>;
      pub fn resize(&mut self, w: u32, h: u32) -> Result<()>;   // w,h = max(1, acotado a max_texture_dim)
      pub fn present(&mut self, scene: &Scene) -> Result<Presented>;
      pub fn driver_kind(&self) -> DriverKind;
  }
  pub enum Presented { Ok, Recovered }   // Recovered = se recreó el dispositivo; el llamante debe repintar entera la escena
  #[cfg(test)] impl GpuSurface { pub fn simulate_device_loss(&mut self); }
  ```
- Ante `DXGI_ERROR_DEVICE_REMOVED`/`DEVICE_RESET` (o `GetDeviceRemovedReason() != S_OK`) tras `Present`, `present` recrea `GpuDevice`, `GpuRenderer`, atlas, texturas de imagen y *swap chain*, y devuelve `Recovered`. La escena siguiente se pinta entera (el atlas se rellena a demanda).

- [ ] **Step 1: Tests que fallan** (con una ventana oculta de `winit`, o con una superficie «headless» sobre una textura si el entorno no puede crear la ventana, igual que `test_d3d11_presenter_lifecycle` de hoy): `test_surface_resize_to_zero_clamps_to_one` (Review Focus 1), `test_surface_resize_beyond_limit_is_clamped` (Review Focus 4), `test_surface_recovers_after_device_loss` (pinta una escena, `simulate_device_loss()`, vuelve a presentar: devuelve `Recovered`, y `render_to_pixels` de la misma escena coincide con el de antes; Review Focus 2).
- [ ] **Step 2: Ejecutarlos: fallan.**
- [ ] **Step 3: Implementar** reutilizando `DXGI_SWAP_EFFECT_FLIP_DISCARD`, `DXGI_SCALING_NONE`, `BufferCount: 2` y `AlphaMode: IGNORE` de `d3d11.rs`. `present(1, 0)` (VSync).
- [ ] **Step 4: Ejecutarlos: pasan.**
- [ ] **Step 5: Commit** `feat(gpu): superficie con swap chain y recuperación de dispositivo`.

### Task 18: Paridad de escenas completas y rendimiento GPU

**Files:** Modify `src/renderer/tests.rs`, `src/renderer/gpu/renderer.rs` (solo si hay desvíos), `docs/superpowers/specs/2026-10-21-gpu-renderer-baseline.md`

- [ ] **Step 1: Test que falla** `test_gpu_scenes_match_cpu_reference`: para cada escena de `build_scenes`, graba la `Scene`, la renderiza con `render_to_pixels` (WARP) y exige `signature_distance(gpu, golden) <= SIGNATURE_TOLERANCE` y diferencia máxima por canal ≤ `PARITY_TOLERANCE` en el 99,9 % de los píxeles.
- [ ] **Step 2: Medir y fijar los umbrales:** ejecutarlo imprimiendo la distancia real de cada escena; fijar `SIGNATURE_TOLERANCE` y `PARITY_TOLERANCE` en el valor medido redondeado hacia arriba con un margen mínimo, y anotar los valores medidos en el documento base. No se acepta una diferencia visible a ojo: abrir los PPM de `preview_ui_scenes` de las dos rutas y comparar `terminal-vertical`, `ajustes` y `visor`.
- [ ] **Step 3: Añadir** `bench_gpu_encode_and_submit_full_frame` (en `mod bench` de `tests.rs`): mide el coste en CPU de grabar + `encode` + enviar una escena de 1080p con texto denso (no el tiempo de GPU) e imprime los ms; compararlo con la base.
- [ ] **Step 4: Ejecutar** `cargo test --release bench_ -- --nocapture`; si el coste de CPU supera la base (~1,3 ms), optimizar `encode` (evitar reservas, reutilizar el `Vec<Instance>`) antes de seguir.
- [ ] **Step 5: Commit** `test(gpu): paridad de escenas completas y benchmark de envío`.

### Task 19: La GPU pasa a ser el camino por defecto

**Files:** Modify `src/app/handler.rs` (~líneas 150–170), `src/app/mod.rs` (campos `d3d11_presenter`, `context`, `surface`), `src/app/redraw.rs` (~líneas 140–195), `src/config.rs` (`gpu`), `examples/axon.toml`

- [ ] **Step 1: Test que falla** `test_config_gpu_key_is_accepted_and_ignored`: un TOML con `[terminal] gpu = false` carga sin error y `AxonConfig::terminal.gpu` no influye en la elección del renderer (no hay ninguna lectura de ese campo fuera de `config.rs`; comprobarlo con un `Grep` en la revisión).
- [ ] **Step 2: Implementar:** `AxonApp` guarda `gpu_surface: Option<GpuSurface>` creado con `GpuSurface::new(hwnd, w, h)`; `on_redraw` llama a `gpu_surface.present(canvas.scene())` en lugar de `rasterize` + presentador antiguo; si devuelve `Presented::Recovered`, se repinta la escena entera. Si `GpuSurface::new` falla incluso con WARP, se registra con `logger` y se muestra un diálogo de error con el mismo patrón que `pty::report_spawn_error`: la app **no arranca en blanco**. El camino `softbuffer` queda sin usar (se borra en la tarea 20).
- [ ] **Step 3:** documentar en `examples/axon.toml` que `gpu` ya no tiene efecto; mantener el campo en `config.rs` como `#[serde(default)]` sin uso.
- [ ] **Step 4: Verificación manual en esta máquina:** `cargo run --release`; comprobar que arranca, redimensiona, abre Ajustes, el visor y un panel dividido, y que con `$env:AXON_RENDERER='warp'` también funciona. Comparar CPU y RAM en reposo con la base y anotarlo.
- [ ] **Step 5: Commit** `feat(ui): el render por GPU (D3D11 + WARP) es el camino por defecto`.

---

## Etapa 3 — Eliminar el camino CPU

### Task 20: Borrado y migración final de tests

**Files:**
- Delete: `src/renderer/d3d11.rs`, `src/renderer/raster_cpu.rs`; las funciones de dibujo sobre `&mut [u32]` de `src/ui/paint.rs` y `src/renderer/draw.rs` (`fill_rect`, `blit_glyph`, `draw_rounded_rect`, `stroke_rounded_rect`, `frame_card`)
- Modify: `Cargo.toml` (quitar `softbuffer`; revisar si siguen necesarias las *features* de `windows-sys` `Win32_Graphics_Gdi`), `src/app/mod.rs`, `src/app/handler.rs`, `src/renderer/mod.rs`, `src/renderer/tests.rs`, `src/renderer/test_support.rs`, `AGENTS.md`, `README.md`

- [ ] **Step 1:** `test_support::with_canvas` y `build_scenes` pasan a devolver `Vec<u32>` obtenido con `GpuRenderer::render_to_pixels(canvas.scene())` (WARP) en lugar de `rasterize`. `test_golden_scene_hashes` se elimina; `test_golden_scene_signatures_match` y `test_gpu_scenes_match_cpu_reference` pasan a comparar contra `signatures.bin`. Los tests que leen píxeles concretos se mantienen con la tolerancia por canal de la tarea 18.
- [ ] **Step 2: Borrar** el código y la dependencia de la lista de arriba. `cargo build` debe quedar sin referencias a `softbuffer`, `D3D11Presenter` ni `buffer: &mut [u32]` en `renderer/` y `ui/`.
- [ ] **Step 3: Documentación:** en `AGENTS.md` reemplazar «Render por software… con fallback automático a `softbuffer`» y las reglas de `fill_rect`/`blit_glyph` por la descripción de `Canvas`/`Scene`/`gpu/`, la regla «al añadir una operación de dibujo, añadir su `Kind`, su `encode` y su test de paridad», la regla de recompilar shaders con `tools/compile-shaders.ps1`, y la nota de que el repintado parcial sigue pendiente; en `README.md` actualizar la sección de rendimiento con las medidas nuevas (las reales, medidas con `bench_` y en reposo) y la línea de `AXON_RENDERER=warp`.
- [ ] **Step 4: Validación completa:** `cargo test; cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo build --release`. Medir tamaño del ejecutable, CPU y RAM en reposo y consumo con salida continua (`1..20000` en PowerShell, o `cargo build` en un proyecto grande) y rellenar la tabla «antes / después» del documento base.
- [ ] **Step 5: Prueba de empaquetado:** `pwsh packaging/package.ps1; pwsh packaging/test-installer.ps1`.
- [ ] **Step 6: Prueba manual en un segundo equipo** (GPU Intel antigua, VM o Windows ARM con emulación x64) antes de publicar; anotar el resultado en el documento base. Sin esta comprobación no se declara «compatible con todos los equipos».
- [ ] **Step 7: Commit** `refactor(ui)!: eliminar el render por software; solo D3D11 con WARP`.
