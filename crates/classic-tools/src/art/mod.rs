//! Placeholder art for the public `generic` pack, drawn from code: plain top-down shapes, our own, with nothing
//! taken or traced from any game. `generate` returns every file the pack's `art/` and `theme/` folders hold, plus
//! the `art/art.json` index and `provenance.jsonl`; the `art` binary writes them, and a test checks the committed
//! files still match, so the drawings here are the source of truth.
//!
//! Faction colour: parts drawn in the four `designs::REMAP` key colours are meant to be swapped, by exact value,
//! for the four shades of the owning faction's ramp (the ramps are listed in `art.json`).

pub mod canvas;
pub mod designs;
pub mod png;

use canvas::{Canvas, Noise, Pen, SUB, finish};
use designs as d;

/// A generated file: its path inside the pack, and its bytes.
pub struct File {
    pub path: String,
    pub bytes: Vec<u8>,
}

const TILE: usize = 32;
const FACINGS: usize = 8;
const ICON: (usize, usize) = (64, 48);

type Draw = fn(&mut Pen);
type DrawBuilding = fn(&mut Pen, i64, i64);
type DrawTile = fn(&mut Canvas, usize, &mut Noise);

/// How an id looks, by kind.
enum Look {
    /// Ground unit: eight facings, a short shadow.
    Ground(Draw),
    /// Aircraft: eight facings, a long shadow.
    Air(Draw),
    /// Building: one frame, `w` by `h` tiles.
    Building(usize, usize, DrawBuilding),
    /// Turret: a fixed base and a gun drawn in eight facings.
    Turret(Draw),
    /// Map feature drawn over the ground, one frame.
    Feature(Draw),
    /// Ground tile, in four variants.
    Terrain(DrawTile),
    /// Superpower: a sidebar icon only.
    Power(Draw),
}

/// Every generic id the engine knows, in the order of `data/rules/entities.json`. Building sizes match the default
/// footprints in the rules data; the `generic_art` test checks them wherever the rules give one.
fn looks() -> Vec<(&'static str, Look)> {
    use Look::*;
    vec![
        ("construction_yard", Building(2, 2, d::construction_yard)),
        ("slab", Building(1, 1, d::slab)),
        ("slab_large", Building(2, 2, d::slab)),
        ("power_plant", Building(2, 2, d::power_plant)),
        ("refinery", Building(3, 2, d::refinery)),
        ("silo", Building(2, 2, d::silo)),
        ("radar", Building(2, 2, d::radar)),
        ("barracks", Building(2, 2, d::barracks)),
        ("barracks_heavy", Building(2, 2, d::barracks_heavy)),
        ("light_factory", Building(2, 2, d::light_factory)),
        ("heavy_factory", Building(3, 2, d::heavy_factory)),
        ("air_factory", Building(3, 2, d::air_factory)),
        ("repair_pad", Building(3, 2, d::repair_pad)),
        ("starport", Building(3, 3, d::starport)),
        ("research_lab", Building(2, 2, d::research_lab)),
        ("palace", Building(3, 3, d::palace)),
        ("wall", Building(1, 1, d::wall)),
        ("gun_turret", Turret(d::gun_turret)),
        ("rocket_turret", Turret(d::rocket_turret)),
        ("infantry", Ground(d::infantry)),
        ("infantry_squad", Ground(d::infantry_squad)),
        ("rocket_infantry", Ground(d::rocket_infantry)),
        ("rocket_squad", Ground(d::rocket_squad)),
        ("scout_bike", Ground(d::scout_bike)),
        ("raider_bike", Ground(d::raider_bike)),
        ("quad", Ground(d::quad)),
        ("harvester", Ground(d::harvester)),
        ("mcv", Ground(d::mcv)),
        ("battle_tank", Ground(d::battle_tank)),
        ("siege_tank", Ground(d::siege_tank)),
        ("missile_tank", Ground(d::missile_tank)),
        ("super_a", Ground(d::super_a)),
        ("super_h", Ground(d::super_h)),
        ("super_o", Ground(d::super_o)),
        ("carrier", Air(d::carrier)),
        ("gunship", Air(d::gunship)),
        ("elite_infantry", Ground(d::elite_infantry)),
        ("guerrilla", Ground(d::guerrilla)),
        ("saboteur", Ground(d::saboteur)),
        ("supply_ship", Air(d::supply_ship)),
        ("power_missile", Power(d::power_missile)),
        ("power_guerrillas", Power(d::power_guerrillas)),
        ("power_saboteur", Power(d::power_saboteur)),
        ("resource", Terrain(d::resource)),
        ("resource_bloom", Feature(d::resource_bloom)),
        ("hazard", Feature(d::hazard)),
        ("rock", Terrain(d::rock)),
        ("open", Terrain(d::open)),
    ]
}

