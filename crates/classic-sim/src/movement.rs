//! Ground movement with collision (rules-movement.md, sections 4 to 6): a tile holds one ground unit, and a unit
//! reserves the next tile of its path before it leaves the centre of the one it is on.
//!
//! **Occupancy is derived, never stored.** A unit holds the tile its centre is on and, while it is between two
//! centres, the tile it is heading to. When its centre crosses into that tile the old one is released, so a unit
//! holds at most two tiles (reserve ahead, release halfway: an idea from an open-source remake, credited in
//! rules-movement.md). The movement phase builds the holder of each tile from the entities in id order, so a
//! lower id that leaves frees its tile for a higher id on the same tick, and the lower id wins a tie.
//!
//! **A unit always finishes the step it is on.** A new path for a unit between centres starts with the tile it is
//! heading to, and stopping it leaves just that tile. So "between centres" always means "on the way to the head
//! of the path".
//!
//! **Blocked units** (our outline after OpenRA's `Move.cs`, with counts after the remake credited there): a
//! building in the way means a new path at once. A unit in the way of a move's last tile, standing still, means the
//! move ends on the nearest free tile near it. Otherwise the unit asks an own blocker that is standing still to step
//! aside, waits `wait_base` plus a random `0..wait_random` ticks, then looks for a way round the units standing
//! still. After `max_repath_fails` failed searches in a row it gives up.
//!
//! **Stepping aside.** A unit asked to yield moves to a free neighbour off the asker's next three path tiles, or,
//! when there is none, any free neighbour (backing off), then carries on with what it was doing. Only own units
//! yield. A unit that is itself waiting yields only to a lower id, so two blocked units never take turns forever.
//! The one exception is a harvester queued next to its dock: it yields to the unit on the dock, which can only
//! leave through it, so a harvester leaving a dock and the next one queued for it never wait on each other.
//! Enemy units never yield, so blocking a dock stays a tactic.

use std::collections::VecDeque;

use rts_core::imath::isqrt;
use rts_core::rng::random_int;

use crate::map::{TILE, Tile};
use crate::path::Pathfinder;
use crate::units::Rules;
use crate::world::{self, Entity, Event, GameState, MoveEnd, Order, Task};

/// Whether the unit's centre is on a tile centre.
pub fn at_centre(e: &Entity) -> bool {
    e.x.rem_euclid(TILE) == TILE / 2 && e.y.rem_euclid(TILE) == TILE / 2
}

/// The tile a unit between two centres is heading to.
fn heading(e: &Entity) -> Option<Tile> {
    if at_centre(e) { None } else { e.path.front().copied() }
}

/// The second tile a unit holds: the one it is heading to, before its centre has crossed into it.
pub fn step_tile(e: &Entity) -> Option<Tile> {
    heading(e).filter(|&t| t != e.tile())
}

/// A path to `goal` that first finishes the step the unit is on.
pub fn route(pf: &mut Pathfinder, e: &Entity, goal: Tile) -> VecDeque<Tile> {
    match heading(e) {
        Some(next) => {
            let mut p = world::path_or_empty(pf, next, goal);
            p.push_front(next);
            p
        }
        None => world::path_or_empty(pf, e.tile(), goal),
    }
}

/// Stop after the step the unit is on.
pub fn halt(e: &mut Entity) {
    let next = heading(e);
    e.path.clear();
    e.path.extend(next);
}

/// Which unit holds each tile.
struct Holders {
    w: i32,
    h: i32,
    by: Vec<Option<u32>>,
}

impl Holders {
    fn build(pf: &Pathfinder, state: &GameState, rules: &Rules) -> Holders {
        let (w, h) = pf.size();
        let mut hs = Holders { w, h, by: vec![None; (w * h) as usize] };
        for e in state.entities.iter().filter(|e| !rules.kind(e.kind).building) {
            hs.hold(e);
        }
        hs
    }

    fn index(&self, t: Tile) -> Option<usize> {
        (t.x >= 0 && t.y >= 0 && t.x < self.w && t.y < self.h).then(|| (t.y * self.w + t.x) as usize)
    }

