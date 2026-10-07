//! Golden values recorded from the TypeScript engine this Rust port replaced (rts-engine main at 7d64886,
//! before the port). Matching them shows the port plays exactly the same games: same paths, same node counts,
//! same state hashes, tick for tick.

use classic_sim::path::Pathfinder;
use classic_sim::{CommandOrder, Game, GameOptions, UnitType, parse_map};
use classic_tools::scene::{Rnd, Scene, fnv_pair, populate, send_due};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game(seed: i32) -> Game {
    Game::new(GameOptions { map: MAP, seed, players: None, rules: None }).unwrap()
}

#[test]
fn idle_games_match_the_typescript_hashes() {
    let golden: [(i32, [&str; 8]); 4] = [
        (1, ["69744172", "54f1da16", "3307913c", "1028f87b", "57387168", "24636eba", "ce4ef88a", "86a3f9be"]),
        (2, ["ba4189cd", "7e068dc9", "8838b0ef", "17868468", "03ed99fb", "17053657", "84e51047", "4c52a7f9"]),
        (3, ["2cd37c5c", "54918044", "998cc68e", "56aeb709", "6b24ee02", "b1f2eb06", "45173d3b", "a4128de2"]),
        (4, ["9bf8383c", "663580aa", "ad52af78", "777cba9d", "2122cbf0", "7a6339e4", "e585e647", "dfd5c20c"]),
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
    let tanks: Vec<(u32, u32)> =
        game.state.entities.iter().filter(|e| e.kind == UnitType::BattleTank).map(|e| (e.id, e.owner)).collect();
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
    assert_eq!(a.hash(), "8b30ab4f");
    let mut b = game(3);
    scripted(&mut b, 6_000);
    assert_eq!(b.hash(), "e0342eba", "the replay hash every earlier change was checked against");
    assert_eq!(b.command_log().len(), 24);
    let mut m = game(1);
    m.order(0, &[3], CommandOrder::Move { x: 20, y: 9 });
    m.step(600);
    assert_eq!(m.hash(), "53fa2480");
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
        tick_nodes: 1_051_074,
        hashes: ["716d5a0b", "60b72f35", "6387e3fe", "872dc7dc", "8bcb701e", "a20c8400"],
        credits: [200, 2200],
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
        tick_nodes: 3_967_322,
        hashes: ["3575d0c0", "152b97c1", "1df2b1f8", "5a1251a9", "0d993cb3", "42847e01"],
        credits: [200, 1000],
    });
}
