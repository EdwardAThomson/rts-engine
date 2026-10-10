//! The Classic engine's sound: turns the game's events into sounds for the mixer. Like the scene, it only reads the
//! game; nothing it does feeds back into the state.
//!
//! The engine's `data/audio/` says which events play which sound id (`events.json`) and how each id is mixed
//! (`sounds.json`). A setting pack supplies only the files for those ids, in its `audio/sounds.json`; any id it
//! leaves out falls back to the generic pack's placeholder. Input sounds (a click on the build rail, an order)
//! belong to the interface, which plays them by id with [`SoundBoard::ui`].
//!
//! Spoken lines are the pack's too: its `audio/voices.json` gives the files for its `lines.json`, by faction, then
//! `advisor` by line id or a unit voice set by moment, the n-th file speaking the n-th line. The feed says which line
//! it showed ([`Speech`]) and [`SoundBoard::speak`] plays the matching take on the voice bus, one line of each kind at
//! a time and no reply over the advisor, holding the sound effects down while it lasts ([`SoundBoard::duck`]).
//! A pack's voices replace the generic pack's whole, so no line is spoken by a voice from another cast. The generic
//! pack's voices speak the engine's own words, so they only play a line the pack left in those words. An advisor
//! line that comes while another is speaking waits in a short queue, most important first, and is dropped if it
//! waits too long ([`SoundBoard::next_line`]); the priorities and limits are `voices` in `data/audio/sounds.json`.
//!
//! **Loops** (engines, a harvester at work) follow what units are doing rather than events: `loops` in
//! `data/audio/events.json` says which loop plays for a unit moving, flying, mining or unloading, and
//! [`SoundBoard::update_loops`] keeps one loop per sound id going while any unit the local player can see needs it,
//! as loud as the nearest one and a little louder for a crowd, panned towards them.

use std::collections::BTreeMap;
use std::path::Path;

use classic_data::json::{self, Value};
use classic_sim::map::TILE;
use classic_sim::{Event, Game};

use crate::lines::Speech;
use crate::platform::Files;
use crate::platform::audio::{Bus, ClipId, LoopId, Mixer, Sound, db};
use crate::platform::wav;

/// A pack's sound index, from the pack's folder: which files each sound id plays.
pub const SOUND_INDEX: &str = "audio/sounds.json";

/// A pack's voice index, from the pack's folder: which files speak each of its lines.
pub const VOICE_INDEX: &str = "audio/voices.json";

/// The mixer keys of the advisor's lines and of the units' replies, well clear of the sound ids' keys. Each plays
/// one at a time; a reply waits out the advisor (see [`SoundBoard::speak`]).
pub const ADVISOR_KEY: u32 = 0xFFFF_0000;
pub const REPLY_KEY: u32 = 0xFFFF_0001;

/// How long a loop fades in, and out once nothing needs it, in seconds.
const LOOP_FADE_IN: f32 = 0.15;
const LOOP_FADE_OUT: f32 = 0.4;
/// Ticks a loop keeps going after the last unit stopped needing it, so a tank pausing to turn doesn't cut out.
const LOOP_LINGER: u32 = 5;

/// Every file the voice index `index` names, for the browser build.
pub fn voice_files_named(index: &str) -> Vec<String> {
    let Ok(v) = json::parse(index) else { return Vec::new() };
    let mut files = Vec::new();
    for (_, who) in v.get("voices").and_then(Value::as_object).unwrap_or(&[]) {
        for (_, keys) in who.as_object().unwrap_or(&[]) {
            for (_, list) in keys.as_object().unwrap_or(&[]) {
                files.extend(list.as_array().unwrap_or(&[]).iter().filter_map(Value::as_str).map(String::from));
            }
        }
    }
    files
}

/// Every file the sound index `index` names, so the browser build knows what to fetch.
pub fn files_named(index: &str) -> Vec<String> {
    let Ok(v) = json::parse(index) else { return Vec::new() };
    let mut files = Vec::new();
    for (_, entry) in v.get("sounds").and_then(Value::as_object).unwrap_or(&[]) {
        files.extend(
            entry
                .get("files")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::as_str)
                .map(String::from),
        );
    }
    files
}

const SOUNDS: &str = include_str!("../../../data/audio/sounds.json");
const EVENTS: &str = include_str!("../../../data/audio/events.json");

/// Every event the simulation emits, by `Event::name`. `facts` matches on the event exhaustively, so a new event
/// stops the build there; add its name here and to `data/audio/events.json` (a rule, or `silent`) at the same time.
pub const EVENT_NAMES: [&str; 58] = [
    "harvester_idle",
    "delivered",
    "regrowth",
    "building_placed",
    "placement_rejected",
    "power_changed",
    "production_queued",
    "production_rejected",
    "production_paused",
    "production_held",
    "production_resumed",
    "primary_set",
    "production_cancelled",
    "building_ready",
    "unit_built",
    "target_acquired",
    "fired",
    "projectile_spawned",
    "projectile_hit",
    "hit",
    "destroyed",
    "move_ended",
    "unit_yielded",
    "unit_stuck",
    "hazard_spawned",
    "hazard_surfaced",
    "hazard_ate",
    "hazard_left",
    "carrier_pickup",
    "carrier_dropoff",
    "carrier_lost_cargo",
    "storage_full",
    "credits_lost",
    "repair_started",
    "repair_stopped",
    "unit_repaired",
    "sell_started",
    "building_sold",
    "captured",
    "capture_refused",
    "beam_fired",
    "converted",
    "reverted",
    "self_destruct_started",
    "sapper_detonated",
    "expired",
    "market_prices_changed",
    "starport_refused",
    "starport_order_placed",
    "supply_ship_landed",
    "starport_order_refunded",
    "supply_ship_left",
    "superpower_ready",
    "superpower_refused",
    "power_missile_launched",
    "power_missile_impact",
    "guerrillas_arrived",
    "saboteur_arrived",
];

