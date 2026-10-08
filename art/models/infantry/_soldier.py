"""Shared parts and cycles for batch I (infantry, rocket_infantry, elite_infantry, guerrilla, saboteur).

Built on the studio's rig (`st.rig`, twelve joints, rest pose standing and facing +y, built at 1.75x). Each model
picks a head, clothing, kit and a weapon here, and says how it holds the weapon (`hold`) and how it fires
(`stance`); `pose` is the tuned version of the studio prototype's cycles (art/studio/samples/soldier.py), with
those two choices added.

Proportions are chunkier than a real person on purpose: a figure is about 15 px tall at zoom 1, so the head,
helmet, shoulders and weapon are 15 to 30% oversize to read at that size. Every size is in rest-pose metres
before the rig's 1.75x.

Rotation signs on the rig (radians about x): positive on a hip or shoulder swings the limb forward (+y), negative
on a knee bends the shin back, positive on the spine or body leans back, positive on the weapon lifts the muzzle.
"""
import math

import rts_studio as st

UPPER_LEG, LOWER_LEG = 0.43, 0.5  # hip to knee; knee to the sole of the boot
HIP_DROP = 0.05  # pelvis to hip joint

# ---------- materials ----------
# Cloth stays well clear of the team hue band (0.2 to 0.45): sand, khaki, brown and grey only.


def fatigues(name="fatigues", base=(0.12, 0.09, 0.05)):
    return st._noisy(name, base, 14, 4, 0.3, 0.85, bump=0.2)


def skin():
    return st.plain("skin", (0.4, 0.26, 0.18), roughness=0.7)


# ---------- body ----------


def body(j, cloth, trousers=None, boots=None, limb=0.14):
    """Torso, hips, arms and legs. Returns nothing; heads, kit and weapons are added separately."""
    trousers = trousers or cloth
    boots = boots or st.rubber()
    st.block((0.44, 0.27, 0.5), (0, 0, 0.25), mat=cloth, parent=j["spine"], bevel=0.06, name="torso")
    st.block((0.35, 0.24, 0.2), (0, -0.01, -0.03), mat=trousers, parent=j["pelvis"], bevel=0.05, name="hips")
    st.sphere(0.085, (0, 0, 0.47), mat=cloth, parent=j["spine"], scale=(1.2, 1, 0.8), name="neck")
    for side, sx in (("l", -1), ("r", 1)):
        st.sphere(limb * 0.62, (0, 0, -0.02), mat=cloth, parent=j[f"shoulder_{side}"], name="shoulder")
        st.block((limb * 0.85, limb * 0.85, 0.3), (0, 0, -0.14), mat=cloth, parent=j[f"shoulder_{side}"],
                 bevel=0.035, name="upper_arm")
        st.block((limb * 0.8, limb * 0.8, 0.27), (0, 0, -0.13), mat=cloth, parent=j[f"elbow_{side}"],
                 bevel=0.035, name="forearm")
        st.sphere(0.06, (0, 0.01, -0.29), mat=skin(), parent=j[f"elbow_{side}"], name="hand")
        st.block((limb, limb * 1.05, 0.45), (0, 0, -0.22), mat=trousers, parent=j[f"hip_{side}"], bevel=0.04,
                 name="thigh")
        st.block((limb * 0.9, limb * 0.95, 0.4), (0, 0, -0.2), mat=trousers, parent=j[f"knee_{side}"],
                  bevel=0.035, name="shin")
        st.block((limb * 1.05, 0.27, 0.11), (0, 0.055, -LOWER_LEG + 0.055), mat=boots, parent=j[f"knee_{side}"],
                 bevel=0.035, name="boot")


def face(j):
    st.sphere(0.12, (0, 0.01, 0.12), mat=skin(), parent=j["head"], scale=(0.95, 1, 1.05), name="face")


def helmet(j, cloth, team, visor=False):
    """A round combat helmet with a team band. `visor` adds a dark glass visor (elite)."""
    face(j)
    st.sphere(0.16, (0, -0.01, 0.17), mat=cloth, parent=j["head"], scale=(1, 1.08, 0.78), name="helmet")
    st.cylinder(0.168, 0.07, (0, -0.01, 0.14), mat=team, parent=j["head"], verts=20, name="helmet_band")
    if visor:
        st.block((0.22, 0.06, 0.08), (0, 0.11, 0.1), mat=st.glass(), parent=j["head"], bevel=0.02, name="visor")


def head_wrap(j, cloth, team):
    """A cloth head wrap with a scarf across the lower face and a team-colour band (guerrilla)."""
    face(j)
    st.sphere(0.145, (0, -0.02, 0.17), mat=cloth, parent=j["head"], scale=(1, 1.05, 0.85), name="wrap")
    st.cylinder(0.15, 0.06, (0, -0.02, 0.2), mat=team, parent=j["head"], verts=20, name="wrap_band")
    st.block((0.22, 0.1, 0.09), (0, 0.07, 0.05), mat=cloth, parent=j["head"], bevel=0.035, name="scarf")
    st.block((0.12, 0.08, 0.22), (0, -0.14, 0.08), mat=cloth, parent=j["head"], bevel=0.03, name="wrap_tail")


