//! The genre-neutral platform layer: the GPU, textures, a sprite batcher and a pixel font. Nothing here knows about
//! tiles, units or setting packs, so it can move to a crate shared with the 3D engine once that engine needs it.

pub mod batch;
pub mod gpu;
pub mod text;

pub use batch::{Rect, SpriteBatch, TexId};
pub use gpu::Gpu;
pub use text::Font;
