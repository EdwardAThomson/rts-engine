//! One computer-against-computer game, with a summary of what each side built, lost and destroyed, for balance work:
//!   cargo run --release --bin arena -- --setting <pack> --map <map> --seed 1 [--minutes 90] [--factions a,b]
//!     [--every 5] [--tech 1-8]
//! Every player goes to the computer. Prints a JSON line every `--every` game minutes (armies and bases) and one at
//! the end: the winner (null for a game still going), and for each player the units built, lost and still alive by
//! kind, and the value each of its kinds destroyed (damage dealt, as a share of the target's health, times the
//! target's cost). Nothing it prints feeds back into the game.

use std::collections::BTreeMap;

use classic_ai::{Ai, Settings};
use classic_sim::world::Event;
use classic_sim::{Game, GameOptions, Kind, Rules};
use classic_tools::setting;

#[derive(Default)]
struct Side {
    built: BTreeMap<String, u32>,
    lost: BTreeMap<String, u32>,
    /// Credits' worth of enemy destroyed, by the kind that did it.
    value: BTreeMap<String, i64>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1)).cloned();
    let num = |name: &str, default: u32| arg(name).map_or(default, |v| v.parse().expect("a whole number"));
    let seed = num("seed", 1) as i32;
    let minutes = num("minutes", 90);
    let every = num("every", 5).max(1) * 900;
    let pack = setting::load(&arg("setting").unwrap_or_else(|| "generic".into())).expect("a setting pack");
    let rules = Rules::from_table(&pack.rules).expect("pack rules match the simulation");
    let text = std::fs::read_to_string(arg("map").expect("--map")).expect("a map file");
    let mut game = Game::new(GameOptions { map: &text, seed, players: None, rules: Some(&rules) }).expect("valid map");
    if let Some(t) = arg("tech") {
        game.set_tech_level(Some(t.parse().expect("a tech level")));
    }
    if let Some(f) = arg("factions") {
        game.set_factions(&f.split(',').map(str::trim).collect::<Vec<_>>());
    }
    let players: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    let mut ais: Vec<Ai> = players.iter().map(|&p| Ai::new(p, Settings::normal())).collect();
    let mut sides: BTreeMap<u32, Side> = players.iter().map(|&p| (p, Side::default())).collect();
    // Every entity seen, by id: its kind and owner (an owner can change by capture or conversion; the last seen wins).
    let mut known: BTreeMap<u32, (Kind, u32)> = BTreeMap::new();
    let mut seen = 0;
    let mut winner = None;
    let waves = args.iter().any(|a| a == "--waves");
    let mut last_wave: Vec<Option<(u32, usize)>> = vec![None; ais.len()];
    let id = |k: Kind| game_kind_id(&rules, k);
    while game.state.tick < minutes * 900 && winner.is_none() {
        for ai in &mut ais {
            ai.tick(&mut game);
        }
        game.step(1);
        for e in &game.state.entities {
            known.insert(e.id, (e.kind, e.owner));
        }
        for ev in &game.events[seen..] {
            match *ev {
                Event::UnitBuilt { entity, kind, .. } => {
                    if let Some(s) = game.state.entity(entity).and_then(|e| sides.get_mut(&e.owner)) {
                        *s.built.entry(id(kind)).or_default() += 1;
                    }
                }
                Event::Destroyed { kind, owner, .. } if !rules.kind(kind).building => {
                    if let Some(s) = sides.get_mut(&owner) {
                        *s.lost.entry(id(kind)).or_default() += 1;
                    }
                }
                Event::Hit { target, attacker, damage, .. } => {
                    let (Some(&(tk, towner)), Some(&(ak, aowner))) = (known.get(&target), known.get(&attacker)) else {
                        continue;
                    };
                    let t = rules.kind(tk);
                    if aowner == towner || t.max_health == 0 || !sides.contains_key(&towner) {
                        continue;
                    }
                    let cost = if t.cost > 0 { t.cost } else { 100 };
                    if let Some(s) = sides.get_mut(&aowner) {
                        *s.value.entry(id(ak)).or_default() += damage * cost / t.max_health;
                    }
                }
                _ => {}
            }
        }
        seen = game.events.len();
        if waves {
            for (n, ai) in ais.iter().enumerate() {
                let now = ai.wave.as_ref().map(|w| (w.launched_at, w.units.len()));
                if now.map(|w| w.0) != last_wave[n].map(|w| w.0) {
                    if let Some((at, left)) = last_wave[n] {
                        println!(
                            "{{\"wave_end\":{},\"launched_at\":{at},\"left\":{left},\"tick\":{}}}",
                            ai.player, game.state.tick
                        );
                    }
                    if let Some(w) = &ai.wave {
                        let obj = game.state.entity(w.objective).map_or("?".into(), |e| rules.kind(e.kind).id.clone());
                        println!(
                            "{{\"wave\":{},\"tick\":{},\"units\":{},\"objective\":\"{obj}\",\"committed\":{}}}",
                            ai.player,
                            game.state.tick,
                            w.units.len(),
                            w.committed
                        );
                    }
                }
                last_wave[n] = now;
            }
        }
        winner = classic_ai::winner(&game);
        if game.state.tick.is_multiple_of(every) {
            println!("{}", sample(&game, &rules, &players));
        }
    }
    let mut out = format!(
        "{{\"seed\":{seed},\"ticks\":{},\"winner\":{},\"players\":[",
        game.state.tick,
        winner.map_or("null".into(), |w| w.to_string())
    );
    for (n, (&p, s)) in sides.iter().enumerate() {
        let alive = alive(&game, &rules, p);
        let pl = game.state.players.iter().find(|q| q.id == p).unwrap();
        out += &format!(
            "{}{{\"player\":{p},\"faction\":{:?},\"credits\":{},\"delivered\":{},\"built\":{},\"lost\":{},\"value\":{},\"alive\":{}}}",
            if n > 0 { "," } else { "" },
            pl.faction.clone().unwrap_or_default(),
            pl.credits,
            pl.delivered,
            map(&s.built),
            map(&s.lost),
            map(&s.value),
            map(&alive)
        );
    }
    out += &format!("],\"resource_left\":{},\"ai\":[", game.snapshot().resource_left);
    for (n, ai) in ais.iter().enumerate() {
        let wave = ai.wave.as_ref().map_or("null".to_string(), |w| {
            let obj = game.state.entity(w.objective).map_or("gone".to_string(), |e| {
                format!("{}@{},{} owner {}", rules.kind(e.kind).id, e.x / 256, e.y / 256, e.owner)
            });
            let orders: Vec<String> = w
                .units
                .iter()
                .filter_map(|&u| game.state.entity(u))
                .map(|e| format!("{}:{:?}:{}@{},{}", rules.kind(e.kind).id, e.order, e.target.map_or(-1, |t| t as i64), e.x / 256, e.y / 256))
                .take(6)
                .collect();
            format!(
                "{{\"units\":{},\"launched_with\":{},\"launched_at\":{},\"objective\":{:?},\"staging\":{},\"committed\":{},\"sample\":{:?}}}",
                w.units.len(), w.launched_with, w.launched_at, obj, w.staging.is_some(), w.committed, orders
            )
        });
        let theirs = || game.state.entities.iter().filter(|e| e.owner != ai.player && rules.kind(e.kind).building);
        let known = theirs().filter(|e| game.known(ai.player, e.id)).count();
        out += &format!(
            "{}{{\"player\":{},\"waves_sent\":{},\"wave_size\":{},\"known_buildings\":{known},\"enemy_buildings\":{},\"wave\":{}}}",
            if n > 0 { "," } else { "" },
            ai.player,
            ai.waves_sent,
            ai.wave_size,
            theirs().count(),
            wave
        );
    }
    out += "]}";
    println!("{out}");
}

