//! Deploying (the `deploy` module; rules-movement.md section 7, "Footprints", and rules-base-building-power.md): a
//! unit whose kind `deploys_into` a building turns into that building where it stands, as a mobile construction
//! vehicle becomes a construction yard.
//!
//! The building's footprint is laid out round the unit's tile: its top-left tile is `(w - 1) / 2` columns left and
//! `(h - 1) / 2` rows up of it, so a 2 by 2 yard has the unit's tile as its top-left one. The ground must pass the
//! placement rules less the one about being near its owner's other buildings: a deployed building may stand anywhere,
//! which is what a base builder is for. On the order the unit finishes the step it is on; then, each tick:
//!
//! - ground that can never take the building (off the map, cliff, open ground under `rock_only`, resource, another
//!   building) refuses the deploy at once, with `deploy_refused` and the placement reason;
//! - units in the way are waited for, up to `deploy.wait_ticks` from the order: the owner's own units standing still
//!   there are sent to the nearest free tile outside the footprint, and enemies may leave by themselves. Still
//!   blocked by then, the deploy is refused as `blocked`;
//! - clear ground deploys: the unit goes, the building appears complete and working (`deployed`), with the same share
//!   of its health as the unit had, so a damaged vehicle gives a damaged building.
//!
//! Any other order before then calls the deploy off.

use crate::map::{MapData, Tile};
use crate::movement;
use crate::path::Pathfinder;
use crate::placement::{self, PlaceError};
use crate::units::{Kind, Rules};
use crate::world::{self, Entity, Event, GameState, Order};

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// The top-left tile of the building a unit of kind `unit` standing on `at` deploys into, and that building's kind;
/// `None` for kinds that don't deploy.
pub fn site(rules: &Rules, unit: Kind, at: Tile) -> Option<(Kind, Tile)> {
    let b = rules.kind(unit).deploys_into?;
    let k = rules.kind(b);
    Some((b, Tile { x: at.x - (k.width - 1) / 2, y: at.y - (k.height - 1) / 2 }))
}

/// Whether unit `e` could deploy if it stood on `at` now, other units counting as in the way. Changes nothing.
pub fn check(map: &MapData, state: &GameState, rules: &Rules, e: &Entity, at: Tile) -> Result<(), PlaceError> {
    let (b, t) = site(rules, e.kind, at).ok_or(PlaceError::NotABuilding)?;
    placement::check_deploy(map, state, rules, b, t.x, t.y, e.id)
}

/// The deploy phase: units under a deploy order, in id order, deploy, wait, or give up.
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    // An order given since calls the deploy off.
    for e in state.entities.iter_mut().filter(|e| e.deploy_by.is_some() && e.order != Order::Deploy) {
        e.deploy_by = None;
    }
    let ids: Vec<u32> = state.entities.iter().filter(|e| e.order == Order::Deploy).map(|e| e.id).collect();
    for id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &state.entities[i];
        // It finishes the step it is on first.
        if !e.path.is_empty() || !movement::at_centre(e) {
            continue;
        }
        let Some((b, at)) = site(rules, e.kind, e.tile()) else { continue };
        let (owner, deploy_by) = (e.owner, e.deploy_by.unwrap_or(tick));
        let refuse = |state: &mut GameState, events: &mut Vec<Event>, reason| {
            let e = &mut state.entities[i];
            e.order = Order::Idle;
            e.deploy_by = None;
            events.push(Event::DeployRefused { tick, player: owner, unit: id, reason });
        };
        match placement::check_deploy(map, state, rules, b, at.x, at.y, id) {
            Ok(()) => deploy(pf, state, rules, i, b, at, events),
            // Units in the way, and only units: wait for them, sending its owner's own out of the way.
            Err(PlaceError::Blocked { .. }) if tick < deploy_by && !building_on(state, rules, b, at) => {
                clear(pf, state, rules, i, b, at);
            }
            Err(reason) => refuse(state, events, reason),
        }
    }
}

/// The footprint of a building of kind `b` with its top-left tile at `at`.
fn covers(rules: &Rules, b: Kind, at: Tile, t: Tile) -> bool {
    let k = rules.kind(b);
    (at.x..at.x + k.width).contains(&t.x) && (at.y..at.y + k.height).contains(&t.y)
}

/// Whether another building stands on any tile of that footprint.
fn building_on(state: &GameState, rules: &Rules, b: Kind, at: Tile) -> bool {
    let k = rules.kind(b);
    state.entities.iter().filter(|e| rules.kind(e.kind).building).any(|e| {
        let (t, ek) = (e.tile(), rules.kind(e.kind));
        t.x < at.x + k.width && at.x < t.x + ek.width && t.y < at.y + k.height && at.y < t.y + ek.height
    })
}

/// Send the deploying unit's owner's own ground units standing still in the footprint to the nearest free tile
/// outside it.
fn clear(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, me: usize, b: Kind, at: Tile) {
    let (owner, my_id) = (state.entities[me].owner, state.entities[me].id);
    let k = rules.kind(b);
    let movers: Vec<usize> = (0..state.entities.len())
        .filter(|&j| {
            let e = &state.entities[j];
            e.id != my_id
                && e.owner == owner
                && world::on_ground(rules, e)
                && e.path.is_empty()
                && e.fuse.is_none()
                && covers(rules, b, at, e.tile())
        })
        .collect();
    for j in movers {
        // Tiles another unit holds or is heading to, the one moving aside excepted.
        let held = |t: Tile| {
            state.entities.iter().enumerate().any(|(n, e)| {
                n != j && world::on_ground(rules, e) && (e.tile() == t || movement::step_tile(e) == Some(t))
            })
        };
        let from = state.entities[j].tile();
        let d2 = |t: &Tile| ((t.x - from.x) as i64).pow(2) + ((t.y - from.y) as i64).pow(2);
        // Ring by ring out from the footprint; the nearest free tile to the unit, then in row order.
        let spot = (1..=4).find_map(|r| {
            world::around(at.x - r + 1, at.y - r + 1, k.width + 2 * (r - 1), k.height + 2 * (r - 1))
                .filter(|&t| pf.passable(t.x, t.y) && !held(t))
                .min_by_key(|t| (d2(t), t.y, t.x))
        });
        if let Some(to) = spot {
            let e = &mut state.entities[j];
            e.path = movement::route(pf, e, to);
            if e.order == Order::Idle {
                e.order = Order::Move;
            }
        }
    }
}

/// Unit `i` becomes a building of kind `b` with its top-left tile at `at`.
fn deploy(
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    i: usize,
    b: Kind,
    at: Tile,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let unit = state.entities.remove(i);
    // Its owner's slabs under it count as foundation, as for a placed building (the `decay` module).
    let foundation = crate::decay::foundation(state, rules, unit.owner, b, at.x, at.y);
    let entity = world::spawn(state, rules, b, unit.owner, at.x, at.y);
    let building = state.entities.last_mut().expect("just spawned");
    building.foundation = foundation;
    // The same share of its health as the unit had, at least 1.
    let (full, was) = (rules.kind(b).max_health, rules.kind(unit.kind).max_health);
    building.health = (full * unit.health.max(0) / was.max(1)).clamp(1, full);
    world::occupy(pf, rules, building, true);
    world::reroute_around_new_building(pf, state, rules);
    events.push(Event::Deployed { tick, unit: unit.id, entity, kind: b, owner: unit.owner, x: at.x, y: at.y });
}
