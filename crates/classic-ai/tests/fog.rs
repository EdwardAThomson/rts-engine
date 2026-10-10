//! The computer opponent under fog of war: it reads only what its side can see, so it has to find the enemy first.
//! Each test prints what it ran (`cargo test -p classic-ai --test fog -- --nocapture`).

use classic_ai::{Ai, Settings, defeated, winner};
use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{Game, GameOptions, Rules, TileView};

const SKIRMISH: &str = include_str!("../../../maps/skirmish-01.txt");

/// The engine's rules with fog on, then `tuning`.
fn rules(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    t.modules.get_mut("fog").unwrap().numbers.get_mut("on").unwrap().value = 1;
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn game(rules: &Rules, seed: i32) -> Game {
    Game::new(GameOptions { map: SKIRMISH, seed, players: None, rules: Some(rules) }).expect("skirmish map is valid")
}

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

#[test]
fn it_scouts_finds_and_defeats_a_player_who_does_nothing() {
    for hide in [1, 0] {
        let r = rules(&format!(r#"{{ "modules": {{ "fog": {{ "hide": {hide} }} }} }}"#));
        let mut g = game(&r, 1);
        let other = g.map.start[0].unwrap();
        assert_eq!(g.tile_view(1, other.x, other.y), TileView::Shroud, "it starts not knowing where the enemy is");
        let mut ai = [Ai::new(1, Settings::normal())];
        let end = play(&mut g, &mut ai, 25_000);
        println!("fog hide {hide}: winner and tick {end:?}, waves {}", ai[0].waves_sent);
        assert_eq!(end.map(|(w, _)| w), Some(1));
        assert!(defeated(&g, 0));
    }
}

#[test]
fn it_ends_a_wave_whose_target_slipped_out_of_sight_and_searches_for_buildings_it_never_saw() {
    for hide in [1, 0] {
        let r = rules(&format!(r#"{{ "modules": {{ "fog": {{ "hide": {hide} }} }} }}"#));
        let mut g = game(&r, 1);
        let plant = g.kind("power_plant").unwrap();
        let mut ai = [Ai::new(1, Settings::normal())];
        // When its last building falls, the enemy gets one more in a corner it has never been near: the game goes on
        // only until it finds that one too.
        let (mut far, mut unseen) = (None, false);
        let mut end = None;
        while g.state.tick < 40_000 && end.is_none() {
            ai[0].tick(&mut g);
            g.step(1);
            let buildings = g.state.entities.iter().filter(|e| e.owner == 0 && g.rules.kind(e.kind).building).count();
            if far.is_none() && buildings == 0 {
                unseen = g.tile_view(1, 2, 36) == TileView::Shroud;
                far = Some(g.spawn(plant, 0, 2, 36));
            }
            end = winner(&g).map(|w| (w, g.state.tick));
        }
        println!("fog hide {hide}: outlying plant {far:?} unseen when built: {unseen}; winner and tick {end:?}");
        assert!(far.is_some() && unseen, "the outlying plant came after the base fell, on ground never seen");
        assert_eq!(end.map(|(w, _)| w), Some(1), "it searched the map, found the plant and destroyed it");
    }
}

#[test]
fn it_never_orders_an_attack_on_what_it_cannot_see() {
    let r = rules("{}");
    let mut g = game(&r, 2);
    let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
    let mut attacks = 0;
    for _ in 0..15_000 {
        for ai in ais.iter_mut() {
            if !ai.due(&g) {
                continue;
            }
            for c in ai.think(&g) {
                if let classic_sim::CommandOrder::Attack { target } = c.order {
                    assert!(g.known(c.player, target), "player {} attacked unseen {target}", c.player);
                    attacks += 1;
                }
                g.order(c.player, &c.ids, c.order);
            }
        }
        g.step(1);
        if winner(&g).is_some() {
            break;
        }
    }
    let fired = g.events.iter().filter(|e| matches!(e, Event::Fired { .. })).count();
    println!(
        "AI against AI under fog: {attacks} attack orders, each on a known target, {fired} shots, tick {}",
        g.state.tick
    );
    assert!(attacks > 0 && fired > 0, "they found each other and fought");
}

#[test]
fn two_ais_under_fog_play_the_same_game_every_time() {
    let r = rules("{}");
    let run = || {
        let mut g = game(&r, 3);
        let mut ais = [Ai::new(0, Settings::normal()), Ai::new(1, Settings::normal())];
        let end = play(&mut g, &mut ais, 20_000);
        (g.hash(), end, ais.iter().map(|a| a.waves_sent).collect::<Vec<_>>())
    };
    let (a, b) = (run(), run());
    println!("AI against AI under fog, seed 3: {a:?}");
    assert_eq!(a, b);
}

#[test]
fn its_search_looks_at_ground_it_never_saw_until_it_finds_the_last_building() {
    // The enemy's last building stands in the far corner of its own half, on ground the computer has never seen,
    // over a point of a coarse search grid and out of sight of every other: a search that only looks from those
    // points never finds it.
    for hide in [1, 0] {
        let r = rules(&format!(r#"{{ "modules": {{ "fog": {{ "hide": {hide} }} }} }}"#));
        let mut g = game(&r, 1);
        for e in g.state.entities.iter_mut().filter(|e| e.owner == 0) {
            e.health = 0;
        }
        g.step(1);
        // Standing on the search point at (3, 39) of a grid every six tiles, five tiles or more from every other.
        let plant = g.spawn(g.kind("power_plant").unwrap(), 0, 2, 38);
        let unseen = g.tile_view(1, 2, 38) == TileView::Shroud;
        let tank = g.kind("battle_tank").unwrap();
        let home = g.map.start[1].unwrap();
        for i in 0..8 {
            g.spawn(tank, 1, home.x - 4 + i % 4, home.y - 3 - i / 4);
        }
        let mut ai = [Ai::new(1, Settings { first_wave_tick: 0, ..Settings::normal() })];
        let end = play(&mut g, &mut ai, 20_000);
        println!("fog hide {hide}: plant {plant} unseen at first: {unseen}; winner and tick {end:?}");
        assert!(unseen);
        assert_eq!(end.map(|(w, _)| w), Some(1), "it found the plant and destroyed it");
    }
}
