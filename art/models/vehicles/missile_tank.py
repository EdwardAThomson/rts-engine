"""Missile tank: a tracked launcher in the house style. A battle-tank-sized hull carries a low turntable with a big
box launcher on side trunnions, tilted up to fire. The box holds eight tubes in two rows of four, their dark
mouths facing forward; the box is the identifying feature, so it is large and square-shouldered to read at 32 px.
Team colour on the side skirts, the fenders and a band round the launcher box. About 6.4 m long, facing north
(+y). The turntable sits on the origin and the turret is built in the vehicle's frame (TURRET_HEIGHT 0), so
turret frames share the hull's pivot. The launcher's tilt is fixed at LAUNCH_PITCH for the sprite; it turns with
the turret and only the turn is animated."""
import math

import rts_studio as st

L, W = 6.1, 3.3          # track run length and overall width
TRACK_W, TRACK_H = 0.7, 0.8
HULL_Z = 0.5
DECK_Z = 1.45
TURRET_Z = DECK_Z
TURRET_HEIGHT = 0
LAUNCH_PITCH = math.radians(22)
TUBES = (2, 4)           # rows, columns
# Small parts the classic style leaves out.
DETAIL = {"wheel_hub", "link", "hatch", "vision", "grille", "exhaust", "stowage", "rail", "cable", "sensor_glass",
          "tube_rim", "jack", "bolt", "antenna"}


