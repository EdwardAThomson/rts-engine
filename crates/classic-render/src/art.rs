//! A setting pack's art (`art/art.json` and its PNG strips) on the GPU, with each faction's copy of every sprite
//! recoloured from the pack's remap colours to that faction's ramp.

use std::collections::BTreeMap;
use std::path::Path;

use classic_data::json::{self, Value};

use crate::platform::{Files, Gpu, Rect, SpriteBatch, TexId};

/// A strip of equal frames laid left to right in one texture.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    pub tex: TexId,
    pub w: f32,
    pub h: f32,
    pub frames: u32,
    /// The average colour of the first frame's opaque pixels, for the minimap.
    pub colour: [u8; 3],
    /// The average of the tenth of those pixels least like the average: what stands out, such as grains on sand.
    pub accent: [u8; 3],
}

impl Strip {
    /// Frame `i`, wrapping round.
    pub fn frame(&self, i: u32) -> Rect {
        Rect::new((i % self.frames.max(1)) as f32 * self.w, 0.0, self.w, self.h)
    }
}

pub struct Art {
    /// Pixels per tile.
    pub tile: f32,
    terrain: BTreeMap<String, Strip>,
    /// Each sprite once per faction ramp, in owner order.
    sprites: BTreeMap<String, Vec<Strip>>,
    effects: BTreeMap<String, Strip>,
    /// Each build icon once per faction ramp, in owner order.
    icons: BTreeMap<String, Vec<Strip>>,
    /// The middle shade of each owner's ramp, in owner order.
    owners: Vec<[u8; 3]>,
}

impl Art {
    /// Load the art of the pack in `dir`, recoloured for `ramps` (the ramp name of each player, in owner order).
    pub fn load(gpu: &Gpu, batch: &mut SpriteBatch, dir: &Path, ramps: &[String]) -> Result<Art, String> {
        Art::from_files(gpu, batch, &Files::Dir(dir.to_path_buf()), ramps)
    }

    /// Load the art of the pack whose files are `files`, recoloured for `ramps`.
    pub fn from_files(gpu: &Gpu, batch: &mut SpriteBatch, files: &Files, ramps: &[String]) -> Result<Art, String> {
        let text = files.read_text(ART_INDEX)?;
        let doc = json::parse(&text).map_err(|e| format!("{ART_INDEX}: {e:?}"))?;
        let tile = doc.get("tile").and_then(Value::as_int).ok_or("art.json: no tile size")? as f32;
        let colours = |v: Option<&Value>| -> Result<Vec<[u8; 3]>, String> {
            v.and_then(Value::as_array).ok_or("art.json: missing colour list")?.iter().map(hex).collect()
        };
        let remap = colours(doc.get("remap"))?;
        let mut owner_ramps = Vec::new();
        for r in ramps {
            owner_ramps.push(colours(doc.get("ramps").and_then(|m| m.get(r)))?);
        }
        let load = |batch: &mut SpriteBatch, entry: &Value, ramp: Option<&[[u8; 3]]>| -> Result<Strip, String> {
            let file = entry.get("file").and_then(Value::as_str).ok_or("art.json: entry without a file")?;
            let (w, h, mut rgba) = decode_png(&files.read(file)?, file)?;
            if let Some(ramp) = ramp {
                recolour(&mut rgba, &remap, ramp);
            }
            // No frame size means one picture, the whole file.
            let frame = entry.get("frame").and_then(Value::as_array).unwrap_or(&[]);
            let fw = frame.first().and_then(Value::as_int).unwrap_or(w as i64) as f32;
            let fh = frame.get(1).and_then(Value::as_int).unwrap_or(h as i64) as f32;
            let frames =
                entry.get("frames").or_else(|| entry.get("variants")).and_then(Value::as_int).unwrap_or(1) as u32;
            let (colour, accent) = average(&rgba, w as usize, fw as usize, fh as usize);
            Ok(Strip { tex: batch.texture(gpu, w, h, &rgba), w: fw, h: fh, frames, colour, accent })
        };
        let entries = |key: &str| doc.get(key).and_then(Value::as_object).unwrap_or(&[]);
        let mut art = Art {
            tile,
            terrain: BTreeMap::new(),
            sprites: BTreeMap::new(),
            effects: BTreeMap::new(),
            icons: BTreeMap::new(),
            owners: owner_ramps.iter().map(|r| r[r.len() / 2]).collect(),
        };
        for (id, entry) in entries("terrain") {
            art.terrain.insert(id.clone(), load(batch, entry, None)?);
        }
        for (id, entry) in entries("effects") {
            art.effects.insert(id.clone(), load(batch, entry, None)?);
        }
        for (id, entry) in entries("sprites") {
            let strips = owner_ramps.iter().map(|r| load(batch, entry, Some(r))).collect::<Result<_, _>>()?;
            art.sprites.insert(id.clone(), strips);
        }
        // Icons are plain file names, one picture each.
        for (id, file) in entries("icons") {
            let entry = Value::Object(vec![("file".into(), file.clone())]);
            let strips = owner_ramps.iter().map(|r| load(batch, &entry, Some(r))).collect::<Result<_, _>>()?;
            art.icons.insert(id.clone(), strips);
        }
        Ok(art)
    }

