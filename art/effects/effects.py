"""Effects for the generic pack, drawn from code: explosions, muzzle flashes, smoke, fire, sparks and shots.

    python3 art/effects/effects.py [--out settings/generic/art/sprites] [--only ID ...] [--sheet FILE]

The asset plan's batch O says effects are "code first; Blender flipbooks only if CPU smoke proves fast enough".
This is the code: every effect is built from a few shapes, lit by the studio's key light (top left) and rendered
supersampled, so the frames match the detailed style of the Blender sprites without Blender. Nothing here comes from
any game's effects; the shapes are plain physics sketches (a fireball of hot puffs that cool to smoke, sparks on
ballistic arcs, a flame ellipse along a barrel).

Output, in the studio's packed format (art/README.md, "Output"): one page `effects-0.png` (no team paint, so no
mask) and `effects/<id>.json` per effect, with a single part `effect` whose `idle` anim holds every frame, facing by
facing. The pivot of each frame is where the effect happens: the ground point of an explosion, the barrel tip of a
muzzle flash, the base of a fire. `scale` is 2: 64 atlas pixels per tile, like the studio's sprites.

Everything is seeded by the effect id, so a run writes the same bytes every time on the same numpy.
"""

import argparse
import json
import math
import sys
import zlib
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
TILE = 64  # atlas pixels per tile
SS = 3  # supersampling
PAGE = "effects-0"

# The studio's key light, as the direction towards it on screen: x right, y down, z towards the camera.
LIGHT = np.array([-0.62, -0.55, 0.56])
LIGHT /= np.linalg.norm(LIGHT)
WARM = np.array([1.0, 0.92, 0.8])
COOL = np.array([0.78, 0.84, 0.95])

# Fire colours by heat, 0 to 1 (straight RGB, linear-ish).
FIRE = [
    (0.00, (0.22, 0.04, 0.02)),
    (0.30, (0.62, 0.12, 0.03)),
    (0.50, (0.95, 0.36, 0.06)),
    (0.70, (1.00, 0.62, 0.16)),
    (0.85, (1.00, 0.85, 0.45)),
    (1.00, (1.00, 0.98, 0.86)),
]

SMOKE = np.array([0.42, 0.38, 0.33])
DUST = np.array([0.55, 0.45, 0.32])
SMOKE_GREY = np.array([0.34, 0.34, 0.36])


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def fire_colour(heat):
    """RGB for an array of heats."""
    keys = np.array([k for k, _ in FIRE])
    cols = np.array([c for _, c in FIRE])
    h = np.clip(heat, 0, 1)
    return np.stack([np.interp(h, keys, cols[:, c]) for c in range(3)], axis=-1)


class Noise:
    """Tileable value-noise fBm on a 256 square, sampled with wrap-round bilinear lookups."""

    def __init__(self, seed, size=256, octaves=5):
        rng = np.random.default_rng(seed)
        total = np.zeros((size, size))
        amp, norm = 1.0, 0.0
        for o in range(octaves):
            cells = 4 * 2**o
            grid = rng.random((cells, cells))
            total += amp * self._upsample(grid, size)
            norm += amp
            amp *= 0.5
        total /= norm
        self.tex = (total - total.min()) / (total.max() - total.min())
        self.size = size

    @staticmethod
    def _upsample(grid, size):
        n = grid.shape[0]
        c = np.arange(size) * n / size
        i0 = np.floor(c).astype(int)
        f = c - i0
        f = f * f * (3 - 2 * f)
        i1 = (i0 + 1) % n
        rows = grid[i0][:, i0] * (1 - f)[None, :] + grid[i0][:, i1] * f[None, :]
        rows1 = grid[i1][:, i0] * (1 - f)[None, :] + grid[i1][:, i1] * f[None, :]
        return rows * (1 - f)[:, None] + rows1 * f[:, None]

    def __call__(self, u, v):
        s = self.size
        u = np.mod(u * s, s)
        v = np.mod(v * s, s)
        x0 = np.floor(u).astype(int)
        y0 = np.floor(v).astype(int)
        fx, fy = u - x0, v - y0
        x1, y1 = (x0 + 1) % s, (y0 + 1) % s
        x0 %= s
        y0 %= s
        t = self.tex
        a = t[y0, x0] * (1 - fx) + t[y0, x1] * fx
        b = t[y1, x0] * (1 - fx) + t[y1, x1] * fx
        return a * (1 - fy) + b * fy


