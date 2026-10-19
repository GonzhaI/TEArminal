//! Presentador nativo Direct3D 11 / DXGI Flip-Model para Windows (GPU).
//!
//! Reemplaza el canal de copia lenta de GDI (`SetDIBitsToDevice`) por un SwapChain
//! con DirectFlip (`DXGI_SWAP_EFFECT_FLIP_DISCARD`), permitiendo presentación
//! por hardware a la GPU sin tearing, con VSync y soporte nativo para monitores de alta frecuencia.

use windows::core::{ComInterface, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};

/// Dispositivo D3D11 en el adaptador que dibuja el monitor de la ventana. Con el adaptador por defecto, un
/// portátil híbrido usa la GPU discreta: carga su driver (~200 MB de RAM), la saca del reposo y copia cada
/// fotograma a la GPU que maneja la pantalla (sin DirectFlip).
fn create_device(hwnd: HWND) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let adapter = adapter_for_window(hwnd);
    let driver_type = if adapter.is_some() {
        D3D_DRIVER_TYPE_UNKNOWN // con adaptador explícito Direct3D exige UNKNOWN
    } else {
        D3D_DRIVER_TYPE_HARDWARE
    };
    let mut device = None;
    let mut context = None;
    let mut feature_level = D3D_FEATURE_LEVEL_11_0;
    let feature_levels = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
    ];
    unsafe {
        D3D11CreateDevice(
            adapter.as_ref(),
            driver_type,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            Some(&mut context),
        )?;
    }
    let device = device.ok_or_else(windows::core::Error::from_win32)?;
    let context = context.ok_or_else(windows::core::Error::from_win32)?;
    Ok((device, context))
}

/// Adaptador cuyo `IDXGIOutput` es el monitor de la ventana (`None` = que decida Windows)
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

pub struct D3D11Presenter {
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    back_buffer: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
    buffer: Vec<u32>,
}

impl D3D11Presenter {
    /// Crea una nueva superficie de presentación D3D11 vinculada a la ventana HWND
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        let width = width.max(1);
        let height = height.max(1);

        let (device, context) = create_device(hwnd)?;

        let dxgi_device: IDXGIDevice = device.cast()?;
        let adapter = unsafe { dxgi_device.GetAdapter()? };
        let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_NONE, // STRETCH deforma el último fotograma mientras se redimensiona
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: 0,
        };

        let swap_chain =
            unsafe { factory.CreateSwapChainForHwnd(&device, hwnd, &desc, None, None)? };

        let back_buffer: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0)? };
        let buffer = vec![0u32; (width * height) as usize];

        Ok(Self {
            _device: device,
            context,
            swap_chain,
            back_buffer: Some(back_buffer),
            width,
            height,
            buffer,
        })
    }

    /// Redimensiona la cadena de intercambio cuando cambia el tamaño de la ventana
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return Ok(());
        }

        // Liberar referencia al back buffer antes de redimensionar
        self.back_buffer = None;

        unsafe {
            self.swap_chain
                .ResizeBuffers(2, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, 0)?;
            let new_bb = self.swap_chain.GetBuffer(0)?;
            self.back_buffer = Some(new_bb);
        }

        self.width = width;
        self.height = height;
        self.buffer.resize((width * height) as usize, 0);
        Ok(())
    }

    /// Dimensiones actuales del búfer
    #[cfg(test)]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Obtiene el búfer de píxeles mutable para renderizar el fotograma actual
    #[inline]
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        &mut self.buffer
    }

    /// Transfiere el búfer a VRAM mediante DMA y presenta el fotograma por hardware (DirectFlip)
    pub fn present(&mut self) -> Result<()> {
        let row_pitch = self.width * 4;
        if let Some(back_buffer) = &self.back_buffer {
            unsafe {
                self.context.UpdateSubresource(
                    back_buffer,
                    0,
                    None,
                    self.buffer.as_ptr() as *const _,
                    row_pitch,
                    0,
                );
                self.swap_chain.Present(1, 0).ok()?;
            }
        }
        Ok(())
    }
}
