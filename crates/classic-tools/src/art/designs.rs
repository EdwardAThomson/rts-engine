//! The drawings themselves: plain top-down shapes for every generic id, our own and deliberately generic. Units are
//! drawn facing north around their centre and turned by the pen; buildings are drawn from their top-left corner.
//! All coordinates are in half pixels.

use super::canvas::{Canvas, Noise, Pen, Rgba};

const fn rgb(c: u32) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

/// The faction key colours, darkest first. A renderer swaps these exact values for the shades of a faction's ramp.
pub const REMAP: [Rgba; 4] = [rgb(0x400040), rgb(0x800080), rgb(0xc000c0), rgb(0xff40ff)];
const R0: Rgba = REMAP[0];
const R1: Rgba = REMAP[1];
const R2: Rgba = REMAP[2];
const R3: Rgba = REMAP[3];

pub const OUTLINE: Rgba = rgb(0x141414);
const METAL_D: Rgba = rgb(0x3a3f44);
const METAL: Rgba = rgb(0x6b737a);
const METAL_L: Rgba = rgb(0x9aa3a8);
const TRACK: Rgba = rgb(0x2a2a2a);
const TRACK_L: Rgba = rgb(0x4a4a4a);
const GLASS: Rgba = rgb(0x5fb3d9);
const GLASS_L: Rgba = rgb(0xa8dcef);
const CONC_D: Rgba = rgb(0x5e5c57);
const CONC: Rgba = rgb(0x8c8a84);
const CONC_L: Rgba = rgb(0xb0aea6);
const WARN: Rgba = rgb(0xd9b43a);
const DRAB: Rgba = rgb(0x6b6a45);
const SAND: Rgba = rgb(0xc9a66b);
const SAND_D: Rgba = rgb(0xa98853);
const SAND_L: Rgba = rgb(0xdcbd86);
const ROCK: Rgba = rgb(0x7d6e5d);
const ROCK_D: Rgba = rgb(0x5a4e42);
const ROCK_L: Rgba = rgb(0x9a8a76);
const ORE: Rgba = rgb(0xe0b040);
const ORE_D: Rgba = rgb(0xa87a20);
const ORE_L: Rgba = rgb(0xfff0a0);
const RED: Rgba = rgb(0xc03020);

// ---- Ground units -------------------------------------------------------------------------------------------------

/// Tracks down both sides of a hull `hw` half wide and `hl` half long.
fn tracks(p: &mut Pen, hw: i64, hl: i64) {
    for side in [-1, 1] {
        let (x0, x1) = if side < 0 { (-hw, -hw + 7) } else { (hw - 7, hw) };
        p.rect(x0, -hl, x1, hl, TRACK);
        let mut y = -hl + 2;
        while y < hl - 2 {
            p.rect(x0 + 1, y, x1 - 1, y + 2, TRACK_L);
            y += 5;
        }
    }
}

fn hull(p: &mut Pen, hw: i64, hl: i64) {
    p.rect(-hw + 6, -hl + 2, hw - 6, hl - 1, R1);
    p.rect(-hw + 8, -hl + 4, hw - 8, hl - 3, R2);
}

fn turret(p: &mut Pen, r: i64, barrel: i64, width: i64, barrels: &[i64]) {
    for &bx in barrels {
        p.line((bx, 2), (bx, -barrel), width, METAL_D);
        p.rect(bx - width / 2 - 1, -barrel - 1, bx + width / 2 + 1, -barrel + 3, METAL);
    }
    p.circle(0, 3, r, R1);
    p.circle(-1, 2, r - 3, R2);
    p.circle(-2, 1, r / 3, R3);
}

pub fn battle_tank(p: &mut Pen) {
    tracks(p, 15, 19);
    hull(p, 15, 19);
    turret(p, 9, 27, 3, &[0]);
}

pub fn siege_tank(p: &mut Pen) {
    tracks(p, 17, 21);
    hull(p, 17, 21);
    p.rect(-6, 8, 6, 18, METAL);
    turret(p, 10, 31, 5, &[0]);
}

pub fn missile_tank(p: &mut Pen) {
    tracks(p, 15, 19);
    hull(p, 15, 19);
    p.rect(-9, -10, 9, 12, METAL_D);
    p.rect(-8, -9, 8, 11, METAL);
    for x in [-5, 0, 5] {
        for y in [-6, 2] {
            p.circle(x, y, 2, RED);
        }
    }
}

