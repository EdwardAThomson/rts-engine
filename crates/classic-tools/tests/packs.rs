//! Setting packs: the checks from `docs/ARCHITECTURE.md` ("How we'll prove the separation holds") that the
//! loader can make today. Story and look never change the game; tuning does, and says so in the rules hash; a pack
//! with problems reports all of them.

use std::path::{Path, PathBuf};

use classic_data::{Pack, RulesTable};
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/test-01.txt");

/// A scratch copy of the generic pack, changed by `edit`, in its own temporary folder.
fn variant(name: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("classic-pack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let generic = setting::root().join("settings/generic");
    for f in ["setting.json", "names.json"] {
        std::fs::copy(generic.join(f), dir.join(f)).unwrap();
    }
    edit(&dir);
    dir
}

fn replace(dir: &Path, file: &str, from: &str, to: &str) {
    let text = std::fs::read_to_string(dir.join(file)).unwrap();
    assert!(text.contains(from), "{file} has no {from:?}");
    std::fs::write(dir.join(file), text.replacen(from, to, 1)).unwrap();
}

/// A short game under a pack's rules, stopped while the tank is still on its way; returns the state hash.
fn play(pack: &Pack) -> String {
    let rules = Rules::from_table(&pack.rules).unwrap();
    let mut g = Game::new(GameOptions { map: MAP, seed: 3, players: None, rules: Some(&rules) }).unwrap();
    g.order(0, &[3], CommandOrder::Move { x: 20, y: 9 });
    g.step(100);
    g.hash()
}

#[test]
fn the_generic_pack_names_every_id_and_loads_cleanly() {
    let pack = setting::load("generic").unwrap();
    assert_eq!(pack.id, "generic");
    assert_eq!(pack.factions.len(), 2, "two factions on purpose: nothing may assume three");
    assert!(pack.warnings.is_empty(), "{:?}", pack.warnings);
    let rules = RulesTable::builtin();
    for id in rules.entities.keys() {
        assert_ne!(pack.name(id), id, "the generic pack names {id}");
    }
    assert_eq!(pack.rules, rules, "the generic pack has no tuning");
}

#[test]
fn the_private_pack_loads_when_it_is_cloned_in() {
    if !setting::root().join("settings-private/setting.json").is_file() {
        eprintln!("settings-private/ is not cloned here; skipping");
        return;
    }
    let pack = setting::load("private").unwrap_or_else(|e| panic!("{e}"));
    assert!(pack.warnings.is_empty(), "{:?}", pack.warnings);
}

#[test]
fn packs_that_differ_only_in_names_play_the_same_game() {
    let generic = setting::load("generic").unwrap();
    let dir = variant("renamed", |d| {
        replace(d, "names.json", "\"Battle Tank\"", "\"Something Else Entirely\"");
        replace(d, "setting.json", "\"Faction A\"", "\"Other Name\"");
    });
    let renamed = Pack::load(&dir, &RulesTable::builtin()).unwrap();
    assert_eq!(renamed.name("battle_tank"), "Something Else Entirely");
    assert_eq!(renamed.rules.hash(), generic.rules.hash());
    assert_eq!(play(&renamed), play(&generic));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tuning_changes_the_rules_hash_and_the_game() {
    let generic = setting::load("generic").unwrap();
    let dir = variant("tuned", |d| {
        std::fs::write(d.join("tuning.json"), r#"{ "battle_tank": { "speed": 40 } }"#).unwrap();
    });
    let tuned = Pack::load(&dir, &RulesTable::builtin()).unwrap();
    assert_eq!(tuned.rules.number("battle_tank", "speed"), Some(40));
    assert_ne!(tuned.rules.hash(), generic.rules.hash());
    assert_ne!(play(&tuned), play(&generic));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_pack_with_problems_reports_every_one() {
    let dir = variant("broken", |d| {
        std::fs::write(d.join("run.sh"), "echo hi\n").unwrap();
        std::fs::write(d.join("tuning.json"), r#"{ "harvester": { "capacity": 0 } }"#).unwrap();
        replace(d, "names.json", "\"refinery\": \"Refinery\",", "\"dragon\": \"Dragon\",");
        replace(d, "setting.json", "\"features\": {}", "\"features\": { \"teleport\": true }");
    });
    let errors = Pack::load(&dir, &RulesTable::builtin()).unwrap_err();
    let expect = [
        "run.sh: not a data or asset file",
        "unknown module \"teleport\"",
        "names.json: unknown id \"dragon\"",
        "harvester.capacity: 0 is outside",
        "no name for \"refinery\"",
    ];
    for e in expect {
        assert!(errors.iter().any(|x| x.contains(e)), "missing {e:?} in {errors:#?}");
    }
    assert_eq!(errors.len(), expect.len(), "{errors:#?}");
    std::fs::remove_dir_all(dir).unwrap();
}
