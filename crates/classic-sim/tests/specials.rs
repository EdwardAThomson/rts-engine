//! The faction specials (rules-combat.md, "Special weapons"): faction-only production, the beam, converting gas,
//! the self-destruct and the cloaked sapper with a lifetime.
//!
//! The map is open rock with each player's starting base in a top corner, well away from rows 8 to 12 where the
//! tests fight. Fog is off, as in the engine's own rules, so only a cloak hides anything.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, ProduceError, Tile};

const OPEN: &str = "\
############################
#1######################2###
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
";

fn game() -> Game {
    Game::new(GameOptions { map: OPEN, seed: 5, players: None, rules: None }).expect("map is valid")
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

fn weapon(g: &Game, id: &str) -> classic_sim::WeaponId {
    g.rules.weapon_id(id).unwrap_or_else(|| panic!("no weapon {id}"))
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

#[test]
fn faction_specials_are_built_only_by_their_faction() {
    let mut g = game();
    let (a, h, o) = (kind(&g, "super_a"), kind(&g, "super_h"), kind(&g, "super_o"));
    // No faction set: none of them.
    for k in [a, h, o] {
        assert_eq!(g.can_build(0, k), Err(ProduceError::Faction));
    }
    g.set_factions(&["faction_a", "faction_b"]);
    // Its own still needs a heavy factory and a research lab; the others' never.
    assert!(matches!(g.can_build(0, a), Err(ProduceError::Requires { .. })));
    assert_eq!(g.can_build(0, h), Err(ProduceError::Faction));
    assert_eq!(g.can_build(1, a), Err(ProduceError::Faction));
    spawn(&mut g, "heavy_factory", 0, 4, 4);
    spawn(&mut g, "research_lab", 0, 8, 4);
    assert_eq!(g.can_build(0, a), Ok(()));
    assert_eq!(g.can_build(0, o), Err(ProduceError::Faction));
    // Ordered anyway, the order is refused for the faction.
    g.order(0, &[], CommandOrder::Produce { kind: o });
    g.step(1);
    let refused = g.events.iter().any(|ev| {
        matches!(*ev, Event::ProductionRejected { player: 0, kind, reason: ProduceError::Faction, .. } if kind == o)
    });
    assert!(refused);
    // Kinds only a power brings are built by nobody.
    assert_eq!(g.can_build(0, kind(&g, "saboteur")), Err(ProduceError::NotBuildable));
    assert_eq!(g.can_build(0, kind(&g, "guerrilla")), Err(ProduceError::NotBuildable));
}

#[test]
fn a_beam_hits_everything_on_its_line_at_once_but_not_its_own_kind() {
    let mut g = game();
    let sonic = spawn(&mut g, "super_a", 0, 4, 10);
    let own = spawn(&mut g, "infantry", 0, 5, 10);
    let twin = spawn(&mut g, "super_a", 0, 6, 10);
    let enemy = spawn(&mut g, "infantry", 1, 7, 10);
    let tank = spawn(&mut g, "battle_tank", 1, 8, 10);
    let aside = spawn(&mut g, "infantry", 1, 7, 12);
    let beyond = spawn(&mut g, "battle_tank", 1, 11, 10);
    // The twin holds its fire, so the first beam is the one under test.
    let i = g.state.entities.iter().position(|x| x.id == twin).unwrap();
    g.state.entities[i].reload = 1000;
    until(&mut g, 60, |g| g.events.iter().any(|ev| matches!(*ev, Event::BeamFired { unit, .. } if unit == sonic)));
    let line = g.events.iter().find_map(|ev| match *ev {
        Event::BeamFired { unit, x1, y1, x2, y2, .. } if unit == sonic => Some((x1, y1, x2, y2)),
        _ => None,
    });
    let (x1, y1, x2, y2) = line.unwrap();
    assert_eq!((x1, y1), (4 * 256 + 128, 10 * 256 + 128), "from the firer's centre");
    assert_eq!((x2 - x1, y2 - y1), (1280, 0), "five tiles along the row, whichever unit on it was the target");
    let w = weapon(&g, "sonic_wave");
    let hit = |id: u32| {
        g.events.iter().find_map(|ev| match *ev {
            Event::Hit { target, attacker, weapon, damage, .. } if target == id && attacker == sonic && weapon == w => {
                Some(damage)
            }
            _ => None,
        })
    };
    // 70 base: sonic does 150% to infantry and 100% to heavy armour, and its own side takes half.
    assert_eq!(hit(enemy), Some(105));
    assert_eq!(hit(tank), Some(70));
    assert_eq!(hit(own), Some(52), "friendly fire at half");
    assert_eq!(hit(twin), None, "the same weapon passes through its own kind");
    assert_eq!(hit(aside), None, "two tiles off the line");
    assert_eq!(hit(beyond), None, "past the end of the line");
    assert!(g.state.projectiles.is_empty(), "a beam is no projectile");
}

#[test]
fn gas_takes_enemy_vehicles_over_for_a_while_and_they_go_back() {
    let mut g = game();
    let gas = spawn(&mut g, "super_o", 0, 4, 10);
    let tank = spawn(&mut g, "battle_tank", 1, 8, 10);
    let foot = spawn(&mut g, "infantry", 1, 8, 11);
    let special = spawn(&mut g, "super_h", 1, 9, 10);
    // The special holds its fire, so the tank lives to go back.
    let i = g.state.entities.iter().position(|x| x.id == special).unwrap();
    g.state.entities[i].reload = 5000;
    until(&mut g, 120, |g| e(g, tank).owner == 0);
    let (tick, until_tick) = (g.state.tick, e(&g, tank).converted.expect("converted").1);
    assert_eq!(e(&g, tank).converted, Some((1, until_tick)));
    assert_eq!(until_tick, tick - 1 + 375, "for the weapon's 375 ticks");
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::Converted { unit, from: 1, to: 0, .. } if unit == tank)));
    assert!(g.events.iter().all(|ev| !matches!(*ev, Event::Hit { target, .. } if target == tank)), "gas hurts nothing");
    assert_eq!(e(&g, foot).owner, 1, "infantry is never taken");
    assert_eq!(e(&g, special).owner, 1, "nor a kind with the unconvertible role");
    // Its new side stops shooting at it, and it turns on its old one.
    assert_ne!(e(&g, gas).target, Some(tank));
    let back = until_tick;
    while g.state.tick < back {
        g.step(1);
    }
    g.step(1);
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::Reverted { unit, from: 0, to: 1, .. } if unit == tank)));
    assert_eq!(e(&g, tank).owner, 1, "back on its own side");
    assert_eq!(e(&g, tank).converted, None);
}

