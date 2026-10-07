"""Radar: a two-storey control block with a window band, and a braced steel tower carrying a big tilted dish. The
dish is the silhouette and the idle overlay: it turns on the tower top. Two tiles by two, built in proportion to
FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
DISH_R = 4.6
DETAIL = {"vent", "brace", "aerial", "stripe", "window", "feed_strut"}
# The parts that move on the idle overlay: everything on the turntable.
IDLE_PARTS = {"turntable", "dish", "dish_back", "dish_rim", "dish_face", "feed_arm", "feed_strut", "feed_horn",
              "dish_mount"}
DOORS = set()


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Control block across the south-west, door and window band facing south.
    cx, cy, cw, cl = W * 0.36, -H * 0.68, W * 0.58, H * 0.46
    st.block((cw, cl, 6.0), (cx, cy, 3.3), mat=conc, parent=root, name="control")
    st.block((cw + 0.1, cl + 0.1, 0.6), (cx, cy, 6.3), mat=paint, parent=root, name="roof_band")
    st.block((cw - 1.4, cl - 1.4, 0.3), (cx, cy, 6.7), mat=dark, parent=root, name="roof")
    st.block((cw - 1.6, 0.12, 1.1), (cx, cy - cl / 2 - 0.05, 4.4), mat=glass, parent=root, name="window")
    st.block((2.4, 0.4, 2.6), (cx - cw / 4, cy - cl / 2 - 0.1, 1.6), mat=armour, parent=root, name="door")
    for k in range(3):
        st.block((1.6, 1.6, 0.9), (cx - cw / 2 + 2.4 + k * (cw - 4.8) / 2, cy + cl / 5, 7.2), mat=steel,
                 parent=root, name="vent")
    st.block((0.3, 0.3, 4.0), (cx + cw / 2 - 1.2, cy + cl / 2 - 1.2, 8.8), mat=dark, parent=root, name="aerial")
    for k in range(6):
        st.block((0.9, 0.2, 0.05), (cx - cw / 4 + 1.6 + k * 0.8, -H * 0.94, 0.33), rot=(0, 0, 0.6), mat=stripe,
                 parent=root, bevel=0.0, name="stripe")

    # Tower in the north-east: four legs, cross braces and a platform.
    tx, ty, tz = W * 0.66, -H * 0.32, 8.0
    leg = 2.0
    for sx in (-1, 1):
        for sy in (-1, 1):
            st.block((0.5, 0.5, tz), (tx + sx * leg, ty + sy * leg, tz / 2 + 0.3), mat=steel, parent=root,
                     name="leg")
            st.block((1.0, 1.0, 0.6), (tx + sx * leg, ty + sy * leg, 0.6), mat=conc, parent=root, name="footing")
    for k in range(3):
        z = 2.0 + k * 2.3
        for side in (-1, 1):
            st.block((leg * 2, 0.3, 0.3), (tx, ty + side * leg, z), mat=dark, parent=root, name="brace")
            st.block((0.3, leg * 2, 0.3), (tx + side * leg, ty, z), mat=dark, parent=root, name="brace")
    st.block((leg * 2 + 1.4, leg * 2 + 1.4, 0.5), (tx, ty, tz + 0.5), mat=steel, parent=root, name="platform")
    st.block((leg * 2 + 1.5, leg * 2 + 1.5, 0.4), (tx, ty, tz + 0.95), mat=paint, parent=root,
             name="platform_band")

    # The dish on a turntable: a shallow bowl tilted up and towards the camera, a feed horn at its focus.
    top = st.group("dish_turntable", (tx, ty, tz + 1.2), parent=root)
    st.cylinder(1.4, 0.7, (0, 0, 0.35), mat=dark, parent=top, verts=24, name="turntable")
    st.block((1.0, 1.2, 1.8), (0, 0, 1.4), mat=steel, parent=top, name="dish_mount")
    tilt = st.group("dish_tilt", (0, 0, 2.5), parent=top)
    tilt.rotation_euler[0] = math.radians(40)
    st.cone(0.9, DISH_R, 1.5, (0, 0, 0.75), mat=steel, parent=tilt, verts=40, name="dish")
    st.cylinder(DISH_R - 0.35, 0.12, (0, 0, 1.46), mat=st.steel("dish_face", (0.42, 0.41, 0.39)), parent=tilt,
                verts=40, bevel=0.0, name="dish_face")
    st.cylinder(DISH_R + 0.05, 0.45, (0, 0, 1.35), mat=paint, parent=tilt, verts=40, caps=False, name="dish_rim")
    st.cylinder(1.0, 0.6, (0, 0, -0.1), mat=dark, parent=tilt, verts=20, name="dish_back")
    st.block((0.35, 0.35, 3.0), (0, 0, 3.0), mat=dark, parent=tilt, name="feed_arm")
    for a in (0, 2 * math.pi / 3, 4 * math.pi / 3):
        st.block((0.25, 0.25, 3.4), (1.6 * math.cos(a), 1.6 * math.sin(a), 2.9),
                 rot=(0.45 * math.sin(a), -0.45 * math.cos(a), 0), mat=dark, parent=tilt, name="feed_strut")
    st.block((0.9, 0.9, 0.9), (0, 0, 4.6), mat=armour, parent=tilt, name="feed_horn")


def damage(rng, root):
    """Damaged frame, for when the studio renders the `damage` hook: the dish slumps on its mount, some walls are
    scorched and small parts are knocked off. `rng` is a random.Random the studio seeds, so every render
    matches."""
    tilt = next(o for o in root.children_recursive if o.name.split(".")[0] == "dish_tilt")
    tilt.rotation_euler[0] = math.radians(rng.uniform(65, 80))
    tilt.rotation_euler[1] = math.radians(rng.uniform(-12, 12))
    scorch(rng, root)


def scorch(rng, root, share=0.25):
    """Burn a share of the larger parts and drop half the detail parts. Local helper; the studio may adopt it."""
    burnt = st.plain("scorch", (0.03, 0.025, 0.02), 0.95)
    for o in st.meshes(root):
        if o.name.split(".")[0] in DETAIL:
            o.hide_render = rng.random() < 0.5
        elif o.data.materials and rng.random() < share:
            o.data.materials[0] = burnt
