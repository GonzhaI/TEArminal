/// Catálogo de iconos Lucide vectoriales embebidos para Axon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LucideIcon {
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
}

impl LucideIcon {
    /// Devuelve el carácter Unicode codificado en la tipografía `lucide.ttf`
    pub fn as_char(&self) -> char {
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
        }
    }
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
}
