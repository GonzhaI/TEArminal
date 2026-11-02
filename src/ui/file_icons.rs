//! Icono de un archivo según su nombre o extensión: glifo de la fuente Nerd embebida (Seti, Devicons,
//! Octicons: logos de lenguajes y tipos de archivo) y el color ANSI (0..16) con que se pinta.
//! Lo usan el explorador y la cabecera del visor. Los glifos están todos en el bloque `U+E000..=F8FF`
//! de `JetBrainsMono-Regular-Nerd.ttf`; `test_every_file_glyph_exists_in_embedded_nerd_font` lo exige.

use std::path::Path;

/// Archivo sin tipo conocido (`oct-file`)
const DEFAULT: (char, usize) = ('\u{f4a5}', 8);

/// Glifo Nerd de una carpeta, cerrada o abierta (`fa-folder_o`, `fa-folder_open_o`).
pub(crate) fn folder_glyph(expanded: bool) -> char {
    if expanded {
        '\u{f115}'
    } else {
        '\u{f114}'
    }
}

/// Glifo Nerd y color ANSI del archivo `path`: primero por nombre exacto o prefijo (`Dockerfile`,
/// `Cargo.toml`, `.env*`...), después por extensión.
pub(crate) fn file_glyph(path: &Path) -> (char, usize) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if let Some(found) = glyph_by_name(&name) {
        return found;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    glyph_by_extension(&ext)
}

fn glyph_by_name(name: &str) -> Option<(char, usize)> {
    Some(match name {
        "cargo.toml" => ('\u{e68b}', 3),   // seti-rust
        "cargo.lock" => ('\u{e672}', 8),   // seti-lock
        "package.json" => ('\u{e71e}', 1), // dev-npm
        "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "composer.lock" => {
            ('\u{e672}', 8) // seti-lock
        }
        "makefile" | "gnumakefile" | "justfile" | "rakefile" => ('\u{e673}', 3), // seti-makefile
        "cmakelists.txt" => ('\u{e794}', 2),                                     // dev-cmake
        ".gitignore" | ".gitattributes" | ".gitmodules" => ('\u{e65d}', 1),      // seti-git_ignore
        ".editorconfig" => ('\u{e652}', 8), // seti-editorconfig
        "license" | "licence" | "copying" => ('\u{e60a}', 3), // seti-license
        "readme" => ('\u{e66a}', 6),        // seti-info
        n if n.starts_with("dockerfile")
            || n.starts_with("containerfile")
            || n.starts_with("docker-compose")
            || n == ".dockerignore" =>
        {
            ('\u{e650}', 4) // seti-docker
        }
        n if n.starts_with(".env") => ('\u{e615}', 3), // seti-config
        n if n.starts_with("tsconfig") => ('\u{e69d}', 4), // seti-tsconfig
        n if n.starts_with(".eslintrc") || n.starts_with("eslint.config") => ('\u{e655}', 5), // seti-eslint
        n if n.starts_with("webpack.config") => ('\u{e6a3}', 4), // seti-webpack
        _ => return None,
    })
}

