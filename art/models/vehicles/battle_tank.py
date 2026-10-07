"""Battle tank: a medium tracked tank with one gun. About 6.3 m long, built facing north (+y)."""
import math

import rts_studio as st

TURRET_HEIGHT = 1.55


def build_hull(root):
    paint, steel, dark, rubber = st.team_paint(), st.steel(), st.dark_steel(), st.rubber()
    for side in (-1, 1):
        x = side * 1.45
        st.block((0.7, 6.0, 0.9), (x, 0, 0.45), mat=rubber, parent=root, name="track")
        for k in range(6):
            st.cylinder(0.36, 0.6, (x, -2.4 + k * 0.96, 0.42), rot=(0, math.pi / 2, 0), mat=dark, parent=root,
                        verts=16, name="wheel")
        st.block((0.62, 5.6, 0.1), (x, 0, 0.95), mat=steel, parent=root, name="fender")
    st.wedge((2.3, 5.8, 0.75), (0, 0, 1.0), slope_front=0.55, mat=paint, parent=root, name="hull")
    st.block((2.1, 1.0, 0.2), (0, -2.55, 1.35), mat=steel, parent=root, name="engine_deck")
    for k in range(4):
        st.block((1.8, 0.08, 0.05), (0, -2.85 + k * 0.2, 1.46), mat=dark, parent=root, name="grille")


def build_turret(ring):
    paint, steel, dark = st.team_paint(), st.steel(), st.dark_steel()
    st.cylinder(1.0, 0.25, (0, 0, -0.05), mat=dark, parent=ring, name="ring")
    st.wedge((2.1, 2.6, 0.7), (0, -0.2, 0.32), slope_front=0.45, mat=paint, parent=ring, name="turret")
    st.cylinder(0.12, 3.0, (0, 2.3, 0.35), rot=(math.pi / 2, 0, 0), mat=steel, parent=ring, verts=12, name="gun")
    st.cylinder(0.17, 0.5, (0, 3.6, 0.35), rot=(math.pi / 2, 0, 0), mat=dark, parent=ring, verts=12, name="muzzle")
    st.cylinder(0.22, 0.25, (0.45, -0.6, 0.75), mat=steel, parent=ring, name="hatch")
