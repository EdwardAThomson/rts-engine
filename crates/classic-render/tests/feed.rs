//! The message feed, the units' replies and the advisor's lines, the selection's control groups and the rail's tab key: client state the player sees, read from
//! the game's events and never fed back into it.

use classic_render::Hud;
use classic_render::Scene;
use classic_render::feed::{self, Feed, LIFE, REPLY_EVERY, REPLY_LIFE, Tone};
use classic_render::lines::{self, Lines, Moment, VOICES};
use classic_render::platform::Files;
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");

fn game() -> (Game, Hud) {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    let game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let hud = Hud::new(&pack, &Files::Dir(pack.dir.clone()), &game, 0, 0);
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
    let f = Feed::new(&Files::Dir(dir.clone()), Default::default(), 0, Lines::engine());
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
    for warning in ["Base under attack", "Units under attack", "Harvester under attack"] {
        let at: Vec<u32> = said.iter().filter(|(_, t)| t == warning).map(|(tick, _)| *tick).collect();
        assert!(at.windows(2).all(|w| w[1] - w[0] >= 20 * 15), "{warning} at {at:?}");
    }
    assert!(said.iter().any(|(_, t)| t == "Harvester under attack"), "a harvester's attack is its own warning");
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

#[test]
fn the_engine_has_a_reply_for_every_voice_and_moment_and_planned_ids_are_new() {
    let lines = Lines::engine();
    for voice in VOICES {
        for m in Moment::ALL {
            let said = &lines.acks[&(voice, m)];
            println!("{voice}.{}: {said:?}", m.id());
            assert!(!said.is_empty());
        }
    }
    let words = feed::default_words();
    let ids = lines::advisor_ids();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "a planned advisor id isn't already a message");
    assert!(ids.len() > words.len());
}

/// A pack in `target/` with `lines` as its `lines.json`.
#[test]
fn the_advisor_warns_of_massing_enemies_and_hazards_now_and_then_and_says_how_the_game_ended() {
    use classic_sim::world::{Event, MoveEnd};
    let (mut game, mut hud) = game();
    let mut said = Vec::new();
    let tank = game.kind("battle_tank").unwrap();
    // A few enemy tanks far off and then near player 0's base: only enough of them near it is a wave.
    let far: Vec<u32> = (0..6).map(|i| game.spawn(tank, 1, 40, 14 + i)).collect();
    run(&mut game, &mut hud, 30, &mut said);
    for i in 0..(feed::WAVE_SIZE as i32 - 1) {
        game.spawn(tank, 1, 8 + feed::WAVE_RANGE, 2 + i);
    }
    run(&mut game, &mut hud, 30, &mut said);
    assert_eq!(feed::enemies_near_base(&game, 0), feed::WAVE_SIZE - 1);
    assert!(said.iter().all(|(_, t)| t != "Enemy forces approaching"), "{far:?} far off, too few near: {said:?}");
    game.spawn(tank, 1, 8 + feed::WAVE_RANGE, 6);
    run(&mut game, &mut hud, 30, &mut said);
    let waves = |said: &Vec<(u32, String)>| said.iter().filter(|(_, t)| t == "Enemy forces approaching").count();
    assert_eq!(waves(&said), 1, "{said:?}");
    run(&mut game, &mut hud, 600, &mut said);
    assert_eq!(waves(&said), 1, "said once while they stay");

    // A hazard appearing is news, but not every time.
    for _ in 0..2 {
        game.events.push(Event::HazardSpawned { tick: game.state.tick, hazard: 1, x: 0, y: 0 });
        run(&mut game, &mut hud, 1, &mut said);
    }
    assert_eq!(said.iter().filter(|(_, t)| t == "Hazard sighted").count(), 1);

    // A local unit giving up on its way says it can't get there.
    let mine = game.spawn(tank, 0, 20, 2);
    run(&mut game, &mut hud, 10, &mut said);
    game.events.push(Event::MoveEnded { tick: game.state.tick, unit: mine, reason: MoveEnd::Blocked, x: 20, y: 2 });
    hud.after_step(&game);
    let reply = hud.feed.reply.clone().expect("a reply");
    assert_eq!(reply.id, "cant");
    assert!(Lines::engine().acks[&("vehicle", Moment::Cant)].contains(&reply.text));

    // The end of the game, once.
    hud.feed.over(&game, true);
    hud.feed.over(&game, false);
    let ends: Vec<_> = hud.feed.lines.iter().filter(|l| l.id.starts_with("game_")).map(|l| l.text.as_str()).collect();
    assert_eq!(ends, ["Battle won"]);
}

#[test]
fn a_move_order_nobody_can_walk_to_is_one_the_units_cant_carry_out() {
    let (mut game, _) = game();
    let tank = game.kind("battle_tank").unwrap();
    let mine = game.spawn(tank, 0, 20, 2);
    assert!(feed::reachable(&game, &[mine], (40, 2)), "open ground");
    assert!(!feed::reachable(&game, &[mine], (31, 5)), "a cliff");
    assert!(!feed::reachable(&game, &[mine], (5, 5)), "under a building");
    assert!(!feed::reachable(&game, &[], (40, 2)), "nobody to go");
}

fn pack_with_lines(name: &str, lines: &str) -> Files {
    let dir = setting::root().join("target").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lines.json"), lines).unwrap();
    Files::Dir(dir)
}

