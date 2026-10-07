//! Golden values recorded from the TypeScript engine this Rust port replaced (rts-engine main at 7d64886,
//! before the port). Matching them shows the port plays exactly the same games: same paths, same node counts,
//! same state hashes, tick for tick.
//!
//! Three rule changes since then re-recorded values on purpose, each checked to change nothing else:
//! - the tank's generic id became `battle_tank` (the hash spells each entity's id): state hashes only;
//! - buildings block ground movement, so units and harvesters path round each refinery: state hashes and the
//!   nodes expanded during ticks. Standalone paths, path checksums, credits and the tank's position are unchanged;
//! - the tick runs in phases (commands, movement, economy, world) and the refinery is 3x2 with its dock under the
//!   middle column (rules-movement.md, rules-base-building-power.md): state hashes, the nodes expanded during ticks,
//!   and one bench scene's credits (seed 1 ends on 2,000, not 2,200, as harvesters drive to the new dock). Standalone
//!   paths, path checksums and the scripted tank's position are unchanged.

use classic_sim::path::Pathfinder;
use classic_sim::{CommandOrder, Game, GameOptions, parse_map};
use classic_tools::scene::{Rnd, Scene, fnv_pair, populate, send_due};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game(seed: i32) -> Game {
    Game::new(GameOptions { map: MAP, seed, players: None, rules: None }).unwrap()
}

#[test]
fn idle_games_match_the_recorded_hashes() {
    let golden: [(i32, [&str; 8]); 4] = [
        (1, ["6ce9136e", "bee3fbd2", "da99395d", "2a69950d", "9fda22a4", "5bea6f41", "f49130dd", "6abb57a9"]),
        (2, ["10355211", "0d528a9d", "95a2bb1e", "60cc84d2", "d1c11e87", "966e7c5e", "a32e78ea", "e2f925c0"]),
        (3, ["2b8bfa68", "71887760", "a5523e33", "ba65d287", "8cdeb046", "f51a9f97", "67b0e57c", "cad3e1aa"]),
        (4, ["a7b315a8", "45f8b1f6", "1fc4e0a7", "696bd307", "316251cc", "4fcb54c1", "e0bc55cc", "d72b1066"]),
    ];
    let ticks = [0u32, 1, 449, 450, 451, 900, 3000, 10000];
    for (seed, hashes) in golden {
        let mut g = game(seed);
        for (t, want) in ticks.iter().zip(hashes) {
            g.step(t - g.state.tick);
            assert_eq!(g.hash(), want, "seed {seed} tick {t}");
        }
    }
}

fn scripted(game: &mut Game, ticks: u32) {
    let tanks: Vec<(u32, u32)> = game
        .state
        .entities
        .iter()
        .filter(|e| game.rules.kind(e.kind).id == "battle_tank")
        .map(|e| (e.id, e.owner))
        .collect();
    let mut t = 0;
    while t < ticks {
        for (k, &(id, owner)) in tanks.iter().enumerate() {
            let k = k as u32;
            game.order(
                owner,
                &[id],
                CommandOrder::Move { x: ((t / 50 + 7 * k) % 30) as i32, y: ((t / 100 + 3 * k) % 18) as i32 },
            );
        }
        game.step(500.min(ticks - t));
        t += 500;
    }
}

#[test]
fn scripted_games_and_the_old_replay_hash_match() {
    let mut a = game(7);
    scripted(&mut a, 10_000);
    assert_eq!(a.hash(), "be542f1e");
    let mut b = game(3);
    scripted(&mut b, 6_000);
    assert_eq!(b.hash(), "e294aed5", "the replay hash every earlier change was checked against");
    assert_eq!(b.command_log().len(), 24);
    let mut m = game(1);
    m.order(0, &[3], CommandOrder::Move { x: 20, y: 9 });
    m.step(600);
    assert_eq!(m.hash(), "f2acad0d");
    let tank = m.state.entity(3).unwrap();
    assert_eq!((tank.x, tank.y), (5248, 2432));
}

