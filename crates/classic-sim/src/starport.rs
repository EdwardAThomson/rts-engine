//! The starport market (rules-economy-production.md, section 12; the `starport` module). A starport is a shop, not
//! a factory: its owner puts units from the catalogue (every kind with a `starport_price`) into an order of up to
//! `max_order`, then pays for the whole order at once at the prices of the moment. After `delivery_ticks` (twice
//! that if they were short of power when they paid) a supply ship lands on the starport, sets the units down at its
//! exits one every `unload_every` ticks, and flies off the map again. If the starport is lost before the last unit
//! is out, what is still aboard is refunded.
//!
//! The market opens when the first starport stands, so a game without one has no market and hashes as before. From
//! then on every `drift_every` ticks each price moves by a step drawn from the game's generator, within
//! `min_percent` to `max_percent` of base, one draw per item in catalogue order; the prices are the same for every
//! player. Stock is each player's own: buying takes one, and it grows back one every `starport_restock` ticks.

use rts_core::hash::{Canon, CanonHasher};
use rts_core::rng::random_int;

use crate::map::{MapData, TILE, Tile};
use crate::path::Pathfinder;
use crate::power::Power;
use crate::production;
use crate::units::{Kind, Rules};
use crate::world::{self, Event, GameState, Order};

/// Prices and every player's stock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    /// Each catalogue item's price as a percent of its base, in catalogue order.
    pub percent: Vec<i64>,
    /// The tick prices next move.
    pub next_drift: u32,
    /// Stock left, player by player, each in catalogue order.
    pub stock: Vec<u32>,
    /// The tick each of those grows back by one; 0 while full.
    pub restock_at: Vec<u32>,
}

impl Canon for Market {
    fn canon(&self, w: &mut CanonHasher) {
        w.object()
            .field("nextDrift", &self.next_drift)
            .array("percent", &self.percent)
            .array("restockAt", &self.restock_at)
            .array("stock", &self.stock)
            .end();
    }
}

/// Where an order is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Being put together; nothing paid yet.
    Open,
    /// Paid for; the ship comes on tick `at`.
    Paid,
    /// The ship is down on the starport; the next unit leaves on tick `at`.
    Landed,
    /// The ship is on its way off the map.
    Leaving,
}

impl Stage {
    pub fn id(self) -> &'static str {
        match self {
            Stage::Open => "open",
            Stage::Paid => "paid",
            Stage::Landed => "landed",
            Stage::Leaving => "leaving",
        }
    }
}

/// One starport's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub starport: u32,
    pub owner: u32,
    /// The units still to come, each with what was paid for it (0 while the order is open), first out first.
    pub items: Vec<(Kind, i64)>,
    pub stage: Stage,
    pub at: u32,
    /// The supply ship bringing it, once it has set off.
    pub ship: Option<u32>,
}

/// An order as the state hash writes it, with kinds spelt as their generic ids.
pub(crate) struct DeliveryCanon<'a>(pub &'a Delivery, pub &'a [String]);

struct ItemCanon<'a>(&'a (Kind, i64), &'a [String]);

impl Canon for ItemCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let (k, paid) = self.0;
        w.object().field("item", self.1[k.0 as usize].as_str()).field("paid", paid).end();
    }
}

impl Canon for DeliveryCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let d = self.0;
        let items: Vec<ItemCanon> = d.items.iter().map(|i| ItemCanon(i, self.1)).collect();
        w.object()
            .field("at", &d.at)
            .array("items", &items)
            .field("owner", &d.owner)
            .opt("ship", d.ship.as_ref())
            .field("stage", d.stage.id())
            .field("starport", &d.starport)
            .end();
    }
}

/// Why a starport order was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarportError {
    /// The module is off, the kind isn't in the catalogue, or the game's tech level is below the kind's.
    NotSold,
    /// The player has no starport (or the one named isn't theirs).
    NoStarport,
    /// This starport's last order hasn't come in yet.
    Busy,
    /// The order already holds `max_order` units.
    Full,
    /// None of that kind left in the player's stock.
    OutOfStock,
    /// Remove or confirm: the order doesn't hold that kind, or holds nothing.
    NotInOrder,
    /// The whole order costs more than the player has.
    Funds,
}

