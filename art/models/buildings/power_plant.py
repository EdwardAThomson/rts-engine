"""Power plant: a turbine hall with three tall exhaust stacks, a big cooling fan on the roof and a transformer
yard facing south. The stacks are the silhouette, so they are tall and thick. The fan is the idle overlay. Two
tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
DETAIL = {"insulator", "cable", "panel_seam", "stripe", "window", "fan_hub"}
# The parts that move on the idle overlay: the fan blades turn in their housing.
IDLE_PARTS = {"fan_blade", "fan_hub"}
DOORS = set()


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    soot = st.plain("soot", (0.02, 0.02, 0.02), 0.95)
    ceramic = st.plain("insulator", (0.5, 0.22, 0.12), 0.4)
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Turbine hall: a long block across the north half with a pitched team-coloured roof band.
    hx, hy, hw, hl = W * 0.5, -H * 0.33, W - 2.4, H * 0.48
    st.block((hw, hl, 5.6), (hx, hy, 3.1), mat=armour, parent=root, name="hall")
    st.block((hw + 0.1, hl + 0.1, 0.6), (hx, hy, 5.9), mat=paint, parent=root, name="roof_band")
    st.block((hw - 1.4, hl - 1.4, 0.3), (hx, hy, 6.3), mat=dark, parent=root, name="roof")
    for k in range(6):
        st.block((0.3, 0.1, 4.6), (hx - hw / 2 + 1.2 + k * (hw - 2.4) / 5, hy - hl / 2 - 0.05, 3.0), mat=steel,
                 parent=root, name="panel_seam")
    for k in range(2):
        st.block((3.0, 0.1, 0.9), (hx - hw / 4 + k * hw / 2, hy - hl / 2 - 0.08, 4.2), mat=glass, parent=root,
                 name="window")

    # Three exhaust stacks along the hall's north edge, banded near the top.
    for k in range(3):
        sx, sy = W * 0.22 + k * 2.6, hy + hl * 0.22
        top = 14.0 - k * 0.8
        stack = st.group("stack", (sx, sy, 0), parent=root)
        st.cylinder(1.0, top, (0, 0, top / 2), mat=conc, parent=stack, verts=24, name="stack_body")
        st.cylinder(1.06, 0.8, (0, 0, top - 1.6), mat=paint, parent=stack, verts=24, name="stack_band")
        st.cylinder(1.12, 0.4, (0, 0, top), mat=dark, parent=stack, verts=24, name="stack_lip")
        st.cylinder(0.75, 0.1, (0, 0, top + 0.2), mat=soot, parent=stack, verts=24, bevel=0.0, name="stack_mouth")

    # Cooling fan on the hall's east roof: a squat housing with a grille ring and four wide blades.
    fx, fy, fz = W * 0.74, hy, 6.5
    st.cylinder(3.2, 1.4, (fx, fy, fz + 0.5), mat=steel, parent=root, verts=40, name="fan_housing")
    st.cylinder(2.8, 0.2, (fx, fy, fz + 1.15), mat=dark, parent=root, verts=40, bevel=0.0, name="fan_well")
    fan = st.group("fan", (fx, fy, fz + 1.35), parent=root)
    for k in range(4):
        a = k * math.pi / 2 + 0.3
        st.block((2.5, 0.75, 0.15), (1.35 * math.cos(a), 1.35 * math.sin(a), 0), rot=(0.3, 0, a), mat=steel,
                 parent=fan, name="fan_blade")
    st.cylinder(0.5, 0.4, (0, 0, 0.1), mat=paint, parent=fan, verts=16, name="fan_hub")

    # Transformer yard in the south half: three transformer blocks with insulators, a fence line of posts and a
    # cable trench back to the hall.
    for k in range(3):
        tx, ty = W * 0.25 + k * W * 0.25, -H * 0.73
        st.block((3.2, 2.6, 2.4), (tx, ty, 1.5), mat=steel, parent=root, name="transformer")
        for side in (-1, 1):
            st.block((0.3, 2.6, 1.8), (tx + side * 1.75, ty, 1.3), mat=dark, parent=root, name="cooling_fins")
        for j in (-1, 0, 1):
            st.cylinder(0.25, 1.2, (tx + j * 0.9, ty, 3.3), mat=ceramic,
                        parent=root, verts=10, name="insulator")
        st.block((3.2, 0.25, 0.25), (tx, ty, 4.0), mat=dark, parent=root, name="cable")
    st.block((W - 3.0, 0.8, 0.12), (W / 2, -H * 0.56, 0.35), mat=dark, parent=root, bevel=0.0, name="trench")
    for k in range(9):
        st.block((0.9, 0.2, 0.05), (2.2 + k * (W - 4.4) / 8, -H * 0.9, 0.33), rot=(0, 0, 0.6), mat=stripe,
                 parent=root, bevel=0.0, name="stripe")


def damage(rng, root):
    """Damaged frame, for when the studio renders the `damage` hook: one stack leans, some walls are scorched and
    small parts are knocked off. `rng` is a random.Random the studio seeds, so every render matches."""
    stacks = [o for o in root.children_recursive if o.name.split(".")[0] == "stack"]
    stacks[rng.randrange(len(stacks))].rotation_euler[0] = math.radians(rng.uniform(8, 14))
    scorch(rng, root)


def scorch(rng, root, share=0.25):
    """Burn a share of the larger parts and drop half the detail parts. Local helper; the studio may adopt it."""
    burnt = st.plain("scorch", (0.03, 0.025, 0.02), 0.95)
    for o in st.meshes(root):
        if o.name.split(".")[0] in DETAIL:
            o.hide_render = rng.random() < 0.5
        elif o.data.materials and rng.random() < share:
            o.data.materials[0] = burnt
