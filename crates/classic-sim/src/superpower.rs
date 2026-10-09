//! The palace powers (rules-world.md, section 8; the `superpowers` module). Each pack faction has one power
//! (`by_faction` in the module's data); a player with no faction has none. A player's one counter charges a tick at a
//! time while they own a palace that isn't being sold and aren't short of power, and waits once full
//! (`superpower_ready`). Using it, at a tile, sets it back to zero:
//!
//! - **Missile.** Launched from the palace at any explored tile, it lands `flight + flight_per_tile × distance` ticks
//!   later, off target by a random point within `spread + distance / spread_tiles` tiles (drawn from the game's
//!   generator at launch, so a replay lands the same). Everything on the ground, every owner's, takes the damage of
//!   its ring: the larger of its x and y tile offsets from where it lands (a building's nearest tile), 0 to 3.
//! - **Guerrillas.** After `guerrillas_delay` ticks, `guerrillas_count` guerrillas appear on free tiles
//!   `guerrillas_min_range` to `guerrillas_max_range` tiles (the larger offset) from an explored tile. They are their
//!   owner's but can't be ordered: they head for that tile and fight whatever they meet there, until killed.
//! - **Saboteur.** A saboteur comes out of the palace at once, under its owner's orders. If the tile holds an enemy
//!   building its owner knows of, it sets off to blow it up; otherwise it walks there.
//!
//! Strikes on their way are part of the game state, so a save or replay carries them. Their damage lands before
//! combat's, in the same tick, and the destroyed are removed with combat's.

use rts_core::hash::{Canon, CanonHasher};
use rts_core::imath::isqrt;
use rts_core::rng::random_int;

use crate::map::{MapData, Tile};
use crate::movement;
use crate::path::Pathfinder;
use crate::power::Power;
use crate::production;
use crate::units::{Rules, Superpower, SuperpowerRules};
use crate::vision::{self, TileView};
use crate::world::{self, Event, GameState, Order};

/// Why a palace power order was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuperpowerError {
    /// The module is off, or the player's faction has no power.
    NoPower,
    /// The player has no palace (or only one being sold).
    NoPalace,
    /// Still charging.
    NotReady,
    /// The missile and the guerrillas need an explored tile on the map.
    Unexplored,
    /// The saboteur has no free tile to come out on.
    Blocked,
}

impl SuperpowerError {
    pub fn id(self) -> &'static str {
        match self {
            SuperpowerError::NoPower => "no_power",
            SuperpowerError::NoPalace => "no_palace",
            SuperpowerError::NotReady => "not_ready",
            SuperpowerError::Unexplored => "unexplored",
            SuperpowerError::Blocked => "blocked",
        }
    }
}

/// A palace power on its way: a missile in flight, or guerrillas about to arrive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
    pub owner: u32,
    pub power: Superpower,
    /// The palace it came from: a missile's hits are reported as that palace's.
    pub from: u32,
    /// Where it lands: the missile's impact tile, or the guerrillas' target tile.
    pub x: i32,
    pub y: i32,
    /// The tick it lands.
    pub at: u32,
}

impl Canon for Strike {
    fn canon(&self, w: &mut CanonHasher) {
        w.object()
            .field("at", &self.at)
            .field("from", &self.from)
            .field("owner", &self.owner)
            .field("power", self.power.id())
            .field("x", &self.x)
            .field("y", &self.y)
            .end();
    }
}

/// The power `player`'s faction gives, while the module is on.
pub fn power(state: &GameState, rules: &Rules, player: u32) -> Option<Superpower> {
    let p = state.players.iter().find(|p| p.id == player)?;
    rules.superpowers.as_ref()?.of(p.faction.as_deref())
}

/// `player`'s power, the ticks it has charged and the ticks it needs, once they have charged it at all.
pub fn charge(state: &GameState, rules: &Rules, player: u32) -> Option<(Superpower, u32, u32)> {
    let power = power(state, rules, player)?;
    let charged = state.players.iter().find(|p| p.id == player)?.charge?;
    Some((power, charged, rules.superpowers.as_ref()?.charge_ticks(power)))
}

/// Whether `player`'s power is charged and they have a palace to use it from.
pub fn ready(state: &GameState, rules: &Rules, player: u32) -> bool {
    palace(state, rules, player).is_some() && charge(state, rules, player).is_some_and(|(_, c, full)| c >= full)
}

/// The index of `player`'s first palace that isn't being sold.
fn palace(state: &GameState, rules: &Rules, player: u32) -> Option<usize> {
    let k = rules.kind_id("palace")?;
    state.entities.iter().position(|e| e.owner == player && e.kind == k && e.selling == 0)
}

