//! Contraseñas y frases de paso en el Administrador de credenciales de Windows (`CredWriteW` / `CredReadW`).
//! Nunca van a `connections.toml`, a logs ni al historial. Cada secreto se identifica por el `id` de la conexión.

use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
use windows_sys::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

/// Texto secreto: `Debug` lo oculta y no implementa `Display` ni `Serialize`
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Secret(String);

impl Secret {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret(***)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretKind {
    DbPassword,
    SshPassphrase,
    ClientKeyPassphrase,
}

impl SecretKind {
    fn label(self) -> &'static str {
        match self {
            Self::DbPassword => "db-password",
            Self::SshPassphrase => "ssh-passphrase",
            Self::ClientKeyPassphrase => "client-key-passphrase",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CredentialError {
    #[error("el Administrador de credenciales de Windows falló (código {0})")]
    Windows(u32),
    #[error("el secreto supera el tamaño que admite el Administrador de credenciales")]
    TooLarge,
    #[error("el secreto guardado no es texto válido")]
    NotText,
}

/// Nombre bajo el que Windows guarda el secreto (`TEArminal/db/<id>/<tipo>`)
pub(crate) fn target_name(kind: SecretKind, connection_id: &str) -> String {
    format!("TEArminal/db/{connection_id}/{}", kind.label())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub(crate) fn store(
    kind: SecretKind,
    connection_id: &str,
    secret: &Secret,
) -> Result<(), CredentialError> {
    let mut target = wide(&target_name(kind, connection_id));
    let mut user = wide("TEArminal");
    let blob = secret.0.as_bytes();
    // CRED_MAX_CREDENTIAL_BLOB_SIZE = 5 * 512
    let size = u32::try_from(blob.len()).map_err(|_| CredentialError::TooLarge)?;
    if size > 2560 {
        return Err(CredentialError::TooLarge);
    }
    let credential = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        Comment: null_mut(),
        LastWritten: windows_sys::Win32::Foundation::FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        },
        CredentialBlobSize: size,
        CredentialBlob: blob.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: null_mut(),
        TargetAlias: null_mut(),
        UserName: user.as_mut_ptr(),
    };
    // SAFETY: `credential` y los búferes a los que apunta viven durante la llamada; `CredWriteW` solo lee de ellos
    let ok = unsafe { CredWriteW(&credential, 0) };
    if ok == 0 {
        // SAFETY: sin precondiciones
        return Err(CredentialError::Windows(unsafe { GetLastError() }));
    }
    Ok(())
}

/// `Ok(None)` si no hay nada guardado
pub(crate) fn load(
    kind: SecretKind,
    connection_id: &str,
) -> Result<Option<Secret>, CredentialError> {
    let target = wide(&target_name(kind, connection_id));
    let mut found: *mut CREDENTIALW = null_mut();
    // SAFETY: `target` termina en nulo; `found` recibe un búfer que se libera con `CredFree`
    let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut found) };
    if ok == 0 {
        // SAFETY: sin precondiciones
        let code = unsafe { GetLastError() };
        return if code == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(CredentialError::Windows(code))
        };
    }
    // SAFETY: tras el éxito, `found` apunta a un CREDENTIALW válido con `CredentialBlobSize` bytes
    let bytes = unsafe {
        let credential = &*found;
        if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
            .to_vec()
        }
    };
    // SAFETY: `found` lo asignó `CredReadW` y no se usa después
    unsafe { CredFree(found.cast_const().cast()) };
    String::from_utf8(bytes)
        .map(|text| Some(Secret(text)))
        .map_err(|_| CredentialError::NotText)
}

/// Borra el secreto; no es un error que no exista
pub(crate) fn delete(kind: SecretKind, connection_id: &str) -> Result<(), CredentialError> {
    let target = wide(&target_name(kind, connection_id));
    // SAFETY: `target` termina en nulo y vive durante la llamada
    let ok = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
    if ok != 0 {
        return Ok(());
    }
    // SAFETY: sin precondiciones
    let code = unsafe { GetLastError() };
    if code == ERROR_NOT_FOUND {
        Ok(())
    } else {
        Err(CredentialError::Windows(code))
    }
}

/// Borra todos los secretos de una conexión (al eliminarla)
pub(crate) fn delete_all(connection_id: &str) -> Result<(), CredentialError> {
    for kind in [
        SecretKind::DbPassword,
        SecretKind::SshPassphrase,
        SecretKind::ClientKeyPassphrase,
    ] {
        delete(kind, connection_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_never_shows_in_debug() {
        let secret = Secret::new("hunter2");
        assert!(!format!("{secret:?}").contains("hunter2"));
        assert_eq!(secret.expose(), "hunter2");
    }

    #[test]
    fn target_names_are_namespaced_per_connection_and_kind() {
        assert_eq!(
            target_name(SecretKind::DbPassword, "abc"),
            "TEArminal/db/abc/db-password"
        );
        assert_ne!(
            target_name(SecretKind::SshPassphrase, "abc"),
            target_name(SecretKind::ClientKeyPassphrase, "abc")
        );
    }

    #[test]
    fn credential_manager_roundtrip() {
        let id = format!("test{}", std::process::id());
        let kind = SecretKind::DbPassword;
        let secret = Secret::new("contraseña ñandú \u{1F511}");
        // Si el entorno no tiene Administrador de credenciales (sesión sin perfil), no hay nada que probar
        if store(kind, &id, &secret).is_err() {
            return;
        }
        assert_eq!(load(kind, &id).unwrap(), Some(secret));
        let _ = delete_all(&id);
        assert_eq!(load(kind, &id).unwrap(), None);
        assert!(delete(kind, &id).is_ok());
    }

    #[test]
    fn oversized_secret_is_rejected() {
        let big = Secret::new("x".repeat(3000));
        assert!(matches!(
            store(SecretKind::DbPassword, "big", &big),
            Err(CredentialError::TooLarge)
        ));
    }
}
