//! The generic pack's UI skin, drawn from code: nine-slice frames for the rail, panels, wells, tabs and tooltips,
//! buttons in four states, the mouse cursors and the faction emblems. All of it is our own plain geometry: steel
//! bevels, arrows and rings, nothing taken from any game. `theme/theme.json` indexes it for the player. The fonts
//! in `theme/fonts/` are baked from open-licence faces by `art/fonts/bake.py`, not drawn here.
//!
//! Cursors and emblems are drawn four times larger and shrunk, so their edges are smooth; the frames are drawn
//! pixel by pixel, since nine-slicing stretches their middles and keeps their corners.

use super::canvas::{Canvas, Pen, Rgba};

/// A finished picture: its path in the pack, and the picture.
pub struct Picture {
    pub path: String,
    pub canvas: Canvas,
}

/// How much larger cursors and emblems are drawn before shrinking.
const SUPER: usize = 4;

const fn rgb(c: u32) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

const DARK: Rgba = rgb(0x0b0d10);
const ACCENT: Rgba = rgb(0xd9b43a);
const ACCENT_DARK: Rgba = rgb(0x8a6f1e);

// ---------------------------------------------------------------------------------------------------- frames

/// One ring of a bevelled frame, outside in: its colour on the top and left sides, and on the bottom and right.
type Ring = (Rgba, Rgba);

const fn flat(c: u32) -> Ring {
    (rgb(c), rgb(c))
}

/// A `w` by `h` frame: `rings` from the edge inwards, then `face` filling the middle. Each ring is lit on its top
/// and left sides and shaded on its bottom and right, so a frame reads as raised or, swapped, as sunk.
fn bevel(w: usize, h: usize, rings: &[Ring], face: Rgba) -> Canvas {
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let e = x.min(y).min(w - 1 - x).min(h - 1 - y);
            c.px[y * w + x] = match rings.get(e) {
                Some(&(lit, shaded)) => {
                    let top_left = (y == e && x < w - 1 - e) || (x == e && y < h - 1 - e);
                    if top_left { lit } else { shaded }
                }
                None => face,
            };
        }
    }
    c
}

/// Bolt heads: a lit pixel with a shaded one below and to its right, at each of the four corners, `at` pixels in.
fn bolts(c: &mut Canvas, at: usize) {
    let (w, h) = (c.w, c.h);
    for (x, y) in [(at, at), (w - 2 - at, at), (at, h - 2 - at), (w - 2 - at, h - 2 - at)] {
        c.px[y * w + x] = rgb(0xb8c0c8);
        c.px[y * w + x + 1] = rgb(0x6a7480);
        c.px[(y + 1) * w + x] = rgb(0x6a7480);
        c.px[(y + 1) * w + x + 1] = rgb(0x22272e);
    }
}

/// The production rail: thick riveted steel, a sunk groove and an amber trim around a dark face. 32 pixels square,
/// sliced 12 from each edge.
fn rail() -> Canvas {
    let rings = [
        flat(0x0b0d10),
        (rgb(0x9aa4b0), rgb(0x1a1f25)),
        (rgb(0x5a6470), rgb(0x343b44)),
        flat(0x4a535e),
        flat(0x4a535e),
        flat(0x46505a),
        (rgb(0x20262d), rgb(0x6a7480)),
        flat(0x0b0d10),
        (ACCENT, ACCENT_DARK),
        flat(0x5c4a14),
        flat(0x101317),
        flat(0x161a20),
    ];
    let mut c = bevel(32, 32, &rings, [0x1b, 0x20, 0x27, 245]);
    bolts(&mut c, 3);
    c
}

/// Panels (the readout, the selection card, the menus): a lighter version of the rail's edge. 24 square, sliced 8.
fn panel() -> Canvas {
    let rings = [
        flat(0x0b0d10),
        (rgb(0x8d97a3), rgb(0x1a1f25)),
        (rgb(0x56606c), rgb(0x3a424c)),
        flat(0x4a535e),
        (rgb(0x20262d), rgb(0x6a7480)),
        flat(0x0b0d10),
        flat(0x232930),
        flat(0x1d2229),
    ];
    let mut c = bevel(24, 24, &rings, [0x1b, 0x20, 0x27, 240]);
    bolts(&mut c, 2);
    c
}

