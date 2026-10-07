//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and, later, the web.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching, a pixel font) that the 3D engine can share
//! once it needs it. `art`, `scene` and `hud` know about the Classic engine: a setting pack's art, the tile map and
//! its entities, and the production rail. The renderer only reads the game; the HUD changes it only by sending the
//! same commands any player sends.

pub mod art;
pub mod hud;
pub mod platform;
pub mod scene;

pub use art::Art;
pub use hud::{Hud, View};
pub use scene::{Camera, Scene};
