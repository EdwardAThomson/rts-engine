"""Heavy barracks: a fortified barracks. A thick-walled concrete bunker with sloped armour faces and firing slits,
a blast door on the south side over the exit tile (one row below the east column), a watchtower at the north-west
corner, and sandbag walls round a gun nest in the south-west. Two tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5                     # ground contact stays this far inside the footprint
BUNKER_H = 5.0
# The blast door sits under the exit tile; for a 2-wide footprint that is the east column.
DOOR_X, DOOR_W, DOOR_H = W * 0.7, 3.6, 3.2
BUNKER = (W * 0.6, -H * 0.42, W * 0.66, H * 0.6)        # centre x, centre y, width, depth
TOWER = (INSET + 2.8, -INSET - 2.8)
TOWER_H = 12.5
# Small parts the classic style leaves out.
DETAIL = {"slit", "door_lamp", "door_bolt", "rung", "antenna", "crate", "bag_top"}
# Parts the studio's door frames move or hide; all built by door() under one group per side.
DOORS = {"door", "door_frame", "door_bolt", "door_lamp", "door_hood"}
# Parts that move in the idle overlay: the tower's searchlight sweeps.
IDLE_PARTS = {"searchlight"}


def sandbags(root, mat, start, end, rows=3, name="sandbag"):
    """A low wall of sandbags from `start` to `end` (x, y), stacked `rows` high in a running bond."""
    (x0, y0), (x1, y1) = start, end
    length = math.hypot(x1 - x0, y1 - y0)
    angle = math.atan2(y1 - y0, x1 - x0)
    n = max(1, round(length / 1.0))
    for r in range(rows):
        shift = 0.5 if r % 2 else 0.0
        for k in range(n - (1 if r % 2 else 0)):
            t = (k + 0.5 + shift) / n
            st.block((1.0, 0.75, 0.42), (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, 0.5 + 0.4 * r),
                     rot=(0, 0, angle), mat=mat, parent=root, bevel=0.15, name=name if r < rows - 1 else "bag_top")


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    bag = st.concrete("sandbag", (0.45, 0.36, 0.22))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Bunker: a thick concrete core with battered (sloped) armour faces on the south and east, a stepped roof with
    # a team-colour band and an armoured roof hatch.
    bx, by, bw, bl = BUNKER
    st.block((bw, bl, BUNKER_H), (bx, by, BUNKER_H / 2 + 0.3), mat=conc, parent=root, bevel=0.2, name="bunker")
    st.block((bw + 0.4, bl + 0.4, 0.8), (bx, by, BUNKER_H + 0.7), mat=armour, parent=root, bevel=0.08,
             name="roof")
    st.block((bw + 0.5, 1.0, 0.5), (bx, by - bl / 2 + 0.1, BUNKER_H + 1.0), mat=paint, parent=root,
             name="roof_band")
    st.block((bw - 3.0, bl - 3.6, 1.1), (bx + 0.4, by + 0.6, BUNKER_H + 1.6), mat=dark, parent=root, bevel=0.15,
             name="roof_step")
    st.block((bw - 4.6, 1.0, 0.4), (bx + 0.4, by + 0.6 - (bl - 3.6) / 2 + 0.4, BUNKER_H + 2.3), mat=paint,
             parent=root, name="roof_band")
    st.block((2.0, 2.0, 0.6), (bx + bw / 4, by + bl / 4, BUNKER_H + 2.4), mat=armour, parent=root, bevel=0.06,
             name="hatch")
    for k in range(4):
        x = bx - bw / 2 + 2.2 + k * 2.6
        if abs(x - DOOR_X) > DOOR_W / 2 + 1.2:
            st.block((1.6, 0.3, 0.35), (x, by - bl / 2 - 0.05, 3.4), mat=dark, parent=root, name="slit")
    for k in range(3):
        st.block((0.3, 1.6, 0.35), (bx + bw / 2 + 0.05, by + bl / 2 - 2.0 - k * 2.6, 3.4), mat=dark, parent=root,
                 name="slit")
    # Sloped armour skirts against the south and east walls, either side of the door.
    for x0, x1 in ((bx - bw / 2, DOOR_X - DOOR_W / 2 - 0.6), (DOOR_X + DOOR_W / 2 + 0.6, bx + bw / 2)):
        st.block((x1 - x0, 1.0, 2.2), ((x0 + x1) / 2, by - bl / 2 - 0.35, 1.2), rot=(math.radians(-25), 0, 0),
                 mat=armour, parent=root, bevel=0.06, name="skirt")
    st.block((1.0, bl, 2.2), (bx + bw / 2 + 0.35, by, 1.2), rot=(0, math.radians(25), 0), mat=armour,
             parent=root, bevel=0.06, name="skirt")
    st.block((0.6, 0.6, 2.5), (bx - bw / 2 + 1.0, by + bl / 2 - 1.0, BUNKER_H + 2.4), mat=steel, parent=root,
             name="antenna")

    # Watchtower: four legs, a sandbagged platform, a roof and a searchlight.
    tx, ty = TOWER
    for sx in (-1, 1):
        for sy in (-1, 1):
            st.block((0.6, 0.6, TOWER_H - 2.6), (tx + sx * 1.5, ty + sy * 1.5, (TOWER_H - 2.6) / 2 + 0.3),
                     mat=steel, parent=root, name="tower_leg")
    st.block((3.4, 0.3, 0.3), (tx, ty - 1.5, 4.0), rot=(0, math.radians(60), 0), mat=steel, parent=root,
             name="tower_brace")
    st.block((4.4, 4.4, 0.4), (tx, ty, TOWER_H - 2.6), mat=dark, parent=root, name="tower_floor")
    for sy in (-1, 1):
        st.block((4.4, 0.6, 1.1), (tx, ty + sy * 1.9, TOWER_H - 1.9), mat=bag, parent=root, bevel=0.15,
                 name="tower_bags")
        st.block((0.6, 3.2, 1.1), (tx + sy * 1.9, ty, TOWER_H - 1.9), mat=bag, parent=root, bevel=0.15,
                 name="tower_bags")
    st.wedge((5.0, 5.0, 0.8), (tx, ty, TOWER_H + 0.3), slope_front=0.6, rot=(0, 0, math.pi), mat=armour,
             parent=root, name="tower_roof")
    st.block((5.1, 0.6, 0.4), (tx, ty - 2.3, TOWER_H + 0.05), mat=paint, parent=root, name="tower_band")
    for sx in (-1, 1):
        st.block((0.3, 0.3, 1.4), (tx + sx * 2.0, ty - 2.0, TOWER_H - 0.6), mat=steel, parent=root,
                 name="tower_post")
    st.cylinder(0.45, 0.8, (tx + 1.2, ty - 1.9, TOWER_H - 0.95), rot=(math.radians(70), 0, 0.5), mat=steel,
                parent=root, verts=14, name="searchlight")
    for k in range(4):
        st.block((0.9, 0.25, 0.25), (tx, ty + 1.75, 1.6 + k * 2.0), mat=steel, parent=root, name="rung")

    # Gun nest in the south-west: a ring of sandbags and a weapon on a post.
    gx, gy = INSET + 3.4, -H + INSET + 3.2
    sandbags(root, bag, (gx - 2.8, gy + 2.4), (gx + 2.8, gy + 2.4))
    sandbags(root, bag, (gx - 2.8, gy - 2.4), (gx + 2.8, gy - 2.4))
    sandbags(root, bag, (gx - 3.0, gy - 2.0), (gx - 3.0, gy + 2.0))
    sandbags(root, bag, (gx + 3.0, gy - 0.4), (gx + 3.0, gy + 2.0))
    st.cylinder(0.3, 1.4, (gx, gy, 1.0), mat=dark, parent=root, verts=10, name="gun_post")
    st.block((0.6, 2.6, 0.5), (gx + 0.3, gy - 0.6, 1.8), rot=(0, 0, 0.5), mat=dark, parent=root, name="gun")
    # A sandbag line across the front of the door, broken for the way out.
    sandbags(root, bag, (bx - bw / 2 + 0.5, -H + INSET + 1.2), (DOOR_X - DOOR_W / 2 - 1.5, -H + INSET + 1.2), rows=2)
    sandbags(root, bag, (DOOR_X + DOOR_W / 2 + 1.5, -H + INSET + 1.2), (W - INSET - 1.0, -H + INSET + 1.2), rows=2)
    for k in range(2):
        st.block((1.4, 1.4, 1.2), (W - INSET - 1.6, by + bl / 2 + 1.2, 0.9 + 1.2 * k), mat=st.cargo(),
                 rot=(0, 0, 0.2 * k), parent=root, bevel=0.05, name="crate")

    door(root, "south")


def door(root, side="south"):
    """The blast door: a heavy single leaf with locking bolts, set in a deep frame under an armoured hood, built in
    one group on the bunker wall for `side`. Only the south door is built today; east and west sit on the
    bunker's side walls."""
    bx, by, bw, bl = BUNKER
    steel, dark, armour = st.steel("door_steel"), st.dark_steel("door_dark"), st.armour("door_paint")
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    place = {"south": ((DOOR_X, by - bl / 2, 0.3), 0.0),
             "east": ((bx + bw / 2, by, 0.3), math.pi / 2),
             "west": ((bx - bw / 2, by, 0.3), -math.pi / 2)}[side]
    g = st.group(f"door_{side}", place[0], parent=root)
    g.rotation_euler[2] = place[1]
    # Local frame: the door faces -y, centred on x = 0, standing on z = 0.
    st.block((DOOR_W + 1.6, 1.6, DOOR_H + 1.0), (0, -0.6, (DOOR_H + 1.0) / 2), mat=dark, parent=g,
             name="door_frame")
    st.block((DOOR_W, 0.45, DOOR_H), (0, -1.45, DOOR_H / 2), mat=steel, parent=g, bevel=0.06, name="door")
    for k in range(3):
        st.block((DOOR_W - 0.6, 0.15, 0.3), (0, -1.72, 0.6 + k * (DOOR_H - 1.2) / 2), mat=dark, parent=g,
                 name="door_bolt")
    st.block((DOOR_W + 2.4, 2.2, 0.7), (0, -1.0, DOOR_H + 1.25), rot=(math.radians(-8), 0, 0), mat=armour,
             parent=g, bevel=0.08, name="door_hood")
    st.block((0.4, 0.3, 0.3), (DOOR_W / 2 + 0.5, -1.8, DOOR_H + 0.6), mat=lamp, parent=g, name="door_lamp")


def damage(rng, root):
    """Damaged frame: scorch marks on the roof, sandbags knocked down and the tower roof tilted. Not rendered yet;
    the studio switches it on with building frames."""
    scorch = st.plain("scorch", (0.03, 0.025, 0.02), roughness=0.95)
    bx, by, bw, bl = BUNKER
    for _ in range(4):
        s = rng.uniform(1.5, 3.0)
        st.block((s, s * rng.uniform(0.6, 1.0), 0.1),
                 (bx + rng.uniform(-bw / 3, bw / 3), by + rng.uniform(-bl / 3, bl / 3), BUNKER_H + 2.25),
                 rot=(0, 0, rng.uniform(0, math.pi)), mat=scorch, parent=root, bevel=0, name="scorch")
    for o in root.children_recursive:
        if o.name.startswith("bag_top") and rng.random() < 0.5:
            o.hide_render = True
        if o.name.startswith("tower_roof"):
            o.rotation_euler[0] += rng.uniform(-0.15, 0.15)
