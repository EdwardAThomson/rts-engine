"""Studio test wall, 1x1: a pillar with an arm towards each joined neighbour (JOINS; bit 1 north, 2 east, 4 south,
8 west). Not a game asset."""
import rts_studio as st

CATEGORY = "buildings"
FOOTPRINT = (1, 1)
JOINS = True
TEAM = False  # walls carry no team colour; check.py skips the coverage check
T = st.STUDIO["metres_per_tile"]


def build(root, joins):
    conc = st.concrete("wall", (0.55, 0.52, 0.47))
    c = (T / 2, -T / 2)
    st.block((4.0, 4.0, 3.2), (c[0], c[1], 1.6), mat=conc, parent=root, name="pillar")
    arms = {1: (0, 1), 2: (1, 0), 4: (0, -1), 8: (-1, 0)}
    for bit, (dx, dy) in arms.items():
        if joins & bit:
            # Reach to the tile edge, so neighbouring walls meet.
            L = T / 2
            st.block((3.2 if dx == 0 else L, 3.2 if dy == 0 else L, 2.8),
                     (c[0] + dx * L / 2, c[1] + dy * L / 2, 1.4), mat=conc, parent=root, name="arm")
