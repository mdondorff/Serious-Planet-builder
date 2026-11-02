# 0088 The M9 terrain bake
Status: accepted (owner, 2026-11-01; the planet bake runs only after the owner has approved the engine's whole-planet preview of the classes and base heights; drafted by the spikes session for opus-planner, amended 2026-11-01)
Date: 2026-10-31
Source: SPIKE-61 to SPIKE-83 (`spikes/m9-*/results/results.md`); ADR 0089; ADR 0006, ADR 0016, ADR 0026, ADR 0030, ADR 0045, ADR 0055, ADR 0056, ADR 0086, ADR 0087; M9 draft (`docs/briefings/m9-draft.md`) decision 7; CON-02, CON-07, CON-11, CON-14; GEN-020, GEN-062

## Context
- Hills and mountains are 4-9x too gentle in slope and relief (SPIKE-55); the relief comes from the amplitude of the base field below about 20 km, x4-6 for hills and about x7.5 for the Alps (SPIKE-65, SPIKE-66).
- Stream power run to steady state erases the landform; a transient carve after the amplitude, with a breach before the fill and thermal at repose after it, reaches 0.62-1.13 of real at the hill sites (SPIKE-65, SPIKE-66).
- A relief factor computed per point from a 10 km stencil breaks batch reads (GEN-020) and plan-check, loosens bounds 25-700x and raises level steps by the factor (SPIKE-66).
- Routing is the bake's main cost, cut 2.7-5.5x by Barnes flood, the receivers' order, jittered D8 and periodic re-routing; thermal and deposition cost seconds per tile with an active-cell kernel (SPIKE-61, SPIKE-62, SPIKE-64).
- Disk (decision 7, owner): the erosion result is a separate layer beside `global16384` first, about 7-10 GB (SPIKE-61).

## Decision
1. **Two new channels in the bake**, each with min and max per block: the relief driver `k`, from the class weights of ADR 0089 and tapered to 1 towards the coast band of ADR 0056; and the ring mean `m`, the bake's mean over a ring. Grids, the ring's radius and the class amplitudes are in the generator spec (GEN-097, GEN-099).
2. **Runtime base field:** `m + k (e - m)` plus `k` times the creases, with `e` the bake. Each term is one read at the point, so nothing at runtime reads with a wide stencil.
3. **Bounds per cap** come from the blocks' ranges of `e`, `k` and `m`, the layer's min/max pyramid (point 8) and the crease bounds, nested as LOD-012 requires.
4. **The erosion layer, in two passes** (routing is global, SPIKE-68; SPIKE-70):
   1. **Per cube face on a coarse grid:** the base of point 2 sampled once per cell (SPIKE-71), then a transient carve: implicit stream power with uplift, a fixed `K t`, not to steady state. It breaches before the fill and re-routes periodically (Barnes flood, order from the receivers, jittered D8). Faces do not route across cube edges.
   2. **Per tile of the layer's grid** (SPIKE-61's tiles, a halo, the core stored):
      - the base of point 2 plus the face carve's change, bilinear;
      - a local carve of small catchments that only incises: no fill, no breach, no uplift (SPIKE-72, SPIKE-73);
      - thermal at the angle of repose on active cells;
      - one deposition sweep on the last route.
   The layer stores each tile's change against the base of point 2 (decision 7). Grid sizes, the re-routing period and the catchment limit are in the spec (GEN-100, GEN-101).
5. **The runtime field adds the layer** bilinearly. Below the layer's cells the field's octaves and the shader detail apply (R6).
6. **Determinism:** every kernel is a Jacobi gather or a fixed topological sweep in f64 with `libm` (CON-14), and the result does not depend on the thread count.
7. **Checkpoints and keys:** the bake checkpoints each face carve and each tile, and resumes from the last complete one. The layer is keyed by the bake key, the channel versions and the kernel parameters (ADR 0006).
8. **A min/max pyramid of the layer:** the min and max of each block of the layer, and of each coarser level from four blocks below, stored with the layer and rounded outward. A cap's bounds read a few blocks; below the layer's cells the runtime detail adds its analytic bound; a cap across a face edge reads both faces' blocks (SPIKE-83). The block size and precision are in the spec (STRM-066).
9. **Crates (CON-02):** the bake kernels and the pyramid's builder live in `planet-generators`; the store keeps the layer and the pyramid in `planet-cache` behind a `planet-generators` trait (as `LatticeSource` today); the cap bounds read it in the field, which `planet-frame`'s planner calls. Dependencies point downward only.

## Consequences
- The relief gap closes in the field itself: every consumer (mesh, physics, water, scorecard) reads the same heights (CON-07). The relief classes own `k` (ADR 0089); R4 owns the carve, thermal and deposition.
- Supersedes ADR 0030 point 2 (the coarse stream-power stage); ADR 0089 amends its point 1. Amends:
  - ADR 0016 (cap bounds read the layer's pyramid);
  - ADR 0026 (two channels with block ranges).
  Changes ADR 0045 (the layer, its pyramid and the channels in the storage budget); ADR 0045 is proposed and is edited in place.
- Estimated bake time (from the spikes, not a run): about 6-7.5 h for the planet; pass 1 about 25 min, pass 2 31-70 s per tile (SPIKE-71).
- Disk: the layer is about 7-10 GB (SPIKE-61); the channels and the pyramid are small against it (SPIKE-83).
- Tile seams: a per-tile carve leaves core-edge steps of p99 134-172 m (SPIKE-68); the two passes leave none at four of four sites once the local carve only incises (SPIKE-72). Cube-edge seams of the face carve are R4's measurement.
- Risks:
  - Level steps grow with `k`, 5-7x at the mountain classes (SPIKE-66, SPIKE-82). With bounds from the blocks' ranges alone the steep classes stop refining at level 13-15 (SPIKE-82); with the pyramid of point 8 they plan to full depth with no capped tile (SPIKE-83). The gate checks it at production tau (criterion 6).
  - Over-steep slope tails need thermal (SPIKE-65); low ground near the coast needs the taper.
- History:
  - decision 4 first made the carve per tile with a halo, which failed SPIKE-68 (routing is non-local); it was revised after SPIKE-70 to the two passes (2026-10-31);
  - the 2026-11-01 amendment took `k` from the relief classes (ADR 0089), added the min/max pyramid (SPIKE-82, SPIKE-83) and the crates, and made pass 2 incise only (SPIKE-72, SPIKE-73).
- Reversible: the layer is separate and the channels have versions; turning both off restores today's field.

## Licences of adopted code or techniques
- Implicit stream power after Braun and Willett (2013), the improved priority flood after Barnes et al. (2014) and the thermal pass after Schott et al. (2024): algorithms from the papers, no code adopted.
