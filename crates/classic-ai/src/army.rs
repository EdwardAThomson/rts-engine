//! The army: new units gather at a rally point a few tiles out from the yard towards the nearest enemy, where a
//! player can see them massing. Armed enemies near its buildings, or near its harvesters close to home, draw out
//! every unit at home.
//!
//! **Waves.** Once the first-wave time has passed and at least `wave_size` units wait at the rally point, it looks
//! for an objective: an enemy building whose defenders (armed enemies near it) the waiting units would
//! beat by `attack_margin` percent, judged by a Lanchester-style count of health times damage per tick from the
//! rules' own numbers. Of those it takes the nearest. With none it keeps waiting and growing, unless `wave_cap`
//! units are waiting, when it goes for the least defended objective anyway. A wave first gathers at a staging point
//! out of reach of the objective's defenders, waiting there for its slowest unit, so it arrives together instead of
//! in a column, then attacks, fast units waiting for slow ones until something can shoot at them: armed
//! enemies near it first, then the objective, then the nearest enemy building. A wave that falls below
//! `retreat_percent` of its starting size comes home, and the next one waits for `wave_growth` more units.
//!
//! **Last stand.** When its income has stopped (nothing delivered for `broke_ticks`, and too few credits for a combat
//! unit), nothing will get better by waiting, so every unit at home goes at once.

use std::collections::BTreeSet;

use classic_sim::{CommandOrder, Entity, Game, Order, Tile};

use classic_sim::map::TILE;

use crate::geo::{Point, at, centre, d2, standable, tiles2, toward};
use crate::{Ai, Orders, View};
use rts_core::imath::isqrt;

