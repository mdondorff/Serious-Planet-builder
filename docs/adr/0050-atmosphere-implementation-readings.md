# 0050 Atmosphere: implementation readings of ADR 0040
Status: proposed (sonnet-explorer, M5 item 5, 2026-10-17; the code of `atmosphere-reference` and `atmosphere-passes` is built on it; every point is reversible: a table layout or step count change is a code change with new known answers)
Date: 2026-10-17
Source: ADR 0040 points 2-5; ADR 0035; ADR 0043; ADR 0048; SPIKE-09; the numerics and spec reviews of `atmosphere-reference`

## Context
ADR 0040 fixes the model (Hillaire 2020), the table sizes, the CPU reference and the bound. Building it needed decisions the text leaves open; the spec-guardian asked for them to be recorded.

## Decision
1. **The transmittance table stores optical depth** (RGB), not transmittance; a lookup interpolates `tau` and exponentiates. The worst error against the converged integral is 2.2 % of T against a 3 % bound (ATMO-007); interpolating T would be worse where it falls by orders of magnitude.
2. **The planet's shadow is a smoothstep** of the sun's cosine across `mu_horizon +- w R / r` (`w` the sun's angular radius, look data), not Hillaire's hard test. A hard test makes a sample jump when a camera moves a millimetre at sunset (CON-11 v1.4 temporal stability) and makes the terminator a line; the smoothstep is not an exact disc-coverage formula.
3. **Sample counts and placement** (the oracle's contract, ADR 0040 point 3): sky view 32 steps with boundaries `s_end (i/32)^2`; aerial-perspective slice `k` at `256 km ((k+1)/32)^2` with `min(2(k+1), 64)` uniform steps per texel and rays that end at the depth, not at the reference sphere; orbit march 40 uniform steps from the atmosphere's entry point; multiple-scattering table 8 x 8 uniform directions and 40 steps, the factor `1/(1 - f_ms)` capped at 1000; transmittance table 512 steps in f64. Sky-view mapping: `theta = theta_h (1 - (1 - 2v)^2)` above the horizon, `theta_h + beta (2v - 1)^2` below, azimuth `phi = pi u^2` from the sun, read back with `atan2(|dir . e2|, dir . e1)`. Texel `i` of an axis of `N` sits at `i / (N - 1)`.
4. **The atmosphere is opt-in per plan** (`LitPlan::with_atmosphere`): the body radius is world data and the plan builder has it, the look file does not. A plan without one is the lit path of items 2-4 unchanged.
5. **The frame targets**: with an atmosphere the terrain draws into a second `Rgba16Float` scene target and the composite writes the lit path's HDR target (8 B per pixel more: 20 B per pixel in all, inside ADR 0048's 40 B + 32 MiB); the static tables (`Rgba32Float`, 0.28 MB) and the two products (`Rgba16Float`, 0.43 MB) are counted by the graph.
6. **REND-005 is scoped to geometry shaders.** The atmosphere shaders hold the body radius as a value in their uniform block (decided in f64) as CON-11 v1.4 allows presentation shading; they are scanned for planet-scale literals but are exempt from the radius-uniform rule, and their bound is ATMO-001/ATMO-010.

7. **Limb-stable discriminants in the orbit march.** At 20,000 km `(r mu)^2 - c` carries 1e8 m^2 of f32 error, which at 1e-6 rad from the tangent moves the ground distance by kilometres. The march forms `(r mu - q)(r mu + q)` and `(r mu - rho)(r mu + rho)` with `r`, `rho` and `q = sqrt(c_top)` sent as a high and a low f32 and `r mu` split with `fma`; `r mu + q` is then exact near the tangent. Measured against the f64 reference on f32-rounded inputs the worst orbit ray is 0.92 of the 1 % + 1e-4 bound on the software adapter (whose `fma` is not fused) and 0.02 on the RTX. The sun disc's angle comes from the chord between the view and sun vectors, and the planet's penumbra width is `w R / r`.
8. **The scene target is 8 B per pixel** (`hdr-scene`, point 5): the composite cannot read and write one target, and a blend cannot apply a per-channel transmittance to the destination and add the in-scatter in one pass.

## Consequences
- ADR 0040 stands; this ADR is the normative home of what its text leaves to the code.
- Moving to Bruneton 2008 (the ADR's fallback) would replace points 1 and 3.

## Licences of adopted code or techniques
Hillaire 2020 (technique, no code adopted). Cornette-Shanks phase function: a published formula.
