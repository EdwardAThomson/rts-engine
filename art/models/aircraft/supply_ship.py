"""Supply ship: a large blocky cargo lander that drops off reinforcements. A box hull with a raised cab at the
front, two main engines on the rear deck, four downward lift thrusters, side cargo doors and four landing legs
that fold in under the belly in flight. Team colour on a roof band, the engine bands and the cab roof. 20 m long
in the world, modelled at half size (10 m) like the reference table says, facing north (+y) with the leg pads on
the ground.

Animations, at 16 facings with shadows:
- `idle` (1 frame): landed, legs down.
- `legs` (3 frames): the legs from folded (frame 0, flying) to down (frame 2). Played backwards on take-off.

Later hooks: `wreck(rng, root)` (studio batch J)."""
import math

import rts_studio as st

ANIMS = {"legs": 3}
HULL_W, HULL_L, HULL_H = 4.4, 7.6, 2.5
BELLY_Z = 1.55            # underside of the hull when landed
TOP_Z = BELLY_Z + HULL_H
LEGS = [(sx, sy) for sy in (1, -1) for sx in (-1, 1)]
LEG_X, LEG_Y = 2.0, 2.75  # leg hinges, on the hull's lower side edges
FOLD = math.radians(100)  # how far a leg swings in under the belly to stow
DETAIL = {"antenna", "lamp", "vent", "step", "hatch", "rung", "handle", "panel_line"}


def build_hull(root):
    body, team = st.armour(), st.team_paint()
    steel, dark, glass = st.steel(), st.dark_steel(), st.glass()
    glass.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.35
    zc = BELLY_Z + HULL_H / 2
    # Hull: one big chamfered box, a slightly narrower upper deck and a raised cab at the front.
    st.block((HULL_W, HULL_L, HULL_H), (0, -0.3, zc), mat=body, bevel=0.2, parent=root, name="hull")
    st.block((HULL_W - 0.6, HULL_L - 1.6, 0.45), (0, -0.8, TOP_Z + 0.2), mat=body, bevel=0.12, parent=root,
             name="deck")
    st.wedge((HULL_W - 0.4, 1.7, 2.6), (0, 4.05, BELLY_Z + 1.4), slope_front=0.45, mat=body, parent=root,
             name="cab")
    st.block((HULL_W - 1.0, 0.12, 0.75), (0, 4.62, BELLY_Z + 2.15), rot=(math.radians(-42), 0, 0), mat=glass,
             bevel=0.02, parent=root, name="windscreen")
    st.block((HULL_W - 1.0, 1.1, 0.1), (0, 3.7, BELLY_Z + 2.73), mat=team, bevel=0.03, parent=root,
             name="cab_roof")
    # Team band across the roof, wrapping down the sides.
    st.block((HULL_W + 0.06, 0.9, HULL_H + 0.06), (0, 1.75, zc), mat=team, bevel=0.15, parent=root,
             name="roof_band")
    st.block((HULL_W - 0.54, 0.9, 0.5), (0, 1.75, TOP_Z + 0.2), mat=team, bevel=0.1, parent=root,
             name="deck_band")
    # Rear deck: two main engines with team bands and nozzles out the back.
    for side in (-1, 1):
        x = side * 1.15
        st.cylinder(0.78, 3.6, (x, -2.7, TOP_Z + 0.75), rot=(math.pi / 2, 0, 0), mat=body, verts=20, bevel=0.04,
                    parent=root, name="engine")
        st.cylinder(0.82, 0.5, (x, -1.6, TOP_Z + 0.75), rot=(math.pi / 2, 0, 0), mat=team, verts=20, bevel=0.03,
                    parent=root, name="engine_band")
        st.cone(0.72, 0.5, 0.7, (x, -4.75, TOP_Z + 0.75), rot=(math.pi / 2, 0, 0), mat=dark, verts=20,
                parent=root, name="nozzle")
        st.cylinder(0.55, 0.4, (x, -0.75, TOP_Z + 0.75), rot=(math.pi / 2, 0, 0), mat=dark, verts=18,
                    parent=root, name="intake")
    st.block((0.5, 3.0, 0.5), (0, -2.6, TOP_Z + 0.65), mat=steel, bevel=0.05, parent=root, name="pylon")
    # Roof fittings: a hatch, radiator vents, an antenna and lamps.
    st.cylinder(0.45, 0.14, (-0.9, 0.4, TOP_Z + 0.48), mat=steel, verts=16, parent=root, name="hatch")
    for i in range(6):
        st.block((1.1, 0.12, 0.05), (0.8, 0.9 - 0.25 * i, TOP_Z + 0.45), mat=dark, bevel=0.01, parent=root,
                 name="vent")
    st.cylinder(0.05, 1.1, (-1.4, 3.3, BELLY_Z + 3.2), mat=steel, verts=6, parent=root, name="antenna")
    for x in (-HULL_W / 2 - 0.02, HULL_W / 2 + 0.02):
        st.sphere(0.13, (x, -3.9, TOP_Z - 0.2), mat=st.plain("lamp", (0.9, 0.2, 0.08), emission=1.5),
                  parent=root, name="lamp")
    # Sides: a big cargo door each side, with a frame, handles and a step ladder.
    for side in (-1, 1):
        x = side * (HULL_W / 2 + 0.03)
        st.block((0.1, 2.8, 1.7), (x, -0.9, BELLY_Z + 1.05), mat=steel, bevel=0.04, parent=root,
                 name="cargo_door")
        for y in (-1.6, -0.2):
            st.block((0.1, 0.12, 1.4), (x + side * 0.05, y, BELLY_Z + 1.05), mat=dark, bevel=0.01, parent=root,
                     name="handle")
        for k in range(3):
            st.block((0.12, 0.6, 0.08), (x + side * 0.04, 2.2, BELLY_Z + 0.25 + 0.4 * k), mat=dark, bevel=0.01,
                     parent=root, name="rung")
    # Belly: four lift thrusters.
    for sx in (-1, 1):
        for y in (2.3, -2.6):
            st.cylinder(0.55, 0.4, (sx * 1.1, y, BELLY_Z - 0.15), mat=dark, verts=16, parent=root,
                        name="thruster")
    for sx, sy in LEGS:
        _leg(root, sx, sy, body, steel, dark)


