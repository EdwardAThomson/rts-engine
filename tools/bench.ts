// Performance bench for the simulation (engineering.md "Performance budget", rules-movement.md section 12).
//
//   node --expose-gc tools/bench.ts [--engine <repo root>] [--seed 1] [--ticks 3000] [--sizes 100,250,500]
//                                   [--paths 1000] [--json out.json]
//
// It builds a seeded 128 x 128 map (open ground, rock plateaus, cliff ridges with gaps, resource fields and one
// walled pocket nothing can reach), then measures:
//   (a) A*: time and nodes expanded for long random paths, p50 / p95 / p99
//   (b) whole ticks with N units on move orders across the map (all ordered at tick 0, then groups re-ordered
//       in turn every 300 ticks), p50 / p95 / p99 tick time and nodes expanded per tick
//   (c) hashState on the 500-unit state
//   (d) heap growth and GC pauses, from perf_hooks "gc" entries
// Every scenario prints a checksum or state hash, so an optimised engine can be checked against the original.
// The bench's own random numbers come from a local xorshift, never Math.random(), so every run sees the same
// map, endpoints and orders. Timing uses performance.now(), which is fine here: the bench is not the sim.

import { PerformanceObserver, performance } from "node:perf_hooks";
import { writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const args = new Map<string, string>();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
const engine = resolve(args.get("engine") ?? new URL("..", import.meta.url).pathname);
const seed = Number(args.get("seed") ?? 1);
const TICKS = Number(args.get("ticks") ?? 3000);
const SIZES = (args.get("sizes") ?? "100,250,500").split(",").map(Number);
const PATHS = Number(args.get("paths") ?? 1000);
const SIZE = 128;

const load = (p: string) => import(pathToFileURL(`${engine}/src/sim/${p}`).href);
const { createGame, hashState } = await load("game.ts");
const pathMod = await load("path.ts");
const { findPath } = pathMod;
const { parseMap, passable } = await load("map.ts");
// Optional instrumentation: an engine may export pathStats = { expanded, searches } that findPath adds to.
const stats: { expanded: number; searches: number } | undefined = pathMod.pathStats;

// ---------- bench random numbers (xorshift32, seeded) ----------
let r = (seed * 2654435761) >>> 0 || 1;
function rnd(n: number): number {
  r ^= r << 13; r >>>= 0;
  r ^= r >>> 17;
  r ^= r << 5; r >>>= 0;
  return r % n;
}

// ---------- map ----------
function makeMap(): string {
  const g: string[][] = Array.from({ length: SIZE }, () => Array(SIZE).fill("."));
  const set = (x: number, y: number, c: string) => { if (x >= 0 && y >= 0 && x < SIZE && y < SIZE) g[y][x] = c; };
  const blob = (cx: number, cy: number, rad: number, c: string) => {
    for (let y = cy - rad; y <= cy + rad; y++)
      for (let x = cx - rad; x <= cx + rad; x++)
        if ((x - cx) ** 2 + (y - cy) ** 2 <= rad * rad + rnd(rad + 1)) set(x, y, c);
  };
  for (let i = 0; i < 18; i++) blob(rnd(SIZE), rnd(SIZE), 3 + rnd(6), "#");       // rock plateaus
  for (let i = 0; i < 22; i++) blob(rnd(SIZE), rnd(SIZE), 2 + rnd(4), "~");       // resource fields
  // Cliff ridges: random walks 20 to 70 tiles long, with a gap every so often so most ground stays connected.
  for (let i = 0; i < 26; i++) {
    let x = rnd(SIZE), y = rnd(SIZE);
    const len = 20 + rnd(50);
    const dir = rnd(4);
    for (let k = 0; k < len; k++) {
      if (k % 17 < 15) { set(x, y, "X"); set(x + 1, y, "X"); }
      if (dir === 0) { x++; y += rnd(3) - 1; } else if (dir === 1) { y++; x += rnd(3) - 1; }
      else if (dir === 2) { x++; y++; } else { x++; y--; }
    }
  }
  // A walled pocket of open ground: reachable from nowhere, so requests into it test the unreachable case.
  for (let y = 56; y <= 70; y++) for (let x = 56; x <= 70; x++) set(x, y, y === 56 || y === 70 || x === 56 || x === 70 ? "X" : ".");
  // Two start plateaus in opposite corners, with clear ground round them.
  for (const [sx, sy, c] of [[8, 8, "1"], [118, 118, "2"]] as const) {
    for (let y = sy - 4; y <= sy + 4; y++) for (let x = sx - 4; x <= sx + 4; x++) set(x, y, ".");
    for (let y = sy - 2; y <= sy; y++) for (let x = sx - 2; x <= sx + 2; x++) set(x, y, "#");
    set(sx, sy, c);
  }
  return g.map((row) => row.join("")).join("\n");
}

const pct = (xs: number[], p: number) => {
  const s = xs.slice().sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
};
const summary = (xs: number[], digits = 3) => {
  const f = (v: number) => Number(v.toFixed(digits));
  return { p50: f(pct(xs, 50)), p95: f(pct(xs, 95)), p99: f(pct(xs, 99)), max: f(Math.max(...xs)), mean: f(xs.reduce((a, b) => a + b, 0) / xs.length) };
};
function fnv(h: number, n: number): number {
  h ^= n & 0xffff; h = Math.imul(h, 0x01000193);
  h ^= n >>> 16; return Math.imul(h, 0x01000193);
}

// ---------- GC observer ----------
type Gc = { kind: number; ms: number; at: number };
let gcLog: Gc[] = [];
const obs = new PerformanceObserver((list) => {
  for (const e of list.getEntries()) gcLog.push({ kind: (e as any).detail?.kind ?? 0, ms: e.duration, at: e.startTime });
});
obs.observe({ entryTypes: ["gc"] });
const GC_KIND: Record<number, string> = { 1: "scavenge", 2: "mark-compact", 4: "incremental", 8: "weak-cb", 16: "minor-mc" };
async function gcSummary() {
  await new Promise((res) => setImmediate(res));   // let the observer flush
  const by: Record<string, { count: number; ms: number; maxMs: number }> = {};
  for (const g of gcLog) {
    const k = GC_KIND[g.kind] ?? String(g.kind);
    by[k] ??= { count: 0, ms: 0, maxMs: 0 };
    by[k].count++; by[k].ms += g.ms; by[k].maxMs = Math.max(by[k].maxMs, g.ms);
  }
  for (const k in by) { by[k].ms = Number(by[k].ms.toFixed(2)); by[k].maxMs = Number(by[k].maxMs.toFixed(2)); }
  const out = { count: gcLog.length, totalMs: Number(gcLog.reduce((a, g) => a + g.ms, 0).toFixed(2)),
    maxMs: Number(Math.max(0, ...gcLog.map((g) => g.ms)).toFixed(2)), by };
  gcLog = [];
  return out;
}
const fullGc = () => (globalThis as any).gc?.();
const mb = (n: number) => Number((n / 1048576).toFixed(1));

// ---------- setup ----------
const MAP = makeMap();
const map = parseMap(MAP);
const passableTiles: number[] = [];
for (let i = 0; i < SIZE * SIZE; i++) if (passable(map, i % SIZE, Math.floor(i / SIZE))) passableTiles.push(i);
const inPocket = (i: number) => { const x = i % SIZE, y = Math.floor(i / SIZE); return x > 56 && x < 70 && y > 56 && y < 70; };
const reachable = passableTiles.filter((i) => !inPocket(i));
const octile = (a: number, b: number) => {
  const dx = Math.abs((a % SIZE) - (b % SIZE)), dy = Math.abs(Math.floor(a / SIZE) - Math.floor(b / SIZE));
  return 10 * Math.max(dx, dy) + 4 * Math.min(dx, dy);
};
const farFrom = (a: number) => { for (;;) { const b = reachable[rnd(reachable.length)]; if (octile(a, b) >= 640) return b; } };
const count = (c: string) => MAP.split("").filter((ch) => ch === c).length;

const report: any = {
  date: new Date().toISOString().slice(0, 10), node: process.version, cpus: cpus().length, cpu: cpus()[0]?.model,
  engine, seed, ticks: TICKS,
  map: { size: SIZE, open: count("."), rock: count("#") + 2, cliff: count("X"), resource: count("~"), passable: passableTiles.length },
};
console.log(JSON.stringify({ machine: { node: report.node, cpus: report.cpus, cpu: report.cpu }, map: report.map }));

// ---------- (a) A* ----------
{
  const pairs: [number, number][] = [];
  for (let i = 0; i < PATHS; i++) { const a = reachable[rnd(reachable.length)]; pairs.push([a, farFrom(a)]); }
  const pocket: [number, number][] = [];
  const pocketTiles = passableTiles.filter(inPocket);
  for (let i = 0; i < 50; i++) pocket.push([reachable[rnd(reachable.length)], pocketTiles[rnd(pocketTiles.length)]]);
  const run = (list: [number, number][]) => {
    const ms: number[] = [], nodes: number[] = [];
    let check = 0x811c9dc5, nulls = 0;
    for (const [a, b] of list) {
      const t0 = performance.now();
      const p = findPath(map, a % SIZE, Math.floor(a / SIZE), b % SIZE, Math.floor(b / SIZE));
      ms.push(performance.now() - t0);
      if (!p) { nulls++; nodes.push(stats ? 0 : 0); check = fnv(check, 0xffffffff); continue; }
      nodes.push(p.expanded);
      check = fnv(check, p.expanded);
      for (const t of p.tiles) check = fnv(check, t.y * SIZE + t.x);
    }
    return { ms, nodes, nulls, check: (check >>> 0).toString(16) };
  };
  run(pairs.slice(0, 100));            // warm up the JIT
  fullGc(); await gcSummary();
  const heap0 = process.memoryUsage().heapUsed;
  const t0 = performance.now();
  const a = run(pairs);
  const total = performance.now() - t0;
  const gc = await gcSummary();
  const before = stats?.expanded ?? 0;
  const t1 = performance.now();
  const u = run(pocket);
  const pocketMs = performance.now() - t1;
  report.astar = {
    paths: PATHS, totalMs: Number(total.toFixed(1)), ms: summary(a.ms), nodes: summary(a.nodes, 0), nulls: a.nulls,
    over4096: a.nodes.filter((n) => n > 4096).length, checksum: a.check,
    unreachable: { requests: pocket.length, totalMs: Number(pocketMs.toFixed(1)), msEach: summary(u.ms),
      nodesEach: stats ? Math.round((stats.expanded - before) / pocket.length) : null, nulls: u.nulls },
    heapGrowthMB: mb(process.memoryUsage().heapUsed - heap0), gc,
  };
  console.log(JSON.stringify({ astar: report.astar }));
}

// ---------- (b) ticks, (c) hash, (d) memory ----------
function scenario(n: number, ticks: number, hashSamples: boolean) {
  const game = createGame({ map: MAP, seed, players: 2 });
  const owners: number[][] = [[], []];
  for (let i = 0; i < n; i++) {
    const owner = i % 2;
    const t = reachable[rnd(reachable.length)];
    const type = i % 10 === 9 ? "harvester" : "tank";
    const id = game.spawn(type, owner, t % SIZE, Math.floor(t / SIZE));
    if (type === "tank") owners[owner].push(id);
  }
  // Groups of 20 tanks of one owner share a target, as a box-selected move order would.
  const groups: { owner: number; ids: number[]; offset: number }[] = [];
  for (const owner of [0, 1]) for (let k = 0; k < owners[owner].length; k += 20)
    groups.push({ owner, ids: owners[owner].slice(k, k + 20), offset: 1 + ((groups.length * 13) % 299) });
  const send = (gr: { owner: number; ids: number[] }) => {
    const e = game.state.entities.find((u: any) => u.id === gr.ids[0]);
    const from = Math.floor(e.y / 256) * SIZE + Math.floor(e.x / 256);
    const to = farFrom(reachable.includes(from) ? from : reachable[0]);
    game.order(gr.owner, gr.ids, "move", to % SIZE, Math.floor(to / SIZE));
  };
  const tickMs: number[] = [], tickNodes: number[] = [];
  let heapPeak = 0, hash: any = null;
  fullGc(); gcLog = [];
  const heap0 = process.memoryUsage().heapUsed;
  for (let t = 0; t < ticks; t++) {
    for (const gr of groups) if (t === 0 || (t % 300 === gr.offset)) send(gr);
    const n0 = stats?.expanded ?? 0;
    const t0 = performance.now();
    game.step(1);
    tickMs.push(performance.now() - t0);
    tickNodes.push((stats?.expanded ?? 0) - n0);
    if (t % 100 === 0) heapPeak = Math.max(heapPeak, process.memoryUsage().heapUsed);
    if (hashSamples && t === 150) {
      const hm: number[] = [];
      for (let k = 0; k < 20; k++) hashState(game.state);     // warm up
      for (let k = 0; k < 100; k++) { const h0 = performance.now(); hashState(game.state); hm.push(performance.now() - h0); }
      const pathTiles = game.state.entities.reduce((a: number, e: any) => a + e.path.length, 0);
      hash = { units: game.state.entities.length, pathTilesInState: pathTiles, ms: summary(hm) };
    }
  }
  return { game, tickMs, tickNodes, heap0, heapPeak, hash };
}

scenario(100, 600, false);   // warm up the JIT on every code path
await gcSummary();
report.ticks = [];
for (const n of SIZES) {
  const s = scenario(n, TICKS, n === Math.max(...SIZES));
  const gc = await gcSummary();
  const heapEnd = process.memoryUsage().heapUsed;
  const rest = s.tickMs.slice(1);
  const row = {
    units: n, entities: s.game.state.entities.length, ticks: TICKS,
    burstTickMs: Number(s.tickMs[0].toFixed(2)), burstNodes: s.tickNodes[0],
    ms: summary(rest), msAll: summary(s.tickMs), over4ms: s.tickMs.filter((m) => m > 4).length,
    nodesPerTick: stats ? summary(s.tickNodes, 0) : null, ticksOver20k: s.tickNodes.filter((x) => x > 20000).length,
    totalMs: Number(s.tickMs.reduce((a, b) => a + b, 0).toFixed(1)),
    heapStartMB: mb(s.heap0), heapPeakMB: mb(Math.max(s.heapPeak, heapEnd)), heapEndMB: mb(heapEnd), gc,
    stateHash: s.game.hash(),
  };
  report.ticks.push(row);
  if (s.hash) report.hash = s.hash;
  console.log(JSON.stringify({ ticks: row }));
}
console.log(JSON.stringify({ hash: report.hash }));
obs.disconnect();
if (args.get("json")) writeFileSync(args.get("json")!, JSON.stringify(report, null, 2));
