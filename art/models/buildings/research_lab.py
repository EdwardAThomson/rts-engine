"""Research lab: a clean, pale, blocky lab. A main wing with a glass band, a big observation dome and a small
sensor dome on the roof, a lower annex with roof plant, and two antenna masts. Two tiles by two, built in
proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
DETAIL = {"vent", "stripe", "mullion", "aerial_bar", "dome_seam", "light"}
IDLE_PARTS = set()
DOORS = set()


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    glass = st.glass()
    # Clean lab panels: a pale concrete, lighter than the pad so the building stands off it.
    pale = st.concrete("lab_panel", (0.68, 0.66, 0.62))
    white = st.plain("dome_shell", (0.72, 0.71, 0.68), roughness=0.35)
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Main wing across the north, with a glass band and an entrance canopy on the south side.
    mx, my, mw, ml = W * 0.42, -H * 0.33, W * 0.74, H * 0.5
    st.block((mw, ml, 6.4), (mx, my, 3.5), mat=pale, parent=root, bevel=0.15, name="wing")
    st.block((mw + 0.1, ml + 0.1, 0.5), (mx, my, 6.75), mat=paint, parent=root, name="roof_band")
    st.block((mw - 1.2, ml - 1.2, 0.25), (mx, my, 7.05), mat=st.steel("roof_deck", (0.4, 0.39, 0.37)),
             parent=root, name="roof")
    st.block((mw - 1.0, 0.14, 1.6), (mx, my - ml / 2 - 0.05, 4.4), mat=glass, parent=root, name="glass_band")
    for k in range(7):
        st.block((0.25, 0.2, 1.7), (mx - (mw - 1.0) / 2 + k * (mw - 1.0) / 6, my - ml / 2 - 0.1, 4.4),
                 mat=steel, parent=root, name="mullion")
    st.block((4.0, 2.4, 0.4), (mx - mw / 4, my - ml / 2 - 1.2, 3.0), mat=paint, parent=root, name="canopy")
    for side in (-1, 1):
        st.block((0.4, 0.4, 2.8), (mx - mw / 4 + side * 1.7, my - ml / 2 - 2.1, 1.6), mat=steel, parent=root,
                 name="canopy_post")
    st.block((2.4, 0.3, 2.4), (mx - mw / 4, my - ml / 2 - 0.1, 1.5), mat=glass, parent=root, name="entrance")

    # Observation dome on the west roof, a small sensor dome east of it.
    dx, dy = mx - mw * 0.18, my + 0.4
    st.cylinder(3.6, 0.8, (dx, dy, 7.5), mat=pale, parent=root, verts=40, name="dome_drum")
    st.sphere(3.4, (dx, dy, 7.9), mat=white, parent=root, scale=(1, 1, 0.9), name="dome")
    for a in (0.4, 2.0):
        st.cylinder(3.43, 0.25, (dx, dy, 8.3 + a), mat=steel, parent=root, verts=40, bevel=0.0,
                    scale=(1 - a * 0.15, 1 - a * 0.15, 1), name="dome_seam")
    st.sphere(1.7, (mx + mw * 0.22, my + 1.0, 7.2), mat=white, parent=root, name="sensor_dome")

    # Annex in the south-east: lower block with roof plant.
    ax, ay, aw, al = W * 0.76, -H * 0.73, W * 0.36, H * 0.4
    st.block((aw, al, 4.0), (ax, ay, 2.3), mat=pale, parent=root, bevel=0.15, name="annex")
    st.block((aw + 0.1, al + 0.1, 0.4), (ax, ay, 4.3), mat=paint, parent=root, name="annex_band")
    st.block((aw - 1.0, al - 1.0, 0.25), (ax, ay, 4.55), mat=st.steel("annex_deck", (0.4, 0.39, 0.37)),
             parent=root, name="annex_roof")
    for k in range(2):
        st.block((2.2, 2.2, 1.0), (ax - 1.6 + k * 3.2, ay + 0.6, 5.0), mat=steel, parent=root, name="plant")
        st.cylinder(0.85, 0.15, (ax - 1.6 + k * 3.2, ay + 0.6, 5.55), mat=dark, parent=root, verts=20,
                    bevel=0.0, name="vent")
    st.block((aw - 1.0, 0.12, 0.9), (ax, ay - al / 2 - 0.05, 2.6), mat=glass, parent=root, name="window")

    # Two antenna masts on the wing's east roof, with chunky cross bars.
    for k, (ax2, h) in enumerate(((mx + mw * 0.36, 6.0), (mx + mw * 0.36 - 2.6, 4.5))):
        st.block((0.4, 0.4, h), (ax2, my + ml / 2 - 1.2, 7.1 + h / 2), mat=steel, parent=root, name="aerial")
        for j in range(2):
            st.block((1.8 - j * 0.6, 0.3, 0.3), (ax2, my + ml / 2 - 1.2, 7.1 + h - 0.6 - j * 1.4), mat=dark,
                     parent=root, name="aerial_bar")
        st.sphere(0.35, (ax2, my + ml / 2 - 1.2, 7.1 + h + 0.2), mat=st.plain("light", (0.8, 0.12, 0.05),
                  emission=2.0), parent=root, name="light")

    # Markings in the open south-west corner.
    for k in range(5):
        st.block((0.9, 0.2, 0.05), (W * 0.12 + k * 1.1, -H * 0.92, 0.33), rot=(0, 0, 0.6), mat=stripe,
                 parent=root, bevel=0.0, name="stripe")

def damage(rng, root):
    """Damaged frame, for when the studio renders the `damage` hook: the big dome is cracked open, some walls are
    scorched and small parts are knocked off. `rng` is a random.Random the studio seeds, so every render
    matches."""
    dome = next(o for o in st.meshes(root) if o.name.split(".")[0] == "dome")
    dome.scale = (1.0, 1.0, rng.uniform(0.55, 0.75))
    dome.rotation_euler[0] = math.radians(rng.uniform(-8, 8))
    scorch(rng, root)


def scorch(rng, root, share=0.25):
    """Burn a share of the larger parts and drop half the detail parts. Local helper; the studio may adopt it."""
    burnt = st.plain("scorch", (0.03, 0.025, 0.02), 0.95)
    for o in st.meshes(root):
        if o.name.split(".")[0] in DETAIL:
            o.hide_render = rng.random() < 0.5
        elif o.data.materials and rng.random() < share:
            o.data.materials[0] = burnt