pub fn harvester(p: &mut Pen) {
    tracks(p, 17, 23);
    p.rect(-11, -14, 11, 21, R1);
    p.rect(-9, -2, 9, 19, ORE_D);
    p.rect(-7, 0, 7, 17, ORE);
    p.rect(-9, -13, 9, -4, R2);
    p.rect(-6, -12, 6, -8, GLASS);
    p.poly(&[(-17, -24), (17, -24), (12, -15), (-12, -15)], METAL);
    for x in [-12, -6, 0, 6, 12] {
        p.rect(x - 1, -27, x + 1, -23, METAL_L);
    }
}

pub fn mcv(p: &mut Pen) {
    tracks(p, 17, 23);
    p.rect(-11, -20, 11, 21, R1);
    p.rect(-9, -18, 9, -8, R2);
    p.rect(-6, -17, 6, -13, GLASS);
    p.rect(-9, -5, 9, 19, METAL);
    p.rect(-7, -3, 7, 17, METAL_L);
    p.line((-5, 14), (5, -2), 3, WARN);
    p.circle(4, 12, 4, METAL_D);
}

pub fn super_a(p: &mut Pen) {
    tracks(p, 16, 20);
    hull(p, 16, 20);
    p.circle(0, 4, 9, R1);
    p.rect(-3, -16, 3, 2, METAL_D);
    for (r, c) in [(9, METAL_L), (7, METAL), (5, GLASS), (2, GLASS_L)] {
        p.circle(0, -18, r, c);
    }
}

pub fn super_h(p: &mut Pen) {
    tracks(p, 19, 23);
    p.rect(-14, -20, 14, 22, R0);
    hull(p, 19, 23);
    turret(p, 12, 31, 4, &[-4, 4]);
}

pub fn super_o(p: &mut Pen) {
    tracks(p, 15, 19);
    hull(p, 15, 19);
    p.circle(0, 10, 8, METAL);
    p.circle(-2, 8, 5, GLASS);
    p.circle(-3, 7, 2, GLASS_L);
    p.line((0, 0), (0, -24), 4, METAL_D);
    p.circle(0, -4, 6, R2);
}

fn bike_wheels(p: &mut Pen) {
    p.rect(-3, -18, 3, -10, TRACK);
    p.rect(-3, 9, 3, 17, TRACK);
}

pub fn scout_bike(p: &mut Pen) {
    bike_wheels(p);
    p.line((0, -14), (0, 13), 7, R1);
    p.line((0, -12), (0, 6), 4, R2);
    p.line((-7, -9), (7, -9), 2, METAL_D);
    p.circle(0, 3, 4, METAL_L);
}

pub fn raider_bike(p: &mut Pen) {
    bike_wheels(p);
    p.line((0, -14), (0, 13), 9, R1);
    p.line((0, -12), (0, 6), 5, R2);
    p.line((6, -2), (6, -16), 3, METAL_D);
    p.line((-6, -2), (-6, -16), 3, METAL_D);
    p.circle(0, 3, 4, METAL_L);
}

pub fn quad(p: &mut Pen) {
    for (x, y) in [(-12, -12), (12, -12), (-12, 12), (12, 12)] {
        p.rect(x - 3, y - 5, x + 3, y + 5, TRACK);
    }
    p.rect(-9, -15, 9, 15, R1);
    p.rect(-7, -13, 7, 5, R2);
    p.rect(-5, -11, 5, -5, GLASS);
    p.line((-4, -10), (-4, -20), 2, METAL_D);
    p.line((4, -10), (4, -20), 2, METAL_D);
}

/// One soldier from above: shoulders, helmet and what they carry. `body` and `kit` change by kind.
fn soldier(p: &mut Pen, x: i64, y: i64, body: Rgba, kit: Kit) {
    match kit {
        Kit::Rifle => p.line((x + 3, y - 1), (x + 3, y - 10), 2, METAL_D),
        Kit::Tube => p.line((x + 4, y + 6), (x + 4, y - 10), 4, METAL),
        Kit::Pack => p.rect(x - 4, y + 2, x + 4, y + 7, WARN),
        Kit::Heavy => p.line((x + 4, y + 1), (x + 4, y - 12), 3, METAL_D),
    }
    p.rect(x - 5, y - 2, x + 5, y + 4, body);
    p.circle(x, y, 3, METAL_L);
}