/// A sunk well for build icons, the minimap and gauges: shaded top and left, lit bottom and right. 12, sliced 4.
fn inset() -> Canvas {
    let rings = [(rgb(0x07080a), rgb(0x59636e)), (rgb(0x0e1114), rgb(0x2c333b)), flat(0x111418), flat(0x12161a)];
    bevel(12, 12, &rings, rgb(0x13171c))
}

/// Tooltips: a thin steel line, an amber line along the top, and a nearly black face. 12 square, sliced 4.
fn tooltip() -> Canvas {
    let rings = [flat(0x5a6470), flat(0x08090b), flat(0x0d1013), flat(0x0d1013)];
    let mut c = bevel(12, 12, &rings, [0x0d, 0x10, 0x13, 235]);
    for x in 1..11 {
        c.px[12 + x] = ACCENT;
    }
    c
}

/// The rail's factory tabs, closed then open, side by side; 16 square each, sliced 5. The open tab is edged in
/// amber.
fn tabs() -> Canvas {
    let closed = [flat(0x0b0d10), (rgb(0x6a7480), rgb(0x1a1f25)), flat(0x3a434e), flat(0x2e353e), flat(0x2e353e)];
    let open = [(ACCENT, ACCENT_DARK), (rgb(0xf0dc90), rgb(0x5c4a14)), flat(0x4a5562), flat(0x3e4854), flat(0x3e4854)];
    let mut c = Canvas::new(32, 16);
    c.stamp(&bevel(16, 16, &closed, rgb(0x2c333c)), 0, 0);
    c.stamp(&bevel(16, 16, &open, rgb(0x3a4450)), 16, 0);
    c
}

/// Buttons in four states side by side, normal, hover, pressed and disabled; 24 by 16 each, sliced 5.
fn buttons() -> Canvas {
    let states: [(&[Ring], Rgba); 4] = [
        (&[flat(0x0b0d10), (rgb(0x7a8694), rgb(0x14181c)), (rgb(0x4a5562), rgb(0x2a323c))], rgb(0x3a4450)),
        (&[(ACCENT, ACCENT_DARK), (rgb(0xa8b4c0), rgb(0x14181c)), (rgb(0x5a6676), rgb(0x323a44))], rgb(0x485466)),
        (&[flat(0x0b0d10), (rgb(0x14181c), rgb(0x6a7684)), (rgb(0x232a32), rgb(0x3a4450))], rgb(0x2a323c)),
        (&[flat(0x1a1d21), (rgb(0x3a3f46), rgb(0x24282d)), flat(0x2c3036)], rgb(0x2a2e34)),
    ];
    let mut c = Canvas::new(24 * 4, 16);
    for (i, (rings, face)) in states.iter().enumerate() {
        c.stamp(&bevel(24, 16, rings, *face), (i * 24) as i64, 0);
    }
    c
}

