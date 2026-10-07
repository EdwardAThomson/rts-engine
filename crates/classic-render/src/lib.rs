//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and, later, the web.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching) that the 3D engine can share once it needs
//! it. `art` and `scene` know about the Classic engine: a setting pack's art, the tile map and its entities. The
//! renderer only reads the game; nothing it does feeds back into the state.

pub mod art;
pub mod platform;
pub mod scene;

pub use art::Art;
pub use scene::{Camera, Scene};