def knit_cap(j, cloth):
    """A close knit cap (saboteur)."""
    face(j)
    st.sphere(0.135, (0, -0.01, 0.19), mat=cloth, parent=j["head"], scale=(1, 1.05, 0.7), name="cap")


def vest(j, mat, team, plates=False):
    """Load vest over the torso, with team panels on the chest. `plates` makes it an armoured carrier."""
    st.block((0.47, 0.31, 0.24), (0, 0, 0.21), mat=mat, parent=j["spine"], bevel=0.05, name="vest")
    st.block((0.3, 0.04, 0.14), (0, 0.165, 0.27), mat=team, parent=j["spine"], bevel=0.015, name="chest_panel")
    if plates:
        st.block((0.5, 0.33, 0.12), (0, 0, 0.38), mat=mat, parent=j["spine"], bevel=0.04, name="collar")
        for side in ("l", "r"):
            st.block((0.2, 0.22, 0.09), (0, 0, 0.07), mat=team, parent=j[f"shoulder_{side}"], bevel=0.035,
                     name="pauldron")
            st.block((0.17, 0.08, 0.17), (0, 0.06, -0.18), mat=mat, parent=j[f"hip_{side}"], bevel=0.03,
                     name="knee_pad")


def patches(j, team):
    """Team patches on both upper arms."""
    for side, sx in (("l", -1), ("r", 1)):
        st.block((0.135, 0.135, 0.13), (0, 0, -0.07), mat=team, parent=j[f"shoulder_{side}"], bevel=0.02,
                 name="patch")


def backpack(j, mat, team, size=(0.32, 0.2, 0.32), flap=True):
    w, d, h = size
    st.block(size, (0, -0.135 - d / 2, 0.3), mat=mat, parent=j["spine"], bevel=0.05, name="pack")
    if flap:
        st.block((w * 0.85, d * 0.6, 0.08), (0, -0.135 - d / 2, 0.3 + h / 2), mat=team, parent=j["spine"],
                 bevel=0.02, name="pack_flap")


def belt(j, mat):
    st.block((0.37, 0.27, 0.07), (0, 0, 0.05), mat=mat, parent=j["pelvis"], bevel=0.02, name="belt")
    for x in (-0.12, 0.12):
        st.block((0.09, 0.07, 0.08), (x, 0.14, 0.04), mat=mat, parent=j["pelvis"], bevel=0.02, name="pouch")


# ---------- weapons ----------
# All held along +y about the weapon joint (the grip), chunky so they read at sprite size. Each puts a `muzzle`
# empty at its barrel tip: the studio records where it lands in every frame (muzzle flashes in 2D) and a glTF
# export keeps it as a node under the weapon joint (where shots start in 3D).


def muzzle(j, y, z=0.0):
    """The barrel tip, `y` metres ahead of the grip along the weapon: an empty, so it renders as nothing."""
    return st.group("muzzle", (0, y, z), parent=j["weapon"])


def rifle(j, mat, length=0.9):
    w = j["weapon"]
    st.block((0.09, length, 0.12), (0, 0.15, 0), mat=mat, parent=w, bevel=0.02, name="rifle")
    st.block((0.08, 0.22, 0.17), (0, -0.28, -0.03), mat=mat, parent=w, bevel=0.02, name="stock")
    st.block((0.07, 0.08, 0.16), (0, 0.12, -0.11), mat=mat, parent=w, bevel=0.015, name="magazine")
    st.block((0.05, 0.12, 0.06), (0, 0.12, 0.09), mat=mat, parent=w, bevel=0.0, name="sight")
    muzzle(j, 0.15 + length / 2)


def heavy_rifle(j, mat):
    """A bulkier support weapon: thick barrel shroud and a drum magazine (elite)."""
    w = j["weapon"]
    st.block((0.12, 1.0, 0.15), (0, 0.18, 0), mat=mat, parent=w, bevel=0.025, name="rifle")
    st.block((0.1, 0.24, 0.19), (0, -0.3, -0.03), mat=mat, parent=w, bevel=0.02, name="stock")
    st.cylinder(0.11, 0.1, (0, 0.08, -0.12), rot=(0, math.pi / 2, 0), mat=mat, parent=w, verts=16, name="drum")
    st.block((0.06, 0.16, 0.08), (0, 0.1, 0.11), mat=mat, parent=w, bevel=0.0, name="sight")
    muzzle(j, 0.68)


