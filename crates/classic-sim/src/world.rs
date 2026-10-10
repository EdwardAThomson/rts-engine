//! The game state and the fixed-tick update. Everything here is deterministic: integer maths, one seeded random
//! generator in the state, entities kept in a vector in id order, and no clock or outside randomness.

use std::collections::VecDeque;
use std::sync::Arc;

use rts_core::hash::{Canon, CanonHasher};
use rts_core::rng::random_int;

use crate::air::{self, Ferry};
use crate::blooms::{self, Blooms};
use crate::capture;
use crate::combat::{self, Projectile, ProjectileCanon};
use crate::decay::{self, Slabs};
use crate::hazard::{self, Hazards, LeftReason};
use crate::map::{MapData, RESOURCE_PER_TILE, TILE, Terrain, Tile};
use crate::movement;
use crate::path::Pathfinder;
use crate::placement::{self, PlaceError};
use crate::power::Power;
use crate::production::{self, EntryCanon, ProduceError, QueueEntry};
use crate::repair;
use crate::sell;
use crate::starport::{self, Delivery, DeliveryCanon, Market, StarportError};
use crate::storage;
use crate::superpower::{self, Strike, SuperpowerError};
use crate::units::{Kind, Rules, Superpower, WeaponId};
use crate::vision::{self, Vision, VisionCanon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Standing guard: armed units look for targets and fire on them.
    Idle,
    /// Moving under orders, ignoring enemies.
    Move,
    Harvest,
    /// Going after one target the player chose, until it is destroyed.
    Attack,
    /// A capturer on its way to take the enemy building in `goal`.
    Capture,
    /// A vehicle on its way to, or waiting at, the repair pad in `goal`.
    Repair,
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
            Order::Attack => "attack",
            Order::Capture => "capture",
            Order::Repair => "repair",
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
    /// Armed entities only: the facing its weapon points along, 0 to 255 clockwise from north.
    pub facing: i64,
    /// Ticks until its weapon can fire again.
    pub reload: u32,
    /// What it is shooting at.
    pub target: Option<u32>,
    /// Who last hit it, and on which tick, so it can answer.
    pub last_attacker: Option<(u32, u32)>,
    /// Ticks left to wait before looking for a way round the units blocking its path.
    pub wait: u32,
    /// Searches for a way round blocking units that failed in a row.
    pub repath_fails: u32,
    /// The unit that asked this one to step aside, and on which tick.
    pub yield_for: Option<u32>,
    pub yield_at: Option<u32>,
    /// Noise made lately on open ground, halved each hazard scan window; stays 0 while the hazard is off.
    pub noise: i64,
    /// Aircraft only: height above the ground in sub-tile units, 0 when landed.
    pub altitude: i64,
    /// The carrier holding this unit up, from the moment it is picked up until it is set down.
    pub carried_by: Option<u32>,
    /// Carriers only: the lift it is doing.
    pub ferry: Option<Ferry>,
    /// What a capture or repair order heads for: the building to take, or the repair pad. A repair pad's is the
    /// vehicle it is mending.
    pub goal: Option<u32>,
    /// Buildings only: repair is on (the `repair` module).
    pub repairing: bool,
    /// Power-scaled ticks towards the next repair step, in hundredths: a repairing building's, or a repair pad's.
    pub repair_due: i64,
    /// Buildings only: ticks left until a building being sold goes; 0 when not being sold.
    pub selling: u32,
    /// Taken over by a converting weapon: the owner it goes back to, and the tick it does.
    pub converted: Option<(u32, u32)>,
    /// Ordered to destroy itself: the tick it blows up. Until then it neither moves nor fires.
    pub fuse: Option<u32>,
    /// The tick it disappears on its own, for kinds with a lifetime.
    pub expires: Option<u32>,
    /// Fighting on its own for its owner, who can't order it, around this tile: a palace power's guerrillas.
    pub autonomous: Option<Tile>,
    /// Factories only: picked by its owner as the one of its kind that takes orders naming no factory.
    pub primary: bool,
    /// Buildings only: how many of its footprint tiles held its owner's slab when it was placed (the `decay` module).
    pub foundation: u32,
}

impl Entity {
    pub fn tile(&self) -> Tile {
        Tile { x: self.x.div_euclid(TILE) as i32, y: self.y.div_euclid(TILE) as i32 }
    }

    /// Off the ground: an aircraft not fully landed, or a unit being carried. Only weapons that hit air can aim at
    /// it, and only a burst in the air can splash it.
    pub fn airborne(&self) -> bool {
        self.altitude > 0 || self.carried_by.is_some()
    }
}

/// Whether `e` is a ground unit taking part in ground movement: not a building, not an aircraft, not carried.
pub fn on_ground(rules: &Rules, e: &Entity) -> bool {
    let k = rules.kind(e.kind);
    !k.building && !k.air && e.carried_by.is_none()
}

/// An entity as the state hash writes it, with its kind spelt as the generic id.
struct EntityCanon<'a>(&'a Entity, &'a [String]);

