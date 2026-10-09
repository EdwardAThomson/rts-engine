//! Repair (rules-base-building-power.md, "Repairing buildings"): an own building mends a step at a time for credits,
//! slower while its owner is short of power, waiting while they can't pay; a damaged vehicle sent to an own repair
//! pad parks beside it and is mended there, one vehicle at a time per pad.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Order};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game() -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: None }).unwrap()
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// Player 0's first entity of this kind.
fn first(g: &Game, owner: u32, id: &str) -> u32 {
    let k = kind(g, id);
    g.state.entities.iter().find(|e| e.owner == owner && e.kind == k).unwrap().id
}

fn hurt(g: &mut Game, id: u32, health: i64) {
    let i = g.state.entities.iter().position(|e| e.id == id).unwrap();
    g.state.entities[i].health = health;
}

fn health(g: &Game, id: u32) -> i64 {
    g.state.entity(id).unwrap().health
}

fn count(g: &Game, name: &str) -> usize {
    g.events.iter().filter(|e| e.name() == name).count()
}

#[test]
fn a_building_mends_step_by_step_for_credits_and_stops_when_whole() {
    let mut g = game();
    let refinery = first(&g, 0, "refinery");
    hurt(&mut g, refinery, 900 - 18 * 10);
    let credits = g.state.players[0].credits;
    g.order(0, &[refinery], CommandOrder::Repair { on: true });
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(*e, Event::RepairStarted { entity, owner: 0, .. } if entity == refinery)));
    assert!(g.state.entity(refinery).unwrap().repairing);
    let start = g.state.tick;
    while g.state.entity(refinery).unwrap().repairing {
        g.step(1);
        assert!(g.state.tick - start < 500, "repair finishes");
    }
    let took = g.state.tick - start;
    let paid = credits - g.state.players[0].credits;
    println!("ten steps of 18 health in {took} ticks for {paid} credits");
    // A step is max(1, 900 / 50) = 18 health every 5 ticks, costing ceil(400 * 18 * 50% / 900) = 4 credits.
    assert_eq!(health(&g, refinery), 900);
    assert_eq!(paid, 10 * 4);
    assert!((49..=52).contains(&took), "ten steps, five ticks apart");
    assert!(
        g.events.iter().any(|e| matches!(*e, Event::RepairStopped { entity, whole: true, .. } if entity == refinery))
    );
}

#[test]
fn repair_waits_without_credits_slows_when_short_and_can_be_turned_off() {
    let mut g = game();
    let refinery = first(&g, 0, "refinery");
    hurt(&mut g, refinery, 450);
    g.state.players[0].credits = 0;
    g.order(0, &[refinery], CommandOrder::Repair { on: true });
    g.step(60);
    assert_eq!(health(&g, refinery), 450, "nothing mended on credit");
    assert!(g.state.entity(refinery).unwrap().repairing, "but repair stays on");
    assert_eq!(g.state.players[0].credits, 0, "credits never go below zero");
    g.state.players[0].credits = 1000;
    g.step(50);
    let at_full_power = health(&g, refinery) - 450;
    assert!(at_full_power >= 9 * 18, "it resumes by itself once paid: +{at_full_power}");

    // A plant at half health gives 50 for a demand of 30: no shortfall. Take it lower, and repair slows down.
    let mut g = game();
    let (refinery, plant) = (first(&g, 0, "refinery"), first(&g, 0, "power_plant"));
    hurt(&mut g, refinery, 450);
    hurt(&mut g, plant, 100);
    assert!(g.power(0).is_short());
    let factor = g.power(0).factor(&g.rules);
    g.order(0, &[refinery], CommandOrder::Repair { on: true });
    g.step(101);
    let short = health(&g, refinery) - 450;
    println!("mended {at_full_power} in 50 ticks at full power, {short} in 100 at a factor of {factor}%");
    assert!(short < 2 * at_full_power, "a shortfall stretches the steps");

    g.order(0, &[refinery], CommandOrder::Repair { on: false });
    g.step(1);
    let left = health(&g, refinery);
    g.step(30);
    assert_eq!(health(&g, refinery), left, "off means off");
    assert!(g.events.iter().any(|e| matches!(*e, Event::RepairStopped { whole: false, .. })));
}

