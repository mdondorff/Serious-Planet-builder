Opened: 2026-10-23
Closed: 2026-10-23
Question: Which components change together, from the history of main since M4: how wide are the PRs, which concepts cut across crates repeatedly, where does the churn sit, and does the built dependency graph follow CON-02?
Why it matters: the owner's architecture review before M8 (opus-planner, 2026-10-23); facts and patterns only, the judgement is the planner's.
Time box: about 2 h, read-only on code.
Success criteria: the five analyses of the brief as tables from `git` and `cargo metadata`.
Code location: none (throwaway scripts in `target/`, not kept); every number below is reproducible with the commands in "Method".

# Method
- **PRs:** the first-parent commits of `main` after the M4 acceptance commit `07f9c511` (2026-10-16) up to `10478bea` (#574, 2026-10-23): 382 squash-merged PRs or direct commits (PR numbers #192 to #574). Files per commit from `git log --first-parent --name-only 07f9c511..origin/main`.
- **Components:** a file under `crates/<name>/` is crate `<name>`; under `xtask/` is `xtask`; under `openspec/specs/<name>/` is spec `<name>`. A *code PR* touches at least one crate or `xtask` (202 of the 382; the other 180 are docs, specs, ADRs, status and logs). All files of a crate count (src, tests, examples, benches). Shaders are WGSL strings inside `crates/render/src` (no separate shader files), so they are part of `render`. Goldens (`tests/goldens`) and `.github` are not components.
- **Dependency graph:** `cargo metadata --no-deps` and `cargo tree -e normal`; `cargo xtask layering`.
- **Limits:** a PR is the unit; one PR with a mechanical edit across crates (a rename, a comment pass) counts like a feature. Stacked draft PRs that carry code are counted as they merged. "Lines" are current file lengths.

# 1. The PRs
382 PRs: 202 code PRs, 180 without code. Of the 202, the crates touched:

| Crates touched | Code PRs | Share |
|---|---|---|
| 1 | 77 | 38.1 % |
| 2 | 68 | 33.7 % |
| 3 | 29 | 14.4 % |
| 4 or more | 28 | 13.9 % |

Mean 2.16 crates per code PR, median 2, maximum 8. Code PRs touching no spec: 58 of 202 (29 %). Specs touched by code PRs: viewer 42, testing 40, streaming-cache 22, renderer 20, build-tooling 20, generator-pipeline 19, atmosphere-ocean 17, world-definition 13, look 9, terrain-lod 5, authored-layers 4.

PRs touching each crate (of 202 code PRs):

| Crate | PRs | Share | Lines in src | Lines in tests | Lines elsewhere (examples, bins) |
|---|---|---|---|---|---|
| app | 117 | 58 % | 16,636 | 3,681 | 1,792 |
| frame | 82 | 41 % | 17,520 | 16,377 | |
| xtask | 45 | 22 % | 5,267 | | |
| render | 40 | 20 % | 9,953 | 8,871 | 903 |
| generators | 32 | 16 % | 7,464 | 8,593 | |
| editor | 29 | 14 % | 5,726 | 4,811 | |
| cache | 22 | 11 % | 4,935 | 5,721 | |
| world-def | 22 | 11 % | 4,384 | 4,334 | |
| perf | 20 | 10 % | 9,847 | | |
| testkit | 14 | 7 % | 1,923 | 127 | |
| streaming | 11 | 5 % | 3,033 | 3,186 | |
| core | 2 | 1 % | 748 | 954 | |

All `.rs` files: 437 files, 146,786 lines (src only: 182 files, 87,436 lines).

# 2. Co-change matrix
Cell = code PRs touching both crates; the diagonal is the PRs touching the crate. Crates in order of PR count.

| | app | frame | xtask | render | generators | editor | cache | world-def | perf | testkit | streaming | core |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **app** | 117 | 60 | 17 | 24 | 23 | 19 | 15 | 9 | 17 | 1 | 9 | 1 |
| **frame** | 60 | 82 | 10 | 35 | 21 | 7 | 14 | 8 | 6 | 1 | 7 | 1 |
| **xtask** | 17 | 10 | 45 | 5 | 3 | 0 | 3 | 1 | 8 | 5 | 5 | 0 |
| **render** | 24 | 35 | 5 | 40 | 8 | 1 | 5 | 1 | 5 | 1 | 5 | 1 |
| **generators** | 23 | 21 | 3 | 8 | 32 | 7 | 15 | 11 | 2 | 1 | 4 | 1 |
| **editor** | 19 | 7 | 0 | 1 | 7 | 29 | 2 | 7 | 0 | 0 | 0 | 0 |
| **cache** | 15 | 14 | 3 | 5 | 15 | 2 | 22 | 9 | 3 | 1 | 4 | 1 |
| **world-def** | 9 | 8 | 1 | 1 | 11 | 7 | 9 | 22 | 0 | 0 | 0 | 1 |
| **perf** | 17 | 6 | 8 | 5 | 2 | 0 | 3 | 0 | 20 | 0 | 3 | 0 |
| **testkit** | 1 | 1 | 5 | 1 | 1 | 0 | 1 | 0 | 0 | 14 | 1 | 0 |
| **streaming** | 9 | 7 | 5 | 5 | 4 | 0 | 4 | 0 | 3 | 1 | 11 | 0 |
| **core** | 1 | 1 | 0 | 1 | 1 | 1 | 1 | 1 | 0 | 0 | 0 | 2 |

The strongest pairs (PRs together; Jaccard = together over either):

| Pair | PRs | Jaccard |
|---|---|---|
| app + frame | 60 | 0.43 |
| frame + render | 35 | 0.40 |
| cache + generators | 15 | 0.38 |
| generators + world-def | 11 | 0.26 |
| cache + world-def | 9 | 0.26 |
| frame + generators | 21 | 0.23 |
| app + render | 24 | 0.18 |
| app + generators | 23 | 0.18 |
| cache + frame | 14 | 0.16 |
| editor + world-def | 7 | 0.16 |
| app + editor | 19 | 0.15 |
| app + perf | 17 | 0.14 |
| perf + xtask | 8 | 0.14 |

- `app` is in 60 of the 82 `frame` PRs (73 %), in 24 of 40 `render` PRs (60 %), in 23 of 32 `generators` PRs (72 %), 19 of 29 `editor` PRs (66 %) and 15 of 22 `cache` PRs (68 %). It is touched by 58 % of all code PRs.
- `editor` changes together with `app` (19) and with `generators` (7), `world-def` (7), `frame` (7), and almost never with `render` (1) or `streaming` (0) although it depends on `render`.
- `streaming` and `core` change rarely (11 and 2 PRs); `streaming` is touched together with `app` 9 times and `frame` 7.
- Triples that occur most (any three crates in one PR): app+frame+render 23, app+frame+generators 18, app+cache+generators 14, cache+frame+generators 13, app+cache+frame 13, app+frame+xtask 9, frame+generators+world-def 7, app+generators+world-def 7, app+perf+xtask 7, app+generators+render 7, frame+generators+render 7.

# 3. The 15 PRs crossing the most crates
Crates counted with `xtask`; files = all files of the PR. Reasons are from the commit text and the files.

| PR | Crates | Files | What had to move across |
|---|---|---|---|
| #304 remove-vertex-render-path (draft OpenSpec change) | 8 (app, cache, frame, generators, perf, render, streaming, xtask) | 100 | Titled as a draft OpenSpec proposal, but the squash carries the code: the vertex tile format is removed (render 32 files, app 21, frame 15, streaming 7; two app examples deleted). One storage format decision threaded through the renderer, the pool, the tile sources and the app and perf harnesses that name it. |
| #303 render-floor-3 | 7 (app, cache, frame, perf, render, streaming, xtask) | 91 | A new rule (the render floor draws from level 3, roots load before frame 0) in the plan, the streaming start, the cache pre-fill and the perf and accept harnesses. |
| #302 compact-slot-v2 (draft OpenSpec change) | 7 (app, cache, frame, generators, perf, render, streaming) | 71 | The compact tile slot layout: generator output, cache record, pool slot size, render vertex stage, perf harness. |
| #466 lattice-normals (ADR 0073) | 6 (app, cache, core, frame, generators, render) | 48 | A field-definition change (the canonical normal is the lattice normal, MESH_VERSION 9): generator, mesh key in the cache, tile hash, frame plan inputs, shader, app test pins. |
| #373 stamp-stage 3/3 | 6 (app, cache, editor, frame, generators, world-def) | 27 | Edits as a height stage: item revision in the plan cache key, bounds by delta range, item file map for the editor. |
| #331 test-binaries-1 | 6 (app, cache, generators, streaming, testkit, xtask) | 21 | A build-structure change: one test binary per tier in four crates plus the runner (BUILD-015). |
| #375 live-definition | 6 (app, cache, editor, frame, generators, world-def) | 15 | The window's terrain reads a replaceable definition and stacks stamp stages: definition handle, stage chain, cache, editor session, frame. |
| #204 sun and base shading (M5 item 4) | 5 (app, cache, frame, generators, render) | 72 | A new field channel (climate) threaded generators, cache record, frame look and plan, render shader and view, app wiring; MESH_VERSION 7. |
| #215 ocean-bake-and-water-level | 5 (app, cache, frame, generators, world-def) | 50 | A new bake channel and a definition table (ocean connectivity, liquids): GLOBAL_VERSION 4, schema 2, generator, store, frame, app. |
| #230 compact-storage-buffers | 5 (app, frame, generators, render, streaming) | 36 | The compact tile format moved from an atlas to storage-buffer blocks: pool, render, tile sink, generator tile type, app. |
| #192 housekeeping, mechanical pass | 5 (app, frame, render, streaming, xtask) | 31 | Not a concept: a mechanical edit of references and stale phrases across crates, plus a BUILD-008 check. |
| #241 m5-perf-scenes | 5 (app, frame, perf, render, xtask) | 25 | Per-pass GPU timestamps through the renderer, frame plan counters, the perf harness and the runner. |
| #366 stamp-stage 2/3 | 5 (app, cache, frame, generators, world-def) | 18 | The stamp stage and its key chain: generator stage, cache key chain, frame plan key, definition items, app. |
| #222 water-pass (M5 item 7) | 4 (app, frame, generators, render) | 56 | Water as a pass on the terrain grid: a plan part in frame, a pass and shader in render, tile water in the generator job, app view. |
| #275 shore ramp follows the mesh's tolerance | 4 (app, frame, perf, render) | 54 | A shading width that follows the LOD tolerance: plan value in frame, per-vertex width in the shader, perf scene and app. |

Ranks 1 to 13 are all the PRs of five or more crates (1 of 8, 2 of 7, 4 of 6, 6 of 5); ranks 14 and 15 are the two 4-crate PRs with the most files (15 PRs touch exactly 4 crates: 28 - 13). Two of the top three (#302, #304) are titled as OpenSpec drafts but carry the code.

# 4. Recurring cross-cutting concepts and hotspots

## Recurring sets
Identical crate sets of three or more crates in two or more PRs:

| PRs | Crate set | Kind of change (examples) |
|---|---|---|
| 10 | app, frame, render | A new view, overlay or toggle key: scale-refs (F7, #552), ground-gloss (F6), lod-capping-ui, stale-tile-view (F3), wireframe overlay, brush preview |
| 5 | app, frame, xtask | A new repro or replay path: path-replay (VIEW-026) and the window scenes with perf harness hooks |
| 4 | app, cache, frame, generators | A field or stage that changes what a tile is: wet-water (ADR 0071), stage-pass-through, coarse water level, stamp key chain |
| 4 | app, perf, xtask | Perf harness changes: preflight, m5 harness, accept-m5 |
| 2 | app, cache, editor, frame, generators, world-def | Authoring as a stage: live-definition, stamp-stage 3/3 |
| 2 | app, cache, frame, generators, world-def | Stamp-stage 2/3 and the ocean channel (a new definition input with a key) |
| 2 | app, editor, frame | Editor panels over plan data: brush-slope-guard, inspector of lines |
| 2 | app, frame, generators | Plan cost and exact water cut |
| 2 | editor, frame, generators, world-def | New item kinds: brush items, line items |

By count of PRs, three concepts recur:
1. **A new visual feature with a debug view** (CON-18: a feature ships with a debug view): app + frame + render in 23 PRs (the triple), plus the app-side key and help text.
2. **A new field channel, stage or key input** threaded through generators, cache, frame and app (and world-def when it is a definition node): generators + cache + frame in 13 PRs, with a version bump in 8 PR subjects (MESH_VERSION, GLOBAL_VERSION).
3. **A new perf scene or harness check**: app + perf + xtask (7), 44 PR subjects name perf, a scene or a harness.

## Churn: files changed most often since M4 (commits touching a file; current lines)

| Commits | Lines | File |
|---|---|---|
| 47 | 2,041 | crates/app/src/window.rs |
| 29 | 1,622 | crates/app/src/lib.rs |
| 27 | 1,588 | xtask/src/testcmd.rs |
| 26 | 2,757 | crates/render/src/terrain.rs |
| 26 | 1,995 | crates/app/src/editwin.rs |
| 21 | 1,672 | crates/app/src/skeleton.rs |
| 20 | 1,747 | crates/frame/src/viewer.rs |
| 20 | 1,065 | crates/frame/src/pipeline.rs |
| 19 | 888 | crates/app/src/litperf.rs |
| 19 | 1,267 | crates/app/src/streamperf.rs |
| 18 | 1,285 | crates/world-def/src/lib.rs |
| 18 | 493 | crates/app/tests/gpu_window.rs |
| 17 | 1,316 | crates/editor/src/session.rs |
| 16 | 112 | crates/frame/tests/it.rs (the test index) |
| 16 | 2,093 | crates/frame/src/lod.rs |
| 15 | 766 | crates/generators/src/stamp.rs |
| 15 | 843 | crates/app/src/worldfield.rs |
| 15 | 1,762 | crates/render/src/atmosphere.rs |
| 15 | 627 | crates/frame/src/terrain.rs |
| 15 | 52 | crates/frame/src/lib.rs |
| 14 | 87 | xtask/src/main.rs |
| 14 | 1,180 | crates/app/src/editperf.rs |

Of the 30 most-changed files, 10 are in `app` (window.rs alone changed in 47 of 382 PRs, 12 %). Index files (`frame/tests/it.rs`, `frame/src/lib.rs`, `xtask/src/main.rs`) change with every addition.

## Largest source files now (lines, churn since M4)

| Lines | File | Commits |
|---|---|---|
| 2,757 | crates/render/src/terrain.rs (contains the WGSL shaders as strings) | 26 |
| 2,093 | crates/frame/src/lod.rs | 16 |
| 2,059 | crates/cache/src/global_store.rs | 5 |
| 2,041 | crates/app/src/window.rs | 47 |
| 1,995 | crates/app/src/editwin.rs | 26 |
| 1,771 | crates/frame/tests/managed.rs | 10 |
| 1,762 | crates/render/src/atmosphere.rs | 15 |
| 1,747 | crates/frame/src/viewer.rs | 20 |
| 1,672 | crates/app/src/skeleton.rs | 21 |
| 1,622 | crates/app/src/lib.rs | 29 |
| 1,588 | xtask/src/testcmd.rs | 27 |
| 1,587 | crates/streaming/src/lib.rs | 8 |
| 1,563 | crates/app/src/panels.rs | 12 |
| 1,551 | crates/frame/src/look/atmosphere.rs | 5 |
| 1,341 | crates/perf/src/accept.rs | 7 |

The files that are both large and changed often (over 1,500 lines and over 20 commits): app/window.rs, render/terrain.rs, app/editwin.rs, app/skeleton.rs, xtask/testcmd.rs, app/lib.rs.

# 5. The dependency graph as built, against CON-02
CON-02: `core → world-def → generators → cache → streaming → frame → render → editor → app`, dependencies point downward only; `core`, `world-def`, `generators`, `cache`, `streaming` and `frame` compile without a graphics crate. `cargo xtask layering`: "layering ok: 9 crates, 6 GPU-free". `cargo tree -e normal` finds no wgpu, winit, egui or naga under those six; `planet-editor` and above carry wgpu and egui.

Workspace crates: the nine of the chain, plus `planet-testkit`, `planet-perf` and `xtask` (not in the chain). Normal dependencies between workspace crates:

| Crate | Depends on (normal) | Dev-dependencies on workspace crates |
|---|---|---|
| core | none | |
| world-def | core | |
| generators | core, world-def | generators (itself) |
| cache | core, generators, world-def | |
| streaming | cache, core | |
| frame | core, generators, streaming, world-def | cache, generators |
| render | core, frame, generators, streaming | generators, render (itself), testkit, world-def |
| editor | core, frame, render, world-def | |
| app (`planet`) | cache, core, editor, frame, generators, perf, render, streaming, world-def | render, testkit |
| testkit | none | |
| perf | none | |
| xtask | perf | |

- **Upward leaks:** none. Every normal dependency points to a lower rank of the chain. Dev-dependencies: `frame` has `cache` and `generators` (both lower); no dev-dependency points upward. Two self dev-dependencies (`generators`, `render`) exist for test features.
- **Skips (a crate depends on a non-adjacent lower crate):** `generators` is a direct dependency of `cache` (adjacent), `frame`, `render` and `app`; `frame` does not depend on `cache` in normal builds (the cache sits between generators and streaming in the chain, and frame reaches it only in tests); `render` depends directly on `generators` and `streaming` (below `frame`); `editor` depends directly on `core` and `world-def` and not on `generators`, `cache` or `streaming`; `app` depends on every crate of the chain and on `perf`.
- **Off the chain:** `perf` has no workspace dependencies (the `app` crate and `xtask` depend on it); `testkit` has none and is a dev-dependency of `render` and `app`. `xtask` depends only on `perf` (it runs the other crates through `cargo`).
- **Coupling the graph does not show:** `app` is touched by 58 % of code PRs and depends on all nine crates; `editor` depends on `render` but changes with it once in 40 PRs.

# Not done and limits
- Co-change by PR, not by commit within a PR (squash merges hide the commits); a PR counts per crate once.
- No weighting by lines changed per crate; the wide PRs above are bulk by file count.
- Churn counts commits touching a file since M4, including test files; renames are not followed.
- Reasons in section 3 are from the commit texts and file lists, not from reading each diff.
- Method commands: `git log --first-parent --name-only --format='@@%h|%ad|%s' 07f9c511..origin/main`, `cargo metadata --no-deps --format-version 1`, `cargo tree -p <crate> -e normal`, `cargo xtask layering`.
