//! Unit and building kinds, and the typed rules the tick reads. Kinds come from the rules data
//! (`data/rules/entities.json`, through `classic-data`): every built unit and building there is a kind, and its
//! roles say which mechanics it takes part in. The numbers carry a setting pack's tuning; they are our own, tuned by
//! simulation runs, never copied from another game's tables. Speeds are sub-tile units per tick (256 per tile, 15
//! ticks per second).

use classic_data::{ARMOURS, RulesTable, WARHEADS};

pub const TICKS_PER_SECOND: u32 = 15;

/// A unit or building kind: an index into `Rules::kinds`, which are in generic-id order. The state hash spells
/// the generic id, never the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Kind(pub u16);

/// A weapon: an index into `Rules::weapons`, which are in generic-id order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeaponId(pub u16);

/// Everything the combat phase needs to know about one weapon (`data/rules/weapons.json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponRules {
    pub id: String,
    /// Row in the damage table, an index into `classic_data::WARHEADS`.
    pub warhead: usize,
    /// Sub-tile units, centre to centre.
    pub range: i64,
    pub min_range: i64,
    /// Ticks between shots.
    pub reload: u32,
    pub damage: i64,
    /// Sub-tile units per tick; 0 hits at once.
    pub speed: i64,
    pub scatter: i64,
    pub splash: i64,
    /// Doesn't fire while its owner is short of power.
    pub needs_power: bool,
    /// May aim at aircraft in flight.
    pub hits_air: bool,
    /// May aim at anything on the ground, landed aircraft included.
    pub hits_ground: bool,
    /// A beam's width in sub-tile units: when it fires it hits at once everything along a line `range` long towards
    /// its target, except the firer and units of its own kind. 0 for every other weapon.
    pub beam: i64,
    /// Ticks an enemy vehicle in its burst changes sides for, instead of being hurt. 0 for every other weapon.
    pub converts: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatRules {
    /// Percent of base damage, by warhead row and armour column.
    pub table: [[i64; 6]; 6],
    /// Splash, beams and blasts hurt their own side at this percent.
    pub own_splash_percent: i64,
    /// Armed units look for targets once every this many ticks, staggered by id.
    pub scan_every: u32,
    /// Ticks from a self-destruct order to the blast.
    pub self_destruct_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarvesterStats {
    pub capacity: i64,
    pub mine_rate: i64,
    pub unload_rate: i64,
}

/// Everything the tick needs to know about one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindRules {
    /// The generic id, as data files and the state hash spell it.
    pub id: String,
    pub building: bool,
    /// Zero for buildings.
    pub speed: i64,
    pub max_health: i64,
    /// Footprint in tiles, from the top-left tile; 1 by 1 for units.
    pub width: i32,
    pub height: i32,
    /// Set for kinds with the `harvester` role.
    pub harvester: Option<HarvesterStats>,
    /// Kinds with the `refinery` role take harvesters' cargo at their dock.
    pub refinery: bool,
    /// Kinds with the `wall` role block movement but don't extend their owner's building area.
    pub wall: bool,
    /// Kinds with the `slab` role are laid as concrete under the footprint rather than standing as a building (the
    /// `decay` module).
    pub slab: bool,
    /// Kinds with the `air` role fly (the air module): straight over any tile, with no ground collision.
    pub air: bool,
    /// Kinds with the `carrier` role lift their owner's harvesters on long trips.
    pub carrier: bool,
    /// False for kinds with the `untargetable` role, which nothing may fire on.
    pub targetable: bool,
    /// Buildings with the `capturable` role can be taken by a `capturer` unit (the capture module).
    pub capturable: bool,
    /// Units with the `capturer` role can take an enemy's `capturable` building.
    pub capturer: bool,
    /// Buildings with the `repair_pad` role mend their owner's vehicles beside them (the repair module).
    pub repair_pad: bool,
    /// A ground vehicle (a unit with `light` or `heavy` armour): what a repair pad mends.
    pub vehicle: bool,
    /// Column in the damage table, an index into `classic_data::ARMOURS`.
    pub armour: usize,
    /// What it fires, if armed.
    pub weapon: Option<WeaponId>,
    /// The blast it leaves when destroyed.
    pub death: Option<WeaponId>,
    /// The blast it leaves instead when destroyed after its owner ordered it to destroy itself; `None` for kinds
    /// that can't.
    pub self_destruct: Option<WeaponId>,
    /// False for kinds with the `unconvertible` role, which a converting weapon never takes over.
    pub convertible: bool,
    /// Kinds with the `sapper` role go only for buildings, and are used up by the one shot that hits.
    pub sapper: bool,
    /// Sub-tile units: another player sees it only while one of their units or buildings is this close. 0 for
    /// kinds that don't hide.
    pub cloak: i64,
    /// Ticks it lasts before it disappears on its own; 0 for kinds that last until destroyed.
    pub lifetime: u32,
    /// The generic faction ids that may build it; empty for every faction.
    pub factions: Vec<String>,
    /// Its base price at the starport; 0 for kinds the starport doesn't sell.
    pub starport_price: i64,
    /// How many each player may buy at the starport before it restocks, and the ticks it takes to restock one.
    pub starport_stock: u32,
    pub starport_restock: u32,
    /// Facing units per tick its weapon turns (256 to a full turn).
    pub turn_rate: i64,
    /// How far it looks for targets, in sub-tile units: its sight, or its weapon's range if it has no sight.
    pub sight: i64,
    /// How far it uncovers the map for its owner while the fog module is on, in tiles: from a building's edge, from
    /// a unit's tile.
    pub vision: i32,
    /// Credits, paid while it builds.
    pub cost: i64,
    /// Ticks to build at full power.
    pub build_ticks: i64,
    /// The building kind that produces it; `None` for kinds no player can build.
    pub built_at: Option<Kind>,
    /// Building kinds its owner must have before it can be built.
    pub requires: Vec<Kind>,
    /// Added to the owner's power supply when positive, drawn from it when negative; zero for units.
    pub power: i64,
    /// Noise it makes each tick it moves on open ground, which draws the hazard; 0 for silent kinds.
    pub noise: i64,
    /// Credits it adds to its owner's storage cap (the `storage` module); 0 for most kinds.
    pub storage: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regrowth {
    pub every_ticks: u32,
    pub amount: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The most empty tiles allowed between a new building and the nearest building its owner already has (walls
    /// don't count). 0 means touching, diagonals included.
    pub max_gap: i32,
    /// Buildings may only stand on rock, not open ground.
    pub rock_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionRules {
    /// The most entries one building's queue holds, the one in progress included.
    pub queue_size: usize,
    /// Each player's credits at the start of a skirmish.
    pub starting_credits: i64,
    /// A test switch: every entry finishes, paid in full, on the tick it reaches the head of its queue.
    pub instant_build: bool,
}

/// Collision and blocked units (rules-movement.md, sections 4 to 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovementRules {
    /// A blocked unit waits `wait_base` plus a random `0..wait_random` ticks before looking for a way round.
    pub wait_base: u32,
    pub wait_random: u32,
    /// Failed searches for a way round, in a row, before a unit gives up.
    pub max_repath_fails: u32,
    /// Ticks a request to step aside stays good.
    pub yield_expires: u32,
    /// How far from a taken last tile a move may end instead.
    pub close_enough_rings: i32,
    /// Nodes a search for a way round may expand.
    pub nodes_local: u32,
}

/// The hazard's numbers (rules-world.md, section 7; the `hazard` module). Distances are in tiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HazardRules {
    /// The most in play at once.
    pub max: u32,
    /// The first tick one may appear.
    pub first_tick: u32,
    /// Ticks after one leaves before the next may appear.
    pub respawn_ticks: u32,
    /// Tiles a new one keeps from every building.
    pub spawn_clearance: i32,
    /// Sub-tile units per tick, underground.
    pub speed: i64,
    /// Ticks between looks for a victim; also how often noise halves and the way is found again.
    pub scan_every: u32,
    /// How far it looks for a victim.
    pub scan_range: i64,
    /// How far a victim may get before it gives up.
    pub give_up_range: i64,
    /// How far it roams when nothing draws it.
    pub wander_range: i32,
    /// Ticks it stays up after a strike.
    pub surface_ticks: u32,
    /// Units it eats before it leaves; 0 means it never leaves.
    pub appetite: u32,
    /// Ticks a full one takes to go.
    pub leave_ticks: u32,
    /// Noise a harvester makes each tick it mines.
    pub mining_noise: i64,
    /// Noise a unit makes on a tick it fires.
    pub firing_noise: i64,
}

/// Aircraft (rules-movement.md section 9; rules-economy-production.md section 13 for the carrier).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AirRules {
    /// The height an aircraft flies at, in sub-tile units; 0 is landed. Only the renderer draws it, but an aircraft
    /// moves and fires only once it is all the way up.
    pub cruise_altitude: i64,
    /// Height gained or lost each tick taking off or landing.
    pub climb: i64,
    /// A harvester whose way still to go is longer than this many tiles is offered a lift.
    pub ferry_min_path: usize,
    /// Ticks a carrier hovers to pick a unit up, and to set one down.
    pub pickup_ticks: u32,
    pub drop_ticks: u32,
    /// How far from a taken drop tile a carrier may set down instead, and how often it looks again if it can't.
    pub drop_rings: i32,
    pub drop_retry_ticks: u32,
    /// Percent of its full health a unit loses when its carrier is destroyed under it.
    pub fall_damage_percent: i64,
}

