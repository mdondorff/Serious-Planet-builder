## Context
- SPIKE-105 §12 F5 (exact quad areas sum to the face area to 3e-14; weighted routing conserves the true area to 1.5e-11) and §13.6 (week 1, A12).
- The face is the equiangular cube face (`TangentWarp`): plane coordinate `x = tan(pi/4 s)`, direction `(x, y, 1)` normalised in the face frame.

## Decisions
1. Area: the cell `[x0, x1] x [y0, y1]` has the solid angle `G(x1, y1) - G(x0, y1) - G(x1, y0) + G(x0, y0)`, `G(x, y) = atan(x y / sqrt(1 + x^2 + y^2))`, times `R^2`. Tables of the edge tangents per column and row (`Arc<[f64]>`); no per-cell storage.
2. Distance: the great circle between the centre directions, `2 R asin(chord / 2)`; the centre tangents are tabulated per column and row.
3. Halo: the window's cell 0 is the plane cell `-halo`; the plane continues past the face's edge with the same mapping, as the base sampling of pass 1 already does (`halo_direction`).
4. Default off, key byte only when on: nothing moves until the prototype turns it on (reversible). The kernel version is not bumped for that reason; turning the switch on as a default is its own version bump.
5. Not in this PR: pass 2 (a tile window is a patch of one face at 102 m: the same metric applies, `Metric::cube_face` at the pass-2 resolution with the tile's offset; held until pass 1's effect is read), the deposition sweep and thermal relaxation of pass 2 (`deposit.rs`, `thermal.rs`, which keep `dx^2`), H0 across cube edges (GEN-139).

6. Area codes (GEN-110): the stored area channel keeps dividing by the constant `dx^2`; with the metric on a code is the true catchment area in units of `dx^2`, the same physical scale everywhere on the face (a per-cell divisor would put the corner bias back). Readers of the channel see codes about 1.8 lower on average (8 steps per octave of 0.85) than with the uniform metric.

## Tuning note for `K dt` (areas shrink about 15 %)
- The stream-power number is `f = k_dt K sqrt(Q) / d`. With the metric `sqrt(Q)` falls by about `sqrt(0.849) = 0.92` on average (0.84 in the corners) and `d` falls by 0.94 at the corners, so the mean `f` falls by about 8 % at the centre and little at the corners.
- To keep today's mean incision with the metric on, scale `k_dt` by `1 / 0.92 = 1.085`; the prototype decides whether to compensate (the tuned default of today is the measured reference). This PR changes no default.
- The uplift per step does not scale with the area, so the transient carve sees less incision with the metric on unless `k_dt` is compensated.

## Earth-specific values (ADR 0108 point 4)
| Name | Value | Unit | Kind | Parameter |
|---|---|---|---|---|
| planet radius | 6,371,000 | m | planet state | `FaceSpec.radius_m` (the world definition's radius) |
| face mapping | `TangentWarp` (equiangular) | - | law of the cube mapping, not Earth | `FaceMapping` |
No calibration constant is introduced.
