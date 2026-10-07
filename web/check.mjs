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
  check("seed 1, 9,000 ticks", hex(g), "ecdb8b5b");
  check("player 1 credits", Number(api.game_credits(g, 0)), 4800);
  api.game_free(g);
}
// The scripted orders from the tests (seed 3, 6,000 ticks): the replay hash every earlier change was checked by.
{
  const g = newGame(3);
  const tanks = [[3, 0], [6, 1]];
  for (let t = 0; t < 6000; t += 500) {
    tanks.forEach(([id, owner], k) => api.game_order_move(g, owner, id, (t / 50 + 7 * k) % 30, (t / 100 + 3 * k) % 18));
    api.game_step(g, 500);
  }
  check("seed 3 scripted, 6,000 ticks", hex(g), "1bcfbaae");
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
  check("kinds", kinds.join(","), "battle_tank,harvester,refinery");
  const fields = 9;
  const buf = api.alloc(4 * 64 * fields);
  const n = api.game_entities(g, buf, 64 * fields);
  const e = new Int32Array(api.memory.buffer, buf, n * fields);
  check("entities", n, api.game_entity_count(g));
  // Player 1's refinery is entity 1, on its start tile (3, 2), centred in sub-tile units.
  check("refinery 1", [...e.subarray(0, 5)].join(","), `1,${kinds.indexOf("refinery")},0,${3 * 256 + 128},${2 * 256 + 128}`);
  check("reading changes nothing", hex(g), hashBefore);
  api.dealloc(buf, 4 * 64 * fields);
  api.game_free(g);
}

for (const c of checks) console.log(`${c.ok ? "ok  " : "FAIL"} ${c.name}: ${c.got}${c.ok ? "" : ` (want ${c.want})`}`);
console.log(`wasm module ${(wasm.length / 1024).toFixed(0)} KB`);
if (checks.some((c) => !c.ok)) process.exit(1);
