//! The hazard (rules-world.md, section 7): a neutral creature, owned by no player, that travels under open ground,
//! hunts the noisiest unit near it, surfaces under it and swallows every ground unit on that tile. Rock is safe:
//! it never leaves open ground.
//!
//! It is off unless a setting pack turns the `hazard` feature on (the module's `on` number). With it off nothing
//! here runs and the state hash is what it was before the module existed.
//!
//! Each tick, after movement and before the economy:
//! 1. Every ground unit on open ground adds the noise it made to its decaying `noise` (moving: its kind's `noise`
//!    number; mining: `mining_noise`; firing: `firing_noise` more).
//! 2. One spawns, submerged, when fewer than `max` exist, from `first_tick` on and `respawn_ticks` after one left,
//!    on a random open tile at least `spawn_clearance` tiles from every building.
//! 3. Each, in id order, keeps or drops its target, scans for a new one every `scan_every_ticks`, moves towards it
//!    (or wanders), and surfaces to swallow it when within a tile.
//!
//! Departures from the design, for now: it can't be shot or driven off (no health), the map can't set its count or
//! timing (the module's numbers do), and harvesters don't steer clear of it.

use std::collections::VecDeque;

use rts_core::hash::{Canon, CanonHasher};
use rts_core::imath::isqrt;
use rts_core::rng::random_int;

use crate::map::{MapData, TILE, Terrain, Tile};
use crate::path::Pathfinder;
use crate::units::{HazardRules, Rules};
use crate::world::{Entity, Event, GameState, Task, centre};

/// One hazard. It takes its id from the same counter as entities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hazard {
    pub id: u32,
    /// Centre, in sub-tile units.
    pub x: i64,
    pub y: i64,
    pub path: VecDeque<Tile>,
    /// The unit it is hunting.
    pub target: Option<u32>,
    /// Ticks left above ground; 0 while submerged.
    pub surfaced: u32,
    /// Units swallowed so far.
    pub eaten: u32,
    /// Ticks left before a full hazard is gone; `None` while it hunts.
    pub leaving: Option<u32>,
}

impl Hazard {
    pub fn tile(&self) -> Tile {
        Tile { x: self.x.div_euclid(TILE) as i32, y: self.y.div_euclid(TILE) as i32 }
    }
}

impl Canon for Hazard {
    fn canon(&self, w: &mut CanonHasher) {
        w.object()
            .field("eaten", &self.eaten)
            .field("id", &self.id)
            .opt("leaving", self.leaving.as_ref())
            .array("path", &self.path)
            .field("surfaced", &self.surfaced)
            .opt("target", self.target.as_ref())
            .field("x", &self.x)
            .field("y", &self.y)
            .end();
    }
}

/// The hazards in play and when the next may come.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hazards {
    /// Sorted by id.
    pub list: Vec<Hazard>,
    /// The first tick another may spawn.
    pub next_spawn: u32,
}

impl Canon for Hazards {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().array("list", &self.list).field("nextSpawn", &self.next_spawn).end();
    }
}

/// Why a hazard left the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftReason {
    /// It ate its fill.
    Full,
}

impl LeftReason {
    pub fn id(self) -> &'static str {
        match self {
            LeftReason::Full => "full",
        }
    }
}

/// The ground the hazard can travel: open tiles only, so rock and cliffs are walls to it.
pub fn pathfinder(map: &MapData) -> Pathfinder {
    Pathfinder::with_ground(map, |t| t == Terrain::Open)
}

/// The hazard phase of one tick. `pf` is the hazard's own pathfinder (`pathfinder`).
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let (Some(h), Some(mut hz)) = (&rules.hazard, state.hazards.take()) else { return };
    listen(map, state, rules, h);
    spawn(map, state, rules, h, &mut hz, events);
    let mut kept = Vec::with_capacity(hz.list.len());
    for mut z in std::mem::take(&mut hz.list) {
        if let Some(left) = z.leaving {
            if left <= 1 {
                hz.next_spawn = state.tick + h.respawn_ticks;
                let reason = LeftReason::Full;
                events.push(Event::HazardLeft { tick: state.tick, hazard: z.id, reason, x: z.x, y: z.y });
                continue;
            }
            z.leaving = Some(left - 1);
        } else if z.surfaced > 0 {
            z.surfaced -= 1;
            if z.surfaced == 0 && h.appetite > 0 && z.eaten >= h.appetite {
                z.leaving = Some(h.leave_ticks);
                z.path.clear();
                z.target = None;
            }
        } else {
            hunt(map, pf, state, rules, h, &mut z, events);
        }
        kept.push(z);
    }
    hz.list = kept;
    state.hazards = Some(hz);
}

