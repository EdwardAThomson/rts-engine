//! The radar rule (rules-world.md, section 4; the `radar` module): with it on, a player has the minimap only while
//! they own a building that gives radar and their power supply meets demand. Off, everyone always has it. Radar is
//! read from the state, so turning it on changes no state hash.

use classic_data::RulesTable;
use classic_sim::{Game, GameOptions, Rules};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn rules(radar: bool) -> Rules {
    let mut t = RulesTable::builtin();
    t.modules.get_mut("radar").unwrap().numbers.get_mut("on").unwrap().value = radar as i64;
    Rules::from_table(&t).unwrap()
}

fn game(rules: &Rules) -> Game {
    Game::new(GameOptions { map: MAP, seed: 3, players: None, rules: Some(rules) }).expect("map is valid")
}

#[test]
fn radar_needs_the_building_and_enough_power() {
    let on = rules(true);
    let mut g = game(&on);
    assert!(!g.radar(0) && !g.radar(1), "nobody starts with a radar building");
    let radar = g.kind("radar").unwrap();
    let id = g.spawn(radar, 0, 2, 12);
    let p = g.power(0);
    println!("with a radar: power {}/{}", p.supply, p.demand);
    assert!(g.radar(0), "a radar with power to spare");
    assert!(!g.radar(1), "one player's radar is theirs alone");
    // More demand than supply switches it off; another power plant switches it back on.
    let hungry = g.kind("radar").unwrap();
    let mut extra = Vec::new();
    while !g.power(0).is_short() {
        extra.push(g.spawn(hungry, 0, 2 + 2 * extra.len() as i32, 16));
    }
    let p = g.power(0);
    println!("overloaded: power {}/{}", p.supply, p.demand);
    assert!(!g.radar(0), "low power takes the radar away");
    let plant = g.kind("power_plant").unwrap();
    while g.power(0).is_short() {
        g.spawn(plant, 0, 2 + 2 * extra.len() as i32, 20);
        extra.push(0);
    }
    assert!(g.radar(0), "power restored, radar back");
    // Losing every radar building loses the radar.
    g.state.entities.retain(|e| e.owner != 0 || e.kind != radar || e.id == id);
    assert!(g.radar(0));
    g.state.entities.retain(|e| e.id != id);
    assert!(!g.radar(0), "no radar building, no radar");
}

#[test]
fn with_the_module_off_everyone_has_radar_and_the_hashes_stay() {
    let off = rules(false);
    let on = rules(true);
    let (mut a, mut b) = (game(&off), game(&on));
    assert!(a.radar(0) && a.radar(1));
    a.step(300);
    b.step(300);
    assert_eq!(a.hash(), b.hash(), "radar is read from the state, not kept in it");
}
