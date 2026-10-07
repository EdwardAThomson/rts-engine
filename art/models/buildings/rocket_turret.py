"""Rocket turret: a taller eight-sided concrete tower with corner buttresses and a team-coloured collar, and a
head of two rocket pods on a yoke, tilted up, that turns on it (build_head, 32 facings). The two boxy pods with
their rows of tube mouths are the silhouette. One tile."""
import math

import rts_studio as st

FOOTPRINT = (1, 1)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
HEAD_HEIGHT = 4.9  # the top of the collar; the head turns about the footprint's centre
DETAIL = {"bolt", "vent", "sensor", "cable", "rung", "tube_mouth"}
OCT = math.pi / 8  # turn an 8-sided cylinder so its flats face the compass points
TILT = 0.28  # the pods' elevation, in radians


def build(root):
    conc, dark, steel, paint = st.concrete(), st.dark_steel(), st.steel(), st.team_paint()
    armour = st.armour()
    cx, cy = W / 2, -H / 2
    st.block((W - 2 * INSET, H - 2 * INSET, 0.4), (cx, cy, 0.2), mat=conc, parent=root, bevel=0.1, name="pad")
    for dx in (-1, 1):
        for dy in (-1, 1):
            st.cylinder(0.3, 0.12, (cx + dx * (W / 2 - 1.3), cy + dy * (H / 2 - 1.3), 0.44), mat=steel,
                        parent=root, verts=8, name="bolt")

    # The tower: straight eight-sided walls, a buttress on each diagonal, a team collar on top.
    st.cylinder(3.4, 4.0, (cx, cy, 2.4), rot=(0, 0, OCT), mat=conc, parent=root, verts=8, bevel=0.1, name="tower")
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        st.wedge((1.4, 2.0, 3.2), (cx + 3.3 * math.sin(a), cy + 3.3 * math.cos(a), 2.0), slope_front=0.8,
                 rot=(0, 0, -a), mat=conc, parent=root, name="buttress")
    st.cylinder(3.6, 0.5, (cx, cy, 4.65), rot=(0, 0, OCT), mat=paint, parent=root, verts=8, bevel=0.05,
                name="collar")
    st.cylinder(2.8, 0.1, (cx, cy, 4.92), mat=dark, parent=root, verts=24, bevel=0.0, name="ring_seat")
    for k in range(4):
        a = k * math.pi / 2
        st.block((1.6, 0.3, 0.4), (cx + 3.42 * math.sin(a), cy + 3.42 * math.cos(a), 3.4), rot=(0, 0, -a),
                 mat=dark, parent=root, name="vent")
    # Armoured door on the south side; ladder rungs up the north side.
    st.block((1.8, 0.5, 2.0), (cx, cy - 3.4, 1.4), mat=armour, parent=root, name="door")
    for k in range(4):
        st.block((1.0, 0.3, 0.25), (cx, cy + 3.5, 1.0 + k * 0.9), mat=steel, parent=root, name="rung")


def _pod(ring, x, armour, dark, paint, soot):
    pod = st.group("pod", (x, 0.3, 1.5), parent=ring)
    pod.rotation_euler = (TILT, 0, 0)
    st.block((1.9, 3.8, 2.0), (0, 0, 0), mat=armour, parent=pod, name="pod_box")
    st.block((1.95, 1.2, 0.16), (0, -0.9, 1.04), mat=paint, parent=pod, bevel=0.02, name="pod_stripe")
    st.block((2.02, 3.0, 0.3), (0, -0.2, -0.55), mat=dark, parent=pod, name="pod_rail")
    # A face of 2 x 3 tube mouths, each a dark ring with a sooty bore.
    for i in (-0.48, 0.48):
        for j in (-0.6, 0.0, 0.6):
            st.cylinder(0.27, 0.3, (i, 1.95, j), rot=(math.pi / 2, 0, 0), mat=dark, parent=pod, verts=12,
                        name="tube")
            st.cylinder(0.19, 0.1, (i, 2.12, j), rot=(math.pi / 2, 0, 0), mat=soot, parent=pod, verts=12,
                        bevel=0.0, name="tube_mouth")


def build_head(ring):
    armour, steel, dark, paint = st.armour(), st.steel(), st.dark_steel(), st.team_paint()
    soot = st.plain("soot", (0.02, 0.02, 0.02), 0.95)
    st.cylinder(2.5, 0.4, (0, 0, 0.2), mat=dark, parent=ring, verts=24, name="turret_ring")
    # A yoke: a central armoured block carrying the trunnion, with a pod on each side.
    st.block((1.6, 2.6, 1.8), (0, -0.2, 1.3), mat=armour, parent=ring, name="yoke")
    st.cylinder(0.45, 4.6, (0, 0.3, 1.5), rot=(0, math.pi / 2, 0), mat=steel, parent=ring, verts=16,
                name="trunnion")
    for x in (-1.85, 1.85):
        _pod(ring, x, armour, dark, paint, soot)
    # A sensor box on the yoke, and a cable loop down its back.
    st.block((0.9, 0.8, 0.6), (0, -0.6, 2.5), mat=steel, parent=ring, name="sensor")
    st.block((0.6, 0.12, 0.35), (0, -0.19, 2.5), mat=st.glass(), parent=ring, name="sensor_lens")
    st.block((0.4, 0.4, 1.0), (0, -1.6, 0.9), mat=dark, parent=ring, name="cable")
