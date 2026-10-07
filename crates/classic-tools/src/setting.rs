//! Finding and loading a setting pack for the tools, by folder or by name.

use std::path::{Path, PathBuf};

use classic_data::{Pack, RulesTable};

/// The workspace root, where `settings/` and the git-ignored `settings-private/` live.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Where a pack is: a folder holding `setting.json`, a name under `settings/`, or a pack in the git-ignored
/// `settings-private/` (see [`private_packs`]) when the name is `private`, its folder name or the id in its
/// `setting.json`. `private` picks the first private pack.
pub fn find(name: &str) -> Result<PathBuf, String> {
    let direct = PathBuf::from(name);
    if direct.join("setting.json").is_file() {
        return Ok(direct);
    }
    let public = root().join("settings").join(name);
    if public.join("setting.json").is_file() {
        return Ok(public);
    }
    let packs = private_packs();
    if name == "private" {
        if let Some(first) = packs.into_iter().next() {
            return Ok(first);
        }
    } else if let Some(dir) =
        packs.into_iter().find(|d| d.file_name().is_some_and(|f| f == name) || pack_id(d).as_deref() == Some(name))
    {
        return Ok(dir);
    }
    Err(format!(
        "no setting pack \"{name}\": not a folder with setting.json, not in settings/, not in settings-private/"
    ))
}

/// The packs in `settings-private/`: each `packs/<name>/` holding `setting.json`, in name order, then the folder
/// itself if it holds one (the older one-pack layout). Empty when the private repository is not cloned in.
pub fn private_packs() -> Vec<PathBuf> {
    let private = root().join("settings-private");
    let mut packs: Vec<PathBuf> = std::fs::read_dir(private.join("packs"))
        .map(|dir| dir.filter_map(|e| e.ok()).map(|e| e.path()).filter(|d| d.join("setting.json").is_file()).collect())
        .unwrap_or_default();
    packs.sort();
    if private.join("setting.json").is_file() {
        packs.push(private);
    }
    packs
}

/// The `id` in a pack folder's `setting.json`.
fn pack_id(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("setting.json")).ok()?;
    let doc = classic_data::json::parse(&text).ok()?;
    doc.get("id").and_then(|s| s.as_str()).map(String::from)
}

/// Find and load a pack against the engine's rules, with every problem in one message.
pub fn load(name: &str) -> Result<Pack, String> {
    let dir = find(name)?;
    Pack::load(&dir, &RulesTable::builtin())
        .map_err(|errors| format!("setting pack {} has problems:\n  {}", dir.display(), errors.join("\n  ")))
}
