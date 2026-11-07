# SPIKE-104: holistic review of the M9 terrain pipeline, from the base surface to the runtime surface

Opened: 2026-11-07 (planner inbox #95; owner D8 a). Run on Opus.
Question: what each stage of the chain adds or removes in relief, closed basins and artefacts; where the basins come from; which of ADRs 0101, 0105 and 0109 to keep, merge or replace; the cost and memory of the resulting chain; the owner decisions it needs.
Why it matters: the erosion and basin chain was amended step by step (ADR 0088 -> 0099 -> 0101 -> 0105 -> 0109) without a whole-chain measurement; the planet bake (M9.2) fixes it.
Time box: about 1.5 lane-days, docs only.
Code location: `spike/terrain-pipeline-review` at 421880ab (from `change/pass1-deposition` at 81bff56f; one probe example, `crates/cache/examples/pipeline_probe.rs`; never merged). Store read: the explorer's G = 0 run `n0` (seed 14, face 2 at 12,288, tile 2/12/10; copied, read only).

## 0. Answer

1. **The basins of pass 2's grid are made by the hand-over between the passes, not by the base and not by the carve.**
   - Pass 1 drains its own grid: its carved surface, read cell by cell at 101.8 m, leaves 0.2 % of the tile's land in pits (p50 1.1 m).
   - The same surface read by the C1 interpolant leaves 13.3 % (p50 130 m, 2.1e12 m3). The read re-dams the one-cell breach channels of pass 1 at every diagonal step and narrow reach.
   - The base's own relief between pass 1's nodes (rms 21 m, p99 84 m) adds shallower pits (12.7 % at p50 35 m with a cell-by-cell read; 3x less volume).
   - Pass 2 cannot reopen either: its carve only incises, has no breach and caps the area at 1 km^2.
2. **A breach at 102 m reopens them at no relief cost.** Pass 1's breach rule (no incision, 4 routes) on pass 2's input:
   - with a limit of 150 m or more: 14.7 % -> 1.1 % of land in pits, volume -95 %, 3 km relief p50 508 -> 513 m;
   - with 50 m: 6.2 %;
   - cost: 21 s for 4 routes of 9.4 M cells on 4 threads; pass 2 already routes twice a tile.
3. **ADR 0109's infill works at the wrong grid.** On this tile pass 1 already drains its grid (0.2 %); the infill at 814 m does not reach the hand-over dams, and it lifts basins by up to 1.7 km (explorer: 8.3-9.8 % of land left in pits). Recommended: replace ADR 0109 by a breach in pass 2 (D-A below); keep ADR 0101 (keep-pits, the fill-depth channel); defer ADR 0105 point 1 (pass-1 deposition); keep ADR 0105 point 2 (arid sinks).
4. **Pass 2's base and the runtime's base differ.** The change is taken against the base sampled every 4 cells (407 m); the runtime adds it to the store's 611 m lattice read C1. The difference is rms 12.3 m, p99 50 m, max 209 m on this tile: relief that the carve shaped and the runtime does not show, or the reverse. Recommended: one base read for both (D-B).
5. **The owner's frames show the legacy in-memory field, not the bake** (section 1). Their bundles record `cpu-erosion-v1` over a 1024 in-memory bake of seed 1: no class source, no erosion layer.
   - The rounded stepped blobs, straight seams and arcs are in the heights. They are the crease filter over trilinear value noise. The same noise and filter make M9's runtime creases (octaves 7-11, about 14 m peak to peak) and the base's hill and mountain bands.
   - The blocky squares at 196 km are the default bilinear (C0) base read of the in-memory lattice under a low sun. The production path reads C1 when the layer is on; making C1 the default is #1137.
6. **Cost and memory.**
   - Pass 1 today: 1,236 s a face at B = 10, so 2.06 h for six faces against the 1.5 h target, and a 10.0 GB peak against CON-21's 8 GB.
   - The recommended chain adds nothing to pass 1 and removes the infill (+1.85 GB transient, one flood) and the pass-1 deposition (+1.2-1.9 GB, +4 % time) from it.
   - The 8 GB still needs an array diet in pass 1 (estimated -1.9 GB, section 5).
   - Pass 2 adds about 1-2 % a tile (estimate).

## 1. The owner's frames, placed

Evidence: `images/frames-placed.jpg`. The bundles were replayed (`test-render --repro`, debug relief view). The poses were re-rendered on the lit path (`--scene capture-stills --seed 1 --global-res 1024 --allow-memory-bake`, owner's binary at 77cc65e7, RTX), with ground detail 0 and 0.6, at sun 15 h and 17.3 h.

| Frame | Pose | What | Stage | Bake or runtime | Does 1024 in memory explain it? |
|---|---|---|---|---|---|
| 1510 | 143 m above ground, nadir, 7.437 N 29.627 W | rounded stepped blobs 20-60 m, straight seam lines, arcs | crease filter of the legacy eroded field: `0.25 - (1 - abs(n))^3` per octave over trilinear value noise (`height.rs:51`, `erosion.rs:157`), fine octaves lifted to the full 900 m amplitude by the landform driver (v1) | runtime field (CPU heights); present with ground detail 0, so not the shader | partly: v1 without the class source and the layer gives the creases their legacy amplitude, about 6x M9's. The noise and the filter are the same in M9 |
| 1162 | 1.6 km, same place | the same family at 150-400 m | the same, coarser octaves | runtime field | as 1510 |
| 3113 | 196 km, 9.6 N 27.8 W | blocky squares about 10 km | the bilinear (C0) read of the in-memory 1024 lattice (9.8 km cells): the slope jumps at every cell line, so cells shade differently under a low sun; at 512 the squares double | runtime field (default `cpu-erosion-v1`) | yes. The production store is 611 m cells, and the layer path reads C1 (GEN-107); the default stays v1 until #1137 |

- **Mechanism of the blobs and seams** (from the code; the straight lines are inferred, not isolated):
  - Value noise interpolated with `t^2 (3 - 2t)` has zero slope across every lattice plane, so each plane is a flat line on the ground.
  - The octaves double their frequency on the same axes, so the planes of a coarse octave are planes of every finer one.
  - The crease filter turns the noise's zero contours into narrow grooves: rounded closed loops, which read as blobs and arcs.
- **Where the same noise sits in M9:**
  - the runtime creases: octaves 7-11, about 14 m peak to peak (SPIKE-100);
  - the hill band (`global.rs:346`);
  - the ridged mountain band at 16-1 km (`classes.rs:562`): all base relief below 10 km (SPIKE-101).

  The shader's ground detail already uses turned lattices per octave (REND-057, #1186). The generator does not.
- **Ground detail** changes normals and albedo only. With it off, every shape in the frames stays.
- **The ~100 m checker** of the earlier erosion preview (crusader #55: one-cell pits from thermal and deposition) is a different artefact. It does not show in these frames.

## 2. Stage by stage

Mountains tile 2/12/10, seed 14, unless noted. "Pits" are land cells more than 0.5 m under the flood; this probe floods the tile's core, open at its border and the sea.

| # | Stage | Grid | Relief added or removed | Closed basins | Artefacts | Bake or runtime |
|---|---|---|---|---|---|---|
| 1 | Continents, plates, classes, `e0` (belt, plateau, coast ramp) | lattices of 40 km and more | wavelengths of 130 km and up; 3 km relief p50 21-48 m (SPIKE-101) | few, large (intermontane) | - | base (field bake) |
| 2 | Hill band, mountain band x4.5, ridged, centred | analytic | all relief below 10 km; exact base 3 km relief p50 478 m (Everest 804, Alps 543) | exact base: 30.6 % of land, p50 292 m, deepest 1.83 km, 1.25e13 m3 | value-noise lattice planes and blobs (section 1) | base |
| 3 | Pass 1: breach 500 m, keep-pits, 20 carve steps, MFD area, K, U, runoff | 814 m, point samples | +9 % 3 km relief over the base on this tile (D against B: 508 against 467 m); valleys deepen up to 1.5 km | on its own grid: 0.2 % of the tile (p50 1 m); 9.0 % of face 2 at p50 5 m (explorer) | point sampling aliases the base's sub-1.6 km octaves (exact minus pass 1's view: rms 21 m, p99 84 m) | bake |
| 4 | Hand-over: pass 1's change read C1 at 101.8 m onto the base sampled every 4 cells | 101.8 m | none | **14.7 %, p50 136 m, 2.5e12 m3**: the C1 read of pass 1's surface alone makes 13.3 % (p50 130 m) | dams across every diagonal link of pass 1's channels | bake |
| 5 | Pass 2: incise-only carve (1 km^2, 10 steps), thermal 33 degrees, deposition sweep, one-cell pit spreading | 101.8 m | about 0 at 3 km (508 -> 508 m) | 11.9 %, p50 139 m, deepest 696 m: only one-cell pits are removed | thermal leaves 0.58 M cells moving at its cap | bake |
| 6 | Fill depth (ADR 0101), runtime adds it back | 101.8 m | the runtime shows the filled surface | stored as the channel | flat filled floors where basins are deep (p50 139 m) | bake channel, runtime read |
| 7 | Runtime base: the store's lattice read C1 | 611 m | relief between 407 and 611 m is lost against pass 2's base (rms 12.3 m) | runtime-like surface (611 m base + change): 12.7 %, p50 116 m | change carved on one base, added to another | runtime |
| 8 | Layer read C1, coast fade 3 m, cube-edge cross-fade | 101.8 m | - | - | - | runtime |
| 9 | Crease band (octaves 7-11, below 200 m) | analytic | about 14 m peak to peak (SPIKE-100) | small, shallow (under about 10 m) | value-noise lattice lines and blobs at that amplitude | runtime |
| 10 | Ground detail (16 m, 2 m, 25 cm, 3 cm; turned lattices) | shader | none in the heights; normals and albedo | none | - | runtime (GPU) |

Raw lines: `probe-2-12-10.txt`.

## 3. Where the basins come from (probe, tile 2/12/10)

| Surface at 101.8 m | Land in pits | Depth p50 / p95 / max (m) | Volume (m3) | 3 km relief p50 (m) |
|---|---|---|---|---|
| A exact base | 30.6 % | 292 / 1,246 / 1,831 | 1.25e13 | 478 |
| B pass 2's base (407 m nodes) | 29.7 % | 299 / 1,235 / 1,799 | 1.22e13 | 467 |
| C pass 1's view of the base (814 m nodes) | 28.5 % | 304 / 1,212 / 1,778 | 1.17e13 | 449 |
| J pass 1's carved surface, read cell by cell | **0.2 %** | 1.1 / 2.2 / 3.0 | 2.0e8 | 465 |
| I pass 1's carved surface, read C1 | **13.3 %** | 130 / 457 / 613 | 2.14e12 | 483 |
| K B + pass-1 change, read cell by cell | 12.7 % | 35 / 211 / 714 | 7.8e11 | 561 |
| D pass 2's input: B + pass-1 change, read C1 | 14.7 % | 136 / 522 / 782 | 2.54e12 | 508 |
| E pass 2's output (B + stored change) | 11.9 % | 139 / 467 / 696 | 2.04e12 | 508 |
| F runtime-like: 611 m base + stored change | 12.7 % | 116 / 445 / 711 | 1.98e12 | 504 |
| L D breached at 101.8 m, limit 50 m | 6.2 % | 131 / 368 / 582 | 9.5e11 | 512 |
| M D breached at 101.8 m, limit 150 m | **1.1 %** | 89 / 327 / 464 | 1.2e11 | 513 |
| N D breached at 101.8 m, limit 500 m | **1.0 %** | 103 / 329 / 484 | 1.2e11 | 514 |
| GLO-30 Alps / Everest / Highlands (101.8 m, SPIKE-102) | 5.3 / 1.1 / 2.9 % | 3 / 7 / 3 (p50; DSM lower bounds) | - | 543 (Alps), 804 (Everest) |

- The breach lowers 6.0 % of the cells, p50 10 m and p95 141-167 m (the dams). It costs 20-22 s for 4 routes of 9.4 M cells on 4 threads.
- **Q2, the root cause.** The base holds deep basins (30 %), but pass 1 drains them on its grid. What is left at 101.8 m is made where grids meet:
  - the C1 read of a D8 channel network;
  - the base's relief between pass 1's nodes.
- Conditioning the base at its source (a fluvially consistent base noise) would remove only the second, smaller part, and it is research beyond the 1-2 month horizon.
- The rule that follows: **every grid that adds relief or interpolates a carved surface conditions its drainage**, by the same breach and keep-pits rule:
  - pass 1 does already;
  - pass 2 does not;
  - the runtime adds only shallow creases.
- Limits:
  - One mountain tile, one seed, the core flooded open at its border; pass 2 itself would breach on its window with the 128-cell halo.
  - Hills and plains tiles are not run. Their dams are lower, so the 50 m limit may suffice there.
  - The breached input is not carried through pass 2's carve and thermal: the stored result would move a little.

## 4. ADRs 0101, 0105, 0109 and the chain as one design

| ADR | Verdict | Reason |
|---|---|---|
| 0101 keep depressions, fill-depth channel | keep, unchanged | the strongest relief lever measured (SPIKE-102: 0.85 of Everest's 3 km relief); the channel carries the lake basins |
| 0105 point 1, deposition in pass 1 | defer (after M9) | does not fill pits (G = 1 and 4: 12.2-12.9 %); +4 % pass-1 time and +1.2-1.9 GB on a pass already over 8 GB; ADR 0109 already took the pit role from it. Valley-floor flatness and fans have no gate yet |
| 0105 point 2, arid sinks | keep | needs the residual basins; applies to the fill-depth channel of either pass |
| 0109 infill before pass 1 | replace by a breach in pass 2 | wrong grid: pass 1 already drains its own (0.2 % on this tile); the infill lifts basins by up to 1.7 km and leaves 8.3-9.8 % of land in pits at 101.8 m (explorer) |

**The chain (proposal):**
1. Base: class source, bands, `e0`. If D-C is taken, the band and crease noise turns its lattice per octave.
2. Pass 1 (814 m): as today. Breach 500 m, route over the filled copy, carve the unfilled heights (ADR 0101), K, U and runoff (ADRs 0102-0104); 20 steps.
3. Hand-over: pass 2 reads the base on the runtime's lattice (D-B) plus the C1 pass-1 change.
4. Pass 2 (101.8 m):
   - **breach first**: pass 1's rule, limit `breach2_max_m`, on the window with its halo, each route of the local carve;
   - the local carve over the filled copy, incise only, with the pits kept;
   - thermal, the deposition sweep, the one-cell pit spreading.
5. Residual basins: the fill depth (the lake channel), arid sinks (ADR 0105 point 2). The runtime shows them filled until lake water exists (F8 pit tolerance, SPIKE-102).
6. Runtime: the layer read as today. The creases are kept; with D-C their noise loses the lattice lines.

**Earth targets (the check on the I4b preview):**

| Measure | Target | Source |
|---|---|---|
| Land in residual basins, per class | 1-5 % in mountains, about 3 % in hills (GLO-30 at 101.8 m); 2-3 % of all land as lakes | SPIKE-102 §6; HydroLAKES (ADR 0109) |
| Basin depth p50 per class | GLO-30's 3-7 m are lower bounds (lake surfaces are flat in a DSM): report, no gate yet | SPIKE-102 §6 |
| 3 km relief p50 | at least 0.9 of the keep-pits surface (D, 508 m here); high mountains against Everest's 804 m, mountains against the Alps' 543 m | SPIKE-101, SPIKE-102 |
| Mass | breach lowers only (no volume check) | - |

**Earth-specific values (ADR 0108 point 4) of the proposal:**

| Name | Default | Unit | Kind | Parameter |
|---|---|---|---|---|
| `breach2_max_m` | 500 (one rule with pass 1; 150 reaches the same on this tile) | m | calibration | `Pass2Params.carve.breach_max_m` (exists) |
| `breach_slope` | 1e-3 | 1 | method | `CarveParams.breach_slope` (exists) |
| Pass 2's base lattice | the store's (16,384 a face edge at Earth's radius) | cells | method | the bake shape, not Earth state |

## 5. Cost and memory

Machine state: the n0 run was on 2026-11-07 before the owner's restart (indicative, B = 10).

| Item | Today | Recommended chain | ADR 0109 + 0105 point 1 chain (as accepted) |
|---|---|---|---|
| Pass 1 time (6 faces, B = 10) | 1,236 s a face, 2.06 h (target 1.5 h) | 2.06 h | 2.1-2.3 h (infill: one flood a face; deposition +4 %) |
| Pass 1 peak | 10.0 GB (CON-21: 8 GB) | 10.0 GB; about 8.1 GB with the diet below (estimate) | 10.0 + 1.85 transient (infill) + 1.2-1.9 GB (deposition arrays) |
| Pass 2 a tile | 157 s on 10 threads (mountains, today) | + one breach sweep each route (about 1 s) and the base every 6 cells (-55 % of the base samples against every 4): about -10 % to +2 % (estimate) | as today |
| Pass 2 peak | under 2 GB (window 11 M cells) | same | same |

- Pass-1 array diet (estimates from the code, `bake/face.rs`, `carve.rs`, `route.rs`; to be measured in P2):
  - the base copy `z0` as f32: -0.6 GB;
  - the carve factors as f32: -0.6 GB;
  - the MFD area as f32 or u16 log code: -0.6 GB;
  - about -1.9 GB in total.
- Time to 1.5 h: needs -27 %. Not a chain choice. P4 re-measures on the restarted machine before options (fewer re-routes, B) are weighed.
- The planet pass-2 projection is open. SPIKE-98's 65 s a tile was single-thread with today's base stride and no rock fields; this tile took 157 s on 10 threads with every driver. P4 needs a single-thread figure on 3 classes before the 9.6 h terrain share can be checked. It is a risk to the bake date, independent of this chain.

## 6. Owner decisions and the change list

**D-A. How pass 2's basins are treated** (recommended: a)
| Option | Effect | Trade-off |
|---|---|---|
| a. Breach in pass 2 (pass 1's rule, limit 500 m), ADR 0109 replaced, ADR 0105 point 1 deferred | pits 14.7 -> 1.0 %, relief kept, residual basins are lakes | the code of #1223 (infill and deposition) is parked, not merged; 0.5-1 lane-day |
| b. ADR 0109 as accepted (infill at 814 m) | 8.3-9.8 % of land in pits at 101.8 m; basins lifted up to 1.7 km | does not reach the hand-over dams; memory over 8 GB |
| c. Carry pass 1's network into pass 2 (rasterise the D8 links at 101.8 m before the read) | keeps pass 1's channel courses exactly | more code (1.5-2 lane-days); the base's sub-node dams (K: 12.7 %, p50 35 m) stay unless a breach follows anyway |

**D-B. One base for pass 2 and the runtime** (recommended: a)
| Option | Effect | Trade-off |
|---|---|---|
| a. Pass 2 samples the base on the store's lattice (every 6 cells) with the runtime's C1 read | no base mismatch (today rms 12 m, p99 50 m, max 209 m); fewer samples | pass 2 carves no relief between 407 and 611 m that the runtime would not show anyway |
| b. As today | the carve and the runtime see different bases | ghost valleys and dams of up to about 200 m in mountains |

**D-C. The generator's noise** (recommended: a, before the bake)
| Option | Effect | Trade-off |
|---|---|---|
| a. Turned lattice per octave (or gradient noise) for the hill and mountain bands and the creases | removes the lattice lines at every scale; the blobs soften | every base key and the class amplitudes move: a re-tune of the band factors (SPIKE-101) and the scorecard, 2-3 lane-days, parallel to D-A |
| b. Creases only (runtime) | runtime lines gone; the band keeps them at 1-16 km (partly eroded) | 0.5 lane-day, no bake impact |
| c. Keep | as today | the owner's "artificial" look stays at low altitude |

**D-D. Default base read C1 (#1137)**: yes, recommended. The 3113 squares are the v1 read; the layer path already reads C1.

**Change list (if a, a, a, yes):**
1. `pass2-breach` (explorer): breach in pass 2's local carve on the window; `PASS2_KERNEL_VERSION` bump; gate on 3 tiles (mountains, hills, plains) by section 4's targets.
2. `pass2-base-lattice` (explorer): pass 2's base on the store's lattice with the runtime's read; keys bump.
3. Park #1223 (infill and pass-1 deposition), recorded as ADR 0109 superseded and ADR 0105 point 1 deferred (planner, ADR 0078).
4. `turned-noise` (crusader): turned lattices for the bands and the creases; class amplitudes re-tuned on the scorecard.
5. `pass1-memory-diet` (explorer): f32 arrays, measured against 8 GB in P2.
6. #1137 merged.

**Effect on the M9.1 dates** (estimates, not commitments):
- Items 1, 2, 5: about 2-3 explorer lane-days. They replace D2's remaining infill and deposition gate work (about 1-2 lane-days), so the net is about +1 day.
- Item 4: 2-3 crusader lane-days, in parallel.
- The 2026-11-20 to 11-25 window moves by about 1-2 days. The pass-2 projection (section 5) is the larger risk to it.

## 7. What this does not show
- Hills and plains tiles, and a second seed; the planet-wide share of residual basins.
- Pass 2 run in full with the breach: the stored result, thermal's response and the seams at tile edges.
- The straight lines' lattice origin is inferred from the code and the pictures, not isolated by a probe of the noise alone.
- Product-path stills of M9 at 143 m (no full store exists yet; I4b's integrated preview should include a 150 m pose for the crease look).
