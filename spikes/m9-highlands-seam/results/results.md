# SPIKE-72: the Highlands seam

Opened: 2026-10-31
Question: What makes the hybrid bake's core-edge step at the Highlands p99 99 m against 67 m off-edge, when the other sites show none (SPIKE-70, SPIKE-71)? Candidates: the 800 m face carve's upsampling, the 1 km^2 local cap, or the local network meeting the coarse one. Test one fix that is not a blend.
Why it matters: the last seam of ADR 0088's two-pass bake; the m9-bake change's PR 5 measures it.
Time box: 1.5 h (opus-planner, 2026-10-31); used about 1 h 15 min, with 25 min of machine time.
Success criteria: a map of where the step sits, the cause, and one fix with its seam and relief at the four sites.
Code location: offline crate patched from SPIKE-71's (`proto/patch.py.txt` on `spikes/m9-bake-time/results/proto/s71bake.rs.txt`), and the step map `proto/stepmap.py.txt`. Spike code, never merged.
Closed: 2026-10-31

## Method
- **Data:** SPIKE-70's windows, joint bake and `hybrid1`, at the four sites.
- **Step map:** the step across the two seam lines per edge cell, against the same surface's steps 150 and 300 cells off the edge, and where the steps over the off-edge p99 sit (distance to the sea, height).
- **Fix variants:** the local carve of pass 2 (capped at 1 km^2) changed; the face pass and thermal as `hybrid1`. Scoring as SPIKE-70.
  | Variant | The local carve |
  |---|---|
  | `hybrid1` | as SPIKE-70: a cell is raised above its receiver on the flooded surface (fills pits), breach before each route |
  | `nofill` | a cell whose receiver is not below it is left alone: pits stay pits for the local carve; the breach stays |
  | `nofill-nobreach` | no fill and no breach: the local carve only incises |
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`).

## Results
### Where the step sits (`images/highlands-stepmap.jpg`)
- Left: `hybrid1`; right: `coarse800` (the face pass alone, SPIKE-70). The seam lines are coloured by step (yellow 0, red 150 m and over); the sea is blue.
- `hybrid1` has 166 cells over the off-edge p99 on the horizontal seam line and 38 on the vertical one.
  - They lie 88-169 km from the sea, on high ground (1,077-2,030 m p50), not at the coast.
  - The worst cluster is at x 5,941-5,980 on the horizontal line.
- `coarse800` has no seam (52 against 58 m): the face carve's upsampling is not the cause.
- SPIKE-70's caps of 10 km^2 and 1 km^2 give the same 99 m: the cap size is not the cause either.
- At the worst cluster the surface is two flat lakes: 2,156-2,157 m north of the seam line and 1,961-1,964 m south of it. The base there is 1,024-1,250 m, a closed basin about 1,100 m deep.
  - The local carve raises every cell above its receiver on the flooded surface, so it fills the basin to its spill point.
  - A tile's window edge is base level, so the spill point, and with it the lake level, depends on the window: about 195 m apart in the two tiles.
  - The joint bake fills the same basin to about 2,316 m for the whole window.

### Fix: the local carve does not fill
Ratios to the joint are relief1 / relief5 / slope p50; seam is the edge p99 against the off-edge p99 (m):
| Site | hybrid1 | nofill | nofill-nobreach | Flat: joint / hybrid1 / nofill-nobreach |
|---|---|---|---|---|
| Highlands | 1.03 / 1.03 / 1.10; **99.0 vs 66.8** | 1.20 / 1.18 / 1.28; 72.5 vs 69.7 | 1.13 / 1.11 / 1.22; **67.8 vs 68.1** | 12.4 / 6.8 / 2.5 % |
| Black Forest | 0.98 / 1.00 / 1.07; 57.3 vs 58.5 | 1.17 / 1.15 / 1.26; 63.2 vs 62.1 | 1.09 / 1.10 / 1.17; 57.3 vs 59.3 | 11.4 / 5.1 / 1.6 % |
| Valley and Ridge | 0.95 / 0.95 / 1.07; 49.9 vs 52.2 | 1.14 / 1.09 / 1.25; 57.4 vs 57.4 | 1.05 / 1.04 / 1.16; 50.1 vs 52.9 | 12.7 / 6.2 / 2.4 % |
| Alps | 1.03 / 0.97 / 1.11; 76.2 vs 76.0 | 1.30 / 1.12 / 1.45; 78.8 vs 79.5 | 1.25 / 1.09 / 1.36; 78.4 vs 79.0 | 14.0 / 7.7 / 0.9 % |
- **`nofill-nobreach` removes the Highlands seam** (67.8 against 68.1 m) and has none at the other sites.
- It leaves the 100 m pits unfilled. Flats fall to 0.9-2.5 % (real 6-9 %), and relief and slope rise above the joint (relief1 1.05-1.25, slope p50 1.16-1.36).
- The seam measure's pond count near the seam lines rises to about 10,000-15,000 cells: unfilled pits, on both sides alike.
- `nofill` (with the breach) is between: the breach still cuts toward window-dependent spill points.

## Findings
1. **The Highlands seam is a filled basin.** A closed depression of about 1,100 m crosses a core edge. The local carve fills it to a spill point that depends on each tile's window, so neighbouring tiles leave lakes about 195 m apart. Neither the face pass's upsampling, the cap size nor the sea is the cause.
2. **Fix (not a blend): the local carve only incises.** A cell whose receiver is not below it stays, and there is no breach. Basins are left to the face pass, which fills them consistently over the face, and to the deposition sweep. This is seam-free at all four sites.
3. **The cost of the fix is in the flats:** 0.9-2.5 % instead of 5-8 %, with relief and slope 5-36 % above the joint. The deposition sweep of pass 2 (SPIKE-64: it fills basins and keeps 15-33 %) is local and seam-free, so it is the place to restore flats. R7 tunes the carve's `K t` and the amplitude.
4. **For ADR 0088 and m9-bake:** pass 2's local carve incises only (no fill, no breach), and the deposition sweep follows it. Pass 1 keeps the breach and the fill.

## Open points
- The deposition sweep after `nofill-nobreach` was not run: whether it brings flats to 6-9 % without a seam is R4 PR 5's check.
- The higher relief at the Alps (relief1 1.25) needs R7's tuning with the deposition in place.
- Four windows, one seed.

## Visuals
- `images/highlands-stepmap.jpg`: the Highlands window, seam lines coloured by step, `hybrid1` (left) against `coarse800` (right).

## Handover
- **Variants:** apply `proto/patch.py.txt` to SPIKE-71's crate (`spikes/m9-bake-time/results/proto/s71bake.rs.txt` as `src/main.rs`), build, and run `s72seam x <SPIKE-70 out dir> <sites>` (`proto/run.sh.txt`).
- **Step map:** `python -I proto/stepmap.py.txt <SPIKE-70 out dir> <site> <variant> <out dir>`.
- **Score:** SPIKE-70's `score.py` with the variants `joint,hybrid1,hybrid1-nofill,hybrid1-nofill-nobreach`.