class Canvas:
    """A premultiplied RGBA float canvas at SS times the final size, with the pivot at (px, py) final pixels."""

    def __init__(self, w, h, px, py):
        self.w, self.h = w, h
        self.px, self.py = px, py
        self.rgb = np.zeros((h * SS, w * SS, 3))
        self.a = np.zeros((h * SS, w * SS))

    def region(self, cx, cy, rx, ry):
        """Pixel grids (x, y in final pixels from the pivot) covering a box, and the slices to write them to."""
        x0 = max(0, int((self.px + cx - rx) * SS))
        x1 = min(self.w * SS, int(math.ceil((self.px + cx + rx) * SS)) + 1)
        y0 = max(0, int((self.py + cy - ry) * SS))
        y1 = min(self.h * SS, int(math.ceil((self.py + cy + ry) * SS)) + 1)
        if x0 >= x1 or y0 >= y1:
            return None
        xs = (np.arange(x0, x1) + 0.5) / SS - self.px
        ys = (np.arange(y0, y1) + 0.5) / SS - self.py
        x, y = np.meshgrid(xs, ys)
        return x, y, (slice(y0, y1), slice(x0, x1))

    def over(self, sl, rgb, alpha):
        """Composite straight `rgb` at `alpha` over what is there."""
        alpha = np.clip(alpha, 0, 1)
        self.rgb[sl] = rgb * alpha[..., None] + self.rgb[sl] * (1 - alpha[..., None])
        self.a[sl] = alpha + self.a[sl] * (1 - alpha)

    def image(self):
        """The final-size straight-alpha RGBA image."""
        h, w = self.h, self.w
        rgb = self.rgb.reshape(h, SS, w, SS, 3).mean(axis=(1, 3))
        a = self.a.reshape(h, SS, w, SS).mean(axis=(1, 3))
        out = np.zeros((h, w, 4))
        safe = np.maximum(a, 1e-6)[..., None]
        out[..., :3] = np.where(a[..., None] > 1e-6, rgb / safe, 0)
        out[..., 3] = a
        return Image.fromarray(np.clip(out * 255 + 0.5, 0, 255).astype(np.uint8), "RGBA")


def project(x, y, h):
    """A world offset in tiles (x east, y north, h up) to screen pixels from the ground point, and a depth (bigger is
    nearer the camera). The studio's camera looks north at 60 degrees and the packer stretches height so ground
    squares stay square, so height rises by cot 60 per unit."""
    sx = x * TILE
    sy = (-y - h / math.tan(math.radians(60))) * TILE
    depth = -y * 0.5 + h * 0.866
    return sx, sy, depth


