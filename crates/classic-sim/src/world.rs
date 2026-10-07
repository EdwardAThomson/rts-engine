//! The game state and the fixed-tick update. Everything here is deterministic: integer maths, one seeded random
//! generator in the state, entities kept in a vector in id order, and no clock or outside randomness.

use std::collections::VecDeque;
use std::sync::Arc;

use rts_core::hash::{Canon, CanonHasher};
use rts_core::imath::isqrt;
use rts_core::rng::random_int;

use crate::map::{MapData, RESOURCE_PER_TILE, TILE, Terrain, Tile};
use crate::path::Pathfinder;
use crate::placement::{self, PlaceError};
use crate::power::Power;
use crate::production::{self, EntryCanon, ProduceError, QueueEntry};
use crate::units::{Kind, Rules};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Idle,
    Move,
    Harvest,
}

/// A harvester's step in its loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Seek,
    ToField,
    Mining,
    ToRefinery,
    Unloading,
    Stuck,
}

impl Order {
    pub fn id(self) -> &'static str {
        match self {
            Order::Idle => "idle",
            Order::Move => "move",
            Order::Harvest => "harvest",
        }
    }
}

impl Task {
    pub fn id(self) -> &'static str {
        match self {
            Task::Seek => "seek",
            Task::ToField => "toField",
            Task::Mining => "mining",
            Task::ToRefinery => "toRefinery",
            Task::Unloading => "unloading",
            Task::Stuck => "stuck",
        }
    }
}

impl Canon for Order {
    fn canon(&self, w: &mut CanonHasher) {
        w.string(self.id());
    }
}

impl Canon for Task {
    fn canon(&self, w: &mut CanonHasher) {
        w.string(self.id());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub kind: Kind,
    pub owner: u32,
    /// Centre, in sub-tile units.
    pub x: i64,
    pub y: i64,
    pub health: i64,
    pub path: VecDeque<Tile>,
    pub order: Order,
    /// Harvesters only.
    pub task: Option<Task>,
    pub cargo: Option<i64>,
    /// The refinery this harvester delivers to.
    pub home_id: Option<u32>,
    /// Producing buildings only: what it is building, the head first.
    pub queue: Vec<QueueEntry>,
}

impl Entity {
    pub fn tile(&self) -> Tile {
        Tile { x: self.x.div_euclid(TILE) as i32, y: self.y.div_euclid(TILE) as i32 }
    }
}

/// An entity as the state hash writes it, with its kind spelt as the generic id.
struct EntityCanon<'a>(&'a Entity, &'a [String]);

impl Canon for EntityCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let e = self.0;
        let queue: Vec<EntryCanon> = e.queue.iter().map(|q| EntryCanon(q, self.1)).collect();
        w.object()
            .opt("cargo", e.cargo.as_ref())
            .field("health", &e.health)
            .opt("homeId", e.home_id.as_ref())
            .field("id", &e.id)
            .field("order", &e.order)
            .field("owner", &e.owner)
            .array("path", &e.path)
            // Written only while something is queued, so a game with no production hashes as it did before.
            .opt("queue", (!queue.is_empty()).then_some(&queue))
            .opt("task", e.task.as_ref())
            .field("type", self.1[e.kind.0 as usize].as_str())
            .field("x", &e.x)
            .field("y", &e.y)
            .end();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: u32,
    pub credits: i64,
    /// Total resource ever delivered, for statistics.
    pub delivered: i64,
}

impl Canon for Player {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().field("credits", &self.credits).field("delivered", &self.delivered).field("id", &self.id).end();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub tick: u32,
    pub rng: i32,
    pub next_id: u32,
    /// Per tile, copied from the map at the start.
    pub resource: Vec<i64>,
    pub players: Vec<Player>,
    /// Always sorted by id.
    pub entities: Vec<Entity>,
    /// Each kind's generic id, in kind order (`Rules::kind_ids`), so the hash can spell kinds. Not hashed itself.
    pub kind_ids: Arc<[String]>,
}

