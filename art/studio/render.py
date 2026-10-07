"""Render one entity's frames with the studio: every facing of each part, plus a shadow pass.

    python3 art/studio/render.py vehicles/battle_tank --out RENDER_DIR [--samples N] [--facings N] [--threads N]

A vehicle model (art/models/vehicles/<id>.py) defines build_hull(root) and, if it has one, build_turret(ring),
where `ring` sits TURRET_HEIGHT metres above the vehicle's origin, so turret frames share the hull's pivot. A
building model (art/models/buildings/<id>.py) defines FOOTPRINT = (w, h) in tiles and build(root), drawn with the
footprint's north-west corner at the origin, east along +x and south along -y.

Writes <part>-idle-fFF-00.png (and <part>-idle-fFF-00.shadow.png for hulls and buildings) and meta.json, which
says where the origin landed in render pixels. The packer (pack.py) does the rest.
"""
import argparse
import importlib.util
import json
import math
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402

ART = Path(__file__).parent.parent


def load(model):
    path = ART / "models" / f"{model}.py"
    spec = importlib.util.spec_from_file_location(model.replace("/", "."), path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def render(scene, path):
    scene.render.filepath = str(path)
    import bpy
    bpy.ops.render.render(write_still=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model", help="category/id under art/models, e.g. vehicles/battle_tank")
    ap.add_argument("--out", required=True)
    ap.add_argument("--samples", type=int, default=st.STUDIO["samples"])
    ap.add_argument("--facings", type=int, help="override the facing count (quick tests)")
    ap.add_argument("--only", type=int, help="render just this facing index (quick tests)")
    ap.add_argument("--threads", type=int, default=0)
    ap.add_argument("--style", default="detailed", help="a style from studio.json")
    args = ap.parse_args()

    mod = load(args.model)
    category, eid = args.model.split("/")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    scene = st.reset_scene()
    scene.cycles.samples = args.samples
    if args.threads:
        scene.render.threads_mode = "FIXED"
        scene.render.threads = args.threads
    st.lights(scene)
    root = st.group(eid)
    parts = {}
    if category == "buildings":
        mod.build(root)
        parts["building"] = st.meshes(root)
        kind = "building"
    else:
        mod.build_hull(root)
        hull = st.meshes(root)
        parts["hull"] = hull
        if hasattr(mod, "build_turret"):
            ring = st.group("turret", (0, 0, mod.TURRET_HEIGHT), parent=root)
            mod.build_turret(ring)
            parts["turret"] = st.meshes(ring)
            parts["hull"] = [o for o in hull]
        kind = "vehicle"

    st.apply_style(scene, root, args.style, getattr(mod, "DETAIL", ()))
    for part in parts:
        parts[part] = [o for o in parts[part] if not o.hide_viewport]
    b = st.bounds(root)
    el = math.radians(st.STUDIO["elevation_deg"])
    sx, sy = st.shadow_extent(b["top"])
    pad = 0.3
    if kind == "vehicle":
        r = b["radius"]
        left, right = r + pad, r + pad + sx
        up = r * math.sin(el) + b["top"] * math.cos(el) + pad
        down = (r + sy) * math.sin(el) + pad
    else:
        left, right = -b["min"][0] + pad, b["max"][0] + sx + pad
        up = b["max"][1] * math.sin(el) + b["top"] * math.cos(el) + pad
        down = (-b["min"][1] + sy) * math.sin(el) + pad
    size, origin = st.camera(scene, left, right, up, down)
    ground = st.shadow_ground(scene)

    facings = st.STUDIO["facings"]
    meta = {"id": eid, "category": category, "kind": kind, "style": args.style, "render_size": list(size), "origin_px": list(origin),
            "bounds_m": b, "parts": {}}
    if kind == "building":
        meta["footprint"] = list(mod.FOOTPRINT)
    t0 = time.time()
    for part, objs in parts.items():
        n = args.facings or facings[part]
        others = [o for p, os in parts.items() if p != part for o in os]
        for o in others:
            o.hide_render = True
        indices = [args.only] if args.only is not None else range(n)
        for i in indices:
            root.rotation_euler[2] = -2 * math.pi * i / n
            name = f"{part}-idle-f{i:02d}-00"
            ground.hide_render = True
            render(scene, out / f"{name}.png")
            if part in ("hull", "building"):
                # Shadow alone: the model hidden from the camera but still casting, the turret included.
                ground.hide_render = False
                hidden = [o for os in parts.values() for o in os]
                for o in others:
                    o.hide_render = False
                for o in hidden:
                    o.visible_camera = False
                keep = scene.cycles.samples
                scene.cycles.samples = st.STUDIO["shadow_samples"]
                render(scene, out / f"{name}.shadow.png")
                scene.cycles.samples = keep
                for o in hidden:
                    o.visible_camera = True
                for o in others:
                    o.hide_render = True
        for o in others:
            o.hide_render = False
        meta["parts"][part] = {"facings": n}
    meta["seconds"] = round(time.time() - t0, 1)
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    print(f"{eid}: render {size[0]}x{size[1]}, {meta['seconds']} s")


if __name__ == "__main__":
    main()