fn game_kind_id(rules: &Rules, k: Kind) -> String {
    rules.kind(k).id.clone()
}

fn alive(game: &Game, rules: &Rules, p: u32) -> BTreeMap<String, i64> {
    let mut m = BTreeMap::new();
    for e in game.state.entities.iter().filter(|e| e.owner == p) {
        *m.entry(rules.kind(e.kind).id.clone()).or_default() += 1;
    }
    m
}

fn sample(game: &Game, rules: &Rules, players: &[u32]) -> String {
    let parts: Vec<String> = players
        .iter()
        .map(|&p| {
            let mine = || game.state.entities.iter().filter(move |e| e.owner == p);
            let armed = mine().filter(|e| !rules.kind(e.kind).building && rules.kind(e.kind).weapon.is_some());
            let (n, value) = armed.fold((0, 0), |(n, v), e| (n + 1, v + rules.kind(e.kind).cost));
            let buildings = mine().filter(|e| rules.kind(e.kind).building).count();
            let credits = game.state.players.iter().find(|q| q.id == p).map_or(0, |q| q.credits);
            format!("{{\"p\":{p},\"army\":{n},\"army_value\":{value},\"buildings\":{buildings},\"credits\":{credits}}}")
        })
        .collect();
    format!("{{\"tick\":{},\"sides\":[{}]}}", game.state.tick, parts.join(","))
}

fn map<V: std::fmt::Display>(m: &BTreeMap<String, V>) -> String {
    let parts: Vec<String> = m.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    format!("{{{}}}", parts.join(","))
}
