mod config;
mod pty;
mod terminal;
mod theme;
mod ui;

use config::AxonConfig;
use eframe::egui::Vec2;
use ui::AxonApp;

fn main() -> eframe::Result<()> {
    // 1. Cargar configuración de Axon
    let config = AxonConfig::load_or_default();

    // 2. Opciones de ventana nativa de Windows (DirectX / GPU)
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Axon")
            .with_inner_size(Vec2::new(1020.0, 680.0))
            .with_min_inner_size(Vec2::new(500.0, 320.0))
            .with_transparent(false),
        ..Default::default()
    };

    // 3. Ejecutar la aplicación e inicializar fuentes y fallbacks
    eframe::run_native(
        "Axon Terminal",
        native_options,
        Box::new(|cc| {
            configure_fonts(&cc.egui_ctx, &config.appearance.font_family, config.appearance.font_path.as_deref());
            Ok(Box::new(AxonApp::new(config)))
        }),
    )
}

fn configure_fonts(ctx: &eframe::egui::Context, preferred_font: &str, custom_path: Option<&str>) {
    let mut fonts = eframe::egui::FontDefinitions::default();

    // 1. Si el usuario especificó una ruta directa a un archivo .ttf/.otf (e.g. Nerd Font descargada)
    let mut loaded_primary = false;
    if let Some(path_str) = custom_path {
        let p = std::path::Path::new(path_str);
        if p.exists() {
            if let Ok(bytes) = std::fs::read(p) {
                fonts.font_data.insert(
                    "axon_custom_font".to_owned(),
                    eframe::egui::FontData::from_owned(bytes),
                );
                fonts.families
                    .entry(eframe::egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "axon_custom_font".to_owned());
                loaded_primary = true;
            }
        }
    }

    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let sys_fonts_dir = std::path::Path::new(&windir).join("Fonts");

    // Si no se cargó una ruta directa, buscar la fuente preferida o fuentes monoespaciadas del sistema
    if !loaded_primary {
        let clean_name = preferred_font.replace(' ', "");
        let candidates = [
            format!("{}.ttf", clean_name),
            format!("{}-Regular.ttf", clean_name),
            "CascadiaCode.ttf".to_string(),
            "CascadiaMono.ttf".to_string(),
            "consola.ttf".to_string(),
        ];

        let mut search_dirs = vec![sys_fonts_dir.clone()];
        if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
            search_dirs.push(std::path::Path::new(&local_appdata).join("Microsoft").join("Windows").join("Fonts"));
        }

        'search: for dir in &search_dirs {
            for cand in &candidates {
                let p = dir.join(cand);
                if p.exists() {
                    if let Ok(bytes) = std::fs::read(&p) {
                        fonts.font_data.insert(
                            "axon_primary_mono".to_owned(),
                            eframe::egui::FontData::from_owned(bytes),
                        );
                        fonts.families
                            .entry(eframe::egui::FontFamily::Monospace)
                            .or_default()
                            .insert(0, "axon_primary_mono".to_owned());
                        break 'search;
                    }
                }
            }
        }
    }

    // 2. Fuente de soporte para Braille, operadores matemáticos y símbolos técnicos (Segoe UI Symbol)
    // Esencial para btop, nvtop, barras de progreso y spinners de CLI AI
    let segoe_sym = sys_fonts_dir.join("seguisym.ttf");
    if segoe_sym.exists() {
        if let Ok(bytes) = std::fs::read(&segoe_sym) {
            fonts.font_data.insert(
                "axon_segoe_sym".to_owned(),
                eframe::egui::FontData::from_owned(bytes),
            );
            fonts.families
                .entry(eframe::egui::FontFamily::Monospace)
                .or_default()
                .push("axon_segoe_sym".to_owned());
        }
    }

    // 3. Fuente de emojis (Segoe UI Emoji)
    let segoe_emoji = sys_fonts_dir.join("seguiemj.ttf");
    if segoe_emoji.exists() {
        if let Ok(bytes) = std::fs::read(&segoe_emoji) {
            fonts.font_data.insert(
                "axon_segoe_emoji".to_owned(),
                eframe::egui::FontData::from_owned(bytes),
            );
            fonts.families
                .entry(eframe::egui::FontFamily::Monospace)
                .or_default()
                .push("axon_segoe_emoji".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_loading_performance() {
        let ctx = eframe::egui::Context::default();
        let start = std::time::Instant::now();
        configure_fonts(&ctx, "Cascadia Code", None);
        println!("configure_fonts took: {:?}", start.elapsed());

        let _ = ctx.run(Default::default(), |ctx| {
            let font_id = eframe::egui::FontId::monospace(14.0);
            let start2 = std::time::Instant::now();
            let width = ctx.fonts(|f| f.glyph_width(&font_id, 'M'));
            println!("glyph_width('M') took: {:?}, width={}", start2.elapsed(), width);
        });
    }
}
