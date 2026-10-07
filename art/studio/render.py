"""Render one entity's frames with the studio: every part, animation, facing and frame the model asks for, plus
shadow passes. The packer (pack.py) does the rest.

    python3 art/studio/render.py vehicles/battle_tank --out RENDER_DIR [--samples N] [--style S] [--scale N]
        [--facings N] [--only I] [--jobs hull/idle,wreck/idle] [--threads N] [--no-log]

What a model file defines, by category (art/models/<category>/<id>.py; see art/README.md for the full contract):

  vehicles, aircraft   build_hull(root); optional build_turret(ring) with TURRET_HEIGHT; DETAIL; ANIMS and
                       pose(root, anim, frame); wreck(rng, root) (vehicles get a default wreck without it)
  buildings            FOOTPRINT and build(root); DETAIL; damage(rng, root); IDLE_PARTS, IDLE_FRAMES and
                       idle_pose(root, t); DOORS, DOOR_FRAMES and door_pose(root, t); build_head(ring) with
                       HEAD_HEIGHT for defence turrets; JOINS = True and build(root, joins) for walls
  infantry             build(root, joints), parts parented to the joints of st.rig(); optional
                       rig_pose(joints, anim, frame, frames) in place of st.pose

Every frame is written as <part>-<anim>-fFF-NN.png (FF the facing clockwise from north, NN the frame), with
<...>.shadow.png beside it where the part casts a shadow, and meta.json describes the jobs and where the origin
landed in render pixels. Each render job's time is appended to art/timings.jsonl.
"""
import argparse
import datetime
import importlib.util
import json
import math
import os
import random
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402

ART = Path(__file__).parent.parent
KINDS = {"vehicles": "vehicle", "aircraft": "aircraft", "buildings": "building", "infantry": "infantry"}
# Rest frames for buildings, then the construction frames: a clip at these fractions of the model's height.
BUILD_STAGES = (0.0, 0.35, 0.7, 1.0)
INFANTRY_ANIMS = {"idle": 1, "walk": 6, "fire": 3, "die-1": 8, "die-2": 8}


PRIVATE = ART.parent / "settings-private"  # the private packs' repository, cloned here and git-ignored


def pack_models(name):
    """A private pack's model folder: art/<pack>/models in the private repository."""
    return PRIVATE / "art" / name / "models"


def model_roots(extra=()):
    """Where models are looked up, first match wins: --models folders, then RTS_ART_MODELS (separated by the
    path separator), then this repository's art/models. A private pack's folder overrides a generic model by id."""
    env = [p for p in os.environ.get("RTS_ART_MODELS", "").split(os.pathsep) if p]
    return [Path(p) for p in (*extra, *env)] + [ART / "models"]


