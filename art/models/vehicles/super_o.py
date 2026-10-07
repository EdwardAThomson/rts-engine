"""Super unit O: a medium tracked launcher for gas canisters. The turret carries an open cradle of four fat
canisters on a raised launch rail; behind it, two pressure tanks lie across the rear deck, piped forward to the
turret ring. About 6.8 m long, lighter armour than the tanks. Our own design in the battle tank's house style:
sandy armour, team-coloured skirts, fenders and turret roof; the canisters carry an orange hazard band (kept out
of the team hue). Front is +y (north), 1 unit = 1 metre. The turret is built in the vehicle's frame
(TURRET_HEIGHT 0), so turret frames share the hull's pivot."""
import math

import rts_studio as st

L, W = 6.8, 3.4
TRACK_W, TRACK_H = 0.7, 0.85
HULL_Z = 0.55
DECK_Z = 1.5
TURRET_Z = DECK_Z
TURRET_HEIGHT = 0
# Small parts the classic style leaves out.
DETAIL = {"link", "wheel", "sprocket", "valve", "gauge", "strap", "vision", "hatch", "clamp", "nozzle", "exhaust"}


def hazard():
    """Hazard orange for the canister bands: hue about 0.07, well below the team band."""
    return st.plain("hazard", (0.55, 0.2, 0.02), 0.6)


