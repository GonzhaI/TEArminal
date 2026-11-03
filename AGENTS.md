# AGENTS.md — Guía para agentes de IA y contribuidores de TEArminal

Terminal nativa para Windows desarrollada en Rust (ConPTY, `winit`, Direct3D 11). Este documento reúne los principios de diseño, la arquitectura y las reglas de plataforma esenciales.

---

## 1. Arquitectura

```text
src/
├── main.rs            # Inicialización, variables de entorno y arranque del EventLoop
├── app.rs & app/      # Estado global (TEArminalApp), host winit, eventos, teclado/ratón, tabs y spaces
├── pty.rs             # Gestión de procesos ConPTY (portable-pty), hilos I/O y saneamiento
├── terminal.rs        # Emulador VT/ECMA-48 (vte, rejillas de celdas, atributos, scrollback)
├── renderer.rs & renderer/
│   ├── canvas.rs      # API de grabación de dibujo en memoria (Scene / Op)
│   ├── geometry.rs    # ÚNICA fuente de verdad de rectángulos para renderizado y eventos de clic
│   ├── gpu/           # Backend Direct3D 11 (pipeline, shaders quad.hlsl/.cso, swapchain DXGI Flip)
│   └── *.rs           # Vistas especializadas (pane, explorer, spaces, settings, viewer, modal)
├── explorer.rs        # Árbol de archivos, integración git y operaciones locales
├── theme.rs & config.rs # Temas iTerm2 embebidos, configuración TOML y persistencia de sesión
└── ui/                # Iconos Lucide y utilidades visuales
```

---

## 2. Principios Fundamentales de Ingeniería

Aplica estos principios rigurosamente en cada cambio, adaptados al desarrollo en Rust:

1. **KISS (Keep It Simple, Stupid):** La solución más simple y directa que resuelva el problema por completo. Evita la sobre-ingeniería y estructuras innecesariamente complejas.
2. **YAGNI (You Aren't Gonna Need It):** Implementa estrictamente lo que se necesita hoy. Prohibido crear abstracciones especulativas o funcionalidades "por si acaso".
3. **DRY (Don't Repeat Yourself):** Centraliza la lógica común. Si una geometría, cálculo o parseo se usa en render y en eventos de entrada, debe vivir en un único lugar (ej. [geometry.rs](file:///src/renderer/geometry.rs)).
4. **Separación de Responsabilidades (SoC):** Cada módulo y struct debe tener un propósito único y bien delimitado: el PTY gestiona procesos, Terminal interpreta VT, Canvas graba primitivas y GPU renderiza.
5. **Composición sobre Herencia:** Estructura datos mediante composición de `structs` pequeños y `traits` cohesivos y específicos (Interface Segregation).
6. **Principios SOLID (Uncle Bob):**
   - **S (Single Responsibility):** Divide archivos y métodos con responsabilidades dispares (máximo ~1 000 líneas por módulo).
   - **O (Open/Closed) & L (Liskov):** Diseña mediante traits y enums exhaustivos que permitan extender comportamientos sin alterar el código base.
   - **I (Interface Segregation):** Prefiere traits pequeños y especializados antes que interfaces monolíticas.
   - **D (Dependency Inversion):** El renderizado depende de la abstracción `Canvas`, no de los detalles del hardware de dibujo.
7. **Early Return & Fail Fast:** Valida precondiciones y límites al inicio de las funciones con cláusulas de guarda (`guard`, `let-else`, `if let`, operador `?`). Retorna lo antes posible para reducir indentación.
8. **Inmutabilidad por Defecto:** Usa `let` inmutable siempre que sea posible. Restringe `mut` al alcance mínimo necesario y evita estados globales o mutaciones colaterales.
9. **Nombres Reveladores de Intención:** Variables, funciones y tipos deben expresar claramente *qué representan y por qué existen*. Evita abreviaturas crípticas o nombres genéricos como `data`, `val` o `temp`.
10. **Manejo Explícito de Errores:**
    - Prohibido `.unwrap()` o `.expect()` en código de producción/runtime.
    - Emplea `Result<T, E>` y `Option<T>` con propagación limpia (`?`) o degradación elegante con valores por defecto.
11. **Logging Contextual y Privacidad:**
    - Emplea logs estructurados y contextuales bajo `TEARMINAL_DEBUG=1` o `TEARMINAL_LOG=1`.
    - **Privacidad estricta:** Jamás registres texto tecleado por el usuario, contraseñas o tokens (solo metadatos o longitudes).
12. **Regla del Boy Scout:** Deja el código, tests y documentación más limpios, seguros y legibles de lo que los encontraste al entrar.
13. **Tests Automatizados:** Todo bugfix o funcionalidad nueva debe ir acompañado de sus correspondientes tests unitarios, de integración o validación de regresión visual (golden tests).

---

## 3. Reglas Críticas del Proyecto (TEArminal)

### Rendimiento y UI
* **Grabación antes de pintar:** Ninguna pantalla dibuja píxeles directamente. El código de UI graba operaciones en un `Canvas` y la GPU las ejecuta en batch con D3D11.
* **Geometría unificada:** La posición y tamaño de cualquier elemento interactivo (botones, tabs, scrolls) proviene exclusivamente de [geometry.rs](file:///src/renderer/geometry.rs). Nunca recalcules rectángulos a mano en controladores de clic o hover.
* **Tokens visuales constantes:** Radios, márgenes y contornos deben usar los tokens de diseño (`RADIUS_CARD`, `RADIUS_CONTROL`, etc.).
* **Reposo absoluto:** En ausencia de eventos o animaciones, el uso de CPU debe ser del 0 % (`ControlFlow::Wait`).

### Plataforma Windows y PTY
* **Ejecución silenciosa:** Cualquier proceso auxiliar o test debe ejecutarse sin ventanas visibles ni robar foco (`CREATE_NO_WINDOW` / `hide_console`).
* **Ciclo de vida del shell:** `PtyManager` implementa `Drop` para liquidar los shells hijos y evitar procesos huérfanos.
* **Antirrebote (debounce):** Los redimensionados hacia ConPTY usan debounce (`resize_deadline`) para evitar que PowerShell redibuje prompts repetidamente.
* **Teclado:** Comprueba modificadores físicos mediante `GetAsyncKeyState` (manejo especial de `AltGr`, que genera `VK_LCONTROL` sintético en Windows).

### Terminal, VT y Seguridad
* **Entrada no confiable:** La salida remota, secuencias OSC y listados remotos son potencialmente maliciosos. Valida en el parser y en el sumidero (destinos SSH, rutas seguras, rechazo de metacaracteres de shell).
* **Buffer OSC amplio:** El parser de VT utiliza un buffer de 128 KB para permitir transferencias de archivos y listas SSH sin truncamientos accidentales.
* **Sanitización de pegado:** Se eliminan caracteres de control que puedan alterar o vulnerar comandos en el shell.

---

## 4. Validación

Antes de dar cualquier tarea por concluida, valida en segundo plano:

```powershell
cargo test
cargo fmt --check
cargo clippy --all-targets
cargo build --release
```