#[derive(Clone, Copy)]
enum Kit {
    Rifle,
    Tube,
    Pack,
    Heavy,
}

const SQUAD: [(i64, i64); 3] = [(0, -8), (-8, 6), (8, 6)];

pub fn infantry(p: &mut Pen) {
    soldier(p, 0, 0, R1, Kit::Rifle);
}

pub fn infantry_squad(p: &mut Pen) {
    for (x, y) in SQUAD {
        soldier(p, x, y, R1, Kit::Rifle);
    }
}

pub fn rocket_infantry(p: &mut Pen) {
    soldier(p, 0, 0, R1, Kit::Tube);
}

pub fn rocket_squad(p: &mut Pen) {
    for (x, y) in SQUAD {
        soldier(p, x, y, R1, Kit::Tube);
    }
}

pub fn elite_infantry(p: &mut Pen) {
    p.circle(0, 2, 7, R0);
    soldier(p, 0, 0, R2, Kit::Heavy);
}

pub fn guerrilla(p: &mut Pen) {
    p.rect(-6, -3, 6, 6, DRAB);
    soldier(p, 0, 0, R1, Kit::Rifle);
}

pub fn saboteur(p: &mut Pen) {
    soldier(p, 0, 0, METAL_D, Kit::Pack);
    p.rect(-5, 1, 5, 3, R2);
}

// ---- Aircraft -----------------------------------------------------------------------------------------------------

pub fn carrier(p: &mut Pen) {
    p.poly(&[(-26, 2), (26, 2), (22, 8), (-22, 8)], R1);
    p.rect(-5, -22, 5, 22, R1);
    p.rect(-3, -20, 3, 18, R2);
    p.rect(-10, 16, 10, 20, R1);
    p.circle(0, -16, 3, GLASS);
    for x in [-16, 16] {
        p.circle(x, 5, 3, METAL_D);
    }
}

pub fn gunship(p: &mut Pen) {
    p.poly(&[(0, -22), (18, 12), (6, 8), (0, 16), (-6, 8), (-18, 12)], R1);
    p.poly(&[(0, -18), (10, 8), (0, 4), (-10, 8)], R2);
    p.circle(0, -8, 3, GLASS);
    p.line((-9, -2), (-9, -14), 2, METAL_D);
    p.line((9, -2), (9, -14), 2, METAL_D);
}

pub fn supply_ship(p: &mut Pen) {
    p.rect(-18, -24, 18, 22, R0);
    p.rect(-16, -22, 16, 20, METAL);
    p.rect(-12, -18, 12, 8, METAL_L);
    p.rect(-12, 10, 12, 14, R2);
    for x in [-10, 0, 10] {
        p.circle(x, 20, 4, METAL_D);
        p.circle(x, 20, 2, WARN);
    }
    p.rect(-4, -26, 4, -20, GLASS);
}

// ---- Buildings ----------------------------------------------------------------------------------------------------

/// The concrete pad under a building `w` by `h` half pixels.
fn pad(p: &mut Pen, w: i64, h: i64) {
    p.rect(1, 1, w - 1, h - 1, CONC_D);
    p.rect(3, 3, w - 3, h - 3, CONC);
    let mut x = 32;
    while x < w - 4 {
        p.rect(x, 3, x + 1, h - 3, CONC_D);
        x += 32;
    }
    let mut y = 32;
    while y < h - 4 {
        p.rect(3, y, w - 3, y + 1, CONC_D);
        y += 32;
    }
}

/// A walled block with a lighter roof and a faction stripe along its front edge.
fn block(p: &mut Pen, x0: i64, y0: i64, x1: i64, y1: i64) {
    p.rect(x0, y0, x1, y1, METAL_D);
    p.rect(x0 + 2, y0 + 2, x1 - 2, y1 - 2, METAL);
    p.rect(x0 + 4, y0 + 4, x1 - 4, y1 - 8, METAL_L);
    p.rect(x0 + 2, y1 - 6, x1 - 2, y1 - 2, R1);
}

fn door(p: &mut Pen, x0: i64, x1: i64, y1: i64) {
    p.rect(x0, y1 - 12, x1, y1 - 2, TRACK);
    let mut x = x0 + 1;
    while x < x1 - 1 {
        p.rect(x, y1 - 11, x + 2, y1 - 9, WARN);
        x += 6;
    }
}

