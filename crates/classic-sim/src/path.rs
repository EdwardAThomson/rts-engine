//! A* on the tile grid with 8-way moves. Integer costs (10 straight, 14 diagonal) and a fixed tie-break order
//! make it deterministic: the same map and endpoints always give the same path.
//!
//! The search keeps g, came-from and closed in arrays reused between searches, with a generation stamp instead
//! of clearing them, and the open list is a binary heap ordered by (f, then h, then tile index). Every entry is
//! unique, because a tile is pushed again only with a strictly lower g, so the pop order, and with it the path,
//! never depends on how the heap is laid out. Nothing iterates these arrays, so they are for speed only.

use crate::map::{MapData, Terrain, Tile};

const DX: [i32; 8] = [0, 1, 0, -1, 1, 1, -1, -1];
const DY: [i32; 8] = [-1, 0, 1, 0, -1, 1, 1, -1];
const COST: [i32; 8] = [10, 10, 10, 10, 14, 14, 14, 14];

fn octile(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    let (dx, dy) = ((ax - bx).abs(), (ay - by).abs());
    10 * dx.max(dy) + 4 * dx.min(dy)
}

/// Measurement only: totals the bench reads. Nothing in the simulation reads them.
#[derive(Clone, Copy, Debug, Default)]
pub struct PathStats {
    pub expanded: u64,
    pub searches: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathResult {
    /// Excludes the start tile, ends at the goal.
    pub tiles: Vec<Tile>,
    /// Nodes taken off the open list, for measurement.
    pub expanded: u32,
}

/// Pathfinding for one map, with scratch buffers made once and reused by every search.
#[derive(Clone, Debug)]
pub struct Pathfinder {
    w: i32,
    h: i32,
    /// 1 if passable: not cliff, and not under a building.
    pass: Vec<u8>,
    /// 1 if the terrain is passable; terrain never changes during a game.
    ground: Vec<u8>,
    /// Buildings covering each tile; a tile is blocked while this is above zero.
    blocked: Vec<u16>,
    /// Connected-region id per passable tile (0 for cliffs and buildings).
    region: Vec<u32>,
    g: Vec<i32>,
    from: Vec<u32>,
    /// Generation in which `g` and `from` were last written.
    seen: Vec<u32>,
    /// Generation in which the tile was closed.
    closed: Vec<u32>,
    gen_: u32,
    /// Open list: `f * h_range + h`, and the tile index as the last tie-break.
    hk: Vec<i64>,
    hi: Vec<u32>,
    /// One more than the largest h on this map.
    h_range: i64,
    pub stats: PathStats,
}

impl Pathfinder {
    pub fn new(map: &MapData) -> Self {
        let n = (map.width * map.height) as usize;
        let pass: Vec<u8> = map.terrain.iter().map(|&t| u8::from(t != Terrain::Cliff)).collect();
        let h_max = octile(0, 0, map.width, map.height) as i64;
        let mut pf = Self {
            w: map.width,
            h: map.height,
            ground: pass.clone(),
            blocked: vec![0; n],
            region: vec![0; n],
            pass,
            g: vec![0; n],
            from: vec![0; n],
            seen: vec![0; n],
            closed: vec![0; n],
            gen_: 0,
            hk: vec![0; 8 * n + 1],
            hi: vec![0; 8 * n + 1],
            h_range: h_max + 1,
            stats: PathStats::default(),
        };
        pf.label_regions();
        pf
    }

    /// Block (or unblock) a building's footprint, clipped to the map, and relabel the regions.
    pub fn set_blocked(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for ty in y.max(0)..(y + h).min(self.h) {
            for tx in x.max(0)..(x + w).min(self.w) {
                let i = (ty * self.w + tx) as usize;
                self.blocked[i] = if on { self.blocked[i] + 1 } else { self.blocked[i].saturating_sub(1) };
                self.pass[i] = u8::from(self.ground[i] != 0 && self.blocked[i] == 0);
            }
        }
        self.label_regions();
    }

    /// Whether ground units can enter this tile now.
    pub fn passable(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.pass[(y * self.w + x) as usize] != 0
    }

    fn label_regions(&mut self) {
        let n = self.pass.len();
        let w = self.w as usize;
        let pass = &self.pass;
        // Connected regions (an idea from OpenRA's domains): a flood fill over passable tiles. Four-way links are
        // enough, because the corner rule only allows a diagonal step when both orthogonal neighbours are
        // passable. Two tiles in different regions have no path, so the search can refuse at once.
        let region = &mut self.region;
        region.fill(0);
        let mut queue = vec![0usize; n];
        let mut next = 0;
        for i in 0..n {
            if pass[i] == 0 || region[i] != 0 {
                continue;
            }
            next += 1;
            region[i] = next;
            let (mut head, mut tail) = (0, 0);
            queue[tail] = i;
            tail += 1;
            while head < tail {
                let t = queue[head];
                head += 1;
                let x = t % w;
                let nb = [
                    (x > 0).then(|| t - 1),
                    (x < w - 1).then(|| t + 1),
                    t.checked_sub(w),
                    Some(t + w).filter(|&m| m < n),
                ];
                for m in nb.into_iter().flatten() {
                    if pass[m] != 0 && region[m] == 0 {
                        region[m] = next;
                        queue[tail] = m;
                        tail += 1;
                    }
                }
            }
        }
    }

