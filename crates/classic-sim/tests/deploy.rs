//! The base builder and the raider bike (rts-engine issue #70): a unit that deploys into a construction yard where
//! it stands (the `deploy` module), and a faction's faster, lighter bike.
//!
//! The map is rock with each player's starting base in a top corner, open ground in the bottom left, a resource
//! field and a cliff on row 14, well away from rows 8 to 12 where most tests deploy. Fog is off, as in the engine's own
//! rules.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Order, PlaceError, ProduceError, Tile};

const MAP: &str = "\
################################
#1##########################2###
################################
################################
################################
################################
################################
################################
################################
################################
################################
################################
################################
################################
#####~~~~####XX#################
......##########################
......##########################
";

fn game() -> Game {
    Game::new(GameOptions { map: MAP, seed: 3, players: None, rules: None }).expect("map is valid")
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn spawn(g: &mut Game, id: &str, owner: u32, x: i32, y: i32) -> u32 {
    let k = kind(g, id);
    g.spawn(k, owner, x, y)
}

/// Step until `done` holds, at most `limit` ticks; the tick count it took.
fn until(g: &mut Game, limit: u32, done: impl Fn(&Game) -> bool) -> u32 {
    for n in 0..limit {
        if done(g) {
            return n;
        }
        g.step(1);
    }
    panic!("not done in {limit} ticks");
}

/// The building a unit deployed into, if it has.
fn deployed(g: &Game, unit: u32) -> Option<(u32, i32, i32)> {
    g.events.iter().find_map(|ev| match *ev {
        Event::Deployed { unit: u, entity, x, y, .. } if u == unit => Some((entity, x, y)),
        _ => None,
    })
}

fn refused(g: &Game, unit: u32) -> Option<PlaceError> {
    g.events.iter().find_map(|ev| match *ev {
        Event::DeployRefused { unit: u, reason, .. } if u == unit => Some(reason),
        _ => None,
    })
}

#[test]
fn the_base_builder_needs_a_heavy_factory_and_a_repair_pad() {
    let mut g = game();
    let mcv = kind(&g, "mcv");
    assert!(matches!(g.can_build(0, mcv), Err(ProduceError::Requires { .. })));
    spawn(&mut g, "heavy_factory", 0, 4, 4);
    assert!(matches!(g.can_build(0, mcv), Err(ProduceError::Requires { .. })), "the repair pad too");
    spawn(&mut g, "repair_pad", 0, 8, 4);
    assert_eq!(g.can_build(0, mcv), Ok(()));
    assert_eq!(g.rules.kind(mcv).deploys_into, g.kind("construction_yard"));
}

#[test]
fn it_deploys_into_a_working_yard_anywhere_on_clear_rock() {
    let mut g = game();
    // Far from every building its owner has: placement's nearness rule doesn't apply.
    let mcv = spawn(&mut g, "mcv", 0, 16, 9);
    assert_eq!(g.can_deploy(mcv, Tile { x: 16, y: 9 }), Ok(()));
    let yard = kind(&g, "construction_yard");
    let yards = |g: &Game| g.state.entities.iter().filter(|e| e.owner == 0 && e.kind == yard).count();
    assert_eq!(yards(&g), 1);
    g.order(0, &[mcv], CommandOrder::Deploy);
    until(&mut g, 5, |g| deployed(g, mcv).is_some());
    let (id, x, y) = deployed(&g, mcv).unwrap();
    assert_eq!((x, y), (16, 9), "a 2 by 2 yard has the unit's tile as its top left");
    assert!(g.state.entity(mcv).is_none(), "the unit is gone");
    let b = g.state.entity(id).expect("the yard stands");
    assert_eq!((b.kind, b.owner, b.tile()), (yard, 0, Tile { x: 16, y: 9 }));
    assert_eq!(b.health, g.rules.kind(yard).max_health);
    assert_eq!(yards(&g), 2);
    // Its footprint blocks movement, and buildings can now go beside it.
    assert!(!g.pathfinder.passable(17, 10));
    let plant = kind(&g, "power_plant");
    assert_eq!(g.can_place(0, plant, 18, 9), Ok(()));
    // It builds like any yard: the next building from it goes beside it.
    g.order(0, &[id], CommandOrder::Produce { kind: plant });
    until(&mut g, 2000, |g| {
        g.events.iter().any(|ev| matches!(*ev, Event::BuildingReady { factory, .. } if factory == id))
    });
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 18, y: 9 });
    g.step(1);
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::BuildingPlaced { x: 18, y: 9, .. })));
}

