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
- About 1 in 3 plain `cargo test` runs with 5 WARP devices end in an access violation inside `d3d10warp.dll`; rare under nextest (one process per test): 2 of 177 aborted in `test gpu` on 2026-10-30, after the Windows update; never on the RTX. Details: `spikes/warp-crash/results/results.md`.
- Keep nextest isolation; plain `cargo test` needs `--test-threads=2` or less; share one device per test binary.
- 2026-10-30 (explorer): the local rate is 2 aborts in 3 full `test gpu` runs (about 1 test in 90; the aborted test differs per run: `gpu_skeleton::the_draw_flag...` at 99 s, `gpu_world_field::a_world_bundle_replays...` at 56 s) and 0 in a fourth. Targeted stress of both crashing test groups (12 tests, nextest, one process each) at `-j 1`, 8 and 20: 132 runs, 0 aborts: not reproduced outside the full mix. `cargo xtask test*` names such an abort as `WARP crash (d3d10warp), CI judges`; the step still fails.
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

## 2026-10-25 The coupled run's steady state does not hold on the real bake (mechanism not found)
Scope: generators::coupled (GEN-040, GEN-063), app::coupledworld; M8 rebuild at the 16384 bake, run at 2048 cells
- The stationary uplift `rho K A^m S` computed from the input holds it at 0.00 m median drift over 20 steps on the smooth synthetic test world, and not on the reduced real bake: run minus input after N steps, median by input band (0-500, 500-1000, 1000-2500, 2500-5000, over 5000 m): N=2 about 0 (p10 to p90 up to -80 to +180 m); N=8 -50, -208, -445, -899, -1010; N=24 -99, -640, -1088, -2432, -4056; N=80 -104, -675, -1186, -2630, -4471 (saturating near step 24).
- Step 1 alone: no land cell goes down by more than 1 m, 4.24 million of 7.37 million land cells go up by more than 1 m (the flood's fill raises pit and lake cells and their donors follow; the other effects are not separated).
- Not found: why the run then relaxes to a much lower surface. Candidates: receiver and area changes after step 1, the fill, ties on the 2048 flats. The planner decided not to spend more time (2026-10-25): the run keeps the input and only fills (GEN-065); the erosion option stays for M9.
- Tool: a scratch diagnostic (run minus input by band after N steps) was not committed; the stored-run report is `chunk_incision --run-vs-base`.
- Obsolete when the stream-power run is calibrated so that its steady state holds on a real bake.

## 2026-10-29 A still-frame detector must assert content
Scope: project (tier C detectors, TEST-068 to TEST-070)
- Two TEST-069 cases passed on black frames: an "identical pixels" check holds trivially for an empty image.
- Every still-frame or flicker detector also asserts that the frame draws terrain (a minimum of non-sky, non-black pixels).

## 2026-10-29 A CPU latched at 17 % performance inflates every timing
Scope: project (perf windows, Rasierklinge)
- On 2026-10-29 the CPU stayed at about 17 % `PercentProcessorPerformance` under load all day (firmware latch); an EC reset (power button held 30 s) restored it.
- Check `Win32_PerfFormattedData_Counters_ProcessorInformation` under a short load before a perf window and treat numbers from a latched day as invalid.
- Obsolete when: a BIOS update removes the latch.

## 2026-10-30 Window presentation on the RTX needs an unlocked desktop
Scope: project (real-GPU window tests, the real-store gate, acceptance)
- While the Windows session is locked or in modern standby, present stalls 1.3 to 2.7 s or the device is lost; unlocked, the same test passes in 26 s. A remote unlock (Chrome Remote Desktop) counts.
- Headless scenes, the software adapter and CI are unaffected. Window checks report NOT RUN when locked (TEST-073).