/// Every frame: id, picture, the frame size in pixels (a strip holds several), the slice from each edge and the
/// state names of a strip, in order.
type Frame = (&'static str, Canvas, (usize, usize), usize, &'static [&'static str]);

fn frames() -> Vec<Frame> {
    vec![
        ("rail", rail(), (32, 32), 12, &[]),
        ("panel", panel(), (24, 24), 8, &[]),
        ("inset", inset(), (12, 12), 4, &[]),
        ("tooltip", tooltip(), (12, 12), 4, &[]),
        ("tab", tabs(), (16, 16), 5, &["closed", "open"]),
        ("button", buttons(), (24, 16), 5, &["normal", "hover", "pressed", "disabled"]),
    ]
}

// ---------------------------------------------------------------------------------------------- smooth shapes

/// Shrink a picture drawn `k` times too large, averaging each `k` by `k` block (weighting colour by alpha).
fn shrink(big: &Canvas, k: usize) -> Canvas {
    let mut out = Canvas::new(big.w / k, big.h / k);
    for y in 0..out.h {
        for x in 0..out.w {
            let mut sum = [0u32; 4];
            for dy in 0..k {
                for dx in 0..k {
                    let p = big.get(x * k + dx, y * k + dy);
                    let a = p[3] as u32;
                    for i in 0..3 {
                        sum[i] += p[i] as u32 * a;
                    }
                    sum[3] += a;
                }
            }
            let n = (k * k) as u32;
            if let (Some(r), Some(g), Some(b)) =
                (sum[0].checked_div(sum[3]), sum[1].checked_div(sum[3]), sum[2].checked_div(sum[3]))
            {
                out.px[y * out.w + x] = [r as u8, g as u8, b as u8, (sum[3] / n) as u8];
            }
        }
    }
    out
}

/// The alpha of `c` grown by `r` pixels in every direction (a square brush, run across then down).
fn grow(c: &Canvas, r: usize) -> Vec<u8> {
    let (w, h) = (c.w, c.h);
    let alpha: Vec<u8> = c.px.iter().map(|p| p[3]).collect();
    let mut across = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
            across[y * w + x] = (x0..=x1).map(|i| alpha[y * w + i]).max().unwrap_or(0);
        }
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
            out[y * w + x] = (y0..=y1).map(|j| across[j * w + x]).max().unwrap_or(0);
        }
    }
    out
}

/// Draw at `SUPER` times `scale` through a pen whose units are half pixels of the picture at scale 1, give the
/// shape a dark edge `edge` pixels wide (at scale 1) and, if `shadow`, a soft shadow down and right, then shrink.
fn smooth(w: usize, h: usize, scale: usize, edge: usize, shadow: bool, draw: impl Fn(&mut Pen)) -> Canvas {
    let k = SUPER * scale;
    let mut layer = Canvas::new(w * scale * SUPER, h * scale * SUPER);
    {
        let mut pen = Pen::at(&mut layer, 0, 0);
        pen.num = k as i64;
        draw(&mut pen);
    }
    let mut out = Canvas::new(layer.w, layer.h);
    let rim = grow(&layer, edge * k);
    if shadow {
        let mut sh = Canvas::new(layer.w, layer.h);
        for (p, &a) in sh.px.iter_mut().zip(&rim) {
            *p = [0, 0, 0, (a as u32 * 90 / 255) as u8];
        }
        out.stamp(&sh, k as i64, k as i64);
    }
    if edge > 0 {
        let mut dark = Canvas::new(layer.w, layer.h);
        for (p, &a) in dark.px.iter_mut().zip(&rim) {
            *p = [DARK[0], DARK[1], DARK[2], a];
        }
        out.stamp(&dark, 0, 0);
    }
    out.stamp(&layer, 0, 0);
    shrink(&out, SUPER)
}

/// A pen at (`x`, `y`) of `p`'s drawing, turned `facing` eighth turns clockwise, for drawing one of several
/// copies of a shape around a middle.
fn turned<'a>(p: &'a mut Pen, x: i64, y: i64, facing: usize) -> Pen<'a> {
    let origin = p.map((x, y));
    Pen { canvas: &mut *p.canvas, origin, facing, num: p.num, den: p.den }
}

// ------------------------------------------------------------------------------------------------- cursors

/// Cursors are 32 pixels square at scale 1; shapes are given in half pixels, so the middle is (32, 32).
const CURSOR: usize = 32;
const MID: i64 = 32;

const LIGHT: Rgba = rgb(0xe8ecf0);
const GREEN: Rgba = rgb(0x78dc6e);
const PALE_GREEN: Rgba = rgb(0xaaf0a0);
const RED: Rgba = rgb(0xeb483c);
const AMBER: Rgba = rgb(0xf0be46);
const CYAN: Rgba = rgb(0x6ecdf0);
const TEAL: Rgba = rgb(0x78e6c8);
const SKY: Rgba = rgb(0x96c8ff);
const ORANGE: Rgba = rgb(0xf58c32);
const STEEL: Rgba = rgb(0xc8d2dc);

