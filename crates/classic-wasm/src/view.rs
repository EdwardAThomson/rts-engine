//! Read-only views of a game for the browser viewer (`web/viewer/`). Nothing here changes the state: the viewer
//! draws what these report and gives orders only through `game_order_move`, as a player would.
//!
//! Map and kind facts are read one call at a time; the entities, which change every tick, are copied into a buffer
//! the caller reserves with `alloc`, `ENTITY_FIELDS` signed 32-bit numbers per entity.

use classic_sim::units::KindRules;
use classic_sim::{Game, Task, Terrain};

/// Numbers per entity in `game_entities`: id, kind, owner, x, y (sub-tile units: a unit's centre, or the centre of
/// a building's top-left tile), health, order, task, cargo.
/// Order is 0 idle, 1 move, 2 harvest; task is the `Task` in declaration order, or -1; cargo is -1 when none.
pub const ENTITY_FIELDS: u32 = 9;

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_map_width(game: *const Game) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.map.width
}

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_map_height(game: *const Game) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.map.height
}

/// Terrain of a tile: 0 open ground, 1 rock, 2 cliff, or -1 off the map.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_terrain(game: *const Game, x: i32, y: i32) -> i32 {
    // SAFETY: a live handle from game_new.
    let map = &unsafe { &*game }.map;
    if !map.in_bounds(x, y) {
        return -1;
    }
    match map.terrain[map.index(x, y)] {
        Terrain::Open => 0,
        Terrain::Rock => 1,
        Terrain::Cliff => 2,
    }
}

/// Resource left on a tile now, or 0 off the map.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_resource(game: *const Game, x: i32, y: i32) -> i32 {
    // SAFETY: a live handle from game_new.
    let g = unsafe { &*game };
    if !g.map.in_bounds(x, y) {
        return 0;
    }
    i32::try_from(g.state.resource[g.map.index(x, y)]).unwrap_or(i32::MAX)
}

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_player_count(game: *const Game) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.state.players.len() as u32
}

/// The number of unit and building kinds in the game's rules; `game_entities` reports each entity's kind as an
/// index below this.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_count(game: *const Game) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.rules.kinds.len() as u32
}

/// A kind's rules, if `kind` is one.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
unsafe fn kind_rules<'a>(game: *const Game, kind: u32) -> Option<&'a KindRules> {
    // SAFETY: a live handle from game_new, which outlives this call's use.
    unsafe { &*game }.rules.kinds.get(kind as usize)
}

fn clamp(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// Copy a kind's generic id (`harvester`, `refinery`, ...) to `out` as UTF-8, at most `cap` bytes. Returns its
/// full length, or 0 for no such kind.
///
/// # Safety
/// `game` must be a live handle from `game_new`, and `out` must point to `cap` writable bytes, such as a buffer
/// from `alloc(cap)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_id(game: *const Game, kind: u32, out: *mut u8, cap: u32) -> u32 {
    // SAFETY: a live handle from game_new.
    let Some(k) = (unsafe { kind_rules(game, kind) }) else { return 0 };
    let id = k.id.as_bytes();
    let n = id.len().min(cap as usize);
    // SAFETY: the caller gives `cap` writable bytes at `out`, and `n <= cap`.
    unsafe { std::ptr::copy_nonoverlapping(id.as_ptr(), out, n) };
    id.len() as u32
}

/// 1 if a kind is a building, 0 if a unit.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_building(game: *const Game, kind: u32) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { kind_rules(game, kind) }.map_or(0, |k| u32::from(k.building))
}

/// A kind's full health under the game's rules, for health bars.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_max_health(game: *const Game, kind: u32) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { kind_rules(game, kind) }.map_or(0, |k| clamp(k.max_health))
}

/// How much a kind can carry, for cargo bars: a harvester's capacity, or 0 for kinds that carry nothing.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_capacity(game: *const Game, kind: u32) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { kind_rules(game, kind) }.and_then(|k| k.harvester.as_ref()).map_or(0, |h| clamp(h.capacity))
}

/// A kind's footprint width in tiles, counted from its top-left tile; 1 for units, 0 for no such kind.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_width(game: *const Game, kind: u32) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { kind_rules(game, kind) }.map_or(0, |k| k.width)
}

/// A kind's footprint height in tiles; 1 for units, 0 for no such kind.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_kind_height(game: *const Game, kind: u32) -> i32 {
    // SAFETY: a live handle from game_new.
    unsafe { kind_rules(game, kind) }.map_or(0, |k| k.height)
}

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_entity_count(game: *const Game) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.state.entities.len() as u32
}

/// Copy every entity, in id order, into `out` as `ENTITY_FIELDS` numbers each, stopping when `cap` numbers are
/// full. Returns how many entities were written.
///
/// # Safety
/// `game` must be a live handle from `game_new`, and `out` must point to `cap` writable, 4-byte-aligned `i32`s,
/// such as a buffer from `alloc(4 * cap)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_entities(game: *const Game, out: *mut i32, cap: u32) -> u32 {
    // SAFETY: a live handle from game_new.
    let g = unsafe { &*game };
    // SAFETY: the caller gives `cap` writable, aligned i32s at `out`.
    let buf = unsafe { std::slice::from_raw_parts_mut(out, cap as usize) };
    let mut written = 0;
    for (e, row) in g.state.entities.iter().zip(buf.chunks_exact_mut(ENTITY_FIELDS as usize)) {
        let task = e.task.map_or(-1, |t| match t {
            Task::Seek => 0,
            Task::ToField => 1,
            Task::Mining => 2,
            Task::ToRefinery => 3,
            Task::Unloading => 4,
            Task::Stuck => 5,
        });
        row.copy_from_slice(&[
            e.id as i32,
            i32::from(e.kind.0),
            e.owner as i32,
            clamp(e.x),
            clamp(e.y),
            clamp(e.health),
            e.order as i32,
            task,
            e.cargo.map_or(-1, clamp),
        ]);
        written += 1;
    }
    written
}

/// Give back a buffer from `alloc(len)` that the caller no longer needs.
///
/// # Safety
/// `ptr` must come from `alloc(len)` with the same `len`, and not have been passed to `game_new` or here before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    // SAFETY: the caller passes back a buffer `alloc(len)` gave, once.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}