#[test]
fn a_self_destruct_counts_down_then_blows_up_with_its_large_blast() {
    let mut g = game();
    let big = spawn(&mut g, "super_h", 0, 8, 10);
    let own = spawn(&mut g, "battle_tank", 0, 10, 10);
    let enemy = spawn(&mut g, "infantry", 1, 9, 10);
    let far = spawn(&mut g, "battle_tank", 1, 13, 10);
    g.order(0, &[big], CommandOrder::SelfDestruct);
    g.step(1);
    let at = g.events.iter().find_map(|ev| match *ev {
        Event::SelfDestructStarted { unit, at, .. } if unit == big => Some(at),
        _ => None,
    });
    assert_eq!(at, Some(30), "a 30 tick fuse from the order on tick 0");
    // It can't be moved or fire while it counts down.
    g.order(0, &[big], CommandOrder::Move { x: 3, y: 10 });
    g.step(10);
    assert_eq!(e(&g, big).tile(), Tile { x: 8, y: 10 });
    assert!(g.events.iter().all(|ev| !matches!(*ev, Event::Fired { unit, .. } if unit == big)));
    while g.state.entity(big).is_some() {
        assert!(g.state.tick <= 31, "goes off on its tick");
        g.step(1);
    }
    let w = weapon(&g, "blast_large");
    let blast = |id: u32| {
        g.events.iter().find_map(|ev| match *ev {
            Event::Hit { target, weapon, damage, .. } if target == id && weapon == w => Some(damage),
            _ => None,
        })
    };
    assert_eq!(blast(enemy), Some(300), "full within half its 640 radius");
    assert_eq!(blast(own), Some(75), "half for the outer ring, halved again for its own side");
    assert_eq!(blast(far), None, "outside the blast");
    // What a blast kills goes on the next tick.
    g.step(1);
    assert!(g.state.entity(enemy).is_none());
}

