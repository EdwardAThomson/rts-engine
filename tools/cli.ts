// Headless run from the command line:
//   node tools/cli.ts [--map maps/test-01.txt] [--seed 1] [--ticks 9000] [--every 1500]
// Prints one JSON line every --every ticks and the event counts at the end. No window, no graphics.

import { readFileSync } from "node:fs";
import { createGame } from "../src/sim/game.ts";

const args = new Map<string, string>();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
const mapPath = args.get("map") ?? new URL("../maps/test-01.txt", import.meta.url).pathname;
const seed = Number(args.get("seed") ?? 1);
const ticks = Number(args.get("ticks") ?? 9000);
const every = Number(args.get("every") ?? 1500);

const game = createGame({ map: readFileSync(mapPath, "utf8"), seed });
const t0 = performance.now();
for (let t = 0; t < ticks; t += every) {
  game.step(Math.min(every, ticks - t));
  const s = game.snapshot();
  console.log(JSON.stringify({
    tick: s.tick, credits: s.players.map((p) => p.credits), resourceLeft: s.resourceLeft,
    harvesters: s.entities.filter((e) => e.type === "harvester").map((e) => `${e.id}:${e.task}:${e.cargo}`),
    hash: game.hash(),
  }));
}
const counts: Record<string, number> = {};
for (const e of game.events) counts[e.event] = (counts[e.event] ?? 0) + 1;
console.log(JSON.stringify({ seed, ticks: game.state.tick, entities: game.state.entities.length, events: counts,
  ms: Math.round(performance.now() - t0) }));
