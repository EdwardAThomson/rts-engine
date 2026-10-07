//! Performance bench for the simulation (engineering.md "Performance budget", performance.md).
//!
//!   cargo run --release --bin bench -- [--seed 1] [--ticks 3000] [--sizes 100,250,500] [--paths 1000]
//!
//! On the seeded 128 x 128 bench scene (see `scene.rs`) it measures:
//!   (a) A*: time and nodes expanded for long random paths, p50 / p95 / p99, and requests into the walled pocket
//!   (b) whole ticks with N units on move orders across the map (all ordered at tick 0, then groups re-ordered
//!       in turn every 300 ticks), p50 / p95 / p99 tick time and nodes expanded per tick
//!   (c) the state hash on the largest state
//! Every scenario prints a checksum or state hash. The scene makes the same random calls as the TypeScript bench
//! it replaces, so the hashes can be compared with that bench's output. Timing reads the clock, which is fine:
//! the bench is not the sim.

use std::time::Instant;

use classic_sim::hash_state;
use classic_sim::path::Pathfinder;
use classic_tools::scene::{Rnd, SIZE, Scene, fnv_pair, populate, send_due};

/// The per-tick node budget in rules-movement.md, reported against.
const NODE_BUDGET: u64 = 10_000;

fn pct(xs: &[f64], p: f64) -> f64 {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    s[((p / 100.0 * s.len() as f64) as usize).min(s.len() - 1)]
}

fn summary(xs: &[f64], digits: i32) -> String {
    let f = |v: f64| {
        let m = 10f64.powi(digits);
        (v * m).round() / m
    };
    let max = xs.iter().copied().fold(f64::MIN, f64::max);
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    format!(
        "{{\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{},\"mean\":{}}}",
        f(pct(xs, 50.0)),
        f(pct(xs, 95.0)),
        f(pct(xs, 99.0)),
        f(max),
        f(mean)
    )
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Peak resident memory in MB, where the platform reports it (Linux).
fn peak_rss_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some((kb / 102.4).round() / 10.0)
}

struct Run {
    ms: Vec<f64>,
    nodes: Vec<f64>,
    nulls: usize,
    check: u32,
}

fn run_paths(pf: &mut Pathfinder, list: &[(usize, usize)]) -> Run {
    let s = SIZE as usize;
    let mut r = Run { ms: Vec::new(), nodes: Vec::new(), nulls: 0, check: 0x811c_9dc5 };
    for &(a, b) in list {
        let t0 = Instant::now();
        let p = pf.find((a % s) as i32, (a / s) as i32, (b % s) as i32, (b / s) as i32);
        r.ms.push(ms(t0));
        match p {
            None => {
                r.nulls += 1;
                r.nodes.push(0.0);
                r.check = fnv_pair(r.check, 0xffff_ffff);
            }
            Some(p) => {
                r.nodes.push(p.expanded as f64);
                r.check = fnv_pair(r.check, p.expanded);
                for t in &p.tiles {
                    r.check = fnv_pair(r.check, (t.y * SIZE + t.x) as u32);
                }
            }
        }
    }
    r
}

struct Scenario {
    entities: usize,
    tick_ms: Vec<f64>,
    tick_nodes: Vec<f64>,
    hash: Option<String>,
    state_hash: String,
}

