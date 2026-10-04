# Axon ⚡

> **Terminal veloz, acelerada por GPU y diseñada para la era de la IA en Windows.**

---

## 🧠 Filosofía y Características

Axon no es un simple emulador de terminal. Está concebida como el puente de alta velocidad entre el desarrollador, sus flujos de datos densos y los modelos/agentes de Inteligencia Artificial:

1. **Rendimiento Nativo en Windows:**
   * Utiliza la API nativa de **Windows ConPTY** (`portable-pty`) para ejecutar PowerShell, CMD o WSL con soporte total de interactividad y colores ANSI.
   * Renderizado acelerado por **GPU (DirectX 12 / Vulkan)** a 60-144 FPS estables sin congelamiento de ventana.

2. **Copiloto de IA Integrado (Sidecar Panel):**
   * Panel lateral interactivo conmutable con `Ctrl + I`.
   * **Diagnóstico automático de errores:** Extrae el contexto reciente del buffer de la terminal y consulta al modelo de IA cómo resolver el fallo.
   * Soporte para modelos locales vía **Ollama** (sin costo ni latencia de red) y APIs en la nube.

3. **Arquitectura Modular en Rust:**
   * `src/pty.rs`: Conexión de entrada/salida no bloqueante con el proceso del shell mediante hilos dedicados.
   * `src/terminal.rs`: Máquina de estados VT (`vte`) con soporte SGR ANSI (24-bit TrueColor), scrollback buffer y cursor interactivo.
   * `src/ai.rs`: Cliente asíncrono con runtime Tokio para inferencia de modelos sin bloquear el bucle de renderizado.
   * `src/ui.rs`: Interfaz de usuario inmediata acelerada por hardware con auto-redimensionamiento de columnas y filas.
   * `src/config.rs`: Configuración personalizable guardada en archivo TOML.

---

## 🛠️ Requisitos Previos para Compilar en Windows

1. **Rust:** Instalar toolchain oficial (`rustup`).
2. **Visual Studio C++ Build Tools:** Necesario para el enlazador nativo de Windows (`link.exe`).
3. **Control Inteligente de Aplicaciones (Smart App Control):**
   * Si Windows 11 tiene activo el *Control inteligente de aplicaciones* en modo bloqueo, impedirá la ejecución de compiladores locales sin certificado comercial. Para compilar código localmente, se desactiva en:
     `Seguridad de Windows -> Control de aplicaciones y explorador -> Control inteligente de aplicaciones`.

---

## 🚀 Compilación y Ejecución

```powershell
# Ir a la carpeta del proyecto
cd C:\Users\gmora\dev\axon

# Compilar en modo desarrollo
cargo build

# Ejecutar el binario generado
cargo run

# O compilar en modo Release optimizado para máxima velocidad
cargo build --release
# El ejecutable quedará en: target\release\axon.exe
```
