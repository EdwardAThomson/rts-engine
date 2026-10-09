//! Placeholder sounds for the public `generic` pack, synthesised from code: plain tones, noise and sweeps, our
//! own, with nothing sampled, traced or imitated from any game. `generate` returns every file the pack's `audio/`
//! folder holds, plus its `sounds.json` index and `provenance.jsonl`; the `sounds` binary writes them, and a test
//! checks the committed files still match, so the recipes here are the source of truth. (The idea of describing
//! sounds as small recipes of oscillators, noise and envelopes comes from sfxr; the code is written fresh.)
//!
//! The synth uses only adding, multiplying and dividing (no `sin`, `exp` or `powf`, whose last bits can differ
//! between platforms' maths libraries), so every machine writes byte-identical files.

use crate::art::File;
use crate::art::canvas::Noise;

/// Samples per second. Plenty for placeholders, and half the size of 44.1 kHz.
pub const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Saw,
    Noise,
}

/// One voice of a recipe: a wave whose pitch slides from `from` to `to` hertz and whose tone darkens or brightens
/// from `bright` to `bright_end` (a one-pole low-pass, 1 for open), shaped by a short attack and an exponential
/// decay, starting `delay` seconds in.
#[derive(Clone, Copy)]
struct Layer {
    wave: Wave,
    from: f64,
    to: f64,
    bright: f64,
    bright_end: f64,
    attack: f64,
    decay: f64,
    gain: f64,
    delay: f64,
}

const fn layer(wave: Wave, from: f64, to: f64, decay: f64, gain: f64) -> Layer {
    Layer { wave, from, to, bright: 1.0, bright_end: 1.0, attack: 0.002, decay, gain, delay: 0.0 }
}

impl Layer {
    const fn tone(self, bright: f64, bright_end: f64) -> Layer {
        Layer { bright, bright_end, ..self }
    }
    const fn at(self, delay: f64) -> Layer {
        Layer { delay, ..self }
    }
    const fn rise(self, attack: f64) -> Layer {
        Layer { attack, ..self }
    }
}

/// A sound: its id (as `data/audio/sounds.json` names it), length, how many takes, layers, and drive (soft
/// distortion, 0 for clean).
struct Recipe {
    id: &'static str,
    folder: &'static str,
    seconds: f64,
    takes: usize,
    drive: f64,
    layers: &'static [Layer],
}

use Wave::*;

