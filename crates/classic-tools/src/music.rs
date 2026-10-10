//! Placeholder music for the public `generic` pack, composed and synthesised from code: our own short pieces in the
//! style `plans/rts/audio.md` asks for (driving electronic music in minor keys, analogue-style bass, gated drums and
//! a little metallic percussion for battle; slower, sparser pieces for the calm), with nothing sampled, traced or
//! imitated from any game or composer. `generate` returns the pack's `audio/music/` files, its `audio/music.json`
//! index and `audio/music/provenance.jsonl`; the `sounds` binary writes them with the sounds, and a test checks the
//! committed files still match.
//!
//! Each piece is a few bars of chords in one key, played by a handful of simple instruments: a kick, a snare, hats,
//! a metallic clank, a saw bass, a soft pad and a plucked arpeggio. Pieces for a pool loop seamlessly (notes that
//! ring past the end carry over to the start); the end stingers don't. The files are 4-bit IMA ADPCM, a quarter of
//! the size of PCM, which the engine plays without decoding them up front.
//!
//! As in `sound`, the synth uses only adding, multiplying and dividing, so every machine writes the same bytes.

use crate::art::File;
use crate::art::canvas::Noise;
use crate::sound::{RATE, sine};

/// 2 to the power of n/12, for n from 0 to 11: an equal-tempered semitone ladder within one octave.
const SEMITONES: [f64; 12] = [
    1.0,
    1.059_463_094_359_295_3,
    1.122_462_048_309_373,
    1.189_207_115_002_721,
    1.259_921_049_894_873_2,
    1.334_839_854_170_034_4,
    std::f64::consts::SQRT_2,
    1.498_307_076_876_681_5,
    1.587_401_051_968_199_4,
    1.681_792_830_507_429,
    1.781_797_436_280_678_6,
    1.887_748_625_363_386_8,
];

/// The frequency of MIDI-style note `n` (69 is the A at 440 Hz).
fn hz(n: i32) -> f64 {
    let (octave, step) = ((n - 69).div_euclid(12), (n - 69).rem_euclid(12));
    let mut f = 440.0 * SEMITONES[step as usize];
    for _ in 0..octave.max(0) {
        f *= 2.0;
    }
    for _ in 0..(-octave).max(0) {
        f /= 2.0;
    }
    f
}

/// An instrument.
#[derive(Clone, Copy, PartialEq)]
enum Voice {
    Kick,
    Snare,
    Hat,
    Clank,
    Bass,
    Pad,
    Pluck,
    Bell,
}

/// One note: when it starts (in samples), how long it is held, its pitch and how hard it is played.
struct Note {
    at: usize,
    held: usize,
    key: i32,
    voice: Voice,
    level: f64,
}

/// A chord as semitones above the key's root (the minor scale's degrees), and the bass note's.
type Chord = [i32; 3];

/// One piece: its file name, pool, tempo, bars, key (a MIDI note number for the root), chords (one per bar, in
/// turn), how busy it is, and whether it loops.
struct Piece {
    id: &'static str,
    pool: &'static str,
    bpm: f64,
    bars: usize,
    root: i32,
    chords: &'static [Chord],
    style: Style,
    loops: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    /// A pad, slow arpeggio, a soft pulse.
    Menu,
    /// A pad and sparse bells, bass on the bar.
    Calm,
    /// A pad, a plucked arpeggio and light hats.
    CalmPulse,
    /// Driving bass, full drums, metal, stabs.
    Battle,
    /// The end: one rising chord.
    Won,
    /// The end: a falling minor line.
    Lost,
}

// Minor-key chords as semitones above the root: i, iv, v, III, VI, VII.
const I: Chord = [0, 3, 7];
const IV: Chord = [5, 8, 12];
const V: Chord = [7, 10, 14];
const III: Chord = [3, 7, 10];
const VI: Chord = [8, 12, 15];
const VII: Chord = [10, 14, 17];

