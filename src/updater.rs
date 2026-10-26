//! Mecanismo de comprobación y ejecución de actualizaciones automáticas para TEArminal.
//! Consulta la API de GitHub Releases en segundo plano sin dependencias pesadas ni sobrecoste de CPU.

use crate::app::platform::hide_console;
use crate::json::extract_json_str;
use std::marker::PhantomData;
use std::path::Path;
use std::process::Command;
use std::sync::RwLock;

// -------------------------------------------------------------------------------------------------
// Patrón Typestate para el instalador (Estados en Tiempo de Compilación)
// -------------------------------------------------------------------------------------------------

/// Marcador zero-sized: instalador descargado en disco pero cuyo hash SHA-256 no ha sido verificado.
pub(crate) struct Unverified;

/// Marcador zero-sized: instalador cuyo hash SHA-256 coincide estrictamente con el oficial de la release.
pub(crate) struct Verified;

/// Instalador de TEArminal con estados en tiempo de compilación (Patrón Typestate).
///
/// El método `launch()` **solo existe** en `UpdateInstaller<Verified>`. Es un error
/// de compilación intentar ejecutar un instalador en estado `Unverified`, garantizando por tipo
/// que ningún binario descargado pueda ejecutarse sin haber superado `verify()`.
pub(crate) struct UpdateInstaller<State = Unverified> {
    path: std::path::PathBuf,
    _state: PhantomData<State>,
}

impl UpdateInstaller<Unverified> {
    pub(crate) fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            _state: PhantomData,
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Consume el instalador no verificado y devuelve `UpdateInstaller<Verified>` solo
    /// si el hash SHA-256 calculado coincide con el esperado.
    pub(crate) fn verify(self, expected_sha256: &str) -> Result<UpdateInstaller<Verified>, String> {
        let computed =
            sha256_hex(&self.path).ok_or_else(|| "no se pudo calcular el hash".to_string())?;
        if computed.eq_ignore_ascii_case(expected_sha256) {
            Ok(UpdateInstaller {
                path: self.path,
                _state: PhantomData,
            })
        } else {
            Err(format!(
                "hash no coincide: esperado {expected_sha256}, calculado {computed}"
            ))
        }
    }
}

