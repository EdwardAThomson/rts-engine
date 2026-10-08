"""Export studio models as glTF binaries (.glb) for a real-time 3D renderer, such as the 3D engine's.

    python3 art/studio/export_gltf.py vehicles/battle_tank [buildings/refinery ...] --out DIR [--tex 1024]
        [--style detailed|classic] [--models DIR] [--pack NAME]
    python3 art/studio/export_gltf.py --all --out DIR

The models are the same files render.py draws sprites from; each is built as its intact frame. glTF carries no
procedural shaders, so every part is joined into one mesh, unwrapped, and its base colour baked into one texture.
Each part then has two materials: `<part>_body` with the baked texture, and `<part>_team` whose baked texture is
neutral grey and whose glTF extras say `"team_paint": true`, so the renderer multiplies in the player's colour.

Parts and pivots (glTF is +Y up, 1 unit = 1 metre; a model's north, its front, is -Z):
  vehicles, aircraft   node `hull` at the vehicle's origin on the ground; node `turret`, a child of the hull,
                       at the turret ring (yaw it about its own Y)
  buildings            node `building` at the centre of the footprint on the ground; node `head` for a defence
                       turret, a child at the head's ring. Idle and door parts are part of the building mesh.
Infantry are skipped: their rig animates, which this static export does not carry.

Polygon counts come straight from the models (bevels and small parts are costly); `--style classic` drops both
and makes a natural far level of detail.
"""
import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bpy  # noqa: E402
import render as R  # noqa: E402
import rts_studio as st  # noqa: E402
from mathutils import Vector  # noqa: E402

BAKE_SAMPLES = 16
# Team paint bakes as this grey (with the paint's own noise), the colour the renderer multiplies by.
TEAM_GREY = (0.48, 0.62)


def parts(mod, kind, eid, style):
    """Build the intact model; return {part: (objects, pivot in Blender world space, parent part or None)}."""
    ctx = R.build_scene(mod, kind, eid, style, BAKE_SAMPLES, 0, "intact")
    g = ctx["groups"]
    if kind == "building":
        T = st.STUDIO["metres_per_tile"]
        w, h = mod.FOOTPRINT
        out = {"building": (g["base"] + g.get("idle", []) + g.get("doors", []), (w * T / 2, -h * T / 2, 0), None)}
        if g.get("head"):
            out["head"] = (g["head"], tuple(ctx["ring"].matrix_world.translation), "building")
    else:
        out = {"hull": (g["hull"], (0, 0, 0), None)}
        if g.get("turret"):
            out["turret"] = (g["turret"], (0, 0, mod.TURRET_HEIGHT), "hull")
    # A part listed twice (an idle part also in base) is joined once.
    return {k: (list(dict.fromkeys(objs)), pivot, parent) for k, (objs, pivot, parent) in out.items() if objs}


def select(objs, active=None):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.hide_set(False)
        o.select_set(True)
    bpy.context.view_layer.objects.active = active or objs[0]


def join(name, objs, pivot):
    """One mesh object for the part, modifiers applied, unparented, its origin on the pivot."""
    select(objs)
    bpy.ops.object.convert(target="MESH")
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.join()
    part = bpy.context.active_object
    free_name(bpy.data.objects, name)
    free_name(bpy.data.meshes, name)
    part.name = part.data.name = name
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)  # the joined part keeps no turn
    bpy.context.scene.cursor.location = pivot
    bpy.ops.object.origin_set(type="ORIGIN_CURSOR")
    return part


def free_name(coll, name):
    """Rename whatever already holds `name` (a model's group or material), so the exported one gets it plain."""
    if coll.get(name):
        coll[name].name = f"{name}_src"


def is_team(mat):
    return mat is not None and bool(mat.get("team_paint"))


