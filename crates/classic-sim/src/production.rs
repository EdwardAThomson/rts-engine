//! Production (rules-economy-production.md, sections 7, 8 and 10): every producing building has its own queue of up
//! to `queue_size` entries, and only the head entry is in progress. Progress is kept in hundredths of a tick and
//! grows by the owner's power factor each tick (100 at full power), and the cost is paid as it goes: an entry owes
//! `cost * progress / (build_ticks * 100)` by now, so a 600-credit, 450-tick item costs a steady 20 credits a
//! second. An entry its owner can't pay for pauses, losing no progress, and resumes by itself. Cancelling refunds
//! what was paid. A finished building waits at its yard, `ready`, until the player places it; a finished unit leaves
//! by its factory's exit tile, or waits `blocked` while that tile and its 8 neighbours are all taken.

use rts_core::hash::{Canon, CanonHasher};

use crate::map::{MapData, Tile};
use crate::path::Pathfinder;
use crate::power::Power;
use crate::units::{Kind, Rules};
use crate::world::{self, Event, GameState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryState {
    /// In the queue, not at the head.
    Waiting,
    /// Progress grows each tick and the cost is paid as it goes.
    Building,
    /// At the head, but its owner can't pay for this tick's progress.
    Paused,
    /// A finished building waiting to be placed.
    Ready,
    /// A finished unit with no free exit tile.
    Blocked,
}

impl EntryState {
    pub fn id(self) -> &'static str {
        match self {
            EntryState::Waiting => "waiting",
            EntryState::Building => "building",
            EntryState::Paused => "paused",
            EntryState::Ready => "ready",
            EntryState::Blocked => "blocked",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueEntry {
    pub item: Kind,
    pub state: EntryState,
    /// Hundredths of a tick at full power; complete at `build_ticks * 100`.
    pub progress: i64,
    /// Credits paid so far, never more than the cost.
    pub paid: i64,
}

/// A queue entry as the state hash writes it, with its item spelt as the generic id.
pub(crate) struct EntryCanon<'a>(pub &'a QueueEntry, pub &'a [String]);

impl Canon for EntryCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let e = self.0;
        w.object()
            .field("item", self.1[e.item.0 as usize].as_str())
            .field("paid", &e.paid)
            .field("progress", &e.progress)
            .field("state", e.state.id())
            .end();
    }
}

/// Why an order to build or cancel was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProduceError {
    /// Nothing builds this kind.
    NotBuildable,
    /// The player has no building that makes it (or the one named isn't theirs, or doesn't).
    NoFactory,
    /// The player doesn't own this required building.
    Requires {
        kind: Kind,
    },
    QueueFull,
    /// Cancel: no such entry in the queue.
    NotQueued,
}

impl ProduceError {
    pub fn id(self) -> &'static str {
        match self {
            ProduceError::NotBuildable => "not_buildable",
            ProduceError::NoFactory => "no_factory",
            ProduceError::Requires { .. } => "requires",
            ProduceError::QueueFull => "queue_full",
            ProduceError::NotQueued => "not_queued",
        }
    }
}

/// Whether `player` may build `item` now: something builds it, and they own every building it requires.
/// (Tech levels, factions and factory upgrades come later.)
pub fn can_build(state: &GameState, rules: &Rules, player: u32, item: Kind) -> Result<(), ProduceError> {
    let k = rules.kinds.get(item.0 as usize).ok_or(ProduceError::NotBuildable)?;
    if k.built_at.is_none() {
        return Err(ProduceError::NotBuildable);
    }
    for &r in &k.requires {
        if !state.entities.iter().any(|e| e.owner == player && e.kind == r) {
            return Err(ProduceError::Requires { kind: r });
        }
    }
    Ok(())
}

/// The index of the building that takes `player`'s orders for `item`: the one named in `ids` if it is theirs and
/// makes it, otherwise their primary one, the first built (lowest id).
fn factory_for(state: &GameState, rules: &Rules, player: u32, ids: &[u32], item: Kind) -> Option<usize> {
    let maker = rules.kinds.get(item.0 as usize)?.built_at?;
    let fits = |i: &usize| state.entities[*i].owner == player && state.entities[*i].kind == maker;
    match ids.first() {
        Some(&id) => state.entities.binary_search_by_key(&id, |e| e.id).ok().filter(fits),
        None => (0..state.entities.len()).find(fits),
    }
}

/// Add `item` to the end of a factory's queue.
pub fn produce(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], item: Kind, events: &mut Vec<Event>) {
    let tick = state.tick;
    let result = can_build(state, rules, player, item).and_then(|()| {
        let i = factory_for(state, rules, player, ids, item).ok_or(ProduceError::NoFactory)?;
        if state.entities[i].queue.len() >= rules.production.queue_size {
            return Err(ProduceError::QueueFull);
        }
        Ok(i)
    });
    match result {
        Ok(i) => {
            let f = &mut state.entities[i];
            f.queue.push(QueueEntry { item, state: EntryState::Waiting, progress: 0, paid: 0 });
            events.push(Event::ProductionQueued { tick, factory: f.id, kind: item });
        }
        Err(reason) => events.push(Event::ProductionRejected { tick, player, kind: item, reason }),
    }
}

