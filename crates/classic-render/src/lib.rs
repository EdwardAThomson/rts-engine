//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and, later, the web.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching, a pixel font, the sound mixer and device)
//! that the 3D engine can share once it needs it. `art`, `scene`, `sound` and `hud` know about the Classic engine: a
//! setting pack's art and sounds, the tile map, its entities and its events, and the production rail. The renderer
//! and the sound board only read the game; the HUD changes it only by sending the same commands any player sends.

pub mod art;
pub mod hud;
pub mod platform;
pub mod scene;
pub mod sound;

pub use art::Art;
pub use hud::{Hud, View};
pub use scene::{Camera, Scene};
pub use sound::{Listener, SoundBoard};
