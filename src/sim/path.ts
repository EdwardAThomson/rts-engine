// A* on the tile grid with 8-way moves. Integer costs (10 straight, 14 diagonal) and a fixed tie-break
// order make it deterministic: the same map and endpoints always give the same path.
//
// Speed (rules-movement.md section 2): the search keeps g, came-from and closed in typed arrays that are reused
// between searches, with a generation stamp instead of clearing them, and the open list is a binary heap of
// single integer keys (f, then h, then tile index, packed into one safe integer). Every key is unique, because a
// tile is pushed again only with a strictly lower g, so the pop order, and with it the path, is exactly the one
// the earlier Map/Set version gave. The code never iterates these arrays, so this is for speed, not determinism.

import { Terrain, type MapData } from "./map.ts";

const DX = [0, 1, 0, -1, 1, 1, -1, -1];
const DY = [-1, 0, 1, 0, -1, 1, 1, -1];
const COST = [10, 10, 10, 10, 14, 14, 14, 14];

function octile(ax: number, ay: number, bx: number, by: number): number {
  const dx = Math.abs(ax - bx), dy = Math.abs(ay - by);
  return 10 * Math.max(dx, dy) + 4 * Math.min(dx, dy);
}

/** Measurement only: totals that tools/bench.ts reads. Nothing in the sim reads them. */
export const pathStats = { expanded: 0, searches: 0 };

/** Per-map scratch buffers, made once and reused by every search on that map. */
interface Scratch {
  pass: Uint8Array;        // 1 if passable; terrain never changes during a game
  region: Int32Array;      // connected-region id per passable tile (0 for cliffs)
  g: Int32Array;
  from: Int32Array;
  seen: Int32Array;        // generation in which g/from were last written
  closed: Int32Array;      // generation in which the tile was closed
  gen: number;
  hk: Int32Array;          // open list: f * hRange + h
  hi: Int32Array;          // open list: tile index, the last tie-break
  hRange: number;          // one more than the largest h on this map
}
const scratch = new WeakMap<MapData, Scratch>();

function scratchFor(map: MapData): Scratch {
  let s = scratch.get(map);
  if (s) return s;
  const n = map.width * map.height;
  const pass = new Uint8Array(n);
  for (let i = 0; i < n; i++) pass[i] = map.terrain[i] !== Terrain.cliff ? 1 : 0;
  // Connected regions (rules-movement.md section 3, after OpenRA's domains): a flood fill over passable tiles.
  // Four-way links are enough, because the corner rule only allows a diagonal step when both orthogonal
  // neighbours are passable. Two tiles in different regions have no path, so the search can refuse at once.
  const region = new Int32Array(n);
  const queue = new Int32Array(n);
  let next = 0;
  for (let i = 0; i < n; i++) {
    if (!pass[i] || region[i]) continue;
    region[i] = ++next;
    let head = 0, tail = 0;
    queue[tail++] = i;
    while (head < tail) {
      const t = queue[head++], x = t % map.width;
      const nb = [x > 0 ? t - 1 : -1, x < map.width - 1 ? t + 1 : -1, t - map.width, t + map.width];
      for (const m of nb) if (m >= 0 && m < n && pass[m] && !region[m]) { region[m] = next; queue[tail++] = m; }
    }
  }
  const hMax = 10 * Math.max(map.width, map.height) + 4 * Math.min(map.width, map.height);
  const hRange = hMax + 1;
  // f is at most 14 per tile on the longest possible path, so f * hRange + h must fit an Int32.
  if ((14 * n + hMax + 1) * hRange > 0x7fffffff) throw new Error("map too large for Int32 A* keys");
  s = { pass, region, g: new Int32Array(n), from: new Int32Array(n), seen: new Int32Array(n), closed: new Int32Array(n),
    gen: 0, hk: new Int32Array(8 * n + 1), hi: new Int32Array(8 * n + 1), hRange };
  scratch.set(map, s);
  return s;
}