impl Canon for EntityCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let e = self.0;
        let queue: Vec<EntryCanon> = e.queue.iter().map(|q| EntryCanon(q, self.1)).collect();
        let attacker = e.last_attacker.map(|(id, _)| id);
        let attacked = e.last_attacker.map(|(_, tick)| tick);
        let (converted_from, reverts_at) = (e.converted.map(|(owner, _)| owner), e.converted.map(|(_, at)| at));
        // Combat fields are written only when set, so an entity that never fought hashes as it did before combat.
        // Air fields are written only when set too, so a game with no aircraft hashes as it did before them.
        w.object()
            .opt("altitude", (e.altitude != 0).then_some(&e.altitude))
            .opt("attackedAt", attacked.as_ref())
            // Written only when set, so a game without the palace powers hashes as it did before them.
            .opt("autonomous", e.autonomous.as_ref())
            .opt("cargo", e.cargo.as_ref())
            .opt("carriedBy", e.carried_by.as_ref())
            // Written only when set, as are `expires`, `fuse` and `revertsAt`, so a game without the faction specials
            // hashes as it did before them.
            .opt("convertedFrom", converted_from.as_ref())
            .opt("expires", e.expires.as_ref())
            .opt("facing", (e.facing != 0).then_some(&e.facing))
            // Written only when set, so a game without slabs hashes as it did before them.
            .opt("foundation", (e.foundation != 0).then_some(&e.foundation))
            .opt("ferry", e.ferry.as_ref())
            .opt("fuse", e.fuse.as_ref())
            // Repair, sell and capture fields are written only when set, so a game without them hashes as before.
            .opt("goal", e.goal.as_ref())
            .field("health", &e.health)
            .opt("homeId", e.home_id.as_ref())
            .field("id", &e.id)
            .opt("lastAttacker", attacker.as_ref())
            .opt("noise", (e.noise != 0).then_some(&e.noise))
            .field("order", &e.order)
            .field("owner", &e.owner)
            .array("path", &e.path)
            // Written only when set, so a game where no one picks a primary factory hashes as it did before.
            .opt("primary", e.primary.then_some(&true))
            // Written only while something is queued, so a game with no production hashes as it did before.
            .opt("queue", (!queue.is_empty()).then_some(&queue))
            .opt("reload", (e.reload != 0).then_some(&e.reload))
            .opt("repairDue", (e.repair_due != 0).then_some(&e.repair_due))
            .opt("repairing", e.repairing.then_some(&true))
            .opt("repathFails", (e.repath_fails != 0).then_some(&e.repath_fails))
            .opt("revertsAt", reverts_at.as_ref())
            .opt("selling", (e.selling != 0).then_some(&e.selling))
            .opt("target", e.target.as_ref())
            .opt("task", e.task.as_ref())
            .field("type", self.1[e.kind.0 as usize].as_str())
            .opt("wait", (e.wait != 0).then_some(&e.wait))
            .field("x", &e.x)
            .field("y", &e.y)
            .opt("yieldAt", e.yield_at.as_ref())
            .opt("yieldFor", e.yield_for.as_ref())
            .end();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: u32,
    pub credits: i64,
    /// Total resource ever delivered, for statistics, stored or not.
    pub delivered: i64,
    /// Total resource delivered with no storage left for it, so lost (the `storage` module).
    pub lost: i64,
    /// When the last `credits_lost` event went out, so they come at most once per `storage.warn_every` ticks.
    pub lost_warned: Option<u32>,
    /// The generic id of the setting pack's faction this player plays, which decides the faction-only kinds it may
    /// build; `None` builds none of them.
    pub faction: Option<String>,
    /// Ticks its palace power has charged, from when it first owns a powered palace (the `superpowers` module).
    pub charge: Option<u32>,
}

