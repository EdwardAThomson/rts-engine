//! Unit and building kinds, and the typed rules the tick reads. Kinds come from the rules data
//! (`data/rules/entities.json`, through `classic-data`): every built unit and building there is a kind, and its
//! roles say which mechanics it takes part in. The numbers carry a setting pack's tuning; they are our own, tuned by
//! simulation runs, never copied from another game's tables. Speeds are sub-tile units per tick (256 per tile, 15
//! ticks per second).

use classic_data::RulesTable;

pub const TICKS_PER_SECOND: u32 = 15;

/// A unit or building kind: an index into `Rules::kinds`, which are in generic-id order. The state hash spells
/// the generic id, never the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Kind(pub u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarvesterStats {
    pub capacity: i64,
    pub mine_rate: i64,
    pub unload_rate: i64,
}

/// Everything the tick needs to know about one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindRules {
    /// The generic id, as data files and the state hash spell it.
    pub id: String,
    pub building: bool,
    /// Zero for buildings.
    pub speed: i64,
    pub max_health: i64,
    /// Footprint in tiles, from the top-left tile; 1 by 1 for units.
    pub width: i32,
    pub height: i32,
    /// Set for kinds with the `harvester` role.
    pub harvester: Option<HarvesterStats>,
    /// Kinds with the `refinery` role take harvesters' cargo at their dock.
    pub refinery: bool,
    /// Kinds with the `wall` role block movement but don't extend their owner's building area.
    pub wall: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regrowth {
    pub every_ticks: u32,
    pub amount: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The most empty tiles allowed between a new building and the nearest building its owner already has (walls
    /// don't count). 0 means touching, diagonals included.
    pub max_gap: i32,
    /// Buildings may only stand on rock, not open ground.
    pub rock_only: bool,
}

/// Every number the tick reads, built once from a rules table before the game starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub kinds: Vec<KindRules>,
    pub regrowth: Regrowth,
    pub placement: Placement,
    /// The rules table's hash, for replays to check they run under the same numbers.
    pub hash: String,
}

impl Rules {
    /// The typed rules from a table, such as `RulesTable::builtin()` or a setting pack's tuned rules.
    pub fn from_table(t: &RulesTable) -> Result<Rules, String> {
        let mut kinds = Vec::new();
        for (id, e) in &t.entities {
            if !e.built || (e.kind != "unit" && e.kind != "building") {
                continue;
            }
            let num = |name: &str| t.number(id, name).ok_or(format!("rules data has no {id}.{name}"));
            let building = e.kind == "building";
            let role = |r: &str| e.roles.iter().any(|x| x == r);
            let harvester = if role("harvester") {
                Some(HarvesterStats {
                    capacity: num("capacity")?,
                    mine_rate: num("mine_rate")?,
                    unload_rate: num("unload_rate")?,
                })
            } else {
                None
            };
            let (width, height) = if building { (num("width")? as i32, num("height")? as i32) } else { (1, 1) };
            kinds.push(KindRules {
                id: id.clone(),
                building,
                speed: if building { 0 } else { num("speed")? },
                max_health: num("max_health")?,
                width,
                height,
                harvester,
                refinery: role("refinery"),
                wall: role("wall"),
            });
        }
        if kinds.len() > u16::MAX as usize {
            return Err("too many kinds".into());
        }
        let num = |id: &str, name: &str| t.number(id, name).ok_or(format!("rules data has no {id}.{name}"));
        let module = |id: &str, name: &str| t.module_number(id, name).ok_or(format!("rules data has no {id}.{name}"));
        let every = num("resource", "regrow_every_ticks")?;
        Ok(Rules {
            kinds,
            regrowth: Regrowth {
                every_ticks: u32::try_from(every)
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or("resource.regrow_every_ticks must be positive")?,
                amount: num("resource", "regrow_amount")?,
            },
            placement: Placement {
                max_gap: module("placement", "max_gap")? as i32,
                rock_only: module("placement", "rock_only")? != 0,
            },
            hash: t.hash(),
        })
    }

    pub fn kind(&self, k: Kind) -> &KindRules {
        &self.kinds[k.0 as usize]
    }

    /// The kind with this generic id, if the simulation has it.
    pub fn kind_id(&self, id: &str) -> Option<Kind> {
        self.kinds.binary_search_by(|k| k.id.as_str().cmp(id)).ok().map(|i| Kind(i as u16))
    }

    /// Every kind's generic id, in kind order, for the state hash.
    pub fn kind_ids(&self) -> Vec<String> {
        self.kinds.iter().map(|k| k.id.clone()).collect()
    }
}

impl Default for Rules {
    /// The engine's own rules data, untuned.
    fn default() -> Self {
        Rules::from_table(&RulesTable::builtin()).expect("data/rules matches the simulation")
    }
}
