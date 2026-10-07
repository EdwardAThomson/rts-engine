"""Infantry prototype on the studio's rig (art-pipeline.md section 6, route A): rigid parts, each on one of the
rig's twelve joints, in rest-pose metres for a 1.75 m soldier (the rig scales it by 1.75). Batch I starts from
this. Generic: helmet, fatigues, webbing, a rifle; the team colour is the helmet band and shoulder patches."""
import rts_studio as st

CATEGORY = "infantry"
DETAIL = {"strap", "sight"}


def build(root, j):
    cloth = st.armour("fatigues", (0.18, 0.14, 0.09))
    skin = st.plain("skin", (0.42, 0.28, 0.2), roughness=0.7)
    dark = st.dark_steel()
    team = st.team_paint()
    boots = st.rubber()
    # Torso and head
    st.block((0.38, 0.24, 0.5), (0, 0, 0.25), mat=cloth, parent=j["spine"], bevel=0.05, name="torso")
    st.block((0.4, 0.28, 0.2), (0, 0, 0.18), mat=dark, parent=j["spine"], bevel=0.04, name="webbing")
    st.block((0.3, 0.2, 0.16), (0, -0.06, 0.4), mat=team, parent=j["spine"], bevel=0.03, name="pack")
    st.block((0.3, 0.2, 0.18), (0, -0.04, -0.02), mat=cloth, parent=j["pelvis"], bevel=0.04, name="hips")
    st.sphere(0.11, (0, 0, 0.1), mat=skin, parent=j["head"], name="face")
    st.sphere(0.14, (0, -0.01, 0.17), mat=cloth, parent=j["head"], scale=(1, 1.05, 0.75), name="helmet")
    st.cylinder(0.15, 0.08, (0, -0.01, 0.15), mat=team, parent=j["head"], verts=16, name="helmet_band")
    # Arms: upper arm on the shoulder, forearm on the elbow; team patch on each shoulder
    for side in ("l", "r"):
        st.block((0.11, 0.11, 0.3), (0, 0, -0.14), mat=cloth, parent=j[f"shoulder_{side}"], bevel=0.03,
                 name="upper_arm")
        st.block((0.13, 0.13, 0.16), (0, 0, -0.06), mat=team, parent=j[f"shoulder_{side}"], bevel=0.02,
                 name="patch")
        st.block((0.1, 0.1, 0.28), (0, 0, -0.14), mat=cloth, parent=j[f"elbow_{side}"], bevel=0.03, name="forearm")
        st.block((0.13, 0.13, 0.45), (0, 0, -0.22), mat=cloth, parent=j[f"hip_{side}"], bevel=0.03, name="thigh")
        st.block((0.12, 0.12, 0.42), (0, 0, -0.21), mat=cloth, parent=j[f"knee_{side}"], bevel=0.03, name="shin")
        st.block((0.13, 0.26, 0.1), (0, 0.06, -0.45), mat=boots, parent=j[f"knee_{side}"], bevel=0.03, name="boot")
    # Rifle, held along +y, chunky so it reads at sprite size
    st.block((0.09, 0.9, 0.12), (0, 0.15, 0), mat=dark, parent=j["weapon"], bevel=0.02, name="rifle")
    st.block((0.08, 0.2, 0.18), (0, -0.25, -0.04), mat=dark, parent=j["weapon"], bevel=0.02, name="stock")
    st.block((0.05, 0.12, 0.06), (0, 0.15, 0.1), mat=dark, parent=j["weapon"], bevel=0.0, name="sight")
    st.beam((0.18, -0.1, 0.45), (-0.18, 0.1, 0.0), 0.04, dark, j["spine"], "strap")
