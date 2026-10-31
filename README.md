# Serious Planet builder - Seriös Planet Bauer

A planet-scale generator, editor and real-time renderer, written in Rust with wgpu. One binary (`planet`) with three run modes; development is spec-first with OpenSpec and driven largely by AI agents under human review.

## Status

M0 to M8 are done; M8 (Natural landscape) was accepted on 2026-10-30 ([report](docs/milestones/M8-report.md)). M8 shipped less than planned: the chunk stage (fine erosion and rivers per 5 km chunk) was removed from the code (B6, ADR 0086); realistic hills and rivers move to M9. The week from 2026-11-02 runs Option B: automated owner checks, one job system, test robustness and M9 groundwork ([week plan](docs/briefings/week-2026-11-02.md)); then the new shadows (ADR 0085) and M9's terrain steps. Current work, open owner decisions and the queue are in [docs/status.md](docs/status.md).

What works today:
- A whole planet from a global bake (16384 cells per cube face, about 0.6 km) with erosion, climate (temperature, precipitation) and an ocean connected through low ground.
- A global coupled run (erosion and drainage at 2048 cells per face, about 20 s from the bake), stored beside the bake; K shows its heights and its river network.
- Real-time rendering from orbit down to a crouched eye at 0.5 m: height-aware CDLOD with morphing and streaming, a Hillaire atmosphere, HDR with adaptive exposure, Gerstner-wave water with shorelines, horizon-map terrain shadows with a sun you can move ([ and ]), procedural ground detail (F6), a look file for the ground (F8), and scale references of known size (F7).
- An interactive editor: stamps (crater, hill, plateau), lines (ridge, channel), regions, a brush (raise, lower, smooth, flatten), handles, an inspector, layers, undo and redo, a globe overview, and a Problems tab for edits the engine cannot draw fully or that stack too heavily (Fly to, Reduce, Merge strokes, Delete).
- `cargo xtask store` builds the production store once and points the editor at it; `--relief-scale S` (1 to 4) builds an exaggerated world into its own store.
- A headless harness with golden images, repro bundles (also replayed from a stored bake), the no-degradation gate on the real store, and a performance harness with a readiness gate.

