//! Saved games, settings and the score: a save loads back into the same game, opponents and all, and refuses one
//! it can't reproduce; settings survive a round trip through their file; the score counts what the events say.

use classic_ai::{Ai, Difficulty};
use classic_render::prefs::{Bind, Prefs};
use classic_render::save::{Save, SaveInfo, command_text, map_hash, parse_command};
use classic_render::score::Score;
use classic_render::store::Store;
use classic_sim::world::Event;
use classic_sim::{Command, CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");

fn rules() -> Rules {
    Rules::from_table(&setting::load("generic").unwrap().rules).unwrap()
}

fn game(seed: i32) -> Game {
    Game::new(GameOptions { map: MAP, seed, players: None, rules: Some(&rules()) }).unwrap()
}

/// The local player's clicks: build a power plant early, send the tank off, queue a factory, and a move on the
/// tick the game is saved on.
fn click(g: &mut Game, save_tick: u32) {
    let kind = |g: &Game, id: &str| g.kind(id).unwrap();
    let tank = |g: &Game| {
        g.state.entities.iter().find(|e| e.owner == 0 && g.rules.kind(e.kind).id == "battle_tank").map(|e| e.id)
    };
    match g.state.tick {
        10 => g.order(0, &[], CommandOrder::Produce { kind: kind(g, "power_plant") }),
        200 | 2600 => {
            if let Some(t) = tank(g) {
                g.order(0, &[t], CommandOrder::Move { x: 20 + g.state.tick as i32 / 200, y: 20 });
            }
        }
        900 => g.order(0, &[], CommandOrder::Produce { kind: kind(g, "barracks") }),
        t if t == save_tick => g.order(0, &[], CommandOrder::Produce { kind: kind(g, "refinery") }),
        _ => {}
    }
}

/// One tick of the player's loop: clicks, then the opponents think, then the step.
fn tick(g: &mut Game, ais: &mut [Ai], save_tick: u32) {
    click(g, save_tick);
    for ai in ais.iter_mut() {
        ai.tick(g);
    }
    g.step(1);
}

fn info() -> SaveInfo {
    SaveInfo {
        setting: "generic".into(),
        map: "Skirmish map 1".into(),
        map_hash: map_hash(MAP),
        seed: 4,
        fog: "pack".into(),
        player: 0,
        faction: 0,
    }
}

#[test]
fn a_save_loads_into_the_same_game_and_plays_on_the_same() {
    const SAVED: u32 = 8000;
    const END: u32 = 14_000;
    // Uninterrupted: the player and a hard opponent play to the end.
    // Each side has its faction, which the load gives out again into a fresh game that has none.
    let factioned = || {
        let mut g = game(4);
        g.set_factions(&["faction_b", "faction_a"]);
        g
    };
    let mut straight = factioned();
    let mut ais = [Ai::new(1, Difficulty::Normal.settings())];
    while straight.state.tick < END {
        tick(&mut straight, &mut ais, SAVED);
    }
    // Saved part way, with a click queued on the saved tick, then loaded from the text.
    let mut live = factioned();
    let mut ais = [Ai::new(1, Difficulty::Normal.settings())];
    while live.state.tick < SAVED {
        tick(&mut live, &mut ais, SAVED);
    }
    click(&mut live, SAVED);
    let text = Save::of(&live, &[(1, Difficulty::Normal)], info()).to_text(&live.rules);
    println!("the opponent has sent {} waves and has one out: {}", ais[0].waves_sent, ais[0].wave.is_some());
    assert!(ais[0].waves_sent > 0, "saved with the opponent's attacks under way");
    let human = text.lines().filter(|l| l.starts_with("c ")).count();
    println!(
        "save at tick {SAVED}: {} lines, {human} commands, of {} in the log",
        text.lines().count(),
        live.command_log().len()
    );
    assert!(human >= 5 && human < live.command_log().len(), "only the player's own commands are kept");
    let save = Save::parse(&text, &live.rules).unwrap();
    assert_eq!(save.to_text(&live.rules), text, "the text reads back as it was written");
    assert!(text.contains("\nfactions faction_b,faction_a\n"));
    let mut ticks = 0;
    let (mut loaded, mut ais) = save.replay(game(4), |_| ticks += 1).unwrap();
    assert_eq!(ticks, SAVED);
    assert_eq!(loaded.hash(), live.hash());
    assert_eq!(loaded.state.players[0].faction.as_deref(), Some("faction_b"));
    // The saved tick's click was queued again by the load, so this first step has no clicks of its own.
    for ai in &mut ais {
        ai.tick(&mut loaded);
    }
    loaded.step(1);
    // The opponent's memory came back, so it plays on the same.
    while loaded.state.tick < END {
        tick(&mut loaded, &mut ais, SAVED);
    }
    println!(
        "at tick {END}: straight {} loaded {}, opponent waves {}",
        straight.hash(),
        loaded.hash(),
        ais[0].waves_sent
    );
    assert_eq!(loaded.hash(), straight.hash());
    assert_eq!(loaded.command_log().len(), straight.command_log().len());
}

#[test]
fn a_save_that_does_not_reproduce_its_game_never_loads() {
    let mut live = game(4);
    let mut ais = [Ai::new(1, Difficulty::Normal.settings())];
    while live.state.tick < 2400 {
        tick(&mut live, &mut ais, 0);
    }
    let save = Save::of(&live, &[(1, Difficulty::Normal)], info());
    let text = save.to_text(&live.rules);

    let wrong_hash = text.replace(&format!("hash {}", save.hash), "hash 0000");
    let e = Save::parse(&wrong_hash, &live.rules).unwrap().replay(game(4), |_| {}).err().unwrap();
    println!("tampered hash: {e}");
    assert!(e.contains("didn't reproduce"));
    let e = Save::parse(&text, &live.rules).unwrap().replay(game(5), |_| {}).err().unwrap();
    println!("another seed: {e}");
    assert!(e.contains("didn't reproduce"));
    let other = Game::new(GameOptions { map: MAP, seed: 4, players: None, rules: None }).unwrap();
    let e = save.replay(other, |_| {}).err().unwrap();
    assert!(e.contains("other rules"), "{e}");
    let easy = text.replace("ai 1 normal", "ai 1 easy");
    assert!(
        Save::parse(&easy, &live.rules).unwrap().replay(game(4), |_| {}).is_err(),
        "another opponent plays otherwise"
    );

    for (bad, why) in [
        ("hello\n".to_string(), "not a saved game"),
        (text.replace("seed 4", "seed four"), "not a number"),
        (format!("{text}c 1 0 - fly 3 4\n"), "unknown order"),
        (format!("{text}c 1 0 - produce teleporter\n"), "no teleporter"),
        (format!("{text}c 9999 0 - harvest\n"), "out of order"),
        (format!("{text}colour blue\n"), "unknown line"),
    ] {
        let e = Save::parse(&bad, &live.rules).unwrap_err();
        assert!(e.contains(why), "{e} should say {why}");
    }
}

#[test]
fn every_order_reads_back_from_its_text() {
    let r = rules();
    let k = |id: &str| r.kind_id(id).unwrap();
    for order in [
        CommandOrder::Move { x: 3, y: -1 },
        CommandOrder::Harvest,
        CommandOrder::Place { kind: k("power_plant"), x: 10, y: 12 },
        CommandOrder::Produce { kind: k("harvester") },
        CommandOrder::Attack { target: 42 },
        CommandOrder::Cancel { kind: k("battle_tank") },
        CommandOrder::Hold { kind: k("battle_tank"), on: true },
        CommandOrder::Hold { kind: k("harvester"), on: false },
        CommandOrder::Primary,
        CommandOrder::Repair { on: true },
        CommandOrder::Repair { on: false },
        CommandOrder::Sell,
        CommandOrder::Capture { target: 9 },
        CommandOrder::RepairAt { pad: 12 },
        CommandOrder::SelfDestruct,
        CommandOrder::StarportAdd { kind: k("battle_tank") },
        CommandOrder::StarportRemove { kind: k("quad") },
        CommandOrder::StarportConfirm,
        CommandOrder::Superpower { x: 20, y: -2 },
        CommandOrder::Deploy,
    ] {
        for ids in [vec![], vec![7], vec![7, 8, 9]] {
            let c = Command { player: 1, ids, order };
            let text = command_text(&r, &c);
            assert_eq!(parse_command(&text, &r).unwrap(), c, "{text}");
        }
    }
    assert_eq!(
        command_text(&r, &Command { player: 0, ids: vec![5, 6], order: CommandOrder::Move { x: 1, y: 2 } }),
        "0 5,6 move 1 2"
    );
}

#[test]
fn settings_survive_their_file_and_skip_what_they_cannot_read() {
    let mut p = Prefs { volume: [80, 0, 100, 30], scroll: 150, difficulty: Difficulty::Hard, ..Prefs::default() };
    p.set_key(Bind::Pause, "KeyP");
    let text = p.to_text();
    println!("{text}");
    assert_eq!(Prefs::parse(&text), (p.clone(), Vec::new()));

    let (q, warnings) = Prefs::parse(
        "volume.music = 140\nvolume.sfx = 50\nscroll = 10\nkey.jump = KeyJ\ndifficulty = brutal\nnonsense\n",
    );
    println!("{warnings:?}");
    assert_eq!(warnings.len(), 5);
    assert_eq!(q, Prefs { volume: [50, 100, 100, 100], ..Prefs::default() }, "only the good line counts");
    // The gains scale the mixer's starting levels.
    assert_eq!(p.bus_gain([1.0, 0.5, 1.0, 0.5]), [0.8, 0.0, 1.0, 0.15]);
    assert_eq!(classic_render::prefs::key_label("ArrowUp"), "ARROW UP");
    assert_eq!(classic_render::prefs::key_label("Digit7"), "7");
}

#[test]
fn a_folder_store_keeps_text_between_runs() {
    let dir = setting::root().join("target/store-test");
    let _ = std::fs::remove_dir_all(&dir);
    let mut store = Store::Dir(dir.clone());
    assert_eq!(store.read("settings.txt"), None);
    store.write("settings.txt", "scroll = 75\n").unwrap();
    // A second store on the same folder, as the next run would make, reads it.
    assert_eq!(Store::Dir(dir).read("settings.txt").as_deref(), Some("scroll = 75\n"));
}

#[test]
fn the_score_counts_what_the_events_say() {
    let mut g = game(3);
    let mut ais = [Ai::new(0, Difficulty::Normal.settings()), Ai::new(1, Difficulty::Normal.settings())];
    let mut score = Score::default();
    let (mut built, mut placed, mut destroyed) = (0, 0, 0);
    for _ in 0..15_000 {
        for ai in &mut ais {
            ai.tick(&mut g);
        }
        g.step(1);
        score.after_step(&g);
        // The player clears its events now and then; the score must not count any twice.
        if g.events.len() > 5_000 {
            for e in &g.events {
                match e {
                    Event::UnitBuilt { .. } => built += 1,
                    Event::BuildingPlaced { .. } => placed += 1,
                    Event::Destroyed { .. } | Event::HazardAte { .. } => destroyed += 1,
                    _ => {}
                }
            }
            g.events.clear();
        }
    }
    for e in &g.events {
        match e {
            Event::UnitBuilt { .. } => built += 1,
            Event::BuildingPlaced { .. } => placed += 1,
            Event::Destroyed { .. } | Event::HazardAte { .. } => destroyed += 1,
            _ => {}
        }
    }
    let lines = score.lines();
    println!("{lines:#?}");
    assert_eq!(lines.iter().map(|l| l.units_built).sum::<u32>(), built);
    assert_eq!(lines.iter().map(|l| l.buildings_built).sum::<u32>(), placed);
    assert_eq!(lines.iter().map(|l| l.units_lost + l.buildings_lost).sum::<u32>(), destroyed);
    assert!(lines.iter().map(|l| l.kills).sum::<u32>() <= destroyed);
    assert!(lines.iter().all(|l| l.kills > 0 && l.units_built > 0), "both sides fought");
    for (l, p) in lines.iter().zip(&g.state.players) {
        assert_eq!(l.harvested, p.delivered);
    }
}
