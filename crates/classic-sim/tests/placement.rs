//! Buildings: footprints, placement rules, and buildings blocking ground movement. On the test map player 0's
//! refinery stands on its start tile (3, 2) on a rock plateau, its harvester on the dock at (3, 3) and its tank at
//! (4, 3).

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, PlaceError, Rules, Tile};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game_with(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules }).expect("test map is valid")
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

/// Order a placement, run the tick, and return what happened to it.
fn place(g: &mut Game, player: u32, id: &str, x: i32, y: i32) -> Result<u32, PlaceError> {
    let k = kind(g, id);
    g.order(player, &[], CommandOrder::Place { kind: k, x, y });
    let from = g.events.len();
    g.step(1);
    let result = g.events[from..].iter().find_map(|e| match *e {
        Event::Placed { building, player: p, .. } if p == player => Some(Ok(building)),
        Event::PlaceRefused { reason, player: p, .. } if p == player => Some(Err(reason)),
        _ => None,
    });
    result.expect("a placement always reports")
}

#[test]
fn a_building_next_to_your_base_is_placed_and_blocks_its_footprint() {
    let mut g = game_with(None);
    let id = place(&mut g, 0, "power_plant", 4, 1).expect("next to the refinery, on rock");
    let e = g.state.entity(id).unwrap();
    assert_eq!((e.owner, e.tile(), g.rules.kind(e.kind).id.as_str()), (0, Tile { x: 4, y: 1 }, "power_plant"));
    assert_eq!(e.health, g.rules.kind(e.kind).max_health);
    for (x, y) in [(4, 1), (5, 1), (4, 2), (5, 2)] {
        assert!(!g.pathfinder.passable(x, y), "{x},{y} is under the building");
    }
    assert!(g.pathfinder.passable(6, 1), "beside it is still open");
    assert!(g.pathfinder.find(0, 0, 4, 1).is_none(), "a building tile is not a goal");
    println!("placed power plant {id}; command log has {} entries", g.command_log().len());
}

#[test]
fn every_placement_rule_refuses_with_its_reason() {
    let g = game_with(None);
    let (plant, turret, harvester) = (kind(&g, "power_plant"), kind(&g, "gun_turret"), kind(&g, "harvester"));
    let cases = [
        ("a unit kind", harvester, 0, 4, 1, PlaceError::NotABuilding),
        ("past the edge", plant, 0, 31, 19, PlaceError::OutOfBounds),
        ("on a cliff", plant, 0, 16, 4, PlaceError::BadGround { x: 16, y: 4 }),
        ("on resource", plant, 0, 7, 6, PlaceError::OnResource { x: 7, y: 6 }),
        ("over the refinery", plant, 0, 2, 1, PlaceError::Blocked { x: 3, y: 2 }),
        ("on the tank", turret, 0, 4, 3, PlaceError::Blocked { x: 4, y: 3 }),
        ("two tiles out", plant, 0, 6, 1, PlaceError::TooFar),
        ("next to someone else's base", plant, 1, 4, 1, PlaceError::TooFar),
    ];
    for (what, k, player, x, y, want) in cases {
        let got = g.can_place(player, k, x, y);
        println!("{what}: {got:?}");
        assert_eq!(got, Err(want), "{what}");
    }
    assert_eq!(g.can_place(0, plant, 5, 4), Ok(()), "one empty tile away, on open ground");
}

#[test]
fn refused_placements_change_nothing_but_the_events() {
    let mut a = game_with(None);
    let mut b = game_with(None);
    assert_eq!(place(&mut a, 0, "power_plant", 6, 1), Err(PlaceError::TooFar));
    b.step(1);
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn rock_only_tuning_keeps_buildings_off_open_ground() {
    let rules = tuned(r#"{ "modules": { "placement": { "rock_only": 1 } } }"#);
    let g = game_with(Some(&rules));
    let plant = kind(&g, "power_plant");
    assert_eq!(g.can_place(0, plant, 5, 4), Err(PlaceError::BadGround { x: 5, y: 4 }));
    assert_eq!(g.can_place(0, plant, 4, 1), Ok(()));
}

#[test]
fn max_gap_tuning_widens_how_far_a_base_can_reach() {
    let g = game_with(Some(&tuned(r#"{ "modules": { "placement": { "max_gap": 2 } } }"#)));
    assert_eq!(g.can_place(0, kind(&g, "power_plant"), 6, 1), Ok(()));
}

#[test]
fn a_unit_already_moving_goes_round_a_building_placed_on_its_path() {
    let mut g = game_with(None);
    let tank = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "battle_tank" && e.owner == 0).unwrap().id;
    // Both in the same tick: the move plans a path, then the placement drops a building across it.
    g.order(0, &[tank], CommandOrder::Move { x: 20, y: 9 });
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 4 });
    let footprint = [(5, 4), (6, 4), (5, 5), (6, 5)];
    let mut steps = 0;
    for _ in 0..700 {
        g.step(1);
        let t = g.state.entity(tank).unwrap().tile();
        assert!(!footprint.contains(&(t.x, t.y)), "tick {}: the tank drove into the building", g.state.tick);
        steps += 1;
    }
    assert!(g.events.iter().any(|e| matches!(e, Event::Placed { .. })));
    let end = g.state.entity(tank).unwrap().tile();
    println!("checked {steps} ticks; tank ends at {},{}", end.x, end.y);
    assert_eq!(end, Tile { x: 20, y: 9 });
}

#[test]
fn a_bigger_refinery_moves_its_dock_and_harvesters_still_deliver() {
    let rules = tuned(r#"{ "refinery": { "width": 2, "height": 2 } }"#);
    let mut g = game_with(Some(&rules));
    let harvester = g.state.entities.iter().find(|e| g.rules.kind(e.kind).harvester.is_some()).unwrap();
    assert_eq!(harvester.tile(), Tile { x: 3, y: 4 }, "the dock is below the 2x2 footprint");
    let tank = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "battle_tank").unwrap();
    assert_eq!(tank.tile(), Tile { x: 5, y: 4 }, "the tank starts clear of the footprint");
    g.step(1200);
    for p in &g.state.players {
        println!("player {} credits {}", p.id, p.credits);
        assert!(p.credits >= 200);
    }
}

#[test]
fn placements_replay_from_the_command_log() {
    let mut live = game_with(None);
    let plant = kind(&live, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 4, y: 1 });
    live.step(300);
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 4 });
    live.step(300);
    let log = live.command_log().to_vec();
    let mut replay = game_with(None);
    for t in 0..600 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    let placed = live.events.iter().filter(|e| matches!(e, Event::Placed { .. })).count();
    println!("placed {placed}, live {} replay {}", live.hash(), replay.hash());
    assert_eq!(placed, 2);
    assert_eq!(replay.hash(), live.hash());
}