#[test]
fn killed_while_counting_down_it_still_leaves_the_large_blast() {
    let mut g = game();
    let big = spawn(&mut g, "super_h", 0, 8, 10);
    let enemy = spawn(&mut g, "battle_tank", 1, 10, 10);
    g.order(0, &[big], CommandOrder::SelfDestruct);
    g.step(1);
    let i = g.state.entities.iter().position(|x| x.id == big).unwrap();
    g.state.entities[i].health = 1;
    until(&mut g, 40, |g| g.state.entity(big).is_none());
    assert!(g.state.tick < 30, "the tank's shot killed it before its fuse");
    let w = weapon(&g, "blast_large");
    assert!(
        g.events.iter().any(|ev| matches!(*ev, Event::Hit { target, weapon, .. } if target == enemy && weapon == w))
    );
    // An ordinary death leaves the ordinary blast.
    let mut g = game();
    let big = spawn(&mut g, "super_h", 0, 8, 10);
    spawn(&mut g, "battle_tank", 1, 10, 10);
    let i = g.state.entities.iter().position(|x| x.id == big).unwrap();
    g.state.entities[i].health = 1;
    until(&mut g, 40, |g| g.state.entity(big).is_none());
    g.step(1);
    assert!(g.events.iter().all(|ev| !matches!(*ev, Event::Hit { weapon, .. } if weapon == w)));
}

#[test]
fn a_sapper_is_cloaked_ignores_units_and_spends_itself_on_a_building() {
    let mut g = game();
    let sab = spawn(&mut g, "saboteur", 0, 10, 10);
    let tank = spawn(&mut g, "battle_tank", 1, 13, 10);
    // Three tiles from the enemy's nearest unit: hidden from them, and so never shot at.
    assert!(!g.visible(1, sab));
    assert!(g.visible(0, sab), "its own side always sees it");
    g.step(60);
    assert!(g.events.iter().all(|ev| !matches!(*ev, Event::Fired { unit, .. } if unit == tank)));
    assert!(
        g.events.iter().all(|ev| !matches!(*ev, Event::Fired { unit, .. } if unit == sab)),
        "units aren't its game"
    );
    // Two tiles away it shows.
    let near = spawn(&mut g, "infantry", 1, 12, 10);
    assert!(g.visible(1, sab));
    g.state.entities.retain(|x| x.id != near && x.id != tank);
    // Sent at a building, it walks up and blows it apart, and itself with it.
    let plant = kind(&g, "power_plant");
    let target = g.state.entities.iter().find(|x| x.owner == 1 && x.kind == plant).unwrap().id;
    g.order(0, &[sab], CommandOrder::Attack { target });
    until(&mut g, 1500, |g| g.state.entity(sab).is_none());
    assert!(
        g.events
            .iter()
            .any(|ev| matches!(*ev, Event::SapperDetonated { unit, target: t, .. } if unit == sab && t == target))
    );
    g.step(1);
    assert!(g.state.entity(target).is_none(), "1,500 is more than a power plant's 500");
}

#[test]
fn a_unit_with_a_lifetime_disappears_when_it_runs_out() {
    let mut g = game();
    let sab = spawn(&mut g, "saboteur", 0, 4, 12);
    assert_eq!(e(&g, sab).expires, Some(2700));
    g.step(2700);
    assert!(g.state.entity(sab).is_some());
    g.step(1);
    assert!(g.state.entity(sab).is_none());
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::Expired { unit, .. } if unit == sab)));
    assert!(g.events.iter().all(|ev| !matches!(*ev, Event::Destroyed { entity, .. } if entity == sab)));
}

#[test]
fn a_game_with_the_specials_replays_to_the_same_hash() {
    let run = || {
        let mut g = game();
        g.set_factions(&["faction_a", "faction_c"]);
        spawn(&mut g, "super_a", 0, 4, 10);
        spawn(&mut g, "super_o", 1, 10, 10);
        spawn(&mut g, "battle_tank", 0, 5, 12);
        let big = spawn(&mut g, "super_h", 0, 6, 8);
        spawn(&mut g, "battle_tank", 1, 9, 12);
        spawn(&mut g, "saboteur", 0, 3, 13);
        g.order(0, &[big], CommandOrder::SelfDestruct);
        g.step(600);
        g.hash()
    };
    assert_eq!(run(), run());
}