impl Canon for Player {
    fn canon(&self, w: &mut CanonHasher) {
        // Written only once something is lost, so a game that never fills its storage hashes as it did before.
        let mut o = w.object();
        // Written only once it has a palace power, so a game without one hashes as it did before.
        o.opt("charge", self.charge.as_ref());
        o.field("credits", &self.credits).field("delivered", &self.delivered);
        // Written only when set, so a game with no factions hashes as it did before them.
        if let Some(f) = &self.faction {
            o.field("faction", f.as_str());
        }
        o.field("id", &self.id)
            .opt("lost", (self.lost != 0).then_some(&self.lost))
            .opt("lostWarned", self.lost_warned.as_ref())
            .end();
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
    /// Shells and rockets in flight, sorted by id; they take ids from `next_id` like entities.
    pub projectiles: Vec<Projectile>,
    /// Set when the rules turn the hazard on.
    pub hazards: Option<Hazards>,
    /// Each player's explored and seen tiles; set when the rules turn fog of war on.
    pub vision: Option<Vision>,
    /// The starport market, from when the first starport stands.
    pub market: Option<Market>,
    /// Starport orders, by starport id.
    pub deliveries: Vec<Delivery>,
    /// Palace powers on their way: missiles in flight and guerrillas about to arrive, in launch order.
    pub strikes: Vec<Strike>,
    /// Whose concrete slab lies on each tile; set when the first slab is laid (the `decay` module).
    pub slabs: Option<Slabs>,
    /// Resource blooms on the map and those to come; set while the `blooms` module is on.
    pub blooms: Option<Blooms>,
    /// Each kind's generic id, in kind order (`Rules::kind_ids`), so the hash can spell kinds. Not hashed itself.
    pub kind_ids: Arc<[String]>,
    /// Each weapon's generic id, in weapon order, likewise.
    pub weapon_ids: Arc<[String]>,
}

impl Canon for GameState {
    fn canon(&self, w: &mut CanonHasher) {
        let entities: Vec<EntityCanon> = self.entities.iter().map(|e| EntityCanon(e, &self.kind_ids)).collect();
        let projectiles: Vec<ProjectileCanon> =
            self.projectiles.iter().map(|p| ProjectileCanon(p, &self.weapon_ids)).collect();
        let deliveries: Vec<DeliveryCanon> = self.deliveries.iter().map(|d| DeliveryCanon(d, &self.kind_ids)).collect();
        // The market and starport orders are written only once they exist, so a game without a starport hashes as
        // it did before them.
        w.object()
            // Written only while blooms are on, so a game without them hashes as it did before.
            .opt("blooms", self.blooms.as_ref())
            .opt("deliveries", (!deliveries.is_empty()).then_some(&deliveries))
            .array("entities", &entities)
            // Written only when the hazard is on, so a game without it hashes as it did before.
            .opt("hazards", self.hazards.as_ref())
            .opt("market", self.market.as_ref())
            .field("nextId", &self.next_id)
            .field("players", &self.players)
            .opt("projectiles", (!projectiles.is_empty()).then_some(&projectiles))
            .field("resource", &self.resource)
            .field("rng", &self.rng)
            // Written only once a slab is laid, so a game without them hashes as it did before.
            .opt("slabs", self.slabs.as_ref())
            // Written only while a palace power is on its way.
            .opt("strikes", (!self.strikes.is_empty()).then_some(&self.strikes))
            .field("tick", &self.tick)
            // Written only when fog is on, so a game without it hashes as it did before.
            .opt("vision", self.vision.as_ref().map(|v| VisionCanon(v, &self.kind_ids)).as_ref())
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
    /// primary building that makes it (the one they picked, else the first built).
    Produce {
        kind: Kind,
    },
    /// Go after one enemy until it is destroyed: armed units only.
    Attack {
        target: u32,
    },
    /// Remove the last queued entry of this kind from the same factory and refund what was paid for it.
    Cancel {
        kind: Kind,
    },
    /// Put the first entry of this kind still being built in the same factory's queue on hold (`on`), or resume the
    /// first held one. A held entry at the head stops that queue, paying nothing.
    Hold {
        kind: Kind,
        on: bool,
    },
    /// Make the first factory in `ids` its owner's primary one of its kind.
    Primary,
    /// Turn repair on or off for the buildings in `ids`.
    Repair {
        on: bool,
    },
    /// Sell the buildings in `ids` back for part of their cost.
    Sell,
    /// Send the capturers in `ids` to take an enemy building.
    Capture {
        target: u32,
    },
    /// Send the damaged vehicles in `ids` to an own repair pad.
    RepairAt {
        pad: u32,
    },
    /// Start the countdown to blowing up: units whose kind has a self-destruct blast only. It can't be called off.
    SelfDestruct,
    /// Put one unit of this kind into the order at the starport in `ids`, or the player's first.
    StarportAdd {
        kind: Kind,
    },
    /// Take the last unit of this kind back out of that order.
    StarportRemove {
        kind: Kind,
    },
    /// Pay for that order and send for it.
    StarportConfirm,
    /// Use the player's charged palace power at this tile (rules-world.md, section 8).
    Superpower {
        x: i32,
        y: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub player: u32,
    pub ids: Vec<u32>,
    pub order: CommandOrder,
}

/// Why a move ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveEnd {
    /// At its goal, or the nearest free tile to it.
    Arrived,
    /// Gave up on a path blocked by units.
    Blocked,
}

impl MoveEnd {
    pub fn id(self) -> &'static str {
        match self {
            MoveEnd::Arrived => "arrived",
            MoveEnd::Blocked => "blocked",
        }
    }
}

/// Why a capture order was refused or ended without a capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// The rules have capture off.
    Off,
    /// Not an enemy building that can be taken (walls, turrets and the like never can).
    NotCapturable,
    /// Not hurt enough: at or above the capture module's `below_percent` of its maximum health.
    TooHealthy,
    /// No capturer among the units ordered.
    NoCapturer,
}