pub fn construction_yard(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 30, w - 8, h - 8);
    p.rect(14, 8, 36, 30, R1);
    p.rect(16, 10, 34, 26, R2);
    p.line((25, 20), (w - 14, 10), 4, WARN);
    p.line((w - 14, 10), (w - 14, 34), 2, METAL_D);
    p.circle(25, 20, 5, METAL_D);
    door(p, w / 2 - 14, w / 2 + 14, h - 8);
}

pub fn slab(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
}

pub fn power_plant(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 48, w - 8, h - 8);
    for x in [36, 92] {
        p.circle(x, 34, 22, METAL_D);
        p.circle(x, 34, 19, METAL_L);
        p.circle(x - 4, 30, 9, rgb(0xd8dfe2));
        p.circle(x, 34, 5, GLASS);
    }
    p.rect(w / 2 - 6, 16, w / 2 + 6, 56, R2);
}

/// Drawn in proportion, so it fits whatever footprint the rules give the refinery.
pub fn refinery(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 6, 6, w * 9 / 16, h * 11 / 16);
    let r = w.min(h) * 7 / 32;
    let (x, y) = (w * 25 / 32, h * 9 / 32);
    p.circle(x, y, r, METAL_D);
    p.circle(x, y, r - 3, ORE_D);
    p.circle(x - r / 4, y - r / 4, r / 3, ORE);
    p.rect(6, h * 3 / 4, w - 6, h - 6, CONC_D);
    let mut x = 8;
    while x < w - 12 {
        p.poly(&[(x + 4, h * 3 / 4 + 2), (x + 8, h * 3 / 4 + 2), (x + 4, h - 8), (x, h - 8)], WARN);
        x += 10;
    }
}

pub fn silo(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    for (x, y) in [(34, 34), (94, 34), (34, 94), (94, 94)] {
        p.circle(x, y, 24, METAL_D);
        p.circle(x, y, 21, ORE_D);
        p.circle(x - 5, y - 5, 10, ORE);
        p.circle(x, y, 4, R2);
    }
}

pub fn radar(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 64, w - 8, h - 8);
    p.rect(w / 2 - 4, 40, w / 2 + 4, 70, METAL_D);
    p.circle(w / 2, 38, 30, METAL_D);
    p.circle(w / 2, 38, 27, METAL_L);
    p.circle(w / 2 + 4, 42, 18, METAL);
    p.line((w / 2, 38), (w / 2 - 16, 22), 3, METAL_D);
    p.circle(w / 2 - 16, 22, 4, R2);
}

pub fn barracks(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    for y in [10, 66] {
        p.rect(10, y, w - 10, y + 46, DRAB);
        p.rect(12, y + 2, w - 12, y + 22, rgb(0x8a8960));
        p.rect(12, y + 24, w - 12, y + 30, R1);
        p.rect(w / 2 - 8, y + 32, w / 2 + 8, y + 44, TRACK);
    }
}

pub fn barracks_heavy(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    p.rect(8, 8, w - 8, h - 8, CONC_D);
    p.rect(14, 14, w - 14, h - 14, CONC_L);
    p.rect(14, h / 2 - 4, w - 14, h / 2 + 4, R1);
    for x in [30, w - 30] {
        p.rect(x - 8, 18, x + 8, 30, TRACK);
    }
    door(p, w / 2 - 14, w / 2 + 14, h - 8);
}

pub fn light_factory(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 8, w - 8, h - 8);
    for x in [24, 56, 88] {
        p.rect(x, 18, x + 16, 30, GLASS);
        p.rect(x + 2, 20, x + 8, 24, GLASS_L);
    }
    door(p, w / 2 - 24, w / 2 + 24, h - 8);
}

pub fn heavy_factory(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 8, w - 8, h - 8);
    let mut y = 16;
    while y < h - 36 {
        p.rect(16, y, w - 16, y + 3, METAL);
        y += 10;
    }
    p.rect(w - 50, 14, w - 18, 46, R2);
    p.circle(w - 34, 30, 10, METAL_D);
    door(p, 24, w - 60, h - 8);
}

pub fn air_factory(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 8, 100, h - 8);
    p.circle(146, 64, 36, CONC_D);
    p.circle(146, 64, 33, CONC_L);
    p.rect(134, 48, 140, 80, WARN);
    p.rect(152, 48, 158, 80, WARN);
    p.rect(134, 61, 158, 67, WARN);
    door(p, 20, 88, h - 8);
}

