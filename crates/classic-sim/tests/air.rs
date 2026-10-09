//! Aircraft (rules-movement.md section 9), anti-air (rules-combat.md) and the carrier's lifts
//! (rules-economy-production.md section 13).
//!
//! The first map is all rock with a cliff wall down column 12, open only at the bottom row; each player's starting
//! base is in a top corner, out of sight of where the tests fly. A gunship flies at 48 a tick and climbs or comes
//! down 32 a tick to and from its cruising height of 256, so it takes 8 ticks to take off. Its rockets (range 3
//! tiles) hit only the ground; a battle tank's cannon hits only the ground; rocket infantry's rockets hit both.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Order, Stage, Task, Tile};

const WALL: &str = "\
########################
#1##########X#######2###
############X###########
############X###########
############X###########
############X###########
############X###########
############X###########
############X###########
############X###########
############X###########
########################
";

const TEST_01: &str = include_str!("../../../maps/test-01.txt");

fn game(map: &str) -> Game {
    Game::new(GameOptions { map, seed: 3, players: None, rules: None }).expect("map is valid")
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn spawn(g: &mut Game, id: &str, owner: u32, x: i32, y: i32) -> u32 {
    let k = kind(g, id);
    g.spawn(k, owner, x, y)
}

fn e(g: &Game, id: u32) -> &classic_sim::Entity {
    g.state.entity(id).unwrap_or_else(|| panic!("entity {id} is gone"))
}

const CRUISE: i64 = 256;

fn hits_on(g: &Game, target: u32) -> usize {
    g.events.iter().filter(|ev| matches!(**ev, Event::Hit { target: t, .. } if t == target)).count()
}

fn fired_at(g: &Game, target: u32) -> usize {
    g.events.iter().filter(|ev| matches!(**ev, Event::Fired { target: t, .. } if t == target)).count()
}

#[test]
fn an_aircraft_takes_off_flies_straight_over_a_cliff_and_lands() {
    let mut g = game(WALL);
    let a = spawn(&mut g, "gunship", 0, 8, 9);
    assert_eq!(e(&g, a).altitude, 0, "built aircraft start landed");
    g.order(0, &[a], CommandOrder::Move { x: 14, y: 9 });
    // Taking off: it climbs straight up before it moves.
    g.step(8);
    assert_eq!(e(&g, a).altitude, CRUISE);
    assert_eq!((e(&g, a).x, e(&g, a).y), (8 * 256 + 128, 9 * 256 + 128), "no ground move while climbing");
    // Straight across the cliff: the row never changes, though a ground unit would go round by the bottom row.
    let mut ticks = 0;
    while e(&g, a).order == Order::Move {
        g.step(1);
        ticks += 1;
        assert_eq!(e(&g, a).y, 9 * 256 + 128, "flies a straight line");
        assert!(ticks < 200, "arrives");
    }
    assert_eq!(e(&g, a).tile(), Tile { x: 14, y: 9 });
    // 6 tiles at 48 a tick is 32 ticks flat out; the ramp over the last 2 tiles slows it to a quarter.
    assert!((32..=64).contains(&ticks), "{ticks} ticks to fly 6 tiles");
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::MoveEnded { unit, .. } if unit == a)));
    g.step(1);
    assert_eq!(e(&g, a).altitude, CRUISE - 32, "comes down once it has nothing to do");
    g.step(7);
    assert_eq!(e(&g, a).altitude, 0, "landed");
}

#[test]
fn aircraft_ignore_each_other_and_ground_units_and_may_share_a_tile() {
    let mut g = game(WALL);
    let tank = spawn(&mut g, "battle_tank", 0, 16, 8);
    let (a, b) = (spawn(&mut g, "carrier", 0, 15, 8), spawn(&mut g, "carrier", 0, 17, 8));
    g.order(0, &[a, b], CommandOrder::Move { x: 16, y: 8 });
    g.step(60);
    assert_eq!(e(&g, a).tile(), Tile { x: 16, y: 8 });
    assert_eq!(e(&g, b).tile(), Tile { x: 16, y: 8 });
    assert_eq!((e(&g, a).x, e(&g, a).y), (e(&g, b).x, e(&g, b).y), "both over the tank's tile");
    assert_eq!(e(&g, tank).tile(), Tile { x: 16, y: 8 }, "the tank was never asked to step aside");
}

