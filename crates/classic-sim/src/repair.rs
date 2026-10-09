//! Repair (rules-base-building-power.md, "Repairing buildings"; the `repair` module).
//!
//! **Buildings.** Its owner turns repair on or off for any own building. While on, the building adds its owner's
//! power factor to `repair_due` each tick and takes a step each time that reaches `every * 100`, so a short player
//! repairs more slowly, as it builds more slowly. A step mends `max(1, max_health / step_div)` health (no more than
//! is missing) and costs that share of `cost_percent` of the kind's cost, rounded up, at least 1 credit: so mending
//! from nothing costs about half the building's price by default. A step its owner can't pay for waits, with repair
//! still on, and credits never go below zero. Repair goes off by itself at full health. It works under fire.
//!
//! **Vehicles.** A damaged vehicle sent to an own repair pad drives to a free tile beside it and waits there. Each
//! pad mends one vehicle at a time, the lowest id among those waiting beside it, `pad_step` health every `pad_every`
//! ticks at full power, at the same price rule from the vehicle's own cost; the one it is mending keeps its turn
//! until whole or gone. A vehicle mended to full goes back to
//! standing guard. The design has the vehicle drive onto the pad itself; here it parks beside it, as a harvester
//! parks beside a refinery, so no unit ever stands inside a footprint (a simplification, noted in the design doc).

use crate::map::Tile;
use crate::movement;
use crate::path::Pathfinder;
use crate::power::Power;
use crate::units::{KindRules, Rules};
use crate::world::{self, Entity, Event, GameState, Order};

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// What mending `step` health of a `k` costs: its share of `cost_percent` of the cost, rounded up, at least 1.
pub fn step_cost(rules: &Rules, k: &KindRules, step: i64) -> i64 {
    let pct = rules.repair.cost_percent;
    if pct == 0 || step <= 0 {
        return 0;
    }
    let (num, den) = (k.cost * step * pct, k.max_health * 100);
    ((num + den - 1) / den).max(1)
}

/// What one repair step of building `e` mends now.
pub fn building_step(rules: &Rules, e: &Entity) -> i64 {
    let k = rules.kind(e.kind);
    (k.max_health / rules.repair.step_div).max(1).min(k.max_health - e.health)
}

/// Turn repair on or off for `player`'s buildings in `ids`. Turning on a whole building, or one being sold, does
/// nothing.
pub fn order(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], on: bool, events: &mut Vec<Event>) {
    let tick = state.tick;
    for &id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &mut state.entities[i];
        let k = rules.kind(e.kind);
        if e.owner != player || !k.building || e.repairing == on {
            continue;
        }
        if on && (e.health >= k.max_health || e.selling > 0) {
            continue;
        }
        e.repairing = on;
        e.repair_due = 0;
        events.push(if on {
            Event::RepairStarted { tick, entity: id, owner: player }
        } else {
            Event::RepairStopped { tick, entity: id, owner: player, whole: false }
        });
    }
}

/// Send `player`'s damaged vehicles in `ids` to their own repair pad `pad`. Anything else in `ids` is left alone.
pub fn send(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], pad: u32) {
    let ok = state.entity(pad).is_some_and(|p| p.owner == player && rules.kind(p.kind).repair_pad && p.selling == 0);
    if !ok {
        return;
    }
    for &id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &state.entities[i];
        let k = rules.kind(e.kind);
        // A unit in a carrier's hold takes no orders until it is set down, and one counting down to its blast none.
        if e.owner != player || !k.vehicle || e.health >= k.max_health || e.carried_by.is_some() || e.fuse.is_some() {
            continue;
        }
        let e = &mut state.entities[i];
        e.order = Order::Repair;
        e.goal = Some(pad);
        e.target = None;
        movement::halt(e);
        let b = state.entity(pad).expect("checked").clone();
        if !beside(rules, &state.entities[i], &b)
            && let Some(to) = spot_beside(pf, state, rules, i, &b)
        {
            let e = &mut state.entities[i];
            e.path = movement::route(pf, e, to);
        }
    }
}

/// A vehicle leaves the repair order: a harvester goes back to its loop, anything else stands guard.
fn done(rules: &Rules, e: &mut Entity) {
    e.goal = None;
    if rules.kind(e.kind).harvester.is_some() {
        e.order = Order::Harvest;
        e.task = Some(crate::world::Task::Seek);
    } else {
        e.order = Order::Idle;
    }
}

/// Whether unit `e` stands still on a tile touching building `b`'s footprint.
pub(crate) fn beside(rules: &Rules, e: &Entity, b: &Entity) -> bool {
    let (t, k, here) = (b.tile(), rules.kind(b.kind), e.tile());
    movement::at_centre(e)
        && e.path.is_empty()
        && (t.x - 1..=t.x + k.width).contains(&here.x)
        && (t.y - 1..=t.y + k.height).contains(&here.y)
}

