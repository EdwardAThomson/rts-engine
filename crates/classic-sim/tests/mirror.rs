//! A map that is its own mirror image left to right gives both sides the same game: the starting bases, paths and
//! harvests come out as mirror images, so no start is favoured by the order tiles happen to be searched in. Each
//! test prints what it ran (`cargo test -- --nocapture`).

use classic_sim::map::TILE;
use classic_sim::path::Pathfinder;
use classic_sim::{Game, GameOptions, Rules, parse_map};

const MAP: &str = include_str!("../../../maps/mirror-01.txt");

/// The rules with chance taken out (no random waits, regrowth or scatter), so any difference is the rules' own.
fn no_chance() -> Rules {
    let mut rules = Rules::default();
    rules.movement.wait_random = 1;
    rules.regrowth.every_ticks = u32::MAX;
    for w in &mut rules.weapons {
        w.scatter = 0;
    }
    rules
}

/// An entity as both sides are compared: kind, footprint centre, health and cargo.
type Seen = (String, i64, i64, i64, i64);

/// Each player's entities, player 0's mirrored, sorted.
fn sides(g: &Game) -> [Vec<Seen>; 2] {
    let w = g.map.width as i64 * TILE;
    [0, 1].map(|p| {
        let mut v: Vec<_> = g
            .state
            .entities
            .iter()
            .filter(|e| e.owner == p)
            .map(|e| {
                let k = g.rules.kind(e.kind);
                let x = e.x - TILE / 2 + k.width as i64 * TILE / 2;
                (k.id.clone(), if p == 0 { w - x } else { x }, e.y, e.health, e.cargo.unwrap_or(0))
            })
            .collect();
        v.sort();
        v
    })
}

#[test]
fn a_start_in_the_right_half_is_laid_out_as_the_mirror_image_of_one_in_the_left() {
    let g = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: None }).unwrap();
    let [left, right] = sides(&g);
    println!("starting bases: {right:?}");
    assert_eq!(left.len(), 5);
    assert_eq!(left, right);
}

#[test]
fn paths_come_out_as_mirror_images() {
    let map = parse_map(MAP).unwrap();
    let w = map.width;
    let mut pf = Pathfinder::new(&map);
    let pairs =
        [((20, 14), (26, 2)), ((10, 14), (28, 30)), ((18, 7), (29, 0)), ((3, 20), (24, 26)), ((6, 15), (30, 15))];
    for ((ax, ay), (bx, by)) in pairs {
        let a = pf.find(ax, ay, bx, by).expect("a path");
        let b = pf.find(w - 1 - ax, ay, w - 1 - bx, by).expect("its mirror");
        let mirrored: Vec<(i32, i32)> = b.tiles.iter().map(|t| (w - 1 - t.x, t.y)).collect();
        let tiles: Vec<(i32, i32)> = a.tiles.iter().map(|t| (t.x, t.y)).collect();
        println!("({ax},{ay}) to ({bx},{by}): {} tiles, {} and {} nodes", tiles.len(), a.expanded, b.expanded);
        assert_eq!(tiles, mirrored);
    }
    // Round a unit standing in the way: going left and going right cost the same, and the side nearer the middle
    // wins on both halves.
    let (x, y) = (10, 20);
    let round = |pf: &mut Pathfinder, x: i32| {
        let p = pf.find_avoiding((x, y), (x, y - 2), &[((y - 1) * w + x) as usize], 1000).expect("a way round");
        p.tiles.iter().map(|t| (t.x, t.y)).collect::<Vec<_>>()
    };
    let (a, b) = (round(&mut pf, x), round(&mut pf, w - 1 - x));
    println!("round a unit: {a:?} and {b:?}");
    assert_eq!(a, b.iter().map(|&(bx, by)| (w - 1 - bx, by)).collect::<Vec<_>>());
    assert_eq!(a[0].0, x + 1, "towards the middle");
}

#[test]
fn both_sides_harvest_as_mirror_images() {
    let rules = no_chance();
    let mut g = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    for _ in 0..60 {
        g.step(150);
        let [left, right] = sides(&g);
        assert_eq!(left, right, "tick {}", g.state.tick);
        assert_eq!(g.state.players[0].delivered, g.state.players[1].delivered);
    }
    println!("9000 ticks, mirror images throughout, each side delivered {}", g.state.players[0].delivered);
    assert!(g.state.players[0].delivered > 0);
}