/// Whether a ground unit stands where the hazard could take it: on open ground, not a building, an aircraft or a
/// unit being carried.
fn exposed(map: &MapData, rules: &Rules, e: &Entity) -> bool {
    let t = e.tile();
    crate::world::on_ground(rules, e) && map.in_bounds(t.x, t.y) && map.terrain[map.index(t.x, t.y)] == Terrain::Open
}

/// Step 1: every unit's noise decays by half each scan window and grows by what it makes this tick.
fn listen(map: &MapData, state: &mut GameState, rules: &Rules, h: &HazardRules) {
    let halve = state.tick.is_multiple_of(h.scan_every);
    for e in &mut state.entities {
        let k = rules.kind(e.kind);
        if k.building {
            continue;
        }
        if halve {
            e.noise /= 2;
        }
        if !exposed(map, rules, e) {
            continue;
        }
        let mut made = if e.path.is_empty() { 0 } else { k.noise };
        if e.task == Some(Task::Mining) {
            made += h.mining_noise;
        }
        if let Some(w) = k.weapon
            && e.reload > 0
            && e.reload == rules.weapon(w).reload
        {
            made += h.firing_noise;
        }
        e.noise = (e.noise + made).min(255);
    }
}

/// Step 2: a new hazard when there is room for one and its time has come.
fn spawn(map: &MapData, state: &mut GameState, rules: &Rules, h: &HazardRules, hz: &mut Hazards, ev: &mut Vec<Event>) {
    if state.tick < hz.next_spawn || hz.list.len() >= h.max as usize {
        return;
    }
    // A few random tries for an open tile far enough from every building; none found means try again next tick.
    let far = |t: Tile| {
        state.entities.iter().filter(|e| rules.kind(e.kind).building).all(|b| {
            let (k, bt) = (rules.kind(b.kind), b.tile());
            // Tiles from `t` to the nearest tile of the footprint, diagonals counting as one.
            let dx = (bt.x - t.x).max(t.x - (bt.x + k.width - 1)).max(0);
            let dy = (bt.y - t.y).max(t.y - (bt.y + k.height - 1)).max(0);
            dx.max(dy) >= h.spawn_clearance
        })
    };
    let n = (map.width * map.height) as u32;
    for _ in 0..16 {
        let t = map.tile_at(random_int(&mut state.rng, n) as usize);
        if map.terrain[map.index(t.x, t.y)] != Terrain::Open || !far(t) {
            continue;
        }
        let z = submerged(state.next_id, t);
        state.next_id += 1;
        ev.push(Event::HazardSpawned { tick: state.tick, hazard: z.id, x: z.x, y: z.y });
        hz.list.push(z);
        return;
    }
}

/// A new submerged hazard at the centre of tile `t`, if the hazard is on.
pub fn place(state: &mut GameState, t: Tile) -> Option<u32> {
    let hz = state.hazards.as_mut()?;
    let id = state.next_id;
    state.next_id += 1;
    hz.list.push(submerged(id, t));
    Some(id)
}

fn submerged(id: u32, t: Tile) -> Hazard {
    let (x, y) = (centre(t.x), centre(t.y));
    Hazard { id, x, y, path: VecDeque::new(), target: None, surfaced: 0, eaten: 0, leaving: None }
}

fn dist2(z: &Hazard, e: &Entity) -> i64 {
    (e.x - z.x).pow(2) + (e.y - z.y).pow(2)
}

