"""Starport: a large round landing pad on a concrete apron, four beacon towers around it, a control building with a
raised glass cab in the north-west, fuel tanks in the north-east and a row of cargo containers down the west side.
The cargo hall's door faces south. Three tiles by three, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 3)
T = st.STUDIO["metres_per_tile"]
INSET = 0.5
# Small parts the classic style leaves out.
DETAIL = {"vent", "rail", "bolt", "lamp_cage", "aerial", "pipe", "light"}
# Beacon lamps blink in an idle overlay; the cargo door opens in a door frame.
IDLE_PARTS = {"lamp"}
DOORS = {"cargo_door"}


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass, load = st.armour(), st.glass(), st.cargo()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    lamp = st.plain("lamp", (1.0, 0.55, 0.12), emission=3.0)
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W - 2 * INSET, H - 2 * INSET, 0.35), (W / 2, -H / 2, 0.175), mat=conc, parent=root, bevel=0.1,
             name="apron")

    # Landing pad: a raised steel disc with a team-coloured rim, a hazard ring and a cross in the middle.
    px, py, r = W * 0.56, -H * 0.6, 9.6
    st.cylinder(r + 0.5, 0.6, (px, py, 0.55), mat=paint, parent=root, verts=64, bevel=0.05, name="pad_rim")
    st.cylinder(r - 0.3, 0.6, (px, py, 0.7), mat=steel, parent=root, verts=64, bevel=0.05, name="pad")
    for k in range(24):
        a = 2 * math.pi * k / 24
        st.block((1.4, 0.5, 0.08), (px + (r - 1.6) * math.cos(a), py + (r - 1.6) * math.sin(a), 1.02),
                 rot=(0, 0, a + math.pi / 2), mat=stripe if k % 2 else dark, parent=root, name="pad_ring")
    for rot in (math.pi / 4, -math.pi / 4):
        st.block((7.0, 1.1, 0.08), (px, py, 1.03), rot=(0, 0, rot), mat=stripe, parent=root, name="pad_cross")
    st.cylinder(1.2, 0.12, (px, py, 1.05), mat=dark, parent=root, verts=24, name="pad_centre")

    # Four beacon towers on the diagonals of the pad: steel shaft, team band, amber lamp under a cage.
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        bx, by = px + (r + 1.6) * math.cos(a), py + (r + 1.6) * math.sin(a)
        bx = min(max(bx, INSET + 1.2), W - INSET - 1.2)
        by = min(max(by, -H + INSET + 1.2), -INSET - 1.2)
        st.block((1.8, 1.8, 0.8), (bx, by, 0.75), mat=armour, parent=root, bevel=0.06, name="beacon_base")
        st.block((1.0, 1.0, 6.0), (bx, by, 4.1), mat=steel, parent=root, name="beacon")
        st.block((1.15, 1.15, 0.9), (bx, by, 5.4), mat=paint, parent=root, name="beacon_band")
        st.cylinder(0.6, 0.9, (bx, by, 7.55), mat=lamp, parent=root, verts=12, name="lamp")
        st.cylinder(0.7, 0.3, (bx, by, 8.15), mat=dark, parent=root, verts=12, name="lamp_cage")

    # Control building in the north-west: a two-storey block, a raised glass cab on a stem, aerials on the roof.
    cx, cy, cw, cl, ch = T * 0.55, -T * 0.42, T * 0.9, T * 0.66, 5.4
    st.block((cw, cl, ch), (cx, cy, ch / 2 + 0.35), mat=conc, parent=root, bevel=0.1, name="control")
    st.block((cw + 0.2, cl + 0.2, 0.5), (cx, cy, ch + 0.5), mat=armour, parent=root, bevel=0.05, name="control_roof")
    st.block((cw - 1.4, 1.0, 0.25), (cx, cy + cl / 2 - 1.4, ch + 0.85), mat=paint, parent=root, name="roof_stripe")
    for z in (1.8, 4.0):
        st.block((cw - 1.6, 0.12, 0.9), (cx, cy - cl / 2 - 0.05, z), mat=glass, parent=root, name="window")
    tx, ty = cx + cw / 2 - 2.4, cy - cl / 2 + 2.4
    st.block((2.4, 2.4, 6.0), (tx, ty, ch + 3.5), mat=conc, parent=root, bevel=0.06, name="cab_stem")
    st.cylinder(2.6, 2.2, (tx, ty, ch + 7.4), mat=glass, parent=root, verts=8, name="cab")
    st.cylinder(2.9, 0.5, (tx, ty, ch + 8.7), mat=paint, parent=root, verts=8, name="cab_roof")
    st.cylinder(2.8, 0.4, (tx, ty, ch + 6.1), mat=armour, parent=root, verts=8, name="cab_floor")
    for dx in (-2.4, -0.8):
        st.cylinder(0.25, 3.0, (cx + dx - cw / 4, cy + cl / 4, ch + 2.2), mat=dark, parent=root, verts=8,
                    name="aerial")
    for k in range(3):
        st.cylinder(0.4, 0.6, (cx - cw / 3 + k * 1.6, cy - cl / 6, ch + 1.0), mat=steel, parent=root, verts=12,
                    name="vent")

    # Fuel tanks in the north-east, on a bund, piped towards the pad.
    fx = W - T * 0.55
    st.block((T * 0.9, T * 0.62, 0.8), (fx, -T * 0.4, 0.75), mat=conc, parent=root, bevel=0.08, name="bund")
    for dx in (-2.4, 2.4):
        st.cylinder(2.0, 4.6, (fx + dx, -T * 0.4, 3.4), mat=steel, parent=root, verts=32, name="fuel_tank")
        st.cylinder(2.03, 0.6, (fx + dx, -T * 0.4, 4.4), mat=paint, parent=root, verts=32, name="tank_band")
        st.sphere(2.0, (fx + dx, -T * 0.4, 5.7), mat=steel, parent=root, scale=(1, 1, 0.3), name="tank_top")
    st.cylinder(0.3, T * 0.9, (fx, -T * 0.4 - 2.6, 1.4), rot=(0, math.pi / 2, 0), mat=dark, parent=root, verts=10,
                name="pipe")

    # Cargo hall and containers down the west side, south of the control building; the hall door faces south.
    hx, hy, hw, hl, hh = T * 0.36, -H + INSET + 4.2, T * 0.6, 7.4, 4.6
    st.block((hw, hl, hh), (hx, hy, hh / 2 + 0.35), mat=armour, parent=root, bevel=0.1, name="cargo_hall")
    st.block((hw + 0.2, hl + 0.2, 0.4), (hx, hy, hh + 0.5), mat=dark, parent=root, bevel=0.04, name="hall_roof")
    st.block((hw - 2.0, 0.25, 3.4), (hx, hy - hl / 2 - 0.1, 2.1), mat=dark, parent=root, name="cargo_door")
    for k in range(5):
        st.block((hw - 2.2, 0.3, 0.12), (hx, hy - hl / 2 - 0.25, 0.8 + k * 0.65), mat=steel, parent=root,
                 name="rail")
    for k, y in enumerate((-T * 0.95, -T * 1.25, -T * 1.55)):
        st.block((2.6, 2.4, 2.6), (T * 0.24, y, 1.65), mat=load if k == 1 else armour, parent=root, bevel=0.05,
                 name="container")
        if k != 1:
            st.block((2.6, 2.4, 2.6), (T * 0.24, y, 4.25), mat=steel, parent=root, bevel=0.05, name="container")


def damage(rng, root):
    """Damaged frame (wave 3): a beacon leans, the cab glass goes dark and a container falls. Not rendered yet."""
    leaned = False
    for o in root.children_recursive:
        base = o.name.split(".")[0]
        if base in ("beacon", "beacon_band", "lamp", "lamp_cage") and not leaned:
            o.rotation_euler[1] = rng.uniform(0.15, 0.3)
        elif base == "lamp_cage":
            leaned = True
        elif base == "container" and o.location.z > 3 and rng.random() < 0.5:
            o.location.z = 1.65
            o.location.x += 2.8
            o.rotation_euler[2] = rng.uniform(-0.5, 0.5)