/// The starport market (rules-economy-production.md, section 12; the `starport` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StarportRules {
    /// What it sells: every kind with a starport price, in kind order.
    pub catalogue: Vec<Kind>,
    /// Ticks between price moves, and the most one move shifts a price, in percent of its base.
    pub drift_every: u32,
    pub drift_step: i64,
    /// Prices stay within these percents of base.
    pub min_percent: i64,
    pub max_percent: i64,
    /// Ticks from paying to the supply ship landing, doubled when its owner is short of power.
    pub delivery_ticks: u32,
    /// Ticks between units leaving a landed supply ship.
    pub unload_every: u32,
    /// The most units one order holds.
    pub max_order: usize,
}

/// A palace power (rules-world.md, section 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Superpower {
    /// A slow, inaccurate missile with a large blast, at any explored tile.
    Missile,
    /// A band of fighters that appear near an explored tile and fight there on their own.
    Guerrillas,
    /// A cloaked sapper that appears at the palace.
    Saboteur,
}

impl Superpower {
    pub const ALL: [Superpower; 3] = [Superpower::Missile, Superpower::Guerrillas, Superpower::Saboteur];

    /// Its generic id.
    pub fn id(self) -> &'static str {
        match self {
            Superpower::Missile => "power_missile",
            Superpower::Guerrillas => "power_guerrillas",
            Superpower::Saboteur => "power_saboteur",
        }
    }
}

