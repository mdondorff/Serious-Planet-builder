# Serious Planet builder - Seriös Planet Bauer

A planet-scale generator, editor and real-time renderer, written in Rust with wgpu. One binary (`planet`) with three run modes; development is spec-first with OpenSpec and driven largely by AI agents under human review.

## Status

Milestones M0 (bootstrap), M1 (skeleton) and M2 (terrain rendering) are done; the owner recorded **go with changes** at the M2 gate on 2026-10-11. M3 (height-aware LOD selection, world definition, generator context, caches, hydrology) is next.

What works today:
- Deterministic cube-sphere terrain tiles from a test generator (`generate`).
- A CDLOD terrain renderer: node selection, vertex morphing, skirts, async streaming, GPU timestamps, debug views.
- A headless render harness with golden images, repro bundles and a real-GPU flight sequence (`test-render`).

Not there yet: the interactive editor window (`editor` runs headless with `--offscreen`; the window is planned per [ADR 0015](docs/adr/0015-window-surface-and-ui-painting-in-render.md)), realistic terrain, hydrology, atmosphere and cities. Current progress, open owner decisions and the queue are in [docs/status.md](docs/status.md).

## Prerequisites

- Rust (stable, edition 2021) with `cargo`.
- A GPU is optional. Everything runs on the software adapter (WARP on Windows, lavapipe on Linux); `--real` and `--adapter hardware` use the discrete GPU.
- Development machine is Windows 11 (Git Bash); CI runs Windows and Linux.

## Running it

All modes are `cargo run -p planet -- <mode> [options]`; an unknown mode or option prints the full usage (no arguments starts the editor skeleton).

```sh
# Generate one height tile (face,level,x,y) as a PNG; prints content hash and cache key
cargo run -p planet -- generate --tile 0,3,2,2 --out height.png

# Render the terrain headlessly from a scripted camera, with a debug view
cargo run -p planet -- test-render --scene terrain --view normals --out terrain.png

# Render the scripted orbit-to-1 m flight as a frame sequence (use --adapter hardware for the GPU)
cargo run --release -p planet -- test-render --scene terrain --flight 240 --out-dir flight --size 1280x720

# Editor/viewer skeleton (no window yet)
cargo run -p planet -- editor --offscreen
```

Useful options of `test-render`: `--scene hello-triangle|tiles|terrain`, `--view face|tile-id|level|morph|height|normals|depth`, `--script N|name`, `--adapter software|hardware` (default from `PLANET_ADAPTER`), `--bundle <file>` to write a repro bundle, `--repro <file>` to replay one.

## Building, testing, checking

Everything goes through `cargo xtask`:

| Command | What it does |
|---|---|
| `cargo xtask test fast` | tier A (unit/property) for changed crates |
| `cargo xtask test` | tiers A + B on the software adapter |
| `cargo xtask test gpu [--real]` | tier C goldens and debug views, on the software adapter or the discrete GPU |
| `cargo xtask check` | rustfmt, clippy `-D warnings`, crate layering, stale `pending.txt` entries |
| `cargo xtask spec-lint` | requirement IDs, tiers and coverage |
| `cargo xtask accept <milestone>` | milestone acceptance, e.g. `M2` |
| `cargo xtask repro <bundle>` | replay a repro bundle |
| `cargo xtask det-diff <a> <b>` | compare Windows and Linux determinism manifests |
| `cargo xtask bless` | list candidate golden images (promotion is the owner's `/bless`) |
| `cargo xtask perf --machine-ready` | performance harness; only on the owner's machine once charger and power profile are set |

Golden images are never updated by agents: candidates are queued in `tests/goldens/pending.txt` for the owner. Failure output goes to `target/review/` (diff images, numeric deltas).

## How the project is organised

```
crates/   core (coordinates, hashing, maths)   world-def   generators   cache   streaming
          frame (FramePlan, LOD selection)     render (only crate that touches wgpu)
          editor   app (the `planet` binary)   testkit   perf
xtask/    build, test, lint, acceptance tooling
openspec/ specs/ (current requirements per subsystem), changes/ (work in flight)
docs/     constitution.md, adr/, status.md, spikes.md, learnings.md, milestones/, background/
tests/    goldens (small PNGs, keyed by adapter family)
```

The renderer only executes a `FramePlan`; all decisions live in GPU-free crates. Positions are f64 on the CPU and tile-local or camera-relative f32 on the GPU.

## Where to read next

1. [docs/constitution.md](docs/constitution.md): the non-negotiable rules.
2. [docs/adr/](docs/adr/README.md): accepted and proposed architecture decisions.
3. [openspec/specs/](openspec/specs/): requirements per subsystem; each test cites its requirement ID.
4. [docs/status.md](docs/status.md): progress log, stop reports, what waits for the owner.
5. [docs/milestones/](docs/milestones/): per-milestone acceptance reports.
6. [CLAUDE.md](CLAUDE.md): the agent operating manual (workflow, git rules, when to stop).
7. [docs/background/](docs/background/): frozen research and setup documents; read only when an ADR or spec points at a section.
