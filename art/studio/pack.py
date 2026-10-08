"""Pack rendered frames into atlases for the renderer (plans/rts/renderer.md, "Data"; art-pipeline.md section 2).

    python3 art/studio/pack.py RENDER_DIR [RENDER_DIR ...] --out settings/generic/art/sprites

Each RENDER_DIR is one entity rendered by render.py. For every frame this:
  1. finds team paint (the studio's green hue band), writes its strength to a mask and turns it neutral grey,
     scaled so the entity's typical paint sits mid-ramp, so the renderer can paint any player's ramp over it;
  2. downscales by the render scale, in premultiplied alpha, and stretches vertically by 1/sin(60), so the
     ground comes out square and footprints match tiles;
  3. draws a dark outline round the body frames of the categories studio.json's `outline` names (not shadows,
     icons or building overlays), so units stand out on any ground;
  4. trims to content plus a margin and records the pivot: the ground point under a unit's origin, or a
     building footprint's north-west corner.
Then it shelf-packs every frame of a category into one page, writing <atlas>.png, <atlas>.mask.png and
<category>/<id>.json with frames as [x, y, w, h, pivotX, pivotY]. Output depends only on the renders.
"""
import argparse
import json
import os
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
STUDIO = json.loads((Path(__file__).parent / "studio.json").read_text())
ATLAS = STUDIO["atlas"]["pages"]
PAGE = STUDIO["atlas"]["page"]
SHADOW_FLOOR = 0.04
FOLDER = {"vehicles": "units", "buildings": "buildings", "infantry": "infantry", "aircraft": "air",
          "effects": "effects", "icons": "icons"}


def load(path):
    return np.asarray(Image.open(path).convert("RGBA"), dtype=np.float32) / 255


def hue_of(rgb):
    """colorsys.rgb_to_hsv's hue, for a whole image at once."""
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    mx, mn = rgb.max(-1), rgb.min(-1)
    d = np.maximum(mx - mn, 1e-12)
    h = np.where(mx == r, (g - b) / d, np.where(mx == g, 2 + (b - r) / d, 4 + (r - g) / d))
    return np.where(mx > mn, (h / 6) % 1.0, 0).astype(np.float32)


def team_mask(rgba):
    """Strength of team paint per pixel, from hue and saturation, and the image with that paint made grey."""
    tp = STUDIO["team_paint"]
    rgb = rgba[..., :3]
    mx, mn = rgb.max(-1), rgb.min(-1)
    sat = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    hue = hue_of(rgb)
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


def shrink(rgba, mask, origin, style, rs):
    """Downscale to atlas size. A style with pixel_size 2 shrinks to half that and doubles back with nearest
    neighbour, for chunky pixels at the same atlas scale; hard_alpha cuts edges to on or off."""
    st = STUDIO["vertical_stretch"]
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


def outline(rgba, mask, pivot):
    """Draw studio.json's `outline` round the opaque part of a frame, `px` atlas pixels wide, under any soft edge.
    The frame grows by that much on every side so nothing is cut off; the pivot moves with it."""
    spec = STUDIO["outline"]
    n = spec["px"]
    rgba = np.pad(rgba, ((n, n), (n, n), (0, 0)))
    mask = None if mask is None else np.pad(mask, n)
    solid = rgba[..., 3] >= 0.5
    grow = solid.copy()
    for _ in range(n):
        g = grow.copy()
        g[1:] |= grow[:-1]
        g[:-1] |= grow[1:]
        g[:, 1:] |= grow[:, :-1]
        g[:, :-1] |= grow[:, 1:]
        grow = g
    edge = grow & ~solid
    col = np.array(spec["rgba"], np.float32) / 255
    a = rgba[..., 3][edge][:, None]
    oa = col[3] * (1 - a)
    out_a = a + oa
    rgba[..., :3][edge] = (rgba[..., :3][edge] * a + col[:3] * oa) / np.maximum(out_a, 1e-4)
    rgba[..., 3][edge] = out_a[:, 0]
    if mask is not None:
        mask[edge] = mask[edge] * (a / np.maximum(out_a, 1e-4))[:, 0]
    return rgba, mask, (pivot[0] + n, pivot[1] + n)


