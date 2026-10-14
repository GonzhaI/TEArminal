#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[allow(dead_code)]
impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub fn r(&self) -> u8 {
        self.r
    }
    #[inline(always)]
    pub fn g(&self) -> u8 {
        self.g
    }
    #[inline(always)]
    pub fn b(&self) -> u8 {
        self.b
    }
    #[inline(always)]
    pub fn a(&self) -> u8 {
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

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub ansi: [Color; 16],
}

impl Theme {
    /// Paleta oficial y firma de Ghostty Default StyleDark
    pub fn ghostty_default_styledark() -> Self {
        Self {
            background: Color::from_rgb(22, 22, 24), // #161618 - Carbón profundo Ghostty
            foreground: Color::from_rgb(230, 230, 235), // #e6e6eb - Texto nítido alto contraste
            cursor: Color::from_rgb(255, 255, 255),  // #ffffff - Cursor blanco sólido
            selection: Color::from_rgba(54, 61, 89, 220), // #363d59 - Selección azul pizarra Ghostty
            ansi: [
                // Colores normales (0 - 7)
                Color::from_rgb(28, 28, 32),    // 0: Negro
                Color::from_rgb(244, 56, 65),   // 1: Rojo Ghostty
                Color::from_rgb(81, 226, 122),  // 2: Verde vibrante
                Color::from_rgb(246, 196, 66),  // 3: Amarillo
                Color::from_rgb(78, 146, 249),  // 4: Azul moderno
                Color::from_rgb(207, 112, 255), // 5: Magenta
                Color::from_rgb(54, 214, 214),  // 6: Cian
                Color::from_rgb(225, 227, 232), // 7: Blanco
                // Colores brillantes (8 - 15)
                Color::from_rgb(92, 95, 110), // 8: Gris / Negro brillante
                Color::from_rgb(255, 107, 112), // 9: Rojo brillante
                Color::from_rgb(120, 247, 156), // 10: Verde brillante
                Color::from_rgb(255, 220, 107), // 11: Amarillo brillante
                Color::from_rgb(120, 175, 255), // 12: Azul brillante
                Color::from_rgb(223, 148, 255), // 13: Magenta brillante
                Color::from_rgb(101, 237, 237), // 14: Cian brillante
                Color::from_rgb(255, 255, 255), // 15: Blanco brillante
            ],
        }
    }

    /// Tema oficial Catppuccin Frappé
    pub fn catppuccin_frappe() -> Self {
        Self {
            background: Color::from_rgb(48, 52, 70),    // #303446 (Base)
            foreground: Color::from_rgb(198, 208, 245), // #c6d0f5 (Text)
            cursor: Color::from_rgb(242, 213, 207),     // #f2d5cf (Rosewater)
            selection: Color::from_rgba(81, 87, 109, 220), // #51576d (Surface 1)
            ansi: [
                Color::from_rgb(81, 87, 109),   // 0: Surface 1 (#51576d)
                Color::from_rgb(231, 130, 132), // 1: Red (#e78284)
                Color::from_rgb(166, 209, 137), // 2: Green (#a6d189)
                Color::from_rgb(229, 200, 144), // 3: Yellow (#e5c890)
                Color::from_rgb(140, 170, 238), // 4: Blue (#8caaee)
                Color::from_rgb(202, 158, 230), // 5: Mauve (#ca9ee6)
                Color::from_rgb(129, 200, 190), // 6: Teal (#81c8be)
                Color::from_rgb(181, 191, 226), // 7: Subtext 1 (#b5bfe2)
                Color::from_rgb(98, 104, 128),  // 8: Surface 2 (#626880)
                Color::from_rgb(234, 153, 156), // 9: Maroon (#ea999c)
                Color::from_rgb(166, 209, 137), // 10: Bright Green
                Color::from_rgb(239, 159, 118), // 11: Peach (#ef9f76)
                Color::from_rgb(133, 193, 220), // 12: Sapphire (#85c1dc)
                Color::from_rgb(244, 184, 228), // 13: Pink (#f4b8e4)
                Color::from_rgb(153, 209, 219), // 14: Sky (#99d1db)
                Color::from_rgb(198, 208, 245), // 15: Text (#c6d0f5)
            ],
        }
    }

