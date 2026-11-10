//! Escala tipográfica y forma del chrome (DISENO §3.2 y §3.3). Los tamaños son px lógicos: `BODY` es la
//! referencia y coincide con `font_size` de la interfaz; el resto escala con él.

/// Rótulos en mayúsculas con tracking (cabeceras de tarjeta, grupos). Ningún texto baja de este tamaño.
pub(crate) const MICRO: f32 = 11.0;
/// Metadatos que se leen: segunda línea de una sesión, descripción de una opción.
pub(crate) const DETAIL: f32 = 12.0;
/// Cuerpo de la interfaz: etiquetas, textos de tarjeta, navegación.
pub(crate) const BODY: f32 = 14.0;
/// Títulos de fila y de pestaña.
pub(crate) const TITLE: f32 = 13.5;
/// Título de sección dentro de una tarjeta.
pub(crate) const SECTION: f32 = 14.0;
/// Título editorial de la página de Ajustes.
pub(crate) const DISPLAY: f32 = 16.0;
/// Tracking de micro-etiquetas (px por hueco): 0,14 em sobre `MICRO`/`DETAIL`.
pub(crate) const TRACKING: f32 = 1.5;
/// Radio de las tarjetas que enmarcan el contenido y el panel: esquinas rectas.
pub(crate) const RADIUS_CARD: usize = 0;
/// Radio de chips, botones de acción, cápsulas y superficies flotantes (menús, paleta, popups).
pub(crate) const RADIUS_CONTROL: usize = 0;
/// Radio de filas de lista, selección y botones pequeños.
pub(crate) const RADIUS_ROW: usize = 0;
/// Checkbox de los toggles de Ajustes.
pub(crate) const CHECKBOX: usize = 18;
/// Keycap de atajo dentro del chrome.
pub(crate) const KEYCAP_H: usize = 18;
