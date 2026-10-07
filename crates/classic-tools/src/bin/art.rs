//! Draw the generic pack's placeholder art and write it into `settings/generic/`, replacing what's there.
//! Run after changing a drawing in `crates/classic-tools/src/art`; the `generic_art` test fails until you do.

use std::collections::BTreeSet;
use std::path::Path;

use classic_tools::{art, setting};

fn main() {
    let pack = setting::root().join("settings/generic");
    let files = art::generate();
    let written: BTreeSet<_> = files.iter().map(|f| pack.join(&f.path)).collect();
    // Clear out drawings that no longer exist, so the folders hold exactly what the generator makes.
    for dir in ["art", "theme"] {
        remove_stale(&pack.join(dir), &written);
    }
    for f in &files {
        let path = pack.join(&f.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &f.bytes).unwrap();
    }
    let bytes: usize = files.iter().map(|f| f.bytes.len()).sum();
    println!("wrote {} files ({} KB) to {}", files.len(), bytes / 1024, pack.display());
}

fn remove_stale(dir: &Path, keep: &BTreeSet<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            remove_stale(&p, keep);
        } else if !keep.contains(&p) {
            std::fs::remove_file(&p).unwrap();
        }
    }
}
