//! A setting pack's art (`art/art.json` and its PNG strips) on the GPU, with each faction's copy of every sprite
//! recoloured from the pack's remap colours to that faction's ramp.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use classic_data::json::{self, Value};

use crate::platform::{Files, Gpu, Rect, SpriteBatch, TexId};
use crate::studio::{self, Studio};
use crate::tiles::{self, Tileset};

/// A strip of equal frames laid left to right in one texture.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    pub tex: TexId,
    pub w: f32,
    pub h: f32,
    pub frames: u32,
    /// How many facings the frames cover, facing by facing: each facing has `frames / facings` frames in a row, a
    /// walk cycle when there is more than one. 1 for a strip with no facings.
    pub facings: u32,
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

    /// The frame for `facing` (0 to 255 clockwise from north) at `step` of the facing's cycle, wrapping round.
    pub fn facing_frame(&self, facing: i64, step: u32) -> Rect {
        let facings = self.facings.max(1);
        let per = (self.frames / facings).max(1);
        let f = ((facing.rem_euclid(256) * i64::from(facings) + 128) / 256) as u32 % facings;
        self.frame(f * per + step % per)
    }

    /// Frames in each facing's cycle.
    pub fn cycle(&self) -> u32 {
        (self.frames / self.facings.max(1)).max(1)
    }
}

/// A unit drawn as several copies of another unit's sprite standing in a formation, such as a squad of three
/// soldiers. It shows one copy fewer for each equal share of health lost.
#[derive(Clone, Debug, PartialEq)]
pub struct Squad {
    /// The sprite each member is drawn with.
    pub member: String,
    /// Where each member stands, in 256ths of a tile from the unit's centre (x right, y down), for a squad facing
    /// north; the formation turns with the unit. Members are lost from the end of the list.
    pub offsets: Vec<(i64, i64)>,
}

impl Squad {
    /// How many members stand at `health` out of `max`: a share of the count, rounded up, never none while alive.
    pub fn shown(&self, health: i64, max: i64) -> usize {
        let n = self.offsets.len() as i64;
        if n == 0 || health <= 0 {
            return 0;
        }
        ((n * health.min(max) + max - 1) / max.max(1)).clamp(1, n) as usize
    }

    /// Where member `i` stands for a squad facing `facing` (0 to 255 clockwise from north), in tiles from its
    /// centre. The formation turns in eighths, with the sprite.
    pub fn place(&self, i: usize, facing: i64) -> (f32, f32) {
        let (ox, oy) = self.offsets.get(i).copied().unwrap_or((0, 0));
        let eighth = ((facing.rem_euclid(256) + 16) / 32) % 8;
        let a = eighth as f32 * std::f32::consts::FRAC_PI_4;
        let (s, c) = a.sin_cos();
        let (x, y) = (ox as f32 * c - oy as f32 * s, ox as f32 * s + oy as f32 * c);
        (x / 256.0, y / 256.0)
    }

    /// Where member `i` is in a walk cycle of `cycle` frames at the squad's step `walk`: each starts at its own
    /// point, spread evenly, so they don't march in step.
    pub fn step(&self, i: usize, walk: u32, cycle: u32) -> u32 {
        walk + i as u32 * cycle / self.offsets.len().max(1) as u32
    }
}

/// The squads listed in an art index, by unit id. An entry is `{ "member": id, "offsets": [[x, y], ...] }`.
pub fn parse_squads(doc: &Value) -> Result<BTreeMap<String, Squad>, String> {
    let mut squads = BTreeMap::new();
    for (id, entry) in doc.get("squads").and_then(Value::as_object).unwrap_or(&[]) {
        let member =
            entry.get("member").and_then(Value::as_str).ok_or(format!("art.json: squad {id} has no member"))?;
        let mut offsets = Vec::new();
        for p in entry.get("offsets").and_then(Value::as_array).unwrap_or(&[]) {
            let xy = p.as_array().filter(|a| a.len() == 2).and_then(|a| Some((a[0].as_int()?, a[1].as_int()?)));
            offsets.push(xy.ok_or(format!("art.json: squad {id}: an offset is not [x, y]"))?);
        }
        if offsets.is_empty() {
            return Err(format!("art.json: squad {id} has no offsets"));
        }
        squads.insert(id.clone(), Squad { member: member.to_string(), offsets });
    }
    Ok(squads)
}