impl CaptureError {
    pub fn id(self) -> &'static str {
        match self {
            CaptureError::Off => "off",
            CaptureError::NotCapturable => "not_capturable",
            CaptureError::TooHealthy => "too_healthy",
            CaptureError::NoCapturer => "no_capturer",
        }
    }
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
    /// A delivery filled the player's storage: credits reached the cap from below.
    StorageFull {
        tick: u32,
        player: u32,
        cap: i64,
    },
    /// A delivery found no storage left and `amount` was lost; at most one every `storage.warn_every` ticks, while
    /// `Player::lost` counts every loss.
    CreditsLost {
        tick: u32,
        player: u32,
        amount: i64,
        cap: i64,
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
    /// A bloom appeared on tile (x, y).
    BloomSeeded {
        tick: u32,
        x: i32,
        y: i32,
    },
    /// A bloom on tile (x, y) burst, adding `added` resource round it.
    BloomBurst {
        tick: u32,
        x: i32,
        y: i32,
        added: i64,
    },
    /// A bloom's burst hurt a unit beside it. Not an attack.
    BloomHurt {
        tick: u32,
        unit: u32,
        damage: i64,
        health: i64,
    },
    /// A player laid a slab of kind `kind` with its top-left tile at (x, y); `tiles` were new.
    SlabLaid {
        tick: u32,
        kind: Kind,
        owner: u32,
        x: i32,
        y: i32,
        tiles: u32,
    },
    /// A building off concrete wore down by `damage` (the `decay` module). Not an attack.
    Decayed {
        tick: u32,
        entity: u32,
        owner: u32,
        damage: i64,
        health: i64,
    },
    /// Its owner put an entry on hold.
    ProductionHeld {
        tick: u32,
        factory: u32,
        kind: Kind,
    },
    /// Its owner took an entry off hold.
    ProductionResumed {
        tick: u32,
        factory: u32,
        kind: Kind,
    },
    /// A player picked `entity` as their primary factory of its kind.
    PrimarySet {
        tick: u32,
        entity: u32,
        kind: Kind,
        owner: u32,
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
    /// A scan picked a new target.
    TargetAcquired {
        tick: u32,
        unit: u32,
        target: u32,
    },
    Fired {
        tick: u32,
        unit: u32,
        weapon: WeaponId,
        target: u32,
    },
    ProjectileSpawned {
        tick: u32,
        projectile: u32,
        weapon: WeaponId,
        x: i64,
        y: i64,
        to_x: i64,
        to_y: i64,
    },
    /// A projectile burst, on target or not.
    ProjectileHit {
        tick: u32,
        projectile: u32,
        weapon: WeaponId,
        x: i64,
        y: i64,
    },
    /// An entity lost health.
    Hit {
        tick: u32,
        target: u32,
        attacker: u32,
        weapon: WeaponId,
        damage: i64,
        health: i64,
    },
    /// An entity was removed by damage. `killer` is whoever hit it last.
    Destroyed {
        tick: u32,
        entity: u32,
        kind: Kind,
        owner: u32,
        killer: Option<u32>,
        x: i64,
        y: i64,
    },
    /// A unit under a move order stopped.
    MoveEnded {
        tick: u32,
        unit: u32,
        reason: MoveEnd,
        x: i32,
        y: i32,
    },
    /// A unit stepped aside for `asker`.
    UnitYielded {
        tick: u32,
        unit: u32,
        asker: u32,
    },
    /// A unit gave up on a path blocked by units.
    UnitStuck {
        tick: u32,
        unit: u32,
    },
    /// A player's power supply, demand or shortfall differs from the previous tick's.
    PowerChanged {
        tick: u32,
        player: u32,
        supply: i64,
        demand: i64,
        shortfall: i64,
    },
    /// A hazard appeared, underground, at its centre (x, y).
    HazardSpawned {
        tick: u32,
        hazard: u32,
        x: i64,
        y: i64,
    },
    /// A hazard rose out of the ground to strike at (x, y).
    HazardSurfaced {
        tick: u32,
        hazard: u32,
        x: i64,
        y: i64,
    },
    /// A hazard swallowed a unit whole: no wreck, no blast.
    HazardAte {
        tick: u32,
        hazard: u32,
        unit: u32,
        kind: Kind,
        owner: u32,
        x: i64,
        y: i64,
    },
    /// A hazard left the map.
    HazardLeft {
        tick: u32,
        hazard: u32,
        reason: LeftReason,
        x: i64,
        y: i64,
    },
    /// A carrier took hold of a unit to lift it to (to_x, to_y), a tile.
    CarrierPickup {
        tick: u32,
        carrier: u32,
        unit: u32,
        to_x: i32,
        to_y: i32,
    },
    /// A carrier set a unit down on tile (x, y).
    CarrierDropoff {
        tick: u32,
        carrier: u32,
        unit: u32,
        x: i32,
        y: i32,
    },
    /// A carrier was destroyed with a unit aboard, which fell to tile (x, y) and was hurt.
    CarrierLostCargo {
        tick: u32,
        carrier: u32,
        unit: u32,
        x: i32,
        y: i32,
    },
    /// Repair was turned on for a damaged building.
    RepairStarted {
        tick: u32,
        entity: u32,
        owner: u32,
    },
    /// Repair went off: the building is whole, or its owner turned it off, sold it or lost it to a capture.
    RepairStopped {
        tick: u32,
        entity: u32,
        owner: u32,
        whole: bool,
    },
    /// A repair pad mended a vehicle to full health.
    UnitRepaired {
        tick: u32,
        unit: u32,
        pad: u32,
        owner: u32,
    },
    /// A building's owner ordered it sold; it goes in `sell.ticks`.
    SellStarted {
        tick: u32,
        entity: u32,
        kind: Kind,
        owner: u32,
    },
    /// A building was sold and removed; `refund` includes what its queue had paid.
    BuildingSold {
        tick: u32,
        entity: u32,
        kind: Kind,
        owner: u32,
        refund: i64,
        x: i64,
        y: i64,
    },
    /// A capturer took a building: the capturer is gone, the building is `to`'s.
    Captured {
        tick: u32,
        entity: u32,
        kind: Kind,
        from: u32,
        to: u32,
        by: u32,
    },
    /// A capture order was refused, or a capturer gave up because its target healed.
    CaptureRefused {
        tick: u32,
        player: u32,
        target: u32,
        reason: CaptureError,
    },
    /// A beam weapon fired along the line from (x1, y1) to (x2, y2), hitting everything on it at once.
    BeamFired {
        tick: u32,
        unit: u32,
        weapon: WeaponId,
        x1: i64,
        y1: i64,
        x2: i64,
        y2: i64,
    },
    /// A converting weapon took a unit from `from` to `to` until tick `until`.
    Converted {
        tick: u32,
        unit: u32,
        from: u32,
        to: u32,
        until: u32,
    },
    /// A converted unit went back from `from` to its own side, `to`.
    Reverted {
        tick: u32,
        unit: u32,
        from: u32,
        to: u32,
    },
    /// A unit's self-destruct countdown began; it blows up on tick `at`.
    SelfDestructStarted {
        tick: u32,
        unit: u32,
        at: u32,
    },
    /// A sapper reached a building and spent itself on it.
    SapperDetonated {
        tick: u32,
        unit: u32,
        target: u32,
    },
    /// A unit's lifetime ran out and it disappeared.
    Expired {
        tick: u32,
        unit: u32,
        kind: Kind,
        owner: u32,
        x: i64,
        y: i64,
    },
    /// The starport market's prices moved (`starport::price` reads them).
    MarketPricesChanged {
        tick: u32,
    },
    /// A starport order, or one unit for it (`kind`), was refused.
    StarportRefused {
        tick: u32,
        player: u32,
        kind: Option<Kind>,
        reason: StarportError,
    },
    /// A starport order was paid for; its ship lands on tick `arrive`.
    StarportOrderPlaced {
        tick: u32,
        starport: u32,
        player: u32,
        cost: i64,
        arrive: u32,
    },
    /// A supply ship came down on its starport to set its units down.
    SupplyShipLanded {
        tick: u32,
        starport: u32,
        ship: u32,
    },
    /// A starport was lost with units still to come, and what was paid for them came back.
    StarportOrderRefunded {
        tick: u32,
        starport: u32,
        player: u32,
        refund: i64,
    },
    /// A supply ship flew off the map.
    SupplyShipLeft {
        tick: u32,
        ship: u32,
    },
    /// A player's palace power finished charging and waits to be used.
    SuperpowerReady {
        tick: u32,
        player: u32,
        power: Superpower,
    },
    /// A palace power order that can't be carried out.
    SuperpowerRefused {
        tick: u32,
        player: u32,
        reason: SuperpowerError,
    },
    /// A palace missile left the palace for tile (x, y); it lands on tile (to_x, to_y) at tick `arrive`. Every player
    /// hears it, so its target gets a warning.
    MissileLaunched {
        tick: u32,
        player: u32,
        palace: u32,
        x: i32,
        y: i32,
        to_x: i32,
        to_y: i32,
        arrive: u32,
    },
    /// A palace missile landed on tile (x, y).
    MissileImpact {
        tick: u32,
        player: u32,
        x: i32,
        y: i32,
    },
    /// A palace power's guerrillas appeared round tile (x, y).
    GuerrillasArrived {
        tick: u32,
        player: u32,
        x: i32,
        y: i32,
        units: u32,
    },
    /// A palace power's saboteur came out of the palace.
    SaboteurArrived {
        tick: u32,
        player: u32,
        palace: u32,
        unit: u32,
    },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::HarvesterIdle { .. } => "harvester_idle",
            Event::Delivered { .. } => "delivered",
            Event::StorageFull { .. } => "storage_full",
            Event::CreditsLost { .. } => "credits_lost",
            Event::Regrowth { .. } => "regrowth",
            Event::BuildingPlaced { .. } => "building_placed",
            Event::PlacementRejected { .. } => "placement_rejected",
            Event::PowerChanged { .. } => "power_changed",
            Event::ProductionQueued { .. } => "production_queued",
            Event::ProductionRejected { .. } => "production_rejected",
            Event::ProductionPaused { .. } => "production_paused",
            Event::ProductionHeld { .. } => "production_held",
            Event::SlabLaid { .. } => "slab_laid",
            Event::BloomSeeded { .. } => "bloom_seeded",
            Event::BloomBurst { .. } => "bloom_burst",
            Event::BloomHurt { .. } => "bloom_hurt",
            Event::Decayed { .. } => "decayed",
            Event::ProductionResumed { .. } => "production_resumed",
            Event::PrimarySet { .. } => "primary_set",
            Event::ProductionCancelled { .. } => "production_cancelled",
            Event::BuildingReady { .. } => "building_ready",
            Event::UnitBuilt { .. } => "unit_built",
            Event::TargetAcquired { .. } => "target_acquired",
            Event::Fired { .. } => "fired",
            Event::ProjectileSpawned { .. } => "projectile_spawned",
            Event::ProjectileHit { .. } => "projectile_hit",
            Event::Hit { .. } => "hit",
            Event::Destroyed { .. } => "destroyed",
            Event::MoveEnded { .. } => "move_ended",
            Event::UnitYielded { .. } => "unit_yielded",
            Event::UnitStuck { .. } => "unit_stuck",
            Event::HazardSpawned { .. } => "hazard_spawned",
            Event::HazardSurfaced { .. } => "hazard_surfaced",
            Event::HazardAte { .. } => "hazard_ate",
            Event::HazardLeft { .. } => "hazard_left",
            Event::CarrierPickup { .. } => "carrier_pickup",
            Event::CarrierDropoff { .. } => "carrier_dropoff",
            Event::CarrierLostCargo { .. } => "carrier_lost_cargo",
            Event::RepairStarted { .. } => "repair_started",
            Event::RepairStopped { .. } => "repair_stopped",
            Event::UnitRepaired { .. } => "unit_repaired",
            Event::SellStarted { .. } => "sell_started",
            Event::BuildingSold { .. } => "building_sold",
            Event::Captured { .. } => "captured",
            Event::CaptureRefused { .. } => "capture_refused",
            Event::BeamFired { .. } => "beam_fired",
            Event::Converted { .. } => "converted",
            Event::Reverted { .. } => "reverted",
            Event::SelfDestructStarted { .. } => "self_destruct_started",
            Event::SapperDetonated { .. } => "sapper_detonated",
            Event::Expired { .. } => "expired",
            Event::MarketPricesChanged { .. } => "market_prices_changed",
            Event::StarportRefused { .. } => "starport_refused",
            Event::StarportOrderPlaced { .. } => "starport_order_placed",
            Event::SupplyShipLanded { .. } => "supply_ship_landed",
            Event::StarportOrderRefunded { .. } => "starport_order_refunded",
            Event::SupplyShipLeft { .. } => "supply_ship_left",
            Event::SuperpowerReady { .. } => "superpower_ready",
            Event::SuperpowerRefused { .. } => "superpower_refused",
            Event::MissileLaunched { .. } => "power_missile_launched",
            Event::MissileImpact { .. } => "power_missile_impact",
            Event::GuerrillasArrived { .. } => "guerrillas_arrived",
            Event::SaboteurArrived { .. } => "saboteur_arrived",
        }
    }

    pub fn tick(&self) -> u32 {
        match *self {
            Event::HarvesterIdle { tick, .. }
            | Event::Delivered { tick, .. }
            | Event::StorageFull { tick, .. }
            | Event::CreditsLost { tick, .. }
            | Event::Regrowth { tick, .. }
            | Event::BuildingPlaced { tick, .. }
            | Event::PlacementRejected { tick, .. }
            | Event::PowerChanged { tick, .. }
            | Event::ProductionQueued { tick, .. }
            | Event::ProductionRejected { tick, .. }
            | Event::ProductionPaused { tick, .. }
            | Event::ProductionHeld { tick, .. }
            | Event::SlabLaid { tick, .. }
            | Event::BloomSeeded { tick, .. }
            | Event::BloomBurst { tick, .. }
            | Event::BloomHurt { tick, .. }
            | Event::Decayed { tick, .. }
            | Event::ProductionResumed { tick, .. }
            | Event::PrimarySet { tick, .. }
            | Event::ProductionCancelled { tick, .. }
            | Event::BuildingReady { tick, .. }
            | Event::UnitBuilt { tick, .. }
            | Event::TargetAcquired { tick, .. }
            | Event::Fired { tick, .. }
            | Event::ProjectileSpawned { tick, .. }
            | Event::ProjectileHit { tick, .. }
            | Event::Hit { tick, .. }
            | Event::Destroyed { tick, .. }
            | Event::MoveEnded { tick, .. }
            | Event::UnitYielded { tick, .. }
            | Event::UnitStuck { tick, .. }
            | Event::HazardSpawned { tick, .. }
            | Event::HazardSurfaced { tick, .. }
            | Event::HazardAte { tick, .. }
            | Event::HazardLeft { tick, .. }
            | Event::CarrierPickup { tick, .. }
            | Event::CarrierDropoff { tick, .. }
            | Event::CarrierLostCargo { tick, .. }
            | Event::BeamFired { tick, .. }
            | Event::Converted { tick, .. }
            | Event::Reverted { tick, .. }
            | Event::SelfDestructStarted { tick, .. }
            | Event::SapperDetonated { tick, .. }
            | Event::Expired { tick, .. }
            | Event::MarketPricesChanged { tick }
            | Event::StarportRefused { tick, .. }
            | Event::StarportOrderPlaced { tick, .. }
            | Event::SupplyShipLanded { tick, .. }
            | Event::StarportOrderRefunded { tick, .. }
            | Event::SupplyShipLeft { tick, .. }
            | Event::SuperpowerReady { tick, .. }
            | Event::SuperpowerRefused { tick, .. }
            | Event::MissileLaunched { tick, .. }
            | Event::MissileImpact { tick, .. }
            | Event::GuerrillasArrived { tick, .. }
            | Event::SaboteurArrived { tick, .. } => tick,
            Event::RepairStarted { tick, .. }
            | Event::RepairStopped { tick, .. }
            | Event::UnitRepaired { tick, .. }
            | Event::SellStarted { tick, .. }
            | Event::BuildingSold { tick, .. }
            | Event::Captured { tick, .. }
            | Event::CaptureRefused { tick, .. } => tick,
        }
    }
}

