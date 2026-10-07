"""Refinery: a processing block with two storage tanks, piping and a loading bay. One tile (10.67 m) square; the
harvester docks just south of the footprint, so the bay faces south."""
import math

import rts_studio as st

FOOTPRINT = (1, 1)
T = st.STUDIO["metres_per_tile"]


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    load, glass = st.cargo(), st.glass()
    c = (T / 2, -T / 2)  # centre of the footprint
    st.block((T - 0.6, T - 0.6, 0.3), (c[0], c[1], 0.15), mat=conc, parent=root, bevel=0.08, name="pad")
    # Processing block, west half.
    st.block((4.6, 6.4, 3.6), (2.8, -4.3, 2.1), mat=steel, parent=root, name="block")
    st.block((4.7, 6.5, 0.5), (2.8, -4.3, 4.1), mat=paint, parent=root, name="roof_band")
    st.block((3.6, 4.8, 0.3), (2.8, -4.3, 4.45), mat=dark, parent=root, name="roof")
    for k in range(3):
        st.cylinder(0.35, 0.6, (1.9 + k * 0.9, -2.2, 4.8), mat=steel, parent=root, verts=12, name="vent")
    st.block((2.2, 0.1, 0.7), (2.8, -7.56, 2.6), mat=glass, parent=root, name="window")
    # Two storage tanks, east half.
    for k, y in enumerate((-2.6, -6.2)):
        st.cylinder(1.6, 3.4, (7.9, y, 2.0), mat=steel, parent=root, verts=32, name="tank")
        st.cylinder(1.62, 0.45, (7.9, y, 2.9), mat=paint, parent=root, verts=32, name="tank_band")
        st.sphere(1.6, (7.9, y, 3.7), mat=load, parent=root, scale=(1, 1, 0.35), name="tank_top")
    # Pipes from the block to the tanks.
    for y in (-2.6, -6.2):
        st.cylinder(0.22, 2.6, (6.1, y, 3.0), rot=(0, math.pi / 2, 0), mat=dark, parent=root, verts=12, name="pipe")
    # Loading bay on the south edge, with hazard stripes.
    st.block((6.0, 1.9, 0.35), (5.0, -9.4, 0.4), mat=dark, parent=root, name="bay")
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    for k in range(6):
        st.block((0.45, 1.7, 0.05), (2.5 + k * 1.0, -9.4, 0.6), rot=(0, 0, 0.5), mat=stripe, parent=root,
                 name="stripe")
    st.block((1.2, 2.4, 2.0), (8.9, -9.0, 1.3), mat=paint, parent=root, name="hopper_chute")
