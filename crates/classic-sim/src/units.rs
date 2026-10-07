//! Unit and building stats. Our own numbers, to be tuned later by simulation runs, never copied from another
//! game's tables. Speeds are sub-tile units per tick (256 per tile, 15 ticks per second).

use engine_core::hash::{Canon, CanonHasher};

pub const TICKS_PER_SECOND: u32 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitType {
    Tank,
    Harvester,
    Refinery,
}

pub struct UnitStats {
    pub speed: i64,
    pub max_health: i64,
    pub building: bool,
}

pub struct HarvesterStats {
    pub capacity: i64,
    pub mine_rate: i64,
    pub unload_rate: i64,
}

pub const HARVESTER: HarvesterStats = HarvesterStats { capacity: 200, mine_rate: 4, unload_rate: 10 };

pub struct Regrowth {
    pub every_ticks: u32,
    pub amount: i64,
}

pub const RESOURCE_REGROWTH: Regrowth = Regrowth { every_ticks: 450, amount: 40 };

impl UnitType {
    pub const ALL: [UnitType; 3] = [UnitType::Tank, UnitType::Harvester, UnitType::Refinery];

    /// The generic id, as data files and the state hash spell it.
    pub fn id(self) -> &'static str {
        match self {
            UnitType::Tank => "tank",
            UnitType::Harvester => "harvester",
            UnitType::Refinery => "refinery",
        }
    }

    pub fn from_id(id: &str) -> Option<UnitType> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn stats(self) -> UnitStats {
        match self {
            UnitType::Tank => UnitStats { speed: 22, max_health: 300, building: false },
            UnitType::Harvester => UnitStats { speed: 14, max_health: 450, building: false },
            UnitType::Refinery => UnitStats { speed: 0, max_health: 900, building: true },
        }
    }
}

impl Canon for UnitType {
    fn canon(&self, w: &mut CanonHasher) {
        w.string(self.id());
    }
}