def trim(rgba, mask, pivot):
    margin = STUDIO["margin_px"]
    ys, xs = np.nonzero(rgba[..., 3] > 1 / 255)
    if len(xs) == 0:
        return rgba[:1, :1], None if mask is None else mask[:1, :1], [0, 0]
    x0, x1 = max(0, xs.min() - margin), min(rgba.shape[1], xs.max() + 1 + margin)
    y0, y1 = max(0, ys.min() - margin), min(rgba.shape[0], ys.max() + 1 + margin)
    crop = rgba[y0:y1, x0:x1]
    return crop, None if mask is None else mask[y0:y1, x0:x1], [round(pivot[0] - x0), round(pivot[1] - y0)]


def image_path(rdir, job, f, n):
    return rdir / f"{job['part']}-{job['anim']}-f{f:02d}-{n:02d}.png"


def uncrop(rgba, job, size):
    """An overlay was rendered inside a border; put it back on a full canvas so it shrinks exactly like the frame
    it is drawn over and their pivots agree."""
    if not job.get("crop"):
        return rgba
    x0, y0 = job["crop"][:2]
    full = np.zeros((size[1], size[0], 4), np.float32)
    h, w = min(rgba.shape[0], size[1] - y0), min(rgba.shape[1], size[0] - x0)
    full[y0:y0 + h, x0:x0 + w] = rgba[:h, :w]
    return full


def frames_of(rdir, meta):
    """Every frame of an entity: (part, anim, kind, rgba, mask, pivot), kind 'image' or 'shadow', in the order
    the JSON lists them: per job, facing by facing, each facing's frames in turn."""
    style = STUDIO["styles"][meta.get("style", "detailed")]
    scale = meta.get("render_scale", STUDIO["render_scale"])
    size = meta["render_size"]
    keys = [(job, f, n) for job in meta["jobs"] for f in range(job["facings"]) for n in range(job["frames"])]
    missing = [str(image_path(rdir, *k)) for k in keys if not image_path(rdir, *k).exists()]
    if missing:
        sys.exit(f"{meta['id']}: {len(missing)} frames missing, e.g. {missing[0]} (a partial render?)")
    masked = neutralise([team_mask(uncrop(load(image_path(rdir, *k)), k[0], size)) for k in keys])
    out = []
    for (job, f, n), (rgba, mask) in zip(keys, masked):
        small, m, piv = shrink(rgba, mask, meta["origin_px"], style, scale)
        if meta["category"] in STUDIO["outline"]["categories"] and not job.get("overlay"):
            small, m, piv = outline(small, m, piv)
        out.append((job["part"], job["anim"], "image", *trim(small, m, piv)))
        sp = image_path(rdir, job, f, n).with_suffix(".shadow.png")
        if job.get("shadow"):
            sh = load(sp)
            sh[..., :3] = 0
            # Drop the catcher's faint noise floor, so a shadow frame trims to the shadow, not the whole canvas.
            sh[..., 3] = np.clip((sh[..., 3] - SHADOW_FLOOR) / (1 - SHADOW_FLOOR), 0, 1)
            small, _, piv = shrink(sh, None, meta["origin_px"], style, scale)
            out.append((job["part"], job["anim"], "shadow", *trim(small, None, piv)))
    return out


def icon_frames(rdir, meta):
    """A build icon: the portrait render shrunk to each icon size, untrimmed, with its team mask."""
    rgba, mask = neutralise([team_mask(load(rdir / "icon.png"))])[0]
    out = []
    for name, size in STUDIO["portrait"]["sizes"].items():
        small = resize(rgba, tuple(size))
        m = np.clip(resize(mask * rgba[..., 3], tuple(size)) / np.maximum(small[..., 3], 1e-4), 0, 1)
        out.append(("icon", name, "image", small, m * (small[..., 3] > 0), [0, 0]))
    return out


