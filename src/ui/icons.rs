/// Catálogo de iconos Lucide vectoriales embebidos para Axon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LucideIcon {
    SquareTerminal,
    Terminal,
    Sparkles,
    Server,
    Plus,
    X,
    Minus,
    Square,
    Copy,
    Settings,
    FolderTree,
    Folder,
    FolderOpen,
    File,
    FileCode,
    FileText,
    FileJson,
    FileCog,
    ChevronRight,
    ChevronDown,
    ChevronLeft,
    RefreshCw,
    ArrowUp,
    Search,
    GitBranch,
    Columns2,
    Rows2,
    PanelRight,
    PanelLeft,
    Dot,
    Bot,
    Palette,
    Command,
    Keyboard,
    Info,
    Sliders,
    Check,
    Maximize2,
    Minimize2,
    Lock,
    Pencil,
    Eye,
    Code,
    Scissors,
    ClipboardPaste,
    Undo2,
    Redo2,
    Save,
    ExternalLink,
    Trash2,
    Link,
    Heading,
    List,
    ListOrdered,
    Quote,
    CheckSquare,
    ListChecks,
    ScanText,
    Image,
    SeparatorHorizontal,
    WrapText,
    Database,
    FileArchive,
    FileSpreadsheet,
    FileAudio,
    FileVideo,
    Binary,
    FileTerminal,
    FileKey,
    FileLock,
    FileDiff,
    FileBadge,
    FileImage,
}

impl LucideIcon {
    /// Devuelve el carácter Unicode codificado en la tipografía `lucide.ttf`
    pub(crate) fn as_char(&self) -> char {
        match self {
            Self::SquareTerminal => '\u{e20a}',
            Self::Terminal => '\u{e181}',
            Self::Sparkles => '\u{e412}',
            Self::Server => '\u{e153}',
            Self::Plus => '\u{e13d}',
            Self::X => '\u{e1b2}',
            Self::Minus => '\u{e11c}',
            Self::Square => '\u{e167}',
            Self::Copy => '\u{e09e}',
            Self::Settings => '\u{e154}',
            Self::FolderTree => '\u{e33c}',
            Self::Folder => '\u{e0d7}',
            Self::FolderOpen => '\u{e247}',
            Self::File => '\u{e0c0}',
            Self::FileCode => '\u{e0c3}',
            Self::FileText => '\u{e0cc}',
            Self::FileJson => '\u{e36b}',
            Self::FileCog => '\u{e316}',
            Self::ChevronRight => '\u{e06f}',
            Self::ChevronDown => '\u{e06d}',
            Self::ChevronLeft => '\u{e06e}',
            Self::RefreshCw => '\u{e145}',
            Self::ArrowUp => '\u{e04a}',
            Self::Search => '\u{e151}',
            Self::GitBranch => '\u{e0e2}',
            Self::Columns2 => '\u{e098}',
            Self::Rows2 => '\u{e439}',
            Self::PanelRight => '\u{e431}',
            Self::PanelLeft => '\u{e430}',
            Self::Dot => '\u{e44f}',
            Self::Bot => '\u{e1bb}',
            Self::Palette => '\u{e1dd}',
            Self::Command => '\u{e09a}',
            Self::Keyboard => '\u{e284}',
            Self::Info => '\u{e0f9}',
            Self::Sliders => '\u{e29a}',
            Self::Check => '\u{e06c}',
            Self::Maximize2 => '\u{e113}',
            Self::Minimize2 => '\u{e11b}',
            Self::Lock => '\u{e10b}',
            Self::Pencil => '\u{e1f9}',
            Self::Eye => '\u{e0ba}',
            Self::Code => '\u{e093}',
            Self::Scissors => '\u{e14e}',
            Self::ClipboardPaste => '\u{e3e8}',
            Self::Undo2 => '\u{e2a1}',
            Self::Redo2 => '\u{e2a0}',
            Self::Save => '\u{e14d}',
            Self::ExternalLink => '\u{e0b9}',
            Self::Trash2 => '\u{e18e}',
            Self::Link => '\u{e102}',
            Self::Heading => '\u{e384}',
            Self::List => '\u{e106}',
            Self::ListOrdered => '\u{e1d1}',
            Self::Quote => '\u{e239}',
            Self::CheckSquare => '\u{e16a}',
            Self::ListChecks => '\u{e1d0}',
            Self::ScanText => '\u{e538}',
            Self::Image => '\u{e0f6}',
            Self::SeparatorHorizontal => '\u{e1c8}',
            Self::WrapText => '\u{e248}',
            Self::Database => '\u{e0ad}',
            Self::FileArchive => '\u{e30d}',
            Self::FileSpreadsheet => '\u{e326}',
            Self::FileAudio => '\u{e31a}',
            Self::FileVideo => '\u{e32b}',
            Self::Binary => '\u{e1f2}',
            Self::FileTerminal => '\u{e328}',
            Self::FileKey => '\u{e31d}',
            Self::FileLock => '\u{e31e}',
            Self::FileDiff => '\u{e317}',
            Self::FileBadge => '\u{e30f}',
            Self::FileImage => '\u{e31c}',
        }
    }
}

