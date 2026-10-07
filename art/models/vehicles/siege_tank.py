"""Siege tank: long-range tracked artillery in the house style. A low, long hull with a broad flat turret over the
middle, one long heavy barrel with a muzzle brake (the identifying feature, kept long and thick so it reads at
32 px), and two folded recoil spades across the rear. Team colour on the side skirts, the fenders and two bands on
the turret roof. About 7.5 m long from the folded spades to the glacis, facing north (+y). The turret ring sits on
the origin and the turret is built in the vehicle's frame (TURRET_HEIGHT 0), so turret frames share the hull's
pivot; the hull runs a little further forward than back so the ring stays at its centre of mass."""
import math

import rts_studio as st

L, W = 6.9, 3.5          # track run length and overall width
HULL_Y = 0.25            # hull centre, ahead of the turret ring
TRACK_W, TRACK_H = 0.75, 0.8
HULL_Z = 0.5             # belly height
DECK_Z = 1.3             # top of the hull deck: low, so the turret and barrel dominate
TURRET_Z = DECK_Z
TURRET_HEIGHT = 0
GUN_Z = TURRET_Z + 0.7  # barrel axis
GUN_PITCH = math.radians(4)  # a slight lift, so the long barrel doesn't read as lying on the hull
# Small parts the classic style leaves out.
DETAIL = {"wheel_hub", "link", "hatch", "vision", "grille", "exhaust", "stowage", "rail", "hinge", "spade_rib",
          "vent", "periscope", "recoil", "clamp", "lifting_eye", "spade_edge"}


def build_hull(root):
    paint, team = st.armour("hull_paint"), st.team_paint("hull_team")
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    tx = W / 2 - TRACK_W / 2
    for side in (-1, 1):
        x = side * tx
        st.block((TRACK_W, L, TRACK_H), (x, HULL_Y, TRACK_H / 2 + 0.02), mat=rubber, bevel=0.3, parent=root,
                 name="track")
        for i in range(30):
            y = HULL_Y - (L - 0.8) / 2 + (L - 0.8) * i / 29
            st.block((TRACK_W + 0.04, 0.1, 0.06), (x, y, TRACK_H + 0.03), mat=steel, bevel=0.01, parent=root,
                     name="link")
        # seven road wheels: one more than the battle tank, for the longer chassis
        for i in range(7):
            y = HULL_Y - 2.75 + 5.5 * i / 6
            st.cylinder(0.32, 0.12, (x + side * 0.35, y, 0.38), rot=(0, math.pi / 2, 0), mat=steel, verts=20,
                        bevel=0.02, parent=root, name="wheel")
            st.cylinder(0.12, 0.06, (x + side * 0.43, y, 0.38), rot=(0, math.pi / 2, 0), mat=dark, verts=10,
                        bevel=0, parent=root, name="wheel_hub")
        st.block((0.12, L - 1.4, 0.45), (side * (W / 2 + 0.02), HULL_Y + 0.1, 0.72), mat=team, bevel=0.03,
                 parent=root, name="skirt")
        st.block((0.66, L - 0.9, 0.08), (x + side * 0.05, HULL_Y + 0.1, TRACK_H + 0.1), mat=team, bevel=0.02,
                 parent=root, name="fender")
    # low hull body with a long shallow glacis
    body_w = W - 2 * TRACK_W + 0.5
    st.block((body_w, L - 1.7, DECK_Z - HULL_Z), (0, HULL_Y - 0.35, (HULL_Z + DECK_Z) / 2), mat=paint,
             bevel=0.06, parent=root, name="body")
    st.wedge((body_w, 1.6, DECK_Z - HULL_Z), (0, HULL_Y + L / 2 - 1.15, (HULL_Z + DECK_Z) / 2), slope_front=0.65,
             mat=paint, parent=root, name="glacis")
    st.block((body_w, 0.5, 0.3), (0, HULL_Y + L / 2 - 0.25, HULL_Z + 0.15), mat=paint, bevel=0.05, parent=root,
             name="nose")
    # driver's hatch and vision blocks at the front left
    st.cylinder(0.28, 0.1, (-0.6, HULL_Y + 2.15, DECK_Z + 0.02), mat=paint, verts=16, bevel=0.02, parent=root,
                name="hatch")
    st.block((0.5, 0.14, 0.14), (-0.6, HULL_Y + 2.5, DECK_Z - 0.04), mat=dark, bevel=0.01, parent=root,
             name="vision")
    # travel clamp: a cradle on the glacis that the barrel rests in on the march
    for side in (-1, 1):
        st.block((0.12, 0.3, 0.5), (side * 0.3, HULL_Y + 2.75, DECK_Z + 0.05), mat=steel, bevel=0.02,
                 parent=root, name="clamp")
    st.block((0.72, 0.3, 0.12), (0, HULL_Y + 2.75, DECK_Z + 0.3), mat=steel, bevel=0.02, parent=root,
             name="clamp")
    # engine deck behind the turret: grille slats, twin exhausts and a stowage box
    for i in range(6):
        st.block((1.4, 0.1, 0.05), (0, -2.15 - 0.17 * i, DECK_Z + 0.03), mat=dark, bevel=0.01, parent=root,
                 name="grille")
    for side in (-1, 1):
        st.cylinder(0.13, 0.5, (side * 0.78, -3.05, DECK_Z + 0.1), rot=(math.pi / 2, 0, 0), mat=steel, verts=12,
                    parent=root, name="exhaust")
    st.block((0.8, 0.5, 0.35), (-0.45, -1.85, DECK_Z + 0.17), mat=paint, bevel=0.04, parent=root, name="stowage")
    # recoil spades, folded up against the rear plate: two broad plates on hinge arms, the second silhouette cue
    rear = HULL_Y - L / 2
    for side in (-1, 1):
        x = side * 0.7
        st.block((0.3, 0.9, 0.3), (x, rear - 0.1, DECK_Z - 0.25), rot=(math.radians(-25), 0, 0), mat=steel,
                 bevel=0.03, parent=root, name="spade_arm")
        st.cylinder(0.14, 0.42, (x, rear + 0.2, DECK_Z - 0.1), rot=(0, math.pi / 2, 0), mat=dark, verts=12,
                    parent=root, name="hinge")
        st.block((1.15, 0.28, 0.95), (x, rear - 0.5, DECK_Z - 0.35), rot=(math.radians(-18), 0, 0), mat=paint,
                 bevel=0.04, parent=root, name="spade")
        st.block((0.12, 0.3, 0.8), (x, rear - 0.63, DECK_Z - 0.4), rot=(math.radians(-18), 0, 0), mat=steel,
                 bevel=0.01, parent=root, name="spade_rib")
        st.block((1.1, 0.3, 0.14), (x, rear - 0.62, DECK_Z - 0.85), rot=(math.radians(-18), 0, 0), mat=dark,
                 bevel=0.01, parent=root, name="spade_edge")