pub(crate) fn centre(t: i32) -> i64 {
    t as i64 * TILE + TILE / 2
}

pub fn spawn(state: &mut GameState, rules: &Rules, kind: Kind, owner: u32, tx: i32, ty: i32) -> u32 {
    let id = state.next_id;
    state.next_id += 1;
    let harvester = rules.kind(kind).harvester.is_some();
    // Only units run out; a building would leave its tiles blocked.
    let lifetime = rules.kind(kind).lifetime;
    let expires = (lifetime > 0 && !rules.kind(kind).building).then_some(state.tick + lifetime);
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
        facing: 0,
        reload: 0,
        target: None,
        last_attacker: None,
        wait: 0,
        repath_fails: 0,
        yield_for: None,
        yield_at: None,
        noise: 0,
        altitude: 0,
        carried_by: None,
        ferry: None,
        goal: None,
        repairing: false,
        repair_due: 0,
        selling: 0,
        converted: None,
        fuse: None,
        expires,
        autonomous: None,
        primary: false,
        foundation: 0,
    });
    id
}

/// The tiles touching a footprint whose top-left tile is (x, y): the ring one tile out, corners included, in row
/// order.
pub fn around(x: i32, y: i32, w: i32, h: i32) -> impl Iterator<Item = Tile> {
    (y - 1..=y + h)
        .flat_map(move |ty| (x - 1..=x + w).map(move |tx| Tile { x: tx, y: ty }))
        .filter(move |t| !(x..x + w).contains(&t.x) || !(y..y + h).contains(&t.y))
}

