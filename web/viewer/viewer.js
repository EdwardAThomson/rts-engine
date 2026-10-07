// Draws a Classic engine game in the browser from the WebAssembly build, with plain coloured shapes. It only reads
// the state (through crates/classic-wasm/src/view.rs) and gives orders through the same command queue a player
// would, so the game it shows is exactly the game the native build plays.
//
//   cargo build --release --target wasm32-unknown-unknown -p classic-wasm
//   python3 -m http.server 8000        # from the repository root
//   open http://localhost:8000/web/viewer/

const root = new URL("../../", import.meta.url);
const WASM_URL = new URL("target/wasm32-unknown-unknown/release/classic_wasm.wasm", root);
const TICKS_PER_SECOND = 15;
const TICK_MS = 1000 / TICKS_PER_SECOND;
const SUB = 256; // sub-tile units per tile
const FIELDS = 9; // numbers per entity from game_entities
const RESOURCE_FULL = 300; // a full resource tile, for shading
const ORDERS = ["idle", "move", "harvest"];
const TASKS = ["seek", "to field", "mining", "to refinery", "unloading", "stuck"];
const PLAYER_COLOURS = ["#4f8ef7", "#e5534b", "#57ab5a", "#c69026", "#a371f7", "#39c5cf", "#e0823d", "#d2a8ff"];
const TERRAIN_COLOURS = ["#b89a68", "#77706a", "#3a302a"]; // open ground, rock, cliff

const $ = (id) => document.getElementById(id);
const canvas = $("view");
const ctx = canvas.getContext("2d");

let api;
let game = 0;
let map = { w: 0, h: 0, terrain: [] };
let kinds = []; // { id, building, maxHealth, capacity }
let prev = new Map(); // entity id -> entity, one tick before `cur`
let cur = new Map();
let heading = new Map(); // entity id -> last direction of travel, in radians
let selected = null;
let playing = true;
let speed = 1;
let acc = 0;
let tilePx = 24;
let buf = { ptr: 0, cap: 0 }; // i32 buffer in wasm memory for game_entities

function fail(err) {
  $("error").textContent = String(err?.message ?? err);
  throw err;
}

async function load() {
  const res = await fetch(WASM_URL);
  if (!res.ok) {
    fail(`Could not load ${WASM_URL.pathname} (${res.status}). Build it first:\n` +
      "  cargo build --release --target wasm32-unknown-unknown -p classic-wasm\n" +
      "and serve the repository root, e.g. python3 -m http.server 8000");
  }
  const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {});
  api = instance.exports;
}

async function newGame() {
  const name = $("map").value;
  const res = await fetch(new URL(`maps/${name}`, root));
  if (!res.ok) fail(`Could not load maps/${name} (${res.status})`);
  const text = new TextEncoder().encode(await res.text());
  if (game) api.game_free(game);
  const ptr = api.alloc(text.length);
  new Uint8Array(api.memory.buffer, ptr, text.length).set(text);
  game = api.game_new(ptr, text.length, Number($("seed").value) | 0);
  if (!game) fail(`game_new refused maps/${name}`);

  map = { w: api.game_map_width(game), h: api.game_map_height(game), terrain: [] };
  for (let y = 0; y < map.h; y++) for (let x = 0; x < map.w; x++) map.terrain.push(api.game_terrain(game, x, y));
  kinds = [];
  const nameBuf = api.alloc(64);
  for (let k = 0; k < api.game_kind_count(game); k++) {
    const len = Math.min(api.game_kind_id(game, k, nameBuf, 64), 64);
    const id = new TextDecoder().decode(new Uint8Array(api.memory.buffer, nameBuf, len));
    kinds.push({ id, building: api.game_kind_building(game, k) === 1, maxHealth: api.game_kind_max_health(game, k),
      capacity: api.game_kind_capacity(game, k) });
  }
  api.dealloc(nameBuf, 64);

  selected = null;
  heading = new Map();
  acc = 0;
  cur = readEntities();
  prev = cur;
  fitCanvas();
}

function readEntities() {
  const need = api.game_entity_count(game) * FIELDS;
  if (need > buf.cap) {
    if (buf.ptr) api.dealloc(buf.ptr, buf.cap * 4);
    buf.cap = Math.max(need * 2, 64 * FIELDS);
    buf.ptr = api.alloc(buf.cap * 4);
    if (buf.ptr % 4) fail("alloc gave an unaligned buffer");
  }
  const n = api.game_entities(game, buf.ptr, buf.cap);
  const a = new Int32Array(api.memory.buffer, buf.ptr, n * FIELDS); // a fresh view: memory may have grown
  const out = new Map();
  for (let i = 0; i < n; i++) {
    const r = a.subarray(i * FIELDS, (i + 1) * FIELDS);
    out.set(r[0], { id: r[0], kind: r[1], owner: r[2], x: r[3], y: r[4], health: r[5], order: r[6], task: r[7], cargo: r[8] });
  }
  return out;
}

