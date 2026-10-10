//! Production (rules-economy-production.md, sections 7, 8 and 10): every producing building has its own queue of up
//! to `queue_size` entries, and only the head entry is in progress. Progress is kept in hundredths of a tick and
//! grows by the owner's power factor each tick (100 at full power), and the cost is paid as it goes: an entry owes
//! `cost * progress / (build_ticks * 100)` by now, so a 600-credit, 450-tick item costs a steady 20 credits a
//! second. An entry its owner can't pay for pauses, losing no progress, and resumes by itself. Cancelling refunds
//! what was paid. A finished building waits at its yard, `ready`, until the player places it; a finished unit leaves
//! by its factory's exit tile, or waits `blocked` while that tile and its 8 neighbours are all taken.
//!
//! A player picks which of their factories of a kind is primary, the one that takes orders naming no factory (and so
//! where new units come out); with none picked it is the first built. An entry can be put on hold, which stops its
//! factory's queue there, paying nothing, until the player resumes or cancels it (rules-economy-production.md,
//! section 8).
//!
//! Tech levels and factory upgrades (sections 9 and 11): a game may have a tech level, 1 to 8, and an item whose
//! `tech_level` is above it can't be built. A producing building with a `max_level` can be upgraded: an upgrade is a
//! queue entry like any other (its item is the building's own kind, marked `upgrade`), paid and built the same way
//! from the building's `upgrade_cost` and `upgrade_ticks`, and when it finishes the building's `level` goes up by
//! one. Each level needs the game to allow its `level_1_tech` or `level_2_tech`. An item with a `factory_level` is
//! built only by a factory of at least that level; orders naming no factory go to the primary one if it is high
//! enough, else to the first that is.

use rts_core::hash::{Canon, CanonHasher};

use crate::map::{MapData, Tile};
use crate::movement;
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
    /// Put on hold by its owner: no progress and no payment until resumed. At the head it holds up the queue.
    Held,
}

impl EntryState {
    pub fn id(self) -> &'static str {
        match self {
            EntryState::Waiting => "waiting",
            EntryState::Building => "building",
            EntryState::Paused => "paused",
            EntryState::Ready => "ready",
            EntryState::Blocked => "blocked",
            EntryState::Held => "held",
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
    /// An upgrade of the building whose queue it is in (its `item` is that building's kind), not an item to build.
    pub upgrade: bool,
}

impl QueueEntry {
    /// A new entry, waiting.
    pub fn new(item: Kind) -> QueueEntry {
        QueueEntry { item, state: EntryState::Waiting, progress: 0, paid: 0, upgrade: false }
    }

    /// What it costs in all.
    pub fn cost(&self, rules: &Rules) -> i64 {
        let k = rules.kind(self.item);
        if self.upgrade { k.upgrade_cost } else { k.cost }
    }

    /// Ticks it takes at full power.
    pub fn ticks(&self, rules: &Rules) -> i64 {
        let k = rules.kind(self.item);
        if self.upgrade { k.upgrade_ticks } else { k.build_ticks }.max(1)
    }
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
            // Written only for an upgrade, so a game without them hashes as it did before.
            .opt("upgrade", e.upgrade.then_some(&true))
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
    /// Only other factions build this kind.
    Faction,
    /// Cancel, hold or resume: no such entry in the queue (none still building, for hold; none held, for resume).
    NotQueued,
    /// The game's tech level is below the item's.
    TechLevel,
    /// None of the player's factories for it has been upgraded far enough.
    FactoryLevel {
        level: u32,
    },
    /// Upgrade: the building is already as high as it can go, or as the game's tech level allows.
    MaxLevel,
}

impl ProduceError {
    pub fn id(self) -> &'static str {
        match self {
            ProduceError::NotBuildable => "not_buildable",
            ProduceError::NoFactory => "no_factory",
            ProduceError::Requires { .. } => "requires",
            ProduceError::QueueFull => "queue_full",
            ProduceError::Faction => "faction",
            ProduceError::NotQueued => "not_queued",
            ProduceError::TechLevel => "tech_level",
            ProduceError::FactoryLevel { .. } => "factory_level",
            ProduceError::MaxLevel => "max_level",
        }
    }
}