impl From<LucideIcon> for char {
    #[inline]
    fn from(icon: LucideIcon) -> Self {
        icon.as_char()
    }
}

/// Resuelve el icono Lucide y el índice de color ANSI (0..16) según el nombre y la extensión del archivo.
pub(crate) fn file_icon_and_slot(path: &std::path::Path) -> (LucideIcon, usize) {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // 1. Coincidencias por nombre exacto de archivo o prefijos especiales
    match file_name.as_str() {
        "dockerfile" | "containerfile" => return (LucideIcon::FileCode, 4),
        "makefile" | "gnumakefile" | "justfile" | "rakefile" | "cmakelists.txt" => {
            return (LucideIcon::FileCog, 3);
        }
        "cargo.toml" => return (LucideIcon::FileJson, 1),
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "composer.lock" => {
            return (LucideIcon::FileLock, 8);
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" => return (LucideIcon::FileCog, 1),
        name if name.starts_with(".env") => return (LucideIcon::FileCog, 3),
        "license" | "licence" | "copying" => return (LucideIcon::FileBadge, 3),
        "readme" | "readme.md" | "readme.txt" => return (LucideIcon::FileText, 6),
        _ => {}
    }

    // 2. Extraer extensión
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        // Rust
        "rs" => (LucideIcon::FileCode, 1),

        // Python
        "py" | "pyw" | "pyi" | "ipynb" => (LucideIcon::FileCode, 4),

        // JavaScript / TypeScript / Web frameworks
        "js" | "mjs" | "cjs" | "jsx" => (LucideIcon::FileCode, 3),
        "ts" | "mts" | "cts" | "tsx" => (LucideIcon::FileCode, 4),
        "html" | "htm" | "xhtml" | "vue" | "svelte" | "astro" => (LucideIcon::FileCode, 1),
        "css" | "scss" | "sass" | "less" | "styl" => (LucideIcon::FileCode, 4),
        "php" | "phtml" | "php3" | "php4" | "php5" | "php7" | "phps" => (LucideIcon::FileCode, 5),

        // Sistemas y nativo: C, C++, C#, F#, Go, Swift, D, Zig
        "c" | "h" | "cpp" | "hpp" | "cc" | "hh" | "cxx" | "hxx" | "inl" => {
            (LucideIcon::FileCode, 4)
        }
        "cs" | "csx" | "fs" | "fsx" | "fsi" | "vb" => (LucideIcon::FileCode, 2),
        "go" => (LucideIcon::FileCode, 6),
        "swift" => (LucideIcon::FileCode, 1),
        "zig" => (LucideIcon::FileCode, 3),
        "d" => (LucideIcon::FileCode, 1),

        // JVM y lenguajes funcionales
        "java" | "kt" | "kts" | "scala" | "groovy" | "gradle" => (LucideIcon::FileCode, 1),
        "rb" | "erb" | "rake" | "gemspec" => (LucideIcon::FileCode, 1),
        "dart" => (LucideIcon::FileCode, 6),
        "lua" => (LucideIcon::FileCode, 4),
        "r" | "rmd" => (LucideIcon::FileCode, 4),
        "jl" => (LucideIcon::FileCode, 5),
        "ex" | "exs" | "erl" | "hrl" => (LucideIcon::FileCode, 5),
        "hs" | "lhs" => (LucideIcon::FileCode, 5),
        "clj" | "cljs" | "cljc" | "edn" => (LucideIcon::FileCode, 2),
        "ml" | "mli" => (LucideIcon::FileCode, 3),
        "nim" => (LucideIcon::FileCode, 3),
        "lisp" | "lsp" | "cl" | "el" => (LucideIcon::FileCode, 5),

        // Shell y scripts
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "csh" | "bat" | "cmd" | "awk" | "sed" => {
            (LucideIcon::FileTerminal, 2)
        }
        "ps1" | "psm1" | "psd1" => (LucideIcon::FileTerminal, 4),

        // Bases de datos / SQL
        "sql" | "sqlite" | "sqlite3" | "db" | "db3" | "psql" | "mysql" | "pgsql" | "prisma" => {
            (LucideIcon::Database, 3)
        }

        // Datos estructurados / serialización
        "json" | "jsonc" | "json5" => (LucideIcon::FileJson, 3),
        "toml" => (LucideIcon::FileJson, 3),
        "yaml" | "yml" => (LucideIcon::FileJson, 3),
        "xml" | "xsl" | "xslt" | "svg" | "plist" => (LucideIcon::FileCode, 3),

        // Configuración
        "ini" | "conf" | "config" | "cfg" | "properties" | "env" | "prefs" => {
            (LucideIcon::FileCog, 7)
        }

        // Texto, documentación y libros
        "txt" | "text" => (LucideIcon::FileText, 7),
        "log" => (LucideIcon::FileText, 8),
        "md" | "markdown" | "mdown" | "mkdn" | "mdx" | "rst" | "adoc" | "asciidoc" | "tex"
        | "latex" => (LucideIcon::FileText, 6),
        "pdf" => (LucideIcon::FileText, 1),
        "doc" | "docx" | "odt" | "rtf" => (LucideIcon::FileText, 4),
        "ppt" | "pptx" | "odp" => (LucideIcon::FileText, 1),

        // Hojas de cálculo
        "csv" | "tsv" | "xls" | "xlsx" | "xlsm" | "ods" => (LucideIcon::FileSpreadsheet, 2),

        // Archivos comprimidos y empaquetados
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "tbz2" | "xz" | "txz" | "7z" | "rar" | "zst"
        | "iso" | "cab" => (LucideIcon::FileArchive, 3),

        // Imágenes
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "tiff" | "tif" | "avif"
        | "heic" | "psd" | "ai" | "raw" => (LucideIcon::FileImage, 5),

        // Audio
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "opus" | "mid" | "midi" => {
            (LucideIcon::FileAudio, 5)
        }

        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" | "3gp" => {
            (LucideIcon::FileVideo, 5)
        }

        // Ejecutables y binarios
        "exe" | "msi" | "com" => (LucideIcon::Binary, 2),
        "dll" | "sys" | "drv" | "bin" | "dat" | "hex" | "rom" | "elf" | "o" | "obj" | "so"
        | "dylib" | "a" | "lib" | "wasm" | "class" | "dex" => (LucideIcon::Binary, 8),

        // Seguridad, llaves y certificados
        "pem" | "key" | "pub" | "crt" | "cer" | "der" | "pfx" | "p12" | "asc" | "sig" | "gpg" => {
            (LucideIcon::FileKey, 3)
        }
        "lock" => (LucideIcon::FileLock, 8),

        // Diff y parches
        "diff" | "patch" => (LucideIcon::FileDiff, 2),

        // Fuentes
        "ttf" | "otf" | "woff" | "woff2" | "eot" => (LucideIcon::FileText, 7),

        // Por defecto
        _ => (LucideIcon::File, 8),
    }
}

