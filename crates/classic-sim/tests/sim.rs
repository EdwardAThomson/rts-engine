//! Checks from guide 01 Part 2, applied to the simulation. Each test prints what it ran, so a pass can't come
//! from a check that silently did nothing (`cargo test -- --nocapture` shows the lines).

use classic_sim::path::Pathfinder;
use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Terrain, Tile, UnitType, parse_map};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game(seed: i32) -> Game {
    Game::new(GameOptions { map: MAP, seed, players: None, rules: None }).expect("test map is valid")
}

/// A fixed script of player orders, so runs exercise commands as well as the automatic harvesters.
pub fn scripted(game: &mut Game, ticks: u32) {
    let tanks: Vec<(u32, u32)> =
        game.state.entities.iter().filter(|e| e.kind == UnitType::BattleTank).map(|e| (e.id, e.owner)).collect();
    let mut t = 0;
    while t < ticks {
        for (k, &(id, owner)) in tanks.iter().enumerate() {
            let k = k as u32;
            let (x, y) = ((t / 50 + 7 * k) % 30, (t / 100 + 3 * k) % 18);
            game.order(owner, &[id], CommandOrder::Move { x: x as i32, y: y as i32 });
        }
        game.step(500.min(ticks - t));
        t += 500;
    }
}

#[test]
fn determinism_same_seed_and_orders_give_the_same_state_after_10000_ticks() {
    let (mut a, mut b) = (game(7), game(7));
    scripted(&mut a, 10_000);
    scripted(&mut b, 10_000);
    let credits: Vec<i64> = a.state.players.iter().map(|p| p.credits).collect();
    println!("ticks={} entities={} credits={credits:?} hash={}", a.state.tick, a.state.entities.len(), a.hash());
    assert_eq!(a.state.tick, 10_000);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.state, b.state);
}

#[test]
fn seeds_matter_a_different_seed_changes_the_game() {
    let (mut a, mut b) = (game(1), game(2));
    a.step(10_000);
    b.step(10_000);
    let grown = |g: &Game| -> String {
        let tiles: Vec<String> = g
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Regrowth { x, y, .. } => Some(format!("{x},{y}")),
                _ => None,
            })
            .collect();
        tiles.join(" ")
    };
    println!("seed1 regrowth: {} | seed2 regrowth: {}", grown(&a), grown(&b));
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn replay_rerunning_seed_map_and_command_log_reproduces_the_game() {
    let mut live = game(3);
    scripted(&mut live, 6_000);
    let log = live.command_log().to_vec();
    let mut replay = game(3);
    for t in 0..6_000 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    println!("commands={} live={} replay={}", log.len(), live.hash(), replay.hash());
    assert!(!log.is_empty());
    assert_eq!(replay.hash(), live.hash());
}

#[test]
fn harvester_round_trip_first_delivery_arrives_within_a_minute_of_game_time() {
    let mut g = game(1);
    g.step(900); // 60 s at 15 ticks per second
    let first: Vec<String> = g
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Delivered { player, tick, .. } => Some(format!("p{player}@{tick}")),
            _ => None,
        })
        .collect();
    println!("deliveries in 900 ticks: {}", first.join(" "));
    for p in &g.state.players {
        assert!(p.credits >= 200, "player {} has {} credits", p.id, p.credits);
    }
}

#[test]
fn invariants_hold_every_tick() {
    // Resource only moves between the map, harvester cargo and delivered credits; regrowth is the only
    // source, adding at most 40 per regrowth event. Credits never go negative and ids stay sorted.
    let mut g = game(11);
    let start: i64 = g.state.resource.iter().sum();
    let mut checked = 0;
    for _ in 0..5_000 {
        g.step(1);
        let s = &g.state;
        let regrowths = g.events.iter().filter(|e| matches!(e, Event::Regrowth { .. })).count() as i64;
        let on_map: i64 = s.resource.iter().sum();
        let carried: i64 = s.entities.iter().map(|e| e.cargo.unwrap_or(0)).sum();
        let delivered: i64 = s.players.iter().map(|p| p.delivered).sum();
        let created = on_map + carried + delivered - start;
        assert!((0..=regrowths * 40).contains(&created), "tick {}: resource balance off by {created}", s.tick);
        assert!(s.players.iter().all(|p| p.credits >= 0));
        assert!(s.entities.windows(2).all(|w| w[0].id < w[1].id));
        checked += 1;
    }
    println!("checked {checked} ticks, resource on map now {}", g.snapshot().resource_left);
    assert_eq!(checked, 5_000);
}

#[test]
fn pathfinding_never_enters_a_cliff_and_goes_round_the_ridge() {
    let map = parse_map(MAP).unwrap();
    let mut pf = Pathfinder::new(&map);
    let p = pf.find(10, 6, 22, 6).expect("a path");
    let on_cliff = p.tiles.iter().filter(|t| map.terrain[map.index(t.x, t.y)] == Terrain::Cliff).count();
    println!("path length {} tiles, {} nodes expanded, cliff tiles on path: {on_cliff}", p.tiles.len(), p.expanded);
    assert_eq!(on_cliff, 0);
    let crossing = p.tiles.iter().find(|t| t.x == 16).unwrap();
    println!("crosses column 16 at row {} (ridge covers rows 4 to 9)", crossing.y);
    assert!(crossing.y < 4 || crossing.y > 9, "path should go round the ridge, not through it");
    assert_eq!(pf.find(10, 6, 16, 6), None, "a cliff tile is not a valid goal");
}

#[test]
fn pathfinding_refuses_a_goal_in_a_sealed_off_region_without_searching() {
    // The right-hand pocket is walled in by cliffs, so no path can reach it.
    let map = parse_map("..........XXXX\n..........X..X\n..........X..X\n..........XXXX").unwrap();
    let mut pf = Pathfinder::new(&map);
    assert_eq!(pf.find(0, 0, 12, 1), None);
    assert_eq!(pf.stats.expanded, 0);
    assert_eq!(pf.find(0, 0, 9, 3).expect("reachable").tiles.len(), 9);
}

#[test]
fn move_order_a_tank_reaches_the_tile_it_was_sent_to() {
    let mut g = game(1);
    let tank = g.state.entities.iter().find(|e| e.kind == UnitType::BattleTank && e.owner == 0).unwrap().id;
    g.order(0, &[tank], CommandOrder::Move { x: 20, y: 9 });
    g.step(600);
    let s = g.snapshot().entities.into_iter().find(|e| e.id == tank).unwrap();
    println!("tank {tank} at tile {},{} order={:?}", s.tile.x, s.tile.y, s.order);
    assert_eq!(s.tile, Tile { x: 20, y: 9 });
    assert_eq!(s.order, classic_sim::Order::Idle);
}

#[test]
fn orders_for_other_players_units_and_buildings_are_ignored() {
    let mut g = game(1);
    let before = g.hash();
    // Unit 6 is player 1's tank and unit 1 is player 0's refinery; neither may be moved by player 0.
    g.order(0, &[6, 1, 999], CommandOrder::Move { x: 5, y: 5 });
    let mut h = game(1);
    g.step(1);
    h.step(1);
    assert_ne!(before, g.hash());
    assert_eq!(g.hash(), h.hash());
}

#[test]
fn maps_report_unknown_tiles_and_missing_starts() {
    assert!(parse_map("..Q..").unwrap_err().contains("unknown tile 'Q'"));
    let err = Game::new(GameOptions { map: "1....", seed: 1, players: Some(2), rules: None }).err().unwrap();
    assert!(err.contains("no start position 2"), "{err}");
}
