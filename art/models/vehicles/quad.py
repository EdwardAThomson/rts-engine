"""Quad: a four-wheeled light attack buggy. Open tub with two crew side by side, a chunky roll cage with twin guns
on a pintle over the front bar, team-coloured fenders, and a hood stripe, and a spare wheel on the
back. About 4 m long, built facing north (+y); the buggy turns to aim. Crew come from scout_bike."""
import importlib.util
import math
from pathlib import Path

import rts_studio as st

_spec = importlib.util.spec_from_file_location("vehicles.scout_bike", Path(__file__).with_name("scout_bike.py"))
scout = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(scout)

WHEEL_R, TYRE_W = 0.5, 0.45
AXLE_X, AXLE_Y = 1.0, 1.3
CAGE_Z = 2.0             # top of the roll cage
DETAIL = {"hub", "visor", "headlight", "jerrycan", "exhaust", "steering", "bumper_lug"}


def build_hull(root):
    team, body = st.team_paint(), st.armour()
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    cloth, helmet, glass = scout.crew_materials()
    for x in (-AXLE_X, AXLE_X):
        for y in (-AXLE_Y, AXLE_Y):
            scout.wheel(root, (x, y, WHEEL_R), r=WHEEL_R, w=TYRE_W, rubber=rubber, steel=steel)
            st.block((0.55, 0.95, 0.1), (x * 1.02, y, 1.08), mat=team, bevel=0.03, parent=root, name="fender")
        # suspension arms from the chassis out to each wheel pair
        st.block((0.7, 0.3, 0.2), (x * 0.6, AXLE_Y, 0.5), mat=dark, parent=root, name="arm")
        st.block((0.7, 0.3, 0.2), (x * 0.6, -AXLE_Y, 0.5), mat=dark, parent=root, name="arm")
    st.block((1.3, 3.5, 0.3), (0, 0, 0.55), mat=dark, parent=root, name="chassis")
    # Open tub and a sloped hood with a team stripe.
    st.block((1.5, 1.9, 0.55), (0, -0.25, 0.92), mat=body, parent=root, name="tub")
    st.wedge((1.4, 1.15, 0.55), (0, 1.25, 0.95), slope_front=0.5, mat=body, parent=root, name="hood")
    st.block((0.42, 1.0, 0.06), (0, 1.12, 1.22), rot=(math.radians(-14), 0, 0), mat=team, bevel=0.01,
             parent=root, name="hood_stripe")
    st.block((1.6, 0.3, 0.3), (0, 1.88, 0.62), mat=steel, parent=root, name="bumper")
    for side in (-1, 1):
        st.block((0.12, 0.1, 0.25), (side * 0.55, 2.04, 0.62), mat=steel, bevel=0.01, parent=root,
                 name="bumper_lug")
        st.block((0.2, 0.06, 0.12), (side * 0.5, 1.83, 1.0), mat=glass, bevel=0.02, parent=root,
                 name="headlight")
    for x in (-0.35, 0.35):
        st.block((0.48, 0.5, 0.18), (x, -0.3, 1.25), mat=rubber, parent=root, name="seat")
        st.block((0.48, 0.14, 0.55), (x, -0.58, 1.5), rot=(math.radians(12), 0, 0), mat=rubber, parent=root,
                 name="seat_back")
        scout.rider(root, (x, -0.28, 1.36), lean_deg=12, cloth=cloth, helmet=helmet, glass=glass)
    st.cylinder(0.18, 0.06, (-0.35, 0.42, 1.65), rot=(math.radians(60), 0, 0), mat=dark, verts=12,
                parent=root, name="steering")
    # Roll cage: four posts, side rails and cross bars, thick enough not to shimmer. Rails are a little thinner than
    # the posts and bars thinner still, so no two parts share a face plane at the joints (that renders dark).
    bar = 0.26
    for side in (-1, 1):
        x = side * 0.72
        st.block((bar, bar, 1.15), (x, 0.55, 1.5), rot=(math.radians(-12), 0, 0), mat=steel, parent=root,
                 name="cage_post")
        st.block((bar, bar, 1.1), (x, -0.95, 1.5), mat=steel, parent=root, name="cage_post")
        st.block((bar - 0.03, 1.75, bar - 0.03), (x, -0.22, CAGE_Z), mat=steel, parent=root, name="cage_rail")
    for y in (0.65, -0.95):
        st.block((1.64, bar - 0.05, bar - 0.05), (0, y, CAGE_Z), mat=steel, parent=root, name="cage_bar")
    # Twin guns on a pintle over the front bar.
    st.cylinder(0.14, 0.35, (0, 0.7, CAGE_Z + 0.25), mat=dark, verts=12, parent=root, name="pintle")
    st.block((0.7, 0.55, 0.28), (0, 0.82, CAGE_Z + 0.45), mat=dark, parent=root, name="gun_cradle")
    for x in (-0.2, 0.2):
        st.cylinder(0.13, 0.95, (x, 1.55, CAGE_Z + 0.47), rot=(math.pi / 2, 0, 0), mat=steel, verts=14,
                    parent=root, name="barrel")
    st.block((0.28, 0.3, 0.32), (0.5, 0.75, CAGE_Z + 0.4), mat=body, parent=root, name="ammo_box")
    # Rear: spare wheel upright on the back, a jerrycan and an exhaust.
    st.cylinder(0.42, 0.3, (0, -1.83, 1.0), rot=(math.pi / 2, 0, 0), mat=rubber, verts=20, bevel=0.06,
                parent=root, name="spare_wheel")
    st.block((0.28, 0.2, 0.4), (0.6, -1.3, 1.35), mat=dark, parent=root, name="jerrycan")
    st.cylinder(0.08, 0.5, (-0.6, -1.75, 0.6), rot=(math.pi / 2, 0, 0), mat=steel, verts=10, parent=root,
                name="exhaust")
