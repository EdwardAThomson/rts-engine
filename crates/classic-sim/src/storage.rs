//! Storage (rules-economy-production.md, section 6): credits from harvests have to be stored. Each building adds its
//! `storage` number in the rules data to its owner's cap (a refinery and a silo by default); a building still in a
//! queue or waiting to be placed adds nothing. The cap is worked out from the buildings standing, never stored, like
//! power.
//!
//! The cap only limits deliveries. On each unload tick a harvester's resource goes into credits up to the cap and
//! the rest is lost, counted in the player's `lost`, so credits never rise above the cap and then fall; the harvester
//! still empties at its normal rate, so a full base never jams its dock. Starting credits and refunds may take
//! credits above the cap, and a lost silo lowers the cap without taking credits away; while above the cap, every
//! delivery is lost. The idea of a storage cap that makes silos worth building is the 1992 original's; the rules
//! and numbers here are ours.

use crate::units::Rules;
use crate::world::{Event, GameState};

/// A player's storage cap now, from their buildings in id order.
pub fn cap(state: &GameState, rules: &Rules, player: u32) -> i64 {
    state.entities.iter().filter(|e| e.owner == player).map(|e| rules.kind(e.kind).storage).sum()
}

/// Every player's storage cap, in player order, in one pass over the entities.
pub fn all(state: &GameState, rules: &Rules) -> Vec<i64> {
    let mut out = vec![0; state.players.len()];
    for e in &state.entities {
        let Some(p) = state.players.iter().position(|p| p.id == e.owner) else { continue };
        out[p] += rules.kind(e.kind).storage;
    }
    out
}

/// Put `amount` delivered by a harvester into player `p`'s (an index into `state.players`) credits, as far as the
/// cap allows, and count the rest lost. Says `storage_full` when this fills the store, and `credits_lost` at the
/// first loss after `warn_every` ticks without one. Returns what was stored.
pub(crate) fn deliver(state: &mut GameState, rules: &Rules, p: usize, amount: i64, events: &mut Vec<Event>) -> i64 {
    let tick = state.tick;
    let player = state.players[p].id;
    let cap = cap(state, rules, player);
    let pl = &mut state.players[p];
    let stored = amount.min((cap - pl.credits).max(0));
    let lost = amount - stored;
    let before = pl.credits;
    pl.credits += stored;
    pl.delivered += amount;
    if before < cap && pl.credits >= cap {
        events.push(Event::StorageFull { tick, player, cap });
    }
    if lost > 0 {
        pl.lost += lost;
        if pl.lost_warned.is_none_or(|t| tick.saturating_sub(t) >= rules.storage.warn_every) {
            pl.lost_warned = Some(tick);
            events.push(Event::CreditsLost { tick, player, amount: lost, cap });
        }
    }
    stored
}