#[test]
fn a_pack_gives_each_faction_its_own_lines_and_mistakes_are_warned_about() {
    let pack = pack_with_lines(
        "lines-test-pack",
        r#"{
          "voice": { "faction_a": "notes for recording, not read by the engine" },
          "advisor": { "low_power": "Everyone: power low", "superpower_ready": "Ready to strike" },
          "acks": { "vehicle": { "move": ["Pack rolling.", "Pack driving."] } },
          "factions": {
            "faction_a": {
              "advisor": { "low_power": ["A: power low", "A: lights dim"], "no_such_line": "x" },
              "acks": { "vehicle": { "select": "A here.", "dance": "x" }, "boat": { "move": "x" } }
            },
            "faction_b": { "acks": { "infantry": { "attack": [] } } },
            "faction_z": {}
          }
        }"#,
    );
    let factions = ["faction_a".to_string(), "faction_b".to_string()];
    let a = Lines::load(&pack, &factions, Some("faction_a"));
    println!("warnings: {:#?}", a.warnings);
    assert_eq!(a.advisor["low_power"], ["A: power low", "A: lights dim"], "the faction's own words win");
    assert_eq!(a.advisor["superpower_ready"], ["Ready to strike"], "a planned id may have lines already");
    assert_eq!(a.acks[&("vehicle", Moment::Select)], ["A here."]);
    assert_eq!(a.acks[&("vehicle", Moment::Move)], ["Pack rolling.", "Pack driving."], "pack-wide lines carry over");
    assert_eq!(a.acks[&("infantry", Moment::Move)], Lines::engine().acks[&("infantry", Moment::Move)]);
    let b = Lines::load(&pack, &factions, Some("faction_b"));
    assert_eq!(b.advisor["low_power"], ["Everyone: power low"]);
    assert_eq!(b.acks[&("vehicle", Moment::Select)], Lines::engine().acks[&("vehicle", Moment::Select)]);
    // Every faction's part is checked, whichever faction is played.
    assert_eq!(a.warnings, b.warnings);
    for w in [
        "no advisor line `no_such_line`",
        "no moment `dance`",
        "no voice set `boat`",
        "infantry.attack needs",
        "no faction `faction_z`",
    ] {
        assert!(a.warnings.iter().any(|x| x.contains(w)), "{w} in {:?}", a.warnings);
    }
    assert_eq!(a.warnings.len(), 5);
}

#[test]
fn units_reply_to_their_own_player_without_saying_the_same_thing_twice() {
    let (mut game, _) = game();
    let pack = setting::load("generic").unwrap();
    let files = pack_with_lines(
        "lines-reply-pack",
        r#"{ "acks": { "vehicle": { "select": ["One.", "Two.", "Three."], "move": "Off." } },
             "advisor": { "low_power": ["Power dropping", "The lights are going"] } }"#,
    );
    let mut hud = Hud::new(&pack, &files, &game, 0, 0);
    assert!(hud.feed.warnings.is_empty(), "{:?}", hud.feed.warnings);
    let tank = game.kind("battle_tank").unwrap();
    let mine = game.spawn(tank, 0, 6, 8);
    let theirs = game.spawn(tank, 1, 40, 30);
    let yard = game.state.entities.iter().find(|e| e.owner == 0 && game.rules.kind(e.kind).building).unwrap().id;

    hud.feed.reply(&game, Moment::Select, &[theirs, yard]);
    assert!(hud.feed.reply.is_none(), "enemies and buildings say nothing");
    let mut said = Vec::new();
    for _ in 0..12 {
        hud.feed.reply(&game, Moment::Select, &[mine, theirs]);
        let r = hud.feed.reply.clone().unwrap();
        // Clicking again at once is too soon to answer.
        hud.feed.reply(&game, Moment::Select, &[mine]);
        assert_eq!(hud.feed.reply.as_ref().unwrap().tick, r.tick);
        said.push(r.text);
        game.step(REPLY_EVERY);
        hud.after_step(&game);
    }
    println!("{said:?}");
    assert!(said.iter().all(|t| ["One.", "Two.", "Three."].contains(&t.as_str())));
    assert!(said.windows(2).all(|w| w[0] != w[1]), "never the same line twice in a row");
    assert!(["One.", "Two.", "Three."].iter().all(|t| said.iter().any(|s| s == t)), "every line gets said");
    hud.feed.reply(&game, Moment::Move, &[mine]);
    assert_eq!(hud.feed.reply.as_ref().unwrap().text, "Off.");
    game.step(REPLY_LIFE);
    hud.after_step(&game);
    assert!(hud.feed.reply.is_none(), "the subtitle goes after its time");

    // The advisor's own words replace the feed's.
    let radar = game.kind("radar").unwrap();
    for i in 0..5 {
        game.spawn(radar, 0, 4 + 2 * i, 10);
    }
    game.step(1);
    hud.after_step(&game);
    let line = hud.feed.lines.iter().find(|l| l.id == "low_power").unwrap();
    assert!(["Power dropping", "The lights are going"].contains(&line.text.as_str()), "{line:?}");
}

#[test]
fn the_private_packs_lines_read_without_mistakes_when_they_are_cloned_in() {
    let packs = setting::private_packs();
    if packs.is_empty() {
        eprintln!("settings-private/ is not cloned here; skipping");
        return;
    }
    for dir in &packs {
        let pack = setting::load(dir.to_str().unwrap()).unwrap();
        let factions: Vec<String> = pack.factions.iter().map(|f| f.id.clone()).collect();
        for f in &factions {
            let lines = Lines::load(&Files::Dir(dir.clone()), &factions, Some(f));
            assert!(lines.warnings.is_empty(), "{}: {:?}", dir.display(), lines.warnings);
            println!("{} {f}: {} advisor lines", dir.display(), lines.advisor.len());
        }
    }
}
