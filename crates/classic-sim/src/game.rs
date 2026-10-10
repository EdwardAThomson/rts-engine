//! The game API that tests, tools, the computer opponent and front ends drive: `step`, `order`, `spawn`,
//! `snapshot`, `hash` and the command log. A native build and the WebAssembly build expose the same thing.

use rts_core::hash::hash_of;
use rts_core::replay::{CommandQueue, Logged};
use rts_core::rng::seed_state;

use crate::blooms;
use crate::capture;
use crate::hazard::{self, Hazards};
use crate::map::{MapData, Tile, parse_map};
use crate::path::Pathfinder;
use crate::placement::{self, PlaceError};
use crate::power::Power;
use crate::production::{self, ProduceError, QueueEntry};
use crate::sell;
use crate::storage;
use crate::units::{Kind, Rules};
use crate::vision::{self, TileView, Vision};
use crate::world::{self, CaptureError, Command, CommandOrder, Event, GameState, Order, Player, Task};

pub struct GameOptions<'a> {
    /// ASCII map text (see `map.rs`).
    pub map: &'a str,
    pub seed: i32,
    /// Defaults to every start position on the map.
    pub players: Option<usize>,
    /// The rules in play, usually a setting pack's tuned ones. Defaults to the engine's own rules data.
    pub rules: Option<&'a Rules>,
}

pub struct Game {
    pub map: MapData,
    pub pathfinder: Pathfinder,
    /// The hazard's: open ground only.
    pub hazard_pathfinder: Pathfinder,
    pub rules: Rules,
    pub state: GameState,
    pub events: Vec<Event>,
    queue: CommandQueue<Command>,
}

/// One entity as `snapshot` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityView {
    pub id: u32,
    pub kind: Kind,
    pub owner: u32,
    pub tile: Tile,
    pub x: i64,
    pub y: i64,
    pub health: i64,
    pub order: Order,
    pub task: Option<Task>,
    pub cargo: Option<i64>,
    pub path_left: usize,
    /// A producing building's queue, the head first.
    pub queue: Vec<QueueEntry>,
    /// What a capture or repair-pad order heads for.
    pub goal: Option<u32>,
    /// Repair is on.
    pub repairing: bool,
    /// Ticks until a building being sold goes; 0 when not being sold.
    pub selling: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: u32,
    pub players: Vec<Player>,
    /// Each player's power, in player order.
    pub power: Vec<Power>,
    /// Each player's storage cap, in player order.
    pub storage: Vec<i64>,
    pub entities: Vec<EntityView>,
    pub resource_left: i64,
}