/// Remove the last entry for `item` from a factory's queue and refund what was paid for it. The refund may take
/// credits above any storage cap.
pub fn cancel(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], item: Kind, events: &mut Vec<Event>) {
    let tick = state.tick;
    let found = factory_for(state, rules, player, ids, item).ok_or(ProduceError::NoFactory).and_then(|i| {
        Ok((i, state.entities[i].queue.iter().rposition(|q| q.item == item).ok_or(ProduceError::NotQueued)?))
    });
    match found {
        Ok((i, at)) => {
            let entry = state.entities[i].queue.remove(at);
            let factory = state.entities[i].id;
            credit(state, player, entry.paid);
            events.push(Event::ProductionCancelled { tick, factory, kind: item, refund: entry.paid });
        }
        Err(reason) => events.push(Event::ProductionRejected { tick, player, kind: item, reason }),
    }
}

/// Take a ready building of kind `item` off the head of `player`'s first yard holding one, for placing.
pub fn take_ready(state: &mut GameState, player: u32, item: Kind) -> Option<QueueEntry> {
    let f = state.entities.iter_mut().find(|e| {
        e.owner == player && e.queue.first().is_some_and(|q| q.item == item && q.state == EntryState::Ready)
    })?;
    Some(f.queue.remove(0))
}

/// Whether `player` has a ready building of kind `item` to place.
pub fn has_ready(state: &GameState, player: u32, item: Kind) -> bool {
    state
        .entities
        .iter()
        .any(|e| e.owner == player && e.queue.first().is_some_and(|q| q.item == item && q.state == EntryState::Ready))
}

fn credit(state: &mut GameState, player: u32, amount: i64) {
    if let Some(p) = state.players.iter_mut().find(|p| p.id == player) {
        p.credits += amount;
    }
}

/// The free tile a finished unit leaves by: the factory's exit tile (below the middle of its footprint), or else
/// the first free one of its 8 neighbours in row order. Free means open to ground movement with no unit on it.
fn exit_tile(map: &MapData, pf: &Pathfinder, state: &GameState, rules: &Rules, factory: usize) -> Option<Tile> {
    let f = &state.entities[factory];
    let t = f.tile();
    let exit = world::dock_at(rules.kind(f.kind), t.x, t.y);
    let free = |x: i32, y: i32| {
        map.in_bounds(x, y)
            && pf.passable(x, y)
            && !state.entities.iter().any(|e| !rules.kind(e.kind).building && e.tile() == Tile { x, y })
    };
    if free(exit.x, exit.y) {
        return Some(exit);
    }
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (exit.x + dx, exit.y + dy)))
        .find(|&(x, y)| free(x, y))
        .map(|(x, y)| Tile { x, y })
}

/// One tick of every queue, factories in id order: the head entry builds and pays, pauses, or finishes.
pub fn tick(map: &MapData, pf: &Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let factors: Vec<i64> = Power::all(state, rules).iter().map(|p| p.factor(rules)).collect();
    for i in 0..state.entities.len() {
        let Some(&head) = state.entities[i].queue.first() else { continue };
        let (factory, owner) = (state.entities[i].id, state.entities[i].owner);
        let Some(p) = state.players.iter().position(|p| p.id == owner) else { continue };
        let k = rules.kind(head.item);
        let mut entry = head;
        if matches!(entry.state, EntryState::Waiting | EntryState::Building | EntryState::Paused) {
            let total = k.build_ticks * 100;
            let speed = if rules.production.instant_build { total } else { factors[p] };
            let progress = (entry.progress + speed).min(total);
            let due = k.cost * progress / total;
            if due - entry.paid > state.players[p].credits {
                if entry.state != EntryState::Paused {
                    events.push(Event::ProductionPaused { tick, factory, kind: head.item });
                }
                entry.state = EntryState::Paused;
            } else {
                state.players[p].credits -= due - entry.paid;
                entry.paid = due;
                entry.progress = progress;
                entry.state = EntryState::Building;
                if progress == total {
                    entry.state = if k.building { EntryState::Ready } else { EntryState::Blocked };
                    if k.building {
                        events.push(Event::BuildingReady { tick, player: owner, factory, kind: head.item });
                    }
                }
            }
        }
        if entry.state == EntryState::Blocked
            && let Some(t) = exit_tile(map, pf, state, rules, i)
        {
            state.entities[i].queue.remove(0);
            let entity = world::spawn(state, rules, head.item, owner, t.x, t.y);
            events.push(Event::UnitBuilt { tick, factory, entity, kind: head.item });
            continue;
        }
        state.entities[i].queue[0] = entry;
    }
}
