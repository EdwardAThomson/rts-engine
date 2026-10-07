//! No protected names in this repository (CLAUDE.md): code, data, docs and file names use generic ids only, and
//! setting-specific names live only in the private pack, cloned into the git-ignored `settings-private/`.
//!
//! The list holds hashes, not the words, so the check itself never names them. Each word of every file, and each
//! pair of neighbouring words run together (two-word names are often written as one), is hashed and looked up.
//! Crediting another project's idea by its own name, as CLAUDE.md asks, is fine; those names are not on the list.

use std::path::Path;

use classic_tools::setting;

/// FNV-1a, 64-bit, of each protected word in lowercase.
const PROTECTED: [u64; 21] = [
    0x1cf58166f5604e43,
    0xc8d90c884603df18,
    0x50c97d0a3052db20,
    0x7fae0e02da08c215,
    0x6c080c7281c50eaa,
    0xd239e295f65f4301,
    0x299ea38a809abc74,
    0x8b454a8adbb237fd,
    0xe77d9670a4e14586,
    0xda11bfd0a12e8144,
    0x00127f525b90764c,
    0x80e836c5696e58d9,
    0xd2e38d2fa4ebd7e3,
    0x5a6aaf57605c64a7,
    0xd5a91593262e5910,
    0x1c6559f4276a7dd0,
    0xe7f32b15475e13b7,
    0x7f3ae496c7fb38d9,
    0x0b658424a7876c94,
    0xb082d8f5b04b49dd,
    0xdee5d2ffab5f7437,
];

/// Folders never scanned: build output, git's own data, and the private pack, which is meant to hold the names.
const SKIP: [&str; 4] = ["target", "settings-private", "node_modules", ".git"];

fn fnv64(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The protected words in `text`, by line number.
fn hits(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let words: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
        let pairs = words.windows(2).map(|p| format!("{}{}", p[0], p[1]));
        for w in words.iter().map(|w| w.to_string()).chain(pairs) {
            if PROTECTED.contains(&fnv64(&w)) {
                found.push((n + 1, w));
            }
        }
    }
    found
}

fn scan(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let shown = p.strip_prefix(root).unwrap_or(&p).display().to_string();
        for (_, w) in hits(&name) {
            found.push(format!("{shown}: file name contains \"{w}\""));
        }
        if p.is_dir() {
            scan(root, &p, found);
        } else if let Ok(bytes) = std::fs::read(&p) {
            for (line, w) in hits(&String::from_utf8_lossy(&bytes)) {
                found.push(format!("{shown}:{line}: \"{w}\""));
            }
        }
    }
}

#[test]
fn no_protected_names_in_the_repository() {
    let root = setting::root().canonicalize().unwrap();
    let mut found = Vec::new();
    scan(&root, &root, &mut found);
    assert!(found.is_empty(), "protected names belong in the private pack only:\n{}", found.join("\n"));
}

#[test]
fn the_check_catches_single_words_and_run_together_pairs() {
    // Built at run time, so this file never holds the words: rot13 of two entries on the list.
    let rot13 = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_ascii_lowercase() { (((c as u8 - b'a' + 13) % 26) + b'a') as char } else { c })
            .collect()
    };
    let one = rot13("fcvpr");
    let two: Vec<String> = ["jvaq", "genc"].iter().map(|w| rot13(w)).collect();
    let text = format!("plain line\nthe {} field, a {} {}\n", one.to_uppercase(), two[0], two[1]);
    let found: Vec<usize> = hits(&text).into_iter().map(|(line, _)| line).collect();
    assert_eq!(found, [2, 2]);
    assert!(hits("harvester refinery battle_tank hazard resource").is_empty());
}