/// Whether `player` may build `item` now: something builds it, the game's tech level allows it, their faction may
/// (when the kind is limited to some), they own every building it requires, and, if they own a factory for it, one
/// is upgraded far enough.
pub fn can_build(state: &GameState, rules: &Rules, player: u32, item: Kind) -> Result<(), ProduceError> {
    let k = rules.kinds.get(item.0 as usize).ok_or(ProduceError::NotBuildable)?;
    // Slabs only mean something while buildings decay.
    if k.built_at.is_none() || (k.slab && !crate::decay::on(rules)) {
        return Err(ProduceError::NotBuildable);
    }
    if state.tech_level.is_some_and(|t| k.tech_level > t) {
        return Err(ProduceError::TechLevel);
    }
    if !k.factions.is_empty() {
        let faction = state.players.iter().find(|p| p.id == player).and_then(|p| p.faction.as_deref());
        if !faction.is_some_and(|f| k.factions.iter().any(|x| x == f)) {
            return Err(ProduceError::Faction);
        }
    }
    for &r in &k.requires {
        if !state.entities.iter().any(|e| e.owner == player && e.kind == r) {
            return Err(ProduceError::Requires { kind: r });
        }
    }
    let maker = k.built_at.expect("checked above");
    let mut makers = state.entities.iter().filter(|e| e.owner == player && e.kind == maker).peekable();
    if makers.peek().is_some() && !makers.any(|e| e.level >= k.factory_level) {
        return Err(ProduceError::FactoryLevel { level: k.factory_level });
    }
    Ok(())
}

/// The highest level the game's tech level lets a building of this kind reach.
pub fn level_cap(state: &GameState, rules: &Rules, kind: Kind) -> u32 {
    let k = rules.kind(kind);
    let allowed = |n: u32| state.tech_level.is_none_or(|t| k.level_tech[(n - 1).min(1) as usize] <= t);
    (1..=k.max_level.min(2)).take_while(|&n| allowed(n)).last().unwrap_or(0)
}

/// The index of `player`'s primary building of kind `factory`: the one they picked, else the first built (lowest id).
/// A building being sold is never primary.
pub fn primary(state: &GameState, player: u32, factory: Kind) -> Option<usize> {
    let fits = |i: &usize| {
        let e = &state.entities[*i];
        e.owner == player && e.kind == factory && e.selling == 0
    };
    let first = (0..state.entities.len()).find(fits)?;
    Some((0..state.entities.len()).filter(fits).find(|&i| state.entities[i].primary).unwrap_or(first))
}

/// The index of the building that takes `player`'s orders for `item`: the one named in `ids` if it is theirs and
/// makes it, otherwise their primary one.
fn factory_for(state: &GameState, rules: &Rules, player: u32, ids: &[u32], item: Kind) -> Option<usize> {
    let maker = rules.kinds.get(item.0 as usize)?.built_at?;
    let fits = |i: &usize| {
        let e = &state.entities[*i];
        e.owner == player && e.kind == maker && e.selling == 0
    };
    let need = rules.kind(item).factory_level;
    match ids.first() {
        Some(&id) => state.entities.binary_search_by_key(&id, |e| e.id).ok().filter(fits),
        None => primary(state, player, maker)
            .filter(|&i| state.entities[i].level >= need)
            .or_else(|| (0..state.entities.len()).find(|i| fits(i) && state.entities[*i].level >= need))
            .or_else(|| primary(state, player, maker)),
    }
}

/// Make the first of `ids` that is `player`'s and makes something their primary building of its kind.
pub fn set_primary(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], events: &mut Vec<Event>) {
    let Some(i) = ids.iter().find_map(|&id| {
        let i = state.entities.binary_search_by_key(&id, |e| e.id).ok()?;
        let e = &state.entities[i];
        let makes = rules.kinds.iter().any(|k| k.built_at == Some(e.kind));
        (e.owner == player && makes && e.selling == 0).then_some(i)
    }) else {
        return;
    };
    let (kind, entity) = (state.entities[i].kind, state.entities[i].id);
    for e in state.entities.iter_mut().filter(|e| e.owner == player && e.kind == kind) {
        e.primary = e.id == entity;
    }
    events.push(Event::PrimarySet { tick: state.tick, entity, kind, owner: player });
}

