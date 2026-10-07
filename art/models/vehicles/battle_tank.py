"""Battle tank: the medium tank from the playbooks' tank experiment (experiments/d2-tank/tank.py, our own design),
ported to this studio's helpers. Twin tracks, sloped hull, octagonal turret, team-coloured side skirts, fenders and
turret roof. Front is +y (north), 1 unit = 1 metre. The turret is built in the vehicle's frame (TURRET_HEIGHT 0),
so turret frames share the hull's pivot."""
import math

import rts_studio as st

L, W = 6.2, 3.4          # overall length and width
TRACK_W, TRACK_H = 0.7, 0.85
HULL_Z = 0.55            # hull belly height
DECK_Z = 1.55            # top of hull deck
TURRET_Z = DECK_Z
TURRET_HEIGHT = 0
# The glacis: a 0.3 m plate tilted down at the front, centred GLACIS_Y forward. Its rear top edge meets the deck,
# so no lip stands proud of it with a hollow underneath (seen from low angles in a 3D export).
GLACIS_Y, GLACIS_TILT = 2.3, math.radians(24)
GLACIS_Z = DECK_Z - 0.75 * math.sin(GLACIS_TILT) - 0.15 * math.cos(GLACIS_TILT)
# Small parts the classic style leaves out, for the simpler, bolder shapes of early 90s sprites.
DETAIL = {"link", "spare_link", "cable", "antenna", "rack_bar", "rack_rail", "sight", "sight_glass", "vision",
          "driver_hatch", "hatch", "deck_plate", "sprocket", "exhaust", "stowage"}


def glacis_top(y):
    """Height of the glacis's upper face at `y`, for parts that sit on it."""
    return GLACIS_Z + (GLACIS_Y - y) * math.tan(GLACIS_TILT) + 0.15 / math.cos(GLACIS_TILT)


