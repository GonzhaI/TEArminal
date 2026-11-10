//! Permisos de Windows de las claves privadas (`.pem`, `.key`, llaves SSH): avisa si otros usuarios pueden leerlas y
//! ofrece «Corregir permisos». Solo se leen los permisos del archivo, nunca su contenido.

use std::path::Path;
use std::process::Command;
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, PSID};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    AclSizeInformation, GetAce, GetAclInformation, GetTokenInformation, TokenUser, ACL,
    ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyAccess {
    /// Solo el propietario, el sistema y los administradores pueden leerla
    Private,
    /// Pueden leerla además estas cuentas (nombre conocido o SID)
    SharedWith(Vec<String>),
    /// No se pudieron leer los permisos (código de Windows)
    Unknown(u32),
}

/// Lectura concedida por una ACE: `FILE_READ_DATA`, `GENERIC_READ` o `GENERIC_ALL`
const READ_MASK: u32 = 0x0000_0001 | 0x8000_0000 | 0x1000_0000;

/// Cuentas que pueden leer el archivo aparte de las esperadas (propietario, usuario actual, SYSTEM,
/// Administradores, CREATOR OWNER y OWNER RIGHTS). Función pura: se prueba sin tocar el disco.
pub(crate) fn unexpected_readers(
    owner: &str,
    current_user: Option<&str>,
    allowed_aces: &[(String, u32)],
) -> Vec<String> {
    let expected = |sid: &str| {
        sid == owner
            || Some(sid) == current_user
            || matches!(sid, "S-1-5-18" | "S-1-5-32-544" | "S-1-3-0" | "S-1-3-4")
    };
    let mut others: Vec<String> = Vec::new();
    for (sid, mask) in allowed_aces {
        if mask & READ_MASK != 0 && !expected(sid) {
            let name = friendly_sid(sid);
            if !others.contains(&name) {
                others.push(name);
            }
        }
    }
    others
}

fn friendly_sid(sid: &str) -> String {
    match sid {
        "S-1-1-0" => "Todos".to_string(),
        "S-1-5-32-545" => "Usuarios".to_string(),
        "S-1-5-11" => "Usuarios autenticados".to_string(),
        "S-1-5-32-546" => "Invitados".to_string(),
        other => other.to_string(),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn sid_to_string(sid: PSID) -> Option<String> {
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `sid` es un SID válido que devuelve Windows; la cadena resultante se libera con `LocalFree`
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 || text.is_null() {
        return None;
    }
    // SAFETY: `text` termina en nulo y sigue vivo hasta el `LocalFree`
    let result = unsafe {
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    };
    // SAFETY: `text` lo asignó `ConvertSidToStringSidW`
    unsafe { LocalFree(text.cast()) };
    Some(result)
}

/// SID (texto) del usuario que ejecuta TEArminal
fn current_user_sid() -> Option<String> {
    let mut token = 0;
    // SAFETY: pseudo-manejador del proceso actual; `token` recibe un manejador que se cierra abajo
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let mut buffer = vec![0u8; 256];
    let mut needed = 0u32;
    // SAFETY: `buffer` tiene `len` bytes; si no alcanza, `needed` dice cuántos hacen falta
    let mut ok = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            256,
            &mut needed,
        )
    };
    if ok == 0 && needed as usize > buffer.len() {
        buffer = vec![0u8; needed as usize];
        // SAFETY: igual que arriba con el tamaño exacto
        ok = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
    }
    // SAFETY: manejador obtenido arriba
    unsafe { windows_sys::Win32::Foundation::CloseHandle(token) };
    if ok == 0 {
        return None;
    }
    // SAFETY: tras el éxito, el búfer empieza con un `TOKEN_USER` y el SID vive dentro del mismo búfer
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    sid_to_string(sid)
}

/// Propietario y ACE de «acceso permitido» del DACL del archivo
fn read_dacl(path: &Path) -> Result<(String, Vec<(String, u32)>), u32> {
    let name = wide(&path.to_string_lossy());
    let mut owner: PSID = std::ptr::null_mut();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: `name` termina en nulo; los punteros de salida son válidos y el descriptor se libera abajo
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(status);
    }
    let result = collect_aces(owner, dacl);
    // SAFETY: `descriptor` lo asignó `GetNamedSecurityInfoW`; `owner` y `dacl` apuntan dentro y ya no se usan
    unsafe { LocalFree(descriptor) };
    result
}

