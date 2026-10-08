//! Collision (rules-movement.md, sections 4 to 7): one ground unit per tile, reserving the next tile before leaving
//! a centre, waiting, stepping aside, finding a way round and giving up.
//!
//! Each map keeps the starting bases in the rows above a cliff line, so the tests below it have the ground to
//! themselves. A battle tank moves 22 sub-tile units a tick, so it crosses a 256-unit tile in about 12 ticks.

use classic_sim::movement::step_tile;
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, MoveEnd, Order, Task, Tile};

/// Bases in rows 0 to 7; a cliff line; then a 1-wide corridor (row 10) with a one-tile pocket above it at x = 12.
const CORRIDOR: &str = "\
########################
#1################2#####
########################
########################
########################
########################
########################
########################
XXXXXXXXXXXXXXXXXXXXXXXX
XXXXXXXXXXXX#XXXXXXXXXXX
X######################X
XXXXXXXXXXXXXXXXXXXXXXXX
";

/// Bases above; below, open rock split by a cliff wall at x = 12 with a one-tile gap at y = 14.
const CHOKE: &str = "\
##########################
#1##################2#####
##########################
##########################
##########################
##########################
##########################
##########################
XXXXXXXXXXXXXXXXXXXXXXXXXX
X###########X############X
X###########X############X
X###########X############X
X###########X############X
X###########X############X
X########################X
X###########X############X
X###########X############X
X###########X############X
X###########X############X
X###########X############X
XXXXXXXXXXXXXXXXXXXXXXXXXX
";

/// Bases above; open rock below.
const FIELD: &str = "\
########################
#1################2#####
########################
########################
########################
########################
########################
########################
XXXXXXXXXXXXXXXXXXXXXXXX
X######################X
X######################X
X######################X
X######################X
X######################X
X######################X
XXXXXXXXXXXXXXXXXXXXXXXX
";

fn game(map: &str) -> Game {
    Game::new(GameOptions { map, seed: 1, players: None, rules: None }).expect("map is valid")
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn tank(g: &mut Game, owner: u32, x: i32, y: i32) -> u32 {
    let k = kind(g, "battle_tank");
    g.spawn(k, owner, x, y)
}

fn at(g: &Game, id: u32) -> Tile {
    g.state.entity(id).unwrap().tile()
}

fn count(g: &Game, name: &str) -> usize {
    g.events.iter().filter(|e| e.name() == name).count()
}

/// The invariant: no two ground units hold one tile, and every unit stands on open ground.
fn check_occupancy(g: &Game) {
    let mut held: Vec<(Tile, u32)> = Vec::new();
    for e in g.state.entities.iter().filter(|e| !g.rules.kind(e.kind).building) {
        for t in std::iter::once(e.tile()).chain(step_tile(e)) {
            assert!(g.pathfinder.passable(t.x, t.y), "tick {}: unit {} on blocked tile {t:?}", g.state.tick, e.id);
            if let Some((_, other)) = held.iter().find(|(h, _)| *h == t) {
                panic!("tick {}: units {other} and {} both hold {t:?}", g.state.tick, e.id);
            }
            held.push((t, e.id));
        }
    }
}

fn run_checked(g: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        g.step(1);
        check_occupancy(g);
    }
}

#[test]
fn a_unit_waits_for_the_tile_ahead_instead_of_driving_through() {
    let mut g = game(FIELD);
    let a = tank(&mut g, 0, 2, 10);
    let b = tank(&mut g, 0, 6, 10);
    // a catches up with b while b is still standing, then follows it along the row.
    g.order(0, &[a], CommandOrder::Move { x: 12, y: 10 });
    run_checked(&mut g, 45);
    assert_eq!(at(&g, a), Tile { x: 5, y: 10 }, "stopped behind b");
    g.order(0, &[b], CommandOrder::Move { x: 13, y: 10 });
    run_checked(&mut g, 200);
    println!("a at {:?}, b at {:?}", at(&g, a), at(&g, b));
    assert_eq!((at(&g, a), at(&g, b)), (Tile { x: 12, y: 10 }, Tile { x: 13, y: 10 }));
}