Not there yet: realistic hills and river valleys (M9), driver maps (soil, moisture) with the plants and rocks they place, instancing, the generator editor, cities.

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
| W A S D, Q / E | free camera: move; down / up (down to a crouch at 0.5 m above the ground) |
| arrows | free camera: look |
| Shift | boost the movement |
| click in the world (viewer), right mouse held (editor) | capture the mouse to look around; Esc releases it |
| C | flight or free camera (does nothing while the editor is on) |
| P, Space | pause |
| R | restart the flight |
| G | globe overview: drag turns, wheel zooms, a click on a marker or the surface flies there and lands |
| L | lit path (atmosphere) or debug path |
| V | next debug view (debug path) |
| J | shade view: normal, horizon, shadow, soil, moisture, insolation (lit path; horizon and shadow need a store with a coupled run) |
| [ / ] | the sun half an hour earlier / later in local solar time at the camera (the title shows the time) |
| `\` | back to the look's sun |
| H | editing light: a sun fixed over your shoulder, so no slope is black (off by default) |
| K | what the climate shows: the climate, the coupled run's heights, its drainage area (the global river network; lit path) |
| F3 | overlay: off, tiles, mesh, regions, stale tiles (lit path) |
| F4 | coarse LOD (error bound x4) |
| F6 | ground detail: natural, full, off |
| F7 | scale references: a chair 0.9 m, a house 8 m, a boat 10 m, a tower 291 m and a block of boxes 1 km across, dropped in front of you |
| F8 | ground look: default, or the look file's ground table |
| F9 | write the camera path of the last minute and a repro bundle to `target/review/` |
| T | vsync on / off |
| Esc | quit (viewer); step back (editor mode) |

### Editor mode (`--edit`)
| Key | What it does |
|---|---|
| Tab | editor on / off (the panels and the outlines of the edits; the pointer is free) |
| 1 | select |
| 2, 3, 4 | place a crater, hill, plateau (then click the planet) |
| 5, 6 | draw a ridge, a channel (click points, Enter finishes) |
| 7 | draw a region (click points, Enter finishes) |
| 8, 9, 0, - | brush: raise, lower, smooth, flatten (drag to paint a stroke; the release puts it, one undo takes it back) |
| mouse wheel, Shift + wheel | brush radius, brush strength (both also in the inspector) |
| click | add a point (line and region tools), else select the edit under the cursor (click again at the same spot for the next one under it, smallest first) |
| drag a handle | circle: move; square: radius; light circle: height; amber dots: the points of a line or region |
| Shift + click, Ctrl + click | add a point on the outline of the selected line or region, remove the point under the cursor |
| Enter | finish the line or region being drawn (in the inspector: keep the typed value) |
| Up / Down (inspector) | step the value (Shift: ten times) |
| Backspace | take the last point back while drawing |
| double-click in a list | fly to the edit |
| Delete | delete the selected edit |
| Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z | undo, redo (the keys labelled Z and Y, whatever the layout) |
| Esc | cancel the drag, the draft, the tool, the selection (does not quit) |
| right mouse held, W A S D Q E | look around and fly |
| F1, ? | list the keys |

The Problems tab lists edits that are too steep to draw fully or heavily stacked, with Fly to, Reduce, Merge strokes and Delete.

## Building, testing, checking

Everything goes through `cargo xtask`:

| Command | What it does |
|---|---|
| `cargo xtask test fast` | tier A (unit/property) for changed crates and the crates above them |
| `cargo xtask test` | tiers A + B on the software adapter |
| `cargo xtask test gpu [--real]` | tier C goldens and debug views, on the software adapter or the discrete GPU |
| `cargo xtask prepush` | before every push: check, test fast, and test gpu when the diff touches a golden-producing path |
| `cargo xtask check` | rustfmt, clippy `-D warnings`, crate layering |
| `cargo xtask layering` | crate layering and the GPU-free dependency check |
| `cargo xtask spec-lint` | requirement IDs, tiers, coverage, retired terms and the documentation size caps |
| `cargo xtask log [--since D] [--limit N]` | the project log, newest first |
| `cargo xtask accept <milestone>` | milestone acceptance |
| `cargo xtask gate [--small] [--site frame-N] [--store DIR]` | the no-degradation gate: the editor window over the real store, a scripted hill and the owner test's camera paths (`--small`: a generated world on the software adapter) |
| `cargo xtask repro <bundle> [--real] [--store DIR]` | replay a repro bundle or an F9 path, also from a stored bake |
| `cargo xtask store [--res N] [--relief-scale S] [--link DIR]` | build the production store (default 16384 cells) and point the editor at it; `--relief-scale` makes an exaggerated world in its own store |
| `cargo xtask golden-review` | classify golden candidates (ADR 0017); everything it escalates waits for the owner's `/bless` |
| `cargo xtask bless` | list candidate goldens; the owner's `/bless` applies what the owner approved |
| `cargo xtask cache-verify <dir>`, `cache-compact <dir>` | check or compact a tile store |
| `cargo xtask det-diff <a> <b>` | compare two determinism manifests (Windows against Linux in CI) |
| `cargo xtask capture flights` | the coastal flights as PNG sequences for the owner's look review |
| `cargo xtask ci-stats` | CI job times against the budget |
| `cargo xtask heavy <command>` | run a command under the machine-wide heavy-run lock |
| `cargo xtask quiet on\|off\|status` | the owner's flag: while it is up every heavy command waits |
| `cargo xtask perf [stream\|lit\|preflight\|envelope\|coupled\|global-bake]` | performance harness on the reference machine; it measures readiness and refuses otherwise; runs only on the architect's go |

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

[docs/README.md](docs/README.md) maps every folder: where to find what.

1. [docs/constitution.md](docs/constitution.md): the non-negotiable rules.
2. [docs/adr/](docs/adr/README.md): accepted and proposed architecture decisions.
3. [openspec/specs/](openspec/specs/): requirements per subsystem; each test cites its requirement ID.
4. [docs/status.md](docs/status.md): what waits for the owner, the current queue, tracked items.
5. [docs/milestones/](docs/milestones/): per-milestone acceptance reports.
6. [CLAUDE.md](CLAUDE.md): the agent operating manual (workflow, git rules, when to stop).
7. [docs/background/](docs/background/): frozen research and setup documents; read only when an ADR or spec points at a section.
