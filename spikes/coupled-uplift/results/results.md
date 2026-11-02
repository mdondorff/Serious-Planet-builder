# SPIKE-45: why the stationary uplift does not hold the coupled run on the real bake

Opened: 2026-10-26 (opus-planner, owner asked)
Question: ADR 0080 point 4 turned the coupled run's stream-power erosion off because the stationary uplift (GEN-063, `rho K A_in^m S_in`) failed on the real 16384 bake (80 steps: run minus input -104 to -4,471 m median by band). Bug or mechanism, and is there a cheap fix that keeps the input's relief with erosion on?
Time box: half a day
Success criteria: the cause with evidence; a fix that keeps run minus input small per band after 80 steps, or the reason none is cheap.
Fallback: erosion stays off (GEN-065, as built).
Code location: `crates/app/examples/coupled_uplift_diag.rs` and a 6-line parameter in `generators/coupled.rs`, spike branch `spike/coupled-uplift` only; copies as `coupled_uplift_diag.rs.txt` and `area-cap.patch.txt` here.

## Method
- Input: the 2048-cell run's heights, the box reduction of the 16384 store `C:/SeriousPlanetData/global16384` (read-only), `K` 2e-5, `m` 0.45, `dt` 1e5 years, jitter 0.4, `rho` 1, as the app built it before ADR 0080 point 4.
- 7,370,117 land cells of 25,165,824; 15.7 % of land is in a depression of the flood; 10.1 % has `U = 0`; `U` runs from 1.1e-5 (p10) to 0.14 m/year (max).
- Each variant is 24 or 80 steps; every row is the median (p10, p90) of run minus input by input band. Raw output: `diag-base-h1-h2-trace.txt`.

## Cause: a real mechanism, not a bug
The step, the flood, the receivers and `stationary_uplift` use the same surfaces and the same formula (checked by reading and by the pre-filled case below, which starts at an exact fixed point: step 1 moves cells by at most 0.5 m, the height floor). The failure is dynamical:
1. `U` is a field fixed in space, `K A_in^m S_in` of the input's network. A trunk river has A = 5.4e12 m² (cell 7,199,203 and its chain): `K A^m` is 10.7 per year, so `U` on the trunk is 1e-2 to 4e-2 m/year (1 to 4 km per 1e5-year step), while its hillslope neighbours get 1e-5. The trunk holds its height only through that uplift.
2. Any change of receivers moves the trunk to neighbouring cells. 36 % of land cells change receiver after step 1 (2.66 M of 7.37 M), and A changes by a factor 0.2 to 6.4 (p10 to p90) for cells above 1,000 m. A cell that stops being trunk keeps its huge `U` with a small `f`: at step 2 cell 7,199,203 (A 5.4e12 in the input, 8.5e7 now) goes from 118 m to 3,339 m, and its chain follows (to 2,717, 2,377, 2,175, 1,961, 1,605 ... m).
3. The cells that become trunk have the small `U` of a hillslope and a huge `f` (2e5): they are pulled down to their receiver, and the whole basin upstream follows rigidly. Step 8 to 24: median -1,010 to -4,056 m in the top band, saturating when the network has settled (mean |dz| falls from 36 to 3 m).
4. The first push is small: the height floor lifts 1.24 M coastal cells by up to 0.5 m at step 1, and the flood's fill raises pits (below). Neither is the cause: without the floor (`min_height_m` 1e-4) the drift is the same (-98.6, -635, -1,084, -2,428, -4,046 m at step 24).

| Test (24 steps, median run minus input m) | 0-500 | 500-1000 | 1000-2500 | 2500-5000 | >5000 |
|---|---|---|---|---|---|
| base (as the app did) | -99 | -640 | -1,088 | -2,432 | -4,056 |
| H1 input pre-filled (no depressions) | -73 | -270 | -579 | -1,229 | -1,543 |
| H2 jitter 0 (plain D8) | -98 | -638 | -1,093 | -2,400 | -3,897 |
| H1 + H2 | -73 | -281 | -533 | -1,179 | -1,468 |
| floor 1e-4 m (no 0.5 m lift) | -99 | -636 | -1,084 | -2,428 | -4,046 |
- Step 1 of the base: 4.24 M land cells rise by more than 1 m: 1.07 M are in a depression (the fill), 3.17 M are not in one but their receiver also rose (the donors follow the fill, f is large); none fall. Fill-only (GEN-065) gives 0 median in every band (p90 up to 38 m).
- The fill makes about half of the drift, the rerouting the rest (H1 still drifts to -1,543 m); jitter and the floor are not factors.
- `U` after step 1 over `U` before, cells above 1,000 m: p10 / p50 / p90 0.39 / 0.94 / 1.68.

## Cheap fix tried: cap the area in `K A^m`
`A_eff = min(A, A_cap)` in the step and in `stationary_uplift` (one field in `CoupledParams`, default infinite; 6 changed lines). It removes the 1e7-fold spread of `K A^m` that makes `U` on trunks and hillslopes incomparable. Median run minus input by band (m), base input with its depressions:
| Cap | Steps | 0-500 | 500-1000 | 1000-2500 | 2500-5000 | >5000 |
|---|---|---|---|---|---|---|
| none (as built) | 24 | -99 | -640 | -1,088 | -2,432 | -4,056 |
| 1e9 m² | 24 | -26 | -74 | -334 | -827 | -1,452 |
| 1e8 m² | 80 | -5 | -38 | -180 | -368 | -591 |
| 5e7 m² | 80 | +0.5 | +5 | -19 | +57 | +241 |
| 3e7 m² (about one cell, 2.4e7) | 80 | +6 | +16 | +30 | +232 | +584 |
| 80 steps, no cap (learning of 2026-10-25) | 80 | -104 | -675 | -1,186 | -2,630 | -4,471 |
- 5e7 m² keeps the input's relief: after 80 steps the median is within 0.5 to 57 m in every band but the top (+241 m, 5 %; p10 -171, p90 +1,058 there); the run is settled (mean |dz| 0.4 m, was 36).
- Cost of the fix: a cap of 5e7 m² is about two cell areas, so `K A^m` is nearly constant: the erosion rate depends on slope only and no longer on drainage area. The run then keeps the relief and still fills, and the network changes the heights less than a stream-power law would: it is closer to "erosion off" than to calibrated erosion. Caps of 1e8 and 1e9 m² keep some area dependence and drift (-591 and -1,452 m in the top band).

## Recommendation
- The failure is a real instability of an uplift fixed in space under a drainage that reorganises (channel capture), not a units or surface bug: do not spend more time looking for one.
- If erosion is wanted on in M9, the uplift must not be tied to the old network: smooth the uplift (a hillslope-scale `U`, not `K A^m S` on trunks), or let `U` follow the network by making it the target of a relaxation (so a cell that leaves the trunk gives its uplift up), or cap the area at about 5e7 m² as above and accept that the area dependence is gone. The cap is a 6-line change but keeps relief by removing the drainage coupling, so it buys little over GEN-065.
- Open: whether a smoothed `U` (for example `U = rho K A_cap^m S` with A_cap rising with distance from the divide, or a Gaussian filter of `U`) keeps both the relief and an area-dependent erosion; not tried.

## Visuals
No images: the question is heights by band; the numbers are in the tables.
