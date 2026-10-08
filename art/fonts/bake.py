"""Bake the generic pack's two fonts into glyph atlases the player draws as sprites.

The faces are open-licence (SIL OFL 1.1) and kept here as WOFF2 with their licences, as `plans/rts/art-pipeline.md`
section 9 asks: a squared display face for headings and numbers, and a plain sans for body text. The player has no
font rasteriser of its own, so this turns each text style, at UI scales 1, 2 and 3, into a white glyph atlas (the
alpha is the coverage; the player tints it) and writes `settings/generic/theme/fonts/`:

  fonts.json            styles, faces and, per atlas, every glyph's place, offset and advance
  <style>-<scale>.png   the atlases
  OFL-<face>.txt        the licences, which travel with the fonts
  provenance.jsonl      where each file came from

Run from the repository root: python3 art/fonts/bake.py   (needs Pillow built with FreeType, which reads WOFF2)
The output is the same on every run with the same Pillow and FreeType.
"""

import json
import os
import re
import shutil
import sys

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "settings", "generic", "theme", "fonts")
PACK_DIR = "theme/fonts"

# Face ids are generic; the family names are only credits.
FACES = {
    "body": {"family": "Inter", "weight": 500, "file": "inter-latin-500-normal.woff2", "licence": "OFL-inter.txt"},
    "display": {
        "family": "Oxanium",
        "weight": 600,
        "file": "oxanium-latin-600-normal.woff2",
        "licence": "OFL-oxanium.txt",
    },
}

# Text styles and their size in pixels at UI scale 1.
STYLES = {
    "small": ("body", 11),
    "body": ("body", 14),
    "heading": ("display", 16),
    "title": ("display", 30),
}
SCALES = [1, 2, 3]
CHARS = [chr(c) for c in range(32, 127)]
ATLAS_W = 512
PAD = 1


def bake(style, face, px, scale):
    font = ImageFont.truetype(os.path.join(HERE, "src", FACES[face]["file"]), px * scale)
    ascent, descent = font.getmetrics()
    glyphs, x, y, row_h = [], PAD, PAD, 0
    pictures = []
    for ch in CHARS:
        x0, y0, x1, y1 = font.getbbox(ch)
        w, h = max(x1 - x0, 0), max(y1 - y0, 0)
        advance = round(font.getlength(ch) * 64)
        if w == 0 or h == 0:
            glyphs.append([ord(ch), 0, 0, 0, 0, 0, 0, advance])
            continue
        if x + w + PAD > ATLAS_W:
            x, y, row_h = PAD, y + row_h + PAD, 0
        g = Image.new("L", (w, h), 0)
        ImageDraw.Draw(g).text((-x0, -y0), ch, font=font, fill=255)
        pictures.append((g, x, y))
        glyphs.append([ord(ch), x, y, w, h, x0, y0, advance])
        x += w + PAD
        row_h = max(row_h, h)
    height = y + row_h + PAD
    alpha = Image.new("L", (ATLAS_W, height), 0)
    for g, gx, gy in pictures:
        alpha.paste(g, (gx, gy))
    atlas = Image.merge("RGBA", (Image.new("L", alpha.size, 255),) * 3 + (alpha,))
    name = f"{style}-{scale}.png"
    atlas.save(os.path.join(OUT, name), optimize=True)
    return {
        "style": style,
        "scale": scale,
        "file": f"{PACK_DIR}/{name}",
        "line": ascent + descent,
        "ascent": ascent,
        "glyphs": glyphs,
    }


def main():
    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)
    atlases = [bake(s, face, px, k) for s, (face, px) in STYLES.items() for k in SCALES]
    faces = {}
    provenance = []
    for fid, f in FACES.items():
        shutil.copy(os.path.join(HERE, "src", f["licence"]), os.path.join(OUT, f["licence"]))
        faces[fid] = {
            "family": f["family"],
            "weight": f["weight"],
            "source": f"art/fonts/src/{f['file']}",
            "licence": f"{PACK_DIR}/{f['licence']}",
        }
    for a in atlases:
        f = FACES[STYLES[a["style"]][0]]
        provenance.append({
            "file": a["file"],
            "source": "open-licence font",
            "made_by": f"art/fonts/bake.py from art/fonts/src/{f['file']} ({f['family']}, via the Fontsource npm package)",
            "licence": "OFL-1.1",
        })
    index = {
        "about": (
            "The generic pack's fonts, baked by art/fonts/bake.py from two SIL OFL 1.1 faces (licences beside them). "
            "Each style is baked at UI scales 1, 2 and 3 into a white atlas whose alpha is the coverage. A glyph is "
            "[code, x, y, w, h, left, top, advance]: its box in the atlas, where its box sits from the pen (left) "
            "and from the line's top (top), and how far the pen moves after it, in 64ths of a pixel. 'line' is the "
            "line height and 'ascent' the baseline's distance from the line's top, in pixels."
        ),
        "faces": faces,
        "styles": {s: {"face": face, "px": px} for s, (face, px) in STYLES.items()},
        "atlases": atlases,
    }
    with open(os.path.join(OUT, "fonts.json"), "w") as fh:
        # One glyph a line, so a change shows up as a readable diff.
        text = json.dumps(index, indent=1, separators=(",", ": "))
        text = re.sub(r"\[\s+(-?\d+(?:,\s+-?\d+)*)\s+\]", lambda m: "[" + re.sub(r",\s+", ", ", m.group(1)) + "]", text)
        fh.write(text + "\n")
    with open(os.path.join(OUT, "provenance.jsonl"), "w") as fh:
        for p in provenance:
            fh.write(json.dumps(p) + "\n")
    print(f"baked {len(atlases)} atlases into {os.path.relpath(OUT, ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
