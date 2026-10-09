//! Combat (rules-combat.md): the phase that runs after commands and before movement. In order, each over entities
//! in id order: armed units look for targets (staggered, once every `scan_every` ticks), weapons turn towards their
//! targets, ready weapons fire, projectiles fly and burst, damage is applied in the order it was dealt, and the
//! destroyed are removed, their death blasts applied as one more pass. Because every shot is fired before any
//! damage lands, two units that kill each other on the same tick both get their shot.
//!
//! Distances are compared squared, never square-rooted, except where a true distance is needed (projectile travel,
//! the target score), which uses the shared integer `isqrt`. Directions are integer facings, 0 to 255 clockwise from
//! north, found by `facing_to` from a checked-in table, never floating point.

use rts_core::hash::{Canon, CanonHasher};
use rts_core::imath::isqrt;
use rts_core::rng::random_int;

use crate::map::{TILE, Tile};
use crate::movement;
use crate::path::Pathfinder;
use crate::power::Power;
use crate::units::{Rules, WeaponId};
use crate::vision;
use crate::world::{self, Entity, Event, GameState, Order};

/// `TABLE[i]` is the facing of the direction (i, -64), i from 0 to 64: `round(atan(i / 64) * 128 / pi)`, made
/// once by a script and checked in so the simulation never calls a floating-point arctangent.
const ATAN: [i64; 65] = [
    0, 1, 1, 2, 3, 3, 4, 4, 5, 6, 6, 7, 8, 8, 9, 9, 10, 11, 11, 12, 12, 13, 13, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19,
    19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 25, 26, 26, 27, 27, 27, 28, 28, 29, 29, 29, 30, 30, 30, 31, 31,
    31, 32, 32,
];

/// The facing from the origin towards (dx, dy): 0 is north (negative y), 64 east, 128 south, 192 west.
pub fn facing_to(dx: i64, dy: i64) -> i64 {
    if dx == 0 && dy == 0 {
        return 0;
    }
    let (ax, ay) = (dx.abs(), dy.abs());
    // The angle from the vertical axis within the quadrant, 0 to 64.
    let t = if ax <= ay { ATAN[(ax * 64 / ay) as usize] } else { 64 - ATAN[(ay * 64 / ax) as usize] };
    match (dx >= 0, dy < 0) {
        (true, true) => t,
        (true, false) => 128 - t,
        (false, false) => 128 + t,
        (false, true) => (256 - t) & 255,
    }
}

/// Turn `from` towards `to` by at most `rate`, the shorter way round; exactly opposite turns clockwise.
pub fn turn(from: i64, to: i64, rate: i64) -> i64 {
    let diff = (to - from).rem_euclid(256);
    let step = if diff <= 128 { diff.min(rate) } else { -(256 - diff).min(rate) };
    (from + step).rem_euclid(256)
}

/// How far apart two facings are, 0 to 128.
fn facing_gap(a: i64, b: i64) -> i64 {
    let d = (a - b).rem_euclid(256);
    d.min(256 - d)
}

/// A shell or rocket in flight. It flies straight at `speed` per tick towards the point it was aimed at, and
/// bursts there; it does not follow its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub id: u32,
    pub weapon: WeaponId,
    /// Who fired it, and their owner at the time.
    pub firer: u32,
    pub owner: u32,
    /// What it was aimed at, which takes a full hit if the burst lands on its body.
    pub target: u32,
    pub x: i64,
    pub y: i64,
    pub to_x: i64,
    pub to_y: i64,
}

/// A projectile as the state hash writes it, with its weapon spelt as the generic id.
pub(crate) struct ProjectileCanon<'a>(pub &'a Projectile, pub &'a [String]);

impl Canon for ProjectileCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let p = self.0;
        w.object()
            .field("firer", &p.firer)
            .field("id", &p.id)
            .field("owner", &p.owner)
            .field("target", &p.target)
            .field("toX", &p.to_x)
            .field("toY", &p.to_y)
            .field("weapon", self.1[p.weapon.0 as usize].as_str())
            .field("x", &p.x)
            .field("y", &p.y)
            .end();
    }
}

/// One hit waiting to be applied: `band` is 100 for a full hit, 50 for the outer splash ring.
struct Damage {
    target: u32,
    attacker: u32,
    owner: u32,
    weapon: WeaponId,
    band: i64,
}

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

fn dist2(a: &Entity, x: i64, y: i64) -> i64 {
    (a.x - x) * (a.x - x) + (a.y - y) * (a.y - y)
}

/// The damage table's percent for this weapon against this entity; 0 means it can't be hurt by it at all.
fn table(rules: &Rules, w: WeaponId, e: &Entity) -> i64 {
    rules.combat.table[rules.weapon(w).warhead][rules.kind(e.kind).armour]
}

