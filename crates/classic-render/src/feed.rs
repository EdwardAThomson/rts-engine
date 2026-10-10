//! The message feed: short lines at the top left of the world for what the local player should know about
//! (a building ready, low power, the base under attack, a unit lost). Design: `plans/rts/ui.md`, section 10.
//!
//! The feed only reads the game's events, like the sound board, and never changes the state. Which event shows which
//! message id is decided here; the words for each id come from `data/ui/messages.json`, and a setting pack can reword
//! any of them in its own `ui/messages.json`, or give its faction's advisor words of its own in `lines.json`
//! (`lines`), which win. The feed also keeps the subtitle: what the local player's units last said back to them.

use std::collections::{BTreeMap, VecDeque};

use classic_data::json::{self, Value};
use classic_sim::units::TICKS_PER_SECOND;
use classic_sim::world::{CaptureError, Event, IdleReason, MoveEnd};
use classic_sim::{Game, Kind, ProduceError, StarportError, SuperpowerError};

use crate::lines::{Lines, Moment, Speech};
use crate::platform::Files;

const MESSAGES: &str = include_str!("../../../data/ui/messages.json");
/// Where a pack rewords messages, in its folder.
pub const MESSAGES_FILE: &str = "ui/messages.json";

/// How long a line stays, in ticks.
pub const LIFE: u32 = 8 * TICKS_PER_SECOND;
/// The most lines shown at once; older ones go first.
pub const MAX_LINES: usize = 5;
/// The same line again within this many ticks refreshes the old one instead of adding another.
const REPEAT: u32 = 3 * TICKS_PER_SECOND;
/// Attack warnings come at most this often, in ticks, for buildings and for units each.
const ATTACK_EVERY: u32 = 20 * TICKS_PER_SECOND;
/// How long a unit's reply stays on screen, in ticks.
pub const REPLY_LIFE: u32 = 2 * TICKS_PER_SECOND;
/// Replies come at most this often, in ticks (a quarter of a second), however fast the player clicks.
pub const REPLY_EVERY: u32 = TICKS_PER_SECOND / 4;
/// A hazard appearing is news at most this often, in ticks.
const HAZARD_EVERY: u32 = 30 * TICKS_PER_SECOND;
/// A full store is news at most this often, in ticks, whether it just filled or a harvest was lost to it.
const STORAGE_EVERY: u32 = 30 * TICKS_PER_SECOND;
/// Armed enemy units the local player can see this close to one of their buildings, in tiles, are coming for the
/// base...
pub const WAVE_RANGE: i32 = 12;
/// ...when there are at least this many of them,
pub const WAVE_SIZE: usize = 2;
/// and the warning comes at most this often, in ticks. The feed looks once a second.
const WAVE_EVERY: u32 = 60 * TICKS_PER_SECOND;

/// How a line reads: news, good news, a warning or a loss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The message id, from `data/ui/messages.json`.
    pub id: &'static str,
    pub text: String,
    pub tone: Tone,
    /// When it was last said.
    pub tick: u32,
}

pub struct Feed {
    /// The player at this screen; the feed only speaks to them.
    pub local: u32,
    pub lines: VecDeque<Line>,
    /// What the local player's units last said, while it shows.
    pub reply: Option<Line>,
    words: BTreeMap<String, String>,
    /// The advisor's and the units' own lines, for the local player's faction.
    pub speech: Lines,
    /// The variant last said, by message id or reply set, so no line comes twice in a row.
    last: BTreeMap<String, usize>,
    /// Lines said in a faction's own words since the caller last took them, for the sound board to voice.
    pub spoken: Vec<Speech>,
    /// The feed's own pick, stirred at every choice; never the game's generator.
    stir: u32,
    names: BTreeMap<String, String>,
    /// How many of the game's events have been read.
    seen: usize,
    short: bool,
    /// Whether the local player had radar at the last look; unknown before the first.
    radar: Option<bool>,
    building_attacked: Option<u32>,
    unit_attacked: Option<u32>,
    harvester_attacked: Option<u32>,
    hazard_sighted: Option<u32>,
    storage_full: Option<u32>,
    enemy_wave: Option<u32>,
    /// How many enemy units were near the base at the last look.
    near: usize,
    /// Whether the end of the game has been said.
    over: bool,
    /// The engine's own words and replies, to tell which lines the pack left in them (`Speech::engine`).
    engine_words: BTreeMap<String, String>,
    engine_acks: BTreeMap<(String, Moment), Vec<String>>,
    pub warnings: Vec<String>,
}

