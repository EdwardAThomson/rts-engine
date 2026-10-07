//! The Classic engine as a WebAssembly module, for the browser build. The same simulation code as the native
//! build, so a match gives the same state hashes in both.
//!
//! The interface is plain functions over a game handle, with no binding generator: JavaScript copies the map
//! text into memory from `alloc`, calls `game_new`, then drives the game. The renderer will read state through
//! further functions added here as it needs them. See `web/check.mjs` for a caller.

use classic_sim::{CommandOrder, Game, GameOptions};

/// Reserve `len` bytes for the caller to write into (the map text). Freed by `game_new`.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Start a game from the map text at `ptr` (from `alloc`, `len` bytes), with every start position played.
/// Returns a handle, or null if the map is invalid.
///
/// # Safety
/// `ptr` must come from `alloc(len)` and hold `len` bytes; it is freed here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_new(ptr: *mut u8, len: usize, seed: i32) -> *mut Game {
    // SAFETY: the caller passes back the buffer `alloc(len)` gave, filled with `len` bytes.
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    let Ok(text) = String::from_utf8(bytes) else { return std::ptr::null_mut() };
    match Game::new(GameOptions { map: &text, seed, players: None }) {
        Ok(game) => Box::into_raw(Box::new(game)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_step(game: *mut Game, n: u32) {
    // SAFETY: a live handle from game_new, used from one thread.
    unsafe { &mut *game }.step(n);
}

/// Order one unit to move to a tile, as a click would.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_order_move(game: *mut Game, player: u32, id: u32, x: i32, y: i32) {
    // SAFETY: a live handle from game_new, used from one thread.
    unsafe { &mut *game }.order(player, &[id], CommandOrder::Move { x, y });
}

/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_tick(game: *const Game) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.state.tick
}

/// The state hash as a number; `hash.toString(16).padStart(8, "0")` gives the hex form.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_hash(game: *const Game) -> u32 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.hash_value()
}

/// Credits of a player.
///
/// # Safety
/// `game` must be a live handle from `game_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_credits(game: *const Game, player: u32) -> i64 {
    // SAFETY: a live handle from game_new.
    unsafe { &*game }.state.players.get(player as usize).map_or(0, |p| p.credits)
}

/// # Safety
/// `game` must be a live handle from `game_new`, and is invalid afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_free(game: *mut Game) {
    if !game.is_null() {
        // SAFETY: a live handle from game_new, freed once.
        drop(unsafe { Box::from_raw(game) });
    }
}
