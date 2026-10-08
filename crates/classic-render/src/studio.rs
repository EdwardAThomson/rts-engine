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
        };
        assert_eq!(walk.index(64, 0), 12, "east is the third of eight facings");
        assert_eq!(walk.index(64, 7), 13);
        assert_eq!(walk.index(250, 0), 0, "nearly north rounds to north");
        let death = Anim { length: 8, facings: 1, ..walk };
        assert_eq!(death.index(64, 3), 3, "one-sided anims ignore the facing");
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
