# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games, in Rust. The simulation runs headless, deterministic
and testable; it builds natively for desktop and to WebAssembly for the browser, and both builds play exactly
the same games. Every world it plays (names, factions, art, audio, campaign, UI theme) comes from a data-only
**setting pack**; today packs supply names, factions and tuning.

```bash
cargo test                                                    # 34 checks, about 1 s after the first build
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500     # a 10-minute game in about 2 ms
cargo run --release --bin cli -- --setting private                       # the same, with settings-private/'s pack
cargo run --release --bin bench                               # performance on a 128 x 128 map, up to 500 units
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs
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
| `crates/classic-sim/src/placement.rs` | Where a building may go: in bounds, firm empty ground, no resource, nothing in the way, near its owner's base. Buildings block ground movement; units already moving path round a new one. |
| `crates/classic-sim/src/world.rs` | The game state and the fixed tick: commands, movement, the harvester loop (find field, mine, return, unload into credits), resource regrowth. |
| `crates/classic-sim/src/game.rs` | The game API: `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. |
| `crates/classic-tools` | The headless CLI, the bench, and the seeded bench scene they and the golden tests share. |
| `crates/classic-wasm` | The WebAssembly build's interface; `web/check.mjs` runs it in Node. |
| `maps/test-01.txt` | Two players, six resource fields, a cliff ridge. |
| `settings/generic/` | The public setting pack: plain names for every id, two factions. Generic placeholder art will live here too. |

How the engine works, and how setting packs keep the code generic, is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
The rules for working in this repository are in [CLAUDE.md](CLAUDE.md).

## Verified

- The Rust port plays the same games as the TypeScript engine it replaced: identical state hashes at eight
  points in four seeds' games, for scripted orders (including the old replay hash `e0342eba`), and every 150 ticks
  of two 500-unit bench games; identical paths, node counts and path checksums. Hashes were re-recorded on purpose
  when the tank's id became `battle_tank`, and again when buildings began to block movement (the replay hash is
  now `06d3ddde`); each time standalone paths, credits and positions were checked unchanged.
- The WebAssembly build gives the same hashes as the native build.
- Two runs with the same seed and orders give the same state after 10,000 ticks; a different seed gives a
  different game; replaying seed, map and command log reproduces the live game.
- Both harvesters deliver within 60 s of game time; for 5,000 ticks resource is conserved, credits are never
  negative and entity ids stay sorted.
- Paths never enter a cliff and go round the ridge; a cliff tile is refused as a goal; a sealed-off goal is
  refused without searching.
- Orders for another player's units or for buildings are ignored.
- Placement accepts a building beside its owner's base and refuses each broken rule with its reason; tuning can
  make buildings rock-only or let a base reach further; a tank already moving drives round a building placed on
  its path; a bigger refinery moves its dock and harvesters still deliver; placements replay from the command log.
- `isqrt` equals `floor(sqrt(n))` on squares, their neighbours and a sweep; the hash streams exactly the
  canonical text; clippy bans floating point, the clock and unordered collections in the simulation crates.

## Not verified / not built yet

- Units pass through each other; there's no collision or tile reservation yet.
- No buildings beyond a 1×1 refinery, no building placement, power, combat or AI.
- No setting pack loader yet; names in code are the generic ids.
- No renderer, UI or audio yet, on desktop or web.
- Bench numbers are from one 4-vCPU cloud VM, not a desktop or a browser.
