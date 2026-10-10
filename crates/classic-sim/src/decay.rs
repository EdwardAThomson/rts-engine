//! Concrete slabs and decay (rules-base-building-power.md, "Foundations and decay"; the `decay` module). Off unless
//! a setting pack turns it on.
//!
//! A slab is tile state, not an entity: each tile records which player's slab lies on it, if any. Slabs come out
//! of the yard like any building (kinds with the `slab` role) and are laid on rock where nothing stands, over the
//! player's own slab or bare rock but never another player's. A player's slabs extend the area they may build in,
//! as their buildings do. Each building records `foundation`, how many of its footprint tiles held its owner's slab
//! when it was placed. While the module is on, every `every_ticks` (staggered by id) a building wears down by
//! `max(1, max_health / step_div)` towards its floor, `max_health * (floor_percent * tiles + (100 - floor_percent) *
//! foundation) / (100 * tiles)`: a fully slabbed building never decays, a bare one goes down to `floor_percent`.
//! Decay never destroys anything, skips walls and buildings being repaired or sold, and is reported as `decayed`
//! rather than a hit, so nothing answers it as an attack. A destroyed building takes the slab under it with it; a
//! sold one leaves it, and a captured one's changes owner. The idea is the 1992 original's, by way of OpenRA's
//! floor reading; the rules and numbers are ours.

use rts_core::hash::{Canon, CanonHasher};

use crate::map::MapData;
use crate::units::{Kind, Rules};
use crate::world::{Entity, Event, GameState};

/// Whose slab lies on each tile, in row order: the owner's id + 1, or 0 for none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slabs {
    /// The map's width, to find a tile; not hashed.
    pub width: i32,
    pub tiles: Vec<u32>,
}

impl Slabs {
    /// The tiles of a footprint, in row order.
    fn under(&self, x: i32, y: i32, w: i32, h: i32) -> impl Iterator<Item = usize> + use<> {
        let width = self.width;
        (y..y + h).flat_map(move |ty| (x..x + w).map(move |tx| (ty * width + tx) as usize))
    }
}

impl Canon for Slabs {
    fn canon(&self, w: &mut CanonHasher) {
        w.array(&self.tiles);
    }
}

/// Whether slabs can be ordered at all: only while the decay module is on.
pub fn on(rules: &Rules) -> bool {
    rules.decay.is_some()
}

/// The player whose slab is on tile (x, y), if any.
pub fn owner(state: &GameState, x: i32, y: i32) -> Option<u32> {
    let s = state.slabs.as_ref()?;
    let o = *s.tiles.get(s.under(x, y, 1, 1).next()?)?;
    (o > 0).then(|| o - 1)
}

/// Lay `player`'s slab of kind `kind` with its top-left tile at (x, y); returns how many tiles were new. The caller
/// has checked the placement.
pub fn lay(map: &MapData, state: &mut GameState, rules: &Rules, player: u32, kind: Kind, x: i32, y: i32) -> u32 {
    let k = rules.kind(kind);
    let slabs = state.slabs.get_or_insert_with(|| Slabs { width: map.width, tiles: vec![0; map.terrain.len()] });
    let mut laid = 0;
    for i in slabs.under(x, y, k.width, k.height) {
        if slabs.tiles[i] == 0 {
            slabs.tiles[i] = player + 1;
            laid += 1;
        }
    }
    laid
}

/// How many of a footprint's tiles hold `player`'s slab.
pub fn foundation(state: &GameState, rules: &Rules, player: u32, kind: Kind, x: i32, y: i32) -> u32 {
    let k = rules.kind(kind);
    let Some(s) = &state.slabs else { return 0 };
    s.under(x, y, k.width, k.height).filter(|&i| s.tiles[i] == player + 1).count() as u32
}

/// Clear the slab under a destroyed building's footprint.
pub fn destroyed(state: &mut GameState, rules: &Rules, e: &Entity) {
    let (k, t) = (rules.kind(e.kind), e.tile());
    let Some(slabs) = state.slabs.as_mut() else { return };
    for i in slabs.under(t.x, t.y, k.width, k.height) {
        slabs.tiles[i] = 0;
    }
}

/// Hand the slab under a captured building to its new owner.
pub fn captured(state: &mut GameState, rules: &Rules, i: usize) {
    let e = &state.entities[i];
    let (k, t, to) = (rules.kind(e.kind), e.tile(), e.owner);
    let Some(slabs) = state.slabs.as_mut() else { return };
    for i in slabs.under(t.x, t.y, k.width, k.height) {
        if slabs.tiles[i] > 0 {
            slabs.tiles[i] = to + 1;
        }
    }
}

/// The health a building decays down to, but never below.
pub fn floor(rules: &Rules, e: &Entity) -> i64 {
    let Some(d) = &rules.decay else { return 0 };
    let k = rules.kind(e.kind);
    let n = (k.width * k.height).max(1) as i64;
    let f = (e.foundation as i64).min(n);
    k.max_health * (d.floor_percent * n + (100 - d.floor_percent) * f) / (100 * n)
}

/// One tick of decay, buildings in id order.
pub fn tick(state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let Some(d) = &rules.decay else { return };
    let tick = state.tick;
    for i in 0..state.entities.len() {
        let e = &state.entities[i];
        let k = rules.kind(e.kind);
        if !k.building || k.wall || e.repairing || e.selling > 0 || !(tick + e.id).is_multiple_of(d.every) {
            continue;
        }
        let floor = floor(rules, e);
        if e.health <= floor {
            continue;
        }
        let damage = (k.max_health / d.step_div).max(1).min(e.health - floor);
        let e = &mut state.entities[i];
        e.health -= damage;
        events.push(Event::Decayed { tick, entity: e.id, owner: e.owner, damage, health: e.health });
    }
}
