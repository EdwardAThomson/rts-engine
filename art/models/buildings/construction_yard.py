"""Construction yard: the base's heart. A concrete pad with an assembly hall along the north, an open assembly bay
facing south, stacks of prefab wall panels in the south-east and a tall tower crane whose jib reaches north-west
over the hall. The crane is the silhouette. Two tiles by two, built in proportion to FOOTPRINT."""
import math

import rts_studio as st

FOOTPRINT = (2, 2)
T = st.STUDIO["metres_per_tile"]
W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
INSET = 0.5  # ground contact stays this far inside the footprint
DETAIL = {"vent", "hook", "panel_seam", "stripe", "window"}
# The parts that move on the idle overlay: the crane turns on its mast.
IDLE_PARTS = {"jib", "counter_jib", "counterweight", "cab", "apex", "trolley", "hook", "hook_block", "hook_load"}
# The assembly bay's shutter (closed on the intact frame it is rolled up, so units can roll out to the south).
DOORS = {"shutter"}


def build(root):
    paint, steel, dark, conc = st.team_paint(), st.steel(), st.dark_steel(), st.concrete()
    armour, glass = st.armour(), st.glass()
    stripe = st.plain("stripe", (0.75, 0.55, 0.08))
    st.block((W - 2 * INSET, H - 2 * INSET, 0.3), (W / 2, -H / 2, 0.15), mat=conc, parent=root, bevel=0.08,
             name="pad")

    # Assembly hall across the north, west two thirds: walls of armour panels, a team stripe round the roof.
    hx, hy, hw, hl = W * 0.38, -H * 0.27, W * 0.66, H * 0.42
    st.block((hw, hl, 5.0), (hx, hy, 2.8), mat=armour, parent=root, name="hall")
    st.block((hw + 0.1, hl + 0.1, 0.6), (hx, hy, 5.4), mat=paint, parent=root, name="roof_band")
    st.block((hw - 1.4, hl - 1.4, 0.3), (hx, hy, 5.8), mat=dark, parent=root, name="roof")
    for k in range(4):
        st.block((1.4, 1.4, 0.7), (hx - hw / 2 + 2.6 + k * (hw - 5.2) / 3, hy + hl / 4, 6.2), mat=steel,
                 parent=root, name="vent")
    for k in range(5):
        st.block((0.3, 0.1, 4.4), (hx - hw / 2 + 1.5 + k * (hw - 3) / 4, hy - hl / 2 - 0.05, 2.7), mat=steel,
                 parent=root, name="panel_seam")

    # Assembly bay: a recess in the hall's south face, with the shutter rolled up under the lintel.
    bx = hx + hw * 0.12
    st.block((7.0, 2.0, 4.2), (bx, hy - hl / 2 + 0.95, 2.4), mat=dark, parent=root, bevel=0.02, name="bay")
    st.block((7.6, 0.56, 0.8), (bx, hy - hl / 2 - 0.2, 4.9), mat=steel, parent=root, name="shutter")
    for side in (-1, 1):
        st.block((0.6, 0.6, 4.4), (bx + side * 3.8, hy - hl / 2 - 0.2, 2.5), mat=paint, parent=root,
                 name="door_post")
    # Apron in front of the bay, chevrons pointing out.
    st.block((7.0, H * 0.4, 0.06), (bx, -H * 0.73, 0.33), mat=dark, parent=root, bevel=0.0, name="apron")
    for k in range(6):
        for side in (-1, 1):
            st.block((1.3, 0.3, 0.05), (bx + side * 1.6, -H * 0.58 - k * 1.4, 0.38), rot=(0, 0, side * 0.5),
                     mat=stripe, parent=root, bevel=0.0, name="stripe")

    # Office with windows at the hall's west end, south side.
    ox = hx - hw / 2 + 2.6
    st.block((4.4, 3.4, 3.2), (ox, hy - hl / 2 - 1.5, 1.9), mat=conc, parent=root, name="office")
    st.block((3.6, 0.1, 0.9), (ox, hy - hl / 2 - 3.25, 2.4), mat=glass, parent=root, name="window")

    # Prefab panels stacked in the south-east.
    for i, (px, py) in enumerate(((W * 0.88, -H * 0.72), (W * 0.88, -H * 0.88), (W * 0.72, -H * 0.88))):
        for layer in range(3 - i % 2):
            st.block((3.0, 1.8, 0.55), (px, py, 0.6 + layer * 0.6), rot=(0, 0, 0.04 * (layer - 1)),
                     mat=conc if layer % 2 else armour, parent=root, name="prefab")

    # Tower crane: a square mast on the east side, jib reaching north-west over the hall so it reads as a
    # diagonal from the camera.
    mx, my, mz = W * 0.77, -H * 0.55, 10.5
    st.block((2.6, 2.6, 0.8), (mx, my, 0.7), mat=conc, parent=root, name="crane_base")
    st.block((1.4, 1.4, mz), (mx, my, mz / 2 + 0.8), mat=steel, parent=root, bevel=0.03, name="mast")
    for k in range(6):
        st.block((1.6, 1.6, 0.3), (mx, my, 2.0 + k * 2.0), mat=paint if k % 2 else dark, parent=root,
                 name="mast_band")
    crane = st.group("crane", (mx, my, mz + 0.8), parent=root)
    crane.rotation_euler[2] = math.radians(-30)
    st.block((2.2, 2.0, 1.8), (0, -1.6, 0.4), mat=armour, parent=crane, name="cab")
    st.block((1.8, 0.1, 0.8), (0, -2.65, 0.6), mat=glass, parent=crane, name="window")
    st.block((15.0, 1.0, 0.9), (-7.0, 0, 1.0), mat=paint, parent=crane, name="jib")
    st.block((3.4, 1.0, 0.8), (2.2, 0, 1.0), mat=steel, parent=crane, name="counter_jib")
    st.block((1.8, 1.6, 1.6), (3.2, 0, 0.4), mat=conc, parent=crane, name="counterweight")
    st.cone(0.7, 0.3, 3.0, (0, 0, 2.6), mat=steel, parent=crane, verts=4, name="apex")
    st.block((1.4, 1.4, 0.6), (-10.0, 0, 0.25), mat=dark, parent=crane, name="trolley")
    st.block((0.3, 0.3, 3.0), (-10.0, 0, -1.6), mat=dark, parent=crane, name="hook")
    st.block((1.2, 1.2, 0.8), (-10.0, 0, -3.4), mat=stripe, parent=crane, name="hook_block")
    st.block((2.6, 2.6, 0.5), (-10.0, 0, -4.1), mat=armour, parent=crane, name="hook_load")


def damage(rng, root):
    """Damaged frame, for when the studio renders the `damage` hook: the crane sags, some walls are scorched and
    small parts are knocked off. `rng` is a random.Random the studio seeds, so every render matches."""
    crane = next(o for o in root.children_recursive if o.name.split(".")[0] == "crane")
    crane.rotation_euler[1] = math.radians(rng.uniform(-7, -4))
    scorch(rng, root)


def scorch(rng, root, share=0.25):
    """Burn a share of the larger parts and drop half the detail parts. Local helper; the studio may adopt it."""
    burnt = st.plain("scorch", (0.03, 0.025, 0.02), 0.95)
    for o in st.meshes(root):
        if o.name.split(".")[0] in DETAIL:
            o.hide_render = rng.random() < 0.5
        elif o.data.materials and rng.random() < share:
            o.data.materials[0] = burnt
