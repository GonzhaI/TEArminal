---
name: Axon
description: Terminal nativa de Windows donde la interfaz desaparece bajo el texto.
colors:
  brand: "#5B9BFF"
  brand-day: "#2563EB"
  canvas: "#141821"
  paper: "#F6F7F9"
  ink: "#D6DEEB"
  ink-day: "#1A2130"
  cursor: "#7CC4FF"
  selection: "#24344D"
  success: "#5BD08A"
  warning: "#F2C66D"
  danger: "#F07178"
  chrome: "#1D212A"
  elevated: "#242831"
  hairline: "#2B3039"
  hairline-strong: "#3F444D"
  text-secondary: "#A0A7B2"
  text-muted: "#797F8A"
  chrome-day: "#ECEDF0"
  hairline-day: "#DCDDE1"
  text-secondary-day: "#585D68"
typography:
  ui:
    fontFamily: "Inter, Segoe UI, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.25
  ui-title:
    fontFamily: "Inter, Segoe UI, sans-serif"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: 1.25
  ui-detail:
    fontFamily: "Inter, Segoe UI, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.2
  ui-label:
    fontFamily: "Inter, Segoe UI, sans-serif"
    fontSize: "9.5px"
    fontWeight: 600
    letterSpacing: "1px"
  terminal:
    fontFamily: "JetBrains Mono, Cascadia Mono, monospace"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.25
rounded:
  hairline: "2px"
  control: "4px"
  card: "6px"
spacing:
  unit: "4px"
  chrome-pad: "12px"
  pane-x: "14px"
  pane-y: "10px"
components:
  button-primary:
    backgroundColor: "{colors.brand}"
    textColor: "{colors.canvas}"
    rounded: "{rounded.control}"
    height: "26px"
    padding: "0 12px"
  button-secondary:
    backgroundColor: "{colors.elevated}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: "26px"
    padding: "0 12px"
  chrome-action:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
    size: "30px"
  tab-active:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    height: "38px"
    width: "190px"
  tab-inactive:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.text-secondary}"
    height: "38px"
    width: "190px"
  session-row:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: "42px"
    padding: "0 10px"
  session-row-active:
    backgroundColor: "{colors.elevated}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: "42px"
    padding: "0 10px"
  card:
    backgroundColor: "{colors.elevated}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "16px"
  input:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: "28px"
    padding: "0 10px"
  context-menu:
    backgroundColor: "{colors.elevated}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "4px"
---

# Design System: Axon

## Overview

**Creative North Star: "El estándar del oficio"**

Axon se comporta como una herramienta bien construida: el chrome es una regla fina alrededor del contenido, no una marca que se presenta. La referencia es el software de trabajo nativo que lleva años ganándose la confianza de quien lo usa a diario —VS Code, Windows Terminal—, ejecutado con el acabado de un producto propio: alineaciones exactas, hairlines de un píxel, estados inequívocos y ninguna decoración que no signifique algo.

El mundo es grafito azulado, tinta hueso y un solo azul vivo. Todo color de interfaz se deriva del tema activo del terminal, de modo que los 817 temas de iTerm2 sigan siendo usables: la UI nunca impone una paleta ajena al texto. El resultado es sobrio de día y de noche, denso sin estar apretado, y estable durante jornadas de ocho horas.

La ambición no está en el ornamento sino en la precisión: la pestaña activa se funde con el lienzo, el canal OSC 133 marca en el margen qué comando falló, la barra de estado cabe en una línea y el explorador es un panel acoplado, no una tarjeta flotante. La firma visual de Axon es información, no adorno.

**Key Characteristics:**
- Grafito azulado y tinta hueso; azul de marca único para lo vivo y el foco.
- Superficies planas separadas por hairlines de 1 px; sombra solo en flotantes.
- Radios de 2–6 px; prohibidas las píldoras, los degradados y los emojis.
- Inter para el lenguaje, JetBrains Mono para valores de máquina; micro-etiquetas en mayúsculas con tracking.
- Retícula de 4 px, chrome vertical contenido (38 px arriba, 26 px abajo).
- Un solo gesto decorativo permitido: el canal de bloques OSC 133, porque informa.

## Colors

Paleta de dos familias —grafito/tinta y azul de marca— con verde, ámbar y rojo reservados al estado. No hay color decorativo.

