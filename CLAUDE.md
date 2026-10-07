# Classic RTS Engine

An engine for classic, 1990s-style real-time strategy games (see the scope in `docs/ARCHITECTURE.md`). The engine knows mechanics, never a story: every world (names, factions, art,
audio, campaign, UI theme, tuning) comes from a data-only setting pack. Read `docs/ARCHITECTURE.md` first. Design docs live in the project's
playbooks repository under `plans/rts/` (start with `settings.md`, `engineering.md` and the four `rules-*.md`).

## Commands

```bash
cargo test                                   # every check, about 1 s after the first build
cargo clippy --all-targets -- -D warnings    # also enforces the determinism rule below
cargo fmt
cargo run --release --bin cli -- --seed 1 --ticks 9000 --every 1500
cargo run --release --bin bench              # performance; prints hashes to compare runs
cargo run --release --bin play               # the desktop player (needs a GPU; tests use Mesa's software one)
cargo build --release --target wasm32-unknown-unknown -p classic-wasm && node web/check.mjs   # web build
python3 -m http.server 8000                  # then http://localhost:8000/web/viewer/ draws a game in the browser
```

The toolchain is pinned in `rust-toolchain.toml`. The simulation, data and tools crates have no third-party
dependencies; only the renderer (`classic-render`) has them: wgpu, winit, pollster, png and cpal (sound; on Linux
it builds against ALSA's headers, `libasound2-dev`, or build without the `device` feature for a silent player). Add
one only when it clearly pays for itself.

The shared core lives in the public `rts-core` repository and is pinned by commit (`Cargo.toml`,
`[workspace.dependencies]`). Changes to it go there, not here; moving the pin forward must keep the golden tests
passing.

## Rules

- **Clean room.** Never copy code from other remakes of the 1992 original, from OpenRA or from the released C&C
  source. Read them for ideas only and credit the idea in a comment or design doc. Never use code derived from
  decompiling the original.
- **No original files.** Never use, extract or trace anything from the original game's data files, and never put
  them in this repository or in a tool's input.
- **No protected names.** Code, data, identifiers, comments and file names use generic ids only (`power_plant`,
  `harvester`, `hazard`, `resource`, `faction_a`). Setting-specific names live only in setting packs; the private
  pack lives in its own private repository, cloned into the git-ignored `settings-private/`.
- **Determinism.** `crates/classic-sim` and the shared `rts-core` use integer maths (`rts_core::imath`), the one
  seeded generator held in the game state, and entities in id order. Never floating point, the clock, outside
  randomness, threads inside a tick, or iteration over `HashMap`/`HashSet`. Each crate's `clippy.toml` bans the
  types; keep it that way. A change that alters any state hash must say so and update the golden tests on purpose.
- **Mechanics are modules.** Never assume three factions, one resource or land-only movement; counts and kinds come
  from data, and new mechanics are new modules.
- **Effects are observers.** Rendering, audio and logs read `events`; nothing in `events` feeds back into the state.
- **Balance numbers are ours.** They come from our own simulation runs, never from the original's tables.
- **Tests prove it.** New rules come with a scenario test; determinism and replay tests must keep passing.
- **Reports end with Verified and Not verified lists.**