impl StarportError {
    pub fn id(self) -> &'static str {
        match self {
            StarportError::NotSold => "not_sold",
            StarportError::NoStarport => "no_starport",
            StarportError::Busy => "busy",
            StarportError::Full => "full",
            StarportError::OutOfStock => "out_of_stock",
            StarportError::NotInOrder => "not_in_order",
            StarportError::Funds => "funds",
        }
    }
}

fn is_starport(rules: &Rules, kind: Kind) -> bool {
    rules.kind(kind).id == "starport"
}

/// The index of a catalogue item, if `kind` is sold.
fn item(rules: &Rules, kind: Kind) -> Option<usize> {
    rules.starport.as_ref()?.catalogue.iter().position(|&k| k == kind)
}

/// What `kind` costs at the starport now, if it is sold. Before the market opens, its base price.
pub fn price(state: &GameState, rules: &Rules, kind: Kind) -> Option<i64> {
    let i = item(rules, kind)?;
    let pct = state.market.as_ref().map_or(100, |m| m.percent[i]);
    Some(rules.kind(kind).starport_price * pct / 100)
}

/// How many `kind` `player` may still buy before it restocks (the stock less what their open orders hold).
pub fn stock(state: &GameState, rules: &Rules, player: u32, kind: Kind) -> u32 {
    let Some(i) = item(rules, kind) else { return 0 };
    let n = rules.starport.as_ref().map_or(0, |s| s.catalogue.len());
    let left = state
        .market
        .as_ref()
        .and_then(|m| m.stock.get(player as usize * n + i).copied())
        .unwrap_or(rules.kind(kind).starport_stock);
    let open = state
        .deliveries
        .iter()
        .filter(|d| d.owner == player && d.stage == Stage::Open)
        .flat_map(|d| &d.items)
        .filter(|(k, _)| *k == kind)
        .count() as u32;
    left.saturating_sub(open)
}

/// The starport that takes `player`'s order: the one named in `ids` if it is theirs, else their first.
fn starport_for(state: &GameState, rules: &Rules, player: u32, ids: &[u32]) -> Option<u32> {
    let fits = |e: &&world::Entity| e.owner == player && is_starport(rules, e.kind) && e.selling == 0;
    match ids.first() {
        Some(&id) => state.entity(id).filter(fits).map(|e| e.id),
        None => state.entities.iter().find(fits).map(|e| e.id),
    }
}

/// Put one `kind` into `player`'s order at their starport.
pub fn add(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], kind: Kind, events: &mut Vec<Event>) {
    let result = (|| {
        let Some(sp) = rules.starport.as_ref() else { return Err(StarportError::NotSold) };
        item(rules, kind).ok_or(StarportError::NotSold)?;
        if state.tech_level.is_some_and(|t| rules.kind(kind).tech_level > t) {
            return Err(StarportError::NotSold);
        }
        let port = starport_for(state, rules, player, ids).ok_or(StarportError::NoStarport)?;
        match state.deliveries.iter().position(|d| d.starport == port) {
            Some(d) if state.deliveries[d].stage != Stage::Open => Err(StarportError::Busy),
            Some(d) if state.deliveries[d].items.len() >= sp.max_order => Err(StarportError::Full),
            _ if stock(state, rules, player, kind) == 0 => Err(StarportError::OutOfStock),
            Some(d) => {
                state.deliveries[d].items.push((kind, 0));
                Ok(())
            }
            None => {
                let at = state.deliveries.partition_point(|d| d.starport < port);
                let d = Delivery {
                    starport: port,
                    owner: player,
                    items: vec![(kind, 0)],
                    stage: Stage::Open,
                    at: 0,
                    ship: None,
                };
                state.deliveries.insert(at, d);
                Ok(())
            }
        }
    })();
    if let Err(reason) = result {
        events.push(Event::StarportRefused { tick: state.tick, player, kind: Some(kind), reason });
    }
}

/// Take the last `kind` back out of `player`'s open order.
pub fn remove(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], kind: Kind, events: &mut Vec<Event>) {
    let port = starport_for(state, rules, player, ids);
    let d = state.deliveries.iter().position(|d| Some(d.starport) == port && d.stage == Stage::Open);
    let found = d.and_then(|d| Some((d, state.deliveries[d].items.iter().rposition(|(k, _)| *k == kind)?)));
    let Some((d, at)) = found else {
        events.push(Event::StarportRefused {
            tick: state.tick,
            player,
            kind: Some(kind),
            reason: StarportError::NotInOrder,
        });
        return;
    };
    state.deliveries[d].items.remove(at);
    if state.deliveries[d].items.is_empty() {
        state.deliveries.remove(d);
    }
}