def compact_gun(j, mat):
    """A short machine pistol (saboteur)."""
    w = j["weapon"]
    st.block((0.08, 0.5, 0.12), (0, 0.12, 0), mat=mat, parent=w, bevel=0.02, name="rifle")
    st.block((0.06, 0.08, 0.18), (0, 0.05, -0.12), mat=mat, parent=w, bevel=0.015, name="magazine")
    muzzle(j, 0.37)


def launcher(j, mat, team):
    """A shoulder rocket tube: fat tube, flared rear, a boxy sight and a team band (rocket_infantry)."""
    w = j["weapon"]
    st.cylinder(0.095, 1.25, (0, 0.12, 0), rot=(math.pi / 2, 0, 0), mat=mat, parent=w, verts=16, name="tube")
    st.cone(0.13, 0.1, 0.16, (0, -0.55, 0), rot=(math.pi / 2, 0, 0), mat=mat, parent=w, verts=16, name="flare")
    st.cylinder(0.105, 0.12, (0, 0.6, 0), rot=(math.pi / 2, 0, 0), mat=team, parent=w, verts=16, name="tube_band")
    st.block((0.08, 0.16, 0.12), (-0.13, 0.22, 0.04), mat=mat, parent=w, bevel=0.02, name="sight")
    st.block((0.06, 0.08, 0.16), (0, 0.18, -0.13), mat=mat, parent=w, bevel=0.015, name="grip")
    muzzle(j, 0.745)  # the tube's front end; the rocket starts here


# ---------- cycles ----------

HOLDS = {
    # The aim: weapon joint location and muzzle lift in the spine's space, and arm angles (shoulder_r, elbow_r,
    # shoulder_l, elbow_l, shoulder_l turned in), set so the right hand sits on the grip and the left under the
    # fore-end.
    "rifle": {"weapon": (0.12, 0.28, 0.3), "lift": 0.0, "arms": (0.55, 1.35, 1.0, 0.6, -0.55)},
    "shoulder": {"weapon": (0.2, 0.0, 0.5), "lift": 0.04, "arms": (0.35, 1.9, 0.95, 0.85, -0.5)},
    "compact": {"weapon": (0.08, 0.36, 0.34), "lift": 0.0, "arms": (0.75, 0.95, 1.05, 0.55, -0.6)},
}
# The relaxed carry for idle and walk: weapon low across the body, muzzle a little up.
CARRY = {
    "rifle": {"weapon": (0.08, 0.22, 0.1), "lift": 0.3, "arms": (0.15, 1.2, 0.7, 0.8, -0.6)},
    "shoulder": {"weapon": (0.2, -0.02, 0.5), "lift": 0.25, "arms": (0.3, 1.9, 0.45, 1.1, -0.3)},
    "compact": {"weapon": (0.1, 0.22, 0.1), "lift": 0.35, "arms": (0.15, 1.2, 0.2, 0.6, -0.2)},
}


def _hold(j, h):
    sr, er, sl, el, splay = h["arms"]
    j["shoulder_r"].rotation_euler.x = sr
    j["elbow_r"].rotation_euler.x = er
    j["shoulder_l"].rotation_euler.x = sl
    j["elbow_l"].rotation_euler.x = el
    j["shoulder_l"].rotation_euler.z = splay
    j["weapon"].location = h["weapon"]
    j["weapon"].rotation_euler.x = h["lift"]


def _leg_reach(a, k):
    """Height from the pelvis joint down to the sole with the hip at `a` and the knee at `k`."""
    return HIP_DROP + UPPER_LEG * math.cos(a) + LOWER_LEG * math.cos(a + k)


def _legs(j, al, kl, ar, kr, drop=0.0):
    """Set both legs and stand the pelvis on the lower foot, so the figure neither floats nor sinks."""
    j["hip_l"].rotation_euler.x, j["knee_l"].rotation_euler.x = al, kl
    j["hip_r"].rotation_euler.x, j["knee_r"].rotation_euler.x = ar, kr
    j["pelvis"].location = (0, 0, max(_leg_reach(al, kl), _leg_reach(ar, kr)) - drop)


def _ease(t):
    return 0.5 - 0.5 * math.cos(math.pi * min(max(t, 0.0), 1.0))


