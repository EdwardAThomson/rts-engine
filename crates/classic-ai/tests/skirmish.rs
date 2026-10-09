//! The computer opponent on the skirmish map. Each test prints what it ran (`cargo test -p classic-ai --
//! --nocapture`), so a pass can't come from a check that silently did nothing.

use classic_ai::{Ai, Settings, defeated, winner};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions};

const SKIRMISH: &str = include_str!("../../../maps/skirmish-01.txt");

fn game(seed: i32) -> Game {
    Game::new(GameOptions { map: SKIRMISH, seed, players: None, rules: None }).expect("skirmish map is valid")
}

/// Run up to `ticks` with these AIs, stopping early when one player is left. Returns the winner and the tick.
fn play(game: &mut Game, ais: &mut [Ai], ticks: u32) -> Option<(u32, u32)> {
    for _ in 0..ticks {
        for ai in ais.iter_mut() {
            ai.tick(game);
        }
        game.step(1);
        if let Some(w) = winner(game) {
            return Some((w, game.state.tick));
        }
    }
    None
}

fn count(game: &Game, player: u32, id: &str) -> usize {
    let k = game.kind(id);
    game.state.entities.iter().filter(|e| e.owner == player && Some(e.kind) == k).count()
}

fn rejected(game: &Game) -> usize {
    game.events
        .iter()
        .filter(|e| matches!(e, Event::PlacementRejected { .. } | Event::ProductionRejected { .. }))
        .count()
}

/// How many of each unit kind a player's factories still standing have built, by generic id, in id order.
fn units_built(game: &Game, player: u32) -> Vec<(String, usize)> {
    let mut built = std::collections::BTreeMap::new();
    for e in &game.events {
        if let Event::UnitBuilt { kind, factory, .. } = e
            && game.state.entity(*factory).is_some_and(|f| f.owner == player)
        {
            *built.entry(game.rules.kind(*kind).id.clone()).or_insert(0) += 1;
        }
    }
    built.into_iter().collect()
}

#[test]
fn builds_a_base_harvests_and_produces_an_army() {
    let mut g = game(1);
    let mut ai = [Ai::new(1, Settings::normal())];
    play(&mut g, &mut ai, 7000);
    let ids =
        ["power_plant", "refinery", "barracks", "light_factory", "heavy_factory", "radar", "gun_turret", "harvester"];
    let have: Vec<(&str, usize)> = ids.iter().map(|&id| (id, count(&g, 1, id))).collect();
    let built = units_built(&g, 1);
    let fighters: usize = built.iter().filter(|(id, _)| id != "harvester").map(|(_, n)| n).sum();
    let delivered = g.state.players[1].delivered;
    println!("after 7000 ticks: {have:?}, units built {built:?}, delivered {delivered}, rejected {}", rejected(&g));
    assert!(have.iter().all(|&(_, n)| n >= 1), "it has one of each");
    assert!(count(&g, 1, "refinery") >= 2 && count(&g, 1, "harvester") >= 3, "a second refinery and its harvesters");
    assert!(fighters >= 2 && delivered > 3000);
    assert_eq!(rejected(&g), 0, "the AI only asks for what the rules allow");
}

#[test]
fn its_army_mixes_infantry_light_vehicles_and_tanks() {
    // It never attacks here, so the game runs the whole time and the army is all it has bought.
    let mut g = game(2);
    let mut ai = [Ai::new(0, Settings { first_wave_tick: u32::MAX, ..Settings::normal() })];
    play(&mut g, &mut ai, 15_000);
    let built = units_built(&g, 0);
    println!("units built in 15000 ticks, never attacking: {built:?}");
    let n = |id: &str| built.iter().find(|(b, _)| b == id).map_or(0, |&(_, n)| n);
    let kinds = [
        "infantry",
        "infantry_squad",
        "rocket_infantry",
        "rocket_squad",
        "scout_bike",
        "quad",
        "battle_tank",
        "siege_tank",
        "missile_tank",
    ];
    for id in kinds {
        assert!(n(id) >= 1, "it built a {id}");
    }
    // Weighted 6 to 3 to 2 to 1 (Settings::normal): battle tanks lead, but the dearest kinds aren't all it buys.
    assert!(kinds.iter().all(|&id| n(id) <= n("battle_tank")));
    let heavy = n("battle_tank") + n("siege_tank") + n("missile_tank");
    assert!(heavy < built.iter().filter(|(id, _)| id != "harvester").map(|(_, n)| n).sum::<usize>());
}

