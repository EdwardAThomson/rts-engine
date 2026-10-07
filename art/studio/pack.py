"""Pack rendered frames into atlases for the renderer (plans/rts/renderer.md, "Data"; art-pipeline.md section 2).

    python3 art/studio/pack.py RENDER_DIR [RENDER_DIR ...] --out settings/generic/art/sprites [--preview PNG]

Each RENDER_DIR is one entity rendered by render.py. For every frame this:
  1. finds team paint (the studio's green hue band), writes its strength to a mask and turns it neutral grey,
     scaled so the entity's typical paint sits mid-ramp, so the renderer can paint any player's ramp over it;
  2. downscales by the render scale, in premultiplied alpha, and stretches vertically by 1/sin(60), so the
     ground comes out square and footprints match tiles;
  3. trims to content plus a margin and records the pivot: the ground point under a unit's origin, or a
     building footprint's north-west corner.
Then it shelf-packs every frame of a category into one page, writing <atlas>.png, <atlas>.mask.png and
<category>/<id>.json with frames as [x, y, w, h, pivotX, pivotY]. Output depends only on the renders.
"""
import argparse
import colorsys
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
STUDIO = json.loads((Path(__file__).parent / "studio.json").read_text())
ATLAS = {"vehicles": "units-0", "buildings": "buildings-0"}
CATEGORY = {"vehicles": "units", "buildings": "buildings"}
PAGE_WIDTH = 2048


def load(path):
    return np.asarray(Image.open(path).convert("RGBA"), dtype=np.float32) / 255


def team_mask(rgba):
    """Strength of team paint per pixel, from hue and saturation, and the image with that paint made grey."""
    tp = STUDIO["team_paint"]
    rgb = rgba[..., :3]
    mx, mn = rgb.max(-1), rgb.min(-1)
    sat = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    hsv = np.vectorize(lambda r, g, b: colorsys.rgb_to_hsv(r, g, b)[0], otypes=[np.float32])
    hue = hsv(rgb[..., 0], rgb[..., 1], rgb[..., 2])
    lo, hi = tp["hue_band"]
    s0 = tp["min_saturation"]
    mask = ((hue >= lo) & (hue <= hi)) * np.clip((sat - s0) / s0, 0, 1) * (rgba[..., 3] > 0)
    return rgba, mask.astype(np.float32)


def neutralise(frames, target=0.5):
    """Turn team paint grey, scaled so the entity's typical paint brightness lands at `target`, the middle of a
    player's ramp. One scale per entity keeps its shading (lit and shadowed sides) intact."""
    lum = np.array([0.299, 0.587, 0.114], np.float32)
    paint = np.concatenate([(f[..., :3] @ lum)[m > 0.9] for f, m in frames])
    k = target / max(float(np.median(paint)), 1e-3) if len(paint) else 1.0
    out = []
    for f, m in frames:
        grey = np.clip((f[..., :3] @ lum) * k, 0, 1)[..., None]
        g = f.copy()
        g[..., :3] = f[..., :3] * (1 - m[..., None]) + grey * m[..., None]
        out.append((g, m))
    return out


def resize(arr, size):
    """Resize RGBA (premultiplied while filtering) or a single channel."""
    if arr.ndim == 2:
        return np.asarray(Image.fromarray(arr.astype(np.float32), "F").resize(size, Image.LANCZOS), np.float32)
    pm = arr.copy()
    pm[..., :3] *= pm[..., 3:4]
    chans = [np.asarray(Image.fromarray(pm[..., c], "F").resize(size, Image.LANCZOS), np.float32) for c in range(4)]
    out = np.clip(np.stack(chans, -1), 0, 1)
    a = out[..., 3:4]
    out[..., :3] = np.where(a > 1e-4, out[..., :3] / np.maximum(a, 1e-4), 0)
    return np.clip(out, 0, 1)


def shrink(rgba, mask, origin, style):
    """Downscale to atlas size. A style with pixel_size 2 shrinks to half that and doubles back with nearest
    neighbour, for chunky pixels at the same atlas scale; hard_alpha cuts edges to on or off."""
    rs, st = STUDIO["render_scale"], STUDIO["vertical_stretch"]
    px = style["pixel_size"]
    h, w = rgba.shape[:2]
    size = (max(1, round(w / rs / px)), max(1, round(h * st / rs / px)))
    small = resize(rgba, size)
    m = None
    if mask is not None:
        m = np.clip(resize(mask * rgba[..., 3], size) / np.maximum(small[..., 3], 1e-4), 0, 1) * (small[..., 3] > 0)
    if style["hard_alpha"]:
        keep = small[..., 3] >= 0.5
        small[..., 3] = keep
        if m is not None:
            m = np.where(keep, (m >= 0.5).astype(np.float32), 0)
    if px > 1:
        small = small.repeat(px, 0).repeat(px, 1)
        m = None if m is None else m.repeat(px, 0).repeat(px, 1)
    return small, m, (origin[0] * size[0] * px / w, origin[1] * size[1] * px / h)


