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
        atlas, mask = pages[a]
        base = "hull" if "hull" in doc["parts"] else "building"
        n = doc["parts"][base]["facings"]
        for team in RAMPS:
            row = []
            step = max(1, n // 8)
            for i in range(0, n, step):
                layers = []
                if "shadow" in doc["parts"][base]:
                    layers.append(sprite(atlas, None, doc["parts"][base]["shadow"]["frames"][i], None, shadow=True))
                layers.append(sprite(atlas, mask, doc["parts"][base]["anims"]["idle"]["frames"][i], team))
                if "turret" in doc["parts"]:
                    t = doc["parts"]["turret"]
                    k = i * t["facings"] // n
                    layers.append(sprite(atlas, mask, t["anims"]["idle"]["frames"][k], team))
                row.append(layers)
            cells.append(row)
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