def _leg(root, sx, sy, body, steel, dark):
    """A landing leg hinged on the hull's lower side edge, built down (landed); `pose` folds it under the belly."""
    hinge = st.group("leg", (sx * LEG_X, sy * LEG_Y, BELLY_Z + 0.3), parent=root)
    foot = (sx * 0.95, sy * 0.45, -BELLY_Z - 0.3 + 0.18)
    st.cylinder(0.3, 0.7, (0, 0, 0), rot=(math.pi / 2, 0, 0), mat=steel, verts=12, parent=hinge, name="leg_hinge")
    st.beam((0, 0, 0), foot, 0.42, mat=body, parent=hinge, name="leg_strut")
    st.beam((-sx * 0.25, 0, -0.9), (foot[0] * 0.75, foot[1] * 0.75, foot[2] * 0.6), 0.26, mat=steel,
            parent=hinge, name="leg_brace")
    st.cylinder(0.55, 0.22, (foot[0], foot[1], foot[2] - 0.07), mat=dark, verts=16, bevel=0.03, parent=hinge,
                name="leg_pad")


def pose(root, anim, frame):
    """Legs down in `idle`; in `legs` they come down from folded (frame 0) to landed (the last frame)."""
    t = 1.0 if anim != "legs" else frame / (ANIMS["legs"] - 1)
    for o in root.children_recursive:
        if o.name.split(".")[0] == "leg":
            # A +x leg swings in under the belly by turning about +y; the -x side mirrors it.
            o.rotation_euler.y = math.copysign(FOLD * (1 - t), o.location.x)