impl Canon for GameState {
    fn canon(&self, w: &mut CanonHasher) {
        let entities: Vec<EntityCanon> = self.entities.iter().map(|e| EntityCanon(e, &self.kind_ids)).collect();
        w.object()
            .array("entities", &entities)
            .field("nextId", &self.next_id)
            .field("players", &self.players)
            .field("resource", &self.resource)
            .field("rng", &self.rng)
            .field("tick", &self.tick)
            .end();
    }
}

impl GameState {
    pub fn entity(&self, id: u32) -> Option<&Entity> {
        self.entities.binary_search_by_key(&id, |e| e.id).ok().map(|i| &self.entities[i])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOrder {
    /// Move to a tile.
    Move {
        x: i32,
        y: i32,
    },
    Harvest,
    /// Place a ready building, with its top-left tile here. Takes no units.
    Place {
        kind: Kind,
        x: i32,
        y: i32,
    },
    /// Add an item to the end of a factory's queue: the building in `ids` if one is given, otherwise the player's
    /// primary (first built) building that makes it.
    Produce {
        kind: Kind,
    },
    /// Remove the last queued entry of this kind from the same factory and refund what was paid for it.
    Cancel {
        kind: Kind,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub player: u32,
    pub ids: Vec<u32>,
    pub order: CommandOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleReason {
    NoResource,
    NoRefinery,
}

/// What happened, for logs and cosmetics. Events never feed back into the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    HarvesterIdle {
        tick: u32,
        unit: u32,
        reason: IdleReason,
    },
    Delivered {
        tick: u32,
        unit: u32,
        player: u32,
        credits: i64,
    },
    Regrowth {
        tick: u32,
        x: i32,
        y: i32,
        amount: i64,
    },
    BuildingPlaced {
        tick: u32,
        entity: u32,
        kind: Kind,
        owner: u32,
        x: i32,
        y: i32,
    },
    PlacementRejected {
        tick: u32,
        player: u32,
        kind: Kind,
        x: i32,
        y: i32,
        reason: PlaceError,
    },
    ProductionQueued {
        tick: u32,
        factory: u32,
        kind: Kind,
    },
    ProductionRejected {
        tick: u32,
        player: u32,
        kind: Kind,
        reason: ProduceError,
    },
    /// The head entry stopped for want of credits; it resumes by itself.
    ProductionPaused {
        tick: u32,
        factory: u32,
        kind: Kind,
    },
    ProductionCancelled {
        tick: u32,
        factory: u32,
        kind: Kind,
        refund: i64,
    },
    /// A building finished at a yard and waits to be placed.
    BuildingReady {
        tick: u32,
        player: u32,
        factory: u32,
        kind: Kind,
    },
    UnitBuilt {
        tick: u32,
        factory: u32,
        entity: u32,
        kind: Kind,
    },
    /// A player's power supply, demand or shortfall differs from the previous tick's.
    PowerChanged {
        tick: u32,
        player: u32,
        supply: i64,
        demand: i64,
        shortfall: i64,
    },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::HarvesterIdle { .. } => "harvester_idle",
            Event::Delivered { .. } => "delivered",
            Event::Regrowth { .. } => "regrowth",
            Event::BuildingPlaced { .. } => "building_placed",
            Event::PlacementRejected { .. } => "placement_rejected",
            Event::PowerChanged { .. } => "power_changed",
            Event::ProductionQueued { .. } => "production_queued",
            Event::ProductionRejected { .. } => "production_rejected",
            Event::ProductionPaused { .. } => "production_paused",
            Event::ProductionCancelled { .. } => "production_cancelled",
            Event::BuildingReady { .. } => "building_ready",
            Event::UnitBuilt { .. } => "unit_built",
        }
    }

    pub fn tick(&self) -> u32 {
        match *self {
            Event::HarvesterIdle { tick, .. }
            | Event::Delivered { tick, .. }
            | Event::Regrowth { tick, .. }
            | Event::BuildingPlaced { tick, .. }
            | Event::PlacementRejected { tick, .. }
            | Event::PowerChanged { tick, .. }
            | Event::ProductionQueued { tick, .. }
            | Event::ProductionRejected { tick, .. }
            | Event::ProductionPaused { tick, .. }
            | Event::ProductionCancelled { tick, .. }
            | Event::BuildingReady { tick, .. }
            | Event::UnitBuilt { tick, .. } => tick,
        }
    }
}

