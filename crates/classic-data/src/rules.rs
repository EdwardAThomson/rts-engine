//! The rules data (`data/rules/`): every generic id the engine knows, its kind, whether it is built yet, and for
//! built ones each tunable number with its default and allowed range. A setting pack's `tuning.json` moves numbers
//! within those ranges; nothing else can change them.

use std::collections::BTreeMap;

use rts_core::hash::CanonHasher;

use crate::json::{self, Value};

/// One tunable number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Number {
    pub default: i64,
    pub min: i64,
    pub max: i64,
    /// The number in play: the default, or a pack's tuning.
    pub value: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `building`, `unit`, `power`, `feature` or `terrain`.
    pub kind: String,
    /// False for ids a pack may already name but the simulation does not have yet.
    pub built: bool,
    pub numbers: BTreeMap<String, Number>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesTable {
    pub entities: BTreeMap<String, Entry>,
    /// Module id to whether it is built.
    pub modules: BTreeMap<String, bool>,
}

const ENTITIES: &str = include_str!("../../../data/rules/entities.json");
const MODULES: &str = include_str!("../../../data/rules/modules.json");
const KINDS: [&str; 5] = ["building", "unit", "power", "feature", "terrain"];

/// A generic id: lowercase ASCII letters, digits and underscores.
pub fn is_generic_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn status(v: &Value, at: &str, errors: &mut Vec<String>) -> bool {
    match v.get("status").and_then(Value::as_str) {
        Some("built") => true,
        Some("planned") => false,
        _ => {
            errors.push(format!("{at}: \"status\" must be \"built\" or \"planned\""));
            false
        }
    }
}

impl RulesTable {
    /// The rules data compiled into the engine.
    pub fn builtin() -> RulesTable {
        RulesTable::parse(ENTITIES, MODULES).unwrap_or_else(|e| panic!("data/rules is invalid: {}", e.join("; ")))
    }

    pub fn parse(entities: &str, modules: &str) -> Result<RulesTable, Vec<String>> {
        let mut errors = Vec::new();
        let mut table = RulesTable { entities: BTreeMap::new(), modules: BTreeMap::new() };
        let doc = json::parse(entities).map_err(|e| vec![format!("entities.json: {e}")])?;
        for (id, v) in doc.get("entities").and_then(Value::as_object).unwrap_or_default() {
            let at = format!("entities.json: {id}");
            if !is_generic_id(id) {
                errors.push(format!("{at}: not a generic id"));
            }
            let kind = v.get("kind").and_then(Value::as_str).unwrap_or_default();
            if !KINDS.contains(&kind) {
                errors.push(format!("{at}: \"kind\" must be one of {}", KINDS.join(", ")));
            }
            let built = status(v, &at, &mut errors);
            let mut numbers = BTreeMap::new();
            for (name, n) in v.get("numbers").and_then(Value::as_object).unwrap_or_default() {
                let get = |k| n.get(k).and_then(Value::as_int);
                match (get("default"), get("min"), get("max")) {
                    (Some(default), Some(min), Some(max)) if min <= default && default <= max => {
                        numbers.insert(name.clone(), Number { default, min, max, value: default });
                    }
                    _ => errors.push(format!(
                        "{at}.{name}: needs whole \"default\", \"min\" and \"max\", min ≤ default ≤ max"
                    )),
                }
            }
            if !built && !numbers.is_empty() {
                errors.push(format!("{at}: a planned entity has no numbers yet"));
            }
            table.entities.insert(id.clone(), Entry { kind: kind.into(), built, numbers });
        }
        if table.entities.is_empty() {
            errors.push("entities.json: no \"entities\"".into());
        }
        let doc = json::parse(modules).map_err(|e| vec![format!("modules.json: {e}")])?;
        for (id, v) in doc.get("modules").and_then(Value::as_object).unwrap_or_default() {
            if !is_generic_id(id) {
                errors.push(format!("modules.json: {id}: not a generic id"));
            }
            let built = status(v, &format!("modules.json: {id}"), &mut errors);
            table.modules.insert(id.clone(), built);
        }
        if errors.is_empty() { Ok(table) } else { Err(errors) }
    }

    /// A number in play, such as `("harvester", "capacity")`.
    pub fn number(&self, id: &str, name: &str) -> Option<i64> {
        self.entities.get(id)?.numbers.get(name).map(|n| n.value)
    }

    /// Apply a pack's `tuning.json`: `{ "<id>": { "<number>": value } }`. Every id and number must exist and every
    /// value must sit in its range. Returns the problems found; on any problem nothing is changed.
    pub fn apply_tuning(&mut self, tuning: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let mut changes = Vec::new();
        let Some(ids) = tuning.as_object() else { return vec!["tuning.json: must be an object".into()] };
        for (id, fields) in ids {
            let Some(entry) = self.entities.get(id) else {
                errors.push(format!("tuning.json: unknown id \"{id}\""));
                continue;
            };
            let Some(fields) = fields.as_object() else {
                errors.push(format!("tuning.json: {id}: must be an object of numbers"));
                continue;
            };
            for (name, v) in fields {
                let Some(n) = entry.numbers.get(name) else {
                    errors.push(format!("tuning.json: {id}.{name}: not a tunable number"));
                    continue;
                };
                match v.as_int() {
                    Some(x) if (n.min..=n.max).contains(&x) => changes.push((id.clone(), name.clone(), x)),
                    Some(x) => errors.push(format!("tuning.json: {id}.{name}: {x} is outside {}..={}", n.min, n.max)),
                    None => errors.push(format!("tuning.json: {id}.{name}: must be a whole number, not {}", v.kind())),
                }
            }
        }
        if errors.is_empty() {
            for (id, name, x) in changes {
                self.entities.get_mut(&id).and_then(|e| e.numbers.get_mut(&name)).expect("checked above").value = x;
            }
        }
        errors
    }

    /// FNV-1a over the canonical JSON of every number in play, `{"<id>":{"<number>":value}}` in key order. Replays
    /// carry it, so a replay made under one tuning refuses to run under another.
    pub fn hash(&self) -> String {
        let mut w = CanonHasher::new();
        w.raw("{");
        let built = self.entities.iter().filter(|(_, e)| !e.numbers.is_empty());
        for (i, (id, e)) in built.enumerate() {
            if i > 0 {
                w.raw(",");
            }
            w.string(id);
            w.raw(":{");
            for (j, (name, n)) in e.numbers.iter().enumerate() {
                if j > 0 {
                    w.raw(",");
                }
                w.string(name);
                w.raw(":");
                w.int(n.value);
            }
            w.raw("}");
        }
        w.raw("}");
        w.hex()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_load() {
        let r = RulesTable::builtin();
        assert_eq!(r.number("harvester", "capacity"), Some(200));
        assert!(r.entities["tank"].built);
        assert!(!r.entities["hazard"].built);
        assert_eq!(r.modules.get("harvesting"), Some(&true));
    }

    #[test]
    fn tuning_moves_numbers_within_range_only() {
        let mut r = RulesTable::builtin();
        let before = r.hash();
        let bad = json::parse(r#"{"harvester":{"capacity":0,"wings":2},"dragon":{}}"#).unwrap();
        let errors = r.apply_tuning(&bad);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert_eq!(r.hash(), before, "a rejected tuning changes nothing");
        let good = json::parse(r#"{"harvester":{"capacity":250}}"#).unwrap();
        assert!(r.apply_tuning(&good).is_empty());
        assert_eq!(r.number("harvester", "capacity"), Some(250));
        assert_ne!(r.hash(), before);
    }
}
