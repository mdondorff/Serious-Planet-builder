## Why
Owner decision 2026-10-25: terrain relief goes into M8 before look review 2, in three parts: (1) chunk bed anchors relative to the drawn ground (done, `m8-anet-chunk` design section 17, GEN-057), (2) the coupled run keeps relief, (3) hill-scale relief in the base field; one shared rebuild of bake, run, coarse horizon and chunks at the end. This change covers parts (2) and (3). It is a proposal for the architect's and the owner's review; no implementation starts before the targets are accepted.

Measured evidence (tools of #648, #652; all at the 16384 bake, 611 m spacing; `docs/milestones/M8/look-review-1-hills/slopes.txt`, `docs/learnings.md` 2026-10-25):
- **The base is smooth at hill scale.** The steepest rise over run within 2 km of a land cell, in degrees:

| Ground band (m) | Land share (%) | p50 | p90 | p99 | Rivers p50 | p90 | p99 |
|---|---|---|---|---|---|---|---|
| 0-500 | 60.8 | 0.83 | 1.27 | 1.65 | 0.82 | 1.26 | 1.64 |
| 500-1000 | 28.7 | 1.05 | 1.41 | 1.77 | 1.04 | 1.40 | 1.76 |
| 1000-2500 | 8.6 | 1.13 | 1.56 | 1.98 | 1.11 | 1.53 | 1.97 |
| 2500-5000 | 1.8 | 1.33 | 1.86 | 2.40 | 1.24 | 1.72 | 2.25 |
| over 5000 | 0.0 | 1.30 | 1.77 | 2.43 | 1.72 | 2.15 | 2.15 |

- **The coupled run compresses relief against the base.** Run height minus base height at cell centres, median over river cells (m): +111 at 0 to 500 m ground, -164 at 500 to 1000, -933 at 1000 to 2500, -2686 at 2500 to 5000, -4649 over 5000.
- **Why the run does that.** Its input is the bake with a constant uplift of 1e-3 m/year and erodibility 2e-5 (`coupledworld.rs`), area exponent 0.45, slope exponent 1, 80 steps of 1e5 years. The steady state of that law is `S = U / (K A^m)`: 0.16 % for a river of 10,000 km2, so the run drives every drained surface toward slopes of 0.1 degree whatever the input held.
- **The relief that exists comes from the detail term.** `cpu-erosion-v1` (`generators/erosion.rs`) adds ridged creases (spacing 25 km down to 12 m, 12 octaves, persistence 0.55) with amplitude `80 + 900 smoothstep(-200, 300, E)` times a slope factor that is 0.15 below slope 0.01 and 1 from 0.2: on the table's slopes (p99 0.04) the factor is about 0.2, so wavelengths of 0.5 to 5 km carry little.
- **The consequence in the chunks.** Anchors from the run lay 1.2 to 2.7 km under the base and the incision ran to the 300 m clamp (R-a fixed the anchors). With the anchors on the base the deepest cut is -79 m and -130 m and no cell reaches the clamp, so relief now matters through the base alone.

## What Changes (if accepted)
Part (3), hill-scale relief, wavelengths 0.5 to 8 km, split by wavelength (a departure from ADR 0030 point 1, below):
1. **A hill band in the bake** for wavelengths of 1.5 to 8 km (at least 2.5 bake cells): a ridged-multifractal band added to the bake's elevation at generation, its amplitude set by a landform driver per cell. The coupled run, the coarse horizon and the hydrology then see the hills, so drainage follows them.
2. **Creases in the field for wavelengths under 1.5 km**: the amplitude of `cpu-erosion-v1`'s creases is keyed by the same landform driver instead of the bake's slope alone (the slope factor stays as a multiplier from its floor, which rises from 0.15 with the driver).
3. **The landform driver** `ruggedness` (a derived channel, not stored): the elevation range within 50 km of the bake cell over its elevation above the sea, from the bake's own coarse neighbourhood (the stage before the field; ADR 0022 option 1 is kept). Amplitude of the hill band by landform (first values, calibration inputs): plains (ruggedness under 0.05) 20 to 40 m, hills (0.05 to 0.2) 100 to 250 m, mountains (over 0.2) 400 to 1000 m; the product at wavelength `L` gives slopes of `2 pi A / L`, 0.42 (23 degrees) for 200 m at 3 km.
4. **Bounds:** `max_abs_height_m` and the bake's elevation clamp are raised by the band's largest amplitude; the stamp, chunk and plan bounds read it (GEN-026, GEN-039).

Part (2), the coupled run keeps relief (crusader's lane):
5. **A stationary uplift field.** `uplift(c) = rho K(c) A_in(c)^m S_in(c)`, evaluated on the input: the input's receivers from the priority flood, its areas by accumulation, its slopes to the receivers; `rho` 1 makes the input a steady state, lower values let the run keep eroding. The constant uplift of `coupledworld.rs` is replaced (a departure from the run's declared inputs, below), and `K(c)` stays constant until a lithology driver exists.
6. **The run then evolves only where the input is not graded** (pits, lakes, breaches), and the run minus the base stays small; the uplift field is part of the stored run's key (STRM-043), the first of ADR 0076's driver maps in effect (uplift ahead of the maps, a reordering of ADR 0076 point 6, below).

## Targets (measured by the tools above; for the owner to accept or move)
Targets for the slope table (same definition: steepest rise over run within 2 km, 611 m grid, degrees), proposed from the shape of Earth's land (to be checked against the owner's look reference or a DEM sample before acceptance; these are estimates, not measurements):

