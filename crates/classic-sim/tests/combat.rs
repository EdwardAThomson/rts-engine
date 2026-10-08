//! Combat (rules-combat.md): facings, targets, turning, firing, shells, splash, damage, death and death blasts.
//!
//! The map below is all rock with no resource. Each player's starting base is in a top corner, far out of sight of
//! the other's; the tests spawn units in the open middle. A battle tank has 300 health, `heavy` armour and a
//! cannon (range 4 tiles, reload 30, 35 damage, `shell` warhead, 100% against heavy armour), and turns its turret
//! 8 facing units a tick.

use classic_data::{RulesTable, json};
use classic_sim::combat::{facing_to, turn};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Order, Rules, Tile};

const MAP: &str = "\
########################
#1#################2####
########################
########################
########################
########################
########################
########################
########################
########################
########################
########################
";

fn game(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: MAP, seed: 5, players: None, rules }).expect("map is valid")
}

fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn tank(g: &mut Game, owner: u32, x: i32, y: i32) -> u32 {
    let k = kind(g, "battle_tank");
    g.spawn(k, owner, x, y)
}

fn health(g: &Game, id: u32) -> Option<i64> {
    g.state.entity(id).map(|e| e.health)
}

fn destroyed(g: &Game) -> Vec<(u32, u32)> {
    g.events
        .iter()
        .filter_map(|e| if let Event::Destroyed { tick, entity, .. } = *e { Some((tick, entity)) } else { None })
        .collect()
}

fn fired_by(g: &Game, unit: u32) -> usize {
    g.events.iter().filter(|e| matches!(**e, Event::Fired { unit: u, .. } if u == unit)).count()
}

#[test]
fn facings_run_clockwise_from_north_and_turn_the_short_way() {
    let cases = [
        ((0, -1), 0),
        ((1, -1), 32),
        ((1, 0), 64),
        ((1, 1), 96),
        ((0, 1), 128),
        ((-1, 1), 160),
        ((-1, 0), 192),
        ((-1, -1), 224),
    ];
    for ((dx, dy), want) in cases {
        assert_eq!(facing_to(dx * 300, dy * 300), want, "({dx}, {dy})");
    }
    assert_eq!(facing_to(256, -512), 19, "a shallow angle from the table");
    assert_eq!(turn(0, 64, 8), 8);
    assert_eq!(turn(0, 192, 8), 248, "anticlockwise when that is shorter");
    assert_eq!(turn(0, 128, 8), 8, "exactly opposite turns clockwise");
    assert_eq!(turn(250, 4, 8), 2, "across north");
    assert_eq!(turn(250, 2, 8), 2, "arriving");
}

#[test]
fn two_tanks_in_sight_find_each_other_turn_and_trade_shells() {
    let mut g = game(None);
    let a = tank(&mut g, 0, 8, 8);
    let b = tank(&mut g, 1, 11, 8);
    g.step(60);
    let first: Vec<u32> =
        g.events.iter().filter_map(|e| if let Event::Fired { tick, .. } = *e { Some(tick) } else { None }).collect();
    println!("shots at {first:?}; health a {:?} b {:?}", health(&g, a), health(&g, b));
    assert!(fired_by(&g, a) >= 1 && fired_by(&g, b) >= 1, "both fire");
    let a_facing = g.state.entity(a).unwrap().facing;
    assert_eq!((a_facing, g.state.entity(b).unwrap().facing), (64, 192), "facing each other");
    assert!(
        g.events.iter().any(|e| matches!(e, Event::Hit { target, damage: 35, .. } if *target == b)),
        "a full shell hit"
    );
    assert!(health(&g, a).unwrap() < 300 && health(&g, b).unwrap() < 300);
}