/// The palace powers (rules-world.md, section 8; the `superpowers` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperpowerRules {
    /// Each pack faction's power, by faction id, in id order.
    pub by_faction: Vec<(String, Superpower)>,
    /// Ticks each power takes to charge, in `Superpower::ALL` order.
    pub charge: [u32; 3],
    /// The missile flies `flight + flight_per_tile` ticks per tile of distance, and lands off target by up to
    /// `spread + distance / spread_tiles` tiles.
    pub missile_flight: u32,
    pub missile_flight_per_tile: u32,
    pub missile_spread: i64,
    pub missile_spread_tiles: i64,
    /// Its damage to everything 0, 1, 2 and 3 tiles (the larger offset) from where it lands.
    pub missile_damage: [i64; 4],
    /// The weapon its hits are reported as.
    pub missile_weapon: WeaponId,
    /// The guerrillas: ticks before they arrive, how many, and how far from the target tile.
    pub guerrillas_delay: u32,
    pub guerrillas_count: u32,
    pub guerrillas_min_range: i32,
    pub guerrillas_max_range: i32,
    pub guerrilla: Kind,
    pub saboteur: Kind,
}

impl SuperpowerRules {
    /// The power a faction's palace gives, if any.
    pub fn of(&self, faction: Option<&str>) -> Option<Superpower> {
        let f = faction?;
        self.by_faction.iter().find(|(id, _)| id == f).map(|&(_, p)| p)
    }