    /// Shortest path from start to goal, or `None`. Diagonal moves may not cut past a cliff corner.
    pub fn find(&mut self, sx: i32, sy: i32, gx: i32, gy: i32) -> Option<PathResult> {
        let (w, hgt) = (self.w, self.h);
        if gx < 0 || gy < 0 || gx >= w || gy >= hgt || sx < 0 || sy < 0 || sx >= w || sy >= hgt {
            return None;
        }
        let goal = (gy * w + gx) as u32;
        let start = (sy * w + sx) as u32;
        if self.pass[goal as usize] == 0 {
            return None;
        }
        // Different regions: no path. Only when the start is passable, so a unit standing somewhere odd still
        // searches.
        if self.pass[start as usize] != 0 && self.region[start as usize] != self.region[goal as usize] {
            return None;
        }
        self.gen_ += 1;
        if self.gen_ == 0x3fff_ffff {
            self.seen.fill(0);
            self.closed.fill(0);
            self.gen_ = 1;
        }
        let gen_ = self.gen_;
        let h_range = self.h_range;
        let Self { pass, g, from, seen, closed, hk, hi, stats, .. } = self;
        g[start as usize] = 0;
        seen[start as usize] = gen_;
        let h0 = octile(sx, sy, gx, gy) as i64;
        hk[0] = h0 * h_range + h0;
        hi[0] = start;
        let mut size = 1usize;
        let mut expanded = 0u32;
        stats.searches += 1;
        while size > 0 {
            let cur = hi[0];
            size -= 1;
            if size > 0 {
                // Move the last entry to the root and sift it down.
                let (lk, lt) = (hk[size], hi[size]);
                let mut i = 0;
                loop {
                    let l = 2 * i + 1;
                    if l >= size {
                        break;
                    }
                    let mut m = l;
                    if l + 1 < size {
                        let (a, b) = (hk[l + 1], hk[l]);
                        if a < b || (a == b && hi[l + 1] < hi[l]) {
                            m = l + 1;
                        }
                    }
                    let mk = hk[m];
                    if mk > lk || (mk == lk && hi[m] > lt) {
                        break;
                    }
                    hk[i] = mk;
                    hi[i] = hi[m];
                    i = m;
                }
                hk[i] = lk;
                hi[i] = lt;
            }
            let c = cur as usize;
            if closed[c] == gen_ {
                continue;
            }
            closed[c] = gen_;
            expanded += 1;
            if cur == goal {
                let mut tiles = Vec::new();
                let mut n = goal;
                while n != start {
                    tiles.push(Tile { x: (n % w as u32) as i32, y: (n / w as u32) as i32 });
                    n = from[n as usize];
                }
                tiles.reverse();
                stats.expanded += expanded as u64;
                return Some(PathResult { tiles, expanded });
            }
            let (cx, cy) = ((cur % w as u32) as i32, (cur / w as u32) as i32);
            let gc = g[c];
            for d in 0..8 {
                let (nx, ny) = (cx + DX[d], cy + DY[d]);
                if nx < 0 || ny < 0 || nx >= w || ny >= hgt {
                    continue;
                }
                let n = (ny * w + nx) as usize;
                if pass[n] == 0 {
                    continue;
                }
                if d >= 4 && (pass[(cy * w + nx) as usize] == 0 || pass[(ny * w + cx) as usize] == 0) {
                    continue;
                }
                if closed[n] == gen_ {
                    continue;
                }
                let ng = gc + COST[d];
                if seen[n] != gen_ || ng < g[n] {
                    g[n] = ng;
                    from[n] = cur;
                    seen[n] = gen_;
                    let h = octile(nx, ny, gx, gy) as i64;
                    let k = (ng as i64 + h) * h_range + h;
                    let nt = n as u32;
                    let mut i = size;
                    size += 1;
                    while i > 0 {
                        let p = (i - 1) >> 1;
                        let pk = hk[p];
                        if pk < k || (pk == k && hi[p] < nt) {
                            break;
                        }
                        hk[i] = pk;
                        hi[i] = hi[p];
                        i = p;
                    }
                    hk[i] = k;
                    hi[i] = nt;
                }
            }
        }
        stats.expanded += expanded as u64;
        None
    }
}