/// How one sound id is mixed, from `data/audio/sounds.json`, and the takes the pack gave it.
#[derive(Clone, Debug)]
pub struct SoundDef {
    pub id: String,
    pub bus: Bus,
    pub gain_db: i64,
    pub priority: i64,
    pub max_instances: i64,
    pub spatial: bool,
    pub jitter_percent: i64,
    /// Played round and round while units need it (`loops` in `events.json`), not by an event.
    pub looping: bool,
    pub clips: Vec<ClipId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerTurn {
    Short,
    Restored,
}

/// One line of `data/audio/events.json`.
#[derive(Clone, Debug)]
struct Rule {
    event: String,
    sound: usize,
    warhead: Option<String>,
    weapon: Option<String>,
    building: Option<bool>,
    /// The armour class of what was destroyed (`classic_data::ARMOURS`).
    armour: Option<String>,
    local: bool,
    power: Option<PowerTurn>,
    /// The generic id of the factory a unit came out of.
    factory: Option<String>,
    /// Plays as well as the rule that follows, rather than instead of it.
    also: bool,
}

/// What a unit is doing that a loop can follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Doing {
    /// On the ground and on the move.
    Moving,
    /// Off the ground (an aircraft, moving or not).
    Flying,
    /// A harvester taking up the resource.
    Mining,
    /// A harvester emptying at a refinery.
    Unloading,
}

const DOINGS: [(&str, Doing); 4] =
    [("moving", Doing::Moving), ("flying", Doing::Flying), ("mining", Doing::Mining), ("unloading", Doing::Unloading)];

/// One line of `loops` in `data/audio/events.json`.
#[derive(Clone, Debug)]
struct LoopRule {
    doing: Doing,
    armour: Option<String>,
    sound: usize,
}

/// How spoken lines are mixed, from `data/audio/sounds.json` `voices`.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceMix {
    /// Level and priority of the advisor's lines and of the units' replies.
    pub advisor: (i64, i64),
    pub unit: (i64, i64),
    /// How far the sound effects and music drop while someone speaks.
    pub duck_db: i64,
    /// Advisor lines that may wait while one speaks, and how long one may wait before it is dropped.
    pub queue: usize,
    pub stale_ms: i64,
    /// Each advisor line's place in the queue, higher first; lines not listed take `default`.
    pub line_priority: BTreeMap<String, i64>,
    pub default_priority: i64,
}

impl VoiceMix {
    pub fn priority(&self, line: &str) -> i64 {
        self.line_priority.get(line).copied().unwrap_or(self.default_priority)
    }
}

/// The engine's sound tables, before any files are loaded.
#[derive(Clone, Debug)]
pub struct Tables {
    pub defs: Vec<SoundDef>,
    rules: Vec<Rule>,
    loops: Vec<LoopRule>,
    pub silent: Vec<String>,
    pub voices: VoiceMix,
}

fn field<'a>(v: &'a Value, key: &str, at: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{at}: no `{key}`"))
}

impl Tables {
    /// The engine's own tables, built in.
    pub fn builtin() -> Tables {
        Tables::parse(SOUNDS, EVENTS).expect("data/audio is valid")
    }

