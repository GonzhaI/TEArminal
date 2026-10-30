//! Coincidencia de teclas por carácter lógico o por tecla física (clave en AltGr / WM_SYSKEYDOWN).

use super::*;

pub(crate) fn key_code_to_char(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Equal => Some('='),
        KeyCode::NumpadAdd => Some('+'),
        KeyCode::Comma => Some(','),
        _ => None,
    }
}

pub(crate) fn key_is_char(key: &Key, physical: PhysicalKey, target: char) -> bool {
    let target_lower = target.to_ascii_lowercase();

    // 1. Comprobar tecla física (PhysicalKey) por hardware KeyCode.
    // En Windows, cuando Alt está presionado (WM_SYSKEYDOWN), ToUnicodeEx no genera
    // caracteres y winit reporta logical_key como Key::Unidentified.
    // Por tanto, verificar physical_key garantiza que los atajos con Alt y Ctrl
    // funcionen de manera 100% confiable independientemente de la plataforma o del estado SYSKEY.
    if let PhysicalKey::Code(code) = physical {
        if let Some(c) = key_code_to_char(code) {
            if c == target_lower {
                return true;
            }
        }
    }

    // 2. Comprobar logical_key (para layouts no QWERTY donde el carácter semántico coincide)
    match key {
        Key::Character(s) => {
            if let Some(c) = s.chars().next() {
                if c.to_ascii_lowercase() == target_lower {
                    return true;
                }
                let target_lower = target.to_ascii_lowercase();
                if target_lower.is_ascii_lowercase() {
                    let code = (target_lower as u8) - b'a' + 1;
                    if c as u32 == code as u32 {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Tipo de evento de tecla para el Kitty Keyboard Protocol
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum KeyEventType {
    #[default]
    Press,
    Repeat,
    Release,
}

/// Resuelve el número de tecla según la especificación del Kitty Keyboard Protocol
pub(super) fn kitty_key_number(logical_key: &Key, physical_key: PhysicalKey) -> Option<u32> {
    match logical_key {
        Key::Named(NamedKey::Enter) => Some(13),
        Key::Named(NamedKey::Tab) => Some(9),
        Key::Named(NamedKey::Backspace) => Some(127),
        Key::Named(NamedKey::Escape) => Some(27),
        Key::Named(NamedKey::Delete) => Some(57359),
        Key::Named(NamedKey::Insert) => Some(57358),
        Key::Named(NamedKey::ArrowLeft) => Some(57360),
        Key::Named(NamedKey::ArrowRight) => Some(57361),
        Key::Named(NamedKey::ArrowUp) => Some(57362),
        Key::Named(NamedKey::ArrowDown) => Some(57363),
        Key::Named(NamedKey::PageUp) => Some(57364),
        Key::Named(NamedKey::PageDown) => Some(57365),
        Key::Named(NamedKey::Home) => Some(57366),
        Key::Named(NamedKey::End) => Some(57367),
        Key::Named(NamedKey::CapsLock) => Some(57368),
        Key::Named(NamedKey::ScrollLock) => Some(57369),
        Key::Named(NamedKey::NumLock) => Some(57370),
        Key::Named(NamedKey::PrintScreen) => Some(57371),
        Key::Named(NamedKey::Pause) => Some(57372),
        Key::Named(NamedKey::ContextMenu) => Some(57373),
        Key::Named(NamedKey::F1) => Some(57376),
        Key::Named(NamedKey::F2) => Some(57377),
        Key::Named(NamedKey::F3) => Some(57378),
        Key::Named(NamedKey::F4) => Some(57379),
        Key::Named(NamedKey::F5) => Some(57380),
        Key::Named(NamedKey::F6) => Some(57381),
        Key::Named(NamedKey::F7) => Some(57382),
        Key::Named(NamedKey::F8) => Some(57383),
        Key::Named(NamedKey::F9) => Some(57384),
        Key::Named(NamedKey::F10) => Some(57385),
        Key::Named(NamedKey::F11) => Some(57386),
        Key::Named(NamedKey::F12) => Some(57387),
        Key::Character(s) => s.chars().next().map(|c| c.to_ascii_lowercase() as u32),
        _ => {
            if let PhysicalKey::Code(code) = physical_key {
                match code {
                    KeyCode::Enter => Some(13),
                    KeyCode::NumpadEnter => Some(57414),
                    KeyCode::Tab => Some(9),
                    KeyCode::Backspace => Some(127),
                    KeyCode::Escape => Some(27),
                    KeyCode::Delete => Some(57359),
                    KeyCode::Insert => Some(57358),
                    KeyCode::ArrowLeft => Some(57360),
                    KeyCode::ArrowRight => Some(57361),
                    KeyCode::ArrowUp => Some(57362),
                    KeyCode::ArrowDown => Some(57363),
                    KeyCode::PageUp => Some(57364),
                    KeyCode::PageDown => Some(57365),
                    KeyCode::Home => Some(57366),
                    KeyCode::End => Some(57367),
                    KeyCode::F1 => Some(57376),
                    KeyCode::F2 => Some(57377),
                    KeyCode::F3 => Some(57378),
                    KeyCode::F4 => Some(57379),
                    KeyCode::F5 => Some(57380),
                    KeyCode::F6 => Some(57381),
                    KeyCode::F7 => Some(57382),
                    KeyCode::F8 => Some(57383),
                    KeyCode::F9 => Some(57384),
                    KeyCode::F10 => Some(57385),
                    KeyCode::F11 => Some(57386),
                    KeyCode::F12 => Some(57387),
                    _ => key_code_to_char(code).map(|c| c.to_ascii_lowercase() as u32),
                }
            } else {
                None
            }
        }
    }
}

/// Codifica una pulsación o liberación según las banderas activas del Kitty Keyboard Protocol (CSI u)
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_kitty_key(
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    win: bool,
    event_type: KeyEventType,
    flags: u8,
) -> Option<Vec<u8>> {
    if flags == 0 {
        return None;
    }

    // Los eventos de liberación solo se reportan si el flag 2 (report event types) está activo
    if event_type == KeyEventType::Release && (flags & 2) == 0 {
        return None;
    }

    let key_num = kitty_key_number(logical_key, physical_key)?;

    // Flechas, Home/End y Supr conservan su forma clásica (`CSI 1;mod D`, `CSI 3;mod ~`) en el
    // protocolo Kitty; los códigos 573xx de la tabla no son los que esperan los clientes.
    let legacy_nav = match key_num {
        57360 => Some(("1", 'D')),
        57361 => Some(("1", 'C')),
        57362 => Some(("1", 'A')),
        57363 => Some(("1", 'B')),
        57366 => Some(("1", 'H')),
        57367 => Some(("1", 'F')),
        57359 => Some(("3", '~')),
        _ => None,
    };

    let mut mod_mask = 0u8;
    if shift {
        mod_mask |= 1;
    }
    if alt {
        mod_mask |= 2;
    }
    if ctrl {
        mod_mask |= 4;
    }
    if win {
        mod_mask |= 8;
    }
    let mod_val = 1 + mod_mask;

    // Reglas de codificación de Kitty Keyboard Protocol (CSI u):
    // 1. Si flag 8 está activo (report all keys as escape codes): codificar todas las teclas.
    // 2. Si flag 2 está activo (report event types) y es repetición o liberación: codificar para informar del evento.
    // 3. Si hay modificadores activos (mod_val > 1): codificar como CSI key;mod u (Shift+Enter, Ctrl+C, Ctrl+Flechas, etc.).
    // 4. Si no hay modificadores (mod_val == 1):
    //    - Solo Escape (27) se codifica bajo flag 1 (disambiguate) para separarlo del prefijo de escape.
    //    - Enter, Tab, Backspace y las teclas de navegación (flechas, Home, End, PageUp, etc.) NO deben
    //      enviarse como CSI u cuando se pulsan sin modificadores a menos que flag 8 esté activo, para
    //      preservar las secuencias VT clásicas (\r, \t, \x7f, \x1b[A, \x1bOA) que esperan los agentes CLI y shells.
    if let Some((num, fin)) = legacy_nav {
        // Sin modificadores ni eventos extra se deja a la ruta VT clásica (\x1b[A, \x1bOA…)
        if mod_val == 1 && (flags & 2) == 0 {
            return None;
        }
        let ev = match event_type {
            KeyEventType::Press => 1,
            KeyEventType::Repeat => 2,
            KeyEventType::Release => 3,
        };
        // El tipo de evento solo se informa con el flag 2, igual que en CSI u
        let suffix = if (flags & 2) != 0 {
            format!(";{mod_val}:{ev}")
        } else {
            format!(";{mod_val}")
        };
        let seq = format!("\x1b[{num}{suffix}{fin}");
        return Some(seq.into_bytes());
    }

    let should_encode =
        (flags & 8) != 0 || (flags & 2) != 0 || mod_val > 1 || (key_num == 27 && (flags & 1) != 0);

    if !should_encode {
        return None;
    }

    let event_code = match event_type {
        KeyEventType::Press => 1,
        KeyEventType::Repeat => 2,
        KeyEventType::Release => 3,
    };

    let seq = if (flags & 2) != 0 {
        format!("\x1b[{key_num};{mod_val}:{event_code}u")
    } else if mod_val > 1 {
        format!("\x1b[{key_num};{mod_val}u")
    } else {
        format!("\x1b[{key_num}u")
    };

    Some(seq.into_bytes())
}
