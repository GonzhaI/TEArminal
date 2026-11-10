#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Color {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

impl Color {
    pub(crate) const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub(crate) const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub(crate) const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub(crate) const fn r(self) -> u8 {
        self.r
    }

    #[inline(always)]
    pub(crate) const fn g(self) -> u8 {
        self.g
    }

    #[inline(always)]
    pub(crate) const fn b(self) -> u8 {
        self.b
    }

    #[inline(always)]
    pub(crate) const fn a(self) -> u8 {
        self.a
    }

    /// Convierte a formato de pixel 32-bit XRGB nativo de Windows (0x00RRGGBB)
    #[inline(always)]
    pub(crate) fn to_u32_xrgb(self) -> u32 {
        (u32::from(self.r) << 16) | (u32::from(self.g) << 8) | u32::from(self.b)
    }

    /// Inversa de `to_u32_xrgb`: color opaco desde un píxel XRGB.
    #[inline(always)]
    pub(crate) const fn from_u32_xrgb(v: u32) -> Self {
        Self::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Mezcla alpha de un color de primer plano sobre este color de fondo
    #[inline(always)]
    pub(crate) fn blend_over(&self, fg: Color, alpha: u8) -> u32 {
        if alpha == 0 {
            return self.to_u32_xrgb();
        }
        if alpha == 255 {
            return fg.to_u32_xrgb();
        }
        let a = u32::from(alpha);
        let inv_a = 255 - a;
        let r = ((u32::from(fg.r) * a + u32::from(self.r) * inv_a) / 255) as u8;
        let g = ((u32::from(fg.g) * a + u32::from(self.g) * inv_a) / 255) as u8;
        let b = ((u32::from(fg.b) * a + u32::from(self.b) * inv_a) / 255) as u8;
        (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    }
}

impl From<(u8, u8, u8)> for Color {
    #[inline]
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::from_rgb(r, g, b)
    }
}

impl From<(u8, u8, u8, u8)> for Color {
    #[inline]
    fn from((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self::from_rgba(r, g, b, a)
    }
}

impl From<[u8; 3]> for Color {
    #[inline]
    fn from([r, g, b]: [u8; 3]) -> Self {
        Self::from_rgb(r, g, b)
    }
}

impl From<[u8; 4]> for Color {
    #[inline]
    fn from([r, g, b, a]: [u8; 4]) -> Self {
        Self::from_rgba(r, g, b, a)
    }
}

/// Temas de iterm2-color-schemes (`.itermcolors` originales de `schemes/`), generados con `tools/fetch-themes.ps1`.
/// Una línea por tema: nombre, fondo, texto, cursor, selección (fondo y texto) y 16 colores ANSI.
static THEMES: &str = include_str!("../assets/themes.txt");

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Theme {
    pub(crate) background: Color,
    pub(crate) foreground: Color,
    pub(crate) cursor: Color,
    pub(crate) selection: Color,
    pub(crate) selection_fg: Color,
    pub(crate) ansi: [Color; 16],
    /// Acento de la interfaz. No es un color ANSI: la UI no debe depender de que el tema tenga
    /// un verde o un azul utilizables como acento.
    pub(crate) ui_accent: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_name(Self::DEFAULT_DARK)
    }
}

impl From<&str> for Theme {
    fn from(name: &str) -> Self {
        Self::from_name(name)
    }
}

impl From<String> for Theme {
    fn from(name: String) -> Self {
        Self::from_name(&name)
    }
}

/// Clave de comparación de nombres: sin mayúsculas, acentos ni separadores
/// (`tokyo night`, `Tokyo-Night` y `TokyoNight` coinciden).
fn name_key(name: &str) -> String {
    name.chars()
        .map(|c| if c == 'é' || c == 'É' { 'e' } else { c })
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn first_field(line: &str) -> &str {
    line.split('\t').next().unwrap_or("")
}

fn hex(s: &str) -> Option<Color> {
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some(Color::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Luminancia relativa WCAG (0..1) de un color opaco.
pub(crate) fn luminance(c: Color) -> f32 {
    let channel = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// Contraste mínimo del acento frente al fondo (componentes de interfaz, WCAG 1.4.11).
const ACCENT_MIN_CONTRAST: f32 = 3.0;

/// Contraste mínimo (WCAG AA) del texto de la interfaz sobre su superficie.
const TEXT_MIN_CONTRAST: f32 = 4.5;

/// Mayor diferencia por canal entre dos colores (0-255).
fn max_channel_diff(a: Color, b: Color) -> u8 {
    a.r.abs_diff(b.r)
        .max(a.g.abs_diff(b.g))
        .max(a.b.abs_diff(b.b))
}

/// Contraste WCAG entre dos colores opacos (1..21).
pub(crate) fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Acento de interfaz: el primer color del tema que se distinga de su fondo (3:1, contraste de componentes
/// WCAG: así el azul de Latte, 4.4:1, sigue siendo el acento). Para texto en acento, `Theme::accent_text`.
/// La UI nunca inventa un color fuera de la paleta, pero tampoco hereda un azul invisible.
fn ui_accent_for(background: Color, foreground: Color, ansi: &[Color; 16]) -> Color {
    [ansi[4], ansi[6], ansi[2], ansi[5], foreground]
        .into_iter()
        .find(|c| contrast_ratio(*c, background) >= ACCENT_MIN_CONTRAST)
        .unwrap_or(foreground)
}

/// Interpreta una línea de `themes.txt`; `None` si está malformada.
fn parse_line(line: &str) -> Option<(&str, Theme)> {
    let mut f = line.split('\t');
    let name = f.next()?;
    let background = hex(f.next()?)?;
    let foreground = hex(f.next()?)?;
    let cursor = f.next().and_then(hex).unwrap_or(foreground);
    let selection = f.next().and_then(hex).unwrap_or(foreground);
    let selection_fg = f.next().and_then(hex).unwrap_or(background);
    let mut ansi = [Color::TRANSPARENT; 16];
    for slot in &mut ansi {
        *slot = hex(f.next()?)?;
    }
    let theme = Theme {
        background,
        foreground,
        cursor,
        selection,
        selection_fg,
        ui_accent: ui_accent_for(background, foreground, &ansi),
        ansi,
    };
    Some((name, theme))
}

impl Theme {
    pub(crate) const DEFAULT_DARK: &'static str = "Catppuccin Frappe";
    pub(crate) const DEFAULT_LIGHT: &'static str = "Catppuccin Latte";

    /// Nombres de todos los temas incorporados (los de iTerm2).
    pub(crate) fn names() -> impl Iterator<Item = &'static str> {
        THEMES.lines().map(first_field)
    }

    /// Temas cuyo nombre contiene `filter` (sin distinguir mayúsculas ni separadores).
    pub(crate) fn search(filter: &str) -> Vec<&'static str> {
        let key = name_key(filter);
        if key.is_empty() {
            return Self::names().collect();
        }
        Self::names()
            .filter(|n| name_key(n).contains(&key))
            .collect()
    }

    fn find(name: &str) -> Option<(&'static str, Theme)> {
        // Camino rápido: el nombre exacto de la lista (la interfaz lo usa en cada fila dibujada)
        if let Some(line) = THEMES.lines().find(|l| first_field(l) == name) {
            return parse_line(line);
        }
        let key = name_key(name);
        let key = match key.as_str() {
            // Valores de versiones anteriores de `tearminal.toml`
            "frappe" => name_key("Catppuccin Frappe"),
            "latte" => name_key("Catppuccin Latte"),
            "dark" | "default" | "tearminalnight" | "tearminal" | "night" => {
                name_key(Self::DEFAULT_DARK)
            }
            "tearminalday" | "day" => name_key(Self::DEFAULT_LIGHT),
            "onedark" => name_key("Atom One Dark"),
            "solarizeddark" => name_key("iTerm2 Solarized Dark"),
            _ => key,
        };
        THEMES
            .lines()
            .find(|l| name_key(first_field(l)) == key)
            .and_then(parse_line)
    }

    /// Nombre canónico del tema configurado (el de por defecto si no existe).
    pub(crate) fn canonical_name(name: &str) -> &'static str {
        Self::find(name).map_or(Self::DEFAULT_DARK, |(n, _)| n)
    }

    /// Resuelve el tema configurado; si no existe, usa el de por defecto.
    pub(crate) fn from_name(name: &str) -> Self {
        Self::find(name)
            .or_else(|| Self::find(Self::DEFAULT_DARK))
            .or_else(|| THEMES.lines().find_map(parse_line))
            .map(|(_, t)| t)
            .unwrap_or(Self {
                background: Color::from_rgb(30, 30, 46),
                foreground: Color::from_rgb(205, 214, 244),
                cursor: Color::from_rgb(245, 224, 220),
                selection: Color::from_rgb(88, 91, 112),
                selection_fg: Color::from_rgb(205, 214, 244),
                ansi: [
                    Color::from_rgb(69, 71, 90),
                    Color::from_rgb(243, 139, 168),
                    Color::from_rgb(166, 227, 161),
                    Color::from_rgb(249, 226, 175),
                    Color::from_rgb(137, 180, 250),
                    Color::from_rgb(245, 194, 231),
                    Color::from_rgb(148, 226, 213),
                    Color::from_rgb(186, 194, 222),
                    Color::from_rgb(88, 91, 112),
                    Color::from_rgb(243, 139, 168),
                    Color::from_rgb(166, 227, 161),
                    Color::from_rgb(249, 226, 175),
                    Color::from_rgb(137, 180, 250),
                    Color::from_rgb(245, 194, 231),
                    Color::from_rgb(148, 226, 213),
                    Color::from_rgb(166, 173, 200),
                ],
                ui_accent: Color::from_rgb(137, 180, 250),
            })
    }

    /// Determina si el tema activo es de tonalidad clara
    #[inline]
    pub(crate) fn is_light(&self) -> bool {
        let lum = 0.2126 * f32::from(self.background.r)
            + 0.7152 * f32::from(self.background.g)
            + 0.0722 * f32::from(self.background.b);
        lum > 140.0
    }

    /// Determina si el tema activo es de tonalidad oscura
    #[inline]
    pub(crate) fn is_dark(&self) -> bool {
        !self.is_light()
    }

    /// Mezcla lineal en sRGB entre dos colores opacos (`t` = 0 devuelve `a`, 1 devuelve `b`).
    pub(crate) fn mix(&self, a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
        Color::from_rgb(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b))
    }

    /// Fondo del chrome (barras, pestañas inactivas). Un paso hacia el texto, nunca un gris ajeno.
    pub(crate) fn chrome_bg(&self) -> Color {
        self.mantle()
    }

    /// `crust` del diseño: marco de la ventana (barras, riel, estado y margen de las tarjetas), un paso hacia
    /// la sombra para que el chrome se separe de la tarjeta de contenido. En oscuro mezcla 26 % hacia negro y en
    /// claro 11 % hacia el texto (calibrado con Frappé y Latte). Un fondo casi negro no puede oscurecerse más:
    /// ahí el marco se aclara un poco hacia el texto.
    pub(crate) fn bar_bg(&self) -> Color {
        let bg = self.background;
        if self.is_light() {
            return self.mix(bg, self.foreground, 0.11);
        }
        if bg.r.max(bg.g).max(bg.b) >= 24 {
            return self.mix(bg, Color::from_rgb(0, 0, 0), 0.26);
        }
        // Fondo casi negro: se aclara hacia el texto lo justo para que el marco se note.
        let mut frame = bg;
        for t in [0.05, 0.1, 0.18, 0.3] {
            frame = self.mix(bg, self.foreground, t);
            if max_channel_diff(frame, bg) >= 4 {
                break;
            }
        }
        frame
    }

    /// `crust` del diseño (alias de `bar_bg`).
    pub(crate) fn crust(&self) -> Color {
        self.bar_bg()
    }

    /// `mantle` del diseño: barras, paneles y cabeceras, entre la tarjeta y `crust`.
    pub(crate) fn mantle(&self) -> Color {
        self.mix(self.background, self.crust(), 0.55)
    }

    /// Superficies `s0`, `s1` y `s2` del diseño (`level` 0..=2): superficie, línea y línea fuerte. Pasos
    /// hacia el texto calibrados con Frappé (11/22/33 %) y Latte (20/30/40 %).
    pub(crate) fn surface_level(&self, level: usize) -> Color {
        const DARK: [f32; 3] = [0.11, 0.22, 0.33];
        const LIGHT: [f32; 3] = [0.20, 0.30, 0.40];
        let steps = if self.is_light() { LIGHT } else { DARK };
        self.mix(
            self.background,
            self.foreground,
            steps[level.min(steps.len() - 1)],
        )
    }

    /// Acerca `color` al texto hasta alcanzar `min_ratio`:1 sobre `on` (o hasta el propio texto si ni así
    /// llega). Solo para texto: los rellenos conservan el color crudo del tema.
    fn with_min_contrast(&self, color: Color, on: Color, min_ratio: f32) -> Color {
        for step in 0..=20 {
            let candidate = self.mix(color, self.foreground, step as f32 / 20.0);
            if contrast_ratio(candidate, on) >= min_ratio {
                return candidate;
            }
        }
        self.foreground
    }

    /// Color de texto legible (`TEXT_MIN_CONTRAST`:1) para `color` sobre `on`.
    pub(crate) fn readable(&self, color: Color, on: Color) -> Color {
        self.with_min_contrast(color, on, TEXT_MIN_CONTRAST)
    }

    /// Texto sobre el acento: `crust`, el fondo o el texto, el primero que alcance `TEXT_MIN_CONTRAST`:1.
    #[allow(dead_code)] // lo consumen botones primarios y chips activos (F1–F6)
    pub(crate) fn on_accent(&self) -> Color {
        let accent = self.ui_accent;
        let candidates = [self.crust(), self.background, self.foreground];
        candidates
            .into_iter()
            .find(|c| contrast_ratio(*c, accent) >= TEXT_MIN_CONTRAST)
            .unwrap_or_else(|| {
                candidates
                    .into_iter()
                    .max_by(|a, b| {
                        contrast_ratio(*a, accent).total_cmp(&contrast_ratio(*b, accent))
                    })
                    .unwrap_or(self.foreground)
            })
    }

    /// Verde del tema (conectado, añadido, correcto).
    #[allow(dead_code)] // semánticos del diseño, consumidos por F1–F6
    pub(crate) fn green(&self) -> Color {
        self.ansi[2]
    }

    /// Amarillo del tema (aviso, staging, modificado).
    #[allow(dead_code)]
    pub(crate) fn yellow(&self) -> Color {
        self.ansi[3]
    }

    /// Rojo del tema (producción, peligro, borrado).
    #[allow(dead_code)]
    pub(crate) fn red(&self) -> Color {
        self.ansi[1]
    }

    /// Malva (agente de IA, palabras clave SQL): los temas traen el magenta como rosa (Catppuccin: `f4b8e4`),
    /// así que se acerca al azul para que sea violeta, como en el diseño.
    #[allow(dead_code)]
    pub(crate) fn mauve(&self) -> Color {
        self.mix(self.ansi[5], self.ansi[4], 0.5)
    }

    /// Cian del tema (visor).
    #[allow(dead_code)]
    pub(crate) fn teal(&self) -> Color {
        self.ansi[6]
    }

    /// Durazno (números, MySQL): los temas no lo traen, así que es la mitad entre rojo y amarillo.
    #[allow(dead_code)]
    pub(crate) fn peach(&self) -> Color {
        self.mix(self.ansi[1], self.ansi[3], 0.5)
    }

    /// Contorno de 1 px de las tarjetas, los chips y las cápsulas: el hairline que separa regiones
    /// (`chrome_line`), reforzado solo cuando no se vería contra la tarjeta (`background`) o contra el
    /// marco (`bar_bg`): en temas claros el hairline casi coincide con el marco, y en los de poco
    /// contraste (texto casi igual al fondo) con ambos.
    pub(crate) fn card_outline(&self) -> Color {
        let (bg, frame) = (self.background, self.bar_bg());
        let visible = |c: Color| max_channel_diff(c, bg) >= 8 && max_channel_diff(c, frame) >= 8;
        let mut line = self.chrome_line();
        for t in [0.12, 0.18, 0.25, 0.35, 0.5] {
            if visible(line) {
                break;
            }
            line = self.mix(frame, self.foreground, t);
        }
        line
    }

    /// Superficie elevada (`s0`): filas activas, tarjetas, menús.
    pub(crate) fn chrome_bg_elevated(&self) -> Color {
        self.surface_level(0)
    }

    /// Hairline de 1 px para separar regiones.
    pub(crate) fn chrome_line(&self) -> Color {
        self.surface_level(1)
    }

    /// Hairline con más presencia (foco, bordes de controles).
    pub(crate) fn chrome_line_strong(&self) -> Color {
        self.surface_level(2)
    }

    /// Texto secundario `sub1` (11 % hacia el fondo), con el suelo de contraste sobre la tarjeta.
    pub(crate) fn text_secondary(&self) -> Color {
        let color = self.mix(self.foreground, self.background, 0.11);
        self.with_min_contrast(color, self.background, TEXT_MIN_CONTRAST)
    }

    /// Texto terciario `sub0` (22 % hacia el fondo): metadatos que se leen, no decoración. Mismo suelo de
    /// contraste que `sub1`.
    pub(crate) fn text_muted(&self) -> Color {
        let color = self.mix(self.foreground, self.background, 0.22);
        self.with_min_contrast(color, self.background, TEXT_MIN_CONTRAST)
    }

    /// Relleno translúcido del acento para estados seleccionados.
    pub(crate) fn accent_soft(&self, alpha: u8) -> Color {
        let a = self.ui_accent;
        Color::from_rgba(a.r, a.g, a.b, alpha)
    }

    /// Fondo de la barra lateral y de las barras de chrome.
    pub(crate) fn sidebar_bg(&self) -> Color {
        self.chrome_bg()
    }

    /// Color divisorio sutil y fino acorde al tema.
    pub(crate) fn border_color(&self) -> Color {
        self.chrome_line()
    }

    /// Color de superficie para tarjetas, ítems activos y botones.
    pub(crate) fn surface_bg(&self) -> Color {
        self.chrome_bg_elevated()
    }

    /// Color de superficie para estados hover.
    pub(crate) fn surface_hover(&self) -> Color {
        self.surface_level(1)
    }

    /// Color de acento de interfaz primario.
    pub(crate) fn accent_color(&self) -> Color {
        self.ui_accent
    }

    /// El acento como color de texto sobre la tarjeta: con el suelo de contraste de `readable`.
    pub(crate) fn accent_text(&self) -> Color {
        self.readable(self.ui_accent, self.background)
    }

    /// Resuelve índices de 256 colores estándar ANSI
    pub(crate) fn resolve_256_color(&self, index: u8) -> Color {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let i = index - 16;
                let r = (i / 36) % 6;
                let g = (i / 6) % 6;
                let b = i % 6;
                let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Color::from_rgb(step(r), step(g), step(b))
            }
            232..=255 => {
                let gray = 8 + (index - 232) * 10;
                Color::from_rgb(gray, gray, gray)
            }
        }
    }
}

