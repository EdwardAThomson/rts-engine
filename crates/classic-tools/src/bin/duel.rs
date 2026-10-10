//! Cost-for-cost fights between two kinds, for balance work:
//!   cargo run --release --bin duel -- [--setting <pack>] [--credits 3000] [--seeds 6] [--kinds a,b,c]
//! For every pair of armed kinds (units, and the armed buildings as defenders), each side gets as many as its credits
//! buy (at least one), set out in a block on open rock eight tiles from the other; units go for the nearest enemy
//! they can hit, buildings stand. Half the seeds swap the sides. Prints, for each pair, the wins of each, the
//! credits' worth each side has left (health as a share of cost) and the exchange: the credits' worth the row kind
//! destroyed per credit it lost (above 1 means it trades well). Nothing here is part of the simulation's rules.

use classic_data::{RulesTable, json};
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Order, Rules};
use classic_tools::setting;

const W: usize = 64;
const H: usize = 40;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1)).cloned();
    let credits: i64 = arg("credits").map_or(3000, |v| v.parse().expect("credits"));
    let seeds: i32 = arg("seeds").map_or(6, |v| v.parse().expect("seeds"));
    // The engine's own rules unless a pack is named; a pack's fog is switched off, so both sides see each other.
    let mut table = match arg("setting") {
        Some(name) => setting::load(&name).expect("a setting pack").rules,
        None => RulesTable::builtin(),
    };
    table.apply_tuning(&json::parse(r#"{"modules":{"fog":{"on":0}}}"#).expect("json"));
    let rules = Rules::from_table(&table).expect("rules match the simulation");
    let mut map = String::new();
    for y in 0..H {
        for x in 0..W {
            map.push(match (x, y) {
                (1, 1) => '1',
                (62, 1) => '2',
                _ => '#',
            });
        }
        map.push('\n');
    }
    let kinds: Vec<Kind> = match arg("kinds") {
        Some(list) => list.split(',').map(|id| rules.kind_id(id.trim()).expect("a kind")).collect(),
        None => (0..rules.kinds.len() as u16)
            .map(Kind)
            .filter(|&k| {
                let r = rules.kind(k);
                r.weapon.is_some() && r.harvester.is_none() && r.cost > 0 && (!r.building || r.width == 1)
            })
            .collect(),
    };
    let name = |k: Kind| rules.kind(k).id.clone();
    println!(
        "{{\"credits\":{credits},\"seeds\":{seeds},\"kinds\":{:?}}}",
        kinds.iter().map(|&k| name(k)).collect::<Vec<_>>()
    );
    for (i, &a) in kinds.iter().enumerate() {
        for &b in &kinds[i..] {
            if rules.kind(a).building && rules.kind(b).building {
                continue;
            }
            let (mut wins, mut left, mut destroyed) = ([0; 2], [0i64; 2], [0i64; 2]);
            for seed in 0..seeds {
                let swap = seed % 2 == 1;
                let r = fight(&map, &rules, if swap { [b, a] } else { [a, b] }, credits, seed + 1);
                let (w, l, d) = if swap { (r.0.map(|w| 1 - w), [r.1[1], r.1[0]], [r.2[1], r.2[0]]) } else { r };
                if let Some(w) = w {
                    wins[w] += 1;
                }
                (left[0], left[1]) = (left[0] + l[0], left[1] + l[1]);
                (destroyed[0], destroyed[1]) = (destroyed[0] + d[0], destroyed[1] + d[1]);
            }
            // What each side lost is what the other destroyed.
            let exchange = |me: usize| destroyed[me] * 100 / destroyed[1 - me].max(1);
            println!(
                "{{\"a\":\"{}\",\"b\":\"{}\",\"wins\":[{},{}],\"left\":[{},{}],\"exchange\":[{},{}]}}",
                name(a),
                name(b),
                wins[0],
                wins[1],
                left[0] / seeds as i64,
                left[1] / seeds as i64,
                exchange(0),
                exchange(1)
            );
        }
    }
}

/// One fight: the winner (0 or 1, none for a draw at the time limit), each side's worth left, and the worth each
/// side destroyed.
fn fight(map: &str, rules: &Rules, kinds: [Kind; 2], credits: i64, seed: i32) -> (Option<usize>, [i64; 2], [i64; 2]) {
    let mut game = Game::new(GameOptions { map, seed, players: None, rules: Some(rules) }).expect("valid map");
    // The starting bases sit in the top corners: clear them away so only the two armies are on the map.
    let ids: Vec<u32> = game.state.entities.iter().map(|e| e.id).collect();
    for id in ids {
        if let Some(e) = game.state.entities.iter_mut().find(|e| e.id == id) {
            e.health = 0;
        }
    }
    game.step(1);
    // A power plant each, far off, for the weapons that need power.
    if let Some(plant) = rules.kind_id("power_plant") {
        game.spawn(plant, 0, 2, 34);
        game.spawn(plant, 1, 60, 34);
    }
    let mut sides: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
    for (s, &k) in kinds.iter().enumerate() {
        let n = (credits / rules.kind(k).cost.max(1)).max(1) as i32;
        let cols = if rules.kind(k).building { 4 } else { 6 };
        for j in 0..n {
            let (row, col) = (j / cols, j % cols);
            let step = if rules.kind(k).building { 2 } else { 1 };
            let x = if s == 0 { 26 - col * step } else { 37 + col * step };
            let y = 14 + row * step;
            sides[s].push(game.spawn(k, s as u32, x, y));
        }
    }
    let worth = |g: &Game, s: usize| -> i64 {
        sides[s]
            .iter()
            .filter_map(|&id| g.state.entity(id))
            .filter(|e| e.owner == s as u32)
            .map(|e| rules.kind(e.kind).cost * e.health / rules.kind(e.kind).max_health.max(1))
            .sum()
    };
    let start = [worth(&game, 0), worth(&game, 1)];
    for t in 0..15 * 60 * 5 {
        if t % 15 == 0 {
            for (s, side) in sides.iter().enumerate() {
                let enemies: Vec<(u32, i64, i64, bool)> = game
                    .state
                    .entities
                    .iter()
                    .filter(|e| e.owner != s as u32 && e.owner < 2)
                    .map(|e| (e.id, e.x, e.y, e.airborne()))
                    .collect();
                for &id in side {
                    let Some(e) = game.state.entity(id).filter(|e| e.owner == s as u32) else { continue };
                    let k = rules.kind(e.kind);
                    if k.building || e.order == Order::Attack {
                        continue;
                    }
                    let Some(w) = k.weapon.map(|w| rules.weapon(w)) else { continue };
                    let near = enemies
                        .iter()
                        .filter(|t| if t.3 { w.hits_air } else { w.hits_ground })
                        .min_by_key(|t| ((t.1 - e.x).pow(2) + (t.2 - e.y).pow(2), t.0));
                    if let Some(t) = near {
                        game.order(s as u32, &[id], CommandOrder::Attack { target: t.0 });
                    }
                }
            }
        }
        game.step(1);
        let now = [worth(&game, 0), worth(&game, 1)];
        if now[0] == 0 || now[1] == 0 {
            let w = if now[0] > 0 {
                Some(0)
            } else if now[1] > 0 {
                Some(1)
            } else {
                None
            };
            return (w, now, [start[1] - now[1], start[0] - now[0]]);
        }
        // Neither side able to hurt the other any more (aircraft over ground-only guns, say): a draw.
        if t > 15 * 60 && now == start {
            break;
        }
    }
    let now = [worth(&game, 0), worth(&game, 1)];
    (None, now, [start[1] - now[1], start[0] - now[0]])
}