    /// Tema oficial Catppuccin Latte (Modo Claro Cálido)
    pub fn catppuccin_latte() -> Self {
        Self {
            background: Color::from_rgb(239, 241, 245), // #eff1f5 (Base)
            foreground: Color::from_rgb(76, 79, 105),   // #4c4f69 (Text)
            cursor: Color::from_rgb(220, 138, 120),     // #dc8a78 (Rosewater)
            selection: Color::from_rgba(188, 192, 204, 220), // #bcc0cc (Surface 1)
            ansi: [
                Color::from_rgb(188, 192, 204), // 0: Surface 1 (#bcc0cc)
                Color::from_rgb(210, 15, 57),   // 1: Red (#d20f39)
                Color::from_rgb(64, 160, 43),   // 2: Green (#40a02b)
                Color::from_rgb(223, 142, 29),  // 3: Yellow (#df8e1d)
                Color::from_rgb(30, 102, 245),  // 4: Blue (#1e66f5)
                Color::from_rgb(136, 57, 239),  // 5: Mauve (#8839ef)
                Color::from_rgb(23, 146, 153),  // 6: Teal (#179299)
                Color::from_rgb(92, 95, 119),   // 7: Subtext 1 (#5c5f77)
                Color::from_rgb(172, 176, 190), // 8: Surface 2 (#acb0be)
                Color::from_rgb(230, 69, 83),   // 9: Maroon (#e64553)
                Color::from_rgb(64, 160, 43),   // 10: Bright Green
                Color::from_rgb(254, 100, 11),  // 11: Peach (#fe640b)
                Color::from_rgb(32, 159, 181),  // 12: Sapphire (#209fb5)
                Color::from_rgb(234, 118, 203), // 13: Pink (#ea76cb)
                Color::from_rgb(4, 165, 229),   // 14: Sky (#04a5e5)
                Color::from_rgb(76, 79, 105),   // 15: Text (#4c4f69)
            ],
        }
    }

    /// Determina si el tema activo es de tonalidad clara (Light Mode)
    #[inline]
    pub fn is_light(&self) -> bool {
        let lum = 0.2126 * (self.background.r as f32)
            + 0.7152 * (self.background.g as f32)
            + 0.0722 * (self.background.b as f32);
        lum > 140.0
    }

    /// Color de fondo optimizado para la barra lateral según la paleta activa
    pub fn sidebar_bg(&self) -> Color {
        if self.background == Color::from_rgb(239, 241, 245) {
            Color::from_rgb(230, 233, 239) // #e6e9ef Catppuccin Mantle (Latte)
        } else if self.background == Color::from_rgb(48, 52, 70) {
            Color::from_rgb(41, 44, 60) // #292c3c Catppuccin Mantle (Frappé)
        } else if self.background == Color::from_rgb(22, 22, 24) {
            Color::from_rgb(18, 18, 20) // Ghostty sidebar
        } else if self.is_light() {
            Color::from_rgb(
                (self.background.r as f32 * 0.94).round() as u8,
                (self.background.g as f32 * 0.94).round() as u8,
                (self.background.b as f32 * 0.94).round() as u8,
            )
        } else {
            Color::from_rgb(
                (self.background.r as f32 * 0.84).round() as u8,
                (self.background.g as f32 * 0.84).round() as u8,
                (self.background.b as f32 * 0.84).round() as u8,
            )
        }
    }

    /// Color divisorio sutil y fino acorde al tema
    pub fn border_color(&self) -> Color {
        if self.background == Color::from_rgb(239, 241, 245) {
            Color::from_rgb(188, 192, 204) // Catppuccin Surface 1 (Latte)
        } else if self.background == Color::from_rgb(48, 52, 70) {
            Color::from_rgb(71, 76, 100) // Catppuccin Surface 1 border (Frappé)
        } else if self.background == Color::from_rgb(22, 22, 24) {
            Color::from_rgb(48, 48, 56) // Ghostty border nítido
        } else if self.is_light() {
            Color::from_rgb(
                self.background.r.saturating_sub(35),
                self.background.g.saturating_sub(35),
                self.background.b.saturating_sub(35),
            )
        } else {
            Color::from_rgb(
                self.background.r.saturating_add(26),
                self.background.g.saturating_add(26),
                self.background.b.saturating_add(32),
            )
        }
    }

