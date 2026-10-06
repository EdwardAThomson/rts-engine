# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games (see the scope in `docs/ARCHITECTURE.md`). The engine knows mechanics, never a story: every world (names, factions, art,
audio, campaign, UI theme, tuning) comes from a data-only setting pack. Read `docs/ARCHITECTURE.md` first. Design docs live in the project's
playbooks repository under `plans/rts/` (start with `settings.md`, `engineering.md` and the four `rules-*.md`).

## Commands

```bash
npm ci                  # TypeScript and Node's types, for type checking only
npm test                # node --test, no build step (Node 22 strips the types)
npm run typecheck
node tools/cli.ts --seed 1 --ticks 9000 --every 1500
```

## Rules

- **Clean room.** Never copy code from other remakes of the 1992 original, from OpenRA or from the released C&C
  source. Read them for ideas only and credit the idea in a comment or design doc. Never use code derived from
  decompiling the original.
- **No original files.** Never use, extract or trace anything from the original game's data files, and never put
  them in this repository or in a tool's input.
- **No protected names.** Code, data, identifiers, comments and file names use generic ids only (`power_plant`,
  `harvester`, `hazard`, `resource`, `faction_a`). Setting-specific names live only in setting packs; the private
  pack lives in its own private repository, cloned into the git-ignored `settings-private/`.
- **Determinism.** Everything under `src/sim` uses integer maths (`src/sim/imath.ts`), the one seeded RNG held in
  the game state, and entities in id order. Never `Math.random()`, the clock, `Math.sqrt`/trig, or iteration
  over unordered collections. `hashState` throws on any fractional number.
- **Mechanics are modules.** Never assume three factions, one resource or land-only movement; counts and kinds come
  from data, and new mechanics are new modules.
- **Effects are observers.** Rendering, audio and logs read `events`; nothing in `events` feeds back into the state.
- **Balance numbers are ours.** They come from our own simulation runs, never from the original's tables.
- **Tests prove it.** New rules come with a scenario test; determinism and replay tests must keep passing.
- **Reports end with Verified and Not verified lists.**