/// Corner brackets `r` half pixels out from the middle, arms `arm` long.
fn brackets(p: &mut Pen, r: i64, arm: i64, c: Rgba) {
    for (sx, sy) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
        let corner = (MID + sx * r, MID + sy * r);
        p.line(corner, (corner.0 - sx * arm, corner.1), 4, c);
        p.line(corner, (corner.0, corner.1 - sy * arm), 4, c);
    }
}

/// An arrow head pointing out from the middle, `from` to `to` half pixels away, in four or eight directions.
fn heads(p: &mut Pen, facings: &[usize], from: i64, to: i64, half_width: i64, c: Rgba) {
    for &f in facings {
        turned(p, MID, MID, f).poly(&[(0, -to), (half_width, -from), (-half_width, -from)], c);
    }
}

fn default_arrow(p: &mut Pen) {
    p.poly(&[(3, 3), (3, 46), (13, 37), (20, 52), (27, 49), (20, 34), (33, 34)], LIGHT);
}

fn select(p: &mut Pen) {
    brackets(p, 20, 10, PALE_GREEN);
    p.circle(MID, MID, 2, PALE_GREEN);
}

fn move_to(p: &mut Pen) {
    // Four heads pointing in at the middle.
    for f in [0, 2, 4, 6] {
        turned(p, MID, MID, f).poly(&[(0, -8), (8, -22), (-8, -22)], GREEN);
    }
    p.circle(MID, MID, 3, GREEN);
}

fn attack(p: &mut Pen) {
    p.ring(MID, MID, 20, 4, RED);
    for f in [0, 2, 4, 6] {
        turned(p, MID, MID, f).rect(-2, -28, 2, -10, RED);
    }
    p.circle(MID, MID, 2, RED);
}

fn no(p: &mut Pen) {
    p.ring(MID, MID, 22, 6, RED);
    p.line((MID - 15, MID - 15), (MID + 15, MID + 15), 6, RED);
}

fn harvest(p: &mut Pen) {
    brackets(p, 22, 10, AMBER);
    p.poly(&[(MID, MID - 11), (MID + 11, MID), (MID, MID + 11), (MID - 11, MID)], AMBER);
}

fn enter(p: &mut Pen) {
    p.line((14, 30), (14, 50), 5, CYAN);
    p.line((14, 50), (50, 50), 5, CYAN);
    p.line((50, 50), (50, 30), 5, CYAN);
    p.rect(29, 8, 35, 30, CYAN);
    p.poly(&[(21, 28), (43, 28), (32, 42)], CYAN);
}

fn repair_pad(p: &mut Pen) {
    for (a, b) in [((14, 14), (50, 14)), ((50, 14), (50, 50)), ((50, 50), (14, 50)), ((14, 50), (14, 14))] {
        p.line(a, b, 4, TEAL);
    }
    p.rect(28, 20, 36, 44, TEAL);
    p.rect(20, 28, 44, 36, TEAL);
}

fn carry(p: &mut Pen) {
    p.line((16, 8), (48, 8), 4, SKY);
    p.rect(29, 26, 35, 54, SKY);
    p.poly(&[(32, 13), (45, 29), (19, 29)], SKY);
}

fn deploy(p: &mut Pen) {
    heads(p, &[0, 2, 4, 6], 13, 28, 9, AMBER);
    p.rect(MID - 6, MID - 6, MID + 6, MID + 6, AMBER);
}

fn place_ok(p: &mut Pen) {
    brackets(p, 24, 14, GREEN);
    p.circle(MID, MID, 3, GREEN);
}

