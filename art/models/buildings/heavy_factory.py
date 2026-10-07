"""Heavy factory: a large assembly hall with a wide door in the middle of the south side, over the exit tile (one
row below the middle column), a gantry crane running on rails along the roof, a lower parts store to the east and
two stacks. Three tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5                     # ground contact stays this far inside the footprint
HALL_H = 8.5
# The door sits under the exit tile: the middle column for a 3-wide footprint.
DOOR_X, DOOR_W, DOOR_H = W / 2, 10.5, 6.5
HALL = (W * 0.44, -H * 0.42, W * 0.7, H * 0.66)     # centre x, centre y, width, depth
# Small parts the classic style leaves out.
DETAIL = {"door_lamp", "door_slat", "window", "rivet_band", "bollard", "stripe", "hook", "cable", "ladder",
          "crate", "rail_tie"}
# Parts the studio's door frames move or hide; all built by door() under one group per side.
DOORS = {"door", "door_slat", "door_frame", "door_lamp", "door_lintel"}
# Parts that move in the idle overlay: the crane trolley runs along its bridge.
IDLE_PARTS = {"crane_trolley", "hook", "cable"}


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass, load = st.armour(), st.glass(), st.cargo()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Assembly hall: concrete base course, armour cladding above, a flat roof with a team band round the parapet.
    hx, hy, hw, hl = HALL
    st.block((hw, hl, 2.0), (hx, hy, 1.3), mat=conc, parent=root, bevel=0.1, name="plinth")
    st.block((hw - 0.3, hl - 0.3, HALL_H - 2.0), (hx, hy, 2.3 + (HALL_H - 2.0) / 2), mat=armour, parent=root,
             bevel=0.08, name="hall")
    for k in range(9):
        x = hx - hw / 2 + 0.6 + k * (hw - 1.2) / 8
        if abs(x - DOOR_X) < DOOR_W / 2 + 0.8:
            continue
        st.block((0.5, 0.4, HALL_H - 2.0), (x, hy - hl / 2 + 0.05, 2.3 + (HALL_H - 2.0) / 2), mat=steel,
                 parent=root, name="pilaster")
    for sy in (-1, 1):
        st.block((hw + 0.3, 0.7, 0.9), (hx, hy + sy * (hl / 2 - 0.2), HALL_H + 0.75), mat=paint, parent=root,
                 name="parapet")
    for sx in (-1, 1):
        st.block((0.7, hl + 0.3, 0.9), (hx + sx * (hw / 2 - 0.2), hy, HALL_H + 0.75), mat=paint, parent=root,
                 name="parapet")
    st.block((hw - 1.2, hl - 1.2, 0.2), (hx, hy, HALL_H + 0.4), mat=dark, parent=root, name="roof")
    for k in range(2):
        st.block((hw - 6.0, 0.9, 0.5), (hx - 1.5, hy + hl / 2 - 3.6 - k * 4.2, HALL_H + 0.7), mat=glass,
                 parent=root, name="skylight")
    for k in range(3):
        st.block((1.8, 1.8, 0.9), (hx + hw / 2 - 2.5 - k * 3.0, hy - hl / 2 + 2.6, HALL_H + 0.95), mat=steel,
                 parent=root, bevel=0.06, name="roof_unit")
    for k in range(8):
        x = hx - hw / 2 + 2.0 + k * (hw - 4.0) / 7
        if abs(x - DOOR_X) > DOOR_W / 2 + 1.6:
            st.block((1.8, 0.12, 1.0), (x, hy - hl / 2 - 0.05, HALL_H - 1.5), mat=glass, parent=root,
                     name="window")

    # Gantry crane: two rails along the hall's length on raised posts, a bridge across them and a trolley with a
    # hook. The bridge is the silhouette feature, so it is chunky and stands well above the roof.
    rail_z = HALL_H + 3.2
    for sy in (-1, 1):
        y = hy + sy * (hl / 2 - 1.0)
        st.block((hw - 0.6, 0.55, 0.6), (hx, y, rail_z), mat=dark, parent=root, name="crane_rail")
        for k in range(4):
            st.block((0.5, 0.5, rail_z - HALL_H), (hx - hw / 2 + 1.0 + k * (hw - 2.0) / 3, y,
                                                    HALL_H + (rail_z - HALL_H) / 2), mat=dark, parent=root,
                     name="crane_post")
    bx = hx + hw * 0.18
    st.block((1.4, hl - 0.6, 1.3), (bx, hy, rail_z + 0.95), mat=paint, parent=root, bevel=0.06, name="crane_bridge")
    for sy in (-1, 1):
        st.block((2.4, 1.2, 1.0), (bx, hy + sy * (hl / 2 - 1.0), rail_z + 0.6), mat=steel, parent=root,
                 bevel=0.06, name="crane_truck")
    ty = hy - hl * 0.12
    st.block((2.0, 2.0, 1.2), (bx, ty, rail_z + 2.0), mat=steel, parent=root, bevel=0.08, name="crane_trolley")
    st.block((0.3, 0.3, 1.6), (bx, ty, rail_z + 0.6), mat=dark, parent=root, name="cable")
    st.block((0.9, 0.4, 0.6), (bx, ty, rail_z - 0.4), mat=stripe, parent=root, name="hook")
    st.block((0.4, 0.9, 1.2), (bx + 1.1, hy + hl / 2 - 1.0, rail_z + 1.0), mat=steel, parent=root, name="ladder")

    # Stacks at the west end of the hall.
    for k, y in enumerate((hy + hl / 4, hy - hl / 8)):
        x = hx - hw / 2 + 2.0
        st.cylinder(0.75, 6.5, (x, y, HALL_H + 3.0), mat=steel, parent=root, verts=20, name="stack")
        st.cylinder(0.8, 0.6, (x, y, HALL_H + 5.0), mat=paint, parent=root, verts=20, name="stack_band")
        st.cylinder(0.6, 0.2, (x, y, HALL_H + 6.25), mat=dark, parent=root, verts=20, caps=False, name="stack_top")

    # Parts store on the east side: a lower shed with crates stacked in its open front.
    sx0 = hx + hw / 2
    sw = W - INSET - sx0 - 0.2
    sl = hl * 0.62
    sy0 = hy + hl / 2 - sl / 2
    st.block((sw, sl, 4.5), (sx0 + sw / 2, sy0, 2.55), mat=conc, parent=root, bevel=0.08, name="store")
    st.wedge((sw + 0.4, sl + 0.4, 1.2), (sx0 + sw / 2, sy0, 5.4), slope_front=0.7, rot=(0, 0, math.pi),
             mat=armour, parent=root, name="store_roof")
    st.block((sw + 0.5, 0.6, 0.5), (sx0 + sw / 2, sy0 - sl / 2 - 0.1, 4.95), mat=paint, parent=root,
             name="store_band")
    st.block((sw - 1.6, 0.2, 3.0), (sx0 + sw / 2, sy0 - sl / 2 - 0.05, 1.8), mat=dark, parent=root,
             name="store_door")
    for k in range(3):
        st.block((1.6, 1.6, 1.4), (sx0 + 1.4 + (k % 2) * 1.8, sy0 - sl / 2 - 1.2 - (k // 2) * 1.9, 1.0), mat=load,
                 rot=(0, 0, 0.15 * k), parent=root, bevel=0.05, name="crate")
    st.block((1.4, 1.4, 1.2), (sx0 + 2.3, sy0 - sl / 2 - 1.6, 2.35), mat=armour, parent=root, bevel=0.05,
             name="crate")

    # Apron in front of the door, with hazard stripes and bollards.
    ay0 = hy - hl / 2
    apron_l = (H - INSET) + ay0
    st.block((DOOR_W + 3.0, apron_l, 0.08), (DOOR_X, ay0 - apron_l / 2, 0.33), mat=dark, parent=root, bevel=0.02,
             name="apron")
    for sx in (-1, 1):
        st.block((0.6, apron_l * 0.9, 0.04), (DOOR_X + sx * (DOOR_W / 2 + 0.9), ay0 - apron_l / 2, 0.38),
                 mat=stripe, parent=root, bevel=0, name="stripe")
        st.cylinder(0.35, 1.1, (DOOR_X + sx * (DOOR_W / 2 + 1.9), ay0 - 0.9, 0.85), mat=stripe, parent=root,
                    verts=12, name="bollard")

    door(root, "south")


def door(root, side="south"):
    """The wide sliding door, its frame, lintel and lamps, built in one group on the hall wall for `side`. Only the
    south door is built today; east and west sit on the hall's end walls when a setting wants them (the east one
    would open through the parts store, so a setting that wants it moves the store)."""
    hx, hy, hw, hl = HALL
    steel, dark, armour = st.steel("door_steel"), st.dark_steel("door_dark"), st.armour("door_paint")
    lamp = st.plain("lamp", (1.0, 0.55, 0.1), emission=2.0)
    place = {"south": ((DOOR_X, hy - hl / 2, 0.3), 0.0),
             "east": ((hx + hw / 2, hy, 0.3), math.pi / 2),
             "west": ((hx - hw / 2, hy, 0.3), -math.pi / 2)}[side]
    g = st.group(f"door_{side}", place[0], parent=root)
    g.rotation_euler[2] = place[1]
    # Local frame: the door faces -y, centred on x = 0, standing on z = 0.
    st.block((DOOR_W + 1.4, 0.6, DOOR_H + 1.0), (0, -0.25, (DOOR_H + 1.0) / 2), mat=dark, parent=g,
             name="door_frame")
    for sx in (-1, 1):
        # Two leaves that slide apart, each with a diagonal brace.
        st.block((DOOR_W / 2 - 0.05, 0.3, DOOR_H), (sx * DOOR_W / 4, -0.5, DOOR_H / 2), mat=steel, parent=g,
                 bevel=0.03, name="door")
        st.block((0.35, 0.12, math.hypot(DOOR_W / 2, DOOR_H) - 0.8), (sx * DOOR_W / 4, -0.7, DOOR_H / 2),
                 rot=(0, sx * math.atan2(DOOR_W / 2, DOOR_H), 0), mat=dark, parent=g, name="door_slat")
    st.block((DOOR_W + 2.0, 1.2, 1.2), (0, -0.8, DOOR_H + 0.8), mat=armour, parent=g, bevel=0.06, name="door_lintel")
    for sx in (-1, 1):
        st.block((0.5, 0.35, 0.35), (sx * (DOOR_W / 2 + 0.5), -1.5, DOOR_H + 0.8), mat=lamp, parent=g,
                 name="door_lamp")


def damage(rng, root):
    """Damaged frame: scorch marks on the roof, a skylight blown out and the crane bridge knocked askew. Not
    rendered yet; the studio switches it on with building frames."""
    scorch = st.plain("scorch", (0.03, 0.025, 0.02), roughness=0.95)
    hx, hy, hw, hl = HALL
    for _ in range(5):
        x = hx + rng.uniform(-hw / 2 + 2, hw / 2 - 2)
        y = hy + rng.uniform(-hl / 2 + 2, hl / 2 - 2)
        s = rng.uniform(2.0, 3.5)
        st.block((s, s * rng.uniform(0.6, 1.0), 0.1), (x, y, HALL_H + 0.55), rot=(0, 0, rng.uniform(0, math.pi)),
                 mat=scorch, parent=root, bevel=0, name="scorch")
    for o in root.children_recursive:
        if o.name.startswith("skylight") and rng.random() < 0.4:
            o.hide_render = True
        if o.name.startswith("crane_bridge"):
            o.rotation_euler[2] += rng.uniform(-0.12, 0.12)
