// The agent debug API from guide 01 section 1.4: load, step, order, spawn, state, hash. The same object is
// what a browser build will hang off window.__game, and what tests and the command line drive in Node.

import { parseMap, type MapData } from "./map.ts";
import { seedState } from "./rng.ts";
import { dockOf, spawn, step, tileOf, type Command, type GameEvent, type GameState, type LoggedCommand } from "./world.ts";
import type { UnitType } from "./units.ts";

export interface GameOptions {
  map: string;               // ASCII map text (see map.ts)
  seed: number;
  players?: number;          // defaults to every start position on the map
}

export function createGame(opts: GameOptions) {
  const map: MapData = parseMap(opts.map);
  const count = opts.players ?? map.start.length;
  const state: GameState = {
    tick: 0, rng: seedState(opts.seed), nextId: 1, resource: map.resource.slice(),
    players: [], entities: [],
  };
  // Each player starts with a refinery on its start tile, a harvester at the dock and a tank beside it.
  for (let p = 0; p < count; p++) {
    const s = map.start[p];
    if (!s) throw new Error(`map has no start position ${p + 1}`);
    state.players.push({ id: p, credits: 0, delivered: 0 });
    const refinery = spawn(state, "refinery", p, s.x, s.y);
    const dock = dockOf(refinery);
    spawn(state, "harvester", p, dock.x, dock.y).homeId = refinery.id;
    spawn(state, "tank", p, s.x + 1, s.y + 1);
  }

  const queued = new Map<number, Command[]>();   // tick -> commands to apply at the start of that tick
  const events: GameEvent[] = [];
  const log: LoggedCommand[] = [];                      // every command with its tick: a replay is seed + map + log

  return {
    map,
    state,
    events,
    /** Advance n ticks. */
    step(n = 1) {
      for (let i = 0; i < n; i++) {
        const cmds = queued.get(state.tick) ?? [];
        queued.delete(state.tick);
        step(map, state, cmds, events);
      }
    },
    /** Queue an order for the next tick, exactly as a player's click would. */
    order(player: number, ids: number[], order: Command["order"], x?: number, y?: number) {
      const cmd: Command = { player, ids, order, x, y };
      const list = queued.get(state.tick) ?? [];
      list.push(cmd);
      queued.set(state.tick, list);
      log.push({ ...cmd, tick: state.tick });
    },
    /** Place a unit or building directly, for tests. */
    spawn(type: UnitType, owner: number, x: number, y: number) {
      return spawn(state, type, owner, x, y).id;
    },
    /** One whole tick as plain JSON, for agents to read. */
    snapshot() {
      return {
        tick: state.tick,
        players: state.players.map((p) => ({ ...p })),
        entities: state.entities.map((e) => ({
          id: e.id, type: e.type, owner: e.owner, tile: tileOf(e), x: e.x, y: e.y, health: e.health,
          order: e.order, task: e.task, cargo: e.cargo, pathLeft: e.path.length,
        })),
        resourceLeft: state.resource.reduce((a, b) => a + b, 0),
      };
    },
    hash: () => hashState(state),
    commandLog: () => log.slice(),
  };
}

export type Game = ReturnType<typeof createGame>;

/**
 * FNV-1a over a canonical JSON of the whole state. Equal hashes mean equal games. Object keys are sorted, so the
 * hash does not depend on the order fields were created in, and any fractional or non-finite number throws,
 * because one would mean floating-point maths has leaked into the sim.
 */
export function hashState(state: GameState): string {
  const text = canonical(state);
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16).padStart(8, "0");
}

function canonical(value: unknown, path = "state"): string {
  if (value === null || value === undefined) return "null";
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new Error(`${path} is ${value}; the sim must hold integers only`);
    return String(value);
  }
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map((v, i) => canonical(v, `${path}[${i}]`)).join(",")}]`;
  if (typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj).filter((k) => obj[k] !== undefined).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical(obj[k], `${path}.${k}`)}`).join(",")}}`;
  }
  throw new Error(`${path} has unsupported type ${typeof value}`);
}
