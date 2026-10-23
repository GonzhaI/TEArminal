//! Superficie de presentación: swap chain Flip sobre una ventana, redimensionado y recuperación de dispositivo.

use super::{DriverKind, GpuDevice, GpuRenderer};
use crate::renderer::scene::Scene;
use windows::core::{ComInterface, Error, Result};
use windows::Win32::Foundation::{E_FAIL, HWND};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

/// Resultado de `present`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presented {
    Ok,
    /// Se recreó el dispositivo: el llamante debe tratar la escena como repintada por completo.
    Recovered,
}

pub(crate) struct GpuSurface {
    renderer: GpuRenderer,
    /// `None` solo si una recuperación falló a medias: el siguiente `present` la reintenta
    swap: Option<IDXGISwapChain1>,
    rtv: Option<ID3D11RenderTargetView>,
    size: (u32, u32),
    hwnd: HWND,
    max_dim: u32,
    /// Fallo inyectado: el próximo `present` se comporta como si DXGI devolviera DEVICE_REMOVED.
    /// Ejercita la lógica de recreación, no un reinicio real del driver (no se puede provocar un TDR en
    /// un test); la prueba real en una segunda máquina es de la tarea 20.
    #[cfg(test)]
    simulated_loss: bool,
}

fn clamp_dim(v: u32, max: u32) -> u32 {
    v.clamp(1, max.max(1))
}

/// Errores que indican que el dispositivo ya no sirve y hay que recrearlo
fn is_device_error(e: &Error) -> bool {
    [
        DXGI_ERROR_DEVICE_REMOVED,
        DXGI_ERROR_DEVICE_RESET,
        DXGI_ERROR_DRIVER_INTERNAL_ERROR,
    ]
    .contains(&e.code())
}

/// Swap chain Flip de 2 búferes para `hwnd` (mismo descriptor que `d3d11.rs`) y la vista de su búfer trasero.
fn create_target(
    dev: &GpuDevice,
    hwnd: HWND,
    (w, h): (u32, u32),
) -> Result<(IDXGISwapChain1, ID3D11RenderTargetView)> {
    let dxgi_device: IDXGIDevice = dev.device.cast()?;
    let adapter = unsafe { dxgi_device.GetAdapter()? };
    let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: w,
        Height: h,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };
    let swap = unsafe { factory.CreateSwapChainForHwnd(&dev.device, hwnd, &desc, None, None)? };
    let rtv = create_rtv(dev, &swap)?;
    Ok((swap, rtv))
}

fn create_rtv(dev: &GpuDevice, swap: &IDXGISwapChain1) -> Result<ID3D11RenderTargetView> {
    let back: ID3D11Texture2D = unsafe { swap.GetBuffer(0)? };
    let mut rtv = None;
    unsafe {
        dev.device
            .CreateRenderTargetView(&back, None, Some(&mut rtv))?
    };
    rtv.ok_or_else(|| Error::from(E_FAIL))
}