def bake(name, part, tex):
    """Unwrap the part, bake its base colour into one image, and swap its materials for body and team."""
    select([part])
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.004)
    bpy.ops.object.mode_set(mode="OBJECT")

    img = bpy.data.images.new(f"{name}_albedo", tex, tex, alpha=False)
    for m in part.data.materials:
        nt = m.node_tree
        for n in [n for n in nt.nodes if n.name.startswith("bake_target")]:
            nt.nodes.remove(n)
        if is_team(m):
            for n in nt.nodes:
                if n.type == "VALTORGB":
                    for el, v in zip(n.color_ramp.elements, TEAM_GREY):
                        el.color = (v, v, v, 1)
        node = nt.nodes.new("ShaderNodeTexImage")
        node.name = "bake_target"
        node.image = img
        nt.nodes.active = node
    scene = bpy.context.scene
    scene.cycles.samples = BAKE_SAMPLES
    bake_settings = scene.render.bake
    bake_settings.use_pass_direct = bake_settings.use_pass_indirect = False
    bake_settings.use_pass_color = True
    bake_settings.margin = 4
    bpy.ops.object.bake(type="DIFFUSE")
    img.pack()

    slots = [1 if is_team(part.data.materials[p.material_index]) else 0 for p in part.data.polygons]
    part.data.materials.clear()  # resets every face to slot 0
    for suffix, team in (("body", False), ("team", True)):
        free_name(bpy.data.materials, f"{name}_{suffix}")
        m = bpy.data.materials.new(f"{name}_{suffix}")
        m.use_nodes = True
        nt = m.node_tree
        bsdf = nt.nodes["Principled BSDF"]
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = img
        nt.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        bsdf.inputs["Roughness"].default_value = 0.6
        bsdf.inputs["Metallic"].default_value = 0.2
        if team:
            m["team_paint"] = True
        part.data.materials.append(m)
    for p, s in zip(part.data.polygons, slots):
        p.material_index = s


def triangles(obj):
    m = obj.data
    m.calc_loop_triangles()
    return len(m.loop_triangles)


def export(model, out, tex, style, roots):
    mod, category, eid, _ = R.load(model, roots)
    if category not in R.KINDS:
        category = getattr(mod, "CATEGORY", "vehicles")
    kind = R.KINDS[category]
    if kind == "infantry":
        print(f"{eid}: skipped (infantry animate on a rig; this export is static)")
        return None
    built, origin = {}, None
    for name, (objs, pivot, parent) in parts(mod, kind, eid, style).items():
        built[name] = (join(name, objs, pivot), parent)
        origin = origin or pivot
    for part, _ in built.values():  # the root part's pivot becomes the file's origin
        part.location = part.location - Vector(origin)
    for name, (part, _) in built.items():
        bake(name, part, tex)
    for part, parent in built.values():
        if parent:
            mw = part.matrix_world.copy()
            part.parent = built[parent][0]
            part.matrix_world = mw
    objs = [p for p, _ in built.values()]
    select(objs)
    path = out / f"{eid}.glb"
    bpy.ops.export_scene.gltf(filepath=str(path), export_format="GLB", use_selection=True, export_apply=True,
                              export_yup=True, export_extras=True)
    stats = {"id": eid, "category": category, "style": style, "bytes": path.stat().st_size,
             "triangles": {name: triangles(p) for name, (p, _) in built.items()}}
    print(json.dumps(stats))
    return stats


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("models", nargs="*", help="category/id under art/models (e.g. vehicles/battle_tank)")
    ap.add_argument("--all", action="store_true", help="every vehicle, aircraft and building in the model roots")
    ap.add_argument("--out", required=True)
    ap.add_argument("--tex", type=int, default=1024, help="baked texture size per part, in pixels")
    ap.add_argument("--style", default="detailed", help="a style from studio.json")
    ap.add_argument("--models", dest="roots", action="append", default=[],
                    help="a pack's model folder (holding <category>/<id>.py), searched before art/models")
    ap.add_argument("--pack", help="a private pack by name: adds settings-private/art/<pack>/models to --models")
    args = ap.parse_args()
    if args.pack:
        args.roots.insert(0, str(R.pack_models(args.pack)))
    roots = R.model_roots(args.roots)
    models = list(args.models)
    if args.all:
        found = {f"{p.parent.name}/{p.stem}" for r in roots for c in ("vehicles", "aircraft", "buildings")
                 for p in (r / c).glob("*.py") if not p.stem.startswith("_")}
        models += sorted(found - set(models))
    if not models:
        ap.error("name a model or pass --all")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    stats = [s for m in models if (s := export(m, out, args.tex, args.style, roots))]
    (out / "export.json").write_text(json.dumps(stats, indent=2) + "\n")


if __name__ == "__main__":
    main()
