//! The computer opponent (playbooks `plans/rts/ai-opponent.md`): a player without a mouse. It reads the game
//! through `&Game`, the same view a front end draws from, and acts only through `Game::order`, so its commands are
//! queued, checked and logged exactly like a human's clicks. It can't move a unit, add credits or place a building
//! any other way, and a replay of the command log plays its games back with the AI switched off.
//!
//! It is deterministic: integer maths, entities in id order, no clock and no randomness of its own, so the same game
//! always gets the same orders and two AIs playing each other give the same hash every run.
//!
//! The opponent comes in three strengths (`Difficulty`: easy, normal, hard), which are only different `Settings`.
//! It has four managers that share a small memory (`Ai`):
//! - the base (`base.rs`): power, a build order of generic ids, where each building goes, and repairing buildings
//!   once the fighting round them stops;
//! - production and the economy: harvesters to fill its refineries, then combat units in a weighted mix;
//! - the army (`army.rs`): gathers new units at a rally point, defends the base, and sends attack waves that grow
//!   each time;
//! - the palace power (`power.rs`): once charged, used on the best target it knows of, if worth it.
//!
//! Under fog of war (the `fog` module) it reads only what its own side can see: enemies in sight, and enemy
//! buildings as it last saw them (`classic_sim::vision`). Knowing no enemy building, it guesses the other players'
//! start positions, as a player who knows the map would, aims its rally point at the nearest, and sends its fastest
//! idle fighter to look at the nearest one it has not explored. With fog off it sees the whole map, as before.

#![deny(clippy::float_arithmetic, clippy::disallowed_types)]

mod army;
mod base;
mod geo;
mod power;

use std::collections::BTreeSet;

use classic_sim::{Command, CommandOrder, Game, Kind, Tile, TileView, vision};
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
    /// A building below this percent of its health is repaired once nothing has hit it for `repair_quiet_ticks`,
    /// while credits are at least `repair_reserve`; 0 means never.
    pub repair_percent: i64,
    pub repair_quiet_ticks: u32,
    pub repair_reserve: i64,
    /// Harvesters wanted for each refinery, and the most in all.
    pub harvesters_per_refinery: usize,
    pub max_harvesters: usize,
    /// Harvesters for each carrier it keeps once it can build them, to lift them on long trips; 0 means none.
    pub harvesters_per_carrier: usize,
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
            ("air_factory", 1),
            ("research_lab", 1),
            ("palace", 1),
        ];
        let mix = [
            ("battle_tank", 6),
            ("rocket_squad", 3),
            ("siege_tank", 2),
            ("quad", 2),
            ("missile_tank", 1),
            ("infantry_squad", 1),
            ("rocket_infantry", 1),
            ("infantry", 1),
            ("scout_bike", 1),
        ];
        Settings {
            think_every: 30,
            build_order: order.iter().map(|&(id, n)| (id.to_string(), n)).collect(),
            power_margin: 20,
            silo_percent: 80,
            repair_percent: 50,
            repair_quiet_ticks: 15 * 5,
            repair_reserve: 200,
            harvesters_per_refinery: 3,
            max_harvesters: 9,
            harvesters_per_carrier: 3,
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

    /// A gentler opponent: it thinks half as often, keeps fewer harvesters and a bigger reserve, builds fewer
    /// turrets, and waits longer before smaller waves that turn back sooner.
    pub fn easy() -> Settings {
        let normal = Settings::normal();
        let order = [
            ("power_plant", 1),
            ("refinery", 1),
            ("light_factory", 1),
            ("heavy_factory", 1),
            ("barracks", 1),
            ("radar", 1),
            ("refinery", 2),
            ("gun_turret", 2),
        ];
        Settings {
            think_every: 60,
            build_order: order.iter().map(|&(id, n)| (id.to_string(), n)).collect(),
            harvesters_per_refinery: 2,
            max_harvesters: 4,
            factory_queue: 1,
            unit_reserve: 600,
            first_wave_tick: 15 * 60 * 10,
            first_wave: 3,
            wave_growth: 1,
            wave_cap: 10,
            retreat_percent: 50,
            ..normal
        }
    }

    /// A tougher opponent: it waits for twice the units before each wave, grows its waves twice as fast, only
    /// attacks where it would win clearly, and keeps a carrier for every two harvesters. AI-versus-AI runs on the
    /// skirmish map found that more harvesters, a third refinery or thinking more often made it no stronger; bigger,
    /// surer waves did (8 won to 2 before aircraft, 7 to 4 after), and the extra carriers on top of them won 12 to 0 (11 to 1
    /// once faction specials came).
    /// The carriers alone, with normal's waves, lost 4 to 7.
    pub fn hard() -> Settings {
        Settings {
            first_wave: 8,
            wave_growth: 4,
            wave_cap: 30,
            attack_margin: 200,
            harvesters_per_carrier: 2,
            ..Settings::normal()
        }
    }
}