def trim(rgba, mask, pivot):
    margin = STUDIO["margin_px"]
    ys, xs = np.nonzero(rgba[..., 3] > 1 / 255)
    if len(xs) == 0:
        return rgba[:1, :1], None if mask is None else mask[:1, :1], [0, 0]
    x0, x1 = max(0, xs.min() - margin), min(rgba.shape[1], xs.max() + 1 + margin)
    y0, y1 = max(0, ys.min() - margin), min(rgba.shape[0], ys.max() + 1 + margin)
    crop = rgba[y0:y1, x0:x1]
    return crop, None if mask is None else mask[y0:y1, x0:x1], [round(pivot[0] - x0), round(pivot[1] - y0)]


def frames_of(rdir, meta):
    """Every frame of an entity: (part, kind, index, rgba, mask, pivot); kind is 'image' or 'shadow'."""
    style = STUDIO["styles"][meta.get("style", "detailed")]
    names = [(part, i) for part, info in meta["parts"].items() for i in range(info["facings"])]
    masked = neutralise([team_mask(load(rdir / f"{p}-idle-f{i:02d}-00.png")) for p, i in names])
    out = []
    for (part, i), (rgba, mask) in zip(names, masked):
        small, m, piv = shrink(rgba, mask, meta["origin_px"], style)
        out.append((part, "image", i, *trim(small, m, piv)))
        sp = rdir / f"{part}-idle-f{i:02d}-00.shadow.png"
        if sp.exists():
            sh = load(sp)
            sh[..., :3] = 0
            small, _, piv = shrink(sh, None, meta["origin_px"], style)
            out.append((part, "shadow", i, *trim(small, None, piv)))
    return out


def pack(frames):
    """Shelf-pack frames, tallest first, ties in input order; returns (x, y) per frame and the page size."""
    order = sorted(range(len(frames)), key=lambda k: (-frames[k].shape[0], k))
    pos, x, y, shelf = [None] * len(frames), 0, 0, 0
    for k in order:
        h, w = frames[k].shape[:2]
        if x + w > PAGE_WIDTH:
            x, y, shelf = 0, y + shelf, 0
        pos[k] = (x, y)
        x += w
        shelf = max(shelf, h)
    return pos, (PAGE_WIDTH, y + shelf)


def palettise(page, colours):
    """Reduce a page's colours to one shared palette, as 90s games did. Team paint is already grey, so the
    palette never spends entries on player colours."""
    rgb = Image.fromarray((page[..., :3] * 255).round().astype(np.uint8), "RGB")
    q = rgb.quantize(colors=colours, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).convert("RGB")
    out = page.copy()
    out[..., :3] = np.asarray(q, np.float32) / 255
    return out


def write_page(path, frames, masks, pos, size, colours=0):
    page = np.zeros((size[1], size[0], 4), np.float32)
    mpage = np.zeros((size[1], size[0]), np.float32)
    for f, m, (x, y) in zip(frames, masks, pos):
        h, w = f.shape[:2]
        page[y:y + h, x:x + w] = f
        if m is not None:
            mpage[y:y + h, x:x + w] = m
    if colours:
        page = palettise(page, colours)
    Image.fromarray((page * 255).round().astype(np.uint8), "RGBA").save(path.with_suffix(".png"), optimize=True)
    Image.fromarray((mpage * 255).round().astype(np.uint8), "L").save(path.with_suffix(".mask.png"), optimize=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("renders", nargs="+")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    by_atlas = {}
    for rdir in map(Path, args.renders):
        meta = json.loads((rdir / "meta.json").read_text())
        by_atlas.setdefault(ATLAS[meta["category"]], []).append((meta, frames_of(rdir, meta)))

    for atlas, entities in sorted(by_atlas.items()):
        flat = [f for _, fs in entities for f in fs]
        pos, size = pack([f[3] for f in flat])
        styles = {m.get("style", "detailed") for m, _ in entities}
        assert len(styles) == 1, f"{atlas}: one style per page, got {sorted(styles)}"
        write_page(out / atlas, [f[3] for f in flat], [f[4] for f in flat], pos, size,
                   STUDIO["styles"][styles.pop()]["palette"])
        k = 0
        for meta, fs in entities:
            parts = {}
            for part, kind, i, img, _, piv in fs:
                x, y = pos[k]
                k += 1
                entry = parts.setdefault(part, {"facings": meta["parts"][part]["facings"],
                                                "anims": {"idle": {"frames": []}}})
                frame = [x, y, img.shape[1], img.shape[0], piv[0], piv[1]]
                if kind == "shadow":
                    entry.setdefault("shadow", {"frames": []})["frames"].append(frame)
                else:
                    entry["anims"]["idle"]["frames"].append(frame)
            if "turret" in parts:
                parts["turret"]["pivotOffset"] = [0, 0]
            doc = {"id": meta["id"], "atlas": atlas, "facings": max(p["facings"] for p in parts.values()),
                   "scale": 2, "parts": parts}
            if "footprint" in meta:
                doc["footprint"] = meta["footprint"]
            path = out / CATEGORY[meta["category"]] / f"{meta['id']}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(doc, separators=(", ", ": ")) + "\n")
        print(f"{atlas}: {len(flat)} frames, page {size[0]}x{size[1]}")


if __name__ == "__main__":
    main()
