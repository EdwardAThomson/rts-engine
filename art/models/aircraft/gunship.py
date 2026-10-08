"""Gunship: a light attack aircraft lifted by two ducted fans on stub wings. A narrow armoured fuselage with a
stepped two-seat canopy, a twin-barrel chin gun, a rocket pod and a missile rail under each wing, a tail boom with
twin fins, and landing skids. Team colour on the duct lips, the fins and a band round the tail boom. About 7 m
long, built facing north (+y) with the skids on the ground; the renderer lifts the sprite to its flying height and
draws the shadow frames where they fall.

Rotors: three blades in each duct, turned 40 degrees a frame, so the three `idle` frames loop (the blades repeat
every 120 degrees). Each fan's blades are grouped under an empty named `rotor` at the hub.

Later hooks: `wreck(rng, root)` (studio batch J)."""
import math

import bpy

import rts_studio as st

ANIMS = {"idle": 3}
BLADES = 3
FAN_X, FAN_Y, FAN_Z = 2.75, -0.05, 1.55     # duct centres, either side
DUCT_R = 1.0                                 # inner radius of the duct
WING_Z = 1.45
DETAIL = {"antenna", "sensor", "step", "vent", "hub_cap", "pitot", "rail_fin", "lamp"}


def _ring(major, minor, loc, mat, parent, name, height=None):
    """A duct lip: a torus `major` metres round its centre line, `minor` thick, squashed to `height` tall."""
    bpy.ops.mesh.primitive_torus_add(major_radius=major, minor_radius=minor, major_segments=40, minor_segments=10)
    o = bpy.context.active_object
    if height:
        o.scale = (1, 1, height / (2 * minor))
        bpy.ops.object.transform_apply(scale=True)
    bpy.ops.object.shade_smooth()
    return st._place(o, loc, (0, 0, 0), mat, parent, name, 0)


def build_hull(root):
    body, team = st.armour(), st.team_paint()
    steel, dark, glass = st.steel(), st.dark_steel(), st.glass()
    # A little rougher than the studio glass, so the canopy doesn't flash white at one facing.
    glass.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.35
    # Fuselage: a long armoured box with a tapered nose, a raised spine behind the canopy and a tail boom.
    st.block((1.45, 3.6, 1.3), (0, 0.6, 1.25), mat=body, bevel=0.12, parent=root, name="fuselage")
    st.wedge((1.3, 1.2, 1.05), (0, 2.95, 1.1), slope_front=0.55, mat=body, parent=root, name="nose")
    st.block((1.0, 0.5, 0.55), (0, 3.4, 0.82), mat=body, bevel=0.1, parent=root, name="chin")
    st.block((1.1, 2.0, 0.55), (0, -0.55, 2.1), mat=body, bevel=0.12, parent=root, name="spine")
    st.cylinder(0.42, 2.2, (0, -2.05, 1.55), rot=(math.pi / 2, 0, 0), mat=body, verts=16, bevel=0.03,
                scale=(1.0, 1.25, 1.0), parent=root, name="tail_boom")
    st.cylinder(0.45, 0.35, (0, -1.7, 1.55), rot=(math.pi / 2, 0, 0), mat=team, verts=16, bevel=0.02,
                scale=(1.0, 1.25, 1.0), parent=root, name="tail_band")
    # Stepped canopy: gunner low in front, pilot higher behind.
    st.wedge((1.05, 1.0, 0.5), (0, 2.0, 2.05), slope_front=0.7, mat=glass, parent=root, name="canopy_front")
    st.wedge((1.05, 1.1, 0.6), (0, 1.05, 2.3), slope_front=0.6, mat=glass, parent=root, name="canopy_rear")
    st.block((1.12, 0.12, 0.62), (0, 1.55, 2.2), mat=body, bevel=0.03, parent=root, name="canopy_frame")
    # Engine intakes and exhausts on the spine shoulders.
    for side in (-1, 1):
        st.cylinder(0.3, 0.9, (side * 0.62, -0.3, 2.15), rot=(math.pi / 2, 0, 0), mat=dark, verts=14,
                    parent=root, name="intake")
        st.cylinder(0.26, 0.5, (side * 0.6, -1.35, 2.1), rot=(math.pi / 2, 0, 0), mat=steel, verts=14,
                    parent=root, name="exhaust")
    for i in range(4):
        st.block((0.7, 0.1, 0.04), (0, -0.9 - 0.22 * i, 2.39), mat=dark, bevel=0.01, parent=root, name="vent")
    # Tail: twin fins (team colour) on a stabiliser, with a small tail bumper.
    st.block((2.2, 0.75, 0.14), (0, -2.85, 1.75), mat=body, bevel=0.04, parent=root, name="stabiliser")
    for side in (-1, 1):
        st.wedge((0.16, 0.85, 1.05), (side * 1.05, -2.8, 2.3), slope_front=0.45, mat=team, parent=root,
                 name="fin")
    st.block((0.3, 0.3, 0.4), (0, -2.95, 1.0), mat=dark, bevel=0.03, parent=root, name="tail_skid")
    # Stub wings with the ducted fans at the tips.
    for side in (-1, 1):
        st.block((FAN_X - DUCT_R - 0.55, 1.25, 0.3), (side * (FAN_X - DUCT_R) / 2 + side * 0.45, FAN_Y, WING_Z),
                 mat=body, bevel=0.06, parent=root, name="wing")
        _fan(root, side, body, team, steel, dark)
        # Under each wing: a rocket pod (inboard) and a rail of two missiles (outboard).
        x = side * 1.2
        st.block((0.16, 0.5, 0.35), (x, FAN_Y, WING_Z - 0.3), mat=dark, bevel=0.02, parent=root, name="pylon")
        st.cylinder(0.3, 1.6, (x, FAN_Y + 0.1, WING_Z - 0.62), rot=(math.pi / 2, 0, 0), mat=body, verts=14,
                    bevel=0.03, parent=root, name="rocket_pod")
        st.cylinder(0.24, 0.05, (x, FAN_Y + 0.92, WING_Z - 0.62), rot=(math.pi / 2, 0, 0), mat=dark, verts=14,
                    parent=root, name="pod_face")
        x = side * 1.55
        st.block((0.5, 1.1, 0.1), (x, FAN_Y, WING_Z - 0.22), mat=steel, bevel=0.02, parent=root, name="rail")
        for dx in (-0.14, 0.14):
            st.cylinder(0.13, 1.3, (x + dx, FAN_Y + 0.15, WING_Z - 0.4), rot=(math.pi / 2, 0, 0), mat=steel,
                        verts=10, parent=root, name="missile")
            st.cone(0.13, 0.0, 0.3, (x + dx, FAN_Y + 0.95, WING_Z - 0.4), rot=(-math.pi / 2, 0, 0), mat=dark,
                    verts=10, name="missile_tip", parent=root)
    # Chin gun: a turret ball with two barrels.
    st.sphere(0.36, (0, 3.2, 0.5), mat=dark, parent=root, name="gun_ball")
    for dx in (-0.12, 0.12):
        st.cylinder(0.12, 0.75, (dx, 3.6, 0.48), rot=(math.pi / 2, 0, 0), mat=steel, verts=10, parent=root,
                    name="gun_barrel")
    # Skids on short struts, and small detail.
    for side in (-1, 1):
        st.block((0.25, 3.5, 0.2), (side * 0.85, 0.5, 0.12), mat=steel, bevel=0.06, parent=root, name="skid")
        for y in (1.6, -0.6):
            st.beam((side * 0.85, y, 0.2), (side * 0.6, y, 0.75), 0.25, mat=steel, parent=root, name="strut")
    st.cylinder(0.05, 0.9, (0, 3.1, 1.6), rot=(math.pi / 2, 0, 0), mat=steel, verts=6, parent=root, name="pitot")
    st.cylinder(0.04, 0.8, (0.3, -1.9, 2.3), mat=steel, verts=6, parent=root, name="antenna")
    st.sphere(0.2, (0, 3.35, 1.3), mat=glass, parent=root, name="sensor")
    for side in (-1, 1):
        st.block((0.15, 0.25, 0.08), (side * 0.75, 1.9, 0.6), mat=steel, bevel=0.01, parent=root, name="step")