def build_turret(root):
    paint, team = st.armour("turret_paint"), st.team_paint("turret_team")
    steel, dark = st.steel("gun_steel", (0.16, 0.155, 0.15)), st.dark_steel()
    z = TURRET_Z + 0.15
    # a dark turret ring under an overhanging turret, so the turret reads apart from the hull deck
    st.cylinder(1.15, 0.2, (0, -0.1, z - 0.05), mat=dark, verts=20, bevel=0.02, parent=root, name="turret_ring")
    # a broad, flat, slab-sided turret: wider and longer than the battle tank's, but low
    st.wedge((2.6, 2.9, 0.9), (0, -0.15, z + 0.45), slope_front=0.35, mat=paint, parent=root,
             name="turret_body")
    st.block((2.2, 0.7, 0.75), (0, -1.85, z + 0.42), mat=paint, bevel=0.05, parent=root, name="bustle")
    # two team bands across the roof
    for y in (-0.85, 0.15):
        st.block((2.4, 0.42, 0.06), (0, y, z + 0.92), mat=team, bevel=0.02, parent=root, name="roof_team")
    # commander's hatch, a periscope block, roof vents and lifting eyes
    st.cylinder(0.3, 0.16, (0.65, -0.35, z + 1.0), mat=paint, verts=16, bevel=0.03, parent=root, name="hatch")
    st.block((0.3, 0.26, 0.2), (-0.65, 0.6, z + 0.96), mat=paint, bevel=0.03, parent=root, name="periscope")
    st.block((0.22, 0.05, 0.1), (-0.65, 0.74, z + 0.98), mat=dark, bevel=0, parent=root, name="periscope")
    for x in (-0.55, -0.25):
        st.block((0.22, 0.4, 0.06), (x, -0.4, z + 0.95), mat=dark, bevel=0.01, parent=root, name="vent")
    for x in (-1.15, 1.15):
        st.block((0.1, 0.2, 0.15), (x, -1.2, z + 0.95), mat=steel, bevel=0.01, parent=root, name="lifting_eye")
    # rail around the bustle roof
    st.block((2.0, 0.06, 0.06), (0, -2.15, z + 0.95), mat=steel, bevel=0, parent=root, name="rail")
    # gun: a deep mantlet, a thick cradle with recoil cylinders, then the long barrel
    gun = st.group("gun", (0, 1.05, GUN_Z), parent=root)
    gun.rotation_euler = (GUN_PITCH, 0, 0)
    st.block((1.2, 0.6, 0.85), (0, 0.1, 0), mat=paint, bevel=0.06, parent=gun, name="mantlet")
    st.cylinder(0.38, 1.2, (0, 0.9, 0), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=gun,
                name="gun_cradle")
    for x in (-0.28, 0.28):
        st.cylinder(0.09, 1.0, (x, 0.85, 0.36), rot=(math.pi / 2, 0, 0), mat=dark, verts=10, parent=gun,
                    name="recoil")
    st.cylinder(0.28, 4.6, (0, 3.75, 0), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, parent=gun, name="barrel")
    st.cylinder(0.38, 0.6, (0, 3.1, 0), rot=(math.pi / 2, 0, 0), mat=steel, verts=16, bevel=0.03, parent=gun,
                name="evacuator")
    st.block((1.0, 0.65, 0.44), (0, 6.1, 0), mat=dark, bevel=0.05, parent=gun, name="muzzle_brake")
    for x in (-0.36, 0.36):
        st.block((0.14, 0.46, 0.5), (x, 6.12, 0), mat=steel, bevel=0.01, parent=gun, name="muzzle_brake")


def wreck(rng, root):
    """Stub for the studio's wreck frames (batch J): the hull with the turret knocked askew and the barrel
    dropped. The studio adds the burnt look when it renders wrecks."""
    build_hull(root)
    ring = st.group("turret", (0, -0.2, TURRET_HEIGHT - 0.1), parent=root)
    ring.rotation_euler = (0, rng.uniform(-0.08, 0.08), rng.uniform(-0.6, 0.6))
    build_turret(ring)
    for o in ring.children:
        if o.name.startswith("gun"):
            o.rotation_euler = (-math.radians(3), 0, 0)
