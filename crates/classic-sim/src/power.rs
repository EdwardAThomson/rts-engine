//! Power (rules-base-building-power.md, "Power"): each building adds to its owner's supply or draws from it (its
//! `power` number in the rules data, positive for a producer). A producer gives power in proportion to its health,
//! floor division each, so a damaged plant gives less. Power is worked out from the buildings standing, never
//! stored, so it can't drift from them. What a shortfall slows down (production, repair, radar, defences) belongs
//! to those modules; they read `Power::factor`.

use crate::units::Rules;
use crate::world::GameState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Power {
    pub supply: i64,
    pub demand: i64,
}

impl Power {
    /// A player's power now, from their buildings in id order.
    pub fn of(state: &GameState, rules: &Rules, player: u32) -> Power {
        let mut p = Power::default();
        for e in state.entities.iter().filter(|e| e.owner == player) {
            let k = rules.kind(e.kind);
            if k.power > 0 {
                p.supply += k.power * e.health.max(0) / k.max_health;
            } else {
                p.demand -= k.power;
            }
        }
        p
    }

    /// Every player's power, in player order, in one pass over the entities.
    pub fn all(state: &GameState, rules: &Rules) -> Vec<Power> {
        let mut out = vec![Power::default(); state.players.len()];
        for e in &state.entities {
            let Some(p) = state.players.iter().position(|p| p.id == e.owner) else { continue };
            let k = rules.kind(e.kind);
            if k.power > 0 {
                out[p].supply += k.power * e.health.max(0) / k.max_health;
            } else {
                out[p].demand -= k.power;
            }
        }
        out
    }

    /// How much demand is not met; zero when supply covers it.
    pub fn shortfall(self) -> i64 {
        (self.demand - self.supply).max(0)
    }

    /// The power factor, an integer percentage: 100 at full power, falling with the share of demand met but never
    /// below `rules.power.min_factor`, so a shortfall slows things down without stopping them.
    pub fn factor(self, rules: &Rules) -> i64 {
        if self.demand == 0 { 100 } else { (self.supply * 100 / self.demand).clamp(rules.power.min_factor, 100) }
    }

    pub fn is_short(self) -> bool {
        self.supply < self.demand
    }
}
