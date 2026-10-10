//! Concrete slabs and decay (rules-base-building-power.md, "Foundations and decay"): slabs come from the yard only
//! while the decay module is on and are laid as tile state, extending the area a player may build in. A building
//! wears down every `every_ticks` towards a floor set by how much of its footprint was slabbed when placed: half its
//! health bare, all of it fully slabbed. A destroyed building takes its slab with it, a sold one leaves it, a
//! captured one hands it over.
//!
//! The map is all rock. Player 0's yard is at (1, 1), its power plant at (3, 1) and its 3x2 refinery at (1, 3);
//! player 1 starts at (14, 1) with the mirror image.

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, PlaceError, ProduceError, Rules};

const MAP: &str = "\
####################
#1############2#####
####################
####################
####################
####################
####################
####################
####################
####################
";

fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn decay_on() -> Rules {
    tuned(r#"{ "modules": { "decay": { "on": 1 }, "production": { "instant_build": 1 } } }"#)
}

fn game(rules: &Rules) -> Game {
    let mut g = Game::new(GameOptions { map: MAP, seed: 1, players: Some(2), rules: Some(rules) }).unwrap();
    g.state.players[0].credits = 10_000;
    g.state.players[1].credits = 10_000;
    // The large slab needs an upgraded yard.
    for e in g.state.entities.iter_mut() {
        e.level = 1;
    }
    g
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// Build `id` at player `p`'s yard and place it at (x, y); returns the event that answered the placement.
fn build(g: &mut Game, p: u32, id: &str, x: i32, y: i32) -> Event {
    let k = kind(g, id);
    g.order(p, &[], CommandOrder::Produce { kind: k });
    g.step(2);
    g.order(p, &[], CommandOrder::Place { kind: k, x, y });
    g.step(1);
    let answer = g
        .events
        .iter()
        .rev()
        .find(|e| matches!(e, Event::SlabLaid { .. } | Event::BuildingPlaced { .. } | Event::PlacementRejected { .. }))
        .cloned()
        .expect("the placement was answered");
    // A refused one goes back, so the yard is free for the next.
    if matches!(answer, Event::PlacementRejected { .. }) {
        g.order(p, &[], CommandOrder::Cancel { kind: k });
        g.step(1);
    }
    answer
}

fn slab_owner(g: &Game, x: i32, y: i32) -> Option<u32> {
    classic_sim::decay::owner(&g.state, x, y)
}

fn health(g: &Game, id: u32) -> i64 {
    g.state.entity(id).unwrap().health
}

fn placed(e: &Event) -> u32 {
    match *e {
        Event::BuildingPlaced { entity, .. } => entity,
        ref other => panic!("not placed: {other:?}"),
    }
}

#[test]
fn slabs_are_offered_only_while_decay_is_on() {
    let off = Game::new(GameOptions { map: MAP, seed: 1, players: Some(2), rules: None }).unwrap();
    assert_eq!(off.can_build(0, kind(&off, "slab")), Err(ProduceError::NotBuildable));
    let on = game(&decay_on());
    assert_eq!(on.can_build(0, kind(&on, "slab")), Ok(()));
    assert_eq!(on.can_build(0, kind(&on, "slab_large")), Ok(()));
}

#[test]
fn slabs_lie_on_rock_extend_the_base_and_never_over_another_players() {
    let rules = decay_on();
    let mut g = game(&rules);
    let before = g.state.entities.len();
    // A 2x2 slab beside the power plant: four new tiles, no entity.
    let laid = build(&mut g, 0, "slab_large", 5, 1);
    println!("{laid:?}");
    assert!(matches!(laid, Event::SlabLaid { owner: 0, x: 5, y: 1, tiles: 4, .. }));
    assert_eq!(g.state.entities.len(), before, "a slab is not an entity");
    assert_eq!((slab_owner(&g, 5, 1), slab_owner(&g, 6, 2), slab_owner(&g, 7, 1)), (Some(0), Some(0), None));
    assert!(g.pathfinder.passable(5, 1), "units drive over concrete");
    // Partly over its own slab: only the new tiles are laid. Wholly over it: refused.
    assert!(matches!(build(&mut g, 0, "slab_large", 6, 1), Event::SlabLaid { tiles: 2, .. }));
    assert!(matches!(
        build(&mut g, 0, "slab", 6, 2),
        Event::PlacementRejected { reason: PlaceError::Blocked { .. }, .. }
    ));
    // Slabs reach out: a slab chain carries the base along, so a building can go where it was too far before.
    assert!(matches!(
        build(&mut g, 0, "power_plant", 10, 1),
        Event::PlacementRejected { reason: PlaceError::TooFar, .. }
    ));
    assert!(matches!(build(&mut g, 0, "slab_large", 8, 1), Event::SlabLaid { .. }));
    let plant = placed(&build(&mut g, 0, "power_plant", 10, 1));
    assert_eq!(g.state.entity(plant).unwrap().foundation, 0, "laid on bare rock");
    // Player 1 can't lay over player 0's slab, and nobody can lay one under a building.
    assert!(matches!(
        build(&mut g, 1, "slab_large", 8, 2),
        Event::PlacementRejected { reason: PlaceError::Blocked { x: 8, y: 2 }, .. }
    ));
    assert!(matches!(
        build(&mut g, 0, "slab", 3, 1),
        Event::PlacementRejected { reason: PlaceError::Blocked { .. }, .. }
    ));
}

#[test]
fn a_building_decays_to_a_floor_set_by_its_slab_and_never_further() {
    let rules = decay_on();
    let mut g = game(&rules);
    // Three power plants (500 health): bare, half on slab and fully on slab.
    let bare = placed(&build(&mut g, 0, "power_plant", 5, 1));
    build(&mut g, 0, "slab_large", 7, 1);
    let full = placed(&build(&mut g, 0, "power_plant", 7, 1));
    build(&mut g, 0, "slab", 9, 1);
    build(&mut g, 0, "slab", 9, 2);
    let half = placed(&build(&mut g, 0, "power_plant", 9, 1));
    let f = |id| g.state.entity(id).unwrap().foundation;
    assert_eq!((f(bare), f(half), f(full)), (0, 2, 4));
    // Ten decay steps of 2% each, 150 ticks apart, then many more.
    g.step(150 * 10);
    println!("after 10 steps: bare {} half {} full {}", health(&g, bare), health(&g, half), health(&g, full));
    assert_eq!(health(&g, bare), 500 - 10 * 10);
    assert_eq!(health(&g, full), 500, "a fully slabbed building never decays");
    g.step(150 * 40);
    println!("after 50 steps: bare {} half {} full {}", health(&g, bare), health(&g, half), health(&g, full));
    assert_eq!(health(&g, bare), 250, "bare: down to half");
    assert_eq!(health(&g, half), 375, "half slabbed: down to three quarters");
    assert_eq!(health(&g, full), 500);
    // Decay is its own event, never a hit, and the old yard and refinery wore down too.
    assert!(g.events.iter().any(|e| matches!(*e, Event::Decayed { entity, damage: 10, .. } if entity == bare)));
    assert!(!g.events.iter().any(|e| matches!(e, Event::Hit { .. })));
    let yard = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "construction_yard").unwrap();
    assert_eq!(yard.health, g.rules.kind(yard.kind).max_health / 2);
}

#[test]
fn a_destroyed_building_takes_its_slab_a_sold_one_leaves_it_and_a_captured_one_hands_it_over() {
    let rules = decay_on();
    let mut g = game(&rules);
    build(&mut g, 0, "slab_large", 5, 1);
    let a = placed(&build(&mut g, 0, "power_plant", 5, 1));
    build(&mut g, 0, "slab_large", 7, 1);
    let b = placed(&build(&mut g, 0, "power_plant", 7, 1));
    build(&mut g, 0, "slab_large", 9, 1);
    let c = placed(&build(&mut g, 0, "power_plant", 9, 1));
    // Destroyed: the slab goes too.
    let i = g.state.entities.iter().position(|e| e.id == a).unwrap();
    g.state.entities[i].health = 0;
    g.step(1);
    assert!(g.state.entity(a).is_none());
    assert_eq!(slab_owner(&g, 5, 1), None);
    // Sold: the slab stays.
    g.order(0, &[b], CommandOrder::Sell);
    g.step(30);
    assert!(g.state.entity(b).is_none());
    assert_eq!(slab_owner(&g, 7, 1), Some(0));
    // Captured: the slab changes hands with the building.
    let i = g.state.entities.iter().position(|e| e.id == c).unwrap();
    g.state.entities[i].health = 10;
    let infantry = g.spawn(kind(&g, "infantry"), 1, 11, 1);
    g.order(1, &[infantry], CommandOrder::Capture { target: c });
    g.step(60);
    assert_eq!(g.state.entity(c).map(|e| e.owner), Some(1), "captured");
    assert_eq!((slab_owner(&g, 9, 1), slab_owner(&g, 10, 2)), (Some(1), Some(1)));
}

#[test]
fn slabs_and_decay_replay_from_the_command_log() {
    let rules = decay_on();
    let mut live = game(&rules);
    build(&mut live, 0, "slab_large", 5, 1);
    build(&mut live, 0, "power_plant", 5, 1);
    build(&mut live, 0, "power_plant", 7, 1);
    live.step(1000);
    let log = live.command_log().to_vec();
    let mut replay = game(&rules);
    let end = live.state.tick;
    for t in 0..end {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    assert!(live.state.slabs.is_some());
    assert_eq!(replay.hash(), live.hash());
}
