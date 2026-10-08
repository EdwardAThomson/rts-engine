//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and the browser.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching, the sound mixer and device, clock, files,
//! the browser page) that the 3D engine can share once it needs it. `art`, `scene` and `sound` know about the
//! Classic engine: a setting pack's art and sounds, the tile map, its entities and its events; `web` fetches a
//! game's files in the browser. The renderer and the sound board only read the game; nothing they do feeds back
//! into the state.

pub mod art;
pub mod platform;
pub mod scene;
pub mod sound;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use art::Art;
pub use scene::{Camera, Scene};
pub use sound::{Listener, SoundBoard};