/// Add `item` to the end of a factory's queue.
pub fn produce(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], item: Kind, events: &mut Vec<Event>) {
    let tick = state.tick;
    let result = can_build(state, rules, player, item).and_then(|()| {
        let i = factory_for(state, rules, player, ids, item).ok_or(ProduceError::NoFactory)?;
        let need = rules.kind(item).factory_level;
        if state.entities[i].level < need {
            return Err(ProduceError::FactoryLevel { level: need });
        }
        if state.entities[i].queue.len() >= rules.production.queue_size {
            return Err(ProduceError::QueueFull);
        }
        Ok(i)
    });
    match result {
        Ok(i) => {
            let f = &mut state.entities[i];
            f.queue.push(QueueEntry::new(item));
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
        Ok((
            i,
            state.entities[i]
                .queue
                .iter()
                .rposition(|q| q.item == item && !q.upgrade)
                .ok_or(ProduceError::NotQueued)?,
        ))
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

/// Put the first entry for `item` still being worked on (waiting, building or paused for funds) in a factory's queue
/// on hold, or with `on` false resume the first held one. A held entry keeps what it has paid and its progress.
pub fn hold(
    state: &mut GameState,
    rules: &Rules,
    player: u32,
    ids: &[u32],
    item: Kind,
    on: bool,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let open = |q: &QueueEntry| {
        q.item == item
            && !q.upgrade
            && if on {
                matches!(q.state, EntryState::Waiting | EntryState::Building | EntryState::Paused)
            } else {
                q.state == EntryState::Held
            }
    };
    let found = factory_for(state, rules, player, ids, item)
        .ok_or(ProduceError::NoFactory)
        .and_then(|i| Ok((i, state.entities[i].queue.iter().position(open).ok_or(ProduceError::NotQueued)?)));
    match found {
        Ok((i, at)) => {
            let f = &mut state.entities[i];
            f.queue[at].state = if on { EntryState::Held } else { EntryState::Waiting };
            let factory = f.id;
            events.push(if on {
                Event::ProductionHeld { tick, factory, kind: item }
            } else {
                Event::ProductionResumed { tick, factory, kind: item }
            });
        }
        Err(reason) => events.push(Event::ProductionRejected { tick, player, kind: item, reason }),
    }
}

/// Queue an upgrade of `player`'s building of kind `kind` (the one named in `ids`, else their primary one), or with
/// `on` false take the last queued upgrade back out and refund what it paid.
pub fn upgrade(
    state: &mut GameState,
    rules: &Rules,
    player: u32,
    ids: &[u32],
    kind: Kind,
    on: bool,
    events: &mut Vec<Event>,
) {
    let tick = state.tick;
    let fits = |e: &crate::world::Entity| e.owner == player && e.kind == kind && e.selling == 0;
    let found = match ids.first() {
        Some(&id) => state.entities.binary_search_by_key(&id, |e| e.id).ok().filter(|&i| fits(&state.entities[i])),
        None => primary(state, player, kind),
    };
    let result = found.ok_or(ProduceError::NoFactory).and_then(|i| {
        let f = &state.entities[i];
        let queued = f.queue.iter().filter(|q| q.upgrade).count() as u32;
        if !on {
            return f.queue.iter().rposition(|q| q.upgrade).map(|at| (i, at)).ok_or(ProduceError::NotQueued);
        }
        if f.level + queued >= level_cap(state, rules, kind) {
            return Err(ProduceError::MaxLevel);
        }
        if f.queue.len() >= rules.production.queue_size {
            return Err(ProduceError::QueueFull);
        }
        Ok((i, f.queue.len()))
    });
    match result {
        Ok((i, at)) if on => {
            let f = &mut state.entities[i];
            f.queue.insert(at, QueueEntry { upgrade: true, ..QueueEntry::new(kind) });
            events.push(Event::ProductionQueued { tick, factory: f.id, kind });
        }
        Ok((i, at)) => {
            let entry = state.entities[i].queue.remove(at);
            let factory = state.entities[i].id;
            credit(state, player, entry.paid);
            events.push(Event::ProductionCancelled { tick, factory, kind, refund: entry.paid });
        }
        Err(reason) => events.push(Event::ProductionRejected { tick, player, kind, reason }),
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

/// The free tile a finished unit leaves by: of the tiles round the factory's footprint, sides and corners alike,
/// the one nearest the middle of the map, so units come out on the side facing the field whichever way the door is
/// drawn and wherever the base starts. A tile that opens onto more of the map comes first, so a unit never comes
/// out into a pocket the base's buildings have closed off while a way out is free. Ties go to the upper row, then
/// the left column. Free means open to ground movement with no unit on it.
fn exit_tile(map: &MapData, pf: &Pathfinder, state: &GameState, rules: &Rules, factory: usize) -> Option<Tile> {
    let free = |t: &Tile| map.in_bounds(t.x, t.y) && pf.passable(t.x, t.y) && !held(state, rules, *t);
    let (x, y, w, h) = factory_rect(state, rules, factory);
    world::around(x, y, w, h)
        .filter(free)
        .min_by_key(|t| (std::cmp::Reverse(pf.reach(t.x, t.y)), t.off_middle(map.width, map.height), t.y, t.x))
}

/// Where a finished aircraft appears: as `exit_tile`, but units on the ground don't stand in its way, so it is
/// never blocked. Off the passable ground only when the factory is walled in, and then it hovers.
fn air_exit(map: &MapData, pf: &Pathfinder, state: &GameState, rules: &Rules, factory: usize) -> Option<Tile> {
    let (x, y, w, h) = factory_rect(state, rules, factory);
    world::around(x, y, w, h).filter(|t| map.in_bounds(t.x, t.y)).min_by_key(|t| {
        (!pf.passable(t.x, t.y), std::cmp::Reverse(pf.reach(t.x, t.y)), t.off_middle(map.width, map.height), t.y, t.x)
    })
}

/// A footprint as (left, top, width, height) in tiles.
type Rect = (i32, i32, i32, i32);

fn factory_rect(state: &GameState, rules: &Rules, factory: usize) -> Rect {
    let f = &state.entities[factory];
    let (t, k) = (f.tile(), rules.kind(f.kind));
    (t.x, t.y, k.width, k.height)
}

/// Whether a unit stands on this tile or is on its way into it.
fn held(state: &GameState, rules: &Rules, t: Tile) -> bool {
    state.entities.iter().any(|e| world::on_ground(rules, e) && (e.tile() == t || movement::step_tile(e) == Some(t)))
}

/// Where a new unit drives to so the next one can come out: the nearest free tile two to four steps from the exit
/// that doesn't touch the factory, ring by ring, the one nearest the middle of the map first (rules-movement.md
/// section 7, with no rally point yet).
fn clear_of_exit(
    map: &MapData,
    pf: &Pathfinder,
    state: &GameState,
    rules: &Rules,
    factory: Rect,
    exit: Tile,
) -> Option<Tile> {
    let (fx, fy, fw, fh) = factory;
    let touches = |t: &Tile| (fx - 1..=fx + fw).contains(&t.x) && (fy - 1..=fy + fh).contains(&t.y);
    (2..=4).find_map(|r: i32| {
        (exit.y - r..=exit.y + r)
            .flat_map(|y| (exit.x - r..=exit.x + r).map(move |x| Tile { x, y }))
            .filter(|t| (t.x - exit.x).abs().max((t.y - exit.y).abs()) == r)
            .filter(|t| !touches(t) && pf.passable(t.x, t.y) && !held(state, rules, *t))
            .min_by_key(|t| (t.off_middle(map.width, map.height), t.y, t.x))
    })
}

/// One tick of every queue, factories in id order: the head entry builds and pays, pauses, or finishes.
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let factors: Vec<i64> = Power::all(state, rules).iter().map(|p| p.factor(rules)).collect();
    for i in 0..state.entities.len() {
        let Some(&head) = state.entities[i].queue.first() else { continue };
        // A factory being sold stops work; its queue is refunded when it goes. A held head holds up the queue.
        if state.entities[i].selling > 0 || head.state == EntryState::Held {
            continue;
        }
        let (factory, owner) = (state.entities[i].id, state.entities[i].owner);
        let Some(p) = state.players.iter().position(|p| p.id == owner) else { continue };
        let k = rules.kind(head.item);
        let mut entry = head;
        if matches!(entry.state, EntryState::Waiting | EntryState::Building | EntryState::Paused) {
            let total = entry.ticks(rules) * 100;
            let speed = if rules.production.instant_build { total } else { factors[p] };
            let progress = (entry.progress + speed).min(total);
            let due = entry.cost(rules) * progress / total;
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
                if progress == total && entry.upgrade {
                    let f = &mut state.entities[i];
                    f.level += 1;
                    f.queue.remove(0);
                    events.push(Event::UpgradeCompleted { tick, factory, kind: head.item, level: f.level });
                    continue;
                }
                if progress == total {
                    entry.state = if k.building { EntryState::Ready } else { EntryState::Blocked };
                    if k.building {
                        events.push(Event::BuildingReady { tick, player: owner, factory, kind: head.item });
                    }
                }
            }
        }
        if entry.state == EntryState::Blocked && deliver(map, pf, state, rules, i, head.item, events).is_some() {
            state.entities[i].queue.remove(0);
            continue;
        }
        state.entities[i].queue[0] = entry;
    }
}

/// Put a new unit of `kind`, its owner's, out of the building `state.entities[i]` by its exit, as a factory does
/// (`unit_built`): a harvester goes about its work, an aircraft waits, anything else drives clear of the exit.
/// `None`, and nothing done, while every exit is taken.
pub(crate) fn deliver(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    i: usize,
    kind: Kind,
    events: &mut Vec<Event>,
) -> Option<u32> {
    let k = rules.kind(kind);
    let t = if k.air { air_exit(map, pf, state, rules, i) } else { exit_tile(map, pf, state, rules, i) }?;
    let (factory, owner) = (state.entities[i].id, state.entities[i].owner);
    let entity = world::spawn(state, rules, kind, owner, t.x, t.y);
    events.push(Event::UnitBuilt { tick: state.tick, factory, entity, kind });
    if k.harvester.is_none()
        && !k.air
        && let Some(to) = clear_of_exit(map, pf, state, rules, factory_rect(state, rules, i), t)
    {
        let e = state.entities.last_mut().expect("just spawned");
        e.path = world::path_or_empty(pf, t, to);
        e.order = world::Order::Move;
    }
    Some(entity)
}
