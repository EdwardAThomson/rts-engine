//! Setting packs: a folder of data files and assets that turns generic ids into a world (`docs/ARCHITECTURE.md`,
//! "Keeping the code generic"). Loading checks the whole pack and reports every problem at once:
//!
//! - only data and assets: no file a browser or shell could run;
//! - `setting.json` names known modules only, and at least one faction, each with a unique id;
//! - `names.json` names known ids only, and every built entity the pack uses has a name;
//! - `tuning.json`, when present, moves known numbers within their ranges.
//!
//! The pack never changes the engine's code paths; it only fills in names, numbers and (later) asset paths.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::json::{self, Value};
use crate::rules::{RulesTable, is_generic_id};

/// File types a pack may hold: data, styles, documents and assets. Anything else, scripts above all, is refused.
const ALLOWED: [&str; 15] =
    ["json", "jsonl", "css", "md", "txt", "png", "webp", "svg", "ogg", "wav", "mp3", "ttf", "otf", "woff2", "cur"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Faction {
    pub id: String,
    pub name: String,
    pub ramp: String,
    pub advisor: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub dir: PathBuf,
    pub id: String,
    pub title: String,
    pub factions: Vec<Faction>,
    /// Module id to switched on.
    pub features: BTreeMap<String, bool>,
    /// The entity ids the pack uses; `None` means every one.
    pub entities: Option<Vec<String>>,
    /// Display name for each generic id.
    pub names: BTreeMap<String, String>,
    /// The engine's rules with the pack's tuning applied.
    pub rules: RulesTable,
    /// Things worth knowing that don't stop the pack loading, such as planned ids without a name yet.
    pub warnings: Vec<String>,
}

impl Pack {
    /// The display name for a generic id, or the id itself when the pack doesn't name it.
    pub fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map_or(id, String::as_str)
    }

    pub fn uses(&self, id: &str) -> bool {
        self.entities.as_ref().is_none_or(|e| e.iter().any(|x| x == id))
    }

    /// Load and check the pack in `dir` against `rules`. Returns every problem found when it doesn't pass.
    pub fn load(dir: &Path, rules: &RulesTable) -> Result<Pack, Vec<String>> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        check_files(dir, dir, &mut errors);

        let read = |file: &str, errors: &mut Vec<String>| -> Option<Value> {
            let text = match std::fs::read_to_string(dir.join(file)) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
                Err(e) => {
                    errors.push(format!("{file}: {e}"));
                    return None;
                }
            };
            json::parse(&text).map_err(|e| errors.push(format!("{file}: {e}"))).ok()
        };

        let Some(setting) = read("setting.json", &mut errors) else {
            errors.push("setting.json: missing".into());
            return Err(errors);
        };
        let text = |v: &Value, key: &str, at: &str, errors: &mut Vec<String>| -> String {
            match v.get(key).and_then(Value::as_str) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    errors.push(format!("{at}: \"{key}\" must be a non-empty string"));
                    String::new()
                }
            }
        };
        let id = text(&setting, "id", "setting.json", &mut errors);
        if !id.is_empty() && !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        {
            errors.push(format!("setting.json: pack id \"{id}\" may use only a-z, 0-9, _ and -"));
        }
        let title = text(&setting, "title", "setting.json", &mut errors);

        let mut factions: Vec<Faction> = Vec::new();
        match setting.get("factions").and_then(Value::as_array) {
            Some(list) if !list.is_empty() => {
                for (i, f) in list.iter().enumerate() {
                    let at = format!("setting.json: factions[{i}]");
                    let fid = text(f, "id", &at, &mut errors);
                    if !fid.is_empty() && !is_generic_id(&fid) {
                        errors.push(format!("{at}: \"{fid}\" is not a generic id"));
                    }
                    if factions.iter().any(|x| x.id == fid) {
                        errors.push(format!("{at}: faction id \"{fid}\" given twice"));
                    }
                    let name = text(f, "name", &at, &mut errors);
                    let ramp = text(f, "ramp", &at, &mut errors);
                    let advisor = f.get("advisor").and_then(Value::as_str).map(String::from);
                    factions.push(Faction { id: fid, name, ramp, advisor });
                }
            }
            _ => errors.push("setting.json: \"factions\" must list at least one faction".into()),
        }

        let mut features = BTreeMap::new();
        for (m, on) in setting.get("features").and_then(Value::as_object).unwrap_or_default() {
            match (rules.modules.get(m), on.as_bool()) {
                (None, _) => errors.push(format!("setting.json: features: unknown module \"{m}\"")),
                (_, None) => errors.push(format!("setting.json: features.{m}: must be true or false")),
                (Some(_), Some(on)) => {
                    features.insert(m.clone(), on);
                }
            }
        }

        let entities = match setting.get("entities") {
            None => None,
            Some(Value::Str(s)) if s == "all" => None,
            Some(Value::Array(list)) => {
                let mut ids = Vec::new();
                for v in list {
                    match v.as_str() {
                        Some(e) if rules.entities.contains_key(e) => ids.push(e.to_string()),
                        Some(e) => errors.push(format!("setting.json: entities: unknown id \"{e}\"")),
                        None => errors.push(format!("setting.json: entities: {} is not an id", v.kind())),
                    }
                }
                Some(ids)
            }
            Some(_) => {
                errors.push("setting.json: \"entities\" must be \"all\" or a list of ids".into());
                None
            }
        };

        let mut names = BTreeMap::new();
        match read("names.json", &mut errors) {
            None => errors.push("names.json: missing".into()),
            Some(doc) => {
                if let Some(p) = doc.get("pack").and_then(Value::as_str)
                    && p != id
                {
                    errors.push(format!("names.json: \"pack\" is \"{p}\" but setting.json's id is \"{id}\""));
                }
                for (k, v) in doc.get("names").and_then(Value::as_object).unwrap_or_default() {
                    match (rules.entities.contains_key(k), v.as_str()) {
                        (false, _) => errors.push(format!("names.json: unknown id \"{k}\"")),
                        (_, Some(n)) if !n.trim().is_empty() => {
                            names.insert(k.clone(), n.to_string());
                        }
                        _ => errors.push(format!("names.json: {k}: must be a non-empty string")),
                    }
                }
            }
        }

        let mut tuned = rules.clone();
        if let Some(t) = read("tuning.json", &mut errors) {
            errors.extend(tuned.apply_tuning(&t));
        }

        let pack = Pack {
            dir: dir.to_path_buf(),
            id,
            title,
            factions,
            features,
            entities,
            names,
            rules: tuned,
            warnings: Vec::new(),
        };
        for (eid, entry) in &rules.entities {
            if !pack.uses(eid) || pack.names.contains_key(eid) {
                continue;
            }
            if entry.built {
                errors.push(format!("names.json: no name for \"{eid}\", which the pack uses"));
            } else {
                warnings.push(format!("names.json: no name yet for planned id \"{eid}\""));
            }
        }
        if errors.is_empty() { Ok(Pack { warnings, ..pack }) } else { Err(errors) }
    }
}

/// Walk the pack, skipping hidden entries such as `.git`, and refuse any file that isn't data or an asset.
fn check_files(root: &Path, dir: &Path, errors: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return errors.push(format!("{}: {e}", dir.display())),
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        let hidden = p.file_name().and_then(|n| n.to_str()).is_none_or(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if p.is_dir() {
            check_files(root, &p, errors);
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        if !ext.is_some_and(|e| ALLOWED.contains(&e.as_str())) {
            let shown = p.strip_prefix(root).unwrap_or(&p).display();
            errors.push(format!("{shown}: not a data or asset file; packs hold no code"));
        }
    }
}