/// An attack wave on its way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wave {
    /// Its units still alive, in id order.
    pub units: Vec<u32>,
    pub launched_with: usize,
    pub launched_at: u32,
    /// The enemy entity it set out to destroy.
    pub objective: u32,
    /// Where it gathers before attacking, until it has gathered.
    pub staging: Option<Tile>,
    /// The tick it stops waiting at the staging point for stragglers: `stage_ticks` after its slowest unit should
    /// have got there.
    pub gather_until: u32,
    /// Sent without the odds (at `wave_cap`, or a last stand): it doesn't turn back when the odds turn against it.
    pub committed: bool,
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
    let home: Vec<&Entity> =
        view.mine.iter().map(|&i| &es[i]).filter(|e| fighter(game, e) && !in_wave.contains(&e.id)).collect();

    // Defence: armed enemies near any of its buildings, or near a harvester working close to home, the one nearest
    // home first.
    let r2 = (s.defend_radius as i64) * (s.defend_radius as i64);
    let near_home = |e: &Entity| view.home.is_some_and(|h| tiles2(at(game, e), h) <= 4 * r2);
    let guarded: Vec<Point> = view
        .mine
        .iter()
        .map(|&i| &es[i])
        .filter(|e| rules.kind(e.kind).building || (rules.kind(e.kind).harvester.is_some() && near_home(e)))
        .map(|e| at(game, e))
        .collect();
    let threat = view
        .enemies
        .iter()
        .map(|&i| &es[i])
        .filter(|e| !rules.kind(e.kind).building && armed(game, e))
        .filter(|e| guarded.iter().any(|&b| tiles2(at(game, e), b) <= r2))
        .min_by_key(|e| (view.home.map_or(0, |h| d2(at(game, e), h)), e.id));
    let last_stand = broke(ai, game);
    if let Some(t) = threat {
        let send: Vec<u32> = home.iter().filter(|e| e.order != Order::Attack).map(|e| e.id).collect();
        if !send.is_empty() {
            out.push(send, CommandOrder::Attack { target: t.id });
        }
    } else if let Some(rally) = rally(game, view, s.rally_distance) {
        let off = |e: &Entity| tiles2(at(game, e), centre(rally));
        // Gather: idle units away from the rally point go there.
        let near = (3 + home.len() as i64 / 3).pow(2);
        let stray: Vec<u32> = home.iter().filter(|e| e.order == Order::Idle && off(e) > near).map(|e| e.id).collect();
        if !stray.is_empty() && !last_stand {
            out.push(stray, CommandOrder::Move { x: rally.x, y: rally.y });
        }
        // Launch once enough are waiting there and they have somewhere worth going, or everything when broke.
        let ready: Vec<&Entity> = if last_stand {
            home.clone()
        } else {
            home.iter()
                .filter(|e| e.order == Order::Idle && off(e) <= near)
                .filter(|e| e.health * 2 >= rules.kind(e.kind).max_health)
                .copied()
                .collect()
        };
        let time = game.state.tick >= s.first_wave_tick;
        if ai.wave.is_none() && time && !ready.is_empty() && (ready.len() >= ai.wave_size || last_stand) {
            let any = last_stand || ready.len() >= s.wave_cap;
            if let Some((obj, wins)) = objective(game, view, &ready, rally, s.attack_margin, any) {
                let staging = stage(game, view, rally, obj);
                let units: Vec<u32> = ready.iter().map(|e| e.id).collect();
                // The slowest unit's trip, with half again for the way round.
                let trip = ready
                    .iter()
                    .map(|e| {
                        isqrt(d2(at(game, e), centre(staging)) as u64) as i64 * 3 / 2 / rules.kind(e.kind).speed.max(1)
                    })
                    .max()
                    .unwrap_or(0);
                ai.waves_sent += 1;
                out.push(units.clone(), CommandOrder::Move { x: staging.x, y: staging.y });
                ai.wave = Some(Wave {
                    launched_with: units.len(),
                    units,
                    launched_at: game.state.tick,
                    objective: obj.id,
                    staging: Some(staging),
                    gather_until: game.state.tick + trip as u32 + s.stage_ticks,
                    committed: !wins,
                });
                return;
            }
        }
    }

    // Steer the wave.
    let rally_distance = s.rally_distance;
    let Some(w) = &mut ai.wave else { return };
    let units: Vec<&Entity> = w.units.iter().filter_map(|&id| game.state.entity(id)).collect();
    let n = units.len() as i64;
    let (sx, sy) = units.iter().fold((0, 0), |(x, y), e| (x + e.x, y + e.y));
    let middle = Point { x: sx / n, y: sy / n };
    let from_middle = |e: &Entity| d2(at(game, e), middle);
    let enemies = || view.enemies.iter().map(|&i| &es[i]);
    let near_armed = enemies()
        .filter(|e| armed(game, e) && from_middle(e) <= 64 * TILE * TILE)
        .min_by_key(|e| (from_middle(e), e.id));

    // Gathering: everyone to the staging point, until most are there, it has waited long enough for the slowest, or
    // it is in a fight.
    if let Some(spot) = w.staging {
        let away = |e: &Entity| tiles2(at(game, e), centre(spot)) > 16;
        let there = units.iter().filter(|e| !away(e)).count();
        let waited = game.state.tick >= w.gather_until;
        if near_armed.is_none() && there * 5 < units.len() * 4 && !waited {
            let idle: Vec<u32> = units.iter().filter(|e| e.order == Order::Idle && away(e)).map(|e| e.id).collect();
            if !idle.is_empty() {
                out.push(idle, CommandOrder::Move { x: spot.x, y: spot.y });
            }
            return;
        }
        w.staging = None;
    }

    let objective = game.state.entity(w.objective).filter(|e| e.owner != ai.player);

    // Turn back while it still can if the fight ahead has turned against it: the survivors join the next wave
    // instead of dying at the turrets.
    if !w.committed {
        let mut guard: Vec<&Entity> = objective.map(|o| defenders(game, view, o)).unwrap_or_default();
        for e in enemies().filter(|e| threatens(game, e, middle)) {
            if !guard.iter().any(|g| g.id == e.id) {
                guard.push(e);
            }
        }
        if strength(game, &units, &guard) < strength(game, &guard, &units) {
            if let Some(rally) = rally(game, view, rally_distance) {
                out.push(w.units.clone(), CommandOrder::Move { x: rally.x, y: rally.y });
            }
            ai.wave = None;
            return;
        }
    }
    let building = enemies()
        .filter(|e| rules.kind(e.kind).building && !rules.kind(e.kind).wall)
        .min_by_key(|e| (from_middle(e), e.id));
    let Some(target) =
        near_armed.or(objective).or(building).or_else(|| enemies().min_by_key(|e| (from_middle(e), e.id)))
    else {
        return;
    };
    // March in step: a unit more than three tiles nearer the target than the wave's rearmost waits where it is for
    // the slower ones, unless an armed enemy can already reach it or it them. Without this, fast units arrive alone
    // and die before slow infantry catch up.
    let reach = |e: &Entity| rules.kind(e.kind).weapon.map_or(0, |w| rules.weapon(w).range);
    // A unit told to attack that is out of range with no tile in range it can get to (boxed in at home, say) leaves
    // the wave, so the others don't wait for it.
    let stuck = |e: &Entity| {
        let Some(t) = e.target.and_then(|t| game.state.entity(t)) else { return false };
        let r = reach(e);
        let in_range = |x: i64, y: i64| (t.x - x) * (t.x - x) + (t.y - y) * (t.y - y) <= r * r;
        if e.order != Order::Attack || !e.path.is_empty() || in_range(e.x, e.y) {
            return false;
        }
        let (here, n) = (e.tile(), (r / TILE) as i32 + 1);
        let tt = t.tile();
        !(tt.y - n..=tt.y + n).any(|y| {
            (tt.x - n..=tt.x + n).any(|x| {
                in_range(x as i64 * TILE + TILE / 2, y as i64 * TILE + TILE / 2)
                    && game.pathfinder.connected((here.x, here.y), (x, y))
            })
        })
    };
    w.units.retain(|&id| game.state.entity(id).is_none_or(|e| !stuck(e)));
    let units: Vec<&Entity> = units.into_iter().filter(|e| !stuck(e)).collect();
    let to_target = |e: &Entity| isqrt(d2(at(game, e), at(game, target)) as u64) as i64;
    let rear = units.iter().map(|e| to_target(e)).max().unwrap_or(0);
    let engaged = |e: &Entity| {
        enemies().any(|x| {
            let r = reach(x).max(reach(e)) + TILE;
            armed(game, x) && d2(at(game, x), at(game, e)) <= r * r
        })
    };
    let ahead: BTreeSet<u32> =
        units.iter().filter(|e| to_target(e) + 3 * TILE < rear && !engaged(e)).map(|e| e.id).collect();
    for e in units.iter().filter(|e| ahead.contains(&e.id) && e.order == Order::Attack) {
        // The tile it is stepping into, not the one its centre is in, so mirrored units stop on mirrored tiles.
        let t = classic_sim::movement::step_tile(e).unwrap_or(e.tile());
        out.push(vec![e.id], CommandOrder::Move { x: t.x, y: t.y });
    }
    let target_armed = armed(game, target);
    let send: Vec<u32> = units
        .iter()
        .filter(|e| !ahead.contains(&e.id))
        .filter(|e| match e.order {
            Order::Attack => {
                e.target != Some(target.id)
                    && target_armed
                    && !e.target.and_then(|t| game.state.entity(t)).is_some_and(|t| armed(game, t))
            }
            _ => true,
        })
        .map(|e| e.id)
        .collect();
    if !send.is_empty() {
        out.push(send, CommandOrder::Attack { target: target.id });
    }
}

