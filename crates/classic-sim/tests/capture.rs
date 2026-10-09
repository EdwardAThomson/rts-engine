//! Capture (rules-base-building-power.md, "Capture"): infantry walk up to an enemy building hurt below a quarter of
//! its health and take it over, going inside. Healthier buildings, and kinds that can't be taken, refuse the order;
//! a building that heals on the way sends the infantry back to guard. Under fog, only a building the player knows of
//! can be named.

use classic_sim::world::{CaptureError, Event};
use classic_sim::{CommandOrder, FogRules, Game, GameOptions, Kind, Order, Rules};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game_with(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules }).unwrap()
}

fn game() -> Game {
    game_with(None)
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn first(g: &Game, owner: u32, id: &str) -> u32 {
    let k = kind(g, id);
    g.state.entities.iter().find(|e| e.owner == owner && e.kind == k).unwrap().id
}

fn hurt(g: &mut Game, id: u32, health: i64) {
    let i = g.state.entities.iter().position(|e| e.id == id).unwrap();
    g.state.entities[i].health = health;
}

fn refused(g: &Game) -> Vec<CaptureError> {
    g.events
        .iter()
        .filter_map(|e| match *e {
            Event::CaptureRefused { reason, .. } => Some(reason),
            _ => None,
        })
        .collect()
}

/// Step until `id` changes hands, or `limit` ticks.
fn until_captured(g: &mut Game, id: u32, limit: u32) -> Option<u32> {
    for _ in 0..limit {
        g.step(1);
        if g.events.iter().any(|e| matches!(*e, Event::Captured { entity, .. } if entity == id)) {
            return Some(g.state.tick);
        }
    }
    None
}

#[test]
fn infantry_take_a_badly_damaged_enemy_building_and_its_power() {
    let mut g = game();
    let plant = first(&g, 1, "power_plant");
    let soldier = g.spawn(kind(&g, "infantry"), 0, 21, 16);
    let tank = first(&g, 0, "battle_tank");
    hurt(&mut g, plant, 100);
    let supply = g.power(0).supply;
    assert_eq!(g.can_capture(0, plant), Ok(()));
    g.order(0, &[tank], CommandOrder::Attack { target: plant });
    g.order(0, &[soldier], CommandOrder::Capture { target: plant });
    g.step(1);
    assert_eq!(g.state.entity(soldier).unwrap().order, Order::Capture);
    let at = until_captured(&mut g, plant, 600).expect("captured");
    println!("captured on tick {at}");
    let p = g.state.entity(plant).unwrap();
    assert_eq!(p.owner, 0);
    assert!(p.health <= 100 && p.health > 0, "at its current health");
    assert!(g.state.entity(soldier).is_none(), "the infantry went inside");
    assert!(g.events.iter().any(|e| matches!(*e, Event::Captured { entity, from: 1, to: 0, by, .. }
        if entity == plant && by == soldier)));
    assert_eq!(g.power(0).supply, supply + 100 * p.health / 500);
    let t = g.state.entity(tank).unwrap();
    assert_eq!((t.order, t.target), (Order::Idle, None), "the tank stops shooting what is now ours");
}

#[test]
fn a_captured_factory_loses_its_queue_with_no_refund() {
    let mut g = game();
    let yard = first(&g, 1, "construction_yard");
    let soldier = g.spawn(kind(&g, "rocket_infantry"), 0, 28, 16);
    hurt(&mut g, yard, 200);
    g.order(1, &[], CommandOrder::Produce { kind: kind(&g, "power_plant") });
    g.order(0, &[soldier], CommandOrder::Capture { target: yard });
    until_captured(&mut g, yard, 600).expect("captured");
    let y = g.state.entity(yard).unwrap();
    assert_eq!(y.owner, 0);
    assert!(y.queue.is_empty());
    let credits = (g.state.players[0].credits, g.state.players[1].credits);
    g.step(30);
    assert_eq!((g.state.players[0].credits, g.state.players[1].credits), credits, "nobody was paid back");
    assert!(g.events.iter().all(|e| e.name() != "production_cancelled"));
}

#[test]
fn healthy_buildings_and_kinds_that_cant_be_taken_refuse_the_order() {
    let mut g = game();
    let plant = first(&g, 1, "power_plant");
    let soldier = g.spawn(kind(&g, "infantry"), 0, 21, 16);
    hurt(&mut g, plant, 125);
    assert_eq!(g.can_capture(0, plant), Err(CaptureError::TooHealthy), "25% is not below 25%");
    g.order(0, &[soldier], CommandOrder::Capture { target: plant });
    g.step(1);
    assert_eq!(refused(&g), [CaptureError::TooHealthy]);
    assert_eq!(g.state.entity(soldier).unwrap().order, Order::Idle, "it never set off");

    let turret = g.spawn(kind(&g, "gun_turret"), 1, 23, 14);
    hurt(&mut g, turret, 1);
    let own = first(&g, 0, "power_plant");
    hurt(&mut g, own, 1);
    let tank = first(&g, 1, "battle_tank");
    hurt(&mut g, tank, 1);
    for target in [turret, own, tank] {
        assert_eq!(g.can_capture(0, target), Err(CaptureError::NotCapturable));
    }
    hurt(&mut g, plant, 1);
    let tank0 = first(&g, 0, "battle_tank");
    g.events.clear();
    g.order(0, &[tank0], CommandOrder::Capture { target: plant });
    g.step(1);
    assert_eq!(refused(&g), [CaptureError::NoCapturer], "tanks can't capture");
}

#[test]
fn a_building_that_heals_on_the_way_sends_the_infantry_back_to_guard() {
    let mut g = game();
    let plant = first(&g, 1, "power_plant");
    let soldier = g.spawn(kind(&g, "infantry"), 0, 12, 16);
    hurt(&mut g, plant, 100);
    g.order(0, &[soldier], CommandOrder::Capture { target: plant });
    g.step(10);
    hurt(&mut g, plant, 300);
    g.step(1);
    assert_eq!(refused(&g), [CaptureError::TooHealthy]);
    let s = g.state.entity(soldier).unwrap();
    assert_eq!((s.order, s.goal), (Order::Idle, None));
    assert_eq!(g.state.entity(plant).unwrap().owner, 1);
}

#[test]
fn under_fog_only_a_known_building_can_be_named_and_a_pack_can_switch_capture_off() {
    let fog = Some(FogRules { hide: true, reveal_ticks: 45, start_explored: false });
    let rules = Rules { fog, ..Rules::default() };
    let mut g = game_with(Some(&rules));
    let plant = first(&g, 1, "power_plant");
    let soldier = g.spawn(kind(&g, "infantry"), 0, 2, 8);
    hurt(&mut g, plant, 100);
    assert!(!g.known(0, plant));
    g.order(0, &[soldier], CommandOrder::Capture { target: plant });
    g.step(1);
    assert_eq!(g.state.entity(soldier).unwrap().order, Order::Idle, "an unseen building can't be named");

    let rules = Rules { capture: None, ..Rules::default() };
    let mut g = game_with(Some(&rules));
    let plant = first(&g, 1, "power_plant");
    let soldier = g.spawn(kind(&g, "infantry"), 0, 21, 16);
    hurt(&mut g, plant, 100);
    g.order(0, &[soldier], CommandOrder::Capture { target: plant });
    g.step(300);
    assert_eq!(refused(&g)[0], CaptureError::Off);
    assert_eq!(g.state.entity(plant).unwrap().owner, 1);
}

#[test]
fn capture_replays_to_the_same_hash() {
    let run = || {
        let mut g = game();
        let plant = first(&g, 1, "power_plant");
        let soldiers: Vec<u32> = (0..3).map(|i| g.spawn(kind(&g, "infantry"), 0, 20 + i, 16)).collect();
        hurt(&mut g, plant, 100);
        g.order(0, &soldiers, CommandOrder::Capture { target: plant });
        g.step(300);
        (g.hash(), g.state.entity(plant).unwrap().owner)
    };
    let (a, b) = (run(), run());
    println!("{a:?}");
    assert_eq!(a, b);
    assert_eq!(a.1, 0);
}