/// The engine's words for every message id.
pub fn default_words() -> BTreeMap<String, String> {
    let v = json::parse(MESSAGES).expect("data/ui/messages.json parses");
    words(&v).expect("data/ui/messages.json has a `messages` object of strings")
}

fn words(v: &Value) -> Option<BTreeMap<String, String>> {
    v.get("messages")?.as_object()?.iter().map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect()
}

impl Feed {
    /// A feed for `local`, in the words of the pack's `ui/messages.json` in `pack` where it has one, naming things as
    /// `names` does (generic id to the pack's name), with `speech` (`Lines::load`) for its advisor and units.
    pub fn new(pack: &Files, names: BTreeMap<String, String>, local: u32, speech: Lines) -> Feed {
        let engine_words = default_words();
        let mut table = engine_words.clone();
        let mut warnings = Vec::new();
        if let Ok(text) = pack.read_text(MESSAGES_FILE) {
            match json::parse(&text).ok().as_ref().and_then(words) {
                Some(own) => {
                    for (id, t) in own {
                        if let Some(w) = table.get_mut(&id) {
                            *w = t;
                        } else {
                            warnings.push(format!("{}: no message `{id}`", pack.name(MESSAGES_FILE)));
                        }
                    }
                }
                None => warnings.push(format!("{}: needs a `messages` object of strings", pack.name(MESSAGES_FILE))),
            }
        }
        warnings.extend(speech.warnings.iter().cloned());
        Feed {
            local,
            lines: VecDeque::new(),
            reply: None,
            words: table,
            speech,
            last: BTreeMap::new(),
            spoken: Vec::new(),
            stir: 1,
            names,
            seen: 0,
            short: false,
            radar: None,
            building_attacked: None,
            unit_attacked: None,
            harvester_attacked: None,
            hazard_sighted: None,
            storage_full: None,
            enemy_wave: None,
            near: 0,
            over: false,
            engine_words,
            engine_acks: Lines::engine().acks,
            warnings,
        }
    }

    fn name(&self, game: &Game, kind: Kind) -> String {
        let id = &game.rules.kind(kind).id;
        self.names.get(id).cloned().unwrap_or_else(|| id.clone())
    }

    fn owner(game: &Game, entity: u32) -> Option<u32> {
        game.state.entity(entity).map(|e| e.owner)
    }

    /// Say message `id` now, about `kind` if it names one: in the advisor's own words where it has some, else the
    /// feed's.
    pub fn say(&mut self, game: &Game, id: &'static str, kind: Option<Kind>, tone: Tone) {
        let name = kind.map(|k| self.name(game, k)).unwrap_or_default();
        let tick = game.state.tick;
        let said: Vec<String> = match self.speech.advisor.get(id) {
            Some(own) => own.iter().map(|w| w.replace("{name}", &name)).collect(),
            None => vec![self.words.get(id).map_or_else(|| id.to_string(), |w| w.replace("{name}", &name))],
        };
        // Saying it again soon refreshes the line already there, whichever of its variants that was.
        if let Some(old) =
            self.lines.iter_mut().find(|l| said.contains(&l.text) && tick.saturating_sub(l.tick) < REPEAT)
        {
            old.tick = tick;
            return;
        }
        let at = self.turn(id, said.len());
        let engine = !self.speech.advisor.contains_key(id) && self.words.get(id) == self.engine_words.get(id);
        self.voice(Speech { who: "advisor".into(), key: id.to_string(), variant: at, engine });
        let text = said[at].clone();
        self.lines.push_back(Line { id, text, tone, tick });
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }

