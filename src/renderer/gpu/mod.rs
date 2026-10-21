//! Backend Direct3D 11 del renderer (etapa 2 de la migración a GPU).

mod atlas;
mod device;
mod pipeline;
mod renderer;
mod surface;

#[cfg(test)]
pub use atlas::ShelfPacker;
pub use atlas::{AtlasError, AtlasRect, GlyphAtlas, ImageTexture};
#[cfg(test)]
pub use device::{max_texture_dim_for, requested_driver_from};
pub use device::{DriverKind, GpuDevice};
pub use pipeline::{Instance, Kind, Pipeline};
pub use renderer::GpuRenderer;
#[cfg(test)]
pub use renderer::{encode, Batch, EncodeStop};
pub use surface::{GpuSurface, Presented};
