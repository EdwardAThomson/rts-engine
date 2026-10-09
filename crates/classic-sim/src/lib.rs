//! The Classic engine's simulation: 1990s-style base-building RTS rules on a 2D tile grid.
//!
//! Everything here is deterministic: integer maths, the one seeded generator held in the game state, entities in
//! id order, and no clock, floating point or iteration over unordered collections. Effects (rendering, audio,
//! logs) read `events` and never feed back into the state.

#![deny(clippy::float_arithmetic, clippy::disallowed_types)]

pub mod combat;
pub mod game;
pub mod hazard;
pub mod map;
pub mod movement;
pub mod path;
pub mod placement;
pub mod power;
pub mod production;
pub mod units;
pub mod vision;
pub mod world;

pub use game::{Game, GameOptions, hash_state};
pub use hazard::{Hazard, Hazards, LeftReason};
pub use map::{MapData, Terrain, Tile, parse_map};
pub use placement::PlaceError;
pub use power::Power;
pub use production::{EntryState, ProduceError, QueueEntry};
pub use units::{FogRules, HazardRules, Kind, Rules, WeaponId};
pub use vision::{Ghost, TileView, Vision};
pub use world::{Command, CommandOrder, Entity, Event, GameState, MoveEnd, Order, Task};