/// The ids this generator draws, for checks against the rules data.
pub fn ids() -> Vec<&'static str> {
    looks().into_iter().map(|(id, _)| id).collect()
}

/// Faction colour ramps, darkest first, that a pack's `setting.json` may name.
const RAMPS: [(&str, [u32; 4]); 7] = [
    ("blue", [0x10204a, 0x204a90, 0x3c78d0, 0x90c0ff]),
    ("red", [0x4a1010, 0x902020, 0xd04030, 0xff9a80]),
    ("green", [0x103a14, 0x206a28, 0x3ca048, 0x9ee08a]),
    ("yellow", [0x4a3a08, 0x907010, 0xd0aa20, 0xfff080]),
    ("purple", [0x2a1040, 0x502080, 0x8040c0, 0xc8a0ff]),
    ("orange", [0x4a2408, 0x904810, 0xe07820, 0xffc080]),
    ("grey", [0x202224, 0x484c50, 0x80868c, 0xc8ccd0]),
];

/// A strip of `frames` frames, each `fw` by `fh`, where `draw` fills frame `i` through a pen at its centre.
fn strip(fw: usize, fh: usize, frames: usize, mut draw: impl FnMut(&mut Canvas, usize)) -> Canvas {
    let mut out = Canvas::new(fw * frames, fh);
    for i in 0..frames {
        let mut layer = Canvas::new(fw, fh);
        draw(&mut layer, i);
        out.stamp(&layer, (i * fw) as i64, 0);
    }
    out
}

/// A pen with its origin at the centre of a `w` by `h` canvas.
fn centred(c: &mut Canvas, facing: usize) -> Pen<'_> {
    let (w, h) = (c.w as i64, c.h as i64);
    Pen { canvas: c, origin: (w * SUB / 2, h * SUB / 2), facing, num: 1, den: 1 }
}

fn unit_strip(draw: Draw, shadow: i64) -> Canvas {
    strip(TILE, TILE, FACINGS, |c, f| {
        draw(&mut centred(c, f));
        *c = finish(c, d::OUTLINE, shadow);
    })
}

fn turret_strip(gun: Draw) -> Canvas {
    strip(TILE, TILE, FACINGS, |c, f| {
        d::turret_base(&mut centred(c, 0));
        let mut top = Canvas::new(TILE, TILE);
        gun(&mut centred(&mut top, f));
        c.stamp(&finish(&top, d::OUTLINE, 0), 0, 0);
    })
}

fn building(w: usize, h: usize, draw: DrawBuilding) -> Canvas {
    let mut c = Canvas::new(w * TILE, h * TILE);
    draw(&mut Pen::at(&mut c, 0, 0), (w * TILE * 2) as i64, (h * TILE * 2) as i64);
    finish(&c, d::OUTLINE, 0)
}

/// A sidebar icon: a bevelled dark panel with `layer` laid over it, already drawn at icon size.
fn icon(layer: &Canvas) -> Canvas {
    let (w, h) = ICON;
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        let t = (y * 255 / (h - 1)) as u32;
        let row = [
            ((0x2c * (255 - t) + 0x1a * t) / 255) as u8,
            ((0x34 * (255 - t) + 0x1f * t) / 255) as u8,
            ((0x40 * (255 - t) + 0x26 * t) / 255) as u8,
            255,
        ];
        for x in 0..w {
            let edge = if x == 0 || y == 0 {
                Some([0x56, 0x60, 0x6c, 255])
            } else if x == w - 1 || y == h - 1 {
                Some([0x0c, 0x0e, 0x10, 255])
            } else {
                None
            };
            c.px[y * w + x] = edge.unwrap_or(row);
        }
    }
    c.stamp(&finish(layer, d::OUTLINE, 2), 0, 0);
    c
}

