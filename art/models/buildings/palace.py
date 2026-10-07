"""Palace: a fortified command complex, one generic model for every faction. A stepped concrete bunker in three
tiers with round bastions at the corners, a command block on top carrying masts and a fixed dish, and an armoured
gate in the south face reached by a ramp between two buttresses. Three tiles by three, built in proportion to
FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 3)
T = st.STUDIO["metres_per_tile"]
INSET = 0.5
# Small parts the classic style leaves out.
DETAIL = {"vent", "slit", "aerial", "stay", "light", "rail"}
# Beacon lights blink in an idle overlay; the gate opens in a door frame.
IDLE_PARTS = {"light"}
DOORS = {"gate"}


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    light = st.plain("light", (1.0, 0.3, 0.1), emission=3.0)
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W - 2 * INSET, H - 2 * INSET, 0.35), (W / 2, -H / 2, 0.175), mat=conc, parent=root, bevel=0.1,
             name="apron")
    cx, cy = W / 2, -H / 2 + 0.8

    # Three tiers, each with a team-coloured band along its top edge and an armour cap.
    tiers = [(24.0, 22.0, 0.35, 3.6, conc), (18.0, 16.0, 3.95, 3.2, armour), (11.0, 9.5, 7.15, 3.4, conc)]
    for k, (w, l, z0, h, mat) in enumerate(tiers):
        st.block((w, l, h + 0.1), (cx, cy, z0 + h / 2 - 0.05), mat=mat, parent=root, bevel=0.12, name="tier")
        if k < 2:
            # a team-coloured parapet round the tier's top edge
            for sx, sy, size in ((0, 1, (w, 0.8, 0.6)), (0, -1, (w, 0.8, 0.6)), (1, 0, (0.8, l - 1.6, 0.6)),
                                 (-1, 0, (0.8, l - 1.6, 0.6))):
                st.block(size, (cx + sx * (w / 2 - 0.4), cy + sy * (l / 2 - 0.4), z0 + h + 0.3), mat=paint,
                         parent=root, bevel=0.04, name="parapet")
        # firing slits along the south face of the lower tiers
        if k < 2:
            for j in range(5 - k):
                st.block((1.4, 0.12, 0.4), (cx - w / 2 + 3.0 + j * (w - 6.0) / (4 - k), cy - l / 2 - 0.05, z0 + h / 2),
                         mat=dark, parent=root, name="slit")

    # Command block on top: armoured roof with a team panel, a glass band facing south.
    z = tiers[2][2] + tiers[2][3]
    st.block((11.8, 10.3, 0.5), (cx, cy, z + 0.25), mat=armour, parent=root, bevel=0.06, name="roof")
    st.block((6.0, 4.0, 0.25), (cx - 1.5, cy + 1.2, z + 0.6), mat=paint, parent=root, name="roof_panel")
    st.block((7.0, 0.12, 1.0), (cx, cy - 4.8, z - 1.4), mat=glass, parent=root, name="window")
    for j in range(3):
        st.cylinder(0.45, 0.6, (cx + 2.5 + (j % 2) * 1.6, cy + 2.6 - j * 1.4, z + 0.8), mat=steel, parent=root,
                    verts=12, name="vent")

    # Two masts and a fixed dish on the command block: the palace's silhouette at zoom 1.
    for mx, my, mh in ((cx - 3.8, cy + 3.0, 4.5), (cx + 3.6, cy - 2.6, 3.6)):
        st.cylinder(0.32, mh, (mx, my, z + 0.5 + mh / 2), mat=steel, parent=root, verts=10, name="mast")
        st.block((1.4, 0.3, 0.3), (mx, my, z + 0.5 + mh * 0.7), mat=steel, parent=root, name="aerial")
        st.cylinder(0.35, 0.4, (mx, my, z + 0.7 + mh), mat=light, parent=root, verts=10, name="light")
    st.cylinder(0.5, 1.4, (cx + 2.4, cy + 2.4, z + 1.2), mat=dark, parent=root, verts=12, name="dish_post")
    st.cylinder(2.0, 0.5, (cx + 2.4, cy + 2.2, z + 2.4), rot=(math.radians(-55), 0, 0), mat=steel, parent=root,
                verts=24, name="dish")

    # Round bastions on the corners of the lowest tier, team-capped, each with a gun slit.
    w0, l0 = tiers[0][0], tiers[0][1]
    for sx in (-1, 1):
        for sy in (-1, 1):
            bx, by = cx + sx * (w0 / 2 - 0.6), cy + sy * (l0 / 2 - 0.6)
            st.cylinder(2.7, 5.6, (bx, by, 3.15), mat=conc, parent=root, verts=24, bevel=0.06, name="bastion")
            st.cylinder(2.85, 0.7, (bx, by, 5.45), mat=paint, parent=root, verts=24, name="bastion_band")
            st.cylinder(2.25, 0.4, (bx, by, 6.15), mat=armour, parent=root, verts=24, name="bastion_cap")
            st.block((1.4, 0.4, 0.4), (bx + sx * 1.6, by + sy * 2.4, 4.2), rot=(0, 0, sx * sy * 0.6), mat=dark,
                     parent=root, name="slit")

    # The gate: buttresses either side of a ramp up to an armoured door in the middle of the south face.
    gy = cy - l0 / 2
    st.block((5.2, 0.6, 3.0), (cx, gy - 0.2, 1.85), mat=dark, parent=root, name="gate_recess")
    st.block((4.6, 0.4, 2.7), (cx, gy - 0.45, 1.7), mat=armour, parent=root, bevel=0.04, name="gate")
    st.block((4.6, 0.45, 0.3), (cx, gy - 0.5, 2.3), mat=paint, parent=root, name="gate_band")
    for sx in (-1, 1):
        st.wedge((2.2, 4.4, 4.2), (cx + sx * 3.8, gy - 2.0, 2.45), slope_front=0.6, rot=(0, 0, math.pi),
                 mat=conc, parent=root, name="buttress")
        st.block((2.3, 0.6, 0.5), (cx + sx * 3.8, gy - 4.0, 4.4), mat=paint, parent=root, name="buttress_band")
    st.block((5.2, 4.2, 0.6), (cx, gy - 2.2, 0.5), rot=(math.radians(-4), 0, 0), mat=conc, parent=root,
             name="ramp")
    for k in range(5):
        st.block((0.7, 0.4, 0.06), (cx - 2.0 + k * 1.0, gy - 4.1, 0.84), rot=(0, 0, 0.6), mat=stripe, parent=root,
                 name="rail")


def damage(rng, root):
    """Damaged frame (wave 3): a mast snaps, the dish tips and a bastion cap is knocked off. Not rendered yet."""
    for o in root.children_recursive:
        base = o.name.split(".")[0]
        if base == "mast" and rng.random() < 0.6:
            o.rotation_euler[0] = rng.uniform(0.6, 1.1)
        elif base == "dish":
            o.rotation_euler[0] += rng.uniform(0.3, 0.6)
        elif base == "bastion_cap" and rng.random() < 0.3:
            o.hide_render = True