pub fn repair_pad(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 8, 70, h - 8);
    p.rect(80, 12, w - 10, h - 12, CONC_D);
    p.rect(84, 16, w - 14, h - 16, CONC_L);
    let (cx, cy) = ((80 + w - 10) / 2, h / 2);
    p.rect(cx - 6, cy - 30, cx + 6, cy + 30, R2);
    p.rect(cx - 30, cy - 6, cx + 30, cy + 6, R2);
}

pub fn starport(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    p.circle(w / 2, h / 2, 80, CONC_D);
    p.circle(w / 2, h / 2, 76, CONC_L);
    p.circle(w / 2, h / 2, 50, CONC);
    p.circle(w / 2, h / 2, 46, CONC_L);
    for (dx, dy) in [(0, -63), (63, 0), (0, 63), (-63, 0)] {
        p.circle(w / 2 + dx, h / 2 + dy, 6, WARN);
    }
    p.rect(w / 2 - 12, h / 2 - 12, w / 2 + 12, h / 2 + 12, R1);
    p.rect(w / 2 - 8, h / 2 - 8, w / 2 + 8, h / 2 + 8, R2);
    p.rect(8, 8, 40, 40, METAL);
    p.rect(12, 12, 36, 24, GLASS);
}

pub fn research_lab(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    block(p, 8, 72, w - 8, h - 8);
    p.circle(w / 2, 50, 38, METAL_D);
    p.circle(w / 2, 50, 35, GLASS);
    p.circle(w / 2 - 10, 40, 14, GLASS_L);
    p.rect(w / 2 - 3, 12, w / 2 + 3, 88, METAL_D);
    p.rect(w / 2 - 35, 47, w / 2 + 35, 53, METAL_D);
}

pub fn palace(p: &mut Pen, w: i64, h: i64) {
    pad(p, w, h);
    p.rect(10, 10, w - 10, h - 10, CONC_D);
    p.rect(14, 14, w - 14, h - 14, CONC_L);
    p.rect(34, 34, w - 34, h - 34, CONC);
    p.rect(38, 38, w - 38, h - 38, CONC_L);
    p.circle(w / 2, h / 2, 34, R0);
    p.circle(w / 2, h / 2, 31, R1);
    p.circle(w / 2 - 8, h / 2 - 8, 16, R2);
    p.circle(w / 2 - 12, h / 2 - 12, 6, R3);
    for (x, y) in [(24, 24), (w - 24, 24), (24, h - 24), (w - 24, h - 24)] {
        p.circle(x, y, 10, METAL_D);
        p.circle(x, y, 7, R2);
    }
}

pub fn wall(p: &mut Pen, w: i64, h: i64) {
    p.rect(2, 2, w - 2, h - 2, CONC_D);
    p.rect(5, 5, w - 5, h - 5, CONC);
    p.rect(8, 8, w - 8, h - 14, CONC_L);
    p.rect(5, h / 2 - 1, w - 5, h / 2 + 1, CONC_D);
}

/// A turret's fixed base, drawn around the centre of its one tile.
pub fn turret_base(p: &mut Pen) {
    p.rect(-28, -28, 28, 28, CONC_D);
    p.rect(-25, -25, 25, 25, CONC);
    p.circle(0, 0, 20, METAL_D);
    p.circle(0, 0, 17, R1);
}

pub fn gun_turret(p: &mut Pen) {
    p.line((0, 0), (0, -26), 5, METAL_D);
    p.rect(-4, -27, 4, -23, METAL);
    p.circle(0, 2, 11, METAL);
    p.circle(-2, 0, 7, METAL_L);
}

pub fn rocket_turret(p: &mut Pen) {
    p.rect(-12, -16, 12, 10, METAL_D);
    p.rect(-10, -14, 10, 8, METAL);
    for x in [-5, 5] {
        for y in [-8, 2] {
            p.circle(x, y, 3, RED);
        }
    }
}

// ---- Map features and powers --------------------------------------------------------------------------------------

pub fn resource_bloom(p: &mut Pen) {
    p.circle(0, 2, 20, SAND_D);
    p.circle(-2, 0, 16, SAND_L);
    for (x, y, r) in [(-8, -4, 5), (6, -8, 4), (8, 6, 5), (-4, 8, 4), (0, -1, 7)] {
        p.circle(x, y, r, ORE_D);
        p.circle(x - 1, y - 1, r - 2, ORE);
        p.circle(x - 2, y - 2, 1, ORE_L);
    }
}

