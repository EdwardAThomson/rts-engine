// Runs the WebAssembly build of the simulation in Node and checks it plays exactly the same games as the native
// build: the same state hashes the Rust tests (and the earlier TypeScript engine) give.
//
//   cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs

import { readFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const wasm = readFileSync(new URL("target/wasm32-unknown-unknown/release/classic_wasm.wasm", root));
const { instance } = await WebAssembly.instantiate(wasm, {});
const api = instance.exports;
const mapText = new TextEncoder().encode(readFileSync(new URL("maps/test-01.txt", root), "utf8"));

function newGame(seed) {
  const ptr = api.alloc(mapText.length);
  new Uint8Array(api.memory.buffer, ptr, mapText.length).set(mapText);
  const game = api.game_new(ptr, mapText.length, seed);
  if (!game) throw new Error("game_new refused the map");
  return game;
}
// WebAssembly returns a u32 as a signed number, so convert it back first.
const hex = (game) => (api.game_hash(game) >>> 0).toString(16).padStart(8, "0");

const checks = [];
function check(name, got, want) {
  checks.push({ name, got, want, ok: got === want });
}

// No orders: the harvesters and regrowth alone (seed 1, 9,000 ticks), as the command-line runner plays it.
{
  const g = newGame(1);
  api.game_step(g, 9000);
  check("seed 1, 9,000 ticks", hex(g), "182e2ffd");
  check("player 1 credits", Number(api.game_credits(g, 0)), 7400);
  api.game_free(g);
}
// The scripted orders from the tests (seed 3, 6,000 ticks): the replay hash every earlier change was checked by.
{
  const g = newGame(3);
  const tanks = [[5, 0], [10, 1]];
  for (let t = 0; t < 6000; t += 500) {
    tanks.forEach(([id, owner], k) => api.game_order_move(g, owner, id, (t / 50 + 7 * k) % 30, (t / 100 + 3 * k) % 18));
    api.game_step(g, 500);
  }
  check("seed 3 scripted, 6,000 ticks", hex(g), "3e9c48fc");
  check("tick count", api.game_tick(g), 6000);
  api.game_free(g);
}

// The viewer's read-only functions report what the native game holds: the map, the kinds and every entity.
{
  const g = newGame(1);
  const hashBefore = hex(g);
  check("map size", `${api.game_map_width(g)}x${api.game_map_height(g)}`, "32x20");
  check("start tile is rock", api.game_terrain(g, 3, 2), 1);
  check("off the map", api.game_terrain(g, -1, 0), -1);
  const names = api.alloc(64);
  const kinds = [];
  for (let k = 0; k < api.game_kind_count(g); k++) {
    const len = api.game_kind_id(g, k, names, 64);
    kinds.push(new TextDecoder().decode(new Uint8Array(api.memory.buffer, names, len)));
  }
  api.dealloc(names, 64);
  // Every built unit and building in the rules data, in generic-id order, with its footprint.
  const rules = JSON.parse(readFileSync(new URL("data/rules/entities.json", root), "utf8")).entities;
  const built = Object.keys(rules).filter((id) => rules[id].status === "built" && ["unit", "building"].includes(rules[id].kind)).sort();
  check("kinds", kinds.join(","), built.join(","));
  const footprint = (id) => `${api.game_kind_width(g, kinds.indexOf(id))}x${api.game_kind_height(g, kinds.indexOf(id))}`;
  const n = (id, name) => rules[id].numbers[name].default;
  check("refinery footprint", footprint("refinery"), `${n("refinery", "width")}x${n("refinery", "height")}`);
  check("unit footprint", footprint("battle_tank"), "1x1");
  const fields = 9;
  const buf = api.alloc(4 * 64 * fields);
  const count = api.game_entities(g, buf, 64 * fields);
  const e = new Int32Array(api.memory.buffer, buf, count * fields);
  check("entities", count, api.game_entity_count(g));
  // Entity 1 is player 1's first building, its top-left tile on the start tile (3, 2), at that tile's centre.
  check("entity 1", [...e.subarray(0, 5)].map((v, i) => (i === 1 ? kinds[v] && api.game_kind_building(g, v) : v)).join(","),
    `1,1,0,${3 * 256 + 128},${2 * 256 + 128}`);
  check("reading changes nothing", hex(g), hashBefore);
  api.dealloc(buf, 4 * 64 * fields);
  api.game_free(g);
}

for (const c of checks) console.log(`${c.ok ? "ok  " : "FAIL"} ${c.name}: ${c.got}${c.ok ? "" : ` (want ${c.want})`}`);
console.log(`wasm module ${(wasm.length / 1024).toFixed(0)} KB`);
if (checks.some((c) => !c.ok)) process.exit(1);