pub struct Art {
    /// Pixels per tile.
    pub tile: f32,
    terrain: BTreeMap<String, Strip>,
    /// Each sprite once per faction ramp, in owner order.
    sprites: BTreeMap<String, Vec<Strip>>,
    effects: BTreeMap<String, Strip>,
    /// The detailed effects (`art/sprites/effects/`), for the ids that have them; their page is in `studio`.
    fx: BTreeMap<String, studio::Sprite>,
    /// Each build icon once per faction ramp, in owner order.
    icons: BTreeMap<String, Vec<Strip>>,
    /// Units drawn as several members, by unit id.
    squads: BTreeMap<String, Squad>,
    /// The studio's packed sprites, for the ids that have them.
    pub studio: Studio,
    /// The terrain tile set and its page, where the pack has one; it replaces the plain `terrain` tiles.
    pub tileset: Option<(Tileset, TexId)>,
    /// The middle shade of each owner's ramp, in owner order.
    owners: Vec<[u8; 3]>,
}

impl Art {
    /// Load the art in the pack folders `dirs` (as `art_dirs` gives them, the first over the rest), recoloured for
    /// `ramps` (the ramp name of each player, in owner order).
    pub fn load(gpu: &Gpu, batch: &mut SpriteBatch, dirs: &[PathBuf], ramps: &[String]) -> Result<Art, String> {
        let layers: Vec<Files> = dirs.iter().cloned().map(Files::Dir).collect();
        Art::from_files(gpu, batch, &layers, ramps)
    }

