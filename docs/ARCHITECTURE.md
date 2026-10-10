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
| Simulation | `crates/classic-sim` | The whole game state and the fixed 15-ticks-per-second step. Map, pathfinding, harvesting, movement and collision, building, power, production, combat, the hazard, fog of war | Built: map, A*, a phased tick, harvesting, movement, regrowth, resource blooms (`blooms.rs`, off unless a pack turns it on, and then in place of regrowth: mounds on the map's bloom points burst into new fields when driven onto, caught in a burst or too old, and grow back nearby), buildings with footprints and placement, power, storage (refineries and silos cap stored credits; the overflow is lost), concrete slabs and decay (`decay.rs`, off unless a pack turns it on: slabs are tile state that extend the build area, and a building wears down towards a floor set by how much of its footprint was slabbed), production queues (a primary factory per kind, picked by the player, and entries put on hold), tech levels and factory upgrades (a game's tech level limits what may be built and how far factories go; an upgrade is a queue entry that raises one building's level, and items list the level they need), combat, collision, repair (buildings for credits, vehicles at a repair pad), selling and capture (`repair.rs`, `sell.rs`, `capture.rs`; selling and capture on unless a pack turns them off), the hazard (off unless a pack turns it on; it can't yet be shot), fog of war (`vision.rs`, off unless a pack turns it on: explored and seen tiles per player, ghosts of enemy buildings, targets only in sight); radar (`Game::radar`, with the `radar` module on: a player sees their minimap while they own a radar building and have power enough; read from the state, not stored), aircraft (`air.rs`: units with the `air` role fly straight over any tile with no collision, climb before they move and land when idle, hovering over buildings and cliffs; weapons say whether they hit air and ground, and shots at aircraft home in; carriers lift harvesters on long trips), faction specials (each player may have a pack faction, and kinds with a `factions` list are built only by those; a `beam` weapon hits a whole line at once, a `converts` weapon takes enemy vehicles over for a while, a kind with a `self_destruct` blast can be ordered to blow up after a countdown, a `cloak` hides a unit from players with nothing near it, a `sapper` spends itself on a building, and a `lifetime` makes a unit disappear), the starport market (`starport.rs`: drifting prices, stock per player, orders paid whole and brought by a supply ship), palace powers (`superpower.rs`: a charge per player, then a missile, guerrillas that fight on their own, or a saboteur, by faction), deploying (`deploy.rs`: a unit with `deploys_into`, the base builder, turns into its building where it stands) |
| Game API | `crates/classic-sim/src/game.rs` | `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. Tests, the tools, the AI and the front ends all drive the game through it | Built |
| Web build | `crates/classic-wasm` | The same simulation compiled to WebAssembly, with a plain function interface for JavaScript. `web/check.mjs` proves it gives the native build's hashes | Built: step, orders, hash, read-only views |
| Rules data | `data/rules/` | Every generic id the engine knows (`entities.json`) and the mechanics a pack can switch (`modules.json`), with each built entity's default numbers and the allowed range for each. Footprints and prerequisites come with placement and production | Built: ids, kinds, numbers and ranges, costs, build times, what builds each item and what it requires |
| Setting loader | `crates/classic-data` | A small JSON reader (whole numbers only), the rules table, and the pack loader: reads a pack, checks it against the rules, merges names and tuning (asset paths later). The tools find a pack with `--setting` | Built: names, factions, features, tuning, file-type check |
| Computer opponent | `crates/classic-ai` | A player without a mouse: reads `&Game` and acts only through `Game::order`, so its commands are checked and logged like a human's and a replay plays its games back with the AI off. Deterministic (integer maths, id order, no clock or randomness), with clippy's bans like the simulation's. Managers for the base (build order, power, placement that keeps exits clear), production and harvesters (and a carrier for every three harvesters, two on hard, once it has an air factory; a research lab last, which opens its faction's special and the gunship), and the army (rally point, defence, waves that go only with the odds from the rules' own numbers, gather out of reach, turn back, raid harvesters, and all go when income stops), and repairs its buildings once the fighting round them stops. Geometry in sub-tile units from exact footprint centres, ties to the side nearer the middle of the map, so a mirrored map plays the same from either side | Built: three levels, easy, normal and hard (`Difficulty`), presets of its settings chosen on the title screen. Under fog it reads only what its side sees and remembers, scouts the nearest unexplored start position, ends a wave whose target has slipped out of sight, and, knowing no enemy building once every start is explored, searches the map nearest the enemy starts first. Personalities as data later. The player hands every other player to it, on the desktop (`--ai`) and in the browser (`?ai=`) |
| Renderer | `crates/classic-render` | Draws the game with wgpu (chosen 2026-10-07: it runs natively and on the web, so desktop and web share it), reading the state and its events; never changes either. `platform` (GPU, textures, sprite batcher, font, files, the browser page) is genre-neutral and lives in the `rts-platform` crate of the `rts-core` repository, shared with the 3D engine and pinned by commit; `art` and `scene` know the Classic engine. The player (`play`) is a window onto a skirmish on the desktop, and the same program draws into a page's canvas in the browser (`web/play/`), on WebGPU or WebGL2, with the pack, art and map fetched first. `web/viewer/` stays as the debug view, drawing coloured shapes from `classic-wasm`'s view functions | Built: desktop and web, pack art in faction colours, a pack's own files over the generic pack's (the studio's packed sprites where a pack has them: atlas pages with team paint painted from each owner's ramp through the mask at load, shadow, body, turret and building overlays; the code-drawn strips elsewhere), map, buildings, units (aircraft drawn above the shadows they leave on the ground, over everything on the ground, a carried unit hanging under its carrier), effects (`effects.rs`: muzzle flashes at each barrel tip, measured from the turret's frames where the packer recorded none and per soldier from the sprite's muzzle points, with the firing pose; shells and rockets leaving the barrel and lowering to where they land; rocket smoke trails; explosions per hit, vehicle and building; smoke from damaged things and fire on badly damaged ones; all timed by ticks with their own random generator), selection, health bars, move and attack orders; squads drawn as their single soldier once per member (the art index's `squads`: member sprite and formation offsets), losing one per share of health with the lost one fading where it fell, and walk cycles staggered per member on strips that have them; fog of war for the local player (`fog.rs`: black shroud and a darker veil of fog, with soft edges baked into a page of corner cases since the sprite batch samples nearest pixels; enemies out of sight hidden, enemy buildings in fog drawn from their ghosts); shape viewer built |
| Audio | `crates/classic-render` | Plays sounds for sim events; never changes the state. `rts-platform`'s `audio` is a genre-neutral mixer (sfx, ui, voice and music buses, voice caps, stealing, merging, pan, a soft limiter, loops with their own cap, music crossfades) and the sound device (cpal, behind the default `device` feature); its `wav` reads clips and tracks (PCM, or IMA ADPCM kept compressed and decoded as it plays). `sound.rs` is the Classic part: `data/audio/events.json` says which event plays which sound id, `data/audio/sounds.json` how each id is mixed, and the pack's `audio/sounds.json` only which files each id plays (any it leaves out fall back to the generic pack). Spoken lines: the pack's `audio/voices.json` gives a take for each line of its `lines.json`; the feed reports which line it showed and `SoundBoard::speak` plays that take on the voice bus (one advisor line and one reply at a time, no reply over the advisor, effects and music ducked by `voices.duck_db`; advisor lines that come while one speaks wait in a short queue by `voices.line_priority` and are dropped once stale). Rules can say which factory a unit left (`factory`) and play `also`, so each factory has its own exit sound beside the ready blip. `loops` in `events.json` say which loop plays while a visible unit is moving, flying, mining or unloading; `SoundBoard::update_loops` keeps one loop per sound id, as loud as the nearest unit. `music.rs` plays the pack's `audio/music.json` tracks by pool (`menu`, `calm`, `battle`, `won`, `lost`), turning to battle and back on a tension value from the local player's fighting, with the numbers in `data/audio/music.json`. Unit replies come from the `infantry`, `vehicle` or `aircraft` voice set, or a set a pack gives one unit by its id. | Built: desktop, sound effects and interface sounds, generic placeholders synthesised from code (`cargo run --bin sounds`). In the browser too, through Web Audio once the page has been clicked or a key pressed. A pack's spoken lines, and generic placeholder voices for the engine's own lines (`audio/make_voices.py`), which a pack's own voices replace whole. Loops, factory exit sounds and music, with generic placeholders made from code (`classic-tools` `sound.rs` and `music.rs`) |
| UI | `crates/classic-render` (`hud`, `feed`, `menu`, `theme`, `skin`) | Production rail, minimap, selection, menus, styled by the pack's theme. Drawn with the renderer (not HTML); turns clicks into the same commands any player sends | Built: selection and control groups, sell (Z) and repair (C) for the buildings selected or the next one clicked, a right click that sends infantry to capture a building hurt badly enough and damaged vehicles to a repair pad, and a selected base builder clicked itself to deploy (with the `sell`, `repair`, `enter`, `repair_pad` and `deploy` cursors), a selection card (picture, name, health, what it is doing; chips with health for a group), the production rail (a tab per factory kind, build grid with item states, the primary factory's queue, tooltips, Tab to change tabs, ctrl+click to put an item on hold or resume it, P to make the selected factory primary, an upgrade icon for the primary factory and items it would unlock shown locked), credits out of storage, power and clock readout, building placement with a ghost, a minimap that moves the view and takes orders, a message feed for the local player (`feed`, worded by `data/ui/messages.json`, which a pack can reword, or by its faction's advisor in the pack's `lines.json`; it also warns of a harvester under attack, storage full, a hazard appearing and armed enemy units in sight near the base, and says how the game ended), the units' replies as a subtitle when they are selected, ordered or given a move none of them can walk to (`lines`, from `data/ui/lines.json` and the pack's `lines.json`), the title, pause, settings, keys and end screens (`menu`) with a choice of map (the pack's own, else the engine's), of the player's faction and of the opponents' difficulty, the end screen's score (`score`, counted from events), the player's settings (`prefs`: bus volumes, scroll speed, eleven rebindable keys) kept between runs (`store`: a settings folder, or the browser's local storage), saving and loading (`save`), and the pack's colours (`theme`, from its `theme/theme.css`). The UI skin (`skin`: nine-slice frames, buttons, emblems, cursors chosen by what a click would do, and the pack's fonts, from `theme/theme.json`) draws the rail, panels, wells, tabs, tooltips, menu buttons and text, and sets the window's cursor. The readout shows the local faction's emblem and the selection card its owner's; SELL and REPAIR buttons sit above the card. The minimap shows the local player's shroud and fog, and static without radar where the pack turns the `radar` module on, with "Radar online" and "Radar offline" in the feed |
| Tools | `crates/classic-tools` | Headless CLI and the bench; later AI-vs-AI batch runs, pack and asset checks | CLI and bench built; `cli --ai 0,1` plays computer opponents against each other |

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
   game exactly, which the tests check by comparing state hashes. A save (`classic_render::save`) adds who the
   computer played and how hard, and the tick and hash it was saved at; it keeps only the human players' commands,
   and loading re-runs the computer opponents, which rebuilds their memory too, then checks the hash.

Because steps 3 and 4 are separate, the same game can run headless (tests, AI training, balance runs), as a
desktop program or in a browser, and give the same result.

## Keeping the code generic

**What a setting pack contains** (all data, no code):

```
settings/<pack-id>/
  setting.json      title, factions (display name, colour ramp, advisor), modules switched on, rule switches, presets,
                    and the skirmish maps it offers ("maps": paths in the pack, in the title screen's order)
  names.json        display name for each generic id
  tuning.json       number overrides, within the ranges in data/rules (optional)
  lines.json        advisor lines (by feed message id) and unit replies (by voice set and moment), pack-wide and per
                    faction; briefings later
  campaign.json     territories and mission list
  maps/             mission and skirmish maps (text, the same format as the engine's maps/)
  art/              sprite sheets, terrain tiles, icons, portraits
  audio/            sound and music lists and files
  theme/            theme.css (colours), theme.json (frames, buttons, cursors, emblems, fonts), logo; the player
                    reads theme.json through `skin` and its colours (--panel-bg,
                    --panel-edge, --button, --button-hover, --text, --text-dim, --accent, --good, --warn, --bad)
  provenance.jsonl  where every asset came from
```

**What a pack may do:**

- Name everything, draw everything, voice everything, and restyle the UI through CSS variables.
- Turn whole mechanics (modules) on or off (`hazard`, `blooms`, `starport`, `superpowers`, `decay`, ...) and leave out entities. A
  module with an `on` number takes that number's default unless the pack's `features` says otherwise: the hazard and
  fog of war, decay and blooms are off by default, selling and capture on.
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
