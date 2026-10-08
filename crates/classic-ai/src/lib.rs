//! The computer opponent (playbooks `plans/rts/ai-opponent.md`): a player without a mouse. It reads the game
//! through `&Game`, the same view a front end draws from, and acts only through `Game::order`, so its commands are
//! queued, checked and logged exactly like a human's clicks. It can't move a unit, add credits or place a building
//! any other way, and a replay of the command log plays its games back with the AI switched off.
//!
//! It is deterministic: integer maths, entities in id order, no clock and no randomness of its own, so the same game
//! always gets the same orders and two AIs playing each other give the same hash every run.
//!
//! This first version is one "normal" opponent with three managers that share a small memory (`Ai`):
//! - the base (`base.rs`): power, a build order of generic ids and where each building goes;
//! - production and the economy: harvesters to fill its refineries, then combat units;
//! - the army (`army.rs`): gathers new units at a rally point, defends the base, and sends attack waves that grow
//!   each time.
//!
//! There is no fog of war yet, so it sees what every player sees: the whole map. Once fog exists it must read only
//! what its own units can see.

#![deny(clippy::float_arithmetic, clippy::disallowed_types)]

mod army;
mod base;

use classic_sim::map::TILE;
use classic_sim::{Command, CommandOrder, Entity, Game, Kind, Tile};

pub use army::Wave;

/// The numbers that set how the opponent plays. Our own starting values, to tune by AI-versus-AI runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Ticks between thinks. Player `p` thinks on ticks where `(tick + 5 * p) % think_every == 0`, so two AIs
    /// rarely think on the same tick.
    pub think_every: u32,
    /// What the base aims for, in order: a generic building id and how many to have. Ids the rules don't have are
    /// skipped, so a pack that leaves a building out still gets an opponent.
    pub build_order: Vec<(String, usize)>,
    /// Power supply kept above demand, counting the building about to be built.
    pub power_margin: i64,
    /// Harvesters wanted for each refinery, and the most in all.
    pub harvesters_per_refinery: usize,
    pub max_harvesters: usize,
    /// Most entries kept in one factory's queue.
    pub factory_queue: usize,
    /// Combat units are only queued while credits are at least this, so the base keeps growing.
    pub unit_reserve: i64,
    /// No attack wave before this tick.
    pub first_wave_tick: u32,
    /// Units in the first wave; each later wave adds `wave_growth`, up to `wave_cap`.
    pub first_wave: usize,
    pub wave_growth: usize,
    pub wave_cap: usize,
    /// A wave that falls below this percent of the units it set out with comes home.
    pub retreat_percent: usize,
    /// Enemy armed units this many tiles from one of its buildings draw out the defenders.
    pub defend_radius: i32,
    /// How far ahead of the yard, towards the nearest enemy, new units gather.
    pub rally_distance: i32,
}

impl Settings {
    /// The normal opponent.
    pub fn normal() -> Settings {
        let order = [
            ("power_plant", 1),
            ("refinery", 1),
            ("light_factory", 1),
            ("heavy_factory", 1),
            ("radar", 1),
            ("refinery", 2),
            ("gun_turret", 2),
            ("heavy_factory", 2),
            ("gun_turret", 4),
        ];
        Settings {
            think_every: 30,
            build_order: order.iter().map(|&(id, n)| (id.to_string(), n)).collect(),
            power_margin: 20,
            harvesters_per_refinery: 2,
            max_harvesters: 6,
            factory_queue: 2,
            unit_reserve: 300,
            first_wave_tick: 15 * 60 * 6,
            first_wave: 4,
            wave_growth: 2,
            wave_cap: 10,
            retreat_percent: 30,
            defend_radius: 10,
            rally_distance: 6,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings::normal()
    }
}

/// One computer player and everything it remembers between thinks.
#[derive(Clone, Debug)]
pub struct Ai {
    pub player: u32,
    pub settings: Settings,
    /// The attack wave out now, if any.
    pub wave: Option<Wave>,
    /// Units the next wave waits for.
    pub wave_size: usize,
    /// Waves sent so far.
    pub waves_sent: u32,
    /// Thinks so far, for the managers that think less often.
    thinks: u32,
}

impl Ai {
    pub fn new(player: u32, settings: Settings) -> Ai {
        let wave_size = settings.first_wave;
        Ai { player, settings, wave: None, wave_size, waves_sent: 0, thinks: 0 }
    }

