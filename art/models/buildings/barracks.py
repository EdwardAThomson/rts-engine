"""Barracks: two low dormitory blocks in an L, with the entrance on the south side of the east block over the exit
tile (one row below the east column), a training yard with low obstacle walls in the south-west, and a flag mast
flying a plain team-colour flag. Two tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5                     # ground contact stays this far inside the footprint
BLOCK_H = 4.6
# The entrance sits under the exit tile; for a 2-wide footprint that is the east column.
DOOR_X, DOOR_W, DOOR_H = W * 0.74, 3.4, 3.0
NORTH = (W / 2, -INSET - 3.6, W - 2 * INSET - 1.0, 7.0)          # north block: centre x, centre y, width, depth
EAST = (W * 0.74, -H * 0.62, 7.6, H * 0.58)                    # east block
MAST = (INSET + 2.0, -H + INSET + 2.0)
# Small parts the classic style leaves out.
DETAIL = {"window", "door_lamp", "aircon", "bar", "sign", "step", "tyre", "mast_cap", "crate"}
# Parts the studio's door frames move or hide; all built by door() under one group per side.
DOORS = {"door", "door_frame", "door_canopy", "door_lamp"}
# Parts that move in the idle overlay: the flag.
IDLE_PARTS = {"flag"}


def _block(root, spec, conc, armour, paint, steel, glass, name, windows_south):
    x, y, w, l = spec
    st.block((w, l, BLOCK_H), (x, y, BLOCK_H / 2 + 0.3), mat=conc, parent=root, bevel=0.1, name=name)
    # Shallow gable roof (ridge along the block's long axis) with a team stripe along its south eave.
    along_x = w >= l
    span, run = (l, w) if along_x else (w, l)
    for side in (-1, 1):
        off = side * span / 4
        loc = (x, y + off, BLOCK_H + 0.9) if along_x else (x + off, y, BLOCK_H + 0.9)
        rot = (-side * math.radians(14), 0, 0) if along_x else (0, side * math.radians(14), 0)
        # one half 4 cm shorter, so the two halves' gable ends don't share a plane (that renders dark)
        r = run + (0.6 if side < 0 else 0.56)
        size = (r, span / 2 + 0.5, 0.35) if along_x else (span / 2 + 0.5, r, 0.35)
        st.block(size, loc, rot=rot, mat=armour, parent=root, bevel=0.04, name=f"{name}_roof")
    if along_x:
        st.block((run + 0.7, 0.7, 0.45), (x, y - l / 2 - 0.15, BLOCK_H + 0.45), mat=paint, parent=root,
                 name="roof_band")
    else:
        st.block((w + 0.9, 0.7, 0.45), (x, y - l / 2 - 0.15, BLOCK_H + 0.45), mat=paint, parent=root,
                 name="roof_band")
        st.block((0.7, l + 0.6, 0.41), (x - w / 2 - 0.15, y, BLOCK_H + 0.45), mat=paint, parent=root,
                 name="roof_band")
    for k, wx in enumerate(windows_south):
        st.block((1.4, 0.12, 1.0), (wx, y - l / 2 - 0.04, 2.8), mat=glass, parent=root, name="window")
    st.block((1.3, 1.0, 0.8), (x + (w / 2 - 1.4 if along_x else 0), y + (0 if along_x else l / 2 - 1.6),
                               BLOCK_H + 1.6), mat=steel, parent=root, bevel=0.05, name="aircon")


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    dirt = st.concrete("yard", (0.42, 0.33, 0.22))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    nx, ny, nw, nl = NORTH
    _block(root, NORTH, conc, armour, paint, steel, glass, "dorm",
           [nx - nw / 2 + 2.0 + k * 2.6 for k in range(5)])
    ex, ey, ew, el = EAST
    _block(root, EAST, conc, armour, paint, steel, glass, "hall", [])
    for k in range(3):
        st.block((0.12, 1.4, 1.0), (ex - ew / 2 - 0.04, ey + el / 2 - 2.0 - k * 2.6, 2.8), mat=glass, parent=root,
                 name="window")

    # Training yard: packed earth with low obstacle walls, a frame of bars and a stack of tyres.
    yx0, yx1 = INSET + 0.6, ex - ew / 2 - 1.0
    yy0, yy1 = -H + INSET + 0.6, ny - nl / 2 - 1.0
    yw, yl = yx1 - yx0, yy1 - yy0
    st.block((yw, yl, 0.1), ((yx0 + yx1) / 2, (yy0 + yy1) / 2, 0.33), mat=dirt, parent=root, bevel=0.03,
             name="yard")
    for k in range(3):
        st.block((2.6, 0.5, 0.9 + 0.3 * k), (yx0 + 4.5 + (k % 2) * 2.0, yy1 - 1.6 - k * 2.3, 0.75 + 0.15 * k),
                 mat=conc, parent=root, bevel=0.05, name="obstacle")
    bx, by = yx1 - 1.6, yy1 - 2.5
    for sy in (-1, 1):
        st.block((0.3, 0.3, 2.6), (bx, by + sy * 1.4, 1.6), mat=steel, parent=root, name="bar_post")
    st.block((0.28, 3.06, 0.28), (bx, by, 2.8), mat=steel, parent=root, name="bar")
    for k in range(3):
        st.cylinder(0.55, 0.35, (yx1 - 1.4, yy0 + 1.3, 0.55 + 0.35 * k), mat=st.rubber(), parent=root, verts=16,
                    name="tyre")

    # Flag mast with a team-colour flag, the barracks' silhouette feature.
    mx, my = MAST
    st.block((1.4, 1.4, 0.5), (mx, my, 0.55), mat=conc, parent=root, bevel=0.05, name="mast_base")
    st.cylinder(0.2, 11.0, (mx, my, 5.8), mat=steel, parent=root, verts=10, name="mast")
    st.sphere(0.3, (mx, my, 11.4), mat=steel, parent=root, name="mast_cap")
    st.block((3.2, 0.25, 2.0), (mx + 1.8, my, 9.9), rot=(0, 0, -0.12), mat=paint, parent=root, name="flag")

    door(root, "south")


def door(root, side="south"):
    """The entrance: a framed double door under a canopy with a lamp, built in one group on the hall block's wall
    for `side`. Only the south entrance is built today; east and west sit on the hall block's side walls."""
    ex, ey, ew, el = EAST
    dark, armour = st.dark_steel("door_dark"), st.armour("door_paint")
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    place = {"south": ((DOOR_X, ey - el / 2, 0.3), 0.0),
             "east": ((ex + ew / 2, ey, 0.3), math.pi / 2),
             "west": ((ex - ew / 2, ey, 0.3), -math.pi / 2)}[side]
    g = st.group(f"door_{side}", place[0], parent=root)
    g.rotation_euler[2] = place[1]
    # Local frame: the door faces -y, centred on x = 0, standing on z = 0.
    st.block((DOOR_W + 0.8, 0.4, DOOR_H + 0.5), (0, -0.15, (DOOR_H + 0.5) / 2), mat=dark, parent=g,
             name="door_frame")
    for sx in (-1, 1):
        st.block((DOOR_W / 2 - 0.1, 0.25, DOOR_H), (sx * DOOR_W / 4, -0.35, DOOR_H / 2), mat=armour, parent=g,
                 bevel=0.03, name="door")
    st.block((DOOR_W + 2.0, 1.8, 0.3), (0, -1.0, DOOR_H + 0.8), rot=(math.radians(-6), 0, 0), mat=armour, parent=g,
             bevel=0.04, name="door_canopy")
    st.block((DOOR_W + 1.6, 1.2, 0.3), (0, -1.0, 0.15), mat=st.concrete("step"), parent=g, bevel=0.04,
             name="step")
    st.block((0.4, 0.3, 0.3), (DOOR_W / 2 + 0.6, -0.5, DOOR_H + 0.3), mat=lamp, parent=g, name="door_lamp")


def damage(rng, root):
    """Damaged frame: scorch marks on the roofs, the flag gone and the mast leaning. Not rendered yet; the studio
    switches it on with building frames."""
    scorch = st.plain("scorch", (0.03, 0.025, 0.02), roughness=0.95)
    for spec in (NORTH, EAST):
        x, y, w, l = spec
        for _ in range(2):
            s = rng.uniform(1.2, 2.4)
            st.block((s, s * rng.uniform(0.6, 1.0), 0.1),
                     (x + rng.uniform(-w / 3, w / 3), y + rng.uniform(-l / 3, l / 3), BLOCK_H + 1.4),
                     rot=(0, 0, rng.uniform(0, math.pi)), mat=scorch, parent=root, bevel=0, name="scorch")
    for o in root.children_recursive:
        if o.name.startswith("flag"):
            o.hide_render = True
        if o.name.startswith("mast"):
            o.rotation_euler[1] += rng.uniform(0.05, 0.12)
