"""Contact sheet of packed sprites, drawn the way the renderer will draw them: shadow, then hull, then turret, each
placed by its pivot, with team paint recoloured by luma along a ramp (renderer.md section 7). Reading the atlases
and JSON back is also a check that the packed data is complete.

    python3 art/studio/preview.py settings/generic/art/sprites OUT.png
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

RAMPS = {  # dark to light; preview only, the game's ramps live with the renderer
    "blue": [(8, 16, 48), (20, 48, 120), (40, 90, 200), (120, 170, 255)],
    "red": [(48, 8, 8), (120, 20, 16), (200, 40, 32), (255, 130, 110)],
    "yellow": [(48, 38, 4), (120, 92, 10), (200, 160, 20), (255, 230, 110)],
}
GROUND = (201, 166, 107)


def ramp(name, t):
    pts = np.array(RAMPS[name], np.float32) / 255
    x = np.clip(t, 0, 1) * (len(pts) - 1)
    i = np.minimum(x.astype(int), len(pts) - 2)
    f = (x - i)[..., None]
    return pts[i] * (1 - f) + pts[i + 1] * f


SHADOW_ALPHA = 0.45  # renderer.md layers.json: shadows are composited once at 45% black


def sprite(atlas, mask, frame, team, shadow=False):
    x, y, w, h, px, py = frame
    img = atlas[y:y + h, x:x + w].copy()
    if shadow:
        img[..., 3] *= SHADOW_ALPHA
    if team and mask is not None:
        m = mask[y:y + h, x:x + w, None]
        luma = img[..., :3] @ np.array([0.299, 0.587, 0.114], np.float32)
        img[..., :3] = img[..., :3] * (1 - m) + ramp(team, luma) * m
    return Image.fromarray((img * 255).round().astype(np.uint8), "RGBA"), (px, py)


def cells_for(doc, atlas, mask):
    """Rows of cells for one entity: its base part turning (8 facings) in each team colour, then one row of every
    other animation at facing 0, overlays drawn over the intact frame."""
    parts = doc["parts"]
    base = next(p for p in ("hull", "building", "body") if p in parts)
    bp = parts[base]
    n = bp["facings"]

    def frame(part, anim, f, k):
        a = parts[part]["anims"][anim]
        return (f % a.get("facings", parts[part]["facings"])) * a["length"] + k

    def view(f, team, anim="idle", k=0, extra=()):
        a = bp["anims"][anim]
        layers = []
        if "shadow" in a:
            layers.append(sprite(atlas, None, a["shadow"][frame(base, anim, f, k)], None, shadow=True))
        layers.append(sprite(atlas, mask, a["frames"][frame(base, anim, f, k)], team))
        if anim == "idle" and "turret" in parts:
            t = parts["turret"]
            layers.append(sprite(atlas, mask, t["anims"]["idle"]["frames"][f * t["facings"] // n], team))
        if anim == "idle":
            for name, p in parts.items():
                if p.get("overlay"):
                    an, ov = next(iter(p["anims"].items()))
                    kk = dict(extra).get(name, 0)
                    layers.append(sprite(atlas, mask, ov["frames"][kk], team))
        return layers

    rows = []
    for team in RAMPS:
        rows.append([view(i, team) for i in range(0, n, max(1, n // 8))])
    extra = []
    for anim, a in bp["anims"].items():
        if anim != "idle" or a["length"] > 1:
            facing = 2 * n // 8  # east, so walks and recoil read side on
            extra += [view(facing, "blue", anim, k) for k in range(a["length"])]
    for name, p in parts.items():
        if p.get("overlay"):
            an, ov = next(iter(p["anims"].items()))
            extra += [view(0, "blue", extra=((name, k),)) for k in range(ov["length"])]
        elif name not in (base, "turret"):
            for anim, a in p["anims"].items():
                step = max(1, p["facings"] // 8)
                extra += [[sprite(atlas, None, a["shadow"][f * a["length"]], None, shadow=True)] * ("shadow" in a) +
                          [sprite(atlas, mask, a["frames"][f * a["length"]], "blue")]
                          for f in range(0, p["facings"], step)]
    if extra:
        rows.append(extra)
    return rows


def main():
    root, out = Path(sys.argv[1]), sys.argv[2]
    pages = {}
    cells = []
    for doc_path in sorted(root.glob("*/*.json")):
        doc = json.loads(doc_path.read_text())
        a = doc["atlas"]
        if a not in pages:
            pages[a] = (np.asarray(Image.open(root / f"{a}.png").convert("RGBA"), np.float32) / 255,
                        np.asarray(Image.open(root / f"{a}.mask.png"), np.float32) / 255)
        if "icon" in doc["parts"]:
            continue
        cells += cells_for(doc, *pages[a])
    cell = max(max(max(im.width, im.height) for im, _ in layers) for row in cells for layers in row) + 8
    cols = max(len(r) for r in cells)
    sheet = Image.new("RGBA", (cols * cell, len(cells) * cell), GROUND + (255,))
    for r, row in enumerate(cells):
        for c, layers in enumerate(row):
            ox, oy = c * cell + cell // 3, r * cell + cell * 2 // 3
            for im, (px, py) in layers:
                sheet.alpha_composite(im, (ox - px, oy - py))
    sheet.save(out)
    print(sheet.size)


if __name__ == "__main__":
    main()
