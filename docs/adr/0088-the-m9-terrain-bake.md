# 0088 The M9 terrain bake
Status: proposed (drafted by the spikes session for opus-planner, 2026-10-31; the owner decides: a data format); open (opus-planner, 2026-10-31): decision 4 (per-tile carve with a halo) fails SPIKE-68 (core-edge steps p99 134-172 m; routing is non-local); decision 4 revised after SPIKE-70 (two passes: the carve per face at about 800 m, the local work per tile; spikes session, 2026-10-31)
Date: 2026-10-31
Source: SPIKE-61 to SPIKE-70 (`spikes/m9-*/results/results.md`); ADR 0006, ADR 0045, ADR 0055, ADR 0056, ADR 0086, ADR 0087; M9 draft (`docs/briefings/m9-draft.md`) decision 7; CON-07, CON-11, CON-14; GEN-020, GEN-062

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
   - the relief driver `k` (at least 1 per landform, tapered to 1 towards the coast band of ADR 0056);
   - the ring mean `m` (the bake's mean over 10 km).
   The values and curves of `k` live in the generator spec.
2. **Runtime base field:** `m + k (e - m)` plus `k` times the creases, with `e` the bake. Each term is one read at the point, so nothing at runtime reads with a 10 km stencil.
3. **Bounds per cap** from the blocks' ranges of `e`, `k` and `m` and the crease bounds, nested as LOD-012 requires.
4. **The erosion layer, in two passes** (routing is global, SPIKE-68; SPIKE-70):
   1. **Per cube face at about 800 m:** block means of the base of point 2, then a transient carve (implicit stream power with uplift, a fixed `K t`, not to steady state), breaching before the fill and re-routing every 3-5 steps (Barnes flood, order from the receivers, jittered D8). Faces do not route across cube edges.
   2. **Per tile of the 100 m grid** (SPIKE-61's tiles, 256-cell halo, core stored):
      - the base of point 2 plus the face carve's change, bilinear, land held at 1 m or more;
      - a local carve of catchments up to about 1 km^2;
      - thermal at the angle of repose with a perturbed critical slope on active cells;
      - one deposition sweep on the last route.
   The layer stores each tile's change against the base of point 2 (decision 7).
5. **The runtime field adds the layer** bilinearly at 100 m. Below 100 m the field's octaves and the shader detail apply (R6).
6. **Determinism:** every kernel is a Jacobi gather or a fixed topological sweep in f64 with `libm` (CON-14), and the result does not depend on the thread count.
7. **Checkpoints and keys:** the bake checkpoints each face carve and each tile, and resumes from the last complete one. The layer is keyed by the bake key, the channel versions and the kernel parameters (ADR 0006).

## Consequences
- The relief gap closes in the field itself: every consumer (mesh, physics, water, scorecard) reads the same heights (CON-07). The landform drivers own `k`, and R4 owns the carve, thermal and deposition.
- Estimated bake time (from the spikes, not a run): 7.5-10 h for the planet's 2,140 land tiles at 4 at a time, at criterion 7's 10 h limit (SPIKE-70).
  - The tiles are sampled twice: once for the face's block means, once for pass 2.
  - The face carve takes about 3 min and 5.5 GB per face.
  - The local carve takes about 20 s per tile, plus thermal with active cells.
  - Taking the block means from a coarse field level saves about 2 h. A full-Jacobi thermal (about 13 h) does not fit.
- Disk: the layer is 7-10 GB; the two channels are under 1 GB at 4 km.
- Tile seams:
  - A per-tile carve leaves core-edge steps of p99 134-172 m (SPIKE-68).
  - The two passes leave none at three of four sites (SPIKE-70); the fourth needs the 1 m land floor, re-measured in R4.
  - Cube-edge seams of the face carve are R4's measurement.
- Risks:
  - The factor raises level steps in proportion (SPIKE-66), so the LOD error at production tau needs a check (gate, criterion 6).
  - Over-steep slope tails need thermal (SPIKE-65).
  - Low ground near the coast needs the taper.
- Reversible: the layer is separate and the channels have versions; turning both off restores today's field.

## Licences of adopted code or techniques
- Implicit stream power after Braun and Willett (2013), the improved priority flood after Barnes et al. (2014) and the thermal pass after Schott et al. (2024): algorithms from the papers, no code adopted.
