// Seeded random numbers for gameplay. The whole generator state is one 32-bit integer stored in the game
// state, so saving, hashing and replaying a game includes it. Never use Math.random() in the simulation.

export function nextRandom(state: { rng: number }): number {
  // xorshift32: small, fast and fully reproducible across platforms (integer maths only).
  let x = state.rng | 0;
  x ^= x << 13;
  x ^= x >>> 17;
  x ^= x << 5;
  state.rng = x | 0;
  return x >>> 0;
}

/** An integer in [0, n). */
export function randomInt(state: { rng: number }, n: number): number {
  return nextRandom(state) % n;
}

export function seedState(seed: number): number {
  // xorshift must not start at 0; mix the seed so nearby seeds diverge quickly.
  let s = (seed ^ 0x9e3779b9) | 0;
  s = Math.imul(s ^ (s >>> 16), 0x85ebca6b) | 0;
  s = Math.imul(s ^ (s >>> 13), 0xc2b2ae35) | 0;
  s ^= s >>> 16;
  return s === 0 ? 1 : s;
}
