//! The genre-neutral platform layer: the GPU, textures and a sprite batcher. Nothing here knows about tiles, units
//! or setting packs, so it can move to a crate shared with the 3D engine once that engine needs it.

pub mod batch;
pub mod gpu;

pub use batch::{Rect, SpriteBatch, TexId};
pub use gpu::Gpu;
