//! TLS con archivos `.pem` (rustls + ring). Modos `disable`, `require`, `verify-ca` y `verify-full` (por defecto).
//!
//! De los `.pem` solo se conoce la **ruta**: el contenido se lee en memoria cuando hace falta (conectar o mostrar
//! vencimiento y huella), nunca se copia a otro archivo, ni se muestra ni se registra. Los errores nombran el rol del
//! archivo («clave de cliente») y el tipo de fallo, jamás un fragmento del contenido.

use super::credentials::Secret;
use super::perm::{self, KeyAccess};
use super::sanitize;
use super::store::{SslMode, TlsSettings};
use base64::Engine as _;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TlsError {
    #[error("{role}: no se pudo leer el archivo ({kind})")]
    Unreadable { role: &'static str, kind: String },
    #[error("{role}: el archivo no contiene un certificado PEM válido")]
    NoCertificate { role: &'static str },
    #[error("{role}: el archivo no contiene una clave privada PEM válida")]
    NoPrivateKey { role: &'static str },
    #[error(
        "la clave privada de cliente está protegida con frase de paso y no hay ninguna guardada"
    )]
    PassphraseRequired,
    #[error("la frase de paso de la clave privada de cliente no es correcta (o el cifrado no es compatible)")]
    WrongPassphrase,
    #[error("clave cifrada en un formato antiguo (Proc-Type): conviértela a PKCS#8 con «openssl pkcs8 -topk8»")]
    LegacyEncryptedKey,
    #[error("certificado de cliente sin clave privada (o al revés)")]
    IncompleteClientIdentity,
    #[error("configuración TLS no válida: {0}")]
    Config(String),
}

fn role_label(role: FileRole) -> &'static str {
    match role {
        FileRole::Ca => "certificado de la CA",
        FileRole::ClientCertificate => "certificado de cliente",
        FileRole::ClientKey => "clave privada de cliente",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileRole {
    Ca,
    ClientCertificate,
    ClientKey,
}

fn read_limited(path: &Path, role: FileRole) -> Result<Vec<u8>, TlsError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(1024 * 1024).read_to_end(&mut bytes))
        .map_err(|e| TlsError::Unreadable {
            role: role_label(role),
            kind: e.kind().to_string(),
        })?;
    Ok(bytes)
}

fn load_certificates(
    path: &Path,
    role: FileRole,
) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let bytes = read_limited(path, role)?;
    let certs: Vec<_> = CertificateDer::pem_slice_iter(&bytes)
        .filter_map(Result::ok)
        .collect();
    if certs.is_empty() {
        return Err(TlsError::NoCertificate {
            role: role_label(role),
        });
    }
    Ok(certs)
}

// -------------------------------------------------------------------------------------------------
// Inspección de archivos (pantalla «Resultado de la prueba» y avisos de vencimiento)
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CertInfo {
    pub(crate) not_before_unix: i64,
    /// El vencimiento más próximo de todos los certificados del archivo
    pub(crate) not_after_unix: i64,
    /// SHA-256 del primer certificado, en hexadecimal con `:` (`4F:A1:9C…`)
    pub(crate) fingerprint_sha256: String,
    pub(crate) certificates: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpiryState {
    Valid { days_left: i64 },
    ExpiresSoon { days_left: i64 },
    Expired { days_ago: i64 },
    NotYetValid,
}

const SECONDS_PER_DAY: i64 = 86_400;

/// `warn_days`: aviso con esta antelación al vencimiento (30 por defecto)
pub(crate) fn expiry_state(info: &CertInfo, now_unix: i64, warn_days: u32) -> ExpiryState {
    if now_unix < info.not_before_unix {
        return ExpiryState::NotYetValid;
    }
    if now_unix >= info.not_after_unix {
        return ExpiryState::Expired {
            days_ago: (now_unix - info.not_after_unix) / SECONDS_PER_DAY,
        };
    }
    let days_left = (info.not_after_unix - now_unix) / SECONDS_PER_DAY;
    if days_left <= i64::from(warn_days) {
        ExpiryState::ExpiresSoon { days_left }
    } else {
        ExpiryState::Valid { days_left }
    }
}

fn hex_fingerprint(der: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, der);
    let mut out = String::with_capacity(digest.as_ref().len() * 3);
    for (i, byte) in digest.as_ref().iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(&format!("{byte:02X}"));
    }
    out
}

