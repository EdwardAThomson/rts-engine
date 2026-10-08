"""Facing-order test model (check.py --facings): a long bar pointing north with a block on its tip, so the
sprite's centre of mass sits well forward of the pivot. Rendered through the normal pipeline, its centroid must
turn clockwise from north, one facing step at a time."""
import rts_studio as st

CATEGORY = "vehicles"


def build_hull(root):
    m = st.plain("arrow", (0.5, 0.45, 0.4))
    st.block((0.6, 6.0, 0.6), (0, 2.0, 0.3), mat=m, parent=root, name="shaft")
    st.block((2.4, 1.6, 0.8), (0, 5.0, 0.4), mat=m, parent=root, name="head")