#[test]
fn a_path_on_the_test_map_matches_tile_for_tile() {
    let map = parse_map(MAP).unwrap();
    let p = Pathfinder::new(&map).find(10, 6, 22, 6).unwrap();
    let tiles: Vec<(i32, i32)> = p.tiles.iter().map(|t| (t.x, t.y)).collect();
    assert_eq!(p.expanded, 37);
    assert_eq!(
        tiles,
        [(11, 6), (12, 6), (13, 5), (14, 4), (15, 3), (16, 3), (17, 3), (18, 4), (19, 5), (20, 6), (21, 6), (22, 6)]
    );
}

struct SceneGolden {
    seed: u32,
    map_hash: u32,
    passable: usize,
    path_check: u32,
    path_nulls: usize,
    path_nodes: u64,
    tick_nodes: u64,
    hashes: [&'static str; 6],
    credits: [i64; 2],
}

/// The bench scene at 500 units: 200 long paths and 20 into the walled pocket, then 900 ticks of group orders.
fn check_scene(want: SceneGolden) {
    let mut rnd = Rnd::new(want.seed);
    let scene = Scene::new(&mut rnd);
    let map_hash = scene.text.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    assert_eq!(map_hash, want.map_hash, "map text");
    assert_eq!(scene.passable.len(), want.passable);

    let (mut pairs, pocket) = scene.path_pairs(&mut rnd, 200, 20);
    pairs.extend(pocket);
    let mut pf = Pathfinder::new(&scene.map);
    let (mut check, mut nulls, mut nodes) = (0x811c_9dc5u32, 0, 0u64);
    let s = 128usize;
    for (a, b) in pairs {
        match pf.find((a % s) as i32, (a / s) as i32, (b % s) as i32, (b / s) as i32) {
            None => {
                nulls += 1;
                check = fnv_pair(check, 0xffff_ffff);
            }
            Some(p) => {
                nodes += p.expanded as u64;
                check = fnv_pair(check, p.expanded);
                for t in &p.tiles {
                    check = fnv_pair(check, (t.y * 128 + t.x) as u32);
                }
            }
        }
    }
    assert_eq!((check, nulls, nodes), (want.path_check, want.path_nulls, want.path_nodes), "A* results");

    let (mut game, groups) = populate(&scene, &mut rnd, want.seed as i32, 500);
    let mut got = Vec::new();
    for t in 0..900 {
        send_due(&mut game, &scene, &mut rnd, &groups, t);
        game.step(1);
        if (t + 1) % 150 == 0 {
            got.push(game.hash());
        }
    }
    assert_eq!(game.pathfinder.stats.expanded, want.tick_nodes, "nodes expanded over the ticks");
    assert_eq!(got, want.hashes, "state hashes every 150 ticks");
    let credits: Vec<i64> = game.state.players.iter().map(|p| p.credits).collect();
    assert_eq!(credits, want.credits);
}

#[test]
fn bench_scene_seed_1_matches() {
    check_scene(SceneGolden {
        seed: 1,
        map_hash: 0xb4aebf11,
        passable: 15209,
        path_check: 0x3b120bc1,
        path_nulls: 23,
        path_nodes: 167_392,
        tick_nodes: 1_060_606,
        hashes: ["5a7414ab", "e227424c", "f5fde499", "5321f042", "55ffc9a0", "74463f42"],
        credits: [200, 2000],
    });
}

#[test]
fn bench_scene_seed_2_matches() {
    check_scene(SceneGolden {
        seed: 2,
        map_hash: 0xa71d3e45,
        passable: 15098,
        path_check: 0x388dc4f4,
        path_nulls: 22,
        path_nodes: 584_041,
        tick_nodes: 3_962_960,
        hashes: ["72cdfb6a", "60cfdfb6", "9c2e4c49", "f69cd5dc", "12688311", "f9145f53"],
        credits: [200, 1000],
    });
}
