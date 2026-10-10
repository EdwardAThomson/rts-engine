//! Resource blooms (rules-world.md, section 6): a bloom bursts when a ground unit drives onto it, when a burst lands
//! on it or when it grows too old, spreading resource over a disc of radius 3 (300 in the middle, 70 less on each
//! ring) and hurting the units beside it; a new one is seeded near the old point after a random wait. With the
//! module off nothing happens and regrowth runs as before.
//!
//! The map is open ground with player 0's rock plateau on the left, player 1's on the right and a bloom point (`*`)
//! at (12, 5).

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Rules};

const MAP: &str = "\
#######...............#######
#1#####...............#####2#
#######...............#######
#######...............#######
#######...............#######
#######.....*.........#######
#######...............#######
#######...............#######
#######...............#######
#######...............#######
";

fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn blooms_on() -> Rules {
    tuned(r#"{ "modules": { "blooms": { "on": 1 } } }"#)
}

fn game(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: MAP, seed: 3, players: Some(2), rules }).unwrap()
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn bursts(g: &Game) -> Vec<(u32, i32, i32, i64)> {
    g.events
        .iter()
        .filter_map(|e| match *e {
            Event::BloomBurst { tick, x, y, added } => Some((tick, x, y, added)),
            _ => None,
        })
        .collect()
}

fn resource(g: &Game, x: i32, y: i32) -> i64 {
    g.state.resource[g.map.index(x, y)]
}

#[test]
fn a_unit_driving_onto_a_bloom_bursts_it_into_a_field_and_is_hurt() {
    let rules = blooms_on();
    let mut g = game(Some(&rules));
    assert_eq!(g.state.blooms.as_ref().unwrap().list.len(), 1, "one bloom on the map's one point");
    let tank = g.spawn(kind(&g, "battle_tank"), 0, 9, 5);
    let beside = g.spawn(kind(&g, "battle_tank"), 0, 13, 6);
    let health = g.state.entity(tank).unwrap().health;
    g.order(0, &[tank], CommandOrder::Move { x: 12, y: 5 });
    for _ in 0..200 {
        g.step(1);
        if !bursts(&g).is_empty() {
            break;
        }
    }
    let b = bursts(&g);
    println!("bursts {b:?}");
    assert_eq!(b.len(), 1);
    assert_eq!((b[0].1, b[0].2), (12, 5));
    // 37 tiles in the disc: 300 in the middle, 230 on the first ring, 160 on the second, 90 on the third.
    let field: Vec<(i32, i32)> =
        (0..10).flat_map(|y| (0..29).map(move |x| (x, y))).filter(|&(x, y)| resource(&g, x, y) > 0).collect();
    assert_eq!(field.len(), 37);
    assert_eq!(
        (resource(&g, 12, 5), resource(&g, 13, 5), resource(&g, 14, 6), resource(&g, 15, 5)),
        (300, 230, 160, 90)
    );
    assert_eq!(resource(&g, 15, 7), 0, "outside the disc");
    assert_eq!(b[0].3, 300 + 8 * 230 + 16 * 160 + 12 * 90);
    assert_eq!(g.state.entity(tank).unwrap().health, health - 50);
    assert_eq!(g.state.entity(beside).unwrap().health, health - 50, "the one beside it too");
    assert!(g.state.blooms.as_ref().unwrap().list.is_empty());
    assert!(!g.events.iter().any(|e| matches!(e, Event::Hit { .. })), "not an attack");
}

#[test]
fn a_bloom_bursts_of_old_age_and_a_new_one_grows_near_its_point() {
    let rules = tuned(
        r#"{ "modules": { "blooms": { "on": 1, "max_age_ticks": 100, "reseed_min_ticks": 50, "reseed_max_ticks": 80 } } }"#,
    );
    let mut g = game(Some(&rules));
    g.step(101);
    let b = bursts(&g);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 100, "on the tick it reached its age");
    g.step(100);
    let seeded: Vec<(u32, i32, i32)> = g
        .events
        .iter()
        .filter_map(|e| if let Event::BloomSeeded { tick, x, y } = *e { Some((tick, x, y)) } else { None })
        .collect();
    println!("seeded {seeded:?}");
    assert_eq!(seeded.len(), 1);
    let (t, x, y) = seeded[0];
    assert!((150..=180).contains(&t), "after a wait of 50 to 80 ticks");
    assert!((x - 12).abs() <= 8 && (y - 5).abs() <= 8, "within 8 tiles of the old point");
    assert_eq!(resource(&g, x, y), 0, "on a tile with no resource");
    assert_eq!(g.state.blooms.as_ref().unwrap().list.len(), 1);
}

#[test]
fn a_burst_landing_on_a_bloom_sets_it_off() {
    let rules = blooms_on();
    let mut g = game(Some(&rules));
    // A burst one tile off does nothing; one on the bloom's tile (as combat reports each ground burst) sets it off.
    classic_sim::blooms::shot(&mut g.state, 13 * 256 + 128, 5 * 256 + 128);
    g.step(1);
    assert!(bursts(&g).is_empty());
    classic_sim::blooms::shot(&mut g.state, 12 * 256 + 40, 5 * 256 + 200);
    g.step(1);
    assert_eq!(bursts(&g).len(), 1);
}

#[test]
fn with_blooms_off_the_map_point_is_plain_ground_and_regrowth_runs() {
    let mut off = game(None);
    assert!(off.state.blooms.is_none());
    let tank = off.spawn(kind(&off, "battle_tank"), 0, 12, 5);
    off.step(10);
    assert!(off.state.entity(tank).is_some());
    assert!(bursts(&off).is_empty());
    // The same map with the bloom point written as plain open ground plays exactly the same.
    let plain = MAP.replace('*', ".");
    let mut same = Game::new(GameOptions { map: &plain, seed: 3, players: Some(2), rules: None }).unwrap();
    same.spawn(kind(&same, "battle_tank"), 0, 12, 5);
    same.step(10);
    assert_eq!(same.hash(), off.hash());
}

#[test]
fn blooms_replay_from_the_command_log() {
    let rules = blooms_on();
    let run = |orders: bool| {
        let mut g = game(Some(&rules));
        let tank = g.spawn(kind(&g, "battle_tank"), 0, 9, 5);
        if orders {
            g.order(0, &[tank], CommandOrder::Move { x: 12, y: 5 });
        }
        g.step(400);
        g
    };
    let live = run(true);
    let log = live.command_log().to_vec();
    let mut replay = game(Some(&rules));
    replay.spawn(kind(&replay, "battle_tank"), 0, 9, 5);
    for t in 0..400 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    assert_eq!(bursts(&live).len(), 1);
    assert_eq!(replay.hash(), live.hash());
}
