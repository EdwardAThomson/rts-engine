# Art studio

Pre-rendered sprites for setting packs, made the way `plans/rts/art-pipeline.md` (playbooks repository) lays out:
models written in Python, rendered with Blender's Python module, then packed into atlases for the renderer.
Every model here is our own and generic. A model anyone could recognise from a particular game goes in a private
pack's own `art/models/`, never here.

```bash
python3 -m pip install bpy numpy pillow                    # Blender as a Python module; Cycles on the CPU
python3 art/studio/render.py vehicles/battle_tank --out /tmp/renders/battle_tank
python3 art/studio/render.py buildings/refinery --out /tmp/renders/refinery
python3 art/studio/icon.py vehicles/battle_tank --out /tmp/renders/icon-battle_tank
python3 art/studio/check.py /tmp/renders/*                 # house rules; exits non-zero on a failure
python3 art/studio/pack.py /tmp/renders/* --out settings/generic/art/sprites
python3 art/studio/preview.py settings/generic/art/sprites /tmp/preview.png
python3 art/studio/check.py --facing-test                  # the pipeline's facing order, with a test arrow
```

Quick test of one facing while modelling: `render.py MODEL --out DIR --samples 16 --facings 16 --only 2`, and
`--jobs building/idle` (or `hull/idle`) to skip the other frames.

## Models from a private pack

The private repository (cloned into the git-ignored `settings-private/`) keeps each pack's models in
`art/<pack>/models/<category>/<id>.py`, written to the same contract. `--pack <name>` adds that folder; `--models
DIR` (repeatable) or `RTS_ART_MODELS` (separated like `PATH`) add any other. They are searched before this
repository's `art/models/`, so a pack's model replaces the generic one with the same id. Models import
`rts_studio` as usual.

```bash
python3 art/studio/render.py vehicles/super_a --pack <pack> --out /tmp/renders/super_a
python3 art/studio/pack.py /tmp/renders/super_a ... --out settings-private/packs/<pack>/art/sprites
```

Renders record where their model came from. `pack.py` refuses to write a pack's renders anywhere in this
repository except under `settings-private/`, and their render times stay out of `art/timings.jsonl`, so nothing
private lands here. Pack a private pack's renders on their own (atlas pages are per output folder), together with
any generic renders it reuses.

## Files

- `studio/studio.json`: the camera (orthographic, facing north, 60°), the scale (64 atlas pixels per 10.67 m
  tile, rendered at 4×, or per category in `render_scale_by_category`), light, samples, facings per kind, team
  paint, reference sizes, the footprint inset, the atlas pages and the portrait camera.
- `studio/rts_studio.py`: scene, lights, materials, part helpers (`block`, `cylinder`, `cone`, `sphere`,
  `wedge`, `beam`, `group`), the frame helpers (`default_damage`, `default_wreck`, `construction`, `rubble`,
  `scorched`, `burnt`), the infantry rig (`rig`, `pose`) and the portrait camera.
- `studio/render.py`: renders every frame a model asks for (below), with shadow passes, into a render folder with
  `meta.json`, and appends the job's time to `art/timings.jsonl`.
- `studio/icon.py`: the build icon from the portrait camera (35° up, turned 30°, dark backdrop).
- `studio/pack.py`: masks team paint and makes it neutral grey, downscales, stretches by 1/sin 60° so footprints
  are square on screen, trims, and packs one 4096 × 4096 page per category (`units-0`, `buildings-0`,
  `infantry-0`, `air-0`, `effects-0`, `icons-0`) with a mask page each. It stops, naming the largest entities, if
  a page would overflow.
- `studio/check.py`: the checks below.
- `studio/preview.py`: draws the packed sprites back in three team colours, plus a row of every other frame.
- `studio/samples/`: the studio's own test models (the facing arrow, a building, a defence turret, a wall) and the
  infantry prototype `soldier.py` on the rig. None is a game asset.
- `jobs.jsonl`: one line per asset in the plan, with its batch, frames and status. `timings.jsonl`: every render's
  real time, so the plan's estimates can be replaced with measurements.

## The model contract

Models face north (+y) and sit on the ground at the origin; buildings put the footprint's north-west corner at the
origin, east along +x and south along -y. Everything below except `build`/`build_hull` and `FOOTPRINT` is
optional: a model that leaves a hook out gets the studio's default.