    /// Keep `s` for the sound board; a few at most, since only the latest matter when nobody takes them.
    fn voice(&mut self, s: Speech) {
        self.spoken.push(s);
        if self.spoken.len() > 8 {
            self.spoken.remove(0);
        }
    }

    /// Which of `n` variants to say for `key`: any but the one said last time. The feed stirs its own number to
    /// choose, so the game's generator is never touched.
    fn turn(&mut self, key: &str, n: usize) -> usize {
        self.stir = self.stir.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let r = (self.stir >> 16) as usize;
        let at = match (self.last.get(key), n) {
            (_, 0 | 1) => 0,
            (None, _) => r % n,
            (Some(&last), _) => (last + 1 + r % (n - 1)) % n,
        };
        self.last.insert(key.to_string(), at);
        at
    }

    /// The local player's `units` answer an order or being selected: one of them says a line from its voice set
    /// ([`crate::lines::voice_of`]) as a subtitle. Others' units, buildings and clicks coming faster
    /// than `REPLY_EVERY` say nothing.
    pub fn reply(&mut self, game: &Game, moment: Moment, units: &[u32]) {
        let mine: Vec<_> = units
            .iter()
            .filter_map(|&id| game.state.entity(id))
            .filter(|e| e.owner == self.local && !game.rules.kind(e.kind).building)
            .collect();
        if mine.is_empty() {
            return;
        }
        let tick = game.state.tick;
        if self.reply.as_ref().is_some_and(|r| tick.saturating_sub(r.tick) < REPLY_EVERY) {
            return;
        }
        let voice = crate::lines::voice_of(game, &mine, &self.speech, moment);
        let Some(said) = self.speech.acks.get(&(voice.clone(), moment)).cloned() else { return };
        let at = self.turn(&format!("{voice}.{}", moment.id()), said.len());
        let engine = self.engine_acks.get(&(voice.clone(), moment)) == Some(&said);
        self.voice(Speech { who: voice, key: moment.id().to_string(), variant: at, engine });
        let text = said[at].clone();
        self.reply = Some(Line { id: moment.id(), text, tone: Tone::Info, tick });
    }

