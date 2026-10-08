"""Basic rifle infantry (batch I): combat helmet, sandy fatigues, a load vest and a rifle; kneels to fire. Team
colour on the helmet band, chest panel, arm patches and pack flap. Shared parts and cycles: _soldier.py."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402
import _soldier as s  # noqa: E402

CATEGORY = "infantry"
DETAIL = {"sight", "pouch", "magazine"}


def build(root, j):
    cloth, team, dark = s.fatigues(), st.team_paint(), st.dark_steel()
    kit = s.fatigues("webbing", (0.065, 0.05, 0.03))
    s.body(j, cloth)
    s.helmet(j, cloth, team)
    s.vest(j, kit, team)
    s.patches(j, team)
    s.backpack(j, kit, team)
    s.belt(j, kit)
    s.rifle(j, dark)


def rig_pose(joints, anim, frame, frames):
    s.pose(joints, anim, frame, frames, hold="rifle", stance="kneel")
