//! The message feed, the selection's control groups and the rail's tab key: client state the player sees, read from
//! the game's events and never fed back into it.

use classic_render::Hud;
use classic_render::Scene;
use classic_render::feed::{self, Feed, LIFE, Tone};
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");

fn game() -> (Game, Hud) {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    let game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let hud = Hud::new(&pack, &game, 0);
    (game, hud)
}

/// Step one tick at a time, letting the HUD read each one, and keep every line said.
fn run(game: &mut Game, hud: &mut Hud, ticks: u32, said: &mut Vec<(u32, String)>) {
    for _ in 0..ticks {
        game.step(1);
        hud.after_step(game);
        for l in &hud.feed.lines {
            if l.tick == game.state.tick {
                said.push((l.tick, l.text.clone()));
            }
        }
    }
}

#[test]
fn every_message_has_words_and_a_pack_can_reword_them() {
    let words = feed::default_words();
    println!("{} messages: {:?}", words.len(), words.keys().collect::<Vec<_>>());
    for (id, text) in &words {
        assert!(!text.is_empty(), "{id} says something");
    }
    // A pack's own file rewords what it names and keeps the rest; unknown ids are warned about.
    let dir = setting::root().join("target/feed-test-pack");
    std::fs::create_dir_all(dir.join("ui")).unwrap();
    std::fs::write(
        dir.join("ui/messages.json"),
        r#"{ "messages": { "low_power": "The lights are dimming", "no_such_message": "x" } }"#,
    )
    .unwrap();
    let f = Feed::new(&dir, Default::default(), 0);
    println!("warnings: {:?}", f.warnings);
    assert_eq!(f.words()["low_power"], "The lights are dimming");
    assert_eq!(f.words()["power_restored"], words["power_restored"]);
    assert_eq!(f.warnings.len(), 1);
    assert!(f.warnings[0].contains("no_such_message"));
}

#[test]
fn the_feed_tells_the_local_player_what_happened_to_them_and_nothing_of_others() {
    let (mut game, mut hud) = game();
    let mut said = Vec::new();
    let plant = game.kind("power_plant").unwrap();
    // Both players build a power plant; only player 0 hears that theirs is ready.
    game.order(0, &[], CommandOrder::Produce { kind: plant });
    game.order(1, &[], CommandOrder::Produce { kind: plant });
    let mut ticks = 0;
    while !said.iter().any(|(_, t): &(u32, String)| t == "Power Plant ready to place") {
        run(&mut game, &mut hud, 1, &mut said);
        ticks += 1;
        assert!(ticks < 5000, "the power plant finishes");
    }
    run(&mut game, &mut hud, 30, &mut said);
    println!("after {ticks} ticks: {said:?}");
    assert_eq!(said.iter().filter(|(_, t)| t.ends_with("ready to place")).count(), 1, "only the local player's");

    // Too many power users: low power, once; removing them brings it back.
    let radar = game.kind("radar").unwrap();
    let radars: Vec<u32> = (0..5).map(|i| game.spawn(radar, 0, 4 + 2 * i, 10)).collect();
    run(&mut game, &mut hud, 5, &mut said);
    println!("power {:?}, said {said:?}", game.power(0));
    assert!(game.power(0).is_short());
    assert_eq!(said.iter().filter(|(_, t)| t == "Low power").count(), 1);
    let line = hud.feed.lines.iter().find(|l| l.text == "Low power").unwrap();
    assert_eq!(line.tone, Tone::Bad);
    game.state.entities.retain(|e| !radars.contains(&e.id));
    run(&mut game, &mut hud, 5, &mut said);
    assert!(said.iter().any(|(_, t)| t == "Power restored"), "{said:?}");

    // Lines go after their time.
    run(&mut game, &mut hud, LIFE, &mut said);
    assert!(hud.feed.lines.is_empty(), "{:?}", hud.feed.lines);
}

