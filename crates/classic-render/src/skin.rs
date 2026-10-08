//! A setting pack's UI skin (`theme/theme.json`, drawn for the generic pack by `classic-tools`' `art/theme.rs`):
//! nine-slice frames for the rail, panels, wells, tabs, tooltips and buttons, the faction emblems, the mouse cursors
//! and the fonts (`theme/fonts/fonts.json`, baked by `art/fonts/bake.py`). Design: `plans/rts/ui.md` sections 1, 2
//! and 9, `plans/rts/art-pipeline.md` section 9.
//!
//! Packs are read in order, the pack's own first: each frame, emblem and cursor comes from the first pack that
//! lists it, and the fonts from the first pack that has any, so a pack can bring only its emblems and keep the
//! generic frames. Whatever no pack has falls back to plain fills and the built-in pixel font, so a pack with no
//! theme still plays.
//!
//! `SkinFiles` is the decoded files, with no GPU; `Skin` is the same on the GPU, ready to draw.

use std::collections::BTreeMap;

use classic_data::json::{self, Value};
use classic_sim::Game;
use classic_sim::Terrain;

use crate::art::decode_png;
use crate::platform::{Files, Font, Gpu, Rect, SpriteBatch, TexId};

/// Where a pack's skin is indexed.
pub const THEME_INDEX: &str = "theme/theme.json";

/// A text style. Each pack font bakes every style at UI scales 1 to 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Style {
    /// Labels on icons, health numbers.
    Small,
    /// Body text: names, the message feed, tooltips.
    Body,
    /// Headings and numbers: credits, power, the clock.
    Heading,
    /// Menu titles.
    Title,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Small, Style::Body, Style::Heading, Style::Title];

    pub fn id(self) -> &'static str {
        match self {
            Style::Small => "small",
            Style::Body => "body",
            Style::Heading => "heading",
            Style::Title => "title",
        }
    }

    /// The built-in pixel font's scale for this style at UI scale 1.
    fn builtin(self) -> f32 {
        match self {
            Style::Small => 1.0,
            Style::Body | Style::Heading => 2.0,
            Style::Title => 4.0,
        }
    }
}

/// A button's state, in the order of the button strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Normal = 0,
    Hover = 1,
    Pressed = 2,
    Disabled = 3,
}

/// A decoded picture: width, height, RGBA pixels.
#[derive(Clone, Debug)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct FrameArt {
    pub image: Image,
    /// One frame's size; a strip holds one frame per state.
    pub frame: (u32, u32),
    /// Corner size in pixels.
    pub slice: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub src: Rect,
    /// Where the glyph's box sits: from the pen, and from the line's top, in atlas pixels.
    pub left: f32,
    pub top: f32,
    /// How far the pen moves after it, in atlas pixels.
    pub advance: f32,
}

#[derive(Clone, Debug)]
pub struct FontAtlas {
    pub image: Image,
    /// The UI scale it was baked for.
    pub scale: u32,
    pub line: f32,
    pub ascent: f32,
    pub glyphs: BTreeMap<char, Glyph>,
}

impl FontAtlas {
    fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c).or_else(|| self.glyphs.get(&'?'))
    }

    /// Width of `text` in this atlas's pixels.
    fn width(&self, text: &str) -> f32 {
        text.chars().filter_map(|c| self.glyph(c)).map(|g| g.advance).sum()
    }
}

#[derive(Clone, Debug)]
pub struct CursorArt {
    pub image: Image,
    /// Twice the size, for screens at twice the density.
    pub image_2x: Option<Image>,
    /// In pixels of `image`.
    pub hotspot: (u32, u32),
}

/// A pack's skin as decoded files.
#[derive(Clone, Debug, Default)]
pub struct SkinFiles {
    pub frames: BTreeMap<String, FrameArt>,
    pub fonts: BTreeMap<(Style, u32), FontAtlas>,
    pub emblems: BTreeMap<String, Image>,
    pub cursors: BTreeMap<String, CursorArt>,
}

fn image(files: &Files, path: &str) -> Result<Image, String> {
    let (w, h, rgba) = decode_png(&files.read(path)?, &files.name(path))?;
    Ok(Image { w, h, rgba })
}

fn pair(v: Option<&Value>) -> Option<(i64, i64)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_int()?, a.get(1)?.as_int()?))
}