/// Whether the burst at (x, y) lands on `e`'s body: a circle of 112 for a unit, its footprint for a building.
fn on_body(rules: &Rules, e: &Entity, x: i64, y: i64) -> bool {
    let k = rules.kind(e.kind);
    if k.building {
        let t = e.tile();
        let (x0, y0) = (t.x as i64 * TILE, t.y as i64 * TILE);
        (x0..x0 + k.width as i64 * TILE).contains(&x) && (y0..y0 + k.height as i64 * TILE).contains(&y)
    } else {
        dist2(e, x, y) <= 112 * 112
    }
}

/// Whether `e`'s weapon could ever hurt `t`: an enemy its warhead affects. Walls are never fair game unless ordered.
fn can_hit(rules: &Rules, e: &Entity, t: &Entity) -> bool {
    let Some(w) = rules.kind(e.kind).weapon else { return false };
    t.owner != e.owner && table(rules, w, t) > 0
}

/// Pick the best target in sight (rules-combat.md, "Target selection and auto-targeting").
fn scan(state: &GameState, rules: &Rules, i: usize) -> Option<u32> {
    let e = &state.entities[i];
    let k = rules.kind(e.kind);
    let w = rules.weapon(k.weapon?);
    let mut best: Option<(i64, u32)> = None;
    for t in &state.entities {
        let tk = rules.kind(t.kind);
        // Under fog, only what its owner can see.
        if tk.wall || !can_hit(rules, e, t) || !vision::visible(state, rules, e.owner, t) {
            continue;
        }
        let d2 = dist2(e, t.x, t.y);
        if d2 > k.sight * k.sight {
            continue;
        }
        let mut score = 0;
        if tk.weapon.is_some() && can_hit(rules, t, e) {
            score += 400;
        }
        if e.last_attacker.is_some_and(|(id, at)| id == t.id && state.tick < at + 45) {
            score += 300;
        }
        if d2 <= w.range * w.range {
            score += 200;
        }
        if tk.building {
            score -= 500;
        }
        score -= isqrt(d2 as u64) as i64 * 10 / TILE;
        if e.target == Some(t.id) {
            score += 50;
        }
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, t.id));
        }
    }
    best.map(|(_, id)| id)
}

/// Where an attacker heads to reach `target`: its tile, or for a building the open tile beside it nearest the
/// attacker, then nearest the middle of the map. When the target can't be reached from where the attacker stands (a
/// building boxed in by others, or a unit walled in behind them), the nearest reachable tile it could fire from
/// instead, so a unit doesn't stand still out of range.
fn approach(pf: &Pathfinder, rules: &Rules, from: &Entity, target: &Entity) -> Option<Tile> {
    let k = rules.kind(target.kind);
    let t = target.tile();
    let here = from.tile();
    let (w, h) = pf.size();
    let key =
        |r: &Tile| ((r.x - here.x) * (r.x - here.x) + (r.y - here.y) * (r.y - here.y), r.off_middle(w, h), r.y, r.x);
    // A unit off the passable grid (still stepping out of a factory) can't be judged; it heads for the target as
    // before.
    let off_grid = !pf.passable(here.x, here.y);
    if !k.building {
        if off_grid || pf.connected((here.x, here.y), (t.x, t.y)) {
            return Some(t);
        }
    } else {
        let ring = (t.y - 1..=t.y + k.height).flat_map(|y| (t.x - 1..=t.x + k.width).map(move |x| Tile { x, y }));
        let ring: Vec<Tile> = ring.filter(|r| pf.passable(r.x, r.y)).collect();
        if off_grid || ring.iter().any(|r| pf.connected((here.x, here.y), (r.x, r.y))) {
            return ring.into_iter().filter(|r| off_grid || pf.connected((here.x, here.y), (r.x, r.y))).min_by_key(key);
        }
    }
    let w_rules = rules.weapon(rules.kind(from.kind).weapon?);
    let reach = (w_rules.range / TILE) as i32 + 1;
    let fires_from = |r: &Tile| {
        let (cx, cy) = (r.x as i64 * TILE + TILE / 2, r.y as i64 * TILE + TILE / 2);
        let d2 = (cx - target.x) * (cx - target.x) + (cy - target.y) * (cy - target.y);
        d2 <= w_rules.range * w_rules.range && d2 >= w_rules.min_range * w_rules.min_range
    };
    (t.y - reach..=t.y + reach)
        .flat_map(|y| (t.x - reach..=t.x + reach).map(move |x| Tile { x, y }))
        .filter(|r| pf.connected((here.x, here.y), (r.x, r.y)) && fires_from(r))
        .min_by_key(key)
}

