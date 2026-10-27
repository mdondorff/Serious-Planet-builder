Opened: 2026-10-27
Question: Is there an uplift for the coupled run that keeps the input's relief while the drainage network reorganises (a smoothed hillslope-scale uplift, or a relaxation target), with erosion that still depends on drainage area?
Why it matters: SPIKE-45 showed the stationary uplift (GEN-063) is unstable under channel capture and that capping the area removes the instability by removing the area dependence; M9 wants valleys in the coarse run (ADR 0080 point 4 turned erosion off).
Time box: 1 day of agent work
Success criteria: after 80 steps on the real 16384 bake input, median run minus input within about 60 m in every input band, with the median lowering rising with the drainage area
Fallback if it fails: erosion stays off (GEN-065, as built)
Code location: `crates/app/examples/coupled_uplift_variants.rs` on branch `spike/m9-uplift` (local, not pushed; `spacing_m` made public in `generators/coupled.rs`, with SPIKE-45's `area_cap_m2`); copy in `proto/`
Closed: 2026-10-27

## Method
- Input and parameters as SPIKE-45: the 2048-cell run's heights reduced from `C:/DEV/SeriousPlanetData/global16384` (read-only), `K` 2e-5, `m` 0.45, `dt` 1e5 years, jitter 0.4; 7,370,117 land cells; 80 steps per variant, one run each.
- Variants (the public `coupled::step` with the uplift array replaced between steps where stated):
  - **cap5e7**: SPIKE-45's fix (area capped at 5e7 m2 in the step and in the uplift), the reference.
  - **hill-scale U**: `U` from `stationary_uplift` with the area capped at 5e7 m2, the steps with the true area.
  - **relax T**: `U = (input - z) / T` every step, `T` = 5e5, 1e6, 3e6 years (the uplift follows the heights, not the old network).
  - **depth cap**: no uplift; after every step a land cell is at least `input - 3 m (A / 1e8)^0.5` (at most 100 or 300 m) high (GEN-057's law, for every cell).
  - **followU**: every step `U = K A^m (input drop to the receiver) / distance` over the receivers and areas of the current surface, so the input is an exact fixed point of the step under any network.
- Reported: median (p10, p90) of run minus input by input band (as SPIKE-45); median of run minus input by the final drainage area class (the area dependence); receivers that differ between step 1 and step 80; cells over 1e11 m2 (the trunks). Raw output in `raw/`.

## Results (step 80, median of run minus input in metres, p10 and p90 in the raw file)
| Variant | 0-500 | 500-1000 | 1000-2500 | 2500-5000 | >5000 | Median by area class (m): <1e8 / 1e8-1e9 / 1e9-1e10 / 1e10-1e11 / >1e11 | Receivers changed |
|---|---|---|---|---|---|---|---|
| cap5e7 (SPIKE-45) | +0.5 | +4.7 | -19 | +57 | +241 | +1.1 / +0.4 / +0.2 / +0.4 / +12.9 | 40.2 % |
| hill-scale U, true A | -118 | -703 | -1,271 | -2,796 | -4,643 | -316 / -333 / -333 / -302 / -306 | 55.7 % |
| relax 5e5 years | -60 | -202 | -521 | -1,120 | -1,453 | -109 / -134 / -147 / -159 / -206 | 57.0 % |
| relax 1e6 years | -90 | -370 | -830 | -1,826 | -2,661 | -196 / -222 / -217 / -211 / -251 | 58.8 % |
| relax 3e6 years | -120 | -594 | -1,163 | -2,600 | -4,183 | -288 / -307 / -278 / -253 / -282 | 59.8 % |
| depth cap, max 100 m | -1.4 | -1.4 | -1.4 | -1.4 | -1.3 | -1.4 / -4.6 / -14.6 / -39.4 / -100.0 | 29.5 % |
| depth cap, max 300 m | -1.4 | -1.4 | -1.4 | -1.4 | -1.3 | -1.4 / -4.7 / -14.6 / -39.5 / -120.6 | 29.8 % |
| followU | +16 | +178 | +257 | +708 | +1,519 | +55 / +39 / +35 / +56 / +47 | 30.7 % |

- Time: 207 to 260 s per variant for 80 steps (213 s for cap5e7); followU 354 s (two floods per step).
- The run settles in every variant but the depth cap (last step mean |dz| 0.07 to 1.5 m); the depth cap's last step moves 32 to 35 m on average: the step lowers and the cap puts back (see finding 4).

## Findings
1. **A smoothed or hillslope-scale uplift does not work** (hill-scale U): with the true area in the erosion and `U` at hillslope scale the trunks (A over 1e11 m2) are cut down to their receivers and the basins follow, as in SPIKE-45's unmodified run: -118 to -4,643 m, no area dependence (the median lowering is 300 to 333 m in every area class, the damage is everywhere). The uplift has to match the erosion where the erosion is large, and a smoothed `U` cannot.
2. **A relaxation to the input does not hold the relief**: every `T` of 5e5 to 3e6 years loses 60 to 4,183 m (the loss rises with `T`), and the lowering is nearly the same in every area class (109 to 206 m at 5e5): the erosion's pull on the trunks (f up to 1e5) is a single step's, a relaxation of `dt / T` = 0.03 to 0.2 per step cannot restore it. `T` must be about `dt` (a full pin to the input per step) to hold, which is erosion off.
3. **followU (the input an exact fixed point of the current network) rises instead of falling**: +16 to +1,519 m, 30.7 % receivers changed. The input is a fixed point only where the filled surface and the input agree; where the flood fills a pit, or the first step lifts the coast to the 0.5 m floor, a cell's receiver in the current surface is not the input's, `U` is computed from the input's drop to a receiver that differs from the input's own, and the step raises the cell; the rise raises its donors (no cap on the donors' `f`). The failure is the mirror image of SPIKE-45's.
4. **Only a bounded incision holds the relief and keeps the area dependence**: the depth cap keeps the median within 1.4 m of the input in every band and the median lowering rises with the area (1.4, 4.6, 14.6, 39.4, 100 m for the five classes: GEN-057's law, 100 m at the cap), with 29.5 % of receivers changed (the network reorganises). It is not an erosion equilibrium: the final heights are the input minus the depth law of the final area (every step the implicit step lowers cells far under the cap, and the cap puts them back: 32 to 35 m of mean change per step). The run is a carve of a fixed depth by area, with the network re-routed over the carved surface.
5. **The area cap of SPIKE-45 is the other stable one and has no area dependence** (median lowering +1.1 to +12.9 m by class: the trunks rise 13 m).
6. **No variant gives an erosion that is both stationary under a changing network and area dependent**: the instability of SPIKE-45 is of the step itself (a huge `f` on a trunk takes the cell to its receiver in one step whatever the uplift); the uplift only chooses where it falls.

## Recommendation
- ADR 0080 point 4 (erosion off, GEN-065) stays the baseline. For valleys in the coarse run (M9) the only tested stable rule is the depth law: lower each cell to at most `d(A)` under the filled input, by the area of the filled surface's network. The depth law is GEN-057's: 3 m (A / 1e8 m2)^0.5 at most 100 m (about 49,000 cells over 1e11 m2, their median at the cap).
- Untested, to try before an OpenSpec change: the same carve in one pass (fill, accumulate, `z = input - d(A)`, then one re-route and accumulate) instead of 80 steps: the final surface of the depth-cap variant is that carve plus the network it reroutes, so it should give the same ground for about 1/80 of the time (one step of the run is 2.7 s of the 213 s).
- Not recommended: smoothing or relaxing the uplift (findings 1 and 2), and a current-network stationary uplift (finding 3). If an erosion equilibrium is wanted the step itself must change (cap `f`, like the chunk stage's `tributary_f_max`, or an implicit solver that keeps the receiver's `z` from the old step); not tried.
- Limits: one run per variant, one cap law (3 m, exponent 0.5); the depth cap's by-area medians depend on that law; heights of the run, not of the drawn ground.

## Visuals
Nothing to look at: tables of heights by band and area class only, no renders.