export interface PathResult {
  tiles: { x: number; y: number }[];   // excludes the start tile, ends at the goal
  expanded: number;                    // nodes taken off the open list, for measurement scenes
}

/** Shortest path from start to goal, or null. Diagonal moves may not cut past a cliff corner. */
export function findPath(map: MapData, sx: number, sy: number, gx: number, gy: number): PathResult | null {
  const w = map.width, hgt = map.height;
  if (gx < 0 || gy < 0 || gx >= w || gy >= hgt) return null;
  const s = scratchFor(map);
  const pass = s.pass;
  if (!pass[gy * w + gx]) return null;
  const start = sy * w + sx, goal = gy * w + gx;
  // Different regions: no path. Only when the start is a passable map tile, so a unit standing somewhere odd
  // still searches exactly as before.
  if (sx >= 0 && sy >= 0 && sx < w && sy < hgt && pass[start] && s.region[start] !== s.region[goal]) return null;
  const gen = ++s.gen;
  if (gen === 0x3fffffff) { s.seen.fill(0); s.closed.fill(0); s.gen = 1; }   // stays a small integer
  const g = s.g, from = s.from, seen = s.seen, closed = s.closed, hRange = s.hRange;
  g[start] = 0; seen[start] = gen;
  // Heap entries are two parallel Int32Arrays: hk = f * hRange + h, then the tile index as the last tie-break.
  // They hold 8 entries per tile, the most a search can push, so they never grow; the push is written out in
  // place below rather than as a closure, so V8 keeps everything in registers.
  const hk = s.hk, hi = s.hi;
  const h0 = octile(sx, sy, gx, gy);
  hk[0] = h0 * hRange + h0; hi[0] = start;
  let size = 1;
  let expanded = 0;
  pathStats.searches++;
  while (size > 0) {
    const cur = hi[0];
    --size;
    if (size > 0) {
      const lk = hk[size], lt = hi[size];
      let i = 0;
      for (;;) {
        const l = 2 * i + 1;
        if (l >= size) break;
        let m = l;
        if (l + 1 < size) {
          const a = hk[l + 1], b = hk[l];
          if (a < b || (a === b && hi[l + 1] < hi[l])) m = l + 1;
        }
        const mk = hk[m];
        if (mk > lk || (mk === lk && hi[m] > lt)) break;
        hk[i] = mk; hi[i] = hi[m];
        i = m;
      }
      hk[i] = lk; hi[i] = lt;
    }
    if (closed[cur] === gen) continue;
    closed[cur] = gen;
    expanded++;
    if (cur === goal) {
      const tiles: { x: number; y: number }[] = [];
      for (let n = goal; n !== start; n = from[n]) tiles.push({ x: n % w, y: (n / w) | 0 });
      pathStats.expanded += expanded;
      return { tiles: tiles.reverse(), expanded };
    }
    const cx = cur % w, cy = (cur / w) | 0, gc = g[cur];
    for (let d = 0; d < 8; d++) {
      const dx = DX[d], dy = DY[d];
      const nx = cx + dx, ny = cy + dy;
      if (nx < 0 || ny < 0 || nx >= w || ny >= hgt) continue;
      const n = ny * w + nx;
      if (!pass[n]) continue;
      if (d >= 4 && (!pass[cy * w + nx] || !pass[ny * w + cx])) continue;
      if (closed[n] === gen) continue;
      const ng = gc + COST[d];
      if (seen[n] !== gen || ng < g[n]) {
        g[n] = ng; from[n] = cur; seen[n] = gen;
        const h = octile(nx, ny, gx, gy);
        const k = (ng + h) * hRange + h;
        let i = size++;
        while (i > 0) {
          const p = (i - 1) >> 1;
          const pk = hk[p];
          if (pk < k || (pk === k && hi[p] < n)) break;
          hk[i] = pk; hi[i] = hi[p];
          i = p;
        }
        hk[i] = k; hi[i] = n;
      }
    }
  }
  pathStats.expanded += expanded;
  return null;
}
