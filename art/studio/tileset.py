#!/usr/bin/env python3
"""Terrain tiles for a setting pack, from a terrain set file (art/terrain/<set>.json), with no Blender render.

    python3 art/studio/tileset.py art/terrain/desert.json --out settings/generic/art/tiles
    python3 art/studio/tileset.py art/terrain/desert.json --out /tmp/tiles --sheet /tmp/tiles-sheet.png

The ground is drawn in layers, bottom first (open ground under everything, then the resource, rock and cliffs).
The renderer draws each layer on a grid offset by half a tile, so every drawn tile has a map tile at each of its
four corners: a layer's tile is the "corner case" of which of those four map tiles are in the layer (16 cases,
1 north-west, 2 north-east, 4 south-east, 8 south-west). Case 15 is the layer's full tile, case 0 draws nothing,
and cases 1 to 14 are its edges, blended to nothing through a noisy mask (art-pipeline.md section 7, done as
layers rather than pairs, so any number of ground kinds can meet).

Tiles join without seams because every texture is one swatch that repeats every `period` tiles (4 by default),
and a tile is cut from the swatch where it lies in the map: the renderer picks the tile for a drawn tile's place
(x mod period, y mod period) as well as its corner case. The noisy edges come from the same swatch, so two tiles
always agree along the edge they share, and the ground repeats only every few tiles, not every tile.

Relief (rock rims, cliffs, ripples, nodules) is a height field lit by the studio's key light, from the same
direction as the rendered units. Output: `tileset.png` (one page, each tile with a 1-pixel border copied from its
edge) and `tileset.json`, read by crates/classic-render/src/tiles.rs.
"""

import argparse
import json
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
BITS = (1, 2, 4, 8)  # north-west, north-east, south-east, south-west
COLUMNS = 32
PAD = 1


