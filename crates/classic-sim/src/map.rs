//! The tile map. Terrain never changes during a game; resource amounts do, so they live in the game state.

use rts_core::hash::{Canon, CanonHasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Terrain {
    Open = 0,
    Rock = 1,
    Cliff = 2,
}

/// A tile position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
}

impl Tile {
    /// How far the tile lies from the middle of a `width` by `height` map, squared (in half tiles). Choices between
    /// equally good tiles go to the one nearer the middle: a tie-break that turns round with the map, so the two
    /// sides of a mirrored map choose alike, where row order would favour the top left.
    pub fn off_middle(self, width: i32, height: i32) -> i64 {
        ((2 * self.x + 1 - width) as i64).pow(2) + ((2 * self.y + 1 - height) as i64).pow(2)
    }
}

impl Canon for Tile {
    fn canon(&self, w: &mut CanonHasher) {
        w.object().field("x", &self.x).field("y", &self.y).end();
    }
}

#[derive(Clone, Debug)]
pub struct MapData {
    pub width: i32,
    pub height: i32,
    pub terrain: Vec<Terrain>,
    /// Amount of resource on each tile at the start.
    pub resource: Vec<i64>,
    /// Player start positions; index 0 is player 1. A gap in the numbering leaves `None`.
    pub start: Vec<Option<Tile>>,
}

/// Sub-tile units per tile; positions are integers in these units.
pub const TILE: i64 = 256;
pub const RESOURCE_PER_TILE: i64 = 300;

/// Parse an ASCII map:
///   `.` open ground   `#` rock (buildable)   `X` cliff (impassable)
///   `~` open ground with resource           `1`-`8` player start (on rock)
/// Lines starting with `;` are comments.
pub fn parse_map(text: &str) -> Result<MapData, String> {
    let rows: Vec<Vec<char>> = text
        .split('\n')
        .map(|r| r.trim_end())
        .filter(|r| !r.is_empty() && !r.starts_with(';'))
        .map(|r| r.chars().collect())
        .collect();
    let height = rows.len();
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut terrain = Vec::with_capacity(width * height);
    let mut resource = Vec::with_capacity(width * height);
    let mut start: Vec<Option<Tile>> = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for x in 0..width {
            let c = row.get(x).copied().unwrap_or('.');
            let t = match c {
                '#' => Terrain::Rock,
                'X' => Terrain::Cliff,
                '1'..='8' => {
                    let i = c as usize - '1' as usize;
                    if start.len() <= i {
                        start.resize(i + 1, None);
                    }
                    start[i] = Some(Tile { x: x as i32, y: y as i32 });
                    Terrain::Rock
                }
                '.' | '~' => Terrain::Open,
                _ => return Err(format!("map: unknown tile '{c}' at {x},{y}")),
            };
            terrain.push(t);
            resource.push(if c == '~' { RESOURCE_PER_TILE } else { 0 });
        }
    }
    Ok(MapData { width: width as i32, height: height as i32, terrain, resource, start })
}

impl MapData {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.terrain[self.index(x, y)] != Terrain::Cliff
    }

    /// Index of an in-bounds tile in the per-tile arrays.
    pub fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn tile_at(&self, i: usize) -> Tile {
        let w = self.width as usize;
        Tile { x: (i % w) as i32, y: (i / w) as i32 }
    }
}
