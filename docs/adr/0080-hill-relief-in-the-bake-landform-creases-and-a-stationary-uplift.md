# 0080 Hill-scale relief: a band in the bake, landform-keyed creases, and a stationary uplift for the coupled run
Status: proposed (sonnet-explorer, 2026-10-25; the architect's review and the owner's acceptance pending; easy to reverse, implementation may build on it)
Date: 2026-10-25
Source: owner, 2026-10-25 (terrain relief into M8; scope: plausible relief, hydrology and horizon shadows); `openspec/changes/m8-relief/proposal.md`; learnings 2026-10-25; ADR 0022, 0026, 0030, 0076; CON-06, CON-07, CON-14

## Context
- The base field is smooth at hill scale: the 99th percentile of the 2 km slope is 2.4 degrees or less in every ground band at the 16384 bake.
- The coupled run flattens relief against the base (median run minus base over river cells: -933 m at 1000 to 2500 m ground, -2686 m at 2500 to 5000 m) because its constant uplift of 1e-3 m/year balances to a slope of `U / (K A^m)`, 0.16 % for a 10,000 km2 river.
- The erosion filter (ADR 0030 point 1) keys its crease amplitude to the bake's slope, which is small everywhere, so wavelengths of 0.5 to 5 km carry little.
- ADR 0076 point 6 puts the stream-power routing before the driver maps; no driver map exists yet.

## Decision
1. **Hill band in the bake.** The bake gains a ridged-multifractal band for wavelengths of 1.5 to 8 km (at least 2.5 bake cells), its amplitude set by the landform driver (point 3); the coupled run, the coarse horizon and the hydrology read it.
2. **Landform-keyed creases.** `cpu-erosion-v1`'s crease amplitude (wavelengths under 1.5 km) follows `ruggedness` as well as the bake's slope; the slope factor stays a multiplier above a floor that rises with `ruggedness`. This amends ADR 0030 point 1.
3. **The landform driver** `global::landform(seed, direction)` in `[0, 1]` is the range-zone strength times the distance from the coast, a pure function of the direction that does not read the band (amended 2026-10-25, crusader, planner's decision: the bake is a pointwise function of `(seed, direction)` at any resolution, so the neighbourhood range first proposed would need a stencil of about 25 pre-band evaluations per sample, about 4 times the bake time, or a global coarse lattice with a cache). It is not stored (ADR 0026 keeps four channels); the field reads it with the seed already in its inputs hash, so ADR 0022 option 1 stands. The erosion field reads it from a fixed lattice of 256 cells a cube-face edge, interpolated bilinearly (amended 2026-10-25, planner's condition: evaluating the driver at every height cost +58 % per height, the lattice +3 to 4 %); the lattice differs from the driver by under 0.012.
4. **The run keeps the input's relief (amended 2026-10-25, planner's decision after the rebuild).** The coupled run's heights are the input plus the priority-flood fill; drainage, receivers and areas are those of that surface; no stream-power erosion at M8's default (`CoupledParams::erode` false for a world's run, part of the key; the erosion code stays for M9's calibration). First proposed: a stationary uplift `rho K A_in^m S_in` so that `rho` 1 makes the input a steady state. It held on a smooth synthetic world and did not hold on the real bake: after the rebuild the run minus its input was -104, -675, -1186, -2630 and -4471 m (median, by input band 0-500, 500-1000, 1000-2500, 2500-5000, over 5000 m) after 80 steps, 0 after 2 steps, saturating near step 24, and step 1 raised 4.24 million of 7.37 million land cells by more than 1 m (the mechanism is unexplained, learnings 2026-10-25). The stationary uplift (GEN-063) stays for the erosion option.
5. **Bounds.** The field's and the bake's maximum absolute height rise by the band's largest amplitude; every stage that reads them (GEN-026, GEN-039) follows.

## Consequences
- Slope percentiles per ground band rise to the proposal's provisional guard rails; run minus base falls to within +-100 m at river cells.
- The bake's version and the field's version bump: one shared rebuild of bake, run, coarse horizon and chunks.
- Easy to reverse: the band's amplitude and `rho` are parameters; `rho` 1 with a zero band amplitude is today's field except for the uplift.

## Licences of adopted code or techniques
None.