    pub fn parse(sounds: &str, events: &str) -> Result<Tables, String> {
        let sounds = json::parse(sounds).map_err(|e| format!("sounds.json: {e:?}"))?;
        let mut defs = Vec::new();
        for (id, v) in
            field(&sounds, "sounds", "sounds.json")?.as_object().ok_or("sounds.json: `sounds` not an object")?
        {
            let int = |k: &str| field(v, k, id)?.as_int().ok_or_else(|| format!("{id}.{k}: not a whole number"));
            let bus =
                field(v, "bus", id)?.as_str().and_then(Bus::from_id).ok_or_else(|| format!("{id}: unknown bus"))?;
            defs.push(SoundDef {
                id: id.clone(),
                bus,
                gain_db: int("gain_db")?,
                priority: int("priority")?,
                max_instances: int("max_instances")?,
                spatial: field(v, "spatial", id)?
                    .as_bool()
                    .ok_or_else(|| format!("{id}.spatial: not true or false"))?,
                jitter_percent: int("jitter_percent")?,
                looping: v.get("loop").and_then(Value::as_bool).unwrap_or(false),
                clips: Vec::new(),
            });
        }
        let v = field(&sounds, "voices", "sounds.json")?;
        let num = |o: &Value, k: &str, at: &str| {
            field(o, k, at)?.as_int().ok_or_else(|| format!("voices.{at}.{k}: not a whole number"))
        };
        let pair = |k: &str| -> Result<(i64, i64), String> {
            let o = field(v, k, "voices")?;
            Ok((num(o, "gain_db", k)?, num(o, "priority", k)?))
        };
        let queue = field(v, "queue", "voices")?;
        let priorities =
            field(v, "line_priority", "voices")?.as_object().ok_or("voices.line_priority: not an object")?;
        let known = crate::lines::advisor_ids();
        let mut line_priority = BTreeMap::new();
        let mut default_priority = None;
        for (id, p) in priorities {
            let p = p.as_int().ok_or_else(|| format!("voices.line_priority.{id}: not a whole number"))?;
            if id == "default" {
                default_priority = Some(p);
            } else if known.contains(id) {
                line_priority.insert(id.clone(), p);
            } else if id != "about" {
                return Err(format!("voices.line_priority: no advisor line `{id}`"));
            }
        }
        let voices = VoiceMix {
            advisor: pair("advisor")?,
            unit: pair("unit")?,
            duck_db: num(v, "duck_db", "voices")?,
            queue: num(queue, "size", "queue")?.max(0) as usize,
            stale_ms: num(queue, "stale_ms", "queue")?,
            line_priority,
            default_priority: default_priority.ok_or("voices.line_priority: no `default`")?,
        };
        let events = json::parse(events).map_err(|e| format!("events.json: {e:?}"))?;
        let mut rules = Vec::new();
        for (i, r) in field(&events, "rules", "events.json")?
            .as_array()
            .ok_or("events.json: `rules` not a list")?
            .iter()
            .enumerate()
        {
            let at = format!("events.json rule {i}");
            let text = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
            let event = text("event").ok_or_else(|| format!("{at}: no `event`"))?;
            if !EVENT_NAMES.contains(&event.as_str()) {
                return Err(format!("{at}: no event is called {event}"));
            }
            let sound = text("sound").ok_or_else(|| format!("{at}: no `sound`"))?;
            let sound =
                defs.iter().position(|d| d.id == sound).ok_or_else(|| format!("{at}: unknown sound {sound}"))?;
            if defs[sound].looping {
                return Err(format!("{at}: {} is a loop, played from `loops`", defs[sound].id));
            }
            let warhead = text("warhead");
            if let Some(w) = &warhead
                && !classic_data::WARHEADS.contains(&w.as_str())
            {
                return Err(format!("{at}: unknown warhead {w}"));
            }
            let armour = text("armour");
            if let Some(a) = &armour
                && !classic_data::ARMOURS.contains(&a.as_str())
            {
                return Err(format!("{at}: unknown armour {a}"));
            }
            let power = match text("power").as_deref() {
                None => None,
                Some("short") => Some(PowerTurn::Short),
                Some("restored") => Some(PowerTurn::Restored),
                Some(p) => return Err(format!("{at}: power is `short` or `restored`, not {p}")),
            };
            rules.push(Rule {
                event,
                sound,
                warhead,
                weapon: text("weapon"),
                building: r.get("building").and_then(Value::as_bool),
                armour,
                local: r.get("local").and_then(Value::as_bool).unwrap_or(false),
                power,
                factory: text("factory"),
                also: r.get("also").and_then(Value::as_bool).unwrap_or(false),
            });
        }
        let mut loops = Vec::new();
        for (i, r) in field(&events, "loops", "events.json")?
            .as_array()
            .ok_or("events.json: `loops` not a list")?
            .iter()
            .enumerate()
        {
            let at = format!("events.json loop {i}");
            let text = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
            let doing = text("while").ok_or_else(|| format!("{at}: no `while`"))?;
            let doing =
                DOINGS.iter().find(|d| d.0 == doing).map(|d| d.1).ok_or_else(|| {
                    format!("{at}: `while` is one of {}, not {doing}", DOINGS.map(|d| d.0).join(", "))
                })?;
            let sound = text("sound").ok_or_else(|| format!("{at}: no `sound`"))?;
            let sound =
                defs.iter().position(|d| d.id == sound).ok_or_else(|| format!("{at}: unknown sound {sound}"))?;
            if !defs[sound].looping {
                return Err(format!("{at}: {} is not a loop (`\"loop\": true` in sounds.json)", defs[sound].id));
            }
            let armour = text("armour");
            if let Some(a) = &armour
                && !classic_data::ARMOURS.contains(&a.as_str())
            {
                return Err(format!("{at}: unknown armour {a}"));
            }
            loops.push(LoopRule { doing, armour, sound });
        }
        let silent: Vec<String> = field(&events, "silent", "events.json")?
            .as_array()
            .ok_or("events.json: `silent` not a list")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        for name in EVENT_NAMES {
            let ruled = rules.iter().any(|r| r.event == name);
            let quiet = silent.iter().any(|s| s == name);
            if ruled == quiet {
                return Err(format!("events.json: {name} must be in a rule or in `silent`, not both or neither"));
            }
        }
        if let Some(s) = silent.iter().find(|s| !EVENT_NAMES.contains(&s.as_str())) {
            return Err(format!("events.json: silent lists {s}, which is no event"));
        }
        Ok(Tables { defs, rules, loops, silent, voices })
    }

    /// Ids that some event or loop plays.
    pub fn played_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.rules.iter().map(|r| self.defs[r.sound].id.as_str()).collect();
        ids.extend(self.loops.iter().map(|r| self.defs[r.sound].id.as_str()));
        ids.sort();
        ids.dedup();
        ids
    }
}

