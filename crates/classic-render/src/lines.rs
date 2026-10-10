//! What the advisor and the units say, as text for now: the advisor's own words for the message feed, and the
//! units' short replies when the local player selects them or gives an order, shown as a subtitle. Voices will
//! play the same lines later. Design: `plans/rts/audio.md` (unit acknowledgements, the advisor) and
//! `plans/rts/settings.md` (`lines.json`).
//!
//! The engine's replies are in `data/ui/lines.json`. A pack's `lines.json` gives its own: top-level `advisor` and
//! `acks` for every faction, and `factions.<id>.advisor` and `.acks` for one, which win over the pack-wide ones.
//! `acks` are by voice set: `infantry`, `vehicle` and `aircraft` (every unit belongs to one), or a unit's generic
//! id, which gives that unit a voice of its own; any moment its own set leaves out it says in its class's set.
//! A line is a string or a list of strings to take turns with. Anything else in the file (such as `voice`, the
//! direction notes for recording) is left for the tools.

use std::collections::BTreeMap;

use classic_data::json::{self, Value};

use crate::platform::Files;

const LINES: &str = include_str!("../../../data/ui/lines.json");
/// Where a pack keeps its lines, in its folder.
pub const LINES_FILE: &str = "lines.json";

/// The voice sets every unit falls in: infantry (infantry armour), aircraft, and vehicles for every other unit. A
/// pack may add a set for one unit, named by its generic id.
pub const VOICES: [&str; 3] = ["infantry", "vehicle", "aircraft"];

/// When a unit replies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Moment {
    Select,
    Move,
    Attack,
    /// An order it can't carry out.
    Cant,
}

impl Moment {
    pub const ALL: [Moment; 4] = [Moment::Select, Moment::Move, Moment::Attack, Moment::Cant];

    pub fn id(self) -> &'static str {
        match self {
            Moment::Select => "select",
            Moment::Move => "move",
            Moment::Attack => "attack",
            Moment::Cant => "cant",
        }
    }
}

/// A line just said, for the sound board to voice: who said it (`advisor`, or a unit voice set), its id (an advisor
/// line id, or a moment's) and which of its variants, from 0, so the voice speaks the words the screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speech {
    pub who: String,
    pub key: String,
    pub variant: usize,
    /// Said in the engine's own words (`data/ui`), which the pack left alone; only these may take the generic
    /// pack's voices, which speak the engine's words.
    pub engine: bool,
}

/// One faction's lines, after the pack's.
#[derive(Clone, Debug, Default)]
pub struct Lines {
    /// The advisor's own words by message id; ids it has none for keep the feed's words.
    pub advisor: BTreeMap<String, Vec<String>>,
    /// Replies by voice set (one of [`VOICES`], or a unit's generic id) and moment.
    pub acks: BTreeMap<(String, Moment), Vec<String>>,
    pub warnings: Vec<String>,
}

/// A string, or a list of strings; `None` for anything else or an empty list.
fn texts(v: &Value) -> Option<Vec<String>> {
    let list = match v {
        Value::Array(a) => a.iter().map(|t| t.as_str().map(String::from)).collect::<Option<Vec<_>>>()?,
        _ => vec![v.as_str()?.to_string()],
    };
    (!list.is_empty() && list.iter().all(|t| !t.trim().is_empty())).then_some(list)
}

/// Advisor ids a pack may write lines for: the feed's messages, and those it doesn't say yet.
pub fn advisor_ids() -> Vec<String> {
    let mut ids: Vec<String> = crate::feed::default_words().into_keys().collect();
    let v = json::parse(LINES).expect("data/ui/lines.json parses");
    ids.extend(
        v.get("planned")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.as_str())
            .map(String::from),
    );
    ids
}

impl Lines {
    /// The engine's own lines: replies only, since the feed's words are the engine's advisor.
    pub fn engine() -> Lines {
        let v = json::parse(LINES).expect("data/ui/lines.json parses");
        let mut lines = Lines::default();
        lines.read_acks(v.get("acks").expect("data/ui/lines.json has `acks`"), "data/ui/lines.json", &[]);
        assert!(lines.warnings.is_empty(), "data/ui/lines.json: {:?}", lines.warnings);
        lines
    }

