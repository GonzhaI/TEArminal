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

## T19 — medidas con GPU por defecto

`targeteleasexon.exe` con la GPU como único renderer (Direct3D 11; `AXON_RENDERER=warp` fuerza WARP). Mismo
protocolo que la base (`Start-Process`, sesión restaurada con 2 pestañas y un panel dividido, 8 s de arranque y
30 s sin interacción, ventana visible y enfocada). **Son ejecuciones únicas, sin repeticiones ni media**, y las
capturas de pantalla (`%TEMP%xon-gpu-run\{default,warp}.png`) solo muestran el primer fotograma: barra
superior, explorador, dos terminales y barra de estado se dibujan completos en ambos modos.

| Medida | Base (CPU + softbuffer/D3D11 viejo) | GPU por defecto (hardware) | `AXON_RENDERER=warp` |
|---|---|---|---|
| CPU en 30 s | 93,8 ms (0,31 %) | 78 ms (0,26 %) | 1 141 ms (3,8 %) |
| Working set | 120,4 MB | 110,5 MB | 62,5 MB |
| Memoria privada | 135,5 MB | 125,2 MB | 40,5 MB |
| Hilos | 45 | 70 | 24 |

- Con hardware el reposo es igual o algo mejor que la base; los 70 hilos son del controlador gráfico.
- WARP gasta más CPU en reposo (cada parpadeo del cursor se rasteriza por software en el controlador) y mucha
  menos memoria privada; la base tenía una sola pestaña y estas ejecuciones dos, así que la comparación de
  memoria es orientativa.

## T20 — antes / después (build release final, sin render por software)

Tras borrar el rasterizador de la CPU, `softbuffer` y el presentador D3D11 antiguo (commit
`refactor(ui)!: eliminar el render por software; solo D3D11 con WARP`). Mismo protocolo y misma máquina que la
base (`cargo build --release`, `Start-Process` de `targeteleasexon.exe`, 30 s sin interacción tras el arranque;
fotograma con `cargo test --release bench_gpu_encode_and_submit_full_frame -- --nocapture`).

| Medida | Antes (base, CPU + softbuffer) | Después (D3D11, hardware) |
|---|---|---|
| Tamaño de `axon.exe` | 5 983 744 B (5,98 MB) | 5 942 272 B (5,94 MB) |
| CPU en reposo, 30 s | 93,8 ms (0,31 %) | 0 / 109 / 203 ms (0,00 / 0,36 / 0,68 %), tres ejecuciones |
| Working set en reposo | 120,4 MB | 111,0-111,3 MB |
| Memoria privada en reposo | 135,5 MB | 125,0-126,2 MB |
| Hilos | 45 | 70 (los crea el controlador gráfico) |
| Coste de CPU de un fotograma 1920x1080 (210x53 celdas, 3 552 ops) | 2,66 ms (`bench_render_pane_full_frame`, grabar + rasterizar, sesión de la base); 0,83-0,85 ms en la sesión de la Etapa 2 | 0,24 / 0,24 / 0,30 ms (grabar 0,08 + encode/envío 0,16-0,22), tres ejecuciones |

Salvedades, sin maquillar:

- **Ejecuciones únicas en una sola máquina**, sin media ni intervalo de confianza. La CPU en reposo varía de 0 a
  203 ms entre ejecuciones idénticas (el parpadeo del cursor solo se pinta con la ventana enfocada y visible,
  y Windows no siempre la deja así): la única afirmación sostenible es "por debajo del 1 % de un núcleo".
  Una cuarta ejecución, con la ventana tapada por otra aplicación, dio 94 ms y no se cuenta como medida de la
  ventana visible.
- La base se midió con **una pestaña** y configuración por defecto; las medidas finales, con la **sesión
  restaurada** (2 pestañas, un panel dividido y el explorador). La memoria y los hilos no son comparables uno a
  uno. Los 70 hilos son del controlador gráfico (en WARP la tarea 19 midió 24).
- El fotograma de "antes" es grabar + rasterizar en la CPU; el de "después" es el coste de CPU de grabar +
  codificar + enviar, **no** el tiempo de GPU (el vaciado de la cola tras 200 fotogramas tardó ~16 ms, unos
  0,08 ms por fotograma como cota superior). La comparación válida es la de la misma sesión (0,83 frente a
  0,24-0,30 ms).
- Las cifras del encabezado de `README.md` (4,2 MB, 47 MB, 1,3 ms) eran anteriores a esta base y no
  coincidían con ella; se sustituyeron por las de esta tabla.
- `src/renderer/golden/hashes.txt` y `test_golden_scene_hashes` (mencionados en las notas de arriba) se
  eliminaron: los hashes exactos de la CPU no pueden cumplirse con la salida de la GPU. Las referencias son ahora
  los PNG de `src/renderer/golden/{refs,scenes}` y `signatures.bin`, congelados con la CPU y comparados con
  tolerancia.
- Pendientes de la tarea 20 que no se han hecho aquí: el empaquetado (`packaging/package.ps1` y
  `test-installer.ps1` los ejecuta la CI) y la prueba manual en un segundo equipo (GPU Intel antigua, VM o
  Windows ARM). Sin esa prueba no se declara "compatible con todos los equipos".
