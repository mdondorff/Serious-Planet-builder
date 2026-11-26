# Learnings

Added with `/learning`. Each entry names its scope, the assumption it changes, the follow-up and when it becomes obsolete. A learning is promoted (a spec line, a `CLAUDE.md` line or a test that cites it), kept while still active and surprising, or archived with its milestone (ADR 0075). Cap: 20 KB. Archives: `docs/archive/M0-M3/learnings.md`, `docs/archive/M4/learnings.md`, `docs/archive/M6/learnings.md` (M5 and M6, and the full text of the entries below), `docs/archive/M7/learnings.md`, `docs/archive/M9/learnings.md`, `docs/archive/P3/learnings.md`.

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

## 2026-10-29 A still-frame detector must assert content
Scope: project (tier C detectors, TEST-068 to TEST-070)
- Two TEST-069 cases passed on black frames: an "identical pixels" check holds trivially for an empty image.
- Every still-frame or flicker detector also asserts that the frame draws terrain (a minimum of non-sky, non-black pixels).

## 2026-10-30 Window presentation on the RTX needs an unlocked desktop
Scope: project (real-GPU window tests, the real-store gate, acceptance)
- While the Windows session is locked or in modern standby, present stalls 1.3 to 2.7 s or the device is lost; unlocked, the same test passes in 26 s. A remote unlock (Chrome Remote Desktop) counts.
- Headless scenes, the software adapter and CI are unaffected. Window checks report NOT RUN when locked (TEST-073).

## 2026-11-01 nextest runs each test in its own process: an in-memory fixture shares nothing
Scope: tests (nextest; SPIKE-84 fix 2)
- SPIKE-84 proposed a `OnceLock` fixture to build the `concurrent_source` store once per binary. Under nextest every test is a process of its own, so a static is built once per test, as before.
- A fixture worth sharing lives on disk (`CARGO_TARGET_TMPDIR`), named by the test binary, built into a private directory and renamed into place (`crates/cache/tests/support/concurrent_source.rs`).
- Obsolete if the tests move to a runner that shares one process per binary.

