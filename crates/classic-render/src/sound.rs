//! The Classic engine's sound: turns the game's events into sounds for the mixer. Like the scene, it only reads the
//! game; nothing it does feeds back into the state.
//!
//! The engine's `data/audio/` says which events play which sound id (`events.json`) and how each id is mixed
//! (`sounds.json`). A setting pack supplies only the files for those ids, in its `audio/sounds.json`; any id it
//! leaves out falls back to the generic pack's placeholder. Input sounds (a click on the build rail, an order)
//! belong to the interface, which plays them by id with [`SoundBoard::ui`].

use std::collections::BTreeMap;
use std::path::Path;

use classic_data::json::{self, Value};
use classic_sim::map::TILE;
use classic_sim::{Event, Game};

use crate::platform::audio::{Bus, ClipId, Mixer, Sound, db};
use crate::platform::wav;

const SOUNDS: &str = include_str!("../../../data/audio/sounds.json");
const EVENTS: &str = include_str!("../../../data/audio/events.json");

/// Every event the simulation emits, by `Event::name`. `facts` matches on the event exhaustively, so a new event
/// stops the build there; add its name here and to `data/audio/events.json` (a rule, or `silent`) at the same time.
pub const EVENT_NAMES: [&str; 21] = [
    "harvester_idle",
    "delivered",
    "regrowth",
    "building_placed",
    "placement_rejected",
    "power_changed",
    "production_queued",
    "production_rejected",
    "production_paused",
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
    local: bool,
    power: Option<PowerTurn>,
}

/// The engine's sound tables, before any files are loaded.
#[derive(Clone, Debug)]
pub struct Tables {
    pub defs: Vec<SoundDef>,
    rules: Vec<Rule>,
    pub silent: Vec<String>,
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
                clips: Vec::new(),
            });
        }
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
            let warhead = text("warhead");
            if let Some(w) = &warhead
                && !classic_data::WARHEADS.contains(&w.as_str())
            {
                return Err(format!("{at}: unknown warhead {w}"));
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
                local: r.get("local").and_then(Value::as_bool).unwrap_or(false),
                power,
            });
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
        Ok(Tables { defs, rules, silent })
    }

    /// Ids that some event plays.
    pub fn played_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.rules.iter().map(|r| self.defs[r.sound].id.as_str()).collect();
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
    at: Option<(i64, i64)>,
    shortfall: Option<i64>,
    power_player: Option<u32>,
}

fn facts(ev: &Event, game: &Game) -> Facts {
    let owner_of = |id: u32| game.state.entity(id).map(|e| e.owner);
    let at_of = |id: u32| game.state.entity(id).map(|e| (e.x, e.y));
    let is_building = |k| game.rules.kind(k).building;
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
        | Event::ProductionRejected { player, .. }
        | Event::BuildingReady { player, .. } => Facts { owner: Some(player), ..none },
        Event::PowerChanged { player, shortfall, .. } => {
            Facts { owner: Some(player), shortfall: Some(shortfall), power_player: Some(player), ..none }
        }
        Event::UnitBuilt { entity, kind, .. } => {
            Facts { owner: owner_of(entity), building: Some(is_building(kind)), at: at_of(entity), ..none }
        }
        Event::Fired { unit, weapon, .. } => {
            Facts { owner: owner_of(unit), weapon: Some(weapon), at: at_of(unit), ..none }
        }
        Event::ProjectileHit { weapon, x, y, .. } => Facts { weapon: Some(weapon), at: Some((x, y)), ..none },
        Event::Destroyed { kind, owner, x, y, .. } => {
            Facts { owner: Some(owner), building: Some(is_building(kind)), at: Some((x, y)), ..none }
        }
        Event::Hit { target, .. } => Facts { owner: owner_of(target), at: at_of(target), ..none },
        Event::ProductionQueued { factory, .. }
        | Event::ProductionPaused { factory, .. }
        | Event::ProductionCancelled { factory, .. } => Facts { owner: owner_of(factory), ..none },
        Event::ProjectileSpawned { x, y, .. } => Facts { at: Some((x, y)), ..none },
        Event::HarvesterIdle { unit, .. }
        | Event::TargetAcquired { unit, .. }
        | Event::MoveEnded { unit, .. }
        | Event::UnitYielded { unit, .. }
        | Event::UnitStuck { unit, .. } => Facts { owner: owner_of(unit), at: at_of(unit), ..none },
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
    pub warnings: Vec<String>,
}

impl SoundBoard {
    /// Load the sounds for `pack`: the generic pack's files first, then the pack's own over them. Files that
    /// can't be read become warnings, and their ids play silence.
    pub fn load(pack_dir: &Path, generic_dir: &Path, local: u32, seed: u64, mixer: &mut Mixer) -> SoundBoard {
        let mut tables = Tables::builtin();
        let mut warnings = Vec::new();
        let mut dirs = vec![generic_dir];
        if pack_dir.canonicalize().ok() != generic_dir.canonicalize().ok() {
            dirs.push(pack_dir);
        }
        for dir in dirs {
            let index = dir.join("audio/sounds.json");
            let Ok(text) = std::fs::read_to_string(&index) else { continue };
            let Ok(v) = json::parse(&text) else {
                warnings.push(format!("{}: not valid JSON", index.display()));
                continue;
            };
            for (id, entry) in v.get("sounds").and_then(Value::as_object).unwrap_or(&[]) {
                let Some(def) = tables.defs.iter_mut().find(|d| &d.id == id) else {
                    warnings.push(format!("{}: {id} is not a sound id the engine plays", index.display()));
                    continue;
                };
                let mut clips = Vec::new();
                for f in entry.get("files").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_str) {
                    let path = dir.join(f);
                    match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| wav::decode(&b)) {
                        Ok(clip) => clips.push(mixer.add_clip(clip)),
                        Err(e) => warnings.push(format!("{}: {e}", path.display())),
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
            let rule = self.tables.rules.iter().find(|r| {
                r.event == name
                    && (!r.local || f.owner == Some(self.local))
                    && r.building.is_none_or(|b| f.building == Some(b))
                    && r.power.is_none_or(|p| turn == Some(p))
                    && r.weapon.as_ref().is_none_or(|w| weapon.is_some_and(|x| &x.id == w))
                    && r.warhead.as_ref().is_none_or(|w| weapon.is_some_and(|x| classic_data::WARHEADS[x.warhead] == w))
            });
            let Some(def) = rule.map(|r| r.sound) else { continue };
            let (gain, pan) = match (self.tables.defs[def].spatial, f.at) {
                (true, Some((x, y))) => listener.place(x as f32, y as f32),
                _ => (1.0, 0.0),
            };
            cues.extend(self.cue(def, gain, pan));
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
