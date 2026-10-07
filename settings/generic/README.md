# The generic pack

Plain names and placeholder art for every generic id, so the engine runs and its tests pass on its own. It has two
factions on purpose: nothing in the engine may assume three. Its art may only ever be our own and generic: if
someone could name the game a picture comes from, it belongs in a private pack instead.

## Art

Everything under `art/` and `theme/` is drawn from code in `crates/classic-tools/src/art` and written by
`cargo run --bin art`. Don't edit the PNGs by hand: change the drawing and rerun it. A test fails when the files
and the drawings disagree. `provenance.jsonl` records each file's source.

- `art/art.json` indexes everything: frame sizes, frame counts and facings for each file.
- `art/units/` and the turrets in `art/buildings/` are strips of eight 32 px frames, facing north first and then
  turning clockwise.
- `art/buildings/` is one frame per building, sized to its default footprint in the rules data at 32 px per tile.
- `art/terrain/` has four 32 px variants of each ground tile. `art/features/` holds things drawn over the ground.
- `art/icons/` holds 64 x 48 sidebar icons for every unit, building and superpower.
- `art/effects/` holds explosions, smoke, sparks, muzzle flashes, projectiles, craters, scorch marks and rubble.
- `theme/` has a nine-slice panel frame, a three-state button and `theme.css` colours.

Faction colour: pixels in the four `remap` colours in `art.json` (magenta shades) are swapped, by exact value, for
the four shades of the owning faction's ramp. The generator refuses any drawing that blends over a key colour.

Not drawn yet: blends between terrain kinds, wrecks, building animations, cursors and fonts.
