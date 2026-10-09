//! The score shown on the end screen: for each player, what they built, what they destroyed, what they lost and
//! what their harvesters brought in. Design: `plans/rts/ui.md`, section 9.
//!
//! Like the rest of the HUD it is an observer: it counts the game's events after each tick and never touches the
//! game. Credits harvested come from the players' own running total (`Player::delivered`).

use std::collections::BTreeMap;

use classic_sim::Game;
use classic_sim::world::Event;

/// One player's line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub player: u32,
    pub units_built: u32,
    pub buildings_built: u32,
    /// Enemy units and buildings their own units and buildings destroyed.
    pub kills: u32,
    pub units_lost: u32,
    pub buildings_lost: u32,
    pub harvested: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Score {
    lines: Vec<Line>,
    /// Who owns each entity seen, so a kill counts for the killer's owner even when the killer died too.
    owners: BTreeMap<u32, u32>,
    /// Events already counted.
    seen: usize,
}

impl Score {
    /// Count what the last ticks did. Call after each `Game::step`; the player may clear `game.events` between
    /// calls.
    pub fn after_step(&mut self, game: &Game) {
        if self.lines.len() != game.state.players.len() {
            self.lines = game.state.players.iter().map(|p| Line { player: p.id, ..Line::default() }).collect();
        }
        for e in &game.state.entities {
            self.owners.entry(e.id).or_insert(e.owner);
        }
        if game.events.len() < self.seen {
            self.seen = 0;
        }
        for event in &game.events[self.seen..] {
            match *event {
                Event::UnitBuilt { factory, entity, .. } => {
                    let owner = self.owners.get(&entity).or(self.owners.get(&factory)).copied();
                    if let Some(l) = owner.and_then(|o| self.line(o)) {
                        l.units_built += 1;
                    }
                }
                Event::BuildingPlaced { entity, owner, .. } => {
                    self.owners.insert(entity, owner);
                    if let Some(l) = self.line(owner) {
                        l.buildings_built += 1;
                    }
                }
                Event::Destroyed { kind, owner, killer, .. } => {
                    let building = game.rules.kind(kind).building;
                    if let Some(l) = self.line(owner) {
                        if building { l.buildings_lost += 1 } else { l.units_lost += 1 }
                    }
                    let by = killer.and_then(|k| self.owners.get(&k).copied()).filter(|&k| k != owner);
                    if let Some(l) = by.and_then(|k| self.line(k)) {
                        l.kills += 1;
                    }
                }
                Event::HazardAte { owner, .. } => {
                    if let Some(l) = self.line(owner) {
                        l.units_lost += 1;
                    }
                }
                _ => {}
            }
        }
        self.seen = game.events.len();
        for p in &game.state.players {
            if let Some(l) = self.line(p.id) {
                l.harvested = p.delivered;
            }
        }
    }

    fn line(&mut self, player: u32) -> Option<&mut Line> {
        self.lines.iter_mut().find(|l| l.player == player)
    }

    /// Every player's line, in player order.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}
