//! Where a building may go. A footprint must sit inside the map on firm, empty ground (no cliff, no resource,
//! rock only when the rules say so), touch no other building or unit, and lie within `max_gap` empty tiles of a
//! building its owner already has. The numbers come from the `placement` module in the rules data.

use crate::map::{MapData, Terrain};
use crate::units::{Kind, Rules};
use crate::world::GameState;

/// Why a placement was refused. The tile, where there is one, is the first offending tile in row order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    NotABuilding,
    OutOfBounds,
    /// Cliff, or open ground when buildings need rock.
    BadGround {
        x: i32,
        y: i32,
    },
    /// Resource lies there.
    OnResource {
        x: i32,
        y: i32,
    },
    /// Another building or a unit is in the way.
    Blocked {
        x: i32,
        y: i32,
    },
    /// Too far from any building the player owns (or the player owns none).
    TooFar,
}

impl PlaceError {
    pub fn id(self) -> &'static str {
        match self {
            PlaceError::NotABuilding => "not_a_building",
            PlaceError::OutOfBounds => "out_of_bounds",
            PlaceError::BadGround { .. } => "bad_ground",
            PlaceError::OnResource { .. } => "on_resource",
            PlaceError::Blocked { .. } => "blocked",
            PlaceError::TooFar => "too_far",
        }
    }
}

/// A rectangle of tiles, inclusive at both ends.
#[derive(Clone, Copy)]
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Rect {
    fn contains(self, x: i32, y: i32) -> bool {
        (self.x0..=self.x1).contains(&x) && (self.y0..=self.y1).contains(&y)
    }

    /// Empty tiles between two rectangles that don't overlap, counted along the wider axis: 0 when they touch,
    /// including at a corner.
    fn gap(self, o: Rect) -> i32 {
        let gx = (o.x0 - self.x1).max(self.x0 - o.x1).max(1);
        let gy = (o.y0 - self.y1).max(self.y0 - o.y1).max(1);
        gx.max(gy) - 1
    }
}

/// Check a placement without changing anything.
pub fn check(
    map: &MapData,
    state: &GameState,
    rules: &Rules,
    player: u32,
    kind: Kind,
    x: i32,
    y: i32,
) -> Result<(), PlaceError> {
    let k = rules.kinds.get(kind.0 as usize).filter(|k| k.building).ok_or(PlaceError::NotABuilding)?;
    let new = Rect { x0: x, y0: y, x1: x + k.width - 1, y1: y + k.height - 1 };
    if !map.in_bounds(new.x0, new.y0) || !map.in_bounds(new.x1, new.y1) {
        return Err(PlaceError::OutOfBounds);
    }
    let footprint = |e: &crate::world::Entity| {
        let t = e.tile();
        let ek = rules.kind(e.kind);
        Rect { x0: t.x, y0: t.y, x1: t.x + ek.width - 1, y1: t.y + ek.height - 1 }
    };
    for ty in new.y0..=new.y1 {
        for tx in new.x0..=new.x1 {
            let i = map.index(tx, ty);
            let ground = map.terrain[i];
            if ground == Terrain::Cliff || (rules.placement.rock_only && ground != Terrain::Rock) {
                return Err(PlaceError::BadGround { x: tx, y: ty });
            }
            if state.resource[i] > 0 {
                return Err(PlaceError::OnResource { x: tx, y: ty });
            }
            if state.entities.iter().any(|e| footprint(e).contains(tx, ty)) {
                return Err(PlaceError::Blocked { x: tx, y: ty });
            }
        }
    }
    let near = state
        .entities
        .iter()
        .filter(|e| e.owner == player && rules.kind(e.kind).building)
        .any(|e| footprint(e).gap(new) <= rules.placement.max_gap);
    if near { Ok(()) } else { Err(PlaceError::TooFar) }
}
