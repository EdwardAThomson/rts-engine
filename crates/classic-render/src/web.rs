//! Loading a game in the browser: the setting pack, its art and the map, fetched from the server the page came
//! from. The browser can't list folders, so this fetches the pack's known data files, the art its `art.json` names
//! and the sounds its `audio/sounds.json` names; the pack is checked as on the desktop, but only over the files
//! fetched.

use std::path::Path;

use classic_data::{Pack, RulesTable};

use crate::art::{self, ART_INDEX};
use crate::platform::{Files, web};
use crate::sound::{self, SOUND_INDEX};

/// The repository root, relative to the player page (`web/play/`). Serve the repository root to play.
pub const ROOT: &str = "../../";
/// The generic pack, whose art and sounds stand in for any a pack lacks.
const GENERIC: &str = "settings/generic";
/// A pack's data files: `setting.json` and `names.json` must be there, `tuning.json` may be.
const PACK_DATA: [&str; 3] = ["setting.json", "names.json", "tuning.json"];

pub struct Loaded {
    pub pack: Pack,
    /// The art's files, by path from the folder that holds `art/art.json`.
    pub art: Files,
    /// The sounds' files: the generic pack's, then the pack's own if it has any.
    pub sounds: Vec<Files>,
    pub map: String,
}

/// Fetch and check a game's files. `setting` is a pack under `settings/` by name, or any pack folder by its path
/// from the repository root (such as `settings-private/packs/<name>`); `map` is a path from the root.
pub async fn load(setting: &str, map: &str) -> Result<Loaded, String> {
    let dir =
        if setting.contains('/') { setting.trim_end_matches('/').to_string() } else { format!("settings/{setting}") };
    let wanted: Vec<String> = PACK_DATA.iter().map(|s| s.to_string()).collect();
    let data = web::fetch_files(&format!("{ROOT}{dir}/"), &wanted).await?;
    let present: Vec<String> = data.keys().cloned().collect();
    let read = |f: &str| data.get(f).map(|b| String::from_utf8(b.clone()).map_err(|e| e.to_string())).transpose();
    let pack = Pack::from_files(Path::new(&dir), &present, &read, &RulesTable::builtin())
        .map_err(|errors| format!("setting pack {dir} has problems:\n  {}", errors.join("\n  ")))?;

    // The pack's own art, else the generic pack's placeholders, as on the desktop.
    let index = [ART_INDEX.to_string()];
    let mut art_dir = dir.clone();
    let mut files = web::fetch_files(&format!("{ROOT}{art_dir}/"), &index).await?;
    if files.is_empty() {
        art_dir = GENERIC.into();
        files = web::fetch_files(&format!("{ROOT}{art_dir}/"), &index).await?;
    }
    let text = files.get(ART_INDEX).ok_or_else(|| format!("{art_dir}/{ART_INDEX}: not found"))?;
    let names = art::art_files(&String::from_utf8_lossy(text))?;
    files.extend(web::fetch_files(&format!("{ROOT}{art_dir}/"), &names).await?);

    // The generic pack's sounds, then the pack's own over them, as on the desktop.
    let mut sounds = Vec::new();
    for d in if dir == GENERIC { vec![GENERIC.to_string()] } else { vec![GENERIC.to_string(), dir.clone()] } {
        let base = format!("{ROOT}{d}/");
        let mut files = web::fetch_files(&base, &[SOUND_INDEX.to_string()]).await?;
        let Some(index) = files.get(SOUND_INDEX) else { continue };
        let names = sound::files_named(&String::from_utf8_lossy(index));
        files.extend(web::fetch_files(&base, &names).await?);
        sounds.push(Files::Memory { label: d, files });
    }

    let mut fetched = web::fetch_files(ROOT, &[map.to_string()]).await?;
    let map_bytes = fetched.remove(map).ok_or_else(|| format!("{map}: not found"))?;
    let map = String::from_utf8(map_bytes).map_err(|e| format!("{map}: {e}"))?;
    Ok(Loaded { pack, art: Files::Memory { label: art_dir, files }, sounds, map })
}