/// With a weight of 0 a kind is never built, and with the mix left empty every armed unit weighs 1.
#[test]
fn the_mix_comes_from_settings_and_the_rules() {
    let only_tanks: Vec<(String, usize)> = [
        "infantry",
        "infantry_squad",
        "rocket_infantry",
        "rocket_squad",
        "scout_bike",
        "quad",
        "siege_tank",
        "missile_tank",
    ]
    .iter()
    .map(|id| (id.to_string(), 0))
    .collect();
    let mut g = game(2);
    let mut ai = [Ai::new(0, Settings { unit_mix: only_tanks, ..Settings::normal() })];
    play(&mut g, &mut ai, 9000);
    let tanks_only = units_built(&g, 0);
    let mut g = game(2);
    let mut ai = [Ai::new(0, Settings { unit_mix: Vec::new(), ..Settings::normal() })];
    play(&mut g, &mut ai, 9000);
    let even = units_built(&g, 0);
    println!("weights 0 for all but tanks: {tanks_only:?}; no list, every armed unit 1: {even:?}");
    assert!(tanks_only.iter().all(|(id, _)| id == "battle_tank" || id == "harvester"));
    assert!(even.iter().filter(|(id, _)| id != "harvester").count() >= 4);
}

#[test]
fn defeats_a_player_who_does_nothing() {
    for (ai_player, seed) in [(0, 1), (1, 2)] {
        let mut g = game(seed);
        let mut ai = [Ai::new(ai_player, Settings::normal())];
        let end = play(&mut g, &mut ai, 20_000);
        println!("AI as player {ai_player}, seed {seed}: winner and tick {end:?}, waves {}", ai[0].waves_sent);
        assert_eq!(end.map(|(w, _)| w), Some(ai_player));
        assert!(defeated(&g, 1 - ai_player));
        assert!(end.is_some_and(|(_, t)| t >= ai[0].settings.first_wave_tick), "no attack before the first-wave time");
    }
}

#[test]
fn two_ais_play_the_same_game_every_time() {
    let run = || {
        let mut g = game(3);
        let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
        let end = play(&mut g, &mut ais, 20_000);
        let waves: Vec<u32> = ais.iter().map(|a| a.waves_sent).collect();
        (g.hash(), g.state.tick, g.command_log().len(), end, waves, rejected(&g))
    };
    let (a, b) = (run(), run());
    println!("AI against AI, seed 3: {a:?}");
    assert_eq!(a, b);
    assert!(a.4.iter().all(|&w| w >= 1), "both sides attacked");
    assert_eq!(a.5, 0);
}

#[test]
fn a_game_between_ais_replays_from_the_command_log_with_the_ai_switched_off() {
    let mut live = game(4);
    let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
    play(&mut live, &mut ais, 12_000);
    let log = live.command_log().to_vec();
    let mut replay = game(4);
    let mut next = 0;
    while replay.state.tick < live.state.tick {
        while next < log.len() && log[next].tick == replay.state.tick {
            let c = &log[next].command;
            replay.order(c.player, &c.ids, c.order);
            next += 1;
        }
        replay.step(1);
    }
    println!("{} AI commands over {} ticks: live {} replay {}", log.len(), live.state.tick, live.hash(), replay.hash());
    assert!(log.len() > 100);
    assert_eq!(replay.hash(), live.hash());
}

#[test]
fn thinking_reads_the_game_and_changes_nothing() {
    let mut g = game(5);
    let mut ai = Ai::new(0, Settings::normal());
    let mut orders = 0;
    for _ in 0..3000 {
        let before = g.hash();
        let cmds = ai.think(&g);
        assert_eq!(g.hash(), before);
        orders += cmds.len();
        for c in cmds {
            assert_eq!(c.player, 0, "it orders only for itself");
            g.order(c.player, &c.ids, c.order);
        }
        g.step(1);
    }
    println!("{orders} orders over 3000 thinks, the hash unchanged by every think");
    assert!(orders > 0);
}

