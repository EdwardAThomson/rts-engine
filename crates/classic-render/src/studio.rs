//! The Blender studio's packed sprites (`art/sprites/` in a pack, written by `art/studio/pack.py`; the format is in
//! `art/README.md`, "Output"): atlas pages with a team mask, and one JSON file per entity naming its parts, anims and
//! frames. A pack's entity drawn this way replaces its placeholder strip.

use std::collections::BTreeMap;

use classic_data::json::{self, Value};

use crate::platform::{Rect, TexId};

/// Where the studio's packed sprites live in a pack.
pub const SPRITES: &str = "art/sprites";

/// The folders the packer may have written an entity of art index kind `kind` into.
pub fn folders(kind: &str) -> &'static [&'static str] {
    match kind {
        "building" | "turret" => &["buildings"],
        "aircraft" => &["air"],
        "unit" => &["units", "infantry"],
        "effect" => &["effects"],
        _ => &[],
    }
}

/// Share of full black a shadow is drawn at.
pub const SHADOW_ALPHA: u8 = 115;

/// One frame in an atlas page: where it is and the point (in atlas pixels from its top left) that sits on the
/// entity's ground point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub src: Rect,
    pub pivot: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Anim {
    /// Frames per facing.
    pub length: u32,
    /// Facings this anim covers, when it differs from its part's (infantry deaths: 1).
    pub facings: u32,
    pub frames: Vec<Frame>,
    /// Lined up with `frames`, where the anim casts a shadow.
    pub shadow: Vec<Frame>,
    /// Lined up with `frames`, where the model has a weapon: its barrel tip in atlas pixels from the pivot, and
    /// whether the body hides it from the camera.
    pub muzzle: Vec<Muzzle>,
}

/// A barrel tip in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muzzle {
    pub x: f32,
    pub y: f32,
    pub hidden: bool,
}

impl Anim {
    /// The frame for `facing` (0 to 255 clockwise from north) at `step`, wrapping round the cycle.
    pub fn index(&self, facing: i64, step: u32) -> usize {
        let n = self.facings.max(1);
        let f = ((facing.rem_euclid(256) * i64::from(n) + 128) / 256) as u32 % n;
        let i = (f * self.length.max(1) + step % self.length.max(1)) as usize;
        i.min(self.frames.len().saturating_sub(1))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub facings: u32,
    /// Drawn over a building's intact frame at the same pivot.
    pub overlay: bool,
    pub anims: BTreeMap<String, Anim>,
}

/// One entity's sprites.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    /// The atlas page its frames are in.
    pub atlas: String,
    /// Atlas pixels per pixel of a 32-pixel tile.
    pub scale: f32,
    pub parts: BTreeMap<String, Part>,
}

impl Sprite {
    pub fn parse(text: &str) -> Result<Sprite, String> {
        let doc = json::parse(text).map_err(|e| format!("{e:?}"))?;
        let atlas = doc.get("atlas").and_then(Value::as_str).ok_or("no atlas")?.to_string();
        let scale = doc.get("scale").and_then(Value::as_int).unwrap_or(1).max(1) as f32;
        let frames = |v: Option<&Value>| -> Result<Vec<Frame>, String> {
            let mut out = Vec::new();
            for f in v.and_then(Value::as_array).unwrap_or(&[]) {
                let n: Vec<f32> =
                    f.as_array().unwrap_or(&[]).iter().filter_map(Value::as_int).map(|n| n as f32).collect();
                let [x, y, w, h, px, py] = n[..] else { return Err("a frame is not [x, y, w, h, px, py]".into()) };
                out.push(Frame { src: Rect::new(x, y, w, h), pivot: (px, py) });
            }
            Ok(out)
        };
        let muzzles = |v: Option<&Value>| -> Result<Vec<Muzzle>, String> {
            let mut out = Vec::new();
            for m in v.and_then(Value::as_array).unwrap_or(&[]) {
                let n: Vec<i64> = m.as_array().unwrap_or(&[]).iter().filter_map(Value::as_int).collect();
                let [x, y, hidden] = n[..] else { return Err("a muzzle is not [dx, dy, hidden]".into()) };
                out.push(Muzzle { x: x as f32, y: y as f32, hidden: hidden != 0 });
            }
            Ok(out)
        };
        let mut parts = BTreeMap::new();
        for (name, p) in doc.get("parts").and_then(Value::as_object).unwrap_or(&[]) {
            let facings = p.get("facings").and_then(Value::as_int).unwrap_or(1).max(1) as u32;
            let mut anims = BTreeMap::new();
            for (anim, a) in p.get("anims").and_then(Value::as_object).unwrap_or(&[]) {
                anims.insert(
                    anim.clone(),
                    Anim {
                        length: a.get("length").and_then(Value::as_int).unwrap_or(1).max(1) as u32,
                        facings: a.get("facings").and_then(Value::as_int).map_or(facings, |n| n.max(1) as u32),
                        frames: frames(a.get("frames"))?,
                        shadow: frames(a.get("shadow"))?,
                        muzzle: muzzles(a.get("muzzle"))?,
                    },
                );
            }
            let overlay = p.get("overlay").is_some_and(|v| *v == Value::Bool(true));
            parts.insert(name.clone(), Part { facings, overlay, anims });
        }
        Ok(Sprite { atlas, scale, parts })
    }

