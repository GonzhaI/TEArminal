//! Escala tipográfica del chrome. Los tamaños son px lógicos y escalan con `ui_font_size`.

/// Micro-etiquetas en mayúsculas con tracking (cabeceras de tarjeta, grupos).
pub const MICRO: f32 = 10.0;
/// Metadatos que se leen: segunda línea de una sesión, descripción de una opción.
pub const DETAIL: f32 = 11.0;
/// Cuerpo de la interfaz: etiquetas, textos de tarjeta, navegación.
pub const BODY: f32 = 12.0;
/// Títulos de fila y de pestaña.
pub const TITLE: f32 = 12.5;
/// Título de sección dentro de una tarjeta.
pub const SECTION: f32 = 13.0;
/// Título editorial de la página de Ajustes.
pub const DISPLAY: f32 = 20.0;
/// Tracking de micro-etiquetas (px por hueco).
pub const TRACKING: f32 = 1.2;
/// Checkbox de los toggles de Ajustes.
pub const CHECKBOX: usize = 18;
/// Keycap de atajo dentro del chrome.
pub const KEYCAP_H: usize = 18;