/// Pay for `player`'s open order at today's prices and send for it, or refuse it whole if they can't pay.
pub fn confirm(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], events: &mut Vec<Event>) {
    let tick = state.tick;
    let refuse =
        |events: &mut Vec<Event>, reason| events.push(Event::StarportRefused { tick, player, kind: None, reason });
    let Some(sp) = rules.starport.as_ref() else { return refuse(events, StarportError::NotSold) };
    let port = starport_for(state, rules, player, ids);
    let Some(d) = state.deliveries.iter().position(|d| Some(d.starport) == port && d.stage == Stage::Open) else {
        return refuse(events, StarportError::NotInOrder);
    };
    let prices: Vec<i64> =
        state.deliveries[d].items.iter().map(|&(k, _)| price(state, rules, k).unwrap_or(0)).collect();
    let cost: i64 = prices.iter().sum();
    let Some(p) = state.players.iter().position(|p| p.id == player) else { return };
    if cost > state.players[p].credits {
        return refuse(events, StarportError::Funds);
    }
    state.players[p].credits -= cost;
    let short = Power::of(state, rules, player).is_short();
    let wait = if short { sp.delivery_ticks * 2 } else { sp.delivery_ticks };
    let n = sp.catalogue.len();
    let kinds: Vec<Kind> = state.deliveries[d].items.iter().map(|&(k, _)| k).collect();
    if let Some(m) = state.market.as_mut() {
        for k in kinds {
            let i = player as usize * n + item(rules, k).expect("checked when added");
            m.stock[i] = m.stock[i].saturating_sub(1);
            if m.restock_at[i] == 0 {
                m.restock_at[i] = tick + rules.kind(k).starport_restock;
            }
        }
    }
    let o = &mut state.deliveries[d];
    for (item, paid) in o.items.iter_mut().zip(prices) {
        item.1 = paid;
    }
    o.stage = Stage::Paid;
    o.at = tick + wait;
    let (starport, arrive) = (o.starport, o.at);
    events.push(Event::StarportOrderPlaced { tick, starport, player, cost, arrive });
}

/// The tile on the map's edge nearest the starport, where its supply ship comes in and goes out.
fn edge(map: &MapData, port: Tile) -> Tile {
    let (w, h) = (map.width - 1, map.height - 1);
    let options = [
        (port.x, Tile { x: 0, y: port.y }),
        (w - port.x, Tile { x: w, y: port.y }),
        (port.y, Tile { x: port.x, y: 0 }),
        (h - port.y, Tile { x: port.x, y: h }),
    ];
    options.into_iter().min_by_key(|&(d, t)| (d, t.y, t.x)).map(|(_, t)| t).expect("four sides")
}

/// The starport phase of one tick: the market opens, drifts and restocks; orders come in.
pub fn tick(map: &MapData, pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let Some(sp) = rules.starport.as_ref() else { return };
    let tick = state.tick;
    let n = sp.catalogue.len();
    // The market opens with the first starport.
    if state.market.is_none() && state.entities.iter().any(|e| is_starport(rules, e.kind)) {
        let players = state.players.len();
        let stock = (0..players).flat_map(|_| sp.catalogue.iter().map(|&k| rules.kind(k).starport_stock)).collect();
        state.market = Some(Market {
            percent: vec![100; n],
            next_drift: tick + sp.drift_every,
            stock,
            restock_at: vec![0; players * n],
        });
    }
    if let Some(mut m) = state.market.take() {
        if tick >= m.next_drift {
            for pct in &mut m.percent {
                let step = random_int(&mut state.rng, (2 * sp.drift_step + 1) as u32) as i64 - sp.drift_step;
                *pct = (*pct + step).clamp(sp.min_percent, sp.max_percent);
            }
            m.next_drift = tick + sp.drift_every;
            events.push(Event::MarketPricesChanged { tick });
        }
        for i in 0..m.stock.len() {
            if m.restock_at[i] != 0 && tick >= m.restock_at[i] {
                let k = rules.kind(sp.catalogue[i % n]);
                m.stock[i] = (m.stock[i] + 1).min(k.starport_stock);
                m.restock_at[i] = if m.stock[i] < k.starport_stock { tick + k.starport_restock } else { 0 };
            }
        }
        state.market = Some(m);
    }
    let Some(ship_kind) = rules.kind_id("supply_ship") else { return };
    let mut d = 0;
    while d < state.deliveries.len() {
        if step(map, pf, state, rules, d, ship_kind, events) {
            state.deliveries.remove(d);
        } else {
            d += 1;
        }
    }
}

