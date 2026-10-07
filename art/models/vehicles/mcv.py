"""MCV: the mobile construction vehicle. An eight-wheeled heavy carrier (wheels, to set it apart from the tracked
harvester) with a cab at the front and a construction yard folded onto its bed: a stack of prefab wall panels, a
boxy site module with a team roof band, a crane turntable at the rear with its boom folded forward over the load
onto a rest above the cab, and four folded outrigger legs. About 8.5 m long, built facing north (+y); no turret.

Later hooks: `wreck(rng, root)` (studio batch J)."""
import math

import rts_studio as st

WHEEL_R, TYRE_W = 0.62, 0.5
WHEEL_X = 1.4
AXLES = (2.75, 1.45, -1.65, -2.95)
DECK_Z = 1.45            # top of the load bed
DETAIL = {"hub", "headlight", "mirror", "vent", "door", "step", "hook", "boom_rib", "lamp", "panel_edge",
          "exhaust_cap"}


def build_hull(root):
    team, body = st.team_paint(), st.armour()
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    glass, slab = st.glass(), st.concrete()
    for y in AXLES:
        for side in (-1, 1):
            st.cylinder(WHEEL_R, TYRE_W, (side * WHEEL_X, y, WHEEL_R), rot=(0, math.pi / 2, 0), mat=rubber,
                        verts=24, bevel=0.08, parent=root, name="tyre")
            st.cylinder(0.3, TYRE_W + 0.04, (side * WHEEL_X, y, WHEEL_R), rot=(0, math.pi / 2, 0), mat=steel,
                        verts=12, parent=root, name="hub")
    # team mudguards over each axle pair
    for y in ((AXLES[0] + AXLES[1]) / 2, (AXLES[2] + AXLES[3]) / 2):
        for side in (-1, 1):
            st.block((0.62, 2.85, 0.1), (side * WHEEL_X, y, 1.36), mat=team, bevel=0.03, parent=root,
                     name="mudguard")
    st.block((2.1, 8.0, 0.55), (0, -0.05, 0.95), mat=dark, parent=root, name="chassis")
    # Cab at the front: sloped nose, wide windscreen, team roof, bumper, lights, mirrors, step.
    st.wedge((3.0, 1.95, 1.65), (0, 3.25, 2.0), slope_front=0.32, mat=body, parent=root, name="cab")
    st.block((2.5, 1.1, 0.08), (0, 2.95, 2.86), mat=team, bevel=0.02, parent=root, name="cab_roof")
    st.block((2.6, 0.12, 0.5), (0, 4.18, 2.1), rot=(math.radians(-30), 0, 0), mat=glass, bevel=0.02,
             parent=root, name="windscreen")
    st.block((3.1, 0.3, 0.45), (0, 4.28, 0.95), mat=steel, parent=root, name="bumper")
    for side in (-1, 1):
        st.block((0.35, 0.08, 0.18), (side * 1.1, 4.24, 1.4), mat=glass, bevel=0.02, parent=root,
                 name="headlight")
        st.block((0.12, 0.25, 0.3), (side * 1.6, 3.85, 2.3), mat=dark, bevel=0.02, parent=root, name="mirror")
        st.block((0.3, 0.5, 0.1), (side * 1.45, 3.2, 0.9), mat=steel, bevel=0.01, parent=root, name="step")
        st.block((0.05, 0.8, 0.9), (side * 1.51, 3.2, 1.95), mat=dark, bevel=0.01, parent=root, name="door")
    st.cylinder(0.16, 1.5, (1.25, 2.1, 2.4), mat=dark, verts=12, parent=root, name="exhaust")
    st.cylinder(0.2, 0.1, (1.25, 2.1, 3.18), mat=steel, verts=12, parent=root, name="exhaust_cap")
    # Load bed.
    st.block((3.0, 6.0, 0.25), (0, -1.15, DECK_Z - 0.12), mat=body, parent=root, name="bed")
    # Stack of prefab wall panels behind the cab.
    for k in range(3):
        st.block((2.6, 1.55, 0.26), (0, 1.0, DECK_Z + 0.15 + k * 0.28), mat=slab, bevel=0.03, parent=root,
                 name="prefab_panel")
        st.block((2.64, 0.1, 0.1), (0, 1.0 + 0.72, DECK_Z + 0.15 + k * 0.28), mat=steel, bevel=0.0,
                 parent=root, name="panel_edge")
    # Boxy site module in the middle, with a team roof band and vents.
    st.block((2.8, 2.5, 1.45), (0, -1.15, DECK_Z + 0.73), mat=body, bevel=0.06, parent=root, name="module")
    st.block((2.84, 0.7, 0.1), (0, -1.15, DECK_Z + 1.49), mat=team, bevel=0.02, parent=root, name="module_band")
    for x in (-0.8, 0.8):
        st.block((0.6, 0.4, 0.15), (x, -0.35, DECK_Z + 1.52), mat=dark, bevel=0.02, parent=root, name="vent")
    # Crane turntable at the rear; the boom lies folded forward over the load onto a rest above the cab.
    st.cylinder(0.85, 0.5, (0, -3.4, DECK_Z + 0.25), mat=steel, verts=20, bevel=0.04, parent=root,
                name="crane_base")
    st.block((1.1, 1.0, 0.9), (0, -3.45, DECK_Z + 0.9), mat=body, parent=root, name="crane_house")
    st.block((0.9, 0.06, 0.35), (0, -2.94, DECK_Z + 1.0), mat=glass, bevel=0.01, parent=root, name="crane_window")
    boom_z = DECK_Z + 1.75
    st.block((0.55, 6.3, 0.5), (0.55, -0.25, boom_z), mat=steel, parent=root, name="boom")
    for k in range(9):
        st.block((0.57, 0.1, 0.52), (0.55, -3.0 + k * 0.7, boom_z), mat=dark, bevel=0.0, parent=root,
                 name="boom_rib")
    st.block((0.3, 0.3, 0.55), (0.55, 2.5, boom_z - 0.45), mat=dark, parent=root, name="boom_rest")
    st.block((0.28, 0.28, 0.4), (0.55, 2.95, boom_z - 0.15), mat=dark, parent=root, name="hook")
    # Folded outrigger legs at the four corners of the bed.
    for y in (1.75, -3.95):
        for side in (-1, 1):
            st.block((0.45, 0.5, 0.75), (side * 1.62, y, 1.1), mat=steel, parent=root, name="outrigger")
            st.block((0.5, 0.55, 0.12), (side * 1.62, y, 0.75), mat=dark, parent=root, name="outrigger_pad")
    st.block((0.25, 0.12, 0.15), (-1.1, -4.18, 1.2), mat=st.plain("lamp", (0.5, 0.06, 0.03), 0.4),
             bevel=0.01, parent=root, name="lamp")
