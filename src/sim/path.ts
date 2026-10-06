// A* on the tile grid with 8-way moves. Integer costs (10 straight, 14 diagonal) and a fixed tie-break
// order make it deterministic: the same map and endpoints always give the same path.

import { passable, type MapData } from "./map.ts";

const DIRS: [number, number, number][] = [
  [0, -1, 10], [1, 0, 10], [0, 1, 10], [-1, 0, 10],
  [1, -1, 14], [1, 1, 14], [-1, 1, 14], [-1, -1, 14],
];

function octile(ax: number, ay: number, bx: number, by: number): number {
  const dx = Math.abs(ax - bx), dy = Math.abs(ay - by);
  return 10 * Math.max(dx, dy) + 4 * Math.min(dx, dy);
}

/** Binary min-heap of [f, h, index]; ties break on h, then on tile index. */
class Heap {
  items: [number, number, number][] = [];
  push(item: [number, number, number]) {
    const a = this.items;
    a.push(item);
    let i = a.length - 1;
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (!less(a[i], a[p])) break;
      [a[i], a[p]] = [a[p], a[i]];
      i = p;
    }
  }
  pop(): [number, number, number] | undefined {
    const a = this.items;
    if (a.length === 0) return undefined;
    const top = a[0];
    const last = a.pop()!;
    if (a.length > 0) {
      a[0] = last;
      let i = 0;
      for (;;) {
        const l = 2 * i + 1, r = l + 1;
        let m = i;
        if (l < a.length && less(a[l], a[m])) m = l;
        if (r < a.length && less(a[r], a[m])) m = r;
        if (m === i) break;
        [a[i], a[m]] = [a[m], a[i]];
        i = m;
      }
    }
    return top;
  }
}

function less(a: [number, number, number], b: [number, number, number]): boolean {
  return a[0] !== b[0] ? a[0] < b[0] : a[1] !== b[1] ? a[1] < b[1] : a[2] < b[2];
}

export interface PathResult {
  tiles: { x: number; y: number }[];   // excludes the start tile, ends at the goal
  expanded: number;                    // nodes taken off the open list, for measurement scenes
}

/** Shortest path from start to goal, or null. Diagonal moves may not cut past a cliff corner. */
export function findPath(map: MapData, sx: number, sy: number, gx: number, gy: number): PathResult | null {
  if (!passable(map, gx, gy)) return null;
  const w = map.width;
  const start = sy * w + sx, goal = gy * w + gx;
  const g = new Map<number, number>([[start, 0]]);
  const from = new Map<number, number>();
  const closed = new Set<number>();
  const open = new Heap();
  open.push([octile(sx, sy, gx, gy), octile(sx, sy, gx, gy), start]);
  let expanded = 0;
  for (let item = open.pop(); item; item = open.pop()) {
    const cur = item[2];
    if (closed.has(cur)) continue;
    closed.add(cur);
    expanded++;
    if (cur === goal) {
      const tiles: { x: number; y: number }[] = [];
      for (let n = goal; n !== start; n = from.get(n)!) tiles.push({ x: n % w, y: Math.floor(n / w) });
      return { tiles: tiles.reverse(), expanded };
    }
    const cx = cur % w, cy = Math.floor(cur / w);
    for (const [dx, dy, cost] of DIRS) {
      const nx = cx + dx, ny = cy + dy;
      if (!passable(map, nx, ny)) continue;
      if (dx !== 0 && dy !== 0 && (!passable(map, cx + dx, cy) || !passable(map, cx, cy + dy))) continue;
      const n = ny * w + nx;
      if (closed.has(n)) continue;
      const ng = g.get(cur)! + cost;
      if (ng < (g.get(n) ?? Infinity)) {
        g.set(n, ng);
        from.set(n, cur);
        const h = octile(nx, ny, gx, gy);
        open.push([ng + h, h, n]);
      }
    }
  }
  return null;
}
