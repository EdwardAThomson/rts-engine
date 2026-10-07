"""Silo: four round storage tanks on a pad, joined by a catwalk to a pump house in the middle. Each tank has a team
band and a gauge stripe on its south face, a glass tube showing the stored resource. Two tiles by two, built in
proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
TANK_R, TANK_H = 3.9, 6.0
DETAIL = {"bolt", "rail", "gauge_mark", "hatch"}
IDLE_PARTS = set()
DOORS = set()


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    load, glass = st.cargo(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Four tanks, a little different in fill so the gauges don't look stamped.
    for i, (cx, cy) in enumerate(((0.27, -0.27), (0.73, -0.27), (0.27, -0.73), (0.73, -0.73))):
        x, y = cx * W, cy * H
        st.cylinder(TANK_R + 0.3, 0.5, (x, y, 0.55), mat=conc, parent=root, verts=40, name="plinth")
        st.cylinder(TANK_R, TANK_H, (x, y, 0.8 + TANK_H / 2), mat=steel, parent=root, verts=40, name="tank")
        st.cylinder(TANK_R + 0.04, 1.4, (x, y, 0.8 + TANK_H * 0.75), mat=paint, parent=root, verts=40,
                    name="tank_band")
        st.sphere(TANK_R, (x, y, 0.8 + TANK_H), mat=steel, parent=root, scale=(1, 1, 0.32), name="tank_top")
        st.cylinder(0.7, 0.4, (x, y, 0.8 + TANK_H + TANK_R * 0.32), mat=dark, parent=root, verts=16,
                    name="hatch")
        for k in range(8):
            a = k * math.pi / 4
            st.block((0.3, 0.3, 0.3), ((TANK_R + 0.02) * math.cos(a) + x, (TANK_R + 0.02) * math.sin(a) + y,
                                       1.2), rot=(0, 0, a), mat=dark, parent=root, name="bolt")

        # Gauge: a yellow backing stripe on the south face, a dark glass tube and the resource filling it.
        gy = y - TANK_R - 0.12
        fill = (0.85, 0.55, 0.7, 0.4)[i]
        gz0, gh = 1.2, TANK_H - 1.0
        st.block((1.0, 0.25, gh + 0.4), (x, gy + 0.05, gz0 + gh / 2), mat=stripe, parent=root, name="gauge")
        st.block((0.5, 0.3, gh), (x, gy - 0.05, gz0 + gh / 2), mat=glass, parent=root, name="gauge_glass")
        st.block((0.42, 0.34, gh * fill), (x, gy - 0.06, gz0 + gh * fill / 2), mat=load, parent=root,
                 bevel=0.0, name="gauge_fill")
        for k in range(4):
            st.block((0.9, 0.36, 0.12), (x, gy - 0.07, gz0 + gh * (k + 1) / 5), mat=dark, parent=root,
                     bevel=0.0, name="gauge_mark")

    # Pump house in the middle, with a raised catwalk crossing it between the tanks.
    st.block((3.6, 3.6, 3.4), (W / 2, -H / 2, 2.0), mat=st.armour(), parent=root, name="pump_house")
    st.block((3.8, 3.8, 0.4), (W / 2, -H / 2, 3.8), mat=paint, parent=root, name="pump_roof")
    st.block((1.2, 1.2, 3.8), (W / 2, -H / 2, 5.8), mat=steel, parent=root, name="catwalk_post")
    for rot in (0, math.pi / 2):
        st.block((W * 0.46, 1.0, 0.3), (W / 2, -H / 2, 0.8 + TANK_H + 0.9), rot=(0, 0, rot), mat=steel,
                 parent=root, name="catwalk")
    for side in (-1, 1):
        st.block((W * 0.46, 0.25, 0.6), (W / 2, -H / 2 + side * 0.5, 0.8 + TANK_H + 1.3), mat=dark,
                 parent=root, bevel=0.0, name="rail")


def damage(rng, root):
    """Damaged frame, for when the studio renders the `damage` hook: one tank top is torn open, some walls are
    scorched and small parts are knocked off. `rng` is a random.Random the studio seeds, so every render
    matches."""
    tops = [o for o in st.meshes(root) if o.name.split(".")[0] == "tank_top"]
    torn = tops[rng.randrange(len(tops))]
    torn.rotation_euler = (math.radians(rng.uniform(15, 25)), math.radians(rng.uniform(-10, 10)), 0)
    torn.location.z += 0.6
    scorch(rng, root)


def scorch(rng, root, share=0.25):
    """Burn a share of the larger parts and drop half the detail parts. Local helper; the studio may adopt it."""
    burnt = st.plain("scorch", (0.03, 0.025, 0.02), 0.95)
    for o in st.meshes(root):
        if o.name.split(".")[0] in DETAIL:
            o.hide_render = rng.random() < 0.5
        elif o.data.materials and rng.random() < share:
            o.data.materials[0] = burnt
