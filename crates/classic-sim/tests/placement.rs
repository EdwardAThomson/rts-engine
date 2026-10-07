//! Buildings: footprints, placement rules (rules-base-building-power.md, "Placement rules"), and buildings blocking
//! ground movement.
//!
//! The base map below is mostly rock. Player 0's 3x2 refinery stands at (2, 2) to (4, 3), its harvester on the dock
//! under the middle column at (3, 4) and its tank at (5, 4). Player 1's refinery stands at (13, 2). The cliff at
//! (6, 4) and (7, 4) sits where a refinery placed at (5, 2) would need its dock.

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Kind, PlaceError, QueueEntry, Rules, Tile};

const BASE: &str = "\
################....
################....
##1##########2##..~~
################..~~
######XX########....
################....
....................
";

const TEST_MAP: &str = include_str!("../../../maps/test-01.txt");

fn game_on(map: &str, rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map, seed: 1, players: None, rules }).expect("map is valid")
}

fn base(rules: Option<&Rules>) -> Game {
    game_on(BASE, rules)
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// Rules with a tuning applied, as a setting pack's tuning.json would.
fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

/// Put a finished `id` at `player`'s construction yard, ready to place, as production would. The yard is spawned
/// in the map's bottom-right corner if the player has none, clear of everything these tests place.
fn give_ready(g: &mut Game, player: u32, id: &str) {
    let (yard, item) = (kind(g, "construction_yard"), kind(g, id));
    if !g.state.entities.iter().any(|e| e.owner == player && e.kind == yard) {
        let (w, h) = (g.map.width, g.map.height);
        g.spawn(yard, player, w - 2, h - 2);
    }
    let y = g.state.entities.iter_mut().find(|e| e.owner == player && e.kind == yard).unwrap();
    y.queue.push(QueueEntry { item, state: EntryState::Ready, progress: 0, paid: 0 });
}

/// Order a placement of a ready building, run the tick, and return what happened to it.
fn place(g: &mut Game, player: u32, id: &str, x: i32, y: i32) -> Result<u32, PlaceError> {
    give_ready(g, player, id);
    let k = kind(g, id);
    g.order(player, &[], CommandOrder::Place { kind: k, x, y });
    let from = g.events.len();
    g.step(1);
    let result = g.events[from..].iter().find_map(|e| match *e {
        Event::BuildingPlaced { entity, owner, .. } if owner == player => Some(Ok(entity)),
        Event::PlacementRejected { reason, player: p, .. } if p == player => Some(Err(reason)),
        _ => None,
    });
    result.expect("a placement always reports")
}

fn unit(g: &Game, owner: u32, id: &str) -> u32 {
    g.state.entities.iter().find(|e| e.owner == owner && g.rules.kind(e.kind).id == id).unwrap().id
}

#[test]
fn the_start_refinery_is_three_by_two_with_its_dock_under_the_middle_column() {
    let g = base(None);
    let r = g.state.entity(unit(&g, 0, "refinery")).unwrap();
    let k = g.rules.kind(r.kind);
    assert_eq!((r.tile(), k.width, k.height), (Tile { x: 2, y: 2 }, 3, 2));
    assert_eq!(g.state.entity(unit(&g, 0, "harvester")).unwrap().tile(), Tile { x: 3, y: 4 });
    assert_eq!(g.state.entity(unit(&g, 0, "battle_tank")).unwrap().tile(), Tile { x: 5, y: 4 });
    for x in 2..=4 {
        for y in 2..=3 {
            assert!(!g.pathfinder.passable(x, y), "{x},{y} is under the refinery");
        }
    }
}

#[test]
fn a_building_touching_your_base_is_placed_and_blocks_its_footprint() {
    let mut g = base(None);
    let id = place(&mut g, 0, "power_plant", 5, 2).expect("touching the refinery, on rock");
    let e = g.state.entity(id).unwrap();
    assert_eq!((e.owner, e.tile(), g.rules.kind(e.kind).id.as_str()), (0, Tile { x: 5, y: 2 }, "power_plant"));
    assert_eq!(e.health, g.rules.kind(e.kind).max_health);
    for (x, y) in [(5, 2), (6, 2), (5, 3), (6, 3)] {
        assert!(!g.pathfinder.passable(x, y), "{x},{y} is under the building");
    }
    assert!(g.pathfinder.passable(7, 2), "beside it is still open");
    assert!(g.pathfinder.find(0, 0, 5, 2).is_none(), "a building tile is not a goal");
}

#[test]
fn every_placement_rule_refuses_with_its_reason() {
    let g = base(None);
    let k = |id| kind(&g, id);
    let cases = [
        ("a unit kind", k("harvester"), 0, 5, 2, PlaceError::NotABuilding),
        ("past the edge", k("power_plant"), 0, 19, 6, PlaceError::OutOfBounds),
        ("on a cliff", k("power_plant"), 0, 6, 3, PlaceError::BadGround { x: 6, y: 4 }),
        ("on open ground", k("power_plant"), 0, 0, 5, PlaceError::BadGround { x: 0, y: 6 }),
        ("over the refinery", k("power_plant"), 0, 1, 1, PlaceError::Blocked { x: 2, y: 2 }),
        ("on the harvester", k("gun_turret"), 0, 3, 4, PlaceError::Blocked { x: 3, y: 4 }),
        ("one tile out", k("power_plant"), 0, 6, 1, PlaceError::TooFar),
        ("next to someone else's base", k("power_plant"), 1, 5, 2, PlaceError::TooFar),
        ("a refinery whose dock is a cliff", k("refinery"), 0, 5, 2, PlaceError::BadExit),
    ];
    for (what, kind, player, x, y, want) in cases {
        let got = g.can_place(player, kind, x, y);
        println!("{what}: {got:?}");
        assert_eq!(got, Err(want), "{what}");
    }
    assert_eq!(g.can_place(0, k("refinery"), 5, 1), Ok(()), "the same refinery a row up has a rock dock");
}

#[test]
fn only_a_finished_building_waiting_at_a_yard_can_be_placed() {
    let mut g = base(None);
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 2 });
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(e, Event::PlacementRejected { reason: PlaceError::NotReady, .. })));
    assert_eq!(place(&mut g, 0, "power_plant", 5, 2).map(|_| ()), Ok(()));
    let yard = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "construction_yard").unwrap();
    assert!(yard.queue.is_empty(), "placing takes the building off the yard's queue");
}

