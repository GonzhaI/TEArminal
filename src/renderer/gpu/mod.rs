//! Backend Direct3D 11 del renderer (etapa 2 de la migración a GPU).

// Nada en producción usa este módulo hasta que la GPU sea el camino por defecto: se quita en la tarea 19.
#![allow(dead_code, unused_imports)]

mod atlas;
mod device;
mod pipeline;
mod renderer;

pub use atlas::{AtlasError, AtlasRect, GlyphAtlas, ImageTexture, ShelfPacker};
pub use device::{
    max_texture_dim_for, requested_driver, requested_driver_from, DriverKind, GpuDevice,
};
pub use pipeline::{Instance, Kind, Pipeline};
pub use renderer::{encode, Batch, EncodeStop, GpuRenderer};