def hex_rgb(s):
    s = s.lstrip("#")
    return np.array([int(s[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.float64) / 255.0


def noise(rng, n, beta, lowest=3):
    """Fractal noise that repeats every n pixels both ways: white noise shaped by 1/f^beta. Mean 0, deviation 1.
    Waves longer than n / lowest are left out: one bump per tile would show the tile grid."""
    white = rng.standard_normal((n, n))
    k = np.fft.fftfreq(n) * n
    kk = np.sqrt(k[None, :] ** 2 + k[:, None] ** 2)
    amp = np.zeros_like(kk)
    amp[kk >= lowest] = kk[kk >= lowest] ** -beta
    f = np.real(np.fft.ifft2(np.fft.fft2(white) * amp))
    return (f - f.mean()) / (f.std() + 1e-12)


def blur(a, sigma):
    """Gaussian blur that wraps round, like the textures."""
    n = a.shape[0]
    k = np.fft.fftfreq(n)
    g = np.exp(-2 * (np.pi * sigma) ** 2 * (k[None, :] ** 2 + k[:, None] ** 2))
    return np.real(np.fft.ifft2(np.fft.fft2(a) * g))


def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def wrap_dist(n, cx, cy):
    """Distance from every pixel centre to (cx, cy), the shortest way round a tile that repeats."""
    ys, xs = np.mgrid[0:n, 0:n] + 0.5
    dx = np.abs(xs - cx)
    dy = np.abs(ys - cy)
    dx = np.minimum(dx, n - dx)
    dy = np.minimum(dy, n - dy)
    return np.sqrt(dx * dx + dy * dy)


def corner_weights(n, pad):
    """Bilinear weight of each corner (NW, NE, SE, SW) at every pixel centre, with `pad` pixels round the tile."""
    v, u = (np.mgrid[0:n + 2 * pad, 0:n + 2 * pad] - pad + 0.5) / n
    return [(1 - u) * (1 - v), u * (1 - v), u * v, (1 - u) * v]


# ---- Materials: each returns a dict of fields on a swatch n pixels square that repeats both ways ---------------
# `t` is the size of one tile in pixels, for sizes that are per tile.


def sand(rng, n, t, p):
    r = p["ripple"]
    ys, xs = np.mgrid[0:n, 0:n] + 0.5
    # Long waves bend the ripples.
    warp = noise(rng, n, 2.5, lowest=1) * r["warp"]
    wx, wy = r["wave"]
    ripple = np.sin(2 * np.pi * (wx * xs + wy * ys) / t + warp)
    # Ripples are sharp on the crest and soft in the trough, and fade in and out across the sand.
    ripple = (ripple + 1) ** 1.6 / 2 ** 1.6 * smoothstep(-1.5, 0.8, noise(rng, n, 2.0, lowest=2))
    swell = noise(rng, n, 2.8, lowest=1)
    height = p["rough"] * (0.35 * swell + r["height"] * ripple)
    tone = 0.5 + 0.08 * noise(rng, n, 2.0) + 0.08 * ripple
    grain = rng.standard_normal((n, n)) * p["grain"]
    speck = (rng.random((n, n)) < 0.01).astype(float)
    return {"height": height, "tone": tone, "grain": grain, "speck": speck, "cavity": np.zeros((n, n))}


def rock(rng, n, t, p):
    base = noise(rng, n, 2.0)
    ridged = 1 - np.abs(noise(rng, n, 1.6))
    height = p["rough"] * (0.6 * base + 0.6 * ridged ** 2)
    crack = 1 - smoothstep(0.0, p["cracks"] * 3, np.abs(noise(rng, n, 1.7)))
    height -= crack * p["rough"] * 0.5
    cavity = np.clip(blur(height, 2.0) - height, 0, None)
    cavity = cavity / (cavity.max() + 1e-9)
    tone = 0.5 + 0.18 * noise(rng, n, 2.2) + 0.12 * (ridged - 0.5)
    grain = rng.standard_normal((n, n)) * p["grain"]
    speck = (rng.random((n, n)) < 0.02).astype(float)
    return {"height": height, "tone": tone, "grain": grain, "speck": speck, "cavity": np.maximum(cavity, crack * 0.8)}


def deposit(rng, n, t, p):
    """A mineral resource on the ground: rounded nodules in clusters over a stain, and crust patches when thick.
    Nodule and cluster counts are per tile."""
    height = np.zeros((n, n))
    nodule = np.zeros((n, n))
    lo, hi = p["radius"]
    tiles = (n // t) ** 2
    clusters = [(rng.random() * n, rng.random() * n) for _ in range(p["clusters"] * tiles)]
    for i in range(p["nodules"] * tiles):
        ox, oy = clusters[i % len(clusters)]
        cx, cy = (ox + rng.normal() * p["spread"]) % n, (oy + rng.normal() * p["spread"]) % n
        r = lo + (hi - lo) * rng.random() ** 1.5
        d = wrap_dist(n, cx, cy)
        dome = np.sqrt(np.clip(r * r - d * d, 0, None)) * 1.1
        height = np.maximum(height, dome)
        nodule = np.maximum(nodule, smoothstep(r + 0.6, r - 0.4, d))
    crust = 0
    if p["patches"]:
        crust = smoothstep(0.62 - p["patches"], 0.8 - p["patches"], noise(rng, n, 2.2) * 0.25 + 0.5)
        crust = crust * (0.75 + 0.25 * smoothstep(-0.5, 1.0, noise(rng, n, 1.2)))
    height = height + crust * (1.0 + 0.6 * noise(rng, n, 1.0))
    stain = p["stain"] * (0.7 + 0.6 * smoothstep(-1.5, 1.5, noise(rng, n, 2.0))) + blur(nodule, 2.0) * 0.8
    solid = np.maximum(nodule, crust)
    return {"height": height, "tone": 0.5 + 0.2 * noise(rng, n, 1.5), "alpha": np.clip(np.maximum(stain, solid), 0, 1),
            "nodule": solid}


MATERIALS = {"sand": sand, "rock": rock, "deposit": deposit}


# ---- Shading ----------------------------------------------------------------------------------------------------


def light_vector():
    """Towards the studio's key light, in tile coordinates (x east, y south, z up)."""
    studio = json.load(open(os.path.join(HERE, "studio.json")))
    dx, dy, dz = studio["key_light"]["direction"]  # the way the light travels, x east, y north, z up
    v = np.array([-dx, dy, -dz], dtype=np.float64)
    return v / np.linalg.norm(v), np.array(studio["key_light"]["colour"])


def shade(height, light):
    """Lambert shading of a height field, 1.0 where flat."""
    gy, gx = np.gradient(height)
    nx, ny, nz = -gx, -gy, np.ones_like(height)
    norm = np.sqrt(nx * nx + ny * ny + nz * nz)
    ndl = np.clip((nx * light[0] + ny * light[1] + nz * light[2]) / norm, 0, 1)
    return 0.38 + 0.62 * ndl / light[2], (nx / norm, ny / norm, nz / norm)


def ramp(c, t):
    """Mix dark, mid and light by t in [0, 1]."""
    t = np.clip(t, 0, 1)[..., None]
    lo = c["dark"] + (c["mid"] - c["dark"]) * np.clip(t * 2, 0, 1)
    return lo + (c["light"] - c["mid"]) * np.clip(t * 2 - 1, 0, 1)


def paint(layer, f, mask, relief, light, light_rgb):
    """One tile's RGBA (0 to 1) from mixed fields `f` and the layer's corner mask."""
    c = {k: hex_rgb(v) for k, v in layer["colours"].items()}
    height = f["height"] + relief
    lum, normal = shade(height, light)
    if layer["material"] == "deposit":
        base = ramp(c, 0.25 + 0.5 * f["tone"] + 0.08 * f["height"])
        nod = np.clip(f["nodule"], 0, 1)[..., None]
        # A glint where the nodule faces between the light and the camera above.
        half = light + np.array([0, 0, 1.0])
        half /= np.linalg.norm(half)
        spec = np.clip(normal[0] * half[0] + normal[1] * half[1] + normal[2] * half[2], 0, 1) ** 24
        rgb = base * lum[..., None] + spec[..., None] * 0.55 * nod
        rgb = c["stain"] * (1 - nod) + rgb * nod
        alpha = np.clip(f["alpha"], 0, 1) * mask
    else:
        tone = f["tone"] + 0.04 * f["height"] + 0.25 * relief / max(layer.get("relief", 0), 1)
        if "strata" in layer:
            # Bands of rock along the slope of a raised edge, so a cliff face reads as a face.
            band = np.sin(2 * np.pi * (relief + f["height"] * 0.5) / layer["strata"])
            tone = tone + 0.12 * band * np.clip(4 * relief * (1 - relief / max(layer["relief"], 1)) / max(layer["relief"], 1), 0, 1)
        rgb = ramp(c, tone) * lum[..., None]
        rgb = rgb * (1 + f["grain"][..., None])
        rgb = rgb * (1 - 0.45 * f["cavity"][..., None])
        speck = f["speck"][..., None]
        rgb = rgb * (1 - speck) + c["grain"] * lum[..., None] * speck
        alpha = mask
    rgb = rgb * (0.85 + 0.15 * light_rgb)
    return np.dstack([np.clip(rgb, 0, 1), np.clip(alpha, 0, 1)])


# ---- The set ----------------------------------------------------------------------------------------------------


def cast_shadow(tile, lin, light, width, strength):
    """Darken the ground just outside a raised layer's edge, on the side away from the light."""
    gy, gx = np.gradient(lin)
    length = np.sqrt(gx * gx + gy * gy) + 1e-9
    to_light = light[:2] / np.linalg.norm(light[:2])
    # The edge's slope faces the light where the layer lies between this pixel and the light.
    facing = np.clip((gx * to_light[0] + gy * to_light[1]) / length, 0, 1)
    shadow = strength * smoothstep(0.5 - width, 0.5, lin) * (0.35 + 0.65 * facing)
    a = tile[..., 3]
    alpha = a + (1 - a) * shadow
    rgb = tile[..., :3] * (a / np.maximum(alpha, 1e-9))[..., None]
    return np.dstack([rgb, alpha])


def window(field, x, y, n, pad):
    """The n-pixel square of a repeating swatch at (x, y), with `pad` pixels round it."""
    rows = np.arange(y - pad, y + n + pad) % field.shape[0]
    cols = np.arange(x - pad, x + n + pad) % field.shape[1]
    return field[np.ix_(rows, cols)]


def make_set(spec):
    """Each layer's tiles: tiles[case][place] for corner cases 1 to 15 and places in the swatch, row by row."""
    n = spec["tile_px"]
    period = spec.get("period", 4)
    light, light_rgb = light_vector()
    # Everything is worked out one pixel beyond the tile, so slopes at its edge are shaded as its neighbour's are.
    w = corner_weights(n, 1)
    e = spec["edge"]
    layers = []
    for li, layer in enumerate(spec["layers"]):
        rng = np.random.default_rng([spec["seed"], li])
        swatch = MATERIALS[layer["material"]](rng, n * period, n, layer)
        swatch["edge"] = noise(rng, n * period, 1.8, lowest=2)
        relief_px = layer.get("relief", 0)
        tiles = [None] * 16
        # A base layer covers every map tile, so it only ever needs its full tile.
        for case in [15] if layer.get("base") else range(1, 16):
            tiles[case] = []
            for place in range(period * period):
                x, y = (place % period) * n, (place // period) * n
                f = {k: window(v, x, y, n, 1) for k, v in swatch.items()}
                if case == 15:
                    tile = paint(layer, f, np.ones((n + 2, n + 2)), relief_px, light, light_rgb)
                else:
                    lin = sum(w[i] * ((case >> i) & 1) for i in range(4)) + e["noise"] * f["edge"] * 0.5
                    mask = smoothstep(0.5 - e["soft"], 0.5 + e["soft"], lin)
                    # The slope up to a raised layer starts at its edge and is a few pixels wide.
                    relief = relief_px * smoothstep(0.5 - e["soft"], 0.5 + layer.get("slope", 4) * e["soft"], lin)
                    tile = paint(layer, f, mask, relief, light, light_rgb)
                    if "shadow" in layer:
                        tile = cast_shadow(tile, lin, light, *layer["shadow"])
                tiles[case].append(tile[1:-1, 1:-1])
        layers.append((layer, tiles))
    return layers


def to_u8(rgba):
    return (np.clip(rgba, 0, 1) * 255 + 0.5).astype(np.uint8)


def write(spec, layers, out):
    n = spec["tile_px"]
    cell = n + 2 * PAD
    count = sum(sum(len(t) for t in tiles if t) for _, tiles in layers)
    rows = (count + COLUMNS - 1) // COLUMNS
    page = np.zeros((rows * cell, COLUMNS * cell, 4), dtype=np.uint8)
    slot = 0
    index = {"about": "Terrain tiles written by art/studio/tileset.py from art/terrain/" + spec["set"] + ".json "
             "(art/README.md, 'Terrain tiles'). Layers are drawn in order. 'on' says which map tiles a layer covers; "
             "'tiles' holds [x, y] in the page of each tile, tile_px square, by corner case (1 NW, 2 NE, 4 SE, 8 SW; "
             "case 0 is null) and then by place: (y mod period) * period + (x mod period) for the drawn tile at "
             "(x, y), the one whose south-east corner is map tile (x, y).",
             "set": spec["set"], "tile_px": n, "period": spec.get("period", 4), "page": "art/tiles/tileset.png",
             "layers": []}

    def place(img):
        nonlocal slot
        x, y = (slot % COLUMNS) * cell, (slot // COLUMNS) * cell
        page[y:y + cell, x:x + cell] = np.pad(to_u8(img), ((PAD, PAD), (PAD, PAD), (0, 0)), mode="edge")
        slot += 1
        return [x + PAD, y + PAD]

    for layer, tiles in layers:
        entry = {"id": layer["id"], "on": layer["on"]}
        entry["tiles"] = [None if t is None else [place(v) for v in t] for t in tiles]
        # The minimap's colour: the full tiles' own average, or for a sparse layer such as a resource, its body.
        full = tiles[15]
        if layer["material"] == "deposit":
            rgb = hex_rgb(layer["colours"]["mid"])
        else:
            rgb = np.concatenate([t[..., :3].reshape(-1, 3) for t in full]).mean(axis=0)
        entry["colour"] = "#%02x%02x%02x" % tuple(int(v * 255 + 0.5) for v in rgb)
        index["layers"].append(entry)
    os.makedirs(out, exist_ok=True)
    Image.fromarray(page, "RGBA").save(os.path.join(out, "tileset.png"), optimize=True)
    with open(os.path.join(out, "tileset.json"), "w") as f:
        f.write(dump(index) + "\n")
    print(f"{out}: {slot} tiles of {n} px on a {page.shape[1]}x{page.shape[0]} page")


def dump(index):
    """JSON with one line per corner case, so diffs stay readable."""
    lines = ["{"]
    for k in ("about", "set", "tile_px", "period", "page"):
        lines.append(f"  {json.dumps(k)}: {json.dumps(index[k])},")
    lines.append('  "layers": [')
    for i, layer in enumerate(index["layers"]):
        lines.append("    {")
        lines.append(f'      "id": {json.dumps(layer["id"])}, "on": {json.dumps(layer["on"])}, '
                     f'"colour": {json.dumps(layer["colour"])},')
        lines.append('      "tiles": [')
        cases = [json.dumps(t, separators=(",", ":")) for t in layer["tiles"]]
        lines.append(",\n".join("        " + c for c in cases))
        lines.append("      ]")
        lines.append("    }" + ("," if i + 1 < len(index["layers"]) else ""))
    lines.append("  ]")
    lines.append("}")
    return "\n".join(lines)


def sheet(spec, layers, path, scale=2):
    """A contact sheet, a row per layer over the bottom layer: its full tiles in one place each, then its 14 edges."""
    n = spec["tile_px"]
    gap = 6
    under = layers[0][1][15][0]
    places = len(layers[0][1][15])
    w = (places + 14) * (n + gap) + 3 * gap
    h = len(layers) * (n + gap) + gap
    img = np.full((h, w, 3), 0.12)
    for r, (_, tiles) in enumerate(layers):
        row = tiles[15] + [tiles[case][0] for case in range(1, 15) if tiles[case]]
        row = row[:places + 14]
        for i, t in enumerate(row):
            a = t[..., 3:4]
            x = gap + i * (n + gap) + (2 * gap if i >= places else 0)
            y = gap + r * (n + gap)
            img[y:y + n, x:x + n] = t[..., :3] * a + under[..., :3] * (1 - a)
    im = Image.fromarray(to_u8(img), "RGB")
    im = im.resize((w * scale, h * scale), Image.NEAREST)
    im.save(path)
    print(f"{path}: contact sheet")


def read_map(path):
    """A map file's ground ('open', 'rock', 'cliff') and resource (True/False) per tile, rows top first."""
    kinds = {".": "open", "~": "open", "#": "rock", "X": "cliff"}
    ground, resource = [], []
    for line in open(path):
        line = line.rstrip("\n")
        if not line or line.startswith(";"):
            continue
        ground.append([kinds.get(c, "rock" if c.isdigit() else "open") for c in line])
        resource.append([c == "~" for c in line])
    return ground, resource


def resource_level(resource, x, y):
    """1 for a light field, 2 for thick: a full tile whose four neighbours all hold some (the renderer's rule)."""
    h, w = len(resource), len(resource[0])
    if not resource[y][x]:
        return 0
    for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0)):
        nx, ny = x + dx, y + dy
        if not (0 <= nx < w and 0 <= ny < h and resource[ny][nx]):
            return 1
    return 2


def preview(index_dir, map_path, path, scale=1):
    """Draw a map with the written tiles the way the renderer does, to look at a set without a GPU."""
    index = json.load(open(os.path.join(index_dir, "tileset.json")))
    page = np.asarray(Image.open(os.path.join(index_dir, "tileset.png")).convert("RGBA")).astype(np.float64) / 255
    n = index["tile_px"]
    ground, resource = read_map(map_path)
    h, w = len(ground), len(ground[0])

    def has(layer, x, y):
        x, y = min(max(x, 0), w - 1), min(max(y, 0), h - 1)
        on = layer["on"]
        if "ground" in on:
            return ground[y][x] in on["ground"]
        return resource_level(resource, x, y) >= on["resource"]

    period = index["period"]

    img = np.zeros((h * n + n, w * n + n, 3))
    for layer in index["layers"]:
        for cy in range(h + 1):
            for cx in range(w + 1):
                corners = [(cx - 1, cy - 1), (cx, cy - 1), (cx, cy), (cx - 1, cy)]
                case = sum(BITS[i] for i, (x, y) in enumerate(corners) if has(layer, x, y))
                if case == 0:
                    continue
                sx, sy = layer["tiles"][case][(cy % period) * period + cx % period]
                t = page[sy:sy + n, sx:sx + n]
                a = t[..., 3:4]
                dst = img[cy * n:cy * n + n, cx * n:cx * n + n]
                dst[:] = t[..., :3] * a + dst * (1 - a)
    img = img[n // 2:n // 2 + h * n, n // 2:n // 2 + w * n]
    im = Image.fromarray(to_u8(img), "RGB")
    if scale != 1:
        im = im.resize((im.width * scale, im.height * scale), Image.NEAREST)
    im.save(path)
    print(f"{path}: {map_path} drawn with {index_dir}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("set", help="a terrain set file, such as art/terrain/desert.json")
    ap.add_argument("--out", required=True, help="the pack's art/tiles folder")
    ap.add_argument("--sheet", help="also write a contact sheet here")
    ap.add_argument("--preview", nargs=2, metavar=("MAP", "PNG"), help="also draw MAP with the tiles into PNG")
    a = ap.parse_args()
    spec = json.load(open(a.set))
    layers = make_set(spec)
    write(spec, layers, a.out)
    if a.sheet:
        sheet(spec, layers, a.sheet)
    if a.preview:
        preview(a.out, *a.preview)


if __name__ == "__main__":
    sys.exit(main())