def load(model, roots=None):
    """A model by category/id from the model roots, or by a path to a .py file (the studio's own test models).
    Returns (module, category, id, path)."""
    if model.endswith(".py"):
        path = Path(model)
    else:
        path = next((r / f"{model}.py" for r in roots or model_roots() if (r / f"{model}.py").exists()), None)
        if path is None:
            sys.exit(f"no model {model} in {', '.join(str(r) for r in roots or model_roots())}")
    spec = importlib.util.spec_from_file_location(path.stem, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod, path.parent.name, path.stem, path.resolve()


def render(scene, path):
    import bpy
    scene.render.filepath = str(path)
    bpy.ops.render.render(write_still=True)


def named(root, names):
    """Mesh objects under `root` whose name (or an ancestor's name) is in `names`; Blender's .001 suffixes are
    ignored. The top-most matching objects are returned separately, for posing."""
    names = set(names)
    tops, objs = [], []

    def base(o):
        return o.name.split(".")[0]

    for o in root.children_recursive:
        if base(o) in names and not (o.parent and any(base(a) in names for a in _ancestors(o))):
            tops.append(o)
    for t in tops:
        objs += [t] if t.type == "MESH" else []
        objs += [c for c in t.children_recursive if c.type == "MESH"]
    return tops, objs


def _ancestors(o):
    p = o.parent
    while p:
        yield p
        p = p.parent


def default_idle_pose(tops, rest, t):
    """Turn each moving part once round its own vertical axis over the loop: dishes, fans, rotors."""
    for o, r in zip(tops, rest):
        o.rotation_euler.z = r[2] + 2 * math.pi * t


def default_door_pose(tops, rest, t):
    """A roller door: each door part shrinks upward into its top edge as `t` goes 0 (shut) to 1 (open)."""
    for o, (loc, scale) in zip(tops, rest):
        h = o.dimensions.z / max(o.scale.z, 1e-6) * scale[2]
        o.scale.z = scale[2] * (1 - 0.85 * t)
        o.location.z = loc[2] + h * 0.85 * t / 2


def external(path):
    """True for a model from outside this repository's tracked files (a private pack, including the git-ignored
    settings-private/ clone): its renders must not be packed into this repository."""
    p, repo = str(path), str(ART.parent.resolve()) + os.sep
    return not p.startswith(repo) or p.startswith(str(PRIVATE.resolve()) + os.sep)


class Job:
    """One part's animation: `facings` x `frames` images, all from one variant of the scene."""

    def __init__(self, part, anim, facings, frames=1, shadow=False, overlay=False, turn=None, pose=None,
                 select=None, yaw=0.0):
        self.part, self.anim, self.facings, self.frames = part, anim, facings, frames
        self.shadow, self.overlay, self.yaw = shadow, overlay, yaw
        self.turn = turn or (lambda ctx, a: setattr_z(ctx["root"], a))
        self.pose = pose or (lambda ctx, n: None)
        self.select = select  # ctx -> (visible, holdout, casters)
        self.crop = None
        self.frame_of_variant = None  # set when each frame comes from its own variant (construction, wall joins)

    def meta(self):
        m = {"part": self.part, "anim": self.anim, "facings": self.facings, "frames": self.frames,
             "shadow": self.shadow}
        if self.overlay:
            m["overlay"] = True
        if self.crop:
            m["crop"] = list(self.crop)
        return m


def setattr_z(o, a):
    o.rotation_euler[2] = a


def build_scene(mod, kind, eid, style, samples, threads, variant):
    """Build the model fresh for one variant ('intact', 'wreck', 'damaged', 'build-K', 'joins-M',
    'joins-M-damaged') and apply its changes. Returns the context the jobs read."""
    scene = st.reset_scene()
    scene.cycles.samples = samples
    if threads:
        scene.render.threads_mode = "FIXED"
        scene.render.threads = threads
    st.lights(scene)
    root = st.group(eid)
    ctx = {"scene": scene, "root": root, "groups": {}}
    rng = random.Random(f"{eid}:{variant}")
    detail = set(getattr(mod, "DETAIL", ()))
    if kind == "building":
        if getattr(mod, "JOINS", False):
            joins = int(variant.split("-")[1]) if variant.startswith("joins") else 0
            mod.build(root, joins)
        else:
            mod.build(root)
        base = st.meshes(root)
        if hasattr(mod, "build_head"):
            W, H = mod.FOOTPRINT
            T = st.STUDIO["metres_per_tile"]
            ring = st.group("head", (W * T / 2, -H * T / 2, mod.HEAD_HEIGHT), parent=root)
            mod.build_head(ring)
            ctx["ring"] = ring
            ctx["groups"]["head"] = st.meshes(ring)
            base = [o for o in base if o not in ctx["groups"]["head"]]
        ctx["groups"]["base"] = base
    elif kind == "infantry":
        ctx["joints"] = st.rig(root)
        mod.build(root, ctx["joints"])
        ctx["groups"]["body"] = st.meshes(root)
    else:
        mod.build_hull(root)
        ctx["groups"]["hull"] = st.meshes(root)
        if hasattr(mod, "build_turret"):
            ring = st.group("turret", (0, 0, mod.TURRET_HEIGHT), parent=root)
            mod.build_turret(ring)
            ctx["groups"]["turret"] = st.meshes(ring)
    ctx["intact_bounds"] = st.bounds(root)
    ctx["groups"] = {k: [o.name for o in v] for k, v in ctx["groups"].items()}  # variants may delete objects

    if variant == "wreck":
        (mod.wreck if hasattr(mod, "wreck") else lambda r, o: st.default_wreck(r, o, detail))(rng, root)
    elif variant.endswith("damaged"):
        (mod.damage if hasattr(mod, "damage") else st.default_damage)(rng, root)
    elif variant.startswith("build-"):
        b = ctx["intact_bounds"]
        frac = BUILD_STAGES[int(variant.split("-")[1])]
        area = (b["min"][0], b["min"][1], b["max"][0], b["max"][1])
        if kind == "building":
            # Scaffold the footprint, not the intact bounds: a turret's barrel reaching past the tile must not
            # stretch the scaffold over the next tile.
            T = st.STUDIO["metres_per_tile"]
            fw, fh = mod.FOOTPRINT[0] * T, mod.FOOTPRINT[1] * T
            inset = st.STUDIO["footprint_inset_m"]
            area = (max(area[0], inset), max(area[1], -fh + inset), min(area[2], fw - inset), min(area[3], -inset))
        st.construction(root, frac, b["top"], area)
    st.apply_style(scene, root, style, detail)
    import bpy
    bpy.context.view_layer.update()
    def live(objs):
        found = [bpy.data.objects.get(o if isinstance(o, str) else o.name) for o in objs]
        return [o for o in found if o is not None and not o.hide_viewport]

    ctx["all"] = live(st.meshes(root))
    ctx["groups"] = {k: live(v) for k, v in ctx["groups"].items()}
    for key, names in (("idle", getattr(mod, "IDLE_PARTS", ())), ("doors", getattr(mod, "DOORS", ()))):
        tops, objs = named(root, names)
        ctx[f"{key}_tops"], ctx["groups"][key] = tops, live(objs)
        ctx[f"{key}_rest"] = [(tuple(o.location), tuple(o.scale)) if key == "doors" else tuple(o.rotation_euler)
                              for o in tops]
    return ctx


def plan(mod, kind, facings, has_wreck):
    """The variants and the jobs each one renders."""
    F = st.STUDIO["facings"]
    anims = {k: (v if isinstance(v, dict) else {"frames": v}) for k, v in getattr(mod, "ANIMS", {}).items()}
    out = []

    def nf(n):
        return facings or n

    if kind in ("vehicle", "aircraft"):
        air = kind == "aircraft"
        pose = (lambda a: lambda ctx, n: mod.pose(ctx["root"], a, n)) if hasattr(mod, "pose") else \
            (lambda a: lambda ctx, n: None)
        jobs = []
        for part in ("hull", "turret"):
            if part == "turret" and not hasattr(mod, "build_turret"):
                continue
            n = F["hull" if part == "hull" else "turret"]
            mine = {a: c for a, c in anims.items() if c.get("part", "hull") == part}
            mine.setdefault("idle", {"frames": 1})
            for a, c in mine.items():
                if part == "hull":
                    sel = (lambda ctx: (ctx["groups"]["hull"], [], ctx["all"]))
                else:
                    sel = (lambda ctx: (ctx["groups"]["turret"], [], None))
                jobs.append(Job(part, a, nf(n), c["frames"], shadow=part == "hull" and (a == "idle" or air),
                                pose=pose(a), select=sel))
        out.append(("intact", jobs))
        if has_wreck:
            out.append(("wreck", [Job("wreck", "idle", nf(F["wreck"]), shadow=True,
                                      select=lambda ctx: (ctx["all"], [], ctx["all"]))]))
    elif kind == "building" and getattr(mod, "DECAL", False):
        out.append(("intact", [Job("building", "idle", 1, select=lambda ctx: (ctx["all"], [], None))]))
    elif kind == "building":
        head = hasattr(mod, "build_head")
        joins = getattr(mod, "JOINS", False)

        def base_sel(ctx):
            moving = ctx["groups"]["idle"] + ctx["groups"]["doors"]
            vis = [o for o in ctx["groups"]["base"] if o not in moving]
            return vis, [], ctx["all"]

        whole = lambda ctx: (ctx["all"], [], ctx["all"])  # noqa: E731
        if joins:
            out += [(f"joins-{m}", [Job("building", "idle", 1, 16, shadow=True, select=whole,
                                        pose=None)]) for m in range(16)]
            out += [(f"joins-{m}-damaged", [Job("building", "damaged", 1, 16, shadow=True, select=whole)])
                    for m in range(16)]
            # Several variants feed one job: the frame index is the join mask.
            for v, js in out:
                js[0].frame_of_variant = int(v.split("-")[1])
        else:
            jobs = [Job("building", "idle", 1, shadow=True, select=base_sel)]
            if head:
                jobs.append(Job("turret", "idle", nf(F["turret"]), select=lambda ctx: (ctx["groups"]["head"], [],
                                                                                         None),
                                turn=lambda ctx, a: setattr_z(ctx["ring"], a)))
            if getattr(mod, "IDLE_PARTS", ()):
                n = getattr(mod, "IDLE_FRAMES", 8)
                fn = getattr(mod, "idle_pose", None)
                jobs.append(Job("overlay-idle", "idle", 1, n, overlay=True,
                                select=lambda ctx: (ctx["groups"]["idle"],
                                                    [o for o in ctx["all"] if o not in ctx["groups"]["idle"]], None),
                                pose=lambda ctx, k, n=n, fn=fn: (fn(ctx["root"], k / n) if fn else
                                                                 default_idle_pose(ctx["idle_tops"],
                                                                                   ctx["idle_rest"], k / n))))
            if getattr(mod, "DOORS", ()):
                n = getattr(mod, "DOOR_FRAMES", 3)
                fn = getattr(mod, "door_pose", None)
                jobs.append(Job("overlay-doors", "open", 1, n + 1, overlay=True,
                                select=lambda ctx: (ctx["groups"]["doors"],
                                                    [o for o in ctx["all"] if o not in ctx["groups"]["doors"]],
                                                    None),
                                pose=lambda ctx, k, n=n, fn=fn: (fn(ctx["root"], k / n) if fn else
                                                                 default_door_pose(ctx["doors_tops"],
                                                                                   ctx["doors_rest"], k / n))))
            out.append(("intact", jobs))
            out.append(("damaged", [Job("building", "damaged", 1, shadow=True, select=whole)]))
        for k in range(len(BUILD_STAGES)):
            j = Job("building", "build", 1, len(BUILD_STAGES), shadow=True, select=whole)
            j.frame_of_variant = k
            out.append((f"build-{k}", [j]))
    elif kind == "infantry":
        jobs = []
        for a, n in INFANTRY_ANIMS.items():
            die = a.startswith("die")
            jobs.append(Job("body", a, 1 if die else nf(F["infantry"]), n, shadow=True,
                            select=lambda ctx: (ctx["all"], [], ctx["all"]),
                            pose=lambda ctx, k, a=a, n=n: getattr(mod, "rig_pose", st.pose)(ctx["joints"], a, k, n),
                            yaw=math.pi / 2 if die else 0.0))
        out.append(("intact", jobs))
    return out


def canvas(kind, b, scaffold):
    """Metres of canvas left, right, up and down of the origin, from the intact model's bounds."""
    el = math.radians(st.STUDIO["elevation_deg"])
    top = b["top"] + (1.5 if scaffold else 0)
    sx, sy = st.shadow_extent(top)
    pad = 0.3
    if kind == "building":
        pad = 0.9 if scaffold else pad
        return (-b["min"][0] + pad, b["max"][0] + sx + pad,
                b["max"][1] * math.sin(el) + top * math.cos(el) + pad,
                (-b["min"][1] + sy) * math.sin(el) + pad)
    r = b["radius"] + (0.6 if kind == "infantry" else 0)
    if kind == "infantry":
        top = max(top, 2.0)
    return (r + pad, r + pad + sx, r * math.sin(el) + top * math.cos(el) + pad, (r + sy) * math.sin(el) + pad)


def shoot(ctx, ground, path, visible, holdout, casters, shadow_samples):
    """One frame: the visible objects with the holdouts cutting them, then (if `casters`) the shadow alone."""
    scene = ctx["scene"]
    vis, hold = {o.name for o in visible}, {o.name for o in holdout}
    for o in ctx["all"]:
        o.hide_render = o.name not in vis and o.name not in hold
        o.is_holdout = o.name in hold
        o.visible_camera = True
    ground.hide_render = True
    render(scene, path.with_suffix(".png"))
    if casters is None:
        return 1
    cast = {o.name for o in casters}
    for o in ctx["all"]:
        o.hide_render = o.name not in cast
        o.is_holdout = False
        o.visible_camera = False
    ground.hide_render = False
    # Key light only: sky light blocked by the model would leave a faint grey over the whole canvas.
    bg = scene.world.node_tree.nodes["Background"].inputs["Strength"]
    keep, scene.cycles.samples = scene.cycles.samples, shadow_samples
    keep_bg, bg.default_value = bg.default_value, 0.0
    render(scene, path.with_suffix(".shadow.png"))
    scene.cycles.samples, bg.default_value = keep, keep_bg
    for o in ctx["all"]:
        o.visible_camera = True
    return 2


def leak_test(ctx, ground, path, visible):
    """Facing 0 again with every team paint swapped for magenta at a few samples: anything still in the team hue
    band afterwards is a leak (another material the packer would recolour). check.py reads it."""
    import bpy
    swapped = []
    for m in bpy.data.materials:
        if m.get("team_paint"):
            bsdf = m.node_tree.nodes["Principled BSDF"]
            for link in list(bsdf.inputs["Base Color"].links):
                m.node_tree.links.remove(link)
            swapped.append((m, tuple(bsdf.inputs["Base Color"].default_value)))
            bsdf.inputs["Base Color"].default_value = (0.25, 0.01, 0.2, 1)
    scene = ctx["scene"]
    keep, scene.cycles.samples = scene.cycles.samples, 16
    shoot(ctx, ground, path, visible, [], None, 0)
    scene.cycles.samples = keep


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("model", help="category/id under art/models (e.g. vehicles/battle_tank) or a .py path")
    ap.add_argument("--out", required=True)
    ap.add_argument("--samples", type=int, default=st.STUDIO["samples"])
    ap.add_argument("--facings", type=int, help="override every multi-facing job's count (quick tests)")
    ap.add_argument("--only", type=int, help="render just this facing index of each job (quick tests)")
    ap.add_argument("--jobs", help="comma-separated part/anim list to render, e.g. hull/idle (quick tests)")
    ap.add_argument("--threads", type=int, default=0)
    ap.add_argument("--style", default="detailed", help="a style from studio.json")
    ap.add_argument("--scale", type=int, help="render scale; default from studio.json for the category")
    ap.add_argument("--no-wreck", action="store_true")
    ap.add_argument("--no-log", action="store_true", help="don't append to art/timings.jsonl")
    ap.add_argument("--models", action="append", default=[],
                    help="a pack's model folder (holding <category>/<id>.py), searched before art/models")
    ap.add_argument("--pack", help="a private pack by name: adds settings-private/art/<pack>/models to --models")
    args = ap.parse_args()
    if args.pack:
        args.models.insert(0, str(pack_models(args.pack)))

    mod, category, eid, path = load(args.model, model_roots(args.models))
    if category not in KINDS:
        category = getattr(mod, "CATEGORY", "vehicles")
    kind = KINDS[category]
    scale = args.scale or st.STUDIO["render_scale_by_category"].get(category, st.STUDIO["render_scale"])
    st.RENDER_SCALE = scale
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    has_wreck = kind == "vehicle" and not args.no_wreck or hasattr(mod, "wreck")
    variants = plan(mod, kind, args.facings, has_wreck)
    wanted = set(args.jobs.split(",")) if args.jobs else None
    variants = [(v, [j for j in js if not wanted or f"{j.part}/{j.anim}" in wanted]) for v, js in variants]
    variants = [(v, js) for v, js in variants if js]

    import bpy
    shadow_samples = st.STUDIO["shadow_samples"]
    meta = {"id": eid, "category": category, "kind": kind, "style": args.style, "render_scale": scale,
            "jobs": [], "images": 0, "model": str(path), "external": external(path)}
    if kind == "building":
        meta["footprint"] = list(mod.FOOTPRINT)
        meta["decal"] = bool(getattr(mod, "DECAL", False))
    seen = {}
    frame_size = None
    t0 = time.time()
    for v, jobs in variants:
        ctx = build_scene(mod, kind, eid, args.style, args.samples, args.threads, v)
        if frame_size is None:
            b = ctx["intact_bounds"]
            if getattr(mod, "JOINS", False):
                # The first variant is the lone post (joins 0); size the canvas for arms reaching every tile edge.
                T = st.STUDIO["metres_per_tile"]
                fw, fh = mod.FOOTPRINT[0] * T, mod.FOOTPRINT[1] * T
                b = dict(b, min=[min(b["min"][0], 0), min(b["min"][1], -fh)],
                         max=[max(b["max"][0], fw), max(b["max"][1], 0)])
            area = canvas(kind, b, kind == "building")
            size, origin = st.camera(ctx["scene"], *area)
            frame_size = area
            meta.update(render_size=list(size), origin_px=list(origin), bounds_m=b, team=getattr(mod, "TEAM", True))
            if "hull" in ctx["groups"]:
                # Length for the scale check: the hull alone, so a gun reaching past the nose doesn't count.
                hb = st.bounds_of(ctx["groups"]["hull"])
                meta["length_m"] = round(hb["max"][1] - hb["min"][1], 3)
            if kind == "building":
                meta["contact_m"] = st.contact(ctx["root"])
        else:
            st.camera(ctx["scene"], *frame_size)
        ground = st.shadow_ground(ctx["scene"])
        scene = ctx["scene"]
        for job in jobs:
            key = (job.part, job.anim)
            if key not in seen:
                seen[key] = job
                meta["jobs"].append(job)
            visible, holdout, casters = job.select(ctx)
            if not job.shadow:
                casters = None
            frames = range(job.frames) if job.frame_of_variant is None else [job.frame_of_variant]
            facing_list = [args.only] if args.only is not None and job.facings > 1 else range(job.facings)
            if job.overlay:
                # Render border round the moving parts over the whole loop: an overlay costs a fraction of a frame.
                box = None
                for n in frames:
                    job.pose(ctx, n)
                    bpy.context.view_layer.update()
                    b = st.screen_box(scene, visible)
                    box = b if box is None else (min(box[0], b[0]), min(box[1], b[1]), max(box[2], b[2]),
                                                 max(box[3], b[3]))
                w, h = scene.render.resolution_x, scene.render.resolution_y
                scene.render.use_border = scene.render.use_crop_to_border = True
                scene.render.border_min_x, scene.render.border_max_x = box[0] / w, box[2] / w
                scene.render.border_min_y, scene.render.border_max_y = 1 - box[3] / h, 1 - box[1] / h
                seen[key].crop = box
            for n in frames:
                job.pose(ctx, n)
                for i in facing_list:
                    ctx["root"].rotation_euler[2] = 0
                    job.turn(ctx, -2 * math.pi * i / job.facings - job.yaw)
                    bpy.context.view_layer.update()
                    meta["images"] += shoot(ctx, ground, out / f"{job.part}-{job.anim}-f{i:02d}-{n:02d}", visible,
                                            holdout, casters, shadow_samples)
            scene.render.use_border = scene.render.use_crop_to_border = False
        if v == variants[0][0] and jobs:
            jobs[0].pose(ctx, 0)
            ctx["root"].rotation_euler[2] = 0
            leak_test(ctx, ground, out / "check-leak", ctx["all"])
    meta["jobs"] = [j.meta() for j in meta["jobs"]]
    meta["seconds"] = round(time.time() - t0, 1)
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    print(f"{eid}: render {meta['render_size'][0]}x{meta['render_size'][1]}, {meta['images']} images, "
          f"{meta['seconds']} s")
    if not args.no_log and not meta["external"]:
        line = {"id": eid, "category": category, "style": args.style, "render_scale": scale,
                "samples": args.samples, "images": meta["images"], "render_size": meta["render_size"],
                "seconds": meta["seconds"], "cores": args.threads or os.cpu_count(),
                "partial": bool(args.only is not None or args.facings or args.jobs),
                "date": datetime.date.today().isoformat()}
        with open(ART / "timings.jsonl", "a") as f:
            f.write(json.dumps(line) + "\n")


if __name__ == "__main__":
    main()