    /// Load the art whose files are `layers`, recoloured for `ramps`. Each file comes from the first layer that has
    /// it, so a pack's own art draws over the generic pack's: the art index, each picture it names, each studio
    /// sprite (with the atlas page beside it) and the terrain tile set.
    pub fn from_files(gpu: &Gpu, batch: &mut SpriteBatch, layers: &[Files], ramps: &[String]) -> Result<Art, String> {
        // The first layer with `path`, and its bytes.
        let read = |path: &str| -> Result<(usize, Vec<u8>), String> {
            let mut missing = format!("{path}: no art layers");
            for (i, files) in layers.iter().enumerate() {
                match files.read(path) {
                    Ok(bytes) => return Ok((i, bytes)),
                    Err(e) => missing = e,
                }
            }
            Err(missing)
        };
        let (_, index) = read(ART_INDEX)?;
        let text = String::from_utf8(index).map_err(|e| format!("{ART_INDEX}: {e}"))?;
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
            let (w, h, mut rgba) = decode_png(&read(file)?.1, file)?;
            if let Some(ramp) = ramp {
                recolour(&mut rgba, &remap, ramp);
            }
            // No frame size means one picture, the whole file.
            let frame = entry.get("frame").and_then(Value::as_array).unwrap_or(&[]);
            let fw = frame.first().and_then(Value::as_int).unwrap_or(w as i64) as f32;
            let fh = frame.get(1).and_then(Value::as_int).unwrap_or(h as i64) as f32;
            let frames =
                entry.get("frames").or_else(|| entry.get("variants")).and_then(Value::as_int).unwrap_or(1) as u32;
            let facings = entry.get("facings").and_then(Value::as_int).unwrap_or(1).max(1) as u32;
            let (colour, accent) = average(&rgba, w as usize, fw as usize, fh as usize);
            Ok(Strip { tex: batch.texture(gpu, w, h, &rgba), w: fw, h: fh, frames, facings, colour, accent })
        };
        let entries = |key: &str| doc.get(key).and_then(Value::as_object).unwrap_or(&[]);
        let mut art = Art {
            tile,
            terrain: BTreeMap::new(),
            sprites: BTreeMap::new(),
            effects: BTreeMap::new(),
            fx: BTreeMap::new(),
            icons: BTreeMap::new(),
            squads: parse_squads(&doc)?,
            studio: Studio::default(),
            tileset: None,
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
        // The studio's packed sprites, where the pack has them, replace the strips they cover.
        let kinds: Vec<(&str, &str)> = entries("sprites")
            .iter()
            .map(|(id, e)| (id.as_str(), e.get("kind").and_then(Value::as_str).unwrap_or("")))
            .collect();
        // Each sprite with the layer it came from: its page is the one beside it, in the same layer, and two layers'
        // pages can share a name, so pages are keyed by layer and name.
        let mut found = Vec::new();
        for path in studio::candidates(kinds).into_iter().chain(effect_candidates()) {
            let Ok((layer, bytes)) = read(&path) else { continue };
            let text = String::from_utf8_lossy(&bytes);
            let mut sprite = studio::Sprite::parse(&text).map_err(|e| format!("{}: {e}", layers[layer].name(&path)))?;
            let page = sprite.atlas.clone();
            sprite.atlas = format!("{layer}/{page}");
            found.push((path, layer, page, sprite));
        }
        // A page at a time: measure the muzzles its sprites lack, then paint it in each owner's colours. A page with
        // no team mask (effects) is loaded once, as it is.
        let atlases: std::collections::BTreeSet<(usize, String)> =
            found.iter().map(|(_, layer, page, _)| (*layer, page.clone())).collect();
        for (layer, page) in atlases {
            let files = &layers[layer];
            let key = format!("{layer}/{page}");
            let [img, mask] = studio::page_files(&page);
            let (w, h, rgba) = decode_png(&files.read(&img)?, &img)?;
            for (path, _, _, sprite) in found.iter_mut().filter(|(_, _, _, s)| s.atlas == key) {
                let body_gun = ["/units/", "/air/"].iter().any(|f| path.contains(f));
                sprite.find_muzzles(&rgba, w, body_gun);
            }
            let pages = match files.read(&mask) {
                Ok(bytes) => {
                    let (mw, mh, mask_px) = decode_png(&bytes, &mask)?;
                    if (mw, mh) != (w, h) {
                        return Err(format!("{mask}: {mw}x{mh}, but its page is {w}x{h}"));
                    }
                    let mut pages = Vec::new();
                    for ramp in &owner_ramps {
                        let mut px = rgba.clone();
                        studio::paint(&mut px, &mask_px, ramp);
                        pages.push(batch.texture(gpu, w, h, &px));
                    }
                    pages
                }
                Err(_) => vec![batch.texture(gpu, w, h, &rgba)],
            };
            art.studio.pages.insert(key, pages);
        }
        for (path, _, _, sprite) in found {
            let id = path.rsplit('/').next().unwrap_or("").trim_end_matches(".json").to_string();
            if path.contains("/effects/") {
                art.fx.insert(id, sprite);
            } else {
                art.studio.sprites.insert(id, sprite);
            }
        }
        // The tile set and its page come from the same layer.
        if let Ok((layer, bytes)) = read(tiles::TILESET) {
            let set = Tileset::parse(&String::from_utf8_lossy(&bytes))?;
            let (w, h, rgba) = decode_png(&layers[layer].read(tiles::TILESET_PAGE)?, tiles::TILESET_PAGE)?;
            art.tileset = Some((set, batch.texture(gpu, w, h, &rgba)));
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

    /// The minimap colour of tile set layer `id`, where the pack has a tile set with that layer.
    pub fn layer_colour(&self, id: &str) -> Option<[u8; 3]> {
        self.tileset.as_ref()?.0.layer(id).map(|l| l.colour)
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

    /// How unit `id` is drawn as a squad, if it is one and its member has a sprite.
    pub fn squad(&self, id: &str) -> Option<&Squad> {
        self.squads.get(id).filter(|s| self.sprites.contains_key(&s.member))
    }

    pub fn effect(&self, id: &str) -> Option<&Strip> {
        self.effects.get(id)
    }

    /// The detailed effect `id` and its page, if the pack has one.
    pub fn fx(&self, id: &str) -> Option<(&studio::Sprite, TexId)> {
        let s = self.fx.get(id)?;
        Some((s, *self.studio.pages.get(&s.atlas)?.first()?))
    }
}

/// The effects the renderer plays, by generic id. A pack draws each as a detailed effect in
/// `art/sprites/effects/<id>.json` (`art/effects/effects.py` makes the generic pack's), else as the placeholder strip
/// `art.json` lists under `effects`, else not at all.
pub const EFFECTS: [&str; 11] = [
    "explosion_small",
    "explosion_medium",
    "explosion_large",
    "smoke_puff",
    "dust_puff",
    "hit_spark",
    "muzzle_flash_gun",
    "muzzle_flash_rocket",
    "shell",
    "rocket",
    "fire",
];

fn effect_candidates() -> Vec<String> {
    studio::candidates(EFFECTS.iter().map(|id| (*id, "effect")))
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
    // The terrain tile set, if the pack has one.
    files.extend([tiles::TILESET.to_string(), tiles::TILESET_PAGE.to_string()]);
    // Where the studio's packed sprites and the detailed effects may be; a pack need not have them.
    let ids = doc.get("sprites").and_then(Value::as_object).unwrap_or(&[]).iter();
    files.extend(studio::candidates(
        ids.map(|(id, e)| (id.as_str(), e.get("kind").and_then(Value::as_str).unwrap_or(""))),
    ));
    files.extend(effect_candidates());
    Ok(files)
}

/// The atlas pages the studio sprites among `files` (path and contents) are on, with their team masks, for a loader
/// that fetches in turn. A page with no team paint (effects) has no mask; the loader takes it as it is.
pub fn atlas_files<'a>(files: impl IntoIterator<Item = (&'a String, &'a Vec<u8>)>) -> Vec<String> {
    let mut pages = std::collections::BTreeSet::new();
    for (path, bytes) in files {
        if path.starts_with(studio::SPRITES)
            && path.ends_with(".json")
            && let Ok(s) = studio::Sprite::parse(&String::from_utf8_lossy(bytes))
        {
            pages.extend(studio::page_files(&s.atlas));
        }
    }
    pages.into_iter().collect()
}

/// Where a pack's art comes from, first over the rest: the pack's own folder, then the generic pack's placeholders
/// for anything it doesn't draw (just the generic pack, for the generic pack).
pub fn art_dirs(pack: &classic_data::Pack) -> Vec<PathBuf> {
    let generic = classic_tools::setting::root().join("settings/generic");
    if pack.dir.canonicalize().ok() == generic.canonicalize().ok() {
        vec![generic]
    } else {
        vec![pack.dir.clone(), generic]
    }
}

/// The ramp each of `players` players is drawn in: the pack's factions in order, repeating if there are more
/// players than factions. The same as `faction_ramps` with player 0 on the first faction.
pub fn player_ramps(pack: &classic_data::Pack, players: usize) -> Vec<String> {
    faction_ramps(pack, players, 0, 0)
}

/// Which of the pack's factions each player has, in owner order, when the player `local` picked faction `chosen`:
/// the others take the remaining factions in the pack's order, then every faction again in order when there are more
/// players than factions.
pub fn player_factions(factions: usize, players: usize, local: usize, chosen: usize) -> Vec<usize> {
    let n = factions.max(1);
    let chosen = chosen % n;
    let mut others = (0..n).filter(|&f| f != chosen).chain((0..n).cycle());
    (0..players).map(|p| if p == local { chosen } else { others.next().unwrap_or(0) }).collect()
}

/// The ramp name of each player, in owner order, when the player `local` picked faction `chosen`.
pub fn faction_ramps(pack: &classic_data::Pack, players: usize, local: usize, chosen: usize) -> Vec<String> {
    player_factions(pack.factions.len(), players, local, chosen)
        .into_iter()
        .map(|f| pack.factions.get(f).map_or_else(|| "grey".into(), |f| f.ramp.clone()))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn three() -> Squad {
        Squad { member: "infantry".into(), offsets: vec![(0, -64), (-64, 48), (64, 48)] }
    }

    #[test]
    fn a_squad_loses_a_member_for_each_third_of_its_health() {
        let s = three();
        let shown: Vec<usize> = [140, 94, 93, 47, 46, 1, 0].iter().map(|&h| s.shown(h, 140)).collect();
        assert_eq!(shown, [3, 3, 2, 2, 1, 1, 0]);
        assert_eq!(s.shown(200, 140), 3, "overhealed still shows the count");
    }

    #[test]
    fn the_formation_turns_with_the_squad() {
        let s = three();
        let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4;
        assert!(near(s.place(0, 0), (0.0, -0.25)), "the leader is in front facing north");
        assert!(near(s.place(0, 64), (0.25, 0.0)), "and in front facing east");
        assert!(near(s.place(1, 128), (0.25, -0.1875)), "the left-hand soldier is on the right facing south");
        assert!(near(s.place(0, 70), s.place(0, 64)), "the formation turns in eighths, with the sprite");
    }

    #[test]
    fn members_walk_out_of_step_on_a_strip_with_walk_cycles() {
        // Eight facings of six walk frames, facing by facing.
        let strip = Strip { tex: TexId(0), w: 32.0, h: 32.0, frames: 48, facings: 8, colour: [0; 3], accent: [0; 3] };
        let x = |r: Rect| (r.x / 32.0) as u32;
        assert_eq!(x(strip.facing_frame(64, 0)), 12, "east starts at the third facing's first frame");
        assert_eq!(x(strip.facing_frame(64, 7)), 13, "and the cycle wraps within the facing");
        let s = three();
        let steps: Vec<u32> = (0..3).map(|i| s.step(i, 0, strip.cycle())).collect();
        assert_eq!(steps, [0, 2, 4]);
        let still = Strip { frames: 8, ..strip };
        assert_eq!(x(still.facing_frame(64, 5)), 2, "a strip of one frame per facing never walks");
    }

    #[test]
    fn squads_parse_from_the_art_index() {
        let doc = json::parse(
            r#"{"squads": {"infantry_squad": {"member": "infantry", "offsets": [[0, -64], [-64, 48], [64, 48]]}}}"#,
        )
        .unwrap();
        assert_eq!(parse_squads(&doc).unwrap().get("infantry_squad"), Some(&three()));
        let bad = json::parse(r#"{"squads": {"x": {"member": "infantry", "offsets": [[0]]}}}"#).unwrap();
        assert!(parse_squads(&bad).is_err());
        assert!(parse_squads(&json::parse("{}").unwrap()).unwrap().is_empty());
    }
}