#[test]
fn only_an_owner_repairs_and_a_whole_building_ignores_the_order() {
    let mut g = game();
    let theirs = first(&g, 1, "refinery");
    let mine = first(&g, 0, "refinery");
    hurt(&mut g, theirs, 100);
    g.order(0, &[theirs, mine], CommandOrder::Repair { on: true });
    g.step(20);
    assert_eq!(health(&g, theirs), 100);
    assert!(!g.state.entity(mine).unwrap().repairing, "already whole");
    assert_eq!(count(&g, "repair_started"), 0);
}

#[test]
fn a_repair_pad_mends_one_vehicle_at_a_time_and_sends_a_harvester_back_to_work() {
    let mut g = game();
    let pad = g.spawn(kind(&g, "repair_pad"), 0, 10, 2);
    let (harvester, tank) = (first(&g, 0, "harvester"), first(&g, 0, "battle_tank"));
    hurt(&mut g, harvester, 450 - 8 * 5);
    hurt(&mut g, tank, 300 - 8 * 25);
    let credits = g.state.players[0].credits;
    let stored = |g: &Game| g.state.players[0].delivered - g.state.players[0].lost;
    let stored_before = stored(&g);
    g.order(0, &[tank, harvester], CommandOrder::RepairAt { pad });
    g.step(1);
    assert_eq!(g.state.entity(tank).unwrap().order, Order::Repair);
    assert_eq!(g.state.entity(harvester).unwrap().order, Order::Repair);
    // Whoever gets there first is mended while the other waits; then the other.
    let mut done: Vec<(u32, i64)> = Vec::new();
    for _ in 0..800 {
        g.step(1);
        for e in &g.events {
            if let Event::UnitRepaired { unit, pad: p, owner: 0, .. } = *e {
                assert_eq!(p, pad);
                let other = if unit == tank { harvester } else { tank };
                done.push((unit, health(&g, other)));
            }
        }
        g.events.clear();
        if done.len() == 2 {
            break;
        }
    }
    println!("mended in turn: {done:?}");
    assert_eq!(done.len(), 2, "both were mended");
    let (first, other_then) = done[0];
    let waited = if first == tank { 450 - 8 * 5 } else { 300 - 8 * 25 };
    assert_eq!(other_then, waited, "the second waited its turn");
    assert_eq!((health(&g, harvester), health(&g, tank)), (450, 300));
    // A step of 8 health costs ceil(cost * 8 * 50% / max_health): 3 for the harvester, 8 for the tank.
    let paid = credits - g.state.players[0].credits + (stored(&g) - stored_before);
    assert_eq!(paid, 5 * 3 + 25 * 8);
    assert_eq!(g.state.entity(tank).unwrap().order, Order::Idle);
    assert_eq!(g.state.entity(harvester).unwrap().order, Order::Harvest, "back to the fields");
    // Nobody stood inside the pad.
    let p = g.state.entity(pad).unwrap().tile();
    for id in [harvester, tank] {
        let u = g.state.entity(id).unwrap().tile();
        assert!(!((p.x..p.x + 3).contains(&u.x) && (p.y..p.y + 2).contains(&u.y)));
    }
}

#[test]
fn a_pad_takes_only_its_owners_damaged_vehicles() {
    let mut g = game();
    let pad = g.spawn(kind(&g, "repair_pad"), 0, 10, 2);
    let whole = first(&g, 0, "battle_tank");
    let enemy = first(&g, 1, "battle_tank");
    let infantry = g.spawn(kind(&g, "infantry"), 0, 8, 8);
    hurt(&mut g, enemy, 10);
    hurt(&mut g, infantry, 10);
    g.order(0, &[whole, infantry], CommandOrder::RepairAt { pad });
    g.order(1, &[enemy], CommandOrder::RepairAt { pad });
    g.step(1);
    for id in [whole, enemy, infantry] {
        assert_ne!(g.state.entity(id).unwrap().order, Order::Repair);
    }
}

#[test]
fn repair_orders_replay_to_the_same_hash() {
    let run = || {
        let mut g = game();
        let pad = g.spawn(kind(&g, "repair_pad"), 0, 10, 2);
        let (refinery, tank) = (first(&g, 0, "refinery"), first(&g, 0, "battle_tank"));
        hurt(&mut g, refinery, 300);
        hurt(&mut g, tank, 120);
        g.order(0, &[refinery], CommandOrder::Repair { on: true });
        g.order(0, &[tank], CommandOrder::RepairAt { pad });
        g.step(400);
        g.hash()
    };
    assert_eq!(run(), run());
}
