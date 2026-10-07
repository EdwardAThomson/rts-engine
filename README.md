# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games, in Rust. The simulation runs headless, deterministic
and testable; it builds natively for desktop and to WebAssembly for the browser, and both builds play exactly
the same games. Every world it plays (names, factions, art, audio, campaign, UI theme) comes from a data-only
**setting pack**; today packs supply names, factions and tuning.

```bash
cargo test                                                    # 85 checks, about 1 s after the first build
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500     # a 10-minute game in about 2 ms
cargo run --release --bin cli -- --setting private                       # the same, with the first pack in settings-private/
cargo run --release --bin bench                               # performance on a 128 x 128 map, up to 500 units
cargo run --release --bin play                                # play on the desktop: build from the rail, drag to select, right-click to order
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs
python3 -m http.server 8000      # then open http://localhost:8000/web/viewer/ to watch a game in the browser
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
| `crates/classic-tools` | The headless CLI, the bench, and the seeded bench scene they and the golden tests share. |
| `crates/classic-render` | The wgpu renderer and the desktop player: the pack's art in faction colours, the map, buildings, units, shells and explosions, selection and orders. `hud` is the production rail (a tab per factory kind, build grid, queue), the credits and power readout, and placing buildings. `platform/` is the genre-neutral part (GPU, textures, sprite batcher, pixel font). |
| `crates/classic-wasm` | The WebAssembly build's interface; `web/check.mjs` runs it in Node. `view.rs` holds the read-only functions the viewer draws from. |
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
  stuck; an own tank on a refinery dock gives way to the harvester while an enemy one blocks it; a factory's new
  units drive clear of its exit; collision replays from the command log.
- Rendering (with Mesa's software GPU, no window): the start base is drawn in its faction's colours with no remap
  colour left on screen and the map covering the frame; the other faction's tank is in its own colours; the same
  frame twice gives the same pixels and drawing never changes the game's hash; shots and explosions appear from
  events; recolouring swaps exact remap pixels only. The desktop player opens, selects and orders under Xvfb.
- The production rail, driven by clicks: it offers what the player's factories can build now plus the next tier,
  locked; shift-click queues five and right-clicks cancel them with every credit refunded; a ready building's
  ghost agrees with the simulation's placement check on every tile on screen, a bad spot orders nothing, a good
  one places it; clicks on the rail never reach the world; the HUD draws only in its own place, and a glyph
  lights exactly its own pixels.
- `isqrt` equals `floor(sqrt(n))` on squares, their neighbours and a sweep; the hash streams exactly the
  canonical text; clippy bans floating point, the clock and unordered collections in the simulation crates.

## Not verified / not built yet

- Collision covers vehicles only: no infantry positions, crushing, air units, group formations, keep-clear tiles or
  bodies that turn before driving yet, and a blocked search returns no partial path.
- Combat has no crushing, infantry, aircraft or special weapons, and guards don't chase or return yet; sight is
  a stand-in until vision exists, and the weapon numbers are first guesses.
- No AI; no storage cap, tech levels, factory upgrades or starport.
- The renderer is desktop only so far, with no minimap, selection card, messages, menus, audio or computer
  opponent; the rail has no tabs by category, hotkeys, pause per item or primary factory choice yet; cliffs are
  plain dark tiles, and the art is the generic pack's placeholders. The player was checked under a virtual display
  with a software GPU, not on a real desktop GPU.
- The web viewer (`web/viewer/`) is the debug view: plain shapes on a 2D canvas, checked in headless Chromium.
- Bench numbers are from one 4-vCPU cloud VM, not a desktop or a browser.