#[test]
fn a_destroyed_tank_is_removed_and_its_blast_hurts_both_sides_but_its_own_half_as_much() {
    let mut g = game(None);
    let a = tank(&mut g, 0, 8, 8);
    let b = tank(&mut g, 1, 11, 8);
    // Two tanks parked on b's tile, 100 units either side of its centre: inside the death blast's outer band
    // (25 damage, radius 160) but clear of the killing shell's splash. Their guns are kept quiet.
    let near_own = tank(&mut g, 1, 11, 8);
    let near_enemy = tank(&mut g, 0, 11, 8);
    for (id, dx) in [(near_own, 100), (near_enemy, -100)] {
        let i = g.state.entities.iter().position(|e| e.id == id).unwrap();
        g.state.entities[i].x += dx;
        g.state.entities[i].reload = 10_000;
    }
    let i = g.state.entities.iter().position(|e| e.id == b).unwrap();
    g.state.entities[i].health = 1;
    g.state.entities[i].reload = 10_000;
    g.order(0, &[a], CommandOrder::Attack { target: b });
    let mut steps = 0;
    while g.state.entity(b).is_some() && steps < 200 {
        g.step(1);
        steps += 1;
    }
    let ev = g.events.iter().find(|e| matches!(e, Event::Destroyed { entity, .. } if *entity == b)).expect("destroyed");
    println!("{ev:?} after {steps} ticks");
    assert!(matches!(ev, Event::Destroyed { killer: Some(k), .. } if *k == a));
    assert!(g.state.entity(b).is_none());
    // Half of 25 for the outer band is 12; b's own side takes half of that.
    assert_eq!((health(&g, near_enemy), health(&g, near_own)), (Some(288), Some(294)));
    g.step(1);
    assert_eq!(g.state.entity(a).unwrap().order, Order::Idle, "an attack ends with its target");
}

#[test]
fn two_units_that_kill_each_other_on_the_same_tick_both_get_their_shot() {
    let mut g = game(None);
    let a = tank(&mut g, 0, 8, 8);
    let b = tank(&mut g, 1, 11, 8);
    for (id, facing) in [(a, 64), (b, 192)] {
        let i = g.state.entities.iter().position(|e| e.id == id).unwrap();
        g.state.entities[i].health = 1;
        g.state.entities[i].facing = facing;
    }
    g.order(0, &[a], CommandOrder::Attack { target: b });
    g.order(1, &[b], CommandOrder::Attack { target: a });
    g.step(40);
    let d = destroyed(&g);
    println!("destroyed {d:?}");
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].0, d[1].0, "on the same tick");
}

#[test]
fn an_attack_order_closes_in_until_in_range_then_stands() {
    let mut g = game(None);
    let a = tank(&mut g, 0, 3, 9);
    let b = tank(&mut g, 1, 14, 9);
    let i = g.state.entities.iter().position(|e| e.id == b).unwrap();
    g.state.entities[i].health = 100_000;
    g.order(0, &[a], CommandOrder::Attack { target: b });
    g.step(300);
    let at = g.state.entity(a).unwrap().tile();
    println!("a stopped at {at:?}, fired {}", fired_by(&g, a));
    assert!(fired_by(&g, a) > 0);
    let gap = (14 - at.x).abs();
    assert!((2..=4).contains(&gap), "stood within range, {gap} tiles off");
}

#[test]
fn an_attack_on_a_walled_in_building_fires_from_the_nearest_reachable_tile_in_range() {
    // A gun turret inside a ring of walls two tiles out: every tile beside it is open but can't be reached. A rocket
    // squad (range 3.5 tiles) can still hit it from outside the walls, so it goes there rather than standing still.
    let mut g = game(None);
    let turret = g.spawn(kind(&g, "gun_turret"), 1, 12, 6);
    let wall = kind(&g, "wall");
    for y in 4..=8 {
        for x in 10..=14 {
            if x == 10 || x == 14 || y == 4 || y == 8 {
                g.spawn(wall, 1, x, y);
            }
        }
    }
    let i = g.state.entities.iter().position(|e| e.id == turret).unwrap();
    g.state.entities[i].health = 100_000;
    g.state.entities[i].reload = 10_000;
    let squad = g.spawn(kind(&g, "rocket_squad"), 0, 3, 6);
    g.order(0, &[squad], CommandOrder::Attack { target: turret });
    g.step(900);
    let at = g.state.entity(squad).unwrap().tile();
    println!("the squad stood at {at:?} and fired {} times", fired_by(&g, squad));
    assert!(fired_by(&g, squad) > 0);
    assert!(!(10..=14).contains(&at.x) || !(4..=8).contains(&at.y), "outside the walls");
}