    /// Read the events since the last call and say what the local player should hear about; drop old lines.
    pub fn after_step(&mut self, game: &Game) {
        // The player clears old events now and then; start over when that happens.
        if game.events.len() < self.seen {
            self.seen = 0;
        }
        let local = self.local;
        for i in self.seen..game.events.len() {
            let ev = &game.events[i];
            let tick = game.state.tick;
            match *ev {
                Event::BuildingReady { player, kind, .. } if player == local => {
                    self.say(game, "building_ready", Some(kind), Tone::Good)
                }
                Event::UnitBuilt { entity, kind, .. } if Self::owner(game, entity) == Some(local) => {
                    self.say(game, "unit_ready", Some(kind), Tone::Good)
                }
                Event::PlacementRejected { player, kind, .. } if player == local => {
                    self.say(game, "cannot_place", Some(kind), Tone::Warn)
                }
                Event::ProductionRejected { player, kind, reason, .. } if player == local => match reason {
                    ProduceError::QueueFull => self.say(game, "queue_full", None, Tone::Warn),
                    ProduceError::Requires { .. } => self.say(game, "needs_building", Some(kind), Tone::Warn),
                    // The rail never offers these; a script might.
                    ProduceError::NotBuildable
                    | ProduceError::NoFactory
                    | ProduceError::NotQueued
                    | ProduceError::Faction => {}
                },
                Event::ProductionPaused { factory, kind, .. } if Self::owner(game, factory) == Some(local) => {
                    self.say(game, "no_credits", Some(kind), Tone::Warn)
                }
                Event::Hit { target, .. } => {
                    let Some(e) = game.state.entity(target).filter(|e| e.owner == local) else { continue };
                    let k = game.rules.kind(e.kind);
                    let (last, id, kind) = if k.building {
                        (&mut self.building_attacked, "base_attacked", None)
                    } else if k.harvester.is_some() {
                        (&mut self.harvester_attacked, "harvester_attacked", Some(e.kind))
                    } else {
                        (&mut self.unit_attacked, "units_attacked", None)
                    };
                    if last.is_none_or(|t| tick.saturating_sub(t) >= ATTACK_EVERY) {
                        *last = Some(tick);
                        self.say(game, id, kind, Tone::Bad);
                    }
                }
                Event::Destroyed { kind, owner, .. } if owner == local => {
                    let id = if game.rules.kind(kind).building { "building_lost" } else { "unit_lost" };
                    self.say(game, id, Some(kind), Tone::Bad);
                }
                Event::HazardAte { kind, owner, .. } if owner == local => {
                    self.say(game, "hazard_ate", Some(kind), Tone::Bad)
                }
                Event::HazardSpawned { .. } => {
                    if self.hazard_sighted.is_none_or(|t| tick.saturating_sub(t) >= HAZARD_EVERY) {
                        self.hazard_sighted = Some(tick);
                        self.say(game, "hazard_sighted", None, Tone::Warn);
                    }
                }
                // The store filled, or a harvest was lost for want of room: build silos.
                Event::StorageFull { player, .. } | Event::CreditsLost { player, .. } if player == local => {
                    if self.storage_full.is_none_or(|t| tick.saturating_sub(t) >= STORAGE_EVERY) {
                        self.storage_full = Some(tick);
                        self.say(game, "storage_full", None, Tone::Warn);
                    }
                }
                Event::StarportOrderPlaced { player, .. } if player == local => {
                    self.say(game, "starport_ordered", None, Tone::Good)
                }
                Event::StarportRefused { player, kind, reason, .. } if player == local => match reason {
                    StarportError::Funds => self.say(game, "starport_funds", None, Tone::Warn),
                    StarportError::OutOfStock => self.say(game, "starport_out_of_stock", kind, Tone::Warn),
                    // The sidebar never offers these; a script might.
                    StarportError::NotSold
                    | StarportError::NoStarport
                    | StarportError::Busy
                    | StarportError::Full
                    | StarportError::NotInOrder => {}
                },
                Event::StarportOrderRefunded { player, .. } if player == local => {
                    self.say(game, "starport_refunded", None, Tone::Bad)
                }
                Event::SuperpowerReady { player, .. } if player == local => {
                    self.say(game, "superpower_ready", None, Tone::Good)
                }
                Event::SuperpowerRefused { player, reason: SuperpowerError::NotReady, .. } if player == local => {
                    self.say(game, "superpower_charging", None, Tone::Warn)
                }
                // Everyone hears a missile go up but the one who sent it.
                Event::MissileLaunched { player, .. } if player != local => {
                    self.say(game, "missile_warning", None, Tone::Bad)
                }
                Event::GuerrillasArrived { player, units, .. } if player == local && units > 0 => {
                    self.say(game, "guerrillas_arrived", None, Tone::Good)
                }
                Event::SaboteurArrived { player, .. } if player == local => {
                    self.say(game, "saboteur_ready", None, Tone::Good)
                }
                // A local unit gave up on its way: it can't get there.
                Event::MoveEnded { unit, reason: MoveEnd::Blocked, .. } if Self::owner(game, unit) == Some(local) => {
                    self.reply(game, Moment::Cant, &[unit])
                }
                Event::HarvesterIdle { unit, reason, .. } => {
                    let Some(e) = game.state.entity(unit).filter(|e| e.owner == local) else { continue };
                    let id = match reason {
                        IdleReason::NoResource => "harvester_no_resource",
                        IdleReason::NoRefinery => "harvester_no_refinery",
                    };
                    self.say(game, id, Some(e.kind), Tone::Warn);
                }
                Event::RepairStarted { entity, owner, .. } if owner == local => {
                    let kind = game.state.entity(entity).map(|e| e.kind);
                    self.say(game, "repairing", kind, Tone::Info)
                }
                Event::RepairStopped { entity, owner, whole: true, .. } if owner == local => {
                    let kind = game.state.entity(entity).map(|e| e.kind);
                    self.say(game, "repaired", kind, Tone::Good)
                }
                Event::UnitRepaired { unit, owner, .. } if owner == local => {
                    let kind = game.state.entity(unit).map(|e| e.kind);
                    self.say(game, "repaired", kind, Tone::Good)
                }
                Event::BuildingSold { kind, owner, .. } if owner == local => {
                    self.say(game, "building_sold", Some(kind), Tone::Info)
                }
                Event::Captured { kind, to, .. } if to == local => {
                    self.say(game, "building_captured", Some(kind), Tone::Good)
                }
                Event::Captured { kind, from, .. } if from == local => {
                    self.say(game, "building_taken", Some(kind), Tone::Bad)
                }
                Event::CaptureRefused { player, target, reason: CaptureError::TooHealthy, .. } if player == local => {
                    let kind = game.state.entity(target).map(|e| e.kind);
                    self.say(game, "cannot_capture", kind, Tone::Warn)
                }
                // Every other event is the sound's and the scene's business, or another player's.
                Event::BuildingReady { .. }
                | Event::UnitBuilt { .. }
                | Event::PlacementRejected { .. }
                | Event::ProductionRejected { .. }
                | Event::ProductionPaused { .. }
                | Event::PowerChanged { .. }
                | Event::Destroyed { .. }
                | Event::Delivered { .. }
                | Event::StorageFull { .. }
                | Event::CreditsLost { .. }
                | Event::BeamFired { .. }
                | Event::Converted { .. }
                | Event::Reverted { .. }
                | Event::SelfDestructStarted { .. }
                | Event::SapperDetonated { .. }
                | Event::Expired { .. }
                | Event::MarketPricesChanged { .. }
                | Event::SuperpowerReady { .. }
                | Event::SuperpowerRefused { .. }
                | Event::MissileLaunched { .. }
                | Event::MissileImpact { .. }
                | Event::GuerrillasArrived { .. }
                | Event::SaboteurArrived { .. }
                | Event::StarportRefused { .. }
                | Event::StarportOrderPlaced { .. }
                | Event::SupplyShipLanded { .. }
                | Event::StarportOrderRefunded { .. }
                | Event::SupplyShipLeft { .. }
                | Event::Regrowth { .. }
                | Event::CarrierPickup { .. }
                | Event::CarrierDropoff { .. }
                | Event::CarrierLostCargo { .. }
                | Event::BuildingPlaced { .. }
                | Event::ProductionQueued { .. }
                | Event::ProductionCancelled { .. }
                | Event::ProductionHeld { .. }
                | Event::ProductionResumed { .. }
                | Event::PrimarySet { .. }
                | Event::SlabLaid { .. }
                | Event::BloomSeeded { .. }
                | Event::BloomBurst { .. }
                | Event::BloomHurt { .. }
                | Event::Decayed { .. }
                | Event::TargetAcquired { .. }
                | Event::Fired { .. }
                | Event::ProjectileSpawned { .. }
                | Event::ProjectileHit { .. }
                | Event::MoveEnded { .. }
                | Event::UnitYielded { .. }
                | Event::UnitStuck { .. }
                | Event::HazardSurfaced { .. }
                | Event::HazardAte { .. }
                | Event::HazardLeft { .. }
                | Event::RepairStarted { .. }
                | Event::RepairStopped { .. }
                | Event::UnitRepaired { .. }
                | Event::SellStarted { .. }
                | Event::BuildingSold { .. }
                | Event::Captured { .. }
                | Event::CaptureRefused { .. } => {}
            }
        }
        self.seen = game.events.len();
        // Read power from the state rather than from its events, so a change made outside a tick counts too.
        let short = game.power(local).is_short();
        if short != self.short {
            let (id, tone) = if short { ("low_power", Tone::Bad) } else { ("power_restored", Tone::Good) };
            self.say(game, id, None, tone);
            self.short = short;
        }
        // Radar coming and going (the `radar` module), but not the lack of it at the start.
        let radar = game.radar(local);
        if self.radar.is_some_and(|was| was != radar) {
            let (id, tone) = if radar { ("radar_online", Tone::Good) } else { ("radar_offline", Tone::Warn) };
            self.say(game, id, None, tone);
        }
        self.radar = Some(radar);
        let now = game.state.tick;
        if now.is_multiple_of(TICKS_PER_SECOND) {
            let near = enemies_near_base(game, local);
            if near >= WAVE_SIZE
                && self.near < WAVE_SIZE
                && self.enemy_wave.is_none_or(|t| now.saturating_sub(t) >= WAVE_EVERY)
            {
                self.enemy_wave = Some(now);
                self.say(game, "enemy_wave", None, Tone::Bad);
            }
            self.near = near;
        }
        self.lines.retain(|l| now.saturating_sub(l.tick) < LIFE);
        if self.reply.as_ref().is_some_and(|r| now.saturating_sub(r.tick) >= REPLY_LIFE) {
            self.reply = None;
        }
    }

