//! Selling (rules-base-building-power.md, "Selling"; the `sell` module, on unless a pack turns it off). Selling is a
//! modern comfort the 1992 original lacked; a pack after the original's feel switches it off.
//!
//! Its owner orders any own building sold. For `sell.ticks` ticks it stops working (no production, no power
//! output, no firing, no repair) but still draws power and can still be shot; destroyed in that window, it pays
//! nothing back. Then it goes, paying back `cost * health * refund_percent / (max_health * 100)` credits, plus
//! whatever its queue had paid, as cancelling would. A player may sell their last construction yard. The refund may
//! take credits above the storage cap, like any refund.

use crate::path::Pathfinder;
use crate::units::Rules;
use crate::world::{self, Event, GameState};

fn index(state: &GameState, id: u32) -> Option<usize> {
    state.entities.binary_search_by_key(&id, |e| e.id).ok()
}

/// What selling building `e` would pay back now, its queue aside.
pub fn refund(rules: &Rules, e: &world::Entity) -> i64 {
    let (Some(s), k) = (&rules.sell, rules.kind(e.kind)) else { return 0 };
    k.cost * e.health.max(0) * s.refund_percent / (k.max_health * 100)
}

/// Start selling `player`'s buildings in `ids`. Does nothing while the rules have selling off.
pub fn order(state: &mut GameState, rules: &Rules, player: u32, ids: &[u32], events: &mut Vec<Event>) {
    let Some(s) = &rules.sell else { return };
    let tick = state.tick;
    for &id in ids {
        let Some(i) = index(state, id) else { continue };
        let e = &mut state.entities[i];
        if e.owner != player || !rules.kind(e.kind).building || e.selling > 0 {
            continue;
        }
        e.selling = s.ticks;
        e.target = None;
        if e.repairing {
            e.repairing = false;
            e.repair_due = 0;
            events.push(Event::RepairStopped { tick, entity: id, owner: player, whole: false });
        }
        events.push(Event::SellStarted { tick, entity: id, kind: e.kind, owner: player });
    }
}

/// The selling phase: each building being sold counts down, in id order, and those at zero go.
pub fn tick(pf: &mut Pathfinder, state: &mut GameState, rules: &Rules, events: &mut Vec<Event>) {
    let tick = state.tick;
    let mut gone = Vec::new();
    for (i, e) in state.entities.iter_mut().enumerate() {
        if e.selling > 0 {
            e.selling -= 1;
            if e.selling == 0 {
                gone.push(i);
            }
        }
    }
    for &i in &gone {
        let e = &state.entities[i];
        let paid: i64 = e.queue.iter().map(|q| q.paid).sum();
        let amount = refund(rules, e) + paid;
        let (entity, kind, owner, x, y) = (e.id, e.kind, e.owner, e.x, e.y);
        if let Some(p) = state.players.iter_mut().find(|p| p.id == owner) {
            p.credits += amount;
        }
        events.push(Event::BuildingSold { tick, entity, kind, owner, refund: amount, x, y });
    }
    for &i in gone.iter().rev() {
        let e = state.entities.remove(i);
        world::occupy(pf, rules, &e, false);
    }
}
