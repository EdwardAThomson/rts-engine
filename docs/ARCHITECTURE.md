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
 data/rules ──►│  sim (src/sim)  ──events──►  renderer, audio, UI  ──►  screen/speakers │
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

## The parts

| Part | Folder | Does | Status |
|---|---|---|---|
| Simulation | `src/sim` | The whole game state and the fixed 15-ticks-per-second step. Map, pathfinding, harvesting, movement; later combat, building, power, production, the hazard, fog | Built: map, A*, harvesting, movement, regrowth |
| Debug API | `src/sim/game.ts` | `step`, `order`, `spawn`, `snapshot`, `hash`, `commandLog`. Tests, the CLI, the AI and (later) the browser all drive the game through it | Built |
| Rules data | `data/rules/` | Every entity's generic id, footprint, prerequisites and default numbers, with an allowed range for each tunable number | Planned; numbers sit in `src/sim/units.ts` for now |
| Setting loader | `src/settings/` | Reads a pack, checks it against the rules, merges names, tuning and asset paths | Planned |
| Computer opponent | `src/ai` | Issues the same commands a player would; never reads hidden state | Planned |
| Renderer | `src/render` | WebGL2 sprite batcher. Reads a per-tick view of the state; never changes it | Planned |
| Audio | `src/audio` | Plays sounds for sim events; never changes the state | Planned |
| UI | `src/ui` | HTML and CSS over the canvas: production rail, minimap, selection, menus. Styled by the pack's theme | Planned |
| Tools | `tools/` | Headless CLI, later AI-vs-AI batch runs, benchmarks, pack and asset checks | CLI built |

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

Because steps 3 and 4 are separate, the same game can run headless in Node (tests, AI training, balance runs)
or in a browser with graphics, and give the same result.

## Keeping the code generic

**What a setting pack contains** (all data, no code):

```
settings/<pack-id>/
  setting.json      title, three factions (display name, colour ramp, advisor), feature switches, presets
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
- Turn whole mechanics on or off (`hazard`, `blooms`, `starport`, `superpowers`, `decay`) and leave out entities.
- Change numbers, within each number's allowed range.
- Provide its own maps and campaign.

**What a pack may not do:**

- Contain code. The loader rejects anything that isn't JSON, CSS or an asset.
- Invent ids. A new kind of unit or mechanic is an engine change first, with its own generic id; then any pack
  can name and draw it.
- Change the faction count (three for now).

**The rule that makes it work:** if a word only makes sense in one story, it belongs in a pack, not here. The code
says `hazard`; a pack says what the hazard is.

**Packs that exist:**

| Pack | Where | Used for |
|---|---|---|
| `generic` | `settings/generic/` in this repo (planned) | Plain names and placeholder art, so this repo runs and its tests pass on its own |
| The private pack | A separate private repository, cloned into `settings-private/` (git-ignored) | Ed's own build; built and run locally only, never deployed |
| Future shareable packs | New folders | Other settings and stories, with no engine changes |

## How we'll prove the separation holds

| Check | Proves |
|---|---|
| Two packs that differ only in names, art and audio give the same state hash for the same seed and commands | Story and look never change the game |
| A pack with tuning gives a different rules hash, and its replays refuse to run under another pack | Replays stay honest |
| Every pack passes a schema check: known ids only, numbers in range, every used id has a name, icon and sprite | No missing or invented content |
| A protected-names check over every tracked file, and a check that the public build contains nothing from `settings-private/` | The private pack never leaks into this repo |

## Order of work

1. Move unit numbers from `src/sim/units.ts` into `data/rules/`, with ranges.
2. The setting loader and the `generic` pack, with the checks above.
3. Core rules: base building and power, production, combat (tests first, as in `CLAUDE.md`).
4. Renderer and UI with the browser debug API, then the computer opponent, then the hazard, fog and campaign.
