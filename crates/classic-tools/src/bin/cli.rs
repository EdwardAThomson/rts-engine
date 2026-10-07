//! Headless run from the command line:
//!   cargo run --release --bin cli -- [--setting generic] [--map maps/test-01.txt] [--seed 1] [--ticks 9000] [--every 1500]
//!     [--ai 0,1]
//! Prints the setting pack in use, one JSON line every --every ticks and the event counts at the end. No window, no
//! graphics. `--ai` hands the listed players (0 is the map's start 1) to the computer opponent; the run then stops
//! early when one player is left, and the last line names the winner. `--setting` (or the SETTING environment variable) takes a pack folder, a name under `settings/`, or
//! a pack in the git-ignored `settings-private/` by its folder name under `packs/` or its id (`private` picks the
//! first); the default is `generic`.

use std::collections::HashMap;
use std::time::Instant;

use classic_ai::{Ai, Settings};
use classic_sim::{Game, GameOptions, Rules};
use classic_tools::setting;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1)).cloned();
    let map_path = arg("map").unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../maps/test-01.txt").into());
    let num = |name: &str, default: u32| arg(name).map_or(default, |v| v.parse().expect("a whole number"));
    let seed = num("seed", 1) as i32;
    let ticks = num("ticks", 9000);
    let every = num("every", 1500).max(1);
    let mut ais: Vec<Ai> = arg("ai")
        .map(|v| {
            v.split(',').map(|p| Ai::new(p.trim().parse().expect("a player number"), Settings::normal())).collect()
        })
        .unwrap_or_default();

    let setting_name = arg("setting").or_else(|| std::env::var("SETTING").ok()).unwrap_or_else(|| "generic".into());
    let pack = setting::load(&setting_name).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    for w in &pack.warnings {
        eprintln!("warning: {w}");
    }
    let rules = Rules::from_table(&pack.rules).expect("pack rules match the simulation");
    let names: Vec<String> =
        rules.kinds.iter().map(|k| format!("\"{}\":{}", k.id, json_string(pack.name(&k.id)))).collect();
    println!(
        "{{\"setting\":{},\"title\":{},\"rules\":\"{}\",\"names\":{{{}}}}}",
        json_string(&pack.id),
        json_string(&pack.title),
        rules.hash,
        names.join(",")
    );

    let text = std::fs::read_to_string(&map_path).unwrap_or_else(|e| panic!("{map_path}: {e}"));
    let mut game = Game::new(GameOptions { map: &text, seed, players: None, rules: Some(&rules) }).expect("valid map");
    let t0 = Instant::now();
    let mut t = 0;
    let mut winner = None;
    while t < ticks && winner.is_none() {
        for _ in 0..every.min(ticks - t) {
            for ai in &mut ais {
                ai.tick(&mut game);
            }
            game.step(1);
            if !ais.is_empty()
                && let Some(w) = classic_ai::winner(&game)
            {
                winner = Some(w);
                break;
            }
        }
        t += every;
        let s = game.snapshot();
        let credits: Vec<String> = s.players.iter().map(|p| p.credits.to_string()).collect();
        let harvesters: Vec<String> = s
            .entities
            .iter()
            .filter(|e| game.rules.kind(e.kind).harvester.is_some())
            .map(|e| format!("\"{}:{}:{}\"", e.id, e.task.map_or("undefined", |t| t.id()), e.cargo.unwrap_or(0)))
            .collect();
        println!(
            "{{\"tick\":{},\"credits\":[{}],\"resourceLeft\":{},\"harvesters\":[{}],\"hash\":\"{}\"}}",
            s.tick,
            credits.join(","),
            s.resource_left,
            harvesters.join(","),
            game.hash()
        );
    }
    // Event counts, in the order each kind first happened. Only for printing; the sim never sees this map.
    let mut order: Vec<&str> = Vec::new();
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for e in &game.events {
        let c = counts.entry(e.name()).or_insert(0);
        if *c == 0 {
            order.push(e.name());
        }
        *c += 1;
    }
    let events: Vec<String> = order.iter().map(|k| format!("\"{k}\":{}", counts[k])).collect();
    println!(
        "{{\"seed\":{seed},\"ticks\":{},\"entities\":{},\"winner\":{},\"events\":{{{}}},\"ms\":{}}}",
        game.state.tick,
        game.state.entities.len(),
        winner.map_or("null".into(), |w| w.to_string()),
        events.join(","),
        t0.elapsed().as_millis()
    );
}

/// A string as a JSON literal, for the names a pack supplies.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