### Primary
- **Azul de marca** (#5B9BFF): acento en modo oscuro. Foco de teclado, pestaña activa (regla superior de 2 px), rama git, valores vivos, botón primario. Su escasez es lo que lo hace legible como señal.
- **Azul de marca diurno** (#2563EB): el mismo rol en modo claro, oscurecido para mantener 4,5:1 sobre papel.

### Neutral
- **Grafito de lienzo** (#141821): fondo del terminal y de la pestaña activa; el color contra el que se mide todo lo demás.
- **Tinta hueso** (#D6DEEB): texto de terminal e interfaz en oscuro; nunca blanco puro.
- **Papel** (#F6F7F9) y **tinta diurna** (#1A2130): las contrapartes del tema claro.
- **Chrome** (#1D212A) y **elevado** (#242831): barras, pestañas inactivas, filas activas, tarjetas. Se obtienen mezclando fondo y texto (4,5 % y 8 %), nunca con un gris ajeno al tema.
- **Hairline** (#2B3039) y **hairline fuerte** (#3F444D): separadores de 1 px y bordes de control o foco.
- **Texto secundario** (#A0A7B2) y **texto atenuado** (#797F8A): metadatos que se leen, no decoración. Mantienen ≥ 4,5:1 sobre el fondo en los temas propios.

### Tertiary
- **Verde de éxito** (#5BD08A): código de salida 0, estado conectado, confirmaciones. Siempre acompañado de forma o etiqueta.
- **Ámbar de aviso** (#F2C66D): cuota próxima al límite, advertencias no bloqueantes.
- **Rojo de fallo** (#F07178): código de salida distinto de 0, errores, cierre destructivo.

### Named Rules
**The One Accent Rule.** El azul de marca ocupa menos del 10 % de cualquier pantalla. Si dos elementos compiten por él, uno deja de ser importante.

**The Theme Derivative Rule.** Ningún color de chrome se escribe a mano: se deriva del tema con `mix(fondo, texto, t)`. Un tema de iTerm2 desconocido debe seguir produciendo una interfaz coherente y legible.

**The State Is Not Color Alone Rule.** Éxito, aviso y error llevan además un punto, un icono o una etiqueta. El color refuerza; nunca es la única señal.

## Typography

**Display Font:** ninguna; la jerarquía se construye con peso y espaciado, no con tamaños grandes.
**Body Font:** Inter (con Segoe UI de respaldo).
**Mono Font:** JetBrains Mono (terminal, rutas, dimensiones, atajos y valores de máquina).

**Character:** Inter aporta neutralidad y buena lectura en tamaños pequeños; JetBrains Mono aporta la textura técnica del terminal. La interfaz nunca usa una fuente "de sistema operativo" genérica ni una display decorativa.

### Hierarchy
- **UI title** (600, 13 px): nombre de pestaña, título de archivo, encabezado de panel.
- **UI body** (400, 13 px): etiquetas de ajustes, textos de menú, contenido de tarjetas.
- **UI detail** (400, 11 px): segunda línea de una sesión, descripción de una opción, ruta contenedora.
- **UI label** (600, 9,5 px, mayúsculas, tracking 1–1,5 px): micro-etiquetas de sección (`SESIONES`, `AGENTES Y HERRAMIENTAS`), marca `AXON`.
- **Terminal** (400, 15 pt por defecto, altura de línea 1,25): texto del PTY con la fuente monoespaciada elegida por el usuario.
- **Números y valores** (mono): dimensiones `cols × rows`, porcentajes, versiones y fechas.

Toda la tipografía de UI escala con `ui_font_size` (base 13 px, factor 0,6–2,5) y con el escalado de Windows.

### Named Rules
**The Two Voices Rule.** Inter habla; JetBrains Mono mide. Un valor de máquina en Inter o un texto de interfaz en mono es un error de sistema, no una variante.

**The Quiet Caps Rule.** Las mayúsculas solo existen como micro-etiqueta de 9,5 px con tracking. Nunca un titular en mayúsculas.

## Layout

Retícula de 4 px con chrome mínimo: 38 px de barra superior, 26 px de barra de estado. Entre ambas queda todo el contenido; el chrome vertical nunca supera 64 px.

- **Barra superior:** bloque de marca de 76 px, pestañas contiguas (separación 0, ancho 20–190 px, altura completa de 38 px), botón `+` de 32 px, acciones de 30×30 px y tres controles de ventana de 46 px. La pestaña activa comparte el fondo del lienzo y se separa con una regla azul de 2 px en el borde superior.
- **Barra lateral:** 190 px por defecto, 38 px de cabecera, lista desde y = 66, ítems de 42 px con 2 px de separación y dos líneas de texto. No es una colección de tarjetas.
- **Lienzo de terminal:** márgenes de 14 px horizontales y 10 px verticales; borde de foco de 2 px; scrollbar de 4 px auto-oculta; botón "ir abajo" de 26 px.
- **Explorador:** panel acoplado a la derecha, 240 px por defecto (mínimo 160, máximo proporcional a la ventana), sin radios ni sombra. Ocupa el alto completo entre las dos barras y el terminal le reserva exactamente su ancho.
- **Visor:** cabecera de 38 px, gutter de números calculado por dígitos (`dígitos × ancho de carácter + 24`), texto a 14 px del separador y 18 px del borde derecho.
- **Ajustes:** contenido bajo la navegación desde y = 128; tarjetas de radio 6 px; filas de tema de 26 px; buscador y listas comparten la misma geometría que el clic.
- **Barra de estado:** una sola línea de 26 px con segmentos separados por hairlines; popup de consumo de IA de 392 px.

La densidad es alta pero con ritmo: 4 px entre elementos relacionados, 8–12 px entre grupos, 16 px de padding interno en tarjetas. Nada de separadores dobles ni cajas dentro de cajas.

### Named Rules
**The 64-Pixel Rule.** Barra superior (38) más barra de estado (26) suman 64 px. Cualquier chrome nuevo debe caber ahí o justificar por qué el texto pierde esa altura.

**The Single Geometry Rule.** Dibujo y zona de clic leen la misma estructura (`TopTabBarRects`, `SettingsPageRects`, `FileViewerLayout`). Recalcular geometría en el manejador de clic es un bug, no una optimización.

## Elevation & Depth

El sistema es plano. La profundidad se comunica con tono y líneas, no con sombras: chrome al 4,5 %, superficie elevada al 8 %, hairline al 12 % y hairline fuerte al 22 %, siempre mezclas del tema. No hay sombras en barras, pestañas, paneles, tarjetas ni botones.

La única excepción son las superficies que flotan sobre el contenido y deben separarse del texto: menú contextual y popup de consumo de IA. Ahí se usa un borde hairline y un fondo elevado; la sombra, si aparece, es difusa y de un solo nivel.

### Named Rules
**The Flat-By-Default Rule.** Una superficie en reposo no proyecta sombra. Si necesitas una sombra para que algo se entienda, el problema es la jerarquía, no la elevación.

**The Floating Exception Rule.** Solo menús y popups flotan. Un panel de trabajo (explorador, ajustes, visor) está acoplado, no suspendido.

## Shapes

Formas rectilíneas con radios pequeños: 2 px para chips y marcas, 4 px para botones, filas y campos, 6 px para tarjetas y flotantes. El radio nunca supera la mitad de la altura del elemento, lo que prohíbe de facto las píldoras.

Los bordes son hairlines de 1 px; el foco de teclado usa un contorno de 2 px en azul de marca. Las pestañas no tienen radio inferior: la activa se funde con el lienzo. El explorador y las barras no tienen radio. Los iconos son Lucide, de trazo, a 14–18 px; no hay iconos rellenos ni emojis.

### Named Rules
**The No-Pill Rule.** Un rectángulo redondeado en sus extremos pertenece al vocabulario de marketing, no al de una terminal. Los badges son rectángulos de 2 px de radio o puntos de estado.

**The Instrument Rule.** Cada forma existe para indicar una zona, un estado o una acción. Si al quitarla no se pierde información, sobra.

## Components

### Buttons
- **Shape:** rectángulos de 26–30 px de alto, radio 4 px.
- **Primary:** fondo azul de marca, texto grafito, padding 0 12 px. Solo una acción primaria por superficie.
- **Secondary:** fondo elevado, texto tinta, hairline de 1 px.
- **Ghost:** sin fondo en reposo; hover con superficie al 12 %. Es el estado por defecto de las acciones del chrome.
- **Hover / Focus:** el hover cambia el tono, no el tamaño; el foco añade un contorno azul de 2 px. Sin transiciones que despierten el bucle de eventos.

### Chrome actions
- **Style:** 30×30 px, radio 4 px, icono Lucide de 14 px en texto secundario.
- **States:** hover con fondo elevado y texto tinta; activo con relleno azul translúcido (alfa 30) y icono azul.

### Tabs
- **Style:** contiguas, sin separación, altura 38 px, ancho 20–190 px según el número de pestañas. Inactivas sobre chrome; activa sobre el lienzo con regla azul de 2 px arriba.
- **Content:** icono de 14 px, título de 13 px y cierre de 16 px que solo aparece en hover o pestaña activa.
- **States:** el hover aclara el fondo; el modo compacto (más de las que caben) reduce a icono centrado.

### Session rows (sidebar)
- **Style:** fila de 42 px a todo el ancho, radio 4 px, título de 13 px y detalle de 11 px en texto atenuado.
- **States:** activa con fondo elevado; hover con superficie al 12 %. El detalle describe contexto real (padre del archivo, host SSH, número de paneles), no una etiqueta genérica.

### Cards / Containers
- **Corner Style:** radio 6 px en ajustes; 0 en paneles acoplados.
- **Background:** superficie elevada sobre el lienzo.
- **Border:** hairline de 1 px. Sin sombra.
- **Internal Padding:** 16 px, con separación de 8–12 px entre grupos.

### Inputs / Fields
- **Style:** 28 px de alto, fondo lienzo, hairline de 1 px, radio 4 px, texto de 13 px.
- **Focus:** borde azul de 1 px y contorno de foco; el texto del placeholder es atenuado, nunca un gris ilegible.
- **Error:** borde rojo y mensaje textual; el color no sustituye al mensaje.

### Navigation
- **Style:** navegación de ajustes por pestañas de texto con subrayado azul de 2 px, sin píldoras ni fondos. En la barra lateral, la navegación es la propia lista de sesiones.
- **States:** inactiva en texto secundario; hover en tinta; activa en tinta con subrayado. El estado nunca depende solo del color: el subrayado y el peso también cambian.

### Signature: canal de bloques OSC 133
- **What:** una columna de 2 px en el margen izquierdo del panel marca cada bloque de comando: verde si el código de salida es 0, rojo si no lo es, azul mientras el comando está en curso.
- **Rules:** la marca es información semántica del shell, no decoración; no se dibuja si el shell no emite OSC 133. Una regla tenue separa cada entrada y el prompt lleva una banda apenas perceptible. El canal nunca roba ancho al texto: vive en el margen de 14 px.

## Do's and Don'ts

### Do:
- **Do** derivar todo color de chrome del tema activo con las mezclas 4,5 / 8 / 12 / 22 %.
- **Do** mantener el chrome vertical en 64 px (38 + 26) y el resto para el texto.
- **Do** usar hairlines de 1 px para separar regiones y dejar que el tono haga el resto.
- **Do** reservar el azul de marca para foco, estado vivo y una única acción primaria.
- **Do** acompañar cada estado de color con una forma, un icono o una etiqueta.
- **Do** usar mono para rutas, dimensiones, atajos, porcentajes y versiones.
- **Do** validar el contraste: 4,5:1 para UI y 7:1 para texto de terminal en los temas propios.
- **Do** mantener los objetivos de clic en al menos 24×24 px y el foco de teclado visible.

### Don't:
- **Don't** usar píldoras, degradados, cristal, neón ni sombras en superficies de trabajo.
- **Don't** introducir un color de interfaz que no exista en el tema o en los tokens de estado.
- **Don't** poner emojis en la interfaz; los iconos son Lucide de trazo.
- **Don't** convertir el explorador o los ajustes en tarjetas flotantes: están acoplados al lienzo.
- **Don't** usar mayúsculas fuera de las micro-etiquetas de 9,5 px con tracking.
- **Don't** animar nada que despierte el bucle de eventos en reposo; el cursor es la única animación permitida.
- **Don't** escribir un color hexadecimal de chrome a mano en un archivo de render.
- **Don't** duplicar geometría entre dibujo y clic.
