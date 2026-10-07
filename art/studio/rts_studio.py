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


def render_px_per_metre():
    return STUDIO["atlas_px_per_tile"] * STUDIO["render_scale"] / STUDIO["metres_per_tile"]


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
    _sun(scene, "rim", (-0.5, 1.0, -0.6), 0.6, (0.7, 0.8, 1.0), 10)
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
    return _noisy(name, rgb, 18, 6, 0.25, 0.55, bump=0.15)


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
    if bevel:
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
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=radius, depth=depth,
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
    bpy.ops.mesh.primitive_cone_add(vertices=verts, radius1=r1, radius2=r2, depth=depth)
    o = bpy.context.active_object
    bpy.ops.object.shade_smooth()
    return _place(o, loc, rot, mat, parent, name, 0)


def sphere(radius, loc, mat=None, parent=None, scale=(1, 1, 1), name="sphere"):
    bpy.ops.mesh.primitive_uv_sphere_add(radius=radius, segments=24, ring_count=12)
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


def meshes(root):
    return [o for o in root.children_recursive if o.type == "MESH"]


def bounds(root):
    """Horizontal radius about the origin and top height, over the evaluated meshes under `root`."""
    deps = bpy.context.evaluated_depsgraph_get()
    radius, top, lo = 0.0, 0.0, [math.inf, math.inf]
    hi = [-math.inf, -math.inf]
    for o in meshes(root):
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
