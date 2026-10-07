"""Scout bike: a fast two-wheeled scout in the house style. Chunky off-road tyres, a light frame, a team-coloured
nose fairing, side panels and rear fender, a rider hunched over the tank and a stubby gun on the bars. About 3 m
long, built facing north (+y). No turret: the bike turns to aim.

`rider(root, ...)` is shared with raider_bike and quad, which import this file."""
import math

import rts_studio as st

WHEEL_R, TYRE_W = 0.42, 0.3
WHEEL_Y = 1.0            # front and rear axles sit this far either side of the origin
DETAIL = {"bars", "exhaust", "headlight", "visor", "ammo_box", "hub", "number_plate"}


def wheel(root, loc, r=WHEEL_R, w=TYRE_W, rubber=None, steel=None):
    st.cylinder(r, w, loc, rot=(0, math.pi / 2, 0), mat=rubber, verts=20, bevel=0.06, parent=root, name="tyre")
    st.cylinder(r * 0.45, w + 0.04, loc, rot=(0, math.pi / 2, 0), mat=steel, verts=12, parent=root, name="hub")


def rider(root, loc, lean_deg=45, cloth=None, helmet=None, glass=None):
    """A crew figure, sitting with its hips at `loc` and leaning forward by `lean_deg`, hands reaching forward to
    bars or a wheel. Built thick enough to read at sprite size."""
    x, y, z = loc
    a = math.radians(lean_deg)
    torso_h = 0.62
    cx, cy, cz = x, y + math.sin(a) * torso_h / 2, z + math.cos(a) * torso_h / 2
    st.block((0.5, 0.32, torso_h), (cx, cy, cz), rot=(-a, 0, 0), mat=cloth, bevel=0.08, parent=root,
             name="torso")
    hy, hz = y + math.sin(a) * (torso_h + 0.16), z + math.cos(a) * (torso_h + 0.16)
    st.sphere(0.19, (x, hy, hz), mat=helmet, parent=root, scale=(1.0, 1.1, 0.95), name="helmet")
    st.block((0.26, 0.06, 0.1), (x, hy + 0.18, hz - 0.02), mat=glass, bevel=0.02, parent=root, name="visor")
    sy, sz = y + math.sin(a) * torso_h * 0.85, z + math.cos(a) * torso_h * 0.85
    for side in (-1, 1):
        # arms reach forward and down from the shoulders; thighs run forward from the hips
        st.block((0.17, 0.55, 0.17), (x + side * 0.26, sy + 0.25, sz - 0.12), rot=(math.radians(-20), 0, 0),
                 mat=cloth, bevel=0.05, parent=root, name="arm")
        st.block((0.2, 0.5, 0.2), (x + side * 0.2, y + 0.22, z + 0.02), mat=cloth, bevel=0.05, parent=root,
                 name="thigh")
        st.block((0.18, 0.2, 0.42), (x + side * 0.22, y + 0.44, z - 0.2), mat=cloth, bevel=0.05, parent=root,
                 name="shin")


def crew_materials():
    """Dusty fatigues and a dark helmet: brown, kept well clear of the team hue band."""
    return st.armour("fatigues", (0.13, 0.1, 0.07)), st.plain("helmet", (0.09, 0.08, 0.07), 0.5), st.glass()


def build_hull(root):
    team, body = st.team_paint(), st.armour()
    steel, dark, rubber = st.steel(), st.dark_steel(), st.rubber()
    cloth, helmet, glass = crew_materials()
    for y in (-WHEEL_Y, WHEEL_Y):
        wheel(root, (0, y, WHEEL_R), rubber=rubber, steel=steel)
    # Frame: swingarm to the rear wheel, engine in the middle, raked fork to the front wheel.
    st.block((0.3, 0.95, 0.16), (0, -0.6, 0.45), mat=dark, parent=root, name="swingarm")
    st.block((0.42, 0.75, 0.45), (0, -0.05, 0.6), mat=dark, parent=root, name="engine")
    st.block((0.36, 0.14, 0.8), (0, 0.82, 0.78), rot=(math.radians(-25), 0, 0), mat=steel, parent=root,
             name="fork")
    # Body: fuel tank, team nose fairing, side panels and rear fender, seat.
    st.block((0.5, 0.72, 0.32), (0, 0.3, 0.98), mat=body, bevel=0.1, parent=root, name="tank")
    st.wedge((0.64, 0.5, 0.55), (0, 0.85, 1.08), slope_front=0.55, mat=team, parent=root, name="fairing")
    for side in (-1, 1):
        st.block((0.07, 0.75, 0.32), (side * 0.27, -0.15, 0.72), mat=team, parent=root, name="side_panel")
    st.block((0.38, 0.72, 0.13), (0, -0.48, 0.93), mat=rubber, parent=root, name="seat")
    st.block((0.4, 0.75, 0.09), (0, -1.08, 0.94), rot=(math.radians(8), 0, 0), mat=team, parent=root,
             name="fender")
    st.block((0.3, 0.05, 0.2), (0, -1.45, 0.85), mat=dark, bevel=0.01, parent=root, name="number_plate")
    st.cylinder(0.09, 0.75, (0.27, -0.75, 0.55), rot=(math.pi / 2 - 0.2, 0, 0), mat=steel, verts=10,
                parent=root, name="exhaust")
    st.block((0.2, 0.06, 0.12), (0, 1.11, 1.08), mat=glass, bevel=0.02, parent=root, name="headlight")
    # Bars, and a stubby gun on them (thickened so it reads at 32 px).
    st.block((0.8, 0.14, 0.14), (0, 0.7, 1.3), mat=steel, parent=root, name="bars")
    st.block((0.26, 0.42, 0.24), (0, 0.92, 1.44), mat=dark, parent=root, name="gun_body")
    st.cylinder(0.13, 0.5, (0, 1.33, 1.46), rot=(math.pi / 2, 0, 0), mat=steel, verts=14, parent=root,
                name="barrel")
    st.block((0.14, 0.2, 0.18), (0.2, 0.9, 1.4), mat=dark, bevel=0.02, parent=root, name="ammo_box")
    rider(root, (0, -0.35, 1.02), lean_deg=48, cloth=cloth, helmet=helmet, glass=glass)
