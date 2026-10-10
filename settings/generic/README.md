# The generic pack

Plain names and placeholder art for every generic id, so the engine runs and its tests pass on its own. It has two
factions on purpose: nothing in the engine may assume three. Its art may only ever be our own and generic: if
someone could name the game a picture comes from, it belongs in a private pack instead.

It turns fog of war on (`setting.json`'s `features`), with the engine's default of fog that hides what is out of
sight, so the player and the tests run with it the way most packs will. The player's `--fog` overrides it.

## Art

Everything under `art/` and `theme/` (but `art/sprites/` and `theme/fonts/`) is drawn from code in `crates/classic-tools/src/art` and written by
`cargo run --bin art`. Don't edit the PNGs by hand: change the drawing and rerun it. A test fails when the files
and the drawings disagree. `provenance.jsonl` records each file's source.

- `art/art.json` indexes everything: frame sizes, frame counts and facings for each file.
- `art/units/` and the turrets in `art/buildings/` are strips of eight 32 px frames, facing north first and then
  turning clockwise.
- `art/buildings/` is one frame per building, sized to its default footprint in the rules data at 32 px per tile.
- `art/tiles/` is the terrain the player draws: desert sand, rock, cliffs and light and thick resource, blended
  into each other, made by `art/studio/tileset.py` from `art/terrain/desert.json` (see `art/README.md`, "Terrain
  tiles"). `art/terrain/` has the older four 32 px variants of each ground tile, still used for the minimap's
  fallback colours and by packs without tiles. `art/features/` holds things drawn over the ground.
- `art/icons/` holds 64 x 48 sidebar icons for every unit, building and superpower.
- `art/effects/` holds explosions, smoke, sparks, muzzle flashes, projectiles, craters, scorch marks and rubble.
  The player draws the detailed effects in `art/sprites/effects/` instead where they exist (below).
- `theme/theme.json` indexes the UI skin: nine-slice frames for the rail, panels, wells (`inset`), tooltips and the
  factory tabs (closed, open), buttons in four states (normal, hover, pressed, disabled), 23 cursors at 32 and 64 px
  with their hotspots (`theme/cursors/`), and an emblem per faction as SVG and PNG (`theme/emblems/`). `theme.css`
  holds the colours.
- `theme/fonts/` holds two open-licence (SIL OFL 1.1) faces baked into glyph atlases by `art/fonts/bake.py`: Inter for
  body text and Oxanium for headings and numbers, each style at UI scales 1 to 3, with their licences beside them.

Faction colour: pixels in the four `remap` colours in `art.json` (magenta shades) are swapped, by exact value, for
the four shades of the owning faction's ramp. The generator refuses any drawing that blends over a key colour.

The detailed effects (`art/sprites/effects-0.png` and `art/sprites/effects/<id>.json`: explosions, smoke and dust
puffs, sparks, muzzle flashes facing 16 ways, shells, rockets and a looping fire) are drawn from code too, by
`python3 art/effects/effects.py` at the studio's scale; see `art/README.md`, "Effects". Craters, scorch marks and
rubble are still the placeholders.

Not drawn yet: wrecks, building animations, a logo and a menu background.

## Audio

Everything under `audio/` is synthesised from code in `crates/classic-tools/src/sound.rs` (plain tones, noise and
sweeps; nothing sampled from anywhere) and written by `cargo run --bin sounds`. Don't edit the WAVs by hand:
change the recipe and rerun it. A test fails when the files and the recipes disagree.

- `audio/sounds.json` lists the files for each sound id; an id with several takes picks one at random each time.
- `audio/sfx/` holds weapons (cannon, rockets, machine guns and rifles), impacts, explosions, a soldier falling, the
  hazard's sounds, the building thud and a unit leaving each kind of factory; `audio/ui/` holds interface sounds.
- `audio/loops/` holds the loops played while units work: light, heavy and aircraft engines, a harvester mining and
  one unloading. Each is crossfaded end into start so it goes round without a click.
- `audio/provenance.jsonl` records each file's source.
- `audio/music/` and `audio/music.json` are the placeholder music, composed and synthesised by
  `crates/classic-tools/src/music.rs` (written by the same `cargo run --bin sounds`): a menu piece, two calm, two
  battle, and won and lost stingers, in 4-bit IMA ADPCM WAV (a quarter of PCM's size). `audio/music/provenance.jsonl`
  records each. A pack with its own `audio/music.json` replaces this music whole.

Which events play which id, and each id's bus, level and limits, are the engine's (`data/audio/`). Another pack
supplies its own files the same way, in its own `audio/sounds.json`; ids it leaves out play these.

Voices are the exception: `audio/voices/` and `audio/voices.json` are spoken by `audio/make_voices.py` with a free
text-to-speech model (Kokoro-82M, Apache-2.0, run locally), as placeholders. They speak the engine's own lines, the
feed's messages (`data/ui/messages.json`, in the advisor's own phrasing, since a voice can't say `{name}`) and the
units' replies (`data/ui/lines.json`), with one cast for every faction. They only play a line a pack left in the
engine's words, and a pack with voices of its own replaces them whole. `audio/voices/provenance.jsonl` records each
take and its text. After changing those lines, rerun the script (see its header).

Not made yet: real recordings of any of it.
