//! The Classic engine's data: a small JSON reader, the rules data (`data/rules/`) with each number's allowed
//! range, and setting packs, which name, tune and (later) draw and voice the generic ids.
//!
//! The simulation builds its typed rules from a `RulesTable`; nothing here runs during a tick. Maps stay ordered
//! (`BTreeMap`) so anything derived from this data is the same on every machine.

#![deny(clippy::float_arithmetic, clippy::disallowed_types)]

pub mod json;
pub mod pack;
pub mod rules;

pub use pack::{Faction, Pack};
pub use rules::{Entry, Module, Number, RulesTable};