impl GpuSurface {
    pub(crate) fn new(hwnd: HWND, w: u32, h: u32) -> Result<Self> {
        let dev = GpuDevice::new(Some(hwnd))?;
        let max_dim = dev.max_texture_dim();
        let size = (clamp_dim(w, max_dim), clamp_dim(h, max_dim));
        let (swap, rtv) = create_target(&dev, hwnd, size)?;
        Ok(Self {
            renderer: GpuRenderer::new(dev)?,
            swap: Some(swap),
            rtv: Some(rtv),
            size,
            hwnd,
            max_dim,
            #[cfg(test)]
            simulated_loss: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn size(&self) -> (u32, u32) {
        self.size
    }

    pub(crate) fn driver_kind(&self) -> DriverKind {
        self.renderer.device().kind
    }

    /// Acota cada lado a `1..=max_dim` (una ventana minimizada llega como 0x0) y redimensiona los búferes.
    pub(crate) fn resize(&mut self, w: u32, h: u32) -> Result<()> {
        let size = (clamp_dim(w, self.max_dim), clamp_dim(h, self.max_dim));
        if size == self.size {
            return Ok(());
        }
        let Some(swap) = self.swap.clone() else {
            // Recuperación pendiente: el próximo `present` crea la cadena con este tamaño
            self.size = size;
            return Ok(());
        };
        // DXGI exige soltar todas las referencias al búfer trasero antes de `ResizeBuffers`
        self.rtv = None;
        unsafe {
            self.renderer
                .device()
                .context
                .OMSetRenderTargets(None, None)
        };
        unsafe { swap.ResizeBuffers(0, size.0, size.1, DXGI_FORMAT_UNKNOWN, 0)? };
        self.size = size;
        self.rtv = Some(create_rtv(self.renderer.device(), &swap)?);
        Ok(())
    }

    /// Dibuja `scene` en el búfer trasero y lo presenta con VSync. Ante un dispositivo perdido lo recrea todo,
    /// repinta la escena y devuelve `Recovered`; los demás errores se propagan.
    pub(crate) fn present(&mut self, scene: &Scene) -> Result<Presented> {
        if self.swap.is_none() || self.rtv.is_none() {
            return self.recover(scene);
        }
        match self.draw_and_present(scene) {
            Ok(()) if self.device_removed() => self.recover(scene),
            Ok(()) => Ok(Presented::Ok),
            Err(e) if is_device_error(&e) || self.device_removed() => self.recover(scene),
            Err(e) => Err(e),
        }
    }

    fn device_removed(&self) -> bool {
        unsafe { self.renderer.device().device.GetDeviceRemovedReason() }.is_err()
    }

    fn draw_and_present(&mut self, scene: &Scene) -> Result<()> {
        #[cfg(test)]
        if std::mem::take(&mut self.simulated_loss) {
            return Err(Error::from(DXGI_ERROR_DEVICE_REMOVED));
        }
        let (Some(swap), Some(rtv)) = (&self.swap, &self.rtv) else {
            return Err(Error::from(E_FAIL));
        };
        self.renderer.render(scene, rtv, self.size)?;
        // `DXGI_STATUS_OCCLUDED` es un código de éxito: la ventana tapada no es un error
        unsafe { swap.Present(1, 0).ok() }
    }

    /// Suelta la vista y la cadena (un HWND solo admite una), recrea dispositivo, renderer (atlas e imágenes
    /// vuelven a rellenarse a demanda), cadena y vista con el mismo tamaño, y pinta la escena de nuevo.
    #[cold]
    fn recover(&mut self, scene: &Scene) -> Result<Presented> {
        let renderer = GpuRenderer::new(GpuDevice::new(Some(self.hwnd))?)?;
        self.rtv = None;
        unsafe {
            let ctx = &self.renderer.device().context;
            ctx.OMSetRenderTargets(None, None);
            ctx.ClearState();
            ctx.Flush();
        }
        self.swap = None;
        self.renderer = renderer; // suelta el dispositivo anterior y sus referencias a los búferes
        self.max_dim = self.renderer.device().max_texture_dim();
        self.size = (
            clamp_dim(self.size.0, self.max_dim),
            clamp_dim(self.size.1, self.max_dim),
        );
        let (swap, rtv) = create_target(self.renderer.device(), self.hwnd, self.size)?;
        self.swap = Some(swap);
        self.rtv = Some(rtv);
        self.draw_and_present(scene)?;
        Ok(Presented::Recovered)
    }
}

#[cfg(test)]
impl GpuSurface {
    pub(crate) fn simulate_device_loss(&mut self) {
        self.simulated_loss = true;
    }

    pub(crate) fn set_max_dim(&mut self, d: u32) {
        self.max_dim = d;
    }

    pub(crate) fn renderer_mut(&mut self) -> &mut GpuRenderer {
        &mut self.renderer
    }
}