    pub fn charge_ticks(&self, p: Superpower) -> u32 {
        self.charge[p as usize]
    }
}

/// Fog of war (rules-world.md, sections 2 and 3; the `fog` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FogRules {
    /// Fog hides enemy units on explored ground out of sight, and shows enemy buildings there as last seen. Off, it
    /// is shroud only: once explored, ground and everything on it stays in view, as in the original.
    pub hide: bool,
    /// Ticks a unit that fires stays in view of the player it fired at.
    pub reveal_ticks: u32,
    /// A test switch: every tile starts explored.
    pub start_explored: bool,
}

/// The storage cap's numbers (rules-economy-production.md, section 6; the `storage` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageRules {
    /// Ticks between `credits_lost` events for one player.
    pub warn_every: u32,
}

/// Repairing buildings for credits, and vehicles at a repair pad (rules-base-building-power.md, "Repairing
/// buildings"; the `repair` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairRules {
    /// A repairing building takes a step every this many ticks at full power; a short player's take longer.
    pub every: u32,
    /// A building's step is `max(1, max_health / step_div)` health.
    pub step_div: i64,
    /// What mending from nothing to full costs, in percent of the kind's cost; each step pays its share, at least 1
    /// credit, unless this is 0.
    pub cost_percent: i64,
    /// A repair pad takes a step every this many ticks at full power, of `pad_step` health.
    pub pad_every: u32,
    pub pad_step: i64,
}

/// Selling buildings back (rules-base-building-power.md, "Selling"; the `sell` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SellRules {
    /// Percent of the cost, scaled by health, paid back.
    pub refund_percent: i64,
    /// Ticks between the order and the building going, while it stops working but can still be shot.
    pub ticks: u32,
}

/// Resource blooms (rules-world.md, section 6; the `blooms` module). Distances are in tiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BloomRules {
    /// How far a burst spreads resource.
    pub radius: i32,
    /// Resource added at the centre, and how much less on each ring out.
    pub centre: i64,
    pub per_tile: i64,
    /// Health taken from every ground unit on the bloom's tile and the 8 round it.
    pub damage: i64,
    /// The range of the random wait before a burst bloom is replaced.
    pub reseed_min: u32,
    pub reseed_max: u32,
    /// How far from the old point the new one may be.
    pub reseed_range: i32,
    /// The wait before trying again when no tile was free.
    pub retry: u32,
    /// A bloom bursts on its own this long after it appeared.
    pub max_age: u32,
}

/// Buildings wearing down off concrete (rules-base-building-power.md, "Foundations and decay"; the `decay` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecayRules {
    /// A building takes a step every this many ticks, staggered by id.
    pub every: u32,
    /// A step is `max(1, max_health / step_div)` health.
    pub step_div: i64,
    /// The percent of its maximum health a building with no slab under it decays down to.
    pub floor_percent: i64,
}

/// Infantry taking enemy buildings (rules-base-building-power.md, "Capture"; the `capture` module).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRules {
    /// A building can be taken only below this percent of its maximum health; 100 means at any health.
    pub below_percent: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerRules {
    /// The lowest power factor, in percent, however short a player is.
    pub min_factor: i64,
}

