//! Points and distances in sub-tile units, so the opponent's choices come out the same way round on a mirrored map.
//! A footprint's centre is exact here (a two-tile building's lies on a tile edge), where rounding it to a tile would
//! push every distance measured from it towards the top left. Where a point has to become a tile and lies exactly on
//! a tile edge, it rounds towards the middle of the map, and ties between equally good tiles go to the one nearer
//! the middle: rules that turn round with the map, as the 3D engine's fix for its start bias did (rts-3d-engine #9).

use classic_sim::map::TILE;
use classic_sim::units::KindRules;
use classic_sim::{Entity, Game, Tile};
use rts_core::imath::isqrt;

/// A point in sub-tile units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Point {
    pub x: i64,
    pub y: i64,
}

/// Where an entity is: a building's footprint centre, a unit's position.
pub(crate) fn at(game: &Game, e: &Entity) -> Point {
    let k = game.rules.kind(e.kind);
    if k.building {
        let t = e.tile();
        footprint(k, t.x, t.y)
    } else {
        Point { x: e.x, y: e.y }
    }
}

/// The centre of a footprint of `k` with its top-left tile at `x`, `y`.
pub(crate) fn footprint(k: &KindRules, x: i32, y: i32) -> Point {
    Point { x: x as i64 * TILE + k.width as i64 * TILE / 2, y: y as i64 * TILE + k.height as i64 * TILE / 2 }
}

/// The centre of a tile.
pub(crate) fn centre(t: Tile) -> Point {
    Point { x: t.x as i64 * TILE + TILE / 2, y: t.y as i64 * TILE + TILE / 2 }
}

/// Distance squared, in sub-tile units.
pub(crate) fn d2(a: Point, b: Point) -> i64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy
}

/// Distance squared, in whole tiles (rounded down).
pub(crate) fn tiles2(a: Point, b: Point) -> i64 {
    d2(a, b) / (TILE * TILE)
}

/// How far a point is from the middle of the map, squared: the tie-break that turns round with the map.
pub(crate) fn off_middle(game: &Game, p: Point) -> i64 {
    d2(p, Point { x: game.map.width as i64 * TILE / 2, y: game.map.height as i64 * TILE / 2 })
}

/// The point `n` sub-tile units from `from` along the line to `to`, or `to` itself if it is nearer. Division
/// truncates towards zero, the same both ways round.
pub(crate) fn toward(from: Point, to: Point, n: i64) -> Point {
    let d = isqrt(d2(from, to) as u64) as i64;
    if d <= n {
        return to;
    }
    Point { x: from.x + (to.x - from.x) * n / d, y: from.y + (to.y - from.y) * n / d }
}

/// The tile a point lies on, or, on an edge between two tiles, the one nearer the middle of the map.
pub(crate) fn tile_of(game: &Game, p: Point) -> Tile {
    let axis = |v: i64, size: i32| {
        let t = v.div_euclid(TILE);
        let edge = v.rem_euclid(TILE) == 0 && 2 * v > size as i64 * TILE;
        (if edge { t - 1 } else { t }) as i32
    };
    Tile { x: axis(p.x, game.map.width), y: axis(p.y, game.map.height) }
}

/// The tile nearest `aim` that a unit can stand on (passable, no resource on it), searched ring by ring out from
/// the tile under it; of equals, the one nearer the middle of the map.
pub(crate) fn standable(game: &Game, aim: Point) -> Option<Tile> {
    let pf = &game.pathfinder;
    let free = |t: Tile| pf.passable(t.x, t.y) && game.state.resource[game.map.index(t.x, t.y)] == 0;
    let c = tile_of(game, aim);
    (0..game.map.width.max(game.map.height)).find_map(|r| {
        (c.y - r..=c.y + r)
            .flat_map(|y| (c.x - r..=c.x + r).map(move |x| Tile { x, y }))
            .filter(|t| (t.x - c.x).abs().max((t.y - c.y).abs()) == r)
            .filter(|&t| game.map.in_bounds(t.x, t.y) && free(t))
            .min_by_key(|&t| (d2(centre(t), aim), off_middle(game, centre(t)), t.y, t.x))
    })
}
