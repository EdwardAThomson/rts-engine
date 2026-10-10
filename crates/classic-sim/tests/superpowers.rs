//! The palace powers (rules-world.md, section 8): a charge that runs only with a powered palace, then the missile,
//! the guerrillas and the saboteur, each used at a tile.
//!
//! The map is open rock with each player's starting base in a top corner. Player 0 plays `faction_b` (the missile)
//! unless a test says otherwise, and player 1 `faction_a` (the guerrillas). Fog is off, as in the engine's own rules.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, Superpower, SuperpowerError, Tile};

const OPEN: &str = "\
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
################################
################################
################################
################################
################################
################################
";

fn game(factions: &[&str]) -> Game {
    let mut g = Game::new(GameOptions { map: OPEN, seed: 3, players: None, rules: None }).expect("map is valid");
    g.set_factions(factions);
    g
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn spawn(g: &mut Game, id: &str, owner: u32, x: i32, y: i32) -> u32 {
    let k = kind(g, id);
    g.spawn(k, owner, x, y)
}

fn charge(g: &Game, p: u32) -> Option<u32> {
    g.state.players[p as usize].charge
}

fn full(g: &Game, power: Superpower) -> u32 {
    g.rules.superpowers.as_ref().expect("module on").charge_ticks(power)
}

/// A game with a palace for player 0 and its power charged.
fn charged(factions: &[&str]) -> (Game, u32) {
    let mut g = game(factions);
    let palace = spawn(&mut g, "palace", 0, 2, 4);
    // Plenty of power, so the palace never runs short.
    spawn(&mut g, "power_plant", 0, 6, 4);
    g.step(1);
    let power = classic_sim::superpower::power(&g.state, &g.rules, 0).expect("a power");
    g.state.players[0].charge = Some(full(&g, power));
    (g, palace)
}

fn refused(g: &Game, reason: SuperpowerError) -> bool {
    g.events.iter().any(|ev| matches!(*ev, Event::SuperpowerRefused { reason: r, .. } if r == reason))
}

#[test]
fn the_power_charges_only_with_a_powered_palace() {
    let mut g = game(&["faction_b"]);
    g.step(10);
    assert_eq!(charge(&g, 0), None, "no palace, no charge, and nothing in the hash");
    let palace = spawn(&mut g, "palace", 0, 2, 4);
    spawn(&mut g, "power_plant", 0, 6, 4);
    spawn(&mut g, "palace", 1, 20, 12);
    spawn(&mut g, "power_plant", 1, 24, 12);
    g.step(100);
    assert_eq!(charge(&g, 0), Some(100));
    assert_eq!(charge(&g, 1), None, "a player with no faction has no power");
    // Short of power: it waits.
    for x in [10, 14, 18, 22] {
        spawn(&mut g, "starport", 0, x, 8);
    }
    assert!(g.power(0).is_short());
    g.step(50);
    assert_eq!(charge(&g, 0), Some(100), "paused while short");
    let starport = kind(&g, "starport");
    g.state.entities.retain(|e| e.kind != starport);
    let need = full(&g, Superpower::Missile);
    g.step(need - 100);
    let ready: Vec<u32> = g
        .events
        .iter()
        .filter_map(|ev| match *ev {
            Event::SuperpowerReady { tick, player: 0, power: Superpower::Missile } => Some(tick),
            _ => None,
        })
        .collect();
    assert_eq!(ready.len(), 1, "said once");
    assert!(classic_sim::superpower::ready(&g.state, &g.rules, 0));
    g.step(10);
    assert_eq!(charge(&g, 0), Some(need), "a full charge waits");
    // Without its palace the power can't be used, and the charge is kept for the next one.
    g.state.entities.retain(|e| e.id != palace);
    g.order(0, &[], CommandOrder::Superpower { x: 20, y: 10 });
    g.step(1);
    assert!(refused(&g, SuperpowerError::NoPalace));
    assert_eq!(charge(&g, 0), Some(need));
}

#[test]
fn a_power_still_charging_is_refused() {
    let mut g = game(&["faction_b", "faction_a"]);
    spawn(&mut g, "palace", 0, 2, 4);
    spawn(&mut g, "power_plant", 0, 6, 4);
    g.step(5);
    g.order(0, &[], CommandOrder::Superpower { x: 20, y: 10 });
    g.step(1);
    assert!(refused(&g, SuperpowerError::NotReady));
    assert!(g.state.strikes.is_empty());
    let mut g = game(&[]);
    spawn(&mut g, "palace", 0, 2, 4);
    g.order(0, &[], CommandOrder::Superpower { x: 20, y: 10 });
    g.step(1);
    assert!(refused(&g, SuperpowerError::NoPower));
}

#[test]
fn the_missile_lands_late_and_off_target_and_hurts_by_ring() {
    let (mut g, palace) = charged(&["faction_b", "faction_a"]);
    g.order(0, &[], CommandOrder::Superpower { x: 20, y: 12 });
    g.step(1);
    assert_eq!(charge(&g, 0), Some(1), "used up, and charging again");
    let launched = g.events.iter().find_map(|ev| match *ev {
        Event::MissileLaunched { palace: p, x, y, to_x, to_y, arrive, tick, .. } if p == palace => {
            Some((x, y, to_x, to_y, arrive, tick))
        }
        _ => None,
    });
    let (x, y, to_x, to_y, arrive, tick) = launched.expect("launched");
    assert_eq!((x, y), (20, 12));
    // From the palace's middle tile (3, 5): about 18 tiles, so 30 + 2 × 18 ticks and up to 2 + 1 tiles off.
    assert_eq!(arrive, tick + 30 + 2 * 18);
    assert!(((to_x - x) as i64).pow(2) + ((to_y - y) as i64).pow(2) <= 9, "within its spread");
    // Tanks of both sides at rings 0 to 4 from where it will land, and a 3×3 building whose nearest tile is in ring 1.
    let mut tanks = Vec::new();
    for ring in 0..5 {
        tanks.push((ring, spawn(&mut g, "battle_tank", 1, to_x + ring, to_y)));
    }
    tanks.push((2, spawn(&mut g, "battle_tank", 0, to_x - 2, to_y - 1)));
    let plant = spawn(&mut g, "palace", 1, to_x - 3, to_y + 1);
    let health = |g: &Game, id: u32| g.state.entity(id).map_or(0, |e| e.health);
    let before: Vec<i64> = tanks.iter().map(|&(_, id)| health(&g, id)).collect();
    let plant_before = health(&g, plant);
    while g.state.tick <= arrive {
        assert!(!g.events.iter().any(|ev| matches!(ev, Event::MissileImpact { .. })) || g.state.tick > arrive);
        g.step(1);
    }
    let landed = g.events.iter().find_map(|ev| match *ev {
        Event::MissileImpact { tick, x, y, .. } => Some((tick, x, y)),
        _ => None,
    });
    assert_eq!(landed, Some((arrive, to_x, to_y)), "on time, where it said");
    let damage = [800, 600, 350, 150, 0];
    for (&(ring, id), was) in tanks.iter().zip(before) {
        let lost = was - health(&g, id);
        assert_eq!(lost, damage[ring as usize].min(was), "ring {ring}, either side");
    }
    assert_eq!(plant_before - health(&g, plant), 600, "a building by its nearest tile");
    assert!(g.state.entity(tanks[0].1).is_none(), "destroyed outright");
    assert!(g.state.strikes.is_empty());
}

#[test]
fn guerrillas_arrive_round_the_tile_and_fight_on_their_own() {
    let (mut g, _) = charged(&["faction_a", "faction_b"]);
    let enemy = spawn(&mut g, "battle_tank", 1, 20, 12);
    g.order(0, &[], CommandOrder::Superpower { x: 20, y: 12 });
    g.step(1);
    let guerrilla = kind(&g, "guerrilla");
    g.step(44);
    assert!(g.state.entities.iter().all(|e| e.kind != guerrilla), "not before 45 ticks");
    g.step(1);
    let arrived = g.events.iter().find_map(|ev| match *ev {
        Event::GuerrillasArrived { units, x, y, .. } => Some((units, x, y)),
        _ => None,
    });
    assert_eq!(arrived, Some((5, 20, 12)));
    let band: Vec<u32> = g.state.entities.iter().filter(|e| e.kind == guerrilla).map(|e| e.id).collect();
    assert_eq!(band.len(), 5);
    for &id in &band {
        let e = g.state.entity(id).unwrap();
        let t = e.tile();
        let ring = (t.x - 20).abs().max((t.y - 12).abs());
        assert!((3..=6).contains(&ring), "between 3 and 6 tiles out, not {ring}");
        assert_eq!(e.owner, 0);
        assert_eq!(e.autonomous, Some(Tile { x: 20, y: 12 }));
    }
    // Their owner can't order them about.
    g.order(0, &band, CommandOrder::Move { x: 1, y: 18 });
    g.step(1);
    assert!(band.iter().all(|&id| g.state.entity(id).is_none_or(|e| e.path.back() != Some(&Tile { x: 1, y: 18 }))));
    // They find the tank by their tile and destroy it.
    let mut n = 0;
    while g.state.entity(enemy).is_some() {
        g.step(1);
        n += 1;
        assert!(n < 2000, "the tank should fall");
    }
    let killers: Vec<u32> = g
        .events
        .iter()
        .filter_map(|ev| match *ev {
            Event::Destroyed { entity, killer, .. } if entity == enemy => killer,
            _ => None,
        })
        .collect();
    assert!(band.contains(&killers[0]), "a guerrilla's kill");
}

#[test]
fn the_saboteur_comes_out_of_the_palace_and_goes_for_the_building() {
    let (mut g, palace) = charged(&["faction_c", "faction_a"]);
    let refinery = spawn(&mut g, "refinery", 1, 20, 12);
    g.order(0, &[], CommandOrder::Superpower { x: 21, y: 13 });
    g.step(1);
    let unit = g.events.iter().find_map(|ev| match *ev {
        Event::SaboteurArrived { palace: p, unit, .. } if p == palace => Some(unit),
        _ => None,
    });
    let unit = unit.expect("a saboteur came out");
    let s = g.state.entity(unit).unwrap();
    assert_eq!(s.kind, kind(&g, "saboteur"));
    assert_eq!(s.target, Some(refinery), "sent at the building");
    assert!(s.autonomous.is_none(), "under its owner's orders");
    let mut n = 0;
    while g.state.entity(refinery).is_some() {
        g.step(1);
        n += 1;
        assert!(n < 2000, "the refinery should go");
    }
    assert!(g.state.entity(unit).is_none(), "spent");
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::SapperDetonated { unit: u, .. } if u == unit)));
}

#[test]
fn games_with_the_powers_replay_to_the_same_hash() {
    let run = |factions: [&str; 2]| {
        let (mut g, _) = charged(&factions);
        spawn(&mut g, "power_plant", 1, 20, 12);
        g.order(0, &[], CommandOrder::Superpower { x: 21, y: 13 });
        g.step(600);
        g.hash()
    };
    for f in ["faction_a", "faction_b", "faction_c"] {
        assert_eq!(run([f, "faction_a"]), run([f, "faction_a"]), "{f}");
    }
}