    /// The part that is the entity itself: its body, hull or building, not a turret, overlay or wreck.
    pub fn body(&self) -> Option<&Part> {
        ["body", "hull", "building"].iter().find_map(|p| self.parts.get(*p)).or_else(|| {
            self.parts
                .iter()
                .find(|(n, p)| !p.overlay && !["turret", "head", "wreck"].contains(&n.as_str()))
                .map(|(_, p)| p)
        })
    }

    /// The turning head on top, if any.
    pub fn turret(&self) -> Option<&Part> {
        self.parts.get("turret").or_else(|| self.parts.get("head"))
    }

    pub fn overlays(&self) -> impl Iterator<Item = &Part> {
        self.parts.values().filter(|p| p.overlay)
    }

    /// Where the weapon's barrel tip is, in atlas pixels from the pivot: on the turret facing `turret` if there is
    /// one, else on the body facing `facing` in the first of `anims` it has, at `step`. The body's idle stands in
    /// for an anim with no muzzle points.
    pub fn muzzle(&self, anims: &[&str], facing: i64, turret: i64, step: u32) -> Option<Muzzle> {
        if let Some(a) = self.turret().and_then(|p| p.anim(&["idle"]))
            && !a.muzzle.is_empty()
        {
            return a.muzzle.get(a.index(turret, 0)).copied();
        }
        let body = self.body()?;
        let a = body.anim(anims).filter(|a| !a.muzzle.is_empty()).or_else(|| body.anim(&["idle"]))?;
        a.muzzle.get(a.index(facing, step)).copied()
    }

    /// Give the turret (or, where `body_gun`, the body) muzzle points the packer didn't record, measured from the
    /// frames' pixels on the atlas page `rgba`, `width` pixels wide: in each frame, the opaque pixels furthest out
    /// from the middle in the way the frame faces. On a turret that is the end of the barrel; on a vehicle with its
    /// gun on the hull, its nose. Infantry come with theirs.
    pub fn find_muzzles(&mut self, rgba: &[u8], width: u32, body_gun: bool) {
        let name = if self.parts.contains_key("turret") {
            "turret"
        } else if self.parts.contains_key("head") {
            "head"
        } else if body_gun {
            match ["body", "hull"].into_iter().find(|n| self.parts.contains_key(*n)) {
                Some(n) => n,
                None => return,
            }
        } else {
            return;
        };
        let Some(part) = self.parts.get_mut(name) else { return };
        if part.anims.values().any(|a| !a.muzzle.is_empty()) {
            return;
        }
        let Some(a) = part.anims.get_mut("idle") else { return };
        let (n, length) = (a.facings.max(1) as usize, a.length.max(1) as usize);
        a.muzzle = a
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let facing = ((i / length) % n * 256 / n) as i64;
                far_point(rgba, width, f, facing)
            })
            .collect();
    }
}