#[test]
fn rebuilds_a_lost_power_plant() {
    let mut g = game(6);
    let mut ai = [Ai::new(0, Settings::normal())];
    play(&mut g, &mut ai, 3000);
    let plants = count(&g, 0, "power_plant");
    // Knock one down, as a test harness may: remove it and clear its footprint.
    let plant = g.kind("power_plant");
    let i = g.state.entities.iter().position(|e| e.owner == 0 && Some(e.kind) == plant).expect("it has a plant");
    let e = g.state.entities.remove(i);
    let t = e.tile();
    g.pathfinder.set_blocked(t.x, t.y, 2, 2, false);
    play(&mut g, &mut ai, 15 * 90);
    println!("plants: {plants}, one removed at tick 3000, {} 90 seconds later", count(&g, 0, "power_plant"));
    assert_eq!(count(&g, 0, "power_plant"), plants);
}

#[test]
fn defends_its_base_against_a_raider() {
    let mut g = game(7);
    let mut ai = [Ai::new(0, Settings::normal())];
    play(&mut g, &mut ai, 3000);
    let tank = g.kind("battle_tank").expect("rules have tanks");
    let yard = g.state.entities.iter().find(|e| e.owner == 0 && Some(e.kind) == g.kind("construction_yard")).unwrap();
    let raider = g.spawn(tank, 1, yard.tile().x + 8, yard.tile().y + 12);
    let from = g.command_log().len();
    play(&mut g, &mut ai, 15 * 20);
    let sent = g.command_log()[from..]
        .iter()
        .filter(|c| c.command.player == 0 && c.command.order == CommandOrder::Attack { target: raider })
        .count();
    let hits = g.events.iter().filter(|e| matches!(e, Event::Hit { target, .. } if *target == raider)).count();
    println!("a raider beside the base: {sent} attack orders on it, hit {hits} times in 20 s");
    assert!(sent >= 1);
    assert!(hits >= 1);
}

/// Player 0's computer opponent with the first wave allowed at once, three tanks at home, and player 1's harvester
/// gone (so there is nothing to raid), with or without a line of gun turrets in front of player 1's base.
fn odds(turrets: bool) -> (Game, [Ai; 1]) {
    let mut g = game(8);
    let tank = g.kind("battle_tank").unwrap();
    for x in 6..9 {
        g.spawn(tank, 0, x, 15);
    }
    if turrets {
        let gun = g.kind("gun_turret").unwrap();
        for x in 53..61 {
            g.spawn(gun, 1, x, 29);
        }
    }
    g.state.entities.retain(|e| !(e.owner == 1 && g.rules.kind(e.kind).harvester.is_some()));
    let settings = Settings { first_wave_tick: 0, first_wave: 3, ..Settings::normal() };
    (g, [Ai::new(0, settings)])
}

#[test]
fn a_wave_waits_until_it_would_beat_the_defenders() {
    let (mut g, mut ai) = odds(true);
    play(&mut g, &mut ai, 1500);
    let tanks = count(&g, 0, "battle_tank");
    println!("against eight turrets: {} waves in 1500 ticks, {tanks} tanks kept at home", ai[0].waves_sent);
    assert_eq!(ai[0].waves_sent, 0);
    assert!(tanks >= 4, "none thrown away");

    let (mut g, mut ai) = odds(false);
    play(&mut g, &mut ai, 1500);
    println!("against no turrets: {} waves in 1500 ticks", ai[0].waves_sent);
    assert!(ai[0].waves_sent >= 1);
}

#[test]
fn with_its_income_gone_it_sends_every_unit() {
    let mut g = game(9);
    g.state.resource.iter_mut().for_each(|r| *r = 0);
    g.state.players[0].credits = 0;
    let tank = g.kind("battle_tank").unwrap();
    for x in 6..9 {
        g.spawn(tank, 0, x, 15);
    }
    // Ten units would be its first wave; with no income it stops waiting for them.
    let settings = Settings { first_wave_tick: 0, first_wave: 10, ..Settings::normal() };
    let broke = settings.broke_ticks;
    let mut ai = [Ai::new(0, settings)];
    play(&mut g, &mut ai, broke - 30);
    assert_eq!(ai[0].waves_sent, 0, "not before its income has stopped for broke_ticks");
    play(&mut g, &mut ai, 300);
    let sent = ai[0].wave.as_ref().map_or(0, |w| w.launched_with);
    println!("no income: a wave of {sent} once broke_ticks passed");
    assert_eq!(ai[0].waves_sent, 1);
    assert_eq!(sent, 4, "the starting tank and the three more");
}

