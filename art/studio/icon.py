"""Render one entity's build icon with the portrait camera (art-pipeline.md section 9): 35 degrees up, turned 30
degrees to the right, on a plain dark backdrop. pack.py shrinks it to every size in studio.json's portrait
"sizes" (96x72 and 192x144) on the icons page, team paint masked like every sprite.

    python3 art/studio/icon.py vehicles/battle_tank --out RENDER_DIR [--samples N]
"""
import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import render as rd  # noqa: E402
import rts_studio as st  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("model")
    ap.add_argument("--out", required=True)
    ap.add_argument("--samples", type=int, default=st.STUDIO["portrait"]["samples"])
    ap.add_argument("--style", default="detailed")
    ap.add_argument("--models", action="append", default=[], help="a pack's model folder, as for render.py")
    ap.add_argument("--pack", help="a private pack by name, as for render.py")
    args = ap.parse_args()
    if args.pack:
        args.models.insert(0, str(rd.pack_models(args.pack)))
    mod, category, eid, path = rd.load(args.model, rd.model_roots(args.models))
    if category not in rd.KINDS:
        category = getattr(mod, "CATEGORY", "vehicles")
    kind = rd.KINDS[category]
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    ctx = rd.build_scene(mod, kind, eid, args.style, args.samples, 0, "intact")
    scene = ctx["scene"]
    scene.render.film_transparent = False
    if kind == "infantry":
        st.pose(ctx["joints"], "idle", 0, 1)
    p = st.STUDIO["portrait"]
    st.backdrop(scene)
    turn = p["turn_deg"]
    if kind != "building":
        # Units face north, away from the camera: turn them round so the icon shows the front three-quarters,
        # keeping the studio's light. Buildings already face the camera (doors south).
        import math
        ctx["root"].rotation_euler[2] = math.pi - math.radians(turn)
        turn = 0
    import bpy
    bpy.context.view_layer.update()
    st.portrait_camera(scene, st.bounds(ctx["root"]), tuple(p["render_size"]), turn)
    rd.render(scene, out / "icon.png")
    meta = {"id": eid, "category": "icons", "kind": "icon", "source": category, "style": args.style,
            "render_size": p["render_size"], "model": str(path),
            "external": rd.external(path), "seconds": round(time.time() - t0, 1)}
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    print(f"{eid}: icon {meta['seconds']} s")


if __name__ == "__main__":
    main()