/// Where harvester `me` parks to unload at a refinery, and its way there: any tile touching the footprint that it can
/// reach, the one nearest the harvester that no other unit stands on, is stepping into or is heading for to unload,
/// then nearest the middle of the map, then in row order. If every one is taken it queues for the nearest. A
/// refinery has no one side to dock on, so a base on any edge of the map unloads on the side facing its fields. If
/// no side can be reached, the nearest side with no way there, as a refinery with one unreachable dock always gave.
fn dock_for(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &GameState,
    rules: &Rules,
    refinery: &Entity,
    me: usize,
) -> (Tile, VecDeque<Tile>) {
    let (t, k) = (refinery.tile(), rules.kind(refinery.kind));
    let here = state.entities[me].tile();
    let taken = |d: Tile| {
        state.entities.iter().enumerate().any(|(j, e)| {
            j != me
                && on_ground(rules, e)
                && (e.tile() == d
                    || movement::step_tile(e) == Some(d)
                    || (e.task == Some(Task::ToRefinery) && e.path.back() == Some(&d)))
        })
    };
    let d2 = |d: &Tile| ((d.x - here.x) as i64).pow(2) + ((d.y - here.y) as i64).pow(2);
    let mut docks: Vec<Tile> = around(t.x, t.y, k.width, k.height).filter(|d| pf.passable(d.x, d.y)).collect();
    docks.sort_by_key(|d| (taken(*d), d2(d), d.off_middle(map.width, map.height), d.y, d.x));
    let nearest = docks.first().copied().unwrap_or_else(|| dock_at(k, t.x, t.y));
    docks
        .into_iter()
        .find_map(|d| {
            let path = path_or_empty(pf, here, d);
            (d == here || !path.is_empty()).then_some((d, path))
        })
        .unwrap_or((nearest, VecDeque::new()))
}

/// The tile below the middle column of a refinery kind whose top-left tile is at (x, y): where the art draws its pad,
/// and where a starting harvester is put.
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

pub(crate) fn path_or_empty(pf: &mut Pathfinder, from: Tile, to: Tile) -> VecDeque<Tile> {
    pf.find(from.x, from.y, to.x, to.y).map(|p| p.tiles.into()).unwrap_or_default()
}

