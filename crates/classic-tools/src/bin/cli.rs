//! Headless run from the command line:
//!   cargo run --release --bin cli -- [--map maps/test-01.txt] [--seed 1] [--ticks 9000] [--every 1500]
//! Prints one JSON line every --every ticks and the event counts at the end. No window, no graphics.

use std::collections::HashMap;
use std::time::Instant;

use classic_sim::{Game, GameOptions, UnitType};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1)).cloned();
    let map_path = arg("map").unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../maps/test-01.txt").into());
    let num = |name: &str, default: u32| arg(name).map_or(default, |v| v.parse().expect("a whole number"));
    let seed = num("seed", 1) as i32;
    let ticks = num("ticks", 9000);
    let every = num("every", 1500).max(1);

    let text = std::fs::read_to_string(&map_path).unwrap_or_else(|e| panic!("{map_path}: {e}"));
    let mut game = Game::new(GameOptions { map: &text, seed, players: None }).expect("valid map");
    let t0 = Instant::now();
    let mut t = 0;
    while t < ticks {
        game.step(every.min(ticks - t));
        t += every;
        let s = game.snapshot();
        let credits: Vec<String> = s.players.iter().map(|p| p.credits.to_string()).collect();
        let harvesters: Vec<String> = s
            .entities
            .iter()
            .filter(|e| e.kind == UnitType::Harvester)
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
        "{{\"seed\":{seed},\"ticks\":{},\"entities\":{},\"events\":{{{}}},\"ms\":{}}}",
        game.state.tick,
        game.state.entities.len(),
        events.join(","),
        t0.elapsed().as_millis()
    );
}