def puff(cv, noise, cx, cy, r, heat=0.0, dens=1.0, colour=SMOKE, seed=0.0, boil=0.0, squash=1.0, rough=0.9):
    """A lit ball of smoke, hot in its core when `heat` is above 0, centred at (cx, cy) final pixels from the pivot."""
    reg = cv.region(cx, cy, r * 1.3, r * 1.3 * squash)
    if reg is None or dens <= 0.002:
        return
    x, y, sl = reg
    dx = (x - cx) / r
    dy = (y - cy) / (r * squash)
    q2 = dx * dx + dy * dy
    t = np.sqrt(np.clip(1 - q2, 0, 1))
    scale = 0.9 / max(r, 1.0)
    u = x * scale * 0.35 + seed
    v = y * scale * 0.35 + seed * 1.7 + boil
    n = noise(u, v)
    e = 1.0 / noise.size
    gx = (noise(u + e * 2, v) - noise(u - e * 2, v)) * 6
    gy = (noise(u, v + e * 2) - noise(u, v - e * 2)) * 6
    shape = t + (n - 0.5) * rough - 0.08
    # Fade out before the edge of the drawn box, whatever the noise says.
    alpha = smoothstep(0.0, 0.32, shape) * smoothstep(1.25, 0.9, np.sqrt(q2)) * dens
    nx, ny, nz = dx - gx, dy - gy, np.maximum(t, 0.12)
    norm = np.sqrt(nx * nx + ny * ny + nz * nz)
    lit = np.clip((nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2]) / norm, 0, 1)
    shade = 0.38 * COOL + lit[..., None] * 0.95 * WARM
    rgb = colour * shade
    if heat > 0:
        hot = heat * (0.45 + 0.75 * t) + (n - 0.5) * 0.55 * heat
        w = smoothstep(0.18, 0.5, hot)[..., None]
        rgb = rgb * (1 - w) + fire_colour(hot) * w
        edge = smoothstep(1.25, 0.9, np.sqrt(q2)) * smoothstep(-0.1, 0.2, shape)
        alpha = np.maximum(alpha, smoothstep(0.25, 0.45, hot) * dens * edge)
    cv.over(sl, rgb, alpha)


def glow(cv, cx, cy, r, strength, colour=(1.0, 0.75, 0.35), squash=1.0):
    """A soft round light, for flashes."""
    reg = cv.region(cx, cy, r * 2, r * 2 * squash)
    if reg is None or strength <= 0:
        return
    x, y, sl = reg
    d2 = ((x - cx) / r) ** 2 + ((y - cy) / (r * squash)) ** 2
    cv.over(sl, np.broadcast_to(np.array(colour), x.shape + (3,)), strength * np.exp(-d2 * 1.6))


def flame(cv, noise, cx, cy, d, length, width, heat, seed=0.0, boil=0.0, dens=1.0):
    """A flame tongue from (cx, cy) along the unit screen direction `d`, `length` by `width` pixels."""
    reach = max(length, width) * 1.2
    reg = cv.region(cx + d[0] * length / 2, cy + d[1] * length / 2, reach, reach)
    if reg is None:
        return
    x, y, sl = reg
    rx, ry = x - cx, y - cy
    along = (rx * d[0] + ry * d[1]) / max(length, 0.5)
    across = (-rx * d[1] + ry * d[0]) / max(width / 2, 0.5)
    # Widest a third of the way along, closing to a point at the tip.
    taper = np.where(along < 0.3, np.sqrt(np.clip(along / 0.3, 0, 1)) * 0.7 + 0.3, 1 - (along - 0.3) / 0.7)
    body = np.clip(taper, 0, 1) - np.abs(across)
    n = noise(x * 0.09 + seed, y * 0.09 + seed * 1.3 + boil)
    body = body + (n - 0.5) * 0.45
    inside = (along > -0.15) & (along < 1.1)
    a = smoothstep(0.0, 0.25, body) * inside * dens
    hot = heat * (0.55 + 0.6 * body) * (1.15 - 0.5 * np.clip(along, 0, 1))
    cv.over(sl, fire_colour(hot), a)


def spark(cv, x0, y0, x1, y1, width, heat, alpha=1.0):
    """A bright streak from (x0, y0) to (x1, y1)."""
    reg = cv.region((x0 + x1) / 2, (y0 + y1) / 2, abs(x1 - x0) / 2 + width * 2, abs(y1 - y0) / 2 + width * 2)
    if reg is None:
        return
    x, y, sl = reg
    vx, vy = x1 - x0, y1 - y0
    ll = max(vx * vx + vy * vy, 1e-6)
    t = np.clip(((x - x0) * vx + (y - y0) * vy) / ll, 0, 1)
    d = np.hypot(x - (x0 + vx * t), y - (y0 + vy * t))
    a = smoothstep(width, width * 0.3, d) * alpha * (0.35 + 0.65 * t)
    cv.over(sl, fire_colour(np.full(x.shape, heat) * (0.7 + 0.3 * t)), a)


