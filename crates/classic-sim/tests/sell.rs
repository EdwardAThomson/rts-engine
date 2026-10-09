//! Selling (rules-base-building-power.md, "Selling"): a building sold stops working for `sell.ticks` ticks, then
//! goes and pays back half its cost scaled by health, plus what its queue had paid. Destroyed first, it pays
//! nothing; with the module off, the order does nothing.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Kind, Rules};

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

fn sold(g: &Game, id: u32) -> Option<i64> {
    g.events.iter().find_map(|e| match *e {
        Event::BuildingSold { entity, refund, .. } if entity == id => Some(refund),
        _ => None,
    })
}

#[test]
fn a_sold_building_stops_working_then_goes_for_half_its_cost_by_health() {
    let mut g = game();
    let plant = first(&g, 0, "power_plant");
    let i = g.state.entities.iter().position(|e| e.id == plant).unwrap();
    g.state.entities[i].health = 400;
    let credits = g.state.players[0].credits;
    let supply = g.power(0).supply;
    assert_eq!(g.sell_refund(plant), 300 * 400 / 500 / 2);
    g.order(0, &[plant], CommandOrder::Sell);
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(*e, Event::SellStarted { entity, owner: 0, .. } if entity == plant)));
    assert_eq!(g.power(0).supply, 0, "no power while being sold");
    assert!(supply > 0);
    let t = g.state.entity(plant).unwrap().tile();
    assert!(!g.pathfinder.passable(t.x, t.y));
    g.step(13);
    assert!(g.state.entity(plant).is_some(), "it takes its time");
    assert_eq!(g.state.players[0].credits, credits);
    g.step(1);
    println!("sold on tick {}", g.state.tick);
    assert!(g.state.entity(plant).is_none());
    assert_eq!(sold(&g, plant), Some(120));
    assert_eq!(g.state.players[0].credits, credits + 120);
    assert!(g.pathfinder.passable(t.x, t.y), "its ground is open again");
}

#[test]
fn a_factory_sold_mid_build_refunds_what_its_queue_paid_and_builds_no_more() {
    let mut g = game();
    let yard = first(&g, 0, "construction_yard");
    g.order(0, &[], CommandOrder::Produce { kind: kind(&g, "power_plant") });
    g.step(100);
    let entry = g.state.entity(yard).unwrap().queue[0];
    assert_eq!(entry.state, EntryState::Building);
    assert!(entry.paid > 0);
    let credits = g.state.players[0].credits;
    g.order(0, &[yard], CommandOrder::Sell);
    g.step(6);
    assert_eq!(g.state.entity(yard).unwrap().queue[0], entry, "no building while being sold");
    g.step(20);
    // The yard costs nothing to build, so its own refund is 0: only the queue is paid back.
    assert_eq!(sold(&g, yard), Some(entry.paid));
    assert_eq!(g.state.players[0].credits, credits + entry.paid);
    // A player may sell their last yard; production then has nowhere to go.
    g.order(0, &[], CommandOrder::Produce { kind: kind(&g, "power_plant") });
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(e, Event::ProductionRejected { player: 0, .. })));
}

#[test]
fn a_building_destroyed_while_being_sold_pays_nothing_and_armed_ones_hold_fire() {
    let mut g = game();
    let refinery = first(&g, 0, "refinery");
    let turret = g.spawn(kind(&g, "gun_turret"), 0, 8, 2);
    let enemy = g.spawn(kind(&g, "battle_tank"), 1, 9, 2);
    g.order(0, &[refinery, turret], CommandOrder::Sell);
    g.step(1);
    let i = g.state.entities.iter().position(|e| e.id == refinery).unwrap();
    g.state.entities[i].health = 0;
    let credits = g.state.players[0].credits;
    g.step(20);
    assert!(sold(&g, refinery).is_none());
    assert!(g.events.iter().any(|e| matches!(*e, Event::Destroyed { entity, .. } if entity == refinery)));
    assert!(
        !g.events.iter().any(|e| matches!(*e, Event::Fired { unit, .. } if unit == turret)),
        "the turret held fire"
    );
    assert!(sold(&g, turret).is_some(), "turrets can be sold");
    assert!(g.state.entity(enemy).is_some());
    assert!(g.state.players[0].credits >= credits);
}

#[test]
fn only_an_owner_sells_and_a_pack_can_switch_selling_off() {
    let mut g = game();
    let theirs = first(&g, 1, "power_plant");
    g.order(0, &[theirs], CommandOrder::Sell);
    g.step(30);
    assert!(g.state.entity(theirs).is_some());

    let rules = Rules { sell: None, ..Rules::default() };
    let mut g = game_with(Some(&rules));
    let plant = first(&g, 0, "power_plant");
    assert_eq!(g.sell_refund(plant), 0);
    g.order(0, &[plant], CommandOrder::Sell);
    g.step(30);
    assert!(g.state.entity(plant).is_some());
    assert!(g.events.iter().all(|e| e.name() != "sell_started"));
}
