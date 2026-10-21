//! Pipeline de quads: una instancia por primitiva (ver `shaders/quad.hlsl`). Los shaders SM 4.0 van
//! precompilados en el binario (`tools/compile-shaders.ps1`): compilar Axon no exige el Windows SDK.

use super::GpuDevice;
use windows::core::{s, Result};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

static VS_BYTECODE: &[u8] = include_bytes!("shaders/quad_vs.cso");
static PS_BYTECODE: &[u8] = include_bytes!("shaders/quad_ps.cso");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    Solid = 0,
    RoundedFill = 1,
    RoundedStroke = 2,
    Glyph = 3,
    Image = 4,
    CardCorner = 5,
}

/// Una primitiva (32 bytes, un elemento por instancia del buffer de vértices; `IASetVertexBuffers` con stride 32).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// x, y, ancho, alto en píxeles del destino (x e y pueden ser negativos)
    pub rect: [i16; 4],
    /// x0, y0, x1, y1 absolutos, límite superior exclusivo, ya recortados al búfer (no negativos)
    pub clip: [u16; 4],
    /// 0xAARRGGBB. `Solid` ignora el alfa (la CPU escribe XRGB opaco); en `CardCorner` es `outer`
    pub color: u32,
    /// Origen del glifo en el atlas (`Glyph`); 0 en el resto
    pub uv: [u16; 2],
    /// bits 0..3 = `Kind`, 8..15 = radio (px), 16..23 = esquina de `CardCorner`
    /// (0 arriba-izq., 1 arriba-der., 2 abajo-izq., 3 abajo-der.), 0 en el resto.
    /// El contorno mide siempre 1 px (`Canvas::rounded_stroke`): no tiene campo de grosor.
    pub kind_radius: u32,
    /// Relleno hasta 32 bytes (los campos suman 28); el input layout no lo lee. Siempre 0
    pub _reserved: u32,
}

impl Instance {
    pub fn new(
        kind: Kind,
        rect: [i16; 4],
        clip: [u16; 4],
        color: u32,
        uv: [u16; 2],
        radius: u8,
        extra: u8,
    ) -> Self {
        Self {
            rect,
            clip,
            color,
            uv,
            kind_radius: kind as u32 | (radius as u32) << 8 | (extra as u32) << 16,
            _reserved: 0,
        }
    }
}

pub struct Pipeline {
    pub vs: ID3D11VertexShader,
    pub ps: ID3D11PixelShader,
    pub layout: ID3D11InputLayout,
    pub blend: ID3D11BlendState,
    /// Punto + clamp. Los shaders leen con `Load` (sin filtrar); queda enlazado por si un tipo futuro muestrea
    pub sampler: ID3D11SamplerState,
    pub raster: ID3D11RasterizerState,
    /// `float2` con el tamaño del destino (16 bytes); se actualiza con `UpdateSubresource`
    pub constants: ID3D11Buffer,
}

impl Pipeline {
    pub fn new(dev: &GpuDevice) -> Result<Self> {
        let d = &dev.device;
        unsafe {
            let mut vs = None;
            d.CreateVertexShader(VS_BYTECODE, None, Some(&mut vs))?;
            let mut ps = None;
            d.CreatePixelShader(PS_BYTECODE, None, Some(&mut ps))?;

            let elem = |name, format| D3D11_INPUT_ELEMENT_DESC {
                SemanticName: name,
                SemanticIndex: 0,
                Format: format,
                InputSlot: 0,
                AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
                InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
                InstanceDataStepRate: 1,
            };
            let elements = [
                elem(s!("RECT"), DXGI_FORMAT_R16G16B16A16_SINT),
                elem(s!("CLIP"), DXGI_FORMAT_R16G16B16A16_UINT),
                elem(s!("COLOR"), DXGI_FORMAT_R32_UINT),
                elem(s!("UV"), DXGI_FORMAT_R16G16_UINT),
                elem(s!("KINDRADIUS"), DXGI_FORMAT_R32_UINT),
            ];
            let mut layout = None;
            d.CreateInputLayout(&elements, VS_BYTECODE, Some(&mut layout))?;

            // SrcAlpha/InvSrcAlpha en RGB; el alfa del destino converge a 1 (src + dst·(1-src))
            let mut blend_desc = D3D11_BLEND_DESC::default();
            blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut blend = None;
            d.CreateBlendState(&blend_desc, Some(&mut blend))?;

            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                MaxLOD: f32::MAX,
                ..Default::default()
            };
            let mut sampler = None;
            d.CreateSamplerState(&sampler_desc, Some(&mut sampler))?;

            let raster_desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            };
            let mut raster = None;
            d.CreateRasterizerState(&raster_desc, Some(&mut raster))?;

            let cb_desc = D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            d.CreateBuffer(&cb_desc, None, Some(&mut constants))?;

            let missing = || windows::core::Error::from(windows::Win32::Foundation::E_FAIL);
            Ok(Pipeline {
                vs: vs.ok_or_else(missing)?,
                ps: ps.ok_or_else(missing)?,
                layout: layout.ok_or_else(missing)?,
                blend: blend.ok_or_else(missing)?,
                sampler: sampler.ok_or_else(missing)?,
                raster: raster.ok_or_else(missing)?,
                constants: constants.ok_or_else(missing)?,
            })
        }
    }
}
