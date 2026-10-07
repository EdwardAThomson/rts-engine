//! Headless tools for the Classic engine. `scene` builds the seeded bench map and orders, shared by the bench
//! and the golden tests, so both exercise exactly the same game. `setting` finds and loads setting packs.

pub mod scene;
pub mod setting;
