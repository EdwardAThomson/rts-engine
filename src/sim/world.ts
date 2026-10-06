// The game state and the fixed-tick update. Everything here is deterministic: integer maths, one seeded
// random generator in the state, entities kept in an array in id order, and no clock or Math.random().

import { findPath } from "./path.ts";
import { inBounds, passable, RESOURCE_PER_TILE, TILE, type MapData } from "./map.ts";
import { isqrt } from "./imath.ts";
import { randomInt } from "./rng.ts";
import { RESOURCE_REGROWTH, UnitTypes, type UnitType } from "./units.ts";

export type HarvestTask = "seek" | "toField" | "mining" | "toRefinery" | "unloading" | "stuck";

export interface Entity {
  id: number;
  type: UnitType;
  owner: number;
  x: number;                 // centre, in sub-tile units
  y: number;
  health: number;
  path: { x: number; y: number }[];
  order: "idle" | "move" | "harvest";
  task?: HarvestTask;        // harvesters only
  cargo?: number;
  homeId?: number;           // refinery this harvester delivers to
}

export interface Player {
  id: number;
  credits: number;
  delivered: number;         // total resource ever delivered, for statistics
}

export interface GameState {
  tick: number;
  rng: number;
  nextId: number;
  resource: number[];        // per tile, copied from the map at start
  players: Player[];
  entities: Entity[];        // always sorted by id
}

export interface Command {
  player: number;
  ids: number[];
  order: "move" | "harvest";
  x?: number;                // target tile for "move"
  y?: number;
}

/** A command as stored in the log: a replay is seed + map + these. */
export type LoggedCommand = Command & { tick: number };

export interface GameEvent {
  tick: number;
  event: string;
  [key: string]: number | string;
}

export const tileOf = (e: { x: number; y: number }) => ({ x: Math.floor(e.x / TILE), y: Math.floor(e.y / TILE) });
const centre = (t: number) => t * TILE + TILE / 2;

export function spawn(state: GameState, type: UnitType, owner: number, tx: number, ty: number): Entity {
  const e: Entity = {
    id: state.nextId++, type, owner, x: centre(tx), y: centre(ty), health: UnitTypes[type].maxHealth,
    path: [], order: "idle",
  };
  if (type === "harvester") {
    e.order = "harvest";
    e.task = "seek";
    e.cargo = 0;
  }
  state.entities.push(e);
  return e;
}

/** The tile a harvester parks on to unload: directly below the refinery. */
export function dockOf(refinery: Entity) {
  const t = tileOf(refinery);
  return { x: t.x, y: t.y + 1 };
}

export function applyCommand(map: MapData, state: GameState, cmd: Command) {
  for (const id of cmd.ids) {
    const e = state.entities.find((u) => u.id === id);
    if (!e || e.owner !== cmd.player || UnitTypes[e.type].building) continue;
    if (cmd.order === "move" && cmd.x !== undefined && cmd.y !== undefined) {
      const from = tileOf(e);
      e.path = findPath(map, from.x, from.y, cmd.x, cmd.y)?.tiles ?? [];
      e.order = "move";
    } else if (cmd.order === "harvest" && e.type === "harvester") {
      e.order = "harvest";
      e.task = "seek";
      e.path = [];
    }
  }
}

/** Advance one tick. `events` receives what happened, for logs and cosmetics; it never feeds back in. */
export function step(map: MapData, state: GameState, commands: Command[], events: GameEvent[]) {
  for (const cmd of commands) applyCommand(map, state, cmd);
  for (const e of state.entities) {
    if (e.type === "harvester" && e.order === "harvest") harvest(map, state, e, events);
    else moveAlongPath(e);
    if (e.order === "move" && e.path.length === 0) e.order = "idle";
  }
  regrow(map, state, events);
  state.tick++;
}

/** Move toward the next path tile's centre. Returns true when the path is finished. */
function moveAlongPath(e: Entity): boolean {
  let budget = UnitTypes[e.type].speed;
  while (budget > 0 && e.path.length > 0) {
    const next = e.path[0];
    const dx = centre(next.x) - e.x, dy = centre(next.y) - e.y;
    const dist = isqrt(dx * dx + dy * dy);
    if (dist <= budget) {
      e.x += dx;
      e.y += dy;
      budget -= dist;
      e.path.shift();
    } else {
      e.x += Math.trunc((dx * budget) / dist);
      e.y += Math.trunc((dy * budget) / dist);
      budget = 0;
    }
  }
  return e.path.length === 0;
}