fn icon_for(look: &Look) -> Option<Canvas> {
    let (w, h) = ICON;
    let mut layer = Canvas::new(w, h);
    let mid = ((w as i64) * SUB / 2, (h as i64) * SUB / 2);
    match *look {
        Look::Ground(draw) | Look::Air(draw) => {
            draw(&mut Pen { canvas: &mut layer, origin: mid, facing: 1, num: 3, den: 2 });
        }
        Look::Turret(gun) => {
            let mut pen = Pen { canvas: &mut layer, origin: mid, facing: 0, num: 3, den: 2 };
            d::turret_base(&mut pen);
            pen.facing = 1;
            gun(&mut pen);
        }
        Look::Power(draw) => draw(&mut Pen { canvas: &mut layer, origin: mid, facing: 0, num: 1, den: 1 }),
        Look::Building(bw, bh, draw) => {
            let (hw, hh) = ((bw * TILE * 2) as i64, (bh * TILE * 2) as i64);
            // Fit inside the panel with a 3 pixel margin, keeping the shape.
            let (aw, ah) = ((w as i64 - 6) * 2, (h as i64 - 6) * 2);
            let (num, den) = if aw * hh <= ah * hw { (aw, hw) } else { (ah, hh) };
            let origin = (mid.0 - hw * 8 * num / den / 2, mid.1 - hh * 8 * num / den / 2);
            draw(&mut Pen { canvas: &mut layer, origin, facing: 0, num, den }, hw, hh);
        }
        Look::Feature(_) | Look::Terrain(_) => return None,
    }
    Some(icon(&layer))
}

/// Effects: (id, frame size, frames).
const EFFECTS: [(&str, usize, usize); 12] = [
    ("explosion_small", 32, 6),
    ("explosion_medium", 48, 8),
    ("explosion_large", 64, 10),
    ("smoke_puff", 16, 6),
    ("hit_spark", 16, 3),
    ("muzzle_flash_gun", 16, 3),
    ("muzzle_flash_rocket", 16, 3),
    ("shell", 8, 1),
    ("rocket", 16, 8),
    ("crater", 32, 1),
    ("scorch", 32, 1),
    ("rubble_1x1", 32, 1),
];

/// Rubble for each building size the art uses, beyond the 1x1 listed in `EFFECTS`.
/// Each squad and the single soldier it is drawn with.
const SQUADS: [(&str, &str); 2] = [("infantry_squad", "infantry"), ("rocket_squad", "rocket_infantry")];

const RUBBLE: [(usize, usize); 3] = [(2, 2), (3, 2), (3, 3)];

fn effect(id: &str, size: usize, frames: usize) -> Canvas {
    strip(size, size, frames, |c, i| {
        let mut n = Noise::new(id);
        let (i, frames) = (i as i64, frames as i64);
        let mut p = centred(c, 0);
        match id {
            "explosion_small" | "explosion_medium" | "explosion_large" => {
                d::explosion(&mut p, &mut n, i, frames, size as i64)
            }
            "smoke_puff" => d::smoke_puff(&mut p, &mut n, i, frames),
            "hit_spark" => d::hit_spark(&mut p, &mut n, i),
            "muzzle_flash_gun" | "muzzle_flash_rocket" => {
                p.origin.1 += 3 * SUB;
                d::muzzle_flash(&mut p, i, id.ends_with("rocket"))
            }
            "shell" => d::shell(&mut p),
            "rocket" => {
                p.facing = i as usize;
                d::rocket(&mut p)
            }
            "crater" => d::crater(&mut p, &mut n),
            "scorch" => d::scorch(&mut p, &mut n),
            "rubble_1x1" => {
                p.origin = (0, 0);
                d::rubble(&mut p, &mut n, 64, 64)
            }
            _ => unreachable!("no drawing for effect {id}"),
        }
    })
}

fn rubble(w: usize, h: usize) -> Canvas {
    let mut c = Canvas::new(w * TILE, h * TILE);
    let mut n = Noise::new(&format!("rubble_{w}x{h}"));
    d::rubble(&mut Pen::at(&mut c, 0, 0), &mut n, (w * TILE * 2) as i64, (h * TILE * 2) as i64);
    c
}

/// A nine-slice panel frame, 24 pixels square with 8 pixel corners, for UI panels.
fn panel() -> Canvas {
    let mut c = Canvas::new(24, 24);
    for y in 0..24 {
        for x in 0..24 {
            let edge = x.min(y).min(23 - x).min(23 - y);
            c.px[y * 24 + x] = match edge {
                0 => [0x0c, 0x0e, 0x10, 255],
                1 if x < 23 - y => [0x7a, 0x84, 0x90, 255],
                1 => [0x22, 0x28, 0x30, 255],
                2 | 3 => [0x48, 0x52, 0x5e, 255],
                4 if x < 23 - y => [0x22, 0x28, 0x30, 255],
                4 => [0x5a, 0x64, 0x70, 255],
                _ => [0x1e, 0x24, 0x2c, 235],
            };
        }
    }
    for (x, y) in [(2, 2), (21, 2), (2, 21), (21, 21)] {
        c.px[y * 24 + x] = [0xb0, 0xb8, 0xc0, 255];
    }
    c
}

