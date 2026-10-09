//! Synthesise the generic pack's placeholder sounds and write them into `settings/generic/audio/`, replacing the
//! sounds there (the voices, from `audio/make_voices.py`, stay). Run after changing a recipe in `crates/classic-tools/src/sound.rs`; the `generic_sound` test fails until
//! you do.

use classic_tools::{setting, sound};

fn main() {
    let pack = setting::root().join("settings/generic");
    let audio = pack.join("audio");
    // Clear out sounds that no longer exist, so the sound folders hold exactly what the generator makes.
    for dir in ["sfx", "ui"] {
        if audio.join(dir).is_dir() {
            std::fs::remove_dir_all(audio.join(dir)).unwrap();
        }
    }
    let files = sound::generate();
    for f in &files {
        let path = pack.join(&f.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &f.bytes).unwrap();
    }
    let bytes: usize = files.iter().map(|f| f.bytes.len()).sum();
    println!("wrote {} files ({} KB) to {}", files.len(), bytes / 1024, audio.display());
}
