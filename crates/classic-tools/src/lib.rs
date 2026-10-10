//! Headless tools for the Classic engine. `scene` builds the seeded bench map and orders, shared by the bench
//! and the golden tests, so both exercise exactly the same game. `setting` finds and loads setting packs. `art`
//! draws the generic pack's placeholder art, `sound` synthesises its placeholder sounds and `music` its placeholder
//! music.

pub mod art;
pub mod music;
pub mod scene;
pub mod setting;
pub mod sound;
