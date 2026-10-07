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

#[test]
fn builds_a_base_harvests_and_produces_an_army() {
    let mut g = game(1);
    let mut ai = [Ai::new(1, Settings::normal())];
    play(&mut g, &mut ai, 7000);
    let ids = ["power_plant", "refinery", "light_factory", "heavy_factory", "radar", "gun_turret", "harvester"];
    let have: Vec<(&str, usize)> = ids.iter().map(|&id| (id, count(&g, 1, id))).collect();
    let tanks_built = g
        .events
        .iter()
        .filter(|e| matches!(e, Event::UnitBuilt { kind, .. } if Some(*kind) == g.kind("battle_tank")))
        .count();
    let delivered = g.state.players[1].delivered;
    println!("after 7000 ticks: {have:?}, tanks built {tanks_built}, delivered {delivered}, rejected {}", rejected(&g));
    assert!(have.iter().all(|&(_, n)| n >= 1), "it has one of each");
    assert!(count(&g, 1, "refinery") >= 2 && count(&g, 1, "harvester") >= 3, "a second refinery and its harvesters");
    assert!(tanks_built >= 2 && delivered > 3000);
    assert_eq!(rejected(&g), 0, "the AI only asks for what the rules allow");
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
