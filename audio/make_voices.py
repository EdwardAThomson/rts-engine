"""Speak the engine's own lines with a free text-to-speech model, as the generic pack's placeholder voices.

    python3 audio/make_voices.py --models DIR             # write settings/generic/audio/voices/ and voices.json
    python3 audio/make_voices.py --models DIR --preview D # also write every take end to end to D/generic.wav

The lines are the engine's: the feed's words for the advisor (data/ui/messages.json) and the units' replies
(data/ui/lines.json `acks`). Every faction of the generic pack shares one cast. The generic pack's voices only
speak lines a pack left in these words (crates/classic-render/src/sound.rs), and a pack with voices of its own
replaces them whole.

A spoken line can't know which building or unit `{name}` is, so the advisor says each line its own way (SAY): the
feed's meaning, with a plain word for the name. SAY also keeps the voice clear of stock phrases from other games.
The unit replies are spoken as written.

The model is Kokoro-82M (Apache-2.0 weights) run through kokoro-onnx (MIT), which phonemises with espeak-ng. The
tools are only run here, never shipped, and the audio they make is ours (MIT, like the rest of the pack). DIR holds
the two model files from the kokoro-onnx releases on GitHub (`model-files-v1.0`): kokoro-v1.0.onnx and
voices-v1.0.bin. Unit replies get a light radio band-pass (300 Hz to 3.5 kHz) and mild saturation; every take is
trimmed, faded and levelled.

Needs: pip install kokoro-onnx soundfile numpy scipy (in a virtual environment). Same files every run.
"""
import argparse
import json
import sys
import wave
from math import gcd
from pathlib import Path

import numpy as np
from scipy.signal import butter, resample_poly, sosfilt

ROOT = Path(__file__).resolve().parent.parent
PACK = ROOT / "settings" / "generic"
VOICES_DIR = "audio/voices"
RATE = 22050  # the pack's other sounds' rate
PEAK = 0.85

# Stock Kokoro voices: (voice, speed, language, radio).
CAST = {
    "advisor": ("af_heart", 1.0, "en-us", False),
    "infantry": ("am_puck", 1.05, "en-us", True),
    "vehicle": ("am_liam", 1.0, "en-us", True),
}

# What the advisor says for each message id: the feed's meaning, without `{name}`.
SAY = {
    "building_ready": "Building ready to place.",
    "unit_ready": "New unit ready.",
    "cannot_place": "That can't be placed there.",
    "queue_full": "The queue is full.",
    "needs_building": "That needs another building first.",
    "no_credits": "Not enough credits. Production is on hold.",
    "low_power": "Power is low.",
    "power_restored": "Power restored.",
    "base_attacked": "The base is taking fire.",
    "units_attacked": "Our units are taking fire.",
    "building_lost": "We lost a building.",
    "unit_lost": "We lost a unit.",
    "hazard_ate": "The hazard took one of ours.",
    "harvester_no_resource": "A harvester is idle. Its field is empty.",
    "harvester_no_refinery": "A harvester is idle. There's no refinery.",
    "harvester_attacked": "A harvester is under fire.",
    "hazard_sighted": "Hazard sighted.",
    "storage_full": "Storage is full. Build more silos.",
    "repairing": "Starting repairs.",
    "repaired": "Repairs finished.",
    "building_sold": "Building sold back.",
    "building_captured": "We've taken an enemy building.",
    "building_taken": "The enemy has taken one of our buildings.",
    "cannot_capture": "That building is too strong to take yet.",
    "starport_ordered": "Order placed. Delivery is on its way.",
    "starport_funds": "Not enough credits for that order.",
    "radar_online": "Radar is online.",
    "radar_offline": "Radar is offline.",
    "starport_out_of_stock": "That unit is out of stock.",
    "starport_refunded": "We lost the starport. The order is refunded.",
    "superpower_ready": "The superpower is ready. Pick a target.",
    "superpower_charging": "The superpower is still charging.",
    "missile_warning": "Warning. Missile launched.",
    "guerrillas_arrived": "Our guerrillas have arrived.",
    "saboteur_ready": "The saboteur is ready.",
    "enemy_wave": "Enemy forces are closing in.",
    "game_won": "The battle is won.",
    "game_lost": "The battle is lost.",
}


