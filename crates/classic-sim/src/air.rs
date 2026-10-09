//! Aircraft (rules-movement.md section 9) and the carrier's lifts (rules-economy-production.md section 13): the
//! phase after ground movement, over aircraft in id order.
//!
//! **Flying.** An aircraft ignores terrain, buildings and units: no occupancy, reservation or path search. It flies
//! straight at the one tile its path holds (or, for a carrier on a lift, at the unit or the drop tile), slowing over
//! its last 2 tiles to `max(speed / 4, speed * d / 512)` (an arrival ramp idea from an open-source remake, credited
//! in rules-movement.md; numbers ours), and turns its facing as it goes without waiting for it. Aircraft may
//! overlap.
//!
//! **Taking off and landing.** `altitude` runs from 0 (landed) to the air module's `cruise_altitude`, by `climb` a
//! tick. An aircraft with somewhere to go, a target or a lift climbs first and moves only once it is all the way up;
//! one with nothing to do comes down where it is, unless that is a building, a cliff or off the ground grid, where
//! it hovers. In the air only weapons that hit air can aim at it; landed, any weapon that hits ground can.
//!
//! **Lifts.** An idle carrier with no orders serves its owner's harvesters. Each tick, harvesters in id order whose
//! way still to go (to a refinery's dock or to a field) is longer than `ferry_min_path` tiles are offered to the
//! nearest idle carrier of their owner (straight-line distance, ties by id); one carrier per harvester. The
//! harvester keeps driving until the carrier reaches it. The carrier takes hold of it (`carrier_pickup`), hovers
//! `pickup_ticks`, flies to where it was going and sets it down there, or on the nearest free tile within
//! `drop_rings` if that is taken, after `drop_ticks` (`carrier_dropoff`); with nowhere free it hovers and looks again
//! every `drop_retry_ticks`. While carried a unit is off the ground: nothing can aim at or splash it, the hazard
//! can't take it, and it takes no orders. If its carrier is destroyed it falls to the nearest free tile and loses
//! `fall_damage_percent` of its full health (`carrier_lost_cargo`).

use rts_core::hash::{Canon, CanonHasher};
use rts_core::imath::isqrt;

use crate::combat::{facing_to, turn};
use crate::map::{MapData, TILE, Tile};
use crate::movement;
use crate::path::Pathfinder;
use crate::units::Rules;
use crate::world::{self, Entity, Event, GameState, MoveEnd, Order, Task, centre, on_ground};

/// A carrier's step in a lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Flying to the unit, which keeps driving.
    Fetch,
    /// Holding the unit, hovering while it is hooked on.
    Lift,
    /// Flying it to where it was going.
    Carry,
    /// Hovering over the drop tile while it is set down.
    Drop,
}

impl Stage {
    pub fn id(self) -> &'static str {
        match self {
            Stage::Fetch => "fetch",
            Stage::Lift => "lift",
            Stage::Carry => "carry",
            Stage::Drop => "drop",
        }
    }
}

/// A carrier's lift of one unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ferry {
    pub unit: u32,
    /// Where the unit was going: a refinery's dock or a field tile.
    pub goal: Tile,
    /// The tile it will be set down on, once the carrier has found it free.
    pub drop: Option<Tile>,
    pub stage: Stage,
    /// Ticks left hooking on or setting down, or before looking again for a free drop tile.
    pub timer: u32,
}

impl Canon for Ferry {
    fn canon(&self, w: &mut CanonHasher) {
        w.object()
            .opt("drop", self.drop.as_ref())
            .field("goal", &self.goal)
            .field("stage", self.stage.id())
            .field("timer", &self.timer)
            .field("unit", &self.unit)
            .end();
    }
}

/// Whether a carrier has a unit aboard (or is hooking one on or setting one down), so it can't be sent elsewhere.
pub fn lifting(e: &Entity) -> bool {
    e.ferry.is_some_and(|f| f.stage != Stage::Fetch)
}

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// The air phase of one tick.
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    fall(map, pf, state, rules, events);
    offer(state, rules);
    for i in 0..state.entities.len() {
        let k = rules.kind(state.entities[i].kind);
        if !k.air {
            continue;
        }
        if k.carrier {
            ferry(map, pf, state, rules, i, events);
        }
        fly(map, pf, state, rules, i);
        let e = &mut state.entities[i];
        if e.order == Order::Move && e.path.is_empty() {
            e.order = Order::Idle;
            let t = e.tile();
            events.push(Event::MoveEnded { tick: state.tick, unit: e.id, reason: MoveEnd::Arrived, x: t.x, y: t.y });
        }
        // What it carries goes with it.
        if let Some(f) = state.entities[i].ferry.filter(|f| f.stage != Stage::Fetch)
            && let Some(u) = index(state, f.unit)
        {
            let (x, y) = (state.entities[i].x, state.entities[i].y);
            (state.entities[u].x, state.entities[u].y) = (x, y);
        }
    }
}