#[test]
fn a_unit_moving_under_orders_ignores_enemies() {
    let mut g = game(None);
    let a = tank(&mut g, 0, 3, 10);
    let _b = tank(&mut g, 1, 12, 8);
    let i = g
        .state
        .entities
        .iter()
        .position(|e| e.owner == 1 && g.rules.kind(e.kind).id == "battle_tank" && e.tile() == Tile { x: 12, y: 8 })
        .unwrap();
    g.state.entities[i].health = 100_000;
    g.state.entities[i].reload = 10_000;
    g.order(0, &[a], CommandOrder::Move { x: 20, y: 10 });
    g.step(250);
    assert_eq!(fired_by(&g, a), 0, "drove straight past");
    assert_eq!(g.state.entity(a).unwrap().tile(), Tile { x: 20, y: 10 });
}

#[test]
fn a_rocket_turret_needs_power_and_a_gun_turret_does_not() {
    // Plants tuned to make nothing, so both players are short of power.
    let rules = tuned(r#"{ "power_plant": { "power": 0 } }"#);
    let mut g = game(Some(&rules));
    let rocket = g.spawn(kind(&g, "rocket_turret"), 0, 6, 7);
    let gun = g.spawn(kind(&g, "gun_turret"), 0, 6, 10);
    let b = tank(&mut g, 1, 9, 8);
    let c = tank(&mut g, 1, 9, 10);
    for id in [b, c] {
        let i = g.state.entities.iter().position(|e| e.id == id).unwrap();
        g.state.entities[i].health = 100_000;
        g.state.entities[i].reload = 10_000;
    }
    g.step(120);
    println!("rocket fired {}, gun fired {}", fired_by(&g, rocket), fired_by(&g, gun));
    assert_eq!(fired_by(&g, rocket), 0);
    assert!(fired_by(&g, gun) > 0);
}

#[test]
fn a_destroyed_building_frees_its_tiles() {
    let mut g = game(None);
    let plant = g.spawn(kind(&g, "power_plant"), 1, 10, 8);
    assert!(!g.pathfinder.passable(10, 8));
    let a = tank(&mut g, 0, 6, 8);
    let i = g.state.entities.iter().position(|e| e.id == plant).unwrap();
    g.state.entities[i].health = 30;
    g.order(0, &[a], CommandOrder::Attack { target: plant });
    g.step(100);
    assert!(g.state.entity(plant).is_none(), "{:?}", destroyed(&g));
    assert!(g.pathfinder.passable(10, 8) && g.pathfinder.passable(11, 9));
}

#[test]
fn guards_prefer_armed_units_to_buildings() {
    let mut g = game(None);
    g.spawn(kind(&g, "silo"), 1, 10, 7);
    let a = tank(&mut g, 0, 8, 8);
    let b = tank(&mut g, 1, 12, 9);
    g.step(9);
    assert_eq!(g.state.entity(a).unwrap().target, Some(b));
}

#[test]
fn a_pack_can_tune_weapons_and_the_damage_table() {
    let rules =
        tuned(r#"{ "weapons": { "cannon": { "damage": 70 } }, "modules": { "combat": { "shell_vs_heavy": 50 } } }"#);
    let mut g = game(Some(&rules));
    tank(&mut g, 0, 8, 8);
    let b = tank(&mut g, 1, 11, 8);
    g.step(60);
    let hits: Vec<i64> = g
        .events
        .iter()
        .filter_map(|e| match *e {
            Event::Hit { target, damage, .. } if target == b => Some(damage),
            _ => None,
        })
        .collect();
    assert!(hits.contains(&35), "70 at 50%: {hits:?}");
}

#[test]
fn combat_replays_from_the_command_log() {
    let setup = |g: &mut Game| {
        for (o, x, y) in [(0, 6, 7), (0, 6, 9), (1, 14, 8), (1, 15, 10)] {
            tank(g, o, x, y);
        }
    };
    let mut live = game(None);
    setup(&mut live);
    let ids: Vec<u32> =
        live.state.entities.iter().filter(|e| live.rules.kind(e.kind).id == "battle_tank").map(|e| e.id).collect();
    live.order(0, &ids[..1], CommandOrder::Move { x: 10, y: 8 });
    live.step(100);
    live.order(1, &[ids[ids.len() - 1]], CommandOrder::Attack { target: ids[0] });
    live.step(400);
    let log = live.command_log().to_vec();
    let mut replay = game(None);
    setup(&mut replay);
    for t in 0..500 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    println!("destroyed {:?}; live {} replay {}", destroyed(&live), live.hash(), replay.hash());
    assert!(live.events.iter().any(|e| matches!(e, Event::Hit { .. })));
    assert_eq!(replay.hash(), live.hash());
}
