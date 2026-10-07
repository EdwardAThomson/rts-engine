"""Saboteur (batch I): a light infiltrator in dark grey overalls and a knit cap, with a satchel of charges on the
back and a compact machine pistol; fires standing, gun held out in front. Slimmer than the rifleman. Team colour
on the satchel's flap and bands, and an armband on each arm. Shared parts and cycles: _soldier.py."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402
import _soldier as s  # noqa: E402

CATEGORY = "infantry"
DETAIL = {"magazine", "pouch", "detonator"}


def build(root, j):
    cloth = s.fatigues("overalls", (0.05, 0.048, 0.045))
    cap = s.fatigues("cap", (0.035, 0.033, 0.03))
    kit = s.fatigues("kit", (0.09, 0.07, 0.045))
    team, dark = st.team_paint(), st.dark_steel()
    s.body(j, cloth, limb=0.13)
    s.knit_cap(j, cap)
    s.belt(j, kit)
    for side in ("l", "r"):
        st.cylinder(0.08, 0.1, (0, 0, -0.12), mat=team, parent=j[f"shoulder_{side}"], verts=12, name="armband")
    # Satchel of charges: a fat box on the back, team flap and two straps round it
    st.block((0.36, 0.22, 0.34), (0, -0.25, 0.28), mat=kit, parent=j["spine"], bevel=0.05, name="satchel")
    st.block((0.34, 0.16, 0.1), (0, -0.25, 0.45), mat=team, parent=j["spine"], bevel=0.02, name="satchel_flap")
    st.block((0.38, 0.24, 0.06), (0, -0.25, 0.2), mat=team, parent=j["spine"], bevel=0.015, name="satchel_band")
    st.block((0.08, 0.06, 0.1), (0.12, 0.15, 0.3), mat=dark, parent=j["spine"], bevel=0.015, name="detonator")
    s.compact_gun(j, dark)


def rig_pose(joints, anim, frame, frames):
    s.pose(joints, anim, frame, frames, hold="compact", stance="stand")