impl Game {
    pub fn new(opts: GameOptions) -> Result<Game, String> {
        let map = parse_map(opts.map)?;
        let rules = opts.rules.cloned().unwrap_or_default();
        let kind = |id: &str| rules.kind_id(id).ok_or(format!("the rules have no built {id}"));
        let [yard, plant, refinery, harvester, tank] =
            ["construction_yard", "power_plant", "refinery", "harvester", "battle_tank"].map(kind);
        let (yard, plant, refinery, harvester, tank) = (yard?, plant?, refinery?, harvester?, tank?);
        let count = opts.players.unwrap_or(map.start.len());
        let mut state = GameState {
            tick: 0,
            rng: seed_state(opts.seed),
            next_id: 1,
            resource: map.resource.clone(),
            players: Vec::new(),
            entities: Vec::new(),
            kind_ids: rules.kind_ids().into(),
            weapon_ids: rules.weapons.iter().map(|w| w.id.clone()).collect::<Vec<_>>().into(),
            projectiles: Vec::new(),
            hazards: rules.hazard.as_ref().map(|h| Hazards { list: Vec::new(), next_spawn: h.first_tick }),
            vision: None,
            market: None,
            deliveries: Vec::new(),
            strikes: Vec::new(),
            slabs: None,
            blooms: blooms::start(&map, &rules),
        };
        // Each player starts with a construction yard on its start tile, a power plant beside it, a refinery
        // beside them both on the side towards the middle of the map (below a start in the top half, above one in
        // the bottom half) with a harvester on its far side, and a battle tank beside the harvester, all laid out
        // from the footprints in the rules (rules-base-building-power.md: "a starting base of yard, one power plant
        // and a refinery"). The plant and the tank go on the side towards the middle of the map too, so a start is
        // laid out as the mirror image of one in the other half, either way round, and mirrored or half-turned maps
        // start even. Maps leave room for it.
        for p in 0..count {
            let s = map.start.get(p).copied().flatten().ok_or(format!("map has no start position {}", p + 1))?;
            let owner = p as u32;
            state.players.push(Player {
                id: owner,
                credits: rules.production.starting_credits,
                delivered: 0,
                lost: 0,
                lost_warned: None,
                charge: None,
                faction: None,
            });
            let (y, r, pk) = (rules.kind(yard), rules.kind(refinery), rules.kind(plant));
            let left = 2 * s.x + y.width > map.width;
            let up = 2 * s.y + y.height > map.height;
            world::spawn(&mut state, &rules, yard, owner, s.x, s.y);
            world::spawn(&mut state, &rules, plant, owner, if left { s.x - pk.width } else { s.x + y.width }, s.y);
            let ry = if up { s.y - r.height } else { s.y + y.height.max(pk.height) };
            let rx = if left { s.x + y.width - r.width } else { s.x };
            let home = world::spawn(&mut state, &rules, refinery, owner, rx, ry);
            let below = world::dock_at(r, rx, ry);
            let dock = if up { Tile { x: below.x, y: ry - 1 } } else { below };
            world::spawn(&mut state, &rules, harvester, owner, dock.x, dock.y);
            state.entities.last_mut().expect("just spawned").home_id = Some(home);
            world::spawn(&mut state, &rules, tank, owner, if left { dock.x - 2 } else { dock.x + 2 }, dock.y);
        }
        let mut pathfinder = Pathfinder::new(&map);
        for e in &state.entities {
            world::occupy(&mut pathfinder, &rules, e, true);
        }
        // Fog: each player starts seeing what its starting base sees.
        state.vision = Vision::new(map.width, map.height, state.players.len(), &rules);
        vision::tick(&mut state, &rules);
        let hazard_pathfinder = hazard::pathfinder(&map);
        Ok(Game {
            map,
            pathfinder,
            hazard_pathfinder,
            rules,
            state,
            events: Vec::new(),
            queue: CommandQueue::default(),
        })
    }

    /// Give each player, in player order, the setting pack's faction with this generic id, which decides the
    /// faction-only kinds they may build. Call it before the first tick; players past the end of `factions` keep
    /// none.
    pub fn set_factions<S: AsRef<str>>(&mut self, factions: &[S]) {
        for (p, f) in self.state.players.iter_mut().zip(factions) {
            p.faction = Some(f.as_ref().to_string());
        }
    }

    /// Advance `n` ticks.
    pub fn step(&mut self, n: u32) {
        for _ in 0..n {
            let cmds = self.queue.take(self.state.tick);
            let (pf, hpf) = (&mut self.pathfinder, &mut self.hazard_pathfinder);
            world::step(&self.map, pf, hpf, &mut self.state, &self.rules, &cmds, &mut self.events);
        }
    }

    /// Queue an order for the next tick, exactly as a player's click would.
    pub fn order(&mut self, player: u32, ids: &[u32], order: CommandOrder) {
        self.queue.push(self.state.tick, Command { player, ids: ids.to_vec(), order });
    }

    /// Place a unit or building directly, skipping every rule, for tests and tools. Players place buildings with
    /// `CommandOrder::Place` instead.
    pub fn spawn(&mut self, kind: Kind, owner: u32, x: i32, y: i32) -> u32 {
        let id = world::spawn(&mut self.state, &self.rules, kind, owner, x, y);
        world::occupy(&mut self.pathfinder, &self.rules, self.state.entities.last().expect("just spawned"), true);
        vision::tick(&mut self.state, &self.rules);
        id
    }

    /// Place a hazard, submerged, at the centre of tile (x, y), skipping every rule, for tests and tools. `None`
    /// when the rules have the hazard off.
    pub fn spawn_hazard(&mut self, x: i32, y: i32) -> Option<u32> {
        hazard::place(&mut self.state, Tile { x, y })
    }

    /// The kind with this generic id, if these rules have it.
    pub fn kind(&self, id: &str) -> Option<Kind> {
        self.rules.kind_id(id)
    }

