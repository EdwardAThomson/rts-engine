"""The studio for the Classic engine's pre-rendered sprites (plans/rts/art-pipeline.md, section 1).

Builds on the playbooks' guide 03 studio (prerender.py), with the changes the art plan asks for: an orthographic
camera facing north at 60 degrees, one fixed scale per tile, separate shadow passes, team paint in one saturated
green that the packer turns into a mask, and helpers that keep transforms on objects instead of baking them into
meshes, so parts can be grouped under pivots (a turret under its ring).

Runs with Blender's Python module (pip install bpy).
"""
import json
import math
from pathlib import Path

import bpy
from mathutils import Vector

STUDIO = json.loads((Path(__file__).parent / "studio.json").read_text())


# ---------- scene ----------

def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = STUDIO["samples"]
    scene.cycles.use_denoising = True
    scene.cycles.max_bounces = 4
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    return scene


RENDER_SCALE = STUDIO["render_scale"]  # render.py sets this per category or from --scale


def render_px_per_metre():
    return STUDIO["atlas_px_per_tile"] * RENDER_SCALE / STUDIO["metres_per_tile"]


def camera(scene, left, right, up, down):
    """Orthographic camera due south of the origin looking north and down at the studio elevation. The canvas
    covers `left`/`right` metres either side of the origin and `up`/`down` metres of screen height above and below
    it (before the packer's vertical stretch). Returns the render size and the origin's pixel."""
    ppm = render_px_per_metre()
    w, h = math.ceil((left + right) * ppm), math.ceil((up + down) * ppm)
    data = bpy.data.cameras.new("cam")
    data.type = "ORTHO"
    data.ortho_scale = max(w, h) / ppm
    cam = bpy.data.objects.new("cam", data)
    scene.collection.objects.link(cam)
    el = math.radians(STUDIO["elevation_deg"])
    cam.location = Vector((0, -math.cos(el), math.sin(el))) * 60
    cam.rotation_euler = (-cam.location).to_track_quat("-Z", "Y").to_euler()
    origin_px = (left * ppm, up * ppm)
    data.shift_x = (w / 2 - origin_px[0]) / max(w, h)
    data.shift_y = (origin_px[1] - h / 2) / max(w, h)
    scene.render.resolution_x, scene.render.resolution_y = w, h
    scene.camera = cam
    return (w, h), origin_px


def lights(scene):
    """Warm key from the top left of the screen, so shadows fall right and down; cool sky fill; faint rim."""
    k = STUDIO["key_light"]
    _sun(scene, "key", k["direction"], k["energy"], k["colour"], 2)
    rim = _sun(scene, "rim", (-0.5, 1.0, -0.6), 0.6, (0.7, 0.8, 1.0), 10)
    rim.data.use_shadow = False  # one shadow per sprite: only the key light casts onto the ground
    world = bpy.data.worlds.new("world")
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.6, 0.68, 0.85, 1)
    bg.inputs["Strength"].default_value = 0.5
    scene.world = world


def shadow_extent(top):
    """How far, in metres on the ground, the shadow of something `top` metres tall reaches east and south."""
    d = Vector(STUDIO["key_light"]["direction"])
    return top * d.x / -d.z, top * -d.y / -d.z


def _sun(scene, name, direction, energy, colour, angle_deg):
    light = bpy.data.lights.new(name, "SUN")
    light.energy = energy
    light.color = colour
    light.angle = math.radians(angle_deg)
    obj = bpy.data.objects.new(name, light)
    obj.rotation_euler = Vector(direction).normalized().to_track_quat("-Z", "Y").to_euler()
    scene.collection.objects.link(obj)
    return obj


def shadow_ground(scene, size=60):
    bpy.ops.mesh.primitive_plane_add(size=size, location=(0, 0, 0))
    plane = bpy.context.active_object
    plane.is_shadow_catcher = True
    return plane


# ---------- materials ----------

