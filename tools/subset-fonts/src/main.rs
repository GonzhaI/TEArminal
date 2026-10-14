//! Regenera las fuentes embebidas reducidas (fontdue guarda TODOS los contornos en RAM).
//!
//!   * JetBrains Mono Nerd Font -> `*-Core.ttf` (todo menos PUA, Regular y Bold) y dos bloques de
//!     iconos PUA solo Regular: `Regular-Nerd.ttf` (U+E000-F8FF) y `Regular-NerdMd.ttf` (U+F0000+).
//!     El renderer carga cada bloque al ver su primer icono.
//!   * Lucide -> `assets/fonts/lucide.ttf` con solo los iconos de `src/ui/icons.rs`.
//!
//! Uso, desde la raíz del repo (ejecutar de nuevo al añadir un `LucideIcon`):
//!   cargo run --release --manifest-path tools/subset-fonts/Cargo.toml
//!
//! Fuentes completas esperadas en `assets/fonts/src/` (no se embeben):
//!   JetBrainsMonoNerdFont-Regular.ttf, JetBrainsMonoNerdFont-Bold.ttf, lucide-full.ttf

use skera::{subset_font, Plan, SubsetFlags, DSIG, KERN};
use std::path::Path;
use write_fonts::read::{
    collections::IntSet,
    types::{NameId, Tag},
    FontRef,
};

const SRC: &str = "assets/fonts/src";
const OUT: &str = "assets/fonts";

fn is_pua(c: u32) -> bool {
    (0xE000..=0xF8FF).contains(&c) || c >= 0xF0000
}

fn cmap_codepoints(bytes: &[u8]) -> Vec<u32> {
    let face = ttf_parser::Face::parse(bytes, 0).expect("fuente inválida");
    let mut cps = Vec::new();
    for st in face.tables().cmap.expect("sin cmap").subtables {
        if st.is_unicode() {
            st.codepoints(|c| cps.push(c));
        }
    }
    cps.sort_unstable();
    cps.dedup();
    cps
}

/// Codepoints `'\u{xxxx}'` de `src/ui/icons.rs`
fn lucide_codepoints() -> Vec<u32> {
    let text = std::fs::read_to_string("src/ui/icons.rs").expect("src/ui/icons.rs");
    let mut cps: Vec<u32> = text
        .split(r"'\u{")
        .skip(1)
        .filter_map(|rest| u32::from_str_radix(rest.split('}').next()?, 16).ok())
        .collect();
    cps.sort_unstable();
    cps.dedup();
    cps
}

fn write_subset(src: &Path, dst: &Path, codepoints: &[u32]) {
    let bytes = std::fs::read(src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    let font = FontRef::new(&bytes).expect("fuente inválida");

    let mut unicodes = IntSet::<u32>::empty();
    unicodes.extend(codepoints.iter().copied());
    // Sin shaping ni hinting: fontdue solo usa cmap + glyf + hmtx
    let mut drop_tables = IntSet::<Tag>::empty();
    drop_tables.extend([DSIG, KERN]);
    for t in [b"GDEF", b"GSUB", b"GPOS"] {
        drop_tables.insert(Tag::new(t));
    }
    let mut scripts = IntSet::<Tag>::empty();
    scripts.invert();
    let mut name_ids = IntSet::<NameId>::empty();
    name_ids.extend([NameId::from(1), NameId::from(2)]);
    let mut langs = IntSet::<u16>::empty();
    langs.insert(0x0409);

    let plan = Plan::new(
        &IntSet::empty(),
        &unicodes,
        &font,
        SubsetFlags::default() | SubsetFlags::SUBSET_FLAGS_NO_HINTING,
        &drop_tables,
        &scripts,
        &IntSet::empty(),
        &name_ids,
        &langs,
    );
    let out = subset_font(&font, &plan).expect("subset falló");
    std::fs::write(dst, &out).unwrap();
    println!("{}: {} codepoints, {} KB", dst.file_name().unwrap().to_string_lossy(), codepoints.len(), out.len() / 1024);
}

fn main() {
    let (src, out) = (Path::new(SRC), Path::new(OUT));
    for weight in ["Regular", "Bold"] {
        let path = src.join(format!("JetBrainsMonoNerdFont-{weight}.ttf"));
        let cps = cmap_codepoints(&std::fs::read(&path).unwrap());
        let core: Vec<u32> = cps.iter().copied().filter(|&c| !is_pua(c)).collect();
        write_subset(&path, &out.join(format!("JetBrainsMono-{weight}-Core.ttf")), &core);
        if weight == "Regular" {
            // Los iconos no tienen variante negrita
            let nerd: Vec<u32> = cps.iter().copied().filter(|c| (0xE000..=0xF8FF).contains(c)).collect();
            let md: Vec<u32> = cps.iter().copied().filter(|&c| c >= 0xF0000).collect();
            write_subset(&path, &out.join("JetBrainsMono-Regular-Nerd.ttf"), &nerd);
            write_subset(&path, &out.join("JetBrainsMono-Regular-NerdMd.ttf"), &md);
        }
    }
    write_subset(&src.join("lucide-full.ttf"), &out.join("lucide.ttf"), &lucide_codepoints());
}