/// Ground units whose remaining path now crosses a blocked tile find a new way to the same end, in id order.
fn reroute_around_new_building(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules) {
    for e in &mut state.entities {
        if on_ground(rules, e) && e.path.iter().any(|t| !pf.passable(t.x, t.y)) {
            let end = *e.path.back().expect("a path that crosses something is not empty");
            e.path = movement::route(pf, e, end);
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
        CommandOrder::Hold { kind, on } => {
            return production::hold(state, rules, cmd.player, &cmd.ids, kind, on, events);
        }
        CommandOrder::Primary => return production::set_primary(state, rules, cmd.player, &cmd.ids, events),
        CommandOrder::Repair { on } => return repair::order(state, rules, cmd.player, &cmd.ids, on, events),
        CommandOrder::Sell => return sell::order(state, rules, cmd.player, &cmd.ids, events),
        CommandOrder::Capture { target } => {
            return capture::order(pf, state, rules, cmd.player, &cmd.ids, target, events);
        }
        CommandOrder::RepairAt { pad } => return repair::send(pf, state, rules, cmd.player, &cmd.ids, pad),
        CommandOrder::StarportAdd { kind } => return starport::add(state, rules, cmd.player, &cmd.ids, kind, events),
        CommandOrder::StarportRemove { kind } => {
            return starport::remove(state, rules, cmd.player, &cmd.ids, kind, events);
        }
        CommandOrder::StarportConfirm => return starport::confirm(state, rules, cmd.player, &cmd.ids, events),
        CommandOrder::Superpower { x, y } => return superpower::fire(map, pf, state, rules, cmd.player, x, y, events),
        _ => {}
    }
    if let CommandOrder::Place { kind, x, y } = cmd.order {
        let tick = state.tick;
        let ready = if production::has_ready(state, cmd.player, kind) { Ok(()) } else { Err(PlaceError::NotReady) };
        match ready.and_then(|()| placement::check(map, state, rules, cmd.player, kind, x, y)) {
            Ok(()) if rules.kind(kind).slab => {
                production::take_ready(state, cmd.player, kind);
                let tiles = decay::lay(map, state, rules, cmd.player, kind, x, y);
                events.push(Event::SlabLaid { tick, kind, owner: cmd.player, x, y, tiles });
            }
            Ok(()) => {
                production::take_ready(state, cmd.player, kind);
                let foundation = decay::foundation(state, rules, cmd.player, kind, x, y);
                let entity = spawn(state, rules, kind, cmd.player, x, y);
                state.entities.last_mut().expect("just spawned").foundation = foundation;
                occupy(pf, rules, state.entities.last().expect("just spawned"), true);
                reroute_around_new_building(pf, state, rules);
                events.push(Event::BuildingPlaced { tick, entity, kind, owner: cmd.player, x, y });
            }
            Err(reason) => events.push(Event::PlacementRejected { tick, player: cmd.player, kind, x, y, reason }),
        }
        return;
    }
    // An attack names a target that must still be there; the units go after it in the combat phase.
    let attack = match cmd.order {
        // Under fog, only a target the player can see, or a building it keeps a ghost of.
        CommandOrder::Attack { target } => match state.entity(target) {
            Some(t) if vision::known(state, rules, cmd.player, t) => Some(target),
            _ => return,
        },
        _ => None,
    };
    for &id in &cmd.ids {
        let Ok(i) = state.entities.binary_search_by_key(&id, |e| e.id) else { continue };
        // A supply ship on a delivery flies itself.
        if state.deliveries.iter().any(|d| d.ship == Some(id)) {
            continue;
        }
        let e = &mut state.entities[i];
        // A unit being carried takes no orders until it is set down, and one counting down to its blast none at all,
        // nor one fighting on its own.
        if e.owner != cmd.player
            || rules.kind(e.kind).building
            || e.carried_by.is_some()
            || e.fuse.is_some()
            || e.autonomous.is_some()
        {
            continue;
        }
        let k = rules.kind(e.kind);
        match cmd.order {
            // A carrier with a unit aboard finishes the lift first.
            CommandOrder::Move { .. } if air::lifting(e) => {}
            CommandOrder::Move { x, y } if k.air => {
                // An aircraft flies straight there; a carrier on its way to a pickup gives the job up.
                e.ferry = None;
                e.path = [Tile { x: x.clamp(0, map.width - 1), y: y.clamp(0, map.height - 1) }].into();
                e.order = Order::Move;
                e.target = None;
            }
            CommandOrder::Move { x, y } => {
                e.path = movement::route(pf, e, Tile { x, y });
                e.order = Order::Move;
                e.target = None;
                e.goal = None;
            }
            CommandOrder::Harvest if rules.kind(e.kind).harvester.is_some() => {
                e.goal = None;
                e.order = Order::Harvest;
                e.task = Some(Task::Seek);
                movement::halt(e);
            }
            CommandOrder::Attack { .. } if k.weapon.is_some() && attack != Some(e.id) => {
                e.goal = None;
                e.order = Order::Attack;
                e.target = attack;
                movement::stop(rules, e);
            }
            CommandOrder::Attack { .. } => {}
            CommandOrder::SelfDestruct if k.self_destruct.is_some() => {
                let at = state.tick + rules.combat.self_destruct_ticks;
                e.fuse = Some(at);
                e.order = Order::Idle;
                e.target = None;
                e.goal = None;
                movement::stop(rules, e);
                events.push(Event::SelfDestructStarted { tick: state.tick, unit: id, at });
            }
            CommandOrder::SelfDestruct
            | CommandOrder::Harvest
            | CommandOrder::Place { .. }
            | CommandOrder::Produce { .. }
            | CommandOrder::Cancel { .. }
            | CommandOrder::Hold { .. }
            | CommandOrder::Primary
            | CommandOrder::Repair { .. }
            | CommandOrder::Sell
            | CommandOrder::Capture { .. }
            | CommandOrder::RepairAt { .. }
            | CommandOrder::StarportAdd { .. }
            | CommandOrder::StarportRemove { .. }
            | CommandOrder::StarportConfirm
            | CommandOrder::Superpower { .. } => {}
        }
    }
}

/// Advance one tick. `events` receives what happened; it never feeds back in. `hazard_pf` is the hazard's own
/// pathfinder (`hazard::pathfinder`), used only while the rules turn it on.
#[allow(clippy::too_many_arguments)]
pub fn step(
    map: &MapData,
    pf: &mut Pathfinder,
    hazard_pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    commands: &[Command],
    events: &mut Vec<Event>,
) {
    // The tick runs in phases, each over entities in id order (rules-movement.md, "Moving within a tick"):
    // commands, combat, movement, aircraft, crush (not built yet), the hazard, capture, repair, selling, decay, economy,
    // world (regrowth or blooms);
    // then each player's sight.
    let power_before = Power::all(state, rules);
    for cmd in commands {
        apply_command(map, pf, state, rules, cmd, events);
    }
    superpower::tick(map, pf, state, rules, events);
    combat::tick(pf, state, rules, events);
    movement::tick(pf, state, rules, events);
    air::tick(map, pf, state, rules, events);
    hazard::tick(map, hazard_pf, state, rules, events);
    capture::tick(pf, state, rules, events);
    repair::tick(pf, state, rules, events);
    sell::tick(pf, state, rules, events);
    decay::tick(state, rules, events);
    economy(map, pf, state, rules, events);
    // Blooms replace the slow regrowth beside fields while they are on.
    if rules.blooms.is_none() {
        regrow(map, pf, state, rules, events);
    }
    blooms::tick(map, state, rules, events);
    vision::tick(state, rules);
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

/// Harvesters on their loop (find a field, mine, return, unload), then every production queue.
fn economy(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    for i in 0..state.entities.len() {
        let e = &state.entities[i];
        // A harvester in a carrier's hold has its loop on hold too.
        if rules.kind(e.kind).harvester.is_some() && e.order == Order::Harvest && e.carried_by.is_none() {
            harvest(map, pf, state, rules, i, events);
        }
    }
    production::tick(map, pf, state, rules, events);
    starport::tick(map, pf, state, rules, events);
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
            let Some(field) = nearest_resource(map, pf, state, rules, e_id, here) else {
                state.entities[i].task = Some(Task::Stuck);
                events.push(Event::HarvesterIdle { tick, unit: e_id, reason: IdleReason::NoResource });
                return;
            };
            let e = &mut state.entities[i];
            e.path = movement::route(pf, e, field);
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
                let Some((_, path)) = home_refinery(map, pf, state, rules, i) else {
                    state.entities[i].task = Some(Task::Stuck);
                    let unit = state.entities[i].id;
                    events.push(Event::HarvesterIdle { tick, unit, reason: IdleReason::NoRefinery });
                    return;
                };
                let e = &mut state.entities[i];
                e.path = path;
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
            storage::deliver(state, rules, owner as usize, amount, events);
            if empty {
                let credits = state.players[owner as usize].credits;
                events.push(Event::Delivered { tick, unit, player: owner, credits });
                state.entities[i].task = Some(Task::Seek);
            }
        }
        Task::Stuck => {}
    }
}