fn collect_aces(owner: PSID, dacl: *mut ACL) -> Result<(String, Vec<(String, u32)>), u32> {
    let owner_text = sid_to_string(owner).ok_or(1u32)?;
    // Sin DACL (puntero nulo) todos tienen acceso total: se refleja como «Todos»
    if dacl.is_null() {
        return Ok((owner_text, vec![("S-1-1-0".to_string(), u32::MAX)]));
    }
    let mut info = ACL_SIZE_INFORMATION {
        AceCount: 0,
        AclBytesInUse: 0,
        AclBytesFree: 0,
    };
    // SAFETY: `dacl` es un ACL válido y `info` tiene el tamaño que declara
    let ok = unsafe {
        GetAclInformation(
            dacl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    };
    if ok == 0 {
        return Err(1);
    }
    let mut allowed = Vec::new();
    for index in 0..info.AceCount {
        let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: `index` < AceCount; `ace` apunta dentro del ACL
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 || ace.is_null() {
            continue;
        }
        // SAFETY: toda ACE empieza con un ACE_HEADER; las de tipo 0 (ACCESS_ALLOWED_ACE) siguen con máscara y SID
        let (ace_type, mask, sid) = unsafe {
            let header = *ace.cast::<windows_sys::Win32::Security::ACE_HEADER>();
            let allowed_ace = ace.cast::<windows_sys::Win32::Security::ACCESS_ALLOWED_ACE>();
            (
                header.AceType,
                (*allowed_ace).Mask,
                std::ptr::addr_of_mut!((*allowed_ace).SidStart).cast(),
            )
        };
        if ace_type != 0 {
            continue;
        }
        if let Some(text) = sid_to_string(sid) {
            allowed.push((text, mask));
        }
    }
    Ok((owner_text, allowed))
}

/// ¿Quién más puede leer esta clave? Solo mira permisos, nunca abre el archivo.
pub(crate) fn key_access(path: &Path) -> KeyAccess {
    match read_dacl(path) {
        Err(code) => KeyAccess::Unknown(code),
        Ok((owner, aces)) => {
            let current = current_user_sid();
            let others = unexpected_readers(&owner, current.as_deref(), &aces);
            if others.is_empty() {
                KeyAccess::Private
            } else {
                KeyAccess::SharedWith(others)
            }
        }
    }
}

/// «Corregir permisos»: quita la herencia y deja solo lectura para el usuario actual (`icacls`, sin ventana)
pub(crate) fn restrict_to_current_user(path: &Path) -> std::io::Result<()> {
    let sid = current_user_sid()
        .ok_or_else(|| std::io::Error::other("no se pudo identificar al usuario actual"))?;
    // Primero `/reset` (descarta ACE explícitas como «Todos»), luego sin herencia y solo lectura para el usuario
    run_icacls(&[path.as_os_str(), "/reset".as_ref()])?;
    let grant = format!("*{sid}:(R)");
    run_icacls(&[
        path.as_os_str(),
        "/inheritance:r".as_ref(),
        "/grant:r".as_ref(),
        grant.as_ref(),
    ])
}

fn run_icacls(args: &[&std::ffi::OsStr]) -> std::io::Result<()> {
    let mut command = Command::new("icacls.exe");
    command.args(args);
    crate::app::platform::hide_console(&mut command);
    let status = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "icacls no pudo cambiar los permisos",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "S-1-5-21-1-2-3-1001";

    fn aces(list: &[(&str, u32)]) -> Vec<(String, u32)> {
        list.iter().map(|(s, m)| (s.to_string(), *m)).collect()
    }

    #[test]
    fn owner_system_and_admins_are_expected() {
        let list = aces(&[
            (OWNER, 0x1f01ff),
            ("S-1-5-18", 0x1f01ff),
            ("S-1-5-32-544", 0x1f01ff),
        ]);
        assert!(unexpected_readers(OWNER, None, &list).is_empty());
    }

    #[test]
    fn everyone_and_users_reading_are_flagged_by_name() {
        let list = aces(&[
            (OWNER, 0x1f01ff),
            ("S-1-1-0", 0x120089),
            ("S-1-5-32-545", 0x1),
        ]);
        assert_eq!(
            unexpected_readers(OWNER, None, &list),
            ["Todos", "Usuarios"]
        );
    }

    #[test]
    fn aces_without_read_rights_are_ignored() {
        // Solo atributos (0x80) o escritura de atributos: no leen el contenido
        let list = aces(&[("S-1-5-11", 0x80), ("S-1-1-0", 0x100)]);
        assert!(unexpected_readers(OWNER, None, &list).is_empty());
    }

    #[test]
    fn current_user_is_expected_even_if_not_the_owner() {
        let list = aces(&[("S-1-5-21-9-9-9-500", 0x1)]);
        assert!(unexpected_readers("S-1-5-32-544", Some("S-1-5-21-9-9-9-500"), &list).is_empty());
        assert_eq!(unexpected_readers("S-1-5-32-544", None, &list).len(), 1);
    }

    #[test]
    fn unknown_sids_are_reported_once_as_text() {
        let list = aces(&[
            ("S-1-5-21-7-7-7-1234", 0x1),
            ("S-1-5-21-7-7-7-1234", 0x8000_0000),
        ]);
        assert_eq!(
            unexpected_readers(OWNER, None, &list),
            ["S-1-5-21-7-7-7-1234"]
        );
    }

    fn temp_key(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tearminal-perm-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("client.key.pem");
        // Contenido de relleno: estas pruebas solo miran permisos
        std::fs::write(&path, b"no es una clave").unwrap();
        path
    }

    #[test]
    fn real_file_can_be_locked_down_and_a_shared_one_is_detected() {
        let path = temp_key("acl");
        let grant = Command::new("icacls.exe")
            .arg(&path)
            .args(["/grant", "*S-1-1-0:(R)"])
            .output();
        let Ok(grant) = grant else { return };
        if !grant.status.success() {
            return; // sin icacls o sin permiso para cambiar la ACL: no hay nada que comprobar
        }
        assert!(
            matches!(key_access(&path), KeyAccess::SharedWith(names) if names.contains(&"Todos".to_string()))
        );
        restrict_to_current_user(&path).unwrap();
        assert_eq!(key_access(&path), KeyAccess::Private);
    }

    #[test]
    fn missing_file_reports_unknown() {
        let path = std::env::temp_dir().join("tearminal-perm-no-existe.pem");
        assert!(matches!(key_access(&path), KeyAccess::Unknown(_)));
    }
}