const RECIPES: &[Recipe] = &[
    // A shell gun: a dark noise blast over a falling thump, with a short bright crack on top.
    Recipe {
        id: "sfx_cannon",
        folder: "sfx",
        seconds: 0.5,
        takes: 2,
        drive: 1.5,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.12, 1.0).tone(0.5, 0.04),
            layer(Sine, 120.0, 45.0, 0.14, 0.9),
            layer(Square, 900.0, 250.0, 0.015, 0.25),
        ],
    },
    // A rocket: a whoosh that opens up as it leaves, over a rising buzz.
    Recipe {
        id: "sfx_rocket",
        folder: "sfx",
        seconds: 0.7,
        takes: 2,
        drive: 0.5,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.3, 1.0).tone(0.05, 0.35).rise(0.04),
            layer(Saw, 180.0, 520.0, 0.25, 0.2).tone(0.3, 0.3).rise(0.03),
        ],
    },
    // A shell bursting: a short dull knock.
    Recipe {
        id: "sfx_impact",
        folder: "sfx",
        seconds: 0.3,
        takes: 2,
        drive: 1.0,
        layers: &[layer(Noise, 0.0, 0.0, 0.05, 1.0).tone(0.6, 0.08), layer(Sine, 190.0, 70.0, 0.06, 0.8)],
    },
    // A vehicle blowing up: a long rolling noise over a deep boom.
    Recipe {
        id: "sfx_explode_small",
        folder: "sfx",
        seconds: 1.0,
        takes: 2,
        drive: 2.0,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.28, 1.0).tone(0.4, 0.03),
            layer(Sine, 85.0, 30.0, 0.3, 1.0),
            layer(Noise, 0.0, 0.0, 0.2, 0.4).tone(0.15, 0.02).at(0.08),
        ],
    },
    // A building coming down: bigger, longer, with a second collapse.
    Recipe {
        id: "sfx_explode_large",
        folder: "sfx",
        seconds: 1.8,
        takes: 2,
        drive: 2.5,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.5, 1.0).tone(0.3, 0.02),
            layer(Sine, 60.0, 24.0, 0.6, 1.0),
            layer(Noise, 0.0, 0.0, 0.45, 0.7).tone(0.12, 0.015).at(0.18),
            layer(Sine, 45.0, 20.0, 0.4, 0.6).at(0.2),
        ],
    },
    // The hazard bursting up: a deep swelling roar of sand, a low growl under it, and a hiss as the sand falls.
    Recipe {
        id: "sfx_hazard_strike",
        folder: "sfx",
        seconds: 2.0,
        takes: 1,
        drive: 2.0,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.6, 1.0).tone(0.06, 0.25).rise(0.25),
            layer(Saw, 48.0, 34.0, 0.7, 0.6).tone(0.12, 0.05).rise(0.2),
            layer(Sine, 70.0, 28.0, 0.5, 0.8).rise(0.15),
            layer(Noise, 0.0, 0.0, 0.5, 0.4).tone(0.4, 0.1).at(0.8),
        ],
    },
    // The hazard moving under the sand: a long low rumble that swells and fades.
    Recipe {
        id: "sfx_hazard_rumble",
        folder: "sfx",
        seconds: 1.6,
        takes: 1,
        drive: 1.0,
        layers: &[
            layer(Noise, 0.0, 0.0, 0.6, 1.0).tone(0.03, 0.02).rise(0.5),
            layer(Sine, 38.0, 32.0, 0.6, 0.7).rise(0.5),
        ],
    },
    // A building set down: a low thud and a little grit.
    Recipe {
        id: "sfx_build",
        folder: "sfx",
        seconds: 0.4,
        takes: 1,
        drive: 0.8,
        layers: &[
            layer(Sine, 140.0, 60.0, 0.12, 1.0),
            layer(Noise, 0.0, 0.0, 0.05, 0.4).tone(0.2, 0.05),
            layer(Square, 70.0, 55.0, 0.07, 0.15).tone(0.2, 0.2),
        ],
    },
    // Something finished building: two rising notes.
    Recipe {
        id: "ui_ready",
        folder: "ui",
        seconds: 0.55,
        takes: 1,
        drive: 0.0,
        layers: &[
            layer(Sine, 660.0, 660.0, 0.16, 0.7),
            layer(Sine, 1320.0, 1320.0, 0.06, 0.15),
            layer(Sine, 880.0, 880.0, 0.2, 0.7).at(0.13),
            layer(Sine, 1760.0, 1760.0, 0.08, 0.15).at(0.13),
        ],
    },
    // A unit rolled out: three rising notes.
    Recipe {
        id: "ui_unit_ready",
        folder: "ui",
        seconds: 0.6,
        takes: 1,
        drive: 0.0,
        layers: &[
            layer(Sine, 523.0, 523.0, 0.12, 0.6),
            layer(Sine, 659.0, 659.0, 0.12, 0.6).at(0.09),
            layer(Sine, 784.0, 784.0, 0.2, 0.7).at(0.18),
        ],
    },
    // Not allowed: a low, beating buzz.
    Recipe {
        id: "ui_error",
        folder: "ui",
        seconds: 0.3,
        takes: 1,
        drive: 0.0,
        layers: &[
            layer(Square, 140.0, 140.0, 0.4, 0.5).tone(0.25, 0.25),
            layer(Square, 147.0, 147.0, 0.4, 0.5).tone(0.25, 0.25),
        ],
    },
    // Units selected: a short high blip.
    Recipe {
        id: "ui_select",
        folder: "ui",
        seconds: 0.09,
        takes: 1,
        drive: 0.0,
        layers: &[layer(Sine, 1200.0, 1250.0, 0.03, 0.8)],
    },
    // An order given: two quick blips.
    Recipe {
        id: "ui_order",
        folder: "ui",
        seconds: 0.15,
        takes: 1,
        drive: 0.0,
        layers: &[layer(Sine, 900.0, 900.0, 0.025, 0.7), layer(Sine, 1300.0, 1300.0, 0.03, 0.7).at(0.06)],
    },
    // Credits counted in: a tiny tick.
    Recipe {
        id: "ui_credits",
        folder: "ui",
        seconds: 0.06,
        takes: 1,
        drive: 0.0,
        layers: &[layer(Sine, 2100.0, 2000.0, 0.012, 0.8), layer(Noise, 0.0, 0.0, 0.004, 0.2).tone(0.8, 0.8)],
    },
    // Power ran short: a falling sweep.
    Recipe {
        id: "ui_power_down",
        folder: "ui",
        seconds: 0.75,
        takes: 1,
        drive: 0.0,
        layers: &[layer(Saw, 420.0, 110.0, 0.45, 0.6).tone(0.25, 0.08)],
    },
    // Power back: a rising sweep.
    Recipe {
        id: "ui_power_up",
        folder: "ui",
        seconds: 0.55,
        takes: 1,
        drive: 0.0,
        layers: &[layer(Saw, 140.0, 460.0, 0.35, 0.6).tone(0.08, 0.25).rise(0.05)],
    },
];

/// Every sound id the generator makes.
pub fn ids() -> Vec<&'static str> {
    RECIPES.iter().map(|r| r.id).collect()
}

/// A sine from a phase in turns (0..1): a parabola, sharpened. Within about 0.1% of the real thing.
fn sine(phase: f64) -> f64 {
    let p = phase - phase.floor();
    let y = if p < 0.5 { 16.0 * p * (0.5 - p) } else { -16.0 * (p - 0.5) * (1.0 - p) };
    0.225 * (y * y.abs() - y) + y
}

