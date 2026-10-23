//! Backend Direct3D 11 del renderer (etapa 2 de la migración a GPU).

mod atlas;
mod device;
mod pipeline;
mod renderer;
mod surface;

#[cfg(test)]
pub(crate) use atlas::ShelfPacker;
pub(super) use atlas::{AtlasError, AtlasRect, GlyphAtlas, ImageTexture};
#[cfg(test)]
pub(crate) use device::{max_texture_dim_for, requested_driver_from};
pub(super) use device::{DriverKind, GpuDevice};
pub(super) use pipeline::{Instance, Kind, Pipeline};
pub(super) use renderer::GpuRenderer;
#[cfg(test)]
pub(crate) use renderer::{encode, Batch, EncodeStop};
pub(crate) use surface::{GpuSurface, Presented};
