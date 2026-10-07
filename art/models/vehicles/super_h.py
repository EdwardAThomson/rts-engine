"""Super unit H: a very large, slow heavy tank. Wide tracks under thick layered side armour, a long hull with a
raised engine deck, and a broad six-sided turret carrying two heavy barrels side by side. About 9 m long, the
biggest hull in the set. Our own design in the battle tank's house style: sandy armour, team-coloured skirts,
fenders and turret roof band. Front is +y (north), 1 unit = 1 metre. The turret is built in the vehicle's frame
(TURRET_HEIGHT 0), so turret frames share the hull's pivot."""
import math

import rts_studio as st

L, W = 9.0, 4.6
TRACK_W, TRACK_H = 1.15, 1.05
HULL_Z = 0.6
DECK_Z = 1.9
TURRET_Z = DECK_Z
TURRET_HEIGHT = 0
# Small parts the classic style leaves out.
DETAIL = {"link", "wheel", "sprocket", "bolt", "grille", "exhaust", "vision", "hatch", "sight", "sight_glass",
          "smoke_tube", "tow_hook", "handle"}


def build_hull(root):
    paint, team = st.armour("hull_paint"), st.team_paint("hull_team")
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        st.block((TRACK_W, L - 0.4, TRACK_H), (x, 0, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.35, parent=root,
                 name="track")
        n = 36
        for i in range(n):
            y = -(L - 1.1) / 2 + (L - 1.1) * i / (n - 1)
            st.block((TRACK_W + 0.04, 0.12, 0.07), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        for i in range(8):
            y = -3.4 + 6.8 * i / 7
            st.cylinder(0.42, 0.16, (x + side * 0.55, y, 0.48), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
        for y in (-(L - 0.8) / 2, (L - 0.8) / 2):
            st.cylinder(0.46, 0.18, (x + side * 0.55, y, 0.65), rot=(0, math.pi / 2, 0), mat=steel, verts=12,
                        parent=root, name="sprocket")
        # two layers of side armour: a sandy plate over the track, the team skirt below its lip
        st.block((0.18, L - 1.6, 0.6), (side * (W / 2 + 0.05), 0, 0.82), mat=team, bevel=0.03, parent=root,
                 name="skirt")
        st.block((TRACK_W + 0.2, L - 1.2, 0.22), (x + side * 0.08, 0, TRACK_H + 0.18), mat=paint, bevel=0.04,
                 parent=root, name="sponson")
        st.block((0.3, L - 1.6, 0.06), (side * (W / 2 - 0.05), 0, TRACK_H + 0.31), mat=team, bevel=0.01,
                 parent=root, name="fender")
        for i in range(5):
            st.cylinder(0.06, 0.05, (side * (W / 2 + 0.15), -3.0 + 1.5 * i, 1.0), rot=(0, math.pi / 2, 0),
                        mat=steel, verts=8, parent=root, name="bolt")
    # hull: a long box, a steep two-plate glacis, and a raised engine deck at the rear
    body_w = W - 2 * TRACK_W + 0.7
    st.block((body_w, L - 2.0, DECK_Z - HULL_Z), (0, -0.4, (HULL_Z + DECK_Z) / 2), mat=paint, bevel=0.07,
             parent=root, name="body")
    st.block((body_w + 0.3, 1.9, 0.38), (0, 3.25, DECK_Z - 0.48), rot=(math.radians(-28), 0, 0), mat=paint,
             bevel=0.06, parent=root, name="glacis")
    st.block((body_w + 0.2, 1.6, 0.85), (0, 3.4, 1.05), mat=paint, bevel=0.07, parent=root, name="nose")
    st.block((0.55, 0.14, 0.14), (-0.95, 3.75, DECK_Z - 0.32), mat=dark, bevel=0.01, parent=root, name="vision")
    st.cylinder(0.3, 0.12, (-0.95, 3.2, DECK_Z + 0.02), mat=paint, verts=16, bevel=0.02, parent=root,
                name="hatch")
    for side in (-1, 1):
        st.block((0.35, 0.3, 0.2), (side * 1.3, 4.25, 0.75), mat=steel, bevel=0.03, parent=root, name="tow_hook")
    st.block((body_w + 0.3, 2.6, 0.45), (0, -3.0, DECK_Z + 0.2), mat=paint, bevel=0.06, parent=root,
             name="engine_deck")
    for i in range(8):
        st.block((2.0, 0.12, 0.06), (0, -2.0 - 0.26 * i, DECK_Z + 0.45), mat=dark, bevel=0.01, parent=root,
                 name="grille")
    for side in (-1, 1):
        st.cylinder(0.17, 0.6, (side * 1.15, -4.25, DECK_Z + 0.25), rot=(math.pi / 2, 0, 0), mat=steel, verts=12,
                    parent=root, name="exhaust")


def build_turret(root):
    paint, team = st.armour("turret_paint"), st.team_paint("turret_team")
    steel = st.metal("gun_steel", (0.1, 0.095, 0.09), (0.2, 0.11, 0.06))
    steel.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.6
    dark = st.plain("optics", (0.04, 0.05, 0.06), 0.25)
    z = TURRET_Z
    # broad six-sided turret, a flat-topped cap, a team band round the roof and a rear bustle
    st.cylinder(1.75, 0.95, (0, -0.2, z + 0.48), rot=(0, 0, math.pi / 6), mat=paint, verts=6, bevel=0.08,
                scale=(1.0, 1.0, 1.0), parent=root, name="turret_body")
    st.cylinder(1.45, 0.25, (0, -0.25, z + 1.05), rot=(0, 0, math.pi / 6), mat=paint, verts=6, bevel=0.05,
                parent=root, name="turret_cap")
    st.cylinder(1.6, 0.1, (0, -0.2, z + 0.93), rot=(0, 0, math.pi / 6), mat=team, verts=6, bevel=0.02,
                parent=root, name="roof_team")
    st.block((2.3, 1.2, 0.8), (0, -2.0, z + 0.55), mat=paint, bevel=0.06, parent=root, name="bustle")
    st.block((2.1, 0.1, 0.3), (0, -2.62, z + 0.6), mat=team, bevel=0.02, parent=root, name="bustle_team")
    # mantlet and two heavy barrels side by side, each with a fume extractor and a box muzzle brake
    st.block((2.0, 0.6, 0.75), (0, 1.55, z + 0.5), mat=paint, bevel=0.06, parent=root, name="mantlet")
    for x in (-0.5, 0.5):
        st.cylinder(0.2, 0.8, (x, 2.1, z + 0.52), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=root,
                    name="gun_sleeve")
        st.cylinder(0.14, 3.2, (x, 3.8, z + 0.52), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=root,
                    name="barrel")
        st.cylinder(0.22, 0.6, (x, 3.4, z + 0.52), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, bevel=0.02,
                    parent=root, name="extractor")
        st.block((0.4, 0.5, 0.36), (x, 5.4, z + 0.52), mat=steel, bevel=0.03, parent=root, name="muzzle_brake")
    # commander's hatch, a sight block and smoke tubes on the cheeks
    st.cylinder(0.36, 0.2, (0.6, -0.5, z + 1.25), mat=paint, verts=16, bevel=0.03, parent=root, name="hatch")
    st.block((0.3, 0.35, 0.28), (-0.7, 0.3, z + 1.3), mat=paint, bevel=0.03, parent=root, name="sight")
    st.block((0.24, 0.05, 0.14), (-0.7, 0.49, z + 1.32), mat=dark, bevel=0.0, parent=root, name="sight_glass")
    for side in (-1, 1):
        for k in range(3):
            st.cylinder(0.09, 0.35, (side * (1.45 + 0.02 * k), 0.4 - 0.22 * k, z + 0.85),
                        rot=(math.radians(-30), side * math.radians(30), 0), mat=steel, verts=10, parent=root,
                        name="smoke_tube")
        st.block((0.06, 0.8, 0.06), (side * 1.62, -1.0, z + 0.75), mat=steel, bevel=0.0, parent=root,
                 name="handle")


def wreck(rng, root):
    """Stub for the studio's wreck pass (batch J): the hull with the turret blown off its ring and dropped askew
    beside it, as the unit's self-destruct would leave it."""
    build_hull(root)
    ring = st.group("turret_wreck", (rng.uniform(-1.5, 1.5), rng.uniform(-2.0, 0.0), -0.6), parent=root)
    ring.rotation_euler = (rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.3), rng.uniform(0, 2 * math.pi))
    build_turret(ring)
