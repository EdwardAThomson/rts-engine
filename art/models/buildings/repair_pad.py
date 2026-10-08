"""Repair pad: an open concrete apron with a service bay in the middle column, open to the south so a vehicle drives
straight in. Two gantry towers carry an overhead beam with a hoist across the bay; a control booth stands at the
west end and tool racks and a parts store at the east end. Three tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 2)
T = st.STUDIO["metres_per_tile"]
INSET = 0.5
# Small parts the classic style leaves out.
DETAIL = {"bolt", "rack_tool", "cable", "light", "rail", "vent", "window_frame"}
# Parts that move in an idle overlay (the hoist runs along the beam) and the parts a door frame opens.
IDLE_PARTS = {"hoist", "hook"}
DOORS = set()


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W - 2 * INSET, H - 2 * INSET, 0.35), (W / 2, -H / 2, 0.175), mat=conc, parent=root, bevel=0.1,
             name="pad")

    # Service bay: a sunken steel deck in the middle column, hazard stripes along both sides and the south lip.
    bx, bw, bl = W / 2, T * 0.9, H - 3.0
    by = -H / 2 - 0.6
    gy = by + 1.0
    st.block((bw, bl, 0.2), (bx, by, 0.42), mat=dark, parent=root, bevel=0.05, name="bay")
    for side in (-1, 1):
        st.block((0.6, bl, 0.12), (bx + side * (bw / 2 - 0.3), by, 0.56), mat=stripe, parent=root, name="kerb")
        for k in range(8):
            st.block((0.62, 0.5, 0.13), (bx + side * (bw / 2 - 0.3), by - bl / 2 + 0.9 + k * (bl - 1.8) / 7, 0.57),
                     mat=dark, parent=root, bevel=0.01, name="kerb_band")
    # Lift platform under the hoist, where the vehicle stands, with a team band round its edge.
    st.block((bw - 2.4, 7.0, 0.3), (bx, gy - 2.6, 0.62), mat=paint, parent=root, bevel=0.05, name="lift_band")
    st.block((bw - 3.0, 6.4, 0.3), (bx, gy - 2.6, 0.72), mat=steel, parent=root, bevel=0.05, name="lift")
    for k in range(4):
        st.block((bw - 3.4, 0.25, 0.08), (bx, gy - 5.2 + k * 1.75, 0.9), mat=dark, parent=root, name="rail")
    for k in range(5):
        st.block((1.6, 0.4, 0.08), (bx - bw / 2 + 1.4 + k * (bw - 2.8) / 4, -H + INSET + 0.9, 0.4),
                 rot=(0, 0, 0.6), mat=stripe, parent=root, name="stripe")

    # Two gantry towers either side of the bay and the beam across them, team-coloured on top.
    top = 9.0
    for side in (-1, 1):
        gx = bx + side * (bw / 2 + 1.3)
        for dy in (-1.4, 1.4):
            st.block((1.1, 1.1, top), (gx, gy + dy, top / 2 + 0.35), mat=steel, parent=root, name="leg")
        for z in (3.0, 6.0):
            st.block((0.4, 2.8, 0.4), (gx, gy, z), mat=steel, parent=root, name="brace")
        st.block((1.6, 4.2, 0.9), (gx, gy, 0.8), mat=armour, parent=root, bevel=0.08, name="foot")
        st.block((1.4, 3.6, 0.5), (gx, gy, top + 0.6), mat=dark, parent=root, name="cap")
        st.cylinder(0.3, 0.3, (gx, gy + 1.4, top + 1.0), mat=st.plain("light", (0.9, 0.5, 0.1), emission=2.0),
                    parent=root, verts=12, name="light")
    span = bw + 2 * 1.3 + 1.36  # 2 cm short of the caps' outer faces at each end
    st.block((span, 1.6, 1.2), (bx, gy, top + 1.3), mat=paint, parent=root, bevel=0.06, name="beam")
    st.block((span - 0.4, 0.3, 0.3), (bx, gy - 0.9, top + 0.9), mat=steel, parent=root, name="rail")
    st.block((1.8, 2.0, 1.2), (bx, gy, top + 0.1), mat=armour, parent=root, bevel=0.06, name="hoist")
    st.cylinder(0.12 * 2.2, 4.0, (bx, gy, top - 2.5), mat=dark, parent=root, verts=8, name="cable")
    st.block((0.9, 0.9, 0.6), (bx, gy, top - 4.8), mat=steel, parent=root, name="hook")

    # Arm on each tower reaching over the bay, with a tool head, so the pad reads as a workshop at sprite size.
    for side in (-1, 1):
        ax = bx + side * (bw / 2 + 1.3)
        st.block((3.6, 0.7, 0.66), (ax - side * 1.8, gy - 3.4, 5.0), mat=armour, parent=root, bevel=0.05, name="arm")
        st.block((0.7, 2.4, 0.7), (ax, gy - 2.2, 5.0), mat=steel, parent=root, name="arm_root")
        st.cylinder(0.45, 1.4, (ax - side * 3.6, gy - 3.4, 4.2), mat=dark, parent=root, verts=12, name="tool_head")

    # Control booth at the west end: concrete block, glass facing the bay, team stripe on the roof.
    cx, cy, cw, cl, ch = T * 0.5, -H * 0.36, T * 0.62, H * 0.5, 4.4
    st.block((cw, cl, ch), (cx, cy, ch / 2 + 0.35), mat=conc, parent=root, bevel=0.1, name="booth")
    st.block((cw + 0.2, cl + 0.2, 0.5), (cx, cy, ch + 0.5), mat=armour, parent=root, bevel=0.05, name="booth_roof")
    st.block((cw - 1.2, 1.0, 0.25), (cx, cy, ch + 0.85), mat=paint, parent=root, name="roof_stripe")
    st.block((0.12, cl - 1.6, 1.2), (cx + cw / 2 + 0.05, cy, ch - 0.6), mat=glass, parent=root, name="window")
    st.block((cw - 1.6, 0.12, 1.2), (cx, cy - cl / 2 - 0.05, ch - 0.6), mat=glass, parent=root, name="window")
    st.block((1.8, 0.2, 2.6), (cx - cw / 4, cy - cl / 2 - 0.1, 1.65), mat=armour, parent=root, name="door")
    for k in range(3):
        st.cylinder(0.4, 0.6, (cx - cw / 3 + k * cw / 3, cy + cl / 4, ch + 1.0), mat=steel, parent=root, verts=12,
                    name="vent")
    st.block((cw * 0.7, 2.4, 1.6), (cx, -H + INSET + 2.2, 1.15), mat=armour, parent=root, bevel=0.08,
             name="generator")

    # East end: two tool racks along the south and a parts store with drums along the north.
    ex = W - T * 0.5
    st.block((T * 0.62, H * 0.32, 3.2), (ex, -H * 0.22, 1.95), mat=armour, parent=root, bevel=0.08, name="store")
    st.block((T * 0.62 + 0.2, H * 0.32 + 0.2, 0.4), (ex, -H * 0.22, 3.7), mat=armour, parent=root, bevel=0.05,
             name="store_roof")
    st.block((T * 0.62 - 1.0, 1.0, 0.25), (ex, -H * 0.22, 4.0), mat=paint, parent=root, name="roof_stripe")
    st.block((3.4, 0.2, 2.6), (ex, -H * 0.22 - H * 0.16 - 0.1, 1.65), mat=dark, parent=root, name="store_door")
    for k, x in enumerate((ex - 2.6, ex + 0.2)):
        y = -H * 0.62
        st.block((2.4, 0.9, 2.6), (x, y, 1.65), mat=steel, parent=root, name="rack")
        for j in range(3):
            st.block((2.1, 1.0, 0.3), (x, y - 0.05, 0.9 + j * 0.8), mat=dark, parent=root, name="rack_tool")
    for k in range(4):
        st.cylinder(0.7, 1.6, (ex - 2.4 + k * 1.6, -H * 0.82, 1.15), mat=st.metal("drum"), parent=root, verts=16,
                    name="drum")
    st.cylinder(0.25, (W - T) / 2 + 0.5, ((cx + bx) / 2, cy + cl / 2 - 0.6, 0.6), rot=(0, math.pi / 2, 0),
                mat=dark, parent=root, verts=10, name="cable")


def damage(rng, root):
    """Damaged frame (wave 3): the hoist drops, the beam sags and a rack topples. Not rendered yet."""
    for o in root.children_recursive:
        base = o.name.split(".")[0]
        if base in ("hoist", "hook"):
            o.location.z -= 2.5 + rng.random()
        elif base == "beam":
            o.rotation_euler[1] = rng.uniform(-0.06, 0.06)
        elif base == "rack" and rng.random() < 0.5:
            o.rotation_euler[0] = rng.uniform(0.3, 0.8)