fn scenario(scene: &Scene, rnd: &mut Rnd, seed: u32, n: usize, ticks: u32, hash_samples: bool) -> Scenario {
    let (mut game, groups) = populate(scene, rnd, seed as i32, n);
    let (mut tick_ms, mut tick_nodes, mut hash) = (Vec::new(), Vec::new(), None);
    for t in 0..ticks {
        send_due(&mut game, scene, rnd, &groups, t);
        let n0 = game.pathfinder.stats.expanded;
        let t0 = Instant::now();
        game.step(1);
        tick_ms.push(ms(t0));
        tick_nodes.push((game.pathfinder.stats.expanded - n0) as f64);
        if hash_samples && t == 150 {
            for _ in 0..20 {
                hash_state(&game.state); // warm up
            }
            let hm: Vec<f64> = (0..100)
                .map(|_| {
                    let h0 = Instant::now();
                    std::hint::black_box(hash_state(&game.state));
                    ms(h0)
                })
                .collect();
            let path_tiles: usize = game.state.entities.iter().map(|e| e.path.len()).sum();
            hash = Some(format!(
                "{{\"units\":{},\"pathTilesInState\":{path_tiles},\"ms\":{}}}",
                game.state.entities.len(),
                summary(&hm, 3)
            ));
        }
    }
    Scenario { entities: game.state.entities.len(), tick_ms, tick_nodes, hash, state_hash: game.hash() }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1)).cloned();
    let seed: u32 = arg("seed").map_or(1, |v| v.parse().expect("--seed"));
    let ticks: u32 = arg("ticks").map_or(3000, |v| v.parse().expect("--ticks"));
    let paths: usize = arg("paths").map_or(1000, |v| v.parse().expect("--paths"));
    let sizes: Vec<usize> =
        arg("sizes").unwrap_or("100,250,500".into()).split(',').map(|v| v.parse().expect("--sizes")).collect();

    let mut rnd = Rnd::new(seed);
    let scene = Scene::new(&mut rnd);
    let count = |c: char| scene.text.chars().filter(|&ch| ch == c).count();
    println!(
        "{{\"machine\":{{\"cpus\":{},\"os\":\"{}\",\"arch\":\"{}\"}},\"map\":{{\"size\":{SIZE},\"open\":{},\"rock\":{},\"cliff\":{},\"resource\":{},\"passable\":{}}}}}",
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        std::env::consts::OS,
        std::env::consts::ARCH,
        count('.'),
        count('#') + 2,
        count('X'),
        count('~'),
        scene.passable.len()
    );

    // (a) A*
    let (pairs, pocket) = scene.path_pairs(&mut rnd, paths, 50);
    let mut pf = Pathfinder::new(&scene.map);
    run_paths(&mut pf, &pairs[..pairs.len().min(100)]); // warm up caches
    let t0 = Instant::now();
    let a = run_paths(&mut pf, &pairs);
    let total = ms(t0);
    let before = pf.stats.expanded;
    let t1 = Instant::now();
    let u = run_paths(&mut pf, &pocket);
    let pocket_ms = ms(t1);
    println!(
        "{{\"astar\":{{\"paths\":{paths},\"totalMs\":{:.1},\"ms\":{},\"nodes\":{},\"nulls\":{},\"over4096\":{},\"checksum\":\"{:x}\",\"unreachable\":{{\"requests\":{},\"totalMs\":{:.1},\"msEach\":{},\"nodesEach\":{},\"nulls\":{}}}}}}}",
        total,
        summary(&a.ms, 4),
        summary(&a.nodes, 0),
        a.nulls,
        a.nodes.iter().filter(|&&n| n > 4096.0).count(),
        a.check,
        pocket.len(),
        pocket_ms,
        summary(&u.ms, 4),
        (pf.stats.expanded - before) / pocket.len().max(1) as u64,
        u.nulls
    );

    // (b) ticks, (c) hash
    scenario(&scene, &mut rnd, seed, 100, 600, false); // warm up, and keep the random stream in step
    let largest = sizes.iter().copied().max().unwrap_or(0);
    let mut hash_line = None;
    for &n in &sizes {
        let s = scenario(&scene, &mut rnd, seed, n, ticks, n == largest);
        let rest = &s.tick_ms[1..];
        println!(
            "{{\"ticks\":{{\"units\":{n},\"entities\":{},\"ticks\":{ticks},\"burstTickMs\":{:.2},\"burstNodes\":{},\"ms\":{},\"msAll\":{},\"over4ms\":{},\"nodesPerTick\":{},\"ticksOverBudget\":{},\"totalMs\":{:.1},\"stateHash\":\"{}\"}}}}",
            s.entities,
            s.tick_ms[0],
            s.tick_nodes[0],
            summary(rest, 3),
            summary(&s.tick_ms, 3),
            s.tick_ms.iter().filter(|&&m| m > 4.0).count(),
            summary(&s.tick_nodes, 0),
            s.tick_nodes.iter().filter(|&&x| x as u64 > NODE_BUDGET).count(),
            s.tick_ms.iter().sum::<f64>(),
            s.state_hash
        );
        if s.hash.is_some() {
            hash_line = s.hash;
        }
    }
    println!("{{\"hash\":{}}}", hash_line.unwrap_or("null".into()));
    if let Some(mb) = peak_rss_mb() {
        println!("{{\"peakRssMB\":{mb}}}");
    }
}
