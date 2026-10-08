# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games, in Rust. The simulation runs headless, deterministic
and testable; it builds natively for desktop and to WebAssembly for the browser, and both builds play exactly
the same games. Every world it plays (names, factions, art, audio, campaign, UI theme) comes from a data-only
**setting pack**; today packs supply names, factions and tuning.

```bash
cargo test                                                    # 118 checks, about 1 s after the first build
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500     # a 10-minute game in about 2 ms
cargo run --release --bin cli -- --setting private                       # the same, with the first pack in settings-private/
cargo run --release --bin cli -- --map maps/skirmish-01.txt --ai 0,1 --ticks 40000   # two computer opponents play it out
cargo run --release --bin bench                               # performance on a 128 x 128 map, up to 500 units
cargo run --release --bin play                                # play against the computer: build from the rail, drag to select, right-click to order, M mutes (--ai none: alone)
cargo run --bin sounds                                        # rewrite the generic pack's placeholder sounds from their recipes
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs
python3 -m http.server 8000      # then open http://localhost:8000/web/viewer/ to watch a game in the browser
```

The same player runs in the browser, drawing with WebGPU, or WebGL2 where the browser has no WebGPU. Build it with
[wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen)'s command-line tool, at the version `Cargo.lock` gives
the `wasm-bindgen` library (`cargo install wasm-bindgen-cli --version <that version>`), then serve the repository
root:

```bash
cargo build --release --target wasm32-unknown-unknown -p classic-render --bin play
wasm-bindgen --target web --no-typescript --out-dir web/play/pkg target/wasm32-unknown-unknown/release/play.wasm
python3 -m http.server 8000      # then open http://localhost:8000/web/play/ (options: ?setting=generic&seed=3)
node web/play/check.mjs          # checks it in headless Chromium, on WebGPU and WebGL2 (needs Playwright)
```

## What's in it

