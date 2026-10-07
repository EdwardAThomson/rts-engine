"""Studio test building, 2x2: exercises every building frame the studio renders. A block with a team roof band,
a turning dish on a mast (IDLE_PARTS, default idle pose) and a roller door facing south (DOORS, default door
pose). Not a game asset."""
import rts_studio as st

CATEGORY = "buildings"
FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
IDLE_PARTS = {"dish"}
IDLE_FRAMES = 8
DOORS = {"door"}
DOOR_FRAMES = 3
DETAIL = {"vent"}


def build(root):
    conc, armour, steel, team = st.concrete(), st.armour(), st.steel(), st.team_paint()
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W - 1.2, H - 1.2, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08, name="pad")
    st.block((W * 0.6, H * 0.5, 5.0), (W * 0.45, -H * 0.45, 2.8), mat=conc, parent=root, name="hall")
    st.block((W * 0.6 + 0.1, H * 0.5 + 0.1, 0.6), (W * 0.45, -H * 0.45, 5.1), mat=team, parent=root, name="band")
    # Faces must never share a plane (Cycles renders coplanar faces black): the roof sits 2 cm proud of the band.
    st.block((W * 0.6 - 1.6, H * 0.5 - 1.6, 0.3), (W * 0.45, -H * 0.45, 5.57), mat=conc, parent=root, name="roof")
    st.block((4.0, 0.3, 3.4), (W * 0.45, -H * 0.7 - 0.1, 2.0), mat=armour, parent=root, name="door")
    for k in range(3):
        st.cylinder(0.4, 0.6, (W * 0.3 + k * 2.2, -H * 0.35, 6.0), mat=steel, parent=root, verts=12, name="vent")
    st.cylinder(0.35, 4.0, (W * 0.82, -H * 0.3, 2.0), mat=steel, parent=root, verts=12, name="mast")
    dish = st.group("dish", (W * 0.82, -H * 0.3, 4.2), parent=root)
    st.cylinder(2.0, 0.3, (0, 0.6, 1.2), rot=(1.1, 0, 0), mat=steel, parent=dish, verts=24, name="dish_face")
    st.block((0.3, 1.0, 0.3), (0, 0.2, 0.4), mat=steel, parent=dish, name="dish_arm")
