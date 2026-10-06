// Unit and building stats. Our own numbers, to be tuned later by simulation runs (plan section 4.5),
// never copied from another game's tables. Speeds are sub-tile units per tick (256 per tile, 15 ticks/s).

export const TICKS_PER_SECOND = 15;

export const UnitTypes = {
  tank: { speed: 22, maxHealth: 300, building: false },
  harvester: { speed: 14, maxHealth: 450, building: false, capacity: 200, mineRate: 4, unloadRate: 10 },
  refinery: { speed: 0, maxHealth: 900, building: true },
} as const;

export type UnitType = keyof typeof UnitTypes;

export const RESOURCE_REGROWTH = { everyTicks: 450, amount: 40 } as const;
