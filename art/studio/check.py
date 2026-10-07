"""Check rendered entities against the house rules (art-pipeline.md, "How we'll check it"; the asset plan's
style guide). Exits non-zero when any check fails; warnings don't fail.

    python3 art/studio/check.py RENDER_DIR [RENDER_DIR ...]
    python3 art/studio/check.py --facing-test        # renders a test arrow through the pipeline (about 30 s)

Per entity:
  frames     every frame meta.json lists exists, with its shadow
  leak       the magenta re-render (check-leak.png) has no pixels left in the team hue band: no other material
             would be recoloured
  coverage   team paint covers 5% to 40% of the visible pixels of the intact frames
  scale      the model's length at facing 0 is within 5% of studio.json's reference size, where it has one
  footprint  a building's ground contact stays the inset (0.5 m) inside its footprint (not for DECAL models)
  height     a building's tallest part stays under 1.5 tiles (warning)
  black      large patches of pure black, the mark of two faces sharing a plane (warning)
"""
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import pack  # noqa: E402

STUDIO = pack.STUDIO
HERE = Path(__file__).parent


class Report:
    def __init__(self):
        self.fails, self.lines = 0, []

    def add(self, eid, name, ok, detail, warn=False):
        tag = "ok" if ok else ("WARN" if warn else "FAIL")
        if not ok and not warn:
            self.fails += 1
        self.lines.append(f"{tag:5} {eid:18} {name:10} {detail}")


def in_band(rgba):
    tp = STUDIO["team_paint"]
    rgb = rgba[..., :3]
    mx, mn = rgb.max(-1), rgb.min(-1)
    sat = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    hue = pack.hue_of(rgb)
    lo, hi = tp["hue_band"]
    return (hue >= lo) & (hue <= hi) & (sat > tp["min_saturation"]) & (rgba[..., 3] > 0.5)


def check_entity(rdir, r):
    meta = json.loads((rdir / "meta.json").read_text())
    eid = meta["id"]
    if meta["kind"] == "icon":
        return
    missing = []
    for job in meta["jobs"]:
        for f in range(job["facings"]):
            for n in range(job["frames"]):
                p = pack.image_path(rdir, job, f, n)
                if not p.exists():
                    missing.append(p.name)
                elif job.get("shadow") and not p.with_suffix(".shadow.png").exists():
                    missing.append(p.with_suffix(".shadow.png").name)
    r.add(eid, "frames", not missing, f"{len(missing)} missing, e.g. {missing[0]}" if missing else
          f"{sum(j['facings'] * j['frames'] for j in meta['jobs'])} frames")

    leak = rdir / "check-leak.png"
    if leak.exists():
        img = pack.load(leak)
        n = int(in_band(img).sum())
        opaque = int((img[..., 3] > 0.5).sum())
        r.add(eid, "leak", n <= opaque * 0.0005, f"{n} of {opaque} pixels in the team band with paint swapped out")
    else:
        r.add(eid, "leak", False, "no check-leak.png", warn=True)

    base = [j for j in meta["jobs"] if j["anim"] == "idle" and j["part"] in ("hull", "turret", "building", "body")
            and not j.get("overlay")]
    paint = seen = 0.0
    dark = opaque = 0
    for job in base:
        for f in range(job["facings"]):
            p = pack.image_path(rdir, job, f, 0)
            if not p.exists():
                continue
            rgba, m = pack.team_mask(pack.load(p))
            a = rgba[..., 3]
            paint += float((m * a).sum())
            seen += float(a.sum())
            solid = a > 0.9
            dark += int((solid & (rgba[..., :3].max(-1) < 0.012)).sum())
            opaque += int(solid.sum())
    if seen and meta.get("team", True):
        lo, hi = STUDIO["team_coverage"]
        c = paint / seen
        r.add(eid, "coverage", lo <= c <= hi, f"team paint {c:.1%} of visible pixels (want {lo:.0%} to {hi:.0%})")
        r.add(eid, "black", dark <= opaque * 0.02, f"{dark / max(opaque, 1):.1%} pure black (coplanar faces?)",
              warn=True)

    b = meta["bounds_m"]
    ref = STUDIO["reference_sizes_m"].get(eid)
    if isinstance(ref, (int, float)) and meta["kind"] != "building":
        length = meta.get("length_m", b["max"][1] - b["min"][1])
        r.add(eid, "scale", abs(length / ref - 1) <= 0.05, f"{length:.2f} m long, reference {ref} m")

    if meta["kind"] == "building":
        T = STUDIO["metres_per_tile"]
        W, H = meta["footprint"][0] * T, meta["footprint"][1] * T
        inset = STUDIO["footprint_inset_m"]
        c = meta.get("contact_m")
        if meta.get("decal"):
            r.add(eid, "footprint", True, "decal: may fill its footprint (slabs), inset not checked")
        elif c:
            margins = [c[0], W - c[2], H + c[1], -c[3]]  # west, east, south, north, in metres
            ok = min(margins) >= inset - 1e-3
            r.add(eid, "footprint", ok, "ground contact inset W {:.2f} E {:.2f} S {:.2f} N {:.2f} m (want >= {})"
                  .format(*margins, inset))
        top = b["top"] / T
        lim = STUDIO["max_building_height_tiles"]
        r.add(eid, "height", top <= lim, f"{b['top']:.1f} m = {top:.2f} tiles (want under {lim})", warn=True)


def facing_test():
    """Render the arrow through render.py and pack.py, then check each packed facing's centroid points the way
    its index says, clockwise from north, never mirrored."""
    with tempfile.TemporaryDirectory() as tmp:
        rdir, sprites = Path(tmp) / "arrow", Path(tmp) / "sprites"
        subprocess.run([sys.executable, str(HERE / "render.py"), str(HERE / "samples" / "arrow.py"), "--out",
                        str(rdir), "--samples", "4", "--no-wreck", "--no-log"], check=True,
                       stdout=subprocess.DEVNULL)
        subprocess.run([sys.executable, str(HERE / "pack.py"), str(rdir), "--out", str(sprites)], check=True,
                       stdout=subprocess.DEVNULL)
        doc = json.loads((sprites / "units" / "arrow.json").read_text())
        atlas = pack.load(sprites / "units-0.png")
        frames = doc["parts"]["hull"]["anims"]["idle"]["frames"]
        worst = 0.0
        for i, (x, y, w, h, px, py) in enumerate(frames):
            a = atlas[y:y + h, x:x + w, 3]
            ys, xs = np.mgrid[0:h, 0:w]
            cx, cy = (a * xs).sum() / a.sum() - px, (a * ys).sum() / a.sum() - py
            # Screen y grows down; with the packer's stretch the ground is square, so this is the true bearing.
            got = math.degrees(math.atan2(cx, -cy)) % 360
            want = 360 * i / len(frames)
            worst = max(worst, abs((got - want + 180) % 360 - 180))
        r = Report()
        r.add("arrow", "facings", worst < 360 / len(frames) / 2,
              f"{len(frames)} facings, worst bearing error {worst:.1f} degrees")
        return r


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    if args == ["--facing-test"]:
        r = facing_test()
    else:
        r = Report()
        for d in args:
            check_entity(Path(d), r)
    print("\n".join(r.lines))
    sys.exit(1 if r.fails else 0)


if __name__ == "__main__":
    main()