// Advance `n` ticks, keeping the state one tick before the end so movement can be drawn between the two.
function advance(n) {
  if (n <= 0) return;
  if (n > 1) api.game_step(game, n - 1);
  prev = n > 1 ? readEntities() : cur;
  api.game_step(game, 1);
  cur = readEntities();
  for (const [id, e] of cur) {
    const p = prev.get(id);
    if (p && (p.x !== e.x || p.y !== e.y)) heading.set(id, Math.atan2(e.y - p.y, e.x - p.x));
  }
}

function fitCanvas() {
  const room = Math.max(320, window.innerWidth - 32 - (window.innerWidth > 900 ? 380 : 0));
  tilePx = Math.max(10, Math.min(40, Math.floor(room / map.w)));
  canvas.width = map.w * tilePx;
  canvas.height = map.h * tilePx;
}

function drawMap() {
  const t = tilePx;
  for (let y = 0; y < map.h; y++) {
    for (let x = 0; x < map.w; x++) {
      const kind = map.terrain[y * map.w + x];
      ctx.fillStyle = TERRAIN_COLOURS[kind] ?? "#000";
      ctx.fillRect(x * t, y * t, t, t);
      if (kind === 2) { // cliff: hatch so it reads as impassable
        ctx.strokeStyle = "#251e19";
        ctx.beginPath();
        ctx.moveTo(x * t, y * t + t);
        ctx.lineTo(x * t + t, y * t);
        ctx.stroke();
      }
      const amount = api.game_resource(game, x, y);
      if (amount > 0) {
        const f = Math.min(1, amount / RESOURCE_FULL);
        ctx.fillStyle = `rgba(232, 160, 40, ${0.25 + 0.6 * f})`;
        const r = Math.max(2, (t / 2 - 2) * (0.4 + 0.6 * f));
        ctx.beginPath();
        ctx.arc(x * t + t / 2, y * t + t / 2, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }
  if ($("grid").checked) {
    ctx.strokeStyle = "rgba(0,0,0,0.15)";
    ctx.beginPath();
    for (let x = 0; x <= map.w; x++) { ctx.moveTo(x * t + 0.5, 0); ctx.lineTo(x * t + 0.5, map.h * t); }
    for (let y = 0; y <= map.h; y++) { ctx.moveTo(0, y * t + 0.5); ctx.lineTo(map.w * t, y * t + 0.5); }
    ctx.stroke();
  }
}

function drawEntities(alpha) {
  const t = tilePx;
  const toPx = (v) => (v / SUB) * t;
  // Buildings first, so units drive over them.
  const order = [...cur.values()].sort((a, b) => Number(kinds[b.kind]?.building) - Number(kinds[a.kind]?.building));
  for (const e of order) {
    const k = kinds[e.kind] ?? { id: "?", building: false, maxHealth: 1, capacity: 0 };
    const p = prev.get(e.id) ?? e;
    const cx = toPx(p.x + (e.x - p.x) * alpha);
    const cy = toPx(p.y + (e.y - p.y) * alpha);
    const colour = PLAYER_COLOURS[e.owner % PLAYER_COLOURS.length];
    ctx.lineWidth = Math.max(1, t / 16);
    ctx.strokeStyle = "#111";
    ctx.fillStyle = colour;
    if (k.building) {
      const s = t - 2;
      ctx.fillRect(cx - s / 2, cy - s / 2, s, s);
      ctx.strokeRect(cx - s / 2, cy - s / 2, s, s);
      ctx.fillStyle = "#111";
      ctx.font = `bold ${Math.floor(t * 0.5)}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(k.id[0].toUpperCase(), cx, cy + 1);
    } else if (e.cargo >= 0) { // anything that carries cargo: a box with a load bar
      const s = t * 0.7;
      ctx.save();
      ctx.translate(cx, cy);
      ctx.rotate(heading.get(e.id) ?? 0);
      ctx.fillRect(-s / 2, -s * 0.35, s, s * 0.7);
      ctx.strokeRect(-s / 2, -s * 0.35, s, s * 0.7);
      ctx.restore();
    } else { // other units: a disc with a barrel facing the way it last moved
      const r = t * 0.32;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      const a = heading.get(e.id) ?? 0;
      ctx.lineWidth = Math.max(2, t / 8);
      ctx.beginPath();
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx + Math.cos(a) * r * 1.5, cy + Math.sin(a) * r * 1.5);
      ctx.stroke();
    }
    if (e.cargo > 0 && k.capacity > 0) bar(cx, cy + t * 0.42, e.cargo / k.capacity, "#e8a028");
    if (e.health < k.maxHealth || selected === e.id) bar(cx, cy - t * 0.48, e.health / k.maxHealth, "#57ab5a");
    if (selected === e.id) {
      ctx.strokeStyle = "#fff";
      ctx.lineWidth = 1.5;
      ctx.strokeRect(cx - t / 2, cy - t / 2, t, t);
    }
  }
}

function bar(cx, y, f, colour) {
  const w = tilePx * 0.8;
  const h = Math.max(2, tilePx / 10);
  ctx.fillStyle = "#111";
  ctx.fillRect(cx - w / 2, y - h / 2, w, h);
  ctx.fillStyle = colour;
  ctx.fillRect(cx - w / 2, y - h / 2, w * Math.max(0, Math.min(1, f)), h);
}

function rows(el, list) {
  el.innerHTML = list.map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`).join("");
}

function drawPanel() {
  const tick = api.game_tick(game);
  const secs = Math.floor(tick / TICKS_PER_SECOND);
  let resource = 0;
  for (let y = 0; y < map.h; y++) for (let x = 0; x < map.w; x++) resource += api.game_resource(game, x, y);
  rows($("game"), [
    ["Tick", tick.toLocaleString()],
    ["Game time", `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`],
    ["State hash", (api.game_hash(game) >>> 0).toString(16).padStart(8, "0")],
    ["Resource left", resource.toLocaleString()],
    ["Entities", cur.size],
  ]);
  const players = [];
  for (let p = 0; p < api.game_player_count(game); p++) {
    const sw = `<span class="swatch" style="background:${PLAYER_COLOURS[p % PLAYER_COLOURS.length]}"></span>`;
    players.push([`${sw}Player ${p + 1}`, `${Number(api.game_credits(game, p)).toLocaleString()} credits`]);
  }
  rows($("players"), players);
  const e = cur.get(selected);
  if (!e) {
    selected = null;
    rows($("selected"), [["Nothing selected", ""]]);
    return;
  }
  const k = kinds[e.kind];
  rows($("selected"), [
    ["Id", e.id],
    ["Kind", k.id],
    ["Owner", `Player ${e.owner + 1}`],
    ["Tile", `${Math.floor(e.x / SUB)}, ${Math.floor(e.y / SUB)}`],
    ["Health", `${e.health} / ${k.maxHealth}`],
    ["Order", ORDERS[e.order] ?? e.order],
    ...(e.task >= 0 ? [["Task", TASKS[e.task] ?? e.task]] : []),
    ...(e.cargo >= 0 ? [["Cargo", e.cargo]] : []),
  ]);
}

let last = performance.now();
let panelAt = 0;
function frame(now) {
  const dt = Math.min(250, now - last); // a hidden tab doesn't fast-forward on return
  last = now;
  if (playing) {
    acc += dt * speed;
    const n = Math.floor(acc / TICK_MS);
    acc -= n * TICK_MS;
    advance(n);
  }
  drawMap();
  drawEntities(playing ? acc / TICK_MS : 1);
  if (now - panelAt > 200) {
    drawPanel();
    panelAt = now;
  }
  requestAnimationFrame(frame);
}

function tileAt(ev) {
  const r = canvas.getBoundingClientRect();
  const sx = canvas.width / r.width;
  return { x: (ev.clientX - r.left) * sx / tilePx, y: (ev.clientY - r.top) * sx / tilePx };
}

canvas.addEventListener("click", (ev) => {
  const at = tileAt(ev);
  let best = null;
  let bestD = 0.8 * 0.8;
  for (const e of cur.values()) {
    const dx = e.x / SUB - at.x;
    const dy = e.y / SUB - at.y;
    const d = dx * dx + dy * dy - (kinds[e.kind]?.building ? 0.1 : 0); // prefer a unit over the building it's on
    if (d < bestD) { bestD = d; best = e.id; }
  }
  selected = best;
  drawPanel();
});

canvas.addEventListener("contextmenu", (ev) => {
  ev.preventDefault();
  const e = cur.get(selected);
  if (!e || kinds[e.kind]?.building) return;
  const at = tileAt(ev);
  api.game_order_move(game, e.owner, e.id, Math.floor(at.x), Math.floor(at.y));
});

$("play").addEventListener("click", () => {
  playing = !playing;
  $("play").textContent = playing ? "Pause" : "Play";
});
$("stepOne").addEventListener("click", () => {
  playing = false;
  $("play").textContent = "Play";
  advance(1);
  drawPanel();
});
$("speed").addEventListener("change", () => { speed = Number($("speed").value); });
$("restart").addEventListener("click", () => newGame());
$("map").addEventListener("change", () => newGame());
window.addEventListener("resize", fitCanvas);
window.addEventListener("keydown", (ev) => {
  if (ev.target.tagName === "INPUT") return;
  if (ev.key === " ") { ev.preventDefault(); $("play").click(); }
  if (ev.key === ".") $("stepOne").click();
});

await load();
await newGame();
// For tests and the console: the live module and game handle.
window.viewer = { get api() { return api; }, get game() { return game; } };
requestAnimationFrame(frame);
