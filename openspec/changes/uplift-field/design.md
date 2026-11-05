## Context
- `carve.rs`: a cell moves to `(z + up + f z_r) / (1 + f)`, `up = u_over_k * k_dt * factor * taper` (1 m a step at the default 20, 20 steps), `taper` the smoothstep of the starting height above the sea over `uplift_taper_m` (60 m). The class carve factor scales `f` and `up` together, so `U / K` is constant.
- Pass 2 (tile) has no uplift (incise only): only the face pass reads the field.
- `lithology-k` (merged, #1200) has the province lattice: 256² nodes a face, `u8` terms craton, core, foreland, basin, rift, old_margin, read bilinearly, built from `ClassSource::eval`.

## Decisions
1. **Field.** `U(d)` is the uplift per step in units of `k_dt` (the ratio `U / K` at K 1), from the province terms and the class source's back-arc term:
   - `U = max(U_FLAT (1 - craton) + U_CRATON craton, U_BELT core, U_BACKARC back_arc, U_RIFT rift)`, then times `1 + 0.2 noise` (`fbm`, frequency 3, two octaves);
   - start values: U_CRATON 2, U_FLAT 24, U_RIFT 28, U_BACKARC 32, U_BELT 70, chosen so that the land mean (by area, seed 14) is near today's 20 and the default relief is comparable: plains keep about today's uplift, belts rise 3 times as fast, cratons a tenth. The preview prints the mean, and the owner tunes.
   - `core` already holds the coast arcs (the class source's coastal term), so arc coasts are positive without a new term; collision belts and arcs share `U_BELT` (the source has no type term).
2. **Back-arc.** The lattice gains a seventh term `back_arc` (the class source's); `LITHOLOGY_VERSION` 2 (no consumer yet). One lattice serves both fields.
3. **Carve interface (the explorer's, plan PR 4; not in this change).** `carve(.., factor, uplift: Option<&[f32]>)`: when given, the per-cell `up = uplift[c]` replaces `u_over_k * factor * taper`; the caller has multiplied `k_dt * U * taper` into the `f32` that replaces the taper array (no memory added on a 1.6e8-cell face). Absent, the carve is bit-for-bit today's. The uplift is not scaled by the factor or by K: with K(x, z) (PR 4) a hard rock steepens and a soft one flattens at the same U.
4. **Keys (the explorer's).** `BakeInputs::id` folds `UPLIFT_VERSION` and `LITHOLOGY_VERSION` (the table is part of the version: a change of a constant bumps it); the layer key follows; the carve params' `u_over_k` stays in the key as the value of a bake without the field. Cold: every face layer of a bake with the field and what builds on them; no read-only store is written. The sea taper (#1173) keeps multiplying `U`. The field is read per face cell as `Uplift::at(&ProvinceLattice, d)` (a bilinear read and one `fbm`, about the cost of `carve_factor`).
5. **Preview (GEN-125).** `generate --uplift-preview DIR [--seed N | --world F] [--km N]` (same options as the lithology preview, `--km` 8 to 500): `uplift.png` (U), `steepness.png` (`U / K` of the lithology at the surface, the settled slope relative to the mean), `section.png` (a great circle from a belt over a back-arc to a craton: U and the base elevation), `uplift.txt` (land mean and quantiles, shares by province, the mean `U / K`). Labels as in GEN-122; the bytes do not depend on the thread count.

## Risks
- The mean `U` and the belt ratio decide the relief; they are start values and change the scorecard (owner's round).
- Lattice resolution (256² a face, about 40 km a node on a 10,000 km face edge) bounds how sharply a belt edge can step; the bilinear read is continuous across face edges (tested for the lithology).
- A bake with the field and K together settles at slopes `U / (K sqrt(A))` that differ from today's by K's spread (0.12 to 1.8): the K PR's preview shows the steepness map first.

## Open for the owner
- Whether arcs and collision belts differ (one `U_BELT` now); whether `U` carries a time term (no).
