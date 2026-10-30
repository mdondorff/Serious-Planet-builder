# 0084 A coarse valley carve in the base
Status: not pursued (opus-planner, 2026-10-30): SPIKE-49 was negative and the chunk stage it served is removed (ADR 0086); M9's bake decides valley carving (SPIKE-60)
Date: 2026-10-27
Source: owner live test 2026-10-27 (`docs/milestones/M8/owner-test-2026-10-27/README.md`, D0, findings 2 and 3); SPIKE-48 (`spikes/m9-uplift/results/results.md`)

## Context
- The chunk stage's change enters at the chunk's apply level and reaches coarser levels only up to the pyramid top; coarser tiles show uneroded ground, so valleys appear and vanish between levels (owner test, findings 2 and 3).
- Coarse levels cannot read chunk solves: the plan and the drawn ground do not depend on which chunks are ready (LOD-013, CON-14).
- ADR 0080 point 4: the coupled run is the input plus the fill, without stream-power erosion.
- SPIKE-48: a carve below the input by a depth law of drainage area (GEN-057's law, at most 100 m) keeps the input's relief (median within 1.4 m in every band) and deepens with area; stream-power variants did not hold.

## Decision
1. **The coupled run carves.** After the fill, the run lowers each cell by a depth law of its drainage area, in one pass with one re-route; the law and its values live in the spec. This amends ADR 0080 point 4 (still no stream-power erosion at M8).
2. **Every level shows the carve.** The base field the plan and the tiles read includes the carved run's valleys at all levels, from stored data, without chunk solves.
3. **The chunk adds detail only.** The chunk stage's change is measured against the carved base, so at its apply level it adds only what the coarse carve cannot resolve; the entry step shrinks to that detail.
4. **Bounds follow.** The stage's stated height bound is set from the measured change against the carved base.

## Consequences
- One rebuild: the coupled run (about 20 s), the chunks (versions bump), the coarse horizon map, the look-review stills; golden candidates go through golden-review.
- Criterion 3's entry step and the held-back tile count are measured again after the rebuild.
- Easy to reverse: a zero depth law is today's run.

## Licences of adopted code or techniques
None.