def chunk(cv, cx, cy, size, shade):
    """A dark piece of debris."""
    reg = cv.region(cx, cy, size, size)
    if reg is None:
        return
    x, y, sl = reg
    a = smoothstep(size, size * 0.6, np.maximum(np.abs(x - cx), np.abs(y - cy)))
    lit = np.where((x - cx) + (y - cy) < 0, 1.0, 0.55)
    cv.over(sl, np.array([0.16, 0.14, 0.12])[None, None, :] * (lit * shade * 1.8)[..., None], a)


# ---- Effects ------------------------------------------------------------------------------------------------------


def seed_of(name):
    return zlib.crc32(name.encode())


def explosion(name, size_tiles, frames, puffs, debris):
    """A fireball of hot puffs thrown out and up that cool to smoke and rise, with a flash, sparks, debris and a
    ring of dust along the ground."""
    rng = np.random.default_rng(seed_of(name))
    noise = Noise(seed_of(name) + 1)
    size = int(size_tiles * TILE)
    w, h = size, int(size * 1.25)
    px, py = w / 2, h * 0.72
    R = size_tiles * 0.32  # tiles the fireball reaches out
    items = []
    for i in range(puffs):
        a = rng.random() * 2 * math.pi
        up = rng.random() ** 0.7
        out = math.sqrt(max(0.0, 1 - up * up))
        speed = R * (0.35 + 0.65 * rng.random())
        items.append(dict(
            v=(math.cos(a) * out * speed, math.sin(a) * out * speed, (0.25 + up) * speed * 0.9),
            r0=(0.07 + 0.05 * rng.random()) * size_tiles,
            grow=(0.11 + 0.09 * rng.random()) * size_tiles,
            life=0.4 + 0.4 * rng.random() * (1.2 - out),
            seed=rng.random() * 10,
            fade=0.25 + 0.3 * rng.random(),
        ))
    sparks = [dict(a=rng.random() * 2 * math.pi, up=0.4 + rng.random(), sp=R * (1.6 + 1.6 * rng.random()))
              for _ in range(6 + puffs // 2)]
    bits = [dict(a=rng.random() * 2 * math.pi, up=1.2 + rng.random() * 1.5, sp=R * (0.8 + rng.random() * 1.4),
                 size=1.0 + rng.random() * size_tiles) for _ in range(debris)]
    dust = [dict(a=i / 20 * 2 * math.pi + rng.random() * 0.3, seed=rng.random() * 10) for i in range(20)]
    out = []
    for f in range(frames):
        s = f / (frames - 1)
        cv = Canvas(w, h, px, py)
        # Dust thrown along the ground, low and flat, under everything.
        if size_tiles >= 1.5:
            for d in dust:
                ring = R * (0.4 + 1.0 * (1 - math.exp(-4 * s)))
                sx, sy, _ = project(math.cos(d["a"]) * ring, math.sin(d["a"]) * ring, 0.05)
                puff(cv, noise, sx, sy, (0.13 + 0.14 * s) * size_tiles * TILE, colour=DUST,
                     dens=0.32 * (1 - s) ** 2, seed=d["seed"], boil=s * 0.3, squash=0.5, rough=1.3)
        drawn = []
        k = 3.0
        travel = (1 - math.exp(-k * s)) / (1 - math.exp(-k))
        for p in items:
            vx, vy, vh = p["v"]
            rise = 0.55 * R * s * s
            x, y, hgt = vx * travel, vy * travel, vh * travel + rise + 0.18 * size_tiles
            sx, sy, depth = project(x, y, hgt)
            r = (p["r0"] + p["grow"] * (1 - math.exp(-3.5 * s))) * TILE
            heat = max(0.0, 1 - s / p["life"]) ** 1.3 * 1.15
            dens = 1.0 if s < p["fade"] else max(0.0, 1 - (s - p["fade"]) / (1 - p["fade"] + 0.05)) ** 1.6
            dens *= 0.85 if heat > 0.2 else 0.7
            drawn.append((depth, sx, sy, r, heat, dens, p["seed"]))
        for depth, sx, sy, r, heat, dens, seed in sorted(drawn):
            puff(cv, noise, sx, sy, r, heat=heat, dens=dens, seed=seed, boil=s * 0.6)
        # The flash: a hot core and a light round it, gone after a few frames.
        if s < 0.3:
            k0 = 1 - s / 0.3
            cx, cy, _ = project(0, 0, 0.22 * size_tiles)
            glow(cv, cx, cy, R * TILE * (0.7 + 0.8 * s), 0.55 * k0)
            puff(cv, noise, cx, cy, R * TILE * (0.35 + 0.5 * s), heat=1.2 * k0, dens=k0, seed=3.3)
        # Sparks on ballistic arcs, a short streak each.
        for sp in sparks:
            if s > 0.5:
                break
            t0, t1 = max(0.0, s - 0.06), s
            pts = []
            for t in (t0, t1):
                d = sp["sp"] * t * 2
                hh = 0.15 * size_tiles + sp["up"] * R * t * 2.2 - 6 * R * t * t
                pts.append(project(math.cos(sp["a"]) * d, math.sin(sp["a"]) * d, max(hh, 0)))
            spark(cv, pts[0][0], pts[0][1], pts[1][0], pts[1][1], 0.9, 1.0 - s, alpha=1 - s / 0.5)
        # Debris on the same arcs, heavier, seen once it is clear of the fireball.
        for b in bits:
            t = s * 1.3
            d = b["sp"] * t
            hh = 0.1 * size_tiles + b["up"] * R * t * 1.6 - 3.2 * R * t * t
            if hh < 0 or t < 0.25:
                continue
            sx, sy, _ = project(math.cos(b["a"]) * d, math.sin(b["a"]) * d, hh)
            chunk(cv, sx, sy, b["size"] * 0.6, 0.8)
        out.append(cv)
    return out


def smoke_puff(name, frames):
    """One puff of smoke that starts small, swells and thins; the renderer moves it. Cool grey and soft, so it never
    reads as a lump of rock."""
    noise = Noise(seed_of(name))
    size = TILE // 2 + 8
    out = []
    for f in range(frames):
        s = f / (frames - 1)
        cv = Canvas(size, size, size / 2, size / 2)
        r = size * (0.12 + 0.3 * s)
        dens = 0.7 * min(1.0, 0.45 + 3 * s) * (1 - s) ** 1.1
        puff(cv, noise, 0, 0, r, dens=dens, colour=SMOKE_GREY, seed=1.7, boil=s * 0.5, rough=0.55)
        out.append(cv)
    return out


def dust_puff(name, frames):
    """Sand kicked up, flatter and paler than smoke."""
    noise = Noise(seed_of(name))
    size = TILE // 2 + 8
    out = []
    for f in range(frames):
        s = f / (frames - 1)
        cv = Canvas(size, size, size / 2, size / 2)
        r = size * (0.2 + 0.22 * s)
        puff(cv, noise, 0, 0, r, dens=0.7 * (1 - s) ** 1.4, colour=DUST, seed=4.2, boil=s * 0.4, squash=0.75)
        out.append(cv)
    return out


def hit_spark(name, frames):
    """Sparks off armour: a white flash and short streaks fanning out."""
    rng = np.random.default_rng(seed_of(name))
    size = TILE // 2
    streaks = [(rng.random() * 2 * math.pi, 0.6 + 0.6 * rng.random()) for _ in range(9)]
    out = []
    for f in range(frames):
        s = f / max(frames - 1, 1)
        cv = Canvas(size, size, size / 2, size / 2)
        glow(cv, 0, 0, 6 * (1 - s) + 1.5, 0.95 * (1 - s), colour=(1.0, 0.92, 0.65))
        for a, sp in streaks:
            r0, r1 = size * 0.4 * sp * s, size * 0.4 * sp * (s + 0.3)
            spark(cv, math.cos(a) * r0, math.sin(a) * r0 + 2 * s * s, math.cos(a) * r1, math.sin(a) * r1 + 3 * s * s,
                  1.1, 1.0 - 0.4 * s, alpha=1 - s * 0.6)
        out.append(cv)
    return out


def facing_dir(i, n):
    a = 2 * math.pi * i / n
    return (math.sin(a), -math.cos(a))


def muzzle_gun(name, facings, frames):
    """A cannon's flash at the barrel tip: a long tongue forward, two short ones out to the sides, then smoke."""
    noise = Noise(seed_of(name))
    size = TILE
    out = []
    for i in range(facings):
        d = facing_dir(i, facings)
        side_l = (d[0] * 0.6 - d[1] * 0.8, d[1] * 0.6 + d[0] * 0.8)
        side_r = (d[0] * 0.6 + d[1] * 0.8, d[1] * 0.6 - d[0] * 0.8)
        for f in range(frames):
            cv = Canvas(size, size, size / 2, size / 2)
            k = [1.0, 0.6, 0.25][min(f, 2)]
            if f >= 1:
                puff(cv, noise, d[0] * 6 * f, d[1] * 6 * f - 2 * f, 4 + 3 * f, dens=0.5 / f, seed=i * 0.37,
                     boil=f * 0.2)
            glow(cv, d[0] * 4, d[1] * 4, 9 * k, 0.7 * k)
            flame(cv, noise, 0, 0, d, 26 * k, 13 * k, 1.15 * k, seed=i * 0.71, boil=f * 0.3)
            flame(cv, noise, 0, 0, side_l, 11 * k, 7 * k, 1.05 * k, seed=i * 0.53 + 2, boil=f * 0.3)
            flame(cv, noise, 0, 0, side_r, 11 * k, 7 * k, 1.05 * k, seed=i * 0.29 + 4, boil=f * 0.3)
            glow(cv, d[0] * 2, d[1] * 2, 3.5 * k, 0.9 * k, colour=(1.0, 0.96, 0.8))
            out.append(cv)
    return out


def muzzle_rocket(name, facings, frames):
    """A rocket leaving its tube: a short flash forward and a long blast of flame and smoke out of the back."""
    noise = Noise(seed_of(name))
    size = TILE + 32
    out = []
    for i in range(facings):
        d = facing_dir(i, facings)
        back = (-d[0], -d[1])
        for f in range(frames):
            cv = Canvas(size, size, size / 2, size / 2)
            k = [1.0, 0.65, 0.3][min(f, 2)]
            for j in range(4):
                dist = 9 + 6 * j + 5 * f
                puff(cv, noise, back[0] * dist, back[1] * dist - 2 * f, 3.5 + 1.2 * j + 2 * f, dens=0.5 - 0.1 * f,
                     colour=SMOKE * 1.5, seed=i * 0.3 + j, boil=f * 0.25)
            glow(cv, 0, 0, 8 * k, 0.5 * k)
            flame(cv, noise, 0, 0, back, 20 * k, 10 * k, 1.05 * k, seed=i * 0.41, boil=f * 0.3)
            flame(cv, noise, 0, 0, d, 10 * k, 7 * k, 1.1 * k, seed=i * 0.19 + 3, boil=f * 0.3)
            out.append(cv)
    return out


def shell(name, facings):
    """A shell in flight: a hot slug with a short glowing trace behind it."""
    size = TILE // 2
    out = []
    for i in range(facings):
        d = facing_dir(i, facings)
        cv = Canvas(size, size, size / 2, size / 2)
        spark(cv, -d[0] * 10, -d[1] * 10, 0, 0, 1.4, 0.7, alpha=0.7)
        glow(cv, 0, 0, 2.2, 0.8, colour=(1.0, 0.85, 0.5))
        spark(cv, -d[0] * 2, -d[1] * 2, d[0] * 1.5, d[1] * 1.5, 1.3, 1.0)
        out.append(cv)
    return out


def rocket(name, facings, frames):
    """A rocket in flight: a dark body, a flickering flame out of the back. The renderer leaves the smoke trail."""
    noise = Noise(seed_of(name))
    size = TILE // 2 + 8
    out = []
    for i in range(facings):
        d = facing_dir(i, facings)
        back = (-d[0], -d[1])
        for f in range(frames):
            cv = Canvas(size, size, size / 2, size / 2)
            flame(cv, noise, back[0] * 4, back[1] * 4, back, 10 + 3 * f, 5, 1.1, seed=i * 0.4, boil=f * 0.5)
            glow(cv, back[0] * 5, back[1] * 5, 4, 0.45)
            reg = cv.region(0, 0, 8, 8)
            x, y, sl = reg
            along = x * d[0] + y * d[1]
            across = -x * d[1] + y * d[0]
            body = (np.abs(along) < 5) & (np.abs(across) < 1.6)
            lit = np.where(across * (d[0] - d[1]) < 0, 0.55, 0.3)
            cv.over(sl, np.array([0.5, 0.48, 0.42])[None, None, :] * lit[..., None] * 1.4, body.astype(float))
            out.append(cv)
    return out


def fire(name, frames):
    """A small fire that loops, for badly damaged buildings and vehicles: tongues of flame licking up, and a little
    smoke off the top."""
    rng = np.random.default_rng(seed_of(name))
    noise = Noise(seed_of(name))
    w, h = TILE * 3 // 4, TILE + 16
    tongues = [(rng.uniform(-10, 10), rng.uniform(18, 34), rng.uniform(0, 1)) for _ in range(7)]
    out = []
    for f in range(frames):
        s = f / frames
        cv = Canvas(w, h, w / 2, h - 6)
        # Loop by walking the noise round a circle.
        bu, bv = math.cos(2 * math.pi * s) * 0.15, math.sin(2 * math.pi * s) * 0.15
        puff(cv, noise, math.sin(2 * math.pi * s) * 1.5, -h * 0.66, 9, dens=0.45, seed=2 + bu, boil=bv)
        glow(cv, 0, -8, 12, 0.35)
        for x, length, ph in tongues:
            sway = math.sin(2 * math.pi * (s + ph)) * 2.5
            lk = 0.8 + 0.2 * math.sin(2 * math.pi * (2 * s + ph))
            d = (sway / length, -1.0)
            n = math.hypot(*d)
            flame(cv, noise, x, 0, (d[0] / n, d[1] / n), length * lk, 11, 1.05, seed=x + bu * 3, boil=bv * 3)
        out.append(cv)
    return out


EFFECTS = {
    "explosion_small": lambda n: (1, explosion(n, 1.0, 8, puffs=14, debris=0)),
    "explosion_medium": lambda n: (1, explosion(n, 1.75, 12, puffs=26, debris=6)),
    "explosion_large": lambda n: (1, explosion(n, 2.75, 14, puffs=40, debris=10)),
    "smoke_puff": lambda n: (1, smoke_puff(n, 8)),
    "dust_puff": lambda n: (1, dust_puff(n, 8)),
    "hit_spark": lambda n: (1, hit_spark(n, 4)),
    "muzzle_flash_gun": lambda n: (16, muzzle_gun(n, 16, 3)),
    "muzzle_flash_rocket": lambda n: (16, muzzle_rocket(n, 16, 3)),
    "shell": lambda n: (16, shell(n, 16)),
    "rocket": lambda n: (16, rocket(n, 16, 2)),
    "fire": lambda n: (1, fire(n, 8)),
}


# ---- Packing ------------------------------------------------------------------------------------------------------


def trim(img):
    """The image cropped to its visible pixels, and the crop's top left. A blank frame keeps one pixel."""
    a = np.asarray(img)[..., 3]
    ys, xs = np.nonzero(a > 2)
    if len(xs) == 0:
        return img.crop((0, 0, 1, 1)), (0, 0)
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    return img.crop((x0, y0, x1, y1)), (x0, y0)


def pack(effects, width=1024):
    """Shelf-pack every frame onto one page. Returns the page and, per effect, its frames as [x, y, w, h, px, py]."""
    items = []
    for name, (facings, canvases) in effects.items():
        for i, cv in enumerate(canvases):
            img, (ox, oy) = trim(cv.image())
            items.append((name, i, img, round(cv.px - ox), round(cv.py - oy)))
    order = sorted(range(len(items)), key=lambda k: (-items[k][2].height, items[k][0], items[k][1]))
    placed = {}
    x = y = shelf = 0
    for k in order:
        img = items[k][2]
        if x + img.width + 1 > width:
            x, y, shelf = 0, y + shelf + 1, 0
        placed[k] = (x, y)
        x += img.width + 1
        shelf = max(shelf, img.height)
    height = 1 << max(0, (y + shelf).bit_length())
    page = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    frames = {name: [] for name in effects}
    for k, (name, i, img, pvx, pvy) in enumerate(items):
        px, py = placed[k]
        page.paste(img, (px, py))
        frames[name].append([px, py, img.width, img.height, pvx, pvy])
    return page, frames


def sheet(effects, path, ground=(196, 170, 120)):
    """A contact sheet at 2x: every frame of every effect on sand, one effect per row (facings 0, 4, 8, 12 only)."""
    rows = []
    for name, (facings, canvases) in effects.items():
        per = len(canvases) // facings
        pick = [canvases[f * per + j] for f in range(0, facings, max(1, facings // 4)) for j in range(per)]
        imgs = [cv.image() for cv in pick]
        rows.append(imgs)
    gap = 6
    W = max(sum(i.width for i in r) + gap * (len(r) + 1) for r in rows)
    H = sum(max(i.height for i in r) + gap for r in rows) + gap
    out = Image.new("RGBA", (W, H), ground + (255,))
    y = gap
    for r in rows:
        x = gap
        for i in r:
            out.alpha_composite(i, (x, y))
            x += i.width + gap
        y += max(i.height for i in r) + gap
    out = out.resize((W * 2, H * 2), Image.NEAREST)
    out.save(path)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", default=str(ROOT / "settings/generic/art/sprites"))
    ap.add_argument("--only", nargs="*", help="draw these effects only, for a contact sheet; writes no page")
    ap.add_argument("--sheet", help="also write a contact sheet here")
    args = ap.parse_args()
    names = args.only or list(EFFECTS)
    effects = {}
    for name in names:
        if name not in EFFECTS:
            sys.exit(f"no effect {name}; known: {', '.join(EFFECTS)}")
        effects[name] = EFFECTS[name](name)
    if args.sheet:
        sheet(effects, args.sheet)
    if args.only:
        return
    out = Path(args.out)
    page, frames = pack(effects)
    (out / "effects").mkdir(parents=True, exist_ok=True)
    page.save(out / f"{PAGE}.png", optimize=True)
    for name, (facings, canvases) in effects.items():
        doc = {"id": name, "atlas": PAGE, "scale": 2, "parts": {"effect": {"facings": facings, "anims": {
            "idle": {"length": len(canvases) // facings, "frames": frames[name]}}}}}
        (out / "effects" / f"{name}.json").write_text(json.dumps(doc, separators=(", ", ": ")) + "\n")
    print(f"{len(effects)} effects, {sum(len(c) for _, c in effects.values())} frames, page {page.size}")


if __name__ == "__main__":
    main()
