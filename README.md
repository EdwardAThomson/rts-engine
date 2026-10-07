# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games, in Rust. The simulation runs headless, deterministic
and testable; it builds natively for desktop and to WebAssembly for the browser, and both builds play exactly
the same games. Every world it plays (names, factions, art, audio, campaign, UI theme) will come from a data-only
**setting pack**.

```bash
cargo test                                                    # 20 checks, about 1 s after the first build
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500     # a 10-minute game in about 2 ms
cargo run --release --bin bench                               # performance on a 128 x 128 map, up to 500 units
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs
```

## What's in it

| Path | Does |
|---|---|
| `crates/engine-core` | Shared with any future sibling engine: `isqrt`, the seeded xorshift generator, the canonical state hash, the command queue and log. |
| `crates/classic-sim/src/map.rs` | ASCII maps: open ground, rock, cliffs, resource fields, start positions. 256 sub-tile units per tile. |
| `crates/classic-sim/src/path.rs` | A* on the grid, 8-way, integer costs and a fixed tie-break; connected regions refuse unreachable goals at once. |
| `crates/classic-sim/src/units.rs` | Unit stats. Our own placeholder numbers, to be tuned later by AI-vs-AI runs. |
| `crates/classic-sim/src/world.rs` | The game state and the fixed tick: commands, movement, the harvester loop (find field, mine, return, unload into credits), resource regrowth. |
| `crates/classic-sim/src/game.rs` | The game API: `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. |
| `crates/classic-tools` | The headless CLI, the bench, and the seeded bench scene they and the golden tests share. |
| `crates/classic-wasm` | The WebAssembly build's interface; `web/check.mjs` runs it in Node. |
| `maps/test-01.txt` | Two players, six resource fields, a cliff ridge. |

How the engine works, and how setting packs keep the code generic, is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
The rules for working in this repository are in [CLAUDE.md](CLAUDE.md).

## Verified

- The Rust port plays the same games as the TypeScript engine it replaced: identical state hashes at eight
  points in four seeds' games, for scripted orders (including the old replay hash `e0342eba`), and every 150 ticks
  of two 500-unit bench games; identical paths, node counts and path checksums.
- The WebAssembly build gives the same hashes as the native build.
- Two runs with the same seed and orders give the same state after 10,000 ticks; a different seed gives a
  different game; replaying seed, map and command log reproduces the live game.
- Both harvesters deliver within 60 s of game time; for 5,000 ticks resource is conserved, credits are never
  negative and entity ids stay sorted.
- Paths never enter a cliff and go round the ridge; a cliff tile is refused as a goal; a sealed-off goal is
  refused without searching.
- Orders for another player's units or for buildings are ignored.
- `isqrt` equals `floor(sqrt(n))` on squares, their neighbours and a sweep; the hash streams exactly the
  canonical text; clippy bans floating point, the clock and unordered collections in the simulation crates.

## Not verified / not built yet

- Units pass through each other; there's no collision or tile reservation yet.
- No buildings beyond a 1×1 refinery, no building placement, power, combat or AI.
- No setting pack loader yet; names in code are the generic ids.
- No renderer, UI or audio yet, on desktop or web.
- Bench numbers are from one 4-vCPU cloud VM, not a desktop or a browser.