/// Step 3 for a submerged hazard: pick or keep a target, move, and strike when close.
fn hunt(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    h: &HazardRules,
    z: &mut Hazard,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let here = z.tile();
    let reachable = |e: &Entity| {
        let t = e.tile();
        exposed(map, rules, e) && pf.connected((here.x, here.y), (t.x, t.y))
    };
    // Keep the target while it is on open ground it can reach and not too far.
    let give_up = (h.give_up_range * TILE).pow(2);
    let kept = z.target.and_then(|id| state.entity(id)).filter(|e| reachable(e) && dist2(z, e) <= give_up);
    if kept.is_none() {
        z.target = None;
    }
    // Anything within a tile is taken at once, whatever its noise; otherwise a scan every window.
    let close = state.entities.iter().find(|e| reachable(e) && dist2(z, e) <= TILE * TILE).map(|e| e.id);
    let scan = z.target.is_none() && (tick + z.id).is_multiple_of(h.scan_every);
    if close.is_some() {
        z.target = close;
    } else if scan {
        let range = (h.scan_range * TILE).pow(2);
        z.target = state
            .entities
            .iter()
            .filter(|e| reachable(e) && dist2(z, e) <= range)
            .map(|e| (e.noise * 100 - isqrt(dist2(z, e) as u64) as i64 / 16, e.id))
            .filter(|&(score, _)| score > 0)
            .max_by_key(|&(score, id)| (score, std::cmp::Reverse(id)))
            .map(|(_, id)| id);
        if z.target.is_none() && z.path.is_empty() {
            wander(state, pf, h, z);
        }
    }
    // Head for the target's tile, finding the way again every scan window.
    if let Some(e) = z.target.and_then(|id| state.entity(id)) {
        let goal = e.tile();
        if z.path.back() != Some(&goal) && (z.path.is_empty() || (tick + z.id).is_multiple_of(h.scan_every)) {
            z.path = route(pf, z, goal);
        }
    }
    advance(z, h.speed);
    if let Some(i) = z.target.and_then(|id| state.entities.binary_search_by_key(&id, |e| e.id).ok())
        && dist2(z, &state.entities[i]) <= TILE * TILE
    {
        strike(map, state, rules, h, z, i, events);
    }
}

/// No target: a random open tile it can reach within `wander_range`.
fn wander(state: &mut GameState, pf: &mut Pathfinder, h: &HazardRules, z: &mut Hazard) {
    let here = z.tile();
    let side = (2 * h.wander_range + 1) as u32;
    let dx = random_int(&mut state.rng, side) as i32 - h.wander_range;
    let dy = random_int(&mut state.rng, side) as i32 - h.wander_range;
    let to = Tile { x: here.x + dx, y: here.y + dy };
    if to != here && pf.connected((here.x, here.y), (to.x, to.y)) {
        z.path = route(pf, z, to);
    }
}

/// A path to `goal` that first finishes the step it is on.
fn route(pf: &mut Pathfinder, z: &Hazard, goal: Tile) -> VecDeque<Tile> {
    let here = z.tile();
    let on_centre = z.x == centre(here.x) && z.y == centre(here.y);
    let from = if on_centre { here } else { z.path.front().copied().unwrap_or(here) };
    let mut path: VecDeque<Tile> = pf.find(from.x, from.y, goal.x, goal.y).map(|p| p.tiles.into()).unwrap_or_default();
    if !on_centre {
        path.push_front(from);
    }
    path
}

/// Move along the path by `speed`, carrying leftover budget past centres. Nothing blocks it underground.
fn advance(z: &mut Hazard, speed: i64) {
    let mut budget = speed;
    while budget > 0 {
        let Some(&next) = z.path.front() else { break };
        let (dx, dy) = (centre(next.x) - z.x, centre(next.y) - z.y);
        let dist = isqrt((dx * dx + dy * dy) as u64) as i64;
        if dist <= budget {
            z.x += dx;
            z.y += dy;
            budget -= dist;
            z.path.pop_front();
        } else {
            z.x += dx * budget / dist;
            z.y += dy * budget / dist;
            budget = 0;
        }
    }
}

/// Surface under the target and swallow every ground unit on its tile, of any owner, in id order.
fn strike(
    map: &MapData,
    state: &mut GameState,
    rules: &Rules,
    h: &HazardRules,
    z: &mut Hazard,
    target: usize,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let at = state.entities[target].tile();
    let (x, y) = (centre(at.x), centre(at.y));
    (z.x, z.y) = (x, y);
    z.surfaced = h.surface_ticks;
    z.path.clear();
    z.target = None;
    events.push(Event::HazardSurfaced { tick, hazard: z.id, x, y });
    state.entities.retain(|e| {
        let eaten = exposed(map, rules, e) && e.tile() == at;
        if eaten {
            z.eaten += 1;
            let (unit, kind, owner) = (e.id, e.kind, e.owner);
            events.push(Event::HazardAte { tick, hazard: z.id, unit, kind, owner, x: e.x, y: e.y });
        }
        !eaten
    });
    // Anyone after a swallowed unit drops it in the next combat phase, as for one destroyed.
}
