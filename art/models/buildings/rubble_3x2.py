"""Rubble left by any destroyed 3x2 building: drawn under the ruins as a decal, shared by every building of
that size. Made by the studio's rubble helper with a fixed seed."""
import random

import rts_studio as st

FOOTPRINT = (3, 2)
DECAL = True  # one intact frame, no shadow, no team colour, no construction or damage frames
TEAM = False


def build(root):
    st.rubble(random.Random("rubble_3x2"), root, FOOTPRINT)
