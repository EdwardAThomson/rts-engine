//! Resource blooms (rules-world.md, section 6; the `blooms` module): hidden mounds on open ground that burst into a
//! new field. Off unless a setting pack turns it on, and while on they replace the slow regrowth beside fields.
//!
//! A map marks bloom points (`*`), and a bloom starts on each. One bursts when a ground unit drives onto its tile,
//! when a shell or rocket bursts on its tile, or when it reaches `max_age_ticks`. The burst hurts every ground unit
//! on its tile and the 8 round it by `burst_damage`, and adds resource to every open tile in the disc of `radius`
//! tiles (the same disc as sight): `centre - per_tile * d`, where `d` is the larger of the x and y offsets, capped
//! at a full tile. After a burst a new bloom
//! is seeded after a random `reseed_min_ticks` to `reseed_max_ticks`, on a random open tile within `reseed_range`
//! tiles of the old point with no resource, unit, building or bloom on it; if 20 tries fail it tries again after
//! `retry_ticks`. Harvesters that had run out of fields look again. Players see a bloom only where they see its
//! tile.
//!
//! Departures from the design: a bloom is not an entity, so it can't be targeted, only caught in a burst; and the
//! burst's damage is not an attack (no `hit` event, no attacker), reported in `bloom_burst` instead.

use rts_core::hash::{Canon, CanonHasher};
use rts_core::rng::random_int;

use crate::map::{MapData, RESOURCE_PER_TILE, TILE, Terrain, Tile};
use crate::units::Rules;
use crate::world::{Event, GameState, Order, Task, on_ground};

/// One bloom on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bloom {
    pub tile: Tile,
    /// The tick it appeared.
    pub born: u32,
    /// Caught in a burst this tick: it bursts in the blooms phase.
    pub shot: bool,
}

impl Canon for Bloom {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().field("born", &self.born).opt("shot", self.shot.then_some(&true)).field("tile", &self.tile).end();
    }
}

/// A bloom to come: near which point, and on which tick it tries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reseed {
    pub near: Tile,
    pub at: u32,
}

impl Canon for Reseed {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().field("at", &self.at).field("near", &self.near).end();
    }
}

/// The blooms on the map and those still to come, in the order they were made.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Blooms {
    pub list: Vec<Bloom>,
    pub reseeds: Vec<Reseed>,
}

impl Canon for Blooms {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().array("list", &self.list).array("reseeds", &self.reseeds).end();
    }
}

/// The blooms a game starts with: one on each of the map's bloom points, or `None` while the module is off.
pub fn start(map: &MapData, rules: &Rules) -> Option<Blooms> {
    rules.blooms.as_ref()?;
    Some(Blooms {
        list: map.blooms.iter().map(|&tile| Bloom { tile, born: 0, shot: false }).collect(),
        reseeds: vec![],
    })
}

/// A ground burst at (x, y), in sub-tile units: a bloom on that tile goes up this tick.
pub fn shot(state: &mut GameState, x: i64, y: i64) {
    let Some(b) = state.blooms.as_mut() else { return };
    let t = Tile { x: x.div_euclid(TILE) as i32, y: y.div_euclid(TILE) as i32 };
    for bloom in b.list.iter_mut().filter(|b| b.tile == t) {
        bloom.shot = true;
    }
}

/// Whether a bloom stands on tile (x, y).
pub fn at(state: &GameState, x: i32, y: i32) -> bool {
    state.blooms.as_ref().is_some_and(|b| b.list.iter().any(|b| b.tile == Tile { x, y }))
}

