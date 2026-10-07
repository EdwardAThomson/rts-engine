//! Finding and loading a setting pack for the tools, by folder or by name.

use std::path::{Path, PathBuf};

use classic_data::{Pack, RulesTable};

/// The workspace root, where `settings/` and the git-ignored `settings-private/` live.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Where a pack is: a folder holding `setting.json`, a name under `settings/`, or `settings-private/` when the name
/// is `private` or matches the id in its `setting.json`.
pub fn find(name: &str) -> Result<PathBuf, String> {
    let direct = PathBuf::from(name);
    if direct.join("setting.json").is_file() {
        return Ok(direct);
    }
    let root = root();
    let public = root.join("settings").join(name);
    if public.join("setting.json").is_file() {
        return Ok(public);
    }
    let private = root.join("settings-private");
    let private_id = std::fs::read_to_string(private.join("setting.json"))
        .ok()
        .and_then(|t| classic_data::json::parse(&t).ok())
        .and_then(|v| v.get("id").and_then(|s| s.as_str()).map(String::from));
    if private_id.is_some() && (name == "private" || private_id.as_deref() == Some(name)) {
        return Ok(private);
    }
    Err(format!("no setting pack \"{name}\": not a folder with setting.json, not in settings/, not settings-private/"))
}

/// Find and load a pack against the engine's rules, with every problem in one message.
pub fn load(name: &str) -> Result<Pack, String> {
    let dir = find(name)?;
    Pack::load(&dir, &RulesTable::builtin())
        .map_err(|errors| format!("setting pack {} has problems:\n  {}", dir.display(), errors.join("\n  ")))
}
