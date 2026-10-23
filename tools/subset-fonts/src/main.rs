//! Regenera las fuentes embebidas reducidas (fontdue guarda TODOS los contornos en RAM).
//!
//!   * `JetBrains` Mono Nerd Font -> `*-Core.ttf` (todo menos PUA, Regular y Bold) y dos bloques de
//!     iconos PUA solo Regular: `Regular-Nerd.ttf` (U+E000-F8FF) y `Regular-NerdMd.ttf` (U+F0000+).
//!     El renderer carga cada bloque al ver su primer icono.
//!   * Lucide -> `assets/fonts/lucide.ttf` con solo los iconos de `src/ui/icons.rs`.
//!
//! Uso, desde la raíz del repo (ejecutar de nuevo al añadir un `LucideIcon`):
//!   cargo run --release --manifest-path tools/subset-fonts/Cargo.toml
//!
//! Fuentes completas esperadas en `assets/fonts/src/` (no se embeben):
//!   JetBrainsMonoNerdFont-Regular.ttf, JetBrainsMonoNerdFont-Bold.ttf, lucide-full.ttf

use anyhow::{Context, Result};
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

fn cmap_codepoints(bytes: &[u8]) -> Result<Vec<u32>> {
    let face = ttf_parser::Face::parse(bytes, 0).context("fuente inválida o corrupta en ttf_parser")?;
    let cmap = face.tables().cmap.context("la fuente carece de tabla cmap")?;
    let mut cps = Vec::new();
    for st in cmap.subtables {
        if st.is_unicode() {
            st.codepoints(|c| cps.push(c));
        }
    }
    cps.sort_unstable();
    cps.dedup();
    Ok(cps)
}

/// Codepoints `'\u{xxxx}'` de `src/ui/icons.rs`
fn lucide_codepoints() -> Result<Vec<u32>> {
    let text = std::fs::read_to_string("src/ui/icons.rs")
        .context("no se pudo leer 'src/ui/icons.rs' para extraer codepoints de Lucide")?;
    let mut cps: Vec<u32> = text
        .split(r"'\u{")
        .skip(1)
        .filter_map(|rest| u32::from_str_radix(rest.split('}').next()?, 16).ok())
        .collect();
    cps.sort_unstable();
    cps.dedup();
    Ok(cps)
}

