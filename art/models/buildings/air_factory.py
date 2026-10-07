"""Air factory: a hangar with a curved roof whose wide door opens south over the exit tile (one row below the middle
column), a landing pad in the north-east, and a small control tower in the south-east corner. Three tiles by two, built in
proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5                     # ground contact stays this far inside the footprint
WALL_H = 4.2                    # the hangar's side walls; the curved roof rises above them
# The hangar runs north-south and opens south, centred on the exit tile (the middle column).
HANGAR = (W / 2 - 1.2, -H * 0.43, 15.0, H * 0.72)      # centre x, centre y, width, depth
DOOR_X = HANGAR[0]
DOOR_W, DOOR_H = HANGAR[2] - 2.4, 6.0
PAD = (W - INSET - 5.0, -INSET - 4.9, 4.4)             # centre x, centre y, radius
# Small parts the classic style leaves out.
DETAIL = {"door_lamp", "door_slat", "window", "rib", "beacon", "antenna", "pad_light", "stripe", "chock"}
# Parts the studio's door frames move or hide; all built by door() under one group per side.
DOORS = {"door", "door_slat", "door_frame", "door_lamp", "door_arch"}
# Parts that move in the idle overlay: the tower's radar bar turns and the beacons blink.
IDLE_PARTS = {"radar_bar", "beacon"}


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    white = st.plain("marking", (0.7, 0.68, 0.62))
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Hangar: low concrete side walls under a barrel-vault roof running north-south, with steel ribs and two
    # team-colour bands across the vault. The rear (north) end is closed by a flat wall.
    hx, hy, hw, hl = HANGAR
    r = hw / 2
    rise = 0.62                 # the vault's height as a share of its half-width
    st.block((hw, hl, WALL_H), (hx, hy, WALL_H / 2 + 0.3), mat=conc, parent=root, bevel=0.1, name="hangar")
    st.cylinder(r, hl, (hx, hy, WALL_H + 0.3), rot=(math.pi / 2, 0, 0), scale=(1, rise, 1), mat=armour,
                parent=root, verts=48, name="vault")
    for k in range(6):
        y = hy - hl / 2 + 0.6 + k * (hl - 1.2) / 5
        st.cylinder(r + 0.12, 0.4, (hx, y, WALL_H + 0.3), rot=(math.pi / 2, 0, 0), scale=(1, rise, 1), mat=steel,
                    parent=root, verts=48, name="rib")
    for y in (hy - hl / 2 + 2.6, hy + hl / 2 - 2.6):
        st.cylinder(r + 0.1, 1.4, (hx, y, WALL_H + 0.3), rot=(math.pi / 2, 0, 0), scale=(1, rise, 1), mat=paint,
                    parent=root, verts=48, name="roof_band")
    st.block((1.0, hl - 1.0, 0.5), (hx, hy, WALL_H + 0.3 + r * rise + 0.05), mat=dark, parent=root, name="ridge")
    for sx in (-1, 1):
        for k in range(3):
            st.block((0.12, 1.6, 0.8), (hx + sx * (hw / 2 + 0.04), hy + hl / 2 - 3.5 - k * 3.4, 2.6), mat=glass,
                     parent=root, name="window")

    # Lean-to workshop on the west side of the hangar.
    lx0 = INSET + 0.4
    lw = hx - hw / 2 - lx0
    ll = hl * 0.55
    ly = hy + hl / 2 - ll / 2
    st.block((lw, ll, 3.6), (lx0 + lw / 2, ly, 2.1), mat=conc, parent=root, bevel=0.08, name="workshop")
    st.wedge((lw + 0.4, ll + 0.4, 0.9), (lx0 + lw / 2, ly, 4.3), slope_front=0.7, rot=(0, 0, -math.pi / 2),
             mat=steel, parent=root, name="workshop_roof")
    st.block((lw * 0.7, 0.12, 0.9), (lx0 + lw / 2, ly - ll / 2 - 0.04, 2.6), mat=glass, parent=root,
             name="window")

    # Landing pad in the north-east: a raised disc with a marking ring, a team-colour band and corner lights.
    px, py, pr = PAD
    st.cylinder(pr, 0.5, (px, py, 0.55), mat=conc, parent=root, verts=40, name="landing_pad")
    st.cylinder(pr - 0.5, 0.06, (px, py, 0.83), mat=white, parent=root, verts=40, name="pad_ring")
    st.cylinder(pr - 1.1, 0.07, (px, py, 0.85), mat=st.concrete("pad_inner"), parent=root, verts=40,
                name="pad_inner")
    # A team-colour ring and a plain touchdown square; no cross, which could read as a protected emblem.
    st.cylinder(pr - 1.6, 0.08, (px, py, 0.9), mat=paint, parent=root, verts=40, name="pad_band")
    st.cylinder(pr - 2.5, 0.09, (px, py, 0.91), mat=st.concrete("pad_core"), parent=root, verts=40,
                name="pad_core")
    st.block((1.6, 1.6, 0.1), (px, py, 0.95), rot=(0, 0, math.pi / 4), mat=stripe, parent=root, bevel=0.02,
             name="pad_mark")
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        st.block((0.4, 0.4, 0.4), (px + math.cos(a) * (pr - 0.2), py + math.sin(a) * (pr - 0.2), 0.95), mat=lamp,
                 parent=root, name="pad_light")

    # Control tower in the south-east corner, clear of the pad on screen: a stalk, a glazed cab and a radar bar.
    cx, cy = W - INSET - 3.0, -H + INSET + 3.4
    st.block((3.0, 3.0, 5.0), (cx, cy, 2.8), mat=conc, parent=root, bevel=0.08, name="tower")
    st.block((4.2, 4.2, 1.8), (cx, cy, 6.2), mat=steel, parent=root, bevel=0.06, name="tower_cab")
    for sx, sy, w, l in ((0, -2.12, 3.6, 0.1), (2.12, 0, 0.1, 3.6), (-2.12, 0, 0.1, 3.6)):
        st.block((w, l, 1.0), (cx + sx, cy + sy, 6.3), mat=glass, parent=root, name="tower_glass")
    st.block((4.6, 4.6, 0.4), (cx, cy, 7.3), mat=armour, parent=root, bevel=0.04, name="tower_roof")
    st.block((4.7, 0.6, 0.45), (cx, cy - 2.05, 7.3), mat=paint, parent=root, name="tower_band")
    st.block((3.2, 0.5, 0.4), (cx, cy, 8.0), rot=(0, 0, 0.6), mat=dark, parent=root, name="radar_bar")
    st.cylinder(0.25, 0.6, (cx, cy, 7.7), mat=steel, parent=root, verts=10, name="radar_post")
    st.block((0.3, 0.3, 2.4), (cx - 1.8, cy + 1.8, 8.6), mat=steel, parent=root, name="antenna")
    for bx in (INSET + 1.0, HANGAR[0] + HANGAR[2] / 2 + 1.6):
        st.block((0.6, 0.6, 2.0), (bx, -H + INSET + 1.0, 1.3), mat=dark, parent=root, name="beacon_post")
        st.block((0.5, 0.5, 0.5), (bx, -H + INSET + 1.0, 2.55), mat=lamp, parent=root, name="beacon")

    # Apron in front of the hangar door with a yellow guide line, and wheel chocks.
    ay0 = hy - hl / 2
    apron_l = (H - INSET) + ay0
    st.block((DOOR_W + 2.0, apron_l, 0.08), (DOOR_X, ay0 - apron_l / 2, 0.33), mat=dark, parent=root, bevel=0.02,
             name="apron")
    st.block((0.5, apron_l * 0.9, 0.04), (DOOR_X, ay0 - apron_l / 2, 0.38), mat=stripe, parent=root, bevel=0,
             name="stripe")
    for sx in (-1, 1):
        st.block((0.6, 0.8, 0.4), (DOOR_X + sx * 2.2, ay0 - apron_l + 1.2, 0.55), mat=stripe, parent=root,
                 name="chock")

    door(root, "south")


def door(root, side="south"):
    """The hangar door: a row of folding steel leaves under an arched header that follows the vault, built in one
    group on the hangar's end for `side`. Only the south door is built today; an east or west door would sit on a
    side wall under the vault's spring line, so it is lower (WALL_H) and narrower."""
    hx, hy, hw, hl = HANGAR
    steel, dark = st.steel("door_steel"), st.dark_steel("door_dark")
    armour = st.armour("door_paint")
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    if side == "south":
        loc, rot, w, h = (DOOR_X, hy - hl / 2, 0.3), 0.0, DOOR_W, DOOR_H
    else:
        sx = 1 if side == "east" else -1
        loc, rot, w, h = (hx + sx * hw / 2, hy, 0.3), sx * math.pi / 2, hl * 0.45, WALL_H - 0.6
    g = st.group(f"door_{side}", loc, parent=root)
    g.rotation_euler[2] = rot
    # Local frame: the door faces -y, centred on x = 0, standing on z = 0.
    st.block((w + 1.0, 0.5, h + 0.6), (0, -0.2, (h + 0.6) / 2), mat=dark, parent=g, name="door_frame")
    leaves = 6
    for k in range(leaves):
        x = -w / 2 + w * (k + 0.5) / leaves
        st.block((w / leaves - 0.12, 0.3, h), (x, -0.45, h / 2), rot=(0, 0, 0.06 * (1 if k % 2 else -1)),
                 mat=steel, parent=g, bevel=0.02, name="door")
        st.block((0.25, 0.12, h - 0.6), (x, -0.65, h / 2), mat=dark, parent=g, name="door_slat")
    if side == "south":
        # The vault's end above the door: an arch in armour, the same curve as the roof.
        r = hw / 2
        st.cylinder(r - 0.05, 0.6, (0, -0.05, WALL_H), rot=(math.pi / 2, 0, 0), scale=(1, 0.62, 1), mat=armour,
                    parent=g, verts=48, name="door_arch")
        st.block((w + 1.6, 0.9, 0.8), (0, -0.6, h + 0.4), mat=armour, parent=g, bevel=0.05, name="door_arch")
    for sx in (-1, 1):
        st.block((0.4, 0.3, 0.3), (sx * (w / 2 + 0.3), -0.9, h + 0.9), mat=lamp, parent=g, name="door_lamp")


def damage(rng, root):
    """Damaged frame: scorch marks on the vault, a rib torn loose and the radar bar gone. Not rendered yet; the
    studio switches it on with building frames."""
    scorch = st.plain("scorch", (0.03, 0.025, 0.02), roughness=0.95)
    hx, hy, hw, hl = HANGAR
    top = WALL_H + 0.3 + hw / 2 * 0.62
    for _ in range(4):
        s = rng.uniform(1.5, 3.0)
        st.block((s * 0.6, s, 0.1), (hx + rng.uniform(-2.0, 2.0), hy + rng.uniform(-hl / 3, hl / 3), top + 0.1),
                 rot=(0, 0, rng.uniform(0, math.pi)), mat=scorch, parent=root, bevel=0, name="scorch")
    for o in root.children_recursive:
        if o.name.startswith("radar_bar"):
            o.hide_render = True
        if o.name.startswith("rib") and rng.random() < 0.2:
            o.rotation_euler[2] += rng.uniform(-0.1, 0.1)