/// The part of the screen the player is looking at, in sub-tile units: sounds there play at full level, and fade
/// out to nothing two view widths beyond it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Listener {
    /// From the renderer's camera: `px` is screen pixels per sub-tile unit at zoom 1.
    pub fn from_camera(cam: &crate::Camera, screen: (f32, f32), px: f32) -> Listener {
        let (x0, y0) = cam.to_world(0.0, 0.0);
        let (x1, y1) = cam.to_world(screen.0, screen.1);
        Listener { x0: x0 / px, y0: y0 / px, x1: x1 / px, y1: y1 / px }
    }

    /// Gain and pan for a sound at (`x`, `y`).
    pub fn place(&self, x: f32, y: f32) -> (f32, f32) {
        let w = (self.x1 - self.x0).max(1.0);
        let h = (self.y1 - self.y0).max(1.0);
        let dx = (self.x0 - x).max(x - self.x1).max(0.0);
        let dy = (self.y0 - y).max(y - self.y1).max(0.0);
        let out = (dx / w).max(dy / h);
        let gain = (1.0 - out / 2.0).max(0.0);
        let pan = ((x - (self.x0 + self.x1) / 2.0) / (w / 2.0)).clamp(-1.0, 1.0);
        (gain, pan)
    }
}

/// A sound the board chose to play: the id, for logs and tests, and the request for the mixer.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub id: String,
    pub sound: Sound,
}

/// What a rule can ask of an event.
#[derive(Default)]
struct Facts {
    owner: Option<u32>,
    weapon: Option<classic_sim::WeaponId>,
    building: Option<bool>,
    armour: Option<usize>,
    at: Option<(i64, i64)>,
    shortfall: Option<i64>,
    power_player: Option<u32>,
    /// The factory a unit came out of.
    factory: Option<classic_sim::Kind>,
}

fn facts(ev: &Event, game: &Game) -> Facts {
    let owner_of = |id: u32| game.state.entity(id).map(|e| e.owner);
    let at_of = |id: u32| game.state.entity(id).map(|e| (e.x, e.y));
    let is_building = |k| game.rules.kind(k).building;
    // A building's footprint centre.
    let centre_of = |id: u32| {
        game.state.entity(id).map(|e| {
            let (t, k) = (e.tile(), game.rules.kind(e.kind));
            (t.x as i64 * TILE + k.width as i64 * TILE / 2, t.y as i64 * TILE + k.height as i64 * TILE / 2)
        })
    };
    let none = Facts::default();
    match *ev {
        Event::Delivered { player, unit, .. } => Facts { owner: Some(player), at: at_of(unit), ..none },
        Event::BuildingPlaced { kind, owner, x, y, .. } => {
            let k = game.rules.kind(kind);
            let centre = |t: i32, n: i32| t as i64 * TILE + n as i64 * TILE / 2;
            Facts {
                owner: Some(owner),
                building: Some(true),
                at: Some((centre(x, k.width), centre(y, k.height))),
                ..none
            }
        }
        Event::PlacementRejected { player, .. }
        | Event::StorageFull { player, .. }
        | Event::CreditsLost { player, .. }
        | Event::ProductionRejected { player, .. }
        | Event::BuildingReady { player, .. } => Facts { owner: Some(player), ..none },
        Event::PowerChanged { player, shortfall, .. } => {
            Facts { owner: Some(player), shortfall: Some(shortfall), power_player: Some(player), ..none }
        }
        Event::UnitBuilt { entity, kind, factory, .. } => Facts {
            owner: owner_of(entity),
            building: Some(is_building(kind)),
            at: at_of(entity),
            factory: game.state.entity(factory).map(|f| f.kind),
            ..none
        },
        Event::Fired { unit, weapon, .. } => {
            Facts { owner: owner_of(unit), weapon: Some(weapon), at: at_of(unit), ..none }
        }
        Event::ProjectileHit { weapon, x, y, .. } => Facts { weapon: Some(weapon), at: Some((x, y)), ..none },
        Event::Destroyed { kind, owner, x, y, .. } => Facts {
            owner: Some(owner),
            building: Some(is_building(kind)),
            armour: Some(game.rules.kind(kind).armour),
            at: Some((x, y)),
            ..none
        },
        Event::Hit { target, .. } => Facts { owner: owner_of(target), at: at_of(target), ..none },
        Event::ProductionQueued { factory, .. }
        | Event::ProductionPaused { factory, .. }
        | Event::ProductionHeld { factory, .. }
        | Event::ProductionResumed { factory, .. }
        | Event::ProductionCancelled { factory, .. } => Facts { owner: owner_of(factory), ..none },
        Event::ProjectileSpawned { x, y, .. } => Facts { at: Some((x, y)), ..none },
        Event::HarvesterIdle { unit, .. }
        | Event::TargetAcquired { unit, .. }
        | Event::MoveEnded { unit, .. }
        | Event::UnitYielded { unit, .. }
        | Event::UnitStuck { unit, .. } => Facts { owner: owner_of(unit), at: at_of(unit), ..none },
        Event::HazardAte { owner, x, y, .. } | Event::Expired { owner, x, y, .. } => {
            Facts { owner: Some(owner), at: Some((x, y)), ..none }
        }
        Event::BeamFired { unit, weapon, x1, y1, .. } => {
            Facts { owner: owner_of(unit), weapon: Some(weapon), at: Some((x1, y1)), ..none }
        }
        Event::Converted { unit, to, .. } | Event::Reverted { unit, to, .. } => {
            Facts { owner: Some(to), at: at_of(unit), ..none }
        }
        Event::SelfDestructStarted { unit, .. } => Facts { owner: owner_of(unit), at: at_of(unit), ..none },
        Event::MarketPricesChanged { .. } => none,
        Event::SuperpowerReady { player, .. } | Event::SuperpowerRefused { player, .. } => {
            Facts { owner: Some(player), ..none }
        }
        // Heard at the palace it leaves, and where it lands.
        Event::MissileLaunched { player, palace, .. } => Facts { owner: Some(player), at: centre_of(palace), ..none },
        Event::MissileImpact { player, x, y, .. } | Event::GuerrillasArrived { player, x, y, .. } => {
            Facts { owner: Some(player), at: Some((x as i64 * TILE + TILE / 2, y as i64 * TILE + TILE / 2)), ..none }
        }
        Event::SaboteurArrived { player, unit, .. } => Facts { owner: Some(player), at: at_of(unit), ..none },
        Event::StarportRefused { player, .. }
        | Event::StarportOrderPlaced { player, .. }
        | Event::StarportOrderRefunded { player, .. } => Facts { owner: Some(player), ..none },
        Event::SupplyShipLanded { ship, .. } | Event::SupplyShipLeft { ship, .. } => {
            Facts { owner: owner_of(ship), at: at_of(ship), ..none }
        }
        // The sapper is gone by the time this is read; the building it hit is where it happened.
        Event::SapperDetonated { unit, target, .. } => Facts { owner: owner_of(unit), at: at_of(target), ..none },
        Event::CarrierPickup { unit, .. }
        | Event::CarrierDropoff { unit, .. }
        | Event::CarrierLostCargo { unit, .. } => Facts { owner: owner_of(unit), at: at_of(unit), ..none },
        Event::HazardSpawned { x, y, .. } | Event::HazardSurfaced { x, y, .. } | Event::HazardLeft { x, y, .. } => {
            Facts { at: Some((x, y)), ..none }
        }
        Event::RepairStarted { entity, owner, .. } | Event::RepairStopped { entity, owner, .. } => {
            Facts { owner: Some(owner), building: Some(true), at: centre_of(entity), ..none }
        }
        Event::UnitRepaired { unit, owner, .. } => Facts { owner: Some(owner), at: at_of(unit), ..none },
        Event::PrimarySet { entity, owner, .. } => {
            Facts { owner: Some(owner), building: Some(true), at: centre_of(entity), ..none }
        }
        Event::SellStarted { entity, owner, .. } => {
            Facts { owner: Some(owner), building: Some(true), at: centre_of(entity), ..none }
        }
        Event::BuildingSold { owner, .. } => Facts { owner: Some(owner), building: Some(true), ..none },
        Event::Captured { entity, to, .. } => {
            Facts { owner: Some(to), building: Some(true), at: centre_of(entity), ..none }
        }
        Event::CaptureRefused { player, .. } => Facts { owner: Some(player), ..none },
        Event::Regrowth { x, y, .. } => {
            Facts { at: Some((x as i64 * TILE + TILE / 2, y as i64 * TILE + TILE / 2)), ..none }
        }
    }
}