fn render(r: &Recipe, take: usize) -> Vec<i16> {
    let n = (r.seconds * RATE as f64) as usize;
    let dt = 1.0 / RATE as f64;
    let mut mix = vec![0.0f64; n];
    let mut noise = Noise::new(&format!("{}#{take}", r.id));
    // Each take is pitched a little differently.
    let detune = 1.0 + (take as f64) * 0.06 - (take as f64 * take as f64) * 0.02;
    for l in r.layers {
        let start = (l.delay * RATE as f64) as usize;
        let len = n.saturating_sub(start).max(1) as f64;
        let (mut phase, mut env, mut low) = (0.0f64, 1.0f64, 0.0f64);
        // Per-sample decay: 1 - dt/decay is exp(-dt/decay) to well within a sample's worth.
        let fall = 1.0 - dt / l.decay;
        for (i, out) in mix.iter_mut().enumerate().skip(start) {
            let t = (i - start) as f64 / len;
            let freq = (l.from + (l.to - l.from) * t) * detune;
            phase += freq * dt;
            phase -= phase.floor();
            let raw = match l.wave {
                Sine => sine(phase),
                Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Saw => 2.0 * phase - 1.0,
                Noise => (noise.roll() >> 11) as f64 / (1u64 << 52) as f64 - 1.0,
            };
            let a = l.bright + (l.bright_end - l.bright) * t;
            low += a * (raw - low);
            let secs = (i - start) as f64 * dt;
            let attack = if secs < l.attack { secs / l.attack } else { 1.0 };
            if secs >= l.attack {
                env *= fall;
            }
            *out += low * env * attack * l.gain;
        }
    }
    if r.drive > 0.0 {
        for s in &mut mix {
            let x = *s * (1.0 + r.drive);
            *s = x / (1.0 + x.abs());
        }
    }
    // Fade the last 10 ms, then bring the peak to 0.9.
    let fade = (RATE as usize / 100).min(n);
    for k in 0..fade {
        mix[n - 1 - k] *= k as f64 / fade as f64;
    }
    let peak = mix.iter().fold(0.0f64, |m, s| m.max(s.abs())).max(1e-9);
    mix.iter().map(|s| (s / peak * 0.9 * 32767.0).round() as i16).collect()
}

/// Encode 16-bit mono samples as a WAV file.
fn wav(samples: &[i16]) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data as usize);
    for part in [&b"RIFF"[..], &(36 + data).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes()] {
        b.extend_from_slice(part);
    }
    for part in [&1u16.to_le_bytes()[..], &RATE.to_le_bytes(), &(RATE * 2).to_le_bytes(), &2u16.to_le_bytes()] {
        b.extend_from_slice(part);
    }
    for part in [&16u16.to_le_bytes()[..], b"data", &data.to_le_bytes()] {
        b.extend_from_slice(part);
    }
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

/// Every file of the generic pack's `audio/` folder: the sounds, `audio/sounds.json` and `audio/provenance.jsonl`.
pub fn generate() -> Vec<File> {
    let mut files = Vec::new();
    let mut index = Vec::new();
    for r in RECIPES {
        let mut paths = Vec::new();
        for take in 0..r.takes {
            let id = r.id.trim_start_matches("sfx_").trim_start_matches("ui_");
            let path = format!("audio/{}/{id}_{}.wav", r.folder, take + 1);
            files.push(File { path: path.clone(), bytes: wav(&render(r, take)) });
            paths.push(format!("\"{path}\""));
        }
        index.push(format!("    \"{}\": {{ \"files\": [{}] }}", r.id, paths.join(", ")));
    }
    let provenance: String = files
        .iter()
        .map(|f| {
            format!(
                "{{\"file\": \"{}\", \"source\": \"original\", \"made_by\": \"synthesised from code: crates/classic-tools/src/sound.rs\", \"licence\": \"MIT\"}}\n",
                f.path
            )
        })
        .collect();
    let index = format!(
        "{{\n  \"about\": \"Placeholder sounds for the generic pack, synthesised from code by crates/classic-tools/src/sound.rs (run: cargo run --bin sounds). Paths are relative to the pack; each sound id lists its takes, and one is picked at random each time it plays. Which events play which sound id, and each id's bus, level and limits, are the engine's (data/audio/).\",\n  \"sounds\": {{\n{}\n  }}\n}}\n",
        index.join(",\n")
    );
    files.push(File { path: "audio/sounds.json".into(), bytes: index.into_bytes() });
    files.push(File { path: "audio/provenance.jsonl".into(), bytes: provenance.into_bytes() });
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_is_close() {
        for i in 0..1000 {
            let p = i as f64 / 1000.0;
            assert!((sine(p) - (p * std::f64::consts::TAU).sin()).abs() < 0.002, "{p}");
        }
    }

    #[test]
    fn every_sound_is_short_loud_enough_and_never_clips() {
        for f in generate().iter().filter(|f| f.path.ends_with(".wav")) {
            let samples: Vec<i16> = f.bytes[44..].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!((29000..=29500).contains(&peak), "{}: peak {peak}", f.path);
            assert!(samples.len() <= 2 * RATE as usize, "{}: over two seconds", f.path);
            assert_eq!(*samples.last().unwrap(), 0, "{}: doesn't fade out", f.path);
        }
    }
}
