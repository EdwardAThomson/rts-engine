"""Wall: a run of cast concrete with a capping ledge and panel joints, one tile per piece. Each piece reaches to
the tile edge towards every joined neighbour (JOINS: 1 north, 2 east, 4 south, 8 west) so runs read as one wall;
a corner, tee, end or lone piece gets a thicker post at its centre, and a straight run has none. No team colour."""
import bpy

import rts_studio as st

FOOTPRINT = (1, 1)
JOINS = True
TEAM = False  # walls carry no team colour; check.py skips the coverage check
T = st.STUDIO["metres_per_tile"]
DETAIL = {"seam", "rebar"}
ARM_W, ARM_H = 3.4, 3.4  # the wall's thickness and height
POST_W, POST_H = 4.4, 4.0
ARMS = {1: (0, 1), 2: (1, 0), 4: (0, -1), 8: (-1, 0)}


def _mats():
    return (st.concrete("wall", (0.4, 0.38, 0.34)), st.concrete("wall_cap", (0.47, 0.45, 0.4)),
            st.plain("seam", (0.12, 0.11, 0.1), 0.9))


def build(root, joins):
    conc, cap, seam = _mats()
    foot = st.concrete("wall_footing", (0.3, 0.28, 0.25))
    cx, cy = T / 2, -T / 2
    straight = joins in (1 | 4, 2 | 8)
    for bit, (dx, dy) in ARMS.items():
        if not joins & bit:
            continue
        L = T / 2  # reach the tile edge so neighbouring pieces meet
        along = dx == 0  # the arm runs north-south
        size = lambda w, h: (w if along else L, L if along else w, h)  # noqa: E731
        mid = (cx + dx * L / 2, cy + dy * L / 2)
        st.block(size(ARM_W + 0.6, 0.4), (*mid, 0.2), mat=foot, parent=root, bevel=0.06, name="footing")
        st.block(size(ARM_W, ARM_H), (*mid, ARM_H / 2 + 0.2), mat=conc, parent=root, bevel=0.06, name="arm")
        st.block(size(ARM_W + 0.5, 0.4), (*mid, ARM_H + 0.35), mat=cap, parent=root, bevel=0.06, name="cap")
        # A panel joint halfway along each face of the arm, standing 2 cm proud so it never shares a plane.
        for side in (-1, 1):
            off = side * (ARM_W / 2 + 0.02)
            j = (cx + dx * L * 0.55 + (off if along else 0), cy + dy * L * 0.55 + (0 if along else off))
            st.block((0.12 if not along else 0.06, 0.06 if not along else 0.12, ARM_H - 0.3),
                     (*j, ARM_H / 2 + 0.25), mat=seam, parent=root, bevel=0.0, name="seam")
    if not straight:
        st.block((POST_W + 0.6, POST_W + 0.6, 0.4), (cx, cy, 0.2), mat=foot, parent=root, bevel=0.06,
                 name="footing")
        st.block((POST_W, POST_W, POST_H), (cx, cy, POST_H / 2 + 0.2), mat=conc, parent=root, bevel=0.1,
                 name="post")
        st.block((POST_W + 0.5, POST_W + 0.5, 0.45), (cx, cy, POST_H + 0.4), mat=cap, parent=root, bevel=0.08,
                 name="post_cap")


def damage(rng, root):
    """Shell-struck: one capping section knocked off, a V-shaped bite out of the top of one arm or the post with
    rebar showing in the break, and loose chunks at the foot beside the wall."""
    parts = sorted(st.meshes(root), key=lambda o: o.name)
    caps = [o for o in parts if o.name.startswith(("cap", "post_cap"))]
    if caps:
        bpy.data.objects.remove(rng.choice(caps), do_unlink=True)
    bodies = sorted((o for o in st.meshes(root) if o.name.startswith(("arm", "post")) and
                     not o.name.startswith("post_cap")), key=lambda o: o.name)
    body = rng.choice(bodies)
    bpy.context.view_layer.update()
    dx, dy, dz = body.dimensions
    along_x = dx >= dy  # the break runs across the wall, so its V shows on the long faces
    top = body.location.z + dz / 2
    t = rng.uniform(-0.25, 0.25) * (dx if along_x else dy)
    bx, by = body.location.x + (t if along_x else 0), body.location.y + (0 if along_x else t)
    # The bite: a cube turned 45 degrees about the wall's thickness, its lower corner well into the wall. The
    # broken faces take the cutter's raw, darker concrete.
    size, depth = rng.uniform(1.8, 2.4), rng.uniform(1.2, 1.6)
    bpy.ops.mesh.primitive_cube_add(size=1)
    cutter = bpy.context.active_object
    cutter.name = "cutter"
    cutter.scale = (size, 8.0, size) if along_x else (8.0, size, size)
    cutter.rotation_euler = (0, 0.785, 0) if along_x else (0.785, 0, 0)
    cutter.location = (bx, by, top - depth + size * 0.707)
    cutter.hide_render = True
    cutter.data.materials.append(st.concrete("broken", (0.24, 0.23, 0.21)))
    hit = [o for o in st.meshes(root) if o.name.startswith(("arm", "post", "cap", "seam"))]
    for o in hit:
        mod = o.modifiers.new("break", "BOOLEAN")
        mod.operation = "DIFFERENCE"
        mod.solver = "EXACT"
        mod.material_mode = "TRANSFER"
        mod.object = cutter
    # Rebar left standing in the bottom of the break, bent a little outwards.
    steel = st.metal("rebar")
    floor = top - depth
    for k in (-1, 0, 1):
        off = k * 0.9
        x, y = (bx, by + off) if along_x else (bx + off, by)
        st.block((0.25, 0.25, 1.1), (x, y, floor + 0.45), rot=(0.25 * k, 0, 0) if along_x else (0, 0.25 * k, 0),
                 mat=steel, parent=root, bevel=0.0, name="rebar")
    # Chunks fall beside the wall, not on it: the side depends on which way the pieces run.
    conc = st.concrete("wall", (0.4, 0.38, 0.34))
    broken = st.concrete("broken", (0.24, 0.23, 0.21))
    ew = any(o.name.startswith("arm") for o in bodies) and all(abs(o.location.y + T / 2) < 0.1 for o in bodies)
    cx, cy = T / 2, -T / 2
    for n in range(5):
        sz = rng.uniform(0.5, 1.0)
        side, along = rng.choice((-1, 1)) * rng.uniform(2.4, 3.2), rng.uniform(-1.5, 1.5) + t
        x, y = (cx + along, cy + side) if ew else (cx + side, cy + along)
        st.block((sz, sz * 0.8, sz * 0.6), (x, y, sz * 0.3), rot=(0, 0, rng.uniform(0, 3)),
                 mat=conc if n % 2 else broken, parent=root, name="chunk")