/// Every number the tick reads, built once from a rules table before the game starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub kinds: Vec<KindRules>,
    pub regrowth: Regrowth,
    pub placement: Placement,
    pub power: PowerRules,
    pub storage: StorageRules,
    pub production: ProductionRules,
    pub movement: MovementRules,
    pub air: AirRules,
    pub weapons: Vec<WeaponRules>,
    pub combat: CombatRules,
    /// Set when a setting pack turns the hazard on.
    pub hazard: Option<HazardRules>,
    /// Set when a setting pack turns fog of war on.
    pub fog: Option<FogRules>,
    /// Set while the starport module is on.
    pub starport: Option<StarportRules>,
    /// Set while the superpowers module is on and the rules have a palace.
    pub superpowers: Option<SuperpowerRules>,
    pub repair: RepairRules,
    /// Set unless a setting pack turns selling off.
    pub sell: Option<SellRules>,
    /// Set unless a setting pack turns capture off.
    pub capture: Option<CaptureRules>,
    /// Set when a setting pack turns decay on, which brings slabs too.
    pub decay: Option<DecayRules>,
    /// Set when a setting pack turns blooms on, which replace regrowth.
    pub blooms: Option<BloomRules>,
    /// The rules table's hash, for replays to check they run under the same numbers.
    pub hash: String,
}