/// The point of frame `f` on a page `width` pixels wide furthest out from its opaque pixels' middle towards
/// `facing` (0 to 255 clockwise from north), from the frame's pivot. The packer stretches height so ground
/// directions keep their angles on screen.
pub fn far_point(rgba: &[u8], width: u32, f: &Frame, facing: i64) -> Muzzle {
    let a = facing as f32 * std::f32::consts::TAU / 256.0;
    let (dx, dy) = (a.sin(), -a.cos());
    let mut px = Vec::new();
    for y in f.src.y as u32..(f.src.y + f.src.h) as u32 {
        for x in f.src.x as u32..(f.src.x + f.src.w) as u32 {
            if rgba.get(((y * width + x) * 4 + 3) as usize).is_some_and(|&a| a >= 128) {
                px.push((x as f32 + 0.5 - f.src.x, y as f32 + 0.5 - f.src.y));
            }
        }
    }
    if px.is_empty() {
        return Muzzle { x: 0.0, y: 0.0, hidden: false };
    }
    let n = px.len() as f32;
    let (cx, cy) = (px.iter().map(|p| p.0).sum::<f32>() / n, px.iter().map(|p| p.1).sum::<f32>() / n);
    let along = |p: &(f32, f32)| (p.0 - cx) * dx + (p.1 - cy) * dy;
    let far = px.iter().map(along).fold(f32::MIN, f32::max);
    let tip: Vec<&(f32, f32)> = px.iter().filter(|p| along(p) > far - 1.0).collect();
    let m = tip.len() as f32;
    let (x, y) = (tip.iter().map(|p| p.0).sum::<f32>() / m, tip.iter().map(|p| p.1).sum::<f32>() / m);
    Muzzle { x: x - f.pivot.0, y: y - f.pivot.1, hidden: false }
}

impl Part {
    /// The first of `names` this part has, else its idle anim, else any.
    pub fn anim(&self, names: &[&str]) -> Option<&Anim> {
        names.iter().chain(&["idle"]).find_map(|n| self.anims.get(*n)).or_else(|| self.anims.values().next())
    }
}

/// The JSON files that may hold the studio's sprites for each (id, kind), for a loader that cannot list folders.
pub fn candidates<'a>(ids: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    ids.into_iter()
        .flat_map(|(id, kind)| folders(kind).iter().map(move |f| format!("{SPRITES}/{f}/{id}.json")))
        .collect()
}

/// An atlas page's two files: the picture and its team mask.
pub fn page_files(atlas: &str) -> [String; 2] {
    [format!("{SPRITES}/{atlas}.png"), format!("{SPRITES}/{atlas}.mask.png")]
}

/// Paint `ramp` (dark to light) over the team paint the mask marks: each painted pixel takes the ramp's shade at its
/// own brightness, mixed in by the mask's strength, so the paint keeps its shading.
pub fn paint(rgba: &mut [u8], mask: &[u8], ramp: &[[u8; 3]]) {
    if ramp.is_empty() {
        return;
    }
    for (px, m) in rgba.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
        let m = u32::from(m[0]);
        if m == 0 {
            continue;
        }
        let luma = (u32::from(px[0]) * 54 + u32::from(px[1]) * 183 + u32::from(px[2]) * 19) / 256;
        // Position along the ramp in 256ths of a step.
        let at = luma * (ramp.len() as u32 - 1);
        let (i, t) = ((at / 255) as usize, (at % 255) * 256 / 255);
        let (a, b) = (ramp[i.min(ramp.len() - 1)], ramp[(i + 1).min(ramp.len() - 1)]);
        for c in 0..3 {
            let team = (u32::from(a[c]) * (256 - t) + u32::from(b[c]) * t) / 256;
            px[c] = ((u32::from(px[c]) * (255 - m) + team * m) / 255) as u8;
        }
    }
}

/// Each entity's sprites with its atlas page in every owner's colours.
#[derive(Default)]
pub struct Studio {
    pub sprites: BTreeMap<String, Sprite>,
    /// Each page once per owner ramp, in owner order.
    pub pages: BTreeMap<String, Vec<TexId>>,
}

