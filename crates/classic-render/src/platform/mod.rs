//! The genre-neutral platform layer: the GPU, textures, a sprite batcher, a pixel font, sound (a mixer, the sound
//! device and WAV files), a clock, file access and (in the browser) the page. Nothing here knows about tiles, units or
//! setting packs, so it can move to a crate shared with the 3D engine once that engine needs it.

pub mod audio;
pub mod batch;
pub mod clock;
pub mod files;
pub mod gpu;
pub mod text;
pub mod wav;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(feature = "device")]
pub use audio::Speaker;
pub use audio::{Bus, ClipId, Mixer, Played, Sound};
pub use batch::{Rect, SpriteBatch, TexId};
pub use clock::Instant;
pub use files::Files;
pub use gpu::Gpu;
pub use text::Font;
pub use wav::Clip;
