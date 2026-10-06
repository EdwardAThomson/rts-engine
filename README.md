# rts-engine

A generic real-time strategy engine in TypeScript. The simulation runs headless, deterministic and testable, and
every world it plays (names, factions, art, audio, campaign, UI theme) will come from a data-only **setting pack**.
TypeScript is run directly by Node 22; there is no build step.

```bash
npm ci                    # only for type checking (TypeScript and Node's types)
npm test                  # 11 checks, under 1 s
npm run typecheck
node tools/cli.ts --seed 1 --ticks 9000 --every 1500     # a 10-minute game in about 30 ms
```

## What's in it

| File | Does |
|---|---|
| `src/sim/map.ts` | ASCII maps: open ground, rock, cliffs, resource fields, start positions. 256 sub-tile units per tile. |
| `src/sim/path.ts` | A* on the grid, 8-way, integer costs and a fixed tie-break, so paths are identical every run. |
| `src/sim/rng.ts` | A seeded xorshift generator whose whole state is one integer in the game state. |
| `src/sim/imath.ts` | Integer maths (`isqrt`), so nothing in the sim uses floating-point functions. |
| `src/sim/units.ts` | Unit stats. Our own placeholder numbers, to be tuned later by AI-vs-AI runs. |
| `src/sim/world.ts` | The game state and the fixed tick: commands, movement, the harvester loop (find field, mine, return, unload into credits), resource regrowth. |
| `src/sim/game.ts` | The agent debug API: `step`, `order`, `spawn`, `snapshot`, `hash`, `commandLog`; and `hashState`. |
| `tools/cli.ts` | Headless run printing JSON lines. |
| `maps/test-01.txt` | Two players, six resource fields, a cliff ridge. |

How the engine works, and how setting packs keep the code generic, is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
The rules for working in this repository are in [CLAUDE.md](CLAUDE.md).

## Verified

- Two runs with the same seed and orders give the same state hash after 10,000 ticks.
- A different seed gives a different game (regrowth lands on different tiles).
- Replaying seed, map and command log reproduces the live game's hash.
- Both harvesters deliver within 60 s of game time.
- Every tick for 5,000 ticks: resource is conserved, credits are never negative, entity ids stay sorted.
- Paths never enter a cliff and go round the ridge; a cliff tile is refused as a goal.
- A tank sent to a tile arrives and goes idle.
- `isqrt` equals `floor(sqrt(n))` on squares, their neighbours and a sweep of 0 to 200,000.
- The state hash ignores key order, changes with any value and refuses fractional numbers.
- `tsc` passes in strict mode.

## Not verified / not built yet

- Units pass through each other; there's no collision or tile reservation yet.
- No buildings beyond a 1×1 refinery, no building placement, power, combat or AI.
- No setting pack loader yet; names in code are the generic ids.
- No renderer or browser build of the debug API yet.
- No determinism lint yet (planned in the engineering design doc); the rules are enforced by review and tests.
- Pathfinding cost on large maps isn't measured (the test map is 32×20).
