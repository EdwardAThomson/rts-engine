# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games, in Rust. The simulation runs headless, deterministic
and testable; it builds natively for desktop and to WebAssembly for the browser, and both builds play exactly
the same games. Every world it plays (names, factions, art, audio, campaign, UI theme) comes from a data-only
**setting pack**; today packs supply names, factions and tuning.

## Install

You need [rustup](https://rustup.rs). The first `cargo` command in the repository installs the pinned toolchain
from `rust-toolchain.toml` (Rust 1.97 with clippy, rustfmt and the WebAssembly target) by itself. The simulation,
data and tools crates have no other requirements.

The player (`classic-render`) draws with wgpu and plays sound through cpal, so it needs a GPU driver (Vulkan, Metal
or DirectX 12) and, on Linux, a C compiler, `pkg-config` and ALSA's headers:

```bash
sudo apt-get install build-essential pkg-config libasound2-dev     # Debian and Ubuntu
sudo apt-get install mesa-vulkan-drivers xvfb                      # only for a machine without a GPU (CI, cloud): Mesa's software GPU and a virtual display
```

For the browser builds: Node (tested with 22) for the checks, Python 3 (or any static file server) to serve the pages,
and [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen)'s command-line tool at exactly the version
`Cargo.lock` gives the `wasm-bindgen` library:

```bash
v=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/version = "\(.*\)"/\1/p')
cargo install wasm-bindgen-cli --version "$v"
npm install --no-save playwright && npx playwright install --with-deps chromium   # only for web/play/check.mjs
```

The art studio (`art/`) needs Python 3.11 with Blender as a module: `python3.11 -m pip install bpy numpy pillow`
(see [art/README.md](art/README.md)). Nothing else needs it; the packed sprites are committed.

## Run

Headless, with no window:

```bash
cargo test                                                    # every check, about 1 s after the first build
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500     # a 10-minute game in a few ms
cargo run --release --bin cli -- --map maps/skirmish-01.txt --ai 0,1 --ticks 40000   # two computer opponents play it out
cargo run --release --bin bench                               # performance on a 128 x 128 map, up to 500 units
cargo run --bin sounds                                        # rewrite the generic pack's placeholder sounds from their recipes
```

The desktop player opens on the title screen, where you pick the map (a pack's own maps, else
`maps/skirmish-01.txt`), your faction and how well the computer plays (easy, normal or hard), then you play it: build from the rail, drag to select, right-click to order, ctrl+number for groups, ctrl+X to self-destruct the selected units that can, F to aim your palace's superpower once charged (or click its bar on the rail), H for home, M to mute, Escape to pause. Z sells and C repairs the buildings selected (or the next one clicked); right-click with infantry on a badly damaged enemy building to capture it, and with damaged vehicles on your repair pad to mend them. The pause menu saves the game and loads it again, and the settings screen (from the title or the pause menu) sets the volume of each sound bus, the scroll speed and the keys. Settings and the save are kept between runs in `~/.config/classic-rts/` (or `$XDG_CONFIG_HOME`, or `%APPDATA%` on Windows), and in the browser in the page's local storage. The end screen shows each player's score.

```bash
cargo run --release --bin play                                # --start skips the title, --ai none plays alone, --mute, --seed 3,
                                                              # --map plays one map, --faction 1 starts on the second faction,
                                                              # --fog on|shroud|off overrides the pack's fog of war,
                                                              # --difficulty easy|normal|hard sets the computer's
xvfb-run -a cargo run --bin play -- --frames 60               # smoke run on a machine with no display
```

The same player runs in the browser, drawing with WebGPU, or WebGL2 where the browser has no WebGPU. Build it,
then serve the repository root:

```bash
cargo build --release --target wasm32-unknown-unknown -p classic-render --bin play
wasm-bindgen --target web --no-typescript --out-dir web/play/pkg target/wasm32-unknown-unknown/release/play.wasm
python3 -m http.server 8000      # then open http://localhost:8000/web/play/ (options: ?setting=generic&seed=3&ai=none&mute&start)
node web/play/check.mjs          # checks it in headless Chromium, on WebGPU and WebGL2
```

The debug viewer draws a game from the simulation's WebAssembly build with plain shapes, with no bindings step:

```bash
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs
python3 -m http.server 8000      # then open http://localhost:8000/web/viewer/
```

### Setting packs

Every command above plays the public `generic` pack in `settings/generic/`. `--setting` (or the `SETTING`
environment variable) picks another: a name under `settings/`, a folder holding `setting.json`, or a private pack.
The private packs live in their own private repository; clone it into the git-ignored `settings-private/` folder
(a clone, not a symlink, which git would not ignore):

```bash
git clone https://github.com/EdwardAThomson/rts-setting-private settings-private
cargo run --release --bin cli -- --setting private            # the first pack in settings-private/packs/
cargo run --release --bin play -- --setting <pack>            # a pack by its folder name or its id
```

In the browser, `?setting=` takes a name under `settings/` or a pack folder's path from the repository root, such
as `?setting=settings-private/packs/<pack>`.

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
| `crates/classic-sim/src/storage.rs` | Each player's storage cap, from the refineries and silos standing; a delivery fills credits only up to the cap, and the rest is lost and counted. |
| `crates/classic-sim/src/repair.rs` | Repair: an own building mends a step at a time for credits while repair is on, slower when its owner is short of power and waiting while they can't pay; a repair pad mends its owner's damaged vehicles parked beside it, one at a time. |
| `crates/classic-sim/src/sell.rs` | Selling (on unless a pack turns it off): a building sold stops working for a moment, then goes and pays back half its cost scaled by health, plus what its queue had paid. |
| `crates/classic-sim/src/capture.rs` | Capture (on unless a pack turns it off): infantry walk up to an enemy building below a quarter of its health and take it over, going inside; walls, turrets and the palace can't be taken. |
| `crates/classic-sim/src/placement.rs` | Where a building may go: in bounds, firm empty ground, no resource, nothing in the way, near its owner's base. Buildings block ground movement; units already moving path round a new one. |
| `crates/classic-sim/src/world.rs` | The game state and the fixed tick: commands, movement, the harvester loop (find field, mine, return, unload into credits), resource regrowth. |
| `crates/classic-sim/src/vision.rs` | Fog of war, when a pack turns the `fog` module on (the generic pack does): each player's explored tiles and a count of their sight sources over each tile, kept up to date by adding and removing discs as entities appear, move tile and die, from a building's edges. Fog hides enemy units out of sight and keeps a ghost of each enemy building as last seen (or, with `hide` off, shroud only: explored ground shows everything, as the original did). Targets must be in their owner's sight, attack orders need a target in sight or a ghost, and a unit that fires is shown to the player it fires at for a moment. Hashed while on. |
| `crates/classic-sim/src/game.rs` | The game API: `step`, `order`, `spawn`, `snapshot`, `hash`, `command_log`. |
| `crates/classic-ai` | The computer opponent: a player without a mouse that reads the game and issues the same commands a player does. Builds a base from a build order of generic ids (power first when short), places each building with the placement check while keeping factory exits and refinery docks clear, fills its refineries with harvesters, makes a weighted mix of every armed unit its factories can build (battle, siege and missile tanks, quads, scout bikes, single infantry and rocket infantry, and infantry and rocket squads by default) from what is left over once the base and harvesters are paid for, gathers them at a rally point and defends its base and harvesters. It attacks only where its waiting units would beat the defenders (health times damage rate, from the rules' own numbers), gathers each wave out of the defenders' reach before going in (waiting for its slowest units), keeps fast units in step with slow ones on the way in (a unit that can't reach its target leaves the wave), turns back when the odds turn, raids harvesters, and sends everything when its income has stopped. Under fog it reads only what its side can see, guesses the other players' start positions and sends its fastest idle fighter to look at the nearest one it hasn't explored. Distances run from exact footprint centres and ties go to the side nearer the middle of the map, so it plays a mirrored map the same way round from either side. One "normal" opponent so far. |
| `crates/classic-tools` | The headless CLI, the bench, and the seeded bench scene they and the golden tests share. |
| `crates/classic-render` | The wgpu renderer and the player, on the desktop and in the browser: the pack's art in faction colours, the map, buildings, units, effects (`effects.rs`: muzzle flashes, shells and rockets, smoke trails, explosions, smoke and fire on damaged things), selection and orders, and computer opponents for every other player. `hud` is the production rail on the right (credits and power readout, a tab per factory kind, build grid, selection card, queue, minimap) and placing buildings; `menu` is the title, pause and end screens, where the player picks the map (the pack's own, listed in its `setting.json`, else the engine's) and their faction; `fog` draws the local player's shroud and fog with soft edges, the minimap shows them too, enemies out of sight are hidden and enemy buildings in fog drawn as last seen, and sounds from the world play only where the player can see; `feed` is the message feed, worded by `data/ui/messages.json` unless the pack rewords it; `theme` reads the pack's colours from its `theme/theme.css`. `platform` is the genre-neutral part (GPU, textures, sprite batcher, pixel font, sound mixer and device, WAV files, clock, files, the browser page), shared with the 3D engine as the `rts-platform` crate in the `rts-core` repository and pinned by commit. `sound.rs` turns the game's events into sounds, by the rules in `data/audio/`; `web.rs` fetches a game's files in the browser. |
| `crates/classic-wasm` | The WebAssembly build's interface; `web/check.mjs` runs it in Node. `view.rs` holds the read-only functions the viewer draws from. |
| `web/play/` | The page for the browser build of the player: a full-window canvas. `check.mjs` opens it in headless Chromium on WebGPU and on WebGL2. |
| `web/viewer/` | A browser page that plays a game from the WebAssembly build and draws it with coloured shapes: terrain, resource fields, buildings, harvesters and tanks moving between ticks. Play, pause, step, speed, seed; click a unit to inspect it, right-click to move it. No dependencies or build step. |
| `maps/test-01.txt` | Two players, six resource fields, a cliff ridge. The tests' map. |
| `maps/skirmish-01.txt` | 64 x 40, two large rock plateaus with room to build, near, far and contested resource fields, outcrops and cliff ridges. The desktop player's map. |
| `maps/mirror-01.txt` | The left half of `skirmish-01` and its mirror image, starts included: the map the fairness tests play, where neither side should be favoured. |
| `settings/generic/` | The public setting pack: plain names for every id, two factions, generic placeholder art and sounds, fog of war on. |

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
  When ties between equally good tiles began to turn round with the map (and a start in the right half to be laid
  out mirrored), every hash was re-recorded on purpose: the standalone path is the same tile for tile with one more
  node expanded, and the bench scenes' path checksums changed (`crates/classic-tools/tests/golden.rs` says what).
  When units began to leave factories and harvesters to unload on any side, and a start in the bottom half to be
  laid out turned round, every hash was re-recorded on purpose; standalone paths, path checksums, the scripted
  command log and the lone tank's position were checked unchanged, and the bench scenes' credits rose.
  When harvests began to be capped by storage, every hash from the first delivery on was re-recorded on purpose;
  with storage tuned out of reach every earlier value matched exactly, and the scripted replay hash is now
  `80655e40`.
- The WebAssembly build gives the same hashes as the native build.
- Two runs with the same seed and orders give the same state after 10,000 ticks; a different seed gives a
  different game; replaying seed, map and command log reproduces the live game.
- Both harvesters deliver within 60 s of game time; for 5,000 ticks resource is conserved, credits are never
  negative and entity ids stay sorted.
- Paths never enter a cliff and go round the ridge; a cliff tile is refused as a goal; a sealed-off goal is
  refused without searching.
- Orders for another player's units or for buildings are ignored.
- Each player starts with a construction yard, a power plant, a 3x2 refinery with its harvester beside it, a tank
  and 1,200 credits; the base has a power margin of 70, and a start in the bottom half of a map turned half round
  is laid out as the half turn of one in the top half. Placement accepts a building on rock touching
  its owner's base (diagonals count, walls don't) and refuses each broken rule with its reason, including a
  refinery with no side to unload on; tuning can allow open ground or let a base reach further; a tank already moving drives round a building placed on
  its path; a bigger refinery moves its dock and harvesters still deliver; placements replay from the command log.
- Storage: a refinery and a silo each add 1,000 to their owner's cap and the yard nothing; a silo waiting to be
  placed adds nothing; a 200 delivery onto 950 of a 1,000 cap stores 50, loses 150, empties at its normal rate and
  says `storage_full` and `credits_lost` once each; credits above the cap (the start's 1,200, a refund) are kept but
  every delivery is lost; a lost silo lowers the cap without taking credits; `credits_lost` comes at most once per
  450 ticks while every loss is counted. The advisor says "Storage full" once for a fill and the loss after it, the
  readout shows credits out of storage, and the computer opponent puts a silo first when credits it has no plans
  for pass 80% of its storage.
- Repair, sell and capture (`crates/classic-sim/tests/repair.rs`, `sell.rs`, `capture.rs`): a refinery mends 18
  health every 5 ticks for 4 credits a step and turns repair off when whole; repair waits with no credits (and
  credits never go negative), resumes when paid, slows under a power shortfall and can be turned off; a repair pad
  mends one vehicle at a time while the other waits its turn, charges 8 health's share of each vehicle's cost per
  step, keeps every unit out of its footprint and sends a mended harvester back to work; a sold power plant gives no
  power at once, goes after 15 ticks and pays back 120 for 300 at 80% health, and its tiles open again; a yard sold
  mid-build pays back what its queue paid and builds nothing more; a building destroyed while being sold pays
  nothing and a turret being sold holds fire; infantry take an enemy plant below 25% health, its power comes over
  and the new owner's tank stops shooting it; a captured yard loses its queue with no refund; a building at 25%, a
  turret, an own building or a tank refuse capture with their reasons, a tank can't capture, a building healed on
  the way sends the infantry back to guard, an unseen building can't be named under fog, and a pack can switch
  selling and capture off. Each replays to the same hash, and no earlier hash moved.
- Power adds up from the buildings standing, a damaged plant gives less, the power factor stops at its 25%
  floor, `power_changed` reports supply, demand and shortfall only when they change, and a pack's tuning changes
  the numbers and the floor.
- Production: each factory queues up to five items and builds the head one, paying as it goes (a 600-credit,
  450-tick tank costs 300 by tick 225 and exactly 600 at the end); a power shortfall slows it to the power factor;
  it pauses without losing progress when credits run out and resumes by itself; cancelling refunds exactly what
  was paid; a finished building waits at the yard until placed, and only a ready building can be placed; a
  finished unit leaves by the free tile round its factory nearest the middle of the map (a factory in a corner
  sends units out of the corner facing the middle) or waits until one frees up; prerequisites, the primary factory and replay
  from the command log all hold.
- Combat: tanks in sight pick each other, turn their turrets the short way and trade shells; a full shell hit on
  heavy armour does exactly its damage; a destroyed unit is removed, credits its killer, and its death blast hurts
  nearby enemies twice as much as its own side; two units can kill each other on the same tick; an attack order
  closes to within range and stands; an attack on a building walled in two tiles out goes to the nearest reachable tile it can fire from; a moving unit ignores enemies; a rocket turret needs power and a gun turret
  doesn't; a destroyed building frees its tiles; guards prefer armed units to buildings; a pack can tune weapons
  and the damage table; combat replays from the command log.
- Collision: a tile holds one ground unit, checked on every tick of every collision test; a unit waits behind
  another instead of driving through it; a move to a taken tile ends next to it; nine tanks sent to one tile end
  on nine tiles round it; an idle own tank steps aside, and an enemy one never does, so the mover gives up; two
  tanks meeting head-on in a corridor get past each other; twelve tanks squeeze through a one-tile gap with none
  stuck; a harvester unloads on another side when a tank stands under the refinery's pad; where cliffs leave one
  side, an own tank on it gives way to the harvester while an enemy one blocks it, and a harvester queued for it
  steps aside for the one leaving it, and both keep delivering; a factory's new
  units drive clear of its exit; collision replays from the command log.
- Rendering (with Mesa's software GPU, no window): the start base is drawn in its faction's colours with no remap
  colour left on screen and the map covering the frame; the other faction's tank is in its own colours; the same
  frame twice gives the same pixels and drawing never changes the game's hash; shots and explosions appear from
  events; a tank fight plays muzzle flashes, bursts, smoke and a kill's explosion, and barrel tips measured from the
  turret frames swing round with the turret; recolouring swaps exact remap pixels only. The desktop player opens, selects and orders under Xvfb, and its computer opponent issues
  orders there.
- The production rail, driven by clicks: it offers what the player's factories can build now plus the next tier,
  locked; shift-click queues five and right-clicks cancel them with every credit refunded; a ready building's
  ghost agrees with the simulation's placement check on every tile on screen, a bad spot orders nothing, a good
  one places it; clicks on the rail never reach the world; the HUD draws only in its own place, and a glyph
  lights exactly its own pixels. The minimap keeps the map's shape, maps its corners and centre to the map's,
  shows the player's base in their colour, and its clicks move the view or ask for an order without ordering
  anything itself.
- Radar, emblems and the card's buttons: with the `radar` module on (the generic pack turns it on), the minimap
  shows only grey static and "NO RADAR" until the player owns a radar with power enough, its clicks do nothing
  then, and the feed says "Radar online" and "Radar offline" as that changes (never at the start). With the module
  off, as in the builtin rules, the minimap always works. The readout shows the local faction's emblem and the
  selection card its owner's. SELL and REPAIR buttons above the card work like Z and C, SELL only where the rules
  allow selling, and show pressed while their mode is on. Radar is read from the state, so no hash changed.
- The selection card and message feed: the card draws in its place between the grid and the queue; the feed tells
  the local player that their building is ready (another player's is not mentioned), that power ran short and came
  back, that units are under attack (at most once per 20 seconds however many hits) and that a harvester was lost,
  by name, and drops each line after 8 seconds; a pack's `ui/messages.json` rewords a message and an unknown id is
  warned about. Control groups keep only the player's own units, a second press asks to centre, and a destroyed
  unit leaves its group; Tab steps through the factory tabs and wraps.
- The menus: the title screen starts a game and switches the computer opponents on or off, and offers the map and
  the player's faction when there is a choice (the other players take the remaining factions in order); Escape pauses and
  resumes; the end screen comes when someone wins (victory) or the player loses their last building (defeat) and
  offers another game; the menus shade the whole screen and draw nothing while playing, and never change the
  game's hash. In the player under a virtual display: start with no opponents, pause, back to the title, quit.
- Difficulty: easy, normal and hard are presets of the computer's settings. On `skirmish-01`, over seeds 1 to 6 from
  both starts, hard beat normal 11 to 1 and normal beat easy 11 to 1 (with aircraft and faction specials; before aircraft,
  bigger waves alone gave hard 8 to 2, and 7 to 4 once carriers came).
- Settings, saves and the score: the settings screen steps each bus's volume and the scroll speed (right click steps
  down), the keys screen puts a key on an action and swaps one that clashes, and the settings file reads back as
  written and skips lines it can't use, with a warning. A game saved at tick 8000, mid-attack, with a click queued
  on that tick, loads to the same hash and plays on to tick 14000 exactly as the game that never stopped; a save
  with a wrong hash, another seed, other rules or another opponent refuses to load. The score's totals match the
  game's events even when the player clears them between ticks.
- Pack maps and theme: a pack's `setting.json` lists its maps, each checked to be a text file in the pack; a pack's
  `theme/theme.css` recolours the HUD and menus, and colours it can't read keep the engine's, with a warning. The
  generic pack's theme is the engine's own colours. In the player under a virtual display, with the private pack: the
  title offers its three maps and three factions, and a game started on the second map and faction plays there in
  that faction's colours.
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
  plant, two refineries, a barracks, both factories, a radar, turrets, harvesters and combat units, with no command refused; never attacking, it builds every armed unit the rules have, battle tanks most and the three tank kinds fewer than the rest together; a kind weighted 0 is never built, and with no weights every armed unit the rules have is; it
  destroys every building of a player who does nothing, from either start, and never before its first-wave time;
  two AIs play the same 20,000-tick game twice to the same hash and command log; a game between two AIs replays
  from its command log with the AI switched off to the same hash; thinking never changes the game's hash; a power
  plant removed at tick 3,000 is rebuilt within 90 seconds; an enemy tank beside its base draws an attack order
  and takes hits within 20 seconds. Three tanks don't attack eight gun turrets and are all kept, but do attack the
  same base without them; with its income gone for `broke_ticks` it sends every unit at once; with its last
  harvester lost while its factories wait on tanks it can't pay for, it cancels them and builds a harvester.
- Fairness (`crates/classic-sim/tests/mirror.rs`, `crates/classic-ai/tests/skirmish.rs`, on `mirror-01`): the two
  starting bases are mirror images; paths, including the way round a unit in the way, are mirror images; with
  chance taken out (no random waits, regrowth or scatter) the two sides harvest as mirror images for 9,000 ticks,
  and two computer opponents play mirror images of each other until their units first meet, after both first waves have gone out (two mirrored units meeting head-on can't stay mirror images, as movement goes in id order). In batch runs (computer against
  computer, 90 game minutes, 200 seeds each way round), left-right mirrored maps split evenly: `mirror-01` 169 to
  143 wins for the left side, and three more mirrored maps 99 to 101, 102 to 97 and 101 to 98.

- Fog of war (`crates/classic-sim/tests/fog.rs`): off in the engine's own rules, so every golden hash is unchanged;
  sight discs from a building's edge, explored ground staying explored, fog hiding units (and shroud only showing
  them), ghosts kept until their empty ground is seen, attack orders and auto-targets limited to what a side sees,
  firing showing the shooter, the `start_explored` switch, and the running counts equal to a fresh count every 50
  ticks of a game. Two computer opponents under fog play the same game every run, every attack order they give names
  a target they know, and one finds and beats a player who does nothing by tick 8,714 with fog and with shroud only
  (`crates/classic-ai/tests/fog.rs`). The renderer test draws a fogged map and checks the enemy base is black and the
  player's own lit; a sound test checks a fight in the shroud is silent for the player who can't see it.
- Aircraft (`crates/classic-sim/tests/air.rs`, the `air` module): no aircraft in any golden game, so every golden
  hash is unchanged. An aircraft climbs before it moves, flies a straight line over a cliff wall a ground unit
  would go round, slows over its last 2 tiles and lands; aircraft share tiles with each other and with ground units,
  and hover instead of landing over a building or a cliff. Only weapons that hit air (rocket infantry, rocket
  squads, the rocket turret) aim at an aircraft in flight, and their shots home in on it; a tank drops an attack
  order on one, but shells a landed one. A gunship crosses the cliff and fires on a tank only from the air. A burst
  on the ground never splashes an aircraft above it, and nothing aims at the untargetable supply ship. An air
  factory builds a carrier, which waits landed beside it; a gunship needs a research lab. An idle carrier lifts a
  full harvester 26 tiles from home and it delivers sooner than by road; aboard, it can't be shot and takes no
  orders, and its carrier finishes the lift before obeying a move; a carrier shot down drops it to the ground with
  half its full health gone. Games with aircraft replay to the same hash. The renderer test draws a carrier and a
  gunship lifted above the shadows they leave on the ground. The computer opponent builds an air factory last in
  its build order and keeps a carrier for every three harvesters: in 16 games on `skirmish-01` and `mirror-01`
  carriers made about 300 lifts a game, and in 8 games on `mirror-01` the two sides lifted and delivered within 4%
  of each other.
- Faction specials (`crates/classic-sim/tests/specials.rs`): no golden game has a faction set or a special in it, so
  every golden hash is unchanged. A player builds only their own faction's special (`super_a` for `faction_a`,
  `super_h` for `faction_b`, `super_o` for `faction_c`, each needing a heavy factory and a research lab), and a
  player with no faction builds none; nobody builds `guerrilla` or `saboteur`, which come with the palace powers.
  The beam (`sonic_wave`) hits everything on a 5-tile line at once, its own side at half and its own kind not at all.
  Converting gas (`convert_gas`) takes an enemy vehicle over for 375 ticks without hurting it, never infantry or a
  special, and the vehicle goes back after. A self-destruct order counts down 30 ticks, during which the unit won't
  move or fire, then leaves `blast_large` (300, splash 640), as it also does if killed during the countdown. The
  sapper (`saboteur`) is hidden from a player with nothing within 2 tiles of it, fires only at buildings, and
  spends itself to destroy a power plant; it disappears after 2,700 ticks. Games with the specials replay to the
  same hash. The player tells the game each player's faction, and the computer opponent now builds a research lab
  last, so it makes its faction's special and gunships. In 120 games on `mirror-01` (20 seeds for each ordered
  pair of factions, 60,000 ticks) the 98 decided games went 38, 33 and 27 to `faction_a`, `faction_b` and
  `faction_c`, after the beam's damage went from 55 to 70 and `super_h`'s health from 600 to 500.
- Starport market (`crates/classic-sim/tests/starport.rs`): no golden game has a starport, and the market opens only
  when the first one stands, so every golden hash is unchanged. An order of up to 5 units is paid in full at the
  prices of the moment, or refused whole and left open to change; a supply ship flies in from the nearest map edge,
  lands on the starport 600 ticks later (1,200 if its owner was short of power when paying), sets one unit down
  every 15 ticks and flies off the map again, taking no orders on the way. Prices drift every 900 ticks within 75%
  to 150% of base, the same in every run of a seed. Each player has their own stock of 3 (2 for the dearer kinds),
  which grows back one every 1,800 ticks. A starport lost before the ship lands refunds the whole order. Games with
  a starport replay to the same hash. In the player the starport has a sidebar tab: icons show today's price or
  SOLD OUT, left click (shift for as many as allowed) adds to the order, right click takes one out, the queue row
  shows the order and SEND pays (`crates/classic-render/tests/hud.rs`).
- Palace powers (`crates/classic-sim/tests/superpowers.rs`): no golden game has a faction or a palace, so every
  golden hash is unchanged. Each faction's palace gives one power (`faction_a` guerrillas, `faction_b` the missile,
  `faction_c` the saboteur, from the `superpowers` module's data); it charges only while its owner has a palace and
  enough power, says so once when full, and a player with no faction has none. The missile lands 30 + 2 ticks a
  tile after launch, within its spread of the tile aimed at, and takes 800, 600, 350 and 150 from everything on the
  ground 0 to 3 tiles away, either side's, a building by its nearest tile. Five guerrillas arrive 45 ticks after
  the order, 3 to 6 tiles from the tile, take no orders from their owner, and fight what they find there. The
  saboteur comes out of the palace and destroys the building it was aimed at. Games with each power replay to the
  same hash. The player shows the charge under the credits; F or a click on it aims, and the next click uses it
  (`crates/classic-render/tests/hud.rs`). The computer opponent builds a palace after its research lab and uses its
  power on the best target it knows of. In 120 games on `mirror-01` (20 seeds for each ordered pair of factions,
  60,000 ticks, fog off) the decided games went 34, 33 and 33 to `faction_a`, `faction_b` and `faction_c` (100
  decided), against 38, 33 and 27 (98 decided) with the same games and no palace.

## Not verified / not built yet

- Radar powers only the minimap: it reveals nothing on the map, and the minimap without one is static, not a
  last-seen picture. Fog of war's paths are still planned with the
  whole map known, so a path can give away ground nobody has seen (rules-world.md wants unexplored tiles treated as
  passable). Sight counts work on whole tiles with a loop over each disc's square, not the precomputed row spans and
  changed-tiles list performance.md plans; the bench runs with fog off. Effects (shots, explosions) in fog are still
  drawn, dimmed by it; in shroud they are covered. The computer opponent knows a ghost building's real health, and
  that it is gone, a little before it should. Fog was checked in tests and screenshots, not yet by a person playing.
- Collision covers vehicles only (aircraft have none, by design): no infantry positions, crushing, group formations, keep-clear tiles or
  bodies that turn before driving yet, and a blocked search returns no partial path.
- Combat has single infantry and rocket infantry, infantry squads, rocket squads, scout bikes, quads, siege tanks and missile tanks (our own first numbers, from `rules-combat.md` and `rules-movement.md` where they give them; a unit whose weapon has a minimum range, the missile tank, backs off to a tile it can fire from), but no crushing, bursts (the missile tank and `super_h` fire one shot for the doc's two), attacks on the ground, factory upgrades (siege and missile tanks need none yet) or tech levels; non-turreted units still fire on the move, and squads don't share tiles, and guards don't chase or return yet; sight is
  a stand-in until vision exists, and the weapon numbers are first guesses.
- The computer opponent has three levels (easy, normal, hard) with numbers in code: no brutal level, personalities, data files
  in `data/ai/`, scouting beyond one unit sent to the nearest unexplored start position under fog, retreat by
  exchange, counter-composition, target scoring, slabs, superpowers or remnant mode. Its memory lives in the `Ai`
  value, not the hashed game state; a save leaves it out and loading rebuilds it by playing the game forward. Hard
  differs from normal in its waves (twice the size, surer odds) and in a carrier for every two harvesters, not
  three: in our runs more harvesters or thinking more often made it no stronger. With the mixed army and factory exits on
  any side (20 seeds, 90 game minutes), two AIs on `skirmish-01` win 8 to 8 with 4 stalls and games last about 40
  minutes; on `mirror-01` (10 seeds) 4 to 4 with 2 stalls. Mixed armies trade evenly, so games run longer than
  with tanks alone, and Twin Plateaus in the private pack still stalls in about 4 games of 10. In the desktop player it was checked only
  in a short smoke run, not played by a person.
- Units leave factories and harvesters unload on any side, so maps turned half round are now about as fair as
  mirrored ones: on `skirmish-01`, with its second start moved to the exact half turn of the first, two computer
  opponents went from 200 of 200 top wins to 70 top, 85 bottom and 45 stalemates in 200 games. The fairer games
  stalemate more often: factories no longer jam (in 20 games on `mirror-01` the old south exits held finished units
  for about 3,000 factory-ticks a side; now none), so both armies come out even and neither breaks through before
  the resource runs out (129 of 200 on `mirror-01`, from 37). Of the decided games on `mirror-01`, the left start won
  50 to 21 in seeds 1 to 200 but 33 to 28 in seeds 101 to 300, so a small left edge may remain, perhaps from units
  acting in id order when even armies meet (inferred, not traced). Maps turned a quarter round are not fully fair,
  because the starting base doesn't turn with them.
- No tech levels or factory upgrades. Aircraft: the gunship fires one 45-damage rocket for the doc's burst of
  3 × 20; carriers lift harvesters on long trips only, not damaged units to a repair pad or units stuck on their
  way. Carriers make the economy faster, so fields run dry and more AI games end:
  of 30 seeds on `mirror-01` to 60,000 ticks, 11 were decided (10 for the right start), against 9 before aircraft
  (8 for the right start), so that lean is older than aircraft and wants a look of its own. Aircraft were checked in tests and a rendered
  frame, not yet by a person playing.
- Faction specials: the beam and the self-destruct blast have stand-in effects (sparks along the line, a large
  explosion) and no sounds of their own; a converted unit isn't drawn any differently. `guerrilla` comes only with
  the palace powers, and its hiding on rough ground isn't built. The computer opponent
  never orders a self-destruct and sends its specials in waves like any other unit. A converted harvester goes back
  to work for its new side, which wasn't tested. Balance comes from computer games only, and not yet played by a
  person: `faction_a` beat `faction_c` 23 to 12, a lean these runs can't yet tell from noise.
- Palace powers: the guerrillas don't hide on rough ground yet, the missile leaves no scorch mark, and its flight
  and blast are stand-in effects (a rocket sprite on an arc, large explosions). The private pack has its own words only
  for `superpower_ready`; the other new lines use the engine's. Tech levels aren't built, so the powers need only a
  palace. The computer scores the tiles of enemies it knows of rather than the design's grid. Checked in tests and
  computer games, not yet by a person playing.
- Starport: the computer opponent never builds one or buys from it, prices and stock show only on the starport's
  tab, and the supply ship has no landing or take-off effect or sound. It was checked in tests, not yet by a person
  playing.
- The menus have one save per setting pack (no slots, autosave, thumbnails or quick keys), and loading replays
  every tick from the start, which on a long game takes a moment rather than an instant (about 0.2 s for 30 game
  minutes of two computer players in a native release build; slower in the browser, not measured there). The settings have no
  master volume, UI scale or display options, and only ten actions can be rebound. A save from an older engine
  that plays differently refuses to load rather than being upgraded. The settings, keys and end screens were drawn
  in a test and the player started under a virtual display, but no person has played through them. The rail has
  no tabs by category, pause per item or primary factory choice yet,
  the card's unit chips can't be clicked. The advisor and the units speak in text (the feed, and a subtitle when units
  are selected or ordered) and aloud where a pack has voices for them; the generic pack's placeholder voices speak
  the engine's own words, and a pack's own voices replace them. The advisor also warns of a harvester under attack,
  a hazard appearing and enemy units massing near the base, and says how the game ended; a move order no unit can
  walk to gets the refused-order reply (the units still go as near as they can). Storage and superweapon lines wait
  for those mechanics. A pack's own art (the Blender
  studio's packed sprites in `art/sprites/`, its terrain tiles in `art/tiles/`) is drawn over the generic pack's,
  file by file, so a pack draws only what it changes. Icons in the rail are still the placeholders, and wrecks and the build-up frames
  aren't drawn. Squads are drawn as three soldiers; their walk and death clips show once the studio's infantry are packed. The player was checked under a virtual display
  with a software GPU, not on a real desktop GPU.
- Sound is effects, interface sounds and a pack's spoken lines: no music, looping sounds or
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