    /// A colour that stands for `owner`: the middle shade of its ramp.
    pub fn owner_colour(&self, owner: u32) -> [u8; 3] {
        self.owners.get(owner as usize % self.owners.len().max(1)).copied().unwrap_or([200, 200, 200])
    }

    pub fn terrain(&self, id: &str) -> Option<&Strip> {
        self.terrain.get(id)
    }

    /// The sprite for generic id `id` in `owner`'s colours.
    pub fn sprite(&self, id: &str, owner: u32) -> Option<&Strip> {
        let strips = self.sprites.get(id)?;
        strips.get(owner as usize % strips.len().max(1))
    }

    /// The build icon for generic id `id` in `owner`'s colours.
    pub fn icon(&self, id: &str, owner: u32) -> Option<&Strip> {
        let strips = self.icons.get(id)?;
        strips.get(owner as usize % strips.len().max(1))
    }

    pub fn effect(&self, id: &str) -> Option<&Strip> {
        self.effects.get(id)
    }
}

/// The art index, from the pack's folder.
pub const ART_INDEX: &str = "art/art.json";

/// Every file the art index at `ART_INDEX` names, so the browser build knows what to fetch.
pub fn art_files(index: &str) -> Result<Vec<String>, String> {
    let doc = json::parse(index).map_err(|e| format!("{ART_INDEX}: {e:?}"))?;
    let mut files = Vec::new();
    for key in ["terrain", "effects", "sprites"] {
        for (_, entry) in doc.get(key).and_then(Value::as_object).unwrap_or(&[]) {
            if let Some(f) = entry.get("file").and_then(Value::as_str) {
                files.push(f.to_string());
            }
        }
    }
    // Icons are plain file names.
    for (_, file) in doc.get("icons").and_then(Value::as_object).unwrap_or(&[]) {
        files.extend(file.as_str().map(String::from));
    }
    Ok(files)
}

/// Where to find a pack's art: the pack itself if it has an `art/art.json`, else the generic pack's placeholders.
pub fn art_dir(pack: &classic_data::Pack) -> std::path::PathBuf {
    if pack.dir.join(ART_INDEX).is_file() {
        pack.dir.clone()
    } else {
        classic_tools::setting::root().join("settings/generic")
    }
}

/// The ramp each of `players` players is drawn in: the pack's factions in order, repeating if there are more
/// players than factions.
pub fn player_ramps(pack: &classic_data::Pack, players: usize) -> Vec<String> {
    let n = pack.factions.len().max(1);
    (0..players).map(|i| pack.factions.get(i % n).map_or_else(|| "grey".into(), |f| f.ramp.clone())).collect()
}

fn hex(v: &Value) -> Result<[u8; 3], String> {
    let s = v.as_str().and_then(|s| s.strip_prefix('#')).filter(|s| s.len() == 6).ok_or("art.json: bad colour")?;
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("art.json: {s}: {e}"));
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

/// The average colour of the opaque pixels in the top-left `fw` by `fh` frame of an image `w` pixels wide, and the
/// average of the tenth of them least like that average.
fn average(rgba: &[u8], w: usize, fw: usize, fh: usize) -> ([u8; 3], [u8; 3]) {
    let mut px: Vec<[u8; 3]> = Vec::new();
    for y in 0..fh.min(rgba.len() / (w * 4).max(1)) {
        for x in 0..fw.min(w) {
            let p = &rgba[(y * w + x) * 4..][..4];
            if p[3] >= 128 {
                px.push([p[0], p[1], p[2]]);
            }
        }
    }
    let mean = |px: &[[u8; 3]]| {
        let n = px.len().max(1) as u64;
        [0, 1, 2].map(|c| (px.iter().map(|p| u64::from(p[c])).sum::<u64>() / n) as u8)
    };
    let all = mean(&px);
    px.sort_by_key(|p| std::cmp::Reverse((0..3).map(|c| p[c].abs_diff(all[c]) as u32).sum::<u32>()));
    (all, mean(&px[..px.len().div_ceil(10)]))
}

/// Swap every pixel in a remap colour, by exact value, for the same shade of the ramp.
pub fn recolour(rgba: &mut [u8], remap: &[[u8; 3]], ramp: &[[u8; 3]]) {
    for px in rgba.chunks_exact_mut(4) {
        if let Some(i) = remap.iter().position(|c| c[..] == px[..3]) {
            px[..3].copy_from_slice(&ramp[i.min(ramp.len() - 1)]);
        }
    }
}

/// A PNG file as (width, height, RGBA pixels).
pub fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    decode_png(&bytes, &path.display().to_string())
}

/// PNG bytes as (width, height, RGBA pixels). `name` is for messages.
pub fn decode_png(bytes: &[u8], name: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("{name}: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{name}: {e}"))?;
    let px = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err(format!("{name}: palette not expanded")),
    };
    Ok((info.width, info.height, rgba))
}