/// Consulta el registro de Windows para detectar si el sistema tiene activo el Modo Claro
pub(crate) fn is_windows_system_light_mode() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        };

        let subkey: Vec<u16> =
            OsStr::new("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0")
                .encode_wide()
                .collect();
        let value_name: Vec<u16> = OsStr::new("AppsUseLightTheme\0").encode_wide().collect();

        unsafe {
            let mut hkey: HKEY = 0;
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut data: u32 = 0;
                let mut data_len = std::mem::size_of::<u32>() as u32;
                let res = RegQueryValueExW(
                    hkey,
                    value_name.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut data as *mut u32 as *mut u8,
                    &mut data_len,
                );
                RegCloseKey(hkey);
                if res == 0 {
                    return data == 1;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_builtin_themes_parse() {
        let lines = THEMES.lines().count();
        assert!(lines >= 800, "themes.txt incompleto: {lines} temas");
        assert_eq!(Theme::names().count(), lines);
        assert!(THEMES.lines().all(|l| parse_line(l).is_some()));
    }

    #[test]
    fn test_frame_and_outline_stay_visible_in_every_theme() {
        for name in Theme::names() {
            let t = Theme::from_name(name);
            assert!(
                max_channel_diff(t.bar_bg(), t.background) >= 4,
                "{name}: el marco no se distingue de la tarjeta"
            );
            assert!(
                max_channel_diff(t.card_outline(), t.background) >= 8,
                "{name}: el contorno no se distingue de la tarjeta"
            );
            // Chips y cápsulas viven sobre el marco, no sobre la tarjeta.
            assert!(
                max_channel_diff(t.card_outline(), t.bar_bg()) >= 8,
                "{name}: el contorno no se distingue del marco"
            );
        }
    }

    #[test]
    fn test_card_outline_is_chrome_line() {
        let t = Theme::default();
        assert_eq!(t.card_outline(), t.chrome_line());
    }

    #[test]
    fn test_color_and_theme_from_conversions() {
        let c1: Color = (10u8, 20u8, 30u8).into();
        assert_eq!(c1, Color::from_rgb(10, 20, 30));
        let c2: Color = (10u8, 20u8, 30u8, 200u8).into();
        assert_eq!(c2, Color::from_rgba(10, 20, 30, 200));
        let c3: Color = [1u8, 2u8, 3u8].into();
        assert_eq!(c3, Color::from_rgb(1, 2, 3));
        let c4: Color = [1u8, 2u8, 3u8, 4u8].into();
        assert_eq!(c4, Color::from_rgba(1, 2, 3, 4));

        let t1: Theme = "Catppuccin Frappe".into();
        assert_eq!(t1, Theme::from_name("Catppuccin Frappe"));
        let t2: Theme = String::from("Catppuccin Latte").into();
        assert_eq!(t2, Theme::from_name("Catppuccin Latte"));
    }

    #[test]
    fn test_color_xrgb_roundtrip() {
        let c = Color::from_rgb(0x12, 0x34, 0x56);
        assert_eq!(Color::from_u32_xrgb(c.to_u32_xrgb()), c);
        assert_eq!(Color::from_u32_xrgb(0xFF12_3456).to_u32_xrgb(), 0x12_3456);
    }

    #[test]
    fn test_default_themes_are_catppuccin() {
        assert_eq!(Theme::DEFAULT_DARK, "Catppuccin Frappe");
        assert_eq!(Theme::DEFAULT_LIGHT, "Catppuccin Latte");
        assert!(Theme::from_name(Theme::DEFAULT_DARK).is_dark());
        assert!(Theme::from_name(Theme::DEFAULT_LIGHT).is_light());
        // Los temas propios retirados se resuelven a los de por defecto
        assert_eq!(
            Theme::canonical_name("TEArminal Night"),
            Theme::DEFAULT_DARK
        );
        assert_eq!(Theme::canonical_name("TEArminal Day"), Theme::DEFAULT_LIGHT);
    }

    #[test]
    fn test_theme_matches_upstream_values() {
        let t = Theme::from_name("Catppuccin Frappe");
        assert_eq!(t.background, Color::from_rgb(0x30, 0x34, 0x46));
        assert_eq!(t.foreground, Color::from_rgb(0xc6, 0xd0, 0xf5));
        assert_eq!(t.selection, Color::from_rgb(0xf2, 0xd5, 0xcf));
        assert_eq!(t.ansi[1], Color::from_rgb(0xe7, 0x82, 0x84));
        assert!(!t.is_light());
        assert!(Theme::from_name("Catppuccin Latte").is_light());
    }

    #[test]
    fn test_theme_name_lookup() {
        assert_eq!(Theme::canonical_name("tokyo night"), "TokyoNight");
        assert_eq!(Theme::canonical_name("DRACULA"), "Dracula");
        // Valores de configuraciones antiguas
        assert_eq!(Theme::canonical_name("frappe"), "Catppuccin Frappe");
        assert_eq!(
            Theme::canonical_name("Catppuccin Frappé"),
            "Catppuccin Frappe"
        );
        assert_eq!(Theme::canonical_name("latte"), "Catppuccin Latte");
        assert_eq!(Theme::canonical_name("one dark"), "Atom One Dark");
        assert_eq!(
            Theme::canonical_name("solarized dark"),
            "iTerm2 Solarized Dark"
        );
        // Desconocido: tema por defecto
        assert_eq!(Theme::canonical_name("no existe"), Theme::DEFAULT_DARK);
        assert_eq!(Theme::canonical_name("dark"), Theme::DEFAULT_DARK);
        assert_eq!(Theme::canonical_name("default"), Theme::DEFAULT_DARK);
        assert!(!Theme::search("solarized dark").is_empty());
        assert_eq!(Theme::search("frappe"), vec!["Catppuccin Frappe"]);
        assert!(Theme::search("zzzz").is_empty());
    }

    fn hex_color(hex: &str) -> Color {
        let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
        Color::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Los tokens derivados reproducen los de Frappé y Latte del diseño (DISENO §3.1) con ±3 por canal.
    /// Los hexadecimales del diseño solo viven aquí.
    #[test]
    fn test_ui_tokens_match_the_design_on_catppuccin() {
        let cases = [
            (
                Theme::DEFAULT_DARK,
                [
                    ("mantle", "#292c3c"),
                    ("crust", "#232634"),
                    ("s0", "#414559"),
                    ("s1", "#51576d"),
                    ("s2", "#626880"),
                    ("sub1", "#b5bfe2"),
                    ("sub0", "#a5adce"),
                    ("blue", "#8caaee"),
                    ("green", "#a6d189"),
                    ("yellow", "#e5c890"),
                    ("red", "#e78284"),
                    ("teal", "#81c8be"),
                ],
            ),
            (
                Theme::DEFAULT_LIGHT,
                [
                    ("mantle", "#e6e9ef"),
                    ("crust", "#dce0e8"),
                    ("s0", "#ccd0da"),
                    ("s1", "#bcc0cc"),
                    ("s2", "#acb0be"),
                    ("sub1", "#5c5f77"),
                    // El diseño da 4.37:1 para `sub0` en Latte: el suelo de 4.5:1 lo oscurece un poco.
                    ("sub0", "#6c6f85"),
                    ("blue", "#1e66f5"),
                    ("green", "#40a02b"),
                    ("yellow", "#df8e1d"),
                    ("red", "#d20f39"),
                    ("teal", "#179299"),
                ],
            ),
        ];
        for (name, expected) in cases {
            let t = Theme::from_name(name);
            for (token, hex) in expected {
                let actual = match token {
                    "mantle" => t.mantle(),
                    "crust" => t.crust(),
                    "s0" => t.surface_level(0),
                    "s1" => t.surface_level(1),
                    "s2" => t.surface_level(2),
                    "sub1" => t.text_secondary(),
                    "sub0" => t.text_muted(),
                    "blue" => t.accent_color(),
                    "green" => t.green(),
                    "yellow" => t.yellow(),
                    "red" => t.red(),
                    _ => t.teal(),
                };
                let want = hex_color(hex);
                assert!(
                    max_channel_diff(actual, want) <= 3,
                    "{name} {token}: {actual:?} se aleja de {hex}"
                );
            }
            // Los métodos de siempre apuntan a los tokens del diseño.
            assert_eq!(t.chrome_bg(), t.mantle());
            assert_eq!(t.bar_bg(), t.crust());
            assert_eq!(t.chrome_bg_elevated(), t.surface_level(0));
            assert_eq!(t.chrome_line(), t.surface_level(1));
            assert_eq!(t.chrome_line_strong(), t.surface_level(2));
        }
    }

    /// Mauve y durazno no existen en los temas: se derivan de su magenta, azul, rojo y amarillo.
    #[test]
    fn test_peach_and_mauve_are_derived_from_the_palette() {
        for name in [Theme::DEFAULT_DARK, Theme::DEFAULT_LIGHT] {
            let t = Theme::from_name(name);
            assert_eq!(t.peach(), t.mix(t.ansi[1], t.ansi[3], 0.5));
            assert_eq!(t.mauve(), t.mix(t.ansi[5], t.ansi[4], 0.5));
            // El malva se separa del rosa de la paleta y el durazno del rojo.
            assert_ne!(t.mauve(), t.ansi[5]);
            assert_ne!(t.peach(), t.red());
        }
    }

    /// En los 817 temas: las superficies se alejan del fondo de forma estrictamente creciente y `mantle`
    /// queda entre la tarjeta y `crust`.
    #[test]
    fn test_surface_hierarchy_in_every_theme() {
        let dist = |a: Color, b: Color| max_channel_diff(a, b);
        for name in Theme::names() {
            let t = Theme::from_name(name);
            let bg = t.background;
            assert!(
                dist(t.surface_level(0), bg) <= dist(t.surface_level(1), bg)
                    && dist(t.surface_level(1), bg) <= dist(t.surface_level(2), bg),
                "{name}: s0..s2 no crecen"
            );
            assert!(
                dist(t.mantle(), bg) <= dist(t.crust(), bg),
                "{name}: mantle no queda entre la tarjeta y crust"
            );
        }
    }

    /// En los 817 temas el texto secundario y los colores `readable` alcanzan 4.5:1 sobre la tarjeta, salvo
    /// que el propio texto del tema no llegue (entonces se quedan en el texto, lo más contrastado posible).
    #[test]
    fn test_text_tokens_meet_the_contrast_floor_in_every_theme() {
        for name in Theme::names() {
            let t = Theme::from_name(name);
            let bg = t.background;
            let reachable = contrast_ratio(t.foreground, bg) >= TEXT_MIN_CONTRAST;
            let mut texts = vec![t.text_secondary(), t.text_muted()];
            texts.extend(
                [
                    t.red(),
                    t.green(),
                    t.yellow(),
                    t.mauve(),
                    t.teal(),
                    t.peach(),
                ]
                .map(|c| t.readable(c, bg)),
            );
            for text in texts {
                if reachable {
                    assert!(
                        contrast_ratio(text, bg) >= TEXT_MIN_CONTRAST,
                        "{name}: texto {text:?} sin contraste suficiente"
                    );
                } else {
                    assert!(contrast_ratio(text, bg) >= contrast_ratio(t.background, bg));
                }
            }
            // El texto sobre el acento es el candidato de mayor contraste si ninguno llega a 4.5:1.
            let on = contrast_ratio(t.on_accent(), t.ui_accent);
            let best = [t.crust(), t.background, t.foreground]
                .into_iter()
                .map(|c| contrast_ratio(c, t.ui_accent))
                .fold(0.0_f32, f32::max);
            assert!(
                on >= TEXT_MIN_CONTRAST.min(best),
                "{name}: on_accent ilegible"
            );
        }
    }

    /// `readable` no toca un color que ya contrasta y acerca al texto el que no.
    #[test]
    fn test_readable_only_moves_colors_that_fail() {
        let t = Theme::from_name(Theme::DEFAULT_LIGHT);
        let on = t.background;
        // En Latte el rojo ya pasa; el amarillo y el verde no.
        assert_eq!(t.readable(t.red(), on), t.red());
        for c in [t.yellow(), t.green()] {
            assert!(contrast_ratio(c, on) < TEXT_MIN_CONTRAST);
            assert!(contrast_ratio(t.readable(c, on), on) >= TEXT_MIN_CONTRAST);
        }
    }
}