/// Plays the game's events as sounds for one player's screen.
pub struct SoundBoard {
    pub tables: Tables,
    /// The player at this screen: `local` rules play only for them.
    pub local: u32,
    /// How many of the game's events have been read.
    seen: usize,
    /// Each player's shortfall as of the last power event, for `short` and `restored`.
    short: BTreeMap<u32, bool>,
    /// For picking a take and its speed; the board's own, never the game's.
    rng: u64,
    /// Spoken takes by faction id, who speaks (`advisor` or a unit voice set) and line id or moment.
    pub voices: BTreeMap<(String, String, String), Vec<ClipId>>,
    /// Whether `voices` are the first pack's (the generic pack's), which speak only the engine's own words.
    pub engine_voices: bool,
    /// The sound effects' and music's levels before a voice held them down, while it does.
    ducked: Option<(f32, f32)>,
    /// Advisor lines waiting for the one speaking to finish: the cue, its priority and when it came, in seconds.
    queue: Vec<(Cue, i64, f64)>,
    /// The loops playing, by sound id: the mixer's loop, and the last tick some unit needed it.
    playing_loops: BTreeMap<usize, (LoopId, u32)>,
    /// Where each unit was last tick, to tell which are moving.
    was_at: BTreeMap<u32, (i64, i64)>,
    pub warnings: Vec<String>,
}

impl SoundBoard {
    /// Load the sounds for `pack`: the generic pack's files first, then the pack's own over them. Files that
    /// can't be read become warnings, and their ids play silence.
    pub fn load(pack_dir: &Path, generic_dir: &Path, local: u32, seed: u64, mixer: &mut Mixer) -> SoundBoard {
        let mut packs = vec![Files::Dir(generic_dir.to_path_buf())];
        if pack_dir.canonicalize().ok() != generic_dir.canonicalize().ok() {
            packs.push(Files::Dir(pack_dir.to_path_buf()));
        }
        SoundBoard::from_files(&packs, local, seed, mixer)
    }

