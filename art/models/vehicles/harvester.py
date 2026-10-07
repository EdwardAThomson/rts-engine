"""Harvester: a large tracked collector with a cab at the front left, a hopper behind it and a cutting drum
across the front. About 8 m long, built facing north (+y)."""
import math

import rts_studio as st


def build_hull(root):
    paint, steel, dark, rubber = st.team_paint(), st.steel(), st.dark_steel(), st.rubber()
    load, glass = st.cargo(), st.glass()
    for side in (-1, 1):
        x = side * 1.75
        st.block((0.8, 7.0, 1.0), (x, -0.3, 0.5), mat=rubber, parent=root, name="track")
        st.block((0.72, 6.6, 0.1), (x, -0.3, 1.05), mat=steel, parent=root, name="fender")
    st.block((2.8, 6.8, 0.5), (0, -0.4, 1.0), mat=dark, parent=root, name="chassis")
    # Hopper: team-painted walls around a heap of cargo.
    st.block((3.2, 4.4, 1.5), (0, -1.5, 2.0), mat=paint, parent=root, name="hopper")
    st.sphere(1.0, (0, -1.5, 2.75), mat=load, parent=root, scale=(1.45, 2.0, 0.35), name="load")
    for k in range(3):
        st.block((3.3, 0.12, 0.12), (0, -3.0 + k * 1.5, 2.75), mat=steel, parent=root, name="rib")
    # Cab at the front left.
    st.wedge((1.5, 1.8, 1.4), (-0.85, 1.85, 1.95), slope_front=0.35, mat=paint, parent=root, name="cab")
    st.block((1.3, 0.1, 0.45), (-0.85, 2.76, 2.0), mat=glass, parent=root, name="window")
    st.block((1.4, 1.6, 0.6), (0.85, 1.9, 1.55), mat=steel, parent=root, name="engine")
    st.cylinder(0.12, 1.0, (1.3, 1.4, 2.3), mat=dark, parent=root, verts=10, name="exhaust")
    # Cutting drum across the front.
    st.cylinder(0.55, 3.8, (0, 3.65, 0.6), rot=(0, math.pi / 2, 0), mat=steel, parent=root, name="drum")
    for k in range(7):
        st.block((0.12, 0.25, 1.3), (-1.65 + k * 0.55, 3.65, 0.6), rot=(k * 0.5, 0, 0), mat=dark, parent=root,
                 name="tooth")
    for side in (-1, 1):
        st.block((0.2, 1.2, 0.3), (side * 1.85, 3.1, 0.9), mat=dark, parent=root, name="arm")