/// A button in three states side by side: normal, hover, pressed; 48 by 16 each.
fn button() -> Canvas {
    let mut c = Canvas::new(48 * 3, 16);
    for s in 0..3 {
        let face: [u8; 3] = [[0x3a, 0x44, 0x50], [0x4a, 0x58, 0x68], [0x2a, 0x32, 0x3c]][s];
        let (hi, lo) =
            if s == 2 { ([0x14, 0x18, 0x1c], [0x6a, 0x76, 0x84]) } else { ([0x7a, 0x86, 0x94], [0x14, 0x18, 0x1c]) };
        for y in 0..16 {
            for x in 0..48 {
                let col = if x == 0 || y == 0 {
                    hi
                } else if x == 47 || y == 15 {
                    lo
                } else {
                    face
                };
                c.px[y * c.w + s * 48 + x] = [col[0], col[1], col[2], 255];
            }
        }
    }
    c
}

const THEME_CSS: &str =
    "/* The generic pack's UI theme: plain dark panels. Colours only; the frames are theme/panel.png (nine-slice,
   8 px corners) and theme/button.png (normal, hover, pressed; 48 x 16 each). */
:root {
  --panel-bg: #1e242c;
  --panel-edge: #48525e;
  --panel-light: #7a8490;
  --panel-dark: #0c0e10;
  --text: #d8dde2;
  --text-dim: #8a929a;
  --accent: #d9b43a;
  --good: #5cbf60;
  --warn: #d9b43a;
  --bad: #d04030;
  --font: \"DejaVu Sans Mono\", \"Consolas\", monospace;
}
";

fn hex(c: u32) -> String {
    format!("\"#{c:06x}\"")
}

/// A finished image as a file. Faction colour only works if remap pixels keep their exact key values, so a blend
/// that tinted one (a shadow or a translucent effect over it) is a drawing mistake and stops the generator.
fn png_file(path: String, c: &Canvas) -> File {
    let tinted = c
        .px
        .iter()
        .find(|p| p[3] > 0 && p[0] == p[2] && p[0] > 0x30 && p[1] < 0x48 && !d::REMAP.iter().any(|k| k[..3] == p[..3]));
    assert!(tinted.is_none(), "{path}: {tinted:?} is a faction key colour changed by blending");
    File { path, bytes: c.png() }
}