pub(crate) fn certificate_info(path: &Path, role: FileRole) -> Result<CertInfo, TlsError> {
    let certs = load_certificates(path, role)?;
    let mut not_before = i64::MIN;
    let mut not_after = i64::MAX;
    for der in &certs {
        let (_, parsed) = x509_parser::parse_x509_certificate(der.as_ref()).map_err(|_| {
            TlsError::NoCertificate {
                role: role_label(role),
            }
        })?;
        let validity = parsed.validity();
        not_before = not_before.max(validity.not_before.timestamp());
        not_after = not_after.min(validity.not_after.timestamp());
    }
    Ok(CertInfo {
        not_before_unix: not_before,
        not_after_unix: not_after,
        fingerprint_sha256: certs
            .first()
            .map(|c| hex_fingerprint(c.as_ref()))
            .unwrap_or_default(),
        certificates: certs.len(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyFormat {
    Pkcs8,
    Pkcs1,
    Sec1,
    /// PKCS#8 cifrado (`ENCRYPTED PRIVATE KEY`): hace falta la frase de paso
    EncryptedPkcs8,
    /// Cifrado antiguo (`Proc-Type: 4,ENCRYPTED`): no compatible
    LegacyEncrypted,
}

impl KeyFormat {
    pub(crate) fn is_encrypted(self) -> bool {
        matches!(self, Self::EncryptedPkcs8 | Self::LegacyEncrypted)
    }
}

/// Qué tipo de clave es, mirando solo las cabeceras PEM (no se interpreta el contenido)
pub(crate) fn key_format(path: &Path) -> Result<KeyFormat, TlsError> {
    let bytes = read_limited(path, FileRole::ClientKey)?;
    let text = String::from_utf8_lossy(&bytes);
    let format = if text.contains("Proc-Type: 4,ENCRYPTED") {
        KeyFormat::LegacyEncrypted
    } else if text.contains("-----BEGIN ENCRYPTED PRIVATE KEY-----") {
        KeyFormat::EncryptedPkcs8
    } else if text.contains("-----BEGIN PRIVATE KEY-----") {
        KeyFormat::Pkcs8
    } else if text.contains("-----BEGIN RSA PRIVATE KEY-----") {
        KeyFormat::Pkcs1
    } else if text.contains("-----BEGIN EC PRIVATE KEY-----") {
        KeyFormat::Sec1
    } else {
        return Err(TlsError::NoPrivateKey {
            role: role_label(FileRole::ClientKey),
        });
    };
    Ok(format)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileStatus {
    Missing,
    Invalid(String),
    Certificate {
        info: CertInfo,
        state: ExpiryState,
    },
    PrivateKey {
        format: KeyFormat,
        access: KeyAccess,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileReport {
    pub(crate) role: FileRole,
    pub(crate) path: PathBuf,
    pub(crate) status: FileStatus,
}

/// Estado de cada `.pem` configurado, para la pestaña SSL / TLS y la prueba de conexión
pub(crate) fn inspect(settings: &TlsSettings, now_unix: i64, warn_days: u32) -> Vec<FileReport> {
    let entries = [
        (FileRole::Ca, settings.ca_path.as_ref()),
        (
            FileRole::ClientCertificate,
            settings.client_cert_path.as_ref(),
        ),
        (FileRole::ClientKey, settings.client_key_path.as_ref()),
    ];
    entries
        .into_iter()
        .filter_map(|(role, path)| Some((role, path?.clone())))
        .map(|(role, path)| {
            let status = inspect_file(role, &path, now_unix, warn_days);
            FileReport { role, path, status }
        })
        .collect()
}

fn inspect_file(role: FileRole, path: &Path, now_unix: i64, warn_days: u32) -> FileStatus {
    if !path.exists() {
        return FileStatus::Missing;
    }
    match role {
        FileRole::Ca | FileRole::ClientCertificate => match certificate_info(path, role) {
            Ok(info) => {
                let state = expiry_state(&info, now_unix, warn_days);
                FileStatus::Certificate { info, state }
            }
            Err(e) => FileStatus::Invalid(e.to_string()),
        },
        FileRole::ClientKey => match key_format(path) {
            Ok(format) => FileStatus::PrivateKey {
                format,
                access: perm::key_access(path),
            },
            Err(e) => FileStatus::Invalid(e.to_string()),
        },
    }
}

/// Aviso previo a conectar (la conexión sigue adelante; la interfaz los muestra en «Resultado de la prueba»)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Warning {
    /// Otros usuarios de Windows pueden leer la clave (acción «Corregir permisos»)
    InsecureKeyPermissions {
        path: PathBuf,
        readers: Vec<String>,
    },
    CertificateExpiresSoon {
        role: FileRole,
        days_left: i64,
    },
    CertificateExpired {
        role: FileRole,
        days_ago: i64,
    },
    CertificateNotYetValid {
        role: FileRole,
    },
    FileMissing {
        role: FileRole,
    },
}

/// Avisos derivados de la inspección. `warn_unsafe_pem` y `warn_days` vienen de la política de seguridad.
pub(crate) fn warnings(reports: &[FileReport], warn_unsafe_pem: bool) -> Vec<Warning> {
    let mut out = Vec::new();
    for report in reports {
        match &report.status {
            FileStatus::Missing => out.push(Warning::FileMissing { role: report.role }),
            FileStatus::Invalid(_) => {}
            FileStatus::Certificate { state, .. } => match *state {
                ExpiryState::Valid { .. } => {}
                ExpiryState::ExpiresSoon { days_left } => {
                    out.push(Warning::CertificateExpiresSoon {
                        role: report.role,
                        days_left,
                    })
                }
                ExpiryState::Expired { days_ago } => out.push(Warning::CertificateExpired {
                    role: report.role,
                    days_ago,
                }),
                ExpiryState::NotYetValid => {
                    out.push(Warning::CertificateNotYetValid { role: report.role });
                }
            },
            FileStatus::PrivateKey {
                access: KeyAccess::SharedWith(readers),
                ..
            } if warn_unsafe_pem => out.push(Warning::InsecureKeyPermissions {
                path: report.path.clone(),
                readers: readers.clone(),
            }),
            FileStatus::PrivateKey { .. } => {}
        }
    }
    out
}

// -------------------------------------------------------------------------------------------------
// Claves privadas de cliente
// -------------------------------------------------------------------------------------------------

/// PKCS#8 DER de una clave cifrada, descifrada en memoria con la frase de paso
fn decrypt_pkcs8(pem_text: &str, passphrase: &Secret) -> Result<Vec<u8>, TlsError> {
    const BEGIN: &str = "-----BEGIN ENCRYPTED PRIVATE KEY-----";
    const END: &str = "-----END ENCRYPTED PRIVATE KEY-----";
    let (Some(start), Some(end)) = (pem_text.find(BEGIN), pem_text.find(END)) else {
        return Err(TlsError::NoPrivateKey {
            role: role_label(FileRole::ClientKey),
        });
    };
    let body: String = pem_text[start + BEGIN.len()..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|_| TlsError::NoPrivateKey {
            role: role_label(FileRole::ClientKey),
        })?;
    let info = pkcs8::EncryptedPrivateKeyInfoRef::try_from(der.as_slice())
        .map_err(|_| TlsError::WrongPassphrase)?;
    let document = info
        .decrypt(passphrase.expose())
        .map_err(|_| TlsError::WrongPassphrase)?;
    Ok(document.as_bytes().to_vec())
}

/// Clave de cliente lista para rustls. Si está cifrada se descifra en memoria; nada se escribe a disco.
pub(crate) fn load_client_key(
    path: &Path,
    passphrase: Option<&Secret>,
) -> Result<PrivateKeyDer<'static>, TlsError> {
    match key_format(path)? {
        KeyFormat::LegacyEncrypted => Err(TlsError::LegacyEncryptedKey),
        KeyFormat::EncryptedPkcs8 => {
            let passphrase = passphrase.ok_or(TlsError::PassphraseRequired)?;
            let bytes = read_limited(path, FileRole::ClientKey)?;
            let der = decrypt_pkcs8(&String::from_utf8_lossy(&bytes), passphrase)?;
            Ok(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(der)))
        }
        _ => {
            let bytes = read_limited(path, FileRole::ClientKey)?;
            PrivateKeyDer::from_pem_slice(&bytes).map_err(|_| TlsError::NoPrivateKey {
                role: role_label(FileRole::ClientKey),
            })
        }
    }
}

/// La misma clave en PEM sin cifrar, solo en memoria (MySQL lee la identidad de un búfer)
pub(crate) fn client_key_pem(
    path: &Path,
    passphrase: Option<&Secret>,
) -> Result<Vec<u8>, TlsError> {
    let der = match load_client_key(path, passphrase)? {
        PrivateKeyDer::Pkcs8(key) => key.secret_pkcs8_der().to_vec(),
        // PKCS#1 y SEC1 ya vienen en PEM en el archivo y no necesitan reempaquetarse
        _ => return read_limited(path, FileRole::ClientKey),
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");
    for line in encoded.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(line));
        pem.push('\n');
    }
    pem.push_str("-----END PRIVATE KEY-----\n");
    Ok(pem.into_bytes())
}

// -------------------------------------------------------------------------------------------------
// Configuración de rustls para PostgreSQL
// -------------------------------------------------------------------------------------------------

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn root_store(settings: &TlsSettings) -> Result<RootCertStore, TlsError> {
    let mut roots = RootCertStore::empty();
    match &settings.ca_path {
        Some(path) => {
            for cert in load_certificates(path, FileRole::Ca)? {
                roots.add(cert).map_err(|_| TlsError::NoCertificate {
                    role: role_label(FileRole::Ca),
                })?;
            }
        }
        None => roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
    }
    Ok(roots)
}

/// `require`: cifra pero no valida la identidad del servidor (verifica solo las firmas del apretón de manos)
#[derive(Debug)]
struct AcceptAnyServer(Arc<CryptoProvider>);

impl ServerCertVerifier for AcceptAnyServer {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// `verify-ca`: valida la cadena de certificación pero no que el nombre coincida con el servidor
#[derive(Debug)]
struct VerifyChainOnly(Arc<WebPkiServerVerifier>);

fn is_name_mismatch(error: &Error) -> bool {
    matches!(
        error,
        Error::InvalidCertificate(
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. }
        )
    )
}

impl ServerCertVerifier for VerifyChainOnly {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match self
            .0
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        {
            Err(e) if is_name_mismatch(&e) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

/// Configuración de rustls para el modo y los `.pem` indicados. `None` con `disable` (sin TLS).
pub(crate) fn client_config(
    settings: &TlsSettings,
    key_passphrase: Option<&Secret>,
) -> Result<Option<ClientConfig>, TlsError> {
    if settings.mode == SslMode::Disable {
        return Ok(None);
    }
    let provider = provider();
    let verifier: Arc<dyn ServerCertVerifier> = match settings.mode {
        SslMode::Disable => return Ok(None),
        SslMode::Require => Arc::new(AcceptAnyServer(Arc::clone(&provider))),
        SslMode::VerifyCa | SslMode::VerifyFull => {
            let webpki = WebPkiServerVerifier::builder_with_provider(
                Arc::new(root_store(settings)?),
                Arc::clone(&provider),
            )
            .build()
            .map_err(|e| TlsError::Config(e.to_string()))?;
            if settings.mode == SslMode::VerifyCa {
                Arc::new(VerifyChainOnly(webpki))
            } else {
                webpki
            }
        }
    };
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsError::Config(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier);
    let config = match (&settings.client_cert_path, &settings.client_key_path) {
        (None, None) => builder.with_no_client_auth(),
        (Some(cert), Some(key)) => builder
            .with_client_auth_cert(
                load_certificates(cert, FileRole::ClientCertificate)?,
                load_client_key(key, key_passphrase)?,
            )
            .map_err(|e| TlsError::Config(sanitize::display(&e.to_string()).into_owned()))?,
        _ => return Err(TlsError::IncompleteClientIdentity),
    };
    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(not_before: i64, not_after: i64) -> CertInfo {
        CertInfo {
            not_before_unix: not_before,
            not_after_unix: not_after,
            fingerprint_sha256: String::new(),
            certificates: 1,
        }
    }

    const DAY: i64 = SECONDS_PER_DAY;

    #[test]
    fn expiry_states_use_the_warning_window() {
        let cert = info(0, 100 * DAY);
        assert_eq!(
            expiry_state(&cert, 10 * DAY, 30),
            ExpiryState::Valid { days_left: 90 }
        );
        assert_eq!(
            expiry_state(&cert, 70 * DAY, 30),
            ExpiryState::ExpiresSoon { days_left: 30 }
        );
        assert_eq!(
            expiry_state(&cert, 69 * DAY, 30),
            ExpiryState::Valid { days_left: 31 }
        );
        assert_eq!(
            expiry_state(&cert, 100 * DAY, 30),
            ExpiryState::Expired { days_ago: 0 }
        );
        assert_eq!(
            expiry_state(&cert, 103 * DAY, 30),
            ExpiryState::Expired { days_ago: 3 }
        );
        assert_eq!(
            expiry_state(&info(50 * DAY, 100 * DAY), 10 * DAY, 30),
            ExpiryState::NotYetValid
        );
    }

    #[test]
    fn disable_has_no_tls_configuration() {
        let settings = TlsSettings {
            mode: SslMode::Disable,
            ..TlsSettings::default()
        };
        assert!(client_config(&settings, None).unwrap().is_none());
    }

    #[test]
    fn verify_full_without_pem_files_uses_public_roots() {
        let config = client_config(&TlsSettings::default(), None).unwrap();
        assert!(config.is_some());
        let require = TlsSettings {
            mode: SslMode::Require,
            ..TlsSettings::default()
        };
        assert!(client_config(&require, None).unwrap().is_some());
    }

    #[test]
    fn client_certificate_without_key_is_rejected() {
        let settings = TlsSettings {
            client_cert_path: Some(PathBuf::from("c.pem")),
            ..TlsSettings::default()
        };
        assert!(matches!(
            client_config(&settings, None),
            Err(TlsError::IncompleteClientIdentity)
        ));
    }

    #[test]
    fn name_mismatch_is_the_only_error_verify_ca_forgives() {
        assert!(is_name_mismatch(&Error::InvalidCertificate(
            CertificateError::NotValidForName
        )));
        assert!(!is_name_mismatch(&Error::InvalidCertificate(
            CertificateError::Expired
        )));
        assert!(!is_name_mismatch(&Error::InvalidCertificate(
            CertificateError::UnknownIssuer
        )));
    }

    #[test]
    fn fingerprint_is_uppercase_hex_with_colons() {
        // SHA-256 de la cadena vacía
        assert_eq!(
            hex_fingerprint(b""),
            "E3:B0:C4:42:98:FC:1C:14:9A:FB:F4:C8:99:6F:B9:24:27:AE:41:E4:64:9B:93:4C:A4:95:99:1B:78:52:B8:55"
        );
    }

    #[test]
    fn errors_never_contain_file_content() {
        let dir =
            std::env::temp_dir().join(format!("tearminal-tls-content-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("junk.pem");
        std::fs::write(
            &path,
            "SECRETO-1234 -----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----",
        )
        .unwrap();
        for result in [
            certificate_info(&path, FileRole::Ca).map(|_| ()),
            key_format(&path).map(|_| ()),
        ] {
            let message = result.unwrap_err().to_string();
            assert!(!message.contains("SECRETO"), "{message}");
        }
    }

    #[test]
    fn warnings_follow_state_and_policy() {
        let cert = |state| FileReport {
            role: FileRole::ClientCertificate,
            path: PathBuf::from("c.pem"),
            status: FileStatus::Certificate {
                info: info(0, 1),
                state,
            },
        };
        let key = |access| FileReport {
            role: FileRole::ClientKey,
            path: PathBuf::from("k.pem"),
            status: FileStatus::PrivateKey {
                format: KeyFormat::Pkcs8,
                access,
            },
        };
        let reports = [
            cert(ExpiryState::ExpiresSoon { days_left: 22 }),
            cert(ExpiryState::Valid { days_left: 400 }),
            cert(ExpiryState::Expired { days_ago: 2 }),
            key(KeyAccess::SharedWith(vec!["Todos".into()])),
            key(KeyAccess::Private),
        ];
        let all = warnings(&reports, true);
        assert_eq!(all.len(), 3);
        assert!(all.contains(&Warning::CertificateExpiresSoon {
            role: FileRole::ClientCertificate,
            days_left: 22
        }));
        assert!(all.iter().any(|w| matches!(w, Warning::InsecureKeyPermissions { readers, .. } if readers == &["Todos"])));
        // Con el aviso de permisos apagado en Ajustes, desaparece
        assert_eq!(warnings(&reports, false).len(), 2);
    }

    #[test]
    fn missing_file_is_reported_without_failing() {
        let settings = TlsSettings {
            ca_path: Some(PathBuf::from(r"C:\no\existe\ca.pem")),
            ..TlsSettings::default()
        };
        let report = inspect(&settings, 0, 30);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].status, FileStatus::Missing);
    }
}

#[cfg(test)]
#[path = "tls_tests.rs"]
mod pem_and_handshake_tests;
