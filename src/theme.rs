#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[allow(dead_code)]
impl Color {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };

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
    pub fn r(&self) -> u8 { self.r }
    #[inline(always)]
    pub fn g(&self) -> u8 { self.g }
    #[inline(always)]
    pub fn b(&self) -> u8 { self.b }
    #[inline(always)]
    pub fn a(&self) -> u8 { self.a }

    /// Convierte a formato de pixel 32-bit XRGB nativo de Windows (0x00RRGGBB)
    #[inline(always)]
    pub fn to_u32_xrgb(&self) -> u32 {
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
    /// Paleta oficial y firma de Ghostty Dark
    pub fn ghostty_dark() -> Self {
        Self {
            background: Color::from_rgb(22, 22, 24),     // #161618 - Carbón profundo
            foreground: Color::from_rgb(230, 230, 235), // #e6e6eb - Texto nítido alto contraste
            cursor: Color::from_rgb(255, 255, 255),     // #ffffff - Cursor blanco sólido
            selection: Color::from_rgba(65, 75, 105, 140), // Selección translúcida
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
                Color::from_rgb(92, 95, 110),   // 8: Gris / Negro brillante
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
