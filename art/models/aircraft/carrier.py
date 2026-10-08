"""Carrier: a heavy lifter that picks up ground vehicles and flies them across the map. A long boxy airframe on
four ducted lift fans at the corners, a cab at the front, an engine house on the spine, and four clamp arms under
the belly that swing out to open and in to grip a vehicle. Team colour on the duct lips and a spine band. About
10 m long, built facing north (+y) with the shut clamps' feet on the ground; the renderer lifts the sprite to its
flying height and draws the shadow frames where they fall.

Animations, all at 16 facings with shadows:
- `idle` (3 frames): flying empty, clamps open, fans turning (three blades, 40 degrees a frame, so it loops).
- `clamp` (4 frames): the clamps closing, open to shut, fans held still. Played backwards to let go.
- `carry` (3 frames): flying loaded, clamps shut, fans turning.

Later hooks: `wreck(rng, root)` (studio batch J)."""
import math

import bpy

import rts_studio as st

ANIMS = {"idle": 3, "clamp": 4, "carry": 3}
BLADES = 3
FANS = [(sx * 2.75, sy * 3.45) for sy in (1, -1) for sx in (-1, 1)]
FAN_Z = 3.1
DUCT_R = 1.2
BODY_Z = 3.0              # centre height of the airframe
BELLY_Z = 2.25            # underside, where the clamp hinges are
CLAMP_Y = (1.55, -1.55)
CLAMP_LEN = 2.0
OPEN = math.radians(65)   # how far the clamp arms swing out when open
DETAIL = {"antenna", "lamp", "vent", "hub_cap", "step", "rail", "hook_pin"}


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
    glass.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.35
    # Airframe: a long box with chamfered edges, a sloped cab at the front and a stepped tail.
    st.block((2.3, 6.4, 1.5), (0, -0.2, BODY_Z), mat=body, bevel=0.15, parent=root, name="airframe")
    st.wedge((2.1, 1.5, 1.35), (0, 3.6, BODY_Z - 0.05), slope_front=0.5, mat=body, parent=root, name="cab")
    st.block((1.8, 0.12, 0.55), (0, 4.12, BODY_Z + 0.2), rot=(math.radians(-48), 0, 0), mat=glass, bevel=0.02,
             parent=root, name="windscreen")
    for side in (-1, 1):
        st.block((0.1, 0.8, 0.4), (side * 1.06, 3.55, BODY_Z + 0.15), mat=glass, bevel=0.01, parent=root,
                 name="side_window")
    st.block((1.7, 0.9, 1.0), (0, -3.6, BODY_Z + 0.15), mat=body, bevel=0.12, parent=root, name="tail")
    # Engine house on the spine with intakes, exhaust grilles and a team band round it.
    st.block((1.5, 3.2, 0.75), (0, -0.4, BODY_Z + 1.05), mat=body, bevel=0.12, parent=root, name="engine_house")
    st.block((2.36, 0.7, 1.52), (0, 1.75, BODY_Z), mat=team, bevel=0.1, parent=root, name="spine_band")
    st.block((1.56, 0.7, 0.78), (0, 0.9, BODY_Z + 1.06), mat=team, bevel=0.1, parent=root, name="house_band")
    for side in (-1, 1):
        st.cylinder(0.3, 0.5, (side * 0.45, 1.35, BODY_Z + 1.1), rot=(math.pi / 2, 0, 0), mat=dark, verts=14,
                    parent=root, name="intake")
    for i in range(5):
        st.block((1.1, 0.12, 0.04), (0, -1.0 - 0.28 * i, BODY_Z + 1.44), mat=dark, bevel=0.01, parent=root,
                 name="vent")
    st.cylinder(0.04, 0.9, (0.5, -1.7, BODY_Z + 1.8), mat=steel, verts=6, parent=root, name="antenna")
    for y in (2.6, -3.9):
        st.sphere(0.12, (0, y, BODY_Z + 0.8), mat=st.plain("lamp", (0.9, 0.2, 0.08), emission=1.5),
                  parent=root, name="lamp")
    # Fan arms: deep beams from the airframe out to each duct.
    for x, y in FANS:
        st.beam((math.copysign(1.0, x), y * 0.62, BODY_Z), (x, y, FAN_Z), 0.5, mat=body, parent=root, name="arm")
        _fan(root, (x, y, FAN_Z), body, team, steel, dark)
    # Belly: the bay between the clamps, a rail along each side and the clamp hinges.
    st.block((1.6, 4.4, 0.25), (0, 0, BELLY_Z - 0.05), mat=dark, bevel=0.04, parent=root, name="bay")
    for side in (-1, 1):
        st.block((0.4, 4.8, 0.35), (side * 1.12, 0, BELLY_Z), mat=steel, bevel=0.05, parent=root, name="rail")
        for y in CLAMP_Y:
            _clamp(root, side, y, body, steel, dark)


