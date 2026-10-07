"""Super unit A: a tracked emitter carrier. A bank of six square horns sits fixed on the hull, fed by a generator
block with cooling fins behind it; there is no turret, so the hull turns to aim. About 7 m long, low and wide so
the horn bank is the silhouette. Our own design in the battle tank's house style: sandy armour, team-coloured
skirts, fenders and generator roof. Front is +y (north), 1 unit = 1 metre."""
import math

import rts_studio as st

L, W = 7.0, 3.6
TRACK_W, TRACK_H = 0.8, 0.9
HULL_Z = 0.55
DECK_Z = 1.45
HORN_Z = 2.35            # centre of the horn bank
# Small parts the classic style leaves out.
DETAIL = {"link", "wheel", "sprocket", "fin", "conduit", "bolt", "vision", "hatch", "exhaust", "brace"}


def build_hull(root):
    paint, team = st.armour("hull_paint"), st.team_paint("hull_team")
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    throat = st.plain("throat", (0.03, 0.03, 0.035), 0.95)
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        st.block((TRACK_W, L - 0.3, TRACK_H), (x, 0, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.3, parent=root,
                 name="track")
        n = 30
        for i in range(n):
            y = -(L - 0.9) / 2 + (L - 0.9) * i / (n - 1)
            st.block((TRACK_W + 0.04, 0.1, 0.06), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        for i in range(7):
            y = -2.6 + 5.2 * i / 6
            st.cylinder(0.34, 0.14, (x + side * 0.38, y, 0.4), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
        for y in (-(L - 0.6) / 2, (L - 0.6) / 2):
            st.cylinder(0.38, 0.16, (x + side * 0.38, y, 0.55), rot=(0, math.pi / 2, 0), mat=steel, verts=12,
                        parent=root, name="sprocket")
        st.block((0.14, L - 1.3, 0.55), (side * (W / 2 + 0.03), 0, 0.8), mat=team, bevel=0.03, parent=root,
                 name="skirt")
        st.block((0.7, L - 1.0, 0.08), (x + side * 0.05, 0, TRACK_H + 0.12), mat=team, bevel=0.02, parent=root,
                 name="fender")
    # hull: a flat box with a short sloped nose
    body_w = W - 2 * TRACK_W + 0.6
    st.block((body_w, L - 1.4, DECK_Z - HULL_Z), (0, -0.2, (HULL_Z + DECK_Z) / 2), mat=paint, bevel=0.06,
             parent=root, name="body")
    # 4 cm narrower than the body, so their sides don't share a plane (coincident faces render dark).
    st.block((body_w - 0.04, 1.3, 0.3), (0, 2.75, DECK_Z - 0.32), rot=(math.radians(-26), 0, 0), mat=paint,
             bevel=0.05, parent=root, name="glacis")
    st.block((body_w - 0.1, 1.2, 0.65), (0, 2.65, 0.9), mat=paint, bevel=0.06, parent=root, name="nose")
    st.block((0.45, 0.12, 0.12), (-0.75, 2.95, DECK_Z - 0.12), mat=dark, bevel=0.01, parent=root, name="vision")
    st.cylinder(0.24, 0.1, (-0.75, 2.45, DECK_Z + 0.02), mat=paint, verts=16, bevel=0.02, parent=root,
                name="hatch")

    # Horn bank: a 3 x 2 grid of square horns flaring forward from a back plate, in a heavy frame on a cradle.
    bank = st.group("horn_bank", (0, 0.9, HORN_Z), parent=root)
    bank.rotation_euler = (math.radians(14), 0, 0)  # tilted up so the mouths read from the camera
    st.block((2.9, 1.5, 0.35), (0, -0.2, -0.75), mat=paint, bevel=0.05, parent=bank, name="cradle")
    st.block((2.9, 0.35, 1.55), (0, -0.85, 0), mat=paint, bevel=0.05, parent=bank, name="back_plate")
    for c in range(3):
        for r in range(2):
            x, z = (c - 1) * 0.92, (r - 0.5) * 0.72
            # a four-sided cone turned 45 degrees is a square horn
            st.cone(0.18, 0.48, 1.2, (x, -0.1, z), rot=(-math.pi / 2, math.pi / 4, 0), mat=steel, verts=4,
                    parent=bank, name="horn")
            st.block((0.62, 0.05, 0.62), (x, 0.47, z), mat=throat, bevel=0.0, parent=bank, name="horn_mouth")
    # frame around the bank: open top rails (so the horns show from above), a bottom rail and side cheeks; the
    # front top rail carries a team band. The rails stop 2 cm inside the cheeks' outer faces and edges.
    st.block((3.06, 0.22, 0.2), (0, 0.4, 0.84), mat=team, bevel=0.03, parent=bank, name="frame_top")
    st.block((3.06, 0.22, 0.2), (0, -0.8, 0.84), mat=paint, bevel=0.03, parent=bank, name="frame_rear")
    st.block((3.06, 1.36, 0.16), (0, -0.2, -0.8), mat=paint, bevel=0.03, parent=bank, name="frame_bottom")
    for side in (-1, 1):
        st.block((0.18, 1.4, 1.8), (side * 1.46, -0.2, 0), mat=paint, bevel=0.03, parent=bank, name="cheek")
        st.block((0.26, 0.9, 0.26), (side * 1.1, -1.1, -0.65), rot=(math.radians(35), 0, 0), mat=steel,
                 bevel=0.02, parent=bank, name="brace")
    for x in (-1.3, -0.45, 0.45, 1.3):
        st.cylinder(0.05, 0.05, (x, -1.03, 0.6), rot=(math.pi / 2, 0, 0), mat=steel, verts=8, parent=bank,
                    name="bolt")

    # Generator block behind the bank: team roof panel, cooling fins on both sides, two short exhausts.
    gz = DECK_Z + 0.5
    st.block((2.3, 2.1, 1.0), (0, -1.9, gz), mat=paint, bevel=0.06, parent=root, name="generator")
    st.block((0.5, 1.9, 0.08), (0, -1.9, gz + 0.53), mat=team, bevel=0.02, parent=root, name="generator_band")
    for x in (-0.65, 0.65):
        st.block((0.55, 1.5, 0.12), (x, -1.9, gz + 0.55), mat=dark, bevel=0.02, parent=root, name="vent")
    for side in (-1, 1):
        for i in range(6):
            st.block((0.28, 0.08, 0.8), (side * 1.25, -1.1 - 0.3 * i, gz), mat=dark, bevel=0.01, parent=root,
                     name="fin")
        st.cylinder(0.13, 0.45, (side * 0.65, -3.05, gz + 0.2), rot=(math.pi / 2, 0, 0), mat=steel, verts=12,
                    parent=root, name="exhaust")
    # thick conduits from the generator into the back of the bank
    for x in (-0.6, 0.6):
        st.cylinder(0.14, 1.0, (x, -0.55, gz + 0.25), rot=(math.pi / 2, 0, 0), mat=dark, verts=12, parent=root,
                    name="conduit")


def wreck(rng, root):
    """Stub for the studio's wreck pass (batch J): the hull with the horn bank knocked askew and blackened."""
    build_hull(root)
    for o in root.children:
        if o.name.startswith("horn_bank"):
            o.rotation_euler = (rng.uniform(-0.25, 0.1), rng.uniform(-0.2, 0.2), rng.uniform(-0.3, 0.3))