    /// A player's power now.
    pub fn power(&self, player: u32) -> Power {
        Power::of(&self.state, &self.rules, player)
    }

    /// What `player` knows of tile (x, y): everything is in sight while fog is off.
    pub fn tile_view(&self, player: u32, x: i32, y: i32) -> TileView {
        self.state.vision.as_ref().map_or(TileView::Visible, |v| v.tile(player, x, y))
    }

    /// Whether `player` can see entity `id` now (its own always; everything while fog is off).
    pub fn visible(&self, player: u32, id: u32) -> bool {
        self.state.entity(id).is_some_and(|e| vision::visible(&self.state, &self.rules, player, e))
    }

    /// Whether `player` knows entity `id` is there: it can see it, or keeps a ghost of it under fog.
    pub fn known(&self, player: u32, id: u32) -> bool {
        self.state.entity(id).is_some_and(|e| vision::known(&self.state, &self.rules, player, e))
    }

    /// Whether `player` has radar (rules-world.md, section 4): with the `radar` module on, they own a building that
    /// gives it and their power supply meets demand; with it off, always. It is read from the state, not kept in it.
    pub fn radar(&self, player: u32) -> bool {
        if !self.rules.radar {
            return true;
        }
        let owns = self.state.entities.iter().any(|e| e.owner == player && self.rules.kind(e.kind).radar);
        owns && !self.power(player).is_short()
    }

    /// A player's storage cap now: the most credits deliveries can bring them to.
    pub fn storage(&self, player: u32) -> i64 {
        storage::cap(&self.state, &self.rules, player)
    }

    /// Whether `player` may order `kind` built now. Changes nothing.
    pub fn can_build(&self, player: u32, kind: Kind) -> Result<(), ProduceError> {
        production::can_build(&self.state, &self.rules, player, kind)
    }

    /// Whether `player` could place `kind` with its top-left tile at (x, y) now. Changes nothing.
    pub fn can_place(&self, player: u32, kind: Kind, x: i32, y: i32) -> Result<(), PlaceError> {
        placement::check(&self.map, &self.state, &self.rules, player, kind, x, y)
    }

    /// Whether `player` may send capturers against building `id` now. Changes nothing.
    pub fn can_capture(&self, player: u32, id: u32) -> Result<(), CaptureError> {
        let b = self.state.entity(id).ok_or(CaptureError::NotCapturable)?;
        capture::can_capture(&self.rules, player, b)
    }

    /// What selling building `id` would pay back now, its queue included; 0 while the rules have selling off.
    pub fn sell_refund(&self, id: u32) -> i64 {
        let Some(e) = self.state.entity(id).filter(|_| self.rules.sell.is_some()) else { return 0 };
        sell::refund(&self.rules, e) + e.queue.iter().map(|q| q.paid).sum::<i64>()
    }

    /// One whole tick as plain data, for agents and tools to read.
    pub fn snapshot(&self) -> Snapshot {
        let s = &self.state;
        Snapshot {
            tick: s.tick,
            players: s.players.clone(),
            power: Power::all(s, &self.rules),
            storage: storage::all(s, &self.rules),
            entities: s
                .entities
                .iter()
                .map(|e| EntityView {
                    id: e.id,
                    kind: e.kind,
                    owner: e.owner,
                    tile: e.tile(),
                    x: e.x,
                    y: e.y,
                    health: e.health,
                    order: e.order,
                    task: e.task,
                    cargo: e.cargo,
                    path_left: e.path.len(),
                    queue: e.queue.clone(),
                    goal: e.goal,
                    repairing: e.repairing,
                    selling: e.selling,
                })
                .collect(),
            resource_left: s.resource.iter().sum(),
        }
    }

    pub fn hash(&self) -> String {
        hash_state(&self.state)
    }

    pub fn hash_value(&self) -> u32 {
        hash_of(&self.state).value()
    }

    pub fn command_log(&self) -> &[Logged<Command>] {
        self.queue.log()
    }
}

/// FNV-1a over the canonical JSON of the whole state, as eight hex digits. Equal hashes mean equal games.
pub fn hash_state(state: &GameState) -> String {
    hash_of(state).hex()
}
