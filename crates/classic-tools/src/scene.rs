//! The bench scene: a seeded 128 x 128 map (open ground, rock plateaus, cliff ridges with gaps, resource fields
//! and one walled pocket nothing can reach), random far-apart path endpoints, and groups of units on move orders.
//!
//! It draws from its own xorshift stream, never the game's generator, and makes exactly the same calls in the
//! same order as the TypeScript bench it replaces, so the two give the same maps, orders and state hashes.

use classic_sim::map::MapData;
use classic_sim::{CommandOrder, Game, GameOptions, UnitType, parse_map};

pub const SIZE: i32 = 128;

/// The scene's random numbers (xorshift32).
pub struct Rnd(u32);

impl Rnd {
    pub fn new(seed: u32) -> Rnd {
        let r = seed.wrapping_mul(2_654_435_761);
        Rnd(if r == 0 { 1 } else { r })
    }

    /// An integer in `[0, n)`.
    pub fn next(&mut self, n: u32) -> u32 {
        let mut r = self.0;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        self.0 = r;
        r % n
    }

    fn int(&mut self, n: i32) -> i32 {
        self.next(n as u32) as i32
    }
}

type Grid = Vec<Vec<u8>>;

fn set(g: &mut Grid, x: i32, y: i32, c: u8) {
    if x >= 0 && y >= 0 && x < SIZE && y < SIZE {
        g[y as usize][x as usize] = c;
    }
}

fn blob(g: &mut Grid, rnd: &mut Rnd, cx: i32, cy: i32, rad: i32, c: u8) {
    for y in cy - rad..=cy + rad {
        for x in cx - rad..=cx + rad {
            if (x - cx).pow(2) + (y - cy).pow(2) <= rad * rad + rnd.int(rad + 1) {
                set(g, x, y, c);
            }
        }
    }
}

/// The map as ASCII text.
pub fn make_map(rnd: &mut Rnd) -> String {
    let mut g: Grid = vec![vec![b'.'; SIZE as usize]; SIZE as usize];
    for _ in 0..18 {
        let (cx, cy, rad) = (rnd.int(SIZE), rnd.int(SIZE), 3 + rnd.int(6));
        blob(&mut g, rnd, cx, cy, rad, b'#'); // rock plateaus
    }
    for _ in 0..22 {
        let (cx, cy, rad) = (rnd.int(SIZE), rnd.int(SIZE), 2 + rnd.int(4));
        blob(&mut g, rnd, cx, cy, rad, b'~'); // resource fields
    }
    // Cliff ridges: random walks 20 to 70 tiles long, with a gap every so often so most ground stays connected.
    for _ in 0..26 {
        let (mut x, mut y) = (rnd.int(SIZE), rnd.int(SIZE));
        let len = 20 + rnd.int(50);
        let dir = rnd.int(4);
        for k in 0..len {
            if k % 17 < 15 {
                set(&mut g, x, y, b'X');
                set(&mut g, x + 1, y, b'X');
            }
            match dir {
                0 => {
                    x += 1;
                    y += rnd.int(3) - 1;
                }
                1 => {
                    y += 1;
                    x += rnd.int(3) - 1;
                }
                2 => {
                    x += 1;
                    y += 1;
                }
                _ => {
                    x += 1;
                    y -= 1;
                }
            }
        }
    }
    // A walled pocket of open ground: reachable from nowhere, so requests into it test the unreachable case.
    for y in 56..=70 {
        for x in 56..=70 {
            let wall = y == 56 || y == 70 || x == 56 || x == 70;
            set(&mut g, x, y, if wall { b'X' } else { b'.' });
        }
    }
    // Two start plateaus in opposite corners, with clear ground round them.
    for (sx, sy, c) in [(8, 8, b'1'), (118, 118, b'2')] {
        for y in sy - 4..=sy + 4 {
            for x in sx - 4..=sx + 4 {
                set(&mut g, x, y, b'.');
            }
        }
        for y in sy - 2..=sy {
            for x in sx - 2..=sx + 2 {
                set(&mut g, x, y, b'#');
            }
        }
        set(&mut g, sx, sy, c);
    }
    let rows: Vec<String> = g.into_iter().map(|r| String::from_utf8(r).expect("ascii")).collect();
    rows.join("\n")
}

/// A path request: start and goal tile indexes.
pub type Pair = (usize, usize);

/// The map, its passable tiles, and which of them can be reached from outside the pocket.
pub struct Scene {
    pub text: String,
    pub map: MapData,
    pub passable: Vec<usize>,
    pub reachable: Vec<usize>,
    pub pocket: Vec<usize>,
    is_reachable: Vec<bool>,
}

