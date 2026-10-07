//! The genre-neutral platform layer: the GPU, textures, a sprite batcher, and sound (a mixer, the sound device and
//! WAV files). Nothing here knows about tiles, units or setting packs, so it can move to a crate shared with the 3D
//! engine once that engine needs it.

pub mod audio;
pub mod batch;
pub mod gpu;
pub mod wav;

#[cfg(feature = "device")]
pub use audio::Speaker;
pub use audio::{Bus, ClipId, Mixer, Played, Sound};
pub use batch::{Rect, SpriteBatch, TexId};
pub use gpu::Gpu;
pub use wav::Clip;
