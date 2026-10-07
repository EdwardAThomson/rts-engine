"""Guerrilla (batch I): an irregular fighter in mixed, loose clothing: a cloth head wrap and scarf, a long tunic
over darker trousers, a bandolier, a shoulder bag and a rifle; kneels to fire. No helmet or vest, so the
silhouette is softer than the regulars'. Team colour on the wrap's band, an armband on each arm and the bag's
flap. Shared parts and cycles: _soldier.py."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402
import _soldier as s  # noqa: E402

CATEGORY = "infantry"
DETAIL = {"sight", "magazine", "bandolier"}


def build(root, j):
    tunic = s.fatigues("tunic", (0.17, 0.12, 0.07))
    trousers = s.fatigues("trousers", (0.07, 0.06, 0.05))
    wrap = s.fatigues("wrap", (0.2, 0.17, 0.12))
    leather = s.fatigues("leather", (0.1, 0.055, 0.03))
    team, dark = st.team_paint(), st.dark_steel()
    s.body(j, tunic, trousers=trousers, boots=leather)
    # The tunic hangs below the belt, front and back
    st.block((0.4, 0.28, 0.32), (0, 0, -0.12), mat=tunic, parent=j["pelvis"], bevel=0.05, name="skirt")
    s.head_wrap(j, wrap, team)
    st.beam((0.2, 0.15, 0.48), (-0.18, 0.15, 0.04), 0.07, leather, j["spine"], "bandolier")
    st.beam((0.2, -0.15, 0.48), (-0.18, -0.15, 0.04), 0.07, leather, j["spine"], "bandolier")
    for side in ("l", "r"):
        st.cylinder(0.085, 0.09, (0, 0, -0.12), mat=team, parent=j[f"shoulder_{side}"], verts=12, name="armband")
    st.block((0.26, 0.14, 0.24), (-0.2, -0.12, 0.08), mat=leather, parent=j["pelvis"], bevel=0.04, name="bag")
    st.block((0.24, 0.15, 0.08), (-0.2, -0.12, 0.2), mat=team, parent=j["pelvis"], bevel=0.02, name="bag_flap")
    s.rifle(j, dark, length=0.85)


def rig_pose(joints, anim, frame, frames):
    s.pose(joints, anim, frame, frames, hold="rifle", stance="kneel")