impl SkinFiles {
    /// Read the skins of `packs`, the pack's own first. A pack with no `theme/theme.json` adds nothing; a broken
    /// one is an error.
    pub fn load(packs: &[&Files]) -> Result<SkinFiles, String> {
        let mut theme = SkinFiles::default();
        for files in packs {
            let Ok(text) = files.read_text(THEME_INDEX) else { continue };
            let doc = json::parse(&text).map_err(|e| format!("{}: {e:?}", files.name(THEME_INDEX)))?;
            let entries = |key: &str| doc.get(key).and_then(Value::as_object).unwrap_or(&[]);
            let file = |e: &Value, key: &str| e.get(key).and_then(Value::as_str).map(str::to_string);
            for (id, e) in entries("frames") {
                if theme.frames.contains_key(id) {
                    continue;
                }
                let path = file(e, "file").ok_or_else(|| format!("{THEME_INDEX}: frame {id} has no file"))?;
                let picture = image(files, &path)?;
                let (fw, fh) = pair(e.get("frame")).unwrap_or((picture.w as i64, picture.h as i64));
                let slice = e.get("slice").and_then(Value::as_int).unwrap_or(0);
                if fw <= 0 || fh <= 0 || slice < 0 || 2 * slice > fw.min(fh) {
                    return Err(format!("{THEME_INDEX}: frame {id}: slice {slice} doesn't fit {fw}x{fh}"));
                }
                let frame = (fw as u32, fh as u32);
                theme.frames.insert(id.clone(), FrameArt { image: picture, frame, slice: slice as u32 });
            }
            for (id, e) in entries("emblems") {
                if !theme.emblems.contains_key(id) {
                    let path = file(e, "file_2x").or_else(|| file(e, "file")).ok_or("emblem without a file")?;
                    theme.emblems.insert(id.clone(), image(files, &path)?);
                }
            }
            for (id, e) in entries("cursors") {
                if theme.cursors.contains_key(id) {
                    continue;
                }
                let path = file(e, "file").ok_or_else(|| format!("{THEME_INDEX}: cursor {id} has no file"))?;
                let picture = image(files, &path)?;
                let image_2x = file(e, "file_2x").map(|p| image(files, &p)).transpose()?;
                let (hx, hy) = pair(e.get("hotspot")).unwrap_or((0, 0));
                if hx < 0 || hy < 0 || hx >= picture.w as i64 || hy >= picture.h as i64 {
                    return Err(format!("{THEME_INDEX}: cursor {id}: hotspot ({hx}, {hy}) is off its picture"));
                }
                theme
                    .cursors
                    .insert(id.clone(), CursorArt { image: picture, image_2x, hotspot: (hx as u32, hy as u32) });
            }
            if theme.fonts.is_empty()
                && let Some(index) = doc.get("fonts").and_then(Value::as_str)
            {
                theme.fonts = load_fonts(files, index)?;
            }
        }
        Ok(theme)
    }
}

/// The atlases a fonts index lists, by style and UI scale.
fn load_fonts(files: &Files, index: &str) -> Result<BTreeMap<(Style, u32), FontAtlas>, String> {
    let doc = json::parse(&files.read_text(index)?).map_err(|e| format!("{}: {e:?}", files.name(index)))?;
    let mut out = BTreeMap::new();
    for a in doc.get("atlases").and_then(Value::as_array).unwrap_or(&[]) {
        let name = a.get("style").and_then(Value::as_str).unwrap_or("");
        let Some(style) = Style::ALL.into_iter().find(|s| s.id() == name) else { continue };
        let scale = a.get("scale").and_then(Value::as_int).unwrap_or(1).max(1) as u32;
        let path = a.get("file").and_then(Value::as_str).ok_or_else(|| format!("{index}: atlas without a file"))?;
        let mut glyphs = BTreeMap::new();
        for g in a.get("glyphs").and_then(Value::as_array).unwrap_or(&[]) {
            let n: Vec<i64> = g.as_array().unwrap_or(&[]).iter().filter_map(Value::as_int).collect();
            let [code, x, y, w, h, left, top, advance] = n[..] else {
                return Err(format!("{index}: {name} glyph {g:?} is not 8 numbers"));
            };
            let Some(c) = char::from_u32(code as u32) else { continue };
            let src = Rect::new(x as f32, y as f32, w as f32, h as f32);
            glyphs.insert(c, Glyph { src, left: left as f32, top: top as f32, advance: advance as f32 / 64.0 });
        }
        let line = a.get("line").and_then(Value::as_int).unwrap_or(0) as f32;
        let ascent = a.get("ascent").and_then(Value::as_int).unwrap_or(0) as f32;
        out.insert((style, scale), FontAtlas { image: image(files, path)?, scale, line, ascent, glyphs });
    }
    Ok(out)
}

