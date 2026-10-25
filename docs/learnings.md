# Learnings

Added with `/learning`. Each entry names its scope, the assumption it changes, the follow-up and when it becomes obsolete. A learning is promoted (a spec line, a `CLAUDE.md` line or a test that cites it), kept while still active and surprising, or archived with its milestone (ADR 0075). Cap: 20 KB. Archives: `docs/archive/M0-M3/learnings.md`, `docs/archive/M4/learnings.md`, `docs/archive/M6/learnings.md` (M5 and M6, and the full text of the entries below), `docs/archive/M7/learnings.md`.

## 2026-10-10 nextest rejects a filterset that matches no binary
Scope: project (ADR 0009)
- `-E 'not binary(~gpu_) and not binary(~oracle_)'` fails when no `oracle_*` binary exists.
- Tier selection by binary-name prefix needs at least one binary per tier; `crates/render/tests/oracle_shader_validation.rs` is the standing tier-B binary.

## 2026-10-11 Harness queries must not depend on the Windows UI language
Scope: project (perf harness)
- `Get-Counter '\Processor(_Total)\% Processor Time'` fails on Rasierklinge's de-DE Windows; the CIM class `Win32_PerfFormattedData_PerfOS_Processor` works.
- Check every platform query on Rasierklinge's locale; an unreadable value refuses loudly.

## 2026-10-11 An f32 remainder in metres added to a local offset rounds to centimetres
Scope: project (CON-11, overlays and shaders)
- A world-anchored grid added a 1,000 km remainder to an f32 tile-local offset: f32 near 5e5 steps by 3 cm, so lines became 40 px plateaus.
- Carry f64 fractional phases per period (cells, not metres); fade a decade out when its resolution exceeds the pixel step.
- Bisect a visual defect by switching components off before fixing; the first plausible diagnosis was wrong.

## 2026-10-12 A threshold that decides a stored flag and a derived value must use the stored number
Scope: project (multi-channel fields)
- Land was decided from the stored f32 continentalness, elevation from the f64: values within 1e-9 of sea level disagreed (a "land" sample at -200 m).
- Round to the stored precision before any decision two outputs must agree on; apply to every field that gains thresholds.

## 2026-10-15 GPU milliseconds depend on how long the GPU has been loaded
Scope: project (perf comparisons)
- The same renderer measured 2.85 to 6.0 ms at ground-1m across 14 processes, by GPU clock state; same-process ratios were stable.
- Compare in the same process (A/B), or record the GPU clock per run.

## 2026-10-16 WARP crashes in its own code with several devices in one process
Scope: project (tests on the software adapter)
- About 1 in 3 plain `cargo test` runs with 5 WARP devices end in an access violation inside `d3d10warp.dll`; never under nextest (one process per test) or on the RTX. Details: `spikes/warp-crash/results/results.md`.
- Keep nextest isolation; plain `cargo test` needs `--test-threads=2` or less; share one device per test binary.
- Obsolete when: a Windows WARP passes `spikes/warp-crash` level 7 with 6 threads in 100 runs.

## 2026-10-16 Reviewing goldens needs CI's WARP, and nextest overwrites its JUnit report
Scope: project (golden-review)
- The local WARP (10.0.26100.9711) differs from CI's (.33438): WARP candidates come from CI's `review-Windows` artifact.
- Each `cargo xtask test*` run overwrites `target/nextest/default/junit.xml`; copy it aside before the next run when `golden-review --tests` needs it.
- Obsolete when: the local and CI WARP agree.

## 2026-10-16 A new trait method with a default body is silently wrong in every wrapper
Scope: project (`HeightField` and its wrappers)
- The climate methods' defaults were not overridden by `ErosionField` or delegated by `CancelGuarded`; tests on `Reference` (whose default is right) saw nothing.
- Every new `HeightField` method (for example `bounds_monotone`, LOD-012) needs a test that every field over a bake and every wrapper answers it as the inner field does.
- Obsolete when: `HeightField` has no default-bodied methods that depend on the field's inputs.

## 2026-10-19 Shortcut letters are logical keys, movement keys are physical keys
Scope: project (editor input)
- On QWERTZ the key labelled Z is `KeyCode::KeyY`: undo and redo would swap.
- New shortcut letters go through `edit_key` (logical first, physical fallback) with a QWERTZ test; WASD and the number row stay physical.
- Obsolete when: bindings come from a settings file with a layout-aware default.

## 2026-10-19 A provisional hand table of another stage's keys drifts
Scope: project (editor model, stamp stage)
- The editor's guessed parameter table (`radius_m`, `rim_m`) disagreed with the stamp stage's spelling (`reach_m`); outlines were off by a quarter.
- A new stamp kind goes into the stage and the editor's table together; a tier-A test asserts `StampKind::param_key`.

## 2026-10-19 A software-adapter window smoke costs its pixels
Scope: project (window smokes on WARP and lavapipe)
- The editor smoke took 67 s alone, up to 220 s under load (limit 300 s): 0.6 s per lit frame at 1280x720, 14 s start-up; CPU work under 1 s.
- A window smoke passes `--size` (for example 480x270) unless it checks pixels.

## 2026-10-20 The lattice normal is not exactly radial on a sphere (ADR 0073)
Scope: generators::normals, core::tile
- Centred lattice differences are radial only to about 0.003 degrees (uneven spacing from the tangent warp); a flat-sphere test needs a 0.01 degree bound.
- Across a cube edge the ring point comes from the exact integer cube point, not from re-projecting the extended plane (off by up to a cell at the corner).