impl Studio {
    /// `id`'s sprites and the page they are on in `owner`'s colours.
    pub fn get(&self, id: &str, owner: u32) -> Option<(&Sprite, TexId)> {
        let s = self.sprites.get(id)?;
        let pages = self.pages.get(&s.atlas)?;
        Some((s, *pages.get(owner as usize % pages.len().max(1))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TANK: &str = r#"{"id": "t", "atlas": "units-0", "facings": 32, "scale": 2, "parts": {
        "hull": {"facings": 16, "anims": {"idle": {"length": 1, "frames": [[0, 0, 30, 48, 15, 27]], "shadow": [[30, 0, 35, 53, 15, 27]]}}},
        "turret": {"facings": 32, "pivotOffset": [0, 0], "anims": {"idle": {"length": 1, "frames": [[0, 50, 21, 46, 11, 38]]}}},
        "wreck": {"facings": 8, "anims": {"idle": {"length": 1, "frames": []}}}}}"#;

    #[test]
    fn a_packed_sprite_parses() {
        let s = Sprite::parse(TANK).unwrap();
        assert_eq!((s.atlas.as_str(), s.scale), ("units-0", 2.0));
        let hull = s.body().unwrap();
        assert_eq!(hull.facings, 16);
        assert_eq!(
            hull.anim(&["walk"]).unwrap().frames[0],
            Frame { src: Rect::new(0.0, 0.0, 30.0, 48.0), pivot: (15.0, 27.0) }
        );
        assert_eq!(s.turret().unwrap().facings, 32);
        assert!(Sprite::parse(r#"{"atlas": "a", "parts": {"x": {"anims": {"idle": {"frames": [[1, 2]]}}}}}"#).is_err());
    }

    #[test]
    fn frames_go_facing_by_facing_and_wrap_their_cycle() {
        let walk = Anim {
            length: 6,
            facings: 8,
            frames: vec![Frame { src: Rect::new(0.0, 0.0, 1.0, 1.0), pivot: (0.0, 0.0) }; 48],
            shadow: vec![],
            muzzle: vec![],
        };
        assert_eq!(walk.index(64, 0), 12, "east is the third of eight facings");
        assert_eq!(walk.index(64, 7), 13);
        assert_eq!(walk.index(250, 0), 0, "nearly north rounds to north");
        let death = Anim { length: 8, facings: 1, ..walk };
        assert_eq!(death.index(64, 3), 3, "one-sided anims ignore the facing");
    }

    #[test]
    fn muzzle_points_parse_and_turrets_get_theirs_measured() {
        let soldier = r#"{"atlas": "infantry-0", "scale": 2, "parts": {"body": {"facings": 8, "anims": {
            "idle": {"length": 1, "frames": [[0, 0, 4, 4, 2, 4]], "muzzle": [[1, -3, 0]]},
            "fire": {"length": 1, "frames": [[0, 0, 4, 4, 2, 4]], "muzzle": [[2, -2, 1]]}}}}}"#;
        let s = Sprite::parse(soldier).unwrap();
        assert_eq!(s.muzzle(&["fire"], 0, 0, 0), Some(Muzzle { x: 2.0, y: -2.0, hidden: true }));
        assert_eq!(s.muzzle(&["walk"], 0, 0, 0), Some(Muzzle { x: 1.0, y: -3.0, hidden: false }), "idle stands in");
        assert!(
            Sprite::parse(r#"{"atlas": "a", "parts": {"x": {"anims": {"idle": {"frames": [], "muzzle": [[1]]}}}}}"#)
                .is_err()
        );

        // A turret on a 16 by 16 page: facing north, a 2-pixel barrel from the middle up to the top; facing east, a
        // barrel out to the right. Each frame is 8 by 8 with its pivot in the middle.
        let mut page = vec![0u8; 16 * 16 * 4];
        let mut put = |x: u32, y: u32| page[((y * 16 + x) * 4 + 3) as usize] = 255;
        for y in 3..6 {
            for x in 3..6 {
                put(x, y);
                put(x + 8, y);
            }
        }
        for y in 0..3 {
            put(4, y);
        }
        for x in 6..8 {
            put(x + 8, 4);
        }
        let mut tank = Sprite::parse(
            r#"{"atlas": "units-0", "parts": {"turret": {"facings": 4, "anims": {"idle": {"length": 1,
            "frames": [[0, 0, 8, 8, 4, 4], [8, 0, 8, 8, 4, 4]]}}}}}"#,
        )
        .unwrap();
        tank.find_muzzles(&page, 16, true);
        let north = tank.muzzle(&[], 0, 0, 0).unwrap();
        let east = tank.muzzle(&[], 0, 64, 0).unwrap();
        assert_eq!((north.x, north.y), (0.5, -3.5), "the tip of the barrel, above the pivot");
        assert_eq!((east.x, east.y), (3.5, 0.5), "and out to the right facing east");
    }

    #[test]
    fn team_paint_takes_the_ramp_and_keeps_its_shading() {
        let ramp = [[0, 0, 40], [0, 0, 100], [0, 0, 180], [0, 0, 255]];
        let mut px = vec![128, 128, 128, 255, 20, 20, 20, 255, 200, 50, 50, 255];
        let mask = vec![255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255];
        paint(&mut px, &mask, &ramp);
        assert!(px[0] == 0 && px[2] > 100 && px[2] < 180, "mid grey lands mid-ramp: {:?}", &px[..4]);
        assert!(px[6] < px[2], "darker paint takes a darker shade");
        assert_eq!(&px[8..12], &[200, 50, 50, 255], "unpainted pixels are left alone");
    }
}
