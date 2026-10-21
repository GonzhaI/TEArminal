# Medidas base antes de la migración a GPU

Fecha: 2026-10-21. Rama `feat/gpu-renderer`, versión `0.11.0` (`Cargo.toml`), commit base `d965a06`.
Este documento fija las cifras de referencia que la migración del renderer (ver
`2026-10-21-gpu-renderer-design.md`) debe comparar. Todas las cifras son de una sola ejecución en la
máquina descrita abajo; no hay repeticiones ni media.

## Máquina de medida

| Elemento | Valor |
|---|---|
| Sistema | Windows 11 Home Single Language, versión `10.0.26300` (build 26300) |
| GPU 1 | AMD Radeon 740M Graphics, driver `32.0.31033.3` (fecha 28-07-2026) |
| GPU 2 | NVIDIA GeForce RTX 3050 Laptop GPU, driver `32.0.16.1742` (fecha 29-09-2026) |

Qué GPU presenta la ventana no se ha comprobado. `AdapterRAM` de `Win32_VideoController` es un campo de
32 bits y no es fiable (la RTX 3050 aparece como 4 GB truncados); por eso no se cita.

## Medidas de rendimiento (`cargo test --release bench_ -- --nocapture`)

Salida real de la ejecución (8 benchmarks, todos `ok`):

| Benchmark | Resultado |
|---|---|
| `bench_csi_cursor_and_erase_churn` | 250 fotogramas TUI (1,44 MB) en 18,14 ms: 79,24 MB/s |
| `bench_throughput_massive_stream` | 1,76 MB, 20 000 líneas en 34,44 ms: 48,74 MB/s, 580 761 líneas/s |
| `bench_massive_scrollback_100k_lines` | 100 000 líneas (9 MB) en 187,59 ms: 51,86 MB/s, 533 085 líneas/s |
| `bench_sdf_rounded_rect_and_stroke` | 12 tarjetas 820x64: 1,543 ms/iteración |
| `bench_render_file_viewer_code_and_markdown` | 1920x1080: código Rust 2,86 ms/fotograma; vista previa Markdown 2,39 ms/fotograma |
| `bench_render_pane_full_frame` | 1920x1080 (210x53 celdas): 2,66 ms/fotograma; `buffer.fill` 0,29 ms |
| `bench_render_ui_chrome` | barra lateral + barra superior + barra de estado 0,22 ms/fotograma; página de Ajustes 5,44 ms/fotograma |
| `bench_viewer_wrap_edit_and_selection` | 200 ciclos (inserción, ajuste, git, selección, borrado sobre 1000 líneas): 4 458,96 µs/ciclo |

Los tests de rendimiento sin el prefijo `bench_` (p. ej. `test_render_frame_latency_and_fps_benchmark`) no
están incluidos.

## Proceso en reposo (`target\release\axon.exe`)

Medido con `Start-Process`, 8 s de arranque y después 30 s sin interacción, una sola pestaña (shell
`pwsh`, configuración por defecto):

| Medida | Valor |
|---|---|
| CPU consumida en 30 s | 93,8 ms, es decir 0,31 % de un núcleo |
| Working set | 120,4 MB |
| Memoria privada (`PrivateMemorySize64`) | 135,5 MB |
| Hilos | 45 |
| Procesos hijos | `pwsh.exe`, `conhost.exe` (su CPU y memoria no se incluyen en las cifras de arriba) |

Tamaño de `target\release\axon.exe`: 5 983 744 bytes (5,98 MB).

## Notas

- La medida de CPU se tomó con la ventana visible y sin tocarla. No se midió en reposo con varias pestañas
  ni con salida continua del shell.
- Lanzar la aplicación crea su configuración y sesión en `%APPDATA%\axon`, como en uso normal.
- Las escenas de referencia de la UI (hashes en `src/renderer/golden/hashes.txt`, test
  `test_golden_scene_hashes`) son la otra base de comparación: cualquier cambio de píxeles en ellas debe ser
  intencional y re-bendecido con `AXON_BLESS=1`.

## Etapa 2 — GPU frente a CPU

Medido con `cargo test test_gpu_scenes_match -- --nocapture` (dispositivo WARP) sobre las 10 escenas de referencia
a 1440x900. Cada escena se graba una vez (`build_recorded_scenes`), se rasteriza con la CPU y se dibuja con
`GpuRenderer::render_to_pixels`; la firma de la CPU coincide exactamente con `golden/signatures.bin`. Se midieron
dos dispositivos: WARP con el nivel de funciones por defecto (FL 11_1) y WARP limitado a FL 10_0
(`GpuDevice::new_at_most`). **Los números son idénticos en ambos niveles** y también con el dispositivo de
hardware de esta máquina (`preview_gpu_vs_cpu`).

