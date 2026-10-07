//! Power (rules-base-building-power.md, "Power"): buildings add to or draw from their owner's supply, a producer
//! gives power in proportion to its health, the power factor never drops below its floor, and `power_changed`
//! reports a player's totals whenever they differ from the previous tick's.
//!
//! On the base map below, player 0 starts with its yard at (1, 1), its power plant at (3, 1) and its 3x2 refinery at
//! (1, 3); player 1's base starts at (12, 1).

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Power, QueueEntry, Rules};

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

fn game_with(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: BASE, seed: 1, players: None, rules }).expect("map is valid")
}

/// Order a placement, run the tick, and return the power reports it made as (player, supply, demand, shortfall).
/// The building is put ready at the player's yard first, as production would.
fn place(g: &mut Game, player: u32, id: &str, x: i32, y: i32) -> Vec<(u32, i64, i64, i64)> {
    let (yard, kind) = (g.kind("construction_yard").unwrap(), g.kind(id).unwrap());
    let at = g.state.entities.iter_mut().find(|e| e.owner == player && e.kind == yard).unwrap();
    at.queue.push(QueueEntry { item: kind, state: EntryState::Ready, progress: 0, paid: 0 });
    g.order(player, &[], CommandOrder::Place { kind, x, y });
    let from = g.events.len();
    g.step(1);
    let events = &g.events[from..];
    assert!(events.iter().any(|e| matches!(e, Event::BuildingPlaced { .. })), "{id} at {x},{y}: {events:?}");
    power_events(events)
}

fn power_events(events: &[Event]) -> Vec<(u32, i64, i64, i64)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::PowerChanged { player, supply, demand, shortfall, .. } => Some((player, supply, demand, shortfall)),
            _ => None,
        })
        .collect()
}

fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

#[test]
fn a_starting_base_has_a_margin_of_seventy() {
    let g = game_with(None);
    let p = g.power(0);
    println!("start: {p:?}, factor {}%", p.factor(&g.rules));
    assert_eq!(p, Power { supply: 100, demand: 30 }, "one plant against a refinery; the yard draws nothing");
    assert_eq!((p.shortfall(), p.is_short(), p.factor(&g.rules)), (0, false, 100));
    assert_eq!(g.snapshot().power, vec![p, p], "both players start the same");
}

#[test]
fn consumers_draw_the_supply_down_until_the_base_is_short() {
    let mut g = game_with(None);
    // A heavy factory (35) and a radar (30) take demand to 95, then a second radar to 125: short by 25.
    assert_eq!(place(&mut g, 0, "heavy_factory", 5, 1), [(0, 100, 65, 0)]);
    assert_eq!(place(&mut g, 0, "radar", 8, 1), [(0, 100, 95, 0)]);
    assert_eq!(place(&mut g, 0, "radar", 5, 3), [(0, 100, 125, 25)]);
    let p = g.power(0);
    println!("after three buildings: {p:?}, factor {}%", p.factor(&g.rules));
    assert_eq!(p.factor(&g.rules), 80);
    assert_eq!(g.power(1), Power { supply: 100, demand: 30 }, "player 1 is untouched");
    assert_eq!(place(&mut g, 0, "power_plant", 8, 3), [(0, 200, 125, 0)], "a second plant covers it");
}

#[test]
fn power_changed_fires_only_on_a_change() {
    let mut g = game_with(None);
    assert!(place(&mut g, 0, "wall", 5, 1).is_empty(), "a wall neither makes nor uses power");
    let from = g.events.len();
    g.step(300);
    assert!(power_events(&g.events[from..]).is_empty(), "nothing changed for 300 ticks");
}

#[test]
fn a_damaged_power_plant_gives_power_in_proportion_to_its_health() {
    let mut g = game_with(None);
    let plant = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "power_plant").unwrap().id;
    let max = g.rules.kind(g.state.entity(plant).unwrap().kind).max_health;
    let i = g.state.entities.iter().position(|e| e.id == plant).unwrap();
    g.state.entities[i].health = max / 4;
    assert_eq!(g.power(0), Power { supply: 25, demand: 30 });
    assert_eq!(g.power(0).factor(&g.rules), 83);
}

#[test]
fn power_numbers_and_the_floor_come_from_the_rules_and_a_pack_can_tune_them() {
    let g = game_with(Some(&tuned(r#"{ "refinery": { "power": -500 } }"#)));
    assert_eq!(g.power(0), Power { supply: 100, demand: 500 });
    assert_eq!(g.power(0).factor(&g.rules), 25, "20% is below the default floor");
    let g =
        game_with(Some(&tuned(r#"{ "refinery": { "power": -500 }, "modules": { "power": { "min_factor": 10 } } }"#)));
    assert_eq!(g.power(0).factor(&g.rules), 20);
}
