# Design note: the graph's line geometry (lead #181; SPIKE-142, SPIKE-143, SPIKE-144)

Status: design note, 2026-11-23. No build. It decides nothing that O8 (the sub-cell centreline) and decision 11 (reading 22: which L lakes are kept) leave to the owner.

## Why one note
- Five local fixes moved the cut to the next stage, each because the graph's lines still cross the cols and rims and every later stage carves along them:
  - `--kh-saddle-split` (#174a): KH -> band6;
  - `--kh-zone line/line8` (#177, #177a): the facets go, the cuts and corner links rise;
  - `--band6-keep-rims` (#180): band6 -> KV, KV's trenches 535 -> 1,046.
- The fix belongs where the lines are decided.

## 1. The line geometry
| | Today | Proposed |
|---|---|---|
| Trunk line (KG-trunk, `rg_chain.rs` `build_graph`) | the L image of the H1 D8 trunk: the lowest path inside a corridor through the H1 cell centres; vertices are 8-connected L cells | a sub-cell centreline: points in continuous L coordinates, smooth, through the trunk's H1 cells, following the lowest ground in its corridor; its width per point (O8) |
| Tributary lines (KG-trib, `kg_trib.rs`) | D8 chains on KS0's routing at L | the same chains ending on the trunk's centreline; the same crossing rule as trunks |
| Raster | the vertex cells themselves | the centreline rasterised 4-connected (no corner-only step); the vertex cells are that raster, so the graph format keeps L cells |
| Crossing a col or a kept rim | implicit: the line runs through it and every later stage carves the cell chain | explicit, one record per crossing: `breach` (cut within the budget), `spill` (the line goes over the rim at the spill height: a lake inlet and outlet), or `around` (the centreline bends round it inside the corridor) |

- Who decides:
  - basins-2's kept set decides which rims are kept (decision 11). It must be known before the lines are final; today basins-2 runs after KG-trib post.
  - KG-trunk decides the trunk centreline and its crossings from the kept set and the budget.
  - KG-trib decides the tributaries' lines and crossings the same way.
  - No later stage chooses a path.
- What each later stage reads instead of the D8 cell chain:

| Stage | Reads | Change |
|---|---|---|
| KH | the centreline | `line8`'s correction along the curve, no quad split (#177a) |
| KS1 | the rasterised centreline bed | targets along the 4-connected raster, not KS0's routing |
| band6 | the crossings | breaches only at `breach` crossings; keeps every `spill` depression (the kept set) |
| KV | the centreline, its width and the crossings | carves along the curve; not below the spill level at a `spill` crossing (as kept rims are held today, lead #176a) |
| KC | the raster | unchanged: breach and fill of the residual pits |

## 2. What it removes (R1h, today's tip)
| Look | Today | What decides it after |
|---|---|---|
| Chainsaw | in-zone Xdiag 0.096, M4 18.53 m (final) | KH along the curve: `line8` showed 0.008 and 8.61 m |
| Straight runs | 34.8 % of trunk length in runs over 2 km (F1 52.2 %, R3b 40.9 %); sinuosity p50 1.12 | the curve (GLO-30: 7.3 %, 1.25) |
| Corner links at 5 m | 260 (`line` 410, `line8` 331, keep-rims 1,114) | a 4-connected raster: none from the lines by construction |
| Hump trenches over 10 m | 2,369 (KS0 505, KS1 720, band6 928, band6 connect 136, KV 535) | `around` and `spill` crossings take the cols the curve can avoid. A trunk still cuts where H1 drains: SPIKE-142's 78 % are real K5 cols and stay `breach` crossings unless they are kept as lakes |
| band6's kept-rim cuts | 365 corner-crest trenches (82 % on saddle blocks) | `spill` crossings at kept rims; `band6-keep-rims` alone took them 365 -> 34, but KV cut them again |

## 3. Cost
- Stages and files:
  - KG-trunk (`rg_chain.rs` `build_graph`, `route_low`);
  - KG-trib (`kg_trib.rs` `extract`);
  - basins-2 (`basins_rg.rs`, its place in the order);
  - KH (`rg_chain.rs` KH block, `kh.rs`);
  - KS1 (the target raster);
  - band6 (`drain.rs`, its keep mask);
  - KV and KC (`kv_kc.rs`);
  - `river_graph.rs` (the attributes and exceptions);
  - `river_gate*` (measures on curves);
  - the flight store and viewer ribbons (R19).
- Contract readings it changes:
  - reading 1 (anchors and markers on vertex cells, now raster cells);
  - readings 2 and 7 (snapping);
  - reading 18 (trunk accumulation along the raster);
  - reading 22 (the kept set moves before the lines);
  - reading 23 (band6's breach set);
  - readings 32 and 41 (rises at reach ends, now at crossings);
  - reading 38 (4-connected carving, now structural).
- Switch rows, in order, each judged by the ablation rule on R1h, then R3a and M1:
  1. KG-trunk's centreline and its 4-connected raster; later stages read the raster as today's cells. Measures straight runs and corner links.
  2. KH along the centreline (`line8`'s form on the curve). Measures the chainsaw (M4, in-zone Xdiag).
  3. The kept set before the lines; `spill` and `around` crossings recorded; band6's keep mask from them. Measures band6's trenches and the shares.
  4. KV holds `spill` crossings. Measures KV's trenches, corner links and spill-missing.
  5. KG-trib to the curves and the kept rims. Measures agreement and the tributary trenches.
  6. Width and meanders (O8's sinuosity, 1.2-1.4 for rivers wider than about 10 m). Measures slope and sinuosity against GLO-30.

## 4. Place in the three-wave design (seam execution brief, items 5 and 6)
- Wave 1 (global decide: K5, H1, sea mask, KG-trunk, KS1 targets): the trunk centreline, its crossings and its raster are wave 1 decisions, since KG-trunk is in wave 1.
  - The kept set they need must then also be decided in wave 1, a basins pass on KS0 or S_KH before KG-trunk; today basins run in wave 2.
  - Risk from #164 (order 3c): a basins pass that sees only trunks left 12 spills with no L line on R1h. The kept set for crossings must therefore come from a pass whose reaches include the tributary heads, or be a trunk-only set that is refined in wave 2.
- Wave 2 (global KG-trib, basins, inlet floor, KV `catch`): the tributary centrelines and their crossings.
- Wave 3 and the tile applies (KH, KS1, band6, KV, KC): read the decided curves, rasters and crossings; no path choice.
- Store fields (R19):
  - per reach, the sub-cell centreline (f32 offsets from the raster cells, or spline control points) and the width per point;
  - per crossing, its type, cell and level;
  - per lake, the water level (O9).
