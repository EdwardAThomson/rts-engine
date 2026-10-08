//! The message feed: short lines at the top left of the world for what the local player should know about
//! (a building ready, low power, the base under attack, a unit lost). Design: `plans/rts/ui.md`, section 10.
//!
//! The feed only reads the game's events, like the sound board, and never changes the state. Which event shows which
//! message id is decided here; the words for each id come from `data/ui/messages.json`, and a setting pack can reword
//! any of them in its own `ui/messages.json`, or give its faction's advisor words of its own in `lines.json`
//! (`lines`), which win. The feed also keeps the subtitle: what the local player's units last said back to them.

use std::collections::{BTreeMap, VecDeque};

use classic_data::ARMOURS;
use classic_data::json::{self, Value};
use classic_sim::units::TICKS_PER_SECOND;
use classic_sim::world::{Event, IdleReason};
use classic_sim::{Game, Kind, ProduceError};

use crate::lines::{Lines, Moment, VOICES};
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
    /// The feed's own pick, stirred at every choice; never the game's generator.
    stir: u32,
    names: BTreeMap<String, String>,
    /// How many of the game's events have been read.
    seen: usize,
    short: bool,
    building_attacked: Option<u32>,
    unit_attacked: Option<u32>,
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
        let mut table = default_words();
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
            stir: 1,
            names,
            seen: 0,
            short: false,
            building_attacked: None,
            unit_attacked: None,
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
        let text = said[self.turn(id, said.len())].clone();
        self.lines.push_back(Line { id, text, tone, tick });
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
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
    /// (infantry when they all are, else vehicle) as a subtitle. Others' units, buildings and clicks coming faster
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
        let infantry = ARMOURS.iter().position(|&a| a == "infantry");
        let voice =
            if mine.iter().all(|e| Some(game.rules.kind(e.kind).armour) == infantry) { VOICES[0] } else { VOICES[1] };
        let Some(said) = self.speech.acks.get(&(voice, moment)).cloned() else { return };
        let at = self.turn(&format!("{voice}.{}", moment.id()), said.len());
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
                    ProduceError::NotBuildable | ProduceError::NoFactory | ProduceError::NotQueued => {}
                },
                Event::ProductionPaused { factory, kind, .. } if Self::owner(game, factory) == Some(local) => {
                    self.say(game, "no_credits", Some(kind), Tone::Warn)
                }
                Event::Hit { target, .. } => {
                    let Some(e) = game.state.entity(target).filter(|e| e.owner == local) else { continue };
                    let building = game.rules.kind(e.kind).building;
                    let last = if building { &mut self.building_attacked } else { &mut self.unit_attacked };
                    if last.is_none_or(|t| tick.saturating_sub(t) >= ATTACK_EVERY) {
                        *last = Some(tick);
                        let id = if building { "base_attacked" } else { "units_attacked" };
                        self.say(game, id, None, Tone::Bad);
                    }
                }
                Event::Destroyed { kind, owner, .. } if owner == local => {
                    let id = if game.rules.kind(kind).building { "building_lost" } else { "unit_lost" };
                    self.say(game, id, Some(kind), Tone::Bad);
                }
                Event::HarvesterIdle { unit, reason, .. } => {
                    let Some(e) = game.state.entity(unit).filter(|e| e.owner == local) else { continue };
                    let id = match reason {
                        IdleReason::NoResource => "harvester_no_resource",
                        IdleReason::NoRefinery => "harvester_no_refinery",
                    };
                    self.say(game, id, Some(e.kind), Tone::Warn);
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
                | Event::Regrowth { .. }
                | Event::BuildingPlaced { .. }
                | Event::ProductionQueued { .. }
                | Event::ProductionCancelled { .. }
                | Event::TargetAcquired { .. }
                | Event::Fired { .. }
                | Event::ProjectileSpawned { .. }
                | Event::ProjectileHit { .. }
                | Event::MoveEnded { .. }
                | Event::UnitYielded { .. }
                | Event::UnitStuck { .. } => {}
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
        let now = game.state.tick;
        self.lines.retain(|l| now.saturating_sub(l.tick) < LIFE);
        if self.reply.as_ref().is_some_and(|r| now.saturating_sub(r.tick) >= REPLY_LIFE) {
            self.reply = None;
        }
    }

    /// The words for every message id, after the pack's.
    pub fn words(&self) -> &BTreeMap<String, String> {
        &self.words
    }
}