fn armed(game: &Game, e: &Entity) -> bool {
    game.rules.kind(e.kind).weapon.is_some()
}

/// Whether an armed enemy can join a fight at `p`: a unit within eight tiles, an armed building whose weapon reaches
/// within two tiles of it.
fn threatens(game: &Game, e: &Entity, p: Point) -> bool {
    let k = game.rules.kind(e.kind);
    let Some(w) = k.weapon.filter(|_| k.harvester.is_none()) else { return false };
    let reach = if k.building { game.rules.weapon(w).range + 2 * TILE } else { 8 * TILE };
    d2(at(game, e), p) <= reach * reach
}

/// An armed unit that isn't a harvester: what the army is made of.
pub(crate) fn fighter(game: &Game, e: &Entity) -> bool {
    let k = game.rules.kind(e.kind);
    !k.building && k.weapon.is_some() && k.harvester.is_none()
}

/// Whether its income has stopped: nothing delivered for `broke_ticks`, and not enough credits for its cheapest
/// combat unit.
fn broke(ai: &Ai, game: &Game) -> bool {
    let rules = &game.rules;
    let credits = game.state.players.iter().find(|p| p.id == ai.player).map_or(0, |p| p.credits);
    let cheapest = rules
        .kinds
        .iter()
        .filter(|k| !k.building && k.weapon.is_some() && k.harvester.is_none() && k.built_at.is_some())
        .map(|k| k.cost)
        .min()
        .unwrap_or(0);
    credits < cheapest && game.state.tick >= ai.delivered_at + ai.settings.broke_ticks
}

