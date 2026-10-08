//! Loading a game in the browser: the setting pack, its art and the maps, fetched from the server the page came
//! from. The browser can't list folders, so this fetches the pack's known data files, the maps its `setting.json`
//! lists, the art its `art.json` names and the sounds its `audio/sounds.json` names; the pack is checked as on the
//! desktop, but only over the files fetched.

use std::path::Path;

use classic_data::json::{self, Value};
use classic_data::{Pack, RulesTable};

use crate::art::{self, ART_INDEX};
use crate::feed::MESSAGES_FILE;
use crate::menu::map_name;
use crate::platform::{Files, web};
use crate::skin;
use crate::sound::{self, SOUND_INDEX};
use crate::theme::THEME_FILE;

/// The repository root, relative to the player page (`web/play/`). Serve the repository root to play.
pub const ROOT: &str = "../../";
/// The generic pack, whose art and sounds stand in for any a pack lacks.
const GENERIC: &str = "settings/generic";
/// A pack's data files: `setting.json` and `names.json` must be there, `tuning.json` may be.
const PACK_DATA: [&str; 3] = ["setting.json", "names.json", "tuning.json"];

pub struct Loaded {
    pub pack: Pack,
    /// The art's files, the pack's own first and then the generic pack's, each by path from its pack folder.
    pub art: Vec<Files>,
    /// The sounds' files: the generic pack's, then the pack's own if it has any.
    pub sounds: Vec<Files>,
    /// The pack's own files the player reads as it goes: its wording of the message feed and its theme, if it has
    /// them.
    pub pack_files: Files,
    /// The maps on offer, as (name, text).
    pub maps: Vec<(String, String)>,
    /// The UI skin's files: the pack's own first, then the generic pack's.
    pub skin: Vec<Files>,
}

/// Fetch and check a game's files. `setting` is a pack under `settings/` by name, or any pack folder by its path
/// from the repository root (such as `settings-private/packs/<name>`); `map` is a path from the root, played alone
/// when `only` is set (the page asked for it), else used when the pack has no maps of its own.
pub async fn load(setting: &str, map: &str, only: bool) -> Result<Loaded, String> {
    let dir =
        if setting.contains('/') { setting.trim_end_matches('/').to_string() } else { format!("settings/{setting}") };
    let wanted: Vec<String> = PACK_DATA.iter().map(|s| s.to_string()).collect();
    let mut data = web::fetch_files(&format!("{ROOT}{dir}/"), &wanted).await?;
    // The maps the pack lists, so the check sees them.
    let listed: Vec<String> = data
        .get("setting.json")
        .and_then(|b| json::parse(&String::from_utf8_lossy(b)).ok())
        .and_then(|v| {
            let list = v.get("maps")?.as_array()?;
            Some(list.iter().filter_map(Value::as_str).map(String::from).collect())
        })
        .unwrap_or_default();
    data.extend(web::fetch_files(&format!("{ROOT}{dir}/"), &listed).await?);
    let present: Vec<String> = data.keys().cloned().collect();
    let read = |f: &str| data.get(f).map(|b| String::from_utf8(b.clone()).map_err(|e| e.to_string())).transpose();
    let pack = Pack::from_files(Path::new(&dir), &present, &read, &RulesTable::builtin())
        .map_err(|errors| format!("setting pack {dir} has problems:\n  {}", errors.join("\n  ")))?;

    // The pack's own art over the generic pack's placeholders, as on the desktop: the first art index found names
    // the files, and each layer fetches those an earlier layer doesn't have, with the atlas pages its own sprites use.
    let index = [ART_INDEX.to_string()];
    let mut art = Vec::new();
    let mut names: Option<Vec<String>> = None;
    let mut have = std::collections::BTreeSet::new();
    for d in if dir == GENERIC { vec![GENERIC.to_string()] } else { vec![dir.clone(), GENERIC.to_string()] } {
        let base = format!("{ROOT}{d}/");
        let mut files = web::fetch_files(&base, &index).await?;
        if names.is_none()
            && let Some(text) = files.get(ART_INDEX)
        {
            names = Some(art::art_files(&String::from_utf8_lossy(text))?);
        }
        let wanted: Vec<String> = names.iter().flatten().filter(|n| !have.contains(*n)).cloned().collect();
        files.extend(web::fetch_files(&base, &wanted).await?);
        let pages = art::atlas_files(&files);
        files.extend(web::fetch_files(&base, &pages).await?);
        have.extend(files.keys().cloned());
        art.push(Files::Memory { label: d, files });
    }
    if names.is_none() {
        return Err(format!("{dir}/{ART_INDEX}: not found, nor in {GENERIC}"));
    }

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

    let own = [MESSAGES_FILE.to_string(), THEME_FILE.to_string()];
    let pack_files =
        Files::Memory { label: dir.clone(), files: web::fetch_files(&format!("{ROOT}{dir}/"), &own).await? };

    let text = |path: &str, bytes: Option<&Vec<u8>>| -> Result<(String, String), String> {
        let bytes = bytes.ok_or_else(|| format!("{path}: not found"))?;
        let text = String::from_utf8(bytes.clone()).map_err(|e| format!("{path}: {e}"))?;
        Ok((map_name(path, &text), text))
    };
    let maps = if only || pack.maps.is_empty() {
        let fetched = web::fetch_files(ROOT, &[map.to_string()]).await?;
        vec![text(map, fetched.get(map))?]
    } else {
        pack.maps.iter().map(|m| text(&format!("{dir}/{m}"), data.get(m))).collect::<Result<_, _>>()?
    };
    // The UI skin: the pack's own, then the generic pack's under it, as on the desktop.
    let mut skin = Vec::new();
    for d in if dir == GENERIC { vec![GENERIC.to_string()] } else { vec![dir.clone(), GENERIC.to_string()] } {
        let base = format!("{ROOT}{d}/");
        let mut files = web::fetch_files(&base, &[skin::THEME_INDEX.to_string()]).await?;
        let Some(index) = files.get(skin::THEME_INDEX) else { continue };
        let names = skin::theme_files(&String::from_utf8_lossy(index));
        files.extend(web::fetch_files(&base, &names).await?);
        let fonts: Vec<String> = names.iter().filter(|n| n.ends_with(".json")).cloned().collect();
        for f in fonts {
            let atlases = files.get(&f).map(|b| skin::font_files(&String::from_utf8_lossy(b))).unwrap_or_default();
            files.extend(web::fetch_files(&base, &atlases).await?);
        }
        skin.push(Files::Memory { label: d, files });
    }
    Ok(Loaded { pack, art, sounds, pack_files, maps, skin })
}
