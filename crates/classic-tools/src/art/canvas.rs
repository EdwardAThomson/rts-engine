//! A tiny integer rasteriser: filled polygons and circles sampled at pixel centres, a pen that moves, scales and
//! turns shapes in eighth turns (the eight facings), and the finishing passes that give sprites a dark outline and
//! a soft drop shadow. Integer maths only, so every machine draws the same pixels.

pub type Rgba = [u8; 4];

/// Coordinates handed to the pen are in half pixels; inside, the canvas works in sixteenths of a pixel.
pub const SUB: i64 = 16;
const HALF: i64 = 8;

#[derive(Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Rgba>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![[0; 4]; w * h] }
    }

    pub fn get(&self, x: usize, y: usize) -> Rgba {
        self.px[y * self.w + x]
    }

    /// Paint `c` over the pixel, blending by its alpha.
    pub fn blend(&mut self, x: usize, y: usize, c: Rgba) {
        let d = &mut self.px[y * self.w + x];
        let a = c[3] as u32;
        if a == 255 || d[3] == 0 {
            *d = c;
            return;
        }
        let da = d[3] as u32;
        let out_a = a + da * (255 - a) / 255;
        for i in 0..3 {
            d[i] = ((c[i] as u32 * a + d[i] as u32 * da * (255 - a) / 255) / out_a.max(1)) as u8;
        }
        d[3] = out_a as u8;
    }

    /// Fill every pixel whose centre passes `inside`, within a box given in sixteenths of a pixel.
    fn fill(&mut self, bx: (i64, i64, i64, i64), c: Rgba, inside: impl Fn(i64, i64) -> bool) {
        let x0 = (bx.0 / SUB - 1).max(0);
        let y0 = (bx.1 / SUB - 1).max(0);
        let x1 = (bx.2 / SUB + 1).min(self.w as i64 - 1);
        let y1 = (bx.3 / SUB + 1).min(self.h as i64 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if inside(x * SUB + SUB / 2, y * SUB + SUB / 2) {
                    self.blend(x as usize, y as usize, c);
                }
            }
        }
    }

    pub fn polygon(&mut self, pts: &[(i64, i64)], c: Rgba) {
        let bx = pts.iter().fold((i64::MAX, i64::MAX, i64::MIN, i64::MIN), |b, &(x, y)| {
            (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y))
        });
        self.fill(bx, c, |px, py| {
            let mut inside = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let (xi, yi) = pts[i];
                let (xj, yj) = pts[j];
                if (yi > py) != (yj > py) && ((px - xi) * (yj - yi) < (xj - xi) * (py - yi)) == (yj > yi) {
                    inside = !inside;
                }
                j = i;
            }
            inside
        });
    }

    pub fn disc(&mut self, cx: i64, cy: i64, r: i64, c: Rgba) {
        self.fill((cx - r, cy - r, cx + r, cy + r), c, |x, y| (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r);
    }

    /// Copy `src` onto this canvas with its top-left corner at pixel (`x`, `y`), blending.
    pub fn stamp(&mut self, src: &Canvas, x: i64, y: i64) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                let (dx, dy) = (x + sx as i64, y + sy as i64);
                let c = src.get(sx, sy);
                if c[3] > 0 && dx >= 0 && dy >= 0 && (dx as usize) < self.w && (dy as usize) < self.h {
                    self.blend(dx as usize, dy as usize, c);
                }
            }
        }
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.px.iter().flatten().copied().collect()
    }

    pub fn png(&self) -> Vec<u8> {
        super::png::encode(self.w, self.h, &self.rgba())
    }
}

/// Cosine and sine of each eighth turn, clockwise from north on a screen whose y grows downwards, times 256.
const TURN: [(i64, i64); 8] =
    [(256, 0), (181, 181), (0, 256), (-181, 181), (-256, 0), (-181, -181), (0, -256), (181, -181)];

/// Draws shapes given in half pixels around its origin: turned by `facing` eighth turns, scaled by `num / den`,
/// then moved to `origin` (in sixteenths of a pixel on the canvas).
pub struct Pen<'a> {
    pub canvas: &'a mut Canvas,
    pub origin: (i64, i64),
    pub facing: usize,
    pub num: i64,
    pub den: i64,
}

impl<'a> Pen<'a> {
    /// A pen whose origin is pixel (`x`, `y`) of the canvas, at full size, facing north.
    pub fn at(canvas: &'a mut Canvas, x: i64, y: i64) -> Pen<'a> {
        Pen { canvas, origin: (x * SUB, y * SUB), facing: 0, num: 1, den: 1 }
    }

    pub fn map(&self, (x, y): (i64, i64)) -> (i64, i64) {
        let (c, s) = TURN[self.facing % 8];
        let rx = (x * c - y * s) * HALF / 256;
        let ry = (x * s + y * c) * HALF / 256;
        (self.origin.0 + rx * self.num / self.den, self.origin.1 + ry * self.num / self.den)
    }

    pub fn poly(&mut self, pts: &[(i64, i64)], c: Rgba) {
        let mapped: Vec<_> = pts.iter().map(|&p| self.map(p)).collect();
        self.canvas.polygon(&mapped, c);
    }

    /// An upright rectangle (before turning) from (`x0`, `y0`) to (`x1`, `y1`).
    pub fn rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, c: Rgba) {
        self.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], c);
    }

    pub fn circle(&mut self, x: i64, y: i64, r: i64, c: Rgba) {
        let (cx, cy) = self.map((x, y));
        self.canvas.disc(cx, cy, r * HALF * self.num / self.den, c);
    }

    /// A thick line from one point to another, `width` half pixels across.
    pub fn line(&mut self, a: (i64, i64), b: (i64, i64), width: i64, c: Rgba) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = isqrt(dx * dx + dy * dy).max(1);
        let (nx, ny) = (-dy * width / (2 * len), dx * width / (2 * len));
        let (nx, ny) = if nx == 0 && ny == 0 { (width / 2, 0) } else { (nx, ny) };
        self.poly(&[(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)], c);
    }
}

pub fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Outline every empty pixel that touches a drawn one, then lay the result over a soft shadow offset down and right.
pub fn finish(layer: &Canvas, outline: Rgba, shadow: i64) -> Canvas {
    let mut lined = layer.clone();
    for y in 0..layer.h {
        for x in 0..layer.w {
            if layer.get(x, y)[3] >= 128 {
                continue;
            }
            let near = [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                nx >= 0
                    && ny >= 0
                    && (nx as usize) < layer.w
                    && (ny as usize) < layer.h
                    && layer.get(nx as usize, ny as usize)[3] >= 128
            });
            if near {
                lined.px[y * layer.w + x] = outline;
            }
        }
    }
    let mut out = Canvas::new(layer.w, layer.h);
    if shadow > 0 {
        let mut sh = Canvas::new(layer.w, layer.h);
        for (i, p) in lined.px.iter().enumerate() {
            if p[3] > 0 {
                sh.px[i] = [0, 0, 0, 80];
            }
        }
        out.stamp(&sh, shadow, shadow);
    }
    out.stamp(&lined, 0, 0);
    out
}

/// A small seeded generator for texture noise: xorshift, seeded from a name so each asset has its own pattern.
pub struct Noise(u64);

impl Noise {
    pub fn new(name: &str) -> Noise {
        let h = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3));
        Noise(h | 1)
    }

    pub fn roll(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A whole number in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.roll() % (hi - lo + 1) as u64) as i64
    }
}
