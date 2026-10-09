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
//! - production and the economy: harvesters to fill its refineries, then combat units in a weighted mix;
//! - the army (`army.rs`): gathers new units at a rally point, defends the base, and sends attack waves that grow
//!   each time.
//!
//! There is no fog of war yet, so it sees what every player sees: the whole map. Once fog exists it must read only
//! what its own units can see.

#![deny(clippy::float_arithmetic, clippy::disallowed_types)]

mod army;
mod base;
mod geo;

use classic_sim::{Command, CommandOrder, Game, Kind, Tile};
use geo::Point;

pub use army::Wave;

/// The numbers that set how the opponent plays. Our own starting values, to tune by AI-versus-AI runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Ticks between thinks. Every computer player thinks on the same ticks, so none is always a few ticks ahead
    /// of another: staggering them gave the first player an edge on a mirrored map.
    pub think_every: u32,
    /// What the base aims for, in order: a generic building id and how many to have. Ids the rules don't have are
    /// skipped, so a pack that leaves a building out still gets an opponent.
    pub build_order: Vec<(String, usize)>,
    /// Power supply kept above demand, counting the building about to be built.
    pub power_margin: i64,
    /// Credits, as a percent of its storage cap, at which a silo comes before the build order; 0 means never.
    pub silo_percent: i64,
    /// Harvesters wanted for each refinery, and the most in all.
    pub harvesters_per_refinery: usize,
    pub max_harvesters: usize,
    /// Most entries kept in one factory's queue.
    pub factory_queue: usize,
    /// Combat units are only queued while credits are at least this, so the base keeps growing.
    pub unit_reserve: i64,
    /// The army's mix: a generic unit id and its weight. Each factory makes whichever armed unit it can that the
    /// army has fewest of for its weight. An armed unit the rules have but this list leaves out weighs 1, so a pack
    /// with other units still sees them built; weight 0 means never.
    pub unit_mix: Vec<(String, usize)>,
    /// No attack wave before this tick.
    pub first_wave_tick: u32,
    /// Units a wave waits for: `first_wave`, then `wave_growth` more after each wave that comes home.
    pub first_wave: usize,
    pub wave_growth: usize,
    /// A wave goes only where it would beat the defenders by this percent of their strength, unless `wave_cap`
    /// units are waiting, when it goes for the least defended target anyway.
    pub attack_margin: i64,
    pub wave_cap: usize,
    /// Most ticks a wave spends gathering at its staging point before it attacks.
    pub stage_ticks: u32,
    /// With nothing delivered for this many ticks and too few credits for a combat unit, every unit attacks.
    pub broke_ticks: u32,
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
            ("barracks", 1),
            ("radar", 1),
            ("refinery", 2),
            ("gun_turret", 2),
            ("heavy_factory", 2),
            ("gun_turret", 4),
        ];
        let mix = [("battle_tank", 6), ("rocket_squad", 3), ("quad", 2), ("infantry_squad", 1), ("scout_bike", 1)];
        Settings {
            think_every: 30,
            build_order: order.iter().map(|&(id, n)| (id.to_string(), n)).collect(),
            power_margin: 20,
            silo_percent: 80,
            harvesters_per_refinery: 3,
            max_harvesters: 9,
            factory_queue: 2,
            unit_reserve: 300,
            unit_mix: mix.iter().map(|&(id, n)| (id.to_string(), n)).collect(),
            first_wave_tick: 15 * 60 * 6,
            first_wave: 4,
            wave_growth: 2,
            attack_margin: 150,
            wave_cap: 20,
            stage_ticks: 15 * 30,
            broke_ticks: 15 * 120,
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
    /// Credits delivered by its harvesters so far, and the tick that total last grew.
    delivered: i64,
    delivered_at: u32,
}

impl Ai {
    pub fn new(player: u32, settings: Settings) -> Ai {
        let wave_size = settings.first_wave;
        Ai { player, settings, wave: None, wave_size, waves_sent: 0, thinks: 0, delivered: 0, delivered_at: 0 }
    }

    /// Whether this AI thinks on the game's current tick.
    pub fn due(&self, game: &Game) -> bool {
        game.state.tick.is_multiple_of(self.settings.think_every.max(1))
    }

    /// Think if it is due, and queue the orders for the next tick. Call once before each `Game::step(1)`.
    pub fn tick(&mut self, game: &mut Game) {
        if self.due(game) {
            for c in self.think(game) {
                game.order(c.player, &c.ids, c.order);
            }
        }
    }

    /// Whether its income has stopped: nothing delivered for `broke_ticks`.
    pub(crate) fn dry(&self, game: &Game) -> bool {
        game.state.tick >= self.delivered_at + self.settings.broke_ticks
    }

    /// Decide this think's orders. Reads the game and changes nothing in it.
    pub fn think(&mut self, game: &Game) -> Vec<Command> {
        self.thinks += 1;
        let mut out = Orders { player: self.player, list: Vec::new() };
        if defeated(game, self.player) {
            return out.list;
        }
        let delivered = game.state.players.iter().find(|p| p.id == self.player).map_or(0, |p| p.delivered);
        if delivered != self.delivered {
            (self.delivered, self.delivered_at) = (delivered, game.state.tick);
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
    /// The centre of its first construction yard, or failing that of its first building.
    pub home: Option<Point>,
    /// The centre of the enemy building nearest home.
    pub enemy_home: Option<Point>,
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
            .map(|&i| geo::at(game, &es[i]));
        let enemy_home = home.and_then(|h| {
            enemies.iter().filter(building).min_by_key(|&&i| (geo::d2(geo::at(game, &es[i]), h), es[i].id))
        });
        let enemy_home = enemy_home.map(|&i| geo::at(game, &es[i]));
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

pub(crate) fn dist2(a: Tile, b: Tile) -> i64 {
    let (dx, dy) = ((a.x - b.x) as i64, (a.y - b.y) as i64);
    dx * dx + dy * dy
}