/// The combat phase of one tick.
pub fn tick(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let short: Vec<bool> = Power::all(state, rules).iter().map(|p| p.is_short()).collect();
    let mut damage = Vec::new();

    for i in 0..state.entities.len() {
        let k = rules.kind(state.entities[i].kind);
        let Some(wid) = k.weapon else { continue };
        let w = rules.weapon(wid);
        // Drop a target that has gone, or a unit that has slipped out of its owner's sight under fog (a building
        // stays a target as its owner last saw it); an attack order ends with it.
        let owner = state.entities[i].owner;
        let target = state.entities[i].target.and_then(|id| index(state, id)).filter(|&t| {
            let t = &state.entities[t];
            rules.kind(t.kind).building || vision::visible(state, rules, owner, t)
        });
        if target.is_none() {
            let e = &mut state.entities[i];
            e.target = None;
            if e.order == Order::Attack {
                e.order = Order::Idle;
                movement::halt(e);
            }
        }
        // 1. Scan, unless moving under orders or attacking a target the player chose.
        let e = &state.entities[i];
        if e.order == Order::Idle && (tick + e.id).is_multiple_of(rules.combat.scan_every) {
            let found = scan(state, rules, i);
            if found.is_some() && found != state.entities[i].target {
                events.push(Event::TargetAcquired { tick, unit: state.entities[i].id, target: found.unwrap_or(0) });
            }
            state.entities[i].target = found;
        }
        let Some(t) = state.entities[i].target.and_then(|id| index(state, id)) else {
            state.entities[i].reload = state.entities[i].reload.saturating_sub(1);
            continue;
        };
        let (tx, ty, tid) = (state.entities[i].x, state.entities[i].y, state.entities[t].id);
        let (dx, dy) = (state.entities[t].x - tx, state.entities[t].y - ty);
        let d2 = dx * dx + dy * dy;
        let in_range = d2 <= w.range * w.range && d2 >= w.min_range * w.min_range;
        // An attack order closes in until the target is in range, then stands.
        if state.entities[i].order == Order::Attack && !k.building {
            if in_range {
                movement::halt(&mut state.entities[i]);
            } else if let Some(goal) = approach(pf, rules, &state.entities[i], &state.entities[t])
                // Set off at once, then follow a moving target on scan ticks.
                && (state.entities[i].path.is_empty() || (tick + state.entities[i].id).is_multiple_of(rules.combat.scan_every))
                && state.entities[i].path.back() != Some(&goal)
                && state.entities[i].tile() != goal
            {
                state.entities[i].path = movement::route(pf, &state.entities[i], goal);
            }
        }
        // 2. Turn towards it.
        let want = facing_to(dx, dy);
        let e = &mut state.entities[i];
        e.facing = turn(e.facing, want, k.turn_rate);
        e.reload = e.reload.saturating_sub(1);
        // 3. Fire when in range, on target, reloaded and powered.
        let powered = !w.needs_power || !state.players.iter().position(|p| p.id == e.owner).is_some_and(|p| short[p]);
        if !in_range || facing_gap(e.facing, want) > 8 || e.reload > 0 || !powered {
            continue;
        }
        e.reload = w.reload;
        let (unit, owner, at) = (e.id, e.owner, e.tile());
        events.push(Event::Fired { tick, unit, weapon: wid, target: tid });
        // Firing shows the shooter to the player it fired at, so a long gun can't hide in the fog.
        if let (Some(v), Some(fog)) = (state.vision.as_mut(), rules.fog.as_ref()) {
            v.reveal(state.entities[t].owner, at, tick, fog.reveal_ticks);
        }
        if w.speed == 0 {
            damage.push(Damage { target: tid, attacker: unit, owner, weapon: wid, band: 100 });
            continue;
        }
        let (mut ox, mut oy) = (0, 0);
        if w.scatter > 0 {
            for _ in 0..4 {
                let span = (2 * w.scatter + 1) as u32;
                let x = random_int(&mut state.rng, span) as i64 - w.scatter;
                let y = random_int(&mut state.rng, span) as i64 - w.scatter;
                if x * x + y * y <= w.scatter * w.scatter {
                    (ox, oy) = (x, y);
                    break;
                }
            }
        }
        let id = state.next_id;
        state.next_id += 1;
        let (to_x, to_y) = (state.entities[t].x + ox, state.entities[t].y + oy);
        state.projectiles.push(Projectile {
            id,
            weapon: wid,
            firer: unit,
            owner,
            target: tid,
            x: tx,
            y: ty,
            to_x,
            to_y,
        });
        events.push(Event::ProjectileSpawned { tick, projectile: id, weapon: wid, x: tx, y: ty, to_x, to_y });
    }

    // 4. Projectiles fly, in id order, and those that arrive burst.
    let mut flying = Vec::with_capacity(state.projectiles.len());
    for mut p in std::mem::take(&mut state.projectiles) {
        let w = rules.weapon(p.weapon);
        let (dx, dy) = (p.to_x - p.x, p.to_y - p.y);
        let left = isqrt((dx * dx + dy * dy) as u64) as i64;
        if left > w.speed {
            p.x += dx * w.speed / left;
            p.y += dy * w.speed / left;
            flying.push(p);
            continue;
        }
        events.push(Event::ProjectileHit { tick, projectile: p.id, weapon: p.weapon, x: p.to_x, y: p.to_y });
        burst(state, rules, &p, &mut damage);
    }
    state.projectiles = flying;

    // 5. Apply damage in the order it was dealt.
    apply(state, rules, &damage, events);

    // 6. Remove the destroyed, in id order; their death blasts land at once, as one more pass.
    let dead: Vec<usize> = (0..state.entities.len()).filter(|&i| state.entities[i].health <= 0).collect();
    if dead.is_empty() {
        return;
    }
    let mut blasts = Vec::new();
    for &i in &dead {
        let e = &state.entities[i];
        let k = rules.kind(e.kind);
        events.push(Event::Destroyed {
            tick,
            entity: e.id,
            kind: e.kind,
            owner: e.owner,
            killer: e.last_attacker.map(|(id, _)| id),
            x: e.x,
            y: e.y,
        });
        if let Some(d) = k.death {
            let p = Projectile {
                id: 0,
                weapon: d,
                firer: e.id,
                owner: e.owner,
                target: e.id,
                x: e.x,
                y: e.y,
                to_x: e.x,
                to_y: e.y,
            };
            splash(state, rules, &p, None, &mut blasts);
        }
    }
    for &i in dead.iter().rev() {
        let e = state.entities.remove(i);
        if rules.kind(e.kind).building {
            world::occupy(pf, rules, &e, false);
        }
    }
    // Blasts don't hurt those already destroyed this tick; anything they kill goes next tick.
    blasts.retain(|d| index(state, d.target).is_some());
    apply(state, rules, &blasts, events);
}

