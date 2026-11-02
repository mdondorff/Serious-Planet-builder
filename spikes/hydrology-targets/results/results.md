Opened: 2026-10-25
Question: What measurable targets should M8's rivers meet, and how far is the world from them today?
Why it matters: Owner direction (2026-10-25): do hydrology right in M8; GEN-057's depth law (3 m (A/1e8)^0.5, cap 100 m) was chosen ad hoc.
Time box: about 4 h of agent work
Success criteria: one table of measure, real range (cited), ours (measured, read-only), proposed M8 target, and the parameter that moves it
Fallback if it fails: literature ranges only, plus the measures that could be taken
Code location: spikes/hydrology-targets/ (throwaway; not part of the main crates)
Closed: 2026-10-25 (used about 2 h)

## Method
- Literature: standard papers, from memory; four claims re-checked by web search on 2026-10-25 are marked **v**, the rest are recalled ranges marked **r** (check before they become spec numbers).
- Ours, read-only on `C:/DEV/SeriousPlanetData/global16384`:
  - the stored coupled run on its 2048 lattice (4.9 km cells, 30 % land), receivers and areas as the engine derives them;
  - the A-net solve (8.04 m cells, 608 x 608 core, 3 km halo) of the chunk at each review site: hills site (-37.689, 3.140; corridor area 1.2e10 to 1.3e10 m2) and lowland site (-11.166, 23.049; corridor area 9.45e12 m2).
- Raw output: `measurements.txt`, `run_vs_base.txt`. Tool: `proto/hydrospike.rs` and `proto/hydro_targets.rs` on branch `spike/hydrology-targets` (removed from main 2026-10-30; copy into `crates/app/src/` and `crates/app/examples/`, add `pub mod hydrospike;`; not in the crates). Runs: coupled report 34 s, both chunks 2 min, CPU only.