const PIECES: &[Piece] = &[
    Piece {
        id: "menu_1",
        pool: "menu",
        bpm: 100.0,
        bars: 16,
        root: 57,
        chords: &[I, VI, III, VII],
        style: Style::Menu,
        loops: true,
    },
    Piece {
        id: "calm_1",
        pool: "calm",
        bpm: 84.0,
        bars: 16,
        root: 50,
        chords: &[I, IV, VI, V],
        style: Style::Calm,
        loops: true,
    },
    Piece {
        id: "calm_2",
        pool: "calm",
        bpm: 90.0,
        bars: 16,
        root: 52,
        chords: &[I, VII, VI, VII],
        style: Style::CalmPulse,
        loops: true,
    },
    Piece {
        id: "battle_1",
        pool: "battle",
        bpm: 124.0,
        bars: 24,
        root: 52,
        chords: &[I, I, VI, VII],
        style: Style::Battle,
        loops: true,
    },
    Piece {
        id: "battle_2",
        pool: "battle",
        bpm: 132.0,
        bars: 24,
        root: 48,
        chords: &[I, VI, VII, V],
        style: Style::Battle,
        loops: true,
    },
    Piece {
        id: "won_1",
        pool: "won",
        bpm: 100.0,
        bars: 3,
        root: 48,
        chords: &[VI, VII, I],
        style: Style::Won,
        loops: false,
    },
    Piece {
        id: "lost_1",
        pool: "lost",
        bpm: 80.0,
        bars: 3,
        root: 45,
        chords: &[I, IV, I],
        style: Style::Lost,
        loops: false,
    },
];

/// The notes of `p`, with the length of a sixteenth note in samples.
fn score(p: &Piece) -> (Vec<Note>, usize) {
    let step = (RATE as f64 * 60.0 / p.bpm / 4.0) as usize;
    let mut notes = Vec::new();
    let mut add = |bar: usize, s: usize, held: usize, key: i32, voice: Voice, level: f64| {
        notes.push(Note { at: (bar * 16 + s) * step, held: held * step, key, voice, level });
    };
    for bar in 0..p.bars {
        let chord = p.chords[bar % p.chords.len()];
        let r = p.root;
        // Every fourth bar the line turns: a fill, or the chord's top note raised.
        let turn = bar % 4 == 3;
        let intro = bar < 2 && p.loops;
        match p.style {
            Style::Menu => {
                for &c in &chord {
                    add(bar, 0, 16, r + c, Voice::Pad, 0.30);
                }
                add(bar, 0, 8, r - 12 + chord[0], Voice::Bass, 0.45);
                add(bar, 8, 8, r - 12 + chord[0], Voice::Bass, 0.35);
                let arp = [chord[0], chord[1], chord[2], chord[1] + 12, chord[2], chord[1]];
                for (i, s) in [0, 3, 6, 8, 11, 14].into_iter().enumerate() {
                    add(bar, s, 3, r + 12 + arp[i], Voice::Pluck, 0.30);
                }
                if !intro {
                    add(bar, 0, 1, 36, Voice::Kick, 0.6);
                    add(bar, 8, 1, 36, Voice::Kick, 0.5);
                    for s in (2..16).step_by(4) {
                        add(bar, s, 1, 0, Voice::Hat, 0.18);
                    }
                }
            }
            Style::Calm => {
                for &c in &chord {
                    add(bar, 0, 16, r + c, Voice::Pad, 0.32);
                }
                add(bar, 0, 16, r - 12 + chord[0], Voice::Bass, 0.30);
                // Bells on a few beats only, a different few each bar.
                let beats: &[usize] = if bar % 2 == 0 { &[0, 6, 10] } else { &[4, 12] };
                for (i, &s) in beats.iter().enumerate() {
                    add(bar, s, 6, r + 24 + chord[(bar + i) % 3], Voice::Bell, 0.25);
                }
            }
            Style::CalmPulse => {
                for &c in &chord {
                    add(bar, 0, 16, r + c, Voice::Pad, 0.28);
                }
                add(bar, 0, 4, r - 12 + chord[0], Voice::Bass, 0.40);
                add(bar, 10, 6, r - 12 + chord[0], Voice::Bass, 0.30);
                for s in (0..16).step_by(2) {
                    let n = chord[(s / 2) % 3] + if s % 8 == 6 { 12 } else { 0 };
                    add(bar, s, 2, r + 12 + n, Voice::Pluck, 0.22);
                }
                if !intro {
                    for s in (4..16).step_by(8) {
                        add(bar, s, 1, 0, Voice::Hat, 0.16);
                    }
                }
            }
            Style::Battle => {
                for &c in &chord {
                    add(bar, 0, 16, r + c, Voice::Pad, 0.18);
                }
                // A driving bass in sixteenths, octave jumps on the off-beats.
                for s in 0..16 {
                    let up = if s % 4 == 2 { 12 } else { 0 };
                    let gap = s % 8 == 7;
                    if !gap {
                        add(bar, s, 1, r - 24 + chord[0] + up, Voice::Bass, if s % 4 == 0 { 0.55 } else { 0.38 });
                    }
                }
                if intro {
                    for s in (0..16).step_by(2) {
                        add(bar, s, 1, 0, Voice::Hat, 0.15);
                    }
                    continue;
                }
                for s in [0, 4, 8, 10, 12] {
                    add(bar, s, 1, 36, Voice::Kick, if s % 4 == 0 { 0.8 } else { 0.55 });
                }
                add(bar, 4, 1, 0, Voice::Snare, 0.55);
                add(bar, 12, 1, 0, Voice::Snare, 0.6);
                for s in 0..16 {
                    add(bar, s, 1, 0, Voice::Hat, if s % 2 == 0 { 0.2 } else { 0.12 });
                }
                add(bar, 6, 1, 0, Voice::Clank, 0.22);
                add(bar, 14, 1, 0, Voice::Clank, 0.16);
                if turn {
                    for s in [13, 14, 15] {
                        add(bar, s, 1, 0, Voice::Snare, 0.35 + 0.08 * (s - 13) as f64);
                    }
                } else {
                    // Stabs on the chord, off the beat.
                    for &s in &[3usize, 11] {
                        for &c in &chord {
                            add(bar, s, 1, r + 12 + c, Voice::Pluck, 0.16);
                        }
                    }
                }
            }
            Style::Won => {
                for &c in &chord {
                    add(
                        bar,
                        0,
                        if bar == 2 { 32 } else { 16 },
                        r + 12 + c + if c == 3 && bar == 2 { 1 } else { 0 },
                        Voice::Pad,
                        0.35,
                    );
                }
                add(bar, 0, 16, r + chord[0], Voice::Bass, 0.45);
                for (i, s) in [0, 4, 8, 12].into_iter().enumerate() {
                    let up = if bar == 2 { [0, 4, 7, 12][i] } else { chord[i % 3] };
                    add(bar, s, 4, r + 24 + up, Voice::Bell, 0.3);
                }
                add(bar, 0, 1, 36, Voice::Kick, 0.6);
            }
            Style::Lost => {
                for &c in &chord {
                    add(bar, 0, if bar == 2 { 32 } else { 16 }, r + 12 + c, Voice::Pad, 0.33);
                }
                add(bar, 0, 16, r - 12 + chord[0], Voice::Bass, 0.4);
                for (i, s) in [0, 6, 12].into_iter().enumerate() {
                    add(bar, s, 6, r + 24 + [7, 3, 0][i] - bar as i32, Voice::Bell, 0.26);
                }
            }
        }
    }
    (notes, step)
}

