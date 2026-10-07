//! The rules data (`data/rules/`): every generic id the engine knows, its kind, whether it is built yet, the roles
//! it plays in built mechanics, and for built ones each tunable number with its default and allowed range. Modules
//! carry numbers too. A setting pack's `tuning.json` moves numbers within those ranges; nothing else can change them.

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
    /// The built mechanics this entity takes part in, such as `harvester` or `refinery`.
    pub roles: Vec<String>,
    pub numbers: BTreeMap<String, Number>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub built: bool,
    pub numbers: BTreeMap<String, Number>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesTable {
    pub entities: BTreeMap<String, Entry>,
    pub modules: BTreeMap<String, Module>,
}

const ENTITIES: &str = include_str!("../../../data/rules/entities.json");
const MODULES: &str = include_str!("../../../data/rules/modules.json");
const KINDS: [&str; 5] = ["building", "unit", "power", "feature", "terrain"];
/// Roles the simulation has code for. A new role is an engine change first.
pub const ROLES: [&str; 3] = ["harvester", "refinery", "wall"];

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

fn numbers(v: &Value, at: &str, built: bool, errors: &mut Vec<String>) -> BTreeMap<String, Number> {
    let mut out = BTreeMap::new();
    for (name, n) in v.get("numbers").and_then(Value::as_object).unwrap_or_default() {
        let get = |k| n.get(k).and_then(Value::as_int);
        match (get("default"), get("min"), get("max")) {
            (Some(default), Some(min), Some(max)) if min <= default && default <= max => {
                out.insert(name.clone(), Number { default, min, max, value: default });
            }
            _ => errors.push(format!("{at}.{name}: needs whole \"default\", \"min\" and \"max\", min ≤ default ≤ max")),
        }
    }
    if !built && !out.is_empty() {
        errors.push(format!("{at}: a planned entry has no numbers yet"));
    }
    out
}

/// Apply one section of tuning (`{ "<id>": { "<number>": value } }`) to a set of entries' numbers.
fn tune<'a>(
    section: &[(String, Value)],
    at: &str,
    mut lookup: impl FnMut(&str) -> Option<&'a BTreeMap<String, Number>>,
    changes: &mut Vec<(String, String, i64)>,
    errors: &mut Vec<String>,
) {
    for (id, fields) in section {
        let Some(nums) = lookup(id) else {
            errors.push(format!("{at}: unknown id \"{id}\""));
            continue;
        };
        let Some(fields) = fields.as_object() else {
            errors.push(format!("{at}{id}: must be an object of numbers"));
            continue;
        };
        for (name, v) in fields {
            let Some(n) = nums.get(name) else {
                errors.push(format!("{at}{id}.{name}: not a tunable number"));
                continue;
            };
            match v.as_int() {
                Some(x) if (n.min..=n.max).contains(&x) => changes.push((id.clone(), name.clone(), x)),
                Some(x) => errors.push(format!("{at}{id}.{name}: {x} is outside {}..={}", n.min, n.max)),
                None => errors.push(format!("{at}{id}.{name}: must be a whole number, not {}", v.kind())),
            }
        }
    }
}

