//! Dispositivo D3D11: adaptador del monitor → WARP. Se crea sin ventana (tests y render sin cabeza).

use windows::core::{ComInterface, Result};
use windows::Win32::Foundation::{E_FAIL, HWND};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DriverKind {
    Hardware,
    Warp,
}

pub(crate) struct GpuDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub kind: DriverKind,
    pub feature_level: D3D_FEATURE_LEVEL,
}

/// Nivel mínimo que Axon acepta: los shaders son SM 4.0
const MIN_LEVEL: D3D_FEATURE_LEVEL = D3D_FEATURE_LEVEL_10_0;
const MAX_LEVEL: D3D_FEATURE_LEVEL = D3D_FEATURE_LEVEL_11_1;

/// `AXON_RENDERER=warp` fuerza WARP (diagnóstico y CI).
pub(super) fn requested_driver() -> Option<DriverKind> {
    requested_driver_from(std::env::var("AXON_RENDERER").ok().as_deref())
}

pub(crate) fn requested_driver_from(value: Option<&str>) -> Option<DriverKind> {
    value
        .filter(|v| v.eq_ignore_ascii_case("warp"))
        .map(|_| DriverKind::Warp)
}

/// Lado máximo de textura: 8192 con FL 10_x, 16384 con 11_0 o más.
pub(crate) fn max_texture_dim_for(level: D3D_FEATURE_LEVEL) -> u32 {
    if level.0 >= D3D_FEATURE_LEVEL_11_0.0 {
        16384
    } else {
        8192
    }
}

impl GpuDevice {
    /// Hardware en el adaptador del monitor de `hwnd` (o el que elija Windows) y, si falla, WARP.
    pub(crate) fn new(hwnd: Option<HWND>) -> Result<GpuDevice> {
        if requested_driver() != Some(DriverKind::Warp) {
            let adapter = hwnd.and_then(adapter_for_window);
            if let Ok(gpu) =
                Self::create(adapter.as_ref(), DriverKind::Hardware, MIN_LEVEL, MAX_LEVEL)
            {
                return Ok(gpu);
            }
        }
        Self::create(None, DriverKind::Warp, MIN_LEVEL, MAX_LEVEL)
    }

    #[cfg(test)]
    /// Fuerza un tipo de dispositivo (`None` = como `new` sin ventana). Para tests.
    pub(crate) fn new_with(
        kind: Option<DriverKind>,
        min_level: D3D_FEATURE_LEVEL,
    ) -> Result<GpuDevice> {
        match kind {
            Some(kind) => Self::create(None, kind, min_level, MAX_LEVEL),
            None => Self::create(None, DriverKind::Hardware, min_level, MAX_LEVEL)
                .or_else(|_| Self::create(None, DriverKind::Warp, min_level, MAX_LEVEL)),
        }
    }

    #[cfg(test)]
    /// Dispositivo de `kind` limitado a `max_level` como mucho (probar FL 10_x en una máquina con 11_x).
    pub(crate) fn new_at_most(kind: DriverKind, max_level: D3D_FEATURE_LEVEL) -> Result<GpuDevice> {
        Self::create(None, kind, MIN_LEVEL, max_level)
    }

    pub(crate) fn max_texture_dim(&self) -> u32 {
        max_texture_dim_for(self.feature_level)
    }

    fn create(
        adapter: Option<&IDXGIAdapter>,
        kind: DriverKind,
        min_level: D3D_FEATURE_LEVEL,
        max_level: D3D_FEATURE_LEVEL,
    ) -> Result<GpuDevice> {
        let driver_type = match (kind, adapter) {
            (DriverKind::Warp, _) => D3D_DRIVER_TYPE_WARP,
            (DriverKind::Hardware, Some(_)) => D3D_DRIVER_TYPE_UNKNOWN, // con adaptador explícito, UNKNOWN
            (DriverKind::Hardware, None) => D3D_DRIVER_TYPE_HARDWARE,
        };
        let levels: Vec<_> = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ]
        .into_iter()
        .filter(|l| (min_level.0..=max_level.0).contains(&l.0))
        .collect();
        let adapter = if driver_type == D3D_DRIVER_TYPE_UNKNOWN {
            adapter
        } else {
            None
        };
        let try_flags = |flags| {
            let mut device = None;
            let mut context = None;
            let mut feature_level = D3D_FEATURE_LEVEL_10_0;
            unsafe {
                D3D11CreateDevice(
                    adapter,
                    driver_type,
                    None,
                    flags,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    Some(&mut feature_level),
                    Some(&mut context),
                )?;
            }
            Ok(GpuDevice {
                device: device.ok_or_else(|| windows::core::Error::from(E_FAIL))?,
                context: context.ok_or_else(|| windows::core::Error::from(E_FAIL))?,
                kind,
                feature_level,
            })
        };
        let base = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        // La capa de depuración puede no estar instalada: se reintenta sin ella
        if cfg!(debug_assertions) {
            if let Ok(gpu) = try_flags(base | D3D11_CREATE_DEVICE_DEBUG) {
                return Ok(gpu);
            }
        }
        try_flags(base)
    }
}

/// Adaptador cuyo `IDXGIOutput` es el monitor de la ventana (`None` = que decida Windows). Un portátil híbrido
/// con el adaptador por defecto usaría la GPU discreta: carga su driver y la saca del reposo.
fn adapter_for_window(hwnd: HWND) -> Option<IDXGIAdapter> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().ok()? };
    (0..)
        .map_while(|i| unsafe { factory.EnumAdapters1(i).ok() })
        .find(|adapter| {
            (0..)
                .map_while(|j| unsafe { adapter.EnumOutputs(j).ok() })
                .any(|out| {
                    let mut desc = DXGI_OUTPUT_DESC::default();
                    unsafe { out.GetDesc(&mut desc) }.is_ok() && desc.Monitor == monitor
                })
        })
        .and_then(|a| a.cast().ok())
}