    fn at(&self, t: Tile) -> Option<u32> {
        self.index(t).and_then(|i| self.by[i])
    }

    fn tiles(e: &Entity) -> impl Iterator<Item = Tile> {
        std::iter::once(e.tile()).chain(step_tile(e))
    }

    /// Mark the unit's tiles. A tile already held (units spawned on top of each other) stays with the first.
    fn hold(&mut self, e: &Entity) {
        for t in Self::tiles(e) {
            if let Some(i) = self.index(t) {
                self.by[i].get_or_insert(e.id);
            }
        }
    }

    fn release(&mut self, e: &Entity) {
        for t in Self::tiles(e) {
            if let Some(i) = self.index(t)
                && self.by[i] == Some(e.id)
            {
                self.by[i] = None;
            }
        }
    }

    /// Free for `id` to enter: open ground with no other unit on it or heading to it.
    fn free(&self, pf: &Pathfinder, t: Tile, id: u32) -> bool {
        pf.passable(t.x, t.y) && self.at(t).is_none_or(|h| h == id)
    }
}

/// Why a unit can't take its next step.
enum Block {
    /// A building or cliff: find a new path at once.
    Static,
    /// Another unit holds the tile.
    Unit(u32),
    /// A diagonal step between two units, which nothing squeezes through.
    Squeeze,
}

fn can_enter(pf: &Pathfinder, hs: &Holders, e: &Entity, next: Tile) -> Result<(), Block> {
    if !pf.passable(next.x, next.y) {
        return Err(Block::Static);
    }
    if let Some(b) = hs.at(next).filter(|&b| b != e.id) {
        return Err(Block::Unit(b));
    }
    let here = e.tile();
    if next.x != here.x && next.y != here.y {
        let side = |t: Tile| hs.at(t).is_some_and(|b| b != e.id);
        if side(Tile { x: next.x, y: here.y }) && side(Tile { x: here.x, y: next.y }) {
            return Err(Block::Squeeze);
        }
    }
    Ok(())
}

/// Whether this unit moves at all in the movement phase: a harvester only while travelling.
fn travelling(e: &Entity) -> bool {
    !matches!((e.order, e.task), (Order::Harvest, Some(Task::Seek | Task::Mining | Task::Unloading | Task::Stuck)))
}

/// Standing still: no path, or waiting on a blocked one.
fn still(e: &Entity) -> bool {
    e.path.is_empty() || e.wait > 0
}

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// The movement phase: every ground unit, in id order, answers a request to step aside, then moves along its path.
pub fn tick(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let mut hs = Holders::build(pf, state, rules);
    for i in 0..state.entities.len() {
        if rules.kind(state.entities[i].kind).building {
            continue;
        }
        answer_yield(pf, state, rules, &mut hs, i, events);
        if travelling(&state.entities[i]) && !state.entities[i].path.is_empty() {
            advance(pf, state, rules, &mut hs, i, events);
        }
        let e = &mut state.entities[i];
        if e.order == Order::Move && e.path.is_empty() {
            e.order = Order::Idle;
            let t = e.tile();
            events.push(Event::MoveEnded { tick: state.tick, unit: e.id, reason: MoveEnd::Arrived, x: t.x, y: t.y });
        }
    }
}

/// Move along the path by this tick's speed, carrying leftover budget past centres and reserving the next tile at
/// each one.
fn advance(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, hs: &mut Holders, i: usize, ev: &mut Vec<Event>) {
    let speed = rules.kind(state.entities[i].kind).speed;
    hs.release(&state.entities[i]);
    let mut budget = speed;
    while budget > 0 {
        let e = &state.entities[i];
        let Some(&next) = e.path.front() else { break };
        if at_centre(e) && next != e.tile() {
            if let Err(block) = can_enter(pf, hs, e, next) {
                blocked(pf, state, rules, hs, i, next, block, ev);
                break;
            }
            let e = &mut state.entities[i];
            e.wait = 0;
            e.repath_fails = 0;
        }
        let e = &mut state.entities[i];
        let (dx, dy) = (world::centre(next.x) - e.x, world::centre(next.y) - e.y);
        let dist = isqrt((dx * dx + dy * dy) as u64) as i64;
        if dist <= budget {
            e.x += dx;
            e.y += dy;
            budget -= dist;
            e.path.pop_front();
        } else {
            // Integer division truncates toward zero, as the original TypeScript's Math.trunc did.
            e.x += dx * budget / dist;
            e.y += dy * budget / dist;
            budget = 0;
        }
    }
    hs.hold(&state.entities[i]);
}

