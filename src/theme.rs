#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub const fn r(self) -> u8 {
        self.r
    }

    #[inline(always)]
    pub const fn g(self) -> u8 {
        self.g
    }

    #[inline(always)]
    pub const fn b(self) -> u8 {
        self.b
    }

    #[inline(always)]
    pub const fn a(self) -> u8 {
        self.a
    }

    /// Convierte a formato de pixel 32-bit XRGB nativo de Windows (0x00RRGGBB)
    #[inline(always)]
    pub fn to_u32_xrgb(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Mezcla alpha de un color de primer plano sobre este color de fondo
    #[inline(always)]
    pub fn blend_over(&self, fg: Color, alpha: u8) -> u32 {
        if alpha == 0 {
            return self.to_u32_xrgb();
        }
        if alpha == 255 {
            return fg.to_u32_xrgb();
        }
        let a = alpha as u32;
        let inv_a = 255 - a;
        let r = ((fg.r as u32 * a + self.r as u32 * inv_a) / 255) as u8;
        let g = ((fg.g as u32 * a + self.g as u32 * inv_a) / 255) as u8;
        let b = ((fg.b as u32 * a + self.b as u32 * inv_a) / 255) as u8;
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}

/// Temas de iterm2-color-schemes (`.itermcolors` originales de `schemes/`), generados con `tools/fetch-themes.ps1`.
/// Una línea por tema: nombre, fondo, texto, cursor, selección (fondo y texto) y 16 colores ANSI.
static THEMES: &str = include_str!("../assets/themes.txt");

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub selection_fg: Color,
    pub ansi: [Color; 16],
    /// Acento de la interfaz. No es un color ANSI: la UI no debe depender de que el tema tenga
    /// un verde o un azul utilizables como acento.
    pub ui_accent: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_name(Self::DEFAULT_DARK)
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
fn luminance(c: Color) -> f32 {
    let channel = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// Contraste WCAG entre dos colores opacos (1..21).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Acento de interfaz: el primer color del tema que sea legible sobre su fondo.
/// La UI nunca inventa un color fuera de la paleta, pero tampoco hereda un azul ilegible.
fn ui_accent_for(background: Color, foreground: Color, ansi: &[Color; 16]) -> Color {
    [ansi[4], ansi[6], ansi[2], ansi[5], foreground]
        .into_iter()
        .find(|c| contrast_ratio(*c, background) >= 4.5)
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
    pub const DEFAULT_DARK: &'static str = "Catppuccin Frappe";
    pub const DEFAULT_LIGHT: &'static str = "Catppuccin Latte";

    /// Nombres de todos los temas incorporados (los de iTerm2).
    pub fn names() -> impl Iterator<Item = &'static str> {
        THEMES.lines().map(first_field)
    }

    /// Temas cuyo nombre contiene `filter` (sin distinguir mayúsculas ni separadores).
    pub fn search(filter: &str) -> Vec<&'static str> {
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
            // Valores de versiones anteriores de `axon.toml`
            "frappe" => name_key("Catppuccin Frappe"),
            "latte" => name_key("Catppuccin Latte"),
            "dark" | "default" | "axonnight" | "axon" | "night" => name_key(Self::DEFAULT_DARK),
            "axonday" | "day" => name_key(Self::DEFAULT_LIGHT),
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
    pub fn canonical_name(name: &str) -> &'static str {
        Self::find(name).map_or(Self::DEFAULT_DARK, |(n, _)| n)
    }

    /// Resuelve el tema configurado; si no existe, usa el de por defecto.
    pub fn from_name(name: &str) -> Self {
        Self::find(name)
            .or_else(|| Self::find(Self::DEFAULT_DARK))
            .map(|(_, t)| t)
            .expect("el tema por defecto está en themes.txt")
    }

    /// Determina si el tema activo es de tonalidad clara
    #[inline]
    pub fn is_light(&self) -> bool {
        let lum = 0.2126 * (self.background.r as f32)
            + 0.7152 * (self.background.g as f32)
            + 0.0722 * (self.background.b as f32);
        lum > 140.0
    }

    /// Determina si el tema activo es de tonalidad oscura
    #[inline]
    pub fn is_dark(&self) -> bool {
        !self.is_light()
    }

    /// Mezcla lineal en sRGB entre dos colores opacos (`t` = 0 devuelve `a`, 1 devuelve `b`).
    pub fn mix(&self, a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color::from_rgb(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b))
    }

    /// Fondo del chrome (barras, pestañas inactivas). Un paso hacia el texto, nunca un gris ajeno.
    pub fn chrome_bg(&self) -> Color {
        self.mix(self.background, self.foreground, 0.045)
    }

    /// Superficie elevada: filas activas, tarjetas, menús.
    pub fn chrome_bg_elevated(&self) -> Color {
        self.mix(self.background, self.foreground, 0.08)
    }

    /// Hairline de 1 px para separar regiones.
    pub fn chrome_line(&self) -> Color {
        self.mix(self.background, self.foreground, 0.12)
    }

    /// Hairline con más presencia (foco, bordes de controles).
    pub fn chrome_line_strong(&self) -> Color {
        self.mix(self.background, self.foreground, 0.22)
    }

    /// Texto secundario legible (mezcla de texto y fondo).
    pub fn text_secondary(&self) -> Color {
        self.mix(self.foreground, self.background, 0.28)
    }

    /// Texto terciario: metadatos que se leen, no decoración.
    pub fn text_muted(&self) -> Color {
        self.mix(self.foreground, self.background, 0.48)
    }

    /// Relleno translúcido del acento para estados seleccionados.
    pub fn accent_soft(&self, alpha: u8) -> Color {
        let a = self.ui_accent;
        Color::from_rgba(a.r, a.g, a.b, alpha)
    }

    /// Fondo de la barra lateral y de las barras de chrome.
    pub fn sidebar_bg(&self) -> Color {
        self.chrome_bg()
    }

    /// Color divisorio sutil y fino acorde al tema.
    pub fn border_color(&self) -> Color {
        self.chrome_line()
    }

    /// Color de superficie para tarjetas, ítems activos y botones.
    pub fn surface_bg(&self) -> Color {
        self.chrome_bg_elevated()
    }

    /// Color de superficie para estados hover.
    pub fn surface_hover(&self) -> Color {
        self.mix(self.background, self.foreground, 0.12)
    }

    /// Color de acento de interfaz primario.
    pub fn accent_color(&self) -> Color {
        self.ui_accent
    }

    /// Resuelve índices de 256 colores estándar ANSI
    pub fn resolve_256_color(&self, index: u8) -> Color {
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
pub fn is_windows_system_light_mode() -> bool {
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
    fn test_default_themes_are_catppuccin() {
        assert_eq!(Theme::DEFAULT_DARK, "Catppuccin Frappe");
        assert_eq!(Theme::DEFAULT_LIGHT, "Catppuccin Latte");
        assert!(Theme::from_name(Theme::DEFAULT_DARK).is_dark());
        assert!(Theme::from_name(Theme::DEFAULT_LIGHT).is_light());
        // Los temas propios retirados se resuelven a los de por defecto
        assert_eq!(Theme::canonical_name("Axon Night"), Theme::DEFAULT_DARK);
        assert_eq!(Theme::canonical_name("Axon Day"), Theme::DEFAULT_LIGHT);
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
}