/// The harvester's refinery: its remembered one if that still stands, else its owner's first. Returns the dock tile
/// and the way there, and remembers the choice.
fn home_refinery(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    i: usize,
) -> Option<(Tile, VecDeque<Tile>)> {
    let (owner, home_id) = (state.entities[i].owner, state.entities[i].home_id);
    let mut own = state.entities.iter().filter(|r| rules.kind(r.kind).refinery && r.owner == owner);
    let first = own.clone().next();
    let chosen = own.find(|r| Some(r.id) == home_id).or(first)?;
    let (id, dock) = (chosen.id, dock_for(map, pf, state, rules, chosen, i));
    state.entities[i].home_id = Some(id);
    Some(dock)
}

/// Breadth-first search outward over tiles ground units can enter; the first tile with resource left and no other
/// unit on it wins.
fn nearest_resource(
    map: &MapData,
    pf: &Pathfinder,
    state: &GameState,
    rules: &Rules,
    me: u32,
    from: Tile,
) -> Option<Tile> {
    let mut held = vec![false; state.resource.len()];
    for e in state.entities.iter().filter(|e| e.id != me && on_ground(rules, e)) {
        for t in std::iter::once(e.tile()).chain(movement::step_tile(e)) {
            if map.in_bounds(t.x, t.y) {
                held[map.index(t.x, t.y)] = true;
            }
        }
    }
    // Ring by ring (steps away), so of the tiles the same number of steps away the one nearest in a straight line
    // wins, then the one nearer the middle of the map: choices that turn round with a mirrored map, where the order
    // the search happens to visit tiles in would favour one side.
    let start = map.index(from.x, from.y);
    let mut seen = vec![false; state.resource.len()];
    seen[start] = true;
    let mut ring = vec![start];
    let key = |t: usize| {
        let tile = map.tile_at(t);
        let d = |a: i32, b: i32| ((a - b) as i64).pow(2);
        (d(tile.x, from.x) + d(tile.y, from.y), tile.off_middle(map.width, map.height), t)
    };
    while !ring.is_empty() {
        if let Some(&t) = ring.iter().filter(|&&t| state.resource[t] > 0 && !held[t]).min_by_key(|&&t| key(t)) {
            return Some(map.tile_at(t));
        }
        let mut next = Vec::new();
        for &t in &ring {
            let Tile { x, y } = map.tile_at(t);
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x + dx, y + dy);
                if pf.passable(nx, ny) {
                    let n = map.index(nx, ny);
                    if !seen[n] {
                        seen[n] = true;
                        next.push(n);
                    }
                }
            }
        }
        ring = next;
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
