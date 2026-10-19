# Perf window 13: plan and report tables

Prepared, not run. The window needs opus-planner's authorization (the owner done with hands-on testing and the laptop on mains power). Runner: sonnet-explorer from the native UI session. Report: `docs/perf/window-13.md`.

## Scope

| Part | What | Source of truth |
|---|---|---|
| A | Window-12 repeat: the removal of the vertex render path against window 11, with a valid ADR 0065 calibration | `docs/perf/window-12.md` (open items) |
| B | E1 edit-to-visible: the exact tiles after a stamp edit, ≤ 300 ms up to 150 m, ≤ 1 s up to 500 m and at 1 m altitude | SPIKE-13, M6 plan row E1 |
| C | E3 preview: the stamp visible on the stale tiles in ≤ 16 ms | SPIKE-13, REND-033 |

B and C are one scene, `edit-latency`.

## Before the window

- Main at the head of the window; `cargo build --release` of that tree; the 16384 store of the earlier windows (`target/accept/global16384` or the path in `docs/perf/window-12.md`).
- Owner's go through opus-planner; mains power confirmed; no other work on the machine (no builds, no `test gpu`); `cargo xtask perf` readiness gate decides (`--machine-ready` is the owner's only).
- Record in the report: top-5 CPU processes at the start, the power plan, the GPU clock and temperature at the start of each part.
- Smoke runs first (no measurement, may run before the window): `cargo xtask perf stream edit-latency --smoke` and `cargo xtask perf m5 --smoke`.

## Commands

```bash
# A: the window-12 repeat (the same command as window 12)
cargo xtask perf m5 --store <16384 store> --out target/perf-w13

# B and C: edit to visible and edit to preview
cargo xtask perf stream edit-latency --store <16384 store> --out target/perf-w13
```

`edit-latency` runs the pipelined managed loop on the real renderer, 2560x1440, over the stored bake, the editor's world (`WorldTerrain::with_edit_layers`: the stage of the stamp layer exists from the start, so the first stamp re-keys only the tiles it touches). Per altitude (1 m, 200 m, 2 km) one rig is settled; per stamp radius (50, 150, 500 m: a hill of height min(0.2 r, 100) m) an edit is made three times: `set_definition`, `ManagedLoop::edit(found tiles)`, `set_preview(preview_delta)`; the clock runs from the call of `set_definition` until the GPU has finished the frame that first shows the preview (preview) and the first frame that is exact and idle (exact); the edit is then taken back and the loop settled again. The median of three is reported. The table is printed and written to `<out>/edit-latency.md`.

## Report tables

### A. Lit views and streaming, window 11 → 12 → 13 (ms median)

| View | terrain | atmosphere | water | frame median / p99 | clock w11 → w12 → w13 (MHz) |
|---|---|---|---|---|---|
| ground-1m | | | | | |
| aerial-20km | | | | | |
| coast-2km | | | | | |
| coast-3m | | | | | |
| orbit | | | | | |

| Scene | median | p99 | max | descent lag mean / worst | session verdict (calibration ms) |
|---|---|---|---|---|---|
| dive | | | | | |
| dive-fast | | | | | |
| dive-cold | | | | | |
| teleport-1m | | | | | |
| teleport-2km | | | | | |
| look-1m | | | | | |
| look-2km | | | | | |

Open from window 12 to settle: orbit terrain 0.053 → 0.145 ms and orbit frame +21 % (floor-3 start or the reference-free draw path?), aerial-20km water 0.108 → 0.047 ms, the dive max 85 ms, the descent lag worst frame 3.13 over the budget 3.

### B and C. Edit to visible (`edit-latency`)

| altitude | stamp radius | preview ms (budget 16) | exact ms (budget) | tiles refreshed / resident | previewed nodes (max) | repeats | verdict |
|---|---|---|---|---|---|---|---|
| 1 m | 50 m | | | | | 3 | |
| 1 m | 150 m | | | | | 3 | |
| 1 m | 500 m | | | | | 3 | |
| 200 m | 50 m | | | | | 3 | |
| 200 m | 150 m | | | | | 3 | |
| 200 m | 500 m | | | | | 3 | |
| 2 km | 50 m | | | | | 3 | |
| 2 km | 150 m | | | | | 3 | |
| 2 km | 500 m | | | | | 3 | |

Budgets: preview 16 ms; exact 300 ms for a radius up to 150 m (above 1 m altitude), 1000 ms for 500 m and at 1 m altitude. The first row of a rig also carries the cold settle time.

### Machine and session

| | start | part A | part B/C |
|---|---|---|---|
| top-5 CPU processes | | | |
| power plan | | | |
| GPU clock / temperature | | | |
| readiness gate | | | |
| host load (CPU %) | | | |

## What can go wrong

- A rig that does not settle (900 s limit) or an edit that does not become exact (600 s limit) is an error that names the frame state (kind, substituted nodes, jobs, pending): read it before re-running.
- The first edit of a world without the edit-layer stage re-keys every tile (STRM-028): the scene uses the editor's world; a run over a plain world would measure a full reload, not an edit.
- A pre-filled pack of the plain world does not serve the editing world (other keys): the first descent over the store generates the coarse tiles. The scene settles before the first edit, so this is in the settle time, not the edit.
- The preview and exact times include the GPU wait of the frame that shows them (the loop waits for it); the pacing sleeps of 1 ms between frames are in them.