#[test]
fn an_aircraft_over_a_building_or_cliff_hovers_instead_of_landing() {
    let mut g = game(WALL);
    let a = spawn(&mut g, "carrier", 0, 10, 4);
    let yard = g.state.entities.iter().find(|x| x.owner == 0).unwrap().id;
    let over = e(&g, yard).tile();
    g.order(0, &[a], CommandOrder::Move { x: over.x, y: over.y });
    let b = spawn(&mut g, "carrier", 0, 10, 6);
    g.order(0, &[b], CommandOrder::Move { x: 12, y: 6 });
    g.step(200);
    assert_eq!(e(&g, a).tile(), over);
    assert_eq!(e(&g, a).altitude, CRUISE, "over the yard");
    assert_eq!(e(&g, b).tile(), Tile { x: 12, y: 6 });
    assert_eq!(e(&g, b).altitude, CRUISE, "over the cliff");
}

#[test]
fn only_weapons_that_hit_air_aim_at_aircraft_in_flight() {
    let mut g = game(WALL);
    // Over the cliff it can't land, so it hovers there.
    let gunship = spawn(&mut g, "gunship", 1, 12, 6);
    let tank = spawn(&mut g, "battle_tank", 0, 10, 6);
    let rockets = spawn(&mut g, "rocket_infantry", 0, 14, 6);
    g.step(8);
    assert_eq!(e(&g, gunship).altitude, CRUISE);
    // An attack order on it is dropped by the tank at once: its cannon can't aim up.
    g.order(0, &[tank], CommandOrder::Attack { target: gunship });
    g.step(60);
    let fired = |unit: u32| {
        g.events.iter().any(|ev| matches!(*ev, Event::Fired { unit: u, target, .. } if u == unit && target == gunship))
    };
    assert!(fired(rockets), "the rocket infantry fires at it");
    assert!(!fired(tank), "the tank never does");
    assert_eq!(e(&g, tank).target, None);
    assert_eq!(e(&g, tank).order, Order::Idle);
    assert!(hits_on(&g, gunship) > 0, "homing rockets reach it");
}

#[test]
fn a_landed_aircraft_is_a_ground_target() {
    let mut g = game(WALL);
    let carrier = spawn(&mut g, "carrier", 1, 16, 6);
    let tank = spawn(&mut g, "battle_tank", 0, 14, 6);
    g.order(0, &[tank], CommandOrder::Attack { target: carrier });
    g.step(60);
    assert_eq!(e(&g, carrier).altitude, 0);
    assert!(
        g.events.iter().any(|ev| matches!(*ev, Event::Fired { unit, target, .. } if unit == tank && target == carrier))
    );
    assert!(hits_on(&g, carrier) > 0, "shells hurt a landed aircraft (75% against air armour)");
}

#[test]
fn a_gunship_attacks_ground_units_from_the_air_only() {
    let mut g = game(WALL);
    let gunship = spawn(&mut g, "gunship", 0, 4, 9);
    let tank = spawn(&mut g, "battle_tank", 1, 16, 9);
    g.order(0, &[gunship], CommandOrder::Attack { target: tank });
    let mut first_fire = None;
    for _ in 0..200 {
        g.step(1);
        if first_fire.is_none() && g.events.iter().any(|ev| matches!(*ev, Event::Fired { unit, .. } if unit == gunship))
        {
            first_fire = Some(g.state.tick);
            assert_eq!(e(&g, gunship).altitude, CRUISE, "fires only from the air");
            let d = (e(&g, gunship).x - e(&g, tank).x).abs();
            assert!(d <= 3 * 256, "within its range of 3 tiles, {d}");
        }
    }
    assert!(first_fire.is_some(), "it crossed the cliff and fired");
    assert!(hits_on(&g, tank) > 0);
    // The tank can't shoot back.
    assert!(!g.events.iter().any(|ev| matches!(*ev, Event::Fired { unit, .. } if unit == tank)));
}