| Name | Kind | What the studio does with it |
|---|---|---|
| `build_hull(root)` | vehicle, aircraft | Hull: 16 facings, with a shadow |
| `build_turret(ring)`, `TURRET_HEIGHT` | vehicle, aircraft | Turret: 32 facings about the ring, no shadow of its own |
| `DETAIL = {names}` | all | Parts the `classic` style leaves out |
| `ANIMS = {anim: frames}` and `pose(root, anim, frame)` | vehicle, aircraft | Extra animations of the hull at every facing (`{"mine": 4}`); a dict value `{"frames": 3, "part": "turret"}` picks the part, and `"idle"` sets the idle loop's length (gunship rotors) |
| `wreck(rng, root)` | vehicle | The wreck, 8 facings with a shadow; default `st.default_wreck` (burnt, turret askew, detail gone) |
| `FOOTPRINT = (w, h)`, `build(root)` | building | Intact frame with a shadow, 4 construction frames (clipped at 0, 35, 70 and 100% of the height, scaffold on the first three), 1 damaged frame |
| `damage(rng, root)` | building | The damaged frame; default `st.default_damage` (soot on a third of the parts, two or three small parts gone, one bent) |
| `IDLE_PARTS = {names}`, `IDLE_FRAMES = 8`, `idle_pose(root, t)` | building | Overlay `overlay-idle`: only those parts, the rest of the building cutting them, in a render border; default pose turns each part once round its own z over the loop. Group a moving part under an empty placed at its pivot |
| `DOORS = {names}`, `DOOR_FRAMES = 3`, `door_pose(root, t)` | building | Overlay `overlay-doors`: shut plus `DOOR_FRAMES` steps open; default pose rolls each door part up into its top edge |
| `build_head(ring)`, `HEAD_HEIGHT` | building | A defence turret's head: 32 facings about the footprint's centre |
| `JOINS = True`, `build(root, joins)` | building | Walls: 16 frames, one per join mask (1 north, 2 east, 4 south, 8 west), intact and damaged |
| `DECAL = True` | building | One frame, no shadow (rubble) |
| `TEAM = False` | all | No team paint expected (walls, rubble): the coverage check is skipped |
| `build(root, joints)` | infantry | Parts parented to the rig's joints; idle 1, walk 6, fire 3 at 8 facings, and die-1, die-2 of 8 frames from one side |

`rng` is a `random.Random` seeded from the id and the frame, so every render of a hook is the same. A moving part
(idle or door) is left out of the intact frame and drawn every frame as its overlay, so nothing shows twice.
Overlays are drawn only on the intact building; construction and damaged frames include every part.

Two rules the checks have caught: **faces must never share a plane** (Cycles draws coplanar faces black; lift a
roof or band 2 cm), and **ground contact stays 0.5 m inside the footprint**.

## Output

Render folders hold `<part>-<anim>-fFF-NN.png` (FF the facing clockwise from north, NN the frame) and
`.shadow.png` beside each frame that casts one. `pack.py` writes per entity, in the format `plans/rts/renderer.md`
gives:

```json
{"id": "battle_tank", "atlas": "units-0", "facings": 32, "scale": 2,
 "parts": {"hull": {"facings": 16, "anims": {"idle": {"length": 1, "frames": [[x, y, w, h, px, py], ...],
                                                    "shadow": [[...], ...]}}},
           "turret": {"facings": 32, "pivotOffset": [0, 0], "anims": {"idle": {...}}},
           "wreck": {"facings": 8, "anims": {"idle": {...}}}}}
```

Frame `k` of facing `f` is entry `f * length + k`. An anim with its own `facings` (infantry deaths: 1) says so.
Parts with `"overlay": true` are drawn over the building's intact frame with the same pivot. Buildings have a
`building` part with `idle`, `build` (4) and `damaged` anims. Icons live in `icons/<id>.json` with `1x` and `2x`
frames.

## Checks

`check.py RENDER_DIR...`: every listed frame exists; no other material lands in the team hue band (a
re-render with team paint swapped to magenta); team paint covers 5% to 40% of the intact frames; vehicle hull
length within 5% of `reference_sizes_m`; building ground contact inside the footprint by the inset; building height
under 1.5 tiles and no large pure-black patches (warnings). `check.py --facing-test` renders a test arrow through
`render.py` and `pack.py` and checks each facing's bearing.

## Styles

`studio.json` lists styles: `detailed` (the default) and `classic` (flat colours, direct light only, small parts a
model lists in `DETAIL` left out, chunky 2× pixels, a 48-colour palette and hard edges). Render with
`--style classic`; the packer keeps one style per atlas page. Every style has the same scale, pivots and frame
layout, so a renderer can offer them as a player option.

Renders are not committed; the packed atlases and JSON are. Cycles output can differ slightly between machines, so
re-rendering may change the PNGs' bytes without changing how they look.
