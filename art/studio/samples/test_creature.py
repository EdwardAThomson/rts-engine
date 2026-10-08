"""Studio test model for the creature kind: a plain post that rises out of the ground and sinks back, to check
that the ground hides whatever is below z = 0 and that the canvas holds the tallest pose. Not a game asset.

    python3 art/studio/render.py art/studio/samples/test_creature.py --out /tmp/renders/test_creature"""
import rts_studio as st

CATEGORY = "creatures"
TEAM = False
ANIMS = {"rise": 4, "sink": 2}
HEIGHT = 6.0


def build(root):
    post = st.group("post", parent=root)
    st.cylinder(1.5, HEIGHT, (0, 0, HEIGHT / 2), mat=st.concrete(), verts=16, parent=post, name="shaft")
    st.cone(1.5, 0.3, 1.2, (0, 0, HEIGHT + 0.6), mat=st.steel(), verts=16, parent=post, name="tip")


def pose(root, anim, frame):
    post = next(o for o in root.children_recursive if o.name.split(".")[0] == "post")
    up = (frame + 1) / ANIMS["rise"] if anim == "rise" else 1 - (frame + 1) / (ANIMS["sink"] + 1)
    post.location.z = -HEIGHT * (1 - up)