fn place_bad(p: &mut Pen) {
    brackets(p, 24, 14, RED);
    p.line((MID - 12, MID - 12), (MID + 12, MID + 12), 6, RED);
    p.line((MID + 12, MID - 12), (MID - 12, MID + 12), 6, RED);
}

fn sell(p: &mut Pen) {
    p.circle(MID, MID, 20, AMBER);
    p.ring(MID, MID, 15, 3, rgb(0x9a6e1a));
    p.rect(MID - 3, MID - 11, MID + 3, MID + 11, rgb(0x9a6e1a));
    p.rect(MID - 8, MID - 7, MID + 8, MID - 3, rgb(0x9a6e1a));
    p.rect(MID - 8, MID + 3, MID + 8, MID + 7, rgb(0x9a6e1a));
}

fn repair(p: &mut Pen) {
    // A box-end spanner: a handle and a ring.
    p.line((12, 52), (37, 27), 7, STEEL);
    p.ring(43, 21, 12, 5, STEEL);
}

fn target(p: &mut Pen) {
    p.ring(MID, MID, 21, 3, ORANGE);
    for f in [0, 2, 4, 6] {
        turned(p, MID, MID, f).rect(-2, -30, 2, -14, ORANGE);
    }
    p.poly(&[(MID, MID - 5), (MID + 5, MID), (MID, MID + 5), (MID - 5, MID)], ORANGE);
}

/// How far the scroll arrows' tips are from the middle, in half pixels.
const SCROLL_TIP: i64 = 26;

fn scroll(p: &mut Pen, facing: usize) {
    let pts = [(0, -SCROLL_TIP), (14, -8), (5, -8), (5, 8), (-5, 8), (-5, -8), (-14, -8)];
    turned(p, MID, MID, facing).poly(&pts, LIGHT);
}

/// The scroll arrows' names, clockwise from north, as their facing.
const SCROLLS: [&str; 8] =
    ["scroll_n", "scroll_ne", "scroll_e", "scroll_se", "scroll_s", "scroll_sw", "scroll_w", "scroll_nw"];

/// A cursor: id, hotspot in pixels at scale 1, and its drawing.
type Cursor = (String, (i64, i64), Box<dyn Fn(&mut Pen)>);

/// Every cursor: id, hotspot in pixels at scale 1, and its drawing. The ids are the modes of `plans/rts/ui.md`
/// section 9.
fn cursors() -> Vec<Cursor> {
    let c = MID / 2;
    let mut v: Vec<Cursor> = vec![
        ("default".into(), (1, 1), Box::new(default_arrow)),
        ("select".into(), (c, c), Box::new(select)),
        ("move".into(), (c, c), Box::new(move_to)),
        ("attack".into(), (c, c), Box::new(attack)),
        ("no".into(), (c, c), Box::new(no)),
        ("harvest".into(), (c, c), Box::new(harvest)),
        ("enter".into(), (c, c), Box::new(enter)),
        ("repair_pad".into(), (c, c), Box::new(repair_pad)),
        ("carry".into(), (c, c), Box::new(carry)),
        ("deploy".into(), (c, c), Box::new(deploy)),
        ("place_ok".into(), (c, c), Box::new(place_ok)),
        ("place_bad".into(), (c, c), Box::new(place_bad)),
        ("sell".into(), (c, c), Box::new(sell)),
        ("repair".into(), (c, c), Box::new(repair)),
        ("target".into(), (c, c), Box::new(target)),
    ];
    // The scroll arrows' hotspots are their tips, which turn with them (181 / 256 is the eighth turn's cosine).
    let tip = SCROLL_TIP / 2;
    let diag = tip * 181 / 256;
    let tips = [(0, -tip), (diag, -diag), (tip, 0), (diag, diag), (0, tip), (-diag, diag), (-tip, 0), (-diag, -diag)];
    for (f, id) in SCROLLS.iter().enumerate() {
        let (dx, dy) = tips[f];
        v.push((id.to_string(), (c + dx, c + dy), Box::new(move |p: &mut Pen| scroll(p, f))));
    }
    v
}