#[test]
fn with_its_income_gone_it_spends_what_is_left_on_units_it_can_pay_for() {
    // Normally it keeps `unit_reserve` credits back so the base can grow. With nothing coming in there is nothing to
    // save for: the last 250 credits buy a unit that costs no more than that, never one it could only half pay for.
    let mut g = game(4);
    let mut ai = [Ai::new(0, Settings::normal())];
    play(&mut g, &mut ai, 7000);
    g.state.resource.iter_mut().for_each(|r| *r = 0);
    let broke = ai[0].settings.broke_ticks;
    // Not `play`, which stops once the idle player is beaten: the income has to stay gone for `broke_ticks`.
    for _ in 0..broke + 900 {
        ai[0].tick(&mut g);
        g.step(1);
    }
    for e in g.state.entities.iter_mut().filter(|e| e.owner == 0) {
        e.queue.clear();
    }
    g.state.players[0].credits = 250;
    let cost = |k: classic_sim::Kind| g.rules.kind(k).cost;
    let bought: Vec<(String, i64)> = ai[0]
        .think(&g)
        .into_iter()
        .filter_map(|c| match c.order {
            CommandOrder::Produce { kind } if !g.rules.kind(kind).building => {
                Some((g.rules.kind(kind).id.clone(), cost(kind)))
            }
            _ => None,
        })
        .collect();
    println!("no income, 250 credits left, reserve {}: queued {bought:?}", ai[0].settings.unit_reserve);
    assert!(!bought.is_empty());
    assert!(bought.iter().map(|b| b.1).sum::<i64>() <= 250);
}

#[test]
fn with_no_harvester_left_it_cancels_other_work_to_make_one() {
    let mut g = game(10);
    let mut ai = [Ai::new(0, Settings::normal())];
    play(&mut g, &mut ai, 3000);
    // Lose every harvester while the factories are full of tanks that can't be paid for.
    g.state.entities.retain(|e| !(e.owner == 0 && g.rules.kind(e.kind).harvester.is_some()));
    let tank = g.kind("battle_tank").unwrap();
    let heavy = g.kind("heavy_factory").unwrap();
    let factories: Vec<u32> =
        g.state.entities.iter().filter(|e| e.owner == 0 && e.kind == heavy).map(|e| e.id).collect();
    for &f in &factories {
        for _ in 0..2 {
            g.order(0, &[f], CommandOrder::Produce { kind: tank });
        }
    }
    g.step(1);
    g.state.players[0].credits = 0;
    play(&mut g, &mut ai, 15 * 60);
    let cancelled = g.events.iter().filter(|e| matches!(e, Event::ProductionCancelled { .. })).count();
    println!(
        "{} heavy factories full of tanks, no credits: {cancelled} cancelled, {} harvesters a minute later",
        factories.len(),
        count(&g, 0, "harvester")
    );
    assert!(!factories.is_empty() && cancelled >= 1);
    assert!(count(&g, 0, "harvester") >= 1);
}