    /// Load the sounds from each pack's files in turn, later packs over earlier ones (the browser build fetches
    /// them first; see [`files_named`]).
    pub fn from_files(packs: &[Files], local: u32, seed: u64, mixer: &mut Mixer) -> SoundBoard {
        let mut tables = Tables::builtin();
        let mut warnings = Vec::new();
        let mut voices = BTreeMap::new();
        // Only the last pack with voices speaks.
        let voiced = packs.iter().rposition(|f| f.read_text(VOICE_INDEX).is_ok());
        for (n, files) in packs.iter().enumerate() {
            if let Some(text) = files.read_text(VOICE_INDEX).ok().filter(|_| voiced == Some(n)) {
                match json::parse(&text).ok().as_ref().and_then(|v| v.get("voices")?.as_object().map(|o| o.to_vec())) {
                    Some(factions) => {
                        for (faction, who) in &factions {
                            for (w, keys) in who.as_object().unwrap_or(&[]) {
                                for (key, list) in keys.as_object().unwrap_or(&[]) {
                                    let mut clips = Vec::new();
                                    for f in list.as_array().unwrap_or(&[]).iter().filter_map(Value::as_str) {
                                        match files.read(f).and_then(|b| {
                                            wav::decode(&b).map_err(|e| format!("{}: {e}", files.name(f)))
                                        }) {
                                            Ok(clip) => clips.push(mixer.add_clip(clip)),
                                            Err(e) => warnings.push(e),
                                        }
                                    }
                                    voices.insert((faction.clone(), w.clone(), key.clone()), clips);
                                }
                            }
                        }
                    }
                    None => warnings.push(format!("{}: needs a `voices` object", files.name(VOICE_INDEX))),
                }
            }
            let Ok(text) = files.read_text(SOUND_INDEX) else { continue };
            let Ok(v) = json::parse(&text) else {
                warnings.push(format!("{}: not valid JSON", files.name(SOUND_INDEX)));
                continue;
            };
            for (id, entry) in v.get("sounds").and_then(Value::as_object).unwrap_or(&[]) {
                let Some(def) = tables.defs.iter_mut().find(|d| &d.id == id) else {
                    warnings.push(format!("{}: {id} is not a sound id the engine plays", files.name(SOUND_INDEX)));
                    continue;
                };
                let mut clips = Vec::new();
                for f in entry.get("files").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_str) {
                    match files.read(f).and_then(|b| wav::decode(&b).map_err(|e| format!("{}: {e}", files.name(f)))) {
                        Ok(clip) => clips.push(mixer.add_clip(clip)),
                        Err(e) => warnings.push(e),
                    }
                }
                if !clips.is_empty() {
                    def.clips = clips;
                }
            }
        }
        SoundBoard {
            tables,
            local,
            seen: 0,
            short: BTreeMap::new(),
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            voices,
            engine_voices: voiced == Some(0),
            ducked: None,
            queue: Vec::new(),
            playing_loops: BTreeMap::new(),
            was_at: BTreeMap::new(),
            warnings,
        }
    }

    fn roll(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    /// A cue for sound `def` at full level, centred, or `None` if the pack has no file for it.
    fn cue(&mut self, def: usize, gain: f32, pan: f32) -> Option<Cue> {
        let d = &self.tables.defs[def];
        if d.clips.is_empty() || gain <= 0.01 {
            return None;
        }
        let (n, jitter) = (d.clips.len() as u64, d.jitter_percent);
        let pick = self.roll();
        let wobble = self.roll();
        let d = &self.tables.defs[def];
        let speed = 1.0 + ((wobble % (2 * jitter as u64 + 1)) as f32 - jitter as f32) / 100.0;
        Some(Cue {
            id: d.id.clone(),
            sound: Sound {
                clip: d.clips[(pick % n) as usize],
                key: def as u32,
                bus: d.bus,
                gain: gain * db(d.gain_db as f32),
                pan,
                speed,
                priority: d.priority as i32,
                max_instances: d.max_instances.max(1) as u32,
            },
        })
    }

    /// An interface sound by id (`ui_select`, `ui_order`, ...), at full level and centred.
    pub fn ui(&mut self, id: &str) -> Option<Cue> {
        let def = self.tables.defs.iter().position(|d| d.id == id)?;
        self.cue(def, 1.0, 0.0)
    }

    /// The take that speaks `s` for `faction`, on the voice bus, to play now; `None` when the pack has no voice for
    /// it, the voices are the generic pack's and `s` isn't in the engine's words, or a unit would talk over the
    /// advisor, who `mixer` is still playing. An advisor line joins the queue and the next line due comes back
    /// (see [`SoundBoard::next_line`]). `now` is any clock in seconds, the same one each call.
    pub fn speak(&mut self, faction: &str, s: &Speech, mixer: &Mixer, now: f64) -> Option<Cue> {
        let advisor = s.who == "advisor";
        if (!advisor && mixer.playing_key(ADVISOR_KEY) > 0) || (self.engine_voices && !s.engine) {
            return None;
        }
        let Some(cue) = self.voice(faction, s) else {
            return if advisor { self.next_line(mixer, now) } else { None };
        };
        if !advisor {
            return Some(cue);
        }
        let priority = self.tables.voices.priority(&s.key);
        // Highest first; a stable sort keeps lines of the same priority in the order they came.
        self.queue.push((cue, priority, now));
        self.queue.sort_by_key(|q| std::cmp::Reverse(q.1));
        self.next_line(mixer, now)
    }

    /// The advisor's next line, once the one speaking in `mixer` has finished: the most important waiting, unless it
    /// has waited longer than `voices.queue.stale_ms`, when it is dropped (stale news is worse than none). While one
    /// speaks, at most `voices.queue.size` wait, and the least important beyond that are dropped. Call once a frame,
    /// and play what comes back.
    pub fn next_line(&mut self, mixer: &Mixer, now: f64) -> Option<Cue> {
        let stale = self.tables.voices.stale_ms as f64 / 1000.0;
        self.queue.retain(|q| now - q.2 <= stale);
        let line = if mixer.playing_key(ADVISOR_KEY) == 0 && !self.queue.is_empty() {
            Some(self.queue.remove(0).0)
        } else {
            None
        };
        self.queue.truncate(self.tables.voices.queue);
        line
    }

    /// Advisor lines waiting, most important first, by cue id.
    pub fn waiting(&self) -> Vec<&str> {
        self.queue.iter().map(|q| q.0.id.as_str()).collect()
    }

    /// The cue that speaks `s` for `faction`, if the pack voiced it.
    fn voice(&self, faction: &str, s: &Speech) -> Option<Cue> {
        let advisor = s.who == "advisor";
        let key = (faction.to_string(), s.who.to_string(), s.key.clone());
        let clip = *self.voices.get(&key)?.get(s.variant)?;
        let (gain_db, priority) = if advisor { self.tables.voices.advisor } else { self.tables.voices.unit };
        Some(Cue {
            id: format!("voice {faction} {}.{}", s.who, s.key),
            sound: Sound {
                clip,
                key: if advisor { ADVISOR_KEY } else { REPLY_KEY },
                bus: Bus::Voice,
                gain: db(gain_db as f32),
                pan: 0.0,
                speed: 1.0,
                priority: priority as i32,
                max_instances: 1,
            },
        })
    }

    /// Set the buses' levels (the player's volume settings), keeping the effects and music ducked if a voice is
    /// speaking.
    pub fn set_levels(&mut self, mixer: &mut Mixer, gain: [f32; 4]) {
        let (sfx, music) = (Bus::Sfx as usize, Bus::Music as usize);
        mixer.bus_gain = gain;
        if self.ducked.is_some() {
            self.ducked = Some((gain[sfx], gain[music]));
            let duck = db(self.tables.voices.duck_db as f32);
            mixer.bus_gain[sfx] *= duck;
            mixer.bus_gain[music] *= duck;
        }
    }

    /// Hold the sound effects and music down while a line is spoken, and bring them back after. Call once a frame.
    pub fn duck(&mut self, mixer: &mut Mixer) {
        let (sfx, music) = (Bus::Sfx as usize, Bus::Music as usize);
        let duck = db(self.tables.voices.duck_db as f32);
        match (mixer.playing_key(ADVISOR_KEY) + mixer.playing_key(REPLY_KEY) > 0, self.ducked) {
            (true, None) => {
                self.ducked = Some((mixer.bus_gain[sfx], mixer.bus_gain[music]));
                mixer.bus_gain[sfx] *= duck;
                mixer.bus_gain[music] *= duck;
            }
            (false, Some((s, m))) => {
                mixer.bus_gain[sfx] = s;
                mixer.bus_gain[music] = m;
                self.ducked = None;
            }
            _ => {}
        }
    }

    /// Keep the loops in `mixer` in step with what the units the local player can see are doing, heard from
    /// `listener`. Call once a game tick.
    pub fn update_loops(&mut self, game: &Game, listener: &Listener, mixer: &mut Mixer) {
        let tick = game.state.tick;
        // Per loop sound: the loudest unit's gain, the gain-weighted pan, and how many units need it.
        let mut wanted: BTreeMap<usize, (f32, f32, f32, u32)> = BTreeMap::new();
        let mut was_at = BTreeMap::new();
        for e in &game.state.entities {
            let k = game.rules.kind(e.kind);
            if k.building {
                continue;
            }
            was_at.insert(e.id, (e.x, e.y));
            if e.carried_by.is_some() || !game.visible(self.local, e.id) {
                continue;
            }
            let moved = self.was_at.get(&e.id).is_some_and(|&p| p != (e.x, e.y));
            let doing = |d: Doing| match d {
                Doing::Flying => e.altitude > 0,
                Doing::Moving => e.altitude == 0 && (moved || !e.path.is_empty()),
                Doing::Mining => e.task == Some(classic_sim::Task::Mining),
                Doing::Unloading => e.task == Some(classic_sim::Task::Unloading),
            };
            let Some(rule) = self
                .tables
                .loops
                .iter()
                .find(|r| doing(r.doing) && r.armour.as_ref().is_none_or(|a| classic_data::ARMOURS[k.armour] == a))
            else {
                continue;
            };
            let (gain, pan) = listener.place(e.x as f32, e.y as f32);
            if gain <= 0.01 {
                continue;
            }
            let w = wanted.entry(rule.sound).or_insert((0.0, 0.0, 0.0, 0));
            *w = (w.0.max(gain), w.1 + pan * gain, w.2 + gain, w.3 + 1);
        }
        self.was_at = was_at;
        for (&def, &(gain, pan_sum, weight, count)) in &wanted {
            let d = &self.tables.defs[def];
            // A little louder for a crowd: up to half as loud again.
            let gain = gain * (1.0 + 0.1 * (count - 1) as f32).min(1.5) * db(d.gain_db as f32);
            let pan = pan_sum / weight.max(1e-6);
            match self.playing_loops.get_mut(&def) {
                Some((id, last)) if mixer.looping(*id) => {
                    mixer.set_loop(*id, gain, pan);
                    *last = tick;
                }
                _ => {
                    let Some(&clip) = d.clips.first() else { continue };
                    let sound = Sound {
                        clip,
                        key: def as u32,
                        bus: d.bus,
                        gain,
                        pan,
                        speed: 1.0,
                        priority: d.priority as i32,
                        max_instances: 1,
                    };
                    match mixer.start_loop(sound, LOOP_FADE_IN) {
                        Some(id) => {
                            self.playing_loops.insert(def, (id, tick));
                        }
                        None => {
                            self.playing_loops.remove(&def);
                        }
                    }
                }
            }
        }
        self.playing_loops.retain(|def, (id, last)| {
            let keep = wanted.contains_key(def) || tick.saturating_sub(*last) < LOOP_LINGER;
            if !keep {
                mixer.stop_loop(*id, LOOP_FADE_OUT);
            }
            keep
        });
    }

    /// Fade every loop out: the game is paused or over.
    pub fn stop_loops(&mut self, mixer: &mut Mixer) {
        for (_, (id, _)) in std::mem::take(&mut self.playing_loops) {
            mixer.stop_loop(id, LOOP_FADE_OUT);
        }
        self.was_at.clear();
    }

    /// The loops playing, by sound id.
    pub fn loops_playing(&self) -> Vec<&str> {
        self.playing_loops.keys().map(|&d| self.tables.defs[d].id.as_str()).collect()
    }

    /// The sounds for every event since the last call, heard from `listener`.
    pub fn after_step(&mut self, game: &Game, listener: &Listener) -> Vec<Cue> {
        // The player clears old events now and then; start over when that happens.
        if game.events.len() < self.seen {
            self.seen = 0;
        }
        let mut cues = Vec::new();
        for i in self.seen..game.events.len() {
            let ev = &game.events[i];
            let name = ev.name();
            let f = facts(ev, game);
            let turn = f.power_player.and_then(|p| {
                let now = f.shortfall.unwrap_or(0) > 0;
                let was = self.short.insert(p, now).unwrap_or(false);
                match (was, now) {
                    (false, true) => Some(PowerTurn::Short),
                    (true, false) => Some(PowerTurn::Restored),
                    _ => None,
                }
            });
            let weapon = f.weapon.map(|w| game.rules.weapon(w));
            let factory = f.factory.map(|k| game.rules.kind(k).id.as_str());
            let holds = |r: &Rule| {
                r.event == name
                    && (!r.local || f.owner == Some(self.local))
                    && r.building.is_none_or(|b| f.building == Some(b))
                    && r.armour.as_ref().is_none_or(|a| f.armour.is_some_and(|x| classic_data::ARMOURS[x] == a))
                    && r.power.is_none_or(|p| turn == Some(p))
                    && r.weapon.as_ref().is_none_or(|w| weapon.is_some_and(|x| &x.id == w))
                    && r.warhead.as_ref().is_none_or(|w| weapon.is_some_and(|x| classic_data::WARHEADS[x.warhead] == w))
                    && r.factory.as_ref().is_none_or(|k| factory == Some(k.as_str()))
            };
            // Every `also` rule that holds before the first plain one plays, then that plain one.
            let mut defs = Vec::new();
            for r in self.tables.rules.iter().filter(|r| holds(r)) {
                defs.push(r.sound);
                if !r.also {
                    break;
                }
            }
            for def in defs {
                // Under fog of war the local player hears what happens out in the world only where they can see it.
                let tile = |v: i64| v.div_euclid(classic_sim::map::TILE) as i32;
                if let (true, Some((x, y))) = (self.tables.defs[def].spatial, f.at)
                    && game.tile_view(self.local, tile(x), tile(y)) != classic_sim::TileView::Visible
                {
                    continue;
                }
                let (gain, pan) = match (self.tables.defs[def].spatial, f.at) {
                    (true, Some((x, y))) => listener.place(x as f32, y as f32),
                    _ => (1.0, 0.0),
                };
                cues.extend(self.cue(def, gain, pan));
            }
        }
        self.seen = game.events.len();
        cues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_tables_cover_every_event() {
        let t = Tables::builtin();
        assert!(!t.played_ids().is_empty());
    }

    #[test]
    fn a_forgotten_event_is_refused() {
        let events = EVENTS.replace("\"unit_stuck\"", "\"unit_stuck_nope\"");
        assert!(Tables::parse(SOUNDS, &events).unwrap_err().contains("unit_stuck"));
    }

    #[test]
    fn placing_follows_the_view() {
        let l = Listener { x0: 0.0, y0: 0.0, x1: 1000.0, y1: 500.0 };
        assert_eq!(l.place(500.0, 250.0), (1.0, 0.0));
        assert_eq!(l.place(0.0, 250.0), (1.0, -1.0));
        assert_eq!(l.place(2000.0, 250.0).0, 0.5);
        assert_eq!(l.place(3000.0, 250.0), (0.0, 1.0));
        assert_eq!(l.place(500.0, -500.0).0, 0.5);
    }
}