    /// Color de superficie para tarjetas, ítems activos y botones
    pub fn surface_bg(&self) -> Color {
        if self.background == Color::from_rgb(239, 241, 245) {
            Color::from_rgb(220, 224, 232) // #dce0e8 Catppuccin Crust / Surface 0 (Latte)
        } else if self.background == Color::from_rgb(48, 52, 70) {
            Color::from_rgb(62, 66, 88) // Catppuccin Surface 0
        } else if self.background == Color::from_rgb(22, 22, 24) {
            Color::from_rgb(34, 34, 39) // Ghostty surface
        } else if self.is_light() {
            Color::from_rgb(
                self.background.r.saturating_sub(20),
                self.background.g.saturating_sub(20),
                self.background.b.saturating_sub(20),
            )
        } else {
            Color::from_rgb(
                self.background.r.saturating_add(18),
                self.background.g.saturating_add(18),
                self.background.b.saturating_add(24),
            )
        }
    }

    /// Color de superficie para estados hover
    pub fn surface_hover(&self) -> Color {
        if self.background == Color::from_rgb(239, 241, 245) {
            Color::from_rgb(172, 176, 190) // Catppuccin Surface 1 (Latte)
        } else if self.background == Color::from_rgb(48, 52, 70) {
            Color::from_rgb(86, 92, 116) // Catppuccin Surface 1 (Frappé)
        } else if self.background == Color::from_rgb(22, 22, 24) {
            Color::from_rgb(46, 46, 54) // Ghostty hover
        } else if self.is_light() {
            Color::from_rgb(
                self.background.r.saturating_sub(40),
                self.background.g.saturating_sub(40),
                self.background.b.saturating_sub(40),
            )
        } else {
            Color::from_rgb(
                self.background.r.saturating_add(32),
                self.background.g.saturating_add(32),
                self.background.b.saturating_add(40),
            )
        }
    }

    /// Color de acento de interfaz primario
    pub fn accent_color(&self) -> Color {
        if self.background == Color::from_rgb(239, 241, 245) {
            Color::from_rgb(30, 102, 245) // Catppuccin Blue (#1e66f5)
        } else if self.background == Color::from_rgb(48, 52, 70) {
            Color::from_rgb(202, 158, 230) // Catppuccin Mauve (#ca9ee6)
        } else {
            self.ansi[2] // Verde vibrante Ghostty / ANSI green
        }
    }

    /// Alias de compatibilidad para la paleta Ghostty
    #[allow(dead_code)]
    pub fn ghostty_dark() -> Self {
        Self::ghostty_default_styledark()
    }

    /// Tema Atom / VS Code One Dark
    pub fn one_dark() -> Self {
        Self {
            background: Color::from_rgb(40, 44, 52),
            foreground: Color::from_rgb(171, 178, 191),
            cursor: Color::from_rgb(82, 139, 255),
            selection: Color::from_rgba(62, 68, 81, 220),
            ansi: [
                Color::from_rgb(40, 44, 52),
                Color::from_rgb(224, 108, 117),
                Color::from_rgb(152, 195, 121),
                Color::from_rgb(229, 192, 123),
                Color::from_rgb(97, 175, 239),
                Color::from_rgb(198, 120, 221),
                Color::from_rgb(86, 182, 194),
                Color::from_rgb(171, 178, 191),
                Color::from_rgb(92, 99, 112),
                Color::from_rgb(224, 108, 117),
                Color::from_rgb(152, 195, 121),
                Color::from_rgb(229, 192, 123),
                Color::from_rgb(97, 175, 239),
                Color::from_rgb(198, 120, 221),
                Color::from_rgb(86, 182, 194),
                Color::from_rgb(255, 255, 255),
            ],
        }
    }