#[test]
fn attacks_are_announced_now_and_then_and_losses_by_name() {
    let (mut game, mut hud) = game();
    let mut said = Vec::new();
    let tank = game.kind("battle_tank").unwrap();
    let harvester = game.kind("harvester").unwrap();
    // Enemy tanks beside player 0's base, and a lone harvester of player 0's in front of them.
    for i in 0..4 {
        game.spawn(tank, 1, 19, 4 + i);
    }
    let victim = game.spawn(harvester, 0, 18, 6);
    let mut ticks = 0;
    while game.state.entity(victim).is_some() {
        run(&mut game, &mut hud, 1, &mut said);
        ticks += 1;
        assert!(ticks < 3000, "the harvester is destroyed");
    }
    run(&mut game, &mut hud, 600, &mut said);
    println!("{said:?}");
    assert!(said.iter().any(|(_, t)| t == "Harvester lost"), "a loss is told by name");
    // However many hits, each kind of attack warning comes at most once per 20 seconds.
    for warning in ["Base under attack", "Units under attack"] {
        let at: Vec<u32> = said.iter().filter(|(_, t)| t == warning).map(|(tick, _)| *tick).collect();
        assert!(at.windows(2).all(|w| w[1] - w[0] >= 20 * 15), "{warning} at {at:?}");
    }
    assert!(said.iter().any(|(_, t)| t == "Units under attack"));
    // Player 1 lost nothing it was told about, and nothing of theirs shows on player 0's feed.
    assert!(said.iter().all(|(_, t)| !t.contains("Battle Tank")), "{said:?}");
}

#[test]
fn control_groups_keep_own_units_and_a_second_press_asks_to_centre() {
    let (mut game, _) = game();
    let tank = game.kind("battle_tank").unwrap();
    let a = game.spawn(tank, 0, 6, 8);
    let b = game.spawn(tank, 0, 7, 8);
    let enemy = game.spawn(tank, 1, 40, 30);
    let yard = game.state.entities.iter().find(|e| e.owner == 0 && game.rules.kind(e.kind).building).unwrap().id;
    let mut scene = Scene::default();
    scene.selected = vec![a, b, enemy, yard];
    scene.set_group(&game, 0, 1);
    assert_eq!(scene.groups[1], vec![a, b], "only the player's own units go in a group");
    scene.selected.clear();
    assert!(!scene.recall_group(2), "an empty group does nothing");
    assert!(scene.selected.is_empty());
    assert!(!scene.recall_group(1), "the first press selects");
    assert_eq!(scene.selected, vec![a, b]);
    assert!(scene.recall_group(1), "the second press asks to centre");
    let (x, y) = scene.selection_centre(&game).unwrap();
    println!("group 1 centre at {x:.2},{y:.2}");
    assert!((x - 7.0).abs() < 0.01 && (y - 8.5).abs() < 0.01);
    // A destroyed unit leaves its group.
    game.state.entities.retain(|e| e.id != a);
    scene.after_step(&game);
    assert_eq!(scene.groups[1], vec![b]);
}

#[test]
fn the_tab_key_steps_through_the_factories() {
    let (mut game, mut hud) = game();
    let factory = game.kind("heavy_factory").unwrap();
    game.spawn(factory, 0, 10, 10);
    let tabs: Vec<_> = hud.layout(&game, (1024.0, 768.0)).tabs.iter().map(|t| t.factory).collect();
    println!("tabs {tabs:?}");
    assert!(tabs.len() >= 2);
    let open = |hud: &Hud| hud.layout(&game, (1024.0, 768.0)).open.unwrap();
    assert_eq!(open(&hud), tabs[0]);
    hud.next_tab(&game, false);
    assert_eq!(open(&hud), tabs[1]);
    hud.next_tab(&game, true);
    hud.next_tab(&game, true);
    assert_eq!(open(&hud), tabs[tabs.len() - 1], "back from the first wraps to the last");
}
