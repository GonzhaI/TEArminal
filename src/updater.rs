//! Mecanismo de comprobación y ejecución de actualizaciones automáticas para Axon.
//! Consulta la API de GitHub Releases en segundo plano sin dependencias pesadas ni sobrecoste de CPU.

use crate::app::platform::hide_console;
use crate::json::extract_json_str;
use std::path::Path;
use std::process::Command;
use std::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: Option<String>,
    /// `SHA256SUMS.txt` de la misma release: el instalador no se ejecuta si su hash no coincide
    pub checksums_url: Option<String>,
    pub html_url: String,
}

/// Único origen aceptado para descargar el instalador y sus sumas de verificación
const RELEASE_PREFIX: &str = "https://github.com/GonzhaI/axon/releases/download/";

static AVAILABLE_UPDATE: RwLock<Option<UpdateInfo>> = RwLock::new(None);

/// Devuelve la información de actualización disponible, si existe
pub fn get_available_update() -> Option<UpdateInfo> {
    AVAILABLE_UPDATE.read().ok().and_then(|lock| lock.clone())
}

/// Almacena la actualización disponible detectada
pub fn set_available_update(info: Option<UpdateInfo>) {
    if let Ok(mut lock) = AVAILABLE_UPDATE.write() {
        *lock = info;
    }
}

/// Extrae la tupla semver `(major, minor, patch)` a partir de una cadena de versión ("0.7.1", "v0.8.0", etc.)
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let clean = v.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = clean.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_str = parts.next().unwrap_or("0");
    let patch_digits: String = patch_str
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let patch = patch_digits.parse().ok().unwrap_or(0);
    Some((major, minor, patch))
}

/// Determina si `latest` es estrictamente mayor que `current` según SemVer
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Primera URL de asset (`browser_download_url`) de la release que cumple `accept`; solo se aceptan las que
/// cuelgan de `RELEASE_PREFIX`
fn extract_asset_url(json: &str, accept: impl Fn(&str) -> bool) -> Option<String> {
    let mut search_from = 0;
    while let Some(pos) = json[search_from..].find("\"browser_download_url\"") {
        let abs_pos = search_from + pos;
        if let Some(val) = extract_json_str(&json[abs_pos..], "browser_download_url") {
            if val.starts_with(RELEASE_PREFIX) && accept(&val) {
                return Some(val);
            }
        }
        search_from = abs_pos + 22;
    }
    None
}

/// Busca en el JSON de GitHub Releases la URL de descarga directa del instalador de Axon
pub fn extract_installer_download_url(json: &str) -> Option<String> {
    extract_asset_url(json, |v| {
        v.ends_with(".exe") && (v.contains("setup") || v.contains("Setup"))
    })
}

/// URL de `SHA256SUMS.txt` de la release
fn extract_checksums_url(json: &str) -> Option<String> {
    extract_asset_url(json, |v| v.ends_with("/SHA256SUMS.txt"))
}

/// Hash esperado de `file_name` en un `SHA256SUMS.txt` (`<hash>  <archivo>` por línea)
fn expected_hash(sums: &str, file_name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        let (hash, name) = (it.next()?, it.next()?);
        (name.trim_start_matches('*') == file_name).then(|| hash.to_ascii_lowercase())
    })
}