/// A unit whose carrier is gone falls to the nearest free tile and is hurt.
fn fall(map: &MapData, pf: &Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    for i in 0..state.entities.len() {
        let Some(carrier) = state.entities[i].carried_by else { continue };
        if index(state, carrier).is_some() {
            continue;
        }
        let here = state.entities[i].tile();
        let to = free_near(map, pf, state, rules, here, 8).unwrap_or(here);
        let k = rules.kind(state.entities[i].kind);
        let e = &mut state.entities[i];
        e.carried_by = None;
        (e.x, e.y) = (centre(to.x), centre(to.y));
        e.path.clear();
        e.health -= k.max_health * rules.air.fall_damage_percent / 100;
        // A harvester picks its loop up again: home if it is full, else to the nearest field.
        if e.task.is_some() {
            e.task = Some(Task::Mining);
        }
        events.push(Event::CarrierLostCargo { tick: state.tick, carrier, unit: e.id, x: to.x, y: to.y });
    }
}

/// Free to set a unit down on: open to ground units, with none on it or stepping into it.
fn free(map: &MapData, pf: &Pathfinder, state: &GameState, rules: &Rules, t: Tile) -> bool {
    map.in_bounds(t.x, t.y)
        && pf.passable(t.x, t.y)
        && !state.entities.iter().any(|e| on_ground(rules, e) && (e.tile() == t || movement::step_tile(e) == Some(t)))
}

/// `at` if it is free, else the nearest free tile within `rings` of it, ring by ring, then nearest the middle of the
/// map, then in row order.
fn free_near(map: &MapData, pf: &Pathfinder, state: &GameState, rules: &Rules, at: Tile, rings: i32) -> Option<Tile> {
    (0..=rings).find_map(|r| {
        (at.y - r..=at.y + r)
            .flat_map(|y| (at.x - r..=at.x + r).map(move |x| Tile { x, y }))
            .filter(|t| (t.x - at.x).abs().max((t.y - at.y).abs()) == r && free(map, pf, state, rules, *t))
            .min_by_key(|t| ((t.x - at.x).pow(2) + (t.y - at.y).pow(2), t.off_middle(map.width, map.height), t.y, t.x))
    })
}

/// A harvester driving a long way to a dock or a field, with no carrier yet.
fn wants_lift(state: &GameState, rules: &Rules, e: &Entity) -> bool {
    rules.kind(e.kind).harvester.is_some()
        && on_ground(rules, e)
        && e.order == Order::Harvest
        && matches!(e.task, Some(Task::ToRefinery | Task::ToField))
        && e.path.len() > rules.air.ferry_min_path
        && !state.entities.iter().any(|c| c.ferry.is_some_and(|f| f.unit == e.id))
}

/// A carrier free to take a lift: no orders, no lift, nowhere to go.
fn idle_carrier(rules: &Rules, c: &Entity) -> bool {
    rules.kind(c.kind).carrier && c.order == Order::Idle && c.ferry.is_none() && c.path.is_empty()
}

/// Offer each harvester that wants a lift, in id order, to the nearest idle carrier of its owner.
fn offer(state: &mut GameState, rules: &Rules) {
    if !state.entities.iter().any(|c| idle_carrier(rules, c)) {
        return;
    }
    for h in 0..state.entities.len() {
        let e = &state.entities[h];
        if !wants_lift(state, rules, e) {
            continue;
        }
        let d2 = |c: &Entity| (c.x - e.x).pow(2) + (c.y - e.y).pow(2);
        let nearest = (0..state.entities.len())
            .filter(|&c| state.entities[c].owner == e.owner && idle_carrier(rules, &state.entities[c]))
            .min_by_key(|&c| (d2(&state.entities[c]), state.entities[c].id));
        let Some(c) = nearest else { continue };
        let (unit, goal) = (e.id, *e.path.back().expect("a long path is not empty"));
        state.entities[c].ferry = Some(Ferry { unit, goal, drop: None, stage: Stage::Fetch, timer: 0 });
    }
}