def _fan(root, c, body, team, steel, dark):
    """One ducted lift fan: duct, team lip, stator cross, hub and three blades grouped under `rotor`."""
    st.cylinder(DUCT_R + 0.2, 0.85, c, mat=body, verts=48, caps=False, bevel=0, parent=root, name="duct")
    st.cylinder(DUCT_R + 0.02, 0.83, c, mat=dark, verts=48, caps=False, bevel=0, parent=root, name="duct_inner")
    _ring(DUCT_R + 0.11, 0.15, (c[0], c[1], c[2] + 0.43), team, root, "duct_lip", height=0.22)
    _ring(DUCT_R + 0.11, 0.13, (c[0], c[1], c[2] - 0.43), body, root, "duct_base", height=0.18)
    for k in range(4):
        st.block((DUCT_R * 2 - 0.1, 0.14, 0.14), (c[0], c[1], c[2] - 0.25), rot=(0, 0, math.pi / 4 + k * math.pi / 2),
                 mat=dark, bevel=0.0, parent=root, name="stator")
    st.cylinder(0.36, 0.55, (c[0], c[1], c[2] - 0.05), mat=steel, verts=16, parent=root, name="hub")
    rotor = st.group("rotor", (c[0], c[1], c[2] + 0.12), parent=root)
    for k in range(BLADES):
        a = 2 * math.pi * k / BLADES
        r = DUCT_R * 0.5 + 0.08
        st.block((DUCT_R - 0.2, 0.4, 0.07), (r * math.cos(a), r * math.sin(a), 0), rot=(0.25, 0, a), mat=steel,
                 bevel=0.01, parent=rotor, name="blade")
    st.cylinder(0.26, 0.14, (0, 0, 0.07), mat=dark, verts=14, parent=rotor, name="hub_cap")


def _clamp(root, side, y, body, steel, dark):
    """A clamp arm hanging from a hinge on the belly rail, with a claw turned in at the foot. Built shut (hanging
    straight down); `pose` swings it out about the hinge."""
    hinge = st.group("clamp", (side * 1.2, y, BELLY_Z - 0.1), parent=root)
    st.cylinder(0.22, 0.9, (0, 0, 0), rot=(math.pi / 2, 0, 0), mat=steel, verts=12, parent=hinge, name="hook_pin")
    st.block((0.45, 0.8, CLAMP_LEN), (0, 0, -CLAMP_LEN / 2), mat=body, bevel=0.06, parent=hinge, name="clamp_arm")
    st.block((0.5, 0.84, 0.3), (0, 0, -CLAMP_LEN * 0.55), mat=dark, bevel=0.03, parent=hinge, name="clamp_band")
    st.block((0.8, 0.86, 0.3), (-side * 0.2, 0, -CLAMP_LEN + 0.05), mat=steel, bevel=0.05, parent=hinge,
             name="claw")


def pose(root, anim, frame):
    """Fans turn 40 degrees a frame in `idle` and `carry` (opposite corners turn opposite ways); the clamps are
    open in `idle`, close over `clamp`'s four frames and are shut in `carry`."""
    n = ANIMS[anim]
    step = 2 * math.pi / BLADES / 3
    if anim == "clamp":
        spin, opening = 0.0, 1 - frame / (n - 1)
    else:
        spin, opening = step * frame, 1.0 if anim == "idle" else 0.0
    for o in root.children_recursive:
        base = o.name.split(".")[0]
        if base == "rotor":
            o.rotation_euler.z = spin * (1 if o.location.x * o.location.y > 0 else -1)
        elif base == "clamp":
            # The +x side swings out by turning about -y; the other side mirrors it.
            o.rotation_euler.y = -math.copysign(OPEN * opening, o.location.x)
