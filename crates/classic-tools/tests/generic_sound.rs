//! The generic pack's placeholder sounds are synthesised from code (`crates/classic-tools/src/sound.rs`). This
//! keeps the committed files equal to what the code makes, and every one of them listed in the provenance file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use classic_tools::{setting, sound};

fn files_under(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.insert(p);
        }
    }
}

#[test]
fn committed_sounds_match_the_recipes() {
    let pack = setting::root().join("settings/generic");
    let mut stale = Vec::new();
    let mut expected = BTreeSet::new();
    for f in sound::generate() {
        let path = pack.join(&f.path);
        if std::fs::read(&path).ok().as_deref() != Some(&f.bytes[..]) {
            stale.push(f.path.clone());
        }
        expected.insert(path);
    }
    let mut on_disk = BTreeSet::new();
    files_under(&pack.join("audio"), &mut on_disk);
    let extra: Vec<_> = on_disk.difference(&expected).map(|p| p.display().to_string()).collect();
    assert!(
        stale.is_empty() && extra.is_empty(),
        "settings/generic/audio is out of date with the recipes; run `cargo run --bin sounds`.\nchanged or missing: {stale:?}\nnot made by the generator: {extra:?}"
    );
}

#[test]
fn every_sound_file_has_provenance() {
    let pack = setting::root().join("settings/generic");
    let provenance = std::fs::read_to_string(pack.join("audio/provenance.jsonl")).unwrap();
    for f in sound::generate().iter().filter(|f| f.path.ends_with(".wav")) {
        assert!(provenance.contains(&format!("\"file\": \"{}\"", f.path)), "{} has no provenance line", f.path);
    }
}