/// Use `player`'s charged power at tile (x, y).
#[allow(clippy::too_many_arguments)]
pub fn fire(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    player: u32,
    x: i32,
    y: i32,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let result = (|| {
        let sp = rules.superpowers.as_ref().ok_or(SuperpowerError::NoPower)?;
        let power = power(state, rules, player).ok_or(SuperpowerError::NoPower)?;
        let i = palace(state, rules, player).ok_or(SuperpowerError::NoPalace)?;
        if !ready(state, rules, player) {
            return Err(SuperpowerError::NotReady);
        }
        let on_map = x >= 0 && y >= 0 && x < map.width && y < map.height;
        let explored = |state: &GameState| {
            on_map && state.vision.as_ref().is_none_or(|v| v.tile(player, x, y) != TileView::Shroud)
        };
        if power != Superpower::Saboteur && !explored(state) {
            return Err(SuperpowerError::Unexplored);
        }
        let from = state.entities[i].id;
        match power {
            Superpower::Missile => launch(map, state, rules, sp, i, x, y, events),
            Superpower::Guerrillas => {
                let at = tick + sp.guerrillas_delay;
                state.strikes.push(Strike { owner: player, power, from, x, y, at });
            }
            Superpower::Saboteur => {
                let mut scratch = Vec::new();
                let unit = production::deliver(map, pf, state, rules, i, sp.saboteur, &mut scratch)
                    .ok_or(SuperpowerError::Blocked)?;
                events.push(Event::SaboteurArrived { tick, player, palace: from, unit });
                let target = state
                    .entities
                    .iter()
                    .find(|b| {
                        let k = rules.kind(b.kind);
                        let t = b.tile();
                        b.owner != player
                            && k.building
                            && !k.wall
                            && (t.x..t.x + k.width).contains(&x)
                            && (t.y..t.y + k.height).contains(&y)
                            && vision::known(state, rules, player, b)
                    })
                    .map(|b| b.id);
                let e = state.entities.last_mut().expect("just delivered");
                if let Some(t) = target {
                    e.order = Order::Attack;
                    e.target = Some(t);
                    movement::stop(rules, e);
                } else if on_map {
                    e.path = movement::route(pf, e, Tile { x, y });
                    e.order = Order::Move;
                }
            }
        }
        let p = state.players.iter_mut().find(|p| p.id == player).expect("has a power");
        p.charge = Some(0);
        Ok(())
    })();
    if let Err(reason) = result {
        events.push(Event::SuperpowerRefused { tick, player, reason });
    }
}

/// Send a missile from palace `i` at tile (x, y).
#[allow(clippy::too_many_arguments)]
fn launch(
    map: &MapData,
    state: &mut GameState,
    rules: &Rules,
    sp: &SuperpowerRules,
    i: usize,
    x: i32,
    y: i32,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let (palace, owner) = (state.entities[i].id, state.entities[i].owner);
    let k = rules.kind(state.entities[i].kind);
    let t = state.entities[i].tile();
    let (cx, cy) = (t.x + k.width / 2, t.y + k.height / 2);
    let (dx, dy) = ((x - cx) as i64, (y - cy) as i64);
    let distance = isqrt((dx * dx + dy * dy) as u64) as i64;
    let flight = sp.missile_flight + sp.missile_flight_per_tile * distance as u32;
    let r = sp.missile_spread + distance / sp.missile_spread_tiles;
    // A point in the disc of radius r: draw from the square and draw again until one lands inside, at most 8 times.
    let (mut ox, mut oy) = (0, 0);
    for _ in 0..8 {
        let span = (2 * r + 1) as u32;
        let a = random_int(&mut state.rng, span) as i64 - r;
        let b = random_int(&mut state.rng, span) as i64 - r;
        if a * a + b * b <= r * r {
            (ox, oy) = (a, b);
            break;
        }
    }
    let to_x = (x + ox as i32).clamp(0, map.width - 1);
    let to_y = (y + oy as i32).clamp(0, map.height - 1);
    let arrive = tick + flight;
    state.strikes.push(Strike { owner, power: Superpower::Missile, from: palace, x: to_x, y: to_y, at: arrive });
    events.push(Event::MissileLaunched { tick, player: owner, palace, x, y, to_x, to_y, arrive });
}