#[allow(clippy::too_many_arguments)]
fn blocked(
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    hs: &Holders,
    i: usize,
    next: Tile,
    block: Block,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let m = &rules.movement;
    let goal = *state.entities[i].path.back().expect("a blocked unit has a path");
    let blocker = match block {
        Block::Static => {
            let e = &state.entities[i];
            state.entities[i].path = route(pf, e, goal);
            return;
        }
        Block::Unit(b) => index(state, b),
        Block::Squeeze => None,
    };
    if let Some(b) = blocker {
        let (e, o) = (&state.entities[i], &state.entities[b]);
        // Ask an own unit standing still to step aside. One that is waiting itself only gives way to a lower id,
        // except a harvester queued for the tile this unit is leaving: it gives way to whoever is on its dock.
        let waits_for_me = queued(o) && at_centre(o) && o.path.back() == Some(&e.tile());
        let gives_way = (can_yield(o) && (o.path.is_empty() || e.id < o.id)) || waits_for_me;
        if o.owner == e.owner && o.yield_for.is_none() && gives_way {
            let id = e.id;
            let o = &mut state.entities[b];
            o.yield_for = Some(id);
            o.yield_at = Some(tick);
        }
        // The last tile is taken by a unit that is staying there.
        if next == goal && state.entities[b].path.is_empty() {
            let e = &mut state.entities[i];
            match (e.order, e.task) {
                // Queue for the dock: the harvester on it leaves when it has unloaded.
                (Order::Harvest, Some(Task::ToRefinery)) => return,
                // Someone else is on the field tile: look for another.
                (Order::Harvest, _) => {
                    e.task = Some(Task::Seek);
                    e.path.clear();
                    return;
                }
                // Close enough to shoot from here; the attack follows its target on the next scan tick.
                (Order::Attack, _) => {
                    e.path.clear();
                    return;
                }
                _ => {
                    let near = nearest_free(pf, hs, e, goal, m.close_enough_rings);
                    match near {
                        Some(t) => e.path = route(pf, e, t),
                        None => e.path.clear(),
                    }
                    return;
                }
            }
        }
    }
    // Wait, then look for a way round the units standing still.
    let e = &mut state.entities[i];
    if e.order == Order::Harvest && e.task == Some(Task::ToRefinery) && next == goal {
        return;
    }
    if e.wait == 0 {
        e.wait = m.wait_base + random_int(&mut state.rng, m.wait_random.max(1));
        return;
    }
    e.wait -= 1;
    if e.wait > 0 {
        return;
    }
    let avoid: Vec<usize> = state
        .entities
        .iter()
        .filter(|o| o.id != state.entities[i].id && !rules.kind(o.kind).building && still(o))
        .flat_map(Holders::tiles)
        .filter_map(|t| hs.index(t))
        .collect();
    let e = &mut state.entities[i];
    let here = e.tile();
    match pf.find_avoiding((here.x, here.y), (goal.x, goal.y), &avoid, m.nodes_local) {
        Some(p) if !p.tiles.is_empty() => e.path = p.tiles.into(),
        _ => {
            e.repath_fails += 1;
            if e.repath_fails >= m.max_repath_fails {
                give_up(e, tick, events);
            }
        }
    }
}