/// A plain warning marker: the generic pack draws the hazard as a sign, not a creature.
pub fn hazard(p: &mut Pen) {
    p.poly(&[(0, -24), (24, 0), (0, 24), (-24, 0)], TRACK);
    p.poly(&[(0, -20), (20, 0), (0, 20), (-20, 0)], WARN);
    p.rect(-2, -12, 2, 4, TRACK);
    p.circle(0, 10, 2, TRACK);
}

pub fn power_missile(p: &mut Pen) {
    p.circle(0, 0, 22, RED);
    p.circle(0, 0, 18, rgb(0x301010));
    p.rect(-24, -1, 24, 1, RED);
    p.rect(-1, -24, 1, 24, RED);
    p.line((-14, 14), (10, -10), 5, METAL_L);
    p.poly(&[(10, -10), (16, -16), (12, -6)], RED);
}

/// A parachute carrying `cargo`.
fn drop(p: &mut Pen, cargo: Rgba) {
    p.poly(&[(-20, -6), (-14, -18), (0, -22), (14, -18), (20, -6)], METAL_L);
    p.poly(&[(-20, -6), (-14, -12), (0, -14), (14, -12), (20, -6)], R2);
    p.line((-20, -6), (-3, 10), 1, METAL_D);
    p.line((20, -6), (3, 10), 1, METAL_D);
    p.rect(-6, 8, 6, 20, cargo);
}

pub fn power_guerrillas(p: &mut Pen) {
    drop(p, DRAB);
}

pub fn power_saboteur(p: &mut Pen) {
    drop(p, METAL_D);
    p.rect(-3, 12, 3, 16, WARN);
}

// ---- Terrain ------------------------------------------------------------------------------------------------------

/// Flat colour with scattered lighter and darker pixels, so a field of tiles doesn't look printed.
fn speckle(c: &mut Canvas, x0: usize, n: &mut Noise, base: Rgba, dark: Rgba, light: Rgba, count: usize) {
    for y in 0..32 {
        for x in 0..32 {
            c.px[y * c.w + x0 + x] = base;
        }
    }
    for _ in 0..count {
        let (x, y) = (n.range(0, 31) as usize, n.range(0, 31) as usize);
        c.px[y * c.w + x0 + x] = if n.roll().is_multiple_of(2) { dark } else { light };
    }
}

pub fn open(c: &mut Canvas, x0: usize, n: &mut Noise) {
    speckle(c, x0, n, SAND, SAND_D, SAND_L, 90);
    for _ in 0..2 {
        let (x, y) = (n.range(2, 28) as usize, n.range(2, 29) as usize);
        for dx in 0..n.range(3, 6) as usize {
            if x + dx < 32 {
                c.px[y * c.w + x0 + x + dx] = SAND_D;
                c.px[(y + 1) * c.w + x0 + x + dx] = SAND_L;
            }
        }
    }
}

pub fn rock(c: &mut Canvas, x0: usize, n: &mut Noise) {
    speckle(c, x0, n, ROCK, ROCK_D, ROCK_L, 120);
    let mut p = Pen::at(c, x0 as i64, 0);
    for _ in 0..3 {
        let (x, y) = (n.range(4, 60), n.range(4, 60));
        let (dx, dy) = (n.range(-14, 14), n.range(-14, 14));
        p.line((x, y), (x + dx, y + dy), 1, ROCK_D);
    }
    for _ in 0..2 {
        let (x, y) = (n.range(8, 56), n.range(8, 56));
        p.circle(x, y, n.range(3, 6), ROCK_L);
    }
}

pub fn resource(c: &mut Canvas, x0: usize, n: &mut Noise) {
    open(c, x0, n);
    let mut p = Pen::at(c, x0 as i64, 0);
    for _ in 0..7 {
        let (x, y, r) = (n.range(6, 58), n.range(6, 58), n.range(2, 4));
        p.circle(x, y, r + 1, ORE_D);
        p.circle(x - 1, y - 1, r, ORE);
        p.circle(x - 1, y - 1, 1, ORE_L);
    }
}

// ---- Effects ------------------------------------------------------------------------------------------------------

