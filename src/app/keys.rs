//! Coincidencia de teclas por carácter lógico o por tecla física (clave en AltGr / WM_SYSKEYDOWN).

use super::*;

pub fn key_code_to_char(code: KeyCode) -> Option<char> {
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

pub fn key_is_char(key: &Key, physical: PhysicalKey, target: char) -> bool {
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