#[test]
fn a_burst_on_the_ground_never_splashes_an_aircraft_above_it() {
    let mut g = game(WALL);
    let tank = spawn(&mut g, "battle_tank", 0, 14, 6);
    let plant = spawn(&mut g, "power_plant", 1, 17, 6);
    // A carrier hovering over the plant's top-left tile, where every shell aimed at the plant lands.
    let carrier = spawn(&mut g, "carrier", 1, 16, 6);
    g.order(1, &[carrier], CommandOrder::Move { x: 17, y: 6 });
    g.step(30);
    assert_eq!((e(&g, carrier).x, e(&g, carrier).y), (e(&g, plant).x, e(&g, plant).y));
    assert_eq!(e(&g, carrier).altitude, CRUISE, "over a building it hovers");
    g.order(0, &[tank], CommandOrder::Attack { target: plant });
    g.step(150);
    assert!(hits_on(&g, plant) > 0, "the shells land on the plant");
    assert_eq!(hits_on(&g, carrier), 0, "and not on the carrier hovering over it");
}

#[test]
fn nothing_aims_at_an_untargetable_aircraft() {
    let mut g = game(WALL);
    let ship = spawn(&mut g, "supply_ship", 1, 16, 6);
    spawn(&mut g, "rocket_infantry", 0, 15, 6);
    spawn(&mut g, "battle_tank", 0, 17, 6);
    g.step(100);
    assert_eq!(fired_at(&g, ship), 0);
    assert_eq!(e(&g, ship).health, 500);
}

#[test]
fn an_air_factory_builds_aircraft_that_wait_landed_beside_it() {
    let mut g = game(TEST_01);
    let factory = spawn(&mut g, "air_factory", 0, 3, 5);
    let carrier = kind(&g, "carrier");
    let gunship = kind(&g, "gunship");
    assert!(g.can_build(0, carrier).is_ok());
    assert!(g.can_build(0, gunship).is_err(), "a gunship needs a research lab");
    g.state.players[0].credits = 5000;
    g.order(0, &[factory], CommandOrder::Produce { kind: carrier });
    let mut built = None;
    for _ in 0..1500 {
        g.step(1);
        built = g.events.iter().find_map(|ev| match *ev {
            Event::UnitBuilt { entity, kind, .. } if kind == carrier => Some(entity),
            _ => None,
        });
        if built.is_some() {
            break;
        }
    }
    let c = built.expect("the carrier is built");
    let t = e(&g, c).tile();
    assert!((2..=6).contains(&t.x) && (4..=7).contains(&t.y), "beside the factory, at {t:?}");
    assert_eq!(e(&g, c).altitude, 0);
    assert_eq!(e(&g, c).order, Order::Idle, "an aircraft doesn't drive clear of the exit");
}

/// Player 0's starting harvester, full and far off at the top right of test-01, sent home.
fn far_full_harvester(g: &mut Game) -> u32 {
    let h = g.state.entities.iter_mut().find(|x| x.owner == 0 && x.cargo.is_some()).unwrap();
    h.cargo = Some(700);
    h.task = Some(Task::Mining);
    h.path.clear();
    (h.x, h.y) = (30 * 256 + 128, 128);
    h.id
}

fn delivered(g: &Game, h: u32) -> Option<u32> {
    g.events.iter().find_map(|ev| match *ev {
        Event::Delivered { unit, tick, .. } if unit == h => Some(tick),
        _ => None,
    })
}

fn run_until_delivered(g: &mut Game, h: u32) -> u32 {
    for _ in 0..3000 {
        g.step(1);
        if let Some(t) = delivered(g, h) {
            return t;
        }
    }
    panic!("the harvester delivered");
}