fn centre(t: i32) -> i64 {
    t as i64 * TILE + TILE / 2
}

pub fn spawn(state: &mut GameState, rules: &Rules, kind: Kind, owner: u32, tx: i32, ty: i32) -> u32 {
    let id = state.next_id;
    state.next_id += 1;
    let harvester = rules.kind(kind).harvester.is_some();
    state.entities.push(Entity {
        id,
        kind,
        owner,
        x: centre(tx),
        y: centre(ty),
        health: rules.kind(kind).max_health,
        path: VecDeque::new(),
        order: if harvester { Order::Harvest } else { Order::Idle },
        task: harvester.then_some(Task::Seek),
        cargo: harvester.then_some(0),
        home_id: None,
        queue: Vec::new(),
    });
    id
}

/// The tile a harvester parks on to unload: just below the middle column of the refinery's footprint.
pub fn dock_of(rules: &Rules, refinery: &Entity) -> Tile {
    let t = refinery.tile();
    dock_at(rules.kind(refinery.kind), t.x, t.y)
}

/// The dock of a refinery kind whose top-left tile is at (x, y).
pub fn dock_at(k: &crate::units::KindRules, x: i32, y: i32) -> Tile {
    Tile { x: x + k.width / 2, y: y + k.height }
}

/// Mark a building's footprint as blocked (or clear again) for ground movement. Units block nothing.
pub fn occupy(pf: &mut Pathfinder, rules: &Rules, e: &Entity, blocked: bool) {
    let k = rules.kind(e.kind);
    if k.building {
        let t = e.tile();
        pf.set_blocked(t.x, t.y, k.width, k.height, blocked);
    }
}

fn path_or_empty(pf: &mut Pathfinder, from: Tile, to: Tile) -> VecDeque<Tile> {
    pf.find(from.x, from.y, to.x, to.y).map(|p| p.tiles.into()).unwrap_or_default()
}

/// Units whose remaining path now crosses a blocked tile find a new way to the same end, in id order.
fn reroute_around_new_building(pf: &mut Pathfinder, state: &mut GameState) {
    for e in &mut state.entities {
        if e.path.iter().any(|t| !pf.passable(t.x, t.y)) {
            let end = *e.path.back().expect("a path that crosses something is not empty");
            e.path = path_or_empty(pf, e.tile(), end);
        }
    }
}

pub fn apply_command(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    cmd: &Command,
    events: &mut Vec<Event>,
) {
    match cmd.order {
        CommandOrder::Produce { kind } => return production::produce(state, rules, cmd.player, &cmd.ids, kind, events),
        CommandOrder::Cancel { kind } => return production::cancel(state, rules, cmd.player, &cmd.ids, kind, events),
        _ => {}
    }
    if let CommandOrder::Place { kind, x, y } = cmd.order {
        let tick = state.tick;
        let ready = if production::has_ready(state, cmd.player, kind) { Ok(()) } else { Err(PlaceError::NotReady) };
        match ready.and_then(|()| placement::check(map, state, rules, cmd.player, kind, x, y)) {
            Ok(()) => {
                production::take_ready(state, cmd.player, kind);
                let entity = spawn(state, rules, kind, cmd.player, x, y);
                occupy(pf, rules, state.entities.last().expect("just spawned"), true);
                reroute_around_new_building(pf, state);
                events.push(Event::BuildingPlaced { tick, entity, kind, owner: cmd.player, x, y });
            }
            Err(reason) => events.push(Event::PlacementRejected { tick, player: cmd.player, kind, x, y, reason }),
        }
        return;
    }
    for &id in &cmd.ids {
        let Ok(i) = state.entities.binary_search_by_key(&id, |e| e.id) else { continue };
        let e = &mut state.entities[i];
        if e.owner != cmd.player || rules.kind(e.kind).building {
            continue;
        }
        match cmd.order {
            CommandOrder::Move { x, y } => {
                e.path = path_or_empty(pf, e.tile(), Tile { x, y });
                e.order = Order::Move;
            }
            CommandOrder::Harvest if rules.kind(e.kind).harvester.is_some() => {
                e.order = Order::Harvest;
                e.task = Some(Task::Seek);
                e.path.clear();
            }
            CommandOrder::Harvest
            | CommandOrder::Place { .. }
            | CommandOrder::Produce { .. }
            | CommandOrder::Cancel { .. } => {}
        }
    }
}