#[test]
fn walls_do_not_extend_the_building_area() {
    let mut g = base(None);
    place(&mut g, 0, "wall", 5, 2).expect("a wall touching the refinery");
    assert_eq!(g.can_place(0, kind(&g, "power_plant"), 6, 2), Err(PlaceError::TooFar), "touching only the wall");
    place(&mut g, 0, "gun_turret", 5, 3).expect("a turret touching the refinery");
    assert_eq!(g.can_place(0, kind(&g, "power_plant"), 6, 2), Ok(()), "now touching the turret too");
}

#[test]
fn refused_placements_change_nothing_but_the_events() {
    let mut a = base(None);
    let mut b = base(None);
    assert_eq!(place(&mut a, 0, "power_plant", 7, 2), Err(PlaceError::TooFar));
    give_ready(&mut b, 0, "power_plant");
    b.step(1);
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn tuning_can_allow_open_ground_and_a_wider_reach() {
    let g = base(Some(&tuned(r#"{ "modules": { "placement": { "rock_only": 0, "max_gap": 1 } } }"#)));
    let plant = kind(&g, "power_plant");
    assert_eq!(g.can_place(0, plant, 6, 1), Ok(()), "one empty tile away");
    assert_eq!(g.can_place(0, plant, 1, 5), Ok(()), "half on open ground");
    assert_eq!(g.can_place(0, plant, 17, 2), Err(PlaceError::OnResource { x: 18, y: 2 }));
}

#[test]
fn a_unit_already_moving_goes_round_a_building_placed_on_its_path() {
    let rules = tuned(r#"{ "modules": { "placement": { "rock_only": 0, "max_gap": 1 } } }"#);
    let mut g = game_on(TEST_MAP, Some(&rules));
    let tank = unit(&g, 0, "battle_tank");
    // Both in the same tick: the move plans a path, then the placement drops a building across it.
    g.order(0, &[tank], CommandOrder::Move { x: 20, y: 9 });
    give_ready(&mut g, 0, "power_plant");
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 7, y: 4 });
    let footprint = [(7, 4), (8, 4), (7, 5), (8, 5)];
    let mut steps = 0;
    for _ in 0..700 {
        g.step(1);
        let t = g.state.entity(tank).unwrap().tile();
        assert!(!footprint.contains(&(t.x, t.y)), "tick {}: the tank drove into the building", g.state.tick);
        steps += 1;
    }
    assert!(g.events.iter().any(|e| matches!(e, Event::BuildingPlaced { .. })));
    let end = g.state.entity(tank).unwrap().tile();
    println!("checked {steps} ticks; tank ends at {},{}", end.x, end.y);
    assert_eq!(end, Tile { x: 20, y: 9 });
}

#[test]
fn a_refinery_tuned_smaller_moves_its_dock_and_harvesters_still_deliver() {
    let rules = tuned(r#"{ "refinery": { "width": 1, "height": 1 } }"#);
    let mut g = game_on(TEST_MAP, Some(&rules));
    assert_eq!(g.state.entity(unit(&g, 0, "harvester")).unwrap().tile(), Tile { x: 3, y: 3 }, "dock right below");
    assert_eq!(g.state.entity(unit(&g, 0, "battle_tank")).unwrap().tile(), Tile { x: 4, y: 3 });
    g.step(1200);
    for p in &g.state.players {
        println!("player {} credits {}", p.id, p.credits);
        assert!(p.credits >= 200);
    }
}

#[test]
fn placements_replay_from_the_command_log() {
    let mut live = base(None);
    give_ready(&mut live, 0, "power_plant");
    let plant = kind(&live, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 2 });
    live.step(300);
    give_ready(&mut live, 0, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 7, y: 2 });
    live.step(300);
    let log = live.command_log().to_vec();
    let mut replay = base(None);
    give_ready(&mut replay, 0, "power_plant");
    give_ready(&mut replay, 0, "power_plant");
    for t in 0..600 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    let placed = live.events.iter().filter(|e| matches!(e, Event::BuildingPlaced { .. })).count();
    println!("placed {placed}, live {} replay {}", live.hash(), replay.hash());
    assert_eq!(placed, 2);
    assert_eq!(replay.hash(), live.hash());
}