def pack(frames, page):
    """Shelf-pack frames, tallest first, ties in input order; returns (x, y) per frame and the page size."""
    order = sorted(range(len(frames)), key=lambda k: (-frames[k].shape[0], k))
    pos, x, y, shelf = [None] * len(frames), 0, 0, 0
    for k in order:
        h, w = frames[k].shape[:2]
        if x + w > page[0]:
            x, y, shelf = 0, y + shelf, 0
        pos[k] = (x, y)
        x += w
        shelf = max(shelf, h)
    return pos, (page[0], y + shelf)


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


def overflow(atlas, entities, size):
    """Stop rather than start a page the renderer does not expect; name the biggest entities to trim."""
    areas = sorted(((sum(f[3].shape[0] * f[3].shape[1] for f in fs), m["id"]) for m, fs in entities), reverse=True)
    top = ", ".join(f"{eid} ({a // 1000}k px)" for a, eid in areas[:5])
    sys.exit(f"{atlas}: needs {size[0]}x{size[1]}, over the {PAGE[0]}x{PAGE[1]} page. Largest: {top}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("renders", nargs="+")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    repo = Path(__file__).resolve().parents[2]
    o = str(out.resolve())
    # The git-ignored settings-private/ clone is the private repository, not this one.
    inside = o.startswith(str(repo) + os.sep) and not o.startswith(str(repo / "settings-private") + os.sep)
    by_atlas = {}
    for rdir in map(Path, args.renders):
        meta = json.loads((rdir / "meta.json").read_text())
        if meta.get("external") and inside:
            sys.exit(f"{meta['id']} comes from {meta['model']}, outside this repository: pack it into its own "
                     f"pack's folder, never under {repo}")
        frames = icon_frames(rdir, meta) if meta["kind"] == "icon" else frames_of(rdir, meta)
        by_atlas.setdefault(ATLAS[meta["category"]], []).append((meta, frames))

    for atlas, entities in sorted(by_atlas.items()):
        flat = [f for _, fs in entities for f in fs]
        pos, size = pack([f[3] for f in flat], PAGE)
        if size[1] > PAGE[1]:
            overflow(atlas, entities, size)
        styles = {m.get("style", "detailed") for m, _ in entities}
        assert len(styles) == 1, f"{atlas}: one style per page, got {sorted(styles)}"
        write_page(out / atlas, [f[3] for f in flat], [f[4] for f in flat], pos, size,
                   STUDIO["styles"][styles.pop()]["palette"])
        k = 0
        for meta, fs in entities:
            jobs = {(j["part"], j["anim"]): j for j in meta.get("jobs", [])}
            parts = {}
            for part, anim, kind, img, _, piv in fs:
                x, y = pos[k]
                k += 1
                job = jobs.get((part, anim), {"facings": 1, "frames": 1})
                entry = parts.setdefault(part, {"facings": job["facings"], "anims": {}})
                if job.get("overlay"):
                    entry["overlay"] = True
                a = entry["anims"].setdefault(anim, {"length": job["frames"], "frames": []})
                if job["facings"] != entry["facings"]:
                    a["facings"] = job["facings"]  # e.g. infantry deaths, drawn the same from every side
                frame = [x, y, img.shape[1], img.shape[0], piv[0], piv[1]]
                a.setdefault("shadow", []).append(frame) if kind == "shadow" else a["frames"].append(frame)
            if "turret" in parts:
                parts["turret"]["pivotOffset"] = [0, 0]
            doc = {"id": meta["id"], "atlas": atlas, "facings": max(p["facings"] for p in parts.values()),
                   "scale": 2, "parts": parts}
            if "footprint" in meta:
                doc["footprint"] = meta["footprint"]
            path = out / FOLDER[meta["category"]] / f"{meta['id']}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(doc, separators=(", ", ": ")) + "\n")
        print(f"{atlas}: {len(flat)} frames, page {size[0]}x{size[1]}")


if __name__ == "__main__":
    main()