def build_hull(root):
    paint, team = st.armour("hull_paint"), st.team_paint("hull_team")
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        st.block((TRACK_W, L, TRACK_H), (x, 0, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.3, parent=root,
                 name="track")
        for i in range(26):
            y = -(L - 0.8) / 2 + (L - 0.8) * i / 25
            st.block((TRACK_W + 0.04, 0.1, 0.06), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        for i in range(6):
            y = -2.2 + 4.4 * i / 5
            st.cylinder(0.32, 0.12, (x + side * 0.33, y, 0.38), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
            st.cylinder(0.12, 0.06, (x + side * 0.41, y, 0.38), rot=(0, math.pi / 2, 0), mat=dark, verts=10,
                        bevel=0, parent=root, name="wheel_hub")
        st.block((0.12, L - 1.3, 0.48), (side * (W / 2 + 0.02), 0.05, 0.74), mat=team, bevel=0.03, parent=root,
                 name="skirt")
        st.block((0.62, L - 0.9, 0.08), (x + side * 0.05, 0.05, TRACK_H + 0.1), mat=team, bevel=0.02,
                 parent=root, name="fender")
    body_w = W - 2 * TRACK_W + 0.5
    st.block((body_w, L - 1.5, DECK_Z - HULL_Z), (0, -0.25, (HULL_Z + DECK_Z) / 2), mat=paint, bevel=0.06,
             parent=root, name="body")
    # 4 cm narrower than the body, so their sides don't share a plane (coincident faces render dark).
    st.wedge((body_w - 0.04, 1.3, DECK_Z - HULL_Z), (0, L / 2 - 0.9, (HULL_Z + DECK_Z) / 2), slope_front=0.55,
             mat=paint, parent=root, name="glacis")
    # a raised crew cab at the front right, so the hull reads differently from the battle tank's
    st.wedge((1.1, 1.2, 0.5), (0.45, 1.9, DECK_Z + 0.2), slope_front=0.5, mat=paint, parent=root, name="cab")
    st.block((0.9, 0.08, 0.2), (0.45, 2.48, DECK_Z + 0.22), mat=st.glass(), bevel=0.01, parent=root,
             name="vision")
    st.cylinder(0.24, 0.1, (-0.5, 1.95, DECK_Z + 0.03), mat=paint, verts=16, bevel=0.02, parent=root,
                name="hatch")
    # engine deck: grille slats, exhausts, a stowage box and stabiliser jacks folded at the rear corners
    for i in range(6):
        st.block((1.3, 0.1, 0.05), (0, -1.95 - 0.17 * i, DECK_Z + 0.03), mat=dark, bevel=0.01, parent=root,
                 name="grille")
    for side in (-1, 1):
        st.cylinder(0.11, 0.45, (side * 0.72, -2.8, DECK_Z + 0.08), rot=(math.pi / 2, 0, 0), mat=steel,
                    verts=12, parent=root, name="exhaust")
        st.block((0.3, 0.3, 0.7), (side * 0.95, -L / 2 + 0.55, DECK_Z - 0.2), mat=steel, bevel=0.03,
                 parent=root, name="jack")
    st.block((0.8, 0.45, 0.32), (-0.4, -1.45, DECK_Z + 0.16), mat=paint, bevel=0.04, parent=root, name="stowage")


def build_turret(root):
    paint, team = st.armour("turret_paint"), st.team_paint("turret_team")
    steel, dark = st.steel(), st.dark_steel()
    z = TURRET_Z
    # low round turntable with a skirt of armour
    st.cylinder(1.15, 0.35, (0, 0, z + 0.17), mat=paint, verts=16, bevel=0.04, parent=root, name="turntable")
    st.cylinder(1.2, 0.08, (0, 0, z + 0.38), mat=steel, verts=16, bevel=0.02, parent=root, name="ring")
    # two trunnion cheeks holding the launcher
    for side in (-1, 1):
        st.block((0.3, 1.1, 0.8), (side * 1.12, -0.1, z + 0.75), mat=paint, bevel=0.04, parent=root,
                 name="cheek")
        st.cylinder(0.2, 0.18, (side * 1.3, -0.15, z + 0.95), rot=(0, math.pi / 2, 0), mat=dark, verts=12,
                    parent=root, name="trunnion")
    # the launcher: a box pivoted near its rear third, tilted up
    rows, cols = TUBES
    bw, bl, bh = 1.95, 2.9, 1.05
    box = st.group("launcher", (0, -0.15, z + 0.95), parent=root)
    box.rotation_euler = (LAUNCH_PITCH, 0, 0)
    cy = 0.45                       # box centre ahead of the pivot
    st.block((bw, bl, bh), (0, cy, 0), mat=paint, bevel=0.06, parent=box, name="launcher_box")
    # a team band round the box, and team on the lid between the stiffening ribs
    st.block((bw + 0.06, 0.5, bh + 0.06), (0, cy - 0.55, 0), mat=team, bevel=0.03, parent=box, name="box_team")
    st.block((bw - 0.3, 0.9, 0.05), (0, cy + 0.65, bh / 2 + 0.02), mat=team, bevel=0.01, parent=box,
             name="lid_team")
    for y in (cy + 0.05, cy + 1.25):
        st.block((bw + 0.04, 0.12, 0.1), (0, y, bh / 2 + 0.03), mat=steel, bevel=0.01, parent=box, name="rib")
    # a front frame and eight tube mouths: dark discs with steel rims, big enough to read as tubes
    front = cy + bl / 2
    st.block((bw + 0.06, 0.14, bh + 0.06), (0, front + 0.02, 0), mat=steel, bevel=0.03, parent=box,
             name="frame")
    pitch_x, pitch_z = (bw - 0.3) / cols, (bh - 0.25) / rows
    for r in range(rows):
        for c in range(cols):
            x = -(bw - 0.3) / 2 + pitch_x * (c + 0.5)
            zz = -(bh - 0.25) / 2 + pitch_z * (r + 0.5)
            st.cylinder(0.19, 0.06, (x, front + 0.1, zz), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, bevel=0,
                        parent=box, name="tube_rim")
            st.cylinder(0.15, 0.08, (x, front + 0.11, zz), rot=(math.pi / 2, 0, 0), mat=dark, verts=16, bevel=0,
                        parent=box, name="tube")
    # rear cover with bolts, a cable run to the turntable and a sensor head on the left cheek
    st.block((bw - 0.1, 0.08, bh - 0.1), (0, cy - bl / 2 - 0.04, 0), mat=steel, bevel=0.01, parent=box,
             name="rear_cover")
    for x in (-0.7, 0, 0.7):
        st.cylinder(0.05, 0.05, (x, cy - bl / 2 - 0.09, 0.3), rot=(math.pi / 2, 0, 0), mat=dark, verts=8,
                    bevel=0, parent=box, name="bolt")
    st.cylinder(0.06, 0.9, (0.6, -0.85, z + 0.45), rot=(math.radians(70), 0, 0), mat=dark, verts=8, bevel=0,
                parent=root, name="cable")
    st.block((0.42, 0.42, 0.36), (-1.12, 0.65, z + 1.35), mat=paint, bevel=0.04, parent=root, name="sensor")
    st.block((0.3, 0.05, 0.2), (-1.12, 0.87, z + 1.37), mat=st.glass(), bevel=0, parent=root,
             name="sensor_glass")
    st.cylinder(0.025, 1.3, (0.9, -0.85, z + 1.0), mat=steel, verts=6, bevel=0, parent=root, name="antenna")


def wreck(rng, root):
    """Stub for the studio's wreck frames (batch J): the hull with the launcher dropped flat and turned askew.
    The studio adds the burnt look when it renders wrecks."""
    build_hull(root)
    ring = st.group("turret", (0, 0, TURRET_HEIGHT - 0.05), parent=root)
    ring.rotation_euler = (0, 0, rng.uniform(-0.8, 0.8))
    build_turret(ring)
    for o in ring.children:
        if o.name.startswith("launcher"):
            o.rotation_euler = (math.radians(-2), 0, 0)