fn glyph_by_extension(ext: &str) -> (char, usize) {
    match ext {
        // Lenguajes de sistemas y nativos
        "rs" => ('\u{e68b}', 1), // seti-rust
        "c" => ('\u{e649}', 4),  // seti-c
        "h" => ('\u{e649}', 5),  // seti-c
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "inl" => ('\u{e646}', 4), // seti-cpp
        "cs" | "csx" => ('\u{e648}', 2), // seti-c_sharp
        "fs" | "fsx" | "fsi" => ('\u{e65a}', 6), // seti-f_sharp
        "go" => ('\u{e627}', 6), // seti-go
        "zig" => ('\u{e6a9}', 3), // seti-zig
        "d" => ('\u{e651}', 1),  // seti-d
        "nim" => ('\u{e677}', 3), // seti-nim
        "swift" => ('\u{e699}', 1), // seti-swift
        "asm" | "s" => ('\u{e637}', 1), // seti-asm

        // JVM
        "java" => ('\u{e66d}', 1),                          // seti-java
        "kt" | "kts" => ('\u{e634}', 5),                    // seti-kotlin
        "scala" | "sbt" => ('\u{e68e}', 1),                 // seti-scala
        "groovy" => ('\u{e775}', 4),                        // dev-groovy
        "gradle" => ('\u{e660}', 6),                        // seti-gradle
        "clj" | "cljs" | "cljc" | "edn" => ('\u{e642}', 2), // seti-clojure

        // Scripting
        "py" | "pyw" | "pyi" => ('\u{e606}', 4), // seti-python
        "ipynb" => ('\u{e678}', 3),              // seti-notebook
        "rb" | "erb" | "rake" | "gemspec" => ('\u{e605}', 1), // seti-ruby
        "php" | "phtml" => ('\u{e608}', 5),      // seti-php
        "lua" => ('\u{e620}', 4),                // seti-lua
        "pl" | "pm" => ('\u{e67e}', 4),          // seti-perl
        "r" => ('\u{e68a}', 4),                  // seti-r
        "jl" => ('\u{e624}', 5),                 // seti-julia
        "dart" => ('\u{e64c}', 6),               // seti-dart
        "sh" | "bash" | "zsh" | "fish" => ('\u{e691}', 2), // seti-shell
        "ps1" | "psm1" | "psd1" => ('\u{e683}', 4), // seti-powershell
        "bat" | "cmd" => ('\u{e795}', 2),        // dev-terminal

        // Funcionales
        "hs" | "lhs" => ('\u{e61f}', 5),          // seti-haskell
        "ex" | "exs" | "heex" => ('\u{e62d}', 5), // seti-elixir
        "elm" => ('\u{e62c}', 6),                 // seti-elm
        "ml" | "mli" => ('\u{e67a}', 3),          // seti-ocaml

        // Web
        "js" | "mjs" | "cjs" => ('\u{e60c}', 3), // seti-javascript
        "jsx" | "tsx" => ('\u{e625}', 6),        // seti-react
        "ts" | "mts" | "cts" => ('\u{e628}', 4), // seti-typescript
        "html" | "htm" | "xhtml" => ('\u{e60e}', 1), // seti-html
        "css" => ('\u{e614}', 4),                // seti-css
        "scss" | "sass" => ('\u{e603}', 5),      // seti-sass
        "less" => ('\u{e60b}', 4),               // seti-less
        "styl" => ('\u{e600}', 2),               // seti-stylus
        "vue" => ('\u{e6a0}', 2),                // seti-vue
        "svelte" => ('\u{e697}', 1),             // seti-svelte
        "astro" => ('\u{e735}', 5),              // dev-astro
        "xml" | "xaml" | "xsd" | "xsl" => ('\u{e619}', 3), // seti-xml
        "svg" => ('\u{e698}', 3),                // seti-svg
        "wasm" => ('\u{e6a1}', 5),               // seti-wasm
        "wat" => ('\u{e6a2}', 5),                // seti-wat
        "graphql" | "gql" => ('\u{e662}', 5),    // seti-graphql
        "prisma" => ('\u{e684}', 6),             // seti-prisma

        // Datos y configuración
        "json" | "jsonc" | "json5" => ('\u{e80b}', 3), // dev-json
        "toml" | "ini" | "cfg" | "conf" | "properties" => ('\u{e615}', 3), // seti-config
        "yaml" | "yml" => ('\u{e6a8}', 5),             // seti-yml
        "csv" | "tsv" => ('\u{e64a}', 2),              // seti-csv
        "sql" | "db" | "sqlite" | "sqlite3" => ('\u{e64d}', 4), // seti-db
        "tf" | "tfvars" => ('\u{e69a}', 5),            // seti-terraform
        "tex" | "bib" => ('\u{e69b}', 6),              // seti-tex

        // Documentos
        "md" | "markdown" | "mdx" => ('\u{e609}', 6), // seti-markdown
        "txt" | "log" | "rst" => ('\u{e64e}', 8),     // seti-text
        "pdf" => ('\u{e67d}', 1),                     // seti-pdf
        "doc" | "docx" | "odt" | "rtf" => ('\u{e6a5}', 4), // seti-word
        "xls" | "xlsx" | "ods" => ('\u{e6a6}', 2),    // seti-xls
        "ppt" | "pptx" | "odp" => ('\u{f1c4}', 1),    // fa-file_powerpoint_o

        // Multimedia
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" | "tiff" | "tif" => {
            ('\u{e60d}', 5) // seti-image
        }
        "psd" => ('\u{e67f}', 4), // seti-photoshop
        "ai" => ('\u{e669}', 3),  // seti-illustrator
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => ('\u{e638}', 5), // seti-audio
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "flv" => ('\u{e69f}', 5), // seti-video
        "ttf" | "otf" | "woff" | "woff2" | "eot" => ('\u{e659}', 8), // seti-font

        // Binarios, archivos comprimidos y seguridad
        "exe" | "msi" => ('\u{f471}', 2), // oct-file_binary
        "dll" | "so" | "dylib" | "bin" | "o" | "obj" | "a" | "lib" | "class" | "dex" => {
            ('\u{f471}', 8) // oct-file_binary
        }
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" => ('\u{e6aa}', 3), // seti-zip
        "pem" | "key" | "pub" | "crt" | "cer" | "der" | "pfx" | "p12" | "asc" | "gpg" => {
            ('\u{eb11}', 3) // cod-key
        }
        "lock" => ('\u{e672}', 8),           // seti-lock
        "diff" | "patch" => ('\u{f4d2}', 3), // oct-file_diff

        _ => DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_glyph_by_extension_and_name() {
        let g = |p: &str| file_glyph(Path::new(p));
        assert_eq!(g("src/main.rs"), ('\u{e68b}', 1));
        assert_eq!(
            g("MAIN.RS"),
            ('\u{e68b}', 1),
            "la extensión no distingue mayúsculas"
        );
        assert_eq!(g("a/b/script.py").0, '\u{e606}');
        assert_eq!(
            g("Cargo.toml"),
            ('\u{e68b}', 3),
            "el nombre gana a la extensión"
        );
        assert_eq!(g("Dockerfile.dev").0, '\u{e650}');
        assert_eq!(g(".env.local").0, '\u{e615}');
        assert_eq!(g("tsconfig.base.json").0, '\u{e69d}');
        assert_eq!(g("sin_extension"), DEFAULT);
        assert_eq!(g("raro.zzz"), DEFAULT);
    }

    /// Cada glifo de la tabla debe existir en el bloque Nerd embebido (si no, se vería un recuadro vacío).
    #[test]
    fn test_every_file_glyph_exists_in_embedded_nerd_font() {
        let font = fontdue::Font::from_bytes(
            &include_bytes!("../../assets/fonts/JetBrainsMono-Regular-Nerd.ttf")[..],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let src = include_str!("file_icons.rs");
        let mut checked = 0;
        for chunk in src.split("'\\u{").skip(1) {
            let hex = chunk.split('}').next().unwrap();
            let c = char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap();
            assert!(
                font.has_glyph(c),
                "U+{hex} no está en la fuente Nerd embebida"
            );
            checked += 1;
        }
        assert!(checked > 90, "se esperaban >90 glifos, hay {checked}");
    }
}