    /// Whether this AI thinks on the game's current tick.
    pub fn due(&self, game: &Game) -> bool {
        (game.state.tick + 5 * self.player).is_multiple_of(self.settings.think_every.max(1))
    }

    /// Think if it is due, and queue the orders for the next tick. Call once before each `Game::step(1)`.
    pub fn tick(&mut self, game: &mut Game) {
        if self.due(game) {
            for c in self.think(game) {
                game.order(c.player, &c.ids, c.order);
            }
        }
    }

    /// Decide this think's orders. Reads the game and changes nothing in it.
    pub fn think(&mut self, game: &Game) -> Vec<Command> {
        self.thinks += 1;
        let mut out = Orders { player: self.player, list: Vec::new() };
        if defeated(game, self.player) {
            return out.list;
        }
        let view = View::new(game, self.player);
        base::think(self, game, &view, &mut out);
        base::produce(self, game, &view, &mut out);
        if self.thinks.is_multiple_of(4) {
            base::harvesters(game, &view, &mut out);
        }
        army::think(self, game, &view, &mut out);
        out.list
    }
}

/// Orders collected during a think.
pub(crate) struct Orders {
    player: u32,
    pub list: Vec<Command>,
}

impl Orders {
    pub fn push(&mut self, ids: Vec<u32>, order: CommandOrder) {
        self.list.push(Command { player: self.player, ids, order });
    }
}

/// What one think works from: indices into `game.state.entities`, in id order.
pub(crate) struct View {
    pub mine: Vec<usize>,
    pub enemies: Vec<usize>,
    /// Its first construction yard, or failing that its first building.
    pub home: Option<Tile>,
    /// The enemy building nearest home.
    pub enemy_home: Option<Tile>,
}

impl View {
    fn new(game: &Game, player: u32) -> View {
        let es = &game.state.entities;
        let mine: Vec<usize> = (0..es.len()).filter(|&i| es[i].owner == player).collect();
        let enemies: Vec<usize> = (0..es.len()).filter(|&i| es[i].owner != player).collect();
        let yard = game.kind("construction_yard");
        let building = |i: &&usize| game.rules.kind(es[**i].kind).building;
        let home = mine
            .iter()
            .find(|&&i| Some(es[i].kind) == yard)
            .or_else(|| mine.iter().find(building))
            .map(|&i| centre_tile(game, &es[i]));
        let enemy_home = home.and_then(|h| {
            enemies.iter().filter(building).min_by_key(|&&i| (dist2(centre_tile(game, &es[i]), h), es[i].id))
        });
        let enemy_home = enemy_home.map(|&i| centre_tile(game, &es[i]));
        View { mine, enemies, home, enemy_home }
    }

    pub fn count(&self, game: &Game, kind: Kind) -> usize {
        self.mine.iter().filter(|&&i| game.state.entities[i].kind == kind).count()
    }
}

/// Whether a player has lost: no buildings left. (Units left over can't build again.)
pub fn defeated(game: &Game, player: u32) -> bool {
    !game.state.entities.iter().any(|e| e.owner == player && game.rules.kind(e.kind).building)
}

/// The one player not defeated, once all the others are.
pub fn winner(game: &Game) -> Option<u32> {
    let alive: Vec<u32> = game.state.players.iter().map(|p| p.id).filter(|&p| !defeated(game, p)).collect();
    match alive[..] {
        [one] if game.state.players.len() > 1 => Some(one),
        _ => None,
    }
}

/// The tile at the middle of an entity's footprint (its own tile for a unit).
pub(crate) fn centre_tile(game: &Game, e: &Entity) -> Tile {
    let k = game.rules.kind(e.kind);
    let t = e.tile();
    Tile { x: t.x + (k.width - 1) / 2, y: t.y + (k.height - 1) / 2 }
}

pub(crate) fn dist2(a: Tile, b: Tile) -> i64 {
    let (dx, dy) = ((a.x - b.x) as i64, (a.y - b.y) as i64);
    dx * dx + dy * dy
}

/// Distance squared, in tiles, from a tile to an entity's centre.
pub(crate) fn tiles2(e: &Entity, t: Tile) -> i64 {
    let (dx, dy) = (e.x - (t.x as i64 * TILE + TILE / 2), e.y - (t.y as i64 * TILE + TILE / 2));
    (dx * dx + dy * dy) / (TILE * TILE)
}