    /// Tema Tokyo Night
    pub fn tokyo_night() -> Self {
        Self {
            background: Color::from_rgb(26, 27, 38),
            foreground: Color::from_rgb(192, 202, 245),
            cursor: Color::from_rgb(192, 202, 245),
            selection: Color::from_rgba(51, 70, 126, 220),
            ansi: [
                Color::from_rgb(21, 22, 30),
                Color::from_rgb(247, 118, 142),
                Color::from_rgb(158, 206, 106),
                Color::from_rgb(224, 175, 104),
                Color::from_rgb(122, 162, 247),
                Color::from_rgb(187, 154, 247),
                Color::from_rgb(125, 207, 255),
                Color::from_rgb(169, 177, 214),
                Color::from_rgb(65, 72, 104),
                Color::from_rgb(247, 118, 142),
                Color::from_rgb(158, 206, 106),
                Color::from_rgb(224, 175, 104),
                Color::from_rgb(122, 162, 247),
                Color::from_rgb(187, 154, 247),
                Color::from_rgb(125, 207, 255),
                Color::from_rgb(192, 202, 245),
            ],
        }
    }

    /// Tema Monokai Pro
    pub fn monokai_pro() -> Self {
        Self {
            background: Color::from_rgb(45, 42, 46),
            foreground: Color::from_rgb(252, 252, 250),
            cursor: Color::from_rgb(255, 216, 102),
            selection: Color::from_rgba(64, 62, 65, 220),
            ansi: [
                Color::from_rgb(45, 42, 46),
                Color::from_rgb(255, 97, 136),
                Color::from_rgb(169, 220, 103),
                Color::from_rgb(255, 216, 102),
                Color::from_rgb(120, 220, 232),
                Color::from_rgb(171, 157, 242),
                Color::from_rgb(120, 220, 232),
                Color::from_rgb(252, 252, 250),
                Color::from_rgb(114, 112, 114),
                Color::from_rgb(255, 97, 136),
                Color::from_rgb(169, 220, 103),
                Color::from_rgb(255, 216, 102),
                Color::from_rgb(120, 220, 232),
                Color::from_rgb(171, 157, 242),
                Color::from_rgb(120, 220, 232),
                Color::from_rgb(255, 255, 255),
            ],
        }
    }

    /// Tema Ártico Nord
    pub fn nord() -> Self {
        Self {
            background: Color::from_rgb(46, 52, 64),
            foreground: Color::from_rgb(216, 222, 233),
            cursor: Color::from_rgb(216, 222, 233),
            selection: Color::from_rgba(67, 76, 94, 220),
            ansi: [
                Color::from_rgb(59, 66, 82),
                Color::from_rgb(191, 97, 106),
                Color::from_rgb(163, 190, 140),
                Color::from_rgb(235, 203, 139),
                Color::from_rgb(129, 161, 193),
                Color::from_rgb(180, 142, 173),
                Color::from_rgb(136, 192, 208),
                Color::from_rgb(229, 233, 240),
                Color::from_rgb(76, 86, 106),
                Color::from_rgb(191, 97, 106),
                Color::from_rgb(163, 190, 140),
                Color::from_rgb(235, 203, 139),
                Color::from_rgb(129, 161, 193),
                Color::from_rgb(180, 142, 173),
                Color::from_rgb(143, 188, 187),
                Color::from_rgb(236, 239, 244),
            ],
        }
    }