## 2026-11-01 An edit re-keys coarse tiles far beyond its reach
Scope: project (editor refresh find; WDEF-008, STRM-070, ADR 0045 point 7)
- A coarse tile (levels 0-5 at Earth's radius) that holds an item reads its region summary over its cells and `margin_rings` rings of cells, sized for the 40 km maximum reach plus whole cells: about 13 rings (450-500 km) at level 5, the whole body at levels 0-2. Any edit anchored there re-keys it, whatever its own reach.
- The editor's find named a tile only when the edit's reach touched it, so such tiles were never marked stale and stayed on their old version (the `gpu_edit_perf` flake: 8 substituted nodes, nothing pending). The find now walks `neighbors8` from the anchor cells and compares with the rings (edit-stale-diagnostics).
- ADR 0045 point 7 meant about one coarse tile per level per edit; today's margins re-key more (a Tracked architect item).
- Obsolete when the summary margins are sized from the reaches present, or coarse keys stop reading margins.

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

## 2026-11-09 The plain flood's FIFO inflates pits with scale
Scope: project (GEN-159, GEN-141, A3 6.4)
What happened: the sediment pass flooded with the plain `priority_flood` (FIFO for flats) while the rounds and H1 use the canonical flood; its pit count was 1.0 times H1's at 3,072 cells, 2.2 times at 6,144 and 18.5 times at 12,288 on one surface. A fixture shows the FIFO overtaking a lower heap entry on a flat chain; that it captures a whole valley is inferred from the scale trend. With the canonical flood the definitions agree (0.26 % and 0.35 % pits after the pass at k = 1 and 2).
Assumption it changes: that the two floods differ only by tie order at epsilon scale; a statistic or sweep on the plain flood (K7, `deposit_on_own_route`, `fill_depth`) can be inflated on large flats.
Obsolete when: the plain flood has no caller.

## 2026-11-10 H1 without the runoff returns areas, not discharges
Scope: project (GEN-165, GEN-145, A3 5.5)
What happened: `hydrology(.., runoff None, ..)` makes `Q` and `Q_w` true areas in m2 (every local term is 1); `k7_region` called it that way and passed `hy.qw` to the width law `w = 7.2 Q^0.5` (m3/s), which cut trenches of tens to hundreds of km and 95,820,316 lowerings; the G-SURF-I bisect (ea6b1b2b..760bf0e7) found the first commit, 59a7deaa. With the runoff in m/s the widest channel of the prototype tile is 93.5 m (0.92 cells), the burn count is 4,631 again and the G-SURF-I numbers equal the old row.
Assumption it changes: that a `Q` read from H1 is a discharge. `Hydro.q_unit` records which it is; the width law (`burn_channels_wide`) refuses a channel whose `q_unit` is an area.
Obsolete when: H1 has no path without the runoff.

## 2026-11-12 The carve amplifies sub-millimetre differences at large relief
Scope: project (GEN-175, GEN-166)
What happened: on a synthetic 60 km world (40 km cone, 384 x 384 cells of 800 m, 4 rounds) f64 at flood epsilon 1.95e-3 m against 1e-3 m differs by a p99 of 77 m, and a 0.2 mm random perturbation of the f64 start by 1,400 m (p50 3.6 m; 4 km control: p99 104 m); a midpoint datum (f32 about 20 km) made f32 differ from f64 by a p99 of 75 m after 4 rounds because it coarsened the rounding grid near the sea (1 mm instead of 1e-7 m), where the ties are; in the sea frame f32 against f64 at the same epsilon is within 0.011 m (p99) after 4 rounds. Differences between runs grow from round 3 as cascades of breach and flat routes.
Assumption it changes: that an f32 against f64 difference over several rounds measures the rounding; it measures the carve's sensitivity. Compare one round, or a perturbation control.
Obsolete when: the carve is stable under start-surface noise.

## 2026-11-12 A flat grade narrowed to f32 collapses at any epsilon near the spacing
Scope: project (GEN-173, GEN-175)
What happened: GEN-173 grades a flat by `epsilon d / sqrt 2` and the receiver is the steepest jittered slope, so neighbouring cells of a flat can differ by a small fraction of epsilon; on a 60 km world (spacing 3.9e-3 m at 40 km) a fill narrowed to f32 collapses at `epsilon_w` (40 lifted cells of a crater floor are not above their receiver), and no factor on the spacing guarantees strictness. H1 floods in f64 and the plan stores no f32 fill; pass 2 does store its heights as f32 (tile.rs), with incise chains of `hr + epsilon`.
Assumption it changes: that `epsilon >= spacing` keeps every flood step strict after narrowing; it holds for the carved epsilon chains, not for the flat grade.
Obsolete when: a stored or narrowed copy of a filled surface exists (re-flood it in f64 or grade it by a full epsilon) or the grade steps by a full epsilon.
## 2026-11-12 K7's halo sets its result; it is not a performance setting
Scope: K7 (ADR 0119)
What happened: one region, two tilings. At halo 128, 0.57 % of non-line cells differ (p99 1.1 mm, max 0.31 m); at halo 64, 2.29 % (max 3.59 m, all at the seam). River line cells are bit-identical at both.
Assumption it changes: that the halo is a margin that only has to be wide enough. Tile size and halo are inputs of K7's result and part of its key.
Obsolete when: K7 reads region-wide inflow at the halo border.

## 2026-11-13 A push is gated on the prepush in the same command
Scope: project
What happened: a push to PR 1482 (68cb12a3, one golden PNG) went out although its prepush had failed (test gpu: 2 known WARP d3d10warp crashes, 3 gpu_skeleton timeouts of 300-450 s under load), because the push was a separate statement after the prepush. Again 2026-11-24, PR 1651: `prepush | tail && git push` returned tail's status.
Assumption it changes: CLAUDE.md "Before every push to a PR branch run `cargo xtask prepush`" assumed the push follows only a passing prepush.
Proposed follow-up: `cargo xtask prepush && git push`, with no pipe before `&&` (or `set -o pipefail`).
Obsolete when: a git pre-push hook runs the prepush.

## 2026-11-14 The L basins come from the 814 m to L hand-over, not from B
Scope: project (hand-over 814 m to L; P3 E1)
- The solved 814 m surface drains at its grid (5.9 % of river length on closed basins); read at L it has 74.5 %: a D8 diagonal between two higher cells is a saddle in any bilinear or C1 read. Five B-stage variants leave 61-70 % against 71 % (spikes/drain-base/results/results.md, #1498).

## 2026-11-14 The basin metric decides a river verdict
Scope: project (river gate, BUILD-055)
- "Any pit over 5 cm" fails SPIKE-115's flat windows (29.1 %, 13.7 %); basins of 0.5 m and 16 cells under a 1e-7 m fill give 0-4.6 % on the same kind of surface (spikes/combo-117/results/results.md). State the metric with every share.

## 2026-11-14 K7 beds are not monotone
Scope: K7 (GEN-169, GEN-176, GEN-185)
- The burn only lowers, so line heights rise downstream: on r3a-e1 (R1h window) 63,167 of 219,390 line cells rise over 0.25 m (max 44.7 m). Cuts over 50 m come from H1 lines crossing L ridges (gprofile-read-2026-11-11.md; crusader's outbox).

## 2026-11-14 A stored channel level bounds the bed from above
Scope: project (K7 levels, river drawing)
- The drawn ground lies below it at 83 % of channel points (median -3.8 m, p5 -30 m); on r3a-p2 the line is a raised one-cell line beside the carved low (SPIKE-113). It is no water surface.

## 2026-11-14 Name a window by face, box and seed
Scope: project (spike and gate reports)
- SPIKE-114/115 called face 1, 8056-8675 x 760-1379, seed 14 "R3a"; the spikes lane called it R1h and used R3a for face 4, seed 15. Canonical: R1h, R3a, R3b (lead, 2026-11-14; #1506).

## 2026-11-14 A 4-neighbour hand-over aligns rivers to the grid
Scope: project (hand-over, ADR 0123 draft)
- 66.5-68.6 % of channel steps run along an axis, against 47 % for B alone or one round at L (spikes/combo-117/results/results.md).

## 2026-11-14 The river network cannot be rebuilt from B
Scope: project (store format, river geometry)
- Only 8.0 % of channel cells agree with routing on B + regional: store the L geometry (about 0.9 MB a region) or the regional change (spikes/rivers-at-l/results/results.md).

## 2026-11-14 One regional-kernel round at L is the optimum
Scope: project (regional kernel at L; K7 flood)
- Three rounds raise the relief to 156/143 %, ten to 521/358 %; Schott 2024 multi-scale with unchanged parameters gives about 3x. K7's canonical flood refuses flats of an eroded surface (spikes/erosion-at-l/results/results.md).

## 2026-11-14 Rounds and routing at L are not tile-bounded
Scope: project (partitioning, CON-30)
- The largest catchment holds 5.07 M cells against 0.59 M in a tile, with 27,636 pits; a region stage, or Barnes 2016's exact tiled flood (spikes/erosion-at-l/results/results.md).

## 2026-11-14 The layer read's clamp made steps at L cell-centre lines
Scope: project (layer read GEN-188; the bake's reads)
- Max 12.5 m, 1.5 % of crossings over 0.1 m (0.001 m without the clamp); the bake's twin reads clamp the same way. SPIKE-114's "41 % steps" counted curvature too (spikes' outbox; rivers-at-l results).

## 2026-11-14 No solver path keeps an inland lake
Scope: project (edits and lakes, GEN-162, ADR 0113)
- The breach cuts every basin; a 400 m dome keeps about 188 m after the rounds (SPIKE-112).

## 2026-11-14 Memoised rounds recompute the serial area sweep
Scope: project (regional solver, edit path)
- About 80 % of a round, so memoised rounds take 2x resident ones (159 s a round at 1.42e8 land cells); a per-catchment parallel sweep: 69 s on 8 threads, bit-equal (spikes' outbox).

## 2026-11-14 Later-stage start-surface fixes failed
Scope: project (start surface; GEN-180 removed)
- The coast tilt made R3a worse at every setting (12.0-15.6 % against 9.9 %); the band's crest rings dam a basin each (SPIKE-109, SPIKE-111; #1500).

## 2026-11-14 A probe that copies library code goes stale
Scope: project (gates and probes)
- G-LOCAL's L1 mismatch (437,249 cells, up to 691 m) was the probe missing GEN-173's graded flood; with it, 0 of 1,040,400 differ (glocal-f32-2026-11-12.md). Probes call the library.

## 2026-11-14 K7's seam difference opens in the carve
Scope: K7 (ADR 0119)
- Under both tilings the burn and breach are bit-identical; the carve differs on 7,032 cells (at most 0.012 m), deposition adds the 0.087 m maximum (k7-seam-2026-11-11.md).

## 2026-11-14 gpu_skeleton times out only under CPU oversubscription
Scope: project (tier C, prepush)
- The two pipelined loops take 2.8 s alone and 28-69 s with 20-80 busy processes on WARP; the RTX is unaffected (spikes/gpu-skeleton/results/results.md).

## 2026-11-14 The CPU latch recurs
Scope: project (perf windows, Rasierklinge)
- About 15-17 % `PercentProcessorPerformance` under load on 2026-10-29 and 2026-11-13; an EC reset (power button 30 s) cleared both; SPIKE-113's timings were void. Check under load before a window. Obsolete when a BIOS update removes it.

## 2026-11-14 Messages pause after 10 sends, and order files wake nobody
Scope: project (lane process)
- Without an owner message the app pauses a lane's sends; an idle lane does not read its order file (explorer idle about 5 h on 2026-11-14; ADR 0118).

## 2026-11-14 A default-on stage after one measurement made its defect worse
Scope: project (generators, process)
- The round-0 breach needed a second PR to switch off; three later-stage basin fixes were dropped (ADR 0118, ADR 0120).

## 2026-11-15 GLO-30 is a surface model: its basins and flat-ground relief are canopy
Scope: project (the GLO-30 benchmark, SPIKE-108's frozen pools, P3 E1)
- Against USGS 3DEP bare earth on 4 tiles (SPIKE-119): GLO-30 stands 0.8-9.7 m above it at p50; basins on river length 1.3-2.9x (Appalachians 64.9 % against 22.1 %); floodplain relief over 10 x 10 cells 15.2 m against 4.7 m.
- GLO-30 basin shares are an upper bound; low-gradient windows are judged against a DTM. Steep ground agrees within 2 % relief.
- Obsolete when: the benchmark uses a bare-earth source for every tile.

## 2026-11-17 River-graph markers: own uniqueness class, walked through
Scope: project (river graph, GEN-189)
- A marker shares a cell only with its reach's up node (contract reading 1); validators follow a chain through a marker's reach, or cycles and outlet-less chains through junctions go unseen.
- Obsolete when: never.

## 2026-11-26 River-graph fixes ran only downstream; S0, the base and the climate were treated as fixed
Scope: project (any pipeline gated on a look or on bars measured against a stage's own input)
- G2 to G3 (2026-11-22 to 11-26): at least 10 fix rows moved a defect to another stage. The attribution then traced 88-93 % of the residual look to S0 (K5 and the base), which no row changed.
- Causes: S0 was every bar's reference; the plan held the base and climate fixed; G3 allowed only output-neutral main changes; the lead widened an owner remark about M1 to every parked item; kept lakes came from a geometric rule, not water balance (ADR 0111 point 5), so precipitation was never a suspect.
- Follow-up: routing rule 10. Obsolete when: never.