#[test]
fn a_move_to_a_taken_tile_ends_on_the_nearest_free_one() {
    let mut g = game(FIELD);
    let sitter = tank(&mut g, 1, 12, 11);
    let a = tank(&mut g, 0, 3, 11);
    g.order(0, &[a], CommandOrder::Move { x: 12, y: 11 });
    run_checked(&mut g, 200);
    let end = at(&g, a);
    println!("ended at {end:?}");
    assert_eq!(at(&g, sitter), Tile { x: 12, y: 11 }, "an enemy never yields");
    assert_eq!((end.x - 12).abs().max((end.y - 11).abs()), 1, "next to the goal");
    assert!(
        g.events.iter().any(|e| matches!(e, Event::MoveEnded { unit, reason: MoveEnd::Arrived, .. } if *unit == a))
    );
    assert_eq!(g.state.entity(a).unwrap().order, Order::Idle);
}

#[test]
fn a_group_sent_to_one_tile_spreads_over_the_tiles_round_it() {
    let mut g = game(FIELD);
    let ids: Vec<u32> = (0..9).map(|i| tank(&mut g, 0, 2 + i % 3, 9 + i / 3)).collect();
    g.order(0, &ids, CommandOrder::Move { x: 16, y: 11 });
    run_checked(&mut g, 400);
    let mut ends: Vec<Tile> = ids.iter().map(|&id| at(&g, id)).collect();
    println!("{ends:?}");
    for t in &ends {
        assert!((t.x - 16).abs().max((t.y - 11).abs()) <= 2, "{t:?} within two rings of the goal");
    }
    ends.sort_by_key(|t| (t.y, t.x));
    ends.dedup();
    assert_eq!(ends.len(), 9, "all on different tiles");
    assert_eq!(count(&g, "unit_stuck"), 0);
}

#[test]
fn an_idle_own_unit_steps_aside_and_an_enemy_does_not() {
    let mut g = game(CORRIDOR);
    // An own tank parked in the corridor gives way; the mover passes.
    let parked = tank(&mut g, 0, 9, 10);
    let a = tank(&mut g, 0, 3, 10);
    g.order(0, &[a], CommandOrder::Move { x: 12, y: 9 });
    run_checked(&mut g, 300);
    println!("own: parked at {:?}, a at {:?}", at(&g, parked), at(&g, a));
    assert!(
        g.events.iter().any(|e| matches!(e, Event::UnitYielded { unit, asker, .. } if *unit == parked && *asker == a))
    );
    assert_eq!(at(&g, a), Tile { x: 12, y: 9 });

    // An enemy tank in the corridor stays; the mover gives up after its searches for a way round fail.
    let mut g = game(CORRIDOR);
    let enemy = tank(&mut g, 1, 9, 10);
    g.state.entities.iter_mut().find(|e| e.id == enemy).unwrap().reload = 100_000;
    let a = tank(&mut g, 0, 3, 10);
    g.state.entities.iter_mut().find(|e| e.id == a).unwrap().reload = 100_000;
    g.order(0, &[a], CommandOrder::Move { x: 20, y: 10 });
    run_checked(&mut g, 300);
    println!("enemy: a at {:?}", at(&g, a));
    assert_eq!(at(&g, enemy), Tile { x: 9, y: 10 });
    assert_eq!(at(&g, a), Tile { x: 8, y: 10 });
    assert_eq!(count(&g, "unit_yielded"), 0);
    assert!(g.events.iter().any(|e| matches!(e, Event::UnitStuck { unit, .. } if *unit == a)));
    assert!(
        g.events.iter().any(|e| matches!(e, Event::MoveEnded { unit, reason: MoveEnd::Blocked, .. } if *unit == a))
    );
}

/// Player 0's base with cliff round its refinery (1..=3, 3..=4) on every side but one tile, (2, 5), under its
/// middle column: the one place a harvester can unload. Resource two rows down. The starting tank, put on the cliff
/// at (4, 5) by the start layout, is moved out of the way by `boxed`.
const DOCK_BOX: &str = "\
########################
#1################2#####
X#######################
X###X###################
X###X###################
XX#XX###################
########################
~~~~~~~~~~~~~~~~~~~~~~~~
XXXXXXXXXXXXXXXXXXXXXXXX
";

/// A game on `map` with player 0's starting tank moved to (10, 5).
fn boxed(map: &str) -> Game {
    let mut g = game(map);
    let t = g.state.entities.iter_mut().find(|e| e.owner == 0 && e.tile() == Tile { x: 4, y: 5 });
    let t = t.expect("the starting tank");
    (t.x, t.y) = (10 * 256 + 128, 5 * 256 + 128);
    g
}

