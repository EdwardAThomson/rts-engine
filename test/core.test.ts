// Tests for the integer maths and the state hash that every determinism check rests on.

import { test } from "node:test";
import assert from "node:assert/strict";
import { isqrt } from "../src/sim/imath.ts";
import { hashState } from "../src/sim/game.ts";
import type { GameState } from "../src/sim/world.ts";

test("isqrt matches floor(sqrt(n)) on squares, neighbours and a sweep", () => {
  for (let r = 0; r < 5_000; r++) {
    assert.equal(isqrt(r * r), r);
    if (r > 0) assert.equal(isqrt(r * r - 1), r - 1);
    assert.equal(isqrt(r * r + r), r);
  }
  for (let n = 0; n < 200_000; n += 7) assert.equal(isqrt(n), Math.floor(Math.sqrt(n)));
  assert.equal(isqrt(2 ** 52), 2 ** 26);
  assert.throws(() => isqrt(-1));
  assert.throws(() => isqrt(2.5));
});

const base = (): GameState => ({ tick: 5, rng: 123, nextId: 2, resource: [0, 300], players: [], entities: [] });

test("hash ignores the order object keys were created in", () => {
  const a = base();
  const b = { entities: [], players: [], resource: [0, 300], nextId: 2, rng: 123, tick: 5 } as GameState;
  assert.equal(hashState(a), hashState(b));
});

test("hash changes when any value changes", () => {
  const a = base();
  const b = base();
  b.resource[1] = 299;
  assert.notEqual(hashState(a), hashState(b));
});

test("hash refuses fractional numbers, which would mean floats leaked into the sim", () => {
  const a = base();
  a.rng = 0.5;
  assert.throws(() => hashState(a), /integers only/);
});