fn write_subset(
    src: &Path,
    dst: &Path,
    codepoints: &[u32],
    keep_calt: bool,
    out_stdout: &mut impl std::io::Write,
) -> Result<()> {
    let file = std::fs::File::open(src)
        .with_context(|| format!("error al abrir archivo fuente '{}'", src.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes)
        .with_context(|| format!("error al leer archivo fuente '{}'", src.display()))?;
    let font = FontRef::new(&bytes)
        .with_context(|| format!("formato de fuente inválido en '{}'", src.display()))?;

    let mut unicodes = IntSet::<u32>::empty();
    unicodes.extend(codepoints.iter().copied());

    let mut drop_tables = IntSet::<Tag>::empty();
    drop_tables.extend([DSIG, KERN]);
    drop_tables.insert(Tag::new(b"GDEF"));
    drop_tables.insert(Tag::new(b"GPOS"));
    if !keep_calt {
        drop_tables.insert(Tag::new(b"GSUB"));
    }

    let mut scripts = IntSet::<Tag>::empty();
    scripts.invert();
    let mut name_ids = IntSet::<NameId>::empty();
    name_ids.extend([NameId::from(1), NameId::from(2)]);
    let mut langs = IntSet::<u16>::empty();
    langs.insert(0x0409);

    let mut layout_features = IntSet::<Tag>::empty();
    if keep_calt {
        layout_features.insert(Tag::new(b"calt"));
    }

    let plan = Plan::new(
        &IntSet::empty(),
        &unicodes,
        &font,
        SubsetFlags::default() | SubsetFlags::SUBSET_FLAGS_NO_HINTING,
        &drop_tables,
        &scripts,
        &layout_features,
        &name_ids,
        &langs,
    );
    let out = subset_font(&font, &plan)
        .map_err(|e| anyhow::anyhow!("falló subset_font para '{}': {e:?}", src.display()))?;
    let file = std::fs::File::create(dst)
        .with_context(|| format!("falló al crear archivo de salida '{}'", dst.display()))?;
    let mut writer = std::io::BufWriter::new(file);
    std::io::Write::write_all(&mut writer, &out)
        .with_context(|| format!("falló al escribir archivo de salida '{}'", dst.display()))?;
    std::io::Write::flush(&mut writer)
        .with_context(|| format!("falló al vaciar buffer de '{}'", dst.display()))?;
    let file_name = dst.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    writeln!(out_stdout, "{}: {} codepoints, {} KB", file_name, codepoints.len(), out.len() / 1024)?;
    Ok(())
}

fn main() -> Result<()> {
    let (src, out) = (Path::new(SRC), Path::new(OUT));
    let stdout = std::io::stdout();
    let mut stdout_lock = stdout.lock();
    for weight in ["Regular", "Bold"] {
        let path = src.join(format!("JetBrainsMonoNerdFont-{weight}.ttf"));
        let font_bytes = std::fs::read(&path)
            .with_context(|| format!("no se pudo leer la fuente base '{}'", path.display()))?;
        let cps = cmap_codepoints(&font_bytes)
            .with_context(|| format!("falló al extraer codepoints de '{}'", path.display()))?;
        let core: Vec<u32> = cps.iter().copied().filter(|&c| !is_pua(c)).collect();
        write_subset(&path, &out.join(format!("JetBrainsMono-{weight}-Core.ttf")), &core, true, &mut stdout_lock)?;
        if weight == "Regular" {
            // Los iconos no tienen variante negrita
            let nerd: Vec<u32> = cps.iter().copied().filter(|c| (0xE000..=0xF8FF).contains(c)).collect();
            let md: Vec<u32> = cps.iter().copied().filter(|&c| c >= 0xF0000).collect();
            write_subset(&path, &out.join("JetBrainsMono-Regular-Nerd.ttf"), &nerd, false, &mut stdout_lock)?;
            write_subset(&path, &out.join("JetBrainsMono-Regular-NerdMd.ttf"), &md, false, &mut stdout_lock)?;
        }
    }
    let lucide_cps = lucide_codepoints()
        .context("falló al extraer codepoints de iconos de Lucide")?;
    write_subset(&src.join("lucide-full.ttf"), &out.join("lucide.ttf"), &lucide_cps, false, &mut stdout_lock)?;
    Ok(())
}

#[test]
fn test_calt_ligatures_preserved_in_subsets() {
    let out_dir = if Path::new(OUT).exists() {
        Path::new(OUT)
    } else {
        Path::new("../../assets/fonts")
    };
    let test_strings = [
        "->", "=>", "!=", "!==", "==", "===", "<=", ">=", ":=", "::",
        "/*", "*/", "//", "///", "<!--", "-->", "##", "###", "&&", "||",
        "??", "?.", "|=", "+=", "-=", "*=", "/=", "%=", "^=", "&=",
        "~=", "<>", "><", "<<=", ">>=", "<<<", ">>>", "...", "..<", "**", "***",
        "--", "---", "----", "----------------",
    ];
    for name in ["JetBrainsMono-Regular-Core.ttf", "JetBrainsMono-Bold-Core.ttf"] {
        let bytes = std::fs::read(out_dir.join(name)).unwrap();
        let face = rustybuzz::Face::from_slice(&bytes, 0).expect("fuente válida");
        for &s in &test_strings {
            let mut buf = rustybuzz::UnicodeBuffer::new();
            buf.push_str(s);
            let glyphs = rustybuzz::shape(&face, &[], buf);
            assert_eq!(
                glyphs.len(),
                s.chars().count(),
                "Fallo en '{}' para {}: longitud {} vs {}",
                s,
                name,
                glyphs.len(),
                s.chars().count()
            );
            for pos in glyphs.glyph_positions() {
                assert_eq!(pos.x_advance, 600);
            }
        }
    }
}
