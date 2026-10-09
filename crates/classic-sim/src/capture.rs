//! Capture (rules-base-building-power.md, "Capture"; the `capture` module, on unless a pack turns it off).
//!
//! Units with the `capturer` role (infantry by default) can be ordered to take an enemy building with the
//! `capturable` role (walls, turrets and the palace have none) that is below `below_percent` of its maximum health.
//! An order for a building at or above that is refused before anyone sets off, and a capturer whose target heals
//! past it on the way gives up and stands guard. Under fog, only a building the player can see or keeps a ghost of
//! can be named. A capturer that reaches a tile touching the footprint goes inside: it is removed and the building
//! changes owner at its current health. Its queue is lost with no refund to either side, its repair goes off and a
//! sale in progress is called off; the new owner's units stop shooting it. Infantry capturing a badly damaged
//! building is the 1992 original's rule; an engineer that takes a building at any health, as later games had, is a
//! pack giving its own capturer kind and `below_percent` 100.

use crate::movement;
use crate::path::Pathfinder;
use crate::repair::{beside, spot_beside};
use crate::units::Rules;
use crate::vision;
use crate::world::{CaptureError, Entity, Event, GameState, Order};

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// Whether `player` may send capturers against `b` now: capture is on, `b` is an enemy building that can be taken,
/// and it is hurt enough.
pub fn can_capture(rules: &Rules, player: u32, b: &Entity) -> Result<(), CaptureError> {
    let c = rules.capture.as_ref().ok_or(CaptureError::Off)?;
    let k = rules.kind(b.kind);
    if !k.capturable || b.owner == player {
        return Err(CaptureError::NotCapturable);
    }
    if b.health * 100 >= k.max_health * c.below_percent {
        return Err(CaptureError::TooHealthy);
    }
    Ok(())
}

/// Send `player`'s capturers in `ids` to take building `target`. Others in `ids` are left alone.
pub fn order(
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    player: u32,
    ids: &[u32],
    target: u32,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let Some(b) = state.entity(target).filter(|b| vision::known(state, rules, player, b)).cloned() else { return };
    let mut refuse = |reason| events.push(Event::CaptureRefused { tick, player, target, reason });
    if let Err(reason) = can_capture(rules, player, &b) {
        return refuse(reason);
    }
    let mut sent = false;
    for &id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &state.entities[i];
        if e.owner != player || !rules.kind(e.kind).capturer {
            continue;
        }
        sent = true;
        let e = &mut state.entities[i];
        e.order = Order::Capture;
        e.goal = Some(target);
        e.target = None;
        movement::halt(e);
        if !beside(rules, &state.entities[i], &b)
            && let Some(to) = spot_beside(pf, state, rules, i, &b)
        {
            let e = &mut state.entities[i];
            e.path = movement::route(pf, e, to);
        }
    }
    if !sent {
        refuse(CaptureError::NoCapturer);
    }
}

/// The capture phase: capturers in id order give up, head for their building, or take it.
pub fn tick(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let ids: Vec<u32> = state.entities.iter().filter(|e| e.order == Order::Capture).map(|e| e.id).collect();
    for id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &state.entities[i];
        let owner = e.owner;
        let target = e.goal.and_then(|g| index(state, g));
        let check = target.map(|b| can_capture(rules, owner, &state.entities[b]));
        let b = match (target, check) {
            (Some(b), Some(Ok(()))) => b,
            (_, reason) => {
                // Healed past the threshold on the way: say so. Gone, or already taken: just stand guard.
                if let Some(Err(CaptureError::TooHealthy)) = reason {
                    let target = state.entities[i].goal.unwrap_or(0);
                    events.push(Event::CaptureRefused {
                        tick,
                        player: owner,
                        target,
                        reason: CaptureError::TooHealthy,
                    });
                }
                let e = &mut state.entities[i];
                e.order = Order::Idle;
                e.goal = None;
                movement::halt(e);
                continue;
            }
        };
        let building = state.entities[b].clone();
        if beside(rules, &state.entities[i], &building) {
            take(state, i, b, events);
            continue;
        }
        let e = &state.entities[i];
        if e.path.is_empty()
            && (tick + e.id).is_multiple_of(rules.combat.scan_every.max(1))
            && let Some(to) = spot_beside(pf, state, rules, i, &building)
        {
            let e = &mut state.entities[i];
            e.path = movement::route(pf, e, to);
        }
    }
}

/// Capturer `u` goes into building `b`, which becomes its owner's.
fn take(state: &mut GameState, u: usize, b: usize, events: &mut Vec<Event>) {
    let tick = state.tick;
    let (by, to) = (state.entities[u].id, state.entities[u].owner);
    let e = &mut state.entities[b];
    let (entity, kind, from) = (e.id, e.kind, e.owner);
    if e.repairing {
        events.push(Event::RepairStopped { tick, entity, owner: from, whole: false });
    }
    e.owner = to;
    e.queue.clear();
    e.repairing = false;
    e.repair_due = 0;
    e.selling = 0;
    e.target = None;
    e.goal = None;
    e.last_attacker = None;
    state.entities.remove(u);
    // The new owner's units stop shooting what is now theirs.
    for e in state.entities.iter_mut().filter(|e| e.owner == to && e.target == Some(entity)) {
        e.target = None;
        if e.order == Order::Attack {
            e.order = Order::Idle;
            movement::halt(e);
        }
    }
    events.push(Event::Captured { tick, entity, kind, from, to, by });
}