/// SHA-256 de un archivo con `certutil` (viene con Windows)
fn sha256_hex(path: &Path) -> Option<String> {
    let out = hide_console(&mut Command::new("certutil"))
        .args(["-hashfile"])
        .arg(path)
        .arg("SHA256")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let hash: String = text.lines().nth(1)?.split_whitespace().collect();
    (out.status.success() && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}

/// GET con `curl.exe` (incluido desde Windows 10 1803; Axon exige 1809): solo HTTPS, falla en códigos HTTP
/// de error y devuelve el cuerpo
fn curl_get(url: &str, max_time: &str, extra: &[&str]) -> Option<Vec<u8>> {
    let out = hide_console(&mut Command::new("curl.exe"))
        .args(["-fsSL", "--proto", "=https", "--proto-redir", "=https"])
        .args(["--max-time", max_time])
        .args(extra)
        .arg(url)
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Comprueba sincrónicamente si hay una nueva versión en GitHub Releases
pub fn check_for_updates_sync() -> Option<UpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let json_text = fetch_releases_json()?;

    let tag_name = extract_json_str(&json_text, "tag_name")?;
    let latest_clean = tag_name.trim_start_matches('v').trim_start_matches('V');

    if !is_newer_version(&current_version, latest_clean) {
        return None;
    }

    let html_url = extract_json_str(&json_text, "html_url")
        .unwrap_or_else(|| "https://github.com/GonzhaI/axon/releases/latest".to_string());

    let download_url = extract_installer_download_url(&json_text);
    let checksums_url = extract_checksums_url(&json_text);

    Some(UpdateInfo {
        current_version,
        latest_version: latest_clean.to_string(),
        download_url,
        checksums_url,
        html_url,
    })
}

/// Última release de GitHub (JSON) con `curl.exe`
fn fetch_releases_json() -> Option<String> {
    let body = curl_get(
        "https://api.github.com/repos/GonzhaI/axon/releases/latest",
        "5",
        &[
            "-H",
            "User-Agent: Axon-Terminal",
            "-H",
            "Accept: application/vnd.github.v3+json",
        ],
    )?;
    let body = String::from_utf8_lossy(&body).into_owned();
    body.contains("\"tag_name\"").then_some(body)
}

/// Lanza la comprobación de actualización en un hilo en segundo plano y notifica al EventLoop
pub fn check_for_updates_background(
    proxy: Option<winit::event_loop::EventLoopProxy<crate::app::AxonEvent>>,
) {
    std::thread::Builder::new()
        .name("axon-update-checker".to_string())
        .spawn(move || {
            if let Some(info) = check_for_updates_sync() {
                crate::logger::log(&format!(
                    "Nueva versión de Axon disponible: v{} (actual: v{})",
                    info.latest_version, info.current_version
                ));
                set_available_update(Some(info));
                if let Some(proxy) = proxy {
                    let _ = proxy.send_event(crate::app::AxonEvent::UpdateCheckDone);
                }
            }
        })
        .ok();
}

/// Descarga el instalador a una carpeta temporal nueva y devuelve su ruta solo si su SHA-256 coincide con el
/// de `SHA256SUMS.txt` de la misma release. Sin sumas, sin coincidencia o con una URL ajena: `None`.
fn download_verified_installer(info: &UpdateInfo) -> Option<std::path::PathBuf> {
    let url = info.download_url.as_deref()?;
    let name = url.rsplit('/').next()?;
    if !url.starts_with(RELEASE_PREFIX)
        || !name.ends_with(".exe")
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return None;
    }
    let sums = curl_get(info.checksums_url.as_deref()?, "15", &[])?;
    let expected = expected_hash(&String::from_utf8_lossy(&sums), name)?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("axon-update-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(name);
    let verified = curl_get(url, "60", &[])
        .is_some_and(|bytes| std::fs::write(&path, bytes).is_ok())
        && sha256_hex(&path).is_some_and(|h| h == expected);
    if !verified {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }
    Some(path)
}

/// Abre una URL en el navegador web por defecto del sistema (solo `http(s)`: ver `shell_open_url`)
pub fn open_url_in_browser(url: &str) {
    crate::app::platform::shell_open_url(url);
}

/// Dispara la acción de actualización: descarga el instalador y lo lanza, o abre la release en el navegador
pub fn trigger_update() {
    let Some(info) = get_available_update() else {
        return;
    };

    std::thread::Builder::new()
        .name("axon-updater".to_string())
        .spawn(move || {
            if let Some(installer_path) = download_verified_installer(&info) {
                crate::logger::log(&format!(
                    "Lanzando instalador de Axon: {}",
                    installer_path.display()
                ));
                if Command::new(&installer_path).spawn().is_ok() {
                    return;
                }
            } else {
                crate::logger::log(
                    "Instalador no descargado o sin verificar: se abre la página de la versión",
                );
            }

            // Fallback: abrir página de descarga en el navegador
            crate::logger::log(&format!(
                "Abriendo página de versión en el navegador: {}",
                info.html_url
            ));
            open_url_in_browser(&info.html_url);
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing_and_comparison() {
        assert_eq!(parse_version("0.7.1"), Some((0, 7, 1)));
        assert_eq!(parse_version("v0.8.0"), Some((0, 8, 0)));
        assert_eq!(parse_version("V1.2.34-beta"), Some((1, 2, 34)));

        assert!(is_newer_version("0.7.1", "0.7.2"));
        assert!(is_newer_version("0.7.1", "0.8.0"));
        assert!(is_newer_version("0.7.1", "1.0.0"));
        assert!(is_newer_version("v0.7.1", "v0.7.2"));

        assert!(!is_newer_version("0.7.1", "0.7.1"));
        assert!(!is_newer_version("0.7.1", "0.7.0"));
        assert!(!is_newer_version("0.8.0", "0.7.9"));
    }

    #[test]
    fn test_extract_installer_download_url() {
        let sample = r#"{
            "assets": [
                {
                    "name": "axon-0.7.0-windows-x64.zip",
                    "browser_download_url": "https://github.com/GonzhaI/axon/releases/download/v0.7.0/axon-0.7.0-windows-x64.zip"
                },
                {
                    "name": "axon-setup-0.7.0.exe",
                    "browser_download_url": "https://github.com/GonzhaI/axon/releases/download/v0.7.0/axon-setup-0.7.0.exe"
                }
            ]
        }"#;

        let url = extract_installer_download_url(sample);
        assert_eq!(
            url,
            Some(
                "https://github.com/GonzhaI/axon/releases/download/v0.7.0/axon-setup-0.7.0.exe"
                    .to_string()
            )
        );
    }

    /// Una URL de otro origen nunca se acepta como instalador ni como sumas
    #[test]
    fn test_asset_urls_must_come_from_the_release_origin() {
        let sample = r#"{"assets":[
            {"browser_download_url": "https://evil.example/axon-setup-9.9.9.exe"},
            {"browser_download_url": "https://evil.example/SHA256SUMS.txt"}]}"#;
        assert_eq!(extract_installer_download_url(sample), None);
        assert_eq!(extract_checksums_url(sample), None);
        let ok = r#"{"browser_download_url": "https://github.com/GonzhaI/axon/releases/download/v1/SHA256SUMS.txt"}"#;
        assert!(extract_checksums_url(ok).is_some());
    }

    #[test]
    fn test_expected_hash_finds_the_installer_line() {
        let sums = "AAAA  axon-0.7.0-windows-x64.zip\nBBBB  axon-setup-0.7.0.exe\n";
        assert_eq!(
            expected_hash(sums, "axon-setup-0.7.0.exe"),
            Some("bbbb".to_string())
        );
        assert_eq!(expected_hash(sums, "otro.exe"), None);
    }

    #[test]
    fn test_sha256_hex_matches_known_vector() {
        let path = std::env::temp_dir().join(format!("axon-sha-{}.txt", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let hash = sha256_hex(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            hash.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