fn hash_numbers<'a>(w: &mut CanonHasher, items: impl Iterator<Item = (&'a String, &'a BTreeMap<String, Number>)>) {
    w.raw("{");
    for (i, (id, nums)) in items.filter(|(_, n)| !n.is_empty()).enumerate() {
        if i > 0 {
            w.raw(",");
        }
        w.string(id);
        w.raw(":{");
        for (j, (name, n)) in nums.iter().enumerate() {
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
            let mut roles = Vec::new();
            for r in v.get("roles").and_then(Value::as_array).unwrap_or_default() {
                match r.as_str() {
                    Some(r) if ROLES.contains(&r) => roles.push(r.to_string()),
                    _ => errors.push(format!("{at}: roles must be from {}", ROLES.join(", "))),
                }
            }
            let numbers = numbers(v, &at, built, &mut errors);
            table.entities.insert(id.clone(), Entry { kind: kind.into(), built, roles, numbers });
        }
        if table.entities.is_empty() {
            errors.push("entities.json: no \"entities\"".into());
        }
        let doc = json::parse(modules).map_err(|e| vec![format!("modules.json: {e}")])?;
        for (id, v) in doc.get("modules").and_then(Value::as_object).unwrap_or_default() {
            let at = format!("modules.json: {id}");
            if !is_generic_id(id) {
                errors.push(format!("{at}: not a generic id"));
            }
            let built = status(v, &at, &mut errors);
            let numbers = numbers(v, &at, built, &mut errors);
            table.modules.insert(id.clone(), Module { built, numbers });
        }
        if errors.is_empty() { Ok(table) } else { Err(errors) }
    }

    /// An entity's number in play, such as `("harvester", "capacity")`.
    pub fn number(&self, id: &str, name: &str) -> Option<i64> {
        self.entities.get(id)?.numbers.get(name).map(|n| n.value)
    }

    /// A module's number in play, such as `("placement", "max_gap")`.
    pub fn module_number(&self, id: &str, name: &str) -> Option<i64> {
        self.modules.get(id)?.numbers.get(name).map(|n| n.value)
    }

    /// Apply a pack's `tuning.json`: `{ "<entity id>": { "<number>": value }, "modules": { "<module id>": { ... } } }`.
    /// Every id and number must exist and every value must sit in its range. Returns the problems found; on any
    /// problem nothing is changed.
    pub fn apply_tuning(&mut self, tuning: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let (mut entity_changes, mut module_changes) = (Vec::new(), Vec::new());
        let Some(ids) = tuning.as_object() else { return vec!["tuning.json: must be an object".into()] };
        let (modules, entities): (Vec<_>, Vec<_>) = ids.iter().cloned().partition(|(k, _)| k == "modules");
        let entities_map = &self.entities;
        tune(
            &entities,
            "tuning.json: ",
            |id| entities_map.get(id).map(|e| &e.numbers),
            &mut entity_changes,
            &mut errors,
        );
        for (_, m) in &modules {
            let Some(section) = m.as_object() else {
                errors.push("tuning.json: \"modules\" must be an object".into());
                continue;
            };
            let modules_map = &self.modules;
            tune(
                section,
                "tuning.json: modules.",
                |id| modules_map.get(id).map(|m| &m.numbers),
                &mut module_changes,
                &mut errors,
            );
        }
        if errors.is_empty() {
            for (id, name, x) in entity_changes {
                self.entities.get_mut(&id).and_then(|e| e.numbers.get_mut(&name)).expect("checked above").value = x;
            }
            for (id, name, x) in module_changes {
                self.modules.get_mut(&id).and_then(|m| m.numbers.get_mut(&name)).expect("checked above").value = x;
            }
        }
        errors
    }

    /// FNV-1a over the canonical JSON of every number in play,
    /// `{"entities":{"<id>":{"<number>":value}},"modules":{...}}` in key order. Replays carry it, so a replay made
    /// under one tuning refuses to run under another.
    pub fn hash(&self) -> String {
        let mut w = CanonHasher::new();
        w.raw("{\"entities\":");
        hash_numbers(&mut w, self.entities.iter().map(|(k, e)| (k, &e.numbers)));
        w.raw(",\"modules\":");
        hash_numbers(&mut w, self.modules.iter().map(|(k, m)| (k, &m.numbers)));
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
        assert!(r.entities["battle_tank"].built);
        assert!(!r.entities["hazard"].built);
        assert_eq!(r.entities["refinery"].roles, ["refinery"]);
        assert!(r.modules["harvesting"].built);
        assert_eq!(r.module_number("placement", "max_gap"), Some(0));
    }

    #[test]
    fn tuning_moves_numbers_within_range_only() {
        let mut r = RulesTable::builtin();
        let before = r.hash();
        let bad = json::parse(
            r#"{"harvester":{"capacity":0,"wings":2},"dragon":{},"modules":{"placement":{"max_gap":9},"teleport":{}}}"#,
        )
        .unwrap();
        let errors = r.apply_tuning(&bad);
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert_eq!(r.hash(), before, "a rejected tuning changes nothing");
        let good = json::parse(r#"{"harvester":{"capacity":250},"modules":{"placement":{"rock_only":0}}}"#).unwrap();
        assert!(r.apply_tuning(&good).is_empty());
        assert_eq!(r.number("harvester", "capacity"), Some(250));
        assert_eq!(r.module_number("placement", "rock_only"), Some(0));
        assert_ne!(r.hash(), before);
    }
}