| Path | Does |
|---|---|
| `rts-core` (separate repository) | Shared with the 3D engine: `isqrt`, the seeded xorshift generator, the canonical state hash, the command queue and log. Pinned by commit in `Cargo.toml`. |
| `crates/classic-sim/src/map.rs` | ASCII maps: open ground, rock, cliffs, resource fields, start positions. 256 sub-tile units per tile. |
| `crates/classic-sim/src/path.rs` | A* on the grid, 8-way, integer costs and a fixed tie-break; connected regions refuse unreachable goals at once. |
| `data/rules/` | Every generic id the engine knows, the switchable modules, and each built entity's numbers with their allowed ranges. Our own placeholder numbers, to be tuned later by AI-vs-AI runs. |
| `crates/classic-data` | A small JSON reader (whole numbers only), the rules table and tuning, and the setting pack loader with its checks. |
| `crates/classic-sim/src/units.rs` | Kinds and the typed rules the tick reads: every built unit and building in the rules data is a kind, and its roles say which mechanics it joins. |
| `crates/classic-sim/src/power.rs` | Each player's power supply and demand, worked out from the buildings standing; producers give power in proportion to their health. |
| `crates/classic-sim/src/placement.rs` | Where a building may go: in bounds, firm empty ground, no resource, nothing in the way, near its owner's base. Buildings block ground movement; units already moving path round a new one. |
| `crates/classic-sim/src/world.rs` | The game state and the fixed tick: commands, movement, the harvester loop (find field, mine, return, unload into credits), resource regrowth. |
| `crates/classic-sim/src/game.rs` | The game API: `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. |
| `crates/classic-ai` | The computer opponent: a player without a mouse that reads the game and issues the same commands a player does. Builds a base from a build order of generic ids (power first when short), places each building with the placement check while keeping factory exits and refinery docks clear, fills its refineries with harvesters, makes tanks, gathers them at a rally point, defends its base and harvesters, and sends attack waves that grow each time. One "normal" opponent so far. |
| `crates/classic-tools` | The headless CLI, the bench, and the seeded bench scene they and the golden tests share. |
| `crates/classic-render` | The wgpu renderer and the player, on the desktop and in the browser: the pack's art in faction colours, the map, buildings, units, shells and explosions, selection and orders, and computer opponents for every other player. `hud` is the production rail on the right (credits and power readout, a tab per factory kind, build grid, queue, minimap) and placing buildings. `platform/` is the genre-neutral part (GPU, textures, sprite batcher, pixel font, sound mixer and device, WAV files, clock, files, the browser page). `sound.rs` turns the game's events into sounds, by the rules in `data/audio/`; `web.rs` fetches a game's files in the browser. |
| `crates/classic-wasm` | The WebAssembly build's interface; `web/check.mjs` runs it in Node. `view.rs` holds the read-only functions the viewer draws from. |
| `web/play/` | The page for the browser build of the player: a full-window canvas. `check.mjs` opens it in headless Chromium on WebGPU and on WebGL2. |
| `web/viewer/` | A browser page that plays a game from the WebAssembly build and draws it with coloured shapes: terrain, resource fields, buildings, harvesters and tanks moving between ticks. Play, pause, step, speed, seed; click a unit to inspect it, right-click to move it. No dependencies or build step. |
| `maps/test-01.txt` | Two players, six resource fields, a cliff ridge. The tests' map. |
| `maps/skirmish-01.txt` | 64 x 40, two large rock plateaus with room to build, near, far and contested resource fields, outcrops and cliff ridges. The desktop player's map. |
| `settings/generic/` | The public setting pack: plain names for every id, two factions. Generic placeholder art will live here too. |

How the engine works, and how setting packs keep the code generic, is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
The rules for working in this repository are in [CLAUDE.md](CLAUDE.md).

## Verified

- The Rust port plays the same games as the TypeScript engine it replaced: identical state hashes at eight
  points in four seeds' games, for scripted orders (including the old replay hash `e0342eba`), and every 150 ticks
  of two 500-unit bench games; identical paths, node counts and path checksums. Hashes were re-recorded on purpose
  when the tank's id became `battle_tank`, when buildings began to block movement, and when the tick split into
  phases with a 3x2 refinery, and when players began with a yard, a power plant and 1,200 credits (the replay hash
  is now `f3cc6748`); each time standalone paths and path checksums
  were checked unchanged; the credit totals that moved as harvesters use the new dock were recorded on purpose.
- The WebAssembly build gives the same hashes as the native build.
- Two runs with the same seed and orders give the same state after 10,000 ticks; a different seed gives a
  different game; replaying seed, map and command log reproduces the live game.
- Both harvesters deliver within 60 s of game time; for 5,000 ticks resource is conserved, credits are never
  negative and entity ids stay sorted.
- Paths never enter a cliff and go round the ridge; a cliff tile is refused as a goal; a sealed-off goal is
  refused without searching.
- Orders for another player's units or for buildings are ignored.
- Each player starts with a construction yard, a power plant, a 3x2 refinery with its harvester at the dock under
  its middle column, a tank and 1,200 credits; the base has a power margin of 70. Placement accepts a building on rock touching
  its owner's base (diagonals count, walls don't) and refuses each broken rule with its reason, including a
  refinery whose dock is a cliff; tuning can allow open ground or let a base reach further; a tank already moving drives round a building placed on
  its path; a bigger refinery moves its dock and harvesters still deliver; placements replay from the command log.
- Power adds up from the buildings standing, a damaged plant gives less, the power factor stops at its 25%
  floor, `power_changed` reports supply, demand and shortfall only when they change, and a pack's tuning changes
  the numbers and the floor.
- Production: each factory queues up to five items and builds the head one, paying as it goes (a 600-credit,
  450-tick tank costs 300 by tick 225 and exactly 600 at the end); a power shortfall slows it to the power factor;
  it pauses without losing progress when credits run out and resumes by itself; cancelling refunds exactly what
  was paid; a finished building waits at the yard until placed, and only a ready building can be placed; a
  finished unit leaves by the exit tile or waits until one frees up; prerequisites, the primary factory and replay
  from the command log all hold.
- Combat: tanks in sight pick each other, turn their turrets the short way and trade shells; a full shell hit on
  heavy armour does exactly its damage; a destroyed unit is removed, credits its killer, and its death blast hurts
  nearby enemies twice as much as its own side; two units can kill each other on the same tick; an attack order
  closes to within range and stands; a moving unit ignores enemies; a rocket turret needs power and a gun turret
  doesn't; a destroyed building frees its tiles; guards prefer armed units to buildings; a pack can tune weapons
  and the damage table; combat replays from the command log.
- Collision: a tile holds one ground unit, checked on every tick of every collision test; a unit waits behind
  another instead of driving through it; a move to a taken tile ends next to it; nine tanks sent to one tile end
  on nine tiles round it; an idle own tank steps aside, and an enemy one never does, so the mover gives up; two
  tanks meeting head-on in a corridor get past each other; twelve tanks squeeze through a one-tile gap with none
  stuck; an own tank on a refinery dock gives way to the harvester while an enemy one blocks it; a harvester
  queued for a dock steps aside for the one leaving it, and both keep delivering; a factory's new
  units drive clear of its exit; collision replays from the command log.
- Rendering (with Mesa's software GPU, no window): the start base is drawn in its faction's colours with no remap
  colour left on screen and the map covering the frame; the other faction's tank is in its own colours; the same
  frame twice gives the same pixels and drawing never changes the game's hash; shots and explosions appear from
  events; recolouring swaps exact remap pixels only. The desktop player opens, selects and orders under Xvfb, and its computer opponent issues
  orders there.
- The production rail, driven by clicks: it offers what the player's factories can build now plus the next tier,
  locked; shift-click queues five and right-clicks cancel them with every credit refunded; a ready building's
  ghost agrees with the simulation's placement check on every tile on screen, a bad spot orders nothing, a good
  one places it; clicks on the rail never reach the world; the HUD draws only in its own place, and a glyph
  lights exactly its own pixels. The minimap keeps the map's shape, maps its corners and centre to the map's,
  shows the player's base in their colour, and its clicks move the view or ask for an order without ordering
  anything itself.
- Sound (no sound card; the mixer renders into a buffer): every game event plays a sound or is listed as silent on
  purpose; every sound id has a generic file; a tank battle plays cannon, impact and explosion sounds and ends
  with the same state hash as the same game unheard; 40 tanks fighting never exceed the 24-voice and
  three-cannons caps; a fight off screen is panned towards its side and one far away is silent; only the local
  player hears their own deliveries; the generic sounds match their recipes byte for byte and each has a
  provenance line.
- The browser build of the player (headless Chromium, software GPU): it draws the map with WebGPU and, with
  WebGPU switched off, with WebGL2; the game ticks and Space pauses it. Art loaded from fetched files draws the same
  frame as art read from its folder, and a pack given as files loads as it does from its folder.
- `isqrt` equals `floor(sqrt(n))` on squares, their neighbours and a sweep; the hash streams exactly the
  canonical text; clippy bans floating point, the clock and unordered collections in the simulation crates.

- Computer opponent (`crates/classic-ai/tests/skirmish.rs`, on `skirmish-01`): within 7,000 ticks it has a power
  plant, two refineries, both factories, a radar, turrets, four harvesters and tanks, with no command refused; it
  destroys every building of a player who does nothing, from either start, and never before its first-wave time;
  two AIs play the same 20,000-tick game twice to the same hash and command log; a game between two AIs replays
  from its command log with the AI switched off to the same hash; thinking never changes the game's hash; a power
  plant removed at tick 3,000 is rebuilt within 90 seconds; an enemy tank beside its base draws an attack order
  and takes hits within 20 seconds.

## Not verified / not built yet

- Collision covers vehicles only: no infantry positions, crushing, air units, group formations, keep-clear tiles or
  bodies that turn before driving yet, and a blocked search returns no partial path.
- Combat has no crushing, infantry, aircraft or special weapons, and guards don't chase or return yet; sight is
  a stand-in until vision exists, and the weapon numbers are first guesses.
- The computer opponent is one "normal" level with numbers in code: no difficulty levels, personalities, data files
  in `data/ai/`, scouting (there is no fog yet, so it sees the whole map, as every player does), retreat by
  exchange, counter-composition, target scoring, slabs, superpowers or remnant mode. Its memory lives in the `Ai`
  value, not the hashed game state, so a save would not carry it yet. Two AIs on `skirmish-01` often play to a
  stalemate behind their turrets. In the desktop player it was checked only in a short smoke run, not played by a
  person.
- No storage cap, tech levels, factory upgrades or starport.
- The renderer has no selection card, messages or menus yet; the rail has no tabs by category, hotkeys, pause per
  item or primary factory choice yet; cliffs are plain dark tiles, and the art is the generic pack's placeholders.
  The player was checked under a virtual display
  with a software GPU, not on a real desktop GPU.
- Sound is effects and interface sounds only: no music, unit replies, advisor announcements, looping sounds or
  volume sliders yet. In the browser, sound starts only after the first click or key press (browsers' rule), and
  it has not been heard there. The placeholder sounds were checked by tests and numbers
  (length, peak, never clipping), not yet by ear, and the player has not been run with a real sound card.
- The browser build was checked only in headless Chromium on its software GPU, not in Firefox or Safari, on a
  phone, or on a real GPU. Headless Chromium never shows a WebGPU canvas, so the check reads that frame back from
  the GPU instead of taking a screenshot. A pack in the browser is checked over the files fetched (its data files,
  the art `art.json` names and the sounds `audio/sounds.json` names), not every file in its folder, and the page has
  no touch controls.
- The web viewer (`web/viewer/`) is the debug view: plain shapes on a 2D canvas, checked in headless Chromium.
- Bench numbers are from one 4-vCPU cloud VM, not a desktop or a browser.