/// The superpowers phase, before combat: charging, strikes landing, and guerrillas finding their way.
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let Some(sp) = rules.superpowers.as_ref() else { return };
    let tick = state.tick;
    for p in 0..state.players.len() {
        let id = state.players[p].id;
        let Some(power) = sp.of(state.players[p].faction.as_deref()) else { continue };
        if palace(state, rules, id).is_none() || Power::of(state, rules, id).is_short() {
            continue;
        }
        let full = sp.charge_ticks(power);
        let c = state.players[p].charge.unwrap_or(0);
        if c < full {
            state.players[p].charge = Some(c + 1);
            if c + 1 == full {
                events.push(Event::SuperpowerReady { tick, player: id, power });
            }
        } else {
            state.players[p].charge = Some(c);
        }
    }
    let (due, later): (Vec<Strike>, Vec<Strike>) =
        std::mem::take(&mut state.strikes).into_iter().partition(|s| s.at <= tick);
    state.strikes = later;
    for s in due {
        match s.power {
            Superpower::Missile => impact(state, rules, sp, &s, events),
            _ => arrive(map, pf, state, rules, sp, &s, events),
        }
    }
    steer(pf, state, rules);
}

/// A missile lands: everything on the ground within 3 tiles takes its ring's damage.
fn impact(state: &mut GameState, rules: &Rules, sp: &SuperpowerRules, s: &Strike, events: &mut Vec<Event>) {
    let tick = state.tick;
    events.push(Event::MissileImpact { tick, player: s.owner, x: s.x, y: s.y });
    for e in &mut state.entities {
        let k = rules.kind(e.kind);
        if e.airborne() || !k.targetable {
            continue;
        }
        let t = e.tile();
        let (w, h) = if k.building { (k.width, k.height) } else { (1, 1) };
        let dx = (t.x - s.x).max(s.x - (t.x + w - 1)).max(0);
        let dy = (t.y - s.y).max(s.y - (t.y + h - 1)).max(0);
        let Some(&damage) = sp.missile_damage.get(dx.max(dy) as usize) else { continue };
        if damage <= 0 {
            continue;
        }
        e.health -= damage;
        e.last_attacker = Some((s.from, tick));
        let (target, health) = (e.id, e.health);
        events.push(Event::Hit { tick, target, attacker: s.from, weapon: sp.missile_weapon, damage, health });
    }
}

/// Guerrillas arrive round their target tile, on free tiles drawn from the game's generator.
fn arrive(
    map: &MapData,
    pf: &Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    sp: &SuperpowerRules,
    s: &Strike,
    events: &mut Vec<Event>,
) {
    let (lo, hi) = (sp.guerrillas_min_range, sp.guerrillas_max_range.max(sp.guerrillas_min_range));
    let mut placed: Vec<Tile> = Vec::new();
    for _ in 0..sp.guerrillas_count * 8 {
        if placed.len() as u32 >= sp.guerrillas_count {
            break;
        }
        let span = (2 * hi + 1) as u32;
        let dx = random_int(&mut state.rng, span) as i32 - hi;
        let dy = random_int(&mut state.rng, span) as i32 - hi;
        let t = Tile { x: s.x + dx, y: s.y + dy };
        let ring = dx.abs().max(dy.abs());
        let taken = |t: Tile| state.entities.iter().any(|e| world::on_ground(rules, e) && e.tile() == t);
        if ring < lo || !(0..map.width).contains(&t.x) || !(0..map.height).contains(&t.y) {
            continue;
        }
        if !pf.passable(t.x, t.y) || taken(t) || placed.contains(&t) {
            continue;
        }
        placed.push(t);
    }
    for &t in &placed {
        world::spawn(state, rules, sp.guerrilla, s.owner, t.x, t.y);
        let e = state.entities.last_mut().expect("just spawned");
        e.autonomous = Some(Tile { x: s.x, y: s.y });
    }
    let units = placed.len() as u32;
    events.push(Event::GuerrillasArrived { tick: state.tick, player: s.owner, x: s.x, y: s.y, units });
}

/// Units fighting on their own, on scan ticks: one that has found a target goes after it; one with nothing to do
/// heads back to its tile.
fn steer(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules) {
    let tick = state.tick;
    let every = rules.combat.scan_every.max(1);
    for e in &mut state.entities {
        let Some(goal) = e.autonomous else { continue };
        if !(tick + e.id).is_multiple_of(every) || e.order != Order::Idle {
            continue;
        }
        if e.target.is_some() {
            e.order = Order::Attack;
            movement::stop(rules, e);
        } else if e.path.is_empty() {
            let t = e.tile();
            if (t.x - goal.x).abs().max((t.y - goal.y).abs()) > 2 {
                e.path = movement::route(pf, e, goal);
            }
        }
    }
}