#[test]
fn an_idle_carrier_lifts_a_harvester_on_a_long_trip_home() {
    let mut alone = game(TEST_01);
    let h = far_full_harvester(&mut alone);
    let by_road = run_until_delivered(&mut alone, h);

    let mut g = game(TEST_01);
    let h = far_full_harvester(&mut g);
    let carrier = spawn(&mut g, "carrier", 0, 9, 2);
    let by_air = run_until_delivered(&mut g, h);
    let pickup = g
        .events
        .iter()
        .position(|ev| matches!(*ev, Event::CarrierPickup { carrier: c, unit, .. } if c == carrier && unit == h));
    let drop = g
        .events
        .iter()
        .position(|ev| matches!(*ev, Event::CarrierDropoff { carrier: c, unit, .. } if c == carrier && unit == h));
    assert!(pickup.is_some() && drop.is_some() && pickup < drop, "picked up, then set down");
    assert!(by_air < by_road, "by air {by_air} ticks, by road {by_road}");
    assert_eq!(e(&g, carrier).ferry, None, "the lift is over");
    assert_eq!(e(&g, h).carried_by, None);
}

#[test]
fn a_harvester_aboard_is_off_the_ground_and_can_t_be_shot() {
    let mut g = game(TEST_01);
    let h = far_full_harvester(&mut g);
    let carrier = spawn(&mut g, "carrier", 0, 28, 1);
    let mut aboard = false;
    for _ in 0..400 {
        g.step(1);
        if e(&g, h).carried_by == Some(carrier) {
            aboard = true;
            break;
        }
    }
    assert!(aboard, "the carrier took hold of it");
    // An enemy tank right under it doesn't fire, and it takes no orders.
    let at = e(&g, h).tile();
    let tank = spawn(&mut g, "battle_tank", 1, at.x, at.y + 1);
    g.order(0, &[h], CommandOrder::Move { x: 0, y: 0 });
    g.step(5);
    assert!(!g.events.iter().any(|ev| matches!(*ev, Event::Fired { unit, target, .. } if unit == tank && target == h)));
    assert_eq!(e(&g, h).carried_by, Some(carrier));
    assert!(e(&g, h).path.is_empty(), "the order was ignored");
    // A move order to the carrier waits until the lift is over.
    g.order(0, &[carrier], CommandOrder::Move { x: 0, y: 19 });
    g.step(1);
    assert!(matches!(e(&g, carrier).ferry, Some(f) if f.stage != Stage::Fetch));
}

#[test]
fn a_carrier_shot_down_drops_its_harvester_hurt() {
    let mut g = game(TEST_01);
    let h = far_full_harvester(&mut g);
    let carrier = spawn(&mut g, "carrier", 0, 28, 1);
    for _ in 0..400 {
        g.step(1);
        if matches!(e(&g, carrier).ferry, Some(f) if f.stage == Stage::Carry) {
            break;
        }
    }
    let full = e(&g, h).health;
    g.state.entities.iter_mut().find(|x| x.id == carrier).unwrap().health = 0;
    g.step(1);
    assert!(g.state.entity(carrier).is_none());
    let lost = g.events.iter().find_map(|ev| match *ev {
        Event::CarrierLostCargo { unit, x, y, .. } if unit == h => Some(Tile { x, y }),
        _ => None,
    });
    assert_eq!(lost, Some(e(&g, h).tile()), "it fell to the ground");
    assert_eq!(e(&g, h).carried_by, None);
    assert_eq!(e(&g, h).health, full - full / 2, "and lost half its full health");
    // It drives the rest of the way home on its own.
    run_until_delivered(&mut g, h);
}

#[test]
fn games_with_aircraft_replay_to_the_same_hash() {
    let run = || {
        let mut g = game(TEST_01);
        far_full_harvester(&mut g);
        let c = spawn(&mut g, "carrier", 0, 9, 2);
        let s = spawn(&mut g, "gunship", 1, 20, 10);
        let r = spawn(&mut g, "rocket_infantry", 0, 12, 10);
        g.order(1, &[s], CommandOrder::Attack { target: r });
        g.step(300);
        g.order(0, &[c], CommandOrder::Move { x: 2, y: 18 });
        g.step(300);
        g.hash()
    };
    assert_eq!(run(), run());
}