pub fn in_pocket(i: usize) -> bool {
    let (x, y) = ((i % SIZE as usize) as i32, (i / SIZE as usize) as i32);
    x > 56 && x < 70 && y > 56 && y < 70
}

fn octile(a: usize, b: usize) -> i32 {
    let s = SIZE as usize;
    let dx = ((a % s) as i32 - (b % s) as i32).abs();
    let dy = ((a / s) as i32 - (b / s) as i32).abs();
    10 * dx.max(dy) + 4 * dx.min(dy)
}

impl Scene {
    pub fn new(rnd: &mut Rnd) -> Scene {
        let text = make_map(rnd);
        let map = parse_map(&text).expect("scene map parses");
        let n = (SIZE * SIZE) as usize;
        let passable: Vec<usize> = (0..n).filter(|&i| map.passable(i as i32 % SIZE, i as i32 / SIZE)).collect();
        let reachable: Vec<usize> = passable.iter().copied().filter(|&i| !in_pocket(i)).collect();
        let pocket: Vec<usize> = passable.iter().copied().filter(|&i| in_pocket(i)).collect();
        let mut is_reachable = vec![false; n];
        for &i in &reachable {
            is_reachable[i] = true;
        }
        Scene { text, map, passable, reachable, pocket, is_reachable }
    }

    pub fn random_reachable(&self, rnd: &mut Rnd) -> usize {
        self.reachable[rnd.next(self.reachable.len() as u32) as usize]
    }

    /// A reachable tile at least 64 tiles' cost away from `a`.
    pub fn far_from(&self, rnd: &mut Rnd, a: usize) -> usize {
        loop {
            let b = self.random_reachable(rnd);
            if octile(a, b) >= 640 {
                return b;
            }
        }
    }

    /// Long random path requests, then 50 into the pocket, as the bench uses them.
    pub fn path_pairs(&self, rnd: &mut Rnd, count: usize, pocket: usize) -> (Vec<Pair>, Vec<Pair>) {
        let pairs = (0..count)
            .map(|_| {
                let a = self.random_reachable(rnd);
                (a, self.far_from(rnd, a))
            })
            .collect();
        let into_pocket = (0..pocket)
            .map(|_| (self.random_reachable(rnd), self.pocket[rnd.next(self.pocket.len() as u32) as usize]))
            .collect();
        (pairs, into_pocket)
    }
}

/// A box-selected group: up to 20 tanks of one owner sharing a target, re-ordered every 300 ticks at `offset`.
pub struct Group {
    pub owner: u32,
    pub ids: Vec<u32>,
    pub offset: u32,
}

/// A two-player game on the scene's map with `n` extra units (every tenth a harvester) in random places.
pub fn populate(scene: &Scene, rnd: &mut Rnd, seed: i32, n: usize) -> (Game, Vec<Group>) {
    let mut game = Game::new(GameOptions { map: &scene.text, seed, players: Some(2) }).expect("scene game");
    let mut owners: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
    for i in 0..n {
        let owner = (i % 2) as u32;
        let t = scene.random_reachable(rnd);
        let kind = if i % 10 == 9 { UnitType::Harvester } else { UnitType::Tank };
        let id = game.spawn(kind, owner, t as i32 % SIZE, t as i32 / SIZE);
        if kind == UnitType::Tank {
            owners[owner as usize].push(id);
        }
    }
    let mut groups: Vec<Group> = Vec::new();
    for owner in [0u32, 1] {
        for chunk in owners[owner as usize].chunks(20) {
            let offset = 1 + ((groups.len() as u32 * 13) % 299);
            groups.push(Group { owner, ids: chunk.to_vec(), offset });
        }
    }
    (game, groups)
}

/// Order every group due on tick `t`: all at tick 0, then each again every 300 ticks at its offset.
pub fn send_due(game: &mut Game, scene: &Scene, rnd: &mut Rnd, groups: &[Group], t: u32) {
    for gr in groups {
        if t == 0 || t % 300 == gr.offset {
            let e = game.state.entity(gr.ids[0]).expect("group leader exists");
            let tile = e.tile();
            let from = (tile.y * SIZE + tile.x) as usize;
            let start = if scene.is_reachable.get(from).copied().unwrap_or(false) { from } else { scene.reachable[0] };
            let to = scene.far_from(rnd, start);
            game.order(gr.owner, &gr.ids, CommandOrder::Move { x: to as i32 % SIZE, y: to as i32 / SIZE });
        }
    }
}

/// The bench's path checksum: FNV-1a over 16-bit halves of each result's node count and tiles.
pub fn fnv_pair(h: u32, n: u32) -> u32 {
    let h = (h ^ (n & 0xffff)).wrapping_mul(0x0100_0193);
    (h ^ (n >> 16)).wrapping_mul(0x0100_0193)
}
