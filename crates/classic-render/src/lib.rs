//! The Classic engine's renderer, on wgpu: one renderer for the desktop player and the browser.
//!
//! `platform` is the genre-neutral layer (GPU, textures, sprite batching, a pixel font, the sound mixer and device,
//! clock, files, the browser page): the `rts-platform` crate from the shared `rts-core` repository, which the 3D
//! engine's renderer uses too. `art`, `tiles`, `scene`, `sound` and `hud` know about the Classic engine: a setting pack's art and
//! sounds, the tile map, its entities and its events, and the production rail; `web` fetches a game's files in the
//! browser. The renderer and the sound board only read the game; the HUD changes it only by sending the same commands
//! any player sends.

pub mod art;
pub mod effects;
pub mod feed;
pub mod hud;
pub mod menu;
pub mod scene;
pub mod skin;
pub mod sound;
pub mod studio;
pub mod theme;
pub mod tiles;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use rts_platform as platform;

pub use art::Art;
pub use hud::{Hud, View};
pub use scene::{Camera, Scene};
pub use skin::Skin;
pub use sound::{Listener, SoundBoard};