// ------------------------------------------------------------------------------------------------- emblems

/// Emblems are 64 pixels square at scale 1; shapes are in half pixels, so 128 across.
const EMBLEM: usize = 64;

enum Shape {
    Poly(Vec<(i64, i64)>, Rgba),
    Circle(i64, i64, i64, Rgba),
}

/// Points of a regular hexagon with a point at the top, `r` half pixels from (64, 64).
fn hexagon(r: i64) -> Vec<(i64, i64)> {
    // 222 / 256 is the cosine of a twelfth turn.
    let (x, y) = (r * 222 / 256, r / 2);
    vec![(64, 64 - r), (64 + x, 64 - y), (64 + x, 64 + y), (64, 64 + r), (64 - x, 64 + y), (64 - x, 64 - y)]
}

/// The generic factions' emblems, plain geometry in their ramp colours: rank chevrons on a hexagon, and a
/// four-pointed star on a disc.
fn emblems() -> Vec<(&'static str, Vec<Shape>)> {
    use Shape::*;
    let blue = [rgb(0x10204a), rgb(0x204a90), rgb(0x3c78d0), rgb(0x90c0ff)];
    let red = [rgb(0x4a1010), rgb(0x902020), rgb(0xd04030), rgb(0xff9a80)];
    let chevron = |y: i64| vec![(64, y), (92, y + 20), (92, y + 30), (64, y + 10), (36, y + 30), (36, y + 20)];
    let shifted = |pts: Vec<(i64, i64)>, d: i64| pts.into_iter().map(|(x, y)| (x + d, y + d)).collect();
    let mut a = vec![Poly(hexagon(62), DARK), Poly(hexagon(57), blue[3]), Poly(hexagon(52), blue[1])];
    for y in [28, 50, 72] {
        a.push(Poly(shifted(chevron(y), 3), blue[0]));
        a.push(Poly(chevron(y), STEEL));
    }
    let star = vec![(64, 14), (74, 54), (114, 64), (74, 74), (64, 114), (54, 74), (14, 64), (54, 54)];
    let b = vec![
        Circle(64, 64, 62, DARK),
        Circle(64, 64, 57, red[3]),
        Circle(64, 64, 52, red[1]),
        Poly(shifted(star.clone(), 3), red[0]),
        Poly(star, STEEL),
        Circle(64, 64, 9, red[2]),
        Circle(64, 64, 5, red[0]),
    ];
    vec![("faction_a", a), ("faction_b", b)]
}

fn draw_shapes(p: &mut Pen, shapes: &[Shape]) {
    for s in shapes {
        match s {
            Shape::Poly(pts, c) => p.poly(pts, *c),
            Shape::Circle(x, y, r, c) => p.circle(*x, *y, *r, *c),
        }
    }
}

/// The same shapes as SVG, in half pixels (a 128 unit square), for pages and anything else that scales.
fn svg(shapes: &[Shape]) -> String {
    let fill = |c: &Rgba| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 128 128\" width=\"128\" height=\"128\">\n",
    );
    for s in shapes {
        match s {
            Shape::Poly(pts, c) => {
                let pts: Vec<String> = pts.iter().map(|(x, y)| format!("{x},{y}")).collect();
                out += &format!("  <polygon points=\"{}\" fill=\"{}\"/>\n", pts.join(" "), fill(c));
            }
            Shape::Circle(x, y, r, c) => {
                out += &format!("  <circle cx=\"{x}\" cy=\"{y}\" r=\"{r}\" fill=\"{}\"/>\n", fill(c))
            }
        }
    }
    out + "</svg>\n"
}

// --------------------------------------------------------------------------------------------------- output

/// The fonts' own index, written by `art/fonts/bake.py`.
pub const FONTS_INDEX: &str = "theme/fonts/fonts.json";