def build_hull(root):
    paint = st.armour("hull_paint")
    team = st.team_paint("hull_team")
    rubber = st.plain("track", (0.07, 0.065, 0.06), 0.9)
    steel = st.metal("steel", (0.2, 0.19, 0.18), (0.3, 0.17, 0.09))
    dark = st.plain("grille", (0.05, 0.045, 0.04), 0.8)
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        # track run, links across the top, road wheels and sprockets on the outside
        st.block((TRACK_W, L - 0.3, TRACK_H), (x, 0, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.3, parent=root,
                 name="track")
        n = 28
        for i in range(n):
            y = -(L - 0.9) / 2 + (L - 0.9) * i / (n - 1)
            st.block((TRACK_W + 0.04, 0.09, 0.06), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        for i in range(6):
            y = -2.2 + 4.4 * i / 5
            st.cylinder(0.33, 0.12, (x + side * 0.33, y, 0.4), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
        for y in (-(L - 0.6) / 2, (L - 0.6) / 2):
            st.cylinder(0.36, 0.14, (x + side * 0.33, y, 0.55), rot=(0, math.pi / 2, 0), mat=steel, verts=12,
                        parent=root, name="sprocket")
        # team-coloured side skirt over the upper run, with a bevelled lip
        st.block((0.12, L - 1.2, 0.5), (side * (W / 2 + 0.02), 0.1, 0.78), mat=team, bevel=0.03, parent=root,
                 name="skirt")
        st.block((0.62, L - 1.0, 0.08), (x + side * 0.05, 0.1, TRACK_H + 0.12), mat=team, bevel=0.02,
                 parent=root, name="fender")
    # hull body between the tracks, with a sloped glacis at the front and a lower rear deck
    body_w = W - 2 * TRACK_W + 0.5
    st.block((body_w, L - 1.6, DECK_Z - HULL_Z), (0, -0.3, (HULL_Z + DECK_Z) / 2), mat=paint, bevel=0.06,
             parent=root, name="body")
    # 4 cm narrower than the body: sides flush with the body's fight over the same plane and render black.
    st.block((body_w - 0.04, 1.5, 0.3), (0, GLACIS_Y, GLACIS_Z), rot=(-GLACIS_TILT, 0, 0), mat=paint,
             bevel=0.05, parent=root, name="glacis")
    st.block((body_w - 0.1, 1.4, 0.7), (0, 2.2, 0.95), mat=paint, bevel=0.06, parent=root, name="nose")
    # driver's hatch and vision block on the glacis
    st.cylinder(0.26, 0.12, (-0.55, 1.75, DECK_Z + 0.02), mat=paint, verts=16, bevel=0.02, parent=root,
                name="driver_hatch")
    st.block((0.4, 0.12, 0.12), (-0.55, 2.05, glacis_top(2.05) + 0.03), mat=dark, bevel=0.01, parent=root,
             name="vision")
    # engine deck: grille slats, two exhausts and a stowage box
    for i in range(7):
        st.block((1.5, 0.1, 0.05), (0, -1.9 - 0.17 * i, DECK_Z + 0.03), mat=dark, bevel=0.01, parent=root,
                 name="grille")
    st.block((1.7, 1.3, 0.04), (0, -2.42, DECK_Z + 0.005), mat=steel, bevel=0.01, parent=root, name="deck_plate")
    for side in (-1, 1):
        st.cylinder(0.11, 0.5, (side * 0.75, -2.75, DECK_Z + 0.08), rot=(math.pi / 2, 0, 0), mat=steel,
                    verts=12, parent=root, name="exhaust")
    st.block((0.9, 0.45, 0.35), (0.6, -1.25, DECK_Z + 0.17), mat=paint, bevel=0.04, parent=root, name="stowage")
    # spare track links on the front plate and a tow cable coil, for small-scale detail
    for i in range(4):
        y = 2.45 - 0.13 * i
        st.block((0.35, 0.09, 0.05), (0.5, y, glacis_top(y) + 0.025), rot=(-GLACIS_TILT, 0, 0), mat=steel,
                 bevel=0.01, parent=root, name="spare_link")
    st.cylinder(0.25, 0.08, (-0.7, -1.3, DECK_Z + 0.04), mat=steel, verts=16, caps=False, parent=root,
                name="cable")



def build_turret(root):
    paint = st.armour("turret_paint")
    team = st.team_paint("turret_team")
    steel = st.metal("gun_steel", (0.1, 0.095, 0.09), (0.2, 0.11, 0.06))
    steel.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.6
    dark = st.plain("optics", (0.04, 0.05, 0.06), 0.25)
    z = TURRET_Z
    # octagonal turret, longer than wide, slightly behind the turret ring centre
    st.cylinder(1.2, 0.72, (0, -0.15, z + 0.36), mat=paint, verts=8, bevel=0.07, scale=(0.85, 1.0, 1.0),
                parent=root, name="turret_body")
    st.cylinder(1.0, 0.08, (0, -0.2, z + 0.75), mat=team, verts=8, bevel=0.02, scale=(0.78, 0.9, 1.0),
                parent=root, name="roof_team")
    st.block((1.0, 0.45, 0.55), (0, 0.95, z + 0.36), mat=paint, bevel=0.05, parent=root, name="mantlet")
    st.cylinder(0.15, 0.7, (0, 1.45, z + 0.38), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=root,
                name="gun_sleeve")
    st.cylinder(0.1, 2.6, (0, 2.95, z + 0.38), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=root,
                name="barrel")
    st.cylinder(0.15, 0.32, (0, 4.2, z + 0.38), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, bevel=0.02,
                parent=root, name="muzzle_brake")
    # commander's cupola, gunner's sight, a bustle rack and an antenna
    st.cylinder(0.32, 0.22, (0.38, -0.35, z + 0.86), mat=paint, verts=16, bevel=0.03, parent=root,
                name="cupola")
    st.cylinder(0.24, 0.06, (0.38, -0.35, z + 1.0), mat=steel, verts=16, parent=root, name="hatch")
    st.block((0.25, 0.3, 0.22), (-0.42, 0.35, z + 0.86), mat=paint, bevel=0.03, parent=root, name="sight")
    st.block((0.2, 0.05, 0.12), (-0.42, 0.51, z + 0.88), mat=dark, bevel=0.0, parent=root, name="sight_glass")
    for x in (-0.6, 0, 0.6):
        st.block((0.05, 0.6, 0.25), (x, -1.45, z + 0.5), mat=steel, bevel=0.0, parent=root, name="rack_bar")
    st.block((1.3, 0.05, 0.05), (0, -1.73, z + 0.62), mat=steel, bevel=0.0, parent=root, name="rack_rail")
    st.cylinder(0.025, 1.6, (-0.55, -0.95, z + 1.5), mat=steel, verts=6, parent=root, name="antenna")
