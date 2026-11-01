# 0088 The M9 terrain bake
Status: proposed (drafted by the spikes session for opus-planner, 2026-10-31; the owner decides: a data format); open (opus-planner, 2026-10-31): decision 4 (per-tile carve with a halo) fails SPIKE-68 (core-edge steps p99 134-172 m; routing is non-local); decision 4 revised after SPIKE-70 (two passes: the carve per face at about 800 m, the local work per tile; spikes session, 2026-10-31); amended draft (spikes session for opus-planner, 2026-11-01): `k` from the relief classes (ADR 0089), a min/max pyramid of the layer (SPIKE-82, SPIKE-83), pass 2 incise-only without uplift (SPIKE-72, SPIKE-73)
Date: 2026-10-31
Source: SPIKE-61 to SPIKE-83 (`spikes/m9-*/results/results.md`); ADR 0089; ADR 0006, ADR 0045, ADR 0055, ADR 0056, ADR 0086, ADR 0087; M9 draft (`docs/briefings/m9-draft.md`) decision 7; CON-07, CON-11, CON-14; GEN-020, GEN-062

## Context
- Hills and mountains are 4-9x too gentle in slope and relief (SPIKE-55). The relief comes from the amplitude of the base field below about 20 km: x4-6 for hills, about x7.5 for the Alps to reach 0.8 of real (SPIKE-65, SPIKE-66). Today's landform driver (GEN-062) is the wrong weight; it is low at the hill sites.
- Stream power run to steady state erases the landform (SPIKE-65). A transient carve after the amplitude, with a breach before the fill and thermal at repose after it, reaches 0.62-1.13 of real at the hill sites, with flats at 10-12 % (real 6-9 %) (SPIKE-65, SPIKE-66).
- A relief factor computed per point from a 10 km stencil breaks the generator's contracts (SPIKE-66):
  - batch reads (GEN-020) and plan-check fail;
  - bounds are 25-700x looser;
  - level steps grow by the factor.
- Routing is the bake's main cost. Barnes flood, the receivers' order, jittered D8 and re-routing every 3-5 steps cut it 2.7-5.5x (SPIKE-61, SPIKE-62). Thermal and deposition do nothing on today's relief and cost seconds per tile with an active-cell thermal kernel (SPIKE-64).
- Disk (decision 7, owner): the erosion result is a separate layer beside `global16384` first, about 7-10 GB at 100 m (SPIKE-61).

## Decision
1. **Two new channels in the bake** at the global bake's grid or coarser, each with min and max per block:
   - the relief driver `k`, from the class weights of ADR 0089 (under 1 for flat arid classes, up to about 7.5 for high mountains), tapered to 1 towards the coast band of ADR 0056;
   - the ring mean `m` (the bake's mean over 10 km).
   The class amplitudes (`k`, the crease floor, the carve's `K t`, the deposition's `kc`, the hill band) live in the generator spec.
2. **Runtime base field:** `m + k (e - m)` plus `k` times the creases, with `e` the bake. Each term is one read at the point, so nothing at runtime reads with a 10 km stencil.
3. **Bounds per cap** from the blocks' ranges of `e`, `k` and `m`, the layer's min/max pyramid (point 8) and the crease bounds, nested as LOD-012 requires.
4. **The erosion layer, in two passes** (routing is global, SPIKE-68; SPIKE-70):
   1. **Per cube face at about 800 m:** the base of point 2 sampled once per cell (SPIKE-71), then a transient carve (implicit stream power with uplift, a fixed `K t`, not to steady state), breaching before the fill and re-routing every 3-5 steps (Barnes flood, order from the receivers, jittered D8). Faces do not route across cube edges.
   2. **Per tile of the 100 m grid** (SPIKE-61's tiles, 256-cell halo, core stored):
      - the base of point 2 plus the face carve's change, bilinear;
      - a local carve of catchments up to about 1 km^2 that only incises: no fill, no breach, no uplift (basins are pass 1's and the deposition's; SPIKE-72, SPIKE-73);
      - thermal at the angle of repose with a perturbed critical slope on active cells;
      - one deposition sweep on the last route.
   The layer stores each tile's change against the base of point 2 (decision 7).
5. **The runtime field adds the layer** bilinearly at 100 m. Below 100 m the field's octaves and the shader detail apply (R6).
6. **Determinism:** every kernel is a Jacobi gather or a fixed topological sweep in f64 with `libm` (CON-14), and the result does not depend on the thread count.
7. **Checkpoints and keys:** the bake checkpoints each face carve and each tile, and resumes from the last complete one. The layer is keyed by the bake key, the channel versions and the kernel parameters (ADR 0006).
8. **A min/max pyramid of the layer:** the min and max of each 32 x 32 block of the 100 m layer, and of each coarser level from four blocks below, stored with the layer (u16, rounded outward). A cap's bounds read at most about 3 x 3 blocks; below the layer's cells the runtime detail adds its analytic bound (SPIKE-83). A cap across a face edge reads both faces' blocks.

## Consequences
- The relief gap closes in the field itself: every consumer (mesh, physics, water, scorecard) reads the same heights (CON-07). The landform drivers own `k`, and R4 owns the carve, thermal and deposition.
- Estimated bake time (from the spikes, not a run): about 6-7.5 h for the planet (central 6.9 h), with the 2,140 land tiles 4 at a time (SPIKE-71).
  - Pass 1, the face sampling and the carve, takes about 25 min.
  - Pass 2 takes 31-70 s per tile: sampling, the local carve, and active-cell thermal (exact, 7-30 % of a full Jacobi).
- Disk: the layer is 7-10 GB; the two channels are under 1 GB at 4 km; the min/max pyramid is about 90 MB (SPIKE-83).
- Tile seams:
  - A per-tile carve leaves core-edge steps of p99 134-172 m (SPIKE-68).
  - The two passes leave none at four of four sites once the local carve only incises (SPIKE-72). A filling local carve fills basins that cross a core edge to window-dependent spill points (Highlands: lakes 195 m apart).
  - Cube-edge seams of the face carve are R4's measurement.
- Risks:
  - The factor raises level steps in proportion: 5-7x at the mountain classes (SPIKE-66, SPIKE-82).
    - With bounds from the blocks' ranges alone, the steep classes stop refining at level 13-15 (SPIKE-82).
    - With the pyramid of point 8 they plan to level 18 with no capped tile at 5-8 % more nodes (SPIKE-83).
    - The gate checks it at production tau (criterion 6).
  - Over-steep slope tails need thermal (SPIKE-65).
  - Low ground near the coast needs the taper.
- Reversible: the layer is separate and the channels have versions; turning both off restores today's field.

## Licences of adopted code or techniques
- Implicit stream power after Braun and Willett (2013), the improved priority flood after Barnes et al. (2014) and the thermal pass after Schott et al. (2024): algorithms from the papers, no code adopted.
