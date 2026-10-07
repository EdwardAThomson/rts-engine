//! Buildings: footprints, placement rules (rules-base-building-power.md, "Placement rules"), and buildings blocking
//! ground movement.
//!
//! On the base map below, player 0 starts with its yard at (1, 1), its power plant at (3, 1) and its 3x2 refinery at
//! (1, 3), its harvester on the dock under the refinery's middle column at (2, 5) and its tank at (4, 5). Player 1's
//! base starts at (12, 1). The cliff at (6, 5) and (7, 5) sits where a refinery placed at (5, 3) would need its dock.

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Kind, PlaceError, QueueEntry, Rules, Tile};

const BASE: &str = "\
####################
#1##########2#######
####################
####################
####################
######XX########..~~
####################
....................
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
fn each_player_starts_with_a_yard_a_power_plant_and_a_refinery_with_its_harvester_docked() {
    let g = base(None);
    let at = |id: &str| g.state.entity(unit(&g, 0, id)).unwrap().tile();
    assert_eq!(at("construction_yard"), Tile { x: 1, y: 1 });
    assert_eq!(at("power_plant"), Tile { x: 3, y: 1 });
    assert_eq!(at("refinery"), Tile { x: 1, y: 3 });
    assert_eq!(at("harvester"), Tile { x: 2, y: 5 }, "the dock, under the refinery's middle column");
    assert_eq!(at("battle_tank"), Tile { x: 4, y: 5 });
    assert_eq!(g.state.players[0].credits, 1200);
    let under =
        (1..=4).flat_map(|x| (1..=2).map(move |y| (x, y))).chain((1..=3).flat_map(|x| (3..=4).map(move |y| (x, y))));
    for (x, y) in under {
        assert!(!g.pathfinder.passable(x, y), "{x},{y} is under the base");
    }
    assert!(g.pathfinder.passable(4, 3), "beside the refinery is open");
}

#[test]
fn a_building_touching_your_base_is_placed_and_blocks_its_footprint() {
    let mut g = base(None);
    let id = place(&mut g, 0, "power_plant", 5, 1).expect("touching the first plant, on rock");
    let e = g.state.entity(id).unwrap();
    assert_eq!((e.owner, e.tile(), g.rules.kind(e.kind).id.as_str()), (0, Tile { x: 5, y: 1 }, "power_plant"));
    assert_eq!(e.health, g.rules.kind(e.kind).max_health);
    for (x, y) in [(5, 1), (6, 1), (5, 2), (6, 2)] {
        assert!(!g.pathfinder.passable(x, y), "{x},{y} is under the building");
    }
    assert!(g.pathfinder.passable(7, 1), "beside it is still open");
    assert!(g.pathfinder.find(0, 0, 5, 1).is_none(), "a building tile is not a goal");
}

#[test]
fn every_placement_rule_refuses_with_its_reason() {
    let g = base(None);
    let k = |id| kind(&g, id);
    let cases = [
        ("a unit kind", k("harvester"), 0, 5, 1, PlaceError::NotABuilding),
        ("past the edge", k("power_plant"), 0, 19, 8, PlaceError::OutOfBounds),
        ("on a cliff", k("power_plant"), 0, 6, 4, PlaceError::BadGround { x: 6, y: 5 }),
        ("on open ground", k("power_plant"), 0, 0, 7, PlaceError::BadGround { x: 0, y: 7 }),
        ("over the yard", k("power_plant"), 0, 0, 0, PlaceError::Blocked { x: 1, y: 1 }),
        ("on the harvester", k("gun_turret"), 0, 2, 5, PlaceError::Blocked { x: 2, y: 5 }),
        ("one tile out", k("power_plant"), 0, 6, 1, PlaceError::TooFar),
        ("next to someone else's base", k("power_plant"), 1, 5, 1, PlaceError::TooFar),
        ("a refinery whose dock is a cliff", k("refinery"), 0, 5, 3, PlaceError::BadExit),
    ];
    for (what, kind, player, x, y, want) in cases {
        let got = g.can_place(player, kind, x, y);
        println!("{what}: {got:?}");
        assert_eq!(got, Err(want), "{what}");
    }
    assert_eq!(g.can_place(0, k("refinery"), 5, 2), Ok(()), "the same refinery a row up has a rock dock");
}

