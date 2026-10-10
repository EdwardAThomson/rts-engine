//! Where a building may go (rules-base-building-power.md, "Placement rules"). A footprint must sit inside the map
//! on firm, empty ground (rock only, unless a pack tunes `rock_only` off), with no resource, building or unit on it,
//! within `max_gap` empty tiles of a building its owner already has (walls don't extend the area; 0 means touching,
//! diagonals included), and a refinery's dock must not be a cliff. The numbers come from the `placement` module in
//! the rules data. The player's concrete slabs (the `decay` module) count as their buildings for that distance. A
//! slab follows the same rules, may lie over the player's own slab (only new tiles are laid) but not another
//! player's, and must lay at least one new tile.

use crate::map::{MapData, Terrain, Tile};
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
    /// A refinery would have no tile round it for a harvester to unload on: all off the map, cliff or buildings.
    BadExit,
    /// The player has no finished building of this kind waiting at a yard.
    NotReady,
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
            PlaceError::BadExit => "bad_exit",
            PlaceError::NotReady => "not_ready",
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

/// Whether one of `player`'s slab tiles is within `max_gap` empty tiles of `new`.
fn slab_near(map: &MapData, state: &GameState, player: u32, new: Rect, max_gap: i32) -> bool {
    if state.slabs.is_none() {
        return false;
    }
    let r = max_gap + 1;
    (new.y0 - r..=new.y1 + r).any(|ty| {
        (new.x0 - r..=new.x1 + r).any(|tx| {
            map.in_bounds(tx, ty)
                && crate::decay::owner(state, tx, ty) == Some(player)
                && Rect { x0: tx, y0: ty, x1: tx, y1: ty }.gap(new) <= max_gap
        })
    })
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
    site(map, state, rules, Some(player), kind, x, y, None)
}

/// Check the ground for a building that a unit deploys into (the `deploy` module): the placement rules less the
/// one about its owner's other buildings, since a deployed building may stand anywhere, and with the deploying
/// unit `unit` itself not in the way.
pub fn check_deploy(
    map: &MapData,
    state: &GameState,
    rules: &Rules,
    kind: Kind,
    x: i32,
    y: i32,
    unit: u32,
) -> Result<(), PlaceError> {
    site(map, state, rules, None, kind, x, y, Some(unit))
}

/// The placement rules: near `player`'s buildings when one is given, and with unit `ignore` not in the way.
#[allow(clippy::too_many_arguments)]
fn site(
    map: &MapData,
    state: &GameState,
    rules: &Rules,
    player: Option<u32>,
    kind: Kind,
    x: i32,
    y: i32,
    ignore: Option<u32>,
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
            let heading_here =
                |e: &crate::world::Entity| crate::movement::step_tile(e).is_some_and(|t| t.x == tx && t.y == ty);
            // Aircraft don't block a building: one landed there takes off again.
            let solid = |e: &&crate::world::Entity| rules.kind(e.kind).building || crate::world::on_ground(rules, e);
            let others = state.entities.iter().filter(|e| Some(e.id) != ignore);
            if others.filter(solid).any(|e| footprint(e).contains(tx, ty) || heading_here(e)) {
                return Err(PlaceError::Blocked { x: tx, y: ty });
            }
            if k.slab && crate::decay::owner(state, tx, ty).is_some_and(|o| Some(o) != player) {
                return Err(PlaceError::Blocked { x: tx, y: ty });
            }
        }
    }
    if k.slab && (new.y0..=new.y1).all(|ty| (new.x0..=new.x1).all(|tx| crate::decay::owner(state, tx, ty).is_some())) {
        return Err(PlaceError::Blocked { x: new.x0, y: new.y0 });
    }
    let near = |player: u32| {
        state
            .entities
            .iter()
            .filter(|e| e.owner == player && rules.kind(e.kind).building && !rules.kind(e.kind).wall)
            .any(|e| footprint(e).gap(new) <= rules.placement.max_gap)
            || slab_near(map, state, player, new, rules.placement.max_gap)
    };
    if player.is_some_and(|p| !near(p)) {
        return Err(PlaceError::TooFar);
    }
    if k.refinery {
        // Harvesters unload on any side, so one tile round it that a harvester could stand on is enough.
        let building_on =
            |t: Tile| state.entities.iter().any(|e| rules.kind(e.kind).building && footprint(e).contains(t.x, t.y));
        if !crate::world::around(x, y, k.width, k.height).any(|t| map.passable(t.x, t.y) && !building_on(t)) {
            return Err(PlaceError::BadExit);
        }
    }
    Ok(())
}
