"""Gun turret: a squat eight-sided concrete bunker with a team-coloured collar, and an armoured gun head with a
long, thick barrel that turns on it (build_head, 32 facings). The barrel is the silhouette, so it reaches well
past the bunker. One tile."""
import math

import rts_studio as st

FOOTPRINT = (1, 1)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
HEAD_HEIGHT = 3.5  # the top of the collar; the head turns about the footprint's centre
DETAIL = {"bolt", "vent", "periscope", "hatch", "smoke_tube", "rung"}
OCT = math.pi / 8  # turn an 8-sided cylinder so its flats face the compass points


def build(root):
    conc, dark, steel, paint = st.concrete(), st.dark_steel(), st.steel(), st.team_paint()
    armour = st.armour()
    cx, cy = W / 2, -H / 2
    st.block((W - 2 * INSET, H - 2 * INSET, 0.4), (cx, cy, 0.2), mat=conc, parent=root, bevel=0.1, name="pad")
    for dx in (-1, 1):
        for dy in (-1, 1):
            st.cylinder(0.3, 0.12, (cx + dx * (W / 2 - 1.3), cy + dy * (H / 2 - 1.3), 0.44), mat=steel,
                        parent=root, verts=8, name="bolt")

    # The bunker: a sloped eight-sided frustum with a team collar on top and a slit vent on each side.
    st.cone(4.4, 3.6, 2.6, (cx, cy, 1.7), rot=(0, 0, OCT), mat=conc, parent=root, verts=8, name="bunker")
    st.cylinder(3.8, 0.5, (cx, cy, 3.25), rot=(0, 0, OCT), mat=paint, parent=root, verts=8, bevel=0.05,
                name="collar")
    st.cylinder(3.0, 0.1, (cx, cy, 3.52), mat=dark, parent=root, verts=24, bevel=0.0, name="ring_seat")
    for k in range(4):
        a = k * math.pi / 2
        r = 3.95
        st.block((2.0, 0.3, 0.35), (cx + r * math.sin(a), cy + r * math.cos(a), 2.1), rot=(0.3, 0, -a),
                 mat=dark, parent=root, name="vent")
    # An armoured ammunition door on the south (camera) side, with a ladder of rungs up the north side.
    st.block((1.8, 0.5, 1.6), (cx, cy - 4.1, 1.2), mat=armour, parent=root, name="door")
    for k in range(3):
        st.block((1.0, 0.3, 0.25), (cx, cy + 4.0 - k * 0.27, 1.0 + k * 0.8), mat=steel, parent=root, name="rung")


def build_head(ring):
    armour, steel, dark, paint = st.armour(), st.steel(), st.dark_steel(), st.team_paint()
    st.cylinder(2.6, 0.4, (0, 0, 0.2), mat=dark, parent=ring, verts=24, name="turret_ring")
    # Low wedge-fronted head with a rear bustle; team paint on the roof plate only.
    st.wedge((3.8, 4.2, 1.7), (0, 0.2, 1.25), slope_front=0.45, mat=armour, parent=ring, name="head")
    st.block((3.4, 1.4, 1.2), (0, -2.4, 1.1), mat=armour, parent=ring, name="bustle")
    st.block((2.8, 2.0, 0.14), (0, -0.6, 2.16), mat=paint, parent=ring, bevel=0.02, name="roof_plate")
    st.block((3.0, 1.0, 0.12), (0, -2.4, 1.76), mat=paint, parent=ring, bevel=0.02, name="bustle_plate")
    # Mantlet, then a long thick barrel with a muzzle brake.
    st.block((1.7, 0.9, 1.1), (0, 2.5, 1.05), mat=steel, parent=ring, name="mantlet")
    st.cylinder(0.34, 4.6, (0, 5.2, 1.05), rot=(math.pi / 2, 0, 0), mat=steel, parent=ring, verts=16,
                name="barrel")
    st.cylinder(0.42, 0.9, (0, 3.6, 1.05), rot=(math.pi / 2, 0, 0), mat=dark, parent=ring, verts=16,
                name="barrel_collar")
    st.block((0.95, 0.8, 0.62), (0, 7.6, 1.05), mat=dark, parent=ring, name="muzzle_brake")
    # Roof detail: a commander's hatch, two periscopes and a smoke launcher on each cheek.
    st.cylinder(0.6, 0.25, (-0.8, -0.9, 2.3), mat=steel, parent=ring, verts=16, name="hatch")
    for x in (0.6, 1.2):
        st.block((0.35, 0.3, 0.35), (x, 0.2, 2.05), mat=dark, parent=ring, name="periscope")
    for side in (-1, 1):
        for j in range(2):
            st.cylinder(0.14, 0.6, (side * 2.05, 0.4 + j * 0.4, 1.3), rot=(math.pi / 2 - 0.4, 0, 0), mat=dark,
                        parent=ring, verts=8, name="smoke_tube")
