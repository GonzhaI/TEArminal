# Migración del renderer a GPU (Direct3D 11) — diseño

Fecha: 2026-10-21 · Estado: pendiente de revisión

## Objetivo

Que todo el fotograma (terminal, barras, paneles, visor, Ajustes) se componga en la GPU con un atlas de glifos y *shaders*, y que la CPU solo prepare datos. Se busca margen a 4K y alta frecuencia de refresco, y dejar viable añadir efectos más adelante.

Destino final: **solo Direct3D 11, con WARP como respaldo**. Se eliminan `softbuffer`, el rasterizado en CPU y las primitivas sobre `&mut [u32]`.

## Decisiones tomadas

| Decisión | Elegido | Alternativas descartadas |
| :--- | :--- | :--- |
| API | Direct3D 11 directo (`windows` 0.52, ya en `Cargo.toml`) | `wgpu` (más tamaño y RAM, DX12/Vulkan con más casos raros en hardware antiguo); Direct2D/DirectWrite (cambia el aspecto del texto y obliga a reescribir fuentes) |
| Respaldo sin GPU | WARP de D3D11 | Mantener `softbuffer` (dos renderers que sincronizar) |
| Estrategia | Etapas, con el rasterizador CPU como referencia temporal | Reescritura de golpe (sin forma fiable de detectar regresiones de píxeles) |

## Contexto del código actual

* Todo el dibujo escribe en un `&mut [u32]` desde `renderer/` y `ui/` (≈11 000 líneas sin tests). Primitivas: `fill_rect`, `blit_glyph`, `draw_rounded_rect`/`stroke_rounded_rect` (SDF) e imágenes RGBA.
* `renderer/d3d11.rs` (179 líneas) solo presenta un fotograma ya pintado: crea dispositivo y *swap chain* DXGI Flip-Model.
* Operaciones que leen o postprocesan el framebuffer: mezcla de bordes anti-aliased (`ui/paint.rs`, `pane.rs`), `frame_card` (recorta lo ya dibujado), Ajustes (dibuja fuera de pantalla y copia, `settings_page.rs`) y el panel derecho (búfer propio y copia, `app/redraw.rs`).
* Los glifos salen de cachés (`fontdue` + `rustybuzz`) con mapa de cobertura de 8 bits, colocados en píxeles enteros. No hay emoji a color.
* `renderer/tests.rs` (≈2 300 líneas) y la regla de `AGENTS.md` «compara hashes de fotogramas» dependen de tener los píxeles en CPU.
* Hoy `create_device` solo pide `D3D_DRIVER_TYPE_HARDWARE` (si falla se cae a `softbuffer`) y no maneja `DXGI_ERROR_DEVICE_REMOVED/RESET`.

## Arquitectura

```text
src/renderer/
├── scene.rs        # Scene: lista de quads + pila de recorte/desplazamiento. Única API de dibujo.
├── raster_cpu.rs   # Scene → &mut [u32]. SOLO referencia y tests; se borra en la etapa 3.
└── gpu/
    ├── device.rs   # HARDWARE (adaptador del monitor) → WARP; swap chain; recuperación de dispositivo
    ├── atlas.rs    # atlas R8 de glifos (1024² creciendo hasta 4096²) + texturas de imagen RGBA
    ├── pipeline.rs # shaders SM 4.0 precompilados y embebidos, blend, input layout
    └── shaders/quad.hlsl
```

Las ~90 funciones de pantalla cambian `buffer: &mut [u32]` por `scene: &mut Scene`; su lógica y su geometría (`geometry.rs`) no cambian. `d3d11.rs` desaparece, absorbido por `gpu/device.rs`.

`Scene` conserva su memoria entre fotogramas (sin reservas por fotograma).

### Primitivas y shader

* Un par vertex/pixel shader con quads instanciados (~32 bytes por instancia). Tipos: `Solid`, `RoundedFill` y `RoundedStroke` (SDF, como hoy), `Glyph` (cobertura del atlas × color), `Image` (RGBA) y `CardCorner`.
* Mezcla alfa por hardware sobre destino UNORM (no sRGB), para conservar la mezcla en espacio gamma de hoy.
* Coordenadas enteras: los glifos siguen en píxeles enteros, igual que el `round()` actual.
* **Recorte:** cada instancia lleva su rectángulo de recorte y el *pixel shader* descarta lo que cae fuera (equivale al `Clip` actual de cada primitiva y evita cambiar el *scissor* entre llamadas). **Desplazamiento:** `Canvas::with_offset` traslada las coordenadas al grabar la escena.
* Las instancias consecutivas con la misma textura (atlas, imagen o ninguna) se dibujan con una llamada instanciada.

### Casos que hoy leen o copian píxeles