/// One frame of an explosion `size` pixels across: a fireball that grows, cools from yellow to red, then thins out
/// as smoke.
pub fn explosion(p: &mut Pen, n: &mut Noise, frame: i64, frames: i64, size: i64) {
    let r_max = size - 4;
    let r = r_max * (frame + 2) / (frames + 1);
    let phase = frame * 4 / frames;
    let smoke_a = (200 - 140 * frame / frames) as u8;
    let (outer, inner, core) = match phase {
        0 => (rgb(0xe06018), rgb(0xffb030), rgb(0xfff4c0)),
        1 => (rgb(0xb03010), rgb(0xf08020), rgb(0xffd060)),
        2 => ([70, 60, 55, smoke_a], rgb(0xa03010), rgb(0xe07020)),
        _ => ([70, 66, 62, smoke_a], [100, 96, 90, smoke_a], [120, 114, 106, smoke_a]),
    };
    for _ in 0..5 {
        let (x, y) = (n.range(-r / 3, r / 3), n.range(-r / 3, r / 3));
        let br = r * n.range(5, 8) / 8;
        p.circle(x, y, br, outer);
        p.circle(x, y - 1, br * 2 / 3, inner);
        if phase < 3 {
            p.circle(x - 1, y - 2, br / 3, core);
        }
    }
}

pub fn smoke_puff(p: &mut Pen, n: &mut Noise, frame: i64, frames: i64) {
    let r = 4 + 9 * frame / frames;
    let a = (180 - 150 * frame / frames) as u8;
    for _ in 0..3 {
        let (x, y) = (n.range(-3, 3), n.range(-3, 3) - frame);
        p.circle(x, y, r, [90, 88, 84, a]);
        p.circle(x - 1, y - 1, r * 2 / 3, [130, 126, 120, a]);
    }
}

pub fn hit_spark(p: &mut Pen, n: &mut Noise, frame: i64) {
    let len = 4 + 4 * frame;
    for k in 0..6 {
        let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (0, -1)][k];
        let j = n.range(-1, 1);
        p.line((dx * len / 3, dy * len / 3), (dx * len + j, dy * len - j), 1, if frame == 2 { WARN } else { ORE_L });
    }
    if frame == 0 {
        p.circle(0, 0, 3, rgb(0xffffff));
    }
}

pub fn muzzle_flash(p: &mut Pen, frame: i64, rocket: bool) {
    let len = [10, 14, 8][frame as usize];
    let w = if rocket { 8 } else { 5 };
    p.poly(&[(-w, 0), (0, -len), (w, 0), (0, 4)], rgb(0xf08020));
    p.poly(&[(-w / 2, 0), (0, -len * 2 / 3), (w / 2, 0), (0, 2)], rgb(0xfff4c0));
    if rocket && frame > 0 {
        p.circle(0, 6, 4 + 2 * frame, [120, 116, 110, 150]);
    }
}

pub fn shell(p: &mut Pen) {
    p.circle(0, 0, 2, ORE_L);
    p.line((0, 1), (0, 6), 2, [255, 200, 80, 140]);
}

pub fn rocket(p: &mut Pen) {
    p.line((0, -8), (0, 6), 3, METAL_L);
    p.poly(&[(-1, -8), (0, -11), (1, -8)], RED);
    p.poly(&[(-3, 6), (3, 6), (0, 3)], METAL_D);
    p.circle(0, 9, 2, rgb(0xffb030));
}

pub fn crater(p: &mut Pen, n: &mut Noise) {
    p.circle(0, 0, 18, [60, 48, 36, 150]);
    p.circle(1, 1, 12, [40, 32, 24, 190]);
    for _ in 0..5 {
        p.circle(n.range(-16, 16), n.range(-16, 16), 2, [90, 76, 60, 170]);
    }
}

pub fn scorch(p: &mut Pen, n: &mut Noise) {
    for _ in 0..6 {
        p.circle(n.range(-10, 10), n.range(-10, 10), n.range(8, 14), [20, 16, 12, 70]);
    }
}

/// Broken concrete and twisted metal over a footprint `w` by `h` half pixels, for a destroyed building.
pub fn rubble(p: &mut Pen, n: &mut Noise, w: i64, h: i64) {
    p.rect(4, 4, w - 4, h - 4, [40, 34, 28, 110]);
    for _ in 0..(w * h / 300) {
        let (x, y, r) = (n.range(8, w - 8), n.range(8, h - 8), n.range(2, 6));
        let c = [CONC_D, CONC, METAL_D, TRACK][n.range(0, 3) as usize];
        p.poly(&[(x - r, y), (x, y - r), (x + r, y + r / 2)], c);
    }
}