| Escena | Distancia de firma | Dif. máx. por canal | Píxeles > 2 | Dif. media por canal |
|---|---|---|---|---|
| terminal-vertical | 0,0070 | 2 | 0 (0 %) | 0,0094 |
| tabs-superiores | 0,0094 | 2 | 0 (0 %) | 0,0083 |
| explorador-acoplado | 0,0064 | 2 | 0 (0 %) | 0,0100 |
| ajustes | 0,0111 | 2 | 0 (0 %) | 0,0098 |
| visor | 0,0076 | 2 | 0 (0 %) | 0,0100 |
| panel-derecha | 0,0105 | 2 | 0 (0 %) | 0,0100 |
| wp1-vertical | 0,0082 | 2 | 0 (0 %) | 0,0097 |
| wp1-superior | 0,0088 | 2 | 0 (0 %) | 0,0098 |
| paneles-divididos | 0,0164 | 2 | 0 (0 %) | 0,0169 |
| bloques-osc133 | 0,0123 | 2 | 0 (0 %) | 0,0125 |

Umbrales fijados en `src/renderer/test_support.rs` (`PARITY_TOLERANCE` sigue en 2, sin tocar):

- `SIGNATURE_TOLERANCE = 0,03`: unas 2 veces el peor caso medido (0,0164). Una región de 48x48 px mal pintada
  subiría la distancia al menos 1-2 órdenes de magnitud por encima.
- `SCENE_OUTLIER_FRACTION = 0,001` (0,1 %): píxeles con diferencia por canal > `PARITY_TOLERANCE`. Medido: 0.
- `SCENE_MAX_DIFF = 3`: ningún píxel puede diferir más; medido 2, más 1 por una capa translúcida más (la GPU
  mezcla en coma flotante con redondeo y la CPU trunca: ~1 por capa apilada, como se midió en la tarea 16). Una
  diferencia mayor es un error de shader o de codificación, no de redondeo.

Revisión visual (`cargo test preview_gpu_vs_cpu -- --ignored --nocapture`, imágenes en
`%TEMP%xon-gpu-parity`: `<escena>-cpu.png`, `-gpu.png` y `-diff.png` = diferencia absoluta x16): en
`terminal-vertical`, `ajustes` y `visor` la diferencia ampliada x16 es casi negra, con trazas de 1-2 niveles
sobre los bordes de los glifos y los trazos de los bordes redondeados; no hay ninguna región, esquina ni
tarjeta con error.

### Coste en CPU de un fotograma (`bench_gpu_encode_and_submit_full_frame`)

Mismo fotograma que `bench_render_pane_full_frame` (1920x1080, 210x53 celdas, 3 552 operaciones grabadas). Mide
el tiempo de CPU de grabar + `GpuRenderer::render` (encode, subida del buffer de instancias y llamadas de
dibujo) sobre una textura fuera de pantalla, NO el tiempo de GPU. 200 iteraciones tras calentar; al final se
vacía la cola con `Flush` y la lectura de un píxel.

| Ruta (misma sesión, `cargo test --release bench_`) | ms/fotograma de CPU |
|---|---|
| CPU: grabar + rasterizar (`bench_render_pane_full_frame`) | 0,83 / 0,85 / 0,83 |
| GPU, hardware (D3D 11_1): grabar + encode/envío | 0,29 / 0,21 / 0,25 (grabar 0,08; envío 0,13-0,20) |
| GPU, WARP (`AXON_RENDERER=warp`): grabar + encode/envío | 0,42 / 0,37 (grabar 0,08) |

Vaciar la cola tras 200 fotogramas tardó 16-19 ms en hardware (unos 0,09 ms de GPU por fotograma como cota
superior) y 520 ms en WARP (2,6 ms por fotograma: WARP rasteriza por software).

### Salvedades

- Una sola máquina y pocas ejecuciones; las cifras de la tabla de arriba (2,66 ms para el mismo benchmark de CPU)
  se tomaron en otra sesión con la máquina más cargada, así que **la comparación válida es la de esta tabla,
  tomada en la misma sesión**: el coste de CPU de la ruta GPU es ~3-4 veces menor que el de la rasterización.
- Los umbrales de paridad se midieron en WARP; el hardware de esta máquina dio las mismas cifras, pero otros
  controladores pueden redondear distinto. Los tests de la suite solo usan WARP.
- El coste de GPU real en hardware no se midió con consultas de temporización; la cifra de vaciado es una cota.