/// One tick: blooms that are driven onto, caught in a burst or too old burst, in the order they were made; then
/// any reseed that is due tries to place a new one.
pub fn tick(map: &MapData, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let (Some(r), Some(mut blooms)) = (&rules.blooms, state.blooms.take()) else { return };
    let tick = state.tick;
    let mut kept = Vec::with_capacity(blooms.list.len());
    for b in std::mem::take(&mut blooms.list) {
        let trodden =
            state.entities.iter().any(|e| on_ground(rules, e) && e.tile() == b.tile && e.carried_by.is_none());
        if !(trodden || b.shot || tick.saturating_sub(b.born) >= r.max_age) {
            kept.push(b);
            continue;
        }
        let added = burst(map, state, rules, b.tile, events);
        events.push(Event::BloomBurst { tick, x: b.tile.x, y: b.tile.y, added });
        let wait = r.reseed_min + random_int(&mut state.rng, r.reseed_max.saturating_sub(r.reseed_min) + 1);
        blooms.reseeds.push(Reseed { near: b.tile, at: tick + wait });
    }
    blooms.list = kept;
    let mut waiting = Vec::with_capacity(blooms.reseeds.len());
    for s in std::mem::take(&mut blooms.reseeds) {
        if s.at > tick {
            waiting.push(s);
            continue;
        }
        match seed_spot(map, state, rules, &blooms, s.near) {
            Some(tile) => {
                blooms.list.push(Bloom { tile, born: tick, shot: false });
                events.push(Event::BloomSeeded { tick, x: tile.x, y: tile.y });
            }
            None => waiting.push(Reseed { near: s.near, at: tick + r.retry }),
        }
    }
    blooms.reseeds = waiting;
    state.blooms = Some(blooms);
}

/// Spread resource round `at` and hurt the ground units beside it; returns the resource added.
fn burst(map: &MapData, state: &mut GameState, rules: &Rules, at: Tile, events: &mut Vec<Event>) -> i64 {
    let r = rules.blooms.as_ref().expect("blooms are on");
    let mut added = 0;
    for y in at.y - r.radius..=at.y + r.radius {
        for x in at.x - r.radius..=at.x + r.radius {
            let (dx, dy) = (x - at.x, y - at.y);
            if dx * dx + dy * dy > r.radius * r.radius + r.radius
                || !map.in_bounds(x, y)
                || map.terrain[map.index(x, y)] != Terrain::Open
            {
                continue;
            }
            let d = dx.abs().max(dy.abs()) as i64;
            let i = map.index(x, y);
            let now = (state.resource[i] + (r.centre - r.per_tile * d).max(0)).min(RESOURCE_PER_TILE);
            added += now - state.resource[i];
            state.resource[i] = now;
        }
    }
    let tick = state.tick;
    for e in state.entities.iter_mut() {
        let t = e.tile();
        let near = (t.x - at.x).abs() <= 1 && (t.y - at.y).abs() <= 1;
        let k = rules.kind(e.kind);
        if near && !k.building && !k.air && e.carried_by.is_none() && r.damage > 0 {
            e.health -= r.damage;
            events.push(Event::BloomHurt { tick, unit: e.id, damage: r.damage, health: e.health });
        }
        // Harvesters that ran out of fields look again.
        if e.task == Some(Task::Stuck) && e.order == Order::Harvest {
            e.task = Some(Task::Seek);
        }
    }
    added
}

/// A random open tile within `reseed_range` of `near` that is clear for a new bloom, in at most 20 tries.
fn seed_spot(map: &MapData, state: &mut GameState, rules: &Rules, blooms: &Blooms, near: Tile) -> Option<Tile> {
    let r = rules.blooms.as_ref().expect("blooms are on");
    let span = (2 * r.reseed_range + 1) as u32;
    for _ in 0..20 {
        let x = near.x - r.reseed_range + random_int(&mut state.rng, span) as i32;
        let y = near.y - r.reseed_range + random_int(&mut state.rng, span) as i32;
        if !map.in_bounds(x, y) || map.terrain[map.index(x, y)] != Terrain::Open || state.resource[map.index(x, y)] > 0
        {
            continue;
        }
        let t = Tile { x, y };
        let taken = state.entities.iter().any(|e| {
            let k = rules.kind(e.kind);
            let et = e.tile();
            if k.building {
                (et.x..et.x + k.width).contains(&x) && (et.y..et.y + k.height).contains(&y)
            } else {
                !k.air && et == t
            }
        });
        if !taken && !blooms.list.iter().any(|b| b.tile == t) {
            return Some(t);
        }
    }
    None
}