/// The nine pieces of a frame drawn into `dst`: (source in the frame, destination). Corners keep their size times
/// `scale`, shrunk to fit a small `dst`; edges stretch along their length; the middle stretches both ways.
pub fn nine_slice(frame: (f32, f32), slice: f32, dst: Rect, scale: f32) -> Vec<(Rect, Rect)> {
    let (fw, fh) = frame;
    let c = (slice * scale).round().min((dst.w / 2.0).floor()).min((dst.h / 2.0).floor()).max(0.0);
    let xs = [0.0, slice, fw - slice, fw];
    let ys = [0.0, slice, fh - slice, fh];
    let (x0, y0) = (dst.x.round(), dst.y.round());
    let (x1, y1) = ((dst.x + dst.w).round(), (dst.y + dst.h).round());
    let dx = [x0, x0 + c, x1 - c, x1];
    let dy = [y0, y0 + c, y1 - c, y1];
    let mut out = Vec::with_capacity(9);
    for j in 0..3 {
        for i in 0..3 {
            let src = Rect::new(xs[i], ys[j], xs[i + 1] - xs[i], ys[j + 1] - ys[j]);
            let d = Rect::new(dx[i], dy[j], dx[i + 1] - dx[i], dy[j + 1] - dy[j]);
            if src.w > 0.0 && src.h > 0.0 && d.w > 0.0 && d.h > 0.0 {
                out.push((src, d));
            }
        }
    }
    out
}

/// The atlas scale to draw `ui` with, and how much to stretch it: the nearest baked scale at or below.
fn atlas_scale(ui: f32, baked: impl Iterator<Item = u32>) -> Option<(u32, f32)> {
    let want = ui.round().max(1.0) as u32;
    let mut scales: Vec<u32> = baked.collect();
    scales.sort_unstable();
    let pick = scales.iter().rev().find(|&&s| s <= want).or(scales.first()).copied()?;
    Some((pick, ui / pick as f32))
}

struct GpuFrame {
    tex: TexId,
    frame: (f32, f32),
    slice: f32,
}

struct GpuFont {
    tex: TexId,
    atlas: FontAtlas,
}

/// The skin on the GPU.
pub struct Skin {
    frames: BTreeMap<String, GpuFrame>,
    fonts: BTreeMap<(Style, u32), GpuFont>,
    emblems: BTreeMap<String, (TexId, f32, f32)>,
    builtin: Font,
    /// The cursors stay on the CPU: the window system draws them.
    pub cursors: BTreeMap<String, CursorArt>,
}

impl Skin {
    pub fn new(gpu: &Gpu, batch: &mut SpriteBatch, theme: SkinFiles) -> Skin {
        let mut up = |i: &Image| batch.texture(gpu, i.w, i.h, &i.rgba);
        let frames = theme
            .frames
            .iter()
            .map(|(id, f)| {
                let g =
                    GpuFrame { tex: up(&f.image), frame: (f.frame.0 as f32, f.frame.1 as f32), slice: f.slice as f32 };
                (id.clone(), g)
            })
            .collect();
        let emblems = theme.emblems.iter().map(|(id, i)| (id.clone(), (up(i), i.w as f32, i.h as f32))).collect();
        let fonts = theme.fonts.into_iter().map(|(k, atlas)| (k, GpuFont { tex: up(&atlas.image), atlas })).collect();
        Skin { frames, fonts, emblems, builtin: Font::new(gpu, batch), cursors: theme.cursors }
    }

    /// Load and upload the skins of `packs`, the pack's own first.
    pub fn load(gpu: &Gpu, batch: &mut SpriteBatch, packs: &[&Files]) -> Result<Skin, String> {
        Ok(Skin::new(gpu, batch, SkinFiles::load(packs)?))
    }

    /// Whether the pack has frame `id`.
    pub fn has(&self, id: &str) -> bool {
        self.frames.contains_key(id)
    }