    /// The lines of `faction` (a faction id in `factions`, the pack's), after the pack's `lines.json` in `pack` where
    /// it has one. `units` are the generic ids a voice set of one unit's own may be named by. The whole file is
    /// checked, every faction's part too.
    pub fn load(pack: &Files, factions: &[String], units: &[String], faction: Option<&str>) -> Lines {
        let mut lines = Lines::engine();
        let Ok(text) = pack.read_text(LINES_FILE) else { return lines };
        let file = pack.name(LINES_FILE);
        let v = match json::parse(&text) {
            Ok(v) if v.as_object().is_some() => v,
            _ => {
                lines.warnings.push(format!("{file}: needs to be a JSON object"));
                return lines;
            }
        };
        let ids = advisor_ids();
        // Pack-wide first, then the faction's, which wins; other factions' parts are read only to check them.
        lines.read(&v, &file, &ids, units);
        if let Some(fs) = v.get("factions") {
            let Some(fs) = fs.as_object() else {
                lines.warnings.push(format!("{file}: `factions` needs to be an object"));
                return lines;
            };
            for (id, part) in fs {
                if !factions.contains(id) {
                    lines.warnings.push(format!("{file}: no faction `{id}` in the pack"));
                }
                let mut own = Lines { acks: lines.acks.clone(), ..Lines::default() };
                own.read(part, &format!("{file}: {id}"), &ids, units);
                lines.warnings.append(&mut own.warnings);
                if faction == Some(id.as_str()) {
                    lines.advisor.append(&mut own.advisor);
                    lines.acks = own.acks;
                }
            }
        }
        lines
    }

    fn read(&mut self, v: &Value, file: &str, ids: &[String], units: &[String]) {
        if let Some(adv) = v.get("advisor") {
            match adv.as_object() {
                Some(adv) => {
                    for (id, t) in adv {
                        if !ids.contains(id) {
                            self.warnings.push(format!("{file}: no advisor line `{id}`"));
                        } else if let Some(t) = texts(t) {
                            self.advisor.insert(id.clone(), t);
                        } else {
                            self.warnings.push(format!("{file}: advisor `{id}` needs a line or a list of lines"));
                        }
                    }
                }
                None => self.warnings.push(format!("{file}: `advisor` needs to be an object")),
            }
        }
        if let Some(acks) = v.get("acks") {
            self.read_acks(acks, file, units);
        }
    }

    fn read_acks(&mut self, v: &Value, file: &str, units: &[String]) {
        let Some(sets) = v.as_object() else {
            self.warnings.push(format!("{file}: `acks` needs to be an object"));
            return;
        };
        for (set, moments) in sets {
            if !VOICES.contains(&set.as_str()) && !units.contains(set) {
                self.warnings.push(format!("{file}: no voice set `{set}` ({} or a unit's id)", VOICES.join(", ")));
                continue;
            }
            let Some(moments) = moments.as_object() else {
                self.warnings.push(format!("{file}: `{set}` needs to be an object"));
                continue;
            };
            for (m, t) in moments {
                let Some(moment) = Moment::ALL.into_iter().find(|x| x.id() == m) else {
                    self.warnings.push(format!("{file}: {set}: no moment `{m}`"));
                    continue;
                };
                match texts(t) {
                    Some(t) => {
                        self.acks.insert((set.clone(), moment), t);
                    }
                    None => self.warnings.push(format!("{file}: {set}.{m} needs a line or a list of lines")),
                }
            }
        }
    }
}

/// The voice set `units` answer in: the one unit kind's own set when they are all of one kind and the lines give
/// it this `moment`, else their class's: infantry when all are infantry, aircraft when all fly, vehicle otherwise.
pub fn voice_of(game: &classic_sim::Game, units: &[&classic_sim::Entity], lines: &Lines, moment: Moment) -> String {
    let kinds: Vec<_> = units.iter().map(|e| game.rules.kind(e.kind)).collect();
    if let Some(first) = kinds.first()
        && kinds.iter().all(|k| k.id == first.id)
        && lines.acks.contains_key(&(first.id.clone(), moment))
    {
        return first.id.clone();
    }
    let infantry = classic_data::ARMOURS.iter().position(|&a| a == "infantry");
    let class = if kinds.iter().all(|k| Some(k.armour) == infantry) {
        VOICES[0]
    } else if kinds.iter().all(|k| k.air) {
        VOICES[2]
    } else {
        VOICES[1]
    };
    class.to_string()
}