/// How long a note rings after it is let go, in samples.
fn release(v: Voice) -> usize {
    let s = match v {
        Voice::Kick => 0.25,
        Voice::Snare => 0.2,
        Voice::Hat => 0.05,
        Voice::Clank => 0.15,
        Voice::Bass => 0.06,
        Voice::Pad => 0.6,
        Voice::Pluck => 0.25,
        Voice::Bell => 1.2,
    };
    (s * RATE as f64) as usize
}

fn white(noise: &mut Noise) -> f64 {
    (noise.roll() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// Synthesise one note into `out` from its start, `len` samples.
fn play(n: &Note, out: &mut [f64], noise: &mut Noise) {
    let dt = 1.0 / RATE as f64;
    let f = hz(n.key);
    let (mut ph, mut ph2, mut ph3) = (0.0f64, 0.0f64, 0.0f64);
    let (mut low, mut low2) = (0.0f64, 0.0f64);
    let mut env = 1.0f64;
    let held = n.held.max(1) as f64;
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f64 * dt;
        // After the note is let go, a short fall to nothing.
        let gate = if (i as f64) < held { 1.0 } else { 1.0 - (i as f64 - held) / release(n.voice).max(1) as f64 };
        let gate = gate.max(0.0);
        let s = match n.voice {
            Voice::Kick => {
                // A falling sine thump.
                let freq = 45.0 + 80.0 * (1.0 - t / 0.08).max(0.0);
                ph += freq * dt;
                env *= 1.0 - dt / 0.11;
                sine(ph) * env
            }
            Voice::Snare => {
                env *= 1.0 - dt / 0.07;
                ph += 190.0 * dt;
                let w = white(noise);
                low += 0.5 * (w - low);
                (low * 0.8 + sine(ph) * 0.35 * (1.0 - t / 0.05).max(0.0)) * env
            }
            Voice::Hat => {
                env *= 1.0 - dt / 0.018;
                let w = white(noise);
                low += 0.3 * (w - low);
                (w - low) * env
            }
            Voice::Clank => {
                // Two squares multiplied: an inharmonic, metallic ring.
                env *= 1.0 - dt / 0.05;
                ph += 1530.0 * dt;
                ph2 += 2210.0 * dt;
                let a = if ph - ph.floor() < 0.5 { 1.0 } else { -1.0 };
                let b = if ph2 - ph2.floor() < 0.5 { 1.0 } else { -1.0 };
                a * b * env * 0.7
            }
            Voice::Bass => {
                ph += f * dt;
                let saw = 2.0 * (ph - ph.floor()) - 1.0;
                // The filter opens on each note and closes as it goes.
                let cut = 0.05 + 0.25 * (1.0 - t / 0.15).max(0.0);
                low += cut * (saw - low);
                low2 += cut * (low - low2);
                low2 * 1.6
            }
            Voice::Pad => {
                // Three saws a little out of tune, darkened, swelling in.
                ph += f * dt;
                ph2 += f * 1.004 * dt;
                ph3 += f * 0.997 * dt;
                let saw = |p: f64| 2.0 * (p - p.floor()) - 1.0;
                let sum = (saw(ph) + saw(ph2) + saw(ph3)) / 3.0;
                low += 0.04 * (sum - low);
                low * (t / 0.35).min(1.0)
            }
            Voice::Pluck => {
                env *= 1.0 - dt / 0.12;
                ph += f * dt;
                // A narrow pulse, its two levels set so it has no offset.
                let sq = if ph - ph.floor() < 0.3 { 0.7 } else { -0.3 };
                low += (0.05 + 0.4 * env) * (sq - low);
                low * env
            }
            Voice::Bell => {
                env *= 1.0 - dt / 0.6;
                ph += f * dt;
                ph2 += f * 2.76 * dt;
                (sine(ph) + 0.3 * sine(ph2) * env) * env
            }
        };
        *o += s * gate * n.level;
    }
}

/// The piece as 16-bit samples.
fn render(p: &Piece) -> Vec<i16> {
    let (notes, step) = score(p);
    let len = p.bars * 16 * step;
    let tail = if p.loops { 0 } else { (1.5 * RATE as f64) as usize };
    let mut mix = vec![0.0f64; len + tail];
    let mut noise = Noise::new(p.id);
    let mut buf = Vec::new();
    for n in &notes {
        buf.clear();
        buf.resize(n.held + release(n.voice), 0.0);
        play(n, &mut buf, &mut noise);
        for (i, s) in buf.iter().enumerate() {
            let at = n.at + i;
            // A loop carries what rings past its end over to its start.
            let at = if p.loops { at % len } else { at };
            if let Some(m) = mix.get_mut(at) {
                *m += s;
            }
        }
    }
    // Take out any offset left over, gently squash the peaks, then bring the loudest to 0.8.
    let mean = mix.iter().sum::<f64>() / mix.len().max(1) as f64;
    for s in &mut mix {
        *s -= mean;
    }
    for s in &mut mix {
        *s /= 1.0 + 0.3 * s.abs();
    }
    if !p.loops {
        let fade = (RATE as usize / 2).min(mix.len());
        let n = mix.len();
        for k in 0..fade {
            mix[n - 1 - k] *= k as f64 / fade as f64;
        }
    }
    let peak = mix.iter().fold(0.0f64, |m, s| m.max(s.abs())).max(1e-9);
    mix.iter().map(|s| (s / peak * 0.8 * 32767.0).round() as i16).collect()
}

/// The IMA ADPCM step table and how the step moves, from the format's public description.
const INDEX_MOVE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107,
    118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894,
    6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

/// Encode mono 16-bit samples as a WAV file of IMA ADPCM in 1024-byte blocks (the engine reads it with
/// `rts_platform::wav::decode_track`).
pub fn adpcm_wav(samples: &[i16]) -> Vec<u8> {
    const ALIGN: usize = 1024;
    let per_block = (ALIGN - 4) * 2 + 1;
    let mut data = Vec::new();
    let mut index = 0i32;
    for block in samples.chunks(per_block) {
        let mut pred = block[0] as i32;
        data.extend_from_slice(&block[0].to_le_bytes());
        data.extend_from_slice(&[index as u8, 0]);
        let mut codes = Vec::with_capacity(per_block - 1);
        for i in 1..per_block {
            let s = block.get(i).copied().unwrap_or(0) as i32;
            let step = STEPS[index as usize];
            let mut d = s - pred;
            let mut code = 0u8;
            if d < 0 {
                code = 8;
                d = -d;
            }
            let mut part = step;
            for bit in [4u8, 2, 1] {
                if d >= part {
                    code |= bit;
                    d -= part;
                }
                part >>= 1;
            }
            // Step the predictor exactly as a decoder will.
            let mut diff = step >> 3;
            if code & 1 != 0 {
                diff += step >> 2;
            }
            if code & 2 != 0 {
                diff += step >> 1;
            }
            if code & 4 != 0 {
                diff += step;
            }
            pred = if code & 8 != 0 { pred - diff } else { pred + diff }.clamp(-32768, 32767);
            index = (index + INDEX_MOVE[(code & 7) as usize]).clamp(0, 88);
            codes.push(code);
        }
        for pair in codes.chunks(2) {
            data.push(pair[0] | pair.get(1).copied().unwrap_or(0) << 4);
        }
    }
    let mut b = Vec::with_capacity(60 + data.len());
    let byte_rate = RATE as usize * ALIGN / per_block;
    b.extend_from_slice(b"RIFF");
    // The size after this field: "WAVE", then the fmt (8 + 20), fact (8 + 4) and data chunks.
    b.extend_from_slice(&(4 + 28 + 12 + 8 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&20u32.to_le_bytes());
    b.extend_from_slice(&0x11u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(byte_rate as u32).to_le_bytes());
    for v in [ALIGN as u16, 4, 2, per_block as u16] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(b"fact");
    b.extend_from_slice(&4u32.to_le_bytes());
    b.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&data);
    b
}

/// Every music file of the generic pack: the tracks, `audio/music.json` and `audio/music/provenance.jsonl`.
pub fn generate() -> Vec<File> {
    let mut files = Vec::new();
    let mut index = Vec::new();
    let mut provenance = String::new();
    for p in PIECES {
        let path = format!("audio/music/{}.wav", p.id);
        files.push(File { path: path.clone(), bytes: adpcm_wav(&render(p)) });
        index.push(format!("    \"{}\": {{ \"file\": \"{path}\", \"pool\": \"{}\", \"volume\": 100 }}", p.id, p.pool));
        provenance.push_str(&format!(
            "{{\"file\": \"{path}\", \"source\": \"original\", \"made_by\": \"composed and synthesised from code: crates/classic-tools/src/music.rs\", \"licence\": \"MIT\"}}\n"
        ));
    }
    let index = format!(
        "{{\n  \"about\": \"Placeholder music for the generic pack, composed and synthesised from code by crates/classic-tools/src/music.rs (run: cargo run --bin sounds). Each track names its file, its pool (menu, calm, battle, won or lost) and its volume from 0 to 100 (0 leaves it out). How the music follows the game is the engine's (data/audio/music.json).\",\n  \"tracks\": {{\n{}\n  }}\n}}\n",
        index.join(",\n")
    );
    files.push(File { path: "audio/music.json".into(), bytes: index.into_bytes() });
    files.push(File { path: "audio/music/provenance.jsonl".into(), bytes: provenance.into_bytes() });
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_in_tune() {
        assert_eq!(hz(69), 440.0);
        assert_eq!(hz(57), 220.0);
        assert!((hz(60) - 261.6256).abs() < 0.001);
    }

    #[test]
    fn every_pool_has_music() {
        for pool in ["menu", "calm", "battle", "won", "lost"] {
            assert!(PIECES.iter().any(|p| p.pool == pool), "{pool}");
        }
    }
}