| Hoy (CPU) | GPU |
| :--- | :--- |
| `frame_card` recorta lo ya dibujado | Últimas instancias del fotograma: márgenes y esquinas con el color `outer` y cobertura SDF |
| Ajustes dibuja fuera de pantalla y copia | `Canvas::with_offset` con recorte a la ventana visible y el scroll de Ajustes |
| Panel derecho en búfer propio y copia | `Canvas::with_offset` con `panel_x` |
| Bordes que leen el destino | Blending de hardware |

## Robustez

* **Dispositivo:** adaptador del monitor (como hoy) → `D3D_DRIVER_TYPE_WARP` si falla. *Feature level* mínimo 10_0, por lo que los shaders son SM 4.0 y el atlas máximo es 8192².
* **Dispositivo perdido** (`DEVICE_REMOVED/RESET`): se recrean dispositivo, *swap chain* y atlas; el atlas se rellena a demanda desde las cachés de glifos, y el fotograma siguiente se pinta completo.
* **Atlas lleno:** se vacía al inicio del fotograma siguiente y se vuelve a llenar a demanda. Es simple; si resultara insuficiente se cambia por un reemplazo por uso.
* **Shaders:** se compilan en desarrollo con `tools/compile-shaders.ps1` y el bytecode va embebido (sin compilación en el equipo del usuario ni dependencia de `d3dcompiler_47.dll`). Un test comprueba que el bytecode corresponde al HLSL del repositorio.
* **Selección de renderer:** `AXON_RENDERER=warp` fuerza WARP (diagnóstico y CI). La clave `terminal.gpu` se acepta en configuraciones antiguas, pero se ignora.
* **Compatibilidad declarada:** Windows 10 1809+ x64, con el mismo mínimo de hoy. WARP cubre VM, Escritorio remoto y equipos sin driver.

## Etapas

Cada etapa deja el proyecto compilando y con tests en verde.

0. **Base:** hashes de fotogramas de todas las pantallas y un `bench_` de referencia (CPU por fotograma, RAM e inactividad), antes de cambiar nada.
1. **Escena + rasterizador CPU:** todo el dibujo se porta a `Scene`. El resultado debe ser **idéntico bit a bit** al de hoy (mismos hashes). `raster_cpu.rs` replica las primitivas actuales.
2. **Backend GPU:** dispositivo, WARP, recuperación, atlas y shaders. Render sin ventana con WARP y comparación contra el rasterizador CPU **con tolerancia por canal** (los bordes SDF no serán bit-idénticos; la tolerancia se fija aquí, con las diferencias reales medidas). La GPU pasa a ser el camino por defecto.
3. **Borrado:** `raster_cpu.rs`, `softbuffer`, las primitivas sobre `u32` de `ui/paint.rs` y `renderer/d3d11.rs`. Los tests de píxeles leen la textura de la GPU con WARP. Se actualizan `AGENTS.md` y `README.md`.

## Pruebas

* **Etapa 1:** igualdad exacta de hashes contra la base de la etapa 0.
* **Etapa 2:** render sin ventana a una textura con WARP, lectura por *staging texture* y comparación con la referencia CPU, pantalla por pantalla.
* **Dispositivo perdido:** hook interno de test que fuerza la recreación del dispositivo y comprueba que el siguiente fotograma coincide con el anterior.
* **Compatibilidad:** ejecución con `AXON_RENDERER=warp` y con *feature level* 10_0 forzado.
* **Rendimiento:** `cargo test --release bench_ -- --nocapture`, comparado con la base.
* **Etapa 3:** los tests existentes de `renderer/tests.rs` se migran a lectura de la GPU.

## Criterios de éxito

* Sin regresiones visuales dentro de la tolerancia fijada en la etapa 2.
* CPU por fotograma a 1080p ≤ la base actual (~1,3 ms).
* CPU en reposo ~0 %. La RAM en reposo se mide antes y después y se informa; no se promete que baje, porque cargar el driver D3D11 ya ocurre hoy con `gpu = true`.
* Funciona con WARP y con *feature level* 10_0 forzado, y se recupera de un dispositivo perdido.
* `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets` y `cargo build --release` en verde; `packaging/test-installer.ps1` si cambia el empaquetado.

## Fuera de alcance

Repintado parcial por filas sucias, emoji a color, transparencia o desenfoque de ventana, y otras plataformas. La escena hace viables esas mejoras después.

## Riesgos y límites conocidos

* Solo se puede probar en un equipo físico y en WARP. «Funciona en todos los Windows» no queda demostrado hasta probarlo en otros equipos (GPU Intel antigua, VM, Windows ARM con emulación x64): se pide una prueba manual en al menos uno antes de publicar.
* La mezcla de bordes SDF cambiará en el nivel de un par de valores por canal frente a la CPU; la tolerancia se documentará con los valores medidos.
* Es el cambio más grande del proyecto: toca prácticamente todo `renderer/` y `ui/`, y conviene que cada etapa se fusione por separado.
