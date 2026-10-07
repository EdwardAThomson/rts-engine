//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and the browser.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching, clock, files, the browser page) that the
//! 3D engine can share once it needs it. `art` and `scene` know about the Classic engine: a setting pack's art, the
//! tile map and its entities; `web` fetches a game's files in the browser. The renderer only reads the game;
//! nothing it does feeds back into the state.

pub mod art;
pub mod platform;
pub mod scene;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use art::Art;
pub use scene::{Camera, Scene};
