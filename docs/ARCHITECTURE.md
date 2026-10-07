# How the engine works

This is the map of the engine: what each part does, how a game runs, and how the code stays generic while every
world (names, factions, art, sound, story) comes from data files. "Built" marks what exists today; everything else
is the plan, from the design docs in the project's playbooks repository (`plans/rts/`).

## The one idea

**The engine knows mechanics. A setting pack knows the story.**

The code talks only in generic ids: `power_plant`, `harvester`, `battle_tank`, `hazard`, `resource`, `faction_a`.
It knows that a `harvester` mines `resource` and unloads at a `refinery`, and that a `hazard` roams open ground
and swallows units. It never knows what they are called or what they look like.

A **setting pack** is a folder of data files and assets that turns those ids into a world: "this `hazard` is
called X, looks like this sprite sheet, makes this sound, and the campaign briefing says this". Swapping the pack
swaps the world. No code changes.

```
               ┌──────────────────────── engine (this repo) ────────────────────────┐
               │                                                                     │
 data/rules ──►│  sim (crates)   ──events──►  renderer, audio, UI  ──►  screen/speakers │
 (generic ids, │    ▲   pure, deterministic,      read-only observers                 │
  default      │    │   integer maths                ▲                                │
  numbers)     │    │                                │                                │
               │  commands ◄── player input, AI      │                                │
               └────┼────────────────────────────────┼────────────────────────────────┘
                    │ tuning (numbers)               │ names, art, audio, text, theme, maps, campaign
               ┌────┴────────────────────────────────┴───────┐
               │        setting pack (data only, no code)     │
               │  settings/generic/   or   settings-private/  │
               └──────────────────────────────────────────────┘
```

## Scope: classic RTS games

This is the **Classic RTS Engine**: one engine for the family of base-building real-time strategy games
that took shape in the 1990s. Their shared core is a flat tile grid seen from above, a harvest economy that turns
a resource into credits, bases built next to your own buildings, factories with build queues, power, fog of war,
and pre-rendered 2D sprites. The first setting pack is a private one, but the engine admits the
mechanics the rest of that family added, such as naval units, engineers that capture buildings, regrowing
resource fields, freely placed walls, a tabbed build sidebar, more factions and superweapons. We provide no
data for those games; the engine just doesn't rule their mechanics out.

**Out of scope:** games built on a different core, such as true 3D terrain with heights and line of sight,
ballistic projectiles, continuous metal-and-energy economies, or armies of thousands in a free 3D camera. Those would replace the simulation, renderer and art pipeline, so they belong in a
sibling engine. That sibling could reuse the parts here that aren't tied to the genre: deterministic stepping,
command logs and replays, lockstep networking, the AI framework, the test harness and setting packs.

## Mechanics are modules

Each mechanic is a self-contained system in `crates/classic-sim` with its own state, its own place in the fixed tick
order, its own events and its own tests: harvesting, power, placement, production, combat, the hazard, blooms,
the starport, superpowers, decay and so on. Later ones (naval movement, capture, regrowth, walls) are added the
same way. A setting pack switches modules on or off in `setting.json`, and the rules data says which entities
each module needs. A switched-off module costs nothing and changes nothing, and the tests check that turning
off one module leaves the others' results unchanged.

Two rules keep this honest:

- **No module assumes a setting.** Nothing in code may assume exactly three factions, one resource type or
  land-only movement. Counts and kinds come from data.
- **New mechanics come as modules, not special cases.** If a future pack needs something new, it is written as
  a module with a generic id, and every pack can then use it.

## The parts

