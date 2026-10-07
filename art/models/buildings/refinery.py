"""Refinery: a processing hall, an unloading bay in the middle of the south side (the harvester docks one row
below the middle column) and two storage tanks. Three tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (3, 2)
T = st.STUDIO["metres_per_tile"]


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    load, glass = st.cargo(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W - 0.6, H - 0.6, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08, name="pad")

    # Processing hall across the west and middle columns, north half.
    hx, hy, hw, hl = W * 0.36, -H * 0.3, W * 0.62, H * 0.5
    st.block((hw, hl, 5.5), (hx, hy, 3.0), mat=steel, parent=root, name="hall")
    st.block((hw + 0.1, hl + 0.1, 0.6), (hx, hy, 5.8), mat=paint, parent=root, name="roof_band")
    st.block((hw - 1.6, hl - 1.6, 0.3), (hx, hy, 6.2), mat=dark, parent=root, name="roof")
    for k in range(5):
        st.cylinder(0.45, 0.8, (hx - hw / 2 + 2.5 + k * (hw - 5) / 4, hy + hl / 4, 6.7), mat=steel, parent=root,
                    verts=12, name="vent")
    st.cylinder(0.5, 6.0, (hx - hw / 2 + 1.6, hy + hl / 2 - 1.6, 8.5), mat=dark, parent=root, verts=16,
                name="stack")
    for k in range(3):
        st.block((2.4, 0.1, 0.9), (hx - hw / 3 + k * hw / 3, hy - hl / 2 - 0.05, 3.4), mat=glass, parent=root,
                 name="window")

    # Unloading bay in the middle column, open to the south edge, with a chute down into it.
    bx = W / 2
    st.block((T * 0.7, H * 0.42, 0.4), (bx, -H * 0.76, 0.45), mat=dark, parent=root, name="bay")
    for k in range(7):
        st.block((0.6, 0.12, 0.06), (bx, -H * 0.58 - k * 1.2, 0.68), rot=(0, 0, 0.6), mat=stripe, parent=root,
                 name="stripe")
    st.block((2.4, 3.0, 3.2), (bx, -H * 0.58, 2.2), mat=paint, parent=root, name="chute")
    st.block((3.6, 0.5, 0.5), (bx, -H * 0.55, 4.0), mat=steel, parent=root, name="gantry")

    # Two storage tanks in the east column, piped to the hall.
    tx = W - T / 2
    for y in (-H * 0.27, -H * 0.73):
        st.cylinder(3.2, 5.4, (tx, y, 3.0), mat=steel, parent=root, verts=40, name="tank")
        st.cylinder(3.23, 0.7, (tx, y, 4.6), mat=paint, parent=root, verts=40, name="tank_band")
        st.sphere(3.2, (tx, y, 5.7), mat=load, parent=root, scale=(1, 1, 0.3), name="tank_top")
        st.cylinder(0.35, tx - 3.2 - (hx + hw / 2), ((tx - 3.2 + hx + hw / 2) / 2, y, 4.2),
                    rot=(0, math.pi / 2, 0), mat=dark, parent=root, verts=12, name="pipe")
    st.block((1.2, H * 0.46, 1.0), (tx - 4.0, -H / 2, 0.8), mat=dark, parent=root, name="manifold")