/// One tick of a carrier's lift, before it flies.
fn ferry(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, i: usize, events: &mut Vec<Event>) {
    let Some(mut f) = state.entities[i].ferry else { return };
    let tick = state.tick;
    let carrier = state.entities[i].id;
    let Some(u) = index(state, f.unit) else {
        state.entities[i].ferry = None;
        return;
    };
    let air = &rules.air;
    match f.stage {
        Stage::Fetch => {
            // The harvester must still be on its way to the same place, on the ground and its owner's.
            let e = &state.entities[u];
            let still = on_ground(rules, e)
                && e.owner == state.entities[i].owner
                && e.order == Order::Harvest
                && matches!(e.task, Some(Task::ToRefinery | Task::ToField))
                && e.path.back() == Some(&f.goal);
            if !still {
                state.entities[i].ferry = None;
                return;
            }
            // Close enough to reach it this tick (it drove on since the carrier last flew): take hold.
            let c = &state.entities[i];
            let reach = rules.kind(c.kind).speed;
            if c.altitude < air.cruise_altitude || (c.x - e.x).pow(2) + (c.y - e.y).pow(2) > reach * reach {
                state.entities[i].ferry = Some(f);
                return;
            }
            let (x, y) = (e.x, e.y);
            (state.entities[i].x, state.entities[i].y) = (x, y);
            let e = &mut state.entities[u];
            e.carried_by = Some(carrier);
            e.path.clear();
            e.wait = 0;
            e.repath_fails = 0;
            e.yield_for = None;
            e.yield_at = None;
            f.stage = Stage::Lift;
            f.timer = air.pickup_ticks;
            events.push(Event::CarrierPickup { tick, carrier, unit: f.unit, to_x: f.goal.x, to_y: f.goal.y });
        }
        Stage::Lift => {
            f.timer -= 1;
            if f.timer == 0 {
                f.stage = Stage::Carry;
            }
        }
        Stage::Carry => {
            let c = &state.entities[i];
            let aim = f.drop.unwrap_or(f.goal);
            if (c.x, c.y) != (centre(aim.x), centre(aim.y)) {
                return;
            }
            if f.timer > 0 {
                f.timer -= 1;
                if f.timer > 0 {
                    state.entities[i].ferry = Some(f);
                    return;
                }
            }
            // Over the drop tile: set down if it is free, else head for the nearest free one, else wait.
            match free_near(map, pf, state, rules, aim, if f.drop.is_some() { 0 } else { air.drop_rings }) {
                Some(t) if t == aim => {
                    f.drop = Some(t);
                    f.stage = Stage::Drop;
                    f.timer = air.drop_ticks;
                }
                Some(t) => f.drop = Some(t),
                None if f.drop.is_some() => f.drop = None,
                None => f.timer = air.drop_retry_ticks,
            }
        }
        Stage::Drop => {
            f.timer -= 1;
            if f.timer > 0 {
                state.entities[i].ferry = Some(f);
                return;
            }
            let d = f.drop.expect("a drop has its tile");
            if !free(map, pf, state, rules, d) {
                // Taken while it was being set down: look again.
                f.stage = Stage::Carry;
                f.drop = None;
                state.entities[i].ferry = Some(f);
                return;
            }
            let path = if d == f.goal { Default::default() } else { world::path_or_empty(pf, d, f.goal) };
            let e = &mut state.entities[u];
            e.carried_by = None;
            (e.x, e.y) = (centre(d.x), centre(d.y));
            // Set down short of a place it can't drive to: a harvester picks its loop up again.
            if d != f.goal && path.is_empty() {
                e.task = Some(Task::Mining);
            }
            e.path = path;
            events.push(Event::CarrierDropoff { tick, carrier, unit: f.unit, x: d.x, y: d.y });
            state.entities[i].ferry = None;
            return;
        }
    }
    state.entities[i].ferry = Some(f);
}

/// Where an aircraft is flying to this tick: a carrier on a lift goes to the unit, then to the drop tile, and hovers
/// while hooking on or setting down; anything else to the tile its path holds.
fn aim(state: &GameState, e: &Entity) -> Option<(i64, i64)> {
    match e.ferry {
        Some(Ferry { stage: Stage::Fetch, unit, .. }) => {
            index(state, unit).map(|u| (state.entities[u].x, state.entities[u].y))
        }
        Some(Ferry { stage: Stage::Carry, drop, goal, .. }) => {
            let t = drop.unwrap_or(goal);
            Some((centre(t.x), centre(t.y)))
        }
        Some(_) => None,
        None => e.path.front().map(|t| (centre(t.x), centre(t.y))),
    }
}

/// Climb, come down, or fly straight towards the aim, slowing over the last 2 tiles.
fn fly(map: &MapData, pf: &Pathfinder, state: &mut GameState, rules: &Rules, i: usize) {
    let air = &rules.air;
    let k = rules.kind(state.entities[i].kind);
    let to = aim(state, &state.entities[i]);
    let e = &mut state.entities[i];
    let t = e.tile();
    let busy = to.is_some() || e.ferry.is_some() || e.target.is_some();
    if !busy && map.in_bounds(t.x, t.y) && pf.passable(t.x, t.y) {
        e.altitude = (e.altitude - air.climb).max(0);
        return;
    }
    if e.altitude < air.cruise_altitude {
        e.altitude = (e.altitude + air.climb).min(air.cruise_altitude);
        return;
    }
    let Some((ax, ay)) = to else { return };
    let (dx, dy) = (ax - e.x, ay - e.y);
    let d = isqrt((dx * dx + dy * dy) as u64) as i64;
    if d > 0 {
        let v = (k.speed * d / (2 * TILE)).clamp((k.speed / 4).max(1), k.speed).min(d);
        if v == d {
            (e.x, e.y) = (ax, ay);
        } else {
            e.x += dx * v / d;
            e.y += dy * v / d;
        }
        if e.target.is_none() {
            e.facing = turn(e.facing, facing_to(dx, dy), k.turn_rate);
        }
    }
    if e.ferry.is_none() && (e.x, e.y) == (ax, ay) {
        e.path.pop_front();
    }
}