    /// Frame `id` (state `state` of a strip) over `r`, its corners at UI scale `ui`. Without that frame, a plain
    /// `fallback` fill with a lighter edge.
    pub fn frame(&self, batch: &mut SpriteBatch, id: &str, state: usize, r: Rect, ui: f32, fallback: [u8; 4]) {
        let Some(f) = self.frames.get(id) else {
            batch.fill(r, fallback);
            batch.outline(r, 1.0, [90, 96, 104, 255]);
            return;
        };
        let x0 = state as f32 * f.frame.0;
        for (src, dst) in nine_slice(f.frame, f.slice, r, ui) {
            batch.sprite(f.tex, Rect::new(src.x + x0, src.y, src.w, src.h), dst, [255; 4]);
        }
    }

    pub fn button(&self, batch: &mut SpriteBatch, r: Rect, state: ButtonState, ui: f32) {
        let fallback = match state {
            ButtonState::Normal => [58, 68, 80, 255],
            ButtonState::Hover => [72, 84, 102, 255],
            ButtonState::Pressed => [42, 50, 60, 255],
            ButtonState::Disabled => [42, 46, 52, 255],
        };
        self.frame(batch, "button", state as usize, r, ui, fallback);
    }

    /// Faction `id`'s emblem fitted into `r`, if the pack has one.
    pub fn emblem(&self, batch: &mut SpriteBatch, id: &str, r: Rect) -> bool {
        let Some(&(tex, w, h)) = self.emblems.get(id) else { return false };
        let fit = (r.w / w).min(r.h / h);
        let (dw, dh) = (w * fit, h * fit);
        batch.sprite(
            tex,
            Rect::new(0.0, 0.0, w, h),
            Rect::new(r.x + (r.w - dw) / 2.0, r.y + (r.h - dh) / 2.0, dw, dh),
            [255; 4],
        );
        true
    }

    fn font(&self, style: Style, ui: f32) -> Option<(&GpuFont, f32)> {
        let baked = self.fonts.keys().filter(|(s, _)| *s == style).map(|&(_, k)| k);
        let (k, stretch) = atlas_scale(ui, baked)?;
        Some((&self.fonts[&(style, k)], stretch))
    }

    /// Width of `text` in `style` at UI scale `ui`, in screen pixels.
    pub fn width(&self, style: Style, text: &str, ui: f32) -> f32 {
        match self.font(style, ui) {
            Some((f, stretch)) => (f.atlas.width(text) * stretch).ceil(),
            None => Font::width(&text.to_uppercase(), (style.builtin() * ui).round().max(1.0)),
        }
    }

    /// Height of a line of `style` at UI scale `ui`.
    pub fn line(&self, style: Style, ui: f32) -> f32 {
        match self.font(style, ui) {
            Some((f, stretch)) => (f.atlas.line * stretch).round(),
            None => Font::height((style.builtin() * ui).round().max(1.0)),
        }
    }

    /// Draw `text` in `style` with the top of its line at (x, y); returns its width.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &self,
        batch: &mut SpriteBatch,
        style: Style,
        text: &str,
        x: f32,
        y: f32,
        ui: f32,
        colour: [u8; 4],
    ) -> f32 {
        let Some((f, stretch)) = self.font(style, ui) else {
            return self.builtin.draw(batch, text, x, y, (style.builtin() * ui).round().max(1.0), colour);
        };
        let mut pen = x;
        for c in text.chars() {
            let Some(g) = f.atlas.glyph(c) else { continue };
            if g.src.w > 0.0 {
                // Whole pixels, so glyphs stay crisp under the nearest-pixel sampler.
                let dst = Rect::new(
                    (pen + g.left * stretch).round(),
                    (y + g.top * stretch).round(),
                    g.src.w * stretch,
                    g.src.h * stretch,
                );
                batch.sprite(f.tex, g.src, dst, colour);
            }
            pen += g.advance * stretch;
        }
        (pen - x).ceil()
    }

    /// Cursor `id` and its hotspot: the larger picture on a screen at `ui` of 2 or more when the pack has one.
    pub fn cursor(&self, id: &str, ui: f32) -> Option<(&Image, (u32, u32))> {
        let c = self.cursors.get(id)?;
        match (&c.image_2x, ui >= 2.0) {
            (Some(i), true) => Some((i, (c.hotspot.0 * 2, c.hotspot.1 * 2))),
            _ => Some((&c.image, c.hotspot)),
        }
    }
}

/// What is under the mouse, for choosing a cursor.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    /// Over the rail or another HUD panel.
    pub over_hud: bool,
    /// Placing a building: whether it fits where it is.
    pub placing: Option<bool>,
    /// Scrolling at a screen edge: the direction, x then y, each -1, 0 or 1.
    pub edge: (i32, i32),
    /// The entity under the mouse.
    pub hovered: Option<u32>,
    /// The tile under the mouse.
    pub tile: (i32, i32),
}

