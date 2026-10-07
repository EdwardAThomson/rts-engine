//! Unit and building kinds, and the typed rules the tick reads. The numbers come from the rules data
//! (`data/rules/entities.json`, through `classic-data`), with a setting pack's tuning applied; they are our own,
//! tuned by simulation runs, never copied from another game's tables. Speeds are sub-tile units per tick (256 per
//! tile, 15 ticks per second).

use classic_data::RulesTable;
use rts_core::hash::{Canon, CanonHasher};

pub const TICKS_PER_SECOND: u32 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitType {
    BattleTank,
    Harvester,
    Refinery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitStats {
    pub speed: i64,
    pub max_health: i64,
    pub building: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarvesterStats {
    pub capacity: i64,
    pub mine_rate: i64,
    pub unload_rate: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regrowth {
    pub every_ticks: u32,
    pub amount: i64,
}

/// Every number the tick reads, built once from a rules table before the game starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    units: [UnitStats; UnitType::ALL.len()],
    pub harvester: HarvesterStats,
    pub regrowth: Regrowth,
    /// The rules table's hash, for replays to check they run under the same numbers.
    pub hash: String,
}

impl UnitType {
    pub const ALL: [UnitType; 3] = [UnitType::BattleTank, UnitType::Harvester, UnitType::Refinery];

    /// The generic id, as data files and the state hash spell it.
    pub fn id(self) -> &'static str {
        match self {
            UnitType::BattleTank => "tank",
            UnitType::Harvester => "harvester",
            UnitType::Refinery => "refinery",
        }
    }

    pub fn from_id(id: &str) -> Option<UnitType> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }
}

impl Rules {
    /// The typed rules from a table, such as `RulesTable::builtin()` or a setting pack's tuned rules.
    pub fn from_table(t: &RulesTable) -> Result<Rules, String> {
        let num = |id: &str, name: &str| t.number(id, name).ok_or(format!("rules data has no {id}.{name}"));
        let unit = |kind: UnitType| -> Result<UnitStats, String> {
            let id = kind.id();
            let entry = t.entities.get(id).filter(|e| e.built).ok_or(format!("rules data has no built {id}"))?;
            let building = entry.kind == "building";
            let speed = if building { 0 } else { num(id, "speed")? };
            Ok(UnitStats { speed, max_health: num(id, "max_health")?, building })
        };
        let every = num("resource", "regrow_every_ticks")?;
        Ok(Rules {
            units: [unit(UnitType::ALL[0])?, unit(UnitType::ALL[1])?, unit(UnitType::ALL[2])?],
            harvester: HarvesterStats {
                capacity: num("harvester", "capacity")?,
                mine_rate: num("harvester", "mine_rate")?,
                unload_rate: num("harvester", "unload_rate")?,
            },
            regrowth: Regrowth {
                every_ticks: u32::try_from(every)
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or("resource.regrow_every_ticks must be positive")?,
                amount: num("resource", "regrow_amount")?,
            },
            hash: t.hash(),
        })
    }

    pub fn stats(&self, kind: UnitType) -> &UnitStats {
        &self.units[kind as usize]
    }
}

impl Default for Rules {
    /// The engine's own rules data, untuned.
    fn default() -> Self {
        Rules::from_table(&RulesTable::builtin()).expect("data/rules matches the simulation")
    }
}

impl Canon for UnitType {
    fn canon(&self, w: &mut CanonHasher) {
        w.string(self.id());
    }
}
