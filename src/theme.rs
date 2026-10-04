use egui::Color32;

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    pub ansi: [Color32; 16],
}

impl Theme {
    /// Paleta oficial y firma de Ghostty Dark
    pub fn ghostty_dark() -> Self {
        Self {
            background: Color32::from_rgb(22, 22, 24),     // #161618 - Carbón profundo con sutil tono azul
            foreground: Color32::from_rgb(230, 230, 235), // #e6e6eb - Texto nítido de alto contraste
            cursor: Color32::from_rgb(255, 255, 255),     // #ffffff - Cursor blanco sólido
            selection: Color32::from_rgba_premultiplied(65, 75, 105, 140), // Selección translúcida
            ansi: [
                // Colores normales (0 - 7)
                Color32::from_rgb(28, 28, 32),    // 0: Negro
                Color32::from_rgb(244, 56, 65),   // 1: Rojo Ghostty
                Color32::from_rgb(81, 226, 122),  // 2: Verde vibrante
                Color32::from_rgb(246, 196, 66),  // 3: Amarillo
                Color32::from_rgb(78, 146, 249),  // 4: Azul moderno
                Color32::from_rgb(207, 112, 255), // 5: Magenta
                Color32::from_rgb(54, 214, 214),  // 6: Cian
                Color32::from_rgb(225, 227, 232), // 7: Blanco
                // Colores brillantes (8 - 15)
                Color32::from_rgb(92, 95, 110),   // 8: Gris / Negro brillante
                Color32::from_rgb(255, 107, 112), // 9: Rojo brillante
                Color32::from_rgb(120, 247, 156), // 10: Verde brillante
                Color32::from_rgb(255, 220, 107), // 11: Amarillo brillante
                Color32::from_rgb(120, 175, 255), // 12: Azul brillante
                Color32::from_rgb(223, 148, 255), // 13: Magenta brillante
                Color32::from_rgb(101, 237, 237), // 14: Cian brillante
                Color32::from_rgb(255, 255, 255), // 15: Blanco brillante
            ],
        }
    }

    /// Resuelve índices de 256 colores estándar ANSI
    pub fn resolve_256_color(&self, index: u8) -> Color32 {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                // Cubo de color 6x6x6
                let i = index - 16;
                let r = (i / 36) % 6;
                let g = (i / 6) % 6;
                let b = i % 6;
                let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Color32::from_rgb(step(r), step(g), step(b))
            }
            232..=255 => {
                // Escala de grises (24 pasos de 8 a 238)
                let gray = 8 + (index - 232) * 10;
                Color32::from_rgb(gray, gray, gray)
            }
        }
    }
}