/// The cursor for `p`, by the ids of `plans/rts/ui.md` section 9. Only the orders the player can give today
/// (select, move, attack, place) get a cursor of their own; harvest, enter, carry, deploy, sell and repair are drawn
/// and wait for their orders.
pub fn choose_cursor(game: &Game, player: u32, selected: &[u32], p: &Pointer) -> &'static str {
    const SCROLL: [[&str; 3]; 3] = [
        ["scroll_nw", "scroll_n", "scroll_ne"],
        ["scroll_w", "default", "scroll_e"],
        ["scroll_sw", "scroll_s", "scroll_se"],
    ];
    if p.edge != (0, 0) {
        return SCROLL[(p.edge.1.signum() + 1) as usize][(p.edge.0.signum() + 1) as usize];
    }
    if p.over_hud {
        return "default";
    }
    if let Some(ok) = p.placing {
        return if ok { "place_ok" } else { "place_bad" };
    }
    let units: Vec<_> = selected
        .iter()
        .filter_map(|&id| game.state.entity(id))
        .filter(|e| e.owner == player && !game.rules.kind(e.kind).building)
        .collect();
    let hovered = p.hovered.and_then(|id| game.state.entity(id));
    if units.is_empty() {
        return if hovered.is_some() { "select" } else { "default" };
    }
    match hovered {
        Some(e) if e.owner != player => {
            if units.iter().any(|u| game.rules.kind(u.kind).weapon.is_some()) {
                "attack"
            } else {
                "no"
            }
        }
        Some(_) => "select",
        None => {
            let (x, y) = p.tile;
            let inside = x >= 0 && y >= 0 && x < game.map.width && y < game.map.height;
            let i = (y * game.map.width + x) as usize;
            if inside && game.map.terrain[i] != Terrain::Cliff { "move" } else { "no" }
        }
    }
}

/// The files a theme index names, for fetching in the browser: its frames, emblems, cursors and fonts index.
pub fn theme_files(index: &str) -> Vec<String> {
    let Ok(doc) = json::parse(index) else { return Vec::new() };
    let mut out = Vec::new();
    for section in ["frames", "emblems", "cursors"] {
        for (_, e) in doc.get(section).and_then(Value::as_object).unwrap_or(&[]) {
            for key in ["file", "file_2x"] {
                out.extend(e.get(key).and_then(Value::as_str).map(str::to_string));
            }
        }
    }
    out.extend(doc.get("fonts").and_then(Value::as_str).map(str::to_string));
    out
}

/// The atlases a fonts index names, for fetching in the browser.
pub fn font_files(index: &str) -> Vec<String> {
    let Ok(doc) = json::parse(index) else { return Vec::new() };
    doc.get("atlases")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|a| a.get("file").and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_slice_keeps_corners_and_covers_the_rect() {
        let parts = nine_slice((24.0, 24.0), 8.0, Rect::new(10.0, 20.0, 100.0, 50.0), 2.0);
        assert_eq!(parts.len(), 9);
        let (src, dst) = parts[0];
        assert_eq!((src.w, dst.w, dst.h), (8.0, 16.0, 16.0), "a corner keeps its size times the scale");
        let area: f32 = parts.iter().map(|(_, d)| d.w * d.h).sum();
        assert_eq!(area, 100.0 * 50.0, "the pieces tile the rectangle exactly");
        // Too small for the corners: they shrink to half the rectangle and the middle disappears.
        let tiny = nine_slice((24.0, 24.0), 8.0, Rect::new(0.0, 0.0, 10.0, 30.0), 1.0);
        assert!(tiny.iter().all(|(_, d)| d.w <= 5.0 || d.w == 0.0));
    }

    #[test]
    fn fonts_use_the_nearest_baked_scale_at_or_below() {
        let baked = || [1, 2, 3].into_iter();
        assert_eq!(atlas_scale(1.0, baked()), Some((1, 1.0)));
        assert_eq!(atlas_scale(2.0, baked()), Some((2, 1.0)));
        assert_eq!(atlas_scale(4.0, baked()), Some((3, 4.0 / 3.0)));
        assert_eq!(atlas_scale(0.75, baked()), Some((1, 0.75)));
        assert_eq!(atlas_scale(1.0, std::iter::empty()), None);
    }
}