function harvest(map: MapData, state: GameState, e: Entity, events: GameEvent[]) {
  const stats = UnitTypes.harvester;
  const here = tileOf(e);
  switch (e.task) {
    case "seek": {
      const field = nearestResource(map, state, here.x, here.y);
      if (!field) {
        e.task = "stuck";
        events.push({ tick: state.tick, event: "harvester_idle", unit: e.id, reason: "no_resource" });
        return;
      }
      e.path = findPath(map, here.x, here.y, field.x, field.y)?.tiles ?? [];
      e.task = "toField";
      return;
    }
    case "toField":
      if (moveAlongPath(e)) e.task = "mining";
      return;
    case "mining": {
      const i = here.y * map.width + here.x;
      const take = Math.min(stats.mineRate, state.resource[i], stats.capacity - e.cargo!);
      state.resource[i] -= take;
      e.cargo! += take;
      if (e.cargo! >= stats.capacity) {
        const home = homeRefinery(state, e);
        if (!home) {
          e.task = "stuck";
          events.push({ tick: state.tick, event: "harvester_idle", unit: e.id, reason: "no_refinery" });
          return;
        }
        const dock = dockOf(home);
        e.path = findPath(map, here.x, here.y, dock.x, dock.y)?.tiles ?? [];
        e.task = "toRefinery";
      } else if (take === 0) {
        e.task = "seek";      // tile ran dry before we were full
      }
      return;
    }
    case "toRefinery":
      if (moveAlongPath(e)) e.task = "unloading";
      return;
    case "unloading": {
      const amount = Math.min(stats.unloadRate, e.cargo!);
      e.cargo! -= amount;
      const p = state.players[e.owner];
      p.credits += amount;
      p.delivered += amount;
      if (e.cargo === 0) {
        events.push({ tick: state.tick, event: "delivered", unit: e.id, player: e.owner, credits: p.credits });
        e.task = "seek";
      }
      return;
    }
    case "stuck":
      return;
  }
}

function homeRefinery(state: GameState, e: Entity): Entity | undefined {
  const own = state.entities.filter((r) => r.type === "refinery" && r.owner === e.owner);
  const chosen = own.find((r) => r.id === e.homeId) ?? own[0];
  if (chosen) e.homeId = chosen.id;
  return chosen;
}

/** Breadth-first search outward over passable tiles; the first tile with resource left wins. */
function nearestResource(map: MapData, state: GameState, sx: number, sy: number) {
  const w = map.width;
  const seen = new Set<number>([sy * w + sx]);
  const queue = [sy * w + sx];
  for (let qi = 0; qi < queue.length; qi++) {
    const t = queue[qi];
    if (state.resource[t] > 0) return { x: t % w, y: Math.floor(t / w) };
    const x = t % w, y = Math.floor(t / w);
    for (const [dx, dy] of [[0, -1], [1, 0], [0, 1], [-1, 0]]) {
      const nx = x + dx, ny = y + dy, n = ny * w + nx;
      if (passable(map, nx, ny) && !seen.has(n)) {
        seen.add(n);
        queue.push(n);
      }
    }
  }
  return null;
}

/** Every so often, a random tile next to an existing field grows some resource back. */
function regrow(map: MapData, state: GameState, events: GameEvent[]) {
  if (state.tick % RESOURCE_REGROWTH.everyTicks !== 0 || state.tick === 0) return;
  const i = randomInt(state, map.width * map.height);
  const x = i % map.width, y = Math.floor(i / map.width);
  const nearField = [[0, 0], [0, -1], [1, 0], [0, 1], [-1, 0]].some(([dx, dy]) =>
    inBounds(map, x + dx, y + dy) && map.resource[(y + dy) * map.width + x + dx] > 0);
  if (!nearField || !passable(map, x, y) || map.terrain[i] !== 0) return;
  state.resource[i] = Math.min(RESOURCE_PER_TILE, state.resource[i] + RESOURCE_REGROWTH.amount);
  events.push({ tick: state.tick, event: "regrowth", x, y, amount: state.resource[i] });
}