def finish(x, sr, radio):
    """Resample to RATE, trim silence, band-limit and saturate for radio, level, and fade the ends."""
    g = gcd(RATE, sr)
    x = resample_poly(np.asarray(x, dtype=np.float64), RATE // g, sr // g)
    loud = np.flatnonzero(np.abs(x) > 0.02 * np.max(np.abs(x)))
    if len(loud):
        a, b = max(loud[0] - int(0.02 * RATE), 0), min(loud[-1] + int(0.06 * RATE), len(x))
        x = x[a:b]
    if radio:
        x = sosfilt(butter(4, [300, 3500], btype="band", fs=RATE, output="sos"), x)
        x = np.tanh(1.6 * x / np.max(np.abs(x))) / np.tanh(1.6)
    x = x / np.max(np.abs(x)) * PEAK
    fade = min(len(x) // 2, int(0.01 * RATE))
    x[:fade] *= np.linspace(0, 1, fade)
    x[len(x) - fade:] *= np.linspace(1, 0, fade)
    return x


def write_wav(path, x):
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes((np.clip(x, -1, 1) * 32767).astype("<i2").tobytes())


def texts(v):
    return [v] if isinstance(v, str) else list(v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", required=True, help="folder with kokoro-v1.0.onnx and voices-v1.0.bin")
    ap.add_argument("--preview", help="also write every take end to end here")
    args = ap.parse_args()
    from kokoro_onnx import Kokoro

    models = Path(args.models)
    kokoro = Kokoro(str(models / "kokoro-v1.0.onnx"), str(models / "voices-v1.0.bin"))
    messages = json.loads((ROOT / "data/ui/messages.json").read_text())["messages"]
    acks = json.loads((ROOT / "data/ui/lines.json").read_text())["acks"]
    missing = sorted(set(messages) - set(SAY))
    if missing:
        sys.exit(f"no SAY line for {missing}")

    # (who speaks, key, texts): the advisor by message id, units by voice set and moment.
    jobs = [("advisor", key, [SAY[key]]) for key in messages]
    for voice_set, moments in acks.items():
        jobs += [(voice_set, moment, texts(v)) for moment, v in moments.items()]
    cast, provenance, preview = {}, [], []
    for who, key, said in jobs:
        voice, speed, lang, radio = CAST[who]
        files = []
        for i, words in enumerate(said, 1):
            samples, sr = kokoro.create(words, voice=voice, speed=speed, lang=lang)
            x = finish(samples, sr, radio)
            rel = f"{VOICES_DIR}/{who}/{key}_{i}.wav"
            write_wav(PACK / rel, x)
            files.append(rel)
            provenance.append({
                "file": rel,
                "source": "generated",
                "made_by": f"audio/make_voices.py: Kokoro-82M (Apache-2.0) via kokoro-onnx (MIT), voice {voice}",
                "licence": "MIT",
                "text": words,
            })
            preview += [x, np.zeros(int(0.35 * RATE))]
            print(f"{rel}: {len(x) / RATE:.2f} s  {words!r}")
        cast.setdefault(who, {})[key] = files
    factions = [f["id"] for f in json.loads((PACK / "setting.json").read_text())["factions"]]
    (PACK / "audio/voices.json").write_text(json.dumps({
        "about": "The generic pack's placeholder voices for the engine's own lines (data/ui/messages.json for the "
                 "advisor, data/ui/lines.json for the units), spoken by audio/make_voices.py. Every faction shares "
                 "one cast. By faction, then `advisor` by message id or a unit voice set by moment; the n-th file "
                 "speaks the n-th line.",
        "voices": {f: cast for f in factions},
    }, indent=2) + "\n")
    (PACK / VOICES_DIR / "provenance.jsonl").write_text("".join(json.dumps(p) + "\n" for p in provenance))
    if args.preview:
        write_wav(Path(args.preview) / "generic.wav", np.concatenate(preview))
    print(f"{len(provenance)} takes", file=sys.stderr)


if __name__ == "__main__":
    main()
