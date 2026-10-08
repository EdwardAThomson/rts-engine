# The generic pack

Plain names and placeholder art for every generic id, so the engine runs and its tests pass on its own. It has two
factions on purpose: nothing in the engine may assume three. Its art may only ever be our own and generic: if
someone could name the game a picture comes from, it belongs in a private pack instead.

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
- `theme/theme.json` indexes the UI skin: nine-slice frames for the rail, panels, wells (`inset`), tooltips and the
  factory tabs (closed, open), buttons in four states (normal, hover, pressed, disabled), 23 cursors at 32 and 64 px
  with their hotspots (`theme/cursors/`), and an emblem per faction as SVG and PNG (`theme/emblems/`). `theme.css`
  holds the colours.
- `theme/fonts/` holds two open-licence (SIL OFL 1.1) faces baked into glyph atlases by `art/fonts/bake.py`: Inter for
  body text and Oxanium for headings and numbers, each style at UI scales 1 to 3, with their licences beside them.

Faction colour: pixels in the four `remap` colours in `art.json` (magenta shades) are swapped, by exact value, for
the four shades of the owning faction's ramp. The generator refuses any drawing that blends over a key colour.

Not drawn yet: wrecks, building animations, a logo and a menu background.

## Audio

Everything under `audio/` is synthesised from code in `crates/classic-tools/src/sound.rs` (plain tones, noise and
sweeps; nothing sampled from anywhere) and written by `cargo run --bin sounds`. Don't edit the WAVs by hand:
change the recipe and rerun it. A test fails when the files and the recipes disagree.

- `audio/sounds.json` lists the files for each sound id; an id with several takes picks one at random each time.
- `audio/sfx/` holds weapons, impacts, explosions and the building thud; `audio/ui/` holds interface sounds.
- `audio/provenance.jsonl` records each file's source.

Which events play which id, and each id's bus, level and limits, are the engine's (`data/audio/`). Another pack
supplies its own files the same way, in its own `audio/sounds.json`; ids it leaves out play these.

Not made yet: music, unit replies, advisor lines and looping sounds.
