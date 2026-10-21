# SPIKE-26 results: the item sweep, an automated stand-in for the owner's P1 retest (owner-items-sweep)

Planner brief 2026-10-21. Main f2382bd8 plus `crates/app/src/sweep.rs` (the sweep), its hooks in `window.rs` and the tier-C test `gpu_window::the_item_sweep_visits_ten_large_plateaus_and_craters_at_three_altitudes_without_a_refused_plan`. Machine: Rasierklinge, RTX 3080 Ti Laptop (Vulkan), release build, other sessions running; no perf window involved.

## Answer
**The owner's 77 items are not clean: 45 of 231 stops (19 %) refused at least one plan, 22 items, all in the base LOD ("morph interval is empty because the height extent of its neighbourhood leaves no room", levels 16 to 22).** Nothing crashed or panicked; each refusal degraded the frame and the window went on (39 new plan-refusal reports in `target/review/crashes`, 0 panics, 0 GPU losses, exit through the sweep's own verdict). Plateau 7 is clean at all three altitudes. The synthetic set of 10 large plateaus and craters (the new test) passes on the RTX in 4.7 s and on the software adapter in 15 s: 0 refusals.

## Method
`PLANET_SWEEP=<report> planet editor --edit --items <copy of target/owner-items>`: the real window loop with the camera overridden. For each of the 77 items in file order, 3 stops at 2,000 m, 200 m and 30 m above the ground, the camera standing 1.5 altitudes south of the item's anchor and looking at it, 90 frames per stop (the arrival frames, where tiles stream in, included; the camera jumps from stop to stop, it does not fly). Per stop: frame times (the window's frame interval), `PlanPipeline::refused_plans()` delta, the first refusal's text. The run ends after the last stop and fails if any plan was refused. World: the editor's default (`cpu-erosion-v1` over a 256-cell bake), the items loaded as the owner's copy (never modified). Wall time 27 minutes (20,790 frames; the frame rate is low while tiles generate after each jump).

## Per altitude (77 stops each)
| altitude | stops with a refused plan | refusals | median p50 / p95 frame (ms) | worst p95 |
|---|---|---|---|---|
| 2,000 m | 4 | 4 | 27 / 47 | 250 |
| 200 m | 18 | 18 | 46 / 65 | 250 |
| 30 m | 23 | 50 | 64 / 88 | 250 |
The 250 ms is the window's frame-interval cap: the first frames of a stop after a jump hit it (cold generation); the medians are the stops' steady state. Frame times include the arrival (streaming) frames, so they are not a perf measurement.

## Per kind (stops, stops refused)
brush 126 / 37, plateau 36 / 1, hill 21 / 2, ridge 18 / 2, crater 18 / 3, channel 6 / 0, region 6 / 0.

## Per item (items with a refusal; altitude: refusals)
- Brush 1 (2 km: 1), Brush 6 (2 km: 1), Brush 10 to 21 and 23 (200 m: 1, 30 m: 2 each), Brush 22 (30 m: 2), Brush 33 (30 m: 2; several items carry this name, 5 stops refused), Brush 42 (200 m: 1, 30 m: 2). The brushes 10 to 23 are one cluster at about 27.9 S 12.5 W: Brush 10 to 12 raise strokes (radius 100 m, strength 5 m), 13 to 19 flatten and smooth strokes (radius 300 m, strength 5 m), 20 smooth (radius 300 m, strength 50 m), 21 to 23 smooth (radius 1,500 m, strength 50 m, reaches of 4.7 to 11.4 km); the first refusals name the same few tiles.
- Ridge 3 (200 m: 1, 30 m: 7), "Platonmax" crater (200 m: 1, 30 m: 1), Crater 6 (200 m: 1), Plateau 10 (2 km: 1), Hill 7 (200 m, 30 m).
- Clean at all altitudes: Plateau 7 (worst p95 at 2 km, 200 m, 30 m: 112 / 250 / 250 ms, the arrival frames), every channel, every region, all other plateaus, hills and craters.
- The full table, 231 lines with each first refusal text: `owner-items-report.txt`.

## What the refusals are
`invalid LOD parameters: tile T (level L): morph interval [a, b] m is empty because the height extent of its neighbourhood leaves no room; this terrain is too steep for cells 32 and viewport_h_px / tau_px 240` at levels 16 (5 stops), 17 (4), 18 (3), 19 (26), 20 (3) and 22 (4). The interval is empty by a few metres to a few hundred metres (e.g. level 19 [300.1, 248.0], level 16 [2043.2, 1983.7]): the tiles are near the limit, not far over it. The raise strokes are 5 m of strength at radius 100 m, so the extent that a tile's neighbourhood carries comes from the bound of the stamp stage over the brushes (GEN-026 as changed by #489) rather than from the relief, which fits SPIKE-22 (the real eroded field stays inside the rule, the items widen the bound). Not investigated here: whether the bound is looser than the stroke's real delta.

## The reusable test (tier C)
`gpu_window::the_item_sweep_visits_ten_large_plateaus_and_craters_at_three_altitudes_without_a_refused_plan` writes 10 plateaus and craters of 15 km reach into a temporary items directory, runs `planet editor --edit --items DIR --size 480x270` with `PLANET_SWEEP` and 4 frames per stop, and asserts `[sweep ok]: 30 stops, 10 items x 3 altitudes, 0 stops with a refused plan` and every stop in the report. `PLANET_SWEEP` makes the window accept the test adapter (`PLANET_ADAPTER`), like `--smoke`. A second tier-C test over an owner-sized cluster (14 brushes of the strokes of Brush 10 to 23, ridge 3, hill 7, crater 6; 200 m and 30 m, 34 stops) fails on the RTX in 430 s with refusals at levels 18 and 19 ("morph interval [321.3, 248.0] m is empty"), the owner's text, and is `#[ignore]`d with a pointer to the tracked entry "Item sweep refusals" in `docs/status.md`. Four tier-A tests in `sweep.rs` cover the item file parse, the stops and the refusal counting, the camera pose and the quantiles.
To rerun on the owner's items: `PLANET_SWEEP=report.txt planet editor --edit --items COPY` (`PLANET_SWEEP_ALTS=2000,200,30`, `PLANET_SWEEP_HOLD=90` are the defaults).

## Limits
The camera teleports between stops (a fly-to would stream more gently); the world is the editor's default, not necessarily the owner's bake; refusals are counted per stop from the pipeline's counter, so a refusal that began in the previous stop's last frames can be attributed to the next one (at most a few per run); the frame times are the window's own intervals with vsync on.
