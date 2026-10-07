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
    "blue": [(16, 32, 74), (32, 74, 144), (60, 120, 208), (144, 192, 255)],
    "red": [(74, 16, 16), (144, 32, 32), (208, 64, 48), (255, 154, 128)],
    "yellow": [(74, 58, 8), (144, 112, 16), (208, 170, 32), (255, 240, 128)],
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