#[test]
fn two_ais_on_a_mirrored_map_play_mirror_images_of_each_other() {
    use classic_sim::map::TILE;
    let mut rules = classic_sim::Rules::default();
    rules.movement.wait_random = 1;
    rules.regrowth.every_ticks = u32::MAX;
    for w in &mut rules.weapons {
        w.scatter = 0;
    }
    let map = include_str!("../../../maps/mirror-01.txt");
    let mut g = Game::new(GameOptions { map, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
    let w = g.map.width as i64 * TILE;
    let side = |g: &Game, p: u32| {
        let mut v: Vec<(String, i64, i64, i64)> = g
            .state
            .entities
            .iter()
            .filter(|e| e.owner == p)
            .map(|e| {
                let k = g.rules.kind(e.kind);
                let x = e.x - TILE / 2 + k.width as i64 * TILE / 2;
                (k.id.clone(), if p == 0 { w - x } else { x }, e.y, e.health)
            })
            .collect();
        v.sort();
        v
    };
    // Until units of the two sides meet. Two mirrored units meeting head-on can't stay mirror images: movement goes
    // in id order, so one of them finds its way round first.
    let met = |g: &Game| {
        let units: Vec<&classic_sim::Entity> =
            g.state.entities.iter().filter(|e| !g.rules.kind(e.kind).building).collect();
        units
            .iter()
            .any(|a| units.iter().any(|b| a.owner != b.owner && (a.x - b.x).abs().max((a.y - b.y).abs()) <= 3 * TILE))
    };
    while g.state.tick < 9000 && !met(&g) {
        assert_eq!(side(&g, 0), side(&g, 1), "tick {}", g.state.tick);
        play(&mut g, &mut ais, 1);
    }
    let waves: Vec<u32> = ais.iter().map(|a| a.waves_sent).collect();
    println!(
        "with chance taken out: mirror images until the two sides met at tick {}, {} entities each, waves {waves:?}",
        g.state.tick,
        side(&g, 0).len()
    );
    assert!(side(&g, 0).len() > 10);
    assert!(waves.iter().all(|&w| w >= 1), "both sides' first waves went out as mirror images");
}

#[test]
fn damaged_units_go_out_with_the_next_wave() {
    // There is no repair yet, so a unit hurt in one fight stays hurt. Kept at home, hurt units piled up there while
    // waves went out without them.
    let (mut g, mut ai) = odds(false);
    (ai[0].settings.first_wave, ai[0].wave_size) = (4, 4);
    let tank = g.kind("battle_tank").unwrap();
    let hurt: Vec<u32> = g.state.entities.iter().filter(|e| e.owner == 0 && e.kind == tank).map(|e| e.id).collect();
    for e in g.state.entities.iter_mut().filter(|e| hurt.contains(&e.id)) {
        e.health /= 3;
    }
    while ai[0].waves_sent == 0 && g.state.tick < 1500 {
        play(&mut g, &mut ai, 1);
    }
    let units = ai[0].wave.as_ref().map(|w| w.units.clone()).unwrap_or_default();
    println!("{} tanks at a third of their health; the first wave, at tick {}: {units:?}", hurt.len(), g.state.tick);
    assert!(hurt.len() >= 4);
    assert!(hurt.iter().all(|id| units.contains(id)), "every hurt tank went");
}

#[test]
fn a_wave_that_turns_back_makes_the_next_one_bigger() {
    let (mut g, mut ai) = odds(false);
    while !ai[0].wave.as_ref().is_some_and(|w| w.staging.is_none()) && g.state.tick < 3000 {
        play(&mut g, &mut ai, 1);
    }
    let wave = ai[0].wave.clone().expect("a wave set out");
    let size = ai[0].wave_size;
    // A strong enemy force turns up beside it.
    let lead = g.state.entity(wave.units[0]).unwrap().tile();
    let tank = g.kind("battle_tank").unwrap();
    for i in 0..12 {
        g.spawn(tank, 1, lead.x + 6 + i % 3, lead.y - 1 + i / 3);
    }
    let think = ai[0].settings.think_every;
    play(&mut g, &mut ai, think);
    let alive = wave.units.iter().filter(|&&id| g.state.entity(id).is_some()).count();
    println!(
        "a wave of {} met twelve tanks: {alive} alive, wave out {}, next wave {} (was {size})",
        wave.units.len(),
        ai[0].wave.is_some(),
        ai[0].wave_size
    );
    assert!(ai[0].wave.is_none(), "turned back");
    assert!(alive * 100 >= wave.launched_with * 30, "while it still could, not after losing it");
    assert_eq!(ai[0].wave_size, size + Settings::normal().wave_growth);
}

#[test]
fn never_shuts_its_own_units_in_with_buildings() {
    // Every ground unit of either side can still reach the edge of the map after an AI game's first ten minutes.
    // A start's units stand among its buildings, and before, a turret could close the last gap round them: the
    // two units shut in were often all that side had left at the end, and the game could never finish.
    let map = include_str!("../../../maps/mirror-01.txt");
    for seed in 1..=3 {
        let mut g = Game::new(GameOptions { map, seed, players: None, rules: None }).unwrap();
        let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
        play(&mut g, &mut ais, 9000);
        let (w, h) = (g.map.width, g.map.height);
        let edge: Vec<(i32, i32)> = (0..w)
            .flat_map(|x| [(x, 0), (x, h - 1)])
            .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]))
            .filter(|&(x, y)| g.pathfinder.passable(x, y))
            .collect();
        let shut: Vec<u32> = g
            .state
            .entities
            .iter()
            .filter(|e| !g.rules.kind(e.kind).building)
            .filter(|e| !edge.iter().any(|&t| g.pathfinder.connected((e.tile().x, e.tile().y), t)))
            .map(|e| e.id)
            .collect();
        let units = g.state.entities.iter().filter(|e| !g.rules.kind(e.kind).building).count();
        println!("seed {seed}: {units} units after ten minutes, shut in: {shut:?}");
        assert!(shut.is_empty());
    }
}