    /// The game is over for the local player, won or lost: the advisor says so, once.
    pub fn over(&mut self, game: &Game, won: bool) {
        if !self.over {
            self.over = true;
            let (id, tone) = if won { ("game_won", Tone::Good) } else { ("game_lost", Tone::Bad) };
            self.say(game, id, None, tone);
        }
    }

    /// The words for every message id, after the pack's.
    pub fn words(&self) -> &BTreeMap<String, String> {
        &self.words
    }
}

/// How many of other players' armed units that `player` can see stand within `WAVE_RANGE` tiles of one of their
/// buildings. Harvesters, carriers and other unarmed units don't count, nor do units hidden by fog: in AI games the
/// computer's waves come in strung out, so few of a wave are near at once. In AI games on `skirmish-01` and
/// `mirror-01` (12 seeds each, normal and hard, the generic pack's fog), 2 units within 12 tiles warned of 62 of 95
/// waves by 15 seconds after half of the wave was within 10 tiles, with 226 warnings in all; the old 4 units of any
/// kind within 10 tiles, seen or not, warned of 55 with 134 warnings, and 3 units within 12 tiles of 51 with 189.
/// Most waves missed stay out of the player's sight until they strike.
pub fn enemies_near_base(game: &Game, player: u32) -> usize {
    let mine: Vec<_> = game
        .state
        .entities
        .iter()
        .filter(|e| e.owner == player && game.rules.kind(e.kind).building)
        .map(|e| (e.tile(), game.rules.kind(e.kind)))
        .collect();
    game.state
        .entities
        .iter()
        .filter(|e| e.owner != player && !game.rules.kind(e.kind).building && game.rules.kind(e.kind).weapon.is_some())
        .filter(|e| game.visible(player, e.id))
        .filter(|e| {
            let t = e.tile();
            // The distance to the building's footprint, in tiles, counting diagonals as one.
            mine.iter().any(|(b, k)| {
                let dx = (b.x - t.x).max(t.x - (b.x + k.width - 1)).max(0);
                let dy = (b.y - t.y).max(t.y - (b.y + k.height - 1)).max(0);
                dx.max(dy) <= WAVE_RANGE
            })
        })
        .count()
}

/// Whether any of `units` could walk to `tile`: a move order anywhere else gets the `cant` reply. Units standing in
/// the way don't count, since they move.
pub fn reachable(game: &Game, units: &[u32], (x, y): (i32, i32)) -> bool {
    units.iter().filter_map(|&id| game.state.entity(id)).any(|e| {
        let t = e.tile();
        game.pathfinder.connected((t.x, t.y), (x, y))
    })
}