/// Player 0's harvester, moved to (6, 6) with a full load, so it heads for a dock next tick, and a tank of `owner`'s
/// parked on (2, 5). Every gun is kept quiet.
fn full_harvester_and_dock_blocker(map: &str, owner: u32) -> (Game, u32, u32) {
    let mut g = boxed(map);
    let full = g.rules.kind(kind(&g, "harvester")).harvester.as_ref().unwrap().capacity;
    let h = g.state.entities.iter().find(|e| e.owner == 0 && e.cargo.is_some()).unwrap().id;
    let parked = tank(&mut g, owner, 2, 5);
    for e in g.state.entities.iter_mut() {
        if e.id == h {
            (e.x, e.y) = (6 * 256 + 128, 6 * 256 + 128);
            e.cargo = Some(full);
            e.task = Some(Task::Mining);
        }
        e.reload = 100_000;
    }
    (g, h, parked)
}

#[test]
fn a_harvester_unloads_on_another_side_when_a_unit_stands_under_the_pad() {
    for owner in [0, 1] {
        let (mut g, h, parked) = full_harvester_and_dock_blocker(FIELD, owner);
        run_checked(&mut g, 300);
        let delivered = g.events.iter().any(|e| matches!(e, Event::Delivered { unit, .. } if *unit == h));
        println!("tank of player {owner} under the pad: delivered {delivered}, tank at {:?}", at(&g, parked));
        assert!(delivered);
        assert_eq!(at(&g, parked), Tile { x: 2, y: 5 }, "nobody had to ask it to move");
        assert_eq!(count(&g, "unit_yielded"), 0);
    }
}

#[test]
fn an_own_unit_on_the_only_dock_gives_way_and_an_enemy_one_blocks_it() {
    let (mut g, h, parked) = full_harvester_and_dock_blocker(DOCK_BOX, 0);
    run_checked(&mut g, 300);
    println!("own: parked moved to {:?}", at(&g, parked));
    assert!(
        g.events.iter().any(|e| matches!(e, Event::UnitYielded { unit, asker, .. } if *unit == parked && *asker == h))
    );
    assert!(g.events.iter().any(|e| matches!(e, Event::Delivered { unit, .. } if *unit == h)));

    let (mut g, h, parked) = full_harvester_and_dock_blocker(DOCK_BOX, 1);
    run_checked(&mut g, 300);
    let e = g.state.entity(h).unwrap();
    println!("enemy: harvester waits at {:?}, {:?}", e.tile(), e.task);
    assert_eq!(at(&g, parked), Tile { x: 2, y: 5 });
    assert_eq!(e.task, Some(Task::ToRefinery), "still queued for the dock");
    let mine = |e: &&Event| matches!(e, Event::Delivered { unit, .. } | Event::UnitStuck { unit, .. } if *unit == h);
    assert_eq!(g.events.iter().filter(mine).count(), 0);
    assert!(!g.events.iter().any(|e| matches!(e, Event::UnitYielded { unit, .. } if *unit == parked)));
}

#[test]
fn two_units_meeting_head_on_in_a_corridor_get_past_each_other() {
    let mut g = game(CORRIDOR);
    let a = tank(&mut g, 0, 2, 10);
    let b = tank(&mut g, 0, 21, 10);
    g.order(0, &[a], CommandOrder::Move { x: 21, y: 10 });
    g.order(0, &[b], CommandOrder::Move { x: 2, y: 10 });
    run_checked(&mut g, 1200);
    println!(
        "a at {:?}, b at {:?}; yielded {}, stuck {}",
        at(&g, a),
        at(&g, b),
        count(&g, "unit_yielded"),
        count(&g, "unit_stuck")
    );
    assert_eq!(at(&g, b), Tile { x: 2, y: 10 }, "the higher id gave way, then went on");
    assert!(count(&g, "unit_yielded") > 0);
}

#[test]
fn twelve_tanks_squeeze_through_a_one_tile_gap() {
    let mut g = game(CHOKE);
    let ids: Vec<u32> = (0..12).map(|i| tank(&mut g, 0, 2 + i % 4, 10 + i / 4)).collect();
    g.order(0, &ids, CommandOrder::Move { x: 20, y: 14 });
    let mut through = 0;
    for t in 0..1500 {
        g.step(1);
        check_occupancy(&g);
        if ids.iter().all(|&id| at(&g, id).x > 12) {
            through = t;
            break;
        }
    }
    println!("all through by tick {through}; stuck {}", count(&g, "unit_stuck"));
    assert!(through > 0, "{:?}", ids.iter().map(|&id| at(&g, id)).collect::<Vec<_>>());
    assert_eq!(count(&g, "unit_stuck"), 0);
}