impl UpdateInstaller<Verified> {
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Ejecuta el instalador verificado. Solo accesible tras `verify()` exitoso en tiempo de compilación.
    pub(crate) fn launch(&self) -> Result<(), std::io::Error> {
        Command::new(&self.path).spawn()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct UpdateInfo {
    pub(crate) current_version: String,
    pub(crate) latest_version: String,
    pub(crate) download_url: Option<String>,
    /// `SHA256SUMS.txt` de la misma release: el instalador no se ejecuta si su hash no coincide
    pub(crate) checksums_url: Option<String>,
    pub(crate) html_url: String,
}

/// Único origen aceptado para descargar el instalador y sus sumas de verificación
const RELEASE_PREFIX: &str = "https://github.com/GonzhaI/TEArminal/releases/download/";

static AVAILABLE_UPDATE: RwLock<Option<UpdateInfo>> = RwLock::new(None);

/// Devuelve la información de actualización disponible, si existe
pub(crate) fn get_available_update() -> Option<UpdateInfo> {
    AVAILABLE_UPDATE.read().ok().and_then(|lock| lock.clone())
}

/// Almacena la actualización disponible detectada
pub(crate) fn set_available_update(info: Option<UpdateInfo>) {
    if let Ok(mut lock) = AVAILABLE_UPDATE.write() {
        *lock = info;
    }
}

/// Extrae la tupla semver `(major, minor, patch)` a partir de una cadena de versión ("0.7.1", "v0.8.0", etc.)
pub(crate) fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
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
pub(crate) fn is_newer_version(current: &str, latest: &str) -> bool {
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

/// Busca en el JSON de GitHub Releases la URL de descarga directa del instalador de TEArminal
pub(crate) fn extract_installer_download_url(json: &str) -> Option<String> {
    extract_asset_url(json, |v| {
        v.ends_with(".exe") && (v.contains("setup") || v.contains("Setup"))
    })
}

/// URL de `SHA256SUMS.txt` de la release
fn extract_checksums_url(json: &str) -> Option<String> {
    extract_asset_url(json, |v| v.ends_with("/SHA256SUMS.txt"))
}

/// Hash esperado de `file_name` desde un lector genérico `BufRead` reutilizando un único buffer
fn expected_hash_reader<R: std::io::BufRead>(mut reader: R, file_name: &str) -> Option<String> {
    let mut line = String::with_capacity(256);
    while reader.read_line(&mut line).ok()? != 0 {
        let mut it = line.split_whitespace();
        if let (Some(hash), Some(name)) = (it.next(), it.next()) {
            if name.trim_start_matches('*') == file_name {
                return Some(hash.to_ascii_lowercase());
            }
        }
        line.clear();
    }
    None
}

/// Hash esperado de `file_name` en un `SHA256SUMS.txt` (`<hash>  <archivo>` por línea)
fn expected_hash(sums: &str, file_name: &str) -> Option<String> {
    expected_hash_reader(sums.as_bytes(), file_name)
}

/// SHA-256 de un archivo con `certutil` (viene con Windows)
pub(crate) fn sha256_hex(path: &Path) -> Option<String> {
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

/// GET con `curl.exe` (incluido desde Windows 10 1803; TEArminal exige 1809): solo HTTPS, falla en códigos HTTP
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
pub(crate) fn check_for_updates_sync() -> Option<UpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let json_text = fetch_releases_json()?;

    let tag_name = extract_json_str(&json_text, "tag_name")?;
    let latest_clean = tag_name.trim_start_matches('v').trim_start_matches('V');

    if !is_newer_version(&current_version, latest_clean) {
        return None;
    }

    let html_url = extract_json_str(&json_text, "html_url")
        .unwrap_or_else(|| "https://github.com/GonzhaI/TEArminal/releases/latest".to_string());

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
        "https://api.github.com/repos/GonzhaI/TEArminal/releases/latest",
        "5",
        &[
            "-H",
            "User-Agent: TEArminal",
            "-H",
            "Accept: application/vnd.github.v3+json",
        ],
    )?;
    let body = String::from_utf8_lossy(&body).into_owned();
    body.contains("\"tag_name\"").then_some(body)
}

/// Lanza la comprobación de actualización en un hilo en segundo plano y notifica al EventLoop
pub(crate) fn check_for_updates_background(
    proxy: Option<winit::event_loop::EventLoopProxy<crate::app::TearminalEvent>>,
) {
    std::thread::Builder::new()
        .name("tearminal-update-checker".to_string())
        .spawn(move || {
            if let Some(info) = check_for_updates_sync() {
                crate::logger::log(&format!(
                    "Nueva versión de TEArminal disponible: v{} (actual: v{})",
                    info.latest_version, info.current_version
                ));
                set_available_update(Some(info));
                if let Some(proxy) = proxy {
                    let _ = proxy.send_event(crate::app::TearminalEvent::UpdateCheckDone);
                }
            }
        })
        .ok();
}

/// Descarga el instalador a una carpeta temporal nueva y devuelve un `UpdateInstaller<Verified>` solo
/// si su SHA-256 coincide con el de `SHA256SUMS.txt` de la misma release. Sin sumas, sin coincidencia o con una URL ajena: `None`.
fn download_verified_installer(info: &UpdateInfo) -> Option<UpdateInstaller<Verified>> {
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
    let dir = std::env::temp_dir().join(format!("tearminal-update-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(name);
    let downloaded =
        curl_get(url, "60", &[]).is_some_and(|bytes| std::fs::write(&path, bytes).is_ok());
    if !downloaded {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }

    let unverified = UpdateInstaller::new(path);
    match unverified.verify(&expected) {
        Ok(verified) => Some(verified),
        Err(e) => {
            crate::logger::log(&format!("Verificación de instalador fallida: {e}"));
            let _ = std::fs::remove_dir_all(&dir);
            None
        }
    }
}

/// Abre una URL en el navegador web por defecto del sistema (solo `http(s)`: ver `shell_open_url`)
pub(crate) fn open_url_in_browser(url: &str) {
    crate::app::platform::shell_open_url(url);
}

/// Dispara la acción de actualización: descarga el instalador y lo lanza, o abre la release en el navegador
pub(crate) fn trigger_update() {
    let Some(info) = get_available_update() else {
        return;
    };

    std::thread::Builder::new()
        .name("tearminal-updater".to_string())
        .spawn(move || {
            if let Some(installer) = download_verified_installer(&info) {
                crate::logger::log(&format!(
                    "Lanzando instalador de TEArminal: {}",
                    installer.path().display()
                ));
                if installer.launch().is_ok() {
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
                    "name": "tearminal-0.7.0-windows-x64.zip",
                    "browser_download_url": "https://github.com/GonzhaI/TEArminal/releases/download/v0.7.0/tearminal-0.7.0-windows-x64.zip"
                },
                {
                    "name": "tearminal-setup-0.7.0.exe",
                    "browser_download_url": "https://github.com/GonzhaI/TEArminal/releases/download/v0.7.0/tearminal-setup-0.7.0.exe"
                }
            ]
        }"#;

        let url = extract_installer_download_url(sample);
        assert_eq!(
            url,
            Some(
                "https://github.com/GonzhaI/TEArminal/releases/download/v0.7.0/tearminal-setup-0.7.0.exe"
                    .to_string()
            )
        );
    }

    /// Una URL de otro origen nunca se acepta como instalador ni como sumas
    #[test]
    fn test_asset_urls_must_come_from_the_release_origin() {
        let sample = r#"{"assets":[
            {"browser_download_url": "https://evil.example/tearminal-setup-9.9.9.exe"},
            {"browser_download_url": "https://evil.example/SHA256SUMS.txt"}]}"#;
        assert_eq!(extract_installer_download_url(sample), None);
        assert_eq!(extract_checksums_url(sample), None);
        let ok = r#"{"browser_download_url": "https://github.com/GonzhaI/TEArminal/releases/download/v1/SHA256SUMS.txt"}"#;
        assert!(extract_checksums_url(ok).is_some());
    }

    #[test]
    fn test_expected_hash_finds_the_installer_line() {
        let sums = "AAAA  tearminal-0.7.0-windows-x64.zip\nBBBB  tearminal-setup-0.7.0.exe\n";
        assert_eq!(
            expected_hash(sums, "tearminal-setup-0.7.0.exe"),
            Some("bbbb".to_string())
        );
        assert_eq!(expected_hash(sums, "otro.exe"), None);
    }

    #[test]
    fn test_sha256_hex_matches_known_vector() {
        let path = std::env::temp_dir().join(format!("tearminal-sha-{}.txt", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let hash = sha256_hex(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            hash.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn test_typestate_update_installer_verification() {
        let path = std::env::temp_dir().join(format!(
            "tearminal-test-installer-{}.exe",
            std::process::id()
        ));
        std::fs::write(&path, b"dummy installer content").unwrap();

        let real_hash = sha256_hex(&path).unwrap();

        // 1. Empieza en estado Unverified
        let unverified = UpdateInstaller::new(path.clone());
        assert_eq!(unverified.path(), &path);

        // 2. Verificación fallida con hash incorrecto rechaza la transición
        let failed_result =
            unverified.verify("0000000000000000000000000000000000000000000000000000000000000000");
        assert!(failed_result.is_err());

        // 3. Verificación exitosa produce UpdateInstaller<Verified>
        let unverified_2 = UpdateInstaller::new(path.clone());
        let verified: UpdateInstaller<Verified> =
            unverified_2.verify(&real_hash).expect("debe verificar");
        assert_eq!(verified.path(), &path);

        // Limpieza
        let _ = std::fs::remove_file(&path);
    }
}
