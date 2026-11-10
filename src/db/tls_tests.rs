//! Pruebas con archivos y un servidor TLS real en `127.0.0.1` (certificado y claves de prueba de `testdata/`,
//! creadas con openssl solo para esto; la frase de la clave cifrada es `frase-de-prueba`).

use super::*;
use std::net::{TcpListener, TcpStream};

const PASSPHRASE: &str = "frase-de-prueba";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/db/testdata")
        .join(name)
}

#[test]
fn certificate_info_reads_validity_and_fingerprint() {
    let info = certificate_info(&fixture("test-cert.pem"), FileRole::ClientCertificate).unwrap();
    assert_eq!(info.not_after_unix, 4_947_915_852);
    assert_eq!(info.not_before_unix, 1_794_315_852);
    assert_eq!(info.certificates, 1);
    assert_eq!(
        info.fingerprint_sha256,
        "C2:42:FB:6D:8F:6B:6F:D2:4E:F9:BB:66:3E:E8:43:4A:49:AE:68:7C:7F:3F:1D:15:98:90:22:B0:CD:0D:AE:1C"
    );
    let ca = certificate_info(&fixture("test-ca.pem"), FileRole::Ca).unwrap();
    assert_eq!(ca.not_after_unix, 4_947_915_850);
    assert!(matches!(
        expiry_state(&info, 1_794_315_852 + 100, 30),
        ExpiryState::Valid { .. }
    ));
    // 20 días antes del vencimiento cae en la ventana de aviso de 30
    assert!(matches!(
        expiry_state(&info, info.not_after_unix - 20 * SECONDS_PER_DAY, 30),
        ExpiryState::ExpiresSoon { days_left: 20 }
    ));
}

#[test]
fn key_formats_and_passphrase_handling() {
    let plain = fixture("test-key.pem");
    let encrypted = fixture("test-key-encrypted.pem");
    assert_eq!(key_format(&plain).unwrap(), KeyFormat::Pkcs8);
    assert_eq!(key_format(&encrypted).unwrap(), KeyFormat::EncryptedPkcs8);
    assert!(key_format(&fixture("test-cert.pem")).is_err());

    assert!(load_client_key(&plain, None).is_ok());
    assert!(matches!(
        load_client_key(&encrypted, None),
        Err(TlsError::PassphraseRequired)
    ));
    assert!(matches!(
        load_client_key(&encrypted, Some(&Secret::new("otra"))),
        Err(TlsError::WrongPassphrase)
    ));
    let decrypted = load_client_key(&encrypted, Some(&Secret::new(PASSPHRASE))).unwrap();
    let original = load_client_key(&plain, None).unwrap();
    assert_eq!(decrypted.secret_der(), original.secret_der());
}

#[test]
fn decrypted_key_pem_is_plain_pkcs8_in_memory() {
    let pem = client_key_pem(
        &fixture("test-key-encrypted.pem"),
        Some(&Secret::new(PASSPHRASE)),
    )
    .unwrap();
    assert!(String::from_utf8(pem.clone())
        .unwrap()
        .starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(PrivateKeyDer::from_pem_slice(&pem).is_ok());
}

#[test]
fn client_identity_builds_with_plain_and_encrypted_keys() {
    let mut settings = TlsSettings {
        ca_path: Some(fixture("test-ca.pem")),
        client_cert_path: Some(fixture("test-cert.pem")),
        client_key_path: Some(fixture("test-key.pem")),
        ..TlsSettings::default()
    };
    assert!(client_config(&settings, None).unwrap().is_some());
    settings.client_key_path = Some(fixture("test-key-encrypted.pem"));
    assert!(client_config(&settings, Some(&Secret::new(PASSPHRASE)))
        .unwrap()
        .is_some());
    assert!(matches!(
        client_config(&settings, None),
        Err(TlsError::PassphraseRequired)
    ));
}

#[test]
fn inspect_reports_each_configured_file() {
    let settings = TlsSettings {
        ca_path: Some(fixture("test-ca.pem")),
        client_key_path: Some(fixture("test-key-encrypted.pem")),
        ..TlsSettings::default()
    };
    let report = inspect(&settings, 1_794_315_852 + 10, 30);
    assert_eq!(report.len(), 2);
    assert!(matches!(
        report[0].status,
        FileStatus::Certificate {
            state: ExpiryState::Valid { .. },
            ..
        }
    ));
    assert!(matches!(
        report[1].status,
        FileStatus::PrivateKey {
            format: KeyFormat::EncryptedPkcs8,
            ..
        }
    ));
}

/// Servidor TLS de un solo apretón de manos con el certificado de prueba
fn spawn_server() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let certs = load_certificates(&fixture("test-cert.pem"), FileRole::ClientCertificate).unwrap();
    let key = load_client_key(&fixture("test-key.pem"), None).unwrap();
    let config = Arc::new(
        rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap(),
    );
    let handle = std::thread::spawn(move || {
        let Ok((mut tcp, _)) = listener.accept() else {
            return;
        };
        let Ok(mut conn) = rustls::ServerConnection::new(config) else {
            return;
        };
        while conn.is_handshaking() {
            if conn.complete_io(&mut tcp).is_err() {
                return;
            }
        }
        // Deja que el cliente lea el final del apretón de manos antes de cerrar
        let _ = conn.complete_io(&mut tcp);
    });
    (port, handle)
}

fn handshake(settings: &TlsSettings, server_name: &str) -> Result<(), String> {
    let (port, server) = spawn_server();
    let config = client_config(settings, None)
        .map_err(|e| e.to_string())?
        .ok_or("sin TLS")?;
    let name = ServerName::try_from(server_name.to_string()).map_err(|e| e.to_string())?;
    let mut conn =
        rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    let mut tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();
    let mut outcome = Ok(());
    while conn.is_handshaking() {
        if let Err(e) = conn.complete_io(&mut tcp) {
            outcome = Err(e.to_string());
            break;
        }
    }
    drop(tcp);
    let _ = server.join();
    outcome
}

fn with_ca(mode: SslMode) -> TlsSettings {
    TlsSettings {
        mode,
        ca_path: Some(fixture("test-ca.pem")),
        ..TlsSettings::default()
    }
}

#[test]
fn verify_full_accepts_the_right_name_and_rejects_a_wrong_one() {
    assert_eq!(
        handshake(&with_ca(SslMode::VerifyFull), "localhost"),
        Ok(())
    );
    assert!(handshake(&with_ca(SslMode::VerifyFull), "otro.example").is_err());
}

#[test]
fn verify_ca_ignores_the_name_but_not_the_issuer() {
    assert_eq!(
        handshake(&with_ca(SslMode::VerifyCa), "otro.example"),
        Ok(())
    );
    // Sin la CA propia, el certificado autofirmado no es de confianza
    let public_roots_only = TlsSettings {
        mode: SslMode::VerifyCa,
        ..TlsSettings::default()
    };
    assert!(handshake(&public_roots_only, "localhost").is_err());
}

#[test]
fn verify_full_is_the_default_and_rejects_an_unknown_issuer() {
    assert_eq!(TlsSettings::default().mode, SslMode::VerifyFull);
    assert!(handshake(&TlsSettings::default(), "localhost").is_err());
}

#[test]
fn require_encrypts_without_verifying_identity() {
    let require = TlsSettings {
        mode: SslMode::Require,
        ..TlsSettings::default()
    };
    assert_eq!(handshake(&require, "cualquier.nombre"), Ok(()));
}