/// Where unit `me` heads to stand beside building `b`: the tile touching the footprint it can reach that no other
/// unit holds, nearest it, then nearest the middle of the map, then in row order. `None` when every reachable one is
/// taken or none can be reached.
pub(crate) fn spot_beside(
    pf: &mut Pathfinder,
    state: &GameState,
    rules: &Rules,
    me: usize,
    b: &Entity,
) -> Option<Tile> {
    let (t, k) = (b.tile(), rules.kind(b.kind));
    let here = state.entities[me].tile();
    let (w, h) = pf.size();
    let taken = |d: Tile| {
        state.entities.iter().enumerate().any(|(j, e)| {
            j != me
                && world::on_ground(rules, e)
                && (e.tile() == d || movement::step_tile(e) == Some(d) || e.path.back() == Some(&d))
        })
    };
    let mut spots: Vec<Tile> =
        world::around(t.x, t.y, k.width, k.height).filter(|d| pf.passable(d.x, d.y) && !taken(*d)).collect();
    spots.sort_by_key(|d| {
        (((d.x - here.x) as i64).pow(2) + ((d.y - here.y) as i64).pow(2), d.off_middle(w, h), d.y, d.x)
    });
    spots.into_iter().find(|&d| d == here || !world::path_or_empty(pf, here, d).is_empty())
}

/// The repair phase: buildings with repair on, in id order, then each repair pad, in id order.
pub fn tick(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let factors: Vec<i64> = Power::all(state, rules).iter().map(|p| p.factor(rules)).collect();
    let player = |state: &GameState, owner: u32| state.players.iter().position(|p| p.id == owner);
    let every = rules.repair.every as i64 * 100;
    for i in 0..state.entities.len() {
        let e = &state.entities[i];
        if !e.repairing {
            continue;
        }
        let (id, owner, k) = (e.id, e.owner, rules.kind(e.kind));
        if e.health >= k.max_health {
            let e = &mut state.entities[i];
            e.repairing = false;
            e.repair_due = 0;
            events.push(Event::RepairStopped { tick, entity: id, owner, whole: true });
            continue;
        }
        let Some(p) = player(state, owner) else { continue };
        let due = (state.entities[i].repair_due + factors[p]).min(every);
        state.entities[i].repair_due = due;
        if due < every {
            continue;
        }
        let step = building_step(rules, &state.entities[i]);
        let cost = step_cost(rules, k, step);
        if state.players[p].credits < cost {
            continue;
        }
        state.players[p].credits -= cost;
        let e = &mut state.entities[i];
        e.health += step;
        e.repair_due = 0;
    }
    pads(pf, state, rules, &factors, events);
}

/// Vehicles sent to a pad: each heads for a free spot beside it, and each pad mends one waiting there (its `goal`).
fn pads(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, factors: &[i64], events: &mut Vec<Event>) {
    let tick = state.tick;
    // Units on their way: drop the order if the pad has gone, changed hands or is being sold, or the unit is whole;
    // otherwise head for a spot beside the pad when not moving.
    for i in 0..state.entities.len() {
        let e = &state.entities[i];
        if e.order != Order::Repair {
            continue;
        }
        let pad = e.goal.and_then(|g| index(state, g)).filter(|&p| {
            let p = &state.entities[p];
            p.owner == e.owner && p.selling == 0 && rules.kind(p.kind).repair_pad
        });
        let whole = e.health >= rules.kind(e.kind).max_health;
        let Some(p) = pad.filter(|_| !whole) else {
            let e = &mut state.entities[i];
            done(rules, e);
            movement::halt(e);
            continue;
        };
        let pad = state.entities[p].clone();
        // Look for a spot when it has none, at once and then on scan ticks, so a crowd waiting for a full pad
        // doesn't search every tick.
        let e = &state.entities[i];
        let fresh = e.path.is_empty() && (tick + e.id).is_multiple_of(rules.combat.scan_every.max(1));
        if fresh
            && !beside(rules, &state.entities[i], &pad)
            && let Some(to) = spot_beside(pf, state, rules, i, &pad)
        {
            let e = &mut state.entities[i];
            e.path = movement::route(pf, e, to);
        }
    }
    let every = rules.repair.pad_every as i64 * 100;
    for p in 0..state.entities.len() {
        let pad = &state.entities[p];
        if !rules.kind(pad.kind).repair_pad || pad.selling > 0 {
            continue;
        }
        let (pad_id, owner) = (pad.id, pad.owner);
        let waiting = |u: &usize| {
            let e = &state.entities[*u];
            e.order == Order::Repair && e.goal == Some(pad_id) && beside(rules, e, &state.entities[p])
        };
        // The vehicle it is mending keeps its turn; otherwise the lowest id waiting beside it.
        let serving = state.entities[p].goal.and_then(|id| index(state, id)).filter(waiting);
        let Some(u) = serving.or_else(|| (0..state.entities.len()).find(waiting)) else {
            let pad = &mut state.entities[p];
            pad.repair_due = 0;
            pad.goal = None;
            continue;
        };
        state.entities[p].goal = Some(state.entities[u].id);
        let Some(pl) = state.players.iter().position(|pl| pl.id == owner) else { continue };
        let due = (state.entities[p].repair_due + factors[pl]).min(every);
        state.entities[p].repair_due = due;
        if due < every {
            continue;
        }
        let k = rules.kind(state.entities[u].kind);
        let step = rules.repair.pad_step.min(k.max_health - state.entities[u].health);
        let cost = step_cost(rules, k, step);
        if state.players[pl].credits < cost {
            continue;
        }
        state.players[pl].credits -= cost;
        state.entities[p].repair_due = 0;
        let e = &mut state.entities[u];
        e.health += step;
        if e.health >= k.max_health {
            done(rules, e);
            events.push(Event::UnitRepaired { tick, unit: e.id, pad: pad_id, owner });
        }
    }
}
