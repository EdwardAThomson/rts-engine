"""Light factory: a workshop hall with a ribbed roller door on the south side, over the exit tile (one row below
the east column), an office annex to the west, a sawtooth roof with glazed bays, and a team-coloured band along the eaves. Two tiles by two, built
in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5                     # ground contact stays this far inside the footprint
HALL_H = 7.0
# The roller door sits under the exit tile; for a 2-wide footprint that is the east column, so the door is pulled
# east of centre (keeping it on the hall's south wall).
DOOR_X, DOOR_W, DOOR_H = W * 0.62, 7.0, 5.2
# Small parts the classic style leaves out.
DETAIL = {"vent_cap", "door_lamp", "door_slat", "window", "downpipe", "bollard", "stripe", "rail", "aircon"}
# Parts the studio's door frames move or hide; all built by door() under one group per side.
DOORS = {"door", "door_slat", "door_frame", "door_lamp", "door_box"}
# Parts that move in the idle overlay.
IDLE_PARTS = {"vent_fan"}

HALL = (W * 0.62, -H * 0.42, W - 2 * INSET - 6.0, H * 0.68)     # centre x, centre y, width, depth


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Workshop hall: concrete walls under a sawtooth roof of three bays, each with a glazed face to the south, and
    # a team band along the eaves.
    hx, hy, hw, hl = HALL
    st.block((hw, hl, HALL_H), (hx, hy, HALL_H / 2 + 0.3), mat=conc, parent=root, bevel=0.1, name="hall")
    for sy in (-1, 1):
        st.block((hw + 0.5, 0.6, 0.6), (hx, hy + sy * (hl / 2 + 0.05), HALL_H + 0.3), mat=paint, parent=root,
                 name="eave")
    for sx in (-1, 1):  # 4 cm slimmer than the north and south eaves, so the corners have no shared face planes
        st.block((0.56, hl + 0.5, 0.56), (hx + sx * (hw / 2 + 0.05), hy, HALL_H + 0.3), mat=paint, parent=root,
                 name="eave")
    bay = hl / 3
    for k in range(3):
        y = hy - hl / 2 + bay * (k + 0.5)
        st.wedge((hw - 0.6, bay, 2.4), (hx, y, HALL_H + 1.5), slope_front=1.0, mat=armour, parent=root,
                 name="roof")
        st.block((hw - 1.4, 0.12, 1.3), (hx, y - bay / 2 - 0.02, HALL_H + 1.85), mat=glass, parent=root,
                 name="roof_glass")
        st.block((hw - 0.4, 0.4, 0.4), (hx, y - bay / 2 + 0.1, HALL_H + 2.75), mat=dark, parent=root, name="rail")
    # Roof fans on the annex side of the hall (they turn in the idle overlay).
    for k in range(2):
        vx, vy = hx - hw / 2 + 2.0, hy + hl / 2 - 2.2 - k * 3.4
        st.block((2.2, 2.2, 1.1), (vx, vy, HALL_H + 1.3), mat=steel, parent=root, bevel=0.08, name="vent")
        st.cylinder(0.85, 0.2, (vx, vy, HALL_H + 1.9), mat=dark, parent=root, verts=20, name="vent_fan")
        st.block((2.4, 0.3, 0.3), (vx, vy, HALL_H + 2.15), mat=steel, parent=root, name="vent_cap")
    # Downpipes at the south corners.
    for sx in (-1, 1):
        st.block((0.3, 0.26, HALL_H), (hx + sx * (hw / 2 - 0.4), hy - hl / 2 - 0.2, HALL_H / 2 + 0.3),
                 mat=dark, parent=root, bevel=0.02, name="downpipe")

    # Office annex on the west side, lower, with a strip of windows facing south.
    ax, aw = INSET + 3.0, 5.4
    al = hl * 0.7
    ay = hy - hl / 2 + al / 2
    st.block((aw, al, 4.2), (ax, ay, 2.4), mat=conc, parent=root, bevel=0.08, name="annex")
    st.block((aw + 0.3, al + 0.3, 0.4), (ax, ay, 4.7), mat=armour, parent=root, bevel=0.04, name="annex_roof")
    st.block((aw + 0.4, 1.0, 0.45), (ax, ay - al / 2 + 0.3, 4.95), mat=paint, parent=root, name="annex_band")
    st.block((1.6, 1.6, 0.9), (ax, ay + al / 4, 5.3), mat=steel, parent=root, bevel=0.06, name="aircon")
    for k in range(2):
        st.block((1.6, 0.12, 1.0), (ax - 1.2 + k * 2.4, ay - al / 2 - 0.04, 3.0), mat=glass, parent=root,
                 name="window")
    st.block((1.1, 0.12, 2.2), (ax + 1.6, ay - al / 2 - 0.04, 1.4), mat=armour, parent=root, name="annex_door")

    # Apron in front of the door, with hazard stripes and bollards either side.
    ay0 = hy - hl / 2
    apron_l = (H - INSET) + ay0
    st.block((DOOR_W + 2.0, apron_l, 0.08), (DOOR_X, ay0 - apron_l / 2, 0.33), mat=dark, parent=root, bevel=0.02,
             name="apron")
    for sx in (-1, 1):
        st.block((0.5, apron_l * 0.9, 0.04), (DOOR_X + sx * (DOOR_W / 2 + 0.6), ay0 - apron_l / 2, 0.38),
                 mat=stripe, parent=root, bevel=0, name="stripe")
        st.cylinder(0.3, 1.0, (DOOR_X + sx * (DOOR_W / 2 + 1.4), ay0 - 0.8, 0.8), mat=stripe, parent=root,
                    verts=12, name="bollard")

    door(root, "south")


def door(root, side="south"):
    """The roller door, its frame, motor box and lamp, built in one group on the hall wall for `side`. Only the
    south door is built today; east and west sit on the hall's end walls when a setting wants them."""
    hx, hy, hw, hl = HALL
    steel, dark, armour = st.steel("door_steel"), st.dark_steel("door_dark"), st.armour("door_paint")
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    place = {"south": ((DOOR_X, hy - hl / 2, 0.3), 0.0),
             "east": ((hx + hw / 2, hy, 0.3), math.pi / 2),
             "west": ((hx - hw / 2, hy, 0.3), -math.pi / 2)}[side]
    g = st.group(f"door_{side}", place[0], parent=root)
    g.rotation_euler[2] = place[1]
    # Local frame: the door faces -y, centred on x = 0, standing on z = 0.
    st.block((DOOR_W + 1.0, 0.5, DOOR_H + 0.8), (0, -0.2, (DOOR_H + 0.8) / 2), mat=dark, parent=g, name="door_frame")
    st.block((DOOR_W, 0.3, DOOR_H), (0, -0.4, DOOR_H / 2), mat=steel, parent=g, bevel=0.02, name="door")
    for k in range(7):
        st.block((DOOR_W - 0.1, 0.12, 0.25), (0, -0.58, 0.5 + k * (DOOR_H - 0.8) / 6), mat=dark, parent=g,
                 bevel=0.01, name="door_slat")
    st.block((DOOR_W + 0.6, 1.0, 0.9), (0, -0.7, DOOR_H + 0.6), mat=armour, parent=g, bevel=0.05, name="door_box")
    st.block((0.5, 0.35, 0.35), (DOOR_W / 2 + 0.2, -0.95, DOOR_H + 1.3), mat=lamp, parent=g, name="door_lamp")


def damage(rng, root):
    """Damaged frame: scorch marks on the roof, a fan cap knocked off and the eave dented. Not rendered yet; the
    studio switches it on with building frames."""
    scorch = st.plain("scorch", (0.03, 0.025, 0.02), roughness=0.95)
    hx, hy, hw, hl = HALL
    for _ in range(4):
        x = hx + rng.uniform(-hw / 2 + 1.5, hw / 2 - 1.5)
        y = hy + rng.uniform(-hl / 2 + 1.5, hl / 2 - 1.5)
        s = rng.uniform(1.5, 3.0)
        st.block((s, s * rng.uniform(0.6, 1.0), 0.1), (x, y, HALL_H + 2.0), rot=(0, 0, rng.uniform(0, math.pi)),
                 mat=scorch, parent=root, bevel=0, name="scorch")
    for o in root.children_recursive:
        if o.name.startswith("vent_cap") and rng.random() < 0.5:
            o.hide_render = True
        if o.name.startswith("rail") and rng.random() < 0.3:
            o.rotation_euler[1] += rng.uniform(-0.08, 0.08)
