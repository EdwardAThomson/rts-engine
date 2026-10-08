//! The generic pack's placeholder art is drawn from code (`crates/classic-tools/src/art`). These checks keep the
//! committed files equal to what the code draws, and make sure every generic id has something to show.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use classic_data::RulesTable;
use classic_data::json::{self, Value};
use classic_tools::{art, setting};

fn pack() -> PathBuf {
    setting::root().join("settings/generic")
}

/// Files under `dir`, leaving out `art/sprites/` (the Blender renders packed by `art/studio`) and `art/tiles/` (the
/// terrain tiles `art/studio/tileset.py` writes).
fn files_under(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.ends_with("art/sprites") || p.ends_with("art/tiles") {
            continue;
        }
        if p.is_dir() {
            files_under(&p, out)
        } else {
            out.insert(p);
        }
    }
}

#[test]
fn committed_art_matches_the_drawings() {
    let pack = pack();
    let mut stale = Vec::new();
    let mut expected = BTreeSet::new();
    for f in art::generate() {
        let path = pack.join(&f.path);
        if std::fs::read(&path).ok().as_deref() != Some(&f.bytes[..]) {
            stale.push(f.path.clone());
        }
        expected.insert(path);
    }
    let mut on_disk = BTreeSet::new();
    files_under(&pack.join("art"), &mut on_disk);
    files_under(&pack.join("theme"), &mut on_disk);
    let extra: Vec<_> = on_disk.difference(&expected).map(|p| p.display().to_string()).collect();
    assert!(
        stale.is_empty() && extra.is_empty(),
        "settings/generic is out of date with the drawings; run `cargo run --bin art`.\nchanged or missing: {stale:?}\nnot drawn by the generator: {extra:?}"
    );
}

#[test]
fn every_generic_id_has_art() {
    let rules = RulesTable::builtin();
    let drawn: BTreeSet<&str> = art::ids().into_iter().collect();
    let known: BTreeSet<&str> = rules.entities.keys().map(String::as_str).collect();
    assert_eq!(drawn, known, "the art generator and data/rules/entities.json must list the same ids");

    let index = json::parse(&std::fs::read_to_string(pack().join("art/art.json")).unwrap()).unwrap();
    let listed = |section: &str| -> BTreeSet<String> {
        index.get(section).and_then(Value::as_object).unwrap().iter().map(|(k, _)| k.clone()).collect()
    };
    let (sprites, terrain, icons) = (listed("sprites"), listed("terrain"), listed("icons"));
    for (id, entry) in &rules.entities {
        let kind = entry.kind.as_str();
        if kind != "power" {
            assert!(sprites.contains(id) || terrain.contains(id), "{id} has no sprite or tile");
        }
        if matches!(kind, "unit" | "building" | "power") {
            assert!(icons.contains(id), "{id} has no icon");
        }
    }
    // Squads are drawn as copies of a single unit's sprite.
    for (id, squad) in index.get("squads").and_then(Value::as_object).unwrap() {
        assert_eq!(rules.entities.get(id).map(|e| e.kind.as_str()), Some("unit"), "squad {id} is a unit id");
        let member = squad.get("member").and_then(Value::as_str).unwrap();
        assert!(sprites.contains(member), "squad {id}: its member {member} has no sprite");
        assert!(!squad.get("offsets").and_then(Value::as_array).unwrap().is_empty(), "squad {id} has no offsets");
    }
    for section in ["terrain", "sprites", "effects"] {
        for (id, v) in index.get(section).and_then(Value::as_object).unwrap() {
            let file = v.get("file").and_then(Value::as_str).unwrap();
            assert!(pack().join(file).is_file(), "{section}.{id}: {file} is missing");
        }
    }
}

#[test]
fn the_generic_pack_still_loads_with_its_art() {
    setting::load("generic").unwrap();
}

#[test]
fn building_art_matches_the_rules_footprints() {
    let rules = RulesTable::builtin();
    let index = json::parse(&std::fs::read_to_string(pack().join("art/art.json")).unwrap()).unwrap();
    for (id, v) in index.get("sprites").and_then(Value::as_object).unwrap() {
        let Some(size) = v.get("size").and_then(Value::as_array) else { continue };
        let art = (size[0].as_int().unwrap(), size[1].as_int().unwrap());
        // Only where the rules give a footprint: buildings without one yet keep their art size.
        if let (Some(w), Some(h)) = (rules.number(id, "width"), rules.number(id, "height")) {
            assert_eq!(art, (w, h), "{id}: art is {art:?} tiles but its footprint is {w}x{h}");
        }
    }
}