/// How strong the computer opponent plays: a name for one of the `Settings` presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn settings(self) -> Settings {
        match self {
            Difficulty::Easy => Settings::easy(),
            Difficulty::Normal => Settings::normal(),
            Difficulty::Hard => Settings::hard(),
        }
    }

    /// Its id in saved settings and saved games: `easy`, `normal` or `hard`.
    pub fn id(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    pub fn from_id(id: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.id() == id)
    }

    /// The next one up, wrapping from hard to easy.
    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
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
    /// The unit it sent to find the enemy under fog, while it is on its way.
    scout: Option<u32>,
    /// Under fog, knowing no enemy building once every start is explored: the search points it has had in sight
    /// since the search began, as (y, x).
    searched: BTreeSet<(i32, i32)>,
    /// Credits delivered by its harvesters so far, and the tick that total last grew.
    delivered: i64,
    delivered_at: u32,
}

impl Ai {
    pub fn new(player: u32, settings: Settings) -> Ai {
        let wave_size = settings.first_wave;
        Ai {
            player,
            settings,
            wave: None,
            wave_size,
            waves_sent: 0,
            thinks: 0,
            scout: None,
            searched: BTreeSet::new(),
            delivered: 0,
            delivered_at: 0,
        }
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
        base::repair(self, game, &view, &mut out);
        if self.thinks.is_multiple_of(4) {
            base::harvesters(game, &view, &mut out);
        }
        army::think(self, game, &view, &mut out);
        army::scout(self, game, &view, &mut out);
        power::think(self, game, &view, &mut out);
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
    /// Its own entities, less those it can't command for long: units fighting on their own, and sappers, which go
    /// for the building their palace power aimed them at.
    pub mine: Vec<usize>,
    /// The enemies it knows of: all of them with fog off; under fog, those in its sight and the buildings it keeps a
    /// ghost of.
    pub enemies: Vec<usize>,
    /// The centre of its first construction yard, or failing that of its first building.
    pub home: Option<Point>,
    /// The centre of the enemy building nearest home, or under fog, knowing none, of the nearest other player's start
    /// position.
    pub enemy_home: Option<Point>,
    /// Under fog, knowing no enemy building: the nearest other player's start position it has not explored.
    pub unexplored_start: Option<Tile>,
    /// Under fog: whether it knows of any enemy building (in sight or kept as a ghost).
    pub knows_building: bool,
}

impl View {
    fn new(game: &Game, player: u32) -> View {
        let es = &game.state.entities;
        let mine: Vec<usize> = (0..es.len())
            .filter(|&i| es[i].owner == player && es[i].autonomous.is_none() && !game.rules.kind(es[i].kind).sapper)
            .collect();
        let enemies: Vec<usize> = (0..es.len())
            .filter(|&i| es[i].owner != player && vision::known(&game.state, &game.rules, player, &es[i]))
            .collect();
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
        let enemy_home_known = enemy_home.is_some();
        // Under fog, knowing no enemy building yet: the other players' start positions, nearest home first.
        let starts: Vec<Tile> = match (enemy_home, home, &game.state.vision) {
            (None, Some(h), Some(_)) => {
                let mut s: Vec<Tile> = game
                    .state
                    .players
                    .iter()
                    .filter(|p| p.id != player)
                    .filter_map(|p| game.map.start.get(p.id as usize).copied().flatten())
                    .collect();
                s.sort_by_key(|&t| (geo::d2(geo::centre(t), h), geo::off_middle(game, geo::centre(t)), t.y, t.x));
                s
            }
            _ => Vec::new(),
        };
        let enemy_home = enemy_home.or(starts.first().map(|&t| geo::centre(t)));
        let unexplored_start = starts.into_iter().find(|t| game.tile_view(player, t.x, t.y) == TileView::Shroud);
        View { mine, enemies, home, enemy_home, unexplored_start, knows_building: enemy_home_known }
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
