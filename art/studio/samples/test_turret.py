"""Studio test defence turret, 1x1: a concrete base and a gun head that turns (build_head, 32 facings). Not a
game asset."""
import math

import rts_studio as st

CATEGORY = "buildings"
FOOTPRINT = (1, 1)
HEAD_HEIGHT = 2.2
T = st.STUDIO["metres_per_tile"]


def build(root):
    conc = st.concrete()
    st.block((T - 1.2, T - 1.2, 0.4), (T / 2, -T / 2, 0.2), mat=conc, parent=root, bevel=0.1, name="pad")
    st.cylinder(3.2, 2.0, (T / 2, -T / 2, 1.2), mat=conc, parent=root, verts=8, name="base")


def build_head(ring):
    armour, steel, team = st.armour(), st.steel(), st.team_paint()
    st.cylinder(2.2, 1.2, (0, 0, 0.6), mat=armour, parent=ring, verts=8, name="head")
    st.cylinder(1.8, 0.2, (0, 0, 1.3), mat=team, parent=ring, verts=8, name="head_top")
    st.cylinder(0.3, 4.0, (0, 3.6, 0.7), rot=(math.pi / 2, 0, 0), mat=steel, parent=ring, verts=12, name="barrel")