#[test]
fn only_a_finished_building_waiting_at_a_yard_can_be_placed() {
    let mut g = base(None);
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 1 });
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(e, Event::PlacementRejected { reason: PlaceError::NotReady, .. })));
    assert_eq!(place(&mut g, 0, "power_plant", 5, 1).map(|_| ()), Ok(()));
    let yard = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "construction_yard").unwrap();
    assert!(yard.queue.is_empty(), "placing takes the building off the yard's queue");
}

#[test]
fn walls_do_not_extend_the_building_area() {
    let mut g = base(None);
    place(&mut g, 0, "wall", 5, 1).expect("a wall touching the plant");
    assert_eq!(g.can_place(0, kind(&g, "power_plant"), 6, 1), Err(PlaceError::TooFar), "touching only the wall");
    place(&mut g, 0, "gun_turret", 5, 2).expect("a turret touching the plant");
    assert_eq!(g.can_place(0, kind(&g, "power_plant"), 6, 1), Ok(()), "now touching the turret too");
}

#[test]
fn refused_placements_change_nothing_but_the_events() {
    let mut a = base(None);
    let mut b = base(None);
    assert_eq!(place(&mut a, 0, "power_plant", 7, 1), Err(PlaceError::TooFar));
    give_ready(&mut b, 0, "power_plant");
    b.step(1);
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn tuning_can_allow_open_ground_and_a_wider_reach() {
    let g = base(Some(&tuned(r#"{ "modules": { "placement": { "rock_only": 0, "max_gap": 1 } } }"#)));
    let plant = kind(&g, "power_plant");
    assert_eq!(g.can_place(0, plant, 6, 1), Ok(()), "one empty tile away");
    assert_eq!(g.can_place(0, plant, 0, 6), Ok(()), "half on open ground");
    assert_eq!(g.can_place(0, plant, 17, 4), Err(PlaceError::OnResource { x: 18, y: 5 }));
}

#[test]
fn a_unit_already_moving_goes_round_a_building_placed_on_its_path() {
    let rules = tuned(r#"{ "modules": { "placement": { "rock_only": 0, "max_gap": 1 } } }"#);
    let mut g = game_on(TEST_MAP, Some(&rules));
    let tank = unit(&g, 0, "battle_tank");
    // Both in the same tick: the move plans a path, then the placement drops a building across it.
    // The tank starts at (6, 6) and heads down and left; the plant drops one tile below the refinery, on its way.
    g.order(0, &[tank], CommandOrder::Move { x: 2, y: 9 });
    give_ready(&mut g, 0, "power_plant");
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 4, y: 7 });
    let footprint = [(4, 7), (5, 7), (4, 8), (5, 8)];
    let mut steps = 0;
    for _ in 0..300 {
        g.step(1);
        let t = g.state.entity(tank).unwrap().tile();
        assert!(!footprint.contains(&(t.x, t.y)), "tick {}: the tank drove into the building", g.state.tick);
        steps += 1;
    }
    assert!(g.events.iter().any(|e| matches!(e, Event::BuildingPlaced { .. })));
    let end = g.state.entity(tank).unwrap().tile();
    println!("checked {steps} ticks; tank ends at {},{}", end.x, end.y);
    assert_eq!(end, Tile { x: 2, y: 9 });
}

#[test]
fn a_refinery_tuned_smaller_moves_its_dock_and_harvesters_still_deliver() {
    let rules = tuned(r#"{ "refinery": { "width": 1, "height": 1 } }"#);
    let mut g = game_on(TEST_MAP, Some(&rules));
    assert_eq!(g.state.entity(unit(&g, 0, "harvester")).unwrap().tile(), Tile { x: 3, y: 5 }, "dock right below");
    assert_eq!(g.state.entity(unit(&g, 0, "battle_tank")).unwrap().tile(), Tile { x: 5, y: 5 });
    g.step(1200);
    for p in &g.state.players {
        println!("player {} credits {}", p.id, p.credits);
        assert!(p.credits >= 1200 + 200);
    }
}

#[test]
fn placements_replay_from_the_command_log() {
    let mut live = base(None);
    give_ready(&mut live, 0, "power_plant");
    let plant = kind(&live, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 1 });
    live.step(300);
    give_ready(&mut live, 0, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 7, y: 1 });
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
