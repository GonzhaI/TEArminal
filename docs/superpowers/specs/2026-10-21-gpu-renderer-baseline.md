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
