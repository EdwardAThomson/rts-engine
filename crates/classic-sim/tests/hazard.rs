//! The hazard (rules-world.md, section 7): off unless a pack turns it on; spawns away from bases; hunts the noisiest
//! unit on open ground it can reach; surfaces and swallows; leaves when full; never crosses rock.
//!
//! The map: each player's base on a rock shelf at a side, open ground with a resource field between them, and a rock
//! island in the middle with open ground all round it.

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Hazard, LeftReason, Rules};

const MAP: &str = "\
##########......................##########
#1########......................########2#
##########......................##########
##########.........~~~~.........##########
##########.........~~~~.........##########
##########......................##########
##########..........####........##########
##########..........####........##########
##########..........####........##########
##########......................##########
##########......................##########
##########......................##########
";

/// The engine's rules with the hazard on, as a pack's `features` would turn it on, then `tuning`.
fn rules(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    t.modules.get_mut("hazard").unwrap().numbers.get_mut("on").unwrap().value = 1;
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn game(rules: &Rules, seed: i32) -> Game {
    Game::new(GameOptions { map: MAP, seed, players: None, rules: Some(rules) }).expect("map is valid")
}

fn hazards(g: &Game) -> &[Hazard] {
    g.state.hazards.as_ref().map_or(&[], |h| &h.list)
}

fn eaten(g: &Game) -> Vec<u32> {
    g.events.iter().filter_map(|e| if let Event::HazardAte { unit, .. } = *e { Some(unit) } else { None }).collect()
}

fn tank(g: &mut Game, owner: u32, x: i32, y: i32) -> u32 {
    let k = g.kind("battle_tank").unwrap();
    g.spawn(k, owner, x, y)
}

/// Step until `done` holds, at most `max` ticks; the ticks taken.
fn until(g: &mut Game, max: u32, done: impl Fn(&Game) -> bool) -> Option<u32> {
    for n in 0..max {
        if done(g) {
            return Some(n);
        }
        g.step(1);
    }
    None
}

#[test]
fn it_is_off_unless_a_pack_turns_it_on() {
    let rules = Rules::default();
    assert!(rules.hazard.is_none());
    let mut g = game(&rules, 1);
    assert!(g.state.hazards.is_none());
    assert_eq!(g.spawn_hazard(15, 9), None);
    g.step(3000);
    assert!(g.events.iter().all(|e| !e.name().starts_with("hazard")));
    assert!(g.state.entities.iter().all(|e| e.noise == 0), "noise is not even counted");
}

#[test]
fn one_spawns_at_its_first_tick_far_from_every_building() {
    let rules = rules(r#"{"modules": {"hazard": {"first_tick": 100, "spawn_clearance": 8}}}"#);
    let mut g = game(&rules, 1);
    g.step(100);
    assert!(hazards(&g).is_empty(), "not before tick 100");
    g.step(1);
    let [z] = hazards(&g) else { panic!("one hazard: {:?}", hazards(&g)) };
    let t = z.tile();
    for b in g.state.entities.iter().filter(|e| g.rules.kind(e.kind).building) {
        let (k, bt) = (g.rules.kind(b.kind), b.tile());
        let dx = (bt.x - t.x).max(t.x - (bt.x + k.width - 1)).max(0);
        let dy = (bt.y - t.y).max(t.y - (bt.y + k.height - 1)).max(0);
        assert!(dx.max(dy) >= 8, "{t:?} is {} tiles from the {}", dx.max(dy), k.id);
    }
    assert_eq!(g.map.terrain[g.map.index(t.x, t.y)], classic_sim::Terrain::Open);
    assert!(g.events.iter().any(|e| matches!(e, Event::HazardSpawned { tick: 100, .. })));
    g.step(500);
    assert!(hazards(&g).len() <= 1, "no more than `max`");
}

#[test]
fn it_hunts_a_noisy_harvester_and_swallows_it_whole() {
    // The starting harvesters drive out to the field in the middle and mine there: the loudest thing on the map.
    let rules = rules(r#"{"modules": {"hazard": {"first_tick": 100000}}}"#);
    let mut g = game(&rules, 1);
    let z = g.spawn_hazard(16, 10).unwrap();
    let harvesters: Vec<u32> =
        g.state.entities.iter().filter(|e| g.rules.kind(e.kind).harvester.is_some()).map(|e| e.id).collect();
    let n = until(&mut g, 3000, |g| !eaten(g).is_empty()).expect("it struck within 3,000 ticks");
    let victim = eaten(&g)[0];
    assert!(harvesters.contains(&victim), "it went for a harvester, the noisiest unit");
    assert!(g.state.entity(victim).is_none());
    assert!(
        !g.events.iter().any(|e| matches!(e, Event::Destroyed { entity, .. } if *entity == victim)),
        "swallowed whole: no wreck, no blast"
    );
    let surfaced = g.events.iter().position(|e| matches!(e, Event::HazardSurfaced { hazard, .. } if *hazard == z));
    let ate = g.events.iter().position(|e| matches!(e, Event::HazardAte { hazard, .. } if *hazard == z));
    assert!(surfaced.unwrap() < ate.unwrap(), "it surfaces, then eats");
    println!("struck after {n} ticks");
    // It stays up for `surface_ticks`, then goes under again.
    let up = hazards(&g)[0].surfaced;
    assert_eq!(up, rules.hazard.as_ref().unwrap().surface_ticks);
    g.step(up);
    assert_eq!(hazards(&g)[0].surfaced, 0);
}

#[test]
fn it_goes_for_the_louder_unit_not_the_nearer_one() {
    let rules = rules(r#"{"modules": {"hazard": {"first_tick": 100000, "mining_noise": 0}}}"#);
    let mut g = game(&rules, 1);
    // A tank standing still 3 tiles away, and one driving to and fro 6 tiles away.
    let still = tank(&mut g, 0, 14, 10);
    let loud = tank(&mut g, 1, 23, 11);
    g.spawn_hazard(17, 10).unwrap();
    for i in 0..4 {
        let x = if i % 2 == 0 { 28 } else { 22 };
        g.order(1, &[loud], CommandOrder::Move { x, y: 11 });
        g.step(14);
    }
    let z = &hazards(&g)[0];
    assert_eq!(z.target, Some(loud), "the moving tank: {z:?}");
    assert_ne!(z.target, Some(still));
}

#[test]
fn rock_is_safe_ground() {
    // A tank on the rock island makes noise moving about, but the hazard can't reach rock, so never takes it.
    let rules = rules(r#"{"modules": {"hazard": {"first_tick": 100000, "mining_noise": 0}}}"#);
    let mut g = game(&rules, 1);
    let safe = tank(&mut g, 0, 20, 6);
    g.spawn_hazard(19, 9).unwrap();
    for i in 0..30 {
        let x = if i % 2 == 0 { 23 } else { 20 };
        g.order(0, &[safe], CommandOrder::Move { x, y: 6 + i % 3 });
        g.step(20);
    }
    assert!(g.state.entity(safe).is_some());
    assert!(!eaten(&g).contains(&safe));
    for z in hazards(&g) {
        let t = z.tile();
        assert_eq!(g.map.terrain[g.map.index(t.x, t.y)], classic_sim::Terrain::Open, "it never leaves open ground");
    }
}

#[test]
fn it_leaves_when_full_and_another_comes_later() {
    let rules = rules(
        r#"{"modules": {"hazard": {"first_tick": 100000, "appetite": 2, "leave_ticks": 20, "respawn_ticks": 50,
            "spawn_clearance": 4}}}"#,
    );
    let mut g = game(&rules, 2);
    g.spawn_hazard(14, 11).unwrap();
    // A lone tank within a tile of where it lies in wait goes first; the second is whatever it finds next.
    let a = tank(&mut g, 0, 15, 11);
    tank(&mut g, 1, 13, 10);
    until(&mut g, 400, |g| eaten(g).len() >= 2).expect("it ate twice");
    assert_eq!(eaten(&g)[0], a);
    let left = until(&mut g, 200, |g| g.events.iter().any(|e| matches!(e, Event::HazardLeft { .. })));
    assert!(left.is_some(), "full, it left");
    assert!(g.events.iter().any(|e| matches!(e, Event::HazardLeft { reason: LeftReason::Full, .. })));
    assert!(hazards(&g).is_empty());
    // `first_tick` is far off, but a hazard that left is replaced after `respawn_ticks` whatever the first tick.
    until(&mut g, 60, |g| !hazards(g).is_empty()).expect("another came");
}

#[test]
fn the_same_seed_gives_the_same_hunt() {
    let rules = rules(r#"{"modules": {"hazard": {"first_tick": 300, "spawn_clearance": 6}}}"#);
    let run = |seed| {
        let mut g = game(&rules, seed);
        g.step(4000);
        (g.hash(), eaten(&g))
    };
    let (h1, e1) = run(7);
    assert_eq!(run(7), (h1.clone(), e1.clone()));
    println!("seed 7: {h1}, ate {e1:?}");
}