fn give_up(e: &mut Entity, tick: u32, events: &mut Vec<Event>) {
    e.path.clear();
    e.wait = 0;
    e.repath_fails = 0;
    events.push(Event::UnitStuck { tick, unit: e.id });
    match (e.order, e.task) {
        // A field it can't reach: look for another one.
        (Order::Harvest, Some(Task::ToField)) => e.task = Some(Task::Seek),
        (Order::Harvest, _) => e.task = Some(Task::Stuck),
        // An attack tries again on its next scan tick.
        (Order::Attack, _) => {}
        _ => {
            e.order = Order::Idle;
            let t = e.tile();
            events.push(Event::MoveEnded { tick, unit: e.id, reason: MoveEnd::Blocked, x: t.x, y: t.y });
        }
    }
}

/// Whether a unit may be asked to step aside: a harvester only on its way somewhere, not while mining or unloading.
fn can_yield(e: &Entity) -> bool {
    at_centre(e) && still(e) && !matches!(e.task, Some(Task::Seek | Task::Mining | Task::Unloading | Task::Stuck))
}

/// A harvester next to its dock, waiting for the one on it to leave.
fn queued(e: &Entity) -> bool {
    e.order == Order::Harvest && e.task == Some(Task::ToRefinery) && e.path.len() == 1
}

/// The nearest free tile within `rings` of `goal`, ring by ring, then nearest the unit, then nearest the middle of
/// the map, then in row order.
fn nearest_free(pf: &Pathfinder, hs: &Holders, e: &Entity, goal: Tile, rings: i32) -> Option<Tile> {
    let here = e.tile();
    let (w, h) = pf.size();
    (1..=rings).find_map(|r| {
        (goal.y - r..=goal.y + r)
            .flat_map(|y| (goal.x - r..=goal.x + r).map(move |x| Tile { x, y }))
            .filter(|t| (t.x - goal.x).abs().max((t.y - goal.y).abs()) == r && hs.free(pf, *t, e.id))
            .min_by_key(|t| ((t.x - here.x).pow(2) + (t.y - here.y).pow(2), t.off_middle(w, h), t.y, t.x))
    })
}

/// A unit asked to step aside does so if it still can, the request is fresh and the asker is still there.
fn answer_yield(
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    hs: &mut Holders,
    i: usize,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let e = &state.entities[i];
    let (Some(asker), Some(at)) = (e.yield_for, e.yield_at) else { return };
    let r = index(state, asker);
    let fresh = tick.saturating_sub(at) < rules.movement.yield_expires;
    if !fresh || r.is_none() || !(can_yield(e) || queued(e) && at_centre(e)) || !travelling(e) {
        let e = &mut state.entities[i];
        e.yield_for = None;
        e.yield_at = None;
        return;
    }
    let r = &state.entities[r.expect("checked")];
    let ahead: Vec<Tile> = std::iter::once(r.tile()).chain(r.path.iter().take(3).copied()).collect();
    let here = e.tile();
    // The four sides, then the diagonals; of two equally good, the one nearer the middle of the map, so the two
    // sides of a mirrored map step aside the same way round.
    let mut around = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)]
        .map(|(dx, dy)| Tile { x: here.x + dx, y: here.y + dy });
    let (w, h) = pf.size();
    around.sort_by_key(|t| ((t.x != here.x && t.y != here.y), t.off_middle(w, h)));
    let open = |t: &Tile| can_enter(pf, hs, e, *t).is_ok();
    let aside = around.iter().find(|t| open(t) && !ahead.contains(t));
    let back = || around.iter().find(|t| open(t) && **t != ahead[0] && ahead.get(1) != Some(*t));
    let Some(&to) = aside.or_else(back) else { return };
    let goal = e.path.back().copied();
    hs.release(e);
    let e = &mut state.entities[i];
    let mut path = match goal {
        Some(g) if g != to => world::path_or_empty(pf, to, g),
        _ => VecDeque::new(),
    };
    path.push_front(to);
    e.path = path;
    e.wait = 0;
    e.repath_fails = 0;
    e.yield_for = None;
    e.yield_at = None;
    hs.hold(e);
    events.push(Event::UnitYielded { tick, unit: e.id, asker });
}