## Table
| Measure | Real range | Ours today | Proposed M8 target | What moves it |
|---|---|---|---|---|
| Bankfull width w(A), Q about 0.01 A m3/s (A km2) | w = 7.2 Q^0.5 (Moody and Troutman 2002, **r**): 70 m at 1e4 km2, 0.7 km at 1e6, 2.2 km at 9.5e6; scatter factor 2; exponent 0.45 to 0.55 (Leopold and Maddock 1953, **r**) | no bankfull channel; the cut is 64 m wide at half depth at 9.5e12 m2 | within a factor 2 of 0.7 A_km2^0.5 m; below one cell (8 m) the width is a sub-cell feature | a new law beside GEN-057, a function of A |
| Bankfull depth d(A) | d = 0.27 Q^0.3 (same source, **r**): 1 m at 1e4 km2, 4 m at 1e6, 9 m at 9.5e6; factor 2 | none (the cut is valley incision) | 1 to 10 m over 1e4 to 1e7 km2 | the same law |
| Valley incision below the surrounding rim | lowland alluvial 5 to 50 m; mountain gorges 100 m to over 1 km; not a function of A alone (**r**) | hills site 42 m median (p10 30, p90 57) at 1.3e10 m2; lowland site 98 m at 9.5e12 m2 (the cap) | a depth law with a relief factor; at 9.5e6 km2 in lowland at most 30 m | `depth_m`, `exponent`, `cap_m` of GEN-057; a relief factor is new |
| Valley width at half depth | alluvial valley floor 5 to 20 bankfull widths (**r**, the weakest number here): 0.3 to 2 km at 1e4 km2, 10 to 50 km at 1e6 | 88 m (p10 32, p90 177) at 1.3e10 m2; 64 m (p10 34, p90 161) at 9.5e12 m2; depth over width 0.5 to 1.5 | at least 5 bankfull widths; side slope under 10 degrees on a plain, a gorge only where relief is high | the cut profile across the corridor, the halo, hillslope diffusion (design.md 7) |
| Drainage density Dd | humid 1 to 5 /km, arid 0.1 to 1 /km, depends on the head threshold (**v**: 0.1 to 2.5 /km on one plateau, 5 /km in Egyptian wadis) | coupled: 0.059 /km at heads of 100 km2, 0.022 at 1000 km2, 0.0078 at 10,000 km2; chunk: 0.32 and 0.81 /km, corridor only | with heads of 0.1 to 1 km2: 0.5 to 3 /km (the coarse values are the grid's) | the area at which a cell becomes a channel |
| Tributaries below the corridor | channels start at 1e4 to 1e6 m2 (**r**) | core cells with area >= 1e5 m2: 1717 of 370k (hills), 2266 (lowland, all corridor); none between 3e5 m2 and 1e10 m2 outside the corridor | a network at every scale from the head threshold to the corridor | the stage has none: the frozen coarse network is the only channel |
| Horton bifurcation ratio Rb | 3 to 5, typically 4 to 4.6 (**r**) | 5.0 to 5.07 at all three thresholds | 3.5 to 5 | receiver jitter, coarse threshold |
| Horton length ratio Rl | 1.5 to 3.5 (**r**) | 1.9 to 2.2 | 1.5 to 3.5 (met) | none |
| Hack exponent (L = c A^h; km, km2) | h 0.57 to 0.6, c about 1.4 (**v**: Hack 1957, Gray 1961, Rigon et al. 1996) | basin mouths (1317 basins over 1e4 km2): L = 3.0 A^0.484; main-stem cells of basins over 1e5 km2: L = 0.449 A^0.615; at 1e4 km2: 260 and 129 km against 351 | h 0.55 to 0.62, L within 25 % of 1.4 A^0.6 from 1e3 to 1e6 km2 | sinuosity (below), receiver jitter |
| Concavity theta | 0.4 to 0.7, reference 0.45 (**v**; Whipple and Tucker 1999: 0.35 to 0.6) | 0.444 in the coupled run (the model's m/n; trivial) | 0.4 to 0.6 | stream-power m/n |
| Channel slope S(A) | S = ks A^-theta, ks 20 to 300 (m^0.9, **r**): 2e-3 to 3e-2 at 1e9 m2, 1e-3 to 6e-3 at 1.3e10 m2 | coupled: 3.9e-3 at 1e9 m2, 1.4e-3 at 1e10, 1.5e-4 at 1e12 (low end of the range); chunk bed: median slope 0 over 400 m windows, p90 0.064 | no flat reach over 400 m, no step over 5 m on a river over 1e10 m2 | anchor spacing (8 segments), breach rule, profile smoothing |
| Chunk bed profile | monotone and concave | a staircase: flat reaches at 3229.3, 3210.0, 3175.2, 3147.1 m, 400 to 1500 m long, steps 19 to 35 m (hills); the lowland core holds 60 cells (517 m) of one flat bed at 184.5 m | concave monotone bed | the anchors are constant over a coarse quarter segment; the bed between anchors needs a profile (design.md 17) |
| Sinuosity (path over chord) | straight under 1.05, sinuous 1.05 to 1.5, meandering over 1.5 (**v**); mountain 1.05 to 1.3, large alluvial 1.3 to 2 (**r**) | coupled main stems on D8: p50 1.07 (p10 1.03, p90 1.13); chunk corridor at 1 km windows p50 1.26 (p10 1.16, p90 1.40), at 4 km 1.3 to 1.6 | lowland rivers over 1e11 m2: 1.3 to 2 at 10 km windows; mountain 1.05 to 1.3 | receiver jitter (SPIKE-35), corridor geometry |

## What the numbers say
- The network statistics of the coupled run (Rb, Rl, Hack h, concavity) are inside or at the edge of the real ranges. Hack's L is 40 to 70 % of real at large A because the stems are too straight (sinuosity 1.07).
- The failures are in the chunk, in the cross-section and the profile:
  - one channel only, no tributaries between 1e6 m2 and the corridor;
  - the large river at the lowland site is a slot 98 m deep and 64 m wide (real: a channel about 2 km wide and 9 m deep in a wide plain);
  - the bed is a staircase of flat reaches;
  - the depth law has no relief term, so the cap decides every large river.
- GEN-057 (3 m (A/1e8)^0.5, cap 100 m) fits the one point where the corridor is a small river (law 40 m, median cut 48 m at 1.3e10 m2). It treats valley incision as a function of A; the literature has two laws, channel size from discharge and valley form from relief.

## Limits
- Valley width and incision ranges, ks and every number marked **r** are recalled, not fetched. The web search confirmed the Hack form (1.4 A^0.6, h 0.57 to 0.6), theta 0.45 with 0.4 to 0.7 observed, and the sinuosity classes; it did not confirm the Moody and Troutman coefficients.
- Two chunks, each with a single corridor. The 4.9 km network statistics are grid-bound (D8, jittered); the coupled sinuosity of 1.07 is a lattice effect as much as a model one.
- Q = 0.01 A m3/s is the global mean runoff (about 1.2e6 m3/s over 1.3e8 km2); real basins range 0.005 to 0.03.
- The coupled run's heights are compressed against the base (`run_vs_base.txt`: median -164 m under the river cells at 500 to 1000 m ground, -2.7 km at 2.5 to 5 km), so the slopes above are the run's, not the drawn ground's.

## Visuals
Nothing to look at: tables of statistics only, no renders.