impl Rules {
    /// The typed rules from a table, such as `RulesTable::builtin()` or a setting pack's tuned rules.
    pub fn from_table(t: &RulesTable) -> Result<Rules, String> {
        let mut kinds = Vec::new();
        for (id, e) in &t.entities {
            if !e.built || (e.kind != "unit" && e.kind != "building") {
                continue;
            }
            let num = |name: &str| t.number(id, name).ok_or(format!("rules data has no {id}.{name}"));
            let building = e.kind == "building";
            let role = |r: &str| e.roles.iter().any(|x| x == r);
            let harvester = if role("harvester") {
                Some(HarvesterStats {
                    capacity: num("capacity")?,
                    mine_rate: num("mine_rate")?,
                    unload_rate: num("unload_rate")?,
                })
            } else {
                None
            };
            let (width, height) = if building { (num("width")? as i32, num("height")? as i32) } else { (1, 1) };
            kinds.push(KindRules {
                id: id.clone(),
                building,
                speed: if building { 0 } else { num("speed")? },
                max_health: num("max_health")?,
                width,
                height,
                harvester,
                refinery: role("refinery"),
                wall: role("wall"),
                slab: building && role("slab"),
                air: role("air"),
                carrier: role("carrier"),
                targetable: !role("untargetable"),
                capturable: building && role("capturable"),
                capturer: !building && role("capturer"),
                repair_pad: building && role("repair_pad"),
                vehicle: !building && matches!(e.armour.as_deref(), Some("light" | "heavy")),
                convertible: !role("unconvertible"),
                sapper: role("sapper"),
                cloak: t.number(id, "cloak").unwrap_or(0) * crate::map::TILE,
                lifetime: t.number(id, "lifetime").unwrap_or(0) as u32,
                factions: e.factions.clone(),
                starport_price: t.number(id, "starport_price").unwrap_or(0),
                starport_stock: t.number(id, "starport_stock").unwrap_or(0) as u32,
                starport_restock: (t.number(id, "starport_restock_ticks").unwrap_or(1) as u32).max(1),
                power: if building { num("power")? } else { 0 },
                cost: t.number(id, "cost").unwrap_or(0),
                build_ticks: t.number(id, "build_ticks").unwrap_or(0),
                built_at: None,
                requires: Vec::new(),
                armour: ARMOURS
                    .iter()
                    .position(|a| e.armour.as_deref() == Some(*a))
                    .ok_or(format!("{id} has no armour"))?,
                weapon: None,
                death: None,
                self_destruct: None,
                turn_rate: t.number(id, "turn_rate").unwrap_or(0),
                sight: t.number(id, "sight").unwrap_or(0) * crate::map::TILE,
                vision: t.number(id, "vision").unwrap_or(2) as i32,
                noise: t.number(id, "noise").unwrap_or(0),
                storage: t.number(id, "storage").unwrap_or(0),
            });
        }
        if kinds.len() > u16::MAX as usize {
            return Err("too many kinds".into());
        }
        // What builds each kind and what it requires, as kinds. The rules table has checked they are built buildings.
        let index = |id: &str| kinds.binary_search_by(|k| k.id.as_str().cmp(id)).ok().map(|i| Kind(i as u16));
        let links: Vec<(Option<Kind>, Vec<Kind>)> = kinds
            .iter()
            .map(|k| {
                let e = &t.entities[&k.id];
                (e.built_at.as_deref().and_then(index), e.requires.iter().filter_map(|r| index(r)).collect())
            })
            .collect();
        for (k, (built_at, requires)) in kinds.iter_mut().zip(links) {
            k.built_at = built_at;
            k.requires = requires;
        }
        let mut weapons = Vec::new();
        for (id, w) in &t.weapons {
            let n = |name: &str| t.weapon_number(id, name).ok_or(format!("rules data has no weapon {id}.{name}"));
            weapons.push(WeaponRules {
                id: id.clone(),
                warhead: WARHEADS.iter().position(|h| *h == w.warhead).ok_or(format!("{id}: unknown warhead"))?,
                range: n("range")?,
                min_range: n("min_range")?,
                reload: n("reload")? as u32,
                damage: n("damage")?,
                speed: n("speed")?,
                scatter: n("scatter")?,
                splash: n("splash")?,
                needs_power: n("needs_power")? != 0,
                hits_air: n("hits_air")? != 0,
                hits_ground: n("hits_ground")? != 0,
                beam: n("beam")?,
                converts: n("converts")? as u32,
            });
        }
        let weapon_index =
            |id: &str| weapons.binary_search_by(|w| w.id.as_str().cmp(id)).ok().map(|i| WeaponId(i as u16));
        for k in &mut kinds {
            let e = &t.entities[&k.id];
            k.weapon = e.weapon.as_deref().and_then(weapon_index);
            k.death = e.death.as_deref().and_then(weapon_index);
            k.self_destruct = e.self_destruct.as_deref().and_then(weapon_index);
            if k.sight == 0 {
                k.sight = k.weapon.map_or(0, |w| weapons[w.0 as usize].range);
            }
        }
        let num = |id: &str, name: &str| t.number(id, name).ok_or(format!("rules data has no {id}.{name}"));
        let module = |id: &str, name: &str| t.module_number(id, name).ok_or(format!("rules data has no {id}.{name}"));
        let mut table = [[0; 6]; 6];
        for (w, row) in WARHEADS.iter().zip(&mut table) {
            for (a, cell) in ARMOURS.iter().zip(row.iter_mut()) {
                *cell = module("combat", &format!("{w}_vs_{a}"))?;
            }
        }
        let every = num("resource", "regrow_every_ticks")?;
        let hz = |name: &str| module("hazard", name);
        let hazard = if hz("on")? != 0 && hz("max")? > 0 {
            Some(HazardRules {
                max: hz("max")? as u32,
                first_tick: hz("first_tick")? as u32,
                respawn_ticks: hz("respawn_ticks")? as u32,
                spawn_clearance: hz("spawn_clearance")? as i32,
                speed: hz("speed")?,
                scan_every: (hz("scan_every_ticks")? as u32).max(1),
                scan_range: hz("scan_range")?,
                give_up_range: hz("give_up_range")?,
                wander_range: hz("wander_range")? as i32,
                surface_ticks: (hz("surface_ticks")? as u32).max(1),
                appetite: hz("appetite")? as u32,
                leave_ticks: (hz("leave_ticks")? as u32).max(1),
                mining_noise: hz("mining_noise")?,
                firing_noise: hz("firing_noise")?,
            })
        } else {
            None
        };
        let fg = |name: &str| module("fog", name);
        let fog = if fg("on")? != 0 {
            Some(FogRules {
                hide: fg("hide")? != 0,
                reveal_ticks: fg("reveal_ticks")? as u32,
                start_explored: fg("start_explored")? != 0,
            })
        } else {
            None
        };
        let repair = RepairRules {
            every: (module("repair", "every_ticks")? as u32).max(1),
            step_div: module("repair", "step_div")?.max(1),
            cost_percent: module("repair", "cost_percent")?,
            pad_every: (module("repair", "pad_every_ticks")? as u32).max(1),
            pad_step: module("repair", "pad_step")?.max(1),
        };
        let sell = (module("sell", "on")? != 0).then_some(SellRules {
            refund_percent: module("sell", "refund_percent")?,
            ticks: (module("sell", "ticks")? as u32).max(1),
        });
        let capture = (module("capture", "on")? != 0)
            .then_some(CaptureRules { below_percent: module("capture", "below_percent")?.max(1) });
        let decay = (module("decay", "on")? != 0).then_some(DecayRules {
            every: (module("decay", "every_ticks")? as u32).max(1),
            step_div: module("decay", "step_div")?.max(1),
            floor_percent: module("decay", "floor_percent")?.clamp(0, 100),
        });
        let bl = |name: &str| module("blooms", name);
        let blooms = if bl("on")? != 0 {
            let reseed_min = bl("reseed_min_ticks")? as u32;
            Some(BloomRules {
                radius: bl("radius")? as i32,
                centre: bl("centre")?,
                per_tile: bl("per_tile")?,
                damage: bl("burst_damage")?,
                reseed_min,
                reseed_max: (bl("reseed_max_ticks")? as u32).max(reseed_min),
                reseed_range: bl("reseed_range")? as i32,
                retry: (bl("retry_ticks")? as u32).max(1),
                max_age: (bl("max_age_ticks")? as u32).max(1),
            })
        } else {
            None
        };
        let sp = |name: &str| module("starport", name);
        let starport = if sp("on")? != 0 {
            Some(StarportRules {
                catalogue: (0..kinds.len())
                    .filter(|&i| kinds[i].starport_price > 0 && !kinds[i].building)
                    .map(|i| Kind(i as u16))
                    .collect(),
                drift_every: (sp("drift_every_ticks")? as u32).max(1),
                drift_step: sp("drift_step_percent")?,
                min_percent: sp("min_percent")?,
                max_percent: sp("max_percent")?,
                delivery_ticks: (sp("delivery_ticks")? as u32).max(1),
                unload_every: (sp("unload_every_ticks")? as u32).max(1),
                max_order: (sp("max_order")? as usize).max(1),
            })
        } else {
            None
        };
        let kind_index = |id: &str| kinds.binary_search_by(|k| k.id.as_str().cmp(id)).ok().map(|i| Kind(i as u16));
        let su = |name: &str| module("superpowers", name);
        let superpowers = if su("on")? != 0 && kind_index("palace").is_some() {
            let by_faction = t.modules["superpowers"]
                .by_faction
                .iter()
                .map(|(f, p)| {
                    let power = Superpower::ALL.into_iter().find(|s| s.id() == p);
                    power.map(|power| (f.clone(), power)).ok_or(format!("superpowers.by_faction.{f}: no power {p}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let need = |id: &str| kind_index(id).ok_or(format!("the superpowers module needs the {id} entity"));
            Some(SuperpowerRules {
                by_faction,
                charge: [
                    (su("missile_charge_ticks")? as u32).max(1),
                    (su("guerrillas_charge_ticks")? as u32).max(1),
                    (su("saboteur_charge_ticks")? as u32).max(1),
                ],
                missile_flight: su("missile_flight_ticks")? as u32,
                missile_flight_per_tile: su("missile_flight_ticks_per_tile")? as u32,
                missile_spread: su("missile_spread")?,
                missile_spread_tiles: su("missile_spread_tiles")?.max(1),
                missile_damage: [
                    su("missile_damage_0")?,
                    su("missile_damage_1")?,
                    su("missile_damage_2")?,
                    su("missile_damage_3")?,
                ],
                missile_weapon: weapon_index("missile_strike").ok_or("the superpowers module needs missile_strike")?,
                guerrillas_delay: su("guerrillas_delay_ticks")? as u32,
                guerrillas_count: su("guerrillas_count")? as u32,
                guerrillas_min_range: su("guerrillas_min_range")? as i32,
                guerrillas_max_range: su("guerrillas_max_range")? as i32,
                guerrilla: need("guerrilla")?,
                saboteur: need("saboteur")?,
            })
        } else {
            None
        };
        Ok(Rules {
            kinds,
            regrowth: Regrowth {
                every_ticks: u32::try_from(every)
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or("resource.regrow_every_ticks must be positive")?,
                amount: num("resource", "regrow_amount")?,
            },
            placement: Placement {
                max_gap: module("placement", "max_gap")? as i32,
                rock_only: module("placement", "rock_only")? != 0,
            },
            power: PowerRules { min_factor: module("power", "min_factor")? },
            storage: StorageRules { warn_every: (module("storage", "warn_every_ticks")? as u32).max(1) },
            production: ProductionRules {
                queue_size: module("production", "queue_size")? as usize,
                instant_build: module("production", "instant_build")? != 0,
                starting_credits: module("production", "starting_credits")?,
            },
            movement: MovementRules {
                wait_base: module("movement", "wait_base")? as u32,
                wait_random: module("movement", "wait_random")? as u32,
                max_repath_fails: module("movement", "max_repath_fails")? as u32,
                yield_expires: module("movement", "yield_expires")? as u32,
                close_enough_rings: module("movement", "close_enough_rings")? as i32,
                nodes_local: module("movement", "nodes_local")? as u32,
            },
            air: AirRules {
                cruise_altitude: module("air", "cruise_altitude")?,
                climb: module("air", "climb")?.max(1),
                ferry_min_path: module("air", "ferry_min_path")? as usize,
                pickup_ticks: (module("air", "pickup_ticks")? as u32).max(1),
                drop_ticks: (module("air", "drop_ticks")? as u32).max(1),
                drop_rings: module("air", "drop_rings")? as i32,
                drop_retry_ticks: (module("air", "drop_retry_ticks")? as u32).max(1),
                fall_damage_percent: module("air", "fall_damage_percent")?,
            },
            combat: CombatRules {
                table,
                own_splash_percent: module("combat", "own_splash_percent")?,
                scan_every: module("combat", "scan_every_ticks")? as u32,
                self_destruct_ticks: (module("combat", "self_destruct_ticks")? as u32).max(1),
            },
            weapons,
            hazard,
            fog,
            repair,
            sell,
            capture,
            decay,
            blooms,
            starport,
            superpowers,
            hash: t.hash(),
        })
    }

    pub fn weapon(&self, w: WeaponId) -> &WeaponRules {
        &self.weapons[w.0 as usize]
    }

    /// The weapon with this generic id.
    pub fn weapon_id(&self, id: &str) -> Option<WeaponId> {
        self.weapons.binary_search_by(|w| w.id.as_str().cmp(id)).ok().map(|i| WeaponId(i as u16))
    }

    pub fn kind(&self, k: Kind) -> &KindRules {
        &self.kinds[k.0 as usize]
    }

    /// The kind with this generic id, if the simulation has it.
    pub fn kind_id(&self, id: &str) -> Option<Kind> {
        self.kinds.binary_search_by(|k| k.id.as_str().cmp(id)).ok().map(|i| Kind(i as u16))
    }

    /// Every kind's generic id, in kind order, for the state hash.
    pub fn kind_ids(&self) -> Vec<String> {
        self.kinds.iter().map(|k| k.id.clone()).collect()
    }
}

impl Default for Rules {
    /// The engine's own rules data, untuned.
    fn default() -> Self {
        Rules::from_table(&RulesTable::builtin()).expect("data/rules matches the simulation")
    }
}