| Part | Folder | Does | Status |
|---|---|---|---|
| Shared core | `rts-core` repository, `rts-core` crate | Genre-neutral parts: integer maths, the seeded generator, the canonical state hash, the command queue and log. Knows nothing about tiles or units; the 3D engine builds on it too. Pinned by commit in `Cargo.toml` | Built |
| Simulation | `crates/classic-sim` | The whole game state and the fixed 15-ticks-per-second step. Map, pathfinding, harvesting, movement; later combat, building, power, production, the hazard, fog | Built: map, A*, harvesting, movement, regrowth |
| Game API | `crates/classic-sim/src/game.rs` | `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. Tests, the tools, the AI and the front ends all drive the game through it | Built |
| Web build | `crates/classic-wasm` | The same simulation compiled to WebAssembly, with a plain function interface for JavaScript. `web/check.mjs` proves it gives the native build's hashes | Built: step, orders, hash |
| Rules data | `data/rules/` | Every generic id the engine knows (`entities.json`) and the mechanics a pack can switch (`modules.json`), with each built entity's default numbers and the allowed range for each. Footprints and prerequisites come with placement and production | Built: ids, kinds, numbers and ranges |
| Setting loader | `crates/classic-data` | A small JSON reader (whole numbers only), the rules table, and the pack loader: reads a pack, checks it against the rules, merges names and tuning (asset paths later). The tools find a pack with `--setting` | Built: names, factions, features, tuning, file-type check |
| Computer opponent | `crates/` (new crate) | Issues the same commands a player would; never reads hidden state | Planned |
| Renderer | `crates/` (new crate) | Sprite batcher reading a per-tick view of the state; never changes it. wgpu, which runs natively and on WebGPU, is the leading choice, so desktop and web share it | Planned |
| Audio | `crates/` (new crate) | Plays sounds for sim events; never changes the state | Planned |
| UI | `crates/` (new crate) | Production rail, minimap, selection, menus, styled by the pack's theme | Planned |
| Tools | `crates/classic-tools` | Headless CLI and the bench; later AI-vs-AI batch runs, pack and asset checks | CLI and bench built |

## Language and builds

Everything is Rust, in one Cargo workspace. One simulation builds two ways:

- **Desktop:** native code, the fast build for large games, and the one free to use threads around the sim.
- **Web:** the same code compiled to WebAssembly, for sharing a match or a replay from a link.

Both builds run the identical simulation, so they give the same state hashes, and replays and matches work
across them. The tick itself stays on one thread on every platform; `plans/rts/performance.md` in the playbooks
repository says where threads and the GPU are used instead. The engine was first written in TypeScript; the
Rust port was checked against that version's hashes, paths and node counts tick for tick
(`crates/classic-tools/tests/golden.rs`).

## How a game runs

1. **Load.** `load({ map, seed, players, setting })` reads the map, the rules data and the chosen setting pack.
   The pack's tuning is merged into the rules (within their allowed ranges).
2. **Input becomes commands.** A click, a hotkey or the AI produces a command such as "units 4 and 7, move to tile
   (20, 9)". Commands are queued for the next tick and written to the command log.
3. **The sim steps.** Each tick applies that tick's commands, then runs every system in a fixed order. Only
   integers, one seeded random generator stored in the state, entities in id order. Nothing reads the clock.
4. **The sim emits events** (`delivered`, `fired`, `destroyed`, `hazard_ate` ...). The renderer, audio and UI
   react to them, and to a read-only view of the state, to draw sprites, play sounds and show messages. Nothing
   they do feeds back into the game.
5. **Replays and saves** are just the map, seed, setting, tuning and command log. Re-running them reproduces the
   game exactly, which the tests check by comparing state hashes.

Because steps 3 and 4 are separate, the same game can run headless (tests, AI training, balance runs), as a
desktop program or in a browser, and give the same result.

## Keeping the code generic

**What a setting pack contains** (all data, no code):

```
settings/<pack-id>/
  setting.json      title, factions (display name, colour ramp, advisor), modules switched on, rule switches, presets
  names.json        display name for each generic id
  tuning.json       number overrides, within the ranges in data/rules (optional)
  lines.json        advisor and unit lines, notifications, briefings
  campaign.json     territories and mission list
  maps/             mission and skirmish maps
  art/              sprite sheets, terrain tiles, icons, portraits
  audio/            sound and music lists and files
  theme/            theme.css (colours, fonts, panel frames), cursors, logo
  provenance.jsonl  where every asset came from
```

**What a pack may do:**

- Name everything, draw everything, voice everything, and restyle the UI through CSS variables.
- Turn whole mechanics (modules) on or off (`hazard`, `blooms`, `starport`, `superpowers`, `decay`, ...) and leave out entities.
- Choose how many factions there are and which faction-specific entities each one gets.
- Change numbers, within each number's allowed range.
- Provide its own maps and campaign.

**What a pack may not do:**

- Contain code. The loader rejects anything that isn't JSON, CSS or an asset.
- Invent ids. A new kind of unit or mechanic is an engine change first, with its own generic id; then any pack
  can name and draw it.

**The rule that makes it work:** if a word only makes sense in one story, it belongs in a pack, not here. The code
says `hazard`; a pack says what the hazard is.

**Packs that exist:**

| Pack | Where | Used for |
|---|---|---|
| `generic` | `settings/generic/` in this repo | Plain names and placeholder art drawn from code, so this repo runs and its tests pass on its own. Two factions, on purpose |
| The private pack | A separate private repository, cloned into `settings-private/` (git-ignored) | Ed's own build; built and run locally only, never deployed |
| Future shareable packs | New folders | Other settings and stories, with no engine changes |

## How we'll prove the separation holds

| Check | Proves |
|---|---|
| Two packs that differ only in names, art and audio give the same state hash for the same seed and commands (names: built, in `tests/packs.rs`) | Story and look never change the game |
| A pack with tuning gives a different rules hash, and its replays refuse to run under another pack | Replays stay honest |
| Every pack passes a schema check: known ids only, numbers in range, every used id has a name, icon and sprite | No missing or invented content |
| A protected-names check over every file and file name (`crates/classic-tools/tests/protected_names.rs`, built), and a check that the public build contains nothing from `settings-private/` (planned) | The private pack never leaks into this repo |

## Order of work

1. ~~Move unit numbers from `crates/classic-sim/src/units.rs` into `data/rules/`, with ranges.~~ Done.
2. ~~The setting loader and the `generic` pack, with the checks above.~~ Done, except the icon and sprite checks,
   which come with the renderer.
3. Core rules: base building and power, production, combat (tests first, as in `CLAUDE.md`).
4. Renderer and UI for the desktop and web builds, then the computer opponent, then the hazard, fog and campaign.
