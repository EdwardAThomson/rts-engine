"""Raider bike: the heavier, armed variant of the scout bike. Wider tyres, team-coloured armour plates hung on both
sides, a gun shield over the bars and a bigger gun with a muzzle brake and a drum magazine, plus a rear ammo
rack. Shares the scout's wheels and rider. About 3.1 m long, built facing north (+y); the bike turns to aim."""
import importlib.util
import math
from pathlib import Path

import rts_studio as st

_spec = importlib.util.spec_from_file_location("vehicles.scout_bike", Path(__file__).with_name("scout_bike.py"))
scout = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(scout)

WHEEL_R, TYRE_W = 0.45, 0.38
DETAIL = {"bars", "exhaust", "headlight", "visor", "hub", "bolt", "rack_strap"}


def build_hull(root):
    team, body = st.team_paint(), st.armour()
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    cloth, helmet, glass = scout.crew_materials()
    for y in (-1.02, 1.02):
        scout.wheel(root, (0, y, WHEEL_R), r=WHEEL_R, w=TYRE_W, rubber=rubber, steel=steel)
    st.block((0.36, 0.95, 0.18), (0, -0.62, 0.48), mat=dark, parent=root, name="swingarm")
    st.block((0.5, 0.8, 0.5), (0, -0.05, 0.62), mat=dark, parent=root, name="engine")
    st.block((0.44, 0.16, 0.85), (0, 0.84, 0.8), rot=(math.radians(-25), 0, 0), mat=steel, parent=root,
             name="fork")
    # Armoured body: tank and an armour nose, with team side plates angled out over the wheels.
    st.block((0.56, 0.75, 0.34), (0, 0.3, 1.0), mat=body, bevel=0.1, parent=root, name="tank")
    st.wedge((0.7, 0.55, 0.5), (0, 0.88, 1.06), slope_front=0.5, mat=body, parent=root, name="nose")
    for side in (-1, 1):
        st.block((0.1, 1.5, 0.5), (side * 0.38, -0.12, 0.78), rot=(0, side * math.radians(-12), 0), mat=team,
                 bevel=0.03, parent=root, name="side_plate")
        for y in (-0.7, -0.12, 0.46):
            st.cylinder(0.04, 0.05, (side * 0.44, y, 0.95), rot=(0, math.pi / 2, 0), mat=steel, verts=8,
                        parent=root, name="bolt")
    st.block((0.42, 0.72, 0.14), (0, -0.48, 0.96), mat=rubber, parent=root, name="seat")
    # Rear ammo rack over the back wheel, with a team lid.
    st.block((0.6, 0.62, 0.32), (0, -1.12, 1.12), mat=body, parent=root, name="ammo_rack")
    st.block((0.62, 0.64, 0.07), (0, -1.12, 1.3), mat=team, parent=root, name="rack_lid")
    st.block((0.66, 0.06, 0.36), (0, -0.95, 1.12), mat=dark, bevel=0.01, parent=root, name="rack_strap")
    st.cylinder(0.1, 0.8, (0.3, -0.8, 0.55), rot=(math.pi / 2 - 0.2, 0, 0), mat=steel, verts=10, parent=root,
                name="exhaust")
    st.block((0.22, 0.06, 0.12), (0, 1.16, 1.0), mat=glass, bevel=0.02, parent=root, name="headlight")
    # Bars, a gun shield and the heavy gun: long barrel, muzzle brake, drum magazine.
    st.block((0.85, 0.14, 0.14), (0, 0.7, 1.32), mat=steel, parent=root, name="bars")
    st.block((0.78, 0.1, 0.42), (0, 0.66, 1.56), rot=(math.radians(-15), 0, 0), mat=body, parent=root,
             name="gun_shield")
    st.block((0.32, 0.55, 0.3), (0, 0.98, 1.5), mat=dark, parent=root, name="gun_body")
    st.cylinder(0.15, 0.55, (0, 1.3, 1.52), rot=(math.pi / 2, 0, 0), mat=steel, verts=14, parent=root,
                name="barrel")
    st.cylinder(0.19, 0.18, (0, 1.58, 1.52), rot=(math.pi / 2, 0, 0), mat=dark, verts=8, bevel=0.02,
                parent=root, name="muzzle_brake")
    st.cylinder(0.2, 0.18, (0.28, 0.92, 1.42), rot=(0, math.pi / 2, 0), mat=dark, verts=14, parent=root,
                name="drum")
    scout.rider(root, (0, -0.35, 1.05), lean_deg=48, cloth=cloth, helmet=helmet, glass=glass)
