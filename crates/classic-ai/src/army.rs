//! The army: new units gather at a rally point a few tiles out from the yard towards the nearest enemy, where a
//! player can see them massing. Armed enemies near its buildings, or near its harvesters close to home, draw out
//! every unit at home. Once the first-wave
//! time has passed and enough units wait at the rally point, they go as one wave: at armed enemies near them first,
//! otherwise at the enemy building nearest them. A wave that falls below `retreat_percent` of its starting size comes
//! home, and each wave after the first waits for `wave_growth` more units, up to `wave_cap`.

use std::collections::BTreeSet;

use classic_sim::{CommandOrder, Entity, Game, Order, Tile};

use crate::base::toward;
use crate::{Ai, Orders, View, centre_tile, dist2, tiles2};

/// An attack wave on its way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wave {
    /// Its units still alive, in id order.
    pub units: Vec<u32>,
    pub launched_with: usize,
    pub launched_at: u32,
}

pub(crate) fn think(ai: &mut Ai, game: &Game, view: &View, out: &mut Orders) {
    let es = &game.state.entities;
    let rules = &game.rules;
    let alive = |id: &u32| game.state.entity(*id).is_some_and(|e| e.owner == ai.player);
    let s = &ai.settings;

    // The wave out now: drop the dead, and come home if it has lost too many.
    if let Some(w) = &mut ai.wave {
        w.units.retain(alive);
        let gone = w.units.is_empty();
        if gone || w.units.len() * 100 < w.launched_with * s.retreat_percent {
            if !gone && let Some(rally) = rally(game, view, s.rally_distance) {
                out.push(w.units.clone(), CommandOrder::Move { x: rally.x, y: rally.y });
            }
            ai.wave = None;
            ai.wave_size = (ai.wave_size + s.wave_growth).min(s.wave_cap);
        }
    }
    let in_wave: BTreeSet<u32> = ai.wave.iter().flat_map(|w| w.units.iter().copied()).collect();
    let fighter = |e: &Entity| {
        let k = rules.kind(e.kind);
        !k.building && k.weapon.is_some() && k.harvester.is_none()
    };
    let home: Vec<&Entity> =
        view.mine.iter().map(|&i| &es[i]).filter(|e| fighter(e) && !in_wave.contains(&e.id)).collect();

    // Defence: armed enemies near any of its buildings, or near a harvester working close to home, the one nearest
    // home first.
    let r2 = (s.defend_radius as i64) * (s.defend_radius as i64);
    let near_home = |e: &Entity| view.home.is_some_and(|h| tiles2(e, h) <= 4 * r2);
    let guarded: Vec<Tile> = view
        .mine
        .iter()
        .map(|&i| &es[i])
        .filter(|e| rules.kind(e.kind).building || (rules.kind(e.kind).harvester.is_some() && near_home(e)))
        .map(|e| centre_tile(game, e))
        .collect();
    let armed = |e: &Entity| rules.kind(e.kind).weapon.is_some();
    let threat = view
        .enemies
        .iter()
        .map(|&i| &es[i])
        .filter(|e| !rules.kind(e.kind).building && armed(e))
        .filter(|e| guarded.iter().any(|&b| tiles2(e, b) <= r2))
        .min_by_key(|e| (view.home.map_or(0, |h| tiles2(e, h)), e.id));
    if let Some(t) = threat {
        let send: Vec<u32> = home.iter().filter(|e| e.order != Order::Attack).map(|e| e.id).collect();
        if !send.is_empty() {
            out.push(send, CommandOrder::Attack { target: t.id });
        }
    } else if let Some(rally) = rally(game, view, s.rally_distance) {
        // Gather: idle units away from the rally point go there.
        let near = (3 + home.len() as i64 / 3).pow(2);
        let stray: Vec<u32> =
            home.iter().filter(|e| e.order == Order::Idle && tiles2(e, rally) > near).map(|e| e.id).collect();
        if !stray.is_empty() {
            out.push(stray, CommandOrder::Move { x: rally.x, y: rally.y });
        }
        // Launch once enough are waiting there.
        let ready: Vec<u32> = home
            .iter()
            .filter(|e| e.order == Order::Idle && tiles2(e, rally) <= near)
            .filter(|e| e.health * 2 >= rules.kind(e.kind).max_health)
            .map(|e| e.id)
            .collect();
        if ai.wave.is_none() && game.state.tick >= s.first_wave_tick && ready.len() >= ai.wave_size {
            ai.waves_sent += 1;
            ai.wave = Some(Wave { launched_with: ready.len(), units: ready, launched_at: game.state.tick });
        }
    }

    // Steer the wave.
    let Some(w) = &ai.wave else { return };
    let units: Vec<&Entity> = w.units.iter().filter_map(|&id| game.state.entity(id)).collect();
    let n = units.len() as i64;
    let (sx, sy) = units.iter().fold((0, 0), |(x, y), e| (x + e.tile().x as i64, y + e.tile().y as i64));
    let centre = Tile { x: (sx / n) as i32, y: (sy / n) as i32 };
    let enemies = || view.enemies.iter().map(|&i| &es[i]);
    let near_armed =
        enemies().filter(|e| armed(e) && tiles2(e, centre) <= 64).min_by_key(|e| (tiles2(e, centre), e.id));
    let building = enemies()
        .filter(|e| rules.kind(e.kind).building && !rules.kind(e.kind).wall)
        .min_by_key(|e| (dist2(centre_tile(game, e), centre), e.id));
    let Some(target) = near_armed.or(building).or_else(|| enemies().min_by_key(|e| (tiles2(e, centre), e.id))) else {
        return;
    };
    let target_armed = armed(target);
    let send: Vec<u32> = units
        .iter()
        .filter(|e| match e.order {
            Order::Attack => {
                e.target != Some(target.id)
                    && target_armed
                    && !e.target.and_then(|t| game.state.entity(t)).is_some_and(armed)
            }
            _ => true,
        })
        .map(|e| e.id)
        .collect();
    if !send.is_empty() {
        out.push(send, CommandOrder::Attack { target: target.id });
    }
}

/// Where new units gather: `distance` tiles out from home towards the nearest enemy building, on the nearest tile a
/// unit can stand on.
pub(crate) fn rally(game: &Game, view: &View, distance: i32) -> Option<Tile> {
    let home = view.home?;
    let aim = view.enemy_home.map_or(home, |e| toward(home, e, distance));
    let pf = &game.pathfinder;
    let free = |t: Tile| pf.passable(t.x, t.y) && game.state.resource[game.map.index(t.x, t.y)] == 0;
    (0..game.map.width.max(game.map.height)).find_map(|r| {
        (aim.y - r..=aim.y + r)
            .flat_map(|y| (aim.x - r..=aim.x + r).map(move |x| Tile { x, y }))
            .filter(|t| (t.x - aim.x).abs().max((t.y - aim.y).abs()) == r)
            .find(|&t| game.map.in_bounds(t.x, t.y) && free(t))
    })
}