def _mat(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    return m, nt, nt.nodes["Principled BSDF"]


def _link(nt, a, out, b, inp):
    nt.links.new(a.outputs[out], b.inputs[inp])


def _noisy(name, base, scale, detail, spread, roughness, metallic=0.0, bump=0.25):
    """A base colour broken up by noise: the look of worn paint, concrete or metal at sprite size."""
    m, nt, bsdf = _mat(name)
    m.diffuse_color = (*base, 1)
    noise = nt.nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = scale
    noise.inputs["Detail"].default_value = detail
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    els = ramp.color_ramp.elements
    els[0].position, els[1].position = 0.3, 0.7
    els[0].color = (*[c * (1 - spread) for c in base], 1)
    els[1].color = (*[min(1, c * (1 + spread)) for c in base], 1)
    _link(nt, noise, "Fac", ramp, "Fac")
    _link(nt, ramp, "Color", bsdf, "Base Color")
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if bump:
        b = nt.nodes.new("ShaderNodeBump")
        b.inputs["Strength"].default_value = bump
        b.inputs["Distance"].default_value = 0.02
        _link(nt, noise, "Fac", b, "Height")
        _link(nt, b, "Normal", bsdf, "Normal")
    return m


def team_paint(name="team"):
    """The one material the packer recolours per player. Keep every other material out of its hue band."""
    rgb = STUDIO["team_paint"]["rgb"]
    m = _noisy(name, rgb, 18, 6, 0.25, 0.55, bump=0.15)
    m["team_paint"] = True
    return m


def armour(name="armour", base=(0.2, 0.14, 0.075)):
    """Painted steel from the tank experiment (playbooks experiments/d2-tank): a sandy base with soft blotches and
    fine grime, slightly glossy. The house look for vehicles: team paint is an accent on top of this."""
    m, nt, bsdf = _mat(name)
    m.diffuse_color = (*base, 1)
    blotch = nt.nodes.new("ShaderNodeTexNoise")
    blotch.inputs["Scale"].default_value = 2.5
    blotch.inputs["Detail"].default_value = 3
    grime = nt.nodes.new("ShaderNodeTexNoise")
    grime.inputs["Scale"].default_value = 30
    grime.inputs["Detail"].default_value = 8
    mix = nt.nodes.new("ShaderNodeMath")
    mix.operation = "MULTIPLY_ADD"
    _link(nt, blotch, "Fac", mix, 0)
    mix.inputs[1].default_value = 0.6
    _link(nt, grime, "Fac", mix, 2)
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    els = ramp.color_ramp.elements
    els[0].position, els[1].position = 0.45, 0.95
    els[0].color = (*[c * 0.62 for c in base], 1)
    els[1].color = (*[min(1, c * 1.2) for c in base], 1)
    _link(nt, mix, "Value", ramp, "Fac")
    _link(nt, ramp, "Color", bsdf, "Base Color")
    bsdf.inputs["Metallic"].default_value = 0.3
    bsdf.inputs["Roughness"].default_value = 0.55
    b = nt.nodes.new("ShaderNodeBump")
    b.inputs["Strength"].default_value = 0.2
    b.inputs["Distance"].default_value = 0.02
    _link(nt, grime, "Fac", b, "Height")
    _link(nt, b, "Normal", bsdf, "Normal")
    return m


def metal(name="iron", base=(0.25, 0.24, 0.23), rust=(0.35, 0.16, 0.07)):
    """Bare metal with a little rust, as in guide 03's studio."""
    m, nt, bsdf = _mat(name)
    m.diffuse_color = (*base, 1)
    noise = nt.nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = 14
    noise.inputs["Detail"].default_value = 6
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    els = ramp.color_ramp.elements
    els[0].position, els[1].position = 0.45, 0.62
    els[0].color, els[1].color = (*base, 1), (*rust, 1)
    _link(nt, noise, "Fac", ramp, "Fac")
    _link(nt, ramp, "Color", bsdf, "Base Color")
    bsdf.inputs["Metallic"].default_value = 0.8
    bsdf.inputs["Roughness"].default_value = 0.45
    return m


def steel(name="steel", base=(0.32, 0.31, 0.3)):
    return _noisy(name, base, 22, 6, 0.25, 0.45, metallic=0.7)


def dark_steel(name="dark_steel"):
    return steel(name, (0.12, 0.12, 0.12))


def concrete(name="concrete", base=(0.5, 0.48, 0.44)):
    return _noisy(name, base, 6, 8, 0.2, 0.9, bump=0.4)


def rubber(name="rubber"):
    return _noisy(name, (0.05, 0.05, 0.05), 30, 3, 0.4, 0.9)


def plain(name, rgb, roughness=0.6, metallic=0.0, emission=0.0):
    m, nt, bsdf = _mat(name)
    m.diffuse_color = (*rgb, 1)
    bsdf.inputs["Base Color"].default_value = (*rgb, 1)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if emission:
        bsdf.inputs["Emission Color"].default_value = (*rgb, 1)
        bsdf.inputs["Emission Strength"].default_value = emission
    return m


def glass():
    return plain("glass", (0.15, 0.3, 0.38), roughness=0.15, metallic=0.3)


def cargo():
    """The resource in a hopper or tank: warm gold, kept clear of the team hue band."""
    return _noisy("cargo", (0.7, 0.45, 0.12), 40, 4, 0.35, 0.8, bump=0.6)


# ---------- geometry ----------
# Every helper builds its mesh at the origin and keeps location, rotation and scale on the object, so a part can
# be parented to a group and turned with it.

# Level of detail. "full" is what the sprites render; "low" is the light real-time version for the 3D engine's
# crowds (export_gltf --lod low): no bevels, 8-sided round parts and 8 x 4 spheres. Models need no changes.
LODS = {"full": {"bevel": True, "round": None, "sphere": (24, 12)},
        "low": {"bevel": False, "round": 8, "sphere": (8, 4)}}
LOD = LODS["full"]


def set_lod(name):
    """Pick the level of detail for every shape built from now on."""
    global LOD
    LOD = LODS[name]


def _verts(n):
    return min(n, LOD["round"]) if LOD["round"] else n

def group(name, loc=(0, 0, 0), parent=None):
    e = bpy.data.objects.new(name, None)
    bpy.context.scene.collection.objects.link(e)
    e.location = loc
    if parent:
        e.parent = parent
    return e


def _place(o, loc, rot, mat, parent, name, bevel):
    o.name = name
    o.location = loc
    o.rotation_euler = rot
    if bevel and LOD["bevel"]:
        mod = o.modifiers.new("bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 2
        mod.limit_method = "ANGLE"
    if mat:
        o.data.materials.append(mat)
    if parent:
        o.parent = parent
    return o


def block(size, loc, rot=(0, 0, 0), mat=None, parent=None, bevel=0.04, name="block"):
    """A bevelled box `size` metres (x, y, z), centred on `loc` in its parent's space."""
    bpy.ops.mesh.primitive_cube_add(size=1)
    o = bpy.context.active_object
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    return _place(o, loc, rot, mat, parent, name, bevel)


def cylinder(radius, depth, loc, rot=(0, 0, 0), mat=None, parent=None, verts=24, bevel=0.02, name="cyl",
             caps=True, scale=None):
    """Upright by default; rot=(pi/2, 0, 0) lays it along y. `scale` squashes it into an oval (applied to the
    mesh), and caps=False leaves a thin open tube."""
    bpy.ops.mesh.primitive_cylinder_add(vertices=_verts(verts), radius=radius, depth=depth,
                                        end_fill_type="NGON" if caps else "NOTHING")
    o = bpy.context.active_object
    if scale:
        o.scale = scale
        bpy.ops.object.transform_apply(scale=True)
    if not caps:
        o.modifiers.new("solid", "SOLIDIFY").thickness = 0.02
    bpy.ops.object.shade_smooth()
    return _place(o, loc, rot, mat, parent, name, bevel)


def cone(r1, r2, depth, loc, rot=(0, 0, 0), mat=None, parent=None, verts=24, name="cone"):
    bpy.ops.mesh.primitive_cone_add(vertices=_verts(verts), radius1=r1, radius2=r2, depth=depth)
    o = bpy.context.active_object
    bpy.ops.object.shade_smooth()
    return _place(o, loc, rot, mat, parent, name, 0)


def sphere(radius, loc, mat=None, parent=None, scale=(1, 1, 1), name="sphere"):
    segments, rings = LOD["sphere"]
    bpy.ops.mesh.primitive_uv_sphere_add(radius=radius, segments=segments, ring_count=rings)
    o = bpy.context.active_object
    o.scale = scale
    bpy.ops.object.transform_apply(scale=True)
    bpy.ops.object.shade_smooth()
    return _place(o, loc, (0, 0, 0), mat, parent, name, 0)


def wedge(size, loc, slope_front=0.4, rot=(0, 0, 0), mat=None, parent=None, name="wedge"):
    """A box whose top front edge (the +y side) is cut down by `slope_front` of its height: glacis plates, cabs."""
    w, l, h = size
    bpy.ops.mesh.primitive_cube_add(size=1)
    o = bpy.context.active_object
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    for v in o.data.vertices:
        if v.co.y > 0 and v.co.z > 0:
            v.co.z -= h * slope_front
    return _place(o, loc, rot, mat, parent, name, 0.03)


def apply_style(scene, root, style, detail_names=()):
    """Turn the scene into a style from studio.json: `flat` drops textures and bevels (one plain colour per
    material, the look of early 90s renders), `detail` False hides parts a model lists as detail-only,
    and `max_bounces` 0 keeps direct light only."""
    cfg = STUDIO["styles"][style]
    scene.cycles.max_bounces = cfg["max_bounces"]
    if cfg["flat"]:
        for m in bpy.data.materials:
            bsdf = m.node_tree.nodes.get("Principled BSDF")
            if not bsdf:
                continue
            for link in list(bsdf.inputs["Base Color"].links) + list(bsdf.inputs["Normal"].links):
                m.node_tree.links.remove(link)
            bsdf.inputs["Base Color"].default_value = m.diffuse_color
            bsdf.inputs["Metallic"].default_value = 0.0
            bsdf.inputs["Roughness"].default_value = 0.8
        for o in meshes(root):
            for mod in [mod for mod in o.modifiers if mod.type == "BEVEL"]:
                o.modifiers.remove(mod)
    if not cfg["detail"]:
        for o in meshes(root):
            if o.name.split(".")[0] in detail_names:
                o.hide_render = True
                o.hide_viewport = True


def meshes(root):
    return [o for o in root.children_recursive if o.type == "MESH"]


def bounds(root):
    """Horizontal radius about the origin and top height, over the evaluated meshes under `root`."""
    return bounds_of(meshes(root))


def bounds_of(objs):
    deps = bpy.context.evaluated_depsgraph_get()
    radius, top, lo = 0.0, 0.0, [math.inf, math.inf]
    hi = [-math.inf, -math.inf]
    for o in objs:
        ev = o.evaluated_get(deps)
        mesh = ev.to_mesh()
        for v in mesh.vertices:
            p = ev.matrix_world @ v.co
            radius = max(radius, math.hypot(p.x, p.y))
            top = max(top, p.z)
            lo = [min(lo[0], p.x), min(lo[1], p.y)]
            hi = [max(hi[0], p.x), max(hi[1], p.y)]
        ev.to_mesh_clear()
    return {"radius": radius, "top": top, "min": lo, "max": hi}


def beam(a, b, thickness, mat=None, parent=None, name="beam"):
    """A square bar from point `a` to point `b` (parent space): scaffolds, frames, rails."""
    a, b = Vector(a), Vector(b)
    d = b - a
    o = block((thickness, thickness, d.length), (a + b) / 2, mat=mat, parent=parent, bevel=0, name=name)
    o.rotation_euler = d.to_track_quat("Z", "Y").to_euler()
    return o


# ---------- frame helpers: damage, wrecks, construction, rubble ----------
# Models may call these from their own hooks (damage, wreck); the studio uses them as the defaults when a model
# leaves a hook out, so every model gets the frames before anyone hand-tunes them.

def scorched(name="scorched"):
    """Soot over paint: dark, patchy, matt. Never in the team hue band."""
    return _noisy(name, (0.035, 0.03, 0.027), 9, 6, 0.6, 0.95, bump=0.3)


def burnt(name="burnt"):
    """Burnt-out metal for wrecks: dark grey-brown with rust."""
    return metal(name, (0.06, 0.05, 0.045), (0.16, 0.07, 0.03))


def is_team(o):
    return any(s.material and s.material.get("team_paint") for s in o.material_slots)


def _set_mat(o, m):
    for s in o.material_slots:
        s.material = m


def _size(o):
    d = o.dimensions
    return d.x * d.y * d.z


def default_damage(rng, root):
    """A damaged look: soot on about a third of the parts, two or three small parts gone, one bent."""
    parts = sorted(meshes(root), key=lambda o: o.name)
    soot = scorched()
    for o in rng.sample(parts, max(1, len(parts) // 3)):
        _set_mat(o, soot)
    if len(parts) >= 8:  # a simple model (a wall) keeps all its parts
        small = sorted(parts, key=lambda o: (_size(o), o.name))[: max(3, len(parts) // 4)]
        for o in rng.sample(small, min(len(small), rng.randint(2, 3))):
            bpy.data.objects.remove(o, do_unlink=True)
    left = sorted(meshes(root), key=lambda o: (-_size(o), o.name))[1: max(2, len(parts) // 3)]
    if left:
        o = rng.choice(left)
        o.rotation_euler.x += rng.uniform(0.15, 0.3) * rng.choice((-1, 1))


def default_wreck(rng, root, detail_names=()):
    """A burnt-out hull: every surface burnt (team paint included, so wrecks are never recoloured), detail and a
    few small parts gone, the turret knocked askew and the whole thing settled lower."""
    soot, iron = scorched(), burnt()
    parts = sorted(meshes(root), key=lambda o: o.name)
    for o in parts:
        if o.name.split(".")[0] in detail_names:
            bpy.data.objects.remove(o, do_unlink=True)
            continue
        _set_mat(o, soot if rng.random() < 0.6 else iron)
    parts = sorted(meshes(root), key=lambda o: (_size(o), o.name))
    for o in rng.sample(parts[: len(parts) // 3], min(len(parts) // 3, 3)):
        bpy.data.objects.remove(o, do_unlink=True)
    turret = [o for o in root.children if o.type == "EMPTY" and o.name.startswith("turret")]
    for t in turret:
        t.rotation_euler.z += rng.uniform(0.4, 1.2) * rng.choice((-1, 1))
        t.rotation_euler.x += rng.uniform(-0.08, 0.08)
        t.location.z -= 0.1
    root.location.z -= 0.15


def construction(root, frac, top, area):
    """Clip the model at `frac` of its height (at least its foundation) and stand a scaffold around what is built:
    the construction frames. `area` is (x0, y0, x1, y1) on the ground. Nothing is clipped at 1.0."""
    if frac >= 1:
        return
    cut_z = max(0.35, frac * top)
    bpy.ops.mesh.primitive_cube_add(size=1)
    cutter = bpy.context.active_object
    cutter.name = "cutter"
    x0, y0, x1, y1 = area
    cutter.scale = (x1 - x0 + 20, y1 - y0 + 20, cut_z + 1)
    cutter.location = ((x0 + x1) / 2, (y0 + y1) / 2, cut_z - (cut_z + 1) / 2)
    cutter.hide_render = True
    # The cut faces take the cutter's material: raw concrete, never a part's paint (a cut team band would show
    # as a slab of team colour).
    cutter.data.materials.append(concrete("unfinished", (0.42, 0.4, 0.37)))
    deps = bpy.context.evaluated_depsgraph_get()
    for o in meshes(root):
        ev = o.evaluated_get(deps)
        low = min((ev.matrix_world @ v.co).z for v in ev.to_mesh().vertices) if len(o.data.vertices) else 0
        ev.to_mesh_clear()
        if low >= cut_z:
            bpy.data.objects.remove(o, do_unlink=True)
            continue
        mod = o.modifiers.new("build", "BOOLEAN")
        mod.operation = "INTERSECT"
        mod.solver = "EXACT"
        mod.material_mode = "TRANSFER"
        mod.object = cutter
    # Scaffold: poles round the edge every 2.5 m or so, rails every 2 m, up to just above the cut.
    pole = steel("scaffold", (0.4, 0.38, 0.33))
    h = cut_z + 1.2
    x0, y0, x1, y1 = x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3
    nx, ny = max(2, round((x1 - x0) / 2.5) + 1), max(2, round((y1 - y0) / 2.5) + 1)
    xs = [x0 + (x1 - x0) * i / (nx - 1) for i in range(nx)]
    ys = [y0 + (y1 - y0) * i / (ny - 1) for i in range(ny)]
    ring = [(x, y0) for x in xs] + [(x1, y) for y in ys[1:]] + [(x, y1) for x in reversed(xs[:-1])] + \
           [(x0, y) for y in reversed(ys[1:-1])]
    group_ = group("scaffold", parent=root)
    for x, y in ring:
        beam((x, y, 0), (x, y, h), 0.25, pole, group_, "scaffold")
    z = 2.0
    while z < h:
        for (xa, ya), (xb, yb) in ((( x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)),
                                   ((x0, y1), (x0, y0))):
            beam((xa, ya, z), (xb, yb, z), 0.25, pole, group_, "scaffold")
        z += 2.0


def rubble(rng, root, footprint):
    """Rubble for a footprint size: a scorched patch and a heap of concrete and steel chunks, shared by every
    building of that size."""
    T = STUDIO["metres_per_tile"]
    W, H = footprint[0] * T, footprint[1] * T
    conc, soot, iron = concrete("rubble_concrete", (0.36, 0.34, 0.31)), scorched(), burnt()
    # Scorch as overlapping flat patches, so the edge is ragged rather than a dark square.
    for k in range(3 * footprint[0] * footprint[1]):
        r = rng.uniform(0.18, 0.3) * T
        x, y = rng.uniform(0.6 + r, W - 0.6 - r), -rng.uniform(0.6 + r, H - 0.6 - r)
        cylinder(r, 0.05, (x, y, 0.025 + 0.004 * k), mat=soot, parent=root, verts=10, bevel=0, name="scorch",
                 scale=(1, rng.uniform(0.6, 1), 1))
    n = int(14 * footprint[0] * footprint[1])
    for k in range(n):
        s = rng.uniform(0.6, 2.2)
        # A tilted chunk reaches about its size from its centre: keep it the inset inside the footprint.
        x, y = rng.uniform(0.6 + s, W - 0.6 - s), -rng.uniform(0.6 + s, H - 0.6 - s)
        m = rng.choice((conc, conc, conc, soot, iron))
        block((s, s * rng.uniform(0.5, 1.2), s * rng.uniform(0.3, 0.7)), (x, y, s * 0.2),
              rot=(rng.uniform(-0.4, 0.4), rng.uniform(-0.4, 0.4), rng.uniform(0, math.pi)), mat=m, parent=root,
              bevel=0.06, name="chunk")
    for k in range(footprint[0] * footprint[1]):
        x, y = rng.uniform(1.5, W - 1.5), -rng.uniform(1.5, H - 1.5)
        x2 = min(max(x + rng.uniform(-2, 2), 1.0), W - 1.0)
        y2 = min(max(y + rng.uniform(-2, 2), -H + 1.0), -1.0)
        beam((x, y, 0.3), (x2, y2, rng.uniform(0.8, 2.0)), 0.3, iron, root, "girder")


# ---------- infantry rig ----------
# Rigid parts on a scripted rig (art-pipeline.md section 6, route A): twelve joints as a hierarchy of empties, each
# part parented to one joint, posed in code. At sprite size rigid parts read the same as a skinned mesh, and an
# empty hierarchy needs no armature binding. Sizes are for a 1.75 m soldier; the model is built at 1.75x
# (art-pipeline.md section 1) by the root's scale.

INFANTRY_SCALE = 1.75
JOINTS = {  # name: (parent, location relative to the parent, metres, unscaled)
    "pelvis": (None, (0, 0, 0.95)),
    "spine": ("pelvis", (0, 0, 0.12)),
    "head": ("spine", (0, 0, 0.5)),
    "shoulder_l": ("spine", (-0.21, 0, 0.42)),
    "elbow_l": ("shoulder_l", (0, 0, -0.29)),
    "shoulder_r": ("spine", (0.21, 0, 0.42)),
    "elbow_r": ("shoulder_r", (0, 0, -0.29)),
    "hip_l": ("pelvis", (-0.1, 0, -0.05)),
    "knee_l": ("hip_l", (0, 0, -0.43)),
    "hip_r": ("pelvis", (0.1, 0, -0.05)),
    "knee_r": ("hip_r", (0, 0, -0.43)),
    "weapon": ("spine", (0.12, 0.3, 0.3)),
}


def rig(root):
    """The twelve joints under `root`, at rest (standing, arms down, facing +y). Returns {name: empty}."""
    body = group("body", parent=root)
    body.scale = (INFANTRY_SCALE,) * 3
    joints = {}
    for name, (parent, loc) in JOINTS.items():
        joints[name] = group(name, loc, parent=joints[parent] if parent else body)
    joints["body"] = body
    return joints


def _ease(t):
    return 0.5 - 0.5 * math.cos(math.pi * min(max(t, 0), 1))


def pose(joints, anim, frame, frames):
    """Set the rig to `frame` of `anim`. Every cycle is a function of the frame alone, so a frame always renders
    the same. Rotations are radians about x (forward swing), with the rest pose at zero."""
    for name, j in joints.items():
        if name == "body":
            j.rotation_euler = (0, 0, 0)
            j.location = (0, 0, 0)
        else:
            j.rotation_euler = (0, 0, 0)
            j.location = JOINTS[name][1]
    p = frame / frames
    aim = {"shoulder_r": 1.35, "elbow_r": 0.25, "shoulder_l": 1.25, "elbow_l": 0.6, "weapon": 0.0}
    if anim in ("idle", "fire", "walk"):
        for k, v in aim.items():
            joints[k].rotation_euler.x = v
        joints["shoulder_l"].rotation_euler.z = -0.35
    if anim == "idle":
        joints["spine"].rotation_euler.x = 0.05
    elif anim == "walk":
        s = math.sin(2 * math.pi * p)
        c = math.cos(2 * math.pi * p)
        joints["hip_l"].rotation_euler.x = 0.45 * s
        joints["hip_r"].rotation_euler.x = -0.45 * s
        joints["knee_l"].rotation_euler.x = -0.6 * max(0, -c)
        joints["knee_r"].rotation_euler.x = -0.6 * max(0, c)
        joints["spine"].rotation_euler.x = 0.12
        joints["pelvis"].location = (0, 0, 0.95 - 0.04 * abs(c))
    elif anim == "fire":
        # Kneel and fire: recoil on the middle frame.
        joints["pelvis"].location = (0, 0, 0.62)
        joints["hip_l"].rotation_euler.x = 1.5
        joints["knee_l"].rotation_euler.x = -1.5
        joints["hip_r"].rotation_euler.x = -0.2
        joints["knee_r"].rotation_euler.x = -1.35
        kick = 0.18 if frame == 1 else 0.0
        joints["spine"].rotation_euler.x = 0.1 - kick
        joints["weapon"].location = (0.12, 0.3 - kick, 0.3)
    elif anim in ("die-1", "die-2"):
        # die-1 falls backwards, die-2 crumples forward; both end flat on the ground.
        t = _ease(p * frames / (frames - 1))
        back = anim == "die-1"
        joints["body"].rotation_euler.x = (1.45 if back else -1.45) * t
        joints["body"].location = (0, 0, 0.25 * t)
        joints["shoulder_r"].rotation_euler.x = 1.3 * (1 - t) + (2.6 if back else -0.4) * t
        joints["shoulder_l"].rotation_euler.x = 1.2 * (1 - t) + (2.4 if back else -0.3) * t
        joints["hip_l"].rotation_euler.x = 0.4 * t
        joints["knee_l"].rotation_euler.x = -0.8 * t
        joints["spine"].rotation_euler.x = (-0.2 if back else 0.4) * t
        joints["head"].rotation_euler.x = (-0.3 if back else 0.5) * t


# ---------- other cameras ----------

def portrait_camera(scene, b, size=(768, 576), turn_deg=30):
    """The build-icon camera: 35 degrees up, turned `turn_deg` to the right of the unit camera, framed to the
    model's bounds. Perspective, slightly long lens, so icons look like a close-up rather than a map sprite."""
    data = bpy.data.cameras.new("portrait")
    data.lens = 85
    cam = bpy.data.objects.new("portrait", data)
    scene.collection.objects.link(cam)
    centre = Vector(((b["min"][0] + b["max"][0]) / 2, (b["min"][1] + b["max"][1]) / 2, b["top"] * 0.3))
    span = max(b["max"][0] - b["min"][0], b["max"][1] - b["min"][1], b["top"]) * 1.05
    el, turn = math.radians(STUDIO["portrait"]["elevation_deg"]), math.radians(turn_deg)
    # 85 mm lens on a 36 mm sensor: the half-width angle fits `span` at this distance, with a margin.
    dist = span * 0.5 / math.tan(math.atan(18 / 85)) * 1.45
    d = Vector((math.sin(turn) * math.cos(el), -math.cos(turn) * math.cos(el), math.sin(el)))
    cam.location = centre + d * dist
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    scene.render.resolution_x, scene.render.resolution_y = size
    scene.camera = cam
    return cam


def backdrop(scene):
    """A plain dark backdrop for icons: a big ground plane in a cool grey."""
    bpy.ops.mesh.primitive_plane_add(size=400, location=(0, 0, -0.01))
    plane = bpy.context.active_object
    plane.name = "backdrop"
    plane.data.materials.append(plain("backdrop", (0.06, 0.07, 0.08), roughness=0.9))
    return plane


def contact(root, below=0.15):
    """Ground contact: the horizontal extent (x0, y0, x1, y1) of every vertex within `below` metres of the
    ground, for the footprint inset check."""
    deps = bpy.context.evaluated_depsgraph_get()
    lo, hi = [math.inf, math.inf], [-math.inf, -math.inf]
    for o in meshes(root):
        ev = o.evaluated_get(deps)
        mesh = ev.to_mesh()
        for v in mesh.vertices:
            p = ev.matrix_world @ v.co
            if p.z <= below:
                lo = [min(lo[0], p.x), min(lo[1], p.y)]
                hi = [max(hi[0], p.x), max(hi[1], p.y)]
        ev.to_mesh_clear()
    return None if lo[0] == math.inf else [lo[0], lo[1], hi[0], hi[1]]


def screen_box(scene, objs, pad_px=4):
    """Pixel box (x0, y0, x1, y1), y down, that `objs` cover in the current camera, padded and clamped."""
    from bpy_extras.object_utils import world_to_camera_view
    w, h = scene.render.resolution_x, scene.render.resolution_y
    deps = bpy.context.evaluated_depsgraph_get()
    xs, ys = [], []
    for o in objs:
        ev = o.evaluated_get(deps)
        mesh = ev.to_mesh()
        for v in mesh.vertices:
            c = world_to_camera_view(scene, scene.camera, ev.matrix_world @ v.co)
            xs.append(c.x * w)
            ys.append((1 - c.y) * h)
        ev.to_mesh_clear()
    x0 = max(0, math.floor(min(xs)) - pad_px)
    y0 = max(0, math.floor(min(ys)) - pad_px)
    return x0, y0, min(w, math.ceil(max(xs)) + pad_px), min(h, math.ceil(max(ys)) + pad_px)