def _fan(root, side, body, team, steel, dark):
    """One ducted fan: a deep duct with a team-coloured lip, four stator struts, a hub and three blades."""
    c = (side * FAN_X, FAN_Y, FAN_Z)
    st.cylinder(DUCT_R + 0.18, 0.75, c, mat=body, verts=40, caps=False, bevel=0, parent=root, name="duct")
    st.cylinder(DUCT_R + 0.02, 0.73, c, mat=dark, verts=40, caps=False, bevel=0, parent=root, name="duct_inner")
    _ring(DUCT_R + 0.1, 0.13, (c[0], c[1], c[2] + 0.38), team, root, "duct_lip", height=0.2)
    _ring(DUCT_R + 0.1, 0.12, (c[0], c[1], c[2] - 0.38), body, root, "duct_base", height=0.16)
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        st.block((DUCT_R * 2 - 0.1, 0.12, 0.12), (c[0], c[1], c[2] - 0.22), rot=(0, 0, a), mat=dark, bevel=0.0,
                 parent=root, name="stator")
    st.cylinder(0.3, 0.5, (c[0], c[1], c[2] - 0.05), mat=steel, verts=16, parent=root, name="hub")
    rotor = st.group("rotor", (c[0], c[1], c[2] + 0.1), parent=root)
    for k in range(BLADES):
        a = 2 * math.pi * k / BLADES
        r = DUCT_R * 0.5 + 0.05
        st.block((DUCT_R - 0.15, 0.32, 0.06), (r * math.cos(a), r * math.sin(a), 0), rot=(0.25, 0, a),
                 mat=steel, bevel=0.01, parent=rotor, name="blade")
    st.cylinder(0.22, 0.12, (0, 0, 0.06), mat=dark, verts=14, parent=rotor, name="hub_cap")


def pose(root, anim, frame):
    """`idle`: both fans' blades turned 40 degrees a frame (one third of a blade's spacing, so frame 3 = frame 0),
    in opposite directions so the torque cancels."""
    step = 2 * math.pi / BLADES / ANIMS["idle"]
    for o in root.children_recursive:
        if o.name.split(".")[0] == "rotor":
            o.rotation_euler.z = math.copysign(step * frame, o.location.x)