#[test]
fn a_factory_with_no_rally_point_keeps_its_exit_clear() {
    let rules = {
        let mut t = classic_data::RulesTable::builtin();
        let errors = t.apply_tuning(
            &classic_data::json::parse(r#"{ "modules": { "production": { "instant_build": 1 } } }"#).unwrap(),
        );
        assert!(errors.is_empty());
        classic_sim::Rules::from_table(&t).unwrap()
    };
    let mut g = Game::new(GameOptions { map: FIELD, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    g.spawn(kind(&g, "power_plant"), 0, 5, 1);
    g.spawn(kind(&g, "power_plant"), 0, 7, 1);
    g.spawn(kind(&g, "heavy_factory"), 0, 9, 1);
    g.state.players[0].credits = 100_000;
    let t = kind(&g, "battle_tank");
    // Five fill the queue; each comes out as soon as its exit is free. Three more join once there's room.
    for n in [5, 3] {
        for _ in 0..n {
            g.order(0, &[], CommandOrder::Produce { kind: t });
        }
        run_checked(&mut g, 200);
    }
    let built: Vec<u32> = g
        .events
        .iter()
        .filter_map(|e| if let Event::UnitBuilt { tick, .. } = *e { Some(tick) } else { None })
        .collect();
    println!("built on ticks {built:?}; rejected {}", count(&g, "production_rejected"));
    assert_eq!(built.len(), 8);
    assert_eq!(count(&g, "production_rejected"), 0);
}

#[test]
fn collision_replays_from_the_command_log() {
    // Six own tanks and two enemy ones, sent through each other.
    let setup =
        |g: &mut Game| -> Vec<u32> { (0..8i32).map(|i| tank(g, (i / 6) as u32, 2 + i % 4, 9 + i / 4)).collect() };
    let mut live = game(FIELD);
    let ids = setup(&mut live);
    live.order(0, &ids[..6], CommandOrder::Move { x: 18, y: 12 });
    live.step(60);
    live.order(1, &ids[6..], CommandOrder::Move { x: 3, y: 9 });
    live.step(400);
    let log = live.command_log().to_vec();
    let mut replay = game(FIELD);
    setup(&mut replay);
    for t in 0..460 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    println!("yielded {}, live {} replay {}", count(&live, "unit_yielded"), live.hash(), replay.hash());
    assert_eq!(replay.hash(), live.hash());
}

/// Two own harvesters share a refinery with one free side: the first unloads on it while the second, also full,
/// queues on the tile between the dock and the field. The one leaving and the one queued must not wait on each other for good.
#[test]
fn a_harvester_leaving_its_dock_and_the_next_one_queued_for_it_both_get_through() {
    let mut g = boxed(DOCK_BOX);
    let hk = kind(&g, "harvester");
    let full = g.rules.kind(hk).harvester.as_ref().unwrap().capacity;
    let h1 = g.state.entities.iter().find(|e| e.owner == 0 && e.cargo.is_some()).unwrap().id;
    let home = g.state.entity(h1).unwrap().home_id;
    assert_eq!(at(&g, h1), Tile { x: 2, y: 5 }, "starts on its dock");
    // The higher id queues, so the old rule (a waiting unit yields only to a lower id) never let it give way.
    let h2 = g.spawn(hk, 0, 2, 6);
    for e in g.state.entities.iter_mut() {
        if e.id == h1 || e.id == h2 {
            e.cargo = Some(full);
            e.task = Some(Task::Mining);
            e.order = Order::Harvest;
            e.home_id = home;
        }
        e.reload = 100_000;
    }
    let delivered_by = |g: &Game, id: u32| {
        g.events.iter().filter(|e| matches!(e, Event::Delivered { unit, .. } if *unit == id)).count()
    };
    let mut last = 0;
    let mut longest_gap = 0;
    for t in 1..=3000 {
        g.step(1);
        check_occupancy(&g);
        let n = count(&g, "delivered");
        if n > last {
            last = n;
            longest_gap = 0;
        } else {
            longest_gap += 1;
            assert!(longest_gap < 900, "tick {t}: no delivery for {longest_gap} ticks");
        }
    }
    let yielded =
        g.events.iter().any(|e| matches!(e, Event::UnitYielded { unit, asker, .. } if *unit == h2 && *asker == h1));
    println!(
        "deliveries: h1 {}, h2 {}; the queued one gave way: {yielded}",
        delivered_by(&g, h1),
        delivered_by(&g, h2)
    );
    assert!(yielded, "the queued harvester stepped aside for the one leaving the dock");
    assert!(delivered_by(&g, h1) >= 2 && delivered_by(&g, h2) >= 2, "both keep delivering");
}