    /// Tema Solarized Dark
    pub fn solarized_dark() -> Self {
        Self {
            background: Color::from_rgb(0, 43, 54),
            foreground: Color::from_rgb(131, 148, 150),
            cursor: Color::from_rgb(147, 161, 161),
            selection: Color::from_rgba(7, 54, 66, 220),
            ansi: [
                Color::from_rgb(7, 54, 66),
                Color::from_rgb(220, 50, 47),
                Color::from_rgb(133, 153, 0),
                Color::from_rgb(181, 137, 0),
                Color::from_rgb(38, 139, 210),
                Color::from_rgb(211, 54, 130),
                Color::from_rgb(42, 161, 152),
                Color::from_rgb(238, 232, 213),
                Color::from_rgb(0, 43, 54),
                Color::from_rgb(203, 75, 22),
                Color::from_rgb(88, 110, 117),
                Color::from_rgb(101, 123, 131),
                Color::from_rgb(131, 148, 150),
                Color::from_rgb(108, 113, 196),
                Color::from_rgb(147, 161, 161),
                Color::from_rgb(253, 246, 227),
            ],
        }
    }

    /// Lista de temas oficiales disponibles en Axon
    pub const AVAILABLE_THEMES: &'static [&'static str] = &[
        "Ghostty Dark",
        "Catppuccin Frappé",
        "Catppuccin Latte",
        "One Dark",
        "Tokyo Night",
        "Monokai Pro",
        "Nord",
        "Solarized Dark",
    ];

    /// Comprueba de forma flexible si dos nombres de tema corresponden al mismo tema visual
    pub fn is_same_theme(cfg_theme: &str, display_name: &str) -> bool {
        let c1 = cfg_theme.trim().to_lowercase().replace(['-', '_'], " ");
        let c2 = display_name.trim().to_lowercase().replace(['-', '_'], " ");
        if c1 == c2 {
            return true;
        }
        if (c1.contains("frappe") || c1.contains("frappé") || c1 == "dark" || c1 == "default")
            && (c2.contains("frappe") || c2.contains("frappé"))
        {
            return true;
        }
        if c1.contains("latte") && c2.contains("latte") {
            return true;
        }
        if c1.contains("ghostty") && c2.contains("ghostty") {
            return true;
        }
        if c1.contains("one") && c2.contains("one") {
            return true;
        }
        if c1.contains("tokyo") && c2.contains("tokyo") {
            return true;
        }
        if c1.contains("monokai") && c2.contains("monokai") {
            return true;
        }
        if c1.contains("nord") && c2.contains("nord") {
            return true;
        }
        if c1.contains("solarized") && c2.contains("solarized") {
            return true;
        }
        false
    }

    /// Resuelve el tema configurado por nombre
    pub fn from_name(name: &str) -> Self {
        let clean = name.trim().to_lowercase().replace(['-', '_'], " ");
        if clean.contains("ghostty") {
            Self::ghostty_default_styledark()
        } else if clean.contains("latte") {
            Self::catppuccin_latte()
        } else if clean.contains("catppuccin")
            || clean.contains("frappe")
            || clean.contains("frappé")
            || clean == "dark"
            || clean == "default"
        {
            Self::catppuccin_frappe()
        } else if clean.contains("one") {
            Self::one_dark()
        } else if clean.contains("tokyo") {
            Self::tokyo_night()
        } else if clean.contains("monokai") {
            Self::monokai_pro()
        } else if clean.contains("nord") {
            Self::nord()
        } else if clean.contains("solarized") {
            Self::solarized_dark()
        } else {
            Self::catppuccin_frappe()
        }
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
    #[test]
    fn test_catppuccin_latte_and_light_mode() {
        let latte = crate::theme::Theme::catppuccin_latte();
        assert!(
            latte.is_light(),
            "Catppuccin Latte debe reportar is_light() == true"
        );
        assert_eq!(
            latte.background,
            crate::theme::Color::from_rgb(0xef, 0xf1, 0xf5)
        );
        assert_eq!(
            latte.foreground,
            crate::theme::Color::from_rgb(0x4c, 0x4f, 0x69)
        );

        let frappe = crate::theme::Theme::catppuccin_frappe();
        assert!(
            !frappe.is_light(),
            "Catppuccin Frappé debe reportar is_light() == false"
        );

        assert!(crate::theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Latte"));
        assert!(crate::theme::Theme::AVAILABLE_THEMES.contains(&"Catppuccin Frappé"));

        let resolved_latte = crate::theme::Theme::from_name("latte");
        assert!(resolved_latte.is_light());
    }
}