#[test]
fn it_keeps_its_share_of_health() {
    let mut g = game();
    let mcv = spawn(&mut g, "mcv", 0, 16, 9);
    let k = kind(&g, "mcv");
    let i = g.state.entities.iter().position(|e| e.id == mcv).unwrap();
    g.state.entities[i].health = g.rules.kind(k).max_health / 3;
    g.order(0, &[mcv], CommandOrder::Deploy);
    until(&mut g, 5, |g| deployed(g, mcv).is_some());
    let (id, ..) = deployed(&g, mcv).unwrap();
    let full = g.rules.kind(kind(&g, "construction_yard")).max_health;
    assert_eq!(g.state.entity(id).unwrap().health, full / 3);
}

#[test]
fn ground_that_can_never_take_the_yard_refuses_at_once() {
    // Open ground, resource, a cliff, the map's edge, and another building: each refused on the first tick, with the
    // placement reason, and the unit stays a unit.
    for (x, y, want) in [
        (1, 15, PlaceError::BadGround { x: 1, y: 15 }),
        // Resource lies on open ground, which the yard can't take either.
        (5, 13, PlaceError::BadGround { x: 5, y: 14 }),
        (12, 13, PlaceError::BadGround { x: 13, y: 14 }),
        (31, 9, PlaceError::OutOfBounds),
    ] {
        let mut g = game();
        let mcv = spawn(&mut g, "mcv", 0, x, y);
        assert_eq!(g.can_deploy(mcv, Tile { x, y }), Err(want));
        g.order(0, &[mcv], CommandOrder::Deploy);
        g.step(2);
        assert_eq!(refused(&g, mcv), Some(want), "at {x},{y}");
        let e = g.state.entity(mcv).expect("still a unit");
        assert_eq!((e.order, e.deploy_by), (Order::Idle, None));
    }
    let mut g = game();
    spawn(&mut g, "power_plant", 1, 17, 10);
    let mcv = spawn(&mut g, "mcv", 0, 16, 9);
    g.order(0, &[mcv], CommandOrder::Deploy);
    g.step(2);
    assert_eq!(refused(&g, mcv), Some(PlaceError::Blocked { x: 17, y: 10 }), "a building never moves");
}

#[test]
fn its_owners_units_step_out_of_the_way() {
    let mut g = game();
    let mcv = spawn(&mut g, "mcv", 0, 16, 9);
    let tank = spawn(&mut g, "battle_tank", 0, 17, 10);
    let foot = spawn(&mut g, "infantry", 0, 16, 10);
    g.order(0, &[mcv], CommandOrder::Deploy);
    let took = until(&mut g, 45, |g| deployed(g, mcv).is_some());
    println!("own units cleared the footprint in {took} ticks");
    let (id, ..) = deployed(&g, mcv).unwrap();
    let b = g.state.entity(id).unwrap();
    for u in [tank, foot] {
        let t = g.state.entity(u).unwrap().tile();
        let inside = (b.tile().x..b.tile().x + 2).contains(&t.x) && (b.tile().y..b.tile().y + 2).contains(&t.y);
        assert!(!inside, "unit {u} at {t:?} is off the yard");
    }
}

#[test]
fn an_enemy_in_the_way_runs_the_wait_out() {
    let mut g = game();
    let mcv = spawn(&mut g, "mcv", 0, 16, 9);
    // An unarmed enemy that stays put.
    spawn(&mut g, "harvester", 1, 17, 9);
    let hv = g.state.entities.last().unwrap().id;
    let i = g.state.entities.iter().position(|e| e.id == hv).unwrap();
    g.state.entities[i].order = Order::Idle;
    g.state.entities[i].task = None;
    g.order(0, &[mcv], CommandOrder::Deploy);
    g.step(1);
    let by = g.state.entity(mcv).unwrap().deploy_by.expect("waiting");
    let took = until(&mut g, 60, |g| refused(g, mcv).is_some());
    assert_eq!(refused(&g, mcv), Some(PlaceError::Blocked { x: 17, y: 9 }));
    assert_eq!(g.state.tick, by + 1, "it waits `deploy.wait_ticks` from the order ({took} ticks)");
    assert!(g.state.entity(mcv).is_some());
}