## 2026-10-20 A bound checked at texel centres and one sun hid 10 to 23 % errors of the aerial-perspective volume
Scope: atmosphere (ATMO-007, ATMO-021)
- Between texels the volume was 10 to 23 % off (14 to 99.5 km), 17 to 18 % at 2 km with a low sun.
- Sweep the sun high, low and below the horizon, towards and away from it, and read a table between its texels.
- Open: the volume below 8 km at a low sun is still 17 to 18 % off (half-resolution haze buffer or a better parameterisation).

## 2026-10-20 Stamp bounds: the range of the running height, not the sum of each item's maximum
Scope: generators::stamp (GEN-026, GEN-030)
- Summing declared maxima gave a fine tile a 10 km extent under a 5 km plateau and the plan refused it; a smooth's per-item counter re-sharpened overlapping strokes.
- Bound a stage by the running height carried through the items in order; test rounding over a base with exact bounds; a per-point count belongs to the layer.
- A new stamp kind needs a monotone profile or a bound rule of its own.

## 2026-10-22 A plan after an edit is hashing thousands of coarse tiles, not bounds queries
Scope: project (streaming cache, LOD, authored items; STRM-027, STRM-034)
- SPIKE-29 assumed the post-edit plan cost was the stamp stage's bounds queries; a timer showed the selection walk re-hashing the reads of every tile it visits when the item revision moves, and a coarse tile's cover is about 20,000 summary nodes (about 10 ms each); the per-tile cost sums over threads, so wall time needs a stage timer, not a call counter.
- An edit announced with where the items changed lets the cache keep every tile the change cannot reach (`SelectionCache::note_item_change`); the content keys stay the guarantee for anything not announced. Test the reach rule against the real tile hashes, on small bodies too: the touch test's relative margin is a millionth of `reach / R`.
- Obsolete when the read set of a coarse tile stops listing its cover nodes one by one.

## 2026-10-25 The chunk stage's 300 m depth is a clamp of a runaway: its corridor anchors lie far under the base ground
Scope: generators::anet, app::anetworld (GEN-047, GEN-051; look review 1, `docs/milestones/M8/look-review-1-hills/`)
- The unclamped solve of the chunk under the steepest-site channel cuts to -2659 m (16,125 of 369,664 core cells deeper than 250 m, 15,318 at the 300 m clamp); the second site cuts to -1322 m. Every cut channel is the same depth because `ANet::compute` clamps to `bound_m`, with vertical walls, which ADR 0074 caps at coarse tiles (121 to 129 of about 1750 plan nodes) and draws as spikes.
- The corridor beds are anchored at the lower of the coupled run's height and the base; the run's heights lie 1.2 to 2.7 km under the base at those channels (anchor minus base, median -2596 m and -1221 m).
- The coupled run compresses relief against the base: its height minus the base's at cell centres, median over river cells, is +111 m at 0 to 500 m ground, -164 m at 500 to 1000, -933 m at 1000 to 2500, -2686 m at 2500 to 5000, -4649 m over 5000. Anchors above the base are ignored (the minimum takes the base), so only the low band cuts mildly.
- Tools: `chunk_incision_report` and `run_against_base_report` (`cargo run --release -p planet --example chunk_incision -- --store DIR --lat DEG --lon DEG | --run-vs-base`), `capped_nodes` in `stills.txt`.
- Obsolete when the chunk stage's anchors are made consistent with the base field.

## 2026-10-25 The horizon bake is cheaper than the spike measured; the shadow engine has three declared limits
Scope: M8-5 (horizon shadows), `generators::horizon`, `coarsehorizon`, the lit shader
- The bake costs 21 us per texel (16 azimuths, 5 km, 4 m texels, release, one thread): about 5 s on 6 threads for a 24 km2 chunk, against SPIKE-32's 0.7 s per km2 (17 s). The coarse horizon at 512 observer cells per face edge over the 2048-cell run takes 7.7 s on 8 threads.
- Limits stated in the specs (GEN-044, GEN-045, REND-038): the tangent warp makes a texel up to 1.41 times as long along `i` as along `j` near a face edge, so the bake's distances (and the horizon angle) are off by up to that factor there; the coarse azimuths use each cell's own geographic frame, which differs from the fragment's near a pole; the shader reads 16 azimuths exactly.
- On the look-review stills the shadow edge along canyon rims is scalloped (16 azimuths fan near an occluder); the far-field outline at sunset could not be judged on flat sites.
- Obsolete when the bake marches in metres, the coarse map gets a pole-free frame and the azimuth count is decided from stills.


## 2026-10-25 The chunk stage's walls exceed the angle of repose because the hillslope steps skip every pair with a channel cell
Scope: generators::anet (GEN-051), app::stills, app::anetworld; look review 1 second site after R-a
- At the steepest site the 131 capped plan nodes (30 m pose) are all level 16 (153 m): 10 are over the drawable slope 3.42 on the 8 m heights, 12 are between 0.9 and 3.42, 109 are flat (median steepest slope 0.10) and held by ADR 0074's ceiling next to a steep tile.
- The base alone has no neighbour pair over 0.12. In the solved chunks 6,010 of 1.48 million edge pairs (site 1) and 20,612 (site 2) are over 0.9, the steepest 5.72 and 9.39.
- 42 % (site 1) and 22 % (site 2) of the steep pairs have a protected cell (channel, corridor, anchor); `slump_deltas` and `diffuse` skip those pairs, so the bank-to-bed step is never relaxed.
- A slump rate of 0.5 (from 0.09) and 90 steps (from 30) cut the pairs of two unprotected cells from 3,494 to about 820 and leave the protected ones at 2,200 to 2,700: rate-limited and structural, in that order.
- Tools: `capped.txt` of the stills tool (first pose), the pair census of `chunk_incision_report` with `PLANET_SLUMP_RATE` and `PLANET_STEPS` overrides (diagnostic only).
- Obsolete when the hillslope steps relax the bank next to a channel cell.