/// The skin's pictures, the emblems' SVG files (path, text) and `theme/theme.json`.
pub fn generate() -> (Vec<Picture>, Vec<(String, String)>, String) {
    let mut pictures = Vec::new();
    let mut texts = Vec::new();

    let mut frame_entries = Vec::new();
    for (id, canvas, (fw, fh), slice, states) in frames() {
        let path = format!("theme/{id}.png");
        let states = if states.is_empty() {
            String::new()
        } else {
            let s: Vec<String> = states.iter().map(|s| format!("\"{s}\"")).collect();
            format!(", \"states\": [{}]", s.join(", "))
        };
        frame_entries.push(format!(
            "    \"{id}\": {{ \"file\": \"{path}\", \"frame\": [{fw}, {fh}], \"slice\": {slice}{states} }}"
        ));
        pictures.push(Picture { path, canvas });
    }

    let mut cursor_entries = Vec::new();
    for (id, (hx, hy), draw) in cursors() {
        for scale in [1, 2] {
            let suffix = if scale == 1 { "" } else { "@2x" };
            let canvas = smooth(CURSOR, CURSOR, scale, 1, true, &draw);
            pictures.push(Picture { path: format!("theme/cursors/{id}{suffix}.png"), canvas });
        }
        cursor_entries.push(format!(
            "    \"{id}\": {{ \"file\": \"theme/cursors/{id}.png\", \"file_2x\": \"theme/cursors/{id}@2x.png\", \"hotspot\": [{hx}, {hy}] }}"
        ));
    }

    let mut emblem_entries = Vec::new();
    for (faction, shapes) in emblems() {
        for scale in [1, 2] {
            let suffix = if scale == 1 { "" } else { "@2x" };
            let canvas = smooth(EMBLEM, EMBLEM, scale, 0, false, |p| draw_shapes(p, &shapes));
            pictures.push(Picture { path: format!("theme/emblems/{faction}{suffix}.png"), canvas });
        }
        texts.push((format!("theme/emblems/{faction}.svg"), svg(&shapes)));
        emblem_entries.push(format!(
            "    \"{faction}\": {{ \"file\": \"theme/emblems/{faction}.png\", \"file_2x\": \"theme/emblems/{faction}@2x.png\", \"svg\": \"theme/emblems/{faction}.svg\" }}"
        ));
    }

    let index = format!(
        "{{\n  \"about\": \"The generic pack's UI skin, drawn from code by crates/classic-tools/src/art/theme.rs (run: cargo run --bin art); the fonts are baked by art/fonts/bake.py. Paths are relative to the pack. Frames are nine-sliced: the corners keep their size (times the UI scale), the edges stretch along their length and the middle stretches both ways; 'slice' is the corner size in pixels. A strip holds one frame per state, left to right. Cursor hotspots are in pixels of the 32 pixel picture; the 2x picture's are double. Emblems are by faction id.\",\n  \"frames\": {{\n{}\n  }},\n  \"cursors\": {{\n{}\n  }},\n  \"emblems\": {{\n{}\n  }},\n  \"fonts\": \"{FONTS_INDEX}\"\n}}\n",
        frame_entries.join(",\n"),
        cursor_entries.join(",\n"),
        emblem_entries.join(",\n"),
    );
    (pictures, texts, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspots_land_on_the_cursor() {
        for (id, (hx, hy), draw) in cursors() {
            let c = smooth(CURSOR, CURSOR, 1, 1, false, &draw);
            let (x, y) = (hx as usize, hy as usize);
            let near = (y.saturating_sub(1)..=(y + 1).min(CURSOR - 1))
                .flat_map(|j| (x.saturating_sub(1)..=(x + 1).min(CURSOR - 1)).map(move |i| (i, j)))
                .any(|(i, j)| c.get(i, j)[3] > 0);
            // A ring or brackets may leave the very middle empty; the arrows and pointers must be under it.
            if id.starts_with("scroll") || id == "default" {
                assert!(near, "{id}: nothing drawn at its hotspot ({hx}, {hy})");
            }
            assert!(x < CURSOR && y < CURSOR, "{id}: hotspot outside the picture");
        }
    }
}