/// A projectile bursts at its aim point: a full hit on its target if the burst lands on its body, and splash on
/// everyone else around.
fn burst(state: &GameState, rules: &Rules, p: &Projectile, out: &mut Vec<Damage>) {
    let direct = index(state, p.target).filter(|&t| on_body(rules, &state.entities[t], p.to_x, p.to_y));
    if let Some(t) = direct {
        let target = state.entities[t].id;
        out.push(Damage { target, attacker: p.firer, owner: p.owner, weapon: p.weapon, band: 100 });
    }
    splash(state, rules, p, direct.map(|t| state.entities[t].id), out);
}

/// Splash in two bands round (to_x, to_y): full within half the radius, half within all of it. It hurts both
/// sides (its own at `own_splash_percent`, applied later) but never the firer, nor the one already hit directly.
fn splash(state: &GameState, rules: &Rules, p: &Projectile, skip: Option<u32>, out: &mut Vec<Damage>) {
    let r = rules.weapon(p.weapon).splash;
    if r == 0 {
        return;
    }
    for e in &state.entities {
        if e.id == p.firer || Some(e.id) == skip {
            continue;
        }
        let d2 = dist2(e, p.to_x, p.to_y);
        let band = if d2 * 4 <= r * r {
            100
        } else if d2 <= r * r {
            50
        } else {
            continue;
        };
        out.push(Damage { target: e.id, attacker: p.firer, owner: p.owner, weapon: p.weapon, band });
    }
}

/// Apply hits: `max(1, base * table * band / 10000)`, at the own-side percent for a hit on the attacker's side.
/// A table entry of 0 means no damage at all.
fn apply(state: &mut GameState, rules: &Rules, hits: &[Damage], events: &mut Vec<Event>) {
    let tick = state.tick;
    for d in hits {
        let Some(i) = index(state, d.target) else { continue };
        let pct = table(rules, d.weapon, &state.entities[i]);
        if pct == 0 {
            continue;
        }
        let own = if state.entities[i].owner == d.owner { rules.combat.own_splash_percent } else { 100 };
        let amount = (rules.weapon(d.weapon).damage * pct * d.band * own / 1_000_000).max(1);
        if own == 0 {
            continue;
        }
        let e = &mut state.entities[i];
        e.health -= amount;
        if d.attacker != e.id {
            e.last_attacker = Some((d.attacker, tick));
        }
        events.push(Event::Hit {
            tick,
            target: e.id,
            attacker: d.attacker,
            weapon: d.weapon,
            damage: amount,
            health: e.health,
        });
    }
}
