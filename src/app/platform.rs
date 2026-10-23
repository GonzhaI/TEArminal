//! Integración nativa con Windows: icono, tema DWM, opacidad y estado físico del teclado.

use super::*;

/// Evita la ventana de consola parpadeante al lanzar un proceso auxiliar (`CREATE_NO_WINDOW`) y le baja la
/// prioridad (`BELOW_NORMAL_PRIORITY_CLASS`): todos corren en segundo plano y no deben quitar CPU a la interfaz
pub(crate) fn hide_console(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000 | 0x0000_4000)
}

/// Abre un archivo con su programa asociado (`ShellExecuteW`): sin `cmd /C start`, donde `&`, `^` o `%` de la
/// ruta se interpretarían como sintaxis del shell. `false` si Windows no pudo abrirlo.
pub(super) fn shell_open(path: &std::path::Path) -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &std::ffi::OsStr| -> Vec<u16> {
        std::os::windows::ffi::OsStrExt::encode_wide(s)
            .chain(Some(0))
            .collect()
    };
    let (verb, file) = (wide("open".as_ref()), wide(path.as_os_str()));
    // SAFETY: cadenas UTF-16 terminadas en nulo que viven durante la llamada; el resto de punteros son nulos
    let result = unsafe {
        ShellExecuteW(
            0,
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    result > 32 // por contrato de ShellExecute, un valor ≤ 32 es un código de error
}

/// Abre una URL web externa con el navegador predeterminado del sistema (`ShellExecuteW`).
/// Solo permite esquemas http://, https:// y mailto: por seguridad.
pub(crate) fn shell_open_url(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("mailto:") {
        return false;
    }
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect()
    };
    let (verb, file) = (wide("open"), wide(url));
    let result = unsafe {
        ShellExecuteW(
            0,
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    result > 32
}

pub(super) fn create_native_icon() -> Icon {
    const ICON_BYTES: &[u8] = include_bytes!("../../assets/icon-256.png");
    if let Ok(img) = image::load_from_memory(ICON_BYTES) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        if let Ok(icon) = Icon::from_rgba(rgba.into_raw(), w, h) {
            return icon;
        }
    }

    let size = 32;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let in_arrow_top =
                (9..=16).contains(&x) && (y as i32 - x as i32).abs() <= 1 && (9..=16).contains(&y);
            let in_arrow_bot = (9..=16).contains(&x)
                && (y as i32 + x as i32 - 32).abs() <= 1
                && (16..=23).contains(&y);
            let in_cursor = (18..=24).contains(&x) && (y == 21 || y == 22);

            if in_arrow_top || in_arrow_bot || in_cursor {
                rgba[idx] = 0x51; // Color de respaldo del icono (#51e27a)
                rgba[idx + 1] = 0xe2;
                rgba[idx + 2] = 0x7a;
                rgba[idx + 3] = 255;
            } else {
                // Totalmente transparente (sin fondo oscuro)
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32)
        .expect("invariante: el buffer rgba tiene tamaño exacto size * size * 4")
}

#[cfg(target_os = "windows")]
pub(super) fn update_windows_theme(window: &Window, theme: &Theme) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                let is_light = theme.is_light();
                // 1. Dark Mode inmersivo (0 = Light, 1 = Dark)
                let dark_mode: i32 = if is_light { 0 } else { 1 };
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    20, // DWMWA_USE_IMMERSIVE_DARK_MODE
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                let to_bgr = |c: crate::theme::Color| -> u32 {
                    let r = c.r as u32;
                    let g = c.g as u32;
                    let b = c.b as u32;
                    (b << 16) | (g << 8) | r
                };

                // 2. Color de fondo del titlebar acorde al sidebar/pestañas
                let caption_color: u32 = to_bgr(theme.sidebar_bg());
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    35, // DWMWA_CAPTION_COLOR
                    &caption_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 3. Color del texto del titlebar
                let text_color: u32 = to_bgr(theme.foreground);
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    36, // DWMWA_TEXT_COLOR
                    &text_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 4. Color del borde sutil de la ventana
                let border_color: u32 = to_bgr(theme.border_color());
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    34, // DWMWA_BORDER_COLOR
                    &border_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 5. Esquinas redondeadas nativas de Windows 11
                let corner_pref: u32 = 2; // DWMWCP_ROUND
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    33, // DWMWA_WINDOW_CORNER_PREFERENCE
                    &corner_pref as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 6. Efecto de fondo Mica nativo de Windows 11
                let backdrop_type: u32 = 2; // 2 = DWMSBT_MAINWINDOW (Mica)
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    38, // DWMWA_SYSTEMBACKDROP_TYPE
                    &backdrop_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 7. Establecer el pincel de fondo de la clase Win32
                let brush =
                    windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(to_bgr(theme.background));
                if brush != 0 {
                    let prev = windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::GCLP_HBRBACKGROUND,
                        brush as isize,
                    );
                    if prev != 0 {
                        windows_sys::Win32::Graphics::Gdi::DeleteObject(prev as isize);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn update_windows_theme(_window: &Window, _theme: &Theme) {}

#[cfg(target_os = "windows")]
pub(super) fn apply_window_opacity_native(window: &Window, opacity: f32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                let alpha = (opacity.clamp(0.20, 1.0) * 255.0).round() as u8;
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                if alpha < 255 {
                    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | (WS_EX_LAYERED as isize));
                    SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
                } else {
                    SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
                    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style & !(WS_EX_LAYERED as isize));
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn apply_window_opacity_native(_window: &Window, _opacity: f32) {}

#[cfg(target_os = "windows")]
pub(super) fn get_physical_modifiers() -> (bool, bool, bool, bool, bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RWIN, VK_SHIFT,
    };

    unsafe {
        let is_down = |vk: u16| -> bool { (GetAsyncKeyState(vk as i32) as i16) < 0 };

        let shift = is_down(VK_SHIFT);
        let rmenu = is_down(VK_RMENU); // AltGr (Right Alt)
        let lmenu = is_down(VK_LMENU); // Left Alt
        let lctrl = is_down(VK_LCONTROL);
        let rctrl = is_down(VK_RCONTROL);
        let lwin = is_down(VK_LWIN);
        let rwin = is_down(VK_RWIN);

        // En Windows, presionar AltGr (RMENU) inyecta automáticamente un LCONTROL sintético.
        // Por tanto, Ctrl solo es genuino si RCONTROL está presionado, o si LCONTROL
        // está presionado SIN que RMENU (AltGr) esté activo.
        let ctrl = rctrl || (lctrl && !rmenu);
        let alt = lmenu;
        let altgr = rmenu;
        let win = lwin || rwin;

        (ctrl, shift, alt, altgr, win)
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn get_physical_modifiers() -> (bool, bool, bool, bool, bool) {
    (false, false, false, false, false)
}

#[cfg(target_os = "windows")]
pub(crate) fn get_screen_cursor_pos() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) } != 0 {
        Some((pt.x, pt.y))
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_screen_cursor_pos() -> Option<(i32, i32)> {
    None
}

/// Texto del diálogo cuando Direct3D 11 no arranca ni siquiera con WARP.
#[cold]
pub(crate) fn gpu_error_message(err: &dyn std::fmt::Display) -> String {
    format!(
        "Axon no pudo inicializar Direct3D 11 (ni siquiera el respaldo por software WARP).\n\n\
         Error: {err}\n\n\
         Axon necesita Windows 10 versión 1809 (build 17763) o superior. Prueba a arrancarlo con \
         la variable de entorno AXON_RENDERER=warp para forzar el render por software y \
         actualiza los controladores de la tarjeta gráfica."
    )
}

/// Error irrecuperable al crear la superficie de GPU: informar (registro + diálogo nativo) y salir,
/// en vez de dejar una ventana en blanco. Mismo patrón que `pty::report_spawn_error`.
#[cold]
pub(crate) fn fatal_gpu_error(err: &dyn std::fmt::Display) -> ! {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    crate::logger::log(&format!("Error al inicializar Direct3D 11: {err}"));
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    let text = wide(&gpu_error_message(err));
    let title = wide("Axon");
    // SAFETY: cadenas UTF-16 terminadas en nulo que viven durante la llamada
    unsafe { MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
    std::process::exit(1)
}
