"""Slab: one tile of flat concrete paving, four cast plates on a darker bed with joints between them and a few
anchor bolts. It is a terrain tile, so it fills the whole tile with no inset and casts no shadow (DECAL)."""
import rts_studio as st

FOOTPRINT = (1, 1)
DECAL = True
TEAM = False
T = st.STUDIO["metres_per_tile"]
DETAIL = {"anchor"}


def build(root):
    bed = st.concrete("slab_bed", (0.26, 0.25, 0.23))
    plate = st.concrete("slab", (0.5, 0.48, 0.44))
    plate_b = st.concrete("slab_b", (0.46, 0.44, 0.4))
    steel = st.dark_steel()
    W, H = FOOTPRINT[0] * T, FOOTPRINT[1] * T
    st.block((W, H, 0.12), (W / 2, -H / 2, 0.06), mat=bed, parent=root, bevel=0.0, name="bed")
    s, gap = T / 2, 0.22
    for i in range(FOOTPRINT[0] * 2):
        for j in range(FOOTPRINT[1] * 2):
            x, y = (i + 0.5) * s, -(j + 0.5) * s
            st.block((s - gap, s - gap, 0.22), (x, y, 0.18), mat=plate if (i + j) % 2 == 0 else plate_b,
                     parent=root, bevel=0.06, name="plate")
            st.cylinder(0.16, 0.06, (x + s * 0.3, y - s * 0.3, 0.31), mat=steel, parent=root, verts=8,
                        bevel=0.0, name="anchor")