def pose(joints, anim, frame, frames, hold="rifle", stance="kneel"):
    """Set the rig to `frame` of `anim`. Each frame is a function of the frame alone, so it always renders the
    same. `hold` is a key of HOLDS; `stance` is "kneel" or "stand" for the fire cycle."""
    j = joints
    for name, o in j.items():
        o.rotation_euler = (0, 0, 0)
        o.location = (0, 0, 0) if name == "body" else st.JOINTS[name][1]
    p = frame / frames
    if anim == "idle":
        _hold(j, CARRY[hold])
        _legs(j, 0.06, -0.08, -0.06, -0.05)
        j["spine"].rotation_euler.x = -0.04
        j["head"].rotation_euler.z = 0.15
    elif anim == "walk":
        # Six frames, contact on 0 and 3. The leg swinging forward lifts its knee; the torso leans in and rocks
        # against the hips; the pelvis rides on the stance leg, so it bobs at passing.
        phi = 2 * math.pi * p
        s, c = math.sin(phi), math.cos(phi)
        _hold(j, CARRY[hold])
        al = 0.55 * s
        kl = -(0.12 + 0.95 * max(0.0, c) * max(0.0, 1 - abs(s)) + 0.35 * max(0.0, c))
        kr = -(0.12 + 0.95 * max(0.0, -c) * max(0.0, 1 - abs(s)) + 0.35 * max(0.0, -c))
        _legs(j, al, kl, -al, kr)
        j["spine"].rotation_euler.x = -0.14
        j["spine"].rotation_euler.z = 0.1 * s
        j["pelvis"].rotation_euler.z = -0.08 * s
        j["shoulder_r"].rotation_euler.x += 0.06 * s
        j["shoulder_l"].rotation_euler.x += 0.06 * s
    elif anim == "fire":
        # Aim, recoil, settle. The weapon kicks back and up and the shoulders follow.
        _hold(j, HOLDS[hold])
        kick = (0.0, 1.0, 0.35)[min(frame, 2)]
        if stance == "kneel":
            _legs(j, 1.45, -1.5, -0.25, -1.3, drop=0.03)
            j["pelvis"].location.z = 0.6
        else:
            _legs(j, 0.25, -0.3, -0.3, -0.1)
            j["hip_l"].rotation_euler.y = 0.12
            j["hip_r"].rotation_euler.y = -0.12
        j["spine"].rotation_euler.x = -0.08 + 0.12 * kick
        wx, wy, wz = HOLDS[hold]["weapon"]
        j["weapon"].location = (wx, wy - 0.1 * kick, wz + 0.02 * kick)
        j["weapon"].rotation_euler.x = HOLDS[hold]["lift"] + 0.18 * kick
        j["head"].rotation_euler.x = 0.06 * kick
    elif anim in ("die-1", "die-2"):
        # die-1 is thrown back, die-2 crumples forward; both end flat on the ground, weapon dropped.
        t = _ease(p * frames / (frames - 1))
        back = anim == "die-1"
        knees = _ease(min(1.0, 2 * t))  # the knees go first
        _hold(j, HOLDS[hold])
        lean = (1.5 if back else -1.5) * t
        j["body"].rotation_euler.x = lean
        j["body"].location = (0, 0, 0.26 * t)
        if back:
            j["shoulder_r"].rotation_euler.x = 1.35 * (1 - t) + 2.7 * t
            j["shoulder_l"].rotation_euler.x = 1.25 * (1 - t) + 2.3 * t
            j["shoulder_l"].rotation_euler.z = -0.35 - 0.6 * t
            j["elbow_r"].rotation_euler.x = 0.25 + 0.5 * t
            j["hip_l"].rotation_euler.x, j["knee_l"].rotation_euler.x = 0.5 * knees * (1 - t) + 0.2 * t, -0.9 * knees
            j["hip_r"].rotation_euler.x, j["knee_r"].rotation_euler.x = 0.1 * t, -0.3 * knees
            j["spine"].rotation_euler.x = -0.15 * t
            j["head"].rotation_euler.x = -0.35 * t
        else:
            j["shoulder_r"].rotation_euler.x = 1.35 * (1 - t) - 0.3 * t
            j["shoulder_l"].rotation_euler.x = 1.25 * (1 - t) + 0.4 * t
            j["shoulder_l"].rotation_euler.z = -0.35 - 0.5 * t
            j["hip_l"].rotation_euler.x, j["knee_l"].rotation_euler.x = 0.7 * knees * (1 - t) + 0.25 * t, -1.1 * knees * (1 - t) - 0.5 * t
            j["hip_r"].rotation_euler.x, j["knee_r"].rotation_euler.x = 0.2 * knees * (1 - t), -0.6 * knees * (1 - t)
            j["spine"].rotation_euler.x = 0.25 * t
            j["head"].rotation_euler.z = 0.9 * t
        # The weapon slides out of the hands and lies flat beside the body: it turns against the body's fall, or a
        # soldier on his back would hold it pointing at the sky (hidden from the sprite camera, plain in 3D).
        drop = _ease(2 * t - 1)
        wx, wy, wz = HOLDS[hold]["weapon"]
        fall = j["body"].rotation_euler.x + j["spine"].rotation_euler.x
        j["weapon"].location = (wx + 0.25 * drop, wy, wz - 0.12 * drop)
        j["weapon"].rotation_euler.x = HOLDS[hold]["lift"] * (1 - drop) - fall * drop