/// Devuelve el icono representativo para una ruta de archivo.
pub(crate) fn file_icon(path: &std::path::Path) -> LucideIcon {
    file_icon_and_slot(path).0
}

/// Devuelve el icono y el color temático (paleta ANSI) representativos para una ruta de archivo.
pub(crate) fn file_icon_and_color(
    path: &std::path::Path,
    theme: &crate::theme::Theme,
) -> (LucideIcon, crate::theme::Color) {
    let (icon, slot) = file_icon_and_slot(path);
    (icon, theme.ansi[slot])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_lucide_glyphs_exist_in_font() {
        const EMBEDDED_LUCIDE: &[u8] = include_bytes!("../../assets/fonts/lucide.ttf");
        let font = fontdue::Font::from_bytes(EMBEDDED_LUCIDE, fontdue::FontSettings::default())
            .expect("lucide.ttf debe ser una fuente OpenType/TrueType válida");

        let all_icons = [
            LucideIcon::SquareTerminal,
            LucideIcon::Terminal,
            LucideIcon::Sparkles,
            LucideIcon::Server,
            LucideIcon::Plus,
            LucideIcon::X,
            LucideIcon::Minus,
            LucideIcon::Square,
            LucideIcon::Copy,
            LucideIcon::Settings,
            LucideIcon::FolderTree,
            LucideIcon::Folder,
            LucideIcon::FolderOpen,
            LucideIcon::File,
            LucideIcon::FileCode,
            LucideIcon::FileText,
            LucideIcon::FileJson,
            LucideIcon::FileCog,
            LucideIcon::ChevronRight,
            LucideIcon::ChevronDown,
            LucideIcon::ChevronLeft,
            LucideIcon::RefreshCw,
            LucideIcon::ArrowUp,
            LucideIcon::Search,
            LucideIcon::GitBranch,
            LucideIcon::Columns2,
            LucideIcon::Rows2,
            LucideIcon::PanelRight,
            LucideIcon::PanelLeft,
            LucideIcon::Dot,
            LucideIcon::Bot,
            LucideIcon::Palette,
            LucideIcon::Command,
            LucideIcon::Keyboard,
            LucideIcon::Info,
            LucideIcon::Sliders,
            LucideIcon::Check,
            LucideIcon::Maximize2,
            LucideIcon::Minimize2,
            LucideIcon::Lock,
            LucideIcon::Pencil,
            LucideIcon::Eye,
            LucideIcon::Code,
            LucideIcon::Scissors,
            LucideIcon::ClipboardPaste,
            LucideIcon::Undo2,
            LucideIcon::Redo2,
            LucideIcon::Save,
            LucideIcon::ExternalLink,
            LucideIcon::Trash2,
            LucideIcon::Link,
            LucideIcon::Heading,
            LucideIcon::List,
            LucideIcon::ListOrdered,
            LucideIcon::Quote,
            LucideIcon::CheckSquare,
            LucideIcon::ListChecks,
            LucideIcon::ScanText,
            LucideIcon::Image,
            LucideIcon::SeparatorHorizontal,
            LucideIcon::WrapText,
            LucideIcon::Database,
            LucideIcon::FileArchive,
            LucideIcon::FileSpreadsheet,
            LucideIcon::FileAudio,
            LucideIcon::FileVideo,
            LucideIcon::Binary,
            LucideIcon::FileTerminal,
            LucideIcon::FileKey,
            LucideIcon::FileLock,
            LucideIcon::FileDiff,
            LucideIcon::FileBadge,
            LucideIcon::FileImage,
        ];

        for icon in all_icons {
            let ch = icon.as_char();
            let idx = font.lookup_glyph_index(ch);
            assert!(
                idx != 0,
                "Glyph para {:?} ('\\u{{{:04x}}}') no existe en lucide.ttf",
                icon,
                ch as u32
            );
        }
    }

    #[test]
    fn test_file_icon_resolution() {
        use std::path::Path;

        // Ejemplos pedidos por el usuario
        assert_eq!(file_icon(Path::new("documento.txt")), LucideIcon::FileText);
        assert_eq!(file_icon(Path::new("query.sql")), LucideIcon::Database);
        assert_eq!(file_icon(Path::new("manual.pdf")), LucideIcon::FileText);
        assert_eq!(file_icon(Path::new("main.rs")), LucideIcon::FileCode);
        assert_eq!(file_icon(Path::new("script.py")), LucideIcon::FileCode);
        assert_eq!(file_icon(Path::new("index.php")), LucideIcon::FileCode);
        assert_eq!(file_icon(Path::new("app.html")), LucideIcon::FileCode);

        // Otros tipos comunes
        assert_eq!(
            file_icon(Path::new("datos.csv")),
            LucideIcon::FileSpreadsheet
        );
        assert_eq!(file_icon(Path::new("archivo.zip")), LucideIcon::FileArchive);
        assert_eq!(file_icon(Path::new("cancion.mp3")), LucideIcon::FileAudio);
        assert_eq!(file_icon(Path::new("video.mp4")), LucideIcon::FileVideo);
        assert_eq!(file_icon(Path::new("app.exe")), LucideIcon::Binary);
        assert_eq!(file_icon(Path::new("build.sh")), LucideIcon::FileTerminal);
        assert_eq!(file_icon(Path::new("run.ps1")), LucideIcon::FileTerminal);
        assert_eq!(file_icon(Path::new("server.key")), LucideIcon::FileKey);
        assert_eq!(file_icon(Path::new("foto.png")), LucideIcon::FileImage);
        assert_eq!(file_icon(Path::new("config.json")), LucideIcon::FileJson);
        assert_eq!(file_icon(Path::new("Cargo.toml")), LucideIcon::FileJson);
        assert_eq!(file_icon(Path::new("Dockerfile")), LucideIcon::FileCode);
        assert_eq!(file_icon(Path::new("Makefile")), LucideIcon::FileCog);
        assert_eq!(file_icon(Path::new("desconocido.xyz")), LucideIcon::File);
    }
}
