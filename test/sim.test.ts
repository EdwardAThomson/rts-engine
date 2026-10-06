// Checks from guide 01 Part 2, applied to the simulation core. Each test prints what it ran, so a pass
// can't come from a check that silently did nothing (guide 01, rule 2).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createGame, type Game } from "../src/sim/game.ts";
import { findPath } from "../src/sim/path.ts";
import { parseMap, Terrain } from "../src/sim/map.ts";

const MAP = readFileSync(new URL("../maps/test-01.txt", import.meta.url), "utf8");

/** A fixed script of player orders, so runs exercise commands as well as the automatic harvesters. */
function scripted(game: Game, ticks: number) {
  const tanks = game.state.entities.filter((e) => e.type === "tank");
  for (let t = 0; t < ticks; t += 500) {
    tanks.forEach((tank, k) => game.order(tank.owner, [tank.id], "move", (t / 50 + 7 * k) % 30, (t / 100 + 3 * k) % 18));
    game.step(Math.min(500, ticks - t));
  }
}

test("determinism: same seed and orders give the same state after 10,000 ticks", () => {
  const a = createGame({ map: MAP, seed: 7 });
  const b = createGame({ map: MAP, seed: 7 });
  scripted(a, 10_000);
  scripted(b, 10_000);
  console.log(`ticks=${a.state.tick} entities=${a.state.entities.length} credits=${a.state.players.map((p) => p.credits)} hash=${a.hash()}`);
  assert.equal(a.state.tick, 10_000);
  assert.equal(a.hash(), b.hash());
});

test("seeds matter: a different seed changes the game (resource regrowth uses the RNG)", () => {
  const a = createGame({ map: MAP, seed: 1 });
  const b = createGame({ map: MAP, seed: 2 });
  a.step(10_000);
  b.step(10_000);
  const grown = (g: Game) => g.events.filter((e) => e.event === "regrowth").map((e) => `${e.x},${e.y}`).join(" ");
  console.log(`seed1 regrowth: ${grown(a) || "none"} | seed2 regrowth: ${grown(b) || "none"}`);
  assert.notEqual(a.hash(), b.hash());
});

test("replay: re-running seed + map + command log reproduces the game", () => {
  const live = createGame({ map: MAP, seed: 3 });
  scripted(live, 6_000);
  const log = live.commandLog();
  const replay = createGame({ map: MAP, seed: 3 });
  for (let t = 0; t < 6_000; t++) {
    for (const c of log.filter((c) => c.tick === t)) replay.order(c.player, c.ids, c.order, c.x, c.y);
    replay.step(1);
  }
  console.log(`commands=${log.length} live=${live.hash()} replay=${replay.hash()}`);
  assert.ok(log.length > 0);
  assert.equal(replay.hash(), live.hash());
});

test("harvester round trip: first delivery arrives in under a minute of game time", () => {
  const game = createGame({ map: MAP, seed: 1 });
  game.step(900);   // 60 s at 15 ticks per second
  const first = game.events.filter((e) => e.event === "delivered");
  console.log(`deliveries in 900 ticks: ${first.map((e) => `p${e.player}@${e.tick}`).join(" ")}`);
  for (const p of game.state.players) assert.ok(p.credits >= 200, `player ${p.id} has ${p.credits} credits`);
});

test("invariants hold every tick: resource is conserved, credits never negative, ids stay sorted", () => {
  const game = createGame({ map: MAP, seed: 11 });
  const start = game.state.resource.reduce((a, b) => a + b, 0);
  let checked = 0;
  for (let t = 0; t < 5_000; t++) {
    game.step(1);
    const s = game.state;
    // Resource only moves between the map, harvester cargo and delivered credits; regrowth is the only
    // source, adding at most 40 per regrowth event.
    const regrowths = game.events.filter((e) => e.event === "regrowth").length;
    const onMap = s.resource.reduce((a, b) => a + b, 0);
    const carried = s.entities.reduce((a, e) => a + (e.cargo ?? 0), 0);
    const delivered = s.players.reduce((a, p) => a + p.delivered, 0);
    const created = onMap + carried + delivered - start;
    assert.ok(created >= 0 && created <= regrowths * 40, `tick ${s.tick}: resource balance off by ${created}`);
    for (const p of s.players) assert.ok(p.credits >= 0);
    for (let i = 1; i < s.entities.length; i++) assert.ok(s.entities[i - 1].id < s.entities[i].id);
    checked++;
  }
  console.log(`checked ${checked} ticks, resource on map now ${game.snapshot().resourceLeft}`);
  assert.equal(checked, 5_000);
});

test("pathfinding: paths never enter a cliff and go round the ridge", () => {
  const map = parseMap(MAP);
  const p = findPath(map, 10, 6, 22, 6);
  assert.ok(p, "no path found");
  const onCliff = p.tiles.filter((t) => map.terrain[t.y * map.width + t.x] === Terrain.cliff);
  console.log(`path length ${p.tiles.length} tiles, ${p.expanded} nodes expanded, cliff tiles on path: ${onCliff.length}`);
  assert.equal(onCliff.length, 0);
  const crossing = p.tiles.find((t) => t.x === 16)!;
  console.log(`crosses column 16 at row ${crossing.y} (ridge covers rows 4 to 9)`);
  assert.ok(crossing.y < 4 || crossing.y > 9, "path should go round the ridge, not through it");
  assert.equal(findPath(map, 10, 6, 16, 6), null, "a cliff tile is not a valid goal");
});

test("move order: a tank reaches the tile it was sent to", () => {
  const game = createGame({ map: MAP, seed: 1 });
  const tank = game.state.entities.find((e) => e.type === "tank" && e.owner === 0)!;
  game.order(0, [tank.id], "move", 20, 9);
  game.step(600);
  const s = game.snapshot().entities.find((e) => e.id === tank.id)!;
  console.log(`tank ${tank.id} at tile ${s.tile.x},${s.tile.y} order=${s.order}`);
  assert.deepEqual(s.tile, { x: 20, y: 9 });
  assert.equal(s.order, "idle");
});