def build_hull(root):
    paint, team = st.armour("hull_paint"), st.team_paint("hull_team")
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    tank_mat = st.armour("tank_paint", (0.16, 0.12, 0.07))
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        st.block((TRACK_W, L - 0.3, TRACK_H), (x, 0, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.3, parent=root,
                 name="track")
        n = 30
        for i in range(n):
            y = -(L - 0.9) / 2 + (L - 0.9) * i / (n - 1)
            st.block((TRACK_W + 0.04, 0.09, 0.06), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        for i in range(6):
            y = -2.5 + 5.0 * i / 5
            st.cylinder(0.33, 0.12, (x + side * 0.33, y, 0.4), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
        for y in (-(L - 0.6) / 2, (L - 0.6) / 2):
            st.cylinder(0.36, 0.14, (x + side * 0.33, y, 0.55), rot=(0, math.pi / 2, 0), mat=steel, verts=12,
                        parent=root, name="sprocket")
        st.block((0.12, L - 1.3, 0.5), (side * (W / 2 + 0.02), 0.1, 0.78), mat=team, bevel=0.03, parent=root,
                 name="skirt")
        st.block((0.62, L - 1.0, 0.08), (x + side * 0.05, 0.1, TRACK_H + 0.12), mat=team, bevel=0.02,
                 parent=root, name="fender")
    body_w = W - 2 * TRACK_W + 0.5
    st.block((body_w, L - 1.6, DECK_Z - HULL_Z), (0, -0.3, (HULL_Z + DECK_Z) / 2), mat=paint, bevel=0.06,
             parent=root, name="body")
    st.block((body_w, 1.4, 0.3), (0, 2.55, DECK_Z - 0.35), rot=(math.radians(-24), 0, 0), mat=paint, bevel=0.05,
             parent=root, name="glacis")
    st.block((body_w - 0.1, 1.3, 0.7), (0, 2.45, 0.95), mat=paint, bevel=0.06, parent=root, name="nose")
    st.block((0.4, 0.12, 0.12), (0.55, 2.3, DECK_Z - 0.02), mat=dark, bevel=0.01, parent=root, name="vision")
    st.cylinder(0.25, 0.1, (0.55, 2.0, DECK_Z + 0.02), mat=paint, verts=16, bevel=0.02, parent=root, name="hatch")
    # Two pressure tanks lying across the rear deck in a saddle, painted darker than the hull, strapped down, with valves.
    for k, y in enumerate((-1.9, -2.75)):
        st.block((2.2, 0.7, 0.2), (0, y, DECK_Z + 0.08), mat=dark, bevel=0.02, parent=root, name="saddle")
        st.cylinder(0.36, 2.3, (0, y, DECK_Z + 0.45), rot=(0, math.pi / 2, 0), mat=tank_mat, verts=24, bevel=0.06,
                    parent=root, name="pressure_tank")
        for side in (-1, 1):
            st.sphere(0.36, (side * 1.15, y, DECK_Z + 0.45), mat=tank_mat, parent=root, scale=(0.35, 1, 1),
                      name="tank_end")
        for x in (-0.75, 0.75):
            st.cylinder(0.39, 0.08, (x, y, DECK_Z + 0.45), rot=(0, math.pi / 2, 0), mat=dark, verts=24,
                        parent=root, name="strap")
        st.cylinder(0.1, 0.25, (0.45, y, DECK_Z + 0.85), mat=steel, verts=10, parent=root, name="valve")
    # a manifold pipe from each tank forward to the turret ring, and a pressure gauge
    for x in (-0.5, 0.5):
        st.cylinder(0.11, 1.9, (x, -1.45, DECK_Z + 0.2), rot=(math.pi / 2, 0, 0), mat=steel, verts=12,
                    parent=root, name="pipe")
    st.block((1.3, 0.3, 0.3), (0, -0.55, DECK_Z + 0.15), mat=steel, bevel=0.04, parent=root, name="manifold")
    st.cylinder(0.14, 0.08, (-0.8, -1.2, DECK_Z + 0.25), rot=(math.radians(60), 0, 0), mat=dark, verts=16,
                parent=root, name="gauge")
    for side in (-1, 1):
        st.cylinder(0.1, 0.4, (side * 0.95, -3.25, DECK_Z + 0.1), rot=(math.pi / 2, 0, 0), mat=steel, verts=12,
                    parent=root, name="exhaust")


def build_turret(root):
    paint, team = st.armour("turret_paint"), st.team_paint("turret_team")
    steel, dark = st.steel(), st.dark_steel()
    can_mat, band = st.steel("canister", (0.36, 0.35, 0.32)), hazard()
    z = TURRET_Z
    # low round turret base with a team roof disc
    st.cylinder(1.15, 0.55, (0, 0, z + 0.28), mat=paint, verts=10, bevel=0.06, parent=root, name="turret_body")
    st.cylinder(0.95, 0.08, (0, -0.1, z + 0.58), mat=team, verts=10, bevel=0.02, parent=root, name="roof_team")
    st.block((0.7, 0.6, 0.45), (-0.55, -0.55, z + 0.8), mat=paint, bevel=0.04, parent=root, name="operator_box")
    st.block((0.5, 0.06, 0.16), (-0.55, -0.24, z + 0.86), mat=dark, bevel=0.0, parent=root, name="vision")
    # launch cradle: two side arms holding an open rack, tilted up 20 degrees, four fat canisters in a 2 x 2 stack
    rack = st.group("launcher", (0.3, 0.35, z + 1.0), parent=root)
    rack.rotation_euler = (math.radians(20), 0, 0)
    for side in (-1, 1):
        st.block((0.16, 1.0, 0.7), (side * 0.78, -0.35, -0.25), mat=paint, bevel=0.03, parent=rack, name="arm")
    st.block((1.5, 2.6, 0.12), (0, 0.4, -0.45), mat=dark, bevel=0.02, parent=rack, name="rail")
    for c in (-0.36, 0.36):
        for r in (0, 0.62):
            st.cylinder(0.3, 2.3, (c, 0.45, -0.1 + r), rot=(math.pi / 2, 0, 0), mat=can_mat, verts=20, bevel=0.04,
                        parent=rack, name="canister")
            st.sphere(0.3, (c, 1.6, -0.1 + r), mat=can_mat, parent=rack, scale=(1, 0.6, 1), name="canister_nose")
            st.cylinder(0.31, 0.35, (c, 0.9, -0.1 + r), rot=(math.pi / 2, 0, 0), mat=band, verts=20, parent=rack,
                        name="canister_band")
            st.cylinder(0.12, 0.2, (c, -0.75, -0.1 + r), rot=(math.pi / 2, 0, 0), mat=steel, verts=10,
                        parent=rack, name="nozzle")
    for y in (-0.3, 1.2):
        st.block((1.6, 0.14, 1.0), (0, y, 0.2), mat=steel, bevel=0.02, parent=rack, name="clamp")


def wreck(rng, root):
    """Stub for the studio's wreck pass (batch J): the hull with one pressure tank split and rolled off its saddle."""
    build_hull(root)
    tanks = [o for o in root.children if o.name.startswith("pressure_tank")]
    if tanks:
        t = tanks[rng.randrange(len(tanks))]
        t.location.x += rng.uniform(-0.6, 0.6)
        t.rotation_euler.z = rng.uniform(-0.4, 0.4)
