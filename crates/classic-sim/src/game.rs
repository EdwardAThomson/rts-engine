//! The game API that tests, tools, the computer opponent and front ends drive: `step`, `order`, `spawn`,
//! `snapshot`, `hash` and the command log. A native build and the WebAssembly build expose the same thing.

use engine_core::hash::hash_of;
use engine_core::replay::{CommandQueue, Logged};
use engine_core::rng::seed_state;

use crate::map::{MapData, Tile, parse_map};
use crate::path::Pathfinder;
use crate::units::UnitType;
use crate::world::{self, Command, CommandOrder, Event, GameState, Order, Player, Task};

pub struct GameOptions<'a> {
    /// ASCII map text (see `map.rs`).
    pub map: &'a str,
    pub seed: i32,
    /// Defaults to every start position on the map.
    pub players: Option<usize>,
}

pub struct Game {
    pub map: MapData,
    pub pathfinder: Pathfinder,
    pub state: GameState,
    pub events: Vec<Event>,
    queue: CommandQueue<Command>,
}

/// One entity as `snapshot` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityView {
    pub id: u32,
    pub kind: UnitType,
    pub owner: u32,
    pub tile: Tile,
    pub x: i64,
    pub y: i64,
    pub health: i64,
    pub order: Order,
    pub task: Option<Task>,
    pub cargo: Option<i64>,
    pub path_left: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: u32,
    pub players: Vec<Player>,
    pub entities: Vec<EntityView>,
    pub resource_left: i64,
}

impl Game {
    pub fn new(opts: GameOptions) -> Result<Game, String> {
        let map = parse_map(opts.map)?;
        let count = opts.players.unwrap_or(map.start.len());
        let mut state = GameState {
            tick: 0,
            rng: seed_state(opts.seed),
            next_id: 1,
            resource: map.resource.clone(),
            players: Vec::new(),
            entities: Vec::new(),
        };
        // Each player starts with a refinery on its start tile, a harvester at the dock and a tank beside it.
        for p in 0..count {
            let s = map.start.get(p).copied().flatten().ok_or(format!("map has no start position {}", p + 1))?;
            state.players.push(Player { id: p as u32, credits: 0, delivered: 0 });
            let refinery = world::spawn(&mut state, UnitType::Refinery, p as u32, s.x, s.y);
            let dock = world::dock_of(state.entities.last().expect("just spawned"));
            world::spawn(&mut state, UnitType::Harvester, p as u32, dock.x, dock.y);
            state.entities.last_mut().expect("just spawned").home_id = Some(refinery);
            world::spawn(&mut state, UnitType::Tank, p as u32, s.x + 1, s.y + 1);
        }
        let pathfinder = Pathfinder::new(&map);
        Ok(Game { map, pathfinder, state, events: Vec::new(), queue: CommandQueue::default() })
    }

    /// Advance `n` ticks.
    pub fn step(&mut self, n: u32) {
        for _ in 0..n {
            let cmds = self.queue.take(self.state.tick);
            world::step(&self.map, &mut self.pathfinder, &mut self.state, &cmds, &mut self.events);
        }
    }

    /// Queue an order for the next tick, exactly as a player's click would.
    pub fn order(&mut self, player: u32, ids: &[u32], order: CommandOrder) {
        self.queue.push(self.state.tick, Command { player, ids: ids.to_vec(), order });
    }

    /// Place a unit or building directly, for tests and tools.
    pub fn spawn(&mut self, kind: UnitType, owner: u32, x: i32, y: i32) -> u32 {
        world::spawn(&mut self.state, kind, owner, x, y)
    }

    /// One whole tick as plain data, for agents and tools to read.
    pub fn snapshot(&self) -> Snapshot {
        let s = &self.state;
        Snapshot {
            tick: s.tick,
            players: s.players.clone(),
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
