# Product

<!-- impeccable:product-schema 1 -->

## Platform

windows

Aplicación nativa de escritorio para Windows 10/11 x64 (Win32 + ConPTY). No hay versión web, iOS ni Android.

## Users

Desarrolladores y operadores que pasan la jornada dentro de una terminal en Windows: trabajan con PowerShell, ejecutan agentes de IA en CLI (Claude Code, Gemini CLI, Aider, OpenCode, AGY), mantienen sesiones SSH y alternan entre varios proyectos y paneles al mismo tiempo. Su atención está en el texto, no en la interfaz; usan teclado de forma intensiva y leen salidas largas y densas.

## Product Purpose

Axon es un emulador de terminal nativo, pequeño y rápido, pensado para PowerShell y para trabajo agéntico. Debe desaparecer bajo el contenido: abrir rápido, mantenerse en <1 % de CPU en reposo, presentar texto perfecto y hacer visibles el contexto (carpeta, rama, host, agente) y la estructura de la salida (bloques de comando, pestañas, paneles) sin competir con el texto.

## Positioning

Un terminal de Windows sin Electron ni WebView: emulación VT propia, render por software (~1,3 ms por fotograma a 1080p) y presentación nativa por GPU, con integración de shell y SSH de primera clase. La interfaz no es un envoltorio web: es parte del mismo motor nativo, por eso puede ser precisa, instantánea y consumir una fracción de los recursos de un terminal basado en web.

## Operating Context

Sesiones largas de varias horas, a menudo con dos o tres paneles y varias pestañas; alternancia entre editor/visor, explorador de archivos y terminal; uso intensivo de teclado; terminales de 80 a 220 columnas; pantallas de 1080p a 4K con escalado de Windows; entornos con poca luz y también oficinas luminosas (de ahí el modo oscuro y claro). SSH a servidores remotos y agentes que reescriben la pantalla. El usuario espera que la interfaz sea discreta, estable y legible durante horas.

## Capabilities and Constraints

- Emulación VT/ECMA-48 xterm estricta, TrueColor, pantalla alterna, ratón SGR, bracketed paste, Kitty Graphics Protocol, iTerm2 inline images, CJK y grapheme clustering.
- OSC 133: bloques semánticos con línea de prompt, comando, salida y código de salida.
- Pestañas horizontales o verticales, paneles divididos, explorador de archivos, visor/editor con resaltado, Markdown, diff git, SSH con integración remota y OSC 52.
- 817 temas de iTerm2 embebidos; el color ANSI del terminal es del tema y no debe falsearse.
- Render por software sobre un framebuffer XRGB; no hay motor web ni CSS. La geometría de dibujo y de clic es la misma fuente de verdad.
- Restricciones de rendimiento: un fotograma completo ~1,3 ms; nada de work por fotograma que crezca con el scrollback; sin animaciones que despierten el bucle de eventos.
- La interfaz se deriva del tema activo; cualquier tema de iTerm2 debe seguir siendo usable.
- Fuentes embebidas: JetBrains Mono (terminal) e Inter (interfaz); iconos Lucide. Sin emojis en la UI.
- Accesibilidad: contraste alto en texto y foco visible; objetivos de clic de al menos 24×24 px; la información nunca depende solo del color.

## Brand Commitments

Nombre Axon, marca de terminal nativa y minimalista. La interfaz habla español. No hay logotipo vectorial oficial más allá de `assets/logo.png`; el icono Lucide de terminal se usa como marca provisional. Tono: preciso, técnico, sin adornos ni jerga de marketing.

## Evidence on Hand

Código y capturas del producto en `src/`, `assets/logo.png`, `README.md`. Temas reales en `assets/themes.txt`. No hay testimonios, clientes, métricas de marketing ni material comercial; nada de eso debe inventarse.

## Product Principles

1. El texto manda: cualquier píxel de interfaz debe justificar el espacio que quita al contenido.
2. Densidad con aire: mucha información por pantalla, pero con jerarquía y ritmo que permitan escanear.
3. Precisión nativa: geometría exacta, estados claros, cero ornamentos que no signifiquen algo.
4. El contexto siempre visible, nunca ruidoso: carpeta, rama, host, agente y estado en una sola línea legible.
5. Coherencia con el tema: la interfaz se adapta al color del terminal, no al revés.

## Accessibility & Inclusion

Contraste mínimo 4,5:1 para texto de interfaz y 7:1 para texto de terminal sobre su fondo en los temas por defecto. Foco de teclado visible. Ningún estado comunicado únicamente por color (los indicadores llevan forma o etiqueta). Respeto al escalado de Windows y a tamaños de fuente configurables.
