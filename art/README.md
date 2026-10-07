# Art studio

Pre-rendered sprites for setting packs, made the way `plans/rts/art-pipeline.md` (playbooks repository) lays out:
models written in Python, rendered with Blender's Python module, then packed into atlases for the renderer.
Every model here is our own and generic. A model anyone could recognise from a particular game goes in a private
pack's own `art/models/`, never here.

```bash
python3 -m pip install bpy numpy pillow                    # Blender as a Python module; Cycles on the CPU
python3 art/studio/render.py vehicles/battle_tank --out /tmp/renders/battle_tank
python3 art/studio/render.py vehicles/harvester --out /tmp/renders/harvester
python3 art/studio/render.py buildings/refinery --out /tmp/renders/refinery
python3 art/studio/pack.py /tmp/renders/* --out settings/generic/art/sprites
python3 art/studio/preview.py settings/generic/art/sprites /tmp/preview.png
```

**Styles.** `studio.json` lists styles: `detailed` (the default) and `classic` (flat colours, direct light only,
small parts a model lists in `DETAIL` left out, chunky 2x pixels, a 48-colour palette and hard edges). Render
with `--style classic`; the packer reads the style from each render and keeps one style per atlas page. Every
style has the same scale, pivots and frame layout, so a renderer can offer them as a player option.

Quick test of one facing while modelling: `render.py MODEL --out DIR --samples 16 --facings 16 --only 2`.

- `studio/studio.json`: the camera (orthographic, facing north, 60°), the scale (64 atlas pixels per 10.67 m
  tile, rendered at 4×), light, samples, facings (16 for hulls, 32 for turrets) and the team paint colour.
- `studio/rts_studio.py`: scene, lights, materials and part helpers. Helpers keep transforms on objects, so a
  turret is a group that turns about its ring.
- `models/vehicles/<id>.py`: `build_hull(root)`, and `build_turret(ring)` with `TURRET_HEIGHT` for turreted ones.
  Models face north (+y) and sit on the ground at the origin.
- `models/buildings/<id>.py`: `FOOTPRINT = (w, h)` matching the rules data, and `build(root)` with the footprint's
  north-west corner at the origin (east is +x, south is -y).
- `studio/pack.py`: masks team paint and makes it neutral grey, then downscales, stretches by 1/sin 60° so
  footprints are square on screen, trims, and packs `units-0` and `buildings-0` pages with a mask page each.
  It writes one `<category>/<id>.json` per entity in the format `plans/rts/renderer.md` gives.
- `studio/preview.py`: draws the packed sprites back in three team colours, as a check of the data.

Renders are not committed; the packed atlases and JSON are. Cycles output can differ slightly between machines, so
re-rendering may change the PNGs' bytes without changing how they look.
