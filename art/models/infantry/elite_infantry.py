"""Elite infantry (batch I): a heavier trooper in an armoured plate carrier with shoulder plates and knee pads, a
closed helmet with a dark visor, and a bulky support rifle with a drum magazine; stands braced to fire. A shade
larger than the rifleman so the two read apart. Team colour on the helmet band, chest panel, shoulder plates and
pack flap. Shared parts and cycles: _soldier.py."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rts_studio as st  # noqa: E402
import _soldier as s  # noqa: E402

CATEGORY = "infantry"
DETAIL = {"sight", "pouch", "drum"}
SIZE = 1.08  # relative to the rifleman


def build(root, j):
    j["body"].scale = (st.INFANTRY_SCALE * SIZE,) * 3
    cloth, team, dark = s.fatigues("fatigues", (0.1, 0.085, 0.06)), st.team_paint(), st.dark_steel()
    plate = st.armour("plate", (0.13, 0.11, 0.08))
    s.body(j, cloth, limb=0.155)
    s.helmet(j, plate, team, visor=True)
    s.vest(j, plate, team, plates=True)
    s.backpack(j, plate, team, size=(0.34, 0.18, 0.27))
    s.belt(j, dark)
    s.heavy_rifle(j, dark)


def rig_pose(joints, anim, frame, frames):
    s.pose(joints, anim, frame, frames, hold="rifle", stance="stand")