| Ground band (m) | Land p50 | p90 | p99 |
|---|---|---|---|
| 0-500 | 1.5 | 6 | 14 |
| 500-1000 | 3 | 11 | 22 |
| 1000-2500 | 5 | 16 | 28 |
| 2500-5000 | 8 | 22 | 33 |
| over 5000 | 10 | 25 | 35 |

Other targets:
- Run minus base at river cells, median within +-100 m and p10 to p90 within +-400 m in every ground band (today -164 to -4649 m medians over 500 m ground).
- Chunk incision at the review sites (`chunk_incision`): no cell at the clamp, the deepest unclamped change at most 150 m, the capped plan nodes at the stills' poses at most 30 (today 126 to 131, mostly vertical walls and the base's own steep ground).
- The look: the owner's review of the two sites and look review 2 judge the shapes; the numbers only guard against a regression.

## Walls at the repose slope (the planner's check)
`crates/frame/tests/wall_repose.rs` (LOD-008): on a synthetic trench along a great circle, 100 m and 300 m deep, the plan from poses like the stills' (300 m, 30 m, 1.8 m over the ground) caps 0 nodes at all three poses for walls of slope 0.5, 0.9 and 2.0, and 23 to 83 nodes (the three poses summed) for walls of slope 4 and for vertical walls (100 m deep: 39 and 23; 300 m deep: 77 and 83). `drawable_slope()` is 3.42 for the stills' parameters. A wall at the angle of repose (0.9) is drawn without capping. The 126 to 131 capped nodes of the stills after R-a are therefore from ground steeper than 3.4 (probably the cut walls at 8 m resolution; not separated).

## Rebuild plan
One shared rebuild after the last of parts (2) and (3), in this order (each stage re-keys the next):
1. The bake at 16384 cells per face edge with the hill band: time to be measured by the global-stage lane (no measurement on file).
2. The coupled run from the new bake and the stationary uplift: the first make from the 16384 bake took 9,916 s before #638's fix of its lattice (the time after it is to be re-measured; a progress line is on the queue).
3. The coarse horizon from the run: time to be measured.
4. Chunks: on demand, 3.4 s per 24 km2 chunk at 8 m cells, one thread (GEN-052); the chunk cache re-keys through the run and base identities.
The machine time of 1 to 3 is the cost to be stated before implementation starts; stage 2 is the one known long step. The slope table (456 s) and the run-against-base report re-measure the targets after the rebuild.

## Lanes
- **Global stage and run calibration (crusader):** items 1 and 3's bake side (the hill band and the driver), 5 and 6, the rebuild.
- **Field chain (this lane):** items 2, 3's field side, 4, the targets' tools, the stills of the two sites.

## Departures from accepted decisions (marked)
- ADR 0030 point 1 (the erosion filter's amplitude follows the bake's slope): the amplitude follows the landform driver as well, and a hill band joins the bake. Needs an amendment or a new ADR.
- ADR 0026 (four global channels): a derived `ruggedness` channel is read from the bake, not stored; no new stored channel. Check against the ADR before building.
- The coupled run's constant uplift (`UPLIFT_M_PER_YEAR`, `declared_inputs_hash`): replaced by a field made from the input. Changes the run's key and the spec of its inputs.
- ADR 0076 point 6 (processes are pre-bake stages, the stream-power routing first): the uplift field lands before the driver maps; no map depends on it yet.
- GEN-057's anchor law stays; it reads the new base through `base_height` with no change.

## Impact
- Specs: generator-pipeline (the hill band, the driver, GEN-026 bounds), world-definition or global-coarse (the bake's version), coupled run (the uplift field), testing (the targets).
- Code: `generators/global.rs` or the bake generator, `generators/erosion.rs`, `generators/coupled.rs`, `app/coupledworld.rs`, the targets' tools (`stills.rs` slope table, `anetworld.rs` reports).
- Cost: the rebuild's machine time (to be measured); about a week for the field chain and the same for the global stage.
- Risk: the hill band changes every height of the planet, so every golden that draws terrain moves (golden-review, ADR 0017: first-of-class looks escalate to the owner's /bless); the bounds rise.
- Out of scope: lithology and erodibility maps, soil and plants from the hills, the chunk stage's calibration of incision and diffusion beyond R-a, the owner's look review 2.