/// Move one order on; true when it is done with.
fn step(
    map: &MapData,
    pf: &mut Pathfinder,
    state: &mut GameState,
    rules: &Rules,
    d: usize,
    ship_kind: Kind,
    events: &mut Vec<Event>,
) -> bool {
    let tick = state.tick;
    let sp = rules.starport.as_ref().expect("the module is on");
    let o = state.deliveries[d].clone();
    let port = state.entity(o.starport).filter(|e| e.owner == o.owner && e.selling == 0).map(|e| (e.tile(), e.id));
    let leave = |state: &mut GameState, ship: Option<u32>| {
        if let Some(s) = ship.and_then(|s| state.entities.iter().position(|e| e.id == s)) {
            let at = state.entities[s].tile();
            let out = edge(map, at);
            let e = &mut state.entities[s];
            e.path = [out].into();
            e.order = Order::Move;
        }
    };
    // Lost, sold or captured: an open order goes; whatever is paid for and not yet out comes back.
    let Some((port_tile, port_id)) = port else {
        let refund: i64 = o.items.iter().map(|&(_, paid)| paid).sum();
        if o.stage != Stage::Open && o.stage != Stage::Leaving {
            if let Some(p) = state.players.iter_mut().find(|p| p.id == o.owner) {
                p.credits += refund;
            }
            events.push(Event::StarportOrderRefunded { tick, starport: o.starport, player: o.owner, refund });
        }
        leave(state, o.ship);
        let o = &mut state.deliveries[d];
        o.items.clear();
        if o.ship.is_none() || o.stage == Stage::Open {
            return true;
        }
        o.stage = Stage::Leaving;
        return false;
    };
    let k = rules.kind(state.entity(port_id).expect("standing").kind);
    let landing = Tile { x: port_tile.x + k.width / 2, y: port_tile.y + k.height / 2 };
    match o.stage {
        Stage::Open => false,
        Stage::Paid => {
            match o.ship {
                // Sets off in time to land when the order is due.
                None => {
                    let from = edge(map, landing);
                    let ship = rules.kind(ship_kind);
                    let dist =
                        (((from.x - landing.x) as i64).pow(2) + ((from.y - landing.y) as i64).pow(2)) * TILE * TILE;
                    // Flat out, plus time for the slow last two tiles.
                    let flight = (rts_core::imath::isqrt(dist as u64) as i64 / ship.speed.max(1)) as u32 + 64;
                    if tick + flight >= o.at {
                        let id = world::spawn(state, rules, ship_kind, o.owner, from.x, from.y);
                        let e = state.entities.last_mut().expect("just spawned");
                        e.altitude = rules.air.cruise_altitude;
                        e.path = [landing].into();
                        e.order = Order::Move;
                        state.deliveries[d].ship = Some(id);
                    }
                }
                Some(s) => {
                    let down = state.entity(s).is_none_or(|e| e.path.is_empty() && e.tile() == landing);
                    if down && tick >= o.at {
                        events.push(Event::SupplyShipLanded { tick, starport: o.starport, ship: s });
                        let o = &mut state.deliveries[d];
                        o.stage = Stage::Landed;
                        o.at = tick;
                    }
                }
            }
            false
        }
        Stage::Landed => {
            if tick < o.at {
                return false;
            }
            let Some(&(kind, _)) = o.items.first() else {
                leave(state, o.ship);
                state.deliveries[d].stage = Stage::Leaving;
                return false;
            };
            let Some(i) = state.entities.iter().position(|e| e.id == port_id) else { return false };
            if production::deliver(map, pf, state, rules, i, kind, events).is_some() {
                let o = &mut state.deliveries[d];
                o.items.remove(0);
                o.at = tick + sp.unload_every;
            }
            false
        }
        Stage::Leaving => {
            let gone = o.ship.and_then(|s| state.entities.iter().position(|e| e.id == s)).is_none_or(|s| {
                let e = &state.entities[s];
                e.path.is_empty() && e.tile() == edge(map, e.tile())
            });
            if gone {
                if let Some(s) = o.ship {
                    state.entities.retain(|e| e.id != s);
                    events.push(Event::SupplyShipLeft { tick, ship: s });
                }
                return true;
            }
            false
        }
    }
}