/// Every file of the generic pack's art, theme, art index and provenance list.
pub fn generate() -> Vec<File> {
    let mut files = Vec::new();
    let mut sprites = Vec::new();
    let mut icons = Vec::new();
    let mut terrain = Vec::new();
    for (id, look) in looks() {
        let entry = |kind: &str, file: &str, fw: usize, fh: usize, frames: usize, extra: &str| {
            format!(
                "    \"{id}\": {{ \"kind\": \"{kind}\", \"file\": \"{file}\", \"frame\": [{fw}, {fh}], \"frames\": {frames}{extra} }}"
            )
        };
        match look {
            Look::Ground(draw) | Look::Air(draw) => {
                let air = matches!(look, Look::Air(_));
                let file = format!("art/units/{id}.png");
                files.push(png_file(file.clone(), &unit_strip(draw, if air { 5 } else { 2 })));
                let kind = if air { "aircraft" } else { "unit" };
                sprites.push(entry(kind, &file, TILE, TILE, FACINGS, ", \"facings\": 8"));
            }
            Look::Turret(gun) => {
                let file = format!("art/buildings/{id}.png");
                files.push(png_file(file.clone(), &turret_strip(gun)));
                sprites.push(entry("turret", &file, TILE, TILE, FACINGS, ", \"facings\": 8, \"size\": [1, 1]"));
            }
            Look::Building(w, h, draw) => {
                let file = format!("art/buildings/{id}.png");
                files.push(png_file(file.clone(), &building(w, h, draw)));
                sprites.push(entry("building", &file, w * TILE, h * TILE, 1, &format!(", \"size\": [{w}, {h}]")));
            }
            Look::Feature(draw) => {
                let file = format!("art/features/{id}.png");
                let c = strip(TILE, TILE, 1, |c, _| {
                    draw(&mut centred(c, 0));
                    *c = finish(c, d::OUTLINE, 1);
                });
                files.push(png_file(file.clone(), &c));
                sprites.push(entry("feature", &file, TILE, TILE, 1, ""));
            }
            Look::Terrain(draw) => {
                let file = format!("art/terrain/{id}.png");
                let mut c = Canvas::new(TILE * 4, TILE);
                let mut n = Noise::new(id);
                for v in 0..4 {
                    draw(&mut c, v * TILE, &mut n);
                }
                files.push(png_file(file.clone(), &c));
                terrain.push(format!(
                    "    \"{id}\": {{ \"file\": \"{file}\", \"frame\": [{TILE}, {TILE}], \"variants\": 4 }}"
                ));
            }
            Look::Power(_) => {}
        }
        if let Some(c) = icon_for(&look) {
            let file = format!("art/icons/{id}.png");
            files.push(png_file(file.clone(), &c));
            icons.push(format!("    \"{id}\": \"{file}\""));
        }
    }

    let mut effects = Vec::new();
    for (id, size, frames) in EFFECTS {
        let file = format!("art/effects/{id}.png");
        files.push(png_file(file.clone(), &effect(id, size, frames)));
        let facings = if id == "rocket" { ", \"facings\": 8" } else { "" };
        effects.push(format!(
            "    \"{id}\": {{ \"file\": \"{file}\", \"frame\": [{size}, {size}], \"frames\": {frames}{facings} }}"
        ));
    }
    for (w, h) in RUBBLE {
        let id = format!("rubble_{w}x{h}");
        let file = format!("art/effects/{id}.png");
        files.push(png_file(file.clone(), &rubble(w, h)));
        effects.push(format!(
            "    \"{id}\": {{ \"file\": \"{file}\", \"frame\": [{}, {}], \"frames\": 1 }}",
            w * TILE,
            h * TILE
        ));
    }

    files.push(png_file("theme/panel.png".into(), &panel()));
    files.push(png_file("theme/button.png".into(), &button()));
    files.push(File { path: "theme/theme.css".into(), bytes: THEME_CSS.as_bytes().to_vec() });

    let remap: Vec<String> = d::REMAP.iter().map(|c| hex(u32::from_be_bytes([0, c[0], c[1], c[2]]))).collect();
    let ramps: Vec<String> = RAMPS
        .iter()
        .map(|(name, shades)| {
            let s: Vec<String> = shades.iter().map(|&c| hex(c)).collect();
            format!("    \"{name}\": [{}]", s.join(", "))
        })
        .collect();
    // Squads: drawn by the renderer as copies of a single soldier, in 256ths of a tile.
    let squads: Vec<String> = SQUADS
        .iter()
        .map(|(id, member)| {
            let at: Vec<String> = d::SQUAD
                .iter()
                .map(|(x, y)| format!("[{}, {}]", x * 256 / TILE as i64, y * 256 / TILE as i64))
                .collect();
            format!("    \"{id}\": {{ \"member\": \"{member}\", \"offsets\": [{}] }}", at.join(", "))
        })
        .collect();
    let index = format!(
        "{{\n  \"about\": \"Placeholder art for the generic pack, drawn from code by crates/classic-tools/src/art (run: cargo run --bin art). Paths are relative to the pack. Strips run left to right; facings start at north and turn clockwise in eighths. Pixels in the remap colours are swapped, by exact value, for the owning faction's ramp, darkest first. Building sizes are in tiles and match the default footprints in the rules data. A squad is drawn as copies of its member's sprite, one per offset, with one fewer for each equal share of health lost (the last listed goes first); offsets are in 256ths of a tile from the unit's middle, x right and y down, for a squad facing north, and turn with it. A unit strip with more frames than facings holds a walk cycle per facing, facing by facing.\",\n  \"tile\": {TILE},\n  \"remap\": [{}],\n  \"ramps\": {{\n{}\n  }},\n  \"terrain\": {{\n{}\n  }},\n  \"sprites\": {{\n{}\n  }},\n  \"icons\": {{\n{}\n  }},\n  \"squads\": {{\n{}\n  }},\n  \"effects\": {{\n{}\n  }}\n}}\n",
        remap.join(", "),
        ramps.join(",\n"),
        terrain.join(",\n"),
        sprites.join(",\n"),
        icons.join(",\n"),
        squads.join(",\n"),
        effects.join(",\n"),
    );
    files.push(File { path: "art/art.json".into(), bytes: index.into_bytes() });

    let provenance: String = files
        .iter()
        .filter(|f| f.path.ends_with(".png") || f.path.ends_with(".css"))
        .map(|f| {
            format!(
                "{{\"file\": \"{}\", \"source\": \"original\", \"made_by\": \"drawn from code: crates/classic-tools/src/art\", \"licence\": \"MIT\"}}\n",
                f.path
            )
        })
        .collect();
    files.push(File { path: "provenance.jsonl".into(), bytes: provenance.into_bytes() });
    files
}