/// Advance one tick. `events` receives what happened; it never feeds back in.
pub fn step(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    commands: &[Command],
    events: &mut Vec<Event>,
) {
    // The tick runs in phases, each over entities in id order (rules-movement.md, "Moving within a tick"):
    // commands, combat (not built yet), movement, crush (not built yet), economy, world.
    let power_before = Power::all(state, rules);
    for cmd in commands {
        apply_command(map, pf, state, rules, cmd, events);
    }
    movement(state, rules);
    economy(map, pf, state, rules, events);
    regrow(map, pf, state, rules, events);
    report_power(state, rules, &power_before, events);
    state.tick += 1;
}

/// Report each player whose supply, demand or shortfall differs from the previous tick's.
fn report_power(state: &GameState, rules: &Rules, before: &[Power], events: &mut Vec<Event>) {
    for ((player, now), was) in state.players.iter().zip(Power::all(state, rules)).zip(before) {
        if now != *was {
            let (supply, demand, shortfall) = (now.supply, now.demand, now.shortfall());
            events.push(Event::PowerChanged { tick: state.tick, player: player.id, supply, demand, shortfall });
        }
    }
}

/// Every ground unit with a path moves along it. A harvester on its loop moves only while travelling.
fn movement(state: &mut GameState, rules: &Rules) {
    for e in &mut state.entities {
        let k = rules.kind(e.kind);
        if k.building {
            continue;
        }
        let travelling = !matches!(
            (e.order, e.task),
            (Order::Harvest, Some(Task::Seek | Task::Mining | Task::Unloading | Task::Stuck))
        );
        if travelling {
            move_along_path(e, k.speed);
        }
        if e.order == Order::Move && e.path.is_empty() {
            e.order = Order::Idle;
        }
    }
}

/// Harvesters on their loop (find a field, mine, return, unload), then every production queue.
fn economy(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    for i in 0..state.entities.len() {
        let e = &state.entities[i];
        if rules.kind(e.kind).harvester.is_some() && e.order == Order::Harvest {
            harvest(map, pf, state, rules, i, events);
        }
    }
    production::tick(map, pf, state, rules, events);
}

/// Move toward the next path tile's centre, carrying leftover budget past centres.
fn move_along_path(e: &mut Entity, speed: i64) {
    let mut budget = speed;
    while budget > 0 {
        let Some(&next) = e.path.front() else { break };
        let (dx, dy) = (centre(next.x) - e.x, centre(next.y) - e.y);
        let dist = isqrt((dx * dx + dy * dy) as u64) as i64;
        if dist <= budget {
            e.x += dx;
            e.y += dy;
            budget -= dist;
            e.path.pop_front();
        } else {
            // Integer division truncates toward zero, as the original TypeScript's Math.trunc did.
            e.x += dx * budget / dist;
            e.y += dy * budget / dist;
            budget = 0;
        }
    }
}