#[test]
fn a_moving_unit_finishes_its_step_and_another_order_calls_it_off() {
    let mut g = game();
    let mcv = spawn(&mut g, "mcv", 0, 12, 9);
    g.order(0, &[mcv], CommandOrder::Move { x: 20, y: 9 });
    g.step(8);
    let e = g.state.entity(mcv).unwrap();
    assert!(e.x % 256 != 128, "between two tiles");
    g.order(0, &[mcv], CommandOrder::Deploy);
    until(&mut g, 60, |g| deployed(g, mcv).is_some());
    let (_, x, _) = deployed(&g, mcv).unwrap();
    assert_eq!(x, 13, "on the tile it was stepping into");

    // Told to move again before it is down, it stays a unit.
    let mut g = game();
    let mcv = spawn(&mut g, "mcv", 0, 12, 9);
    let tank = spawn(&mut g, "battle_tank", 1, 13, 10);
    g.state.entities.iter_mut().find(|e| e.id == tank).unwrap().reload = 10_000;
    g.order(0, &[mcv], CommandOrder::Deploy);
    g.step(3);
    assert_eq!(g.state.entity(mcv).unwrap().order, Order::Deploy);
    g.order(0, &[mcv], CommandOrder::Move { x: 20, y: 9 });
    g.step(2);
    let e = g.state.entity(mcv).unwrap();
    assert_eq!((e.order, e.deploy_by), (Order::Move, None));
    assert!(deployed(&g, mcv).is_none() && refused(&g, mcv).is_none());
}

#[test]
fn only_a_unit_that_deploys_takes_the_order() {
    let mut g = game();
    let tank = spawn(&mut g, "battle_tank", 0, 16, 9);
    g.order(0, &[tank], CommandOrder::Deploy);
    g.step(2);
    assert_eq!(g.state.entity(tank).unwrap().order, Order::Idle);
    assert_eq!(g.can_deploy(tank, Tile { x: 16, y: 9 }), Err(PlaceError::NotABuilding));
    // Nor another player's.
    let mcv = spawn(&mut g, "mcv", 1, 16, 12);
    g.order(0, &[mcv], CommandOrder::Deploy);
    g.step(2);
    assert_eq!(g.state.entity(mcv).unwrap().order, Order::Idle);
}

#[test]
fn deploying_replays_to_the_same_hash() {
    let run = || {
        let mut g = game();
        let mcv = spawn(&mut g, "mcv", 0, 16, 9);
        spawn(&mut g, "infantry", 0, 17, 10);
        g.order(0, &[mcv], CommandOrder::Deploy);
        g.step(120);
        g.hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn the_raider_bike_is_one_factions_faster_lighter_bike() {
    let mut g = game();
    let (raider, scout) = (kind(&g, "raider_bike"), kind(&g, "scout_bike"));
    assert_eq!(g.can_build(0, raider), Err(ProduceError::Faction));
    g.set_factions(&["faction_c", "faction_a"]);
    assert_eq!(g.can_build(0, raider), Ok(()));
    assert_eq!(g.can_build(1, raider), Err(ProduceError::Faction));
    assert_eq!(g.can_build(1, scout), Ok(()), "the others keep the scout bike");
    let (r, s) = (g.rules.kind(raider), g.rules.kind(scout));
    assert!(r.speed > s.speed && r.max_health < s.max_health && r.cost < s.cost);
    assert_eq!(r.weapon, s.weapon);
    // Over the same twelve tiles it gets there first.
    let a = spawn(&mut g, "raider_bike", 0, 4, 9);
    let b = spawn(&mut g, "scout_bike", 0, 4, 11);
    g.order(0, &[a], CommandOrder::Move { x: 16, y: 9 });
    g.order(0, &[b], CommandOrder::Move { x: 16, y: 11 });
    let there = |g: &Game, id: u32, x: i32| g.state.entity(id).unwrap().tile().x == x;
    until(&mut g, 200, |g| there(g, a, 16));
    assert!(!there(&g, b, 16));
}
