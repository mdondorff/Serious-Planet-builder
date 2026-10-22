# Serious Planet builder - Seriös Planet Bauer

A planet-scale generator, editor and real-time renderer, written in Rust with wgpu. One binary (`planet`) with three run modes; development is spec-first with OpenSpec and driven largely by AI agents under human review.

## Status

M0 to M6 are done; M6 (the editor) was closed by the owner on 2026-10-21. M7 (ground detail and edit health) is in progress; M8 (generation principles: driver maps) and M9 (global authoring) are planned. Current work, open owner decisions and the queue are in [docs/status.md](docs/status.md).

What works today:
- A whole planet from a global bake (16384 cells per cube face, about 0.6 km) with erosion, climate (temperature, precipitation) and an ocean connected through low ground.
- Real-time rendering from orbit down to a crouched eye at 0.5 m: height-aware CDLOD with morphing and streaming, a Hillaire atmosphere, HDR with adaptive exposure, Gerstner-wave water with shorelines, procedural ground detail with a sun highlight.
- An interactive editor: stamps (crater, hill, plateau), lines (ridge, channel), regions, a brush (raise, lower, smooth, flatten), handles, an inspector, layers, undo and redo, a globe overview, and a problems list for edits the engine cannot draw fully.
- A headless harness with golden images, repro bundles and a performance harness with a readiness gate.

Not there yet: driver maps (soil, moisture) and the plants and rocks they place, instancing, the generator editor, cities.

## Prerequisites

- Rust (stable) with `cargo`.
- The window needs a discrete GPU (integrated GPUs are refused). Tests run on the software adapter (WARP on Windows, lavapipe on Linux); `--real` and `--adapter hardware` use the discrete GPU.
- Development machine is Windows 11 (Git Bash); CI runs Windows and Linux.

## Running it

All modes are `cargo run --release -p planet -- <mode> [options]`; an unknown mode or option prints the full usage.

```sh
# The editor (a window); edits are stored as item files in the folder given
cargo run --release -p planet -- editor --edit --items target/my-items

# The viewer only (same window, no editing)
cargo run --release -p planet -- editor

# Generate one height tile (face,level,x,y) as a PNG; prints content hash and cache key
cargo run --release -p planet -- generate --tile 0,3,2,2 --out height.png

# Render headlessly from a scripted camera, with a debug view
cargo run --release -p planet -- test-render --scene terrain --view normals --out terrain.png
```

## Keys

### Viewer (always available)
| Key | What it does |
|---|---|
| W A S D, Q / E | move; down / up (down to a crouch at 0.5 m above the ground) |
| Shift | move faster |
| arrows | look around |
| click (viewer), right mouse held (editor) | capture the mouse to look around; Esc releases it |
| C | flight or free camera (not in the editor mode) |
| G | globe overview: drag turns, wheel zooms, a click on a marker or the surface flies there and lands |
| P, Space | pause |
| R | restart the flight |
| L | lit path (atmosphere) or debug path |
| V | next debug view (debug path) |
| F3 | overlay: off, tiles, mesh, regions, stale tiles (exact green, stale amber, stand-in red, previewed violet, capped grey) |
| F4 | coarse LOD (error bound x4) |
| F6 | ground detail: natural, full, off |
| F7 | scale references: a chair, house, boat, tower and block of boxes of known size, dropped in front of you |
| F9 | write the camera path of the last minute and a repro bundle to `target/review/` |
| H | editing light: a sun over your shoulder, so no slope is black |
| T | vsync on / off |
| Esc | quit (viewer); step back (editor mode) |

### Editor mode (`--edit`)
| Key | What it does |
|---|---|
| Tab | editor on / off |
| 1 | select |
| 2, 3, 4 | place a crater, hill, plateau (then click the planet) |
| 5, 6 | draw a ridge, a channel (click points, Enter finishes) |
| 7 | draw a region (click points, Enter finishes) |
| 8, 9, 0, - | brush: raise, lower, smooth, flatten (drag to paint a stroke) |
| mouse wheel, Shift + wheel | brush radius, brush strength |
| click | select the edit under the cursor (click again at the same spot for the next one under it) |
| drag a handle | circle: move; square: radius; light circle: height; amber dots: the points of a line or region |
| Shift + click, Ctrl + click | add a point to the selected line or region, remove the point under the cursor |
| Backspace | take the last point back while drawing |
| Enter, Up / Down (inspector) | keep the typed value; step it (Shift: ten times) |
| double-click in a list | fly to the edit |
| Delete | delete the selected edit |
| Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z | undo, redo |
| Esc | cancel the drag, the draft, the tool, the selection (does not quit) |
| F1, ? | list the keys |

The Problems tab lists edits that are too steep to draw fully or heavily stacked, with Fly to, Reduce, Merge strokes and Delete.

## Building, testing, checking

Everything goes through `cargo xtask`:

| Command | What it does |
|---|---|
| `cargo xtask test fast` | tier A (unit/property) for changed crates |
| `cargo xtask test` | tiers A + B on the software adapter |
| `cargo xtask test gpu [--real]` | tier C goldens and debug views, on the software adapter or the discrete GPU |
| `cargo xtask prepush` | check, test fast, and test gpu when the diff touches rendering |
| `cargo xtask check` | rustfmt, clippy `-D warnings`, crate layering |
| `cargo xtask spec-lint` | requirement IDs, tiers, coverage and the documentation size caps |
| `cargo xtask log` | the project log, newest first |
| `cargo xtask accept <milestone>` | milestone acceptance |
| `cargo xtask repro <bundle>` | replay a repro bundle |
| `cargo xtask golden-review` | classify golden candidates (ADR 0017); everything it escalates waits for the owner's `/bless` |
| `cargo xtask ci-stats` | CI job times against the budget |
| `cargo xtask perf` | performance harness on the reference machine; it measures readiness and refuses otherwise; runs only on the architect's go |

Failure output goes to `target/review/` (diff images, numeric deltas, crash reports).

## How the project is organised

```
crates/   core (coordinates, hashing, maths)   world-def   generators   cache   streaming
          frame (FramePlan, LOD selection)     render (only crate that touches wgpu)
          editor   app (the `planet` binary)   testkit   perf
xtask/    build, test, lint, acceptance tooling
openspec/ specs/ (current requirements per subsystem), changes/ (work in flight)
docs/     constitution.md, adr/, status.md, spikes.md, learnings.md, milestones/, briefings/, archive/, background/
tests/    goldens (small PNGs, per adapter)
spikes/   time-boxed experiments and their results
```

The renderer only executes a `FramePlan`; all decisions live in GPU-free crates. Positions are f64 on the CPU and tile-local or camera-relative f32 on the GPU.

## Where to read next

1. [docs/constitution.md](docs/constitution.md): the non-negotiable rules.
2. [docs/adr/](docs/adr/README.md): accepted and proposed architecture decisions.
3. [openspec/specs/](openspec/specs/): requirements per subsystem; each test cites its requirement ID.
4. [docs/status.md](docs/status.md): what waits for the owner, the current queue, tracked items.
5. [docs/milestones/](docs/milestones/): per-milestone acceptance reports.
6. [CLAUDE.md](CLAUDE.md): the agent operating manual (workflow, git rules, when to stop).
7. [docs/background/](docs/background/): frozen research and setup documents; read only when an ADR or spec points at a section.