fn harvest(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    i: usize,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let kind = rules.kind(state.entities[i].kind);
    let Some(h) = &kind.harvester else { return };
    let here = state.entities[i].tile();
    let Some(task) = state.entities[i].task else { return };
    match task {
        Task::Seek => {
            let e_id = state.entities[i].id;
            let Some(field) = nearest_resource(map, pf, state, here) else {
                state.entities[i].task = Some(Task::Stuck);
                events.push(Event::HarvesterIdle { tick, unit: e_id, reason: IdleReason::NoResource });
                return;
            };
            let e = &mut state.entities[i];
            e.path = path_or_empty(pf, here, field);
            e.task = Some(Task::ToField);
        }
        Task::ToField => {
            let e = &mut state.entities[i];
            if e.path.is_empty() {
                e.task = Some(Task::Mining);
            }
        }
        Task::Mining => {
            let t = map.index(here.x, here.y);
            let cargo = state.entities[i].cargo.unwrap_or(0);
            let take = h.mine_rate.min(state.resource[t]).min(h.capacity - cargo);
            state.resource[t] -= take;
            let cargo = cargo + take;
            state.entities[i].cargo = Some(cargo);
            if cargo >= h.capacity {
                let Some(home) = home_refinery(state, rules, i) else {
                    state.entities[i].task = Some(Task::Stuck);
                    let unit = state.entities[i].id;
                    events.push(Event::HarvesterIdle { tick, unit, reason: IdleReason::NoRefinery });
                    return;
                };
                let e = &mut state.entities[i];
                e.path = path_or_empty(pf, here, home);
                e.task = Some(Task::ToRefinery);
            } else if take == 0 {
                // The tile ran dry before we were full.
                state.entities[i].task = Some(Task::Seek);
            }
        }
        Task::ToRefinery => {
            let e = &mut state.entities[i];
            if e.path.is_empty() {
                e.task = Some(Task::Unloading);
            }
        }
        Task::Unloading => {
            let e = &mut state.entities[i];
            let cargo = e.cargo.unwrap_or(0);
            let amount = h.unload_rate.min(cargo);
            e.cargo = Some(cargo - amount);
            let (unit, owner, empty) = (e.id, e.owner, cargo == amount);
            let p = &mut state.players[owner as usize];
            p.credits += amount;
            p.delivered += amount;
            if empty {
                events.push(Event::Delivered { tick, unit, player: owner, credits: p.credits });
                state.entities[i].task = Some(Task::Seek);
            }
        }
        Task::Stuck => {}
    }
}

/// The harvester's refinery: its remembered one if that still stands, else its owner's first. Returns the dock
/// tile, and remembers the choice.
fn home_refinery(state: &mut GameState, rules: &Rules, i: usize) -> Option<Tile> {
    let (owner, home_id) = (state.entities[i].owner, state.entities[i].home_id);
    let mut own = state.entities.iter().filter(|r| rules.kind(r.kind).refinery && r.owner == owner);
    let first = own.clone().next();
    let chosen = own.find(|r| Some(r.id) == home_id).or(first)?;
    let (id, dock) = (chosen.id, dock_of(rules, chosen));
    state.entities[i].home_id = Some(id);
    Some(dock)
}

/// Breadth-first search outward over tiles ground units can enter; the first tile with resource left wins.
fn nearest_resource(map: &MapData, pf: &Pathfinder, state: &GameState, from: Tile) -> Option<Tile> {
    let start = map.index(from.x, from.y);
    let mut seen = vec![false; state.resource.len()];
    seen[start] = true;
    let mut queue = vec![start];
    let mut qi = 0;
    while qi < queue.len() {
        let t = queue[qi];
        qi += 1;
        if state.resource[t] > 0 {
            return Some(map.tile_at(t));
        }
        let Tile { x, y } = map.tile_at(t);
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            if pf.passable(nx, ny) {
                let n = map.index(nx, ny);
                if !seen[n] {
                    seen[n] = true;
                    queue.push(n);
                }
            }
        }
    }
    None
}

/// Every so often, a random tile next to an original resource field grows some resource back.
fn regrow(map: &MapData, pf: &Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    if !state.tick.is_multiple_of(rules.regrowth.every_ticks) || state.tick == 0 {
        return;
    }
    let i = random_int(&mut state.rng, (map.width * map.height) as u32) as usize;
    let Tile { x, y } = map.tile_at(i);
    let near_field = [(0, 0), (0, -1), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .any(|(dx, dy)| map.in_bounds(x + dx, y + dy) && map.resource[map.index(x + dx, y + dy)] > 0);
    if !near_field || !pf.passable(x, y) || map.terrain[i] != Terrain::Open {
        return;
    }
    state.resource[i] = RESOURCE_PER_TILE.min(state.resource[i] + rules.regrowth.amount);
    events.push(Event::Regrowth { tick: state.tick, x, y, amount: state.resource[i] });
}
