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


## 2026-11-01 The clipmap gather needs one atomic, against ADR 0085 point 9
Scope: render (REND-047, REND-053; ADR 0085 point 9)
- Point 9 says layer 1 uses no atomics. Overlapping blocks written by unordered dispatches raced on the software adapter (an incremental gather differed from a full one at random). The gather now clears, claims (`atomicMax` of the job index) and writes, so the finest block wins in any dispatch order.
- The result of an integer maximum does not depend on order, which is what point 9 protects. REND-053 now says that (the march, upsample and temporal passes stay atomic-free; a tier A scan checks the shaders). The ADR wording needs a one-line amendment (ADR 0078): "no atomics except the owner pass's integer `atomicMax`".
- Obsolete when the gather resolves overlaps without an atomic (for example one thread per texel scanning the level's blocks, or a spatial index of winners).

## 2026-11-01 nextest runs each test in its own process: an in-memory fixture shares nothing
Scope: tests (nextest; SPIKE-84 fix 2)
- SPIKE-84 proposed a `OnceLock` fixture to build the `concurrent_source` store once per binary. Under nextest every test is a process of its own, so a static is built once per test, as before.
- A fixture worth sharing lives on disk (`CARGO_TARGET_TMPDIR`), named by the test binary, built into a private directory and renamed into place (`crates/cache/tests/support/concurrent_source.rs`).
- Obsolete if the tests move to a runner that shares one process per binary.

## 2026-11-02 A belt narrower than about 190 km cannot meet GEN-097's step limit of 2.0
Scope: generators (GEN-094, GEN-097; `classes.rs`)
- The class lattice has a cell of about 39 km and `k` runs from 1 to 7.5, so a step of at most 2.0 needs the weights to change over about 130 km (190 km for a smoothstep). Gaussian belts of 130 km half-width gave steps of 6.5; tent belts of 220 to 500 km and gates over 250 km give 4.9, with 0.22 % of steps over 2.0.
- A pair of plates must contribute a term that does not change when the two plates swap (distance, convergence and the continental difference are symmetric), and the third plate's weight must fall to 0 where it trades places with the fourth; otherwise the mask jumps at plate bisectors and triple junctions (the straight edges of iteration 0).
- Obsolete when the limit is restated (a feathering rule, a smaller `k` range) or the lattice is finer.

## 2026-11-01 An edit re-keys coarse tiles far beyond its reach
Scope: project (editor refresh find; WDEF-008, STRM-070, ADR 0045 point 7)
- A coarse tile (levels 0-5 at Earth's radius) that holds an item reads its region summary over its cells and `margin_rings` rings of cells, sized for the 40 km maximum reach plus whole cells: about 13 rings (450-500 km) at level 5, the whole body at levels 0-2. Any edit anchored there re-keys it, whatever its own reach.
- The editor's find named a tile only when the edit's reach touched it, so such tiles were never marked stale and stayed on their old version (the `gpu_edit_perf` flake: 8 substituted nodes, nothing pending). The find now walks `neighbors8` from the anchor cells and compares with the rings (edit-stale-diagnostics).
- ADR 0045 point 7 meant about one coarse tile per level per edit; today's margins re-key more (a Tracked architect item).
- Obsolete when the summary margins are sized from the reaches present, or coarse keys stop reading margins.

## 2026-11-02 A rough coast has no gradient at the lattice's scale
Scope: generators (continent stage GEN-104, class source GEN-094, landform driver GEN-064)
- The continent value has 8 octaves at gain 0.72 (wavelengths down to about 30 km), so `land depth / |gradient|` is not a distance to the coast: on seed 1 the coast gate went from 0.0003 to 0.70 over one 40 km lattice cell and the step of `k` was 6.5.
- The class source's coast distances and coastal arcs read the smooth value (4 octaves, no islands), the landform driver the coarse value (3 octaves); both are the full value without its detail terms (the octave series is normalised by its full weight sum, or the low octaves would be rescaled). A driver on the full value fails the lattice's closeness test (0.05 to 0.33 against 0.03).
- Obsolete when the bake replaces the landform driver (m9-bake) and the class lattice's cell is finer than the roughest octave.

## 2026-11-02 The coast gate on the rough depth sets the largest steps of k
Scope: generators (class source GEN-094, GEN-097's step limit)
- The coast gate switched at the full continent value's coast, which is rough at the lattice's scale: the gate went from 0 to 0.6 over one 40 km cell (largest `k` step 6.0).
- Ramping over the smooth depth (0.3) and the true depth (0.2) took it to 5.1-5.8; the rest is geometric (a belt flank changes by 0.2-0.3 of its core per cell while the classes differ by 4-6 in `k`).
- Plate draws beyond 64 widen the seed spread (seed 2's land above 2 km went to 27.7 % at 256 draws): the retry scores land at a convergent margin, not relief.
- Obsolete when the arcs are convergent-only or the owner moves GEN-097 to a p99 form.

## 2026-11-03 Edit finds must cover in-flight tiles
Scope: project (editor refresh find; STRM-026, STRM-071)
What happened: `gpu_edit_perf` did not settle at 5 tile workers ("kind Stale, substituted 2-6, unfinished jobs 0, pending 0"; #1117's default share on Rasierklinge) and passed at 6 and 18. The stuck tiles were resident level-14 tiles with another mesh key than the plan's, with the streamer reporting them not stale, not held, not queued. Every find (window.rs, editperf.rs, the coupled-lens refresh) ran over `managed.resident()` only. A tile whose job was in flight when `set_definition` swapped the field had read the old field, landed after the edit with the old key, and nothing ever marked it stale. A larger pool finished those jobs first: timing, not a thread-count-dependent result (CON-14 holds).
Assumption it changes: STRM-026's "the footprint find of the edit" named resident tiles only; the earlier "did not settle under load" failures of `gpu_edit_perf` were blamed on load. The stale-find entry above (region summary rings, `neighbors8`, edit-stale-diagnostics) is a different gap: resident tiles the reach missed.
Proposed follow-up: none; `ManagedLoop::edit_candidates` (resident plus in flight) is the find's input (STRM-071, tier A in `crates/frame/tests/managed.rs`). A request still queued needs no name: a job reads the field when it starts, and one handed to the spawner is already in flight.
Obsolete when: tile jobs carry the field version they read and the streamer drops a result whose version is older than the current one.

## 2026-11-03 Background shell children outlive TaskStop and their session; check for strays before a perf slot
Scope: project (agent tooling; perf windows)
- SPIKE-90 window 2 (11-02): TaskStop reported success but the script's `bash.exe` kept launching perf runs for 7 min. A root-wide `find` moved to the background (11-02 22:09) outlived its session as an orphan, held one core for about 10 h and failed the next perf slot (7 of 7 preflights).
- Assumption it changes: TaskStop ends the whole process tree; a quiet machine needs only idle lanes.
- Follow-up: after TaskStop on a script runner, find it by command line and `Stop-Process` it; no search from a drive root; check `tasklist` for stray `find.exe`, `bash.exe`, `planet.exe`, `xtask.exe` before a slot's first step; the owner stops them.
- Obsolete when: TaskStop ends Git Bash children and background commands end with their session, or perf runners are an xtask command and a hook refuses root searches.

## 2026-11-03 Count partitions balance test counts, not time
Scope: project (CI partitions; BUILD-018, BUILD-036)
What happened: nextest `count:N/K` gave tier A+B parts of 535 s and 209 s, and tier C a part of 22 min beside two of 15 min: slow tests cluster by name order. A replay of measured durations on each scheme's real membership predicted the jobs exactly.
Assumption it changes: that equal test counts give equal job times.
Proposed follow-up: none; A+B runs `hash:N/3` (stable when tests are added); check a split by replaying recorded durations before changing it.
Obsolete when: CI splits by recorded durations.

## 2026-11-04 The runtime lattice and the surface the layer was carved on are two generators
Scope: project (M9 bake; the erosion layer's runtime read, GEN-112)
What happened: on seed 14, mountains tile 2/12/10, the stored bake's surface (`global::sample`, the legacy generator, read C1 from a 611 m lattice) is 2,064 m rms from `BakeInputs::base` (the class source the layer was carved on): p5 -3,372 m, p95 +125 m; the relief channels `m + k (e - m)` (512-cell estimate) leave 2,061 m. A lattice of `BakeInputs::base` itself read C1 is 3.4 m rms (p95 5.5 m, max 28 m) at 611 m, 4.9 m at 1.2 km, 2.0 m at 305 m.
Assumption it changes: that the layer can be added to the runtime base plus `k` and `m`; and that the lattice of a store is the surface a bake read.
Proposed follow-up: the product's field bake is made from the class source and carries `BakeInputs::id()` (the viewer refuses `--erosion on` without that marker); `with_layer` needs no relief channels.
Obsolete when: the field bake is made from the class source and the legacy generator is retired.

## 2026-11-05 The stored change of keep-pits includes the pits
Scope: project (GEN-120, STRM-075)
What happened: the stored change (kind 13) keeps the pits and the runtime adds kind 17 back, so `removed_depth_m` on the stored change counts a pit as erosion; `erosion_depth_m` adds the pit depth first.
Assumption it changes: that kind 13 is the change the viewer draws.
Obsolete when: lake water replaces the filled look.

## 2026-11-09 The plain flood's FIFO inflates pits with scale
Scope: project (GEN-159, GEN-141, A3 6.4)
What happened: the sediment pass flooded with the plain `priority_flood` (FIFO for flats) while the rounds and H1 use the canonical flood; its pit count was 1.0 times H1's at 3,072 cells, 2.2 times at 6,144 and 18.5 times at 12,288 on one surface. A fixture shows the FIFO overtaking a lower heap entry on a flat chain; that it captures a whole valley is inferred from the scale trend. With the canonical flood the definitions agree (0.26 % and 0.35 % pits after the pass at k = 1 and 2).
Assumption it changes: that the two floods differ only by tie order at epsilon scale; a statistic or sweep on the plain flood (K7, `deposit_on_own_route`, `fill_depth`) can be inflated on large flats.
Obsolete when: the plain flood has no caller.
