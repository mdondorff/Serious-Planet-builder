# 0088 The M9 terrain bake
Status: proposed (drafted by the spikes session for opus-planner, 2026-10-31; the owner decides: a data format)
Date: 2026-10-31
Source: SPIKE-61 to SPIKE-66 (`spikes/m9-*/results/results.md`); ADR 0006, ADR 0045, ADR 0055, ADR 0056, ADR 0086, ADR 0087; M9 draft (`docs/briefings/m9-draft.md`) decision 7; CON-07, CON-11, CON-14; GEN-020, GEN-062

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
4. **The erosion layer:** per tile of the 100 m grid (SPIKE-61's tiles, 256-cell halo, core stored), the bake, in order:
   1. samples the base of point 2;
   2. runs a transient carve (implicit stream power with uplift, a fixed `K t`, not to steady state), breaching before the fill and re-routing every 3-5 steps (Barnes flood, order from the receivers, jittered D8, the coupled network's inflow);
   3. runs thermal at the angle of repose with a perturbed critical slope on active cells;
   4. runs one deposition sweep on the last route.
   It stores the change against step 4.1 as the erosion layer (decision 7).
5. **The runtime field adds the layer** bilinearly at 100 m. Below 100 m the field's octaves and the shader detail apply (R6).
6. **Determinism:** every kernel is a Jacobi gather or a fixed topological sweep in f64 with `libm` (CON-14), and the result does not depend on the thread count.
7. **Checkpoints and keys:** the bake checkpoints per tile and resumes from the last complete tile. The layer is keyed by the bake key, the channel versions and the kernel parameters (ADR 0006).

## Consequences
- The relief gap closes in the field itself: every consumer (mesh, physics, water, scorecard) reads the same heights (CON-07). The landform drivers own `k`, and R4 owns the carve, thermal and deposition.
- Estimated bake time (from the spikes, not a run): carve about 10 s per tile and thermal a few seconds, so 2.5-4 h for the planet's 2,140 land tiles at 4 at a time. Criterion 7's 10 h is held with room; a GPU bake is not needed.
- Disk: the layer is 7-10 GB; the two channels are under 1 GB at 4 km.
- Tile seams of the layer are not measured yet: R4's first PR measures the step across a core edge and blends the halo if it is visible.
- Risks:
  - The factor raises level steps in proportion (SPIKE-66), so the LOD error at production tau needs a check (gate, criterion 6).
  - Over-steep slope tails need thermal (SPIKE-65).
  - Low ground near the coast needs the taper.
- Reversible: the layer is separate and the channels have versions; turning both off restores today's field.

## Licences of adopted code or techniques
- Implicit stream power after Braun and Willett (2013), the improved priority flood after Barnes et al. (2014) and the thermal pass after Schott et al. (2024): algorithms from the papers, no code adopted.