/// Damage per 1000 ticks one armed entity does to another, from its weapon and the damage table.
fn rate(game: &Game, from: &Entity, to: &Entity) -> i64 {
    let rules = &game.rules;
    let Some(w) = rules.kind(from.kind).weapon.map(|w| rules.weapon(w)) else { return 0 };
    w.damage * rules.combat.table[w.warhead][rules.kind(to.kind).armour] * 10 / w.reload.max(1) as i64
}

/// A side's fighting strength against another: its total health times its damage rate, each unit's rate averaged
/// over the other side's health. Lanchester's square law in integers: the side with more wins, and keeps about
/// `sqrt(1 - weaker / stronger)` of itself.
fn strength(game: &Game, side: &[&Entity], against: &[&Entity]) -> i64 {
    let health: i64 = side.iter().map(|e| e.health).sum();
    let theirs: i64 = against.iter().map(|e| e.health).sum::<i64>().max(1);
    let rate: i64 =
        side.iter().map(|a| against.iter().map(|d| d.health * rate(game, a, d)).sum::<i64>() / theirs).sum();
    health * rate
}

/// The armed enemies that would fight for `target`: armed buildings whose weapons reach near it, and armed units
/// within twelve tiles.
fn defenders<'a>(game: &'a Game, view: &View, target: &Entity) -> Vec<&'a Entity> {
    let rules = &game.rules;
    let spot = at(game, target);
    view.enemies
        .iter()
        .map(|&i| &game.state.entities[i])
        .filter(|e| {
            let k = rules.kind(e.kind);
            let Some(w) = k.weapon else { return false };
            if k.harvester.is_some() {
                return false;
            }
            let reach = if k.building { rules.weapon(w).range + 3 * TILE } else { 12 * TILE };
            d2(at(game, e), spot) <= reach * reach
        })
        .collect()
}

/// Where a wave of `units` gathered at `from` should go: the nearest enemy building or harvester it would beat the
/// defenders of by `margin` percent, or, when `any`, the one with the weakest defence (the nearest of equals). The
/// flag says whether it has the odds.
fn objective<'a>(
    game: &'a Game,
    view: &View,
    units: &[&Entity],
    from: Tile,
    margin: i64,
    any: bool,
) -> Option<(&'a Entity, bool)> {
    let rules = &game.rules;
    let mut best: Option<(i64, i64, u32, &Entity)> = None;
    for e in view.enemies.iter().map(|&i| &game.state.entities[i]) {
        let k = rules.kind(e.kind);
        if !(k.building && !k.wall) && k.harvester.is_none() {
            continue;
        }
        let guard = defenders(game, view, e);
        let (ours, theirs) = (strength(game, units, &guard), strength(game, &guard, units));
        let wins = ours * 100 >= theirs * margin;
        let d = d2(at(game, e), centre(from));
        let key = if wins { (0, d, e.id) } else { (1, theirs, e.id) };
        if (wins || any) && best.is_none_or(|(a, b, c, _)| key < (a, b, c)) {
            best = Some((key.0, key.1, key.2, e));
        }
    }
    best.map(|b| (b.3, b.0 == 0))
}

/// The staging point for an attack on `target` from `from`: on the line between them, five tiles out of reach of
/// the armed buildings near the target, on the nearest tile a unit can stand on.
fn stage(game: &Game, view: &View, from: Tile, target: &Entity) -> Tile {
    let rules = &game.rules;
    let (aim, start) = (at(game, target), centre(from));
    let reach = defenders(game, view, target)
        .iter()
        .filter_map(|e| rules.kind(e.kind).weapon.filter(|_| rules.kind(e.kind).building))
        .map(|w| rules.weapon(w).range)
        .max()
        .unwrap_or(0)
        + 5 * TILE;
    let gap = isqrt(d2(aim, start) as u64) as i64;
    if gap <= reach {
        return from;
    }
    standable(game, toward(start, aim, gap - reach)).unwrap_or(from)
}

/// Where new units gather: `distance` tiles out from home towards the nearest enemy building, on the nearest tile a
/// unit can stand on.
pub(crate) fn rally(game: &Game, view: &View, distance: i32) -> Option<Tile> {
    let home = view.home?;
    let aim = view.enemy_home.map_or(home, |e| toward(home, e, distance as i64 * TILE));
    standable(game, aim)
}
